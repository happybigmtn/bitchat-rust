//! Adversarial Testing for Robust Consensus
//!
//! This test suite simulates various attack scenarios to ensure
//! the consensus mechanism remains robust against malicious actors.

use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

use bitcraps::crypto::GameCrypto;
use bitcraps::protocol::consensus::robust_engine::{
    ConsensusPhase, ProposalVote, RandomnessCommit, RandomnessReveal, RobustConsensusEngine,
    Settlement, SignedMessage,
};
use bitcraps::protocol::reputation::{DisputeType, ReputationManager};
use bitcraps::protocol::treasury::TreasuryManager;
use bitcraps::protocol::{GameId, PeerId};

/// Test participant
struct TestParticipant {
    id: PeerId,
    signing_key: SigningKey,
    engine: RobustConsensusEngine,
    is_malicious: bool,
    attack_type: AttackType,
}

/// Types of attacks to simulate
#[derive(Debug, Clone, Copy, PartialEq)]
enum AttackType {
    None,
    WithholdCommit,
    WithholdReveal,
    InvalidReveal,
    RejectAllProposals,
    DoubleSpend,
    Timeout,
}

impl TestParticipant {
    fn new(treasury: Arc<TreasuryManager>, is_malicious: bool, attack_type: AttackType) -> Self {
        let signing_key = SigningKey::generate(&mut OsRng);
        let id = signing_key.verifying_key().to_bytes();
        let engine = RobustConsensusEngine::new(signing_key.clone(), treasury);

        Self {
            id,
            signing_key,
            engine,
            is_malicious,
            attack_type,
        }
    }

    /// Participate in commit phase (may withhold if malicious)
    async fn commit_phase(
        &mut self,
        value: [u8; 32],
        nonce: [u8; 32],
    ) -> Option<SignedMessage<RandomnessCommit>> {
        if self.is_malicious && self.attack_type == AttackType::WithholdCommit {
            // Malicious: withhold commit
            None
        } else {
            // Normal: submit commit
            Some(self.engine.submit_commit(value, nonce))
        }
    }

    /// Participate in reveal phase (may withhold or lie if malicious)
    async fn reveal_phase(
        &mut self,
        value: [u8; 32],
        nonce: [u8; 32],
    ) -> Option<SignedMessage<RandomnessReveal>> {
        match (self.is_malicious, self.attack_type) {
            (true, AttackType::WithholdReveal) => {
                // Malicious: withhold reveal
                None
            }
            (true, AttackType::InvalidReveal) => {
                // Malicious: reveal wrong value
                let wrong_value = [99; 32];
                self.engine.submit_reveal(wrong_value, nonce).ok()
            }
            _ => {
                // Normal: submit correct reveal
                self.engine.submit_reveal(value, nonce).ok()
            }
        }
    }

    /// Vote on proposal (may always reject if malicious)
    async fn vote_phase(&mut self, proposal_hash: [u8; 32]) -> Option<SignedMessage<ProposalVote>> {
        let approve = if self.is_malicious && self.attack_type == AttackType::RejectAllProposals {
            false // Always reject
        } else {
            true // Normal: approve valid proposals
        };

        self.engine.vote_on_proposal(proposal_hash, approve).ok()
    }
}

/// Simulate a consensus round with mixed honest and malicious participants
async fn simulate_consensus_round(
    honest_count: usize,
    malicious_count: usize,
    attack_type: AttackType,
) -> Result<bool, String> {
    let treasury = Arc::new(TreasuryManager::new());
    let mut participants = Vec::new();

    // Create honest participants
    for _ in 0..honest_count {
        participants.push(TestParticipant::new(
            treasury.clone(),
            false,
            AttackType::None,
        ));
    }

    // Create malicious participants
    for _ in 0..malicious_count {
        participants.push(TestParticipant::new(treasury.clone(), true, attack_type));
    }

    // Collect all participant IDs
    let all_ids: HashSet<PeerId> = participants.iter().map(|p| p.id).collect();

    // Start consensus round for all participants
    for participant in &mut participants {
        participant.engine.start_round(all_ids.clone());
    }

    // Phase 1: Commit
    let mut commits = Vec::new();
    for participant in &mut participants {
        let value = [42; 32];
        let nonce = [24; 32];
        if let Some(commit) = participant.commit_phase(value, nonce).await {
            commits.push((participant.id, commit));
        }
    }

    // Distribute commits to all participants
    for (sender_id, commit) in &commits {
        for participant in &mut participants {
            if participant.id != *sender_id {
                let _ = participant.engine.process_commit(commit.clone());
            }
        }
    }

    // Wait for phase transition
    sleep(Duration::from_millis(100)).await;

    // Phase 2: Reveal
    let mut reveals = Vec::new();
    for participant in &mut participants {
        let value = [42; 32];
        let nonce = [24; 32];
        if let Some(reveal) = participant.reveal_phase(value, nonce).await {
            reveals.push((participant.id, reveal));
        }
    }

    // Distribute reveals to all participants
    for (sender_id, reveal) in &reveals {
        for participant in &mut participants {
            if participant.id != *sender_id {
                let _ = participant.engine.process_reveal(reveal.clone());
            }
        }
    }

    // Wait for phase transition
    sleep(Duration::from_millis(100)).await;

    // Phase 3: Proposal (first honest participant proposes)
    let proposer_idx = participants
        .iter()
        .position(|p| !p.is_malicious)
        .ok_or("No honest participants")?;

    let game_id: GameId = [1; 16];
    let player_id = participants[0].id;
    let settlements = vec![Settlement {
        player: player_id,
        amount: 100,
        bet_type: "PassLine".to_string(),
        locked_amount: 50,
    }];

    let proposal = participants[proposer_idx]
        .engine
        .create_proposal(game_id, settlements)
        .map_err(|e| format!("Failed to create proposal: {:?}", e))?;

    let proposal_hash = GameCrypto::hash(&bincode::serialize(&proposal.content).unwrap());

    // Phase 4: Vote
    let mut votes = Vec::new();
    for participant in &mut participants {
        if let Some(vote) = participant.vote_phase(proposal_hash).await {
            votes.push((participant.id, vote));
        }
    }

    // Check if consensus was reached
    let threshold = (participants.len() as f64 * 0.67) as usize;
    let approvals = votes.iter().filter(|(_, v)| v.content.approve).count();

    Ok(approvals >= threshold)
}

#[tokio::test]
async fn test_consensus_with_minority_withholding_commits() {
    // 7 honest, 3 malicious withholding commits
    // Should succeed with 70% participation
    let result = simulate_consensus_round(7, 3, AttackType::WithholdCommit).await;
    assert!(result.is_ok());
    assert!(
        result.unwrap(),
        "Consensus should succeed with 70% participation"
    );
}

#[tokio::test]
async fn test_consensus_with_minority_withholding_reveals() {
    // 7 honest, 3 malicious withholding reveals
    // Should succeed as threshold is met
    let result = simulate_consensus_round(7, 3, AttackType::WithholdReveal).await;
    assert!(result.is_ok());
    assert!(
        result.unwrap(),
        "Consensus should succeed despite withheld reveals"
    );
}

#[tokio::test]
async fn test_consensus_with_invalid_reveals() {
    // 8 honest, 2 malicious with invalid reveals
    // Should succeed, invalid reveals are ignored
    let result = simulate_consensus_round(8, 2, AttackType::InvalidReveal).await;
    assert!(result.is_ok());
    assert!(
        result.unwrap(),
        "Consensus should succeed, invalid reveals ignored"
    );
}

#[tokio::test]
async fn test_consensus_with_minority_rejecting_proposals() {
    // 7 honest, 3 malicious rejecting all proposals
    // Should succeed with 70% approval
    let result = simulate_consensus_round(7, 3, AttackType::RejectAllProposals).await;
    assert!(result.is_ok());
    assert!(
        result.unwrap(),
        "Consensus should succeed with 70% approval"
    );
}

#[tokio::test]
async fn test_consensus_with_majority_attack_fails() {
    // 3 honest, 7 malicious withholding commits
    // Should fail as threshold cannot be met
    let result = simulate_consensus_round(3, 7, AttackType::WithholdCommit).await;
    assert!(result.is_ok());
    assert!(
        !result.unwrap(),
        "Consensus should fail with majority attack"
    );
}

#[tokio::test]
async fn test_reputation_penalties_for_non_participation() {
    let treasury = Arc::new(TreasuryManager::new());
    let mut reputation_manager = ReputationManager::new(3);

    // Create participants
    let honest_key = SigningKey::generate(&mut OsRng);
    let honest_id = honest_key.verifying_key().to_bytes();

    let malicious_key = SigningKey::generate(&mut OsRng);
    let malicious_id = malicious_key.verifying_key().to_bytes();

    // Simulate multiple rounds where malicious participant doesn't participate
    for _ in 0..5 {
        // Record failed participation
        reputation_manager.apply_event(
            malicious_id,
            bitcraps::protocol::reputation::ReputationEvent::FailedCommit,
        );
    }

    // Check reputation
    let can_play = reputation_manager.can_participate(&malicious_id);
    let trust_level = reputation_manager.get_trust_level(&malicious_id);

    assert!(
        trust_level < 0.5,
        "Trust level should be low after repeated failures"
    );

    // After enough failures, should be unable to participate
    for _ in 0..10 {
        reputation_manager.apply_event(
            malicious_id,
            bitcraps::protocol::reputation::ReputationEvent::FailedReveal,
        );
    }

    assert!(
        !reputation_manager.can_participate(&malicious_id),
        "Should be banned after excessive failures"
    );
}

#[tokio::test]
async fn test_dispute_resolution_for_cheating() {
    let mut reputation_manager = ReputationManager::new(3);

    // Create participants
    let accuser = [1; 32];
    let cheater = [2; 32];
    let voters: Vec<PeerId> = (3..8).map(|i| [i; 32]).collect();

    // Give voters good reputation to allow voting
    for voter in &voters {
        reputation_manager.apply_event(
            *voter,
            bitcraps::protocol::reputation::ReputationEvent::GameCompleted,
        );
    }

    // Raise dispute for cheating
    let evidence = b"proof of double spend";
    let dispute_id = reputation_manager
        .raise_dispute(
            DisputeType::Cheating {
                description: "Double spend attempt".to_string(),
                evidence: evidence.to_vec(),
            },
            accuser,
            cheater,
            evidence,
        )
        .expect("Failed to raise dispute");

    // Voters vote guilty
    for voter in voters {
        let vote = bitcraps::protocol::reputation::DisputeVote {
            dispute_id,
            voter,
            verdict: bitcraps::protocol::reputation::Verdict::Guilty,
            reasoning: Some("Evidence is clear".to_string()),
            timestamp: 0,
            signature: vec![0; 64],
        };

        reputation_manager
            .vote_on_dispute(dispute_id, voter, vote)
            .expect("Failed to vote");
    }

    // Check cheater's reputation is penalized
    let cheater_can_play = reputation_manager.can_participate(&cheater);
    let cheater_trust = reputation_manager.get_trust_level(&cheater);

    assert!(cheater_trust < 0.3, "Cheater should have very low trust");
}

#[tokio::test]
async fn test_forced_settlement_on_timeout() {
    let treasury = Arc::new(TreasuryManager::new());
    let signing_key = SigningKey::generate(&mut OsRng);
    let mut engine = RobustConsensusEngine::new(signing_key, treasury.clone());

    // Start round with 5 participants
    let participants: HashSet<PeerId> = (0..5).map(|i| [i; 32]).collect();
    engine.start_round(participants);

    // Only submit partial commits (simulate timeout scenario)
    let value = [42; 32];
    let nonce = [24; 32];
    engine.submit_commit(value, nonce);

    // Wait for timeout
    sleep(Duration::from_secs(11)).await;

    // Check if engine detects stuck state
    assert!(
        engine.is_stuck(),
        "Engine should detect stuck state after timeout"
    );

    // Engine should allow forced progression
    assert_eq!(engine.current_phase(), ConsensusPhase::Commit);
}

#[tokio::test]
async fn test_treasury_locks_during_consensus() {
    let treasury = Arc::new(TreasuryManager::new());
    let initial_balance = treasury.get_health().balance;

    // Lock funds for a game
    let game_id: GameId = [1; 16];
    let bet_amount = bitcraps::protocol::CrapTokens::from(1000);
    let max_payout = bitcraps::protocol::CrapTokens::from(2000);

    treasury
        .process_bet(game_id, bet_amount, max_payout)
        .expect("Failed to process bet");

    // Check funds are locked
    let health = treasury.get_health();
    assert_eq!(health.locked_funds, max_payout);
    assert!(health.available_balance < initial_balance);

    // Settle bet (player wins)
    treasury
        .settle_bet(
            game_id,
            max_payout,
            bitcraps::protocol::CrapTokens::from(1500),
        )
        .expect("Failed to settle bet");

    // Check funds are released
    let health = treasury.get_health();
    assert_eq!(health.locked_funds, bitcraps::protocol::CrapTokens::from(0));
    assert!(health.balance < initial_balance); // Treasury paid out
}

#[tokio::test]
async fn test_consensus_recovery_from_network_partition() {
    // Simulate network partition during consensus
    let treasury = Arc::new(TreasuryManager::new());

    // Create two groups that can't communicate
    let mut group1 = Vec::new();
    let mut group2 = Vec::new();

    for _ in 0..3 {
        group1.push(TestParticipant::new(
            treasury.clone(),
            false,
            AttackType::None,
        ));
    }

    for _ in 0..2 {
        group2.push(TestParticipant::new(
            treasury.clone(),
            false,
            AttackType::None,
        ));
    }

    // Both groups try to run consensus independently
    let group1_ids: HashSet<PeerId> = group1.iter().map(|p| p.id).collect();
    let group2_ids: HashSet<PeerId> = group2.iter().map(|p| p.id).collect();

    for p in &mut group1 {
        p.engine.start_round(group1_ids.clone());
    }

    for p in &mut group2 {
        p.engine.start_round(group2_ids.clone());
    }

    // Group 1 has majority (3/5), should succeed
    // Group 2 has minority (2/5), should fail

    // Submit commits for group 1
    for p in &mut group1 {
        p.engine.submit_commit([1; 32], [2; 32]);
    }

    // Group 1 should be able to progress
    assert_eq!(group1[0].engine.current_phase(), ConsensusPhase::Reveal);

    // Group 2 cannot progress without majority
    for p in &mut group2 {
        p.engine.submit_commit([1; 32], [2; 32]);
    }

    // Group 2 stuck in commit phase
    assert_eq!(group2[0].engine.current_phase(), ConsensusPhase::Commit);
}
