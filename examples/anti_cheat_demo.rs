//! Anti-cheat system demonstration
//!
//! Run with: cargo run --example anti_cheat_demo

use bitcraps::error::Result;
use bitcraps::protocol::anti_cheat::{
    AntiCheatConfig, AntiCheatEngine, CheatEvidence, CheatType, PeerBehaviorProfile,
    RandomnessValidator,
};
use bitcraps::protocol::{DiceRoll, GameId, PeerId};
use std::time::Duration;

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
    };

    let mut engine = AntiCheatEngine::new(config);

    // Demonstration 1: Statistical Anomaly Detection
    println!("Demo 1: Statistical Anomaly Detection");
    println!("--------------------------------------");

    let mut validator = RandomnessValidator::new();

    // Generate normal dice rolls
    println!("Testing normal dice rolls (should pass):");
    for _ in 0..100 {
        let die1 = (rand::random::<u8>() % 6) + 1;
        let die2 = (rand::random::<u8>() % 6) + 1;
        validator.record_roll(DiceRoll { die1, die2 });
    }

    match validator.validate_randomness() {
        Ok(true) => println!("  ✓ Normal dice validated successfully"),
        Ok(false) => println!("  ✗ Normal dice failed validation (unexpected)"),
        Err(e) => println!("  Error: {}", e),
    }

    // Generate biased dice rolls
    println!("\nTesting biased dice (always 6, should fail):");
    let mut biased_validator = RandomnessValidator::new();
    for _ in 0..100 {
        biased_validator.record_roll(DiceRoll { die1: 6, die2: 6 });
    }

    match biased_validator.validate_randomness() {
        Ok(true) => println!("  ✗ Biased dice passed validation (unexpected)"),
        Ok(false) => println!("  ✓ Biased dice detected successfully"),
        Err(e) => println!("  Error: {}", e),
    }
    println!();

    // Demonstration 2: Behavior Profiling
    println!("Demo 2: Behavior Profiling");
    println!("--------------------------");

    let cheater_id = PeerId::random();
    let mut cheater_profile = PeerBehaviorProfile::new(cheater_id);

    // Simulate suspicious behavior
    println!("Simulating suspicious behavior:");
    cheater_profile.add_suspicious_activity(CheatType::TimeManipulation);
    cheater_profile.add_suspicious_activity(CheatType::StatisticalAnomaly);
    cheater_profile.add_suspicious_activity(CheatType::OverBetting);

    let trust_score = cheater_profile.calculate_trust_score();
    println!(
        "  Suspicious activities: {}",
        cheater_profile.suspicious_activities.len()
    );
    println!("  Trust score: {:.2}", trust_score);
    println!("  Is suspicious: {}", cheater_profile.is_suspicious());
    println!();

    // Demonstration 3: Evidence Collection
    println!("Demo 3: Evidence Collection");
    println!("---------------------------");

    let evidence = CheatEvidence::new(
        cheater_id,
        CheatType::DoubleSpending,
        vec![1, 2, 3, 4], // Mock evidence data
    );

    println!("Created evidence:");
    println!("  Evidence ID: {:?}", evidence.evidence_id);
    println!("  Suspect: {:?}", evidence.suspect);
    println!("  Cheat Type: {:?}", evidence.cheat_type);
    println!("  Severity: {:.2}", evidence.severity);
    println!("  Timestamp: {}", evidence.detected_at);
    println!();

    // Demonstration 4: Martingale Detection
    println!("Demo 4: Martingale Betting Pattern Detection");
    println!("--------------------------------------------");

    use bitcraps::protocol::craps::{Bet, BetType, CrapTokens};

    // Create Martingale pattern (doubling after losses)
    let mut bets = Vec::new();
    let mut amount = 100;

    for i in 0..5 {
        let bet = Bet {
            player: PeerId::random(),
            bet_type: BetType::Pass,
            amount: CrapTokens::new(amount).unwrap(),
            outcome: if i > 0 { Some(BetOutcome::Lost) } else { None },
        };
        bets.push(bet);
        amount *= 2; // Double bet after loss
    }

    let is_martingale = detect_martingale(&bets);
    println!("Bet sequence:");
    for (i, bet) in bets.iter().enumerate() {
        println!(
            "  Bet {}: {} tokens (outcome: {:?})",
            i + 1,
            bet.amount.0,
            bet.outcome
        );
    }
    println!("  Martingale pattern detected: {}", is_martingale);
    println!();

    // Demonstration 5: Collusion Detection
    println!("Demo 5: Collusion Detection");
    println!("---------------------------");

    // Create suspicious game sessions
    let player_a = PeerId::random();
    let player_b = PeerId::random();

    println!("Simulating games between two players:");
    println!("  Player A: {:?}", player_a);
    println!("  Player B: {:?}", player_b);

    // Simulate Player A always winning against Player B
    let mut wins_a = 0;
    let mut wins_b = 0;

    for i in 0..10 {
        if i < 9 {
            wins_a += 1; // A wins 9 times
        } else {
            wins_b += 1; // B wins 1 time
        }
    }

    let win_ratio = wins_a as f64 / (wins_a + wins_b) as f64;
    let is_collusion = win_ratio > 0.8;

    println!("  Games played: {}", wins_a + wins_b);
    println!("  Player A wins: {}", wins_a);
    println!("  Player B wins: {}", wins_b);
    println!("  Win ratio: {:.2}", win_ratio);
    println!("  Collusion suspected: {}", is_collusion);

    println!("\n✓ Anti-cheat demonstration complete!");

    Ok(())
}

// Helper function for Martingale detection
fn detect_martingale(bets: &[Bet]) -> bool {
    if bets.len() < 3 {
        return false;
    }

    let mut martingale_count = 0;

    for window in bets.windows(2) {
        let prev = &window[0];
        let curr = &window[1];

        if let Some(BetOutcome::Lost) = prev.outcome {
            let ratio = curr.amount.0 as f64 / prev.amount.0 as f64;
            if (ratio - 2.0).abs() < 0.1 {
                martingale_count += 1;
            }
        }
    }

    martingale_count >= 2
}

// Mock types for demonstration
#[derive(Debug)]
enum BetOutcome {
    Lost,
    Won,
}

use bitcraps::protocol::craps::Bet;

/// Exercise 1: Implement Time-Based Attack Detection
///
/// Create a function that detects time manipulation attacks
/// by checking for impossible timestamp sequences.
#[allow(dead_code)]
fn exercise_time_attack_detection() {
    // TODO: Implement time attack detection
    // Hints:
    // 1. Create sequence of operations with timestamps
    // 2. Check for future timestamps
    // 3. Check for impossible ordering
    // 4. Detect replay attacks (old timestamps)
}

/// Exercise 2: Build Reputation Decay System
///
/// Implement a reputation system that decays over time,
/// rewarding recent good behavior and forgetting old sins.
#[allow(dead_code)]
fn exercise_reputation_decay() {
    // TODO: Implement reputation decay
    // Hints:
    // 1. Create reputation scores with timestamps
    // 2. Apply exponential decay based on age
    // 3. Test that old violations matter less
    // 4. Verify recent behavior has more weight
}

/// Exercise 3: Consensus-Based Ban System
///
/// Create a voting system where multiple nodes must agree
/// before a player is banned for cheating.
#[allow(dead_code)]
async fn exercise_consensus_ban() {
    // TODO: Implement consensus banning
    // Hints:
    // 1. Collect evidence from multiple nodes
    // 2. Implement voting mechanism
    // 3. Require 2/3 majority for ban
    // 4. Handle vote manipulation attempts
}
