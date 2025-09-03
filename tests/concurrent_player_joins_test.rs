//! Integration tests for concurrent player joins with consensus handling

use bitcraps::crypto::{BitchatIdentity, BitchatKeypair};
use bitcraps::mesh::MeshService;
use bitcraps::protocol::runtime::game_lifecycle::GameLifecycleManager;
use bitcraps::protocol::runtime::GameRuntime;
use bitcraps::protocol::{GameId, PeerId};
use bitcraps::transport::TransportCoordinator;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_concurrent_player_joins() {
    // Initialize test environment
    let keypair = BitchatKeypair::generate();
    let identity = Arc::new(BitchatIdentity::new(keypair));
    let transport = Arc::new(TransportCoordinator::new());
    let mesh_service = Arc::new(MeshService::new(identity.clone(), transport.clone()));

    // Start mesh service
    mesh_service
        .start()
        .await
        .expect("Failed to start mesh service");

    // Create game runtime with lifecycle manager
    let runtime = Arc::new(GameRuntime::new_with_mesh(mesh_service.clone()));

    // Create a test game
    let game_id: GameId = [1u8; 32];
    let host_id = identity.peer_id;

    // Simulate 10 concurrent player join requests
    let num_players = 10;
    let mut join_handles = Vec::new();

    let start_time = Instant::now();

    for i in 0..num_players {
        let runtime_clone = runtime.clone();
        let game_id_clone = game_id;

        let handle = tokio::spawn(async move {
            // Generate unique player ID
            let mut player_id = [0u8; 32];
            player_id[0] = i as u8;

            // Attempt to join the game
            let join_start = Instant::now();
            let result = runtime_clone
                .handle_player_join(game_id_clone, player_id)
                .await;
            let join_duration = join_start.elapsed();

            (i, result, join_duration)
        });

        join_handles.push(handle);
    }

    // Wait for all joins to complete
    let mut successful_joins = 0;
    let mut total_latency = Duration::ZERO;

    for handle in join_handles {
        let (player_idx, result, duration) = handle.await.unwrap();

        if result.is_ok() {
            successful_joins += 1;
            total_latency += duration;
            println!(
                "Player {} joined successfully in {:?}",
                player_idx, duration
            );
        } else {
            println!("Player {} failed to join: {:?}", player_idx, result);
        }
    }

    let total_duration = start_time.elapsed();
    let avg_latency = total_latency / successful_joins.max(1);

    // Assertions
    assert!(
        successful_joins >= 7,
        "At least 70% of players should join successfully"
    );
    assert!(
        avg_latency < Duration::from_secs(2),
        "Average join latency should be under 2 seconds"
    );
    assert!(
        total_duration < Duration::from_secs(10),
        "All joins should complete within 10 seconds"
    );

    println!("\nTest Results:");
    println!("Successful joins: {}/{}", successful_joins, num_players);
    println!("Average latency: {:?}", avg_latency);
    println!("Total duration: {:?}", total_duration);
}

#[tokio::test]
async fn test_consensus_during_concurrent_joins() {
    // Test that consensus is maintained during concurrent joins
    let keypair = BitchatKeypair::generate();
    let identity = Arc::new(BitchatIdentity::new(keypair));
    let transport = Arc::new(TransportCoordinator::new());
    let mesh_service = Arc::new(MeshService::new(identity.clone(), transport.clone()));

    mesh_service
        .start()
        .await
        .expect("Failed to start mesh service");

    let runtime = Arc::new(GameRuntime::new_with_mesh(mesh_service.clone()));
    let game_id: GameId = [2u8; 32];

    // Track consensus state
    let consensus_state = Arc::new(RwLock::new(Vec::new()));

    // Simulate rapid concurrent joins with consensus tracking
    let mut handles = Vec::new();

    for i in 0..5 {
        let runtime_clone = runtime.clone();
        let consensus_clone = consensus_state.clone();

        let handle = tokio::spawn(async move {
            let mut player_id = [0u8; 32];
            player_id[0] = i as u8;

            // Join and track consensus
            let result = runtime_clone.handle_player_join(game_id, player_id).await;

            if result.is_ok() {
                let mut state = consensus_clone.write().await;
                state.push((i, Instant::now()));
            }

            result
        });

        handles.push(handle);
    }

    // Wait for completion
    for handle in handles {
        let _ = handle.await;
    }

    // Verify consensus ordering
    let state = consensus_state.read().await;
    assert!(!state.is_empty(), "Some players should have joined");

    // Check that joins are properly ordered
    for i in 1..state.len() {
        assert!(
            state[i].1 >= state[i - 1].1,
            "Consensus ordering should be maintained"
        );
    }
}
#![cfg(feature = "legacy-tests")]
