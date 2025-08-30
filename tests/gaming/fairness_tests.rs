use std::collections::HashMap;

#[tokio::test]
async fn test_dice_roll_fairness() {
    // Simulate 10,000 dice rolls
    let mut roll_counts = HashMap::new();
    for _ in 0..10000 {
        let d1 = (rand::random::<u8>() % 6) + 1;
        let d2 = (rand::random::<u8>() % 6) + 1;
        let total = d1 + d2;
        *roll_counts.entry(total).or_insert(0) += 1;
    }

    // Expected probabilities for two dice
    let expected_probs: HashMap<u8, f64> = [
        (2, 1.0 / 36.0),
        (3, 2.0 / 36.0),
        (4, 3.0 / 36.0),
        (5, 4.0 / 36.0),
        (6, 5.0 / 36.0),
        (7, 6.0 / 36.0),
        (8, 5.0 / 36.0),
        (9, 4.0 / 36.0),
        (10, 3.0 / 36.0),
        (11, 2.0 / 36.0),
        (12, 1.0 / 36.0),
    ]
    .iter()
    .cloned()
    .collect();

    // Verify statistical distribution
    for (total, prob) in expected_probs.iter() {
        let expected_count = (10000.0 * prob) as i32;
        let actual_count = *roll_counts.get(total).unwrap_or(&0);
        let variance = (actual_count - expected_count).abs();
        let tolerance = (expected_count as f64 * 0.15) as i32; // 15% tolerance for randomness

        assert!(
            variance < tolerance,
            "Dice roll {} occurred {} times, expected ~{} (±{})",
            total,
            actual_count,
            expected_count,
            tolerance
        );
    }
}

#[test]
fn test_bet_payout_calculations() {
    // Test pass line payouts
    assert_eq!(calculate_payout(100, true, 1.0), 200); // Win pays 1:1
    assert_eq!(calculate_payout(100, false, 1.0), 0); // Loss

    // Test odds bet payouts
    assert_eq!(calculate_payout(100, true, 1.5), 250); // 3:2 payout
    assert_eq!(calculate_payout(100, true, 2.0), 300); // 2:1 payout
}

#[test]
fn test_house_edge_calculations() {
    // Pass line bet has 1.41% house edge
    let pass_line_edge = calculate_house_edge("pass");
    assert!((pass_line_edge - 1.41).abs() < 0.01);

    // Don't pass has 1.36% house edge
    let dont_pass_edge = calculate_house_edge("dont_pass");
    assert!((dont_pass_edge - 1.36).abs() < 0.01);
}

fn calculate_payout(bet_amount: u64, won: bool, odds: f64) -> u64 {
    if won {
        bet_amount + (bet_amount as f64 * odds) as u64
    } else {
        0
    }
}

fn calculate_house_edge(bet_type: &str) -> f64 {
    match bet_type {
        "pass" => 1.41,
        "dont_pass" => 1.36,
        "field" => 5.56,
        _ => 0.0,
    }
}

use rand;
