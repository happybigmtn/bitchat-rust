// src/protocol/error.rs
use thiserror::Error;
use crate::protocol::types::{GameId, PeerId};

#[derive(Error, Debug)]
pub enum ProtocolError {
    #[error("Invalid packet header: {0}")]
    InvalidHeader(String),
    
    #[error("Packet too small: expected at least {expected}, got {actual}")]
    PacketTooSmall { expected: usize, actual: usize },
    
    #[error("Packet too large: maximum {max}, got {actual}")]
    PacketTooLarge { max: usize, actual: usize },
    
    #[error("Invalid packet version: expected {expected}, got {actual}")]
    InvalidVersion { expected: u8, actual: u8 },
    
    #[error("Invalid packet type: {0}")]
    InvalidPacketType(u8),
    
    #[error("Compression error: {0}")]
    CompressionError(String),
    
    #[error("Decompression error: {0}")]
    DecompressionError(String),
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
    
    #[error("Cryptographic error: {0}")]
    CryptographicError(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    // Gaming-specific errors
    #[error("Invalid game state: {0}")]
    InvalidGameState(String),
    
    #[error("Insufficient tokens: required {required}, available {available}")]
    InsufficientTokens { required: u64, available: u64 },
    
    #[error("Invalid bet: {0}")]
    InvalidBet(String),
    
    #[error("Game not found: {0:?}")]
    GameNotFound(GameId),
    
    #[error("Player not in game: {0:?}")]
    PlayerNotInGame(PeerId),
    
    #[error("Invalid dice roll: die1={die1}, die2={die2}")]
    InvalidDiceRoll { die1: u8, die2: u8 },
    
    #[error("Commitment verification failed")]
    CommitmentVerificationFailed,
    
    #[error("Game phase error: expected {expected}, got {actual}")]
    InvalidGamePhase { expected: String, actual: String },
}

pub type ProtocolResult<T> = Result<T, ProtocolError>;