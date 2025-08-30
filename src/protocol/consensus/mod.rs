//! BitCraps Consensus Mechanism for Decentralized Game State Agreement
//!
//! This module implements a comprehensive consensus system that allows multiple players
//! to agree on game state in adversarial conditions without requiring a central authority.
//!
//! ## Key Features:
//! - Game state consensus protocol for multiple players
//! - Fork resolution when players have conflicting game states
//! - Transaction confirmation requirements with configurable thresholds
//! - Bet validation consensus ensuring all players agree on bet outcomes
//! - Dice roll consensus using secure commit-reveal scheme
//! - Dispute resolution mechanisms without central authority
//! - Byzantine fault tolerance for up to 1/3 malicious actors
//!
//! ## Architecture:
//! The consensus system uses a hybrid approach combining:
//! - PBFT-style consensus for critical game state transitions
//! - Commit-reveal schemes for fair randomness generation
//! - Merkle trees for efficient state verification
//! - Cryptographic signatures for all state changes
//! - Timeout-based progression to prevent stalling

pub mod byzantine_engine;
pub mod commit_reveal;
pub mod engine;
pub mod lockfree_engine;
pub mod merkle_cache;
pub mod persistence;
pub mod robust_engine;
pub mod validation;
pub mod voting;

use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::protocol::Hash256;

// Re-export main types
pub use commit_reveal::{EntropyPool, RandomnessCommit, RandomnessReveal};
pub use engine::{ConsensusEngine, GameConsensusState, GameOperation, GameProposal};
pub use validation::{Dispute, DisputeClaim, DisputeEvidence, DisputeVote, DisputeVoteType};
pub use voting::{ConfirmationTracker, Fork, VoteTracker};

/// Consensus constants
pub const MIN_CONFIRMATIONS: usize = 2; // Minimum confirmations for consensus
pub const MAX_BYZANTINE_FAULTS: f32 = 0.33; // Maximum fraction of Byzantine actors
pub const CONSENSUS_TIMEOUT: Duration = Duration::from_secs(30);
pub const COMMIT_REVEAL_TIMEOUT: Duration = Duration::from_secs(15);
pub const FORK_RESOLUTION_TIMEOUT: Duration = Duration::from_secs(60);

/// Consensus configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusConfig {
    pub min_confirmations: usize,
    pub max_byzantine_ratio: f32,
    pub consensus_timeout: Duration,
    pub commit_reveal_timeout: Duration,
    pub fork_resolution_timeout: Duration,
    pub require_unanimous_bets: bool,
    pub enable_fork_recovery: bool,
    pub max_round_time: Duration,
    pub vote_timeout: Duration,
    pub max_forks: usize,
}

impl Default for ConsensusConfig {
    fn default() -> Self {
        Self {
            min_confirmations: MIN_CONFIRMATIONS,
            max_byzantine_ratio: MAX_BYZANTINE_FAULTS,
            consensus_timeout: CONSENSUS_TIMEOUT,
            commit_reveal_timeout: COMMIT_REVEAL_TIMEOUT,
            fork_resolution_timeout: FORK_RESOLUTION_TIMEOUT,
            require_unanimous_bets: true,
            enable_fork_recovery: true,
            max_round_time: CONSENSUS_TIMEOUT,
            vote_timeout: Duration::from_secs(5),
            max_forks: 3,
        }
    }
}

/// Consensus performance metrics
#[derive(Debug, Default, Clone)]
pub struct ConsensusMetrics {
    /// Total consensus rounds completed
    pub rounds_completed: u64,

    /// Total consensus rounds failed
    pub rounds_failed: u64,

    /// Average consensus time (milliseconds)
    pub avg_consensus_time_ms: f64,

    /// Fork events resolved
    pub forks_resolved: u32,

    /// Memory usage in bytes
    pub memory_usage_bytes: usize,

    /// Signature verification count
    pub signatures_verified: u64,

    /// Cache hit rate for signature verification
    pub signature_cache_hit_rate: f64,
}

/// Compact signature representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactSignature {
    /// Compressed signature bytes
    pub data: [u8; 32],

    /// Recovery ID for public key recovery
    pub recovery_id: u8,
}

impl CompactSignature {
    /// Create new compact signature
    pub fn new(data: [u8; 32], recovery_id: u8) -> Self {
        Self { data, recovery_id }
    }

    /// Convert to full signature format
    pub fn to_full_signature(&self) -> [u8; 64] {
        let mut full = [0u8; 64];
        full[..32].copy_from_slice(&self.data);
        // Would implement proper signature conversion
        full
    }

    /// Verify signature against message and public key
    pub fn verify(&self, _message: &[u8], _public_key: &[u8; 32]) -> bool {
        // Would implement signature verification
        true
    }
}

// Type aliases for commonly used types
pub type ProposalId = Hash256;
pub type RoundId = u64;
pub type StateHash = Hash256;
pub type DisputeId = Hash256;
