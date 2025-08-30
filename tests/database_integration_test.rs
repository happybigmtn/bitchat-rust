//! Comprehensive Database Integration Tests for BitChat-Rust
//!
//! Tests all database operations including:
//! - Connection pooling
//! - Transaction handling
//! - Data persistence
//! - Migration system
//! - Error recovery
//! - Concurrent access

use bitcraps::database::{Database, DatabasePool, Migration};
use bitcraps::protocol::{CrapTokens, GameId, PeerId};
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::Barrier;

/// Helper to create a test database
async fn create_test_db() -> (DatabasePool, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let pool = DatabasePool::new(&db_path.to_str().unwrap())
        .await
        .expect("Failed to create test database");
    (pool, temp_dir)
}

#[tokio::test]
async fn test_database_initialization() {
    let (pool, _temp_dir) = create_test_db().await;

    // Verify database was created
    assert!(pool.is_healthy().await);

    // Test getting a connection
    let conn = pool.get_connection().await;
    assert!(conn.is_ok());
}

#[tokio::test]
async fn test_migration_system() {
    let (pool, _temp_dir) = create_test_db().await;

    // Apply migrations
    let migrations_applied = pool.apply_migrations().await.unwrap();
    assert!(migrations_applied > 0);

    // Verify schema version
    let version = pool.get_schema_version().await.unwrap();
    assert!(version > 0);

    // Test idempotency - applying again should do nothing
    let second_apply = pool.apply_migrations().await.unwrap();
    assert_eq!(second_apply, 0);
}

#[tokio::test]
async fn test_player_persistence() {
    let (pool, _temp_dir) = create_test_db().await;
    pool.apply_migrations().await.unwrap();

    // Create player
    let player_id = PeerId::generate();
    let initial_balance = CrapTokens::from_raw(1000);

    pool.create_player(player_id, initial_balance)
        .await
        .unwrap();

    // Retrieve player
    let balance = pool.get_player_balance(player_id).await.unwrap();
    assert_eq!(balance, initial_balance);

    // Update balance
    let new_balance = CrapTokens::from_raw(2000);
    pool.update_player_balance(player_id, new_balance)
        .await
        .unwrap();

    // Verify update
    let updated = pool.get_player_balance(player_id).await.unwrap();
    assert_eq!(updated, new_balance);
}

#[tokio::test]
async fn test_game_session_persistence() {
    let (pool, _temp_dir) = create_test_db().await;
    pool.apply_migrations().await.unwrap();

    // Create game session
    let game_id = GameId::generate();
    let creator = PeerId::generate();
    let participants = vec![creator, PeerId::generate(), PeerId::generate()];

    pool.create_game_session(game_id, creator, participants.clone())
        .await
        .unwrap();

    // Retrieve game session
    let session = pool.get_game_session(game_id).await.unwrap();
    assert_eq!(session.creator, creator);
    assert_eq!(session.participants, participants);

    // Update game state
    pool.update_game_state(game_id, "active").await.unwrap();

    // List active games
    let active_games = pool.list_active_games().await.unwrap();
    assert!(active_games.contains(&game_id));
}

#[tokio::test]
async fn test_transaction_handling() {
    let (pool, _temp_dir) = create_test_db().await;
    pool.apply_migrations().await.unwrap();

    let player1 = PeerId::generate();
    let player2 = PeerId::generate();
    let amount = CrapTokens::from_raw(500);

    // Create players
    pool.create_player(player1, CrapTokens::from_raw(1000))
        .await
        .unwrap();
    pool.create_player(player2, CrapTokens::from_raw(1000))
        .await
        .unwrap();

    // Test successful transaction
    let result = pool
        .transfer_tokens_transactional(player1, player2, amount)
        .await;
    assert!(result.is_ok());

    // Verify balances
    let balance1 = pool.get_player_balance(player1).await.unwrap();
    let balance2 = pool.get_player_balance(player2).await.unwrap();
    assert_eq!(balance1, CrapTokens::from_raw(500));
    assert_eq!(balance2, CrapTokens::from_raw(1500));

    // Test failed transaction (insufficient funds)
    let large_amount = CrapTokens::from_raw(5000);
    let result = pool
        .transfer_tokens_transactional(player1, player2, large_amount)
        .await;
    assert!(result.is_err());

    // Verify no changes on failure (rollback)
    let balance1_after = pool.get_player_balance(player1).await.unwrap();
    let balance2_after = pool.get_player_balance(player2).await.unwrap();
    assert_eq!(balance1_after, balance1);
    assert_eq!(balance2_after, balance2);
}

#[tokio::test]
async fn test_concurrent_access() {
    let (pool, _temp_dir) = create_test_db().await;
    pool.apply_migrations().await.unwrap();

    let player_id = PeerId::generate();
    pool.create_player(player_id, CrapTokens::from_raw(0))
        .await
        .unwrap();

    // Spawn multiple tasks to increment balance concurrently
    let pool = Arc::new(pool);
    let barrier = Arc::new(Barrier::new(10));
    let mut handles = vec![];

    for _ in 0..10 {
        let pool_clone = pool.clone();
        let barrier_clone = barrier.clone();

        let handle = tokio::spawn(async move {
            barrier_clone.wait().await;

            for _ in 0..100 {
                let current = pool_clone.get_player_balance(player_id).await.unwrap();
                let new_balance = CrapTokens::from_raw(current.as_raw() + 1);
                pool_clone
                    .update_player_balance(player_id, new_balance)
                    .await
                    .unwrap();
            }
        });

        handles.push(handle);
    }

    // Wait for all tasks
    for handle in handles {
        handle.await.unwrap();
    }

    // Verify final balance
    let final_balance = pool.get_player_balance(player_id).await.unwrap();
    assert_eq!(final_balance, CrapTokens::from_raw(1000));
}

#[tokio::test]
async fn test_data_integrity() {
    let (pool, _temp_dir) = create_test_db().await;
    pool.apply_migrations().await.unwrap();

    // Insert test data
    let player_id = PeerId::generate();
    let game_id = GameId::generate();

    pool.create_player(player_id, CrapTokens::from_raw(1000))
        .await
        .unwrap();
    pool.create_game_session(game_id, player_id, vec![player_id])
        .await
        .unwrap();

    // Test foreign key constraints
    let invalid_game = GameId::generate();
    let result = pool
        .record_bet(invalid_game, player_id, CrapTokens::from_raw(100))
        .await;
    assert!(result.is_err()); // Should fail due to missing game

    // Test unique constraints
    let duplicate_result = pool
        .create_player(player_id, CrapTokens::from_raw(500))
        .await;
    assert!(duplicate_result.is_err()); // Should fail due to duplicate
}

#[tokio::test]
async fn test_backup_and_restore() {
    let (pool, temp_dir) = create_test_db().await;
    pool.apply_migrations().await.unwrap();

    // Add test data
    let player_id = PeerId::generate();
    pool.create_player(player_id, CrapTokens::from_raw(5000))
        .await
        .unwrap();

    // Create backup
    let backup_path = temp_dir.path().join("backup.db");
    pool.backup_to(&backup_path).await.unwrap();

    // Corrupt original (simulate failure)
    pool.update_player_balance(player_id, CrapTokens::from_raw(0))
        .await
        .unwrap();

    // Restore from backup
    pool.restore_from(&backup_path).await.unwrap();

    // Verify data restored
    let balance = pool.get_player_balance(player_id).await.unwrap();
    assert_eq!(balance, CrapTokens::from_raw(5000));
}

#[tokio::test]
async fn test_connection_pooling() {
    let (pool, _temp_dir) = create_test_db().await;

    // Get multiple connections
    let mut connections = vec![];
    for _ in 0..5 {
        connections.push(pool.get_connection().await.unwrap());
    }

    // Verify all connections are valid
    for conn in &connections {
        assert!(conn.is_valid());
    }

    // Return connections to pool
    drop(connections);

    // Pool should be healthy
    assert!(pool.is_healthy().await);

    // Get connection again (should reuse from pool)
    let conn = pool.get_connection().await.unwrap();
    assert!(conn.is_valid());
}

#[tokio::test]
async fn test_query_performance() {
    let (pool, _temp_dir) = create_test_db().await;
    pool.apply_migrations().await.unwrap();

    // Insert many records
    for i in 0..1000 {
        let player_id = PeerId::from_bytes(&i.to_le_bytes());
        pool.create_player(player_id, CrapTokens::from_raw(i as u64))
            .await
            .unwrap();
    }

    // Test indexed query performance
    let start = std::time::Instant::now();
    let player_id = PeerId::from_bytes(&500u64.to_le_bytes());
    let _balance = pool.get_player_balance(player_id).await.unwrap();
    let duration = start.elapsed();

    // Should be fast with index
    assert!(duration.as_millis() < 10);

    // Test aggregation query
    let start = std::time::Instant::now();
    let total = pool.get_total_token_supply().await.unwrap();
    let duration = start.elapsed();

    assert_eq!(total, CrapTokens::from_raw(499500)); // sum(0..1000)
    assert!(duration.as_millis() < 50);
}

#[tokio::test]
async fn test_error_recovery() {
    let (pool, temp_dir) = create_test_db().await;
    pool.apply_migrations().await.unwrap();

    // Add test data
    let player_id = PeerId::generate();
    pool.create_player(player_id, CrapTokens::from_raw(1000))
        .await
        .unwrap();

    // Simulate database corruption by deleting file
    drop(pool);
    std::fs::remove_file(temp_dir.path().join("test.db")).unwrap();

    // Try to reconnect
    let result = DatabasePool::new(temp_dir.path().join("test.db").to_str().unwrap()).await;

    // Should handle missing database gracefully
    assert!(result.is_ok());
    let new_pool = result.unwrap();

    // Should reinitialize
    new_pool.apply_migrations().await.unwrap();

    // Data should be gone but operations should work
    let balance_result = new_pool.get_player_balance(player_id).await;
    assert!(balance_result.is_err()); // Player doesn't exist
}

#[tokio::test]
async fn test_database_vacuum() {
    let (pool, _temp_dir) = create_test_db().await;
    pool.apply_migrations().await.unwrap();

    // Add and delete many records
    for i in 0..100 {
        let player_id = PeerId::from_bytes(&i.to_le_bytes());
        pool.create_player(player_id, CrapTokens::from_raw(1000))
            .await
            .unwrap();
    }

    for i in 0..50 {
        let player_id = PeerId::from_bytes(&i.to_le_bytes());
        pool.delete_player(player_id).await.unwrap();
    }

    // Vacuum database
    let freed_pages = pool.vacuum().await.unwrap();
    assert!(freed_pages > 0);

    // Database should still work
    let player_id = PeerId::from_bytes(&75u64.to_le_bytes());
    let balance = pool.get_player_balance(player_id).await.unwrap();
    assert_eq!(balance, CrapTokens::from_raw(1000));
}

#[tokio::test]
async fn test_database_statistics() {
    let (pool, _temp_dir) = create_test_db().await;
    pool.apply_migrations().await.unwrap();

    // Add test data
    for i in 0..10 {
        let player_id = PeerId::from_bytes(&i.to_le_bytes());
        pool.create_player(player_id, CrapTokens::from_raw(100 * i as u64))
            .await
            .unwrap();
    }

    // Get statistics
    let stats = pool.get_statistics().await.unwrap();

    assert_eq!(stats.total_players, 10);
    assert_eq!(stats.total_games, 0);
    assert!(stats.database_size_bytes > 0);
    assert!(stats.cache_hit_rate >= 0.0 && stats.cache_hit_rate <= 1.0);
}
