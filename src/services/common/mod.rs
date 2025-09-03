//! Common Service Components
//!
//! Shared components used across all microservices.

pub mod discovery;
pub mod health;
pub mod metrics;

use crate::error::{Error, Result};
use crate::protocol::PeerId;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Service registration information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceRegistration {
    pub service_name: String,
    pub service_id: String,
    pub address: SocketAddr,
    pub tags: Vec<String>,
    pub metadata: std::collections::HashMap<String, String>,
    pub health_check: Option<HealthCheck>,
    pub ttl: Option<Duration>,
}

/// Health check configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    pub http: Option<HttpHealthCheck>,
    pub tcp: Option<TcpHealthCheck>,
    pub interval: Duration,
    pub timeout: Duration,
    pub deregister_critical_service_after: Option<Duration>,
}

/// HTTP health check configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpHealthCheck {
    pub url: String,
    pub method: String,
    pub headers: std::collections::HashMap<String, String>,
    pub expected_status: u16,
}

/// TCP health check configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpHealthCheck {
    pub address: SocketAddr,
}

/// Service discovery client interface
#[async_trait::async_trait]
pub trait ServiceDiscovery: Send + Sync {
    /// Register a service
    async fn register(&self, registration: ServiceRegistration) -> Result<()>;
    
    /// Deregister a service
    async fn deregister(&self, service_id: &str) -> Result<()>;
    
    /// Discover services by name
    async fn discover(&self, service_name: &str) -> Result<Vec<ServiceInstance>>;
    
    /// Get all services
    async fn list_services(&self) -> Result<Vec<String>>;
    
    /// Health check a service
    async fn health_check(&self, service_id: &str) -> Result<ServiceHealth>;
}

/// Service instance information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInstance {
    pub service_id: String,
    pub service_name: String,
    pub address: SocketAddr,
    pub tags: Vec<String>,
    pub metadata: std::collections::HashMap<String, String>,
    pub health: ServiceHealth,
}

/// Service health status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServiceHealth {
    Passing,
    Warning,
    Critical,
    Unknown,
}

impl ServiceHealth {
    pub fn is_healthy(&self) -> bool {
        matches!(self, ServiceHealth::Passing | ServiceHealth::Warning)
    }
}

/// Circuit breaker pattern implementation
pub struct CircuitBreaker {
    failure_threshold: u32,
    recovery_timeout: Duration,
    success_threshold: u32,
    state: CircuitBreakerState,
    failure_count: u32,
    success_count: u32,
    last_failure_time: Option<SystemTime>,
}

/// Circuit breaker states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitBreakerState {
    Closed,
    Open,
    HalfOpen,
}

impl CircuitBreaker {
    pub fn new(failure_threshold: u32, recovery_timeout: Duration, success_threshold: u32) -> Self {
        Self {
            failure_threshold,
            recovery_timeout,
            success_threshold,
            state: CircuitBreakerState::Closed,
            failure_count: 0,
            success_count: 0,
            last_failure_time: None,
        }
    }
    
    /// Check if operation can be executed
    pub async fn can_execute(&self) -> bool {
        match self.state {
            CircuitBreakerState::Closed => true,
            CircuitBreakerState::Open => {
                if let Some(last_failure) = self.last_failure_time {
                    let elapsed = SystemTime::now().duration_since(last_failure).unwrap_or_default();
                    elapsed >= self.recovery_timeout
                } else {
                    false
                }
            },
            CircuitBreakerState::HalfOpen => true,
        }
    }
    
    /// Record successful operation
    pub async fn record_success(&mut self) {
        match self.state {
            CircuitBreakerState::Closed => {
                self.failure_count = 0;
            },
            CircuitBreakerState::HalfOpen => {
                self.success_count += 1;
                if self.success_count >= self.success_threshold {
                    self.state = CircuitBreakerState::Closed;
                    self.failure_count = 0;
                    self.success_count = 0;
                }
            },
            CircuitBreakerState::Open => {
                // Transition to half-open
                self.state = CircuitBreakerState::HalfOpen;
                self.success_count = 1;
            }
        }
    }
    
    /// Record failed operation
    pub async fn record_failure(&mut self) {
        self.failure_count += 1;
        self.last_failure_time = Some(SystemTime::now());
        
        match self.state {
            CircuitBreakerState::Closed => {
                if self.failure_count >= self.failure_threshold {
                    self.state = CircuitBreakerState::Open;
                }
            },
            CircuitBreakerState::HalfOpen => {
                self.state = CircuitBreakerState::Open;
                self.success_count = 0;
            },
            CircuitBreakerState::Open => {
                // Already open, just update failure time
            }
        }
    }
    
    pub fn get_state(&self) -> CircuitBreakerState {
        self.state
    }
}

/// Retry policy with exponential backoff
pub struct RetryPolicy {
    max_attempts: u32,
    initial_delay: Duration,
    max_delay: Duration,
    multiplier: f64,
    jitter: bool,
}

impl RetryPolicy {
    pub fn new(max_attempts: u32, initial_delay: Duration) -> Self {
        Self {
            max_attempts,
            initial_delay,
            max_delay: Duration::from_secs(60),
            multiplier: 2.0,
            jitter: true,
        }
    }
    
    pub fn with_max_delay(mut self, max_delay: Duration) -> Self {
        self.max_delay = max_delay;
        self
    }
    
    pub fn with_multiplier(mut self, multiplier: f64) -> Self {
        self.multiplier = multiplier;
        self
    }
    
    pub async fn execute<F, T, E>(&self, mut operation: F) -> std::result::Result<T, E>
    where
        F: FnMut() -> std::pin::Pin<Box<dyn std::future::Future<Output = std::result::Result<T, E>> + Send>>,
        E: std::fmt::Debug,
    {
        let mut attempt = 0;
        let mut delay = self.initial_delay;
        
        loop {
            attempt += 1;
            
            match operation().await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    if attempt >= self.max_attempts {
                        return Err(e);
                    }
                    
                    // Calculate delay with exponential backoff
                    let actual_delay = if self.jitter {
                        let jitter_factor = 1.0 + (rand::random::<f64>() - 0.5) * 0.2;
                        Duration::from_millis((delay.as_millis() as f64 * jitter_factor) as u64)
                    } else {
                        delay
                    };
                    
                    tokio::time::sleep(actual_delay).await;
                    
                    // Increase delay for next attempt
                    delay = std::cmp::min(
                        Duration::from_millis((delay.as_millis() as f64 * self.multiplier) as u64),
                        self.max_delay,
                    );
                }
            }
        }
    }
}

/// Request/response correlation
#[derive(Debug, Clone)]
pub struct CorrelationContext {
    pub correlation_id: String,
    pub trace_id: Option<String>,
    pub span_id: Option<String>,
    pub user_id: Option<PeerId>,
    pub session_id: Option<String>,
    pub timestamp: SystemTime,
}

impl CorrelationContext {
    pub fn new() -> Self {
        Self {
            correlation_id: uuid::Uuid::new_v4().to_string(),
            trace_id: None,
            span_id: None,
            user_id: None,
            session_id: None,
            timestamp: SystemTime::now(),
        }
    }
    
    pub fn with_trace(mut self, trace_id: String, span_id: String) -> Self {
        self.trace_id = Some(trace_id);
        self.span_id = Some(span_id);
        self
    }
    
    pub fn with_user(mut self, user_id: PeerId) -> Self {
        self.user_id = Some(user_id);
        self
    }
    
    pub fn elapsed(&self) -> Duration {
        self.timestamp.elapsed().unwrap_or_default()
    }
}

impl Default for CorrelationContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Generic service response wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<ServiceError>,
    pub correlation_id: String,
    pub timestamp: u64,
    pub service: String,
}

impl<T> ServiceResponse<T> {
    pub fn success(data: T, correlation_id: String, service: String) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            correlation_id,
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            service,
        }
    }
    
    pub fn error(error: ServiceError, correlation_id: String, service: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error),
            correlation_id,
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            service,
        }
    }
}

/// Common service error types
#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
pub enum ServiceError {
    #[error("Service unavailable: {message}")]
    ServiceUnavailable { message: String },
    #[error("Invalid request: {message}")]
    InvalidRequest { message: String },
    #[error("Authentication failed: {message}")]
    AuthenticationFailed { message: String },
    #[error("Authorization failed: {message}")]
    AuthorizationFailed { message: String },
    #[error("Resource not found: {resource}")]
    NotFound { resource: String },
    #[error("Request timeout after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },
    #[error("Rate limit exceeded: {limit} requests per {window_seconds}s")]
    RateLimitExceeded { limit: u32, window_seconds: u32 },
    #[error("Internal error: {message}")]
    InternalError { message: String },
}

impl ServiceError {
    pub fn status_code(&self) -> u16 {
        match self {
            ServiceError::ServiceUnavailable { .. } => 503,
            ServiceError::InvalidRequest { .. } => 400,
            ServiceError::AuthenticationFailed { .. } => 401,
            ServiceError::AuthorizationFailed { .. } => 403,
            ServiceError::NotFound { .. } => 404,
            ServiceError::Timeout { .. } => 408,
            ServiceError::RateLimitExceeded { .. } => 429,
            ServiceError::InternalError { .. } => 500,
        }
    }
}