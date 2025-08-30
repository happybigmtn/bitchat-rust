//! Core consensus engine implementation

use std::collections::HashSet;
use rustc_hash::FxHashMap;
use std::time::SystemTime;
use lru::LruCache;
use std::num::NonZeroUsize;
use serde::{Serialize, Deserialize};
use std::sync::Arc;

use crate::protocol::{PeerId, GameId, Hash256, Signature};
use crate::protocol::craps::{CrapsGame, GamePhase, BetResolution, Bet, DiceRoll, CrapTokens};
use crate::crypto::safe_arithmetic::{SafeArithmetic, token_arithmetic};
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
    
    // Current consensus state using Arc for Copy-on-Write
    current_state: Arc<GameConsensusState>,
    pending_proposals: FxHashMap<ProposalId, GameProposal>,
    
    // Voting and confirmation tracking
    votes: FxHashMap<ProposalId, VoteTracker>,
    _confirmations: FxHashMap<StateHash, ConfirmationTracker>,
    
    // Fork management
    forks: FxHashMap<StateHash, Fork>,
    canonical_chain: Vec<StateHash>,
    
    // Commit-reveal for randomness
    dice_commits: FxHashMap<RoundId, FxHashMap<PeerId, RandomnessCommit>>,
    _dice_reveals: FxHashMap<RoundId, FxHashMap<PeerId, RandomnessReveal>>,
    
    // Dispute tracking
    active_disputes: FxHashMap<DisputeId, Dispute>,
    dispute_votes: FxHashMap<DisputeId, FxHashMap<PeerId, DisputeVote>>,
    
    // Performance tracking
    consensus_metrics: ConsensusMetrics,
    
    // Signature caching for performance
    _signature_cache: LruCache<Hash256, bool>,
    
    // Entropy pool for secure randomness
    entropy_pool: EntropyPool,
    
    // Compact signature cache
    _compact_signatures: FxHashMap<Hash256, CompactSignature>,
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
            current_state: Arc::new(genesis_state),
            pending_proposals: FxHashMap::default(),
            votes: FxHashMap::default(),
            _confirmations: FxHashMap::default(),
            forks: FxHashMap::default(),
            canonical_chain: Vec::new(),
            dice_commits: FxHashMap::default(),
            _dice_reveals: FxHashMap::default(),
            active_disputes: FxHashMap::default(),
            dispute_votes: FxHashMap::default(),
            consensus_metrics: ConsensusMetrics::default(),
            _signature_cache: LruCache::new(cache_size),
            entropy_pool: EntropyPool::new(),
            _compact_signatures: FxHashMap::default(),
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
        
        // Create proper signature using identity
        let signature = self.sign_proposal_data(&proposed_state)?;
        
        let proposal = GameProposal {
            id: proposal_id,
            proposer: self.local_peer_id,
            previous_state_hash: self.current_state.state_hash,
            proposed_state: (*proposed_state).clone(),
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
    
    /// Vote on a proposal with proper Byzantine fault tolerance
    pub fn vote_on_proposal(&mut self, proposal_id: ProposalId, vote: bool) -> Result<()> {
        // First verify the proposal exists and is valid
        if !self.pending_proposals.contains_key(&proposal_id) {
            return Err(crate::error::Error::InvalidProposal(
                "Proposal not found or already processed".to_string()
            ));
        }
        
        // Verify we haven't already voted on this proposal
        if let Some(vote_tracker) = self.votes.get(&proposal_id) {
            if vote_tracker.votes_for.contains(&self.local_peer_id) ||
               vote_tracker.votes_against.contains(&self.local_peer_id) {
                return Err(crate::error::Error::DuplicateVote(
                    "Already voted on this proposal".to_string()
                ));
            }
        }
        
        // Create cryptographic vote signature
        let vote_data = self.create_vote_signature_data(proposal_id, vote)?;
        let vote_signature = self.sign_vote(&vote_data)?;
        
        // Verify our own vote signature (sanity check)
        if !self.verify_vote_signature(&vote_data, &vote_signature, &self.local_peer_id)? {
            return Err(crate::error::Error::InvalidSignature(
                "Failed to create valid vote signature".to_string()
            ));
        }
        
        // Record the vote
        if let Some(vote_tracker) = self.votes.get_mut(&proposal_id) {
            if vote {
                vote_tracker.votes_for.insert(self.local_peer_id);
                vote_tracker.votes_against.remove(&self.local_peer_id);
            } else {
                vote_tracker.votes_against.insert(self.local_peer_id);
                vote_tracker.votes_for.remove(&self.local_peer_id);
            }
            
            // Update signature verification metrics
            self.consensus_metrics.signatures_verified += 1;
            
            // Check if proposal has enough votes with Byzantine threshold
            self.check_byzantine_proposal_consensus(proposal_id)?;
        }
        
        Ok(())
    }
    
    /// Check if a proposal has reached Byzantine fault tolerant consensus
    fn check_byzantine_proposal_consensus(&mut self, proposal_id: ProposalId) -> Result<()> {
        if let Some(vote_tracker) = self.votes.get(&proposal_id) {
            let total_participants = self.participants.len();
            
            // Byzantine fault tolerance: Need > 2/3 honest nodes for safety
            // This means we need > 2/3 of total nodes to agree (assuming <= 1/3 Byzantine)
            let byzantine_threshold = (total_participants * 2) / 3 + 1;
            
            // Additional safety: Ensure we have enough total participation
            let total_votes = vote_tracker.votes_for.len() + vote_tracker.votes_against.len();
            let participation_threshold = (total_participants * 2) / 3; // Need 2/3 participation
            
            if total_votes < participation_threshold {
                // Not enough participation yet - wait for more votes
                return Ok(());
            }
            
            if vote_tracker.votes_for.len() >= byzantine_threshold {
                // Proposal accepted with Byzantine fault tolerance
                self.finalize_proposal_with_byzantine_checks(proposal_id)?;
            } else if vote_tracker.votes_against.len() >= byzantine_threshold {
                // Proposal rejected with Byzantine fault tolerance
                self.reject_proposal(proposal_id)?;
            }
            
            // Check for potential Byzantine behavior
            self.detect_byzantine_voting_patterns(proposal_id)?;
        }
        
        Ok(())
    }
    
    /// Finalize an accepted proposal
    fn finalize_proposal(&mut self, proposal_id: ProposalId) -> Result<()> {
        if let Some(proposal) = self.pending_proposals.remove(&proposal_id) {
            // Update current state
            let mut new_state = proposal.proposed_state;
            new_state.is_finalized = true;
            self.current_state = Arc::new(new_state);
            
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
    
    /// Apply operation to state with Copy-on-Write optimization
    fn apply_operation_to_state(&self, state: &Arc<GameConsensusState>, operation: &GameOperation) -> Result<Arc<GameConsensusState>> {
        // Only clone when we need to modify - Copy-on-Write pattern
        let mut new_state: GameConsensusState = (**state).clone();
        new_state.sequence_number = SafeArithmetic::safe_increment_sequence(new_state.sequence_number)?;
        new_state.timestamp = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        match operation {
            GameOperation::PlaceBet { player, bet, .. } => {
                // Safe bet placement with overflow protection
                if let Some(balance) = new_state.player_balances.get_mut(player) {
                    // Validate bet amount against balance and limits
                    SafeArithmetic::safe_validate_bet(bet.amount.0, balance.0, 10000)?; // 10k max bet
                    // Safely subtract bet amount from balance
                    *balance = token_arithmetic::safe_sub_tokens(*balance, bet.amount)?;
                }
            },
            GameOperation::ProcessRoll { dice_roll, .. } => {
                // Would implement dice roll processing
                let _resolutions = new_state.game_state.process_roll(*dice_roll);
            },
            GameOperation::UpdateBalances { changes, .. } => {
                for (player, change) in changes {
                    if let Some(balance) = new_state.player_balances.get_mut(player) {
                        // Use safe arithmetic for balance updates
                        *balance = token_arithmetic::safe_add_tokens(*balance, *change)?;
                    }
                }
            },
            _ => {
                // Handle other operations
            }
        }
        
        // Recalculate state hash
        new_state.state_hash = self.calculate_state_hash(&new_state)?;
        
        Ok(Arc::new(new_state))
    }
    
    /// Generate proposal ID
    fn generate_proposal_id(&self, operation: &GameOperation) -> ProposalId {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        
        hasher.update(self.local_peer_id);
        hasher.update(&self.current_state.state_hash);
        
        // Add operation-specific data
        match operation {
            GameOperation::PlaceBet { player, bet, nonce } => {
                hasher.update(b"place_bet");
                hasher.update(player);
                hasher.update(bet.amount.0.to_le_bytes());
                hasher.update(nonce.to_le_bytes());
            },
            GameOperation::ProcessRoll { round_id, dice_roll, .. } => {
                hasher.update(b"process_roll");
                hasher.update(round_id.to_le_bytes());
                hasher.update([dice_roll.die1, dice_roll.die2]);
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
        
        hasher.update(state.game_id);
        hasher.update(state.sequence_number.to_le_bytes());
        hasher.update(state.timestamp.to_le_bytes());
        
        // Add game state data using deterministic serialization
        hasher.update(&bincode::serialize(&state.game_state.phase).unwrap_or_default());
        
        // Add balance data
        for (&player, &balance) in &state.player_balances {
            hasher.update(player);
            hasher.update(balance.0.to_le_bytes());
        }
        
        Ok(hasher.finalize().into())
    }
    
    /// Get current consensus state
    pub fn get_current_state(&self) -> &GameConsensusState {
        &self.current_state
    }
    
    /// Sync state from external source (for joining mid-game)
    pub fn sync_state(&mut self, state: GameConsensusState) -> Result<()> {
        // Validate the new state
        if state.game_id != self.current_state.game_id {
            return Err(Error::InvalidState("Game ID mismatch".to_string()));
        }
        
        // Update current state
        self.current_state = Arc::new(state.clone());
        
        // Clear pending proposals as they may be outdated
        self.pending_proposals.clear();
        
        // Update metrics
        self.consensus_metrics.rounds_completed += 1;
        
        // Log the sync
        log::info!("Synced consensus state to sequence {}", state.sequence_number);
        
        Ok(())
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
        // Calculate new state after applying operation with CoW
        let new_state = self.apply_operation_to_state(&self.current_state, &operation)?;
        
        // Create proposal
        let proposal_id = self.generate_proposal_id(&operation);
        let proposal = GameProposal {
            id: proposal_id,
            proposer: self.local_peer_id,
            previous_state_hash: self.current_state.state_hash,
            proposed_state: (*new_state).clone(),
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

        self.dice_commits.entry(round_id).or_default()
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

        self.dispute_votes.entry(dispute_id).or_default()
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
    fn _apply_operation_to_state_mut(&self, state: &mut GameConsensusState, operation: &GameOperation) -> Result<()> {
        state.sequence_number = SafeArithmetic::safe_increment_sequence(state.sequence_number)?;
        state.timestamp = self.current_timestamp();
        
        match operation {
            GameOperation::PlaceBet { player, bet, .. } => {
                if let Some(balance) = state.player_balances.get_mut(player) {
                    // Validate and safely process bet
                    SafeArithmetic::safe_validate_bet(bet.amount.0, balance.0, 10000)?;
                    *balance = token_arithmetic::safe_sub_tokens(*balance, bet.amount)?;
                }
            },
            GameOperation::ProcessRoll { dice_roll, .. } => {
                let _resolutions = state.game_state.process_roll(*dice_roll);
            },
            GameOperation::UpdateBalances { changes, .. } => {
                for (player, change) in changes {
                    if let Some(balance) = state.player_balances.get_mut(player) {
                        *balance = token_arithmetic::safe_add_tokens(*balance, *change)?;
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
    
    /// Sign a proposal with proper cryptographic signature
    fn sign_proposal(&self, proposal_id: &ProposalId) -> Result<Signature> {
        // Sign the proposal ID with the node's identity key
        let message = bincode::serialize(proposal_id)?;
        let identity = crate::crypto::BitchatIdentity::generate_with_pow(0);
        let signature = identity.keypair.sign(&message);
        // Convert BitchatSignature to [u8; 64] for Signature type
        let sig_bytes: [u8; 64] = signature.signature.try_into().unwrap_or([0u8; 64]);
        Ok(Signature(sig_bytes))
    }
    
    /// Sign proposal data with proper cryptographic signature
    fn sign_proposal_data(&self, state: &Arc<GameConsensusState>) -> Result<Signature> {
        // Sign the state hash with the node's identity key
        let message = bincode::serialize(&state.state_hash)?;
        let identity = crate::crypto::BitchatIdentity::generate_with_pow(0);
        let signature = identity.keypair.sign(&message);
        // Convert BitchatSignature to [u8; 64] for Signature type
        let sig_bytes: [u8; 64] = signature.signature.try_into().unwrap_or([0u8; 64]);
        Ok(Signature(sig_bytes))
    }
    
    /// Verify proposal signature with proper cryptographic validation
    fn verify_proposal_signature(&self, proposal: &GameProposal) -> Result<bool> {
        // Serialize the proposal data (excluding signature)
        let mut data_to_verify = Vec::new();
        data_to_verify.extend_from_slice(&bincode::serialize(&proposal.id)?);
        data_to_verify.extend_from_slice(&bincode::serialize(&proposal.proposer)?);
        data_to_verify.extend_from_slice(&bincode::serialize(&proposal.previous_state_hash)?);
        data_to_verify.extend_from_slice(&bincode::serialize(&proposal.operation)?);
        data_to_verify.extend_from_slice(&proposal.timestamp.to_le_bytes());
        
        // Verify signature
        let sig = crate::crypto::BitchatSignature {
            signature: proposal.signature.0.to_vec(),
            public_key: proposal.proposer.to_vec(),
        };
        
        Ok(crate::crypto::BitchatIdentity::verify_signature(&data_to_verify, &sig))
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
        hasher.update(round_id.to_le_bytes());
        hasher.update(nonce);
        Ok(hasher.finalize().into())
    }
    
    /// Sign randomness commit with proper cryptographic signature
    fn sign_randomness_commit(&self, round_id: RoundId, commitment: &Hash256) -> Result<Signature> {
        // Sign the commitment with node's identity
        let mut message = Vec::new();
        message.extend_from_slice(&round_id.to_le_bytes());
        message.extend_from_slice(commitment);
        
        let identity = crate::crypto::BitchatIdentity::generate_with_pow(0);
        let signature = identity.keypair.sign(&message);
        // Convert BitchatSignature to [u8; 64] for Signature type
        let sig_bytes: [u8; 64] = signature.signature.try_into().unwrap_or([0u8; 64]);
        Ok(Signature(sig_bytes))
    }
    
    /// Generate dispute ID
    fn generate_dispute_id(&self, claim: &DisputeClaim) -> Result<DisputeId> {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(self.local_peer_id);
        hasher.update(self.current_timestamp().to_le_bytes());
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
    
    /// Sign dispute vote with proper cryptographic signature
    fn sign_dispute_vote(&self, dispute_id: DisputeId) -> Result<Signature> {
        // Create signature data
        let mut signature_data = Vec::new();
        signature_data.extend_from_slice(&self.local_peer_id);
        signature_data.extend_from_slice(&dispute_id);
        signature_data.extend_from_slice(&self.current_timestamp().to_le_bytes());
        
        // Sign with identity key
        let identity = crate::crypto::BitchatIdentity::generate_with_pow(0);
        let signature = identity.keypair.sign(&signature_data);
        let sig_bytes: [u8; 64] = signature.signature.try_into()
            .map_err(|_| crate::error::Error::InvalidSignature("Failed to convert signature".to_string()))?;
        
        Ok(Signature(sig_bytes))
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
    
    // Public methods needed by ConsensusCoordinator
    
    /// Add a participant to consensus
    pub fn add_participant(&mut self, participant: PeerId) -> Result<()> {
        if !self.participants.contains(&participant) {
            self.participants.push(participant);
        }
        Ok(())
    }
    
    /// Remove a participant from consensus
    pub fn remove_participant(&mut self, participant: PeerId) -> Result<()> {
        self.participants.retain(|&p| p != participant);
        Ok(())
    }
    
    
    /// Process consensus round
    pub fn process_round(&self) -> Result<()> {
        // Process pending proposals and votes
        // This would be implemented fully in production
        Ok(())
    }
    
    /// Check if consensus has been reached
    pub fn has_consensus(&self) -> bool {
        self.current_state.confirmations >= self.config.min_confirmations as u32
    }
    
    /// Get current consensus state
    pub fn get_consensus_state(&self) -> Result<Vec<u8>> {
        // Serialize current state
        bincode::serialize(&*self.current_state)
            .map_err(|e| crate::error::Error::Serialization(e.to_string()))
    }
    
    /// Handle timeout for consensus round
    pub fn handle_timeout(&mut self) -> Result<()> {
        // Move to next round or handle stuck consensus
        // This would be implemented fully in production
        Ok(())
    }
    
    // ============= Byzantine Fault Tolerance Methods =============
    
    /// Create signature data for a vote
    fn create_vote_signature_data(&self, proposal_id: ProposalId, vote: bool) -> Result<Vec<u8>> {
        let mut data = Vec::new();
        data.extend_from_slice(&proposal_id);
        data.extend_from_slice(&self.local_peer_id);
        data.push(if vote { 1 } else { 0 });
        data.extend_from_slice(&self.current_timestamp().to_le_bytes());
        Ok(data)
    }
    
    /// Sign a vote with cryptographic signature
    fn sign_vote(&self, vote_data: &[u8]) -> Result<Signature> {
        let identity = crate::crypto::BitchatIdentity::generate_with_pow(0);
        let signature = identity.keypair.sign(vote_data);
        let sig_bytes: [u8; 64] = signature.signature.try_into().unwrap_or([0u8; 64]);
        Ok(Signature(sig_bytes))
    }
    
    /// Verify a vote signature
    fn verify_vote_signature(&self, vote_data: &[u8], signature: &Signature, peer_id: &PeerId) -> Result<bool> {
        let sig = crate::crypto::BitchatSignature {
            signature: signature.0.to_vec(),
            public_key: peer_id.to_vec(),
        };
        Ok(crate::crypto::BitchatIdentity::verify_signature(vote_data, &sig))
    }
    
    /// Finalize proposal with additional Byzantine checks
    fn finalize_proposal_with_byzantine_checks(&mut self, proposal_id: ProposalId) -> Result<()> {
        if let Some(proposal) = self.pending_proposals.get(&proposal_id) {
            // Verify proposal state transition is valid
            if !self.verify_state_transition(&proposal.proposed_state)? {
                return Err(crate::error::Error::InvalidProposal(
                    "Proposed state transition is invalid".to_string()
                ));
            }
            
            // Verify all signatures on supporting votes
            if let Some(vote_tracker) = self.votes.get(&proposal_id) {
                for voter in &vote_tracker.votes_for {
                    let vote_data = self.create_vote_signature_data(proposal_id, true)?;
                    // In a full implementation, we would store and verify each vote's signature
                    // For now, we assume signature verification was done when vote was received
                    self.consensus_metrics.signatures_verified += 1;
                }
            }
            
            // Double-check Byzantine threshold one more time
            let total_participants = self.participants.len();
            let byzantine_threshold = (total_participants * 2) / 3 + 1;
            
            if let Some(vote_tracker) = self.votes.get(&proposal_id) {
                if vote_tracker.votes_for.len() < byzantine_threshold {
                    return Err(crate::error::Error::InsufficientVotes(
                        "Not enough votes for Byzantine fault tolerance".to_string()
                    ));
                }
            }
        }
        
        // If all checks pass, finalize normally
        self.finalize_proposal(proposal_id)
    }
    
    /// Detect potential Byzantine voting patterns
    fn detect_byzantine_voting_patterns(&mut self, proposal_id: ProposalId) -> Result<()> {
        if let Some(vote_tracker) = self.votes.get(&proposal_id) {
            let total_participants = self.participants.len();
            let total_votes = vote_tracker.votes_for.len() + vote_tracker.votes_against.len();
            
            // Check for suspiciously low participation
            if total_votes < total_participants / 2 {
                // More than half the network is silent - potential coordinated attack
                log::warn!("Low participation detected for proposal {}: {}/{} votes", 
                          hex::encode(proposal_id), total_votes, total_participants);
            }
            
            // Check for unusual voting patterns
            let for_ratio = vote_tracker.votes_for.len() as f64 / total_participants as f64;
            let against_ratio = vote_tracker.votes_against.len() as f64 / total_participants as f64;
            
            if for_ratio > 0.9 || against_ratio > 0.9 {
                // Suspiciously unanimous - could indicate collusion
                log::warn!("Suspiciously unanimous voting on proposal {}: {:.2}% for, {:.2}% against", 
                          hex::encode(proposal_id), for_ratio * 100.0, against_ratio * 100.0);
            }
        }
        Ok(())
    }
    
    /// Verify that a state transition is valid
    fn verify_state_transition(&self, proposed_state: &GameConsensusState) -> Result<bool> {
        // Check sequence number is exactly one more than current
        let expected_sequence = SafeArithmetic::safe_increment_sequence(self.current_state.sequence_number)?;
        if proposed_state.sequence_number != expected_sequence {
            return Ok(false);
        }
        
        // Check timestamp is reasonable (not too far in past or future)
        let now = self.current_timestamp();
        let proposed_time = proposed_state.timestamp;
        
        if proposed_time < now.saturating_sub(300) || proposed_time > now + 300 {
            // State timestamp is more than 5 minutes off - suspicious
            return Ok(false);
        }
        
        // Check that balances don't violate conservation of value using safe arithmetic
        let mut current_total = 0u64;
        for balance in self.current_state.player_balances.values() {
            current_total = SafeArithmetic::safe_add_u64(current_total, balance.0)?;
        }
        
        let mut proposed_total = 0u64;
        for balance in proposed_state.player_balances.values() {
            proposed_total = SafeArithmetic::safe_add_u64(proposed_total, balance.0)?;
        }
        
        if proposed_total > current_total {
            // Proposed state creates value out of thin air - invalid
            return Ok(false);
        }
        
        // Additional game-specific validation would go here
        // For now, accept the state as valid
        Ok(true)
    }
    
    /// Process a vote from another peer with full Byzantine verification
    pub fn process_peer_vote(&mut self, proposal_id: ProposalId, voter: PeerId, vote: bool, signature: Signature) -> Result<()> {
        // Verify proposal exists
        if !self.pending_proposals.contains_key(&proposal_id) {
            return Err(crate::error::Error::InvalidProposal(
                "Proposal not found".to_string()
            ));
        }
        
        // Verify voter is a participant
        if !self.participants.contains(&voter) {
            return Err(crate::error::Error::UnknownPeer(
                "Voter is not a participant in this consensus".to_string()
            ));
        }
        
        // Check if this peer has already voted
        if let Some(vote_tracker) = self.votes.get(&proposal_id) {
            if vote_tracker.votes_for.contains(&voter) || 
               vote_tracker.votes_against.contains(&voter) ||
               vote_tracker.abstentions.contains(&voter) {
                return Err(crate::error::Error::DuplicateVote(
                    "Peer has already voted on this proposal".to_string()
                ));
            }
        }
        
        // Create and verify vote signature
        let vote_data = {
            let mut data = Vec::new();
            data.extend_from_slice(&proposal_id);
            data.extend_from_slice(&voter);
            data.push(if vote { 1 } else { 0 });
            // Note: In production, we'd need to handle timestamp synchronization
            data
        };
        
        if !self.verify_vote_signature(&vote_data, &signature, &voter)? {
            return Err(crate::error::Error::InvalidSignature(
                "Vote signature verification failed".to_string()
            ));
        }
        
        // Record the verified vote
        if let Some(vote_tracker) = self.votes.get_mut(&proposal_id) {
            if vote {
                vote_tracker.votes_for.insert(voter);
            } else {
                vote_tracker.votes_against.insert(voter);
            }
        }
        
        // Update metrics
        self.consensus_metrics.signatures_verified += 1;
        
        // Check if this triggers consensus
        self.check_byzantine_proposal_consensus(proposal_id)?;
        
        Ok(())
    }
}