//! Error types and handling for BitChat

use thiserror::Error;

/// Result type alias for BitChat operations
pub type Result<T> = std::result::Result<T, Error>;

/// BitChat error types
#[derive(Debug, Error)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(String),
    
    #[error("Bincode error: {0}")]
    Bincode(#[from] bincode::Error),
    
    #[error("Cryptographic error: {0}")]
    Crypto(String),
    
    #[error("Protocol error: {0}")]
    Protocol(String),
    
    #[error("Network error: {0}")]
    Network(String),
    
    #[error("Invalid data: {0}")]
    InvalidData(String),
    
    #[error("Game error: {0}")]
    GameError(String),
    
    #[error("Game not found")]
    GameNotFound,
    
    #[error("Invalid bet: {0}")]
    InvalidBet(String),
    
    #[error("Insufficient balance")]
    InsufficientBalance,
    
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
}