// cargo test --features "transport-rate-limited" --test test_rate_limited_transport

use rmcp::{
    model::{ClientRequest, JsonRpcMessage, JsonRpcRequest, RequestId, RequestNoParam, PingRequestMethod},
    service::{RoleClient, RxJsonRpcMessage, TxJsonRpcMessage},
    transport::{
        rate_limited::{MessageType, RateLimitConfig, RateLimitedTransport, TokenBucketConfig},
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

    fn receive(&mut self) -> impl std::future::Future<Output = Option<RxJsonRpcMessage<RoleClient>>> + Send {
        async { None }
    }

    fn close(&mut self) -> impl std::future::Future<Output = Result<(), Self::Error>> + Send {
        async { Ok(()) }
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

#[tokio::test]
async fn test_message_classification() {
    use rmcp::transport::rate_limited::classify_method;

    // Test progress notification classification
    assert_eq!(
        classify_method("notifications/progress"),
        MessageType::ProgressNotification
    );

    // Test logging message classification  
    assert_eq!(
        classify_method("logging/message"),
        MessageType::LoggingMessage
    );

    // Test sampling request classification
    assert_eq!(
        classify_method("sampling/createMessage"),
        MessageType::SamplingRequest
    );

    // Test tool call classification
    assert_eq!(
        classify_method("tools/call"),
        MessageType::ToolCall
    );

    // Test unknown method classification
    assert_eq!(
        classify_method("unknown/method"),
        MessageType::Other
    );
}

#[tokio::test]
async fn test_token_bucket_refill() {
    use rmcp::transport::rate_limited::TokenBucket;

    let config = TokenBucketConfig::new(10, 5); // 10 tokens per second, burst 5
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