//! Reputation System with Dispute Resolution
//! 
//! This module implements a decentralized reputation system that tracks
//! player behavior and automatically resolves disputes through voting
//! and evidence-based mechanisms.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use serde::{Serialize, Deserialize};
use crate::protocol::{PeerId, Hash256};
use crate::error::Error;
use crate::crypto::GameCrypto;

/// Reputation score bounds
pub const MIN_REPUTATION: i64 = -1000;
pub const MAX_REPUTATION: i64 = 1000;
pub const INITIAL_REPUTATION: i64 = 100;

/// Reputation change amounts
pub const REP_SUCCESSFUL_GAME: i64 = 5;
pub const REP_FAILED_COMMIT: i64 = -20;
pub const REP_FAILED_REVEAL: i64 = -30;
pub const REP_INVALID_SIGNATURE: i64 = -50;
pub const REP_CHEATING_ATTEMPT: i64 = -100;
pub const REP_DISPUTE_WIN: i64 = 20;
pub const REP_DISPUTE_LOSS: i64 = -40;
pub const REP_FALSE_ACCUSATION: i64 = -60;
pub const REP_TIMEOUT_PENALTY: i64 = -15;

/// Minimum reputation to participate
pub const MIN_REP_TO_PLAY: i64 = -500;
pub const MIN_REP_TO_VOTE: i64 = 0;

/// Dispute timeout
pub const DISPUTE_TIMEOUT: Duration = Duration::from_secs(60);
pub const VOTE_TIMEOUT: Duration = Duration::from_secs(30);

/// Reputation event types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReputationEvent {
    /// Successfully completed a game
    GameCompleted,
    /// Failed to submit commit in consensus
    FailedCommit,
    /// Failed to reveal after commit
    FailedReveal,
    /// Submitted invalid signature
    InvalidSignature,
    /// Attempted to cheat (double spend, invalid state, etc.)
    CheatingAttempt { evidence: String },
    /// Won a dispute
    DisputeWon { dispute_id: Hash256 },
    /// Lost a dispute
    DisputeLost { dispute_id: Hash256 },
    /// Made false accusation
    FalseAccusation { dispute_id: Hash256 },
    /// Timeout during critical phase
    TimeoutPenalty { phase: String },
    /// Positive contribution (relay, validation, etc.)
    PositiveContribution { reason: String },
}

/// Reputation record for a peer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReputationRecord {
    /// Current reputation score
    pub score: i64,
    /// Total games played
    pub games_played: u64,
    /// Games completed successfully
    pub games_completed: u64,
    /// Disputes raised
    pub disputes_raised: u32,
    /// Disputes won
    pub disputes_won: u32,
    /// Last update timestamp
    pub last_updated: u64,
    /// Recent events (limited history)
    pub recent_events: VecDeque<(u64, ReputationEvent)>,
    /// Ban expiry (if banned)
    pub ban_expiry: Option<u64>,
}

impl ReputationRecord {
    /// Create new reputation record
    pub fn new() -> Self {
        Self {
            score: INITIAL_REPUTATION,
            games_played: 0,
            games_completed: 0,
            disputes_raised: 0,
            disputes_won: 0,
            last_updated: current_timestamp(),
            recent_events: VecDeque::with_capacity(100),
            ban_expiry: None,
        }
    }
    
    /// Apply reputation event
    pub fn apply_event(&mut self, event: ReputationEvent) {
        let change = match &event {
            ReputationEvent::GameCompleted => {
                self.games_completed += 1;
                REP_SUCCESSFUL_GAME
            },
            ReputationEvent::FailedCommit => REP_FAILED_COMMIT,
            ReputationEvent::FailedReveal => REP_FAILED_REVEAL,
            ReputationEvent::InvalidSignature => REP_INVALID_SIGNATURE,
            ReputationEvent::CheatingAttempt { .. } => REP_CHEATING_ATTEMPT,
            ReputationEvent::DisputeWon { .. } => {
                self.disputes_won += 1;
                REP_DISPUTE_WIN
            },
            ReputationEvent::DisputeLost { .. } => REP_DISPUTE_LOSS,
            ReputationEvent::FalseAccusation { .. } => REP_FALSE_ACCUSATION,
            ReputationEvent::TimeoutPenalty { .. } => REP_TIMEOUT_PENALTY,
            ReputationEvent::PositiveContribution { .. } => REP_SUCCESSFUL_GAME / 2,
        };
        
        // Update score with bounds
        self.score = (self.score + change).clamp(MIN_REPUTATION, MAX_REPUTATION);
        
        // Track event
        self.recent_events.push_back((current_timestamp(), event));
        if self.recent_events.len() > 100 {
            self.recent_events.pop_front();
        }
        
        // Update timestamp
        self.last_updated = current_timestamp();
        
        // Check for auto-ban on severe negative reputation
        if self.score <= MIN_REPUTATION / 2 {
            self.ban_expiry = Some(current_timestamp() + 86400); // 24 hour ban
        }
    }
    
    /// Check if peer is banned
    pub fn is_banned(&self) -> bool {
        if let Some(expiry) = self.ban_expiry {
            current_timestamp() < expiry
        } else {
            false
        }
    }
    
    /// Check if peer can participate in games
    pub fn can_play(&self) -> bool {
        !self.is_banned() && self.score >= MIN_REP_TO_PLAY
    }
    
    /// Check if peer can vote in disputes
    pub fn can_vote(&self) -> bool {
        !self.is_banned() && self.score >= MIN_REP_TO_VOTE
    }
    
    /// Get trust level (0.0 to 1.0)
    pub fn trust_level(&self) -> f64 {
        let normalized = (self.score - MIN_REPUTATION) as f64 / (MAX_REPUTATION - MIN_REPUTATION) as f64;
        normalized.clamp(0.0, 1.0)
    }
}

/// Dispute types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DisputeType {
    /// Invalid game state proposed
    InvalidState { proposed_state: Vec<u8>, evidence: Vec<u8> },
    /// Failed to reveal after commit
    FailedReveal { round_id: u64, peer: PeerId },
    /// Invalid signature on message
    InvalidSignature { message: Vec<u8>, signature: Vec<u8> },
    /// Cheating attempt detected
    Cheating { description: String, evidence: Vec<u8> },
    /// Timeout violation
    TimeoutViolation { phase: String, deadline: u64 },
    /// Double spending tokens
    DoubleSpend { tx1: Vec<u8>, tx2: Vec<u8> },
}

/// Dispute record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dispute {
    /// Unique dispute ID
    pub id: Hash256,
    /// Type of dispute
    pub dispute_type: DisputeType,
    /// Accuser
    pub accuser: PeerId,
    /// Accused
    pub accused: PeerId,
    /// Timestamp of dispute
    pub timestamp: u64,
    /// Deadline for resolution
    pub deadline: u64,
    /// Evidence hash
    pub evidence_hash: Hash256,
    /// Votes received
    pub votes: HashMap<PeerId, DisputeVote>,
    /// Resolution
    pub resolution: Option<DisputeResolution>,
}

impl Dispute {
    /// Create new dispute
    pub fn new(
        dispute_type: DisputeType,
        accuser: PeerId,
        accused: PeerId,
        evidence: &[u8],
    ) -> Self {
        let timestamp = current_timestamp();
        let deadline = timestamp + DISPUTE_TIMEOUT.as_secs();
        
        let mut data = Vec::new();
        data.extend_from_slice(&accuser);
        data.extend_from_slice(&accused);
        data.extend_from_slice(&timestamp.to_le_bytes());
        data.extend_from_slice(evidence);
        
        Self {
            id: GameCrypto::hash(&data),
            dispute_type,
            accuser,
            accused,
            timestamp,
            deadline,
            evidence_hash: GameCrypto::hash(evidence),
            votes: HashMap::new(),
            resolution: None,
        }
    }
    
    /// Add a vote
    pub fn add_vote(&mut self, voter: PeerId, vote: DisputeVote) -> Result<(), Error> {
        // Check if already voted
        if self.votes.contains_key(&voter) {
            return Err(Error::InvalidState("Already voted on this dispute".into()));
        }
        
        // Check if dispute is still open
        if self.resolution.is_some() {
            return Err(Error::InvalidState("Dispute already resolved".into()));
        }
        
        // Check deadline
        if current_timestamp() > self.deadline {
            return Err(Error::InvalidState("Dispute voting period expired".into()));
        }
        
        self.votes.insert(voter, vote);
        Ok(())
    }
    
    /// Check if dispute has enough votes to resolve
    pub fn can_resolve(&self, min_votes: usize) -> bool {
        self.votes.len() >= min_votes && self.resolution.is_none()
    }
    
    /// Resolve dispute based on votes
    pub fn resolve(&mut self) -> DisputeResolution {
        if self.resolution.is_some() {
            return self.resolution.clone().unwrap();
        }
        
        let mut guilty_votes = 0;
        let mut innocent_votes = 0;
        let mut _invalid_votes = 0;
        
        for vote in self.votes.values() {
            match vote.verdict {
                Verdict::Guilty => guilty_votes += 1,
                Verdict::Innocent => innocent_votes += 1,
                Verdict::Invalid => _invalid_votes += 1,
            }
        }
        
        let total_votes = self.votes.len();
        let resolution = if guilty_votes > total_votes / 2 {
            DisputeResolution::Guilty {
                penalty_multiplier: 1.0 + (guilty_votes as f64 / total_votes as f64),
            }
        } else if innocent_votes > total_votes / 2 {
            DisputeResolution::Innocent {
                false_accusation: true,
            }
        } else {
            DisputeResolution::Invalid
        };
        
        self.resolution = Some(resolution.clone());
        resolution
    }
}

/// Dispute vote
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisputeVote {
    pub dispute_id: Hash256,
    pub voter: PeerId,
    pub verdict: Verdict,
    pub reasoning: Option<String>,
    pub timestamp: u64,
    pub signature: Vec<u8>,
}

/// Verdict options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Verdict {
    Guilty,
    Innocent,
    Invalid, // Dispute itself is invalid
}

/// Dispute resolution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DisputeResolution {
    /// Accused found guilty
    Guilty { penalty_multiplier: f64 },
    /// Accused found innocent
    Innocent { false_accusation: bool },
    /// Dispute invalid or insufficient evidence
    Invalid,
}

/// Reputation manager
pub struct ReputationManager {
    /// Reputation records by peer
    records: HashMap<PeerId, ReputationRecord>,
    /// Active disputes
    disputes: HashMap<Hash256, Dispute>,
    /// Minimum votes required for dispute resolution
    min_dispute_votes: usize,
}

impl ReputationManager {
    /// Create new reputation manager
    pub fn new(min_dispute_votes: usize) -> Self {
        Self {
            records: HashMap::new(),
            disputes: HashMap::new(),
            min_dispute_votes,
        }
    }
    
    /// Get or create reputation record
    pub fn get_or_create(&mut self, peer: PeerId) -> &mut ReputationRecord {
        self.records.entry(peer).or_insert_with(ReputationRecord::new)
    }
    
    /// Apply reputation event
    pub fn apply_event(&mut self, peer: PeerId, event: ReputationEvent) {
        self.get_or_create(peer).apply_event(event);
    }
    
    /// Check if peer can participate
    pub fn can_participate(&self, peer: &PeerId) -> bool {
        self.records.get(peer)
            .map(|r| r.can_play())
            .unwrap_or(true) // New peers can play
    }
    
    /// Check if peer can vote
    pub fn can_vote(&self, peer: &PeerId) -> bool {
        self.records.get(peer)
            .map(|r| r.can_vote())
            .unwrap_or(false) // New peers cannot vote
    }
    
    /// Get peer trust level
    pub fn get_trust_level(&self, peer: &PeerId) -> f64 {
        self.records.get(peer)
            .map(|r| r.trust_level())
            .unwrap_or(0.5) // Neutral for unknown peers
    }
    
    /// Raise a dispute
    pub fn raise_dispute(
        &mut self,
        dispute_type: DisputeType,
        accuser: PeerId,
        accused: PeerId,
        evidence: &[u8],
    ) -> Result<Hash256, Error> {
        // Check accuser can raise disputes
        if !self.can_participate(&accuser) {
            return Err(Error::InvalidState("Insufficient reputation to raise dispute".into()));
        }
        
        // Create dispute
        let dispute = Dispute::new(dispute_type, accuser, accused, evidence);
        let dispute_id = dispute.id;
        
        // Track dispute raised
        self.get_or_create(accuser).disputes_raised += 1;
        
        // Store dispute
        self.disputes.insert(dispute_id, dispute);
        
        Ok(dispute_id)
    }
    
    /// Vote on a dispute
    pub fn vote_on_dispute(
        &mut self,
        dispute_id: Hash256,
        voter: PeerId,
        vote: DisputeVote,
    ) -> Result<(), Error> {
        // Check voter can vote
        if !self.can_vote(&voter) {
            return Err(Error::InvalidState("Insufficient reputation to vote".into()));
        }
        
        // Get dispute
        let dispute = self.disputes.get_mut(&dispute_id)
            .ok_or_else(|| Error::InvalidState("Dispute not found".into()))?;
        
        // Add vote
        dispute.add_vote(voter, vote)?;
        
        // Check if ready to resolve
        if dispute.can_resolve(self.min_dispute_votes) {
            self.resolve_dispute(dispute_id)?;
        }
        
        Ok(())
    }
    
    /// Resolve a dispute
    fn resolve_dispute(&mut self, dispute_id: Hash256) -> Result<(), Error> {
        let dispute = self.disputes.get_mut(&dispute_id)
            .ok_or_else(|| Error::InvalidState("Dispute not found".into()))?;
        
        let resolution = dispute.resolve();
        let accuser = dispute.accuser;
        let accused = dispute.accused;
        
        // Apply reputation changes based on resolution
        match resolution {
            DisputeResolution::Guilty { penalty_multiplier } => {
                // Accused is guilty
                self.apply_event(accused, ReputationEvent::DisputeLost { dispute_id });
                
                // Apply additional penalty based on severity
                let extra_penalty = (REP_CHEATING_ATTEMPT as f64 * penalty_multiplier) as i64;
                if let Some(record) = self.records.get_mut(&accused) {
                    record.score = (record.score + extra_penalty).clamp(MIN_REPUTATION, MAX_REPUTATION);
                }
                
                // Reward accuser
                self.apply_event(accuser, ReputationEvent::DisputeWon { dispute_id });
            },
            DisputeResolution::Innocent { false_accusation } => {
                // Accused is innocent
                if false_accusation {
                    // Penalize false accuser
                    self.apply_event(accuser, ReputationEvent::FalseAccusation { dispute_id });
                }
                
                // Small reputation boost for accused
                if let Some(record) = self.records.get_mut(&accused) {
                    record.score = (record.score + 10).clamp(MIN_REPUTATION, MAX_REPUTATION);
                }
            },
            DisputeResolution::Invalid => {
                // No reputation changes
            }
        }
        
        Ok(())
    }
    
    /// Clean up expired disputes
    pub fn cleanup_expired(&mut self) {
        let now = current_timestamp();
        let expired: Vec<Hash256> = self.disputes.iter()
            .filter(|(_, d)| now > d.deadline && d.resolution.is_none())
            .map(|(id, _)| *id)
            .collect();
        
        for id in expired {
            // Auto-resolve as invalid
            if let Some(dispute) = self.disputes.get_mut(&id) {
                dispute.resolution = Some(DisputeResolution::Invalid);
            }
        }
    }
    
    /// Get reputation leaderboard
    pub fn get_leaderboard(&self, limit: usize) -> Vec<(PeerId, i64)> {
        let mut scores: Vec<(PeerId, i64)> = self.records.iter()
            .map(|(peer, record)| (*peer, record.score))
            .collect();
        
        scores.sort_by_key(|(_, score)| -score);
        scores.truncate(limit);
        scores
    }
}

/// Get current timestamp in seconds
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_reputation_events() {
        let mut record = ReputationRecord::new();
        assert_eq!(record.score, INITIAL_REPUTATION);
        
        record.apply_event(ReputationEvent::GameCompleted);
        assert_eq!(record.score, INITIAL_REPUTATION + REP_SUCCESSFUL_GAME);
        
        record.apply_event(ReputationEvent::FailedCommit);
        assert_eq!(record.score, INITIAL_REPUTATION + REP_SUCCESSFUL_GAME + REP_FAILED_COMMIT);
        
        assert!(record.can_play());
        assert!(record.can_vote());
    }
    
    #[test]
    fn test_dispute_creation() {
        let accuser = [1; 32];
        let accused = [2; 32];
        let evidence = b"proof of cheating";
        
        let dispute = Dispute::new(
            DisputeType::Cheating {
                description: "Invalid state transition".to_string(),
                evidence: evidence.to_vec(),
            },
            accuser,
            accused,
            evidence,
        );
        
        assert_eq!(dispute.accuser, accuser);
        assert_eq!(dispute.accused, accused);
        assert!(dispute.votes.is_empty());
        assert!(dispute.resolution.is_none());
    }
    
    #[test]
    fn test_dispute_voting() {
        let mut dispute = Dispute::new(
            DisputeType::Cheating {
                description: "Test".to_string(),
                evidence: vec![],
            },
            [1; 32],
            [2; 32],
            &[],
        );
        
        // Add votes
        for i in 0..5 {
            let vote = DisputeVote {
                dispute_id: dispute.id,
                voter: [i; 32],
                verdict: if i < 3 { Verdict::Guilty } else { Verdict::Innocent },
                reasoning: None,
                timestamp: current_timestamp(),
                signature: vec![0; 64],
            };
            
            assert!(dispute.add_vote([i; 32], vote).is_ok());
        }
        
        // Resolve
        let resolution = dispute.resolve();
        match resolution {
            DisputeResolution::Guilty { .. } => {
                // Majority voted guilty
            },
            _ => panic!("Expected guilty verdict"),
        }
    }
}