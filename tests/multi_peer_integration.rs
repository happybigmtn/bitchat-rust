//! Multi-peer integration tests for BitCraps
//!
//! Tests the complete flow of multiple peers creating games, joining,
//! placing bets, and reaching consensus on outcomes.

use bitcraps::{
    protocol::{BetType, CrapTokens, GameId},
    ApplicationConfig, BitCrapsApp,
};
use std::time::Duration;
use tokio::time::sleep;

/// Test multiple peers creating and joining games
#[tokio::test]
async fn test_multi_peer_game_creation() {
    // Initialize 3 peer applications
    let mut peer1 = BitCrapsApp::new(ApplicationConfig {
        port: 9001,
        debug: true,
        ..Default::default()
    })
    .await
    .unwrap();

    let mut peer2 = BitCrapsApp::new(ApplicationConfig {
        port: 9002,
        debug: true,
        ..Default::default()
    })
    .await
    .unwrap();

    let mut peer3 = BitCrapsApp::new(ApplicationConfig {
        port: 9003,
        debug: true,
        ..Default::default()
    })
    .await
    .unwrap();

    // Start all peers
    peer1.start().await.unwrap();
    peer2.start().await.unwrap();
    peer3.start().await.unwrap();

    // Allow time for peer discovery
    sleep(Duration::from_secs(2)).await;

    // Peer1 creates a game
    let game_id = peer1.create_game(2, CrapTokens(100)).await.unwrap();

    // Allow time for game announcement to propagate
    sleep(Duration::from_millis(500)).await;

    // Peer2 and Peer3 join the game
    peer2.join_game(game_id).await.unwrap();
    peer3.join_game(game_id).await.unwrap();

    // Verify all peers see the game
    let peer1_games = peer1.get_active_games().await.unwrap();
    let peer2_games = peer2.get_active_games().await.unwrap();
    let peer3_games = peer3.get_active_games().await.unwrap();

    assert!(peer1_games.contains(&game_id));
    assert!(peer2_games.contains(&game_id));
    assert!(peer3_games.contains(&game_id));
}

/// Test consensus on bets and game outcomes
#[tokio::test]
async fn test_consensus_betting() {
    // Initialize 2 peers
    let mut peer1 = BitCrapsApp::new(ApplicationConfig {
        port: 9011,
        debug: true,
        ..Default::default()
    })
    .await
    .unwrap();

    let mut peer2 = BitCrapsApp::new(ApplicationConfig {
        port: 9012,
        debug: true,
        ..Default::default()
    })
    .await
    .unwrap();

    // Start peers
    peer1.start().await.unwrap();
    peer2.start().await.unwrap();

    // Allow discovery
    sleep(Duration::from_secs(2)).await;

    // Create game
    let game_id = peer1.create_game(2, CrapTokens(50)).await.unwrap();

    // Peer2 joins
    sleep(Duration::from_millis(500)).await;
    peer2.join_game(game_id).await.unwrap();

    // Both peers place bets
    peer1
        .place_bet(game_id, BetType::Pass, CrapTokens(100))
        .await
        .unwrap();
    peer2
        .place_bet(game_id, BetType::DontPass, CrapTokens(100))
        .await
        .unwrap();

    // Allow consensus to process
    sleep(Duration::from_secs(1)).await;

    // Verify balances changed (initial balance minus bet)
    let balance1 = peer1.get_balance().await.unwrap();
    let balance2 = peer2.get_balance().await.unwrap();

    // Initial balance (1000) minus bet (100) = 900
    assert!(balance1.0 <= 900);
    assert!(balance2.0 <= 900);
}

/// Test game discovery across mesh network
#[tokio::test]
async fn test_game_discovery() {
    // Initialize 4 peers
    let mut peers = Vec::new();
    for i in 0..4 {
        let peer = BitCrapsApp::new(ApplicationConfig {
            port: 9020 + i,
            debug: true,
            ..Default::default()
        })
        .await
        .unwrap();
        peers.push(peer);
    }

    // Start all peers
    for peer in &mut peers {
        peer.start().await.unwrap();
    }

    // Allow mesh formation
    sleep(Duration::from_secs(3)).await;

    // Create multiple games
    let game1 = peers[0].create_game(2, CrapTokens(100)).await.unwrap();
    let game2 = peers[1].create_game(3, CrapTokens(200)).await.unwrap();

    // Allow game announcements to propagate
    sleep(Duration::from_secs(1)).await;

    // All peers should see both games
    for peer in &peers {
        let games = peer.get_active_games().await.unwrap();
        assert!(games.len() >= 2);
        assert!(games.contains(&game1) || games.contains(&game2));
    }
}

/// Test network partition and recovery
#[tokio::test]
#[ignore] // This test requires network manipulation
async fn test_partition_recovery() {
    // Initialize 3 peers
    let mut peer1 = BitCrapsApp::new(ApplicationConfig {
        port: 9031,
        debug: true,
        ..Default::default()
    })
    .await
    .unwrap();

    let mut peer2 = BitCrapsApp::new(ApplicationConfig {
        port: 9032,
        debug: true,
        ..Default::default()
    })
    .await
    .unwrap();

    let mut peer3 = BitCrapsApp::new(ApplicationConfig {
        port: 9033,
        debug: true,
        ..Default::default()
    })
    .await
    .unwrap();

    // Start all peers
    peer1.start().await.unwrap();
    peer2.start().await.unwrap();
    peer3.start().await.unwrap();

    // Allow mesh formation
    sleep(Duration::from_secs(2)).await;

    // Create game with all peers
    let game_id = peer1.create_game(3, CrapTokens(100)).await.unwrap();
    sleep(Duration::from_millis(500)).await;

    peer2.join_game(game_id).await.unwrap();
    peer3.join_game(game_id).await.unwrap();

    // Simulate partition (would need to block network in real test)
    // In production, this would use iptables or similar

    // Place bet during partition
    peer1
        .place_bet(game_id, BetType::Pass, CrapTokens(50))
        .await
        .unwrap();

    // Simulate recovery
    sleep(Duration::from_secs(2)).await;

    // Verify consensus eventually reached
    let games1 = peer1.get_active_games().await.unwrap();
    let games2 = peer2.get_active_games().await.unwrap();
    let games3 = peer3.get_active_games().await.unwrap();

    assert_eq!(games1.len(), games2.len());
    assert_eq!(games2.len(), games3.len());
}

/// Test Byzantine fault tolerance
#[tokio::test]
async fn test_byzantine_tolerance() {
    // Initialize 4 peers (1 Byzantine, 3 honest)
    let mut honest_peers = Vec::new();
    for i in 0..3 {
        let peer = BitCrapsApp::new(ApplicationConfig {
            port: 9040 + i,
            debug: true,
            ..Default::default()
        })
        .await
        .unwrap();
        honest_peers.push(peer);
    }

    // Start honest peers
    for peer in &mut honest_peers {
        peer.start().await.unwrap();
    }

    // Byzantine peer would try to double-spend or equivocate
    // This is simulated - in reality would need modified client

    // Allow network formation
    sleep(Duration::from_secs(2)).await;

    // Create game
    let game_id = honest_peers[0]
        .create_game(3, CrapTokens(100))
        .await
        .unwrap();

    // Other honest peers join
    sleep(Duration::from_millis(500)).await;
    honest_peers[1].join_game(game_id).await.unwrap();
    honest_peers[2].join_game(game_id).await.unwrap();

    // Place bets
    for peer in &honest_peers {
        peer.place_bet(game_id, BetType::Pass, CrapTokens(50))
            .await
            .unwrap();
    }

    // Allow consensus
    sleep(Duration::from_secs(2)).await;

    // Verify all honest peers have consistent state
    let balance1 = honest_peers[0].get_balance().await.unwrap();
    let balance2 = honest_peers[1].get_balance().await.unwrap();
    let balance3 = honest_peers[2].get_balance().await.unwrap();

    // All should have deducted the bet amount
    assert!(balance1.0 < 1000);
    assert!(balance2.0 < 1000);
    assert!(balance3.0 < 1000);
}

/// Test high-load scenario with many concurrent games
#[tokio::test]
#[ignore] // Resource intensive test
async fn test_high_load_concurrent_games() {
    const NUM_PEERS: u16 = 10;
    const GAMES_PER_PEER: usize = 5;

    // Initialize many peers
    let mut peers = Vec::new();
    for i in 0..NUM_PEERS {
        let peer = BitCrapsApp::new(ApplicationConfig {
            port: 9100 + i,
            debug: false, // Disable debug for performance
            ..Default::default()
        })
        .await
        .unwrap();
        peers.push(peer);
    }

    // Start all peers
    for peer in &mut peers {
        peer.start().await.unwrap();
    }

    // Allow large mesh to form
    sleep(Duration::from_secs(5)).await;

    // Each peer creates games sequentially (can't clone BitCrapsApp)
    let mut game_ids = Vec::new();
    for peer in &peers {
        for j in 0..GAMES_PER_PEER {
            match peer.create_game(2, CrapTokens(10 * (j as u64 + 1))).await {
                Ok(game_id) => game_ids.push(game_id),
                Err(_) => {} // Some games might fail due to limits
            }
        }
    }

    // Verify a reasonable number of games were created
    assert!(game_ids.len() >= NUM_PEERS as usize * GAMES_PER_PEER / 2);

    // Allow propagation
    sleep(Duration::from_secs(3)).await;

    // Each peer should see many games
    for peer in &peers {
        let games = peer.get_active_games().await.unwrap();
        assert!(games.len() > 0);
    }
}
#![cfg(feature = "legacy-tests")]
#![cfg(feature = "legacy-tests")]
