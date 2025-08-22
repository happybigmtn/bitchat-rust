//! Vote tracking and fork management

use std::collections::{HashMap, HashSet};
use std::time::SystemTime;

use crate::protocol::PeerId;

use super::{ProposalId, StateHash};

/// Vote tracking for proposals
#[derive(Debug, Clone)]
pub struct VoteTracker {
    pub proposal_id: ProposalId,
    pub votes_for: HashSet<PeerId>,
    pub votes_against: HashSet<PeerId>,
    pub abstentions: HashSet<PeerId>,
    pub created_at: SystemTime,
}

/// Confirmation tracking for finalized states
#[derive(Debug, Clone)]
pub struct ConfirmationTracker {
    pub state_hash: StateHash,
    pub confirmations: HashSet<PeerId>,
    pub rejections: HashSet<PeerId>,
    pub finalized_at: Option<SystemTime>,
}

/// Fork representation for conflicting states
#[derive(Debug, Clone)]
pub struct Fork {
    pub fork_id: StateHash,
    pub parent_state: StateHash,
    pub competing_states: Vec<StateHash>,
    pub supporters: HashMap<StateHash, HashSet<PeerId>>,
    pub created_at: SystemTime,
    pub resolution_deadline: SystemTime,
}

impl VoteTracker {
    /// Create new vote tracker
    pub fn new(proposal_id: ProposalId) -> Self {
        Self {
            proposal_id,
            votes_for: HashSet::new(),
            votes_against: HashSet::new(),
            abstentions: HashSet::new(),
            created_at: SystemTime::now(),
        }
    }
    
    /// Add vote for proposal
    pub fn add_vote_for(&mut self, voter: PeerId) {
        self.votes_for.insert(voter);
        self.votes_against.remove(&voter);
        self.abstentions.remove(&voter);
    }
    
    /// Add vote against proposal
    pub fn add_vote_against(&mut self, voter: PeerId) {
        self.votes_against.insert(voter);
        self.votes_for.remove(&voter);
        self.abstentions.remove(&voter);
    }
    
    /// Add abstention
    pub fn add_abstention(&mut self, voter: PeerId) {
        self.abstentions.insert(voter);
        self.votes_for.remove(&voter);
        self.votes_against.remove(&voter);
    }
    
    /// Get total votes cast
    pub fn total_votes(&self) -> usize {
        self.votes_for.len() + self.votes_against.len() + self.abstentions.len()
    }
    
    /// Check if proposal passes with given threshold
    pub fn passes_threshold(&self, total_participants: usize, threshold_ratio: f32) -> bool {
        let required_votes = ((total_participants as f32) * threshold_ratio).ceil() as usize;
        self.votes_for.len() >= required_votes
    }
}

impl ConfirmationTracker {
    /// Create new confirmation tracker
    pub fn new(state_hash: StateHash) -> Self {
        Self {
            state_hash,
            confirmations: HashSet::new(),
            rejections: HashSet::new(),
            finalized_at: None,
        }
    }
    
    /// Add confirmation from peer
    pub fn add_confirmation(&mut self, peer: PeerId) {
        self.confirmations.insert(peer);
        self.rejections.remove(&peer);
    }
    
    /// Add rejection from peer
    pub fn add_rejection(&mut self, peer: PeerId) {
        self.rejections.insert(peer);
        self.confirmations.remove(&peer);
    }
    
    /// Check if state is confirmed
    pub fn is_confirmed(&self, min_confirmations: usize) -> bool {
        self.confirmations.len() >= min_confirmations
    }
    
    /// Finalize the state
    pub fn finalize(&mut self) {
        self.finalized_at = Some(SystemTime::now());
    }
}

impl Fork {
    /// Create new fork
    pub fn new(fork_id: StateHash, parent_state: StateHash) -> Self {
        let resolution_deadline = SystemTime::now() + std::time::Duration::from_secs(300); // 5 minutes
        
        Self {
            fork_id,
            parent_state,
            competing_states: Vec::new(),
            supporters: HashMap::new(),
            created_at: SystemTime::now(),
            resolution_deadline,
        }
    }
    
    /// Add competing state
    pub fn add_competing_state(&mut self, state_hash: StateHash) {
        if !self.competing_states.contains(&state_hash) {
            self.competing_states.push(state_hash);
            self.supporters.insert(state_hash, HashSet::new());
        }
    }
    
    /// Add supporter for a state
    pub fn add_supporter(&mut self, state_hash: StateHash, supporter: PeerId) {
        if let Some(supporters) = self.supporters.get_mut(&state_hash) {
            supporters.insert(supporter);
        }
    }
    
    /// Get winning state (most supporters)
    pub fn get_winning_state(&self) -> Option<StateHash> {
        self.supporters
            .iter()
            .max_by_key(|(_, supporters)| supporters.len())
            .map(|(&state_hash, _)| state_hash)
    }
    
    /// Check if fork is expired
    pub fn is_expired(&self) -> bool {
        SystemTime::now() > self.resolution_deadline
    }
}