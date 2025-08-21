//! Incentive layer for BitChat
//! 
//! Manages service mining, reputation scoring, and quality of service metrics
//! to incentivize network participation.

pub mod mining;
pub mod reputation;
pub mod qos;
pub mod economics;

pub use mining::{MiningService, MiningMetrics};
pub use reputation::{ReputationScore, PeerReputation};
pub use qos::{QualityMetrics, ServiceLevel};
pub use economics::{EconomicConfig, TokenomicsParameters};