//! P2P Consensus Messages for BitChat-Rust Casino
//! 
//! This module defines the message types for distributed consensus
//! between game participants over the mesh network.

use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use crate::protocol::{PeerId, GameId, Hash256, Signature, CrapTokens};
use crate::protocol::consensus::engine::{GameProposal, GameOperation};
use crate::protocol::consensus::validation::{Dispute, DisputeVote};
use crate::protocol::consensus::{ProposalId, DisputeId};
use crate::protocol::consensus::commit_reveal::{RandomnessCommit, RandomnessReveal};
use crate::error::Result;

/// Round identifier for consensus rounds
pub type RoundId = u64;

/// Network-level consensus message envelope
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusMessage {
    /// Message ID for deduplication
    pub message_id: [u8; 32],
    /// Sender's peer ID
    pub sender: PeerId,
    /// Target game session
    pub game_id: GameId,
    /// Consensus round number
    pub round: RoundId,
    /// Message timestamp
    pub timestamp: u64,
    /// Message payload
    pub payload: ConsensusPayload,
    /// Cryptographic signature
    pub signature: Signature,
    /// Compression flags for BLE optimization
    pub compressed: bool,
}

/// Core consensus message payloads
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConsensusPayload {
    // Proposal phase messages
    Proposal {
        proposal: GameProposal,
        priority: u8, // For leader election ordering
    },
    
    // Voting phase messages  
    Vote {
        proposal_id: ProposalId,
        vote: bool, // true = accept, false = reject
        reasoning: Option<String>,
    },
    
    // State synchronization messages
    StateSync {
        state_hash: Hash256,
        sequence_number: u64,
        partial_state: Option<CompressedGameState>,
    },
    
    // Randomness generation (commit-reveal)
    RandomnessCommit {
        round_id: RoundId,
        commitment: RandomnessCommit,
    },
    RandomnessReveal {
        round_id: RoundId,
        reveal: RandomnessReveal,
    },
    
    // Dispute resolution
    DisputeClaim {
        dispute: Dispute,
        evidence_hash: Hash256,
    },
    DisputeVote {
        dispute_id: DisputeId,
        vote: DisputeVote,
    },
    
    // Network management
    JoinRequest {
        participant_info: ParticipantInfo,
        stake_proof: CrapTokens,
    },
    JoinAccept {
        session_state: CompressedGameState,
        participants: Vec<PeerId>,
    },
    JoinReject {
        reason: String,
    },
    
    // Heartbeat and liveness
    Heartbeat {
        alive_participants: Vec<PeerId>,
        network_view: NetworkView,
    },
    
    // Leader election
    LeaderProposal {
        proposed_leader: PeerId,
        term: u64,
        priority_score: u64,
    },
    LeaderAccept {
        term: u64,
        leader: PeerId,
    },
    
    // Network partition recovery
    PartitionRecovery {
        partition_id: u64,
        participants: Vec<PeerId>,
        state_summary: StateSummary,
    },
    
    // Anti-cheat and validation
    CheatAlert {
        suspected_peer: PeerId,
        violation_type: CheatType,
        evidence: Vec<u8>,
    },
}

/// Compressed game state for efficient BLE transmission
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressedGameState {
    /// State sequence number
    pub sequence: u64,
    /// Compressed state data (LZ4 compressed)
    pub data: Vec<u8>,
    /// Checksum for integrity
    pub checksum: u32,
    /// Uncompressed size
    pub original_size: u32,
}

/// Participant information for join requests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantInfo {
    pub peer_id: PeerId,
    pub display_name: String,
    pub initial_balance: CrapTokens,
    pub reputation_score: f64,
    pub supported_features: Vec<String>,
}

/// Network topology view for partition detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkView {
    pub participants: Vec<PeerId>,
    pub connections: Vec<(PeerId, PeerId)>,
    pub partition_id: Option<u64>,
    pub leader: Option<PeerId>,
}

/// State summary for partition recovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSummary {
    pub state_hash: Hash256,
    pub sequence_number: u64,
    pub participant_balances: Vec<(PeerId, CrapTokens)>,
    pub game_phase: u8,
    pub last_operation: Option<GameOperation>,
}

/// Types of cheat detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CheatType {
    DoubleVoting,
    InvalidStateTransition,
    TimestampManipulation,
    SignatureForgery,
    BalanceViolation,
    ConsensusViolation,
}

impl ConsensusMessage {
    /// Create a new consensus message
    pub fn new(
        sender: PeerId,
        game_id: GameId,
        round: RoundId,
        payload: ConsensusPayload,
    ) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
            
        // Generate unique message ID
        let message_id = Self::generate_message_id(&sender, &game_id, round, timestamp);
        
        Self {
            message_id,
            sender,
            game_id,
            round,
            timestamp,
            payload,
            signature: Signature([0u8; 64]), // Will be set by signing process
            compressed: false,
        }
    }
    
    /// Generate deterministic message ID
    fn generate_message_id(
        sender: &PeerId,
        game_id: &GameId,
        round: RoundId,
        timestamp: u64,
    ) -> [u8; 32] {
        use sha2::{Sha256, Digest};
        
        let mut hasher = Sha256::new();
        hasher.update(sender);
        hasher.update(game_id);
        hasher.update(round.to_le_bytes());
        hasher.update(timestamp.to_le_bytes());
        
        hasher.finalize().into()
    }
    
    /// Check if message is recent (for spam prevention)
    pub fn is_recent(&self, max_age_seconds: u64) -> bool {
        let now = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
            
        self.timestamp + max_age_seconds >= now
    }
    
    /// Compress payload for BLE transmission
    pub fn compress(&mut self) -> Result<()> {
        if self.compressed {
            return Ok(()); // Already compressed
        }
        
        // Serialize payload
        let payload_bytes = bincode::serialize(&self.payload)
            .map_err(|e| crate::error::Error::Serialization(e.to_string()))?;
        
        // Compress with LZ4
        let compressed_bytes = lz4_flex::compress_prepend_size(&payload_bytes);
        
        // Replace payload with compressed version if beneficial
        if compressed_bytes.len() < payload_bytes.len() {
            // Create a compressed payload marker
            self.payload = ConsensusPayload::StateSync {
                state_hash: [0u8; 32], // Placeholder
                sequence_number: 0,
                partial_state: Some(CompressedGameState {
                    sequence: 0,
                    data: compressed_bytes,
                    checksum: crc32fast::hash(&payload_bytes),
                    original_size: payload_bytes.len() as u32,
                }),
            };
            self.compressed = true;
        }
        
        Ok(())
    }
    
    /// Get payload size in bytes (for BLE MTU planning)
    pub fn payload_size(&self) -> usize {
        bincode::serialized_size(&self.payload).unwrap_or(0) as usize
    }
}

/// Message priority for bandwidth management
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MessagePriority {
    Critical = 0,   // Consensus votes, disputes
    High = 1,       // Proposals, state sync
    Normal = 2,     // Randomness commits/reveals
    Low = 3,        // Heartbeats, network maintenance
}

impl ConsensusPayload {
    /// Get message priority for bandwidth management
    pub fn priority(&self) -> MessagePriority {
        match self {
            ConsensusPayload::Vote { .. } => MessagePriority::Critical,
            ConsensusPayload::DisputeVote { .. } => MessagePriority::Critical,
            ConsensusPayload::DisputeClaim { .. } => MessagePriority::Critical,
            
            ConsensusPayload::Proposal { .. } => MessagePriority::High,
            ConsensusPayload::StateSync { .. } => MessagePriority::High,
            ConsensusPayload::PartitionRecovery { .. } => MessagePriority::High,
            
            ConsensusPayload::RandomnessCommit { .. } => MessagePriority::Normal,
            ConsensusPayload::RandomnessReveal { .. } => MessagePriority::Normal,
            ConsensusPayload::LeaderProposal { .. } => MessagePriority::Normal,
            ConsensusPayload::LeaderAccept { .. } => MessagePriority::Normal,
            
            ConsensusPayload::Heartbeat { .. } => MessagePriority::Low,
            ConsensusPayload::JoinRequest { .. } => MessagePriority::Low,
            ConsensusPayload::JoinAccept { .. } => MessagePriority::Low,
            ConsensusPayload::JoinReject { .. } => MessagePriority::Low,
            ConsensusPayload::CheatAlert { .. } => MessagePriority::Critical,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_message_creation() {
        let sender = [1u8; 32];
        let game_id = [2u8; 16];
        let payload = ConsensusPayload::Heartbeat {
            alive_participants: vec![sender],
            network_view: NetworkView {
                participants: vec![sender],
                connections: vec![],
                partition_id: None,
                leader: None,
            },
        };
        
        let message = ConsensusMessage::new(sender, game_id, 1, payload);
        
        assert_eq!(message.sender, sender);
        assert_eq!(message.game_id, game_id);
        assert_eq!(message.round, 1);
        assert!(!message.compressed);
    }
    
    #[test]
    fn test_message_priority() {
        let vote_payload = ConsensusPayload::Vote {
            proposal_id: [0u8; 32],
            vote: true,
            reasoning: None,
        };
        assert_eq!(vote_payload.priority(), MessagePriority::Critical);
        
        let heartbeat_payload = ConsensusPayload::Heartbeat {
            alive_participants: vec![],
            network_view: NetworkView {
                participants: vec![],
                connections: vec![],
                partition_id: None,
                leader: None,
            },
        };
        assert_eq!(heartbeat_payload.priority(), MessagePriority::Low);
    }
}