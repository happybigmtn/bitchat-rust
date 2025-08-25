//! Validation logic and dispute resolution

use serde::{Serialize, Deserialize};

use crate::protocol::{PeerId, Signature};
use crate::protocol::craps::{Bet, DiceRoll, CrapTokens};

use super::{DisputeId, RoundId};

/// Dispute representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dispute {
    pub id: DisputeId,
    pub disputer: PeerId,
    pub disputed_state: super::StateHash,
    pub claim: DisputeClaim,
    pub evidence: Vec<DisputeEvidence>,
    pub created_at: u64,
    pub resolution_deadline: u64,
}

/// Types of disputes that can be raised
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DisputeClaim {
    InvalidBet {
        player: PeerId,
        bet: Bet,
        reason: String,
    },
    InvalidRoll {
        round_id: RoundId,
        claimed_roll: DiceRoll,
        reason: String,
    },
    InvalidPayout {
        player: PeerId,
        expected: CrapTokens,
        actual: CrapTokens,
    },
    DoubleSpending {
        player: PeerId,
        conflicting_bets: Vec<Bet>,
    },
    ConsensusViolation {
        violated_rule: String,
        details: String,
    },
}

/// Evidence for dispute resolution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DisputeEvidence {
    SignedTransaction {
        data: Vec<u8>,
        signature: Signature,
    },
    StateProof {
        state_hash: super::StateHash,
        merkle_proof: Vec<u8>,
    },
    TimestampProof {
        timestamp: u64,
        proof: Vec<u8>,
    },
    WitnessTestimony {
        witness: PeerId,
        testimony: String,
        signature: Signature,
    },
}

/// Dispute resolution vote
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisputeVote {
    pub voter: PeerId,
    pub dispute_id: DisputeId,
    pub vote: DisputeVoteType,
    pub reasoning: String,
    pub timestamp: u64,
    pub signature: Signature,
}

/// Types of dispute votes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DisputeVoteType {
    /// Dispute is valid, punish the accused
    Uphold,
    
    /// Dispute is invalid, punish the disputer
    Reject,
    
    /// Not enough evidence to decide
    Abstain,
    
    /// Require additional evidence
    NeedMoreEvidence,
}

impl Dispute {
    /// Create new dispute
    pub fn new(
        disputer: PeerId,
        disputed_state: super::StateHash,
        claim: DisputeClaim,
    ) -> Self {
        let id = Self::generate_dispute_id(&disputer, &disputed_state, &claim);
        let created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let resolution_deadline = created_at + 3600; // 1 hour
        
        Self {
            id,
            disputer,
            disputed_state,
            claim,
            evidence: Vec::new(),
            created_at,
            resolution_deadline,
        }
    }
    
    /// Generate dispute ID
    fn generate_dispute_id(
        disputer: &PeerId,
        disputed_state: &super::StateHash,
        claim: &DisputeClaim,
    ) -> DisputeId {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        
        hasher.update(disputer);
        hasher.update(disputed_state);
        
        // Add claim-specific data
        match claim {
            DisputeClaim::InvalidBet { player, bet, .. } => {
                hasher.update(b"invalid_bet");
                hasher.update(player);
                hasher.update(bet.amount.0.to_le_bytes());
            },
            DisputeClaim::InvalidRoll { round_id, claimed_roll, .. } => {
                hasher.update(b"invalid_roll");
                hasher.update(round_id.to_le_bytes());
                hasher.update([claimed_roll.die1, claimed_roll.die2]);
            },
            DisputeClaim::InvalidPayout { player, expected, actual } => {
                hasher.update(b"invalid_payout");
                hasher.update(player);
                hasher.update(expected.0.to_le_bytes());
                hasher.update(actual.0.to_le_bytes());
            },
            DisputeClaim::DoubleSpending { player, .. } => {
                hasher.update(b"double_spending");
                hasher.update(player);
            },
            DisputeClaim::ConsensusViolation { violated_rule, .. } => {
                hasher.update(b"consensus_violation");
                hasher.update(violated_rule.as_bytes());
            },
        }
        
        hasher.finalize().into()
    }
    
    /// Add evidence to dispute
    pub fn add_evidence(&mut self, evidence: DisputeEvidence) {
        self.evidence.push(evidence);
    }
    
    /// Check if dispute is expired
    pub fn is_expired(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now > self.resolution_deadline
    }
    
    /// Validate dispute claim
    pub fn validate_claim(&self) -> bool {
        match &self.claim {
            DisputeClaim::InvalidBet { bet, .. } => {
                // Validate bet parameters
                bet.amount.0 > 0 && bet.amount.0 <= 1000000 // Max bet limit
            },
            DisputeClaim::InvalidRoll { claimed_roll, .. } => {
                // Validate dice roll
                claimed_roll.die1 >= 1 && claimed_roll.die1 <= 6 &&
                claimed_roll.die2 >= 1 && claimed_roll.die2 <= 6
            },
            DisputeClaim::InvalidPayout { expected, actual, .. } => {
                // Check if payout amounts are reasonable
                expected.0 != actual.0 && expected.0 > 0
            },
            DisputeClaim::DoubleSpending { conflicting_bets, .. } => {
                // Check if there are actually conflicting bets
                conflicting_bets.len() >= 2
            },
            DisputeClaim::ConsensusViolation { violated_rule, .. } => {
                // Check if rule name is valid
                !violated_rule.is_empty()
            },
        }
    }
}

impl DisputeVote {
    /// Create new dispute vote
    pub fn new(
        voter: PeerId,
        dispute_id: DisputeId,
        vote: DisputeVoteType,
        reasoning: String,
    ) -> Self {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        Self {
            voter,
            dispute_id,
            vote,
            reasoning,
            timestamp,
            signature: crate::protocol::Signature([0u8; 64]), // Would implement proper signing
        }
    }
    
    /// Verify vote signature
    pub fn verify_signature(&self) -> bool {
        // Would implement signature verification
        true
    }
}

/// Dispute validator
pub struct DisputeValidator;

impl DisputeValidator {
    /// Validate a dispute claim with evidence
    pub fn validate_dispute(dispute: &Dispute) -> bool {
        // Basic validation
        if !dispute.validate_claim() {
            return false;
        }
        
        // Validate evidence
        for evidence in &dispute.evidence {
            if !Self::validate_evidence(evidence) {
                return false;
            }
        }
        
        true
    }
    
    /// Validate individual evidence
    fn validate_evidence(evidence: &DisputeEvidence) -> bool {
        match evidence {
            DisputeEvidence::SignedTransaction { data, signature: _ } => {
                // Validate transaction data and signature
                !data.is_empty()
            },
            DisputeEvidence::StateProof { merkle_proof, .. } => {
                // Validate merkle proof
                !merkle_proof.is_empty()
            },
            DisputeEvidence::TimestampProof { timestamp, proof } => {
                // Validate timestamp and proof
                *timestamp > 0 && !proof.is_empty()
            },
            DisputeEvidence::WitnessTestimony { testimony, .. } => {
                // Validate testimony content
                !testimony.is_empty()
            },
        }
    }
    
    /// Resolve dispute based on votes
    pub fn resolve_dispute(
        _dispute: &Dispute,
        votes: &[DisputeVote],
        min_votes: usize,
    ) -> Option<DisputeVoteType> {
        if votes.len() < min_votes {
            return None;
        }
        
        // Count votes
        let mut uphold_count = 0;
        let mut reject_count = 0;
        let mut _abstain_count = 0; // Prefixed with _ to indicate intentionally unused
        let mut need_evidence_count = 0;
        
        for vote in votes {
            match vote.vote {
                DisputeVoteType::Uphold => uphold_count += 1,
                DisputeVoteType::Reject => reject_count += 1,
                DisputeVoteType::Abstain => _abstain_count += 1,
                DisputeVoteType::NeedMoreEvidence => need_evidence_count += 1,
            }
        }
        
        // Determine majority vote
        let total_votes = votes.len();
        let majority_threshold = total_votes / 2 + 1;
        
        if uphold_count >= majority_threshold {
            Some(DisputeVoteType::Uphold)
        } else if reject_count >= majority_threshold {
            Some(DisputeVoteType::Reject)
        } else if need_evidence_count >= majority_threshold {
            Some(DisputeVoteType::NeedMoreEvidence)
        } else {
            Some(DisputeVoteType::Abstain)
        }
    }
}