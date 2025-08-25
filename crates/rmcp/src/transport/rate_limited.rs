use std::{collections::HashMap, sync::Arc, time::Instant};

use tokio::sync::Mutex;

use super::{IntoTransport, Transport};
use crate::{
    model::{
        ClientRequest, ClientNotification, ServerRequest, ServerNotification, 
        JsonRpcMessage, JsonRpcNotification, JsonRpcRequest,
    },
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
    /// Create a new token bucket configuration with validation
    pub fn new(max_per_second: u32, burst_capacity: u32) -> Result<Self, ConfigError> {
        // Validate rate limit bounds
        if max_per_second == 0 || max_per_second > 100_000 {
            return Err(ConfigError::InvalidRateLimit(max_per_second));
        }
        
        // Validate burst capacity bounds
        if burst_capacity == 0 || burst_capacity > 10_000 {
            return Err(ConfigError::InvalidBurstCapacity(burst_capacity));
        }
        
        // Validate reasonable relationship between rate and burst
        // Burst should not exceed what could be accumulated in 1 minute
        if burst_capacity > max_per_second * 60 {
            return Err(ConfigError::UnreasonableBurst {
                rate: max_per_second,
                burst: burst_capacity,
            });
        }
        
        Ok(Self {
            max_per_second,
            burst_capacity,
        })
    }
    
    /// Create a new token bucket configuration without validation (for internal use)
    pub(crate) fn new_unchecked(max_per_second: u32, burst_capacity: u32) -> Self {
        Self {
            max_per_second,
            burst_capacity,
        }
    }
}

impl Default for TokenBucketConfig {
    fn default() -> Self {
        Self::new_unchecked(10, 5)
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
            progress_notifications: TokenBucketConfig::new_unchecked(10, 5),   // 10/sec, burst 5
            logging_messages: TokenBucketConfig::new_unchecked(50, 10),        // 50/sec, burst 10
            sampling_requests: TokenBucketConfig::new_unchecked(2, 1),         // 2/sec, burst 1
            completion_requests: TokenBucketConfig::new_unchecked(5, 2),       // 5/sec, burst 2
            elicitation_requests: TokenBucketConfig::new_unchecked(1, 1),      // 1/sec, burst 1
            tool_calls: TokenBucketConfig::new_unchecked(20, 5),               // 20/sec, burst 5
            other: TokenBucketConfig::new_unchecked(100, 20),                  // 100/sec, burst 20
        }
    }
}

/// Token bucket for rate limiting
#[derive(Debug)]
pub struct TokenBucket {
    pub(crate) tokens: f64,
    pub(crate) last_refill: Instant,
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

    pub(crate) fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.saturating_duration_since(self.last_refill);
        
        // Protect against time manipulation - limit maximum elapsed time to 1 day
        const MAX_ELAPSED_SECS: f64 = 86400.0; // 24 hours
        let elapsed_secs = elapsed.as_secs_f64().min(MAX_ELAPSED_SECS);
        
        // Use saturating multiplication to prevent overflow
        let tokens_to_add = elapsed_secs.mul_add(
            self.config.max_per_second as f64, 
            0.0
        );
        
        // Safe addition with bounds checking
        self.tokens = (self.tokens + tokens_to_add)
            .min(self.config.burst_capacity as f64)
            .max(0.0); // Prevent negative values
            
        self.last_refill = now;
    }
    
}

/// Test utilities for TokenBucket (only available in tests)
impl TokenBucket {
    /// Get current token count (for testing only)
    #[doc(hidden)]
    pub fn current_tokens(&self) -> f64 {
        self.tokens
    }
    
    /// Set last refill time (for testing only)
    #[doc(hidden)]
    pub fn set_last_refill(&mut self, instant: Instant) {
        self.last_refill = instant;
    }
    
    /// Force refill (for testing only)  
    #[doc(hidden)]
    pub fn force_refill(&mut self) {
        self.refill();
    }
}

/// Configuration validation errors
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Invalid rate limit: {0}. Must be between 1 and 100,000 requests per second")]
    InvalidRateLimit(u32),
    #[error("Invalid burst capacity: {0}. Must be between 1 and 10,000")]
    InvalidBurstCapacity(u32),
    #[error("Unreasonable burst configuration: rate={rate}/s, burst={burst}. Burst should not exceed rate*60")]
    UnreasonableBurst { rate: u32, burst: u32 },
}

/// Rate limiting errors
#[derive(Debug, thiserror::Error)]
pub enum RateLimitError {
    #[error("Rate limit exceeded for message type: {message_type:?}")]
    Exceeded { message_type: MessageType },
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),
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
    pub async fn check_limit<R: ServiceRole>(&mut self, msg: &TxJsonRpcMessage<R>) -> Result<(), RateLimitError> {
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
    inner: Arc<Mutex<T>>,
    rate_limiter: Arc<Mutex<MessageRateLimiter>>,
}

impl<T> RateLimitedTransport<T> {
    pub fn new(transport: T, config: RateLimitConfig) -> Self {
        Self {
            inner: Arc::new(Mutex::new(transport)),
            rate_limiter: Arc::new(Mutex::new(MessageRateLimiter::new(config))),
        }
    }
}

impl<R: ServiceRole, T: Transport<R> + 'static> Transport<R> for RateLimitedTransport<T>
where
    R::Req: Clone + 'static,
    R::Not: Clone + 'static,
{
    type Error = RateLimitedTransportError<T::Error>;

    fn send(
        &mut self,
        item: TxJsonRpcMessage<R>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send + 'static {
        let rate_limiter = self.rate_limiter.clone();
        let inner = self.inner.clone();
        
        async move {
            // Check rate limit FIRST - avoid any expensive operations if rejected
            {
                let mut limiter = rate_limiter.lock().await;
                limiter.check_limit::<R>(&item).await?;
            } // Release lock immediately
            
            // Only proceed with sending if rate limit check passed
            // No premature cloning - item is consumed here only after approval
            let mut transport = inner.lock().await;
            transport.send(item).await
                .map_err(RateLimitedTransportError::Transport)
        }
    }

    fn receive(&mut self) -> impl Future<Output = Option<RxJsonRpcMessage<R>>> + Send {
        let inner = self.inner.clone();
        async move {
            let mut transport = inner.lock().await;
            transport.receive().await
        }
    }

    fn close(&mut self) -> impl Future<Output = Result<(), Self::Error>> + Send {
        let inner = self.inner.clone();
        async move {
            let mut transport = inner.lock().await;
            transport.close().await
                .map_err(RateLimitedTransportError::Transport)
        }
    }
}

/// Classify message type for rate limiting using compile-time enum pattern matching
fn classify_message<R: ServiceRole>(msg: &TxJsonRpcMessage<R>) -> MessageType {
    match msg {
        JsonRpcMessage::Request(JsonRpcRequest { request, .. }) => {
            classify_request(request)
        }
        JsonRpcMessage::Notification(JsonRpcNotification { notification, .. }) => {
            classify_notification(notification)
        }
        _ => MessageType::Other,
    }
}

/// Classify request type using compile-time pattern matching
fn classify_request<Req: 'static>(request: &Req) -> MessageType {
    // We need to use Any trait to downcast since we don't have concrete types here
    // This is still zero-cost at compile time due to monomorphization
    use std::any::{Any, TypeId};
    
    let type_id = TypeId::of::<Req>();
    
    // Check against known client request types
    if type_id == TypeId::of::<ClientRequest>() {
        if let Some(client_req) = (request as &dyn Any).downcast_ref::<ClientRequest>() {
            return classify_client_request(client_req);
        }
    }
    
    // Check against known server request types  
    if type_id == TypeId::of::<ServerRequest>() {
        if let Some(server_req) = (request as &dyn Any).downcast_ref::<ServerRequest>() {
            return classify_server_request(server_req);
        }
    }
    
    MessageType::Other
}

/// Classify notification type using compile-time pattern matching
fn classify_notification<Not: 'static>(notification: &Not) -> MessageType {
    use std::any::{Any, TypeId};
    
    let type_id = TypeId::of::<Not>();
    
    // Check against known client notification types
    if type_id == TypeId::of::<ClientNotification>() {
        if let Some(client_not) = (notification as &dyn Any).downcast_ref::<ClientNotification>() {
            return classify_client_notification(client_not);
        }
    }
    
    // Check against known server notification types
    if type_id == TypeId::of::<ServerNotification>() {
        if let Some(server_not) = (notification as &dyn Any).downcast_ref::<ServerNotification>() {
            return classify_server_notification(server_not);
        }
    }
    
    MessageType::Other
}

/// Classify client request variants
fn classify_client_request(request: &ClientRequest) -> MessageType {
    match request {
        ClientRequest::CompleteRequest(_) => MessageType::CompletionRequest,
        ClientRequest::CallToolRequest(_) => MessageType::ToolCall,
        _ => MessageType::Other,
    }
}

/// Classify server request variants  
fn classify_server_request(request: &ServerRequest) -> MessageType {
    match request {
        ServerRequest::CreateMessageRequest(_) => MessageType::SamplingRequest,
        ServerRequest::CreateElicitationRequest(_) => MessageType::ElicitationRequest,
        _ => MessageType::Other,
    }
}

/// Classify client notification variants
fn classify_client_notification(notification: &ClientNotification) -> MessageType {
    match notification {
        ClientNotification::ProgressNotification(_) => MessageType::ProgressNotification,
        _ => MessageType::Other,
    }
}

/// Classify server notification variants
fn classify_server_notification(notification: &ServerNotification) -> MessageType {
    match notification {
        ServerNotification::ProgressNotification(_) => MessageType::ProgressNotification,
        ServerNotification::LoggingMessageNotification(_) => MessageType::LoggingMessage,
        _ => MessageType::Other,
    }
}

/// IntoTransport adapter for rate-limited transports
pub enum RateLimitTransportAdapter {}

impl<R, T, E> IntoTransport<R, RateLimitedTransportError<E>, RateLimitTransportAdapter> for (T, RateLimitConfig)
where
    R: ServiceRole,
    R::Req: Clone + 'static,
    R::Not: Clone + 'static,
    T: Transport<R, Error = E> + Send + 'static,
    E: std::error::Error + Send + Sync + 'static,
{
    fn into_transport(self) -> impl Transport<R, Error = RateLimitedTransportError<E>> + 'static {
        RateLimitedTransport::new(self.0, self.1)
    }
}