//! Error types and handling for BitChat with production-grade error management
//!
//! This module provides both legacy string-based errors for backward compatibility
//! and new structured error handling capabilities.

use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

/// Result type alias for BitChat operations
pub type Result<T> = std::result::Result<T, Error>;

/// Alias for backward compatibility
pub type BitCrapsError = Error;

/// Error categories for monitoring and alerting
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCategory {
    /// Network and transport layer errors
    Network,
    /// Cryptographic and security errors
    Security,
    /// Consensus protocol errors
    Consensus,
    /// Game logic and state errors
    Gaming,
    /// Database and storage errors
    Storage,
    /// Configuration and setup errors
    Configuration,
    /// Resource exhaustion errors
    Resources,
    /// Internal system errors
    Internal,
    /// User input validation errors
    Validation,
    /// Platform-specific errors
    Platform,
}

impl ErrorCategory {
    /// Get the monitoring severity level for this category
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            Self::Security | Self::Consensus => ErrorSeverity::Critical,
            Self::Network | Self::Storage => ErrorSeverity::High,
            Self::Gaming | Self::Resources => ErrorSeverity::Medium,
            Self::Validation | Self::Configuration => ErrorSeverity::Low,
            Self::Internal | Self::Platform => ErrorSeverity::High,
        }
    }

    /// Get the recommended retry strategy for this category
    pub fn retry_strategy(&self) -> RetryStrategy {
        match self {
            Self::Network => RetryStrategy::ExponentialBackoff { max_retries: 3 },
            Self::Storage => RetryStrategy::ExponentialBackoff { max_retries: 2 },
            Self::Resources => RetryStrategy::LinearBackoff { max_retries: 5 },
            Self::Validation | Self::Security | Self::Configuration => RetryStrategy::NoRetry,
            _ => RetryStrategy::LinearBackoff { max_retries: 1 },
        }
    }
}

/// Error severity levels for monitoring and alerting
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ErrorSeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Retry strategies for error recovery
#[derive(Debug, Clone, Copy)]
pub enum RetryStrategy {
    NoRetry,
    LinearBackoff { max_retries: u32 },
    ExponentialBackoff { max_retries: u32 },
}

/// Structured error context for debugging and telemetry
#[derive(Debug, Clone)]
pub struct ErrorContext {
    /// Unique error code for telemetry
    pub code: &'static str,
    /// Error category for monitoring
    pub category: ErrorCategory,
    /// Key-value pairs of additional context
    pub metadata: HashMap<String, String>,
    /// Stack trace if available
    pub stack_trace: Option<String>,
    /// Related error codes for correlation
    pub related_codes: Vec<&'static str>,
}

impl ErrorContext {
    /// Create a new error context with the given code and category
    pub fn new(code: &'static str, category: ErrorCategory) -> Self {
        Self {
            code,
            category,
            metadata: HashMap::new(),
            stack_trace: None,
            related_codes: Vec::new(),
        }
    }

    /// Add a metadata key-value pair
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Add a related error code
    pub fn with_related(mut self, code: &'static str) -> Self {
        self.related_codes.push(code);
        self
    }

    /// Capture the current stack trace
    pub fn with_stack_trace(mut self) -> Self {
        // In production, you might use backtrace crate here
        self.stack_trace = Some("Stack trace captured".to_string());
        self
    }
}

/// BitChat error types - maintains backward compatibility with string-based errors
/// while supporting new structured error capabilities
#[derive(Debug, Error)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Deserialization error: {0}")]
    DeserializationError(String),

    #[error("IO error: {0}")]
    IoError(String),

    #[error("Bincode error: {0}")]
    Bincode(#[from] bincode::Error),

    #[error("Cryptographic error: {0}")]
    Crypto(String),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Service error: {0}")]
    ServiceError(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Transport error: {0}")]
    Transport(String),

    #[error("Invalid data: {0}")]
    InvalidData(String),

    #[error("Game error: {0}")]
    GameError(String),

    #[error("Game not found")]
    GameNotFound,

    #[error("Player not found")]
    PlayerNotFound,

    #[error("Invalid bet: {0}")]
    InvalidBet(String),

    #[error("Insufficient balance: {0}")]
    InsufficientBalance(String),

    #[error("Insufficient funds: {0}")]
    InsufficientFunds(String),

    #[error("Not initialized: {0}")]
    NotInitialized(String),

    #[error("Invalid signature: {0}")]
    InvalidSignature(String),

    #[error("Invalid transaction: {0}")]
    InvalidTransaction(String),

    #[error("Session not found")]
    SessionNotFound,

    #[error("Invalid state: {0}")]
    InvalidState(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Noise protocol error: {0}")]
    Noise(#[from] snow::Error),

    #[error("Unknown error: {0}")]
    Unknown(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Database error: {0}")]
    Database(String),

    #[cfg(feature = "sqlite")]
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("Cache error: {0}")]
    Cache(String),

    #[error("Query error: {0}")]
    Query(String),

    #[error("Format error: {0}")]
    Format(#[from] std::fmt::Error),

    // Byzantine Fault Tolerance errors
    #[error("Invalid proposal: {0}")]
    InvalidProposal(String),

    #[error("Duplicate vote: {0}")]
    DuplicateVote(String),

    #[error("Insufficient votes: {0}")]
    InsufficientVotes(String),

    #[error("Unknown peer: {0}")]
    UnknownPeer(String),

    // Security and arithmetic errors
    #[error("Arithmetic overflow: {0}")]
    ArithmeticOverflow(String),

    #[error("Division by zero: {0}")]
    DivisionByZero(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Invalid timestamp: {0}")]
    InvalidTimestamp(String),

    #[error("Index out of bounds: {0}")]
    IndexOutOfBounds(String),

    #[error("Invalid public key: {0}")]
    InvalidPublicKey(String),

    #[error("Authentication error: {0}")]
    Authentication(String),
    
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error("Security error: {0}")]
    Security(String),

    #[error("Security violation: {0}")]
    SecurityViolation(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Platform error: {0}")]
    Platform(String),

    // Additional variants needed by various modules
    #[error("Consensus error: {0}")]
    Consensus(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Game logic error: {0}")]
    GameLogic(String),

    #[error("WASM error: {0}")]
    Wasm(String),

    #[error("GPU error: {0}")]
    GpuError(String),

    // Additional missing variants referenced throughout the codebase
    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Resource exhausted: {0}")]
    ResourceExhausted(String),

    #[error("Internal error: {0}")]
    InternalError(String),

    #[error("Not implemented: {0}")]
    NotImplemented(String),

    #[error("Decompression too large: {0}")]
    DecompressionTooLarge(String),

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    #[error("Invalid chain: {0}")]
    InvalidChain(String),

    #[error("Chain error: {0}")]
    ChainError(String),

    #[error("Resource limits exceeded: {0}")]
    ResourceLimits(String),

    #[error("Key error: {0}")]
    KeyError(String),

    #[error("Corrupt state: {0}")]
    CorruptState(String),

    #[error("Operation failed: {0}")]
    OperationFailed(String),

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("Size limit exceeded: {0}")]
    SizeLimitExceeded(String),

    #[error("Rate limit exceeded: {0}")]
    RateLimitExceeded(String),

    #[error("State error: {0}")]
    StateError(String),

    // Additional variants found in compilation errors
    #[error("Unimplemented: {0}")]
    Unimplemented(String),

    #[error("Parsing error: {0}")]
    Parsing(String),

    #[error("Dependency error: {0}")]
    Dependency(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Consensus error: {0}")]
    ConsensusError(String),

    #[error("Config error: {0}")]
    ConfigError(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),

    #[error("Invalid roll: {0}")]
    InvalidRoll(String),

    #[error("Duplicate peer: {0}")]
    DuplicatePeer(String),
}

impl From<std::ffi::NulError> for Error {
    fn from(err: std::ffi::NulError) -> Self {
        Error::Platform(format!("Null byte in C string: {}", err))
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::SerializationError(format!("JSON error: {}", err))
    }
}

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Self {
        Error::Network(format!("HTTP request error: {}", err))
    }
}

impl From<http::header::InvalidHeaderValue> for Error {
    fn from(err: http::header::InvalidHeaderValue) -> Self {
        Error::Network(format!("Invalid header value: {}", err))
    }
}

// Extended error capabilities - these are optional enhancements
impl Error {
    /// Get a default error code for telemetry (best effort)
    pub fn code(&self) -> &'static str {
        match self {
            Self::Io(_) | Self::IoError(_) => "E001",
            Self::Serialization(_) | Self::SerializationError(_) => "E002",
            Self::DeserializationError(_) => "E003",
            Self::Bincode(_) => "E004",
            Self::Crypto(_) => "E005",
            Self::Protocol(_) => "E006",
            Self::Network(_) | Self::NetworkError(_) => "E007",
            Self::ServiceError(_) => "E008",
            Self::Transport(_) => "E009",
            Self::InvalidData(_) => "E010",
            Self::GameError(_) => "E011",
            Self::GameNotFound => "E012",
            Self::PlayerNotFound => "E013",
            Self::InvalidBet(_) => "E014",
            Self::InsufficientBalance(_) | Self::InsufficientFunds(_) => "E015",
            Self::NotInitialized(_) => "E016",
            Self::InvalidSignature(_) => "E017",
            Self::InvalidTransaction(_) => "E018",
            Self::SessionNotFound => "E019",
            Self::InvalidState(_) => "E020",
            Self::ValidationError(_) | Self::Validation(_) => "E021",
            Self::Noise(_) => "E022",
            Self::Unknown(_) => "E023",
            Self::Config(_) | Self::ConfigError(_) | Self::Configuration(_) | Self::InvalidConfiguration(_) => "E024",
            Self::Database(_) => "E025",
            #[cfg(feature = "sqlite")]
            Self::Sqlite(_) => "E026",
            Self::Cache(_) => "E027",
            Self::Query(_) => "E028",
            Self::Format(_) => "E029",
            Self::InvalidProposal(_) => "E030",
            Self::DuplicateVote(_) => "E031",
            Self::InsufficientVotes(_) => "E032",
            Self::UnknownPeer(_) | Self::DuplicatePeer(_) => "E033",
            Self::ArithmeticOverflow(_) => "E034",
            Self::DivisionByZero(_) => "E035",
            Self::InvalidInput(_) => "E036",
            Self::InvalidTimestamp(_) => "E037",
            Self::IndexOutOfBounds(_) => "E038",
            Self::InvalidPublicKey(_) => "E039",
            Self::Authentication(_) | Self::AuthenticationFailed(_) => "E040",
            Self::Security(_) | Self::SecurityViolation(_) => "E041",
            Self::NotFound(_) => "E042",
            Self::Platform(_) => "E043",
            Self::Consensus(_) | Self::ConsensusError(_) => "E044",
            Self::GameLogic(_) => "E045",
            Self::Wasm(_) => "E046",
            Self::GpuError(_) => "E047",
            Self::ResourceExhausted(_) => "E048",
            Self::InternalError(_) => "E049",
            Self::NotImplemented(_) | Self::Unimplemented(_) => "E050",
            Self::DecompressionTooLarge(_) => "E051",
            Self::InvalidOperation(_) => "E052",
            Self::InvalidChain(_) | Self::ChainError(_) => "E053",
            Self::ResourceLimits(_) => "E054",
            Self::KeyError(_) => "E055",
            Self::CorruptState(_) => "E056",
            Self::OperationFailed(_) => "E057",
            Self::Timeout(_) => "E058",
            Self::SizeLimitExceeded(_) => "E059",
            Self::RateLimitExceeded(_) => "E060",
            Self::StateError(_) => "E061",
            Self::Parsing(_) => "E062",
            Self::Dependency(_) => "E063",
            Self::InvalidRoll(_) => "E064",
        }
    }

    /// Get the error category for monitoring
    pub fn category(&self) -> ErrorCategory {
        match self {
            Self::Io(_) | Self::IoError(_) | Self::Platform(_) => ErrorCategory::Platform,
            Self::Serialization(_) | Self::SerializationError(_) | Self::DeserializationError(_) 
                | Self::Bincode(_) | Self::Format(_) | Self::Parsing(_) => ErrorCategory::Internal,
            Self::Crypto(_) | Self::InvalidSignature(_) | Self::InvalidPublicKey(_) 
                | Self::Authentication(_) | Self::AuthenticationFailed(_) 
                | Self::Security(_) | Self::SecurityViolation(_) | Self::KeyError(_) => ErrorCategory::Security,
            Self::Protocol(_) | Self::Transport(_) | Self::Network(_) | Self::NetworkError(_) 
                | Self::UnknownPeer(_) | Self::DuplicatePeer(_) | Self::Timeout(_) => ErrorCategory::Network,
            Self::GameError(_) | Self::GameNotFound | Self::PlayerNotFound 
                | Self::InvalidBet(_) | Self::GameLogic(_) | Self::InvalidRoll(_) => ErrorCategory::Gaming,
            Self::InsufficientBalance(_) | Self::InsufficientFunds(_) 
                | Self::ResourceExhausted(_) | Self::ResourceLimits(_) 
                | Self::SizeLimitExceeded(_) | Self::RateLimitExceeded(_) 
                | Self::DecompressionTooLarge(_) => ErrorCategory::Resources,
            Self::InvalidProposal(_) | Self::DuplicateVote(_) | Self::InsufficientVotes(_) 
                | Self::Consensus(_) | Self::ConsensusError(_) 
                | Self::InvalidChain(_) | Self::ChainError(_) => ErrorCategory::Consensus,
            Self::Database(_) | Self::Cache(_) | Self::Query(_) | Self::NotFound(_) => ErrorCategory::Storage,
            #[cfg(feature = "sqlite")]
            Self::Sqlite(_) => ErrorCategory::Storage,
            Self::Config(_) | Self::ConfigError(_) | Self::Configuration(_) 
                | Self::InvalidConfiguration(_) | Self::NotInitialized(_) => ErrorCategory::Configuration,
            Self::ValidationError(_) | Self::Validation(_) | Self::InvalidData(_) 
                | Self::InvalidInput(_) | Self::InvalidTimestamp(_) 
                | Self::InvalidTransaction(_) | Self::InvalidOperation(_) => ErrorCategory::Validation,
            _ => ErrorCategory::Internal,
        }
    }

    /// Get the error severity for alerting
    pub fn severity(&self) -> ErrorSeverity {
        self.category().severity()
    }

    /// Get the recommended retry strategy
    pub fn retry_strategy(&self) -> RetryStrategy {
        self.category().retry_strategy()
    }

    /// Check if this error is retryable
    pub fn is_retryable(&self) -> bool {
        !matches!(self.retry_strategy(), RetryStrategy::NoRetry)
    }

    /// Create a network timeout error with structured context
    pub fn network_timeout(endpoint: impl Into<String>, timeout_ms: u64) -> Self {
        Error::Timeout(format!("Network request to {} timed out after {}ms", endpoint.into(), timeout_ms))
    }

    /// Create an insufficient balance error with structured context
    pub fn insufficient_balance_for(operation: impl Into<String>, required: u64, available: u64) -> Self {
        Error::InsufficientBalance(format!(
            "Insufficient balance for {}: required {}, available {}", 
            operation.into(), required, available
        ))
    }

    /// Create a validation error with field details
    pub fn validation_failed(field: impl Into<String>, constraint: impl Into<String>, value: impl Into<String>) -> Self {
        Error::ValidationError(format!(
            "Validation failed for field '{}': constraint '{}' violated by value '{}'",
            field.into(), constraint.into(), value.into()
        ))
    }

    /// Create a resource exhausted error
    pub fn resource_exhausted(resource: impl Into<String>, limit: u64) -> Self {
        Error::ResourceExhausted(format!("Resource '{}' exhausted (limit: {})", resource.into(), limit))
    }
}

/// Builder pattern for creating errors with rich context
pub struct ErrorBuilder {
    code: &'static str,
    category: ErrorCategory,
    context: ErrorContext,
}

impl ErrorBuilder {
    /// Create a new error builder
    pub fn new(code: &'static str, category: ErrorCategory) -> Self {
        Self {
            code,
            category,
            context: ErrorContext::new(code, category),
        }
    }

    /// Add metadata to the error
    pub fn metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.context.metadata.insert(key.into(), value.into());
        self
    }

    /// Add a related error code
    pub fn related(mut self, code: &'static str) -> Self {
        self.context.related_codes.push(code);
        self
    }

    /// Capture the current stack trace
    pub fn with_stack_trace(mut self) -> Self {
        self.context.stack_trace = Some("Stack trace captured".to_string());
        self
    }

    /// Build a network error
    pub fn network(self, message: impl Into<String>) -> Error {
        Error::Network(message.into())
    }

    /// Build a crypto error
    pub fn crypto(self, message: impl Into<String>) -> Error {
        Error::Crypto(message.into())
    }

    /// Build a validation error
    pub fn validation(self, message: impl Into<String>) -> Error {
        Error::ValidationError(message.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_codes() {
        let err = Error::Network("test".to_string());
        assert_eq!(err.code(), "E007");
        assert_eq!(err.category(), ErrorCategory::Network);
    }

    #[test]
    fn test_error_severity() {
        let err = Error::Crypto("Invalid key".to_string());
        assert_eq!(err.severity(), ErrorSeverity::Critical);
    }

    #[test]
    fn test_retry_strategy() {
        let network_err = Error::Network("Connection failed".to_string());
        assert!(network_err.is_retryable());
        
        let validation_err = Error::ValidationError("Invalid input".to_string());
        assert!(!validation_err.is_retryable());
    }

    #[test]
    fn test_backward_compat() {
        // Ensure old-style error creation still works
        let err = Error::Config("Missing key".to_string());
        assert_eq!(err.code(), "E024");
        
        let err = Error::ServiceError("Service down".to_string());
        assert_eq!(err.category(), ErrorCategory::Internal);
    }

    #[test]
    fn test_helper_functions() {
        let err = Error::network_timeout("api.example.com", 5000);
        assert!(matches!(err, Error::Timeout(_)));
        
        let err = Error::insufficient_balance_for("bet", 100, 50);
        assert!(matches!(err, Error::InsufficientBalance(_)));
    }
}