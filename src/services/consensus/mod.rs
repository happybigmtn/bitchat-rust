//! Consensus Service
//!
//! Microservice responsible for distributed consensus, Byzantine fault tolerance,
//! and coordinating agreement across the network.

pub mod api;
pub mod service;
pub mod types;
pub mod byzantine;
pub mod http;

pub use service::ConsensusService;
pub use types::*;

use crate::error::{Error, Result};
use crate::protocol::{GameId, PeerId, TransactionId};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, BTreeMap};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Consensus configuration
#[derive(Debug, Clone)]
pub struct ConsensusConfig {
    /// Byzantine fault tolerance threshold (typically f < n/3)
    pub byzantine_threshold: usize,
    /// Timeout for consensus rounds
    pub round_timeout: Duration,
    /// Maximum number of consensus rounds
    pub max_rounds: u32,
    /// Minimum number of validators required
    pub min_validators: usize,
    /// Consensus algorithm variant
    pub algorithm: ConsensusAlgorithm,
}

/// Supported consensus algorithms
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConsensusAlgorithm {
    PBFT,        // Practical Byzantine Fault Tolerance
    Tendermint,  // Tendermint-style consensus
    HotStuff,    // HotStuff consensus
}

impl Default for ConsensusConfig {
    fn default() -> Self {
        Self {
            byzantine_threshold: 1, // Allow up to 1 Byzantine node (for 4+ node network)
            round_timeout: Duration::from_secs(10),
            max_rounds: 10,
            min_validators: 3,
            algorithm: ConsensusAlgorithm::PBFT,
        }
    }
}

/// Consensus proposal that needs agreement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusProposal {
    pub id: TransactionId,
    pub proposer: PeerId,
    pub game_id: Option<GameId>,
    pub proposal_type: ProposalType,
    pub data: Vec<u8>,
    pub timestamp: u64,
    pub round: u32,
}

/// Types of proposals that can be made
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProposalType {
    GameAction { action: String },
    StateTransition { from_state: String, to_state: String },
    ValidatorChange { validator: PeerId, action: ValidatorAction },
    NetworkUpgrade { version: String },
    EconomicParameter { parameter: String, value: u64 },
}

/// Validator actions for network changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidatorAction {
    Add,
    Remove,
    Suspend,
    Reinstate,
}

/// Vote on a consensus proposal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusVote {
    pub proposal_id: TransactionId,
    pub voter: PeerId,
    pub vote_type: VoteType,
    pub round: u32,
    pub signature: Vec<u8>,
    pub timestamp: u64,
}

/// Types of votes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VoteType {
    Prepare,    // PBFT prepare phase
    Commit,     // PBFT commit phase
    PreVote,    // Tendermint pre-vote
    PreCommit,  // Tendermint pre-commit
    Abort,      // Abort the proposal
}

/// Consensus round state
#[derive(Debug, Clone)]
pub struct ConsensusRound {
    pub round_number: u32,
    pub proposal: Option<ConsensusProposal>,
    pub votes: HashMap<PeerId, HashMap<VoteType, ConsensusVote>>,
    pub start_time: SystemTime,
    pub status: RoundStatus,
}

/// Status of a consensus round
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoundStatus {
    Proposed,
    Prepared,
    Committed,
    Aborted,
    Timeout,
}

/// Validator information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Validator {
    pub peer_id: PeerId,
    pub stake: u64,
    pub reputation: f64,
    pub is_active: bool,
    pub last_seen: u64,
}

/// Network state maintained by consensus service
#[derive(Debug, Clone)]
pub struct NetworkState {
    pub validators: BTreeMap<PeerId, Validator>,
    pub current_height: u64,
    pub current_round: u32,
    pub leader: Option<PeerId>,
    pub last_commit_time: SystemTime,
}

impl NetworkState {
    pub fn new() -> Self {
        Self {
            validators: BTreeMap::new(),
            current_height: 0,
            current_round: 0,
            leader: None,
            last_commit_time: SystemTime::now(),
        }
    }
    
    /// Get active validators
    pub fn active_validators(&self) -> Vec<&Validator> {
        self.validators.values()
            .filter(|v| v.is_active)
            .collect()
    }
    
    /// Calculate if we have enough validators for Byzantine fault tolerance
    pub fn has_sufficient_validators(&self, config: &ConsensusConfig) -> bool {
        let active_count = self.active_validators().len();
        active_count >= config.min_validators && 
        active_count >= 3 * config.byzantine_threshold + 1
    }
    
    /// Select leader for current round (round-robin based on stake)
    pub fn select_leader(&mut self) -> Option<PeerId> {
        let active_validators: Vec<_> = self.active_validators().into_iter().collect();
        if active_validators.is_empty() {
            return None;
        }
        
        // Weighted round-robin based on stake
        let total_stake: u64 = active_validators.iter().map(|v| v.stake).sum();
        if total_stake == 0 {
            // Fallback to simple round-robin
            let index = (self.current_round as usize) % active_validators.len();
            self.leader = Some(active_validators[index].peer_id);
        } else {
            // Stake-weighted selection
            let target = (self.current_round as u64 * total_stake) % total_stake;
            let mut accumulator = 0u64;
            
            for validator in &active_validators {
                accumulator += validator.stake;
                if accumulator > target {
                    self.leader = Some(validator.peer_id);
                    break;
                }
            }
        }
        
        self.leader
    }
}

/// Consensus result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusResult {
    pub proposal_id: TransactionId,
    pub status: ConsensusStatus,
    pub final_round: u32,
    pub commit_time: u64,
    pub participating_validators: Vec<PeerId>,
    /// Optional quorum certificate bytes (engine-specific encoding)
    pub quorum_certificate: Option<Vec<u8>>,
}

/// Final status of consensus
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConsensusStatus {
    Committed,
    Rejected,
    Timeout,
    ByzantineFault,
}

/// Metrics for consensus performance
#[derive(Debug, Clone, Default)]
pub struct ConsensusMetrics {
    pub total_proposals: u64,
    pub committed_proposals: u64,
    pub rejected_proposals: u64,
    pub timeout_proposals: u64,
    pub byzantine_faults_detected: u64,
    pub average_rounds_to_commit: f64,
    pub average_time_to_commit: Duration,
}

impl ConsensusMetrics {
    pub fn record_consensus(&mut self, result: &ConsensusResult, duration: Duration) {
        self.total_proposals += 1;
        
        match result.status {
            ConsensusStatus::Committed => {
                self.committed_proposals += 1;
                self.update_averages(result.final_round, duration);
            },
            ConsensusStatus::Rejected => self.rejected_proposals += 1,
            ConsensusStatus::Timeout => self.timeout_proposals += 1,
            ConsensusStatus::ByzantineFault => self.byzantine_faults_detected += 1,
        }
    }
    
    fn update_averages(&mut self, rounds: u32, duration: Duration) {
        // Exponential moving average with alpha = 0.1
        const ALPHA: f64 = 0.1;
        
        self.average_rounds_to_commit = 
            ALPHA * rounds as f64 + (1.0 - ALPHA) * self.average_rounds_to_commit;
        
        let new_time = duration.as_secs_f64();
        let current_time = self.average_time_to_commit.as_secs_f64();
        self.average_time_to_commit = 
            Duration::from_secs_f64(ALPHA * new_time + (1.0 - ALPHA) * current_time);
    }
}
