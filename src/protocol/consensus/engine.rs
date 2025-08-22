//! Core consensus engine implementation

use std::collections::HashSet;
use rustc_hash::FxHashMap;
use std::time::SystemTime;
use lru::LruCache;
use std::num::NonZeroUsize;
use serde::{Serialize, Deserialize};

use crate::protocol::{PeerId, GameId, Hash256, Signature};
use crate::protocol::craps::{CrapsGame, GamePhase, BetResolution, Bet, DiceRoll, CrapTokens};
use crate::error::{Error, Result};

use super::{ConsensusConfig, ConsensusMetrics, CompactSignature};
use super::voting::{VoteTracker, ConfirmationTracker, Fork};
use super::commit_reveal::{RandomnessCommit, RandomnessReveal, EntropyPool};
use super::validation::{Dispute, DisputeClaim, DisputeEvidence, DisputeVote};
use super::{ProposalId, RoundId, StateHash, DisputeId};

/// Main consensus engine for BitCraps
pub struct ConsensusEngine {
    config: ConsensusConfig,
    _game_id: GameId,
    participants: Vec<PeerId>,
    local_peer_id: PeerId,
    
    // Current consensus state
    current_state: GameConsensusState,
    pending_proposals: FxHashMap<ProposalId, GameProposal>,
    
    // Voting and confirmation tracking
    votes: FxHashMap<ProposalId, VoteTracker>,
    confirmations: FxHashMap<StateHash, ConfirmationTracker>,
    
    // Fork management
    forks: FxHashMap<StateHash, Fork>,
    canonical_chain: Vec<StateHash>,
    
    // Commit-reveal for randomness
    dice_commits: FxHashMap<RoundId, FxHashMap<PeerId, RandomnessCommit>>,
    dice_reveals: FxHashMap<RoundId, FxHashMap<PeerId, RandomnessReveal>>,
    
    // Dispute tracking
    active_disputes: FxHashMap<DisputeId, Dispute>,
    dispute_votes: FxHashMap<DisputeId, FxHashMap<PeerId, DisputeVote>>,
    
    // Performance tracking
    consensus_metrics: ConsensusMetrics,
    
    // Signature caching for performance
    signature_cache: LruCache<Hash256, bool>,
    
    // Entropy pool for secure randomness
    entropy_pool: EntropyPool,
    
    // Compact signature cache
    compact_signatures: FxHashMap<Hash256, CompactSignature>,
}

/// Game consensus state
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GameConsensusState {
    pub game_id: GameId,
    pub state_hash: StateHash,
    pub sequence_number: u64,
    pub timestamp: u64,
    
    // Core game state
    pub game_state: CrapsGame,
    pub player_balances: FxHashMap<PeerId, CrapTokens>,
    
    // Consensus metadata
    pub last_proposer: PeerId,
    pub confirmations: u32,
    pub is_finalized: bool,
}

/// Game state proposal for consensus
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameProposal {
    pub id: ProposalId,
    pub proposer: PeerId,
    pub previous_state_hash: StateHash,
    pub proposed_state: GameConsensusState,
    pub operation: GameOperation,
    pub timestamp: u64,
    pub signature: Signature,
}

/// Operations that can be proposed to change game state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GameOperation {
    PlaceBet { 
        player: PeerId, 
        bet: Bet,
        nonce: u64,
    },
    CommitRandomness {
        player: PeerId,
        round_id: RoundId,
        commitment: Hash256,
    },
    RevealRandomness {
        player: PeerId,
        round_id: RoundId,
        nonce: [u8; 32],
    },
    ProcessRoll {
        round_id: RoundId,
        dice_roll: DiceRoll,
        entropy_proof: Vec<Hash256>,
    },
    ResolvePhase {
        new_phase: GamePhase,
        resolutions: Vec<BetResolution>,
    },
    UpdateBalances {
        changes: FxHashMap<PeerId, CrapTokens>,
        reason: String,
    },
}

impl ConsensusEngine {
    /// Create new consensus engine
    pub fn new(
        game_id: GameId,
        participants: Vec<PeerId>,
        local_peer_id: PeerId,
        config: ConsensusConfig
    ) -> Result<Self> {
        let cache_size = NonZeroUsize::new(10000).unwrap();
        
        // Initialize genesis state
        let genesis_state = GameConsensusState {
            game_id,
            state_hash: [0u8; 32], // Will be calculated
            sequence_number: 0,
            timestamp: SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            game_state: CrapsGame::new(game_id, local_peer_id),
            player_balances: participants.iter()
                .map(|&p| (p, CrapTokens::new_unchecked(1000)))
                .collect(),
            last_proposer: local_peer_id,
            confirmations: 0,
            is_finalized: false,
        };
        
        Ok(Self {
            config,
            _game_id: game_id,
            participants,
            local_peer_id,
            current_state: genesis_state,
            pending_proposals: FxHashMap::default(),
            votes: FxHashMap::default(),
            confirmations: FxHashMap::default(),
            forks: FxHashMap::default(),
            canonical_chain: Vec::new(),
            dice_commits: FxHashMap::default(),
            dice_reveals: FxHashMap::default(),
            active_disputes: FxHashMap::default(),
            dispute_votes: FxHashMap::default(),
            consensus_metrics: ConsensusMetrics::default(),
            signature_cache: LruCache::new(cache_size),
            entropy_pool: EntropyPool::new(),
            compact_signatures: FxHashMap::default(),
        })
    }
    
    /// Submit a new proposal for consensus
    pub fn submit_proposal(&mut self, operation: GameOperation) -> Result<ProposalId> {
        // Create new proposal
        let proposal_id = self.generate_proposal_id(&operation);
        let timestamp = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        // Calculate proposed state after operation
        let proposed_state = self.apply_operation_to_state(&self.current_state, &operation)?;
        
        // Create signature (simplified)
        let signature = Signature([0u8; 64]); // Would implement proper signing
        
        let proposal = GameProposal {
            id: proposal_id,
            proposer: self.local_peer_id,
            previous_state_hash: self.current_state.state_hash,
            proposed_state,
            operation,
            timestamp,
            signature,
        };
        
        // Add to pending proposals
        self.pending_proposals.insert(proposal_id, proposal);
        
        // Initialize vote tracker
        let vote_tracker = VoteTracker {
            proposal_id,
            votes_for: HashSet::new(),
            votes_against: HashSet::new(),
            abstentions: HashSet::new(),
            created_at: SystemTime::now(),
        };
        self.votes.insert(proposal_id, vote_tracker);
        
        Ok(proposal_id)
    }
    
    /// Vote on a proposal
    pub fn vote_on_proposal(&mut self, proposal_id: ProposalId, vote: bool) -> Result<()> {
        if let Some(vote_tracker) = self.votes.get_mut(&proposal_id) {
            if vote {
                vote_tracker.votes_for.insert(self.local_peer_id);
                vote_tracker.votes_against.remove(&self.local_peer_id);
            } else {
                vote_tracker.votes_against.insert(self.local_peer_id);
                vote_tracker.votes_for.remove(&self.local_peer_id);
            }
            
            // Check if proposal has enough votes
            self.check_proposal_consensus(proposal_id)?;
        }
        
        Ok(())
    }
    
    /// Check if a proposal has reached consensus
    fn check_proposal_consensus(&mut self, proposal_id: ProposalId) -> Result<()> {
        if let Some(vote_tracker) = self.votes.get(&proposal_id) {
            let total_participants = self.participants.len();
            let required_votes = (total_participants * 2) / 3 + 1; // 2/3 majority
            
            if vote_tracker.votes_for.len() >= required_votes {
                // Proposal accepted
                self.finalize_proposal(proposal_id)?;
            } else if vote_tracker.votes_against.len() > total_participants / 3 {
                // Proposal rejected
                self.reject_proposal(proposal_id)?;
            }
        }
        
        Ok(())
    }
    
    /// Finalize an accepted proposal
    fn finalize_proposal(&mut self, proposal_id: ProposalId) -> Result<()> {
        if let Some(proposal) = self.pending_proposals.remove(&proposal_id) {
            // Update current state
            self.current_state = proposal.proposed_state;
            self.current_state.is_finalized = true;
            
            // Add to canonical chain
            self.canonical_chain.push(self.current_state.state_hash);
            
            // Clean up vote tracker
            self.votes.remove(&proposal_id);
            
            // Update metrics
            self.consensus_metrics.rounds_completed += 1;
        }
        
        Ok(())
    }
    
    /// Reject a proposal
    fn reject_proposal(&mut self, proposal_id: ProposalId) -> Result<()> {
        self.pending_proposals.remove(&proposal_id);
        self.votes.remove(&proposal_id);
        self.consensus_metrics.rounds_failed += 1;
        Ok(())
    }
    
    /// Apply operation to state (simplified)
    fn apply_operation_to_state(&self, state: &GameConsensusState, operation: &GameOperation) -> Result<GameConsensusState> {
        let mut new_state = state.clone();
        new_state.sequence_number += 1;
        new_state.timestamp = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        match operation {
            GameOperation::PlaceBet { player, bet, .. } => {
                // Would implement bet placement logic
                if let Some(balance) = new_state.player_balances.get_mut(player) {
                    if balance.amount >= bet.amount.amount {
                        *balance = CrapTokens::new_unchecked(balance.amount - bet.amount.amount);
                    }
                }
            },
            GameOperation::ProcessRoll { dice_roll, .. } => {
                // Would implement dice roll processing
                let _resolutions = new_state.game_state.process_roll(*dice_roll);
            },
            GameOperation::UpdateBalances { changes, .. } => {
                for (player, change) in changes {
                    if let Some(balance) = new_state.player_balances.get_mut(player) {
                        *balance = CrapTokens::new_unchecked(balance.amount.saturating_add(change.amount));
                    }
                }
            },
            _ => {
                // Handle other operations
            }
        }
        
        // Recalculate state hash
        new_state.state_hash = self.calculate_state_hash(&new_state)?;
        
        Ok(new_state)
    }
    
    /// Generate proposal ID
    fn generate_proposal_id(&self, operation: &GameOperation) -> ProposalId {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        
        hasher.update(&self.local_peer_id);
        hasher.update(&self.current_state.state_hash);
        
        // Add operation-specific data
        match operation {
            GameOperation::PlaceBet { player, bet, nonce } => {
                hasher.update(b"place_bet");
                hasher.update(player);
                hasher.update(&bet.amount.amount.to_le_bytes());
                hasher.update(&nonce.to_le_bytes());
            },
            GameOperation::ProcessRoll { round_id, dice_roll, .. } => {
                hasher.update(b"process_roll");
                hasher.update(&round_id.to_le_bytes());
                hasher.update(&[dice_roll.die1, dice_roll.die2]);
            },
            _ => {
                hasher.update(b"other_operation");
            }
        }
        
        hasher.finalize().into()
    }
    
    /// Calculate state hash
    fn calculate_state_hash(&self, state: &GameConsensusState) -> Result<Hash256> {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        
        hasher.update(&state.game_id);
        hasher.update(&state.sequence_number.to_le_bytes());
        hasher.update(&state.timestamp.to_le_bytes());
        
        // Add game state data
        hasher.update(&format!("{:?}", state.game_state.phase));
        
        // Add balance data
        for (&player, &balance) in &state.player_balances {
            hasher.update(&player);
            hasher.update(&balance.amount.to_le_bytes());
        }
        
        Ok(hasher.finalize().into())
    }
    
    /// Get current consensus state
    pub fn get_current_state(&self) -> &GameConsensusState {
        &self.current_state
    }
    
    /// Get consensus metrics
    pub fn get_metrics(&self) -> &ConsensusMetrics {
        &self.consensus_metrics
    }
    
    /// Get active proposals
    pub fn get_pending_proposals(&self) -> &FxHashMap<ProposalId, GameProposal> {
        &self.pending_proposals
    }
    
    /// Propose a new operation for consensus
    pub fn propose_operation(&mut self, operation: GameOperation) -> Result<ProposalId> {
        // Calculate new state after applying operation
        let mut new_state = self.current_state.clone();
        self.apply_operation_to_state_mut(&mut new_state, &operation)?;
        
        // Create proposal
        let proposal_id = self.generate_proposal_id(&operation);
        let proposal = GameProposal {
            id: proposal_id,
            proposer: self.local_peer_id,
            previous_state_hash: self.current_state.state_hash,
            proposed_state: new_state,
            operation,
            timestamp: self.current_timestamp(),
            signature: self.sign_proposal(&proposal_id)?,
        };

        // Add to pending proposals
        self.pending_proposals.insert(proposal_id, proposal);
        
        // Initialize vote tracker
        self.votes.insert(proposal_id, VoteTracker {
            proposal_id,
            votes_for: HashSet::new(),
            votes_against: HashSet::new(),
            abstentions: HashSet::new(),
            created_at: SystemTime::now(),
        });

        self.consensus_metrics.rounds_completed += 1;
        
        Ok(proposal_id)
    }

    /// Process a proposal from another participant
    pub fn process_proposal(&mut self, proposal: GameProposal) -> Result<bool> {
        // Verify proposal signature
        if !self.verify_proposal_signature(&proposal)? {
            return Ok(false);
        }

        // Check if proposal is for current state
        if proposal.previous_state_hash != self.current_state.state_hash {
            // Handle potential fork
            return self.handle_fork(&proposal);
        }

        // Validate the operation
        if !self.validate_operation(&proposal.operation)? {
            self.vote_on_proposal(proposal.id, false)?;
            return Ok(false);
        }

        // Store proposal and vote positively
        let proposal_id = proposal.id;
        self.pending_proposals.insert(proposal_id, proposal);
        self.vote_on_proposal(proposal_id, true)?;
        
        Ok(true)
    }
    
    /// Start commit phase for dice roll randomness
    pub fn start_dice_commit_phase(&mut self, round_id: RoundId) -> Result<Hash256> {
        // Generate cryptographically secure nonce from entropy pool
        let nonce_bytes = self.entropy_pool.generate_bytes(32);
        let mut nonce = [0u8; 32];
        nonce.copy_from_slice(&nonce_bytes);

        // Add our own entropy contribution
        self.entropy_pool.add_entropy(nonce);

        // Create commitment
        let commitment = self.create_randomness_commitment(round_id, &nonce)?;
        
        // Store our commitment
        let commit = RandomnessCommit {
            player: self.local_peer_id,
            round_id,
            commitment,
            timestamp: self.current_timestamp(),
            signature: self.sign_randomness_commit(round_id, &commitment)?,
        };

        self.dice_commits.entry(round_id).or_insert_with(FxHashMap::default)
            .insert(self.local_peer_id, commit);

        Ok(commitment)
    }
    
    /// Raise a dispute about game state
    pub fn raise_dispute(&mut self, claim: DisputeClaim, evidence: Vec<DisputeEvidence>) -> Result<DisputeId> {
        let dispute_id = self.generate_dispute_id(&claim)?;
        let dispute = Dispute {
            id: dispute_id,
            disputer: self.local_peer_id,
            disputed_state: self.current_state.state_hash,
            claim,
            evidence,
            created_at: self.current_timestamp(),
            resolution_deadline: self.current_timestamp() + 3600, // 1 hour
        };

        self.active_disputes.insert(dispute_id, dispute);
        
        Ok(dispute_id)
    }

    /// Vote on a dispute
    pub fn vote_on_dispute(
        &mut self, 
        dispute_id: DisputeId, 
        vote: super::validation::DisputeVoteType, 
        rationale: String
    ) -> Result<()> {
        let dispute_vote = DisputeVote {
            voter: self.local_peer_id,
            dispute_id,
            vote,
            reasoning: rationale,
            timestamp: self.current_timestamp(),
            signature: self.sign_dispute_vote(dispute_id)?,
        };

        self.dispute_votes.entry(dispute_id).or_insert_with(FxHashMap::default)
            .insert(self.local_peer_id, dispute_vote);

        // Check if dispute can be resolved
        self.check_dispute_resolution(dispute_id)?;
        
        Ok(())
    }
    
    /// Check if consensus is healthy
    pub fn is_consensus_healthy(&self) -> bool {
        self.active_disputes.is_empty() && 
        self.forks.is_empty() &&
        self.pending_proposals.len() < 10
    }
    
    // Helper methods
    
    /// Apply operation to state mutably (helper for propose_operation)
    fn apply_operation_to_state_mut(&self, state: &mut GameConsensusState, operation: &GameOperation) -> Result<()> {
        state.sequence_number += 1;
        state.timestamp = self.current_timestamp();
        
        match operation {
            GameOperation::PlaceBet { player, bet, .. } => {
                if let Some(balance) = state.player_balances.get_mut(player) {
                    if balance.amount >= bet.amount.amount {
                        *balance = CrapTokens::new_unchecked(balance.amount - bet.amount.amount);
                    }
                }
            },
            GameOperation::ProcessRoll { dice_roll, .. } => {
                let _resolutions = state.game_state.process_roll(*dice_roll);
            },
            GameOperation::UpdateBalances { changes, .. } => {
                for (player, change) in changes {
                    if let Some(balance) = state.player_balances.get_mut(player) {
                        *balance = CrapTokens::new_unchecked(balance.amount.saturating_add(change.amount));
                    }
                }
            },
            _ => {
                // Handle other operations
            }
        }
        
        // Recalculate state hash
        state.state_hash = self.calculate_state_hash(state)?;
        
        Ok(())
    }
    
    /// Get current timestamp
    fn current_timestamp(&self) -> u64 {
        SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
    
    /// Sign a proposal (simplified implementation)
    fn sign_proposal(&self, _proposal_id: &ProposalId) -> Result<Signature> {
        // Simplified signature - in production would use proper cryptographic signing
        Ok(Signature([0u8; 64]))
    }
    
    /// Verify proposal signature (simplified implementation)
    fn verify_proposal_signature(&self, _proposal: &GameProposal) -> Result<bool> {
        // Simplified verification - in production would use proper cryptographic verification
        Ok(true)
    }
    
    /// Handle potential fork
    fn handle_fork(&mut self, _proposal: &GameProposal) -> Result<bool> {
        // For now, reject forks - in production would implement proper fork handling
        Ok(false)
    }
    
    /// Validate operation
    fn validate_operation(&self, _operation: &GameOperation) -> Result<bool> {
        // Simplified validation - in production would implement proper validation
        Ok(true)
    }
    
    /// Create randomness commitment
    fn create_randomness_commitment(&self, round_id: RoundId, nonce: &[u8; 32]) -> Result<Hash256> {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(&round_id.to_le_bytes());
        hasher.update(nonce);
        Ok(hasher.finalize().into())
    }
    
    /// Sign randomness commit
    fn sign_randomness_commit(&self, _round_id: RoundId, _commitment: &Hash256) -> Result<Signature> {
        // Simplified signature
        Ok(Signature([0u8; 64]))
    }
    
    /// Generate dispute ID
    fn generate_dispute_id(&self, claim: &DisputeClaim) -> Result<DisputeId> {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(&self.local_peer_id);
        hasher.update(&self.current_timestamp().to_le_bytes());
        // Add claim-specific data
        match claim {
            DisputeClaim::InvalidBet { .. } => hasher.update(b"invalid_bet"),
            DisputeClaim::InvalidRoll { .. } => hasher.update(b"invalid_roll"),
            DisputeClaim::InvalidPayout { .. } => hasher.update(b"invalid_payout"),
            DisputeClaim::DoubleSpending { .. } => hasher.update(b"double_spending"),
            DisputeClaim::ConsensusViolation { .. } => hasher.update(b"consensus_violation"),
        }
        Ok(hasher.finalize().into())
    }
    
    /// Sign dispute vote
    fn sign_dispute_vote(&self, _dispute_id: DisputeId) -> Result<Signature> {
        // Simplified signature
        Ok(Signature([0u8; 64]))
    }
    
    /// Check dispute resolution
    fn check_dispute_resolution(&mut self, dispute_id: DisputeId) -> Result<()> {
        let votes = self.dispute_votes.get(&dispute_id);
        if let Some(votes) = votes {
            let total_participants = self.participants.len();
            let required_votes = (total_participants * 2) / 3 + 1;

            let uphold_votes = votes.values().filter(|v| matches!(v.vote, super::validation::DisputeVoteType::Uphold)).count();
            let dismiss_votes = votes.values().filter(|v| matches!(v.vote, super::validation::DisputeVoteType::Reject)).count();

            if uphold_votes >= required_votes {
                self.resolve_dispute(dispute_id, true)?;
            } else if dismiss_votes >= required_votes {
                self.resolve_dispute(dispute_id, false)?;
            }
        }
        Ok(())
    }
    
    /// Resolve dispute
    fn resolve_dispute(&mut self, dispute_id: DisputeId, _upheld: bool) -> Result<()> {
        self.active_disputes.remove(&dispute_id);
        self.dispute_votes.remove(&dispute_id);
        Ok(())
    }
}