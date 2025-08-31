//! Comprehensive database tests for migrations, repositories, and caching
//!
//! Tests all aspects of the database layer including:
//! - Migration system functionality
//! - Repository operations and transactions
//! - Cache layer performance and consistency
//! - Query builder type safety
//! - Error handling and edge cases

use std::collections::HashMap;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::time::{sleep, Duration};

use bitcraps::config::DatabaseConfig;
use bitcraps::database::{
    cache::{CacheConfig, DatabaseCache, WriteStrategy},
    models::*,
    query_builder::{ComparisonOperator, GameQueries, QueryBuilder, SortDirection, UserQueries},
    DatabasePool, GameRepository, Migration, MigrationManager, StatsRepository,
    TransactionRepository, UserRepository,
};
use bitcraps::error::Error;

/// Test migration system functionality
#[tokio::test]
async fn test_migration_system() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("migration_test.db");

    // Create database connection
    let conn = rusqlite::Connection::open(&db_path).unwrap();

    // Initialize migration manager
    let mut manager = MigrationManager::new().with_connection(conn);

    // Run migrations
    let report = manager.migrate().unwrap();
    assert!(report.is_success(), "Migrations should succeed");
    assert!(
        !report.successful.is_empty(),
        "Should have applied migrations"
    );

    // Check migration status
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let status = manager
        .with_connection(conn)
        .status(&rusqlite::Connection::open(&db_path).unwrap())
        .unwrap();
    assert!(status.is_up_to_date, "Database should be up to date");
    assert!(!status.applied.is_empty(), "Should have applied migrations");
    assert!(
        status.pending.is_empty(),
        "Should have no pending migrations"
    );
}

/// Test migration validation and rollback
#[tokio::test]
async fn test_migration_validation_and_rollback() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("rollback_test.db");

    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let mut manager = MigrationManager::new().with_connection(conn);

    // Apply all migrations
    let report = manager.migrate().unwrap();
    assert!(report.is_success());
    let final_version = report.final_version;

    // Test rollback
    if final_version > 1 {
        let rollback_report = manager.rollback_to(final_version - 1).unwrap();
        assert!(rollback_report.is_success(), "Rollback should succeed");
        assert_eq!(rollback_report.final_version, final_version - 1);
    }

    // Test validation
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let validation = manager
        .with_connection(conn)
        .validate(&rusqlite::Connection::open(&db_path).unwrap())
        .unwrap();
    assert!(validation.is_valid, "Migration validation should pass");
}

/// Test database pool operations
#[tokio::test]
async fn test_database_pool_operations() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("pool_test.db");

    let config = DatabaseConfig {
        url: db_path.to_str().unwrap().to_string(),
        max_connections: 3,
        connection_timeout: Duration::from_secs(5),
        idle_timeout: Duration::from_secs(300),
        enable_wal: true,
        checkpoint_interval: Duration::from_secs(60),
        backup_dir: temp_dir.path().join("backups"),
        backup_interval: Duration::from_secs(3600),
        log_retention_days: 7,
    };

    let pool = DatabasePool::new(config).await.unwrap();

    // Test basic connection operations
    pool.with_connection(|conn| {
        conn.execute(
            "CREATE TABLE test_table (id INTEGER PRIMARY KEY, name TEXT)",
            [],
        )
        .map_err(|e| Error::Database(e.to_string()))?;
        Ok(())
    })
    .await
    .unwrap();

    // Test transaction
    pool.transaction(|tx| {
        tx.execute("INSERT INTO test_table (name) VALUES (?)", ["test_value"])
            .map_err(|e| Error::Database(e.to_string()))?;
        Ok(())
    })
    .await
    .unwrap();

    // Verify data was inserted
    let count: i64 = pool
        .with_connection(|conn| {
            let count = conn
                .query_row("SELECT COUNT(*) FROM test_table", [], |row| row.get(0))
                .map_err(|e| Error::Database(e.to_string()))?;
            Ok(count)
        })
        .await
        .unwrap();

    assert_eq!(count, 1, "Should have one row in test table");

    // Test pool statistics
    let stats = pool.get_stats().await.unwrap();
    assert!(stats.total_connections > 0);
    assert!(!stats.corrupted);

    // Cleanup
    pool.shutdown().await.unwrap();
}

/// Test user repository operations
#[tokio::test]
async fn test_user_repository() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("user_repo_test.db");

    let config = DatabaseConfig {
        url: db_path.to_str().unwrap().to_string(),
        max_connections: 2,
        connection_timeout: Duration::from_secs(5),
        idle_timeout: Duration::from_secs(300),
        enable_wal: true,
        checkpoint_interval: Duration::from_secs(60),
        backup_dir: temp_dir.path().join("backups"),
        backup_interval: Duration::from_secs(3600),
        log_retention_days: 7,
    };

    let pool = Arc::new(DatabasePool::new(config).await.unwrap());

    // Run initial migration to create tables
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let mut manager = MigrationManager::new().with_connection(conn);
    manager.migrate().unwrap();

    let user_repo = UserRepository::new(pool.clone());

    // Test user creation
    let user_id = "test_user_123";
    let username = "alice";
    let public_key = vec![1u8; 32];

    user_repo
        .create_user(user_id, username, &public_key)
        .await
        .unwrap();

    // Test user retrieval
    let retrieved_user = user_repo.get_user(user_id).await.unwrap();
    assert!(retrieved_user.is_some());
    let user = retrieved_user.unwrap();
    assert_eq!(user.id, user_id);
    assert_eq!(user.username, username);
    assert_eq!(user.public_key, public_key);
    assert_eq!(user.reputation, 0.0);

    // Test reputation update
    user_repo.update_reputation(user_id, 50.0).await.unwrap();
    let updated_user = user_repo.get_user(user_id).await.unwrap().unwrap();
    assert_eq!(updated_user.reputation, 50.0);

    // Test user listing
    let users = user_repo.list_users(10).await.unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].id, user_id);

    pool.shutdown().await.unwrap();
}

/// Test game repository operations
#[tokio::test]
async fn test_game_repository() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("game_repo_test.db");

    let config = DatabaseConfig {
        url: db_path.to_str().unwrap().to_string(),
        max_connections: 2,
        connection_timeout: Duration::from_secs(5),
        idle_timeout: Duration::from_secs(300),
        enable_wal: true,
        checkpoint_interval: Duration::from_secs(60),
        backup_dir: temp_dir.path().join("backups"),
        backup_interval: Duration::from_secs(3600),
        log_retention_days: 7,
    };

    let pool = Arc::new(DatabasePool::new(config).await.unwrap());

    // Run migrations
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let mut manager = MigrationManager::new().with_connection(conn);
    manager.migrate().unwrap();

    let game_repo = GameRepository::new(pool.clone());

    // Create a test game
    let mut game = Game::new("game_123".to_string(), GameType::Craps);
    game.pot_size = 1000;

    // Test game creation
    game_repo.create_game(&game).await.unwrap();

    // Test game retrieval
    let retrieved_game = game_repo.get_game("game_123").await.unwrap();
    assert!(retrieved_game.is_some());
    let retrieved = retrieved_game.unwrap();
    assert_eq!(retrieved.id, "game_123");
    assert_eq!(retrieved.pot_size, 1000);

    // Test game update
    let mut updated_game = retrieved;
    updated_game.pot_size = 2000;
    updated_game.winner_id = Some("winner_456".to_string());
    updated_game.completed_at = Some(chrono::Utc::now().timestamp());

    game_repo.update_game(&updated_game).await.unwrap();

    // Verify update
    let final_game = game_repo.get_game("game_123").await.unwrap().unwrap();
    assert_eq!(final_game.pot_size, 2000);
    assert_eq!(final_game.winner_id, Some("winner_456".to_string()));

    // Test active games listing (should be empty since game is completed)
    let active_games = game_repo.list_active_games(10).await.unwrap();
    assert!(active_games.is_empty());

    pool.shutdown().await.unwrap();
}

/// Test transaction repository operations
#[tokio::test]
async fn test_transaction_repository() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("tx_repo_test.db");

    let config = DatabaseConfig {
        url: db_path.to_str().unwrap().to_string(),
        max_connections: 2,
        connection_timeout: Duration::from_secs(5),
        idle_timeout: Duration::from_secs(300),
        enable_wal: true,
        checkpoint_interval: Duration::from_secs(60),
        backup_dir: temp_dir.path().join("backups"),
        backup_interval: Duration::from_secs(3600),
        log_retention_days: 7,
    };

    let pool = Arc::new(DatabasePool::new(config).await.unwrap());

    // Run migrations
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let mut manager = MigrationManager::new().with_connection(conn);
    manager.migrate().unwrap();

    let tx_repo = TransactionRepository::new(pool.clone());

    // Create test transactions
    let mut tx1 = Transaction::new(
        "tx_001".to_string(),
        Some("user_alice".to_string()),
        Some("user_bob".to_string()),
        100,
        TransactionType::Transfer,
    )
    .unwrap();

    let mut tx2 = Transaction::new(
        "tx_002".to_string(),
        None,
        Some("user_alice".to_string()),
        500,
        TransactionType::Deposit,
    )
    .unwrap();

    // Test transaction creation
    tx_repo.create_transaction(&tx1).await.unwrap();
    tx_repo.create_transaction(&tx2).await.unwrap();

    // Test status update
    tx_repo
        .update_transaction_status("tx_001", "confirmed")
        .await
        .unwrap();
    tx_repo
        .update_transaction_status("tx_002", "confirmed")
        .await
        .unwrap();

    // Test user transaction history
    let alice_txs = tx_repo
        .get_user_transactions("user_alice", 10)
        .await
        .unwrap();
    assert_eq!(alice_txs.len(), 2); // Alice is in both transactions

    let bob_txs = tx_repo.get_user_transactions("user_bob", 10).await.unwrap();
    assert_eq!(bob_txs.len(), 1); // Bob is only in one transaction

    // Test balance calculation
    let alice_balance = tx_repo.get_balance("user_alice").await.unwrap();
    assert_eq!(alice_balance, 400); // +500 deposit -100 sent = 400

    let bob_balance = tx_repo.get_balance("user_bob").await.unwrap();
    assert_eq!(bob_balance, 100); // +100 received = 100

    pool.shutdown().await.unwrap();
}

/// Test statistics repository operations
#[tokio::test]
async fn test_stats_repository() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("stats_repo_test.db");

    let config = DatabaseConfig {
        url: db_path.to_str().unwrap().to_string(),
        max_connections: 2,
        connection_timeout: Duration::from_secs(5),
        idle_timeout: Duration::from_secs(300),
        enable_wal: true,
        checkpoint_interval: Duration::from_secs(60),
        backup_dir: temp_dir.path().join("backups"),
        backup_interval: Duration::from_secs(3600),
        log_retention_days: 7,
    };

    let pool = Arc::new(DatabasePool::new(config).await.unwrap());

    // Run migrations
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let mut manager = MigrationManager::new().with_connection(conn);
    manager.migrate().unwrap();

    let stats_repo = StatsRepository::new(pool.clone());

    // Create test game statistics
    let game_stats = GameStats {
        game_id: "game_123".to_string(),
        total_bets: 50,
        total_wagered: 10000,
        total_won: 9500,
        house_edge: Some(0.05),
        duration_seconds: Some(1200),
        player_count: 4,
        max_pot_size: 2500,
        average_bet_size: Some(200.0),
        volatility_index: Some(0.3),
        fairness_score: 0.95,
        created_at: chrono::Utc::now().timestamp(),
    };

    stats_repo
        .update_game_stats("game_123", &game_stats)
        .await
        .unwrap();

    // Create test player statistics
    let mut player_stats = PlayerStats::new("player_alice".to_string());
    player_stats.update_game_result(true, 100, 150); // Won
    player_stats.update_game_result(false, 200, 0); // Lost
    player_stats.update_game_result(true, 150, 225); // Won

    stats_repo
        .update_player_stats("player_alice", &player_stats)
        .await
        .unwrap();

    // Test statistics retrieval
    let retrieved_stats = stats_repo.get_player_stats("player_alice").await.unwrap();
    assert!(retrieved_stats.is_some());
    let stats = retrieved_stats.unwrap();
    assert_eq!(stats.games_played, 3);
    assert_eq!(stats.games_won, 2);
    assert_eq!(stats.total_wagered, 450);
    assert_eq!(stats.total_won, 375);
    assert!((stats.win_rate - 2.0 / 3.0).abs() < 0.001);

    // Test leaderboard
    let leaderboard = stats_repo.get_leaderboard(10).await.unwrap();
    assert!(!leaderboard.is_empty());

    pool.shutdown().await.unwrap();
}

/// Test cache layer functionality
#[tokio::test]
async fn test_cache_layer() {
    let config = CacheConfig {
        l1_size: 3,
        l2_size: 5,
        ttl_seconds: 1,
        write_strategy: WriteStrategy::WriteThrough,
        enable_compression: true,
        enable_metrics: true,
    };

    let cache = DatabaseCache::new(config);

    // Test basic cache operations
    cache
        .put("key1".to_string(), b"value1".to_vec())
        .await
        .unwrap();
    cache
        .put("key2".to_string(), b"value2".to_vec())
        .await
        .unwrap();

    let value1 = cache.get("key1").await.unwrap();
    assert_eq!(value1, Some(b"value1".to_vec()));

    let value2 = cache.get("key2").await.unwrap();
    assert_eq!(value2, Some(b"value2".to_vec()));

    // Test cache miss
    let missing = cache.get("nonexistent").await.unwrap();
    assert_eq!(missing, None);

    // Test cache eviction
    cache
        .put("key3".to_string(), b"value3".to_vec())
        .await
        .unwrap();
    cache
        .put("key4".to_string(), b"value4".to_vec())
        .await
        .unwrap();
    cache
        .put("key5".to_string(), b"value5".to_vec())
        .await
        .unwrap();

    let stats = cache.stats().await;
    assert!(stats.evictions > 0);

    // Test expiration
    sleep(Duration::from_secs(2)).await;
    let expired = cache.get("key1").await.unwrap();
    assert_eq!(expired, None);

    // Test cleanup
    let cleaned = cache.cleanup_expired().await.unwrap();
    assert!(cleaned > 0);

    // Test metrics
    let metrics = cache.metrics().await;
    assert!(metrics.total_requests > 0);
    assert!(metrics.hit_rate >= 0.0 && metrics.hit_rate <= 1.0);
}

/// Test query builder functionality
#[tokio::test]
fn test_query_builder() {
    // Test SELECT query
    let query = QueryBuilder::select()
        .columns(&["id", "username", "reputation"])
        .from("users")
        .where_eq("active", true)
        .where_op("reputation", ComparisonOperator::GreaterThan, 50.0)
        .order_by_desc("reputation")
        .limit(10)
        .build()
        .unwrap();

    assert!(query.sql.contains("SELECT id, username, reputation"));
    assert!(query.sql.contains("FROM users"));
    assert!(query.sql.contains("WHERE active = ?"));
    assert!(query.sql.contains("AND reputation > ?"));
    assert!(query.sql.contains("ORDER BY reputation DESC"));
    assert!(query.sql.contains("LIMIT 10"));
    assert_eq!(query.parameters.len(), 2);

    // Test INSERT query
    let mut values = HashMap::new();
    values.insert("id", "user123".into());
    values.insert("username", "alice".into());
    values.insert("reputation", 75.5.into());

    let query = QueryBuilder::insert()
        .table("users")
        .values(values)
        .build()
        .unwrap();

    assert!(query.sql.contains("INSERT INTO users"));
    assert!(query.sql.contains("VALUES"));
    assert_eq!(query.parameters.len(), 3);

    // Test UPDATE query
    let query = QueryBuilder::update()
        .table("users")
        .set("reputation", 80.0)
        .set("updated_at", 1234567890i64)
        .where_eq("id", "user123")
        .build()
        .unwrap();

    assert!(query.sql.contains("UPDATE users SET"));
    assert!(query.sql.contains("reputation = ?"));
    assert!(query.sql.contains("WHERE id = ?"));
    assert_eq!(query.parameters.len(), 3);

    // Test specialized queries
    let user_query = UserQueries::find_by_id("user123").expect("Valid query");
    assert!(user_query.sql.contains("SELECT * FROM users WHERE id = ?"));
    assert_eq!(user_query.parameters.len(), 1);

    let reputation_query = UserQueries::find_by_reputation_range(10.0, 90.0).expect("Valid query");
    assert!(user_query.sql.contains("reputation"));

    let search_query = UserQueries::search_by_username("alice").expect("Valid query");
    assert!(search_query.sql.contains("LIKE"));
    assert!(search_query.sql.contains("is_active"));
}

/// Test database models validation
#[tokio::test]
fn test_database_models() {
    // Test User model
    let user = User::new("user123".to_string(), "alice".to_string(), vec![1u8; 32]).unwrap();

    assert_eq!(user.id, "user123");
    assert_eq!(user.username, "alice");
    assert_eq!(user.reputation, 0.0);
    assert!(user.is_active);
    assert!(user.is_good_standing());

    // Test invalid user creation
    let invalid_user = User::new(
        "user456".to_string(),
        "".to_string(), // Empty username
        vec![1u8; 32],
    );
    assert!(invalid_user.is_err());

    // Test Game model
    let mut game = Game::new("game123".to_string(), GameType::Craps);
    assert_eq!(game.state, GameState::Waiting);
    assert_eq!(game.phase, GamePhase::Betting);

    // Test adding players
    game.add_player("player1".to_string()).unwrap();
    game.add_player("player2".to_string()).unwrap();
    assert_eq!(game.players.len(), 2);

    // Test starting game
    game.start().unwrap();
    assert_eq!(game.state, GameState::Playing);

    // Test completing game
    game.complete(Some("player1".to_string())).unwrap();
    assert_eq!(game.state, GameState::Completed);
    assert_eq!(game.winner_id, Some("player1".to_string()));

    // Test Bet model
    let bet = Bet::new(
        vec![1, 2, 3, 4],
        "game123".to_string(),
        "player1".to_string(),
        BetType::PassLine,
        100,
    )
    .unwrap();

    assert_eq!(bet.amount, 100);
    assert_eq!(bet.odds_multiplier, 1.0);
    assert!(bet.outcome.is_none());

    // Test Transaction model
    let mut transaction = Transaction::new(
        "tx123".to_string(),
        Some("alice".to_string()),
        Some("bob".to_string()),
        500,
        TransactionType::Transfer,
    )
    .unwrap();

    assert_eq!(transaction.status, TransactionStatus::Pending);

    transaction
        .confirm(Some(12345), Some("hash123".to_string()))
        .unwrap();
    assert_eq!(transaction.status, TransactionStatus::Confirmed);
    assert_eq!(transaction.block_height, Some(12345));

    // Test PlayerStats model
    let mut stats = PlayerStats::new("player1".to_string());
    stats.update_game_result(true, 100, 150); // Win
    stats.update_game_result(false, 200, 0); // Loss
    stats.update_game_result(true, 150, 200); // Win

    assert_eq!(stats.games_played, 3);
    assert_eq!(stats.games_won, 2);
    assert_eq!(stats.games_lost, 1);
    assert!((stats.win_rate - 2.0 / 3.0).abs() < 0.001);
    assert_eq!(stats.current_streak, 1);
    assert_eq!(stats.current_streak_type, Some(StreakType::Win));
}

/// Test error handling and edge cases
#[tokio::test]
async fn test_error_handling() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("error_test.db");

    // Test invalid database path
    let invalid_config = DatabaseConfig {
        url: "/invalid/path/database.db".to_string(),
        max_connections: 2,
        connection_timeout: Duration::from_secs(5),
        idle_timeout: Duration::from_secs(300),
        enable_wal: true,
        checkpoint_interval: Duration::from_secs(60),
        backup_dir: temp_dir.path().join("backups"),
        backup_interval: Duration::from_secs(3600),
        log_retention_days: 7,
    };

    let result = DatabasePool::new(invalid_config).await;
    assert!(result.is_err());

    // Test cache errors
    let cache = DatabaseCache::new(CacheConfig::default());

    // Test removing non-existent key
    let removed = cache.remove("nonexistent").await.unwrap();
    assert!(!removed);

    // Test query builder errors
    let result = QueryBuilder::insert().build(); // Missing table name
    assert!(result.is_err());

    let result = QueryBuilder::update().build(); // Missing table name
    assert!(result.is_err());

    let result = QueryBuilder::delete().build(); // Missing table name
    assert!(result.is_err());
}
