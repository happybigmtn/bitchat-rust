//! Byzantine Fault Detection
//!
//! Detects and handles Byzantine behavior in the consensus network.

use super::{ConsensusConfig, ConsensusProposal, ConsensusVote, VoteType};
use crate::protocol::{PeerId, TransactionId};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Byzantine behavior patterns we detect
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ByzantineBehavior {
    /// Validator voted differently on the same proposal and round
    DoubleVoting {
        proposal_id: TransactionId,
        round: u32,
        vote1: ConsensusVote,
        vote2: ConsensusVote,
    },
    /// Validator proposed conflicting proposals
    DoubleProposal {
        round: u32,
        proposal1: ConsensusProposal,
        proposal2: ConsensusProposal,
    },
    /// Validator consistently votes against network consensus
    MaliciousVoting {
        consecutive_malicious_votes: u32,
        success_rate: f64,
    },
    /// Validator fails to participate in consensus rounds
    Liveness {
        missed_rounds: u32,
        total_rounds: u32,
        availability_rate: f64,
    },
    /// Validator sends invalid or malformed messages
    InvalidMessages {
        invalid_count: u32,
        total_count: u32,
    },
}

/// Evidence of Byzantine behavior
#[derive(Debug, Clone)]
pub struct ByzantineEvidence {
    pub validator: PeerId,
    pub behavior: ByzantineBehavior,
    pub detected_at: SystemTime,
    pub confidence: f64, // 0.0 to 1.0
    pub severity: ByzantineSeverity,
}

/// Severity levels for Byzantine behavior
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ByzantineSeverity {
    Suspected,  // Unusual behavior, monitor closely
    Likely,     // Strong evidence of Byzantine behavior
    Confirmed,  // Cryptographic proof of Byzantine behavior
    Critical,   // Behavior that threatens network safety
}

/// Validator behavior tracking
#[derive(Debug, Clone)]
struct ValidatorBehavior {
    peer_id: PeerId,
    votes: VecDeque<(TransactionId, u32, ConsensusVote)>, // (proposal, round, vote)
    proposals: VecDeque<(u32, ConsensusProposal)>,       // (round, proposal)
    missed_rounds: u32,
    total_rounds: u32,
    invalid_messages: u32,
    total_messages: u32,
    reputation: f64,
    last_activity: SystemTime,
}

impl ValidatorBehavior {
    fn new(peer_id: PeerId) -> Self {
        Self {
            peer_id,
            votes: VecDeque::new(),
            proposals: VecDeque::new(),
            missed_rounds: 0,
            total_rounds: 0,
            invalid_messages: 0,
            total_messages: 0,
            reputation: 1.0,
            last_activity: SystemTime::now(),
        }
    }
    
    fn add_vote(&mut self, proposal_id: TransactionId, round: u32, vote: ConsensusVote) {
        self.votes.push_back((proposal_id, round, vote));
        if self.votes.len() > 1000 {
            self.votes.pop_front();
        }
        self.total_messages += 1;
        self.last_activity = SystemTime::now();
    }
    
    fn add_proposal(&mut self, round: u32, proposal: ConsensusProposal) {
        self.proposals.push_back((round, proposal));
        if self.proposals.len() > 100 {
            self.proposals.pop_front();
        }
        self.total_messages += 1;
        self.last_activity = SystemTime::now();
    }
    
    fn record_invalid_message(&mut self) {
        self.invalid_messages += 1;
        self.total_messages += 1;
        self.reputation = (self.reputation * 0.9).max(0.1); // Decay reputation
    }
    
    fn record_round_participation(&mut self, participated: bool) {
        self.total_rounds += 1;
        if !participated {
            self.missed_rounds += 1;
        }
    }
}

/// Byzantine fault detector
pub struct ByzantineDetector {
    config: ConsensusConfig,
    validators: Arc<DashMap<PeerId, ValidatorBehavior>>,
    evidence: Arc<DashMap<PeerId, Vec<ByzantineEvidence>>>,
    detection_window: Duration,
    reputation_threshold: f64,
}

impl ByzantineDetector {
    pub fn new(config: ConsensusConfig) -> Self {
        Self {
            config,
            validators: Arc::new(DashMap::new()),
            evidence: Arc::new(DashMap::new()),
            detection_window: Duration::from_secs(300), // 5 minutes
            reputation_threshold: 0.3,
        }
    }
    
    /// Record a vote and check for Byzantine behavior
    pub async fn record_vote(&self, vote: ConsensusVote, proposal_id: TransactionId, round: u32) -> Vec<ByzantineEvidence> {
        let mut evidence = Vec::new();
        
        // Get or create validator behavior tracking
        let mut behavior = self.validators.entry(vote.voter)
            .or_insert_with(|| ValidatorBehavior::new(vote.voter));
        
        // Check for double voting
        if let Some(double_vote_evidence) = self.check_double_voting(&behavior, &vote, proposal_id, round) {
            evidence.push(double_vote_evidence);
        }
        
        // Record the vote
        behavior.add_vote(proposal_id, round, vote.clone());
        
        // Check for malicious voting patterns
        if let Some(malicious_evidence) = self.check_malicious_voting(&behavior) {
            evidence.push(malicious_evidence);
        }
        
        // Store evidence
        if !evidence.is_empty() {
            let mut validator_evidence = self.evidence.entry(vote.voter).or_insert_with(Vec::new);
            validator_evidence.extend(evidence.clone());
            
            // Keep only recent evidence
            let cutoff = SystemTime::now() - self.detection_window;
            validator_evidence.retain(|e| e.detected_at > cutoff);
        }
        
        evidence
    }
    
    /// Record a proposal and check for Byzantine behavior
    pub async fn record_proposal(&self, proposal: ConsensusProposal, round: u32) -> Vec<ByzantineEvidence> {
        let mut evidence = Vec::new();
        
        let mut behavior = self.validators.entry(proposal.proposer)
            .or_insert_with(|| ValidatorBehavior::new(proposal.proposer));
        
        // Check for double proposal
        if let Some(double_proposal_evidence) = self.check_double_proposal(&behavior, &proposal, round) {
            evidence.push(double_proposal_evidence);
        }
        
        behavior.add_proposal(round, proposal.clone());
        
        if !evidence.is_empty() {
            let mut validator_evidence = self.evidence.entry(proposal.proposer).or_insert_with(Vec::new);
            validator_evidence.extend(evidence.clone());
        }
        
        evidence
    }
    
    /// Record invalid message from validator
    pub async fn record_invalid_message(&self, validator: PeerId) {
        if let Some(mut behavior) = self.validators.get_mut(&validator) {
            behavior.record_invalid_message();
            
            // Check if invalid message rate is too high
            let invalid_rate = behavior.invalid_messages as f64 / behavior.total_messages as f64;
            if invalid_rate > 0.1 && behavior.total_messages > 10 {
                let evidence = ByzantineEvidence {
                    validator,
                    behavior: ByzantineBehavior::InvalidMessages {
                        invalid_count: behavior.invalid_messages,
                        total_count: behavior.total_messages,
                    },
                    detected_at: SystemTime::now(),
                    confidence: invalid_rate,
                    severity: if invalid_rate > 0.5 {
                        ByzantineSeverity::Confirmed
                    } else {
                        ByzantineSeverity::Likely
                    },
                };
                
                let mut validator_evidence = self.evidence.entry(validator).or_insert_with(Vec::new);
                validator_evidence.push(evidence);
            }
        }
    }
    
    /// Check validator liveness and record missed rounds
    pub async fn check_liveness(&self, active_validators: &[PeerId], round: u32) {
        let now = SystemTime::now();
        let liveness_threshold = now - Duration::from_secs(60); // 1 minute
        
        for validator in active_validators {
            let mut behavior = self.validators.entry(*validator)
                .or_insert_with(|| ValidatorBehavior::new(*validator));
            
            let participated = behavior.last_activity > liveness_threshold;
            behavior.record_round_participation(participated);
            
            if !participated {
                let availability_rate = 1.0 - (behavior.missed_rounds as f64 / behavior.total_rounds as f64);
                
                if availability_rate < 0.8 && behavior.total_rounds > 10 {
                    let evidence = ByzantineEvidence {
                        validator: *validator,
                        behavior: ByzantineBehavior::Liveness {
                            missed_rounds: behavior.missed_rounds,
                            total_rounds: behavior.total_rounds,
                            availability_rate,
                        },
                        detected_at: now,
                        confidence: 1.0 - availability_rate,
                        severity: if availability_rate < 0.5 {
                            ByzantineSeverity::Critical
                        } else {
                            ByzantineSeverity::Likely
                        },
                    };
                    
                    let mut validator_evidence = self.evidence.entry(*validator).or_insert_with(Vec::new);
                    validator_evidence.push(evidence);
                }
            }
        }
    }
    
    /// Get all evidence for a validator
    pub fn get_evidence(&self, validator: &PeerId) -> Vec<ByzantineEvidence> {
        self.evidence.get(validator)
            .map(|evidence| evidence.clone())
            .unwrap_or_default()
    }
    
    /// Get validators with Byzantine evidence above threshold
    pub fn get_byzantine_validators(&self) -> HashMap<PeerId, Vec<ByzantineEvidence>> {
        let mut byzantine = HashMap::new();
        
        for entry in self.evidence.iter() {
            let evidence_list = entry.value();
            let critical_evidence: Vec<_> = evidence_list.iter()
                .filter(|e| e.severity >= ByzantineSeverity::Likely)
                .cloned()
                .collect();
            
            if !critical_evidence.is_empty() {
                byzantine.insert(*entry.key(), critical_evidence);
            }
        }
        
        byzantine
    }
    
    /// Clear old evidence and reset reputation
    pub fn cleanup(&self) {
        let cutoff = SystemTime::now() - self.detection_window;
        
        // Clean up old evidence
        for mut entry in self.evidence.iter_mut() {
            entry.retain(|e| e.detected_at > cutoff);
        }
        
        // Remove empty entries
        self.evidence.retain(|_, evidence| !evidence.is_empty());
        
        // Reset reputation for validators with no recent evidence
        for mut behavior in self.validators.iter_mut() {
            if !self.evidence.contains_key(&behavior.peer_id) {
                behavior.reputation = (behavior.reputation + 0.1).min(1.0);
            }
        }
    }
    
    // Private helper methods
    
    fn check_double_voting(
        &self,
        behavior: &ValidatorBehavior,
        new_vote: &ConsensusVote,
        proposal_id: TransactionId,
        round: u32,
    ) -> Option<ByzantineEvidence> {
        // Look for existing vote on same proposal and round
        for (existing_proposal, existing_round, existing_vote) in &behavior.votes {
            if *existing_proposal == proposal_id && 
               *existing_round == round && 
               existing_vote.vote_type == new_vote.vote_type {
                
                // Check if votes are different (Byzantine behavior)
                if existing_vote.signature != new_vote.signature {
                    return Some(ByzantineEvidence {
                        validator: new_vote.voter,
                        behavior: ByzantineBehavior::DoubleVoting {
                            proposal_id,
                            round,
                            vote1: existing_vote.clone(),
                            vote2: new_vote.clone(),
                        },
                        detected_at: SystemTime::now(),
                        confidence: 1.0, // Cryptographic proof
                        severity: ByzantineSeverity::Confirmed,
                    });
                }
            }
        }
        
        None
    }
    
    fn check_double_proposal(
        &self,
        behavior: &ValidatorBehavior,
        new_proposal: &ConsensusProposal,
        round: u32,
    ) -> Option<ByzantineEvidence> {
        // Look for existing proposal in same round
        for (existing_round, existing_proposal) in &behavior.proposals {
            if *existing_round == round && existing_proposal.id != new_proposal.id {
                return Some(ByzantineEvidence {
                    validator: new_proposal.proposer,
                    behavior: ByzantineBehavior::DoubleProposal {
                        round,
                        proposal1: existing_proposal.clone(),
                        proposal2: new_proposal.clone(),
                    },
                    detected_at: SystemTime::now(),
                    confidence: 1.0,
                    severity: ByzantineSeverity::Confirmed,
                });
            }
        }
        
        None
    }
    
    fn check_malicious_voting(&self, behavior: &ValidatorBehavior) -> Option<ByzantineEvidence> {
        if behavior.votes.len() < 20 {
            return None; // Need sufficient history
        }
        
        // Simple heuristic: check if validator consistently votes against majority
        // In a real implementation, this would be more sophisticated
        let recent_votes = behavior.votes.iter().rev().take(20);
        let mut malicious_count = 0;
        
        for (proposal_id, round, vote) in recent_votes {
            // This is a simplified check - in reality you'd compare against actual consensus results
            if vote.vote_type == VoteType::Abort {
                malicious_count += 1;
            }
        }
        
        let malicious_rate = malicious_count as f64 / 20.0;
        if malicious_rate > 0.6 {
            return Some(ByzantineEvidence {
                validator: behavior.peer_id,
                behavior: ByzantineBehavior::MaliciousVoting {
                    consecutive_malicious_votes: malicious_count,
                    success_rate: 1.0 - malicious_rate,
                },
                detected_at: SystemTime::now(),
                confidence: malicious_rate,
                severity: ByzantineSeverity::Likely,
            });
        }
        
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::craps::BetType;
    
    #[tokio::test]
    async fn test_double_voting_detection() {
        let config = ConsensusConfig::default();
        let detector = ByzantineDetector::new(config);
        
        let validator = PeerId::new();
        let proposal_id = TransactionId::default();
        let round = 1;
        
        // First vote
        let vote1 = ConsensusVote {
            proposal_id,
            voter: validator,
            vote_type: VoteType::Commit,
            round,
            signature: vec![1, 2, 3],
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        };
        
        let evidence1 = detector.record_vote(vote1.clone(), proposal_id, round).await;
        assert!(evidence1.is_empty()); // No evidence yet
        
        // Second conflicting vote
        let vote2 = ConsensusVote {
            proposal_id,
            voter: validator,
            vote_type: VoteType::Commit,
            round,
            signature: vec![4, 5, 6], // Different signature
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        };
        
        let evidence2 = detector.record_vote(vote2, proposal_id, round).await;
        assert_eq!(evidence2.len(), 1);
        
        match &evidence2[0].behavior {
            ByzantineBehavior::DoubleVoting { .. } => {
                assert_eq!(evidence2[0].severity, ByzantineSeverity::Confirmed);
            },
            _ => panic!("Expected DoubleVoting evidence"),
        }
    }
    
    #[tokio::test]
    async fn test_invalid_message_tracking() {
        let config = ConsensusConfig::default();
        let detector = ByzantineDetector::new(config);
        
        let validator = PeerId::new();
        
        // Record multiple invalid messages
        for _ in 0..15 {
            detector.record_invalid_message(validator).await;
        }
        
        let evidence = detector.get_evidence(&validator);
        assert!(!evidence.is_empty());
        
        match &evidence[0].behavior {
            ByzantineBehavior::InvalidMessages { invalid_count, total_count } => {
                assert!(*invalid_count > 0);
                assert!(*total_count > 0);
            },
            _ => panic!("Expected InvalidMessages evidence"),
        }
    }
}