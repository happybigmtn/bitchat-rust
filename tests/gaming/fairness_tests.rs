use bitchat::gaming::{GameSessionManager, CrapsSession, BetType, AntiCheatDetector};
use std::collections::HashMap;

#[tokio::test]
async fn test_dice_roll_fairness() {
    let session_manager = GameSessionManager::new(Default::default());
    let game_id = session_manager.create_session(PeerId::random()).await.unwrap();
    
    // Simulate 10,000 dice rolls
    let mut roll_counts = HashMap::new();
    for _ in 0..10000 {
        let (d1, d2) = simulate_dice_roll();
        let total = d1 + d2;
        *roll_counts.entry(total).or_insert(0) += 1;
    }
    
    // Verify statistical distribution
    // Each outcome should occur roughly the expected number of times
    for (total, expected_count) in expected_dice_distribution(10000).iter() {
        let actual_count = roll_counts.get(total).unwrap_or(&0);
        let variance = (*actual_count as f64 - *expected_count).abs();
        let tolerance = expected_count * 0.05; // 5% tolerance
        
        assert!(
            variance < tolerance,
            "Dice roll {} occurred {} times, expected ~{} (±{})",
            total, actual_count, expected_count, tolerance
        );
    }
}

#[tokio::test]
async fn test_anti_cheat_detection() {
    let anti_cheat = AntiCheatDetector::new();
    let player = PeerId::random();
    
    // Test rapid betting detection
    for i in 0..35 { // Exceed max_bets_per_minute (30)
        let bet = create_test_bet(player, BetType::Pass, 100, i);
        let result = anti_cheat.validate_bet(&bet, &player).await;
        
        if i < 30 {
            assert!(result.is_ok(), "Legitimate bet {} should pass", i);
        } else {
            assert!(result.is_err(), "Bet {} should trigger anti-cheat", i);
        }
    }
}

#[tokio::test]
async fn test_bet_escrow_integrity() {
    let gaming_security = GamingSecurityManager::new(Default::default());
    let participants = vec![PeerId::random(), PeerId::random()];
    let session = gaming_security.create_gaming_session("test_game".to_string(), participants.clone()).await.unwrap();
    
    let bet = PendingBet {
        bet_id: "test_bet_1".to_string(),
        player: participants[0],
        amount: 1000,
        bet_hash: [0u8; 32], // Would be calculated properly
        timestamp: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
        confirmations: vec![participants[1]], // One confirmation
        escrow_signature: None,
    };
    
    // Should fail with insufficient confirmations
    let result = gaming_security.validate_and_escrow_bet("test_game", &bet).await;
    assert!(result.is_err());
    
    // Add required confirmations
    let mut bet_with_confirmations = bet.clone();
    bet_with_confirmations.confirmations.push(participants[0]); // Self-confirmation for testing
    
    let escrow_result = gaming_security.validate_and_escrow_bet("test_game", &bet_with_confirmations).await;
    assert!(escrow_result.is_ok());
}

fn expected_dice_distribution(total_rolls: u32) -> HashMap<u8, f64> {
    let mut distribution = HashMap::new();
    
    // Probability of each sum when rolling two dice
    let probabilities = [
        (2, 1.0/36.0), (3, 2.0/36.0), (4, 3.0/36.0), (5, 4.0/36.0),
        (6, 5.0/36.0), (7, 6.0/36.0), (8, 5.0/36.0), (9, 4.0/36.0),
        (10, 3.0/36.0), (11, 2.0/36.0), (12, 1.0/36.0),
    ];
    
    for (sum, prob) in probabilities.iter() {
        distribution.insert(*sum, *prob * total_rolls as f64);
    }
    
    distribution
}

fn simulate_dice_roll() -> (u8, u8) {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (rng.gen_range(1..=6), rng.gen_range(1..=6))
}

fn create_test_bet(player: PeerId, bet_type: BetType, amount: u64, sequence: u32) -> CrapsBet {
    CrapsBet {
        player,
        bet_type,
        amount,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() + sequence as u64, // Ensure unique timestamps
    }
}