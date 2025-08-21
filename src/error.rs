//! Error types for the BitChat protocol
//! 
//! Feynman: Think of errors like warning lights in your car.
//! Each light tells you exactly what went wrong so you can fix it.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Cryptographic operation failed: {0}")]
    Crypto(String),
    
    #[error("Protocol error: {0}")]
    Protocol(String),
    
    #[error("Network error: {0}")]
    Network(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(String),
    
    #[error("Session error: {0}")]
    Session(String),
    
    #[error("Peer not found: {0}")]
    PeerNotFound(String),
    
    #[error("Invalid packet: {0}")]
    InvalidPacket(String),
    
    #[error("Storage error: {0}")]
    Storage(#[from] rusqlite::Error),
    
    #[error("Channel error: {0}")]
    Channel(String),
    
    #[error("Unknown error: {0}")]
    Unknown(String),
}

pub type Result<T> = std::result::Result<T, Error>;