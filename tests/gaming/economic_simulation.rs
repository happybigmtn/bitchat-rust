use std::collections::HashMap;

/// Simulate casino house edge over many games
#[tokio::test]
async fn test_house_edge_simulation() {
    const SIMULATION_RUNS: u32 = 10_000;
    const STARTING_BALANCE: u64 = 10_000;
    
    let mut total_player_losses = 0i64;
    let mut total_bets_placed = 0u64;
    
    for _run in 0..SIMULATION_RUNS {
        let mut balance = STARTING_BALANCE;
        
        // Simulate a series of bets
        for _bet_num in 0..100 {
            let bet_amount = 100; // Fixed bet size
            
            if balance < bet_amount {
                break;
            }
            
            balance -= bet_amount;
            total_bets_placed += bet_amount;
            
            // Simulate pass line bet outcome (win rate ~49.3%)
            let won = rand::random::<f64>() < 0.493;
            
            if won {
                balance += bet_amount * 2; // 1:1 payout
            }
        }
        
        let final_balance = balance;
        total_player_losses += STARTING_BALANCE as i64 - final_balance as i64;
    }
    
    let house_edge = (total_player_losses as f64 / total_bets_placed as f64) * 100.0;
    
    // Pass line bet in craps has a house edge of approximately 1.41%
    // Allow for some variance in simulation
    assert!(house_edge > 0.5 && house_edge < 2.5, 
           "House edge of {:.2}% is outside expected range", house_edge);
    
    println!("Simulated house edge: {:.2}%", house_edge);
    println!("Total player losses: {}", total_player_losses);
    println!("Total bets placed: {}", total_bets_placed);
}

/// Test different betting strategies
#[tokio::test]
async fn test_betting_strategies() {
    const PLAYERS: u32 = 100;
    const GAMES_PER_PLAYER: u32 = 50;
    
    let mut strategy_results: HashMap<&str, i64> = HashMap::new();
    
    // Conservative strategy - bet 5% of bankroll
    let mut total_profit = 0i64;
    for _player in 0..PLAYERS {
        let mut balance = 1000u64;
        
        for _game in 0..GAMES_PER_PLAYER {
            let bet_amount = balance / 20; // 5% of bankroll
            if bet_amount == 0 || balance < bet_amount {
                break;
            }
            
            balance -= bet_amount;
            
            // Simulate outcome
            if rand::random::<f64>() < 0.493 {
                balance += bet_amount * 2;
            }
        }
        
        total_profit += balance as i64 - 1000;
    }
    strategy_results.insert("conservative", total_profit);
    
    // Aggressive strategy - bet 20% of bankroll
    total_profit = 0;
    for _player in 0..PLAYERS {
        let mut balance = 1000u64;
        
        for _game in 0..GAMES_PER_PLAYER {
            let bet_amount = balance / 5; // 20% of bankroll
            if bet_amount == 0 || balance < bet_amount {
                break;
            }
            
            balance -= bet_amount;
            
            // Simulate outcome
            if rand::random::<f64>() < 0.493 {
                balance += bet_amount * 2;
            }
        }
        
        total_profit += balance as i64 - 1000;
    }
    strategy_results.insert("aggressive", total_profit);
    
    // All strategies should lose money on average due to house edge
    for (strategy, profit) in &strategy_results {
        println!("Strategy '{}' total profit: {}", strategy, profit);
        assert!(*profit < 0, "Strategy {} should not be profitable long-term", strategy);
    }
}

/// Test token economics with minting and burning
#[test]
fn test_token_economics() {
    let mut total_supply = 1_000_000u64;
    let burn_rate = 0.001; // 0.1% burn per transaction
    let mint_rate = 0.0005; // 0.05% mint for rewards
    
    // Simulate 10000 transactions
    for _ in 0..10000 {
        let transaction_amount = 100u64;
        
        // Burn tokens
        let burned = (transaction_amount as f64 * burn_rate) as u64;
        total_supply = total_supply.saturating_sub(burned);
        
        // Mint rewards
        let minted = (transaction_amount as f64 * mint_rate) as u64;
        total_supply = total_supply.saturating_add(minted);
    }
    
    // Supply should decrease due to higher burn rate
    assert!(total_supply < 1_000_000, "Total supply should decrease with higher burn rate");
    println!("Final token supply: {}", total_supply);
}

use rand;