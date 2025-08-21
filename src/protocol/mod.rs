//! protocol module

pub mod constants;
pub mod types;
pub mod error;
pub mod binary;
pub mod utils;

// Re-export commonly used types
pub use constants::*;
pub use types::*;
pub use error::{ProtocolError, ProtocolResult};
pub use binary::BinaryProtocol;
pub use utils::PacketUtils;

#[cfg(test)]
pub mod tests;
