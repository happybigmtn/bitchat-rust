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
    
    #[error("Cryptographic error: {0}")]
    Crypto(String),
    
    #[error("Protocol error: {0}")]
    Protocol(String),
    
    #[error("Network error: {0}")]
    Network(String),
    
    #[error("Invalid data: {0}")]
    InvalidData(String),
    
    #[error("Unknown error: {0}")]
    Unknown(String),
}