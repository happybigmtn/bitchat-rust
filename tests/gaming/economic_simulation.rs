use bitchat::gaming::{GameSessionManager, WalletInterface, BetResult};
use std::collections::HashMap;

/// Simulate casino house edge over many games
#[tokio::test]
async fn test_house_edge_simulation() {
    const SIMULATION_RUNS: u32 = 100_000;
    const STARTING_BALANCE: u64 = 10_000;
    
    let mut total_player_losses = 0i64;
    let mut total_bets_placed = 0u64;
    
    for run in 0..SIMULATION_RUNS {
        let mut wallet = WalletInterface::new(STARTING_BALANCE);
        let session_manager = GameSessionManager::new(Default::default());
        
        // Simulate a series of bets
        for bet_num in 0..100 {
            let bet_amount = 100; // Fixed bet size
            
            if wallet.get_available_balance() < bet_amount {
                break; // Player broke
            }
            
            let bet_id = format!("sim_{}_{}", run, bet_num);
            wallet.place_bet(bet_id.clone(), bet_amount).unwrap();
            total_bets_placed += bet_amount;
            
            // Simulate various bet outcomes based on actual craps odds
            let outcome = simulate_craps_outcome(BetType::Pass);
            let payout = calculate_payout(BetType::Pass, bet_amount, outcome);
            
            wallet.resolve_bet(&bet_id, outcome, payout).unwrap();
        }
        
        let final_balance = wallet.get_available_balance() + wallet.get_total_balance();
        total_player_losses += STARTING_BALANCE as i64 - final_balance as i64;
    }
    
    let house_edge = (total_player_losses as f64 / total_bets_placed as f64) * 100.0;
    
    // Pass line bet in craps has a house edge of approximately 1.41%
    assert!(house_edge > 1.0 && house_edge < 2.0, 
           "House edge of {:.2}% is outside expected range", house_edge);
    
    println!("Simulated house edge: {:.2}%", house_edge);
    println!("Total player losses: {}", total_player_losses);
    println!("Total bets placed: {}", total_bets_placed);
}

/// Test game economics under various player strategies
#[tokio::test]
async fn test_player_strategy_analysis() {
    const PLAYERS: u32 = 1000;
    const GAMES_PER_PLAYER: u32 = 50;
    
    let strategies = vec![
        ("conservative", conservative_betting_strategy),
        ("aggressive", aggressive_betting_strategy),
        ("martingale", martingale_betting_strategy),
    ];
    
    for (strategy_name, strategy_fn) in strategies {
        let mut total_profit = 0i64;
        let mut successful_players = 0u32;
        
        for player_id in 0..PLAYERS {
            let mut wallet = WalletInterface::new(1000);
            let session_manager = GameSessionManager::new(Default::default());
            
            let starting_balance = wallet.get_available_balance();
            
            for game_num in 0..GAMES_PER_PLAYER {
                if wallet.get_available_balance() < 10 {
                    break; // Player broke
                }
                
                let bet_amount = strategy_fn(
                    wallet.get_available_balance(),
                    game_num,
                    // Would include game history for more sophisticated strategies
                );
                
                if bet_amount > wallet.get_available_balance() {
                    break;
                }
                
                let bet_id = format!("strat_{}_{}", player_id, game_num);
                wallet.place_bet(bet_id.clone(), bet_amount).unwrap();
                
                let outcome = simulate_craps_outcome(BetType::Pass);
                let payout = calculate_payout(BetType::Pass, bet_amount, outcome);
                
                wallet.resolve_bet(&bet_id, outcome, payout).unwrap();
            }
            
            let final_balance = wallet.get_available_balance() + wallet.get_total_balance();
            let profit = final_balance as i64 - starting_balance as i64;
            total_profit += profit;
            
            if profit > 0 {
                successful_players += 1;
            }
        }
        
        let avg_profit = total_profit as f64 / PLAYERS as f64;
        let success_rate = successful_players as f64 / PLAYERS as f64 * 100.0;
        
        println!("Strategy: {} | Avg Profit: {:.2} | Success Rate: {:.1}%",
                strategy_name, avg_profit, success_rate);
        
        // All strategies should show negative expected value due to house edge
        assert!(avg_profit < 0.0, "Strategy {} shows positive expected value", strategy_name);
    }
}

fn conservative_betting_strategy(balance: u64, _game_num: u32) -> u64 {
    (balance / 100).max(10) // Bet 1% of balance, minimum 10
}

fn aggressive_betting_strategy(balance: u64, _game_num: u32) -> u64 {
    (balance / 10).max(50) // Bet 10% of balance, minimum 50
}

fn martingale_betting_strategy(balance: u64, game_num: u32) -> u64 {
    // Double bet after each loss (simplified)
    let base_bet = 10;
    let multiplier = 2_u64.pow((game_num % 6).min(5)); // Cap at 5 doublings
    (base_bet * multiplier).min(balance / 4) // Don't bet more than 25% of balance
}

fn simulate_craps_outcome(bet_type: BetType) -> BetResult {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    
    match bet_type {
        BetType::Pass => {
            // Simplified pass line simulation
            let roll = rng.gen_range(1..=36); // Simulate probability space
            if roll <= 8 { // ~22% win on come-out
                BetResult::Won
            } else if roll <= 12 { // ~11% lose on come-out
                BetResult::Lost
            } else { 
                // Point phase - simplified to overall pass line probability
                if rng.gen_range(1..=495) <= 244 { // 49.3% win rate
                    BetResult::Won
                } else {
                    BetResult::Lost
                }
            }
        },
        _ => {
            // Simplified for other bet types
            if rng.gen_range(1..=100) <= 49 {
                BetResult::Won
            } else {
                BetResult::Lost
            }
        }
    }
}

fn calculate_payout(bet_type: BetType, bet_amount: u64, result: BetResult) -> u64 {
    match result {
        BetResult::Won => {
            match bet_type {
                BetType::Pass | BetType::DontPass => bet_amount * 2, // 1:1 payout
                BetType::Field => bet_amount * 2, // Simplified
                BetType::Any7 => bet_amount * 5, // 4:1 payout
                BetType::Any11 => bet_amount * 16, // 15:1 payout
                BetType::AnyCraps => bet_amount * 8, // 7:1 payout
                _ => bet_amount * 2, // Default 1:1
            }
        },
        BetResult::Push => bet_amount, // Return original bet
        _ => 0, // Lost
    }
}