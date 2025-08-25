use std::{collections::HashMap, sync::Arc, time::Instant};

use tokio::sync::Mutex;

use super::{IntoTransport, Transport};
use crate::{
    model::{JsonRpcMessage, JsonRpcNotification, JsonRpcRequest},
    service::{RxJsonRpcMessage, ServiceRole, TxJsonRpcMessage},
};

/// Message types for rate limiting classification
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum MessageType {
    ProgressNotification,
    LoggingMessage,
    SamplingRequest,
    CompletionRequest,
    ElicitationRequest,
    ToolCall,
    Other,
}

/// Token bucket configuration for rate limiting
#[derive(Debug, Clone)]
pub struct TokenBucketConfig {
    /// Maximum tokens per second
    pub max_per_second: u32,
    /// Maximum burst capacity
    pub burst_capacity: u32,
}

impl TokenBucketConfig {
    pub fn new(max_per_second: u32, burst_capacity: u32) -> Self {
        Self {
            max_per_second,
            burst_capacity,
        }
    }
}

impl Default for TokenBucketConfig {
    fn default() -> Self {
        Self::new(10, 5)
    }
}

/// Rate limiting configuration for different message types
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    pub progress_notifications: TokenBucketConfig,
    pub logging_messages: TokenBucketConfig,
    pub sampling_requests: TokenBucketConfig,
    pub completion_requests: TokenBucketConfig,
    pub elicitation_requests: TokenBucketConfig,
    pub tool_calls: TokenBucketConfig,
    pub other: TokenBucketConfig,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            progress_notifications: TokenBucketConfig::new(10, 5),   // 10/sec, burst 5
            logging_messages: TokenBucketConfig::new(50, 10),        // 50/sec, burst 10
            sampling_requests: TokenBucketConfig::new(2, 1),         // 2/sec, burst 1
            completion_requests: TokenBucketConfig::new(5, 2),       // 5/sec, burst 2
            elicitation_requests: TokenBucketConfig::new(1, 1),      // 1/sec, burst 1
            tool_calls: TokenBucketConfig::new(20, 5),               // 20/sec, burst 5
            other: TokenBucketConfig::new(100, 20),                  // 100/sec, burst 20
        }
    }
}

/// Token bucket for rate limiting
#[derive(Debug)]
pub struct TokenBucket {
    tokens: f64,
    last_refill: Instant,
    config: TokenBucketConfig,
}

impl TokenBucket {
    pub fn new(config: TokenBucketConfig) -> Self {
        Self {
            tokens: config.burst_capacity as f64,
            last_refill: Instant::now(),
            config,
        }
    }

    /// Try to consume a token, returns true if successful
    pub fn try_consume(&mut self) -> bool {
        self.refill();
        
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill);
        let tokens_to_add = elapsed.as_secs_f64() * self.config.max_per_second as f64;
        
        self.tokens = (self.tokens + tokens_to_add).min(self.config.burst_capacity as f64);
        self.last_refill = now;
    }
}

/// Rate limiting errors
#[derive(Debug, thiserror::Error)]
pub enum RateLimitError {
    #[error("Rate limit exceeded for message type: {message_type:?}")]
    Exceeded { message_type: MessageType },
}

/// Transport error that includes both rate limiting and inner transport errors
#[derive(Debug, thiserror::Error)]
pub enum RateLimitedTransportError<E> {
    #[error("Rate limiting error: {0}")]
    RateLimit(#[from] RateLimitError),
    #[error("Transport error: {0}")]
    Transport(E),
}

/// Rate limiter for MCP messages
#[derive(Debug)]
pub struct MessageRateLimiter {
    buckets: HashMap<MessageType, TokenBucket>,
}

impl MessageRateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        let mut buckets = HashMap::new();
        buckets.insert(MessageType::ProgressNotification, TokenBucket::new(config.progress_notifications));
        buckets.insert(MessageType::LoggingMessage, TokenBucket::new(config.logging_messages));
        buckets.insert(MessageType::SamplingRequest, TokenBucket::new(config.sampling_requests));
        buckets.insert(MessageType::CompletionRequest, TokenBucket::new(config.completion_requests));
        buckets.insert(MessageType::ElicitationRequest, TokenBucket::new(config.elicitation_requests));
        buckets.insert(MessageType::ToolCall, TokenBucket::new(config.tool_calls));
        buckets.insert(MessageType::Other, TokenBucket::new(config.other));
        
        Self { buckets }
    }

    /// Check rate limit for a message
    pub async fn check_limit<R: ServiceRole>(&mut self, msg: &TxJsonRpcMessage<R>) -> Result<(), RateLimitError>
    where
        R::Req: serde::Serialize,
        R::Not: serde::Serialize,
    {
        let msg_type = classify_message::<R>(msg);
        
        if let Some(bucket) = self.buckets.get_mut(&msg_type) {
            if bucket.try_consume() {
                Ok(())
            } else {
                tracing::warn!("Rate limit exceeded for {:?}", msg_type);
                Err(RateLimitError::Exceeded { message_type: msg_type })
            }
        } else {
            // If no bucket configured, allow the message
            Ok(())
        }
    }
}

/// Rate-limited transport wrapper
pub struct RateLimitedTransport<T> {
    inner: T,
    rate_limiter: Arc<Mutex<MessageRateLimiter>>,
}

impl<T> RateLimitedTransport<T> {
    pub fn new(transport: T, config: RateLimitConfig) -> Self {
        Self {
            inner: transport,
            rate_limiter: Arc::new(Mutex::new(MessageRateLimiter::new(config))),
        }
    }
}

impl<R: ServiceRole, T: Transport<R>> Transport<R> for RateLimitedTransport<T>
where
    R::Req: serde::Serialize + Clone,
    R::Not: serde::Serialize + Clone,
{
    type Error = RateLimitedTransportError<T::Error>;

    fn send(
        &mut self,
        item: TxJsonRpcMessage<R>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send + 'static {
        let rate_limiter = self.rate_limiter.clone();
        
        // We need to create a future from the inner transport send
        let inner_future = self.inner.send(item.clone());
        
        async move {
            // Check rate limit first
            rate_limiter.lock().await.check_limit::<R>(&item).await?;
            
            // Forward to inner transport
            inner_future.await
                .map_err(RateLimitedTransportError::Transport)
        }
    }

    fn receive(&mut self) -> impl Future<Output = Option<RxJsonRpcMessage<R>>> + Send {
        // Rate limiting only applies to outgoing messages
        self.inner.receive()
    }

    fn close(&mut self) -> impl Future<Output = Result<(), Self::Error>> + Send {
        async move {
            self.inner.close().await
                .map_err(RateLimitedTransportError::Transport)
        }
    }
}

/// Classify message type for rate limiting
fn classify_message<R: ServiceRole>(msg: &TxJsonRpcMessage<R>) -> MessageType 
where
    R::Req: serde::Serialize,
    R::Not: serde::Serialize,
{
    match msg {
        JsonRpcMessage::Request(JsonRpcRequest { request, .. }) => {
            classify_method(&get_method(request))
        }
        JsonRpcMessage::Notification(JsonRpcNotification { notification, .. }) => {
            classify_method(&get_method(notification))
        }
        _ => MessageType::Other,
    }
}

/// Extract method name from request/notification by serializing and parsing
fn get_method<T: serde::Serialize>(obj: &T) -> String {
    // Serialize to JSON and extract method field
    if let Ok(json) = serde_json::to_value(obj) {
        if let Some(method) = json.get("method").and_then(|v| v.as_str()) {
            return method.to_string();
        }
    }
    "unknown".to_string()
}

/// Classify method name into message type
pub fn classify_method(method: &str) -> MessageType {
    match method {
        "notifications/progress" => MessageType::ProgressNotification,
        "logging/message" => MessageType::LoggingMessage,
        "sampling/createMessage" => MessageType::SamplingRequest,
        "completion/complete" => MessageType::CompletionRequest,
        method if method.starts_with("elicitation/") => MessageType::ElicitationRequest,
        method if method.contains("/call") => MessageType::ToolCall,
        _ => MessageType::Other,
    }
}

/// IntoTransport adapter for rate-limited transports
pub enum RateLimitTransportAdapter {}

impl<R, T, E> IntoTransport<R, RateLimitedTransportError<E>, RateLimitTransportAdapter> for (T, RateLimitConfig)
where
    R: ServiceRole,
    T: Transport<R, Error = E> + Send + 'static,
    E: std::error::Error + Send + Sync + 'static,
{
    fn into_transport(self) -> impl Transport<R, Error = RateLimitedTransportError<E>> + 'static {
        RateLimitedTransport::new(self.0, self.1)
    }
}