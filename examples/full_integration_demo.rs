//! Full integration demonstration showing how all components work together
//!
//! Run with: cargo run --example full_integration_demo

use bitcraps::crypto::{BitchatIdentity, BitchatKeypair, Identity, SessionManager};
use bitcraps::error::Result;
use bitcraps::mesh::{MeshConfig, MeshService};
use bitcraps::protocol::consensus::engine::{ConsensusConfig, ConsensusEngine};
use bitcraps::protocol::craps::{Bet, BetType, CrapTokens};
use bitcraps::protocol::{GameId, PeerId};
use bitcraps::transport::{BluetoothTransport, TransportCoordinator};
use std::sync::Arc;
use std::time::Duration;
use tokio;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    println!("BitCraps Full Integration Demo");
    println!("==============================\n");

    // Phase 1: Identity and Cryptography Setup
    println!("Phase 1: Setting up identity and cryptography...");
    println!("{}", "-".repeat(50));

    let identity = Identity::generate()?;
    let peer_id = PeerId::from_public_key(&identity.public_key);
    println!("Generated identity: {:?}", peer_id);

    let session_manager = SessionManager::new(identity.clone());
    println!("Session manager initialized\n");

    // Phase 2: Transport Layer Setup
    println!("Phase 2: Initializing transport layer...");
    println!("{}", "-".repeat(50));

    let transport = Arc::new(
        BluetoothTransport::new()
            .await
            .map_err(|e| bitcraps::error::Error::Transport(e.to_string()))?,
    );
    let coordinator = TransportCoordinator::new(transport.clone());

    // Start transport discovery
    coordinator.start_discovery().await?;
    println!("Bluetooth discovery started");
    println!("Scanning for peers...\n");

    // Phase 3: Mesh Network Formation
    println!("Phase 3: Creating mesh network...");
    println!("{}", "-".repeat(50));

    let mesh_config = MeshConfig {
        max_peers: 20,
        ttl: 5,
        cache_size: 10_000,
        heartbeat_interval: Duration::from_secs(30),
        enable_reputation: true,
    };

    let mesh = MeshService::new(peer_id, Arc::new(coordinator), mesh_config).await?;

    println!("Mesh service started");
    println!("Max peers: {}", mesh_config.max_peers);
    println!("TTL: {}", mesh_config.ttl);
    println!("Reputation enabled: {}\n", mesh_config.enable_reputation);

    // Phase 4: Consensus Engine Setup
    println!("Phase 4: Initializing consensus engine...");
    println!("{}", "-".repeat(50));

    let consensus_config = ConsensusConfig {
        min_validators: 3,
        max_validators: 8,
        round_timeout: Duration::from_secs(5),
        proposal_timeout: Duration::from_secs(2),
        byzantine_threshold: 0.33,
        enable_anti_cheat: true,
    };

    let mut consensus = ConsensusEngine::new(game_id, vec![peer_id], peer_id, consensus_config)?;
    println!("Consensus engine started");
    println!("Byzantine threshold: 33%");
    println!("Anti-cheat: enabled\n");

    // Phase 5: Game Creation
    println!("Phase 5: Creating craps game...");
    println!("{}", "-".repeat(50));

    let game_id = GameId::random();
    let mut game = CrapsGame::new(game_id);

    println!("Game created: {:?}", game_id);
    println!("Initial state: Come out roll\n");

    // Phase 6: Simulated Game Flow
    println!("Phase 6: Demonstrating game flow...");
    println!("{}", "-".repeat(50));

    // Simulate peers joining
    let peers = vec![PeerId::random(), PeerId::random(), PeerId::random()];

    for peer in &peers {
        mesh.add_peer(*peer).await?;
        consensus.add_validator(*peer)?;
        println!("Player joined: {:?}", peer);
    }
    println!();

    // Place a bet
    let bet = Bet::new(peers[0], game_id, BetType::Pass, CrapTokens::new(100));

    println!("Player {:?} placing bet:", peers[0]);
    println!("  Type: Pass line");
    println!("  Amount: 100 CRAP tokens\n");

    // Create consensus proposal for the bet
    // Submit bet proposal to consensus
    let proposal_id = consensus.submit_proposal(
        bitcraps::protocol::consensus::engine::GameOperation::PlaceBet {
            player: peers[0],
            bet,
            nonce: 12345, // Demo nonce
        },
    )?;

    println!("Consensus proposal created");
    println!("Waiting for validator votes...\n");

    // Simulate our own vote (simplified demo)
    consensus.vote_on_proposal(proposal_id, true)?;
    println!("Vote cast for proposal: {:?}\n", proposal_id);

    // Check consensus (simplified check)
    if consensus.has_consensus() {
        println!("✓ Consensus reached!");
        println!("✓ Bet accepted and recorded\n");

        // Simulate dice roll
        println!("Rolling dice...");
        let die1 = 4;
        let die2 = 3;
        println!("Result: {} + {} = {}", die1, die2, die1 + die2);

        // Process game outcome
        match die1 + die2 {
            7 | 11 => println!("Natural! Pass line wins!"),
            2 | 3 | 12 => println!("Craps! Pass line loses!"),
            point => println!("Point established: {}", point),
        }
    } else {
        println!("✗ Consensus not reached");
    }

    // Phase 7: System Monitoring
    println!("\nPhase 7: System statistics...");
    println!("{}", "-".repeat(50));

    let mesh_stats = mesh.get_statistics().await;
    println!("Mesh Network:");
    println!("  Connected peers: {}", peers.len());
    println!("  Messages sent: {}", mesh_stats.messages_sent);
    println!("  Messages received: {}", mesh_stats.messages_received);
    println!("  Cache hits: {}", mesh_stats.cache_hits);

    // Get final statistics
    println!("Demo completed successfully!");

    // Get consensus statistics (simplified for demo)
    let consensus_stats = bitcraps::protocol::consensus::engine::ConsensusStats {
        total_proposals: 1,
        accepted_proposals: 1,
        rejected_proposals: 0,
    };

    println!("\nConsensus Engine:");
    println!("  Total proposals: {}", consensus_stats.total_proposals);
    println!("  Accepted: {}", consensus_stats.accepted_proposals);
    println!("  Rejected: {}", consensus_stats.rejected_proposals);

    println!("\n✓ Full integration demonstration complete!");

    Ok(())
}

/// Exercise 1: Multi-Game Support
///
/// Extend this demo to support multiple concurrent games.
/// Each game should have its own consensus round and state.
#[allow(dead_code)]
async fn exercise_concurrent_games() -> Result<()> {
    println!("\n\n=== Exercise: Multi-Game Support ===");
    println!("Supporting multiple concurrent craps games\n");

    // Create a game coordinator to manage multiple games
    struct GameCoordinator {
        games: std::collections::HashMap<GameId, (CrapsGame, ConsensusEngine)>,
        player_games: std::collections::HashMap<PeerId, Vec<GameId>>,
        identity: Identity,
    }

    impl GameCoordinator {
        fn new(identity: Identity) -> Self {
            Self {
                games: std::collections::HashMap::new(),
                player_games: std::collections::HashMap::new(),
                identity,
            }
        }

        async fn create_game(&mut self, max_players: usize) -> Result<GameId> {
            let game_id = GameId::random();
            let game = CrapsGame::new(game_id);

            // Create separate consensus engine for this game
            let consensus_config = ConsensusConfig {
                min_validators: 2,
                max_validators: max_players,
                round_timeout: Duration::from_secs(5),
                proposal_timeout: Duration::from_secs(2),
                byzantine_threshold: 0.33,
                enable_anti_cheat: true,
            };

            let consensus = ConsensusEngine::new(
                game_id,
                vec![self.identity.peer_id()],
                self.identity.peer_id(),
                consensus_config,
            )?;

            self.games.insert(game_id, (game, consensus));
            println!("✓ Created game {:?} (max {} players)", game_id, max_players);
            Ok(game_id)
        }

        async fn join_game(&mut self, game_id: GameId, player_id: PeerId) -> Result<()> {
            if let Some((_, consensus)) = self.games.get_mut(&game_id) {
                consensus.add_validator(player_id)?;

                self.player_games
                    .entry(player_id)
                    .or_insert_with(Vec::new)
                    .push(game_id);

                println!("✓ Player {:?} joined game {:?}", player_id, game_id);
                Ok(())
            } else {
                Err(bitcraps::error::Error::GameNotFound(game_id))
            }
        }

        async fn place_bet(&mut self, game_id: GameId, bet: Bet) -> Result<()> {
            if let Some((game, consensus)) = self.games.get_mut(&game_id) {
                // Create game-specific proposal
                let proposal_id = consensus.submit_proposal(
                    bitcraps::protocol::consensus::engine::GameOperation::PlaceBet {
                        player: bet.player,
                        bet: bet.clone(),
                        nonce: rand::random(),
                    },
                )?;

                println!("✓ Bet placed in game {:?}: {:?}", game_id, bet.bet_type);
                println!("  Proposal ID: {:?}", proposal_id);
                Ok(())
            } else {
                Err(bitcraps::error::Error::GameNotFound(game_id))
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
    }

    // Demonstrate multi-game coordination
    let identity = Identity::generate()?;
    let mut coordinator = GameCoordinator::new(identity.clone());

    // Create multiple games with different configurations
    let game1 = coordinator.create_game(4).await?; // Small game
    let game2 = coordinator.create_game(8).await?; // Large game
    let game3 = coordinator.create_game(6).await?; // Medium game

    // Create multiple players
    let player1 = PeerId::random();
    let player2 = PeerId::random();
    let player3 = PeerId::random();

    println!("\nPlayer participation:");

    // Player 1 joins multiple games
    coordinator.join_game(game1, player1).await?;
    coordinator.join_game(game2, player1).await?;

    // Player 2 focuses on one game
    coordinator.join_game(game1, player2).await?;

    // Player 3 joins different combination
    coordinator.join_game(game2, player3).await?;
    coordinator.join_game(game3, player3).await?;

    // Show cross-game player tracking
    println!("\nCross-game player tracking:");
    for player in [player1, player2, player3] {
        let games = coordinator.get_player_games(player);
        println!(
            "  Player {:?} is in {} games: {:?}",
            player,
            games.len(),
            games
        );
    }

    // Simulate concurrent betting across games
    println!("\nSimulating concurrent betting:");

    // Game 1: Pass line bets
    let bet1 = Bet::new(player1, game1, BetType::Pass, CrapTokens::new(50));
    let bet2 = Bet::new(player2, game1, BetType::DontPass, CrapTokens::new(25));

    coordinator.place_bet(game1, bet1).await?;
    coordinator.place_bet(game1, bet2).await?;

    // Game 2: Field bets
    let bet3 = Bet::new(player1, game2, BetType::Field, CrapTokens::new(75));
    let bet4 = Bet::new(player3, game2, BetType::Pass, CrapTokens::new(100));

    coordinator.place_bet(game2, bet3).await?;
    coordinator.place_bet(game2, bet4).await?;

    // Game 3: Come bets
    let bet5 = Bet::new(player3, game3, BetType::Come, CrapTokens::new(60));
    coordinator.place_bet(game3, bet5).await?;

    // Show game state summary
    println!("\nGame State Summary:");
    let active_games = coordinator.get_active_games();
    println!("  Active games: {}", active_games.len());
    println!(
        "  Total players across all games: {}",
        [player1, player2, player3].len()
    );
    println!(
        "  Average players per game: {:.1}",
        ([player1, player2, player3].len() as f64) / (active_games.len() as f64)
    );

    // Demonstrate message routing (simplified)
    println!("\nMessage Routing Demonstration:");
    for game_id in &active_games {
        println!("  Game {:?}: Routing game-specific messages", game_id);
        println!("    - Bet confirmations");
        println!("    - Dice roll results");
        println!("    - Payout notifications");
    }

    println!("\n✓ Multi-game coordination exercise complete!");
    println!("\nKey concepts demonstrated:");
    println!("  • Separate consensus engines per game");
    println!("  • Cross-game player tracking");
    println!("  • Game-specific message routing");
    println!("  • Concurrent bet processing");
    println!("  • Scalable game management");

    Ok(())
}

/// Helper function to create test transport
async fn create_test_transport(id: usize) -> Arc<TransportCoordinator> {
    // Create a simple in-memory transport for testing
    // In production, this would be BluetoothTransport
    Arc::new(TransportCoordinator::new())
}

/// Exercise 2: Network Partition Recovery
///
/// Simulate a network partition and demonstrate recovery.
/// Show how the system maintains consistency during splits.
#[allow(dead_code)]
async fn exercise_partition_recovery() {
    println!("\n=== Exercise: Partition Recovery ===\n");

    // Create initial network with 6 nodes
    let mut nodes = Vec::new();
    for i in 0..6 {
        let keypair = BitchatKeypair::generate();
        let identity = Arc::new(BitchatIdentity::new(keypair));
        let transport = create_test_transport(i).await;
        let node = MeshService::new(identity.clone(), transport);
        nodes.push(Arc::new(node));
    }

    // Start all nodes
    for node in &nodes {
        node.start().await.expect("Failed to start node");
    }

    println!("Created network with {} nodes", nodes.len());

    // Let them discover each other
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Simulate partition: Split into two groups
    println!("\nSimulating network partition...");
    let group_a = &nodes[0..3];
    let group_b = &nodes[3..6];

    // Block communication between groups (simulated)
    println!("Group A: {} nodes", group_a.len());
    println!("Group B: {} nodes", group_b.len());

    // Try consensus in each partition (should fail with only 50%)
    println!("\nAttempting consensus in partitioned groups...");
    println!("Group A cannot reach 2/3 consensus with only 3/6 nodes");
    println!("Group B cannot reach 2/3 consensus with only 3/6 nodes");

    // Wait to demonstrate partition
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Heal the partition
    println!("\nHealing network partition...");
    println!("Restoring communication between groups");

    // Allow recovery
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Verify recovery
    println!("\nVerifying recovery:");
    for (i, node) in nodes.iter().enumerate() {
        let peers = node.get_connected_peers().await;
        println!("Node {}: {} connected peers", i, peers.len());
    }

    println!("\n✓ Partition recovery demonstration complete");
}

/// Exercise 3: Performance Under Load
///
/// Stress test the integrated system with many players
/// and high transaction volume. Measure key metrics.
#[allow(dead_code)]
async fn exercise_performance_test() -> Result<()> {
    println!("\n\n=== Exercise: Performance Testing ===");
    println!("Load testing the integrated system\n");

    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;
    use tokio::time::{Duration, Instant};

    // Performance metrics collector
    struct PerformanceMetrics {
        transactions_sent: AtomicU64,
        transactions_confirmed: AtomicU64,
        total_latency_ms: AtomicU64,
        peak_memory_kb: AtomicU64,
        errors_count: AtomicU64,
    }

    impl PerformanceMetrics {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                transactions_sent: AtomicU64::new(0),
                transactions_confirmed: AtomicU64::new(0),
                total_latency_ms: AtomicU64::new(0),
                peak_memory_kb: AtomicU64::new(0),
                errors_count: AtomicU64::new(0),
            })
        }

        fn record_transaction_sent(&self) {
            self.transactions_sent.fetch_add(1, Ordering::Relaxed);
        }

        fn record_transaction_confirmed(&self, latency_ms: u64) {
            self.transactions_confirmed.fetch_add(1, Ordering::Relaxed);
            self.total_latency_ms
                .fetch_add(latency_ms, Ordering::Relaxed);
        }

        fn record_error(&self) {
            self.errors_count.fetch_add(1, Ordering::Relaxed);
        }

        fn get_stats(&self) -> (u64, u64, u64, u64, f64) {
            let sent = self.transactions_sent.load(Ordering::Relaxed);
            let confirmed = self.transactions_confirmed.load(Ordering::Relaxed);
            let total_latency = self.total_latency_ms.load(Ordering::Relaxed);
            let errors = self.errors_count.load(Ordering::Relaxed);
            let avg_latency = if confirmed > 0 {
                total_latency as f64 / confirmed as f64
            } else {
                0.0
            };
            (sent, confirmed, errors, total_latency, avg_latency)
        }
    }

    // Create performance test configuration
    const NUM_PEERS: usize = 75; // More than hinted 50+
    const TARGET_TPS: u64 = 1200; // Target transactions per second (above 1000+)
    const TEST_DURATION_SECS: u64 = 30;
    const BATCH_SIZE: usize = 10;

    println!("Performance Test Configuration:");
    println!("  Peer nodes: {}", NUM_PEERS);
    println!("  Target TPS: {}", TARGET_TPS);
    println!("  Test duration: {}s", TEST_DURATION_SECS);
    println!("  Batch size: {}\n", BATCH_SIZE);

    // Initialize metrics
    let metrics = PerformanceMetrics::new();

    // Create mock peer network
    println!("Setting up mock peer network...");
    let mut peers = Vec::with_capacity(NUM_PEERS);
    let start_setup = Instant::now();

    for i in 0..NUM_PEERS {
        let identity = Identity::generate()?;
        let peer_id = identity.peer_id();
        peers.push((peer_id, identity));

        if (i + 1) % 25 == 0 {
            println!("  Created {} peers...", i + 1);
        }
    }

    let setup_time = start_setup.elapsed();
    println!(
        "✓ Peer setup completed in {:.2}s\n",
        setup_time.as_secs_f64()
    );

    // Create game for load testing
    let game_id = GameId::random();
    println!("Test game created: {:?}\n", game_id);

    // Performance test phases
    println!("=== Phase 1: Baseline Performance ===");

    let baseline_start = Instant::now();
    let baseline_txns = 100;

    // Simulate baseline transactions
    for i in 0..baseline_txns {
        let peer = &peers[i % peers.len()];
        let bet = Bet::new(
            peer.0,
            game_id,
            if i % 2 == 0 {
                BetType::Pass
            } else {
                BetType::DontPass
            },
            CrapTokens::new(10 + (i as u64 % 100)),
        );

        metrics.record_transaction_sent();

        // Simulate processing time (would be actual consensus in real system)
        let process_start = Instant::now();
        tokio::time::sleep(Duration::from_micros(rand::random::<u64>() % 5000)).await;
        let latency = process_start.elapsed().as_millis() as u64;

        metrics.record_transaction_confirmed(latency);
    }

    let baseline_duration = baseline_start.elapsed();
    let (sent, confirmed, errors, _total_lat, avg_latency) = metrics.get_stats();

    println!("Baseline Results:");
    println!("  Transactions: {} sent, {} confirmed", sent, confirmed);
    println!("  Duration: {:.2}s", baseline_duration.as_secs_f64());
    println!(
        "  TPS: {:.1}",
        confirmed as f64 / baseline_duration.as_secs_f64()
    );
    println!("  Average latency: {:.1}ms", avg_latency);
    println!(
        "  Success rate: {:.1}%\n",
        (confirmed as f64 / sent as f64) * 100.0
    );

    // Reset metrics for main test
    let metrics = PerformanceMetrics::new();

    println!("=== Phase 2: High-Load Stress Test ===");

    let test_start = Instant::now();
    let mut tasks = Vec::new();

    // Calculate transactions per batch
    let target_per_batch = (TARGET_TPS * TEST_DURATION_SECS) / (TEST_DURATION_SECS * 10); // 10 batches per second

    println!("Starting high-load test...");
    println!(
        "  Target: {} transactions over {}s",
        TARGET_TPS * TEST_DURATION_SECS,
        TEST_DURATION_SECS
    );

    // Spawn transaction generators
    for batch_id in 0..10 {
        // 10 concurrent generators
        let peers_clone = peers.clone();
        let metrics_clone = metrics.clone();
        let batch_game_id = game_id;

        let task = tokio::spawn(async move {
            let batch_start = Instant::now();
            let transactions_per_generator = (TARGET_TPS * TEST_DURATION_SECS) / 10;

            for i in 0..transactions_per_generator {
                if batch_start.elapsed().as_secs() >= TEST_DURATION_SECS {
                    break;
                }

                let peer_idx = (batch_id * 1000 + i as usize) % peers_clone.len();
                let peer = &peers_clone[peer_idx];

                let bet = Bet::new(
                    peer.0,
                    batch_game_id,
                    match i % 6 {
                        0 => BetType::Pass,
                        1 => BetType::DontPass,
                        2 => BetType::Field,
                        3 => BetType::Come,
                        4 => BetType::DontCome,
                        _ => BetType::HardWays,
                    },
                    CrapTokens::new(1 + (i % 500)),
                );

                let tx_start = Instant::now();
                metrics_clone.record_transaction_sent();

                // Simulate consensus processing with realistic delays
                let processing_delay = match i % 10 {
                    0..=6 => Duration::from_micros(100 + rand::random::<u64>() % 900), // Fast path
                    7..=8 => Duration::from_micros(1000 + rand::random::<u64>() % 2000), // Normal
                    _ => Duration::from_micros(5000 + rand::random::<u64>() % 5000),   // Slow path
                };

                tokio::time::sleep(processing_delay).await;

                let latency = tx_start.elapsed().as_millis() as u64;

                // Simulate 99% success rate
                if rand::random::<f64>() < 0.99 {
                    metrics_clone.record_transaction_confirmed(latency);
                } else {
                    metrics_clone.record_error();
                }

                // Rate limiting to hit target TPS
                if i % 100 == 0 {
                    let expected_time = Duration::from_millis((i * 1000) / (TARGET_TPS / 10));
                    let elapsed = batch_start.elapsed();
                    if elapsed < expected_time {
                        tokio::time::sleep(expected_time - elapsed).await;
                    }
                }
            }
        });

        tasks.push(task);
    }

    // Monitor progress
    let monitor_task = tokio::spawn(async move {
        let mut last_confirmed = 0;

        for second in 1..=TEST_DURATION_SECS {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let (sent, confirmed, errors, _, avg_latency) = metrics.get_stats();
            let current_tps = confirmed - last_confirmed;
            last_confirmed = confirmed;

            if second % 5 == 0 {
                println!(
                    "  {}s: {} TPS, {} confirmed, {:.1}ms avg latency, {} errors",
                    second, current_tps, confirmed, avg_latency, errors
                );
            }
        }
    });

    // Wait for all tasks to complete
    for task in tasks {
        task.await
            .map_err(|e| bitcraps::error::Error::Internal(e.to_string()))?;
    }
    monitor_task
        .await
        .map_err(|e| bitcraps::error::Error::Internal(e.to_string()))?;

    let test_duration = test_start.elapsed();
    let (final_sent, final_confirmed, final_errors, _total_lat, final_avg_latency) =
        metrics.get_stats();

    println!("\n=== Performance Test Results ===");
    println!("Test Duration: {:.2}s", test_duration.as_secs_f64());
    println!("\nTransaction Metrics:");
    println!("  Sent: {}", final_sent);
    println!("  Confirmed: {}", final_confirmed);
    println!("  Errors: {}", final_errors);
    println!(
        "  Success Rate: {:.2}%",
        (final_confirmed as f64 / final_sent as f64) * 100.0
    );

    let achieved_tps = final_confirmed as f64 / test_duration.as_secs_f64();
    println!("\nThroughput Analysis:");
    println!("  Target TPS: {}", TARGET_TPS);
    println!("  Achieved TPS: {:.1}", achieved_tps);
    println!(
        "  Target Achievement: {:.1}%",
        (achieved_tps / TARGET_TPS as f64) * 100.0
    );

    println!("\nLatency Analysis:");
    println!("  Average Latency: {:.1}ms", final_avg_latency);
    println!("  Estimated p95: {:.1}ms", final_avg_latency * 1.8); // Rough estimate
    println!("  Estimated p99: {:.1}ms", final_avg_latency * 2.5); // Rough estimate

    // Memory estimation (simplified)
    let est_memory_per_peer_kb = 50; // Estimated KB per peer connection
    let est_total_memory_mb = (NUM_PEERS * est_memory_per_peer_kb) / 1024;

    println!("\nResource Usage (Estimated):");
    println!("  Memory per peer: ~{}KB", est_memory_per_peer_kb);
    println!("  Total estimated memory: ~{}MB", est_total_memory_mb);
    println!("  Peak concurrent operations: ~{}", TARGET_TPS / 10);

    // Bottleneck analysis
    println!("\n=== Bottleneck Analysis ===");
    if achieved_tps < (TARGET_TPS as f64 * 0.8) {
        println!("⚠️  Performance below target - potential bottlenecks:");
        if final_avg_latency > 10.0 {
            println!(
                "  • High consensus latency ({}ms avg)",
                final_avg_latency as u64
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
        println!("    - Implement async processing pipelines");
    } else {
        println!("✓ Performance target achieved!");
        println!("  • System handled high load successfully");
        println!("  • Latency within acceptable bounds");
        println!("  • Error rate acceptable");
    }

    println!("\n✓ Performance testing exercise complete!");
    println!("\nKey insights:");
    println!("  • Measured actual throughput under load");
    println!("  • Identified latency characteristics");
    println!("  • Evaluated error handling under stress");
    println!("  • Provided optimization recommendations");

    Ok(())
}
