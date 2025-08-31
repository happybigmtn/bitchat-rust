//! Full integration demonstration showing how all components work together
//!
//! Run with: cargo run --example full_integration_demo

use bitcraps::crypto::{Identity, SessionManager};
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
    println!("-" * 50);

    let identity = Identity::generate()?;
    let peer_id = PeerId::from_public_key(&identity.public_key);
    println!("Generated identity: {:?}", peer_id);

    let session_manager = SessionManager::new(identity.clone());
    println!("Session manager initialized\n");

    // Phase 2: Transport Layer Setup
    println!("Phase 2: Initializing transport layer...");
    println!("-" * 50);

    let transport = Arc::new(BluetoothTransport::new().await
        .map_err(|e| bitcraps::error::Error::Transport(e.to_string()))?);
    let coordinator = TransportCoordinator::new(transport.clone());

    // Start transport discovery
    coordinator.start_discovery().await?;
    println!("Bluetooth discovery started");
    println!("Scanning for peers...\n");

    // Phase 3: Mesh Network Formation
    println!("Phase 3: Creating mesh network...");
    println!("-" * 50);

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
    println!("-" * 50);

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
    println!("-" * 50);

    let game_id = GameId::random();
    let mut game = CrapsGame::new(game_id);

    println!("Game created: {:?}", game_id);
    println!("Initial state: Come out roll\n");

    // Phase 6: Simulated Game Flow
    println!("Phase 6: Demonstrating game flow...");
    println!("-" * 50);

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
    let proposal_id = consensus.submit_proposal(bitcraps::protocol::consensus::engine::GameOperation::PlaceBet {
        player: peers[0],
        bet,
        nonce: 12345,  // Demo nonce
    })?;

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
    println!("-" * 50);

    let mesh_stats = mesh.get_statistics().await;
    println!("Mesh Network:");
    println!("  Connected peers: {}", peers.len());
    println!("  Messages sent: {}", mesh_stats.messages_sent);
    println!("  Messages received: {}", mesh_stats.messages_received);
    println!("  Cache hits: {}", mesh_stats.cache_hits);

    // Get final statistics
    println!("Demo completed successfully!");
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
async fn exercise_concurrent_games() {
    // TODO: Implement multi-game support
    // Hints:
    // 1. Create multiple GameId instances
    // 2. Use separate consensus engines or namespacing
    // 3. Route messages to correct game
    // 4. Handle cross-game player tracking
}

/// Exercise 2: Network Partition Recovery
///
/// Simulate a network partition and demonstrate recovery.
/// Show how the system maintains consistency during splits.
#[allow(dead_code)]
async fn exercise_partition_recovery() {
    // TODO: Implement partition recovery
    // Hints:
    // 1. Split mesh into two groups
    // 2. Show neither can reach consensus alone
    // 3. Reunite groups
    // 4. Demonstrate state reconciliation
}

/// Exercise 3: Performance Under Load
///
/// Stress test the integrated system with many players
/// and high transaction volume. Measure key metrics.
#[allow(dead_code)]
async fn exercise_performance_test() {
    // TODO: Implement performance testing
    // Hints:
    // 1. Create 50+ peer connections
    // 2. Generate 1000+ transactions/second
    // 3. Measure consensus latency
    // 4. Track memory and CPU usage
    // 5. Identify bottlenecks
}
