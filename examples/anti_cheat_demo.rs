//! Anti-cheat system demonstration
//!
//! Run with: cargo run --example anti_cheat_demo

use bitcraps::error::Result;
use bitcraps::protocol::anti_cheat::{AntiCheatConfig, CheatEvidence};
use bitcraps::protocol::p2p_messages::CheatType;
use bitcraps::protocol::{PeerId, PeerIdExt};
use std::time::{Duration, SystemTime};

fn main() -> Result<()> {
    println!("BitCraps Anti-Cheat Demonstration");
    println!("==================================\n");

    // Create anti-cheat configuration
    let config = AntiCheatConfig {
        max_time_skew: Duration::from_secs(30),
        min_operation_interval: Duration::from_millis(100),
        max_bet_ratio: 1.0,
        suspicion_threshold: 3,
        evidence_retention: Duration::from_secs(3600),
        min_dice_value: 1,
        max_dice_value: 6,
        anomaly_threshold: 0.001,
        chi_square_significance: 0.05,
        min_samples_for_testing: 30,
    };

    println!("Anti-cheat configuration:");
    println!("  Max time skew: {:?}", config.max_time_skew);
    println!(
        "  Min operation interval: {:?}",
        config.min_operation_interval
    );
    println!("  Suspicion threshold: {}", config.suspicion_threshold);
    println!(
        "  Statistical significance: {}",
        config.chi_square_significance
    );
    println!();

    // Demonstration 1: Statistical Anomaly Detection
    println!("Demo 1: Statistical Dice Analysis");
    println!("----------------------------------");

    // Generate normal dice rolls
    println!("Testing normal dice distribution:");
    let mut normal_outcomes = std::collections::HashMap::new();
    for _ in 0..300 {
        let die1 = (rand::random::<u8>() % 6) + 1;
        let die2 = (rand::random::<u8>() % 6) + 1;
        *normal_outcomes.entry(die1).or_insert(0) += 1;
        *normal_outcomes.entry(die2).or_insert(0) += 1;
    }

    let chi_square_normal = calculate_chi_square(&normal_outcomes, 600);
    println!(
        "  Normal dice chi-square: {:.3} (expected: ~11.07)",
        chi_square_normal
    );
    println!("  Distribution: {:?}", normal_outcomes);

    // Generate biased dice rolls
    println!("\nTesting biased dice (weighted toward 6s):");
    let mut biased_outcomes = std::collections::HashMap::new();
    for _ in 0..300 {
        // Biased toward 6s (70% chance of 6)
        let die1 = if rand::random::<f32>() < 0.7 {
            6
        } else {
            (rand::random::<u8>() % 5) + 1
        };
        let die2 = if rand::random::<f32>() < 0.7 {
            6
        } else {
            (rand::random::<u8>() % 5) + 1
        };
        *biased_outcomes.entry(die1).or_insert(0) += 1;
        *biased_outcomes.entry(die2).or_insert(0) += 1;
    }

    let chi_square_biased = calculate_chi_square(&biased_outcomes, 600);
    println!("  Biased dice chi-square: {:.3}", chi_square_biased);
    println!("  Distribution: {:?}", biased_outcomes);

    let critical_value = 20.515; // p < 0.001
    println!("  Critical value (p<0.001): {}", critical_value);
    println!(
        "  Normal dice anomaly: {}",
        chi_square_normal > critical_value
    );
    println!(
        "  Biased dice anomaly: {}",
        chi_square_biased > critical_value
    );

    // Demonstration 2: Cheat Evidence Creation
    println!("\nDemo 2: Cheat Evidence System");
    println!("------------------------------");

    let cheater_id = PeerId::random();
    let evidence = CheatEvidence {
        evidence_id: generate_evidence_id(&cheater_id, CheatType::InvalidRoll),
        suspect: cheater_id,
        cheat_type: CheatType::InvalidRoll,
        evidence_data: vec![6, 6, 6, 6, 6, 6], // All 6s pattern
        detected_at: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        witnesses: vec![PeerId::random(), PeerId::random()],
        severity: 0.8,
        related_operation: None,
    };

    println!("Created cheat evidence:");
    println!("  Suspect: {:?}", hex::encode(&evidence.suspect[..8]));
    println!("  Type: {:?}", evidence.cheat_type);
    println!("  Severity: {:.2}", evidence.severity);
    println!("  Witnesses: {}", evidence.witnesses.len());
    println!("  Evidence data: {:?}", evidence.evidence_data);

    // Demonstrate the three implemented exercises
    exercise_time_attack_detection();
    exercise_reputation_decay();
    tokio::runtime::Runtime::new()?.block_on(exercise_consensus_ban())?;

    println!("✓ Anti-cheat demonstration complete!");
    Ok(())
}

/// Calculate chi-square test statistic for dice fairness
fn calculate_chi_square(outcomes: &std::collections::HashMap<u8, u64>, total_rolls: u64) -> f64 {
    let expected_per_outcome = total_rolls as f64 / 6.0;
    let mut chi_square = 0.0;

    for face in 1..=6 {
        let observed = *outcomes.get(&face).unwrap_or(&0) as f64;
        chi_square += (observed - expected_per_outcome).powi(2) / expected_per_outcome;
    }

    chi_square
}

/// Generate evidence ID from suspect and cheat type
fn generate_evidence_id(suspect: &PeerId, cheat_type: CheatType) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(suspect);
    hasher.update(format!("{:?}", cheat_type).as_bytes());
    hasher.update(
        &SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_le_bytes(),
    );
    hasher.finalize().into()
}

/// Exercise 1: Implement Time-Based Attack Detection
///
/// Create a function that detects time manipulation attacks
/// by checking for impossible timestamp sequences.
#[allow(dead_code)]
fn exercise_time_attack_detection() {
    println!("Exercise 1: Time Attack Detection");
    println!("=================================\n");

    use std::time::{Duration, SystemTime};

    let mut detector = TimeAttackDetector::new();
    let now = SystemTime::now();

    // Test 1: Normal sequence (should pass)
    println!("Test 1: Normal timestamp sequence");
    let normal_ops = vec![
        TimedOperation {
            id: 1,
            timestamp: now,
            operation: "PlaceBet".to_string(),
        },
        TimedOperation {
            id: 2,
            timestamp: now + Duration::from_secs(1),
            operation: "RollDice".to_string(),
        },
        TimedOperation {
            id: 3,
            timestamp: now + Duration::from_secs(2),
            operation: "PayWinners".to_string(),
        },
    ];

    for op in &normal_ops {
        match detector.validate_timestamp(&op) {
            Ok(true) => println!("  ✓ Operation {} timestamp valid", op.id),
            Ok(false) => println!("  ✗ Operation {} timestamp invalid", op.id),
            Err(e) => println!("  Error validating operation {}: {}", op.id, e),
        }
    }

    // Test 2: Future timestamp attack (should fail)
    println!("\nTest 2: Future timestamp attack");
    let future_op = TimedOperation {
        id: 4,
        timestamp: now + Duration::from_secs(3600), // 1 hour in future
        operation: "FutureAttack".to_string(),
    };

    match detector.validate_timestamp(&future_op) {
        Ok(true) => println!("  ✗ Future timestamp incorrectly accepted"),
        Ok(false) => println!("  ✓ Future timestamp correctly rejected"),
        Err(e) => println!("  Error: {}", e),
    }

    // Test 3: Replay attack with old timestamp (should fail)
    println!("\nTest 3: Replay attack detection");
    let old_op = TimedOperation {
        id: 5,
        timestamp: now - Duration::from_secs(3600), // 1 hour ago
        operation: "ReplayAttack".to_string(),
    };

    match detector.validate_timestamp(&old_op) {
        Ok(true) => println!("  ✗ Replay attack not detected"),
        Ok(false) => println!("  ✓ Replay attack correctly detected"),
        Err(e) => println!("  Error: {}", e),
    }

    // Test 4: Out-of-order sequence (should fail)
    println!("\nTest 4: Out-of-order sequence detection");
    detector.reset(); // Reset for clean test

    let out_of_order_ops = vec![
        TimedOperation {
            id: 6,
            timestamp: now + Duration::from_secs(5),
            operation: "Future".to_string(),
        },
        TimedOperation {
            id: 7,
            timestamp: now + Duration::from_secs(3),
            operation: "Past".to_string(),
        }, // Earlier than previous
    ];

    for op in &out_of_order_ops {
        match detector.validate_timestamp(&op) {
            Ok(true) => println!("  Operation {} accepted", op.id),
            Ok(false) => println!("  ✓ Out-of-order operation {} correctly rejected", op.id),
            Err(e) => println!("  Error: {}", e),
        }
    }

    println!("\n✓ Time attack detection exercise complete!\n");
}

#[derive(Debug, Clone)]
struct TimedOperation {
    id: u32,
    timestamp: SystemTime,
    operation: String,
}

struct TimeAttackDetector {
    max_future_skew: Duration,
    max_past_skew: Duration,
    last_timestamp: Option<SystemTime>,
    operation_window: Duration,
}

impl TimeAttackDetector {
    fn new() -> Self {
        Self {
            max_future_skew: Duration::from_secs(30), // Allow 30s clock skew
            max_past_skew: Duration::from_secs(300),  // Allow 5min for network delays
            last_timestamp: None,
            operation_window: Duration::from_millis(100), // Min time between operations
        }
    }

    fn reset(&mut self) {
        self.last_timestamp = None;
    }

    fn validate_timestamp(&mut self, operation: &TimedOperation) -> Result<bool> {
        let now = SystemTime::now();

        // Check for future timestamps (beyond allowable clock skew)
        if operation.timestamp > now + self.max_future_skew {
            eprintln!(
                "    Future timestamp detected: {:?} > {:?}",
                operation.timestamp,
                now + self.max_future_skew
            );
            return Ok(false);
        }

        // Check for replay attacks (too far in the past)
        if operation.timestamp < now - self.max_past_skew {
            eprintln!(
                "    Replay attack detected: {:?} < {:?}",
                operation.timestamp,
                now - self.max_past_skew
            );
            return Ok(false);
        }

        // Check for out-of-order operations
        if let Some(last_ts) = self.last_timestamp {
            if operation.timestamp < last_ts {
                eprintln!(
                    "    Out-of-order operation: {:?} < {:?}",
                    operation.timestamp, last_ts
                );
                return Ok(false);
            }

            // Check minimum interval between operations (prevent rapid-fire attacks)
            if operation.timestamp < last_ts + self.operation_window {
                eprintln!(
                    "    Operation too fast: interval < {:?}",
                    self.operation_window
                );
                return Ok(false);
            }
        }

        // Update last timestamp for ordering checks
        self.last_timestamp = Some(operation.timestamp);
        Ok(true)
    }
}

/// Exercise 2: Build Reputation Decay System
///
/// Implement a reputation system that decays over time,
/// rewarding recent good behavior and forgetting old sins.
#[allow(dead_code)]
fn exercise_reputation_decay() {
    println!("Exercise 2: Reputation Decay System");
    println!("===================================\n");

    use std::time::{Duration, SystemTime};

    let mut reputation_system = ReputationDecaySystem::new();
    let player_id = PeerId::random();

    println!(
        "Testing reputation decay for player: {:?}\n",
        hex::encode(&player_id[..8])
    );

    // Test 1: Fresh player starts with neutral reputation
    println!("Test 1: Initial reputation");
    let initial_score = reputation_system.get_reputation_score(player_id);
    println!("  Initial score: {:.3}", initial_score);
    assert!((initial_score - 0.5).abs() < 0.001); // Should be 0.5 (neutral)

    // Test 2: Add some violations (negative events)
    println!("\nTest 2: Adding violations");
    reputation_system.add_violation(player_id, "TimeManipulation", 0.3);
    reputation_system.add_violation(player_id, "StatisticalAnomaly", 0.2);
    let after_violations = reputation_system.get_reputation_score(player_id);
    println!("  Score after violations: {:.3}", after_violations);
    assert!(after_violations < initial_score); // Should decrease

    // Test 3: Add good behavior (positive events)
    println!("\nTest 3: Good behavior");
    reputation_system.add_positive_behavior(player_id, "HonestPlay", 0.1);
    reputation_system.add_positive_behavior(player_id, "HelpfulToNetwork", 0.15);
    let after_good_behavior = reputation_system.get_reputation_score(player_id);
    println!("  Score after good behavior: {:.3}", after_good_behavior);
    assert!(after_good_behavior > after_violations); // Should improve

    // Test 4: Time decay simulation
    println!("\nTest 4: Time-based reputation decay");
    let now = SystemTime::now();

    // Simulate time passing by manually aging events
    reputation_system.simulate_time_passage(Duration::from_secs(3600)); // 1 hour
    let after_1_hour = reputation_system.get_reputation_score(player_id);
    println!("  Score after 1 hour: {:.3}", after_1_hour);

    reputation_system.simulate_time_passage(Duration::from_secs(86400)); // 1 day total
    let after_1_day = reputation_system.get_reputation_score(player_id);
    println!("  Score after 1 day: {:.3}", after_1_day);

    reputation_system.simulate_time_passage(Duration::from_secs(604800)); // 1 week total
    let after_1_week = reputation_system.get_reputation_score(player_id);
    println!("  Score after 1 week: {:.3}", after_1_week);

    // Old violations should matter less over time
    assert!(after_1_week > after_1_day);
    assert!(after_1_day > after_1_hour);
    println!("  ✓ Reputation correctly recovers over time");

    println!("\n✓ Reputation decay system exercise complete!\n");
}

struct ReputationDecaySystem {
    player_events: std::collections::HashMap<PeerId, Vec<ReputationEvent>>,
    base_reputation: f64,
    decay_half_life: Duration, // Time for reputation impact to halve
}

#[derive(Debug, Clone)]
struct ReputationEvent {
    event_type: String,
    impact: f64, // Negative for violations, positive for good behavior
    timestamp: SystemTime,
}

impl ReputationDecaySystem {
    fn new() -> Self {
        Self {
            player_events: std::collections::HashMap::new(),
            base_reputation: 0.5, // Neutral starting point
            decay_half_life: Duration::from_secs(86400 * 7), // 1 week
        }
    }

    fn add_violation(&mut self, player: PeerId, event_type: &str, severity: f64) {
        let event = ReputationEvent {
            event_type: event_type.to_string(),
            impact: -severity, // Negative impact
            timestamp: SystemTime::now(),
        };

        self.player_events
            .entry(player)
            .or_insert_with(Vec::new)
            .push(event);
    }

    fn add_positive_behavior(&mut self, player: PeerId, event_type: &str, value: f64) {
        let event = ReputationEvent {
            event_type: event_type.to_string(),
            impact: value, // Positive impact
            timestamp: SystemTime::now(),
        };

        self.player_events
            .entry(player)
            .or_insert_with(Vec::new)
            .push(event);
    }

    fn get_reputation_score(&self, player: PeerId) -> f64 {
        let events = match self.player_events.get(&player) {
            Some(events) => events,
            None => return self.base_reputation,
        };

        let now = SystemTime::now();
        let mut total_impact = 0.0;

        for event in events {
            // Calculate how much time has passed
            let age = now
                .duration_since(event.timestamp)
                .unwrap_or(Duration::ZERO);

            // Apply exponential decay: impact * (1/2)^(age/half_life)
            let decay_factor = 0.5_f64.powf(age.as_secs_f64() / self.decay_half_life.as_secs_f64());
            let decayed_impact = event.impact * decay_factor;

            total_impact += decayed_impact;
        }

        // Clamp reputation score between 0 and 1
        (self.base_reputation + total_impact).max(0.0).min(1.0)
    }

    fn simulate_time_passage(&mut self, duration: Duration) {
        // This is a simulation helper - in a real system, time passes naturally
        // We simulate by adjusting all event timestamps backwards
        for events in self.player_events.values_mut() {
            for event in events {
                event.timestamp = event
                    .timestamp
                    .checked_sub(duration)
                    .unwrap_or(SystemTime::UNIX_EPOCH);
            }
        }
    }
}

/// Exercise 3: Consensus-Based Ban System
///
/// Create a voting system where multiple nodes must agree
/// before a player is banned for cheating.
#[allow(dead_code)]
async fn exercise_consensus_ban() -> Result<()> {
    println!("Exercise 3: Consensus-Based Ban System");
    println!("=====================================\n");

    let mut ban_system = ConsensusBanSystem::new();
    let suspect = PeerId::random();

    // Create validator network
    let validators = vec![
        PeerId::random(), // Validator 1
        PeerId::random(), // Validator 2
        PeerId::random(), // Validator 3
        PeerId::random(), // Validator 4
        PeerId::random(), // Validator 5
    ];

    for validator in &validators {
        ban_system.add_validator(*validator);
    }

    println!("Network setup:");
    println!("  Suspect: {:?}", hex::encode(&suspect[..8]));
    println!("  Validators: {}", validators.len());
    println!("  Required votes: {}\n", ban_system.required_votes());

    // Test 1: Submit evidence from multiple nodes
    println!("Test 1: Evidence collection");

    let evidence1 = CheatEvidence {
        evidence_id: generate_evidence_id(&suspect, CheatType::InvalidRoll),
        suspect,
        cheat_type: CheatType::InvalidRoll,
        evidence_data: vec![1, 2, 3],
        detected_at: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        witnesses: vec![validators[0]],
        severity: 0.8,
        related_operation: None,
    };

    ban_system.submit_evidence(validators[0], evidence1.clone())?;
    println!("  Evidence submitted by validator 1");
    println!(
        "  Evidence count: {}",
        ban_system.get_evidence_count(suspect)
    );

    // Test 2: Voting process
    println!("\nTest 2: Voting on ban proposal");

    let ban_proposal = ban_system.create_ban_proposal(suspect)?;
    println!("  Ban proposal created: {:?}", ban_proposal.id);

    // Vote from validators (4 out of 5 vote yes = 80% > 66.7% threshold)
    ban_system.submit_vote(
        ban_proposal.id,
        validators[0],
        BanVote::Ban,
        "Strong evidence of invalid rolls",
    )?;
    ban_system.submit_vote(
        ban_proposal.id,
        validators[1],
        BanVote::Ban,
        "Statistical anomaly confirmed",
    )?;
    ban_system.submit_vote(
        ban_proposal.id,
        validators[2],
        BanVote::Ban,
        "Pattern consistent with cheating",
    )?;
    ban_system.submit_vote(
        ban_proposal.id,
        validators[3],
        BanVote::Ban,
        "Evidence is convincing",
    )?;
    ban_system.submit_vote(
        ban_proposal.id,
        validators[4],
        BanVote::NoVote,
        "Abstaining",
    )?;

    let result = ban_system.tally_votes(ban_proposal.id)?;
    println!("  Vote tally:");
    println!("    Ban votes: {}", result.ban_votes);
    println!("    No-ban votes: {}", result.no_ban_votes);
    println!("    Abstentions: {}", result.abstentions);
    println!("    Percentage for ban: {:.1}%", result.ban_percentage());

    let is_banned = result.ban_percentage() >= ban_system.ban_threshold();
    println!(
        "  Result: {}",
        if is_banned { "BANNED" } else { "NOT BANNED" }
    );

    if is_banned {
        ban_system.execute_ban(suspect, ban_proposal.id)?;
        println!("  ✓ Ban executed successfully");
    }

    println!("\n✓ Consensus ban system exercise complete!\n");
    Ok(())
}

#[derive(Debug, Clone, Copy)]
enum BanVote {
    Ban,
    NoBan,
    NoVote, // Abstention
}

#[derive(Debug, Clone)]
struct BanProposal {
    id: uuid::Uuid,
    suspect: PeerId,
    evidence_ids: Vec<[u8; 32]>,
    created_at: SystemTime,
}

#[derive(Debug)]
struct VoteResult {
    ban_votes: u32,
    no_ban_votes: u32,
    abstentions: u32,
    total_validators: u32,
}

impl VoteResult {
    fn ban_percentage(&self) -> f64 {
        if self.total_validators == 0 {
            return 0.0;
        }
        (self.ban_votes as f64 / self.total_validators as f64) * 100.0
    }
}

struct ConsensusBanSystem {
    validators: std::collections::HashSet<PeerId>,
    evidence: std::collections::HashMap<PeerId, Vec<CheatEvidence>>,
    ban_proposals: std::collections::HashMap<uuid::Uuid, BanProposal>,
    votes:
        std::collections::HashMap<uuid::Uuid, std::collections::HashMap<PeerId, (BanVote, String)>>,
    banned_players: std::collections::HashSet<PeerId>,
    ban_threshold_percent: f64,
}

impl ConsensusBanSystem {
    fn new() -> Self {
        Self {
            validators: std::collections::HashSet::new(),
            evidence: std::collections::HashMap::new(),
            ban_proposals: std::collections::HashMap::new(),
            votes: std::collections::HashMap::new(),
            banned_players: std::collections::HashSet::new(),
            ban_threshold_percent: 66.7, // Require 2/3 majority
        }
    }

    fn add_validator(&mut self, validator: PeerId) {
        self.validators.insert(validator);
    }

    fn required_votes(&self) -> u32 {
        ((self.validators.len() as f64 * self.ban_threshold_percent / 100.0).ceil() as u32).max(1)
    }

    fn ban_threshold(&self) -> f64 {
        self.ban_threshold_percent
    }

    fn submit_evidence(&mut self, validator: PeerId, evidence: CheatEvidence) -> Result<()> {
        if !self.validators.contains(&validator) {
            return Err(bitcraps::error::Error::Validation(
                "Only validators can submit evidence".to_string(),
            ));
        }

        self.evidence
            .entry(evidence.suspect)
            .or_insert_with(Vec::new)
            .push(evidence);
        Ok(())
    }

    fn get_evidence_count(&self, suspect: PeerId) -> usize {
        self.evidence.get(&suspect).map(|e| e.len()).unwrap_or(0)
    }

    fn create_ban_proposal(&mut self, suspect: PeerId) -> Result<BanProposal> {
        let evidence_for_suspect = self.evidence.get(&suspect);
        let evidence_ids = evidence_for_suspect
            .map(|evidence| evidence.iter().map(|e| e.evidence_id).collect())
            .unwrap_or_else(Vec::new);

        let proposal = BanProposal {
            id: uuid::Uuid::new_v4(),
            suspect,
            evidence_ids,
            created_at: SystemTime::now(),
        };

        self.ban_proposals.insert(proposal.id, proposal.clone());
        self.votes
            .insert(proposal.id, std::collections::HashMap::new());

        Ok(proposal)
    }

    fn submit_vote(
        &mut self,
        proposal_id: uuid::Uuid,
        validator: PeerId,
        vote: BanVote,
        reason: &str,
    ) -> Result<()> {
        if !self.validators.contains(&validator) {
            return Err(bitcraps::error::Error::Validation(
                "Only validators can vote".to_string(),
            ));
        }

        let votes_for_proposal = self
            .votes
            .get_mut(&proposal_id)
            .ok_or_else(|| bitcraps::error::Error::Validation("Proposal not found".to_string()))?;

        if votes_for_proposal.contains_key(&validator) {
            return Err(bitcraps::error::Error::Validation(
                "Validator has already voted".to_string(),
            ));
        }

        votes_for_proposal.insert(validator, (vote, reason.to_string()));
        Ok(())
    }

    fn tally_votes(&self, proposal_id: uuid::Uuid) -> Result<VoteResult> {
        let votes = self
            .votes
            .get(&proposal_id)
            .ok_or_else(|| bitcraps::error::Error::Validation("Proposal not found".to_string()))?;

        let mut ban_votes = 0;
        let mut no_ban_votes = 0;
        let mut abstentions = 0;

        for (_, (vote, _)) in votes {
            match vote {
                BanVote::Ban => ban_votes += 1,
                BanVote::NoBan => no_ban_votes += 1,
                BanVote::NoVote => abstentions += 1,
            }
        }

        // Count non-voters as abstentions
        abstentions += (self.validators.len() as u32) - votes.len() as u32;

        Ok(VoteResult {
            ban_votes,
            no_ban_votes,
            abstentions,
            total_validators: self.validators.len() as u32,
        })
    }

    fn execute_ban(&mut self, suspect: PeerId, _proposal_id: uuid::Uuid) -> Result<()> {
        self.banned_players.insert(suspect);
        Ok(())
    }
}
