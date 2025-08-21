//! Token economics module for BitChat
//! 
//! Implements cryptographic receipts, ledger management, and reward calculations
//! for the BitChat incentive system.

pub mod receipt;
pub mod ledger;
pub mod rewards;
pub mod consensus;
pub mod settlement;

pub use receipt::{MiningReceipt, ReceiptType};
pub use ledger::{TokenLedger, TokenBalance};
pub use rewards::{RewardCalculator, NetworkAction};
pub use consensus::{ConsensusState, Validator};
pub use settlement::{SettlementBatch, MerkleProof};