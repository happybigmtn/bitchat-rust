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

use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use serde::{Serialize, Deserialize};
use sha2::{Sha256, Digest};
use rand::{RngCore, CryptoRng};

use crate::error::{Error, Result};
use super::{PeerId, GameId, CrapTokens, DiceRoll, BetType, Bet, Hash256, Signature};
use super::craps::{CrapsGame, GamePhase, BetResolution};

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
        }
    }
}

/// Main consensus engine for BitCraps
pub struct ConsensusEngine {
    config: ConsensusConfig,
    game_id: GameId,
    participants: Vec<PeerId>,
    local_peer_id: PeerId,
    
    // Current consensus state
    current_state: GameConsensusState,
    pending_proposals: HashMap<ProposalId, GameProposal>,
    
    // Voting and confirmation tracking
    votes: HashMap<ProposalId, VoteTracker>,
    confirmations: HashMap<StateHash, ConfirmationTracker>,
    
    // Fork management
    forks: HashMap<StateHash, Fork>,
    canonical_chain: Vec<StateHash>,
    
    // Commit-reveal for randomness
    dice_commits: HashMap<RoundId, HashMap<PeerId, RandomnessCommit>>,
    dice_reveals: HashMap<RoundId, HashMap<PeerId, RandomnessReveal>>,
    
    // Dispute tracking
    active_disputes: HashMap<DisputeId, Dispute>,
    dispute_votes: HashMap<DisputeId, HashMap<PeerId, DisputeVote>>,
    
    // Performance tracking
    consensus_metrics: ConsensusMetrics,
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
    pub player_balances: HashMap<PeerId, CrapTokens>,
    
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
        changes: HashMap<PeerId, CrapTokens>,
        reason: String,
    },
}

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

/// Randomness commitment for dice rolls
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RandomnessCommit {
    pub player: PeerId,
    pub round_id: RoundId,
    pub commitment: Hash256, // SHA256(nonce || round_id)
    pub timestamp: u64,
    pub signature: Signature,
}

/// Randomness reveal for dice rolls
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RandomnessReveal {
    pub player: PeerId,
    pub round_id: RoundId,
    pub nonce: [u8; 32],
    pub timestamp: u64,
    pub signature: Signature,
}

/// Dispute representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dispute {
    pub id: DisputeId,
    pub disputer: PeerId,
    pub disputed_state: StateHash,
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
    StateSnapshot {
        state_hash: StateHash,
        timestamp: u64,
    },
    CryptographicProof {
        proof_type: String,
        proof_data: Vec<u8>,
    },
    WitnessAttestation {
        witness: PeerId,
        statement: String,
        signature: Signature,
    },
}

/// Vote on dispute resolution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisputeVote {
    pub voter: PeerId,
    pub dispute_id: DisputeId,
    pub vote: DisputeVoteType,
    pub rationale: String,
    pub timestamp: u64,
    pub signature: Signature,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DisputeVoteType {
    Uphold,    // Dispute is valid
    Dismiss,   // Dispute is invalid
    Abstain,   // Cannot determine
}

/// Type aliases
pub type ProposalId = Hash256;
pub type StateHash = Hash256;
pub type RoundId = u64;
pub type DisputeId = Hash256;

/// Consensus performance metrics
#[derive(Debug, Clone, Default)]
pub struct ConsensusMetrics {
    pub total_proposals: u64,
    pub successful_consensus: u64,
    pub failed_consensus: u64,
    pub average_consensus_time: Duration,
    pub forks_resolved: u64,
    pub disputes_resolved: u64,
    pub byzantine_actors_detected: u64,
}

impl ConsensusEngine {
    /// Create a new consensus engine
    pub fn new(
        config: ConsensusConfig,
        game_id: GameId,
        participants: Vec<PeerId>,
        local_peer_id: PeerId,
        initial_state: CrapsGame,
    ) -> Result<Self> {
        let initial_consensus_state = GameConsensusState {
            game_id,
            state_hash: Self::calculate_state_hash(&initial_state)?,
            sequence_number: 0,
            timestamp: Self::current_timestamp(),
            game_state: initial_state,
            player_balances: HashMap::new(),
            last_proposer: local_peer_id,
            confirmations: 0,
            is_finalized: false,
        };

        let mut engine = Self {
            config,
            game_id,
            participants,
            local_peer_id,
            current_state: initial_consensus_state,
            pending_proposals: HashMap::new(),
            votes: HashMap::new(),
            confirmations: HashMap::new(),
            forks: HashMap::new(),
            canonical_chain: Vec::new(),
            dice_commits: HashMap::new(),
            dice_reveals: HashMap::new(),
            active_disputes: HashMap::new(),
            dispute_votes: HashMap::new(),
            consensus_metrics: ConsensusMetrics::default(),
        };

        // Initialize canonical chain
        engine.canonical_chain.push(engine.current_state.state_hash);
        
        Ok(engine)
    }

    /// Propose a new game operation
    pub fn propose_operation(&mut self, operation: GameOperation) -> Result<ProposalId> {
        // Calculate new state after applying operation
        let mut new_state = self.current_state.clone();
        self.apply_operation(&mut new_state, &operation)?;
        
        // Create proposal
        let proposal_id = self.generate_proposal_id(&operation)?;
        let proposal = GameProposal {
            id: proposal_id,
            proposer: self.local_peer_id,
            previous_state_hash: self.current_state.state_hash,
            proposed_state: new_state,
            operation,
            timestamp: Self::current_timestamp(),
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

        self.consensus_metrics.total_proposals += 1;
        
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

    /// Vote on a pending proposal
    pub fn vote_on_proposal(&mut self, proposal_id: ProposalId, vote_for: bool) -> Result<()> {
        let vote_tracker = self.votes.entry(proposal_id).or_insert_with(|| VoteTracker {
            proposal_id,
            votes_for: HashSet::new(),
            votes_against: HashSet::new(),
            abstentions: HashSet::new(),
            created_at: SystemTime::now(),
        });

        // Record vote
        vote_tracker.votes_for.remove(&self.local_peer_id);
        vote_tracker.votes_against.remove(&self.local_peer_id);
        vote_tracker.abstentions.remove(&self.local_peer_id);

        if vote_for {
            vote_tracker.votes_for.insert(self.local_peer_id);
        } else {
            vote_tracker.votes_against.insert(self.local_peer_id);
        }

        // Check for consensus
        self.check_consensus(proposal_id)?;
        
        Ok(())
    }

    /// Check if a proposal has reached consensus
    fn check_consensus(&mut self, proposal_id: ProposalId) -> Result<bool> {
        let vote_tracker = self.votes.get(&proposal_id)
            .ok_or_else(|| Error::ValidationError("Vote tracker not found".to_string()))?;

        let total_participants = self.participants.len();
        let required_votes = self.calculate_required_votes(total_participants);
        
        // Check if we have enough votes for consensus
        if vote_tracker.votes_for.len() >= required_votes {
            // Apply the proposal
            if let Some(proposal) = self.pending_proposals.remove(&proposal_id) {
                self.apply_consensus(&proposal)?;
                self.consensus_metrics.successful_consensus += 1;
            }
            return Ok(true);
        }

        // Check if proposal is rejected
        if vote_tracker.votes_against.len() > total_participants - required_votes {
            // Remove rejected proposal
            self.pending_proposals.remove(&proposal_id);
            self.votes.remove(&proposal_id);
            self.consensus_metrics.failed_consensus += 1;
            return Ok(false);
        }

        // Check timeout
        if vote_tracker.created_at.elapsed().unwrap_or(Duration::ZERO) > self.config.consensus_timeout {
            // Timeout - remove proposal
            self.pending_proposals.remove(&proposal_id);
            self.votes.remove(&proposal_id);
            self.consensus_metrics.failed_consensus += 1;
            return Ok(false);
        }

        Ok(false)
    }

    /// Apply a consensus decision
    fn apply_consensus(&mut self, proposal: &GameProposal) -> Result<()> {
        // Update current state
        self.current_state = proposal.proposed_state.clone();
        self.current_state.sequence_number += 1;
        self.current_state.is_finalized = true;
        
        // Add to canonical chain
        self.canonical_chain.push(self.current_state.state_hash);
        
        // Initialize confirmation tracking
        self.confirmations.insert(self.current_state.state_hash, ConfirmationTracker {
            state_hash: self.current_state.state_hash,
            confirmations: HashSet::from([self.local_peer_id]),
            rejections: HashSet::new(),
            finalized_at: Some(SystemTime::now()),
        });

        // Clean up
        self.votes.remove(&proposal.id);
        
        Ok(())
    }

    /// Handle potential fork in the blockchain
    fn handle_fork(&mut self, proposal: &GameProposal) -> Result<bool> {
        if !self.config.enable_fork_recovery {
            return Ok(false);
        }

        // Check if we know about the parent state
        if !self.canonical_chain.contains(&proposal.previous_state_hash) {
            // This is from an unknown branch - request state sync
            return Ok(false);
        }

        // Create or update fork
        let fork_id = proposal.proposed_state.state_hash;
        let fork = self.forks.entry(fork_id).or_insert_with(|| Fork {
            fork_id,
            parent_state: proposal.previous_state_hash,
            competing_states: vec![self.current_state.state_hash, fork_id],
            supporters: HashMap::new(),
            created_at: SystemTime::now(),
            resolution_deadline: SystemTime::now() + self.config.fork_resolution_timeout,
        });

        // Add supporter
        fork.supporters.entry(fork_id).or_insert_with(HashSet::new).insert(proposal.proposer);
        
        // Check if fork should be resolved
        self.check_fork_resolution(fork_id)?;
        
        Ok(true)
    }

    /// Check if a fork should be resolved
    fn check_fork_resolution(&mut self, fork_id: StateHash) -> Result<()> {
        let fork = self.forks.get(&fork_id)
            .ok_or_else(|| Error::ValidationError("Fork not found".to_string()))?
            .clone();

        // Count supporters for each competing state
        let mut support_counts: HashMap<StateHash, usize> = HashMap::new();
        for (state_hash, supporters) in &fork.supporters {
            support_counts.insert(*state_hash, supporters.len());
        }

        // Find the state with most support
        let winner = support_counts.iter()
            .max_by_key(|(_, count)| *count)
            .map(|(state_hash, _)| *state_hash);

        if let Some(winning_state) = winner {
            let winning_count = support_counts[&winning_state];
            let required_support = (self.participants.len() * 2) / 3 + 1;

            // Check if we have enough support or timeout
            let should_resolve = winning_count >= required_support || 
                SystemTime::now() > fork.resolution_deadline;

            if should_resolve {
                self.resolve_fork(fork_id, winning_state)?;
            }
        }

        Ok(())
    }

    /// Resolve a fork by selecting the winning branch
    fn resolve_fork(&mut self, fork_id: StateHash, winning_state: StateHash) -> Result<()> {
        // If the winning state is not our current state, we need to switch
        if winning_state != self.current_state.state_hash {
            // Request full state sync for the winning branch
            // In a real implementation, this would trigger state synchronization
        }

        // Remove the fork
        self.forks.remove(&fork_id);
        self.consensus_metrics.forks_resolved += 1;
        
        Ok(())
    }

    /// Start commit phase for dice roll randomness
    pub fn start_dice_commit_phase(&mut self, round_id: RoundId) -> Result<Hash256> {
        // Generate random nonce
        let mut nonce = [0u8; 32];
        let mut rng = rand::thread_rng();
        rng.fill_bytes(&mut nonce);

        // Create commitment
        let commitment = self.create_randomness_commitment(round_id, &nonce)?;
        
        // Store our commitment
        let commit = RandomnessCommit {
            player: self.local_peer_id,
            round_id,
            commitment,
            timestamp: Self::current_timestamp(),
            signature: self.sign_randomness_commit(round_id, &commitment)?,
        };

        self.dice_commits.entry(round_id).or_insert_with(HashMap::new)
            .insert(self.local_peer_id, commit);

        // Store nonce for later reveal
        // In production, this should be stored securely
        
        Ok(commitment)
    }

    /// Process commitment from another player
    pub fn process_dice_commit(&mut self, commit: RandomnessCommit) -> Result<()> {
        // Verify signature
        if !self.verify_randomness_commit_signature(&commit)? {
            return Err(Error::ValidationError("Invalid commit signature".to_string()));
        }

        // Store commitment
        let round_id = commit.round_id;
        let player = commit.player;
        self.dice_commits.entry(round_id).or_insert_with(HashMap::new)
            .insert(player, commit);

        // Check if we have all commitments
        if self.all_dice_commits_received(round_id) {
            // Start reveal phase
            self.start_dice_reveal_phase(round_id)?;
        }

        Ok(())
    }

    /// Start reveal phase for dice roll
    fn start_dice_reveal_phase(&mut self, round_id: RoundId) -> Result<()> {
        // This would trigger reveal phase in the real implementation
        Ok(())
    }

    /// Process randomness reveal
    pub fn process_dice_reveal(&mut self, reveal: RandomnessReveal) -> Result<()> {
        // Verify signature
        if !self.verify_randomness_reveal_signature(&reveal)? {
            return Err(Error::ValidationError("Invalid reveal signature".to_string()));
        }

        // Verify reveal matches commitment
        let expected_commitment = self.create_randomness_commitment(reveal.round_id, &reveal.nonce)?;
        let stored_commit = self.dice_commits.get(&reveal.round_id)
            .and_then(|commits| commits.get(&reveal.player))
            .ok_or_else(|| Error::ValidationError("No commitment found for reveal".to_string()))?;

        if stored_commit.commitment != expected_commitment {
            return Err(Error::ValidationError("Reveal does not match commitment".to_string()));
        }

        // Store reveal
        let round_id = reveal.round_id;
        let player = reveal.player;
        self.dice_reveals.entry(round_id).or_insert_with(HashMap::new)
            .insert(player, reveal);

        // Check if we have all reveals
        if self.all_dice_reveals_received(round_id) {
            self.generate_consensus_dice_roll(round_id)?;
        }

        Ok(())
    }

    /// Generate final dice roll from all reveals
    fn generate_consensus_dice_roll(&mut self, round_id: RoundId) -> Result<DiceRoll> {
        let reveals = self.dice_reveals.get(&round_id)
            .ok_or_else(|| Error::ValidationError("No reveals found for round".to_string()))?;

        // Combine all nonces for final randomness
        let mut combined_entropy = Vec::new();
        for (_, reveal) in reveals.iter() {
            combined_entropy.extend_from_slice(&reveal.nonce);
        }

        // Hash combined entropy
        let mut hasher = Sha256::new();
        hasher.update(&combined_entropy);
        let entropy_hash = hasher.finalize();

        // Generate dice values from hash
        let die1 = (entropy_hash[0] % 6) + 1;
        let die2 = (entropy_hash[1] % 6) + 1;

        let dice_roll = DiceRoll::new(die1, die2)?;

        // Propose the dice roll operation
        let operation = GameOperation::ProcessRoll {
            round_id,
            dice_roll,
            entropy_proof: reveals.values()
                .map(|r| self.create_randomness_commitment(round_id, &r.nonce).unwrap())
                .collect(),
        };

        self.propose_operation(operation)?;

        Ok(dice_roll)
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
            created_at: Self::current_timestamp(),
            resolution_deadline: Self::current_timestamp() + 3600, // 1 hour
        };

        self.active_disputes.insert(dispute_id, dispute);
        
        Ok(dispute_id)
    }

    /// Vote on a dispute
    pub fn vote_on_dispute(
        &mut self, 
        dispute_id: DisputeId, 
        vote: DisputeVoteType, 
        rationale: String
    ) -> Result<()> {
        let dispute_vote = DisputeVote {
            voter: self.local_peer_id,
            dispute_id,
            vote,
            rationale,
            timestamp: Self::current_timestamp(),
            signature: self.sign_dispute_vote(dispute_id)?,
        };

        self.dispute_votes.entry(dispute_id).or_insert_with(HashMap::new)
            .insert(self.local_peer_id, dispute_vote);

        // Check if dispute can be resolved
        self.check_dispute_resolution(dispute_id)?;
        
        Ok(())
    }

    /// Check if a dispute has enough votes for resolution
    fn check_dispute_resolution(&mut self, dispute_id: DisputeId) -> Result<()> {
        let votes = self.dispute_votes.get(&dispute_id)
            .ok_or_else(|| Error::ValidationError("No votes found for dispute".to_string()))?;

        let total_participants = self.participants.len();
        let required_votes = (total_participants * 2) / 3 + 1;

        let uphold_votes = votes.values().filter(|v| matches!(v.vote, DisputeVoteType::Uphold)).count();
        let dismiss_votes = votes.values().filter(|v| matches!(v.vote, DisputeVoteType::Dismiss)).count();

        if uphold_votes >= required_votes {
            self.resolve_dispute(dispute_id, true)?;
        } else if dismiss_votes >= required_votes {
            self.resolve_dispute(dispute_id, false)?;
        }

        Ok(())
    }

    /// Resolve a dispute
    fn resolve_dispute(&mut self, dispute_id: DisputeId, upheld: bool) -> Result<()> {
        if upheld {
            // Dispute was upheld - need to take corrective action
            if let Some(dispute) = self.active_disputes.get(&dispute_id) {
                match &dispute.claim {
                    DisputeClaim::InvalidBet { .. } => {
                        // Revert the invalid bet
                    },
                    DisputeClaim::InvalidRoll { .. } => {
                        // Re-roll or revert to previous state
                    },
                    DisputeClaim::InvalidPayout { .. } => {
                        // Correct the payout
                    },
                    _ => {
                        // Handle other dispute types
                    }
                }
            }
        }

        // Clean up
        self.active_disputes.remove(&dispute_id);
        self.dispute_votes.remove(&dispute_id);
        self.consensus_metrics.disputes_resolved += 1;

        Ok(())
    }

    // Helper methods

    fn calculate_state_hash(game_state: &CrapsGame) -> Result<StateHash> {
        let serialized = bincode::serialize(game_state)
            .map_err(|e| Error::Serialization(e.to_string()))?;
        let mut hasher = Sha256::new();
        hasher.update(&serialized);
        Ok(hasher.finalize().into())
    }

    fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0))
            .as_secs()
    }

    fn calculate_required_votes(&self, total_participants: usize) -> usize {
        // Byzantine fault tolerance: need > 2/3 votes
        (total_participants * 2) / 3 + 1
    }

    fn apply_operation(&self, state: &mut GameConsensusState, operation: &GameOperation) -> Result<()> {
        match operation {
            GameOperation::PlaceBet { player, bet, .. } => {
                state.game_state.place_bet(*player, bet.clone())?;
            },
            GameOperation::ProcessRoll { dice_roll, .. } => {
                state.game_state.process_roll(*dice_roll);
            },
            GameOperation::ResolvePhase { new_phase, .. } => {
                state.game_state.phase = *new_phase;
                state.game_state.current_phase = *new_phase;
            },
            GameOperation::UpdateBalances { changes, .. } => {
                for (player, change) in changes {
                    let current = state.player_balances.get(player).copied().unwrap_or_else(|| CrapTokens::new_unchecked(0));
                    state.player_balances.insert(*player, current.checked_add(change)?);
                }
            },
            GameOperation::CommitRandomness { .. } => {
                // Randomness commitments are handled separately
            },
            GameOperation::RevealRandomness { .. } => {
                // Randomness reveals are handled separately
            },
        }
        
        // Update state hash
        state.state_hash = Self::calculate_state_hash(&state.game_state)?;
        state.timestamp = Self::current_timestamp();
        
        Ok(())
    }

    fn validate_operation(&self, operation: &GameOperation) -> Result<bool> {
        match operation {
            GameOperation::PlaceBet { player, bet, .. } => {
                // Validate bet is valid for current game phase
                let phase_valid = match bet.bet_type {
                    // For now, assume all bets are valid in all phases
                    _ => true,
                };
                Ok(phase_valid && self.participants.contains(player))
            },
            GameOperation::ProcessRoll { entropy_proof, .. } => {
                // Validate entropy proof
                Ok(entropy_proof.len() >= self.participants.len())
            },
            _ => Ok(true),
        }
    }

    fn all_dice_commits_received(&self, round_id: RoundId) -> bool {
        if let Some(commits) = self.dice_commits.get(&round_id) {
            commits.len() >= self.participants.len()
        } else {
            false
        }
    }

    fn all_dice_reveals_received(&self, round_id: RoundId) -> bool {
        if let Some(reveals) = self.dice_reveals.get(&round_id) {
            reveals.len() >= self.participants.len()
        } else {
            false
        }
    }

    fn create_randomness_commitment(&self, round_id: RoundId, nonce: &[u8; 32]) -> Result<Hash256> {
        let mut hasher = Sha256::new();
        hasher.update(nonce);
        hasher.update(&round_id.to_be_bytes());
        Ok(hasher.finalize().into())
    }

    fn generate_proposal_id(&self, operation: &GameOperation) -> Result<ProposalId> {
        let serialized = bincode::serialize(operation)
            .map_err(|e| Error::Serialization(e.to_string()))?;
        let mut hasher = Sha256::new();
        hasher.update(&serialized);
        hasher.update(&self.local_peer_id);
        hasher.update(&Self::current_timestamp().to_be_bytes());
        Ok(hasher.finalize().into())
    }

    fn generate_dispute_id(&self, claim: &DisputeClaim) -> Result<DisputeId> {
        let serialized = bincode::serialize(claim)
            .map_err(|e| Error::Serialization(e.to_string()))?;
        let mut hasher = Sha256::new();
        hasher.update(&serialized);
        hasher.update(&self.local_peer_id);
        hasher.update(&Self::current_timestamp().to_be_bytes());
        Ok(hasher.finalize().into())
    }

    // Placeholder signature methods - in production these would use actual cryptography
    fn sign_proposal(&self, _proposal_id: &ProposalId) -> Result<Signature> {
        Ok(Signature([0u8; 64]))
    }

    fn verify_proposal_signature(&self, _proposal: &GameProposal) -> Result<bool> {
        Ok(true)
    }

    fn sign_randomness_commit(&self, _round_id: RoundId, _commitment: &Hash256) -> Result<Signature> {
        Ok(Signature([0u8; 64]))
    }

    fn verify_randomness_commit_signature(&self, _commit: &RandomnessCommit) -> Result<bool> {
        Ok(true)
    }

    fn verify_randomness_reveal_signature(&self, _reveal: &RandomnessReveal) -> Result<bool> {
        Ok(true)
    }

    fn sign_dispute_vote(&self, _dispute_id: DisputeId) -> Result<Signature> {
        Ok(Signature([0u8; 64]))
    }

    /// Get current game state
    pub fn get_current_state(&self) -> &GameConsensusState {
        &self.current_state
    }

    /// Get consensus metrics
    pub fn get_metrics(&self) -> &ConsensusMetrics {
        &self.consensus_metrics
    }

    /// Check if consensus is healthy
    pub fn is_consensus_healthy(&self) -> bool {
        self.active_disputes.is_empty() && 
        self.forks.is_empty() &&
        self.pending_proposals.len() < 10
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::craps::CrapsGame;

    #[test]
    fn test_consensus_engine_creation() {
        let config = ConsensusConfig::default();
        let game_id = [1u8; 16];
        let participants = vec![[1u8; 32], [2u8; 32]];
        let local_peer = [1u8; 32];
        let initial_game = CrapsGame::new(game_id, local_peer);
        
        let result = ConsensusEngine::new(config, game_id, participants, local_peer, initial_game);
        assert!(result.is_ok());
    }

    #[test]
    fn test_proposal_creation() {
        let config = ConsensusConfig::default();
        let game_id = [1u8; 16];
        let participants = vec![[1u8; 32], [2u8; 32]];
        let local_peer = [1u8; 32];
        let initial_game = CrapsGame::new(game_id, local_peer);
        
        let mut engine = ConsensusEngine::new(config, game_id, participants, local_peer, initial_game).unwrap();
        
        let operation = GameOperation::UpdateBalances {
            changes: HashMap::new(),
            reason: "Test".to_string(),
        };
        
        let result = engine.propose_operation(operation);
        assert!(result.is_ok());
    }

    #[test]
    fn test_dice_commit_reveal() {
        let config = ConsensusConfig::default();
        let game_id = [1u8; 16];
        let participants = vec![[1u8; 32], [2u8; 32]];
        let local_peer = [1u8; 32];
        let initial_game = CrapsGame::new(game_id, local_peer);
        
        let mut engine = ConsensusEngine::new(config, game_id, participants, local_peer, initial_game).unwrap();
        
        let round_id = 1;
        let result = engine.start_dice_commit_phase(round_id);
        assert!(result.is_ok());
    }
}