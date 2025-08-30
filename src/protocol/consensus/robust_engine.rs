//! Robust Consensus Engine with Forced Settlement
//!
//! This engine ensures that consensus proceeds even when losing players
//! attempt to block or delay settlement. Key features:
//! - Threshold signatures for consensus (2/3 majority)
//! - Automatic timeout progression
//! - Penalty system for non-participation
//! - Forced settlement after timeout

use crate::crypto::GameCrypto;
use crate::error::Error;
use crate::protocol::{GameId, Hash256, PeerId};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Consensus parameters
pub const CONSENSUS_THRESHOLD: f64 = 0.67; // 2/3 majority required
pub const COMMIT_TIMEOUT: Duration = Duration::from_secs(10);
pub const REVEAL_TIMEOUT: Duration = Duration::from_secs(10);
pub const SETTLEMENT_TIMEOUT: Duration = Duration::from_secs(15);
pub const PENALTY_MULTIPLIER: f64 = 0.1; // 10% penalty for non-participation

/// Consensus round states
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConsensusPhase {
    /// Waiting for participants to commit
    Commit,
    /// Waiting for reveals after all commits received
    Reveal,
    /// Proposing final state
    Propose,
    /// Voting on proposed state
    Vote,
    /// Settlement phase - distributing winnings
    Settlement,
    /// Completed round
    Completed,
    /// Failed round - will retry or force settlement
    Failed,
}

/// Signed message wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedMessage<T: Serialize> {
    /// The message content
    pub content: T,
    /// The signature over the serialized content (as bytes)
    #[serde(with = "serde_bytes")]
    pub signature: Vec<u8>,
    /// The signer's public key (as bytes)
    #[serde(with = "serde_bytes")]
    pub signer: Vec<u8>,
    /// Timestamp of signature
    pub timestamp: u64,
}

impl<T: Serialize + for<'de> Deserialize<'de>> SignedMessage<T> {
    /// Create a new signed message
    pub fn new(content: T, signing_key: &SigningKey) -> Self {
        let serialized = bincode::serialize(&content).unwrap();
        let signature = signing_key.sign(&serialized);

        Self {
            content,
            signature: signature.to_bytes().to_vec(),
            signer: signing_key.verifying_key().to_bytes().to_vec(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    /// Verify the signature
    pub fn verify(&self) -> bool {
        // Convert Vec<u8> back to fixed arrays
        let signer_bytes: [u8; 32] = match self.signer.as_slice().try_into() {
            Ok(bytes) => bytes,
            Err(_) => return false,
        };

        let signature_bytes: [u8; 64] = match self.signature.as_slice().try_into() {
            Ok(bytes) => bytes,
            Err(_) => return false,
        };

        let Ok(verifying_key) = VerifyingKey::from_bytes(&signer_bytes) else {
            return false;
        };

        let signature = Signature::from_bytes(&signature_bytes);

        let Ok(serialized) = bincode::serialize(&self.content) else {
            return false;
        };

        verifying_key.verify(&serialized, &signature).is_ok()
    }

    /// Check if message has expired
    pub fn is_expired(&self, timeout: Duration) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        now - self.timestamp > timeout.as_secs()
    }
}

/// Commit for randomness generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RandomnessCommit {
    pub round_id: u64,
    pub commitment: Hash256,
    pub peer_id: PeerId,
}

/// Reveal for randomness generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RandomnessReveal {
    pub round_id: u64,
    pub value: [u8; 32],
    pub nonce: [u8; 32],
    pub peer_id: PeerId,
}

/// State proposal for consensus
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateProposal {
    pub round_id: u64,
    pub game_id: GameId,
    pub state_hash: Hash256,
    pub dice_roll: Option<(u8, u8)>,
    pub settlements: Vec<Settlement>,
    pub proposer: PeerId,
}

/// Settlement record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settlement {
    pub player: PeerId,
    pub amount: i64, // Positive for wins, negative for losses
    pub bet_type: String,
    pub locked_amount: u64,
}

/// Vote on a proposal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalVote {
    pub round_id: u64,
    pub proposal_hash: Hash256,
    pub approve: bool,
    pub voter: PeerId,
}

/// Robust consensus engine
pub struct RobustConsensusEngine {
    /// Current round ID
    round_id: u64,

    /// Current phase
    phase: ConsensusPhase,

    /// Phase start time for timeout tracking
    phase_start: Instant,

    /// Active participants
    participants: HashSet<PeerId>,

    /// Received commits
    commits: HashMap<PeerId, SignedMessage<RandomnessCommit>>,

    /// Received reveals
    reveals: HashMap<PeerId, SignedMessage<RandomnessReveal>>,

    /// Current proposal
    current_proposal: Option<SignedMessage<StateProposal>>,

    /// Votes on current proposal
    votes: HashMap<PeerId, SignedMessage<ProposalVote>>,

    /// Penalty tracking for non-participation
    penalties: HashMap<PeerId, f64>,

    /// Our signing key
    signing_key: SigningKey,

    /// Treasury reference for settlement
    treasury: Arc<crate::protocol::treasury::TreasuryManager>,
}

impl RobustConsensusEngine {
    /// Create a new consensus engine
    pub fn new(
        signing_key: SigningKey,
        treasury: Arc<crate::protocol::treasury::TreasuryManager>,
    ) -> Self {
        Self {
            round_id: 0,
            phase: ConsensusPhase::Commit,
            phase_start: Instant::now(),
            participants: HashSet::new(),
            commits: HashMap::new(),
            reveals: HashMap::new(),
            current_proposal: None,
            votes: HashMap::new(),
            penalties: HashMap::new(),
            signing_key,
            treasury,
        }
    }

    /// Start a new consensus round
    pub fn start_round(&mut self, participants: HashSet<PeerId>) -> u64 {
        self.round_id += 1;
        self.phase = ConsensusPhase::Commit;
        self.phase_start = Instant::now();
        self.participants = participants;
        self.commits.clear();
        self.reveals.clear();
        self.current_proposal = None;
        self.votes.clear();

        self.round_id
    }

    /// Submit our commit
    pub fn submit_commit(
        &mut self,
        value: [u8; 32],
        nonce: [u8; 32],
    ) -> SignedMessage<RandomnessCommit> {
        let mut data = Vec::new();
        data.extend_from_slice(&value);
        data.extend_from_slice(&nonce);
        let commitment = GameCrypto::hash(&data);

        let commit = RandomnessCommit {
            round_id: self.round_id,
            commitment,
            peer_id: self.signing_key.verifying_key().to_bytes(),
        };

        let signed = SignedMessage::new(commit, &self.signing_key);
        self.commits
            .insert(self.signing_key.verifying_key().to_bytes(), signed.clone());
        signed
    }

    /// Process received commit
    pub fn process_commit(
        &mut self,
        signed_commit: SignedMessage<RandomnessCommit>,
    ) -> Result<(), Error> {
        // Verify signature
        if !signed_commit.verify() {
            return Err(Error::InvalidSignature("Invalid commit signature".into()));
        }

        // Check round ID
        if signed_commit.content.round_id != self.round_id {
            return Err(Error::InvalidState("Commit for wrong round".into()));
        }

        // Check phase
        if self.phase != ConsensusPhase::Commit {
            return Err(Error::InvalidState("Not in commit phase".into()));
        }

        // Check if participant is valid
        if !self.participants.contains(&signed_commit.content.peer_id) {
            return Err(Error::InvalidState("Unknown participant".into()));
        }

        // Store commit
        self.commits
            .insert(signed_commit.content.peer_id, signed_commit);

        // Check if we have enough commits to proceed
        self.check_phase_progression();

        Ok(())
    }

    /// Submit our reveal
    pub fn submit_reveal(
        &mut self,
        value: [u8; 32],
        nonce: [u8; 32],
    ) -> Result<SignedMessage<RandomnessReveal>, Error> {
        if self.phase != ConsensusPhase::Reveal {
            return Err(Error::InvalidState("Not in reveal phase".into()));
        }

        let reveal = RandomnessReveal {
            round_id: self.round_id,
            value,
            nonce,
            peer_id: self.signing_key.verifying_key().to_bytes(),
        };

        let signed = SignedMessage::new(reveal, &self.signing_key);
        self.reveals
            .insert(self.signing_key.verifying_key().to_bytes(), signed.clone());
        Ok(signed)
    }

    /// Process received reveal
    pub fn process_reveal(
        &mut self,
        signed_reveal: SignedMessage<RandomnessReveal>,
    ) -> Result<(), Error> {
        // Verify signature
        if !signed_reveal.verify() {
            return Err(Error::InvalidSignature("Invalid reveal signature".into()));
        }

        // Check round ID
        if signed_reveal.content.round_id != self.round_id {
            return Err(Error::InvalidState("Reveal for wrong round".into()));
        }

        // Check phase
        if self.phase != ConsensusPhase::Reveal {
            return Err(Error::InvalidState("Not in reveal phase".into()));
        }

        // Verify reveal matches commit
        if let Some(commit) = self.commits.get(&signed_reveal.content.peer_id) {
            let mut data = Vec::new();
            data.extend_from_slice(&signed_reveal.content.value);
            data.extend_from_slice(&signed_reveal.content.nonce);
            let computed_commitment = GameCrypto::hash(&data);

            if computed_commitment != commit.content.commitment {
                // Penalize for invalid reveal
                *self
                    .penalties
                    .entry(signed_reveal.content.peer_id)
                    .or_insert(0.0) += PENALTY_MULTIPLIER;
                return Err(Error::InvalidState("Reveal doesn't match commit".into()));
            }
        } else {
            return Err(Error::InvalidState("No commit for this reveal".into()));
        }

        // Store reveal
        self.reveals
            .insert(signed_reveal.content.peer_id, signed_reveal);

        // Check if we have enough reveals to proceed
        self.check_phase_progression();

        Ok(())
    }

    /// Create and sign a state proposal
    pub fn create_proposal(
        &mut self,
        game_id: GameId,
        settlements: Vec<Settlement>,
    ) -> Result<SignedMessage<StateProposal>, Error> {
        if self.phase != ConsensusPhase::Propose {
            return Err(Error::InvalidState("Not in propose phase".into()));
        }

        // Calculate dice roll from reveals
        let dice_roll = self.calculate_dice_roll();

        // Calculate state hash
        let mut state_data = Vec::new();
        state_data.extend_from_slice(&game_id);
        if let Some((d1, d2)) = dice_roll {
            state_data.push(d1);
            state_data.push(d2);
        }
        for settlement in &settlements {
            state_data.extend_from_slice(&settlement.player);
            state_data.extend_from_slice(&settlement.amount.to_le_bytes());
        }
        let state_hash = GameCrypto::hash(&state_data);

        let proposal = StateProposal {
            round_id: self.round_id,
            game_id,
            state_hash,
            dice_roll,
            settlements,
            proposer: self.signing_key.verifying_key().to_bytes(),
        };

        let signed = SignedMessage::new(proposal, &self.signing_key);
        self.current_proposal = Some(signed.clone());
        self.phase = ConsensusPhase::Vote;
        self.phase_start = Instant::now();

        Ok(signed)
    }

    /// Vote on a proposal
    pub fn vote_on_proposal(
        &mut self,
        proposal_hash: Hash256,
        approve: bool,
    ) -> Result<SignedMessage<ProposalVote>, Error> {
        if self.phase != ConsensusPhase::Vote {
            return Err(Error::InvalidState("Not in vote phase".into()));
        }

        let vote = ProposalVote {
            round_id: self.round_id,
            proposal_hash,
            approve,
            voter: self.signing_key.verifying_key().to_bytes(),
        };

        let signed = SignedMessage::new(vote, &self.signing_key);
        self.votes
            .insert(self.signing_key.verifying_key().to_bytes(), signed.clone());

        // Check if we have enough votes
        self.check_phase_progression();

        Ok(signed)
    }

    /// Check if we should progress to the next phase
    fn check_phase_progression(&mut self) {
        match self.phase {
            ConsensusPhase::Commit => {
                // Progress if we have threshold of commits or timeout
                let threshold = (self.participants.len() as f64 * CONSENSUS_THRESHOLD) as usize;
                if self.commits.len() >= threshold || self.phase_start.elapsed() > COMMIT_TIMEOUT {
                    self.phase = ConsensusPhase::Reveal;
                    self.phase_start = Instant::now();

                    // Penalize non-participants
                    for peer in &self.participants {
                        if !self.commits.contains_key(peer) {
                            *self.penalties.entry(*peer).or_insert(0.0) += PENALTY_MULTIPLIER;
                        }
                    }
                }
            }
            ConsensusPhase::Reveal => {
                // Progress if we have threshold of reveals or timeout
                let threshold = (self.commits.len() as f64 * CONSENSUS_THRESHOLD) as usize;
                if self.reveals.len() >= threshold || self.phase_start.elapsed() > REVEAL_TIMEOUT {
                    self.phase = ConsensusPhase::Propose;
                    self.phase_start = Instant::now();

                    // Penalize non-revealers
                    for peer in self.commits.keys() {
                        if !self.reveals.contains_key(peer) {
                            *self.penalties.entry(*peer).or_insert(0.0) += PENALTY_MULTIPLIER;
                        }
                    }
                }
            }
            ConsensusPhase::Vote => {
                // Check if proposal is accepted
                let threshold = (self.participants.len() as f64 * CONSENSUS_THRESHOLD) as usize;
                let approvals = self.votes.values().filter(|v| v.content.approve).count();

                if approvals >= threshold {
                    self.phase = ConsensusPhase::Settlement;
                    self.phase_start = Instant::now();
                } else if self.votes.len() >= self.participants.len()
                    || self.phase_start.elapsed() > SETTLEMENT_TIMEOUT
                {
                    // Force settlement with penalties for non-voters
                    self.force_settlement();
                }
            }
            _ => {}
        }
    }

    /// Calculate dice roll from reveals
    fn calculate_dice_roll(&self) -> Option<(u8, u8)> {
        if self.reveals.is_empty() {
            return None;
        }

        // Combine all revealed values
        let mut combined = [0u8; 32];
        for reveal in self.reveals.values() {
            for i in 0..32 {
                combined[i] ^= reveal.content.value[i];
            }
        }

        // Generate dice values
        let die1 = (combined[0] % 6) + 1;
        let die2 = (combined[1] % 6) + 1;

        Some((die1, die2))
    }

    /// Force settlement when consensus cannot be reached
    fn force_settlement(&mut self) {
        self.phase = ConsensusPhase::Settlement;

        // Apply penalties to non-participants
        for peer in &self.participants {
            if !self.votes.contains_key(peer) {
                *self.penalties.entry(*peer).or_insert(0.0) += PENALTY_MULTIPLIER * 2.0;
                // Double penalty for blocking settlement
            }
        }
    }

    /// Execute settlement based on approved proposal
    pub fn execute_settlement(&mut self, game_id: GameId) -> Result<Vec<(PeerId, i64)>, Error> {
        if self.phase != ConsensusPhase::Settlement {
            return Err(Error::InvalidState("Not in settlement phase".into()));
        }

        let proposal = self
            .current_proposal
            .as_ref()
            .ok_or_else(|| Error::InvalidState("No proposal to settle".into()))?;

        let mut results = Vec::new();

        // Process each settlement with penalty adjustments
        for settlement in &proposal.content.settlements {
            let penalty = self
                .penalties
                .get(&settlement.player)
                .copied()
                .unwrap_or(0.0);
            let adjusted_amount = if settlement.amount > 0 {
                // Reduce winnings by penalty
                (settlement.amount as f64 * (1.0 - penalty)) as i64
            } else {
                // Increase losses by penalty
                (settlement.amount as f64 * (1.0 + penalty)) as i64
            };

            // Execute treasury settlement
            use crate::protocol::CrapTokens;
            if adjusted_amount > 0 {
                self.treasury.settle_bet(
                    game_id,
                    CrapTokens::new_unchecked(settlement.locked_amount),
                    CrapTokens::new_unchecked(adjusted_amount as u64),
                )?;
            } else {
                self.treasury.settle_bet(
                    game_id,
                    CrapTokens::new_unchecked(settlement.locked_amount),
                    CrapTokens::new_unchecked(0),
                )?;
            }

            results.push((settlement.player, adjusted_amount));
        }

        self.phase = ConsensusPhase::Completed;
        Ok(results)
    }

    /// Get current phase
    pub fn current_phase(&self) -> ConsensusPhase {
        self.phase
    }

    /// Check if consensus is stuck and needs intervention
    pub fn is_stuck(&self) -> bool {
        match self.phase {
            ConsensusPhase::Commit => self.phase_start.elapsed() > COMMIT_TIMEOUT * 2,
            ConsensusPhase::Reveal => self.phase_start.elapsed() > REVEAL_TIMEOUT * 2,
            ConsensusPhase::Vote => self.phase_start.elapsed() > SETTLEMENT_TIMEOUT * 2,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;

    #[test]
    fn test_signed_message() {
        let signing_key = SigningKey::generate(&mut OsRng);

        let commit = RandomnessCommit {
            round_id: 1,
            commitment: [0; 32],
            peer_id: [1; 32],
        };

        let signed = SignedMessage::new(commit, &signing_key);
        assert!(signed.verify());

        // Tamper with signature
        let mut tampered = signed.clone();
        tampered.signature[0] ^= 1;
        assert!(!tampered.verify());
    }

    #[test]
    fn test_consensus_round() {
        let treasury = Arc::new(crate::protocol::treasury::TreasuryManager::new());
        let signing_key = SigningKey::generate(&mut OsRng);
        let mut engine = RobustConsensusEngine::new(signing_key, treasury);

        let participants = vec![[1; 32], [2; 32], [3; 32]].into_iter().collect();
        let round_id = engine.start_round(participants);

        assert_eq!(round_id, 1);
        assert_eq!(engine.current_phase(), ConsensusPhase::Commit);

        // Submit commit
        let value = [42; 32];
        let nonce = [24; 32];
        let _commit = engine.submit_commit(value, nonce);

        assert!(!engine.commits.is_empty());
    }
}
