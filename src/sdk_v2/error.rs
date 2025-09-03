//! SDK Error Types and Handling
//!
//! Comprehensive error handling system for the BitCraps SDK with detailed error information,
//! recovery suggestions, and developer-friendly messages.

use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// SDK Result type alias
pub type SDKResult<T> = Result<T, SDKError>;

/// Comprehensive SDK error types
#[derive(Debug, Error, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "details")]
pub enum SDKError {
    /// Configuration errors (invalid config, missing required fields)
    #[error("Configuration error: {0}")]
    ConfigurationError(String),
    
    /// Network communication errors
    #[error("Network error: {0}")]
    NetworkError(String),
    
    /// Authentication and authorization errors
    #[error("Authentication error: {0}")]
    AuthenticationError(String),
    
    /// API-specific errors with HTTP status codes
    #[error("API error: {status} - {message}")]
    ApiError {
        status: u16,
        message: String,
        error_code: Option<String>,
        details: Option<serde_json::Value>,
    },
    
    /// Rate limiting errors
    #[error("Rate limit exceeded: {message}")]
    RateLimitError {
        message: String,
        retry_after: Option<u64>,
        limit: Option<u32>,
        remaining: Option<u32>,
    },
    
    /// Validation errors for input data
    #[error("Validation error: {message}")]
    ValidationError {
        message: String,
        field: Option<String>,
        invalid_value: Option<String>,
    },
    
    /// Game-specific errors
    #[error("Game error: {0}")]
    GameError(GameErrorKind),
    
    /// Consensus-related errors
    #[error("Consensus error: {0}")]
    ConsensusError(ConsensusErrorKind),
    
    /// WebSocket communication errors
    #[error("WebSocket error: {0}")]
    WebSocketError(String),
    
    /// Serialization/deserialization errors
    #[error("Serialization error: {0}")]
    SerializationError(String),
    
    /// Timeout errors
    #[error("Timeout error: {operation} timed out after {duration_ms}ms")]
    TimeoutError {
        operation: String,
        duration_ms: u64,
    },
    
    /// Resource not found errors
    #[error("Resource not found: {resource_type} with id '{resource_id}' not found")]
    NotFoundError {
        resource_type: String,
        resource_id: String,
    },
    
    /// Conflict errors (resource already exists, state conflicts)
    #[error("Conflict error: {0}")]
    ConflictError(String),
    
    /// Service unavailable errors
    #[error("Service unavailable: {service} is currently unavailable - {reason}")]
    ServiceUnavailableError {
        service: String,
        reason: String,
    },
    
    /// Internal SDK errors
    #[error("Internal SDK error: {0}")]
    InternalError(String),
    
    /// Feature not supported in current environment
    #[error("Feature not supported: {feature} is not supported in {environment} environment")]
    FeatureNotSupportedError {
        feature: String,
        environment: String,
    },
    
    /// Quota/limit exceeded errors
    #[error("Quota exceeded: {quota_type} limit of {limit} exceeded")]
    QuotaExceededError {
        quota_type: String,
        limit: u64,
        current_usage: Option<u64>,
    },
}

/// Game-specific error kinds
#[derive(Debug, Error, Clone, Serialize, Deserialize)]
pub enum GameErrorKind {
    #[error("Game not found: {game_id}")]
    GameNotFound { game_id: String },
    
    #[error("Game already started: {game_id}")]
    GameAlreadyStarted { game_id: String },
    
    #[error("Game is full: {game_id} has {current_players}/{max_players} players")]
    GameFull { game_id: String, current_players: u32, max_players: u32 },
    
    #[error("Player not in game: {player_id} is not in game {game_id}")]
    PlayerNotInGame { player_id: String, game_id: String },
    
    #[error("Invalid game state: expected {expected}, got {actual}")]
    InvalidGameState { expected: String, actual: String },
    
    #[error("Invalid bet: {reason}")]
    InvalidBet { reason: String, min_bet: Option<u64>, max_bet: Option<u64> },
    
    #[error("Insufficient balance: need {required}, have {available}")]
    InsufficientBalance { required: u64, available: u64 },
    
    #[error("Game rule violation: {rule}")]
    RuleViolation { rule: String },
    
    #[error("Game timeout: {operation} timed out")]
    GameTimeout { operation: String },
}

/// Consensus-specific error kinds
#[derive(Debug, Error, Clone, Serialize, Deserialize)]
pub enum ConsensusErrorKind {
    #[error("Proposal not found: {proposal_id}")]
    ProposalNotFound { proposal_id: String },
    
    #[error("Invalid proposal: {reason}")]
    InvalidProposal { reason: String },
    
    #[error("Consensus timeout: proposal {proposal_id} timed out")]
    ConsensusTimeout { proposal_id: String },
    
    #[error("Insufficient votes: need {required}, got {received}")]
    InsufficientVotes { required: u32, received: u32 },
    
    #[error("Vote rejected: {reason}")]
    VoteRejected { reason: String },
    
    #[error("Byzantine fault detected: {details}")]
    ByzantineFault { details: String },
    
    #[error("Network partition detected")]
    NetworkPartition,
    
    #[error("State synchronization failed: {reason}")]
    StateSyncFailed { reason: String },
}

/// Error context for better debugging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorContext {
    pub request_id: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub sdk_version: String,
    pub environment: Option<String>,
    pub operation: Option<String>,
    pub additional_info: std::collections::HashMap<String, serde_json::Value>,
}

impl Default for ErrorContext {
    fn default() -> Self {
        Self {
            request_id: None,
            timestamp: chrono::Utc::now(),
            sdk_version: crate::sdk_v2::SDK_VERSION.to_string(),
            environment: None,
            operation: None,
            additional_info: std::collections::HashMap::new(),
        }
    }
}

impl SDKError {
    /// Create error with context
    pub fn with_context(mut self, context: ErrorContext) -> ContextualError {
        ContextualError {
            error: self,
            context,
        }
    }
    
    /// Get error code for programmatic handling
    pub fn error_code(&self) -> &'static str {
        match self {
            SDKError::ConfigurationError(_) => "CONFIGURATION_ERROR",
            SDKError::NetworkError(_) => "NETWORK_ERROR",
            SDKError::AuthenticationError(_) => "AUTHENTICATION_ERROR",
            SDKError::ApiError { .. } => "API_ERROR",
            SDKError::RateLimitError { .. } => "RATE_LIMIT_ERROR",
            SDKError::ValidationError { .. } => "VALIDATION_ERROR",
            SDKError::GameError(_) => "GAME_ERROR",
            SDKError::ConsensusError(_) => "CONSENSUS_ERROR",
            SDKError::WebSocketError(_) => "WEBSOCKET_ERROR",
            SDKError::SerializationError(_) => "SERIALIZATION_ERROR",
            SDKError::TimeoutError { .. } => "TIMEOUT_ERROR",
            SDKError::NotFoundError { .. } => "NOT_FOUND_ERROR",
            SDKError::ConflictError(_) => "CONFLICT_ERROR",
            SDKError::ServiceUnavailableError { .. } => "SERVICE_UNAVAILABLE_ERROR",
            SDKError::InternalError(_) => "INTERNAL_ERROR",
            SDKError::FeatureNotSupportedError { .. } => "FEATURE_NOT_SUPPORTED_ERROR",
            SDKError::QuotaExceededError { .. } => "QUOTA_EXCEEDED_ERROR",
        }
    }
    
    /// Check if error is retryable
    pub fn is_retryable(&self) -> bool {
        match self {
            SDKError::NetworkError(_) => true,
            SDKError::ApiError { status, .. } => *status >= 500 || *status == 429,
            SDKError::RateLimitError { .. } => true,
            SDKError::TimeoutError { .. } => true,
            SDKError::ServiceUnavailableError { .. } => true,
            SDKError::ConsensusError(ConsensusErrorKind::ConsensusTimeout { .. }) => true,
            SDKError::ConsensusError(ConsensusErrorKind::NetworkPartition) => true,
            _ => false,
        }
    }
    
    /// Get suggested retry delay
    pub fn retry_delay(&self) -> Option<std::time::Duration> {
        match self {
            SDKError::RateLimitError { retry_after, .. } => {
                retry_after.map(|secs| std::time::Duration::from_secs(secs))
            }
            SDKError::ApiError { status, .. } if *status == 429 => {
                Some(std::time::Duration::from_secs(60))
            }
            SDKError::NetworkError(_) => Some(std::time::Duration::from_secs(5)),
            SDKError::TimeoutError { .. } => Some(std::time::Duration::from_secs(10)),
            SDKError::ServiceUnavailableError { .. } => Some(std::time::Duration::from_secs(30)),
            _ => None,
        }
    }
    
    /// Get user-friendly error message
    pub fn user_message(&self) -> String {
        match self {
            SDKError::NetworkError(_) => "Network connection failed. Please check your internet connection and try again.".to_string(),
            SDKError::AuthenticationError(_) => "Authentication failed. Please check your API key and try again.".to_string(),
            SDKError::RateLimitError { .. } => "Request limit exceeded. Please wait a moment and try again.".to_string(),
            SDKError::GameError(GameErrorKind::GameNotFound { .. }) => "The requested game was not found.".to_string(),
            SDKError::GameError(GameErrorKind::GameFull { .. }) => "The game is currently full. Please try joining another game.".to_string(),
            SDKError::GameError(GameErrorKind::InsufficientBalance { required, available }) => {
                format!("Insufficient balance. You need {} but only have {} available.", required, available)
            }
            SDKError::ServiceUnavailableError { service, .. } => {
                format!("The {} service is currently unavailable. Please try again later.", service)
            }
            _ => self.to_string(),
        }
    }
    
    /// Get recovery suggestions
    pub fn recovery_suggestions(&self) -> Vec<String> {
        match self {
            SDKError::NetworkError(_) => vec![
                "Check your internet connection".to_string(),
                "Verify firewall settings".to_string(),
                "Try again in a few moments".to_string(),
            ],
            SDKError::AuthenticationError(_) => vec![
                "Verify your API key is correct".to_string(),
                "Check if your API key has expired".to_string(),
                "Ensure you have the required permissions".to_string(),
            ],
            SDKError::RateLimitError { retry_after, .. } => {
                let mut suggestions = vec!["Reduce request frequency".to_string()];
                if let Some(delay) = retry_after {
                    suggestions.push(format!("Wait {} seconds before retrying", delay));
                }
                suggestions
            }
            SDKError::GameError(GameErrorKind::GameFull { .. }) => vec![
                "Try joining a different game".to_string(),
                "Create your own game".to_string(),
                "Wait for a player to leave".to_string(),
            ],
            SDKError::ValidationError { field, .. } => {
                let mut suggestions = vec!["Check your input data".to_string()];
                if let Some(field_name) = field {
                    suggestions.push(format!("Verify the '{}' field", field_name));
                }
                suggestions
            }
            _ => vec!["Try the operation again".to_string()],
        }
    }
}

/// Error with additional context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextualError {
    pub error: SDKError,
    pub context: ErrorContext,
}

impl fmt::Display for ContextualError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.error)?;
        if let Some(request_id) = &self.context.request_id {
            write!(f, " (request_id: {})", request_id)?;
        }
        Ok(())
    }
}

impl std::error::Error for ContextualError {}

/// Convert from common error types
impl From<reqwest::Error> for SDKError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            SDKError::TimeoutError {
                operation: "HTTP request".to_string(),
                duration_ms: 30000, // Default timeout
            }
        } else if err.is_connect() {
            SDKError::NetworkError(format!("Connection failed: {}", err))
        } else {
            SDKError::NetworkError(err.to_string())
        }
    }
}

impl From<serde_json::Error> for SDKError {
    fn from(err: serde_json::Error) -> Self {
        SDKError::SerializationError(err.to_string())
    }
}

impl From<url::ParseError> for SDKError {
    fn from(err: url::ParseError) -> Self {
        SDKError::ConfigurationError(format!("Invalid URL: {}", err))
    }
}

impl From<tokio_tungstenite::tungstenite::Error> for SDKError {
    fn from(err: tokio_tungstenite::tungstenite::Error) -> Self {
        SDKError::WebSocketError(err.to_string())
    }
}

/// Helper macro for creating contextual errors
#[macro_export]
macro_rules! sdk_error {
    ($error_type:expr, $context:expr) => {{
        $error_type.with_context($context)
    }};
    
    ($error_type:expr) => {{
        $error_type.with_context(ErrorContext::default())
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_error_codes() {
        let network_error = SDKError::NetworkError("test".to_string());
        assert_eq!(network_error.error_code(), "NETWORK_ERROR");
        
        let auth_error = SDKError::AuthenticationError("test".to_string());
        assert_eq!(auth_error.error_code(), "AUTHENTICATION_ERROR");
    }
    
    #[test]
    fn test_retryable_errors() {
        let network_error = SDKError::NetworkError("test".to_string());
        assert!(network_error.is_retryable());
        
        let config_error = SDKError::ConfigurationError("test".to_string());
        assert!(!config_error.is_retryable());
        
        let rate_limit_error = SDKError::RateLimitError {
            message: "test".to_string(),
            retry_after: Some(60),
            limit: None,
            remaining: None,
        };
        assert!(rate_limit_error.is_retryable());
        assert_eq!(rate_limit_error.retry_delay(), Some(std::time::Duration::from_secs(60)));
    }
    
    #[test]
    fn test_game_errors() {
        let game_error = SDKError::GameError(GameErrorKind::InsufficientBalance {
            required: 100,
            available: 50,
        });
        
        let user_msg = game_error.user_message();
        assert!(user_msg.contains("100"));
        assert!(user_msg.contains("50"));
    }
    
    #[test]
    fn test_error_serialization() {
        let error = SDKError::ApiError {
            status: 404,
            message: "Not found".to_string(),
            error_code: Some("RESOURCE_NOT_FOUND".to_string()),
            details: None,
        };
        
        let serialized = serde_json::to_string(&error).unwrap();
        let deserialized: SDKError = serde_json::from_str(&serialized).unwrap();
        
        match deserialized {
            SDKError::ApiError { status, .. } => assert_eq!(status, 404),
            _ => panic!("Unexpected error type"),
        }
    }
}