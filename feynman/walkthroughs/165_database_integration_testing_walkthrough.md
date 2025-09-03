# Chapter 51: Database Integration Testing - Where Code Meets Persistence

Implementation Status: Partial
- Lines of code analyzed: to be confirmed
- Key files: see references within chapter
- Gaps/Future Work: clarifications pending


## A Primer on Database Integration Testing: From Bank Ledgers to ACID Guarantees

In 1494, Luca Pacioli published "Summa de arithmetica," describing double-entry bookkeeping - a system where every transaction affects at least two accounts, and debits always equal credits. This wasn't just accounting; it was the world's first data integrity system. A single error would make the books not balance, immediately revealing the problem. Modern database testing follows the same principle: every operation must maintain invariants, and any violation signals a bug. The challenge isn't just storing data correctly; it's maintaining consistency while multiple users modify data concurrently.

Database integration testing sits at a crucial junction. Unit tests mock database interactions, testing business logic in isolation. System tests use real databases but focus on user workflows. Database integration tests specifically verify that your code correctly interacts with the database - that queries return expected results, transactions maintain consistency, constraints are enforced, and performance meets requirements. They catch the subtle bugs that only appear when code meets persistence.

The challenge begins with test isolation. Each test needs a clean database state. But databases are stateful by nature - data from one test affects others. The naive solution is to delete all data between tests, but this is slow and might miss issues with existing data. Better approaches include transaction rollback (wrap each test in a transaction that never commits), database snapshots (restore a known state before each test), or parallel databases (each test gets its own database). Each approach has tradeoffs between speed, isolation, and realism.

ACID properties make database testing both easier and harder. Atomicity means transactions either complete fully or not at all - test this by forcing failures mid-transaction and verifying rollback. Consistency means data always satisfies constraints - test by attempting invalid operations. Isolation means concurrent transactions don't interfere - test with parallel operations. Durability means committed data survives crashes - test by killing the database and restarting. Each property requires specific testing strategies.

The impedance mismatch between application code and databases creates subtle bugs. Your code might use 64-bit integers, but the database uses 32-bit. Your code assumes UTC timestamps, but the database uses local time. Your code expects null to mean "unknown," but the database treats it as "no value." These mismatches only surface during integration testing when actual data flows between systems.

Connection pooling adds complexity. Databases limit concurrent connections, so applications share a pool. But pooling introduces new failure modes. What if all connections are busy? What if a connection dies mid-transaction? What if connections leak? Integration tests must verify proper pool management - that connections are returned, that the pool recovers from failures, that deadlocks are detected and broken.

Migration testing is often overlooked but crucial. Schema changes are dangerous - a bad migration can corrupt data or break the application. Test migrations both forward and backward. Verify data survives migration. Test migration with existing data, not just empty databases. Test concurrent migration (what if two servers try to migrate simultaneously?). Test failed migrations - does the system recover or remain broken?

Performance testing at the database layer reveals problems invisible in unit tests. An O(n²) algorithm might seem fine with test data but explode with production volumes. Missing indexes cause full table scans. Lock contention serializes parallel operations. Connection exhaustion causes mysterious hangs. Database integration tests with realistic data volumes catch these issues early.

Transaction testing requires special attention. Transactions are all-or-nothing, but testing them is tricky. You need to verify both success (all changes committed) and failure (all changes rolled back) paths. But you also need to test partial failures - what if the network dies after COMMIT but before acknowledgment? What if the database crashes mid-transaction? These edge cases cause real-world data corruption.

Concurrent access testing finds race conditions and deadlocks. Launch multiple threads that modify the same data. Some should succeed, some should retry, some might deadlock. The database should maintain consistency regardless. But timing-dependent tests are notoriously flaky. Use barriers to synchronize thread starts, but accept that some race conditions only appear under load.

Constraint testing verifies data integrity rules. Foreign keys prevent orphaned records. Unique constraints prevent duplicates. Check constraints enforce business rules. But constraints can also cause problems - cascading deletes might remove more than expected, unique constraints might reject valid data due to race conditions. Test both constraint enforcement and violation handling.

The test database dilemma is challenging. Using production databases is dangerous. Using different databases (SQLite for tests, PostgreSQL for production) misses database-specific issues. Using the same database but different versions causes subtle incompatibilities. The best solution is usually containerized databases - spin up a real database in Docker for tests, ensuring consistency between test and production.

Data factory patterns help create realistic test data. Instead of hardcoding test data in each test, create factories that generate valid objects. `PlayerFactory.create()` returns a valid player with randomized attributes. `GameFactory.withPlayers(3)` creates a game with three players. Factories ensure test data remains valid as schemas evolve and make tests more readable by hiding irrelevant details.

Backup and restore testing is critical but often skipped. Backups are your last defense against data loss, but untested backups are worthless. Test that backups complete successfully, that they can be restored, that restored data is complete and consistent. Test incremental backups, point-in-time recovery, and backup corruption detection. These tests take time but prevent catastrophic data loss.

Query optimization testing ensures performance at scale. Explain plans reveal how the database executes queries. Test that queries use indexes, that joins are efficient, that subqueries don't cause n+1 problems. But query plans change with data distribution - a query that's fast with uniform data might be slow with skewed data. Test with realistic data distributions.

Connection leak testing finds resource exhaustion bugs. Open connections without closing them. Eventually, the pool exhausts and the application hangs. But connection leaks might only appear under specific error conditions. Test error paths thoroughly - when queries fail, when transactions abort, when the network disconnects. Ensure connections always return to the pool.

The future of database testing involves property-based testing and chaos engineering. Property-based testing generates random operations and verifies invariants hold. Chaos engineering randomly kills database connections, corrupts network packets, and fills disks. Machine learning can generate test cases that maximize code coverage or find performance regressions. These techniques find bugs that human-written tests miss.

## The BitCraps Database Integration Testing Implementation

Now let's examine how BitCraps implements comprehensive database integration tests that verify data persistence, consistency, and performance at scale.

```rust
//! Comprehensive Database Integration Tests for BitChat-Rust
//! 
//! Tests all database operations including:
//! - Connection pooling
//! - Transaction handling
//! - Data persistence
//! - Migration system
//! - Error recovery
//! - Concurrent access
```

This header reveals comprehensive testing scope. Each aspect - pooling, transactions, migrations - has unique failure modes that only integration tests can catch.

```rust
/// Helper to create a test database
async fn create_test_db() -> (DatabasePool, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let pool = DatabasePool::new(&db_path.to_str().unwrap())
        .await
        .expect("Failed to create test database");
    (pool, temp_dir)
}
```

Test isolation through temporary databases is crucial. Each test gets a fresh database in a temporary directory. The TempDir is returned to keep it alive - when dropped, it cleans up automatically. This ensures tests don't interfere with each other.

```rust
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
```

Migration testing verifies schema evolution works correctly. First application should run migrations. Schema version should update. Crucially, migrations must be idempotent - running them again should do nothing. This prevents corruption if migration runs multiple times.

```rust
#[tokio::test]
async fn test_transaction_handling() {
    let (pool, _temp_dir) = create_test_db().await;
    pool.apply_migrations().await.unwrap();
    
    let player1 = PeerId::generate();
    let player2 = PeerId::generate();
    let amount = CrapTokens::from_raw(500);
    
    // Create players
    pool.create_player(player1, CrapTokens::from_raw(1000)).await.unwrap();
    pool.create_player(player2, CrapTokens::from_raw(1000)).await.unwrap();
    
    // Test successful transaction
    let result = pool.transfer_tokens_transactional(player1, player2, amount).await;
    assert!(result.is_ok());
    
    // Verify balances
    let balance1 = pool.get_player_balance(player1).await.unwrap();
    let balance2 = pool.get_player_balance(player2).await.unwrap();
    assert_eq!(balance1, CrapTokens::from_raw(500));
    assert_eq!(balance2, CrapTokens::from_raw(1500));
```

Transaction testing verifies ACID properties. The transfer must be atomic - either both balances change or neither. This tests the happy path where everything succeeds.

```rust
// Test failed transaction (insufficient funds)
let large_amount = CrapTokens::from_raw(5000);
let result = pool.transfer_tokens_transactional(player1, player2, large_amount).await;
assert!(result.is_err());

// Verify no changes on failure (rollback)
let balance1_after = pool.get_player_balance(player1).await.unwrap();
let balance2_after = pool.get_player_balance(player2).await.unwrap();
assert_eq!(balance1_after, balance1);
assert_eq!(balance2_after, balance2);
```

The failure path is equally important. When a transaction fails (insufficient funds), all changes must roll back. Balances should remain unchanged. This tests atomicity under failure conditions.

Concurrent access testing:

```rust
#[tokio::test]
async fn test_concurrent_access() {
    let (pool, _temp_dir) = create_test_db().await;
    pool.apply_migrations().await.unwrap();
    
    let player_id = PeerId::generate();
    pool.create_player(player_id, CrapTokens::from_raw(0)).await.unwrap();
    
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
                pool_clone.update_player_balance(player_id, new_balance).await.unwrap();
            }
        });
        
        handles.push(handle);
    }
```

This test is sophisticated. Ten tasks increment the same balance concurrently. The barrier ensures all tasks start simultaneously, maximizing contention. Each task increments 100 times, so the final balance should be 1000. This tests isolation - concurrent transactions shouldn't interfere.

Data integrity testing:

```rust
#[tokio::test]
async fn test_data_integrity() {
    let (pool, _temp_dir) = create_test_db().await;
    pool.apply_migrations().await.unwrap();
    
    // Test foreign key constraints
    let invalid_game = GameId::generate();
    let result = pool.record_bet(invalid_game, player_id, CrapTokens::from_raw(100)).await;
    assert!(result.is_err()); // Should fail due to missing game
    
    // Test unique constraints
    let duplicate_result = pool.create_player(player_id, CrapTokens::from_raw(500)).await;
    assert!(duplicate_result.is_err()); // Should fail due to duplicate
}
```

Constraint testing verifies the database enforces integrity rules. Foreign keys prevent orphaned records. Unique constraints prevent duplicates. These constraints catch application bugs that would otherwise corrupt data.

Backup and restore testing:

```rust
#[tokio::test]
async fn test_backup_and_restore() {
    let (pool, temp_dir) = create_test_db().await;
    pool.apply_migrations().await.unwrap();
    
    // Add test data
    let player_id = PeerId::generate();
    pool.create_player(player_id, CrapTokens::from_raw(5000)).await.unwrap();
    
    // Create backup
    let backup_path = temp_dir.path().join("backup.db");
    pool.backup_to(&backup_path).await.unwrap();
    
    // Corrupt original (simulate failure)
    pool.update_player_balance(player_id, CrapTokens::from_raw(0)).await.unwrap();
    
    // Restore from backup
    pool.restore_from(&backup_path).await.unwrap();
    
    // Verify data restored
    let balance = pool.get_player_balance(player_id).await.unwrap();
    assert_eq!(balance, CrapTokens::from_raw(5000));
}
```

Backup testing is critical but often skipped. This test creates data, backs it up, corrupts the original, restores from backup, and verifies data integrity. This ensures backups actually work when needed.

Performance testing:

```rust
#[tokio::test]
async fn test_query_performance() {
    let (pool, _temp_dir) = create_test_db().await;
    pool.apply_migrations().await.unwrap();
    
    // Insert many records
    for i in 0..1000 {
        let player_id = PeerId::from_bytes(&i.to_le_bytes());
        pool.create_player(player_id, CrapTokens::from_raw(i as u64)).await.unwrap();
    }
    
    // Test indexed query performance
    let start = std::time::Instant::now();
    let player_id = PeerId::from_bytes(&500u64.to_le_bytes());
    let _balance = pool.get_player_balance(player_id).await.unwrap();
    let duration = start.elapsed();
    
    // Should be fast with index
    assert!(duration.as_millis() < 10);
```

Performance tests catch missing indexes and inefficient queries. Finding one record among 1000 should be fast with an index. The 10ms threshold catches full table scans that would be catastrophic with production data volumes.

Error recovery testing:

```rust
#[tokio::test]
async fn test_error_recovery() {
    let (pool, temp_dir) = create_test_db().await;
    pool.apply_migrations().await.unwrap();
    
    // Simulate database corruption by deleting file
    drop(pool);
    std::fs::remove_file(temp_dir.path().join("test.db")).unwrap();
    
    // Try to reconnect
    let result = DatabasePool::new(
        temp_dir.path().join("test.db").to_str().unwrap()
    ).await;
    
    // Should handle missing database gracefully
    assert!(result.is_ok());
}
```

Resilience testing simulates failures. Deleting the database file simulates corruption. The system should detect the problem and recover gracefully, perhaps by recreating the database. This tests error handling paths rarely executed in production.

## Key Lessons from Database Integration Testing

This implementation embodies several crucial database testing principles:

1. **Test Isolation**: Each test gets a fresh database to prevent interference.

2. **Transaction Verification**: Test both commit and rollback paths.

3. **Concurrent Access**: Verify isolation under concurrent modifications.

4. **Constraint Enforcement**: Ensure integrity rules are enforced.

5. **Performance Bounds**: Set and verify acceptable query times.

6. **Backup Validation**: Test that backups can actually be restored.

7. **Error Recovery**: Simulate failures and verify graceful handling.

The implementation demonstrates important patterns:

- **Temporary Databases**: Automatic cleanup prevents test pollution
- **Barrier Synchronization**: Ensures true concurrent execution
- **Performance Assertions**: Catch performance regressions early
- **Comprehensive Coverage**: Test success, failure, and edge cases
- **Migration Testing**: Ensure schema evolution works correctly

This database integration testing framework ensures BitCraps can reliably persist game state, maintain consistency under concurrent access, and recover from failures - critical requirements for a distributed gaming platform.
