//! Anti-Cheat Integration with Consensus Validation
//!
//! This module provides comprehensive anti-cheat mechanisms integrated with
//! the consensus system, detecting and preventing various forms of cheating
//! in the decentralized casino environment.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::RwLock;

use crate::error::Result;
use crate::protocol::consensus::engine::{GameOperation, GameProposal};
use crate::protocol::consensus::ProposalId;
use crate::protocol::craps::{Bet, BetType, DiceRoll};
use crate::protocol::p2p_messages::{CheatType, ConsensusMessage};
use crate::protocol::{CrapTokens, GameId, Hash256, PeerId, Signature};

/// Anti-cheat configuration
#[derive(Debug, Clone)]
pub struct AntiCheatConfig {
    /// Maximum allowed time skew for operations
    pub max_time_skew: Duration,
    /// Minimum time between operations from same peer
    pub min_operation_interval: Duration,
    /// Maximum bet amount relative to balance
    pub max_bet_ratio: f64,
    /// Suspicious behavior threshold
    pub suspicion_threshold: u32,
    /// Evidence retention period
    pub evidence_retention: Duration,
    /// Maximum dice value (should be 6)
    pub max_dice_value: u8,
    /// Minimum dice value (should be 1)
    pub min_dice_value: u8,
    /// Statistical anomaly threshold
    pub anomaly_threshold: f64,
}

impl Default for AntiCheatConfig {
    fn default() -> Self {
        Self {
            max_time_skew: Duration::from_secs(30),
            min_operation_interval: Duration::from_millis(100),
            max_bet_ratio: 1.0, // Can't bet more than current balance
            suspicion_threshold: 3,
            evidence_retention: Duration::from_secs(3600), // 1 hour
            max_dice_value: 6,
            min_dice_value: 1,
            anomaly_threshold: 0.001, // 0.1% probability threshold
        }
    }
}

/// Evidence of cheating behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheatEvidence {
    /// Evidence ID
    pub evidence_id: [u8; 32],
    /// Suspected cheater
    pub suspect: PeerId,
    /// Type of cheat detected
    pub cheat_type: CheatType,
    /// Evidence data
    pub evidence_data: Vec<u8>,
    /// Timestamp when detected
    pub detected_at: u64,
    /// Witnesses (peers that can verify)
    pub witnesses: Vec<PeerId>,
    /// Severity score (0.0 to 1.0)
    pub severity: f64,
    /// Related operation/proposal ID
    pub related_operation: Option<ProposalId>,
}

/// Peer behavior tracking
#[derive(Debug, Clone)]
struct PeerBehaviorProfile {
    peer_id: PeerId,
    operations_count: u64,
    last_operation_time: Instant,
    total_bets_placed: CrapTokens,
    total_winnings: CrapTokens,
    dice_rolls_witnessed: Vec<DiceRoll>,
    suspicious_activities: Vec<CheatType>,
    trust_score: f64, // 0.0 to 1.0
    statistical_anomalies: u32,
}

/// Statistical analysis for randomness validation
#[derive(Debug, Clone)]
struct RandomnessStats {
    dice_outcomes: HashMap<u8, u64>, // Die face -> count
    total_rolls: u64,
    expected_frequency: f64,
    chi_square_value: f64,
    last_update: Instant,
}

/// Anti-cheat detection and validation system
pub struct AntiCheatValidator {
    config: AntiCheatConfig,
    game_id: GameId,
    local_peer_id: PeerId,

    // Evidence collection
    cheat_evidence: Arc<RwLock<HashMap<[u8; 32], CheatEvidence>>>,

    // Peer behavior tracking
    peer_profiles: Arc<RwLock<HashMap<PeerId, PeerBehaviorProfile>>>,

    // Statistical analysis
    randomness_stats: Arc<RwLock<HashMap<PeerId, RandomnessStats>>>,
    global_randomness_stats: Arc<RwLock<RandomnessStats>>,

    // Operation validation
    recent_operations: Arc<RwLock<HashMap<PeerId, VecDeque<(Instant, GameOperation)>>>>,

    // Consensus validation
    proposal_signatures: Arc<RwLock<HashMap<ProposalId, HashMap<PeerId, Signature>>>>,
    state_checksums: Arc<RwLock<HashMap<Hash256, HashSet<PeerId>>>>,
}

impl AntiCheatValidator {
    /// Create new anti-cheat validator
    pub fn new(config: AntiCheatConfig, game_id: GameId, local_peer_id: PeerId) -> Self {
        Self {
            config,
            game_id,
            local_peer_id,
            cheat_evidence: Arc::new(RwLock::new(HashMap::new())),
            peer_profiles: Arc::new(RwLock::new(HashMap::new())),
            randomness_stats: Arc::new(RwLock::new(HashMap::new())),
            global_randomness_stats: Arc::new(RwLock::new(RandomnessStats {
                dice_outcomes: HashMap::new(),
                total_rolls: 0,
                expected_frequency: 1.0 / 6.0, // Fair die
                chi_square_value: 0.0,
                last_update: Instant::now(),
            })),
            recent_operations: Arc::new(RwLock::new(HashMap::new())),
            proposal_signatures: Arc::new(RwLock::new(HashMap::new())),
            state_checksums: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Validate a game operation for potential cheating
    pub async fn validate_operation(
        &self,
        peer_id: PeerId,
        operation: &GameOperation,
    ) -> Result<ValidationResult> {
        let mut violations = Vec::new();

        // Time-based validation
        violations.extend(self.validate_timing(peer_id, operation).await?);

        // Operation-specific validation
        match operation {
            GameOperation::PlaceBet { player, bet, .. } => {
                violations.extend(self.validate_bet(*player, bet).await?);
            }
            GameOperation::ProcessRoll { dice_roll, .. } => {
                violations.extend(self.validate_dice_roll(peer_id, dice_roll).await?);
            }
            GameOperation::UpdateBalances { changes, .. } => {
                violations.extend(self.validate_balance_changes(peer_id, changes).await?);
            }
            _ => {}
        }

        // Update peer behavior profile
        self.update_peer_profile(peer_id, operation.clone()).await;

        // Record operation for future validation
        self.record_operation(peer_id, operation.clone()).await;

        if violations.is_empty() {
            Ok(ValidationResult::Valid)
        } else {
            // Generate cheat evidence
            let evidence = self
                .create_cheat_evidence(peer_id, violations.clone())
                .await;
            Ok(ValidationResult::Suspicious {
                violations,
                evidence,
            })
        }
    }

    /// Validate proposal signatures and consensus integrity
    pub async fn validate_proposal(&self, proposal: &GameProposal) -> Result<ValidationResult> {
        let mut violations = Vec::new();

        // Signature validation
        if !self.validate_proposal_signature(proposal).await? {
            violations.push(CheatType::SignatureForgery);
        }

        // State transition validation
        violations.extend(self.validate_state_transition(proposal).await?);

        // Timestamp validation
        violations.extend(self.validate_proposal_timestamp(proposal).await?);

        // Store signature for cross-validation
        self.store_proposal_signature(proposal).await;

        if violations.is_empty() {
            Ok(ValidationResult::Valid)
        } else {
            let evidence = self
                .create_cheat_evidence(proposal.proposer, violations.clone())
                .await;
            Ok(ValidationResult::Suspicious {
                violations,
                evidence,
            })
        }
    }

    /// Validate consensus message integrity
    pub async fn validate_consensus_message(
        &self,
        message: &ConsensusMessage,
    ) -> Result<ValidationResult> {
        let mut violations = Vec::new();

        // Basic message validation
        violations.extend(self.validate_message_structure(message).await?);

        // Signature validation
        if !self.validate_message_signature(message).await? {
            violations.push(CheatType::SignatureForgery);
        }

        // Replay attack detection
        if self.is_replay_attack(message).await {
            violations.push(CheatType::ConsensusViolation);
        }

        if violations.is_empty() {
            Ok(ValidationResult::Valid)
        } else {
            let evidence = self
                .create_cheat_evidence(message.sender, violations.clone())
                .await;
            Ok(ValidationResult::Suspicious {
                violations,
                evidence,
            })
        }
    }

    /// Validate timing constraints
    async fn validate_timing(
        &self,
        peer_id: PeerId,
        operation: &GameOperation,
    ) -> Result<Vec<CheatType>> {
        let mut violations = Vec::new();
        let now = Instant::now();

        // Check minimum interval between operations
        let mut recent_ops = self.recent_operations.write().await;
        if let Some(peer_ops) = recent_ops.get_mut(&peer_id) {
            if let Some((last_time, _)) = peer_ops.back() {
                if now.duration_since(*last_time) < self.config.min_operation_interval {
                    violations.push(CheatType::TimestampManipulation);
                }
            }
        }

        // Check operation timestamp if available
        let operation_timestamp = match operation {
            GameOperation::PlaceBet { .. } => {
                // Get timestamp from bet - simplified for now
                SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
            }
            _ => SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        let current_time = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        if operation_timestamp > current_time + self.config.max_time_skew.as_secs() {
            violations.push(CheatType::TimestampManipulation);
        }

        Ok(violations)
    }

    /// Validate bet operation
    async fn validate_bet(&self, player: PeerId, bet: &Bet) -> Result<Vec<CheatType>> {
        let mut violations = Vec::new();

        // Validate bet amount
        if bet.amount == CrapTokens::ZERO {
            violations.push(CheatType::InvalidStateTransition);
        }

        // Check against player balance (would need to get from game state)
        // For now, simplified validation
        if bet.amount.amount() > 1_000_000 {
            // Arbitrary large amount
            violations.push(CheatType::BalanceViolation);
        }

        // Validate bet type
        if !self.is_valid_bet_type(&bet.bet_type) {
            violations.push(CheatType::InvalidStateTransition);
        }

        // Check for rapid-fire betting (update peer profile)
        let mut profiles = self.peer_profiles.write().await;
        if let Some(profile) = profiles.get_mut(&player) {
            let time_since_last = profile.last_operation_time.elapsed();
            if time_since_last < Duration::from_millis(50) {
                // Too fast
                violations.push(CheatType::TimestampManipulation);
            }
            profile.last_operation_time = Instant::now();
        }

        Ok(violations)
    }

    /// Validate dice roll
    async fn validate_dice_roll(
        &self,
        peer_id: PeerId,
        dice_roll: &DiceRoll,
    ) -> Result<Vec<CheatType>> {
        let mut violations = Vec::new();

        // Basic value validation
        if dice_roll.die1 < self.config.min_dice_value
            || dice_roll.die1 > self.config.max_dice_value
        {
            violations.push(CheatType::InvalidRoll);
        }
        if dice_roll.die2 < self.config.min_dice_value
            || dice_roll.die2 > self.config.max_dice_value
        {
            violations.push(CheatType::InvalidRoll);
        }

        // Update statistical analysis
        self.update_randomness_stats(peer_id, dice_roll).await;

        // Check for statistical anomalies
        if self.detect_statistical_anomaly(peer_id).await? {
            violations.push(CheatType::InvalidRoll);
        }

        Ok(violations)
    }

    /// Validate balance changes
    async fn validate_balance_changes(
        &self,
        peer_id: PeerId,
        changes: &rustc_hash::FxHashMap<PeerId, CrapTokens>,
    ) -> Result<Vec<CheatType>> {
        let mut violations = Vec::new();

        // Check for impossible balance changes
        let total_change: i64 = changes.values().map(|c| c.amount() as i64).sum();

        // Conservation check - total should be zero or negative (house edge)
        if total_change > 0 {
            violations.push(CheatType::BalanceViolation);
        }

        // Check for self-benefit
        if let Some(self_change) = changes.get(&peer_id) {
            if self_change.amount() > 0 {
                // Player is giving themselves money - highly suspicious
                violations.push(CheatType::BalanceViolation);
            }
        }

        Ok(violations)
    }

    /// Validate proposal signature
    async fn validate_proposal_signature(&self, proposal: &GameProposal) -> Result<bool> {
        // Simplified signature validation
        // In a real implementation, this would verify the cryptographic signature
        // against the proposer's public key

        // Check if signature is not all zeros (invalid)
        Ok(proposal.signature.0 != [0u8; 64])
    }

    /// Validate state transition
    async fn validate_state_transition(&self, proposal: &GameProposal) -> Result<Vec<CheatType>> {
        let mut violations = Vec::new();

        // Check sequence number progression
        if proposal.proposed_state.sequence_number <= proposal.proposed_state.sequence_number {
            // Sequence should always increase
            violations.push(CheatType::InvalidStateTransition);
        }

        // Validate balance conservation
        let current_total: u64 = proposal
            .proposed_state
            .player_balances
            .values()
            .map(|b| b.0)
            .sum();
        // In real implementation, would compare against previous state

        // Check for impossible state changes
        if proposal.proposed_state.confirmations > 1000 {
            // Arbitrary sanity check
            violations.push(CheatType::InvalidStateTransition);
        }

        Ok(violations)
    }

    /// Validate proposal timestamp
    async fn validate_proposal_timestamp(&self, proposal: &GameProposal) -> Result<Vec<CheatType>> {
        let mut violations = Vec::new();

        let current_time = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let max_future_time = current_time + self.config.max_time_skew.as_secs();

        if proposal.timestamp > max_future_time {
            violations.push(CheatType::TimestampManipulation);
        }

        Ok(violations)
    }

    /// Validate message structure
    async fn validate_message_structure(
        &self,
        message: &ConsensusMessage,
    ) -> Result<Vec<CheatType>> {
        let mut violations = Vec::new();

        // Check message game ID
        if message.game_id != self.game_id {
            violations.push(CheatType::ConsensusViolation);
        }

        // Check for reasonable message size
        if message.payload_size() > 1_000_000 {
            // 1MB limit
            violations.push(CheatType::ConsensusViolation);
        }

        // Validate timestamp
        let current_time = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        if message.timestamp > current_time + self.config.max_time_skew.as_secs() {
            violations.push(CheatType::TimestampManipulation);
        }

        Ok(violations)
    }

    /// Validate message signature
    async fn validate_message_signature(&self, message: &ConsensusMessage) -> Result<bool> {
        // Simplified signature validation
        Ok(message.signature.0 != [0u8; 64])
    }

    /// Check for replay attacks
    async fn is_replay_attack(&self, message: &ConsensusMessage) -> bool {
        // Check if we've seen this exact message before
        // In real implementation, would maintain a cache of recent message IDs
        false // Simplified for now
    }

    /// Update randomness statistics for a peer
    async fn update_randomness_stats(&self, peer_id: PeerId, dice_roll: &DiceRoll) {
        let mut stats_map = self.randomness_stats.write().await;
        let stats = stats_map.entry(peer_id).or_insert_with(|| RandomnessStats {
            dice_outcomes: HashMap::new(),
            total_rolls: 0,
            expected_frequency: 1.0 / 6.0,
            chi_square_value: 0.0,
            last_update: Instant::now(),
        });

        // Record dice outcomes
        *stats.dice_outcomes.entry(dice_roll.die1).or_insert(0) += 1;
        *stats.dice_outcomes.entry(dice_roll.die2).or_insert(0) += 1;
        stats.total_rolls += 2; // Two dice
        stats.last_update = Instant::now();

        // Calculate chi-square statistic
        if stats.total_rolls >= 30 {
            // Minimum sample size
            stats.chi_square_value =
                self.calculate_chi_square(&stats.dice_outcomes, stats.total_rolls);
        }

        // Update global stats
        let mut global_stats = self.global_randomness_stats.write().await;
        *global_stats
            .dice_outcomes
            .entry(dice_roll.die1)
            .or_insert(0) += 1;
        *global_stats
            .dice_outcomes
            .entry(dice_roll.die2)
            .or_insert(0) += 1;
        global_stats.total_rolls += 2;
    }

    /// Detect statistical anomalies in dice rolls
    async fn detect_statistical_anomaly(&self, peer_id: PeerId) -> Result<bool> {
        let stats_map = self.randomness_stats.read().await;

        if let Some(stats) = stats_map.get(&peer_id) {
            if stats.total_rolls >= 30 {
                // Minimum sample size
                // Chi-square test with 5 degrees of freedom (6 outcomes - 1)
                // Critical value at 0.001 significance level is approximately 20.515
                let critical_value = 20.515;

                if stats.chi_square_value > critical_value {
                    log::warn!(
                        "Statistical anomaly detected for peer {:?}: chi-square = {:.3}",
                        peer_id,
                        stats.chi_square_value
                    );
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    /// Calculate chi-square statistic
    fn calculate_chi_square(&self, outcomes: &HashMap<u8, u64>, total_rolls: u64) -> f64 {
        let expected_per_outcome = total_rolls as f64 / 6.0; // Expected frequency for fair die
        let mut chi_square = 0.0;

        for face in 1..=6 {
            let observed = *outcomes.get(&face).unwrap_or(&0) as f64;
            let expected = expected_per_outcome;
            chi_square += (observed - expected).powi(2) / expected;
        }

        chi_square
    }

    /// Check if bet type is valid
    fn is_valid_bet_type(&self, bet_type: &BetType) -> bool {
        // All bet types defined in the protocol are valid
        // This could be extended to check game phase constraints
        true
    }

    /// Update peer behavior profile
    async fn update_peer_profile(&self, peer_id: PeerId, operation: GameOperation) {
        let mut profiles = self.peer_profiles.write().await;
        let profile = profiles
            .entry(peer_id)
            .or_insert_with(|| PeerBehaviorProfile {
                peer_id,
                operations_count: 0,
                last_operation_time: Instant::now(),
                total_bets_placed: CrapTokens::ZERO,
                total_winnings: CrapTokens::ZERO,
                dice_rolls_witnessed: Vec::new(),
                suspicious_activities: Vec::new(),
                trust_score: 1.0, // Start with full trust
                statistical_anomalies: 0,
            });

        profile.operations_count += 1;
        profile.last_operation_time = Instant::now();

        match operation {
            GameOperation::PlaceBet { bet, .. } => {
                profile.total_bets_placed = profile.total_bets_placed.saturating_add(bet.amount);
            }
            GameOperation::ProcessRoll { dice_roll, .. } => {
                profile.dice_rolls_witnessed.push(dice_roll);
                // Keep only recent rolls (limit memory usage)
                if profile.dice_rolls_witnessed.len() > 100 {
                    profile.dice_rolls_witnessed.remove(0);
                }
            }
            _ => {}
        }
    }

    /// Record operation for timing validation
    async fn record_operation(&self, peer_id: PeerId, operation: GameOperation) {
        let mut recent_ops = self.recent_operations.write().await;
        let peer_ops = recent_ops.entry(peer_id).or_default();

        peer_ops.push_back((Instant::now(), operation));

        // Keep only recent operations (last 10)
        if peer_ops.len() > 10 {
            peer_ops.pop_front();
        }
    }

    /// Store proposal signature for validation
    async fn store_proposal_signature(&self, proposal: &GameProposal) {
        let mut signatures = self.proposal_signatures.write().await;
        signatures
            .entry(proposal.id)
            .or_default()
            .insert(proposal.proposer, proposal.signature);
    }

    /// Create cheat evidence
    async fn create_cheat_evidence(
        &self,
        suspect: PeerId,
        violations: Vec<CheatType>,
    ) -> CheatEvidence {
        let evidence_id = {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(suspect);
            hasher.update(
                &SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
                    .to_le_bytes(),
            );
            for violation in &violations {
                hasher.update(format!("{:?}", violation).as_bytes());
            }
            hasher.finalize().into()
        };

        // Determine primary violation type
        let primary_violation = violations
            .first()
            .cloned()
            .unwrap_or(CheatType::ConsensusViolation);

        // Calculate severity
        let severity = match primary_violation {
            CheatType::BalanceViolation => 1.0, // Critical
            CheatType::InvalidStateTransition => 0.9,
            CheatType::SignatureForgery => 0.8,
            CheatType::DoubleVoting => 0.7,
            CheatType::InvalidRoll => 0.6,
            CheatType::TimestampManipulation => 0.5,
            CheatType::ConsensusViolation => 0.4,
        };

        let evidence = CheatEvidence {
            evidence_id,
            suspect,
            cheat_type: primary_violation,
            evidence_data: bincode::serialize(&violations).unwrap_or_default(),
            detected_at: SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            witnesses: vec![self.local_peer_id],
            severity,
            related_operation: None,
        };

        // Store evidence
        self.cheat_evidence
            .write()
            .await
            .insert(evidence_id, evidence.clone());

        evidence
    }

    /// Get peer trust score
    pub async fn get_peer_trust_score(&self, peer_id: PeerId) -> f64 {
        let profiles = self.peer_profiles.read().await;
        profiles.get(&peer_id).map(|p| p.trust_score).unwrap_or(0.5)
    }

    /// Get all cheat evidence for a peer
    pub async fn get_peer_evidence(&self, peer_id: PeerId) -> Vec<CheatEvidence> {
        let evidence_map = self.cheat_evidence.read().await;
        evidence_map
            .values()
            .filter(|e| e.suspect == peer_id)
            .cloned()
            .collect()
    }

    /// Clean up old evidence
    pub async fn cleanup_old_evidence(&self) {
        let mut evidence_map = self.cheat_evidence.write().await;
        let cutoff_time = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .saturating_sub(self.config.evidence_retention.as_secs());

        evidence_map.retain(|_, evidence| evidence.detected_at >= cutoff_time);
    }

    /// Get anti-cheat statistics
    pub async fn get_anti_cheat_stats(&self) -> AntiCheatStats {
        let evidence_map = self.cheat_evidence.read().await;
        let profiles = self.peer_profiles.read().await;
        let randomness_stats = self.randomness_stats.read().await;

        AntiCheatStats {
            total_evidence_collected: evidence_map.len(),
            active_investigations: evidence_map
                .values()
                .filter(|e| {
                    e.detected_at
                        >= SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs()
                            .saturating_sub(3600)
                }) // Last hour
                .count(),
            monitored_peers: profiles.len(),
            average_trust_score: if profiles.is_empty() {
                1.0
            } else {
                profiles.values().map(|p| p.trust_score).sum::<f64>() / profiles.len() as f64
            },
            statistical_anomalies_detected: profiles
                .values()
                .map(|p| p.statistical_anomalies)
                .sum(),
            randomness_tests_performed: randomness_stats.len(),
        }
    }
}

/// Validation result
#[derive(Debug, Clone)]
pub enum ValidationResult {
    Valid,
    Suspicious {
        violations: Vec<CheatType>,
        evidence: CheatEvidence,
    },
}

/// Anti-cheat statistics
#[derive(Debug, Clone)]
pub struct AntiCheatStats {
    pub total_evidence_collected: usize,
    pub active_investigations: usize,
    pub monitored_peers: usize,
    pub average_trust_score: f64,
    pub statistical_anomalies_detected: u32,
    pub randomness_tests_performed: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::DiceRoll;

    #[tokio::test]
    async fn test_dice_validation() {
        let config = AntiCheatConfig::default();
        let validator = AntiCheatValidator::new(config, [0u8; 16], [1u8; 32]);

        let valid_roll = DiceRoll::new(3, 4).unwrap();
        let violations = validator
            .validate_dice_roll([2u8; 32], &valid_roll)
            .await
            .unwrap();
        assert!(violations.is_empty());

        // Invalid roll with impossible values
        let invalid_roll = DiceRoll {
            die1: 7,
            die2: 8,
            timestamp: 0,
        };
        let violations = validator
            .validate_dice_roll([2u8; 32], &invalid_roll)
            .await
            .unwrap();
        assert!(!violations.is_empty());
        assert!(violations.contains(&CheatType::InvalidRoll));
    }

    #[tokio::test]
    async fn test_statistical_analysis() {
        let config = AntiCheatConfig::default();
        let validator = AntiCheatValidator::new(config, [0u8; 16], [1u8; 32]);

        // Simulate biased dice (all 6s)
        let biased_peer = [3u8; 32];
        for _ in 0..50 {
            let biased_roll = DiceRoll {
                die1: 6,
                die2: 6,
                timestamp: 0,
            };
            validator
                .update_randomness_stats(biased_peer, &biased_roll)
                .await;
        }

        let is_anomalous = validator
            .detect_statistical_anomaly(biased_peer)
            .await
            .unwrap();
        assert!(is_anomalous);
    }
}
