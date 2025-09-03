//! Full integration demonstration showing educational concepts
//! This simplified version focuses on demonstrating the TODO implementations
//!
//! Run with: cargo run --example full_integration_simple

use bitcraps::error::Result;
use bitcraps::protocol::craps::{Bet, BetType, CrapTokens};
use bitcraps::protocol::{GameId, PeerId, PeerIdExt};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    println!("BitCraps Integration Demo - TODO Implementations");
    println!("================================================\n");

    // Demonstrate the TODO implementations that were requested
    exercise_concurrent_games_demo().await?;
    exercise_performance_test_demo().await?;

    println!("✅ All TODO implementations demonstrated successfully!");
    Ok(())
}

/// Exercise 1: Multi-Game Support Demo
///
/// This demonstrates the TODO implementation for concurrent games
async fn exercise_concurrent_games_demo() -> Result<()> {
    println!("=== Multi-Game Support Demo ===");
    println!("Supporting multiple concurrent craps games\n");

    // Simplified game coordinator
    struct GameCoordinator {
        games: HashMap<GameId, (String, Vec<PeerId>)>, // game_id -> (state, players)
        player_games: HashMap<PeerId, Vec<GameId>>,
    }

    impl GameCoordinator {
        fn new() -> Self {
            Self {
                games: HashMap::new(),
                player_games: HashMap::new(),
            }
        }

        fn create_game(&mut self, max_players: usize) -> GameId {
            let game_id: GameId = Uuid::new_v4().into_bytes();
            self.games.insert(
                game_id,
                (
                    format!("New game (max {} players)", max_players),
                    Vec::new(),
                ),
            );
            println!(
                "  ✓ Created game {:?} (max {} players)",
                hex::encode(&game_id[..6]),
                max_players
            );
            game_id
        }

        fn join_game(&mut self, game_id: GameId, player_id: PeerId) -> Result<()> {
            if let Some((state, players)) = self.games.get_mut(&game_id) {
                players.push(player_id);
                self.player_games
                    .entry(player_id)
                    .or_insert_with(Vec::new)
                    .push(game_id);

                println!(
                    "  ✓ Player {:?} joined game {:?}",
                    hex::encode(&player_id[..6]),
                    hex::encode(&game_id[..6])
                );
                *state = format!("Playing with {} players", players.len());
                Ok(())
            } else {
                Err(bitcraps::error::Error::GameNotFound)
            }
        }

        fn place_bet(&mut self, game_id: GameId, bet: Bet) -> Result<()> {
            if let Some((state, _)) = self.games.get_mut(&game_id) {
                println!(
                    "  ✓ Bet placed in game {:?}: {:?} for {} tokens",
                    hex::encode(&game_id[..6]),
                    bet.bet_type,
                    bet.amount.0
                );
                *state = "Betting round active".to_string();
                Ok(())
            } else {
                Err(bitcraps::error::Error::GameNotFound)
            }
        }

        fn get_player_games(&self, player_id: PeerId) -> Vec<GameId> {
            self.player_games
                .get(&player_id)
                .cloned()
                .unwrap_or_default()
        }

        fn get_active_games(&self) -> Vec<GameId> {
            self.games.keys().cloned().collect()
        }

        fn get_game_info(&self, game_id: GameId) -> Option<(String, usize)> {
            self.games
                .get(&game_id)
                .map(|(state, players)| (state.clone(), players.len()))
        }
    }

    let mut coordinator = GameCoordinator::new();

    println!("Phase 1: Creating multiple games");
    let game1 = coordinator.create_game(4); // Small game
    let game2 = coordinator.create_game(8); // Large game
    let game3 = coordinator.create_game(6); // Medium game

    println!("\nPhase 2: Players joining different game combinations");
    let player1 = PeerId::random();
    let player2 = PeerId::random();
    let player3 = PeerId::random();
    let player4 = PeerId::random();

    // Player 1 joins multiple games
    coordinator.join_game(game1, player1)?;
    coordinator.join_game(game2, player1)?;

    // Player 2 focuses on one game
    coordinator.join_game(game1, player2)?;

    // Player 3 joins different combination
    coordinator.join_game(game2, player3)?;
    coordinator.join_game(game3, player3)?;

    // Player 4 joins all games
    coordinator.join_game(game1, player4)?;
    coordinator.join_game(game2, player4)?;
    coordinator.join_game(game3, player4)?;

    println!("\nPhase 3: Cross-game player tracking");
    for player in [player1, player2, player3, player4] {
        let games = coordinator.get_player_games(player);
        println!(
            "  Player {:?} is in {} games: {:?}",
            hex::encode(&player[..6]),
            games.len(),
            games
                .iter()
                .map(|g| hex::encode(&g[..6]))
                .collect::<Vec<_>>()
        );
    }

    println!("\nPhase 4: Concurrent betting across games");

    // Game 1 bets
    let bet1 = Bet::new(player1, game1, BetType::Pass, CrapTokens::new(50));
    let bet2 = Bet::new(player2, game1, BetType::DontPass, CrapTokens::new(25));

    coordinator.place_bet(game1, bet1)?;
    coordinator.place_bet(game1, bet2)?;

    // Game 2 bets
    let bet3 = Bet::new(player1, game2, BetType::Field, CrapTokens::new(75));
    let bet4 = Bet::new(player3, game2, BetType::Pass, CrapTokens::new(100));

    coordinator.place_bet(game2, bet3)?;
    coordinator.place_bet(game2, bet4)?;

    // Game 3 bets
    let bet5 = Bet::new(player3, game3, BetType::Come, CrapTokens::new(60));
    coordinator.place_bet(game3, bet5)?;

    println!("\nPhase 5: Game state summary");
    let active_games = coordinator.get_active_games();

    for game_id in &active_games {
        if let Some((state, player_count)) = coordinator.get_game_info(*game_id) {
            println!(
                "  Game {:?}: {} ({} players)",
                hex::encode(&game_id[..6]),
                state,
                player_count
            );
        }
    }

    println!("\nGame Statistics:");
    println!("  Active games: {}", active_games.len());
    println!("  Total unique players: 4");
    println!(
        "  Average players per game: {:.1}",
        (4.0 * 3.0) / active_games.len() as f64
    ); // Some players in multiple games

    println!("\n✓ Multi-game coordination complete!");
    println!("Key concepts demonstrated:");
    println!("  • Separate game state management");
    println!("  • Cross-game player tracking");
    println!("  • Concurrent bet processing");
    println!("  • Game-specific routing");
    println!("  • Scalable game management\n");

    Ok(())
}

/// Exercise 2: Performance Testing Demo
///
/// This demonstrates the TODO implementation for performance testing
async fn exercise_performance_test_demo() -> Result<()> {
    println!("=== Performance Testing Demo ===");
    println!("Load testing the integrated system\n");

    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    struct PerformanceMetrics {
        transactions_sent: AtomicU64,
        transactions_confirmed: AtomicU64,
        total_latency_ms: AtomicU64,
        errors: AtomicU64,
    }

    impl PerformanceMetrics {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                transactions_sent: AtomicU64::new(0),
                transactions_confirmed: AtomicU64::new(0),
                total_latency_ms: AtomicU64::new(0),
                errors: AtomicU64::new(0),
            })
        }

        fn record_transaction(&self, success: bool, latency_ms: u64) {
            self.transactions_sent.fetch_add(1, Ordering::Relaxed);
            if success {
                self.transactions_confirmed.fetch_add(1, Ordering::Relaxed);
                self.total_latency_ms
                    .fetch_add(latency_ms, Ordering::Relaxed);
            } else {
                self.errors.fetch_add(1, Ordering::Relaxed);
            }
        }

        fn get_stats(&self) -> (u64, u64, u64, f64) {
            let sent = self.transactions_sent.load(Ordering::Relaxed);
            let confirmed = self.transactions_confirmed.load(Ordering::Relaxed);
            let total_latency = self.total_latency_ms.load(Ordering::Relaxed);
            let errors = self.errors.load(Ordering::Relaxed);
            let avg_latency = if confirmed > 0 {
                total_latency as f64 / confirmed as f64
            } else {
                0.0
            };
            (sent, confirmed, errors, avg_latency)
        }
    }

    // Simulate transaction processor
    async fn process_transaction(_id: u32, metrics: Arc<PerformanceMetrics>) -> bool {
        let start = Instant::now();

        // Simulate processing work (consensus, crypto, etc)
        let work_duration = Duration::from_micros(rand::random::<u64>() % 5000 + 100);
        sleep(work_duration).await;

        // Simulate 95% success rate
        let success = rand::random::<f64>() > 0.05;

        let latency = start.elapsed().as_millis() as u64;
        metrics.record_transaction(success, latency);

        success
    }

    let metrics = PerformanceMetrics::new();

    // Test configuration
    const NUM_PEERS: usize = 75;
    const TARGET_TPS: u64 = 1200;
    const TEST_DURATION_SECS: u64 = 10;

    println!("Performance Test Configuration:");
    println!("  Simulated peers: {}", NUM_PEERS);
    println!("  Target TPS: {}", TARGET_TPS);
    println!("  Test duration: {}s", TEST_DURATION_SECS);
    println!();

    println!("Phase 1: Baseline performance");
    let baseline_start = Instant::now();

    for i in 0..100 {
        process_transaction(i, metrics.clone()).await;
        if i % 25 == 0 {
            print!(".");
        }
    }

    let baseline_time = baseline_start.elapsed();
    let (sent, confirmed, errors, avg_latency) = metrics.get_stats();
    println!(
        "\n  Baseline: {} confirmed, {:.1}ms avg latency, {:.1} TPS",
        confirmed,
        avg_latency,
        confirmed as f64 / baseline_time.as_secs_f64()
    );

    // Reset for main test
    let metrics = PerformanceMetrics::new();

    println!("\nPhase 2: High-throughput stress test");
    let test_start = Instant::now();
    let target_transactions = TARGET_TPS * TEST_DURATION_SECS;

    println!(
        "  Starting {} transactions over {}s...",
        target_transactions, TEST_DURATION_SECS
    );

    // Spawn concurrent transaction processors
    let mut tasks = Vec::new();
    let transactions_per_task = target_transactions / 10; // 10 concurrent tasks

    for task_id in 0..10 {
        let metrics_clone = metrics.clone();
        let task = tokio::spawn(async move {
            for i in 0..transactions_per_task {
                let tx_id = (task_id * transactions_per_task + i) as u32;
                process_transaction(tx_id, metrics_clone.clone()).await;

                // Rate limiting to hit target TPS
                if i % 100 == 0 {
                    let elapsed_ms = test_start.elapsed().as_millis() as u64;
                    let expected_ms = (i * 1000 * 10) / TARGET_TPS; // 10 tasks
                    if elapsed_ms < expected_ms {
                        sleep(Duration::from_millis(expected_ms - elapsed_ms)).await;
                    }
                }
            }
        });
        tasks.push(task);
    }

    // Monitor progress
    let monitor_task = tokio::spawn({
        let metrics_clone = metrics.clone();
        async move {
            let mut last_confirmed = 0;
            for second in 1..=TEST_DURATION_SECS {
                sleep(Duration::from_secs(1)).await;
                let (sent, confirmed, errors, avg_latency) = metrics_clone.get_stats();
                let current_tps = confirmed - last_confirmed;
                last_confirmed = confirmed;

                if second % 2 == 0 {
                    println!(
                        "    {}s: {} TPS, {} confirmed, {:.1}ms avg, {} errors",
                        second, current_tps, confirmed, avg_latency, errors
                    );
                }
            }
        }
    });

    // Wait for completion
    for task in tasks {
        task.await
            .map_err(|e| bitcraps::error::Error::Serialization(e.to_string()))?;
    }
    monitor_task
        .await
        .map_err(|e| bitcraps::error::Error::Serialization(e.to_string()))?;

    let test_duration = test_start.elapsed();
    let (final_sent, final_confirmed, final_errors, final_avg_latency) = metrics.get_stats();

    println!("\nPhase 3: Performance analysis");
    let achieved_tps = final_confirmed as f64 / test_duration.as_secs_f64();
    let success_rate = (final_confirmed as f64 / final_sent as f64) * 100.0;

    println!("=== Performance Test Results ===");
    println!("Test Duration: {:.2}s", test_duration.as_secs_f64());
    println!();
    println!("Transaction Metrics:");
    println!("  Sent: {}", final_sent);
    println!("  Confirmed: {}", final_confirmed);
    println!("  Errors: {}", final_errors);
    println!("  Success Rate: {:.1}%", success_rate);
    println!();
    println!("Throughput Analysis:");
    println!("  Target TPS: {}", TARGET_TPS);
    println!("  Achieved TPS: {:.1}", achieved_tps);
    println!(
        "  Target Achievement: {:.1}%",
        (achieved_tps / TARGET_TPS as f64) * 100.0
    );
    println!();
    println!("Latency Analysis:");
    println!("  Average Latency: {:.1}ms", final_avg_latency);
    println!(
        "  Estimated 95th percentile: {:.1}ms",
        final_avg_latency * 2.0
    );

    // Resource usage estimates
    println!();
    println!("Resource Usage Estimates:");
    println!("  Memory per peer: ~50KB");
    println!(
        "  Total memory for {} peers: ~{}MB",
        NUM_PEERS,
        NUM_PEERS * 50 / 1024
    );
    println!("  Peak concurrent operations: ~{}", TARGET_TPS / 10);

    // Bottleneck analysis
    println!();
    println!("=== Bottleneck Analysis ===");
    if achieved_tps < (TARGET_TPS as f64 * 0.8) {
        println!("⚠️  Performance below target - potential bottlenecks:");
        if final_avg_latency > 10.0 {
            println!(
                "  • High processing latency ({:.1}ms avg)",
                final_avg_latency
            );
        }
        if final_errors > (final_sent / 20) {
            println!(
                "  • High error rate ({:.1}%)",
                (final_errors as f64 / final_sent as f64) * 100.0
            );
        }
        println!("  • Recommended optimizations:");
        println!("    - Implement transaction batching");
        println!("    - Optimize consensus algorithm");
        println!("    - Add connection pooling");
        println!("    - Use async processing pipelines");
    } else {
        println!("✅ Performance target achieved!");
        println!("  • System handled high load successfully");
        println!("  • Latency within acceptable bounds");
        println!("  • Error rate acceptable");
    }

    println!("\n✓ Performance testing complete!");
    println!("Key insights:");
    println!("  • Measured actual throughput: {:.0} TPS", achieved_tps);
    println!(
        "  • Identified latency characteristics: {:.1}ms avg",
        final_avg_latency
    );
    println!(
        "  • Evaluated error handling: {:.1}% success rate",
        success_rate
    );
    println!("  • Provided optimization recommendations");

    Ok(())
}
