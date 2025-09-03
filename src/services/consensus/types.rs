//! Consensus Service Types
//!
//! Request/response types for the consensus service API.

use super::{ConsensusProposal, ConsensusVote, ProposalType, ValidatorAction};
use crate::protocol::{GameId, PeerId, TransactionId};
use serde::{Deserialize, Serialize};

/// Request to propose something for consensus
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposeRequest {
    pub game_id: Option<GameId>,
    pub proposal_type: ProposalType,
    pub data: Vec<u8>,
}

/// Response to a propose request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposeResponse {
    pub proposal_id: TransactionId,
    pub status: String,
}

/// Request to vote on a proposal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoteRequest {
    pub proposal_id: TransactionId,
    pub vote: ConsensusVote,
}

/// Response to a vote request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoteResponse {
    pub accepted: bool,
    pub current_round: u32,
}

/// Request for consensus status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusRequest {
    pub proposal_id: Option<TransactionId>,
}

/// Response with consensus status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusResponse {
    pub network_height: u64,
    pub current_round: u32,
    pub active_validators: u32,
    pub leader: Option<PeerId>,
    pub active_proposals: Vec<ActiveProposal>,
    pub metrics: ConsensusMetricsResponse,
}

/// Active proposal information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveProposal {
    pub proposal_id: TransactionId,
    pub round: u32,
    pub status: String,
    pub votes_received: u32,
    pub votes_required: u32,
}

/// Consensus metrics for API response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusMetricsResponse {
    pub total_proposals: u64,
    pub committed_proposals: u64,
    pub rejected_proposals: u64,
    pub timeout_proposals: u64,
    pub byzantine_faults_detected: u64,
    pub average_rounds_to_commit: f64,
    pub average_time_to_commit_ms: u64,
}

/// Request to update validator set
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateValidatorRequest {
    pub peer_id: PeerId,
    pub action: ValidatorUpdateAction,
    pub stake: Option<u64>,
}

/// Validator update actions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidatorUpdateAction {
    Add,
    Remove,
    Suspend,
    Reinstate,
}

/// Response to validator update
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateValidatorResponse {
    pub success: bool,
    pub active_validators: u32,
}

/// Health check response for consensus service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusHealthResponse {
    pub status: String,
    pub version: String,
    pub uptime_seconds: u64,
    pub network_height: u64,
    pub active_validators: u32,
    pub is_leader: bool,
    pub metrics: ConsensusMetricsResponse,
}

/// Consensus event types for subscriptions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConsensusEvent {
    ProposalReceived(ConsensusProposal),
    VoteReceived { 
        proposal_id: TransactionId, 
        vote: ConsensusVote 
    },
    ConsensusReached { 
        proposal_id: TransactionId, 
        result: super::ConsensusResult 
    },
    RoundTimeout { 
        proposal_id: TransactionId, 
        round: u32 
    },
    LeaderChanged { 
        old_leader: Option<PeerId>, 
        new_leader: Option<PeerId> 
    },
    ValidatorAdded(PeerId),
    ValidatorRemoved(PeerId),
    ByzantineFaultDetected { 
        validator: PeerId, 
        fault_type: String 
    },
}

/// Configuration for consensus event subscriptions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionRequest {
    pub event_types: Vec<String>,
    pub game_id_filter: Option<GameId>,
    pub validator_filter: Option<PeerId>,
}

/// Batch proposal request for efficiency
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchProposeRequest {
    pub proposals: Vec<ProposeRequest>,
    pub atomic: bool, // All proposals must succeed or all fail
}

/// Batch proposal response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchProposeResponse {
    pub results: Vec<Result<ProposeResponse, String>>,
    pub batch_id: Option<TransactionId>,
}

/// Request to get detailed proposal information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetProposalRequest {
    pub proposal_id: TransactionId,
}

/// Detailed proposal response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetProposalResponse {
    pub proposal: ConsensusProposal,
    pub current_round: u32,
    pub votes: Vec<ConsensusVote>,
    pub status: String,
    pub started_at: u64,
    pub timeout_at: u64,
}

/// Request to cancel a proposal (if still in progress)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelProposalRequest {
    pub proposal_id: TransactionId,
    pub reason: String,
}

/// Response to proposal cancellation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelProposalResponse {
    pub success: bool,
    pub message: String,
}

/// Network partition recovery request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionRecoveryRequest {
    pub last_known_height: u64,
    pub validator_set: Vec<PeerId>,
}

/// Network partition recovery response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionRecoveryResponse {
    pub current_height: u64,
    pub missing_proposals: Vec<TransactionId>,
    pub current_validator_set: Vec<PeerId>,
    pub leader: Option<PeerId>,
}

/// Service error types specific to consensus
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConsensusServiceError {
    ProposalNotFound(TransactionId),
    InvalidVote(String),
    InsufficientValidators,
    ByzantineThresholdExceeded,
    NetworkPartition,
    ConsensusTimeout,
    InvalidProposal(String),
    ServiceUnavailable(String),
    InternalError(String),
}

impl std::fmt::Display for ConsensusServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConsensusServiceError::ProposalNotFound(id) => {
                write!(f, "Proposal not found: {:?}", id)
            },
            ConsensusServiceError::InvalidVote(msg) => {
                write!(f, "Invalid vote: {}", msg)
            },
            ConsensusServiceError::InsufficientValidators => {
                write!(f, "Insufficient validators for consensus")
            },
            ConsensusServiceError::ByzantineThresholdExceeded => {
                write!(f, "Byzantine fault threshold exceeded")
            },
            ConsensusServiceError::NetworkPartition => {
                write!(f, "Network partition detected")
            },
            ConsensusServiceError::ConsensusTimeout => {
                write!(f, "Consensus operation timed out")
            },
            ConsensusServiceError::InvalidProposal(msg) => {
                write!(f, "Invalid proposal: {}", msg)
            },
            ConsensusServiceError::ServiceUnavailable(msg) => {
                write!(f, "Service unavailable: {}", msg)
            },
            ConsensusServiceError::InternalError(msg) => {
                write!(f, "Internal error: {}", msg)
            },
        }
    }
}

impl std::error::Error for ConsensusServiceError {}

/// Convert consensus service errors to HTTP status codes
impl ConsensusServiceError {
    pub fn status_code(&self) -> u16 {
        match self {
            ConsensusServiceError::ProposalNotFound(_) => 404,
            ConsensusServiceError::InvalidVote(_) => 400,
            ConsensusServiceError::InvalidProposal(_) => 400,
            ConsensusServiceError::InsufficientValidators => 503,
            ConsensusServiceError::ByzantineThresholdExceeded => 503,
            ConsensusServiceError::NetworkPartition => 503,
            ConsensusServiceError::ConsensusTimeout => 408,
            ConsensusServiceError::ServiceUnavailable(_) => 503,
            ConsensusServiceError::InternalError(_) => 500,
        }
    }
}

/// Request authentication for consensus operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusAuth {
    pub peer_id: PeerId,
    pub signature: Vec<u8>,
    pub timestamp: u64,
}

/// Authenticated request wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticatedRequest<T> {
    pub auth: ConsensusAuth,
    pub payload: T,
}

/// Generic service response wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub timestamp: u64,
}

impl<T> ServiceResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }
    
    pub fn error(error: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }
}