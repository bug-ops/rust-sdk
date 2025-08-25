// cargo test --features "transport-rate-limited" --test test_rate_limited_transport

use rmcp::{
    model::{ClientRequest, JsonRpcMessage, JsonRpcRequest, RequestId, RequestNoParam, PingRequestMethod},
    service::{RoleClient, RxJsonRpcMessage, TxJsonRpcMessage},
    transport::{
        rate_limited::{RateLimitConfig, RateLimitedTransport, TokenBucketConfig},
        Transport,
    },
};
use std::sync::Arc;
use tokio::sync::Mutex;

// Mock transport for testing
struct MockTransport {
    send_count: Arc<Mutex<usize>>,
}

impl MockTransport {
    fn new() -> Self {
        Self {
            send_count: Arc::new(Mutex::new(0)),
        }
    }

    #[allow(dead_code)]
    async fn get_send_count(&self) -> usize {
        *self.send_count.lock().await
    }
}

impl Transport<RoleClient> for MockTransport {
    type Error = std::io::Error;

    fn send(
        &mut self,
        _item: TxJsonRpcMessage<RoleClient>,
    ) -> impl std::future::Future<Output = Result<(), Self::Error>> + Send + 'static {
        let count = self.send_count.clone();
        async move {
            let mut c = count.lock().await;
            *c += 1;
            Ok(())
        }
    }

    async fn receive(&mut self) -> Option<RxJsonRpcMessage<RoleClient>> {
        None
    }

    async fn close(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[tokio::test]
async fn test_rate_limiting_basic() {
    let mock = MockTransport::new();
    let config = RateLimitConfig::default();
    let mut transport = RateLimitedTransport::new(mock, config);

    // Create a test message
    let msg = JsonRpcMessage::Request(JsonRpcRequest {
        jsonrpc: rmcp::model::JsonRpcVersion2_0,
        id: RequestId::Number(1),
        request: ClientRequest::PingRequest(RequestNoParam {
            method: PingRequestMethod,
            extensions: Default::default(),
        }),
    });

    // Should succeed initially
    let result = transport.send(msg).await;
    assert!(result.is_ok());
}

// Message classification tests removed since we now use compile-time enum matching
// instead of runtime string-based classification

#[tokio::test]
async fn test_token_bucket_refill() {
    use rmcp::transport::rate_limited::TokenBucket;

    let config = TokenBucketConfig::new(10, 5).unwrap(); // 10 tokens per second, burst 5
    let mut bucket = TokenBucket::new(config);

    // Should start with full capacity
    assert!(bucket.try_consume());
    assert!(bucket.try_consume());
    assert!(bucket.try_consume());
    assert!(bucket.try_consume());
    assert!(bucket.try_consume());

    // Should be empty now
    assert!(!bucket.try_consume());

    // Wait a bit and try again - tokens should refill
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    assert!(bucket.try_consume());
}

#[tokio::test]
async fn test_overflow_protection() {
    use rmcp::transport::rate_limited::TokenBucket;
    use std::time::{Duration, Instant};
    
    let config = TokenBucketConfig::new(10, 5).unwrap(); // 10 tokens per second, burst 5
    let mut bucket = TokenBucket::new(config);
    
    // Simulate extreme time jump (should be protected)
    bucket.set_last_refill(Instant::now() - Duration::from_secs(365 * 24 * 3600)); // 1 year ago
    
    // After refill, tokens should be capped at burst capacity
    bucket.force_refill();
    
    // Should be limited to burst capacity, not overflow
    assert!(bucket.current_tokens() <= 5.0);
    assert!(bucket.current_tokens() >= 0.0);
    
    // Should allow burst capacity number of tokens
    for _ in 0..5 {
        assert!(bucket.try_consume());
    }
    
    // Should be exhausted after burst
    assert!(!bucket.try_consume());
}

#[test]
fn test_config_validation() {
    use rmcp::transport::rate_limited::{TokenBucketConfig, ConfigError};
    
    // Valid configurations should work
    assert!(TokenBucketConfig::new(10, 5).is_ok());
    assert!(TokenBucketConfig::new(1, 1).is_ok());
    assert!(TokenBucketConfig::new(100_000, 10_000).is_ok());
    
    // Invalid rate limits
    assert!(matches!(
        TokenBucketConfig::new(0, 5),
        Err(ConfigError::InvalidRateLimit(0))
    ));
    assert!(matches!(
        TokenBucketConfig::new(100_001, 5),
        Err(ConfigError::InvalidRateLimit(100_001))
    ));
    
    // Invalid burst capacities
    assert!(matches!(
        TokenBucketConfig::new(10, 0),
        Err(ConfigError::InvalidBurstCapacity(0))
    ));
    assert!(matches!(
        TokenBucketConfig::new(10, 10_001),
        Err(ConfigError::InvalidBurstCapacity(10_001))
    ));
    
    // Unreasonable burst configuration
    assert!(matches!(
        TokenBucketConfig::new(10, 601), // 601 > 10 * 60
        Err(ConfigError::UnreasonableBurst { rate: 10, burst: 601 })
    ));
}