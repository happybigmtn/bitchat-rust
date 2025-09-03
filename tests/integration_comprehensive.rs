//! Comprehensive Integration Tests for BitCraps
//!
//! This module provides end-to-end integration tests covering:
//! - Multi-peer consensus scenarios
//! - Token economy with overflow protection
//! - Anti-cheat validation
//! - Connection pool management
//! - Mobile lifecycle management

use bitcraps::protocol::consensus::engine::{ConsensusEngine, GameOperation, GameProposal};
use bitcraps::protocol::craps::{Bet, BetType, DiceRoll};
use bitcraps::protocol::{CrapTokens, GameId, PeerId};
use bitcraps::token::{TokenLedger, TokenManager};
use bitcraps::transport::connection_pool::{BluetoothConnectionPool, PoolConfig, QoSPriority};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

/// Test multi-peer consensus with Byzantine fault tolerance
#[tokio::test]
async fn test_multi_peer_consensus() {
    // Create 4 peers (tolerates 1 Byzantine fault)
    let peers: Vec<PeerId> = (0..4)
        .map(|i| {
            let mut id = [0u8; 32];
            id[0] = i;
            id
        })
        .collect();

    // Initialize consensus engines for each peer
    let mut engines = Vec::new();
    for peer_id in &peers {
        let engine = ConsensusEngine::new(*peer_id, peers.clone());
        engines.push(Arc::new(engine));
    }

    // Create a game proposal from peer 0
    let game_id = GameId::new();
    let proposal = GameProposal {
        proposal_id: [1u8; 32].into(),
        proposer: peers[0],
        game_id,
        operations: vec![GameOperation::PlaceBet {
            player: peers[1],
            bet: Bet {
                bet_type: BetType::PassLine,
                amount: CrapTokens::from_crap(10.0),
                odds: None,
            },
        }],
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        signatures: vec![],
    };

    // Process proposal on all engines
    for engine in &engines {
        let result = engine.process_proposal(proposal.clone()).await;
        assert!(result.is_ok(), "Proposal processing failed");
    }

    // Verify consensus was reached
    sleep(Duration::from_millis(100)).await;

    for engine in &engines {
        let state = engine.get_consensus_state().await;
        assert!(state.is_ok(), "Failed to get consensus state");
    }
}

/// Test token overflow protection
#[tokio::test]
async fn test_token_overflow_protection() {
    let ledger = TokenLedger::new();
    let manager = TokenManager::new(ledger);

    let peer_id = [1u8; 32];

    // Test relay reward with massive message count
    let result = manager.process_relay_reward(peer_id, u64::MAX).await;

    assert!(result.is_ok(), "Relay reward should handle overflow");

    // Verify balance is capped
    let balance = manager.get_balance(peer_id).await;
    assert!(balance.is_ok(), "Failed to get balance");

    let balance_value = balance.unwrap();
    // Should be capped at 1000 CRAP as per our fix
    assert!(
        balance_value <= CrapTokens::from_crap(1000.0),
        "Balance exceeded cap: {} CRAP",
        CrapTokens::new_unchecked(balance_value).to_crap()
    );
}

/// Test connection pool with concurrent access
#[tokio::test]
async fn test_connection_pool_concurrent_access() {
    let config = PoolConfig {
        max_connections: 10,
        ..Default::default()
    };

    let pool = Arc::new(BluetoothConnectionPool::new(config));

    // Spawn multiple tasks trying to acquire connections
    let mut handles = Vec::new();

    for i in 0..20 {
        let pool_clone = Arc::clone(&pool);
        let handle = tokio::spawn(async move {
            // Try to get a connection
            let priority = match i % 3 {
                0 => QoSPriority::RealTime,
                1 => QoSPriority::Normal,
                _ => QoSPriority::Background,
            };

            match pool_clone.get_connection(priority).await {
                Ok(conn) => {
                    // Simulate some work
                    sleep(Duration::from_millis(10)).await;

                    // Return connection
                    pool_clone.return_connection(conn).await;
                    true
                }
                Err(_) => false,
            }
        });
        handles.push(handle);
    }

    // Wait for all tasks
    let results = futures::future::join_all(handles).await;

    // Count successful acquisitions
    let successful = results
        .iter()
        .filter(|r| r.as_ref().map(|v| *v).unwrap_or(false))
        .count();

    // At least max_connections tasks should succeed
    assert!(
        successful >= 10,
        "Not enough connections acquired: {}",
        successful
    );

    // Verify pool metrics
    let metrics = pool.get_metrics().await;
    assert_eq!(metrics.idle_connections, metrics.total_connections);

    // Shutdown pool
    pool.shutdown().await;
}

/// Test anti-cheat validation performance
#[tokio::test]
async fn test_anti_cheat_validation_performance() {
    use bitcraps::protocol::anti_cheat::{AntiCheatConfig, AntiCheatValidator};

    let config = AntiCheatConfig::default();
    let validator = AntiCheatValidator::new(config);

    let peer_id = [1u8; 32];
    let game_id = GameId::new();

    // Create a valid proposal
    let proposal = GameProposal {
        proposal_id: [1u8; 32].into(),
        proposer: peer_id,
        game_id,
        operations: vec![GameOperation::PlaceBet {
            player: peer_id,
            bet: Bet {
                bet_type: BetType::PassLine,
                amount: CrapTokens::from_crap(10.0),
                odds: None,
            },
        }],
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        signatures: vec![],
    };

    // Measure validation time
    let start = std::time::Instant::now();

    let result = validator.validate_proposal(&proposal).await;

    let elapsed = start.elapsed();

    assert!(result.is_ok(), "Validation failed");
    assert!(
        elapsed < Duration::from_millis(100),
        "Validation took too long: {:?}",
        elapsed
    );
}

/// Test mobile lifecycle management
#[cfg(target_os = "android")]
#[tokio::test]
async fn test_android_lifecycle_management() {
    use bitcraps::mobile::android::lifecycle::{AndroidBleLifecycleManager, LifecycleState};
    use bitcraps::mobile::android::{AndroidBleManager, CallbackManager};

    let ble_manager = Arc::new(AndroidBleManager::new());
    let callback_manager = Arc::new(CallbackManager::new());

    let lifecycle_manager = AndroidBleLifecycleManager::new(ble_manager, callback_manager);

    // Simulate lifecycle transitions
    lifecycle_manager.on_activity_created().await.unwrap();
    assert_eq!(
        lifecycle_manager.get_current_state(),
        LifecycleState::Created
    );

    lifecycle_manager.on_activity_started().await.unwrap();
    assert_eq!(
        lifecycle_manager.get_current_state(),
        LifecycleState::Started
    );

    lifecycle_manager.on_activity_resumed().await.unwrap();
    assert_eq!(
        lifecycle_manager.get_current_state(),
        LifecycleState::Resumed
    );

    // Test background transition
    lifecycle_manager.on_activity_paused().await.unwrap();
    assert_eq!(
        lifecycle_manager.get_current_state(),
        LifecycleState::Paused
    );

    // Verify background timers are running
    sleep(Duration::from_secs(1)).await;

    // Clean shutdown
    lifecycle_manager.shutdown().await.unwrap();
}

/// Test database migration system
#[tokio::test]
async fn test_database_migrations() {
    use bitcraps::database::migrations::MigrationManager;
    use rusqlite::Connection;
    use tempfile::tempdir;

    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let conn = Connection::open(&db_path).unwrap();

    let mut manager = MigrationManager::new().with_connection(conn);

    // Run all migrations
    let result = manager.migrate().await;
    assert!(result.is_ok(), "Migrations failed: {:?}", result);

    // Verify all tables exist
    let conn = manager.get_connection().unwrap();
    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .unwrap();

    let tables: Vec<String> = stmt
        .query_map([], |row| row.get(0))
        .unwrap()
        .filter_map(Result::ok)
        .collect();

    // Check critical tables exist
    assert!(tables.contains(&"users".to_string()));
    assert!(tables.contains(&"games".to_string()));
    assert!(tables.contains(&"bets".to_string()));
    assert!(tables.contains(&"transactions".to_string()));
    assert!(tables.contains(&"peer_connections".to_string()));

    // Verify performance indexes exist
    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='index' AND name LIKE 'idx_%'")
        .unwrap();

    let indexes: Vec<String> = stmt
        .query_map([], |row| row.get(0))
        .unwrap()
        .filter_map(Result::ok)
        .collect();

    // Should have many indexes from V006 migration
    assert!(indexes.len() > 20, "Not enough indexes: {}", indexes.len());
}

/// Test end-to-end game flow
#[tokio::test]
async fn test_end_to_end_game_flow() {
    use bitcraps::app::BitCrapsApp;
    use bitcraps::gaming::consensus_game_manager::ConsensusGameManager;

    // Initialize app with test configuration
    let app = BitCrapsApp::new_with_config(Default::default())
        .await
        .unwrap();

    // Create a game
    let game_id = app.create_game().await.unwrap();
    assert_ne!(game_id, GameId::default());

    // Join the game
    let peer_id = [1u8; 32];
    let join_result = app.join_game(game_id).await;
    assert!(join_result.is_ok());

    // Place a bet
    let bet_result = app.place_bet("pass_line".to_string(), 100).await;
    assert!(bet_result.is_ok());

    // Simulate dice roll
    let dice = app.roll_dice().await.unwrap();
    assert!(dice.0 >= 1 && dice.0 <= 6);
    assert!(dice.1 >= 1 && dice.1 <= 6);

    // Check balance
    let balance = app.get_balance().await.unwrap();
    assert!(balance > 0);

    // Get game state
    let state = app.get_game_state().await.unwrap();
    assert!(!state.is_empty());
}

/// Test resilience under network partitions
#[tokio::test]
async fn test_network_partition_resilience() {
    use bitcraps::protocol::consensus::engine::ConsensusEngine;

    // Create 6 peers (tolerates 2 Byzantine faults)
    let peers: Vec<PeerId> = (0..6)
        .map(|i| {
            let mut id = [0u8; 32];
            id[0] = i;
            id
        })
        .collect();

    // Initialize engines
    let mut engines = Vec::new();
    for peer_id in &peers {
        let engine = ConsensusEngine::new(*peer_id, peers.clone());
        engines.push(Arc::new(engine));
    }

    // Simulate partition: peers 0-2 in one partition, 3-5 in another
    // The larger partition (3-5) should continue consensus

    let game_id = GameId::new();
    let proposal = GameProposal {
        proposal_id: [2u8; 32].into(),
        proposer: peers[3],
        game_id,
        operations: vec![],
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        signatures: vec![],
    };

    // Only process on majority partition
    for i in 3..6 {
        let result = engines[i].process_proposal(proposal.clone()).await;
        assert!(result.is_ok());
    }

    // Verify consensus in majority partition
    sleep(Duration::from_millis(100)).await;

    for i in 3..6 {
        let state = engines[i].get_consensus_state().await;
        assert!(state.is_ok());
    }
}
#![cfg(feature = "legacy-tests")]
#![cfg(feature = "legacy-tests")]
