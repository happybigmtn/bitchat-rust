//! Error types and handling for BitChat

use thiserror::Error;

/// Result type alias for BitChat operations
pub type Result<T> = std::result::Result<T, Error>;

/// Alias for backward compatibility
pub type BitCrapsError = Error;

/// BitChat error types
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

    #[error("Security error: {0}")]
    Security(String),

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
}

impl From<std::ffi::NulError> for Error {
    fn from(err: std::ffi::NulError) -> Self {
        Error::Platform(format!("Null byte in C string: {}", err))
    }
}
