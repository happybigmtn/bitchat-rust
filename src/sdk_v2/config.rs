//! SDK Configuration Module
//!
//! Provides flexible configuration options for the BitCraps SDK with builder pattern support.

use crate::sdk_v2::{error::{SDKError, SDKResult}, testing::MockEnvironment};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use url::Url;

/// SDK Environment types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Environment {
    /// Production environment
    Production,
    /// Staging environment for pre-production testing
    Staging,
    /// Development environment
    Development,
    /// Sandbox environment for safe experimentation
    Sandbox,
    /// Testing environment with mocks
    Testing,
    /// Local development with mock services
    Local,
}

impl Environment {
    pub fn default_base_url(&self) -> &'static str {
        match self {
            Environment::Production => "https://api.bitcraps.com/v2",
            Environment::Staging => "https://staging-api.bitcraps.com/v2",
            Environment::Development => "https://dev-api.bitcraps.com/v2",
            Environment::Sandbox => "https://sandbox-api.bitcraps.com/v2",
            Environment::Testing => "http://localhost:3000/v2",
            Environment::Local => "http://localhost:8080/v2",
        }
    }
    
    pub fn default_websocket_url(&self) -> &'static str {
        match self {
            Environment::Production => "wss://ws.bitcraps.com/v2",
            Environment::Staging => "wss://staging-ws.bitcraps.com/v2",
            Environment::Development => "wss://dev-ws.bitcraps.com/v2",
            Environment::Sandbox => "wss://sandbox-ws.bitcraps.com/v2",
            Environment::Testing => "ws://localhost:3001/v2",
            Environment::Local => "ws://localhost:8081/v2",
        }
    }
    
    pub fn is_production(&self) -> bool {
        matches!(self, Environment::Production | Environment::Staging)
    }
    
    pub fn supports_real_money(&self) -> bool {
        matches!(self, Environment::Production)
    }
}

/// SDK Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// API key for authentication
    pub api_key: String,
    
    /// Environment to connect to
    pub environment: Environment,
    
    /// Base URL for REST API
    pub base_url: String,
    
    /// WebSocket URL for real-time communication
    pub websocket_url: String,
    
    /// Request timeout duration
    pub request_timeout: Duration,
    
    /// Connection timeout for WebSocket
    pub websocket_timeout: Duration,
    
    /// Maximum number of retry attempts
    pub max_retries: u32,
    
    /// Retry delay base duration
    pub retry_delay: Duration,
    
    /// Enable request/response logging
    pub debug_logging: bool,
    
    /// User agent string for HTTP requests
    pub user_agent: String,
    
    /// Enable TLS certificate verification
    pub verify_tls: bool,
    
    /// Custom headers for all requests
    pub custom_headers: std::collections::HashMap<String, String>,
    
    /// Rate limiting configuration
    pub rate_limit: RateLimitConfig,
    
    /// Circuit breaker configuration  
    pub circuit_breaker: CircuitBreakerConfig,
    
    /// Testing configuration (only used in test environments)
    #[serde(skip)]
    pub mock_environment: Option<MockEnvironment>,
}

/// Rate limiting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Maximum requests per second
    pub requests_per_second: u32,
    /// Burst capacity
    pub burst_capacity: u32,
    /// Enable rate limiting
    pub enabled: bool,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            requests_per_second: 100,
            burst_capacity: 200,
            enabled: true,
        }
    }
}

/// Circuit breaker configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    /// Failure threshold to open circuit
    pub failure_threshold: u32,
    /// Success threshold to close circuit
    pub success_threshold: u32,
    /// Timeout before retrying when circuit is open
    pub timeout: Duration,
    /// Enable circuit breaker
    pub enabled: bool,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 3,
            timeout: Duration::from_secs(60),
            enabled: true,
        }
    }
}

impl Config {
    /// Create a new configuration builder
    pub fn builder() -> ConfigBuilder {
        ConfigBuilder::new()
    }
    
    /// Validate the configuration
    pub fn validate(&self) -> SDKResult<()> {
        if self.api_key.is_empty() {
            return Err(SDKError::ConfigurationError("API key is required".to_string()));
        }
        
        // Validate URLs
        if let Err(_) = Url::parse(&self.base_url) {
            return Err(SDKError::ConfigurationError("Invalid base URL".to_string()));
        }
        
        if let Err(_) = Url::parse(&self.websocket_url) {
            return Err(SDKError::ConfigurationError("Invalid WebSocket URL".to_string()));
        }
        
        // Validate timeouts
        if self.request_timeout.is_zero() {
            return Err(SDKError::ConfigurationError("Request timeout must be greater than zero".to_string()));
        }
        
        if self.websocket_timeout.is_zero() {
            return Err(SDKError::ConfigurationError("WebSocket timeout must be greater than zero".to_string()));
        }
        
        // Validate rate limiting
        if self.rate_limit.enabled && self.rate_limit.requests_per_second == 0 {
            return Err(SDKError::ConfigurationError("Rate limit requests per second must be greater than zero when enabled".to_string()));
        }
        
        Ok(())
    }
    
    /// Get configuration for specific environment
    pub fn for_environment(environment: Environment, api_key: String) -> Self {
        let base_url = environment.default_base_url().to_string();
        let websocket_url = environment.default_websocket_url().to_string();
        
        Self {
            api_key,
            environment,
            base_url,
            websocket_url,
            request_timeout: Duration::from_secs(30),
            websocket_timeout: Duration::from_secs(10),
            max_retries: 3,
            retry_delay: Duration::from_millis(1000),
            debug_logging: !environment.is_production(),
            user_agent: format!("BitCraps-SDK/2.0.0 ({})", std::env::consts::OS),
            verify_tls: environment.is_production(),
            custom_headers: std::collections::HashMap::new(),
            rate_limit: RateLimitConfig::default(),
            circuit_breaker: CircuitBreakerConfig::default(),
            mock_environment: None,
        }
    }
}

/// Configuration builder with fluent API
#[derive(Debug, Default)]
pub struct ConfigBuilder {
    api_key: Option<String>,
    environment: Option<Environment>,
    base_url: Option<String>,
    websocket_url: Option<String>,
    request_timeout: Option<Duration>,
    websocket_timeout: Option<Duration>,
    max_retries: Option<u32>,
    retry_delay: Option<Duration>,
    debug_logging: Option<bool>,
    user_agent: Option<String>,
    verify_tls: Option<bool>,
    custom_headers: std::collections::HashMap<String, String>,
    rate_limit: Option<RateLimitConfig>,
    circuit_breaker: Option<CircuitBreakerConfig>,
    mock_environment: Option<MockEnvironment>,
}

impl ConfigBuilder {
    /// Create a new configuration builder
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Set API key
    pub fn api_key<S: Into<String>>(mut self, key: S) -> Self {
        self.api_key = Some(key.into());
        self
    }
    
    /// Set environment
    pub fn environment(mut self, env: Environment) -> Self {
        self.environment = Some(env);
        self
    }
    
    /// Set custom base URL
    pub fn base_url<S: Into<String>>(mut self, url: S) -> Self {
        self.base_url = Some(url.into());
        self
    }
    
    /// Set custom WebSocket URL
    pub fn websocket_url<S: Into<String>>(mut self, url: S) -> Self {
        self.websocket_url = Some(url.into());
        self
    }
    
    /// Set request timeout
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = Some(timeout);
        self
    }
    
    /// Set WebSocket timeout
    pub fn websocket_timeout(mut self, timeout: Duration) -> Self {
        self.websocket_timeout = Some(timeout);
        self
    }
    
    /// Set maximum retry attempts
    pub fn max_retries(mut self, retries: u32) -> Self {
        self.max_retries = Some(retries);
        self
    }
    
    /// Set retry delay
    pub fn retry_delay(mut self, delay: Duration) -> Self {
        self.retry_delay = Some(delay);
        self
    }
    
    /// Enable or disable debug logging
    pub fn debug_logging(mut self, enabled: bool) -> Self {
        self.debug_logging = Some(enabled);
        self
    }
    
    /// Set custom user agent
    pub fn user_agent<S: Into<String>>(mut self, agent: S) -> Self {
        self.user_agent = Some(agent.into());
        self
    }
    
    /// Enable or disable TLS verification
    pub fn verify_tls(mut self, verify: bool) -> Self {
        self.verify_tls = Some(verify);
        self
    }
    
    /// Add custom header
    pub fn header<K: Into<String>, V: Into<String>>(mut self, key: K, value: V) -> Self {
        self.custom_headers.insert(key.into(), value.into());
        self
    }
    
    /// Set rate limiting configuration
    pub fn rate_limit(mut self, config: RateLimitConfig) -> Self {
        self.rate_limit = Some(config);
        self
    }
    
    /// Set circuit breaker configuration
    pub fn circuit_breaker(mut self, config: CircuitBreakerConfig) -> Self {
        self.circuit_breaker = Some(config);
        self
    }
    
    /// Set mock environment for testing
    pub fn mock_environment(mut self, mock: Option<MockEnvironment>) -> Self {
        self.mock_environment = mock;
        self
    }
    
    /// Build the configuration
    pub fn build(self) -> SDKResult<Config> {
        let api_key = self.api_key
            .ok_or_else(|| SDKError::ConfigurationError("API key is required".to_string()))?;
            
        let environment = self.environment.unwrap_or(Environment::Development);
        
        let base_url = self.base_url
            .unwrap_or_else(|| environment.default_base_url().to_string());
            
        let websocket_url = self.websocket_url
            .unwrap_or_else(|| environment.default_websocket_url().to_string());
        
        let config = Config {
            api_key,
            environment,
            base_url,
            websocket_url,
            request_timeout: self.request_timeout.unwrap_or(Duration::from_secs(30)),
            websocket_timeout: self.websocket_timeout.unwrap_or(Duration::from_secs(10)),
            max_retries: self.max_retries.unwrap_or(3),
            retry_delay: self.retry_delay.unwrap_or(Duration::from_millis(1000)),
            debug_logging: self.debug_logging.unwrap_or(!environment.is_production()),
            user_agent: self.user_agent.unwrap_or_else(|| 
                format!("BitCraps-SDK/2.0.0 ({})", std::env::consts::OS)
            ),
            verify_tls: self.verify_tls.unwrap_or(environment.is_production()),
            custom_headers: self.custom_headers,
            rate_limit: self.rate_limit.unwrap_or_default(),
            circuit_breaker: self.circuit_breaker.unwrap_or_default(),
            mock_environment: self.mock_environment,
        };
        
        config.validate()?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_config_builder_required_fields() {
        // Should fail without API key
        let result = Config::builder().build();
        assert!(result.is_err());
        
        // Should succeed with API key
        let result = Config::builder()
            .api_key("test-key")
            .build();
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_config_builder_with_environment() {
        let config = Config::builder()
            .api_key("test-key")
            .environment(Environment::Production)
            .build()
            .unwrap();
            
        assert_eq!(config.environment, Environment::Production);
        assert_eq!(config.base_url, "https://api.bitcraps.com/v2");
        assert_eq!(config.verify_tls, true);
        assert_eq!(config.debug_logging, false);
    }
    
    #[test]
    fn test_config_validation() {
        let config = Config::builder()
            .api_key("")
            .build();
        
        assert!(config.is_err());
        assert!(matches!(config, Err(SDKError::ConfigurationError(_))));
    }
    
    #[test]
    fn test_environment_defaults() {
        assert_eq!(Environment::Production.default_base_url(), "https://api.bitcraps.com/v2");
        assert_eq!(Environment::Testing.default_base_url(), "http://localhost:3000/v2");
        
        assert!(Environment::Production.is_production());
        assert!(!Environment::Testing.is_production());
        
        assert!(Environment::Production.supports_real_money());
        assert!(!Environment::Sandbox.supports_real_money());
    }
    
    #[test]
    fn test_config_custom_headers() {
        let config = Config::builder()
            .api_key("test-key")
            .header("X-Custom", "value")
            .header("X-Another", "value2")
            .build()
            .unwrap();
            
        assert_eq!(config.custom_headers.get("X-Custom"), Some(&"value".to_string()));
        assert_eq!(config.custom_headers.get("X-Another"), Some(&"value2".to_string()));
    }
}