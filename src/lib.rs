//! BitChat - A decentralized, peer-to-peer messaging protocol
//! 
//! This library implements the BitChat protocol as specified in the whitepaper,
//! providing secure, anonymous communication over various transport layers.

pub mod error;
pub mod protocol;
pub mod crypto;
pub mod mesh;
pub mod transport;
pub mod session;
pub mod app;
pub mod token;
pub mod incentive;

// Re-export commonly used types
pub use error::{Error, Result};

