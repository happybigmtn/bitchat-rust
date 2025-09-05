//! API Gateway Service
//!
//! Central gateway that routes requests to appropriate microservices,
//! handles authentication, rate limiting, and load balancing.

pub mod gateway;
pub mod middleware;
pub mod routing;
pub mod load_balancer;
pub mod circuit_breaker;
pub mod geo;

#[cfg(feature = "api-gateway")]
pub use gateway::ApiGateway;

use crate::error::{Error, Result};
use crate::protocol::PeerId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Duration;

/// API Gateway configuration
#[derive(Debug, Clone)]
pub struct GatewayConfig {
    /// Gateway listening address
    pub listen_addr: SocketAddr,
    /// Request timeout
    pub request_timeout: Duration,
    /// Rate limiting configuration
    pub rate_limit: RateLimitConfig,
    /// Authentication configuration
    pub auth: AuthConfig,
    /// Service discovery configuration
    pub service_discovery: ServiceDiscoveryConfig,
    /// Circuit breaker configuration
    pub circuit_breaker: CircuitBreakerConfig,
    /// Broker configuration for fan-out
    pub broker: BrokerConfig,
    /// Load balancing strategy
    pub lb_strategy: LoadBalancingStrategy,
    /// Optional self region code for region-aware routing
    pub region_self: Option<String>,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            listen_addr: "127.0.0.1:8080".parse().unwrap(),
            request_timeout: Duration::from_secs(30),
            rate_limit: RateLimitConfig::default(),
            auth: AuthConfig::default(),
            service_discovery: ServiceDiscoveryConfig::default(),
            circuit_breaker: CircuitBreakerConfig::default(),
            broker: BrokerConfig::default(),
            lb_strategy: LoadBalancingStrategy::WeightedRoundRobin,
            region_self: None,
        }
    }
}

/// Rate limiting configuration
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum requests per window
    pub max_requests: u32,
    /// Time window for rate limiting
    pub window: Duration,
    /// Enable rate limiting
    pub enabled: bool,
    /// Rate limit by IP address
    pub by_ip: bool,
    /// Rate limit by API key
    pub by_api_key: bool,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_requests: 1000,
            window: Duration::from_secs(60),
            enabled: true,
            by_ip: true,
            by_api_key: true,
        }
    }
}

/// Authentication configuration
#[derive(Debug, Clone)]
pub struct AuthConfig {
    /// Enable authentication
    pub enabled: bool,
    /// JWT secret key
    pub jwt_secret: String,
    /// Token expiration time
    pub token_expiration: Duration,
    /// API key authentication
    pub api_keys: HashMap<String, ApiKeyInfo>,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            jwt_secret: std::env::var("JWT_SECRET")
                .unwrap_or_else(|_| {
                    // Generate a random secret if not provided
                    use rand::Rng;
                    use rand::rngs::OsRng;
                    let mut rng = OsRng;
                    let secret: String = (0..32)
                        .map(|_| rng.sample(rand::distributions::Alphanumeric) as char)
                        .collect();
                    log::warn!("JWT_SECRET not set, using randomly generated secret. Set JWT_SECRET environment variable in production!");
                    secret
                }),
            token_expiration: Duration::from_secs(24 * 3600), // 24 hours
            api_keys: HashMap::new(),
        }
    }
}

/// API key information
#[derive(Debug, Clone)]
pub struct ApiKeyInfo {
    pub peer_id: PeerId,
    pub permissions: Vec<String>,
    pub rate_limit_override: Option<u32>,
    pub expires_at: Option<std::time::SystemTime>,
}

/// Service discovery configuration
#[derive(Debug, Clone)]
pub struct ServiceDiscoveryConfig {
    /// Service discovery method
    pub method: ServiceDiscoveryMethod,
    /// Consul configuration
    pub consul: Option<ConsulConfig>,
    /// Static service configuration
    pub static_services: HashMap<String, Vec<ServiceEndpoint>>,
    /// Health check interval
    pub health_check_interval: Duration,
}

impl Default for ServiceDiscoveryConfig {
    fn default() -> Self {
        let mut static_services = HashMap::new();
        static_services.insert("game-engine".to_string(), vec![
            ServiceEndpoint {
                address: "127.0.0.1:8081".parse().unwrap(),
                weight: 100,
                health_check_path: Some("/health".to_string()),
                region: None,
            }
        ]);
        static_services.insert("consensus".to_string(), vec![
            ServiceEndpoint {
                address: "127.0.0.1:8082".parse().unwrap(),
                weight: 100,
                health_check_path: Some("/health".to_string()),
                region: None,
            }
        ]);
        
        Self {
            method: ServiceDiscoveryMethod::Static,
            consul: None,
            static_services,
            health_check_interval: Duration::from_secs(30),
        }
    }
}

/// Service discovery methods
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceDiscoveryMethod {
    Static,
    Consul,
    Kubernetes,
}

/// Consul configuration
#[derive(Debug, Clone)]
pub struct ConsulConfig {
    pub address: String,
    pub datacenter: String,
    pub token: Option<String>,
}

/// Service endpoint information
#[derive(Debug, Clone)]
pub struct ServiceEndpoint {
    pub address: SocketAddr,
    pub weight: u32,
    pub health_check_path: Option<String>,
    /// Optional region code for this endpoint (e.g., "iad")
    pub region: Option<String>,
}

/// Circuit breaker configuration
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Enable circuit breaker
    pub enabled: bool,
    /// Failure threshold to open circuit
    pub failure_threshold: u32,
    /// Recovery timeout
    pub recovery_timeout: Duration,
    /// Success threshold to close circuit
    pub success_threshold: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            failure_threshold: 5,
            recovery_timeout: Duration::from_secs(60),
            success_threshold: 3,
        }
    }
}

/// Broker configuration
#[derive(Debug, Clone)]
pub struct BrokerConfig {
    pub method: BrokerMethod,
    pub url: Option<String>,
}

impl Default for BrokerConfig {
    fn default() -> Self {
        Self { method: BrokerMethod::InMemory, url: None }
    }
}

/// Broker methods
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrokerMethod {
    InMemory,
    #[cfg(feature = "broker-nats")]
    Nats,
    #[cfg(feature = "broker-redis")]
    Redis,
}

/// Service route configuration
#[derive(Debug, Clone)]
pub struct RouteConfig {
    /// Route path pattern
    pub path: String,
    /// Target service name
    pub service: String,
    /// HTTP methods allowed
    pub methods: Vec<String>,
    /// Authentication required
    pub auth_required: bool,
    /// Rate limit override
    pub rate_limit_override: Option<u32>,
    /// Request timeout override
    pub timeout_override: Option<Duration>,
}

/// Load balancing strategies
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadBalancingStrategy {
    RoundRobin,
    WeightedRoundRobin,
    LeastConnections,
    Random,
    IPHash,
}

/// Service health status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    Healthy,
    Unhealthy,
    Unknown,
}

/// Service instance with health information
#[derive(Debug, Clone)]
pub struct ServiceInstance {
    pub endpoint: ServiceEndpoint,
    pub health_status: HealthStatus,
    pub last_health_check: std::time::Instant,
    pub active_connections: u32,
    pub total_requests: u64,
    pub failed_requests: u64,
}

impl ServiceInstance {
    pub fn new(endpoint: ServiceEndpoint) -> Self {
        Self {
            endpoint,
            health_status: HealthStatus::Unknown,
            last_health_check: std::time::Instant::now(),
            active_connections: 0,
            total_requests: 0,
            failed_requests: 0,
        }
    }
    
    pub fn success_rate(&self) -> f64 {
        if self.total_requests == 0 {
            1.0
        } else {
            1.0 - (self.failed_requests as f64 / self.total_requests as f64)
        }
    }
}

/// Gateway metrics
#[derive(Debug, Clone, Default)]
pub struct GatewayMetrics {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub rate_limited_requests: u64,
    pub circuit_breaker_open_count: u64,
    pub average_response_time: f64,
    pub requests_per_second: f64,
}

impl GatewayMetrics {
    pub fn record_request(&mut self, success: bool, response_time: Duration) {
        self.total_requests += 1;
        if success {
            self.successful_requests += 1;
        } else {
            self.failed_requests += 1;
        }
        
        // Update average response time using exponential moving average
        let alpha = 0.1;
        let response_time_ms = response_time.as_millis() as f64;
        self.average_response_time = alpha * response_time_ms + (1.0 - alpha) * self.average_response_time;
    }
    
    pub fn record_rate_limited(&mut self) {
        self.rate_limited_requests += 1;
    }
    
    pub fn record_circuit_breaker_open(&mut self) {
        self.circuit_breaker_open_count += 1;
    }
    
    pub fn success_rate(&self) -> f64 {
        if self.total_requests == 0 {
            1.0
        } else {
            self.successful_requests as f64 / self.total_requests as f64
        }
    }
}

/// Request context information
#[derive(Debug, Clone)]
pub struct RequestContext {
    pub request_id: String,
    pub client_ip: std::net::IpAddr,
    pub user_agent: Option<String>,
    pub api_key: Option<String>,
    pub peer_id: Option<PeerId>,
    pub start_time: std::time::Instant,
}

impl RequestContext {
    pub fn new(client_ip: std::net::IpAddr) -> Self {
        Self {
            request_id: uuid::Uuid::new_v4().to_string(),
            client_ip,
            user_agent: None,
            api_key: None,
            peer_id: None,
            start_time: std::time::Instant::now(),
        }
    }
    
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }
}

/// Gateway response types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub request_id: String,
    pub timestamp: u64,
    pub service: Option<String>,
}

impl<T> GatewayResponse<T> {
    pub fn success(data: T, request_id: String, service: Option<String>) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            request_id,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            service,
        }
    }
    
    pub fn error(error: String, request_id: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error),
            request_id,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            service: None,
        }
    }
}

/// Gateway error types
#[derive(Debug, Clone, thiserror::Error)]
pub enum GatewayError {
    #[error("Service not found: {0}")]
    ServiceNotFound(String),
    #[error("All service instances unhealthy: {0}")]
    NoHealthyInstances(String),
    #[error("Circuit breaker open: {0}")]
    CircuitBreakerOpen(String),
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),
    #[error("Request timeout")]
    RequestTimeout,
    #[error("Internal gateway error: {0}")]
    InternalError(String),
}

impl GatewayError {
    pub fn status_code(&self) -> u16 {
        match self {
            GatewayError::ServiceNotFound(_) => 404,
            GatewayError::NoHealthyInstances(_) => 503,
            GatewayError::CircuitBreakerOpen(_) => 503,
            GatewayError::RateLimitExceeded => 429,
            GatewayError::AuthenticationFailed(_) => 401,
            GatewayError::RequestTimeout => 408,
            GatewayError::InternalError(_) => 500,
        }
    }
}
