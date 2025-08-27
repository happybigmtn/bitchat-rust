# Chapter 76: Production Database Integration

*Welcome back to BitCraps! You've mastered distributed consensus and security. Now let's dive deep into production-grade database management - the backbone of persistent state in our distributed casino.*

## The Database Challenge in Distributed Systems

Imagine you're running a real casino. Every bet, every roll, every payout needs to be recorded permanently. In BitCraps, we face the same challenge, but distributed across multiple nodes with no central authority. How do we ensure data consistency, durability, and performance?

Let's explore our production database implementation in `/src/database/mod.rs`:

## Connection Pool Architecture

The heart of our database system is the connection pool. Think of it like a team of cashiers at our casino - we need enough to handle the load, but not so many that we waste resources:

```rust
pub struct DatabasePool {
    connections: Arc<RwLock<Vec<DatabaseConnection>>>,
    config: DatabaseConfig,
    backup_manager: Arc<BackupManager>,
    health_monitor: Arc<HealthMonitor>,
    shutdown: Arc<AtomicBool>,
}
```

Each connection is carefully managed:

```rust
pub struct DatabaseConnection {
    conn: Connection,
    in_use: bool,
    created_at: Instant,
    last_used: Instant,
    transaction_count: u64,
}
```

### Understanding Connection Pooling

Why not just create a new connection for each operation? Let's understand through an analogy:

Imagine hiring a new cashier every time a player wants to place a bet, then firing them immediately after. The hiring process (TCP handshake, authentication, initialization) takes longer than the actual work! Connection pooling keeps our "cashiers" ready and waiting.

## WAL Mode: The Secret to Concurrent Access

Our database uses Write-Ahead Logging (WAL) mode - a critical optimization for concurrent access:

```rust
fn create_connection(config: &DatabaseConfig) -> Result<Connection> {
    let conn = Connection::open(&config.url)?;
    
    // Enable WAL mode for better concurrency
    if config.enable_wal {
        conn.query_row("PRAGMA journal_mode = WAL", [], |_| Ok(()))?;
    }
    
    // Set optimal pragmas
    conn.execute("PRAGMA synchronous = NORMAL", [])?;
    conn.execute("PRAGMA cache_size = -64000", [])?;  // 64MB cache
    conn.execute("PRAGMA temp_store = MEMORY", [])?;
    conn.execute("PRAGMA mmap_size = 268435456", [])?; // 256MB mmap
}
```

### WAL Mode Explained

Traditional databases lock the entire database during writes. WAL mode is like having a notepad where you jot down changes before applying them to the main ledger. Readers can continue reading the main database while writers append to the WAL file.

Benefits:
1. **Concurrent Reads**: Multiple readers don't block each other
2. **Non-blocking Writes**: Writers don't block readers
3. **Crash Recovery**: WAL file acts as a redo log
4. **Better Performance**: Fewer disk syncs required

## Transaction Management

Every critical operation is wrapped in a transaction with automatic rollback:

```rust
pub async fn transaction<F, R>(&self, f: F) -> Result<R>
where
    F: FnOnce(&rusqlite::Transaction) -> Result<R>,
{
    self.with_connection(|conn| {
        let tx = conn.transaction()?;
        
        match f(&tx) {
            Ok(result) => {
                tx.commit()?;
                Ok(result)
            }
            Err(e) => {
                // Transaction automatically rolls back on drop
                Err(e)
            }
        }
    }).await
}
```

### ACID Guarantees

Our transactions provide ACID guarantees:
- **Atomicity**: All or nothing - no partial updates
- **Consistency**: Database remains valid after each transaction
- **Isolation**: Concurrent transactions don't interfere
- **Durability**: Committed data survives crashes

## Connection Management Strategy

The pool intelligently manages connections with several strategies:

```rust
pub async fn with_connection<F, R>(&self, f: F) -> Result<R> {
    loop {
        let mut connections = self.connections.write().await;
        
        // Find an available connection
        for conn in connections.iter_mut() {
            if !conn.in_use {
                // Check if connection is still valid
                if conn.created_at.elapsed() > self.config.idle_timeout {
                    // Recreate stale connection
                    conn.conn = Self::create_connection(&self.config)?;
                    conn.created_at = Instant::now();
                }
                
                conn.in_use = true;
                conn.last_used = Instant::now();
                
                let result = f(&mut conn.conn);
                conn.in_use = false;
                
                return result;
            }
        }
        
        // Try to create a new connection if under limit
        if connections.len() < self.config.max_connections as usize {
            let new_conn = Self::create_connection(&self.config)?;
            connections.push(DatabaseConnection { /* ... */ });
            continue;
        }
        
        // Check timeout
        if start.elapsed() > timeout {
            return Err(Error::Database("Connection pool timeout"));
        }
        
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}
```

### Connection Lifecycle

1. **Acquisition**: Find available connection or create new one
2. **Validation**: Check if connection is still healthy
3. **Recreation**: Replace stale connections transparently
4. **Usage**: Execute operation with exclusive access
5. **Release**: Mark as available for reuse

## Backup Strategy

Production systems need reliable backups. Our backup manager runs automatically:

```rust
impl BackupManager {
    async fn run_backup(&self) -> Result<()> {
        // Generate timestamped backup
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let backup_file = self.backup_dir.join(format!("backup_{}.db", timestamp));
        
        // Perform backup
        std::fs::copy(&db_path, &backup_file)?;
        
        // Clean up old backups
        self.cleanup_old_backups().await?;
        
        Ok(())
    }
    
    async fn cleanup_old_backups(&self) -> Result<()> {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(retention_days);
        
        for entry in std::fs::read_dir(&self.backup_dir)? {
            if modified_time < cutoff {
                std::fs::remove_file(entry.path())?;
            }
        }
    }
}
```

### Backup Best Practices

1. **Regular Intervals**: Hourly backups by default
2. **Retention Policy**: Keep 7 days of backups
3. **Timestamped Names**: Easy to identify and restore
4. **Automatic Cleanup**: Prevent disk space issues
5. **Non-blocking**: Backups don't interrupt operations

## Health Monitoring

The health monitor continuously checks database integrity:

```rust
impl HealthMonitor {
    async fn check_health(&self) -> Result<()> {
        // Check for corruption
        if *self.corruption_detected.read().await {
            return Err(Error::Database("Corruption detected"));
        }
        
        // Connection health verified during acquisition
        Ok(())
    }
    
    pub async fn check_connection(&self, conn: &mut Connection) -> bool {
        conn.execute("SELECT 1", []).is_ok()
    }
}
```

## Schema Migration System

Production databases evolve over time. Our migration system (in `/src/database/migrations.rs`) handles schema changes safely:

```rust
pub struct Migration {
    pub version: u32,
    pub description: String,
    pub up_sql: String,
    pub down_sql: String,
}

impl MigrationManager {
    pub fn run_migrations(&mut self, conn: &mut Connection) -> Result<MigrationReport> {
        // Create migration table if it doesn't exist
        self.ensure_migration_table(conn)?;
        
        // Get current version
        let current = self.get_current_version(conn)?;
        
        // Apply pending migrations
        for migration in &self.migrations {
            if migration.version > current {
                self.apply_migration(conn, migration)?;
            }
        }
    }
}
```

## Repository Pattern

We use the repository pattern to abstract database operations:

```rust
pub struct GameRepository<'a> {
    conn: &'a Connection,
}

impl<'a> GameRepository<'a> {
    pub fn create_game(&self, game: &Game) -> Result<()> {
        self.conn.execute(
            "INSERT INTO games (id, state, created_at) VALUES (?1, ?2, ?3)",
            params![game.id, serde_json::to_string(&game.state)?, game.created_at],
        )?;
        Ok(())
    }
    
    pub fn find_game(&self, id: &GameId) -> Result<Option<Game>> {
        // Efficient query with prepared statement
        let mut stmt = self.conn.prepare_cached(
            "SELECT state, created_at FROM games WHERE id = ?1"
        )?;
        
        stmt.query_row([id], |row| {
            Ok(Game {
                id: *id,
                state: serde_json::from_str(row.get(0)?)?,
                created_at: row.get(1)?,
            })
        }).optional()
    }
}
```

## Performance Optimizations

Our database is tuned for maximum performance:

### 1. Memory-Mapped I/O
```rust
conn.execute("PRAGMA mmap_size = 268435456", [])?; // 256MB
```
The OS maps database pages directly into memory, avoiding system calls.

### 2. Page Cache
```rust
conn.execute("PRAGMA cache_size = -64000", [])?; // 64MB
```
Frequently accessed pages stay in memory.

### 3. Prepared Statement Cache
```rust
let mut stmt = self.conn.prepare_cached("SELECT ...");
```
SQL compilation happens once, execution many times.

### 4. Busy Timeout
```rust
conn.busy_timeout(Duration::from_secs(30))?;
```
Automatic retry on lock conflicts instead of immediate failure.

## Concurrency Control

The database handles concurrent access elegantly:

```rust
// Multiple readers
let stats1 = pool.with_connection(|conn| read_stats(conn)).await?;
let stats2 = pool.with_connection(|conn| read_stats(conn)).await?;

// Serialized writers
pool.transaction(|tx| {
    // This blocks other writers but not readers
    update_game_state(tx)
}).await?;
```

## Error Recovery

Every operation includes comprehensive error handling:

```rust
match pool.transaction(|tx| {
    risky_operation(tx)
}).await {
    Ok(result) => {
        // Success path
        metrics.record_success();
        result
    }
    Err(Error::Database(msg)) if msg.contains("locked") => {
        // Retry on lock conflicts
        retry_with_backoff().await
    }
    Err(Error::Database(msg)) if msg.contains("corrupt") => {
        // Trigger recovery procedure
        initiate_recovery().await
    }
    Err(e) => {
        // Log and propagate other errors
        tracing::error!("Database error: {}", e);
        Err(e)
    }
}
```

## Testing Strategy

Our database tests ensure reliability:

```rust
#[tokio::test]
async fn test_concurrent_access() {
    let pool = create_test_pool().await;
    
    // Spawn multiple concurrent operations
    let handles: Vec<_> = (0..10).map(|i| {
        let pool = pool.clone();
        tokio::spawn(async move {
            pool.transaction(|tx| {
                tx.execute("INSERT INTO test VALUES (?)", [i])?;
                Ok(())
            }).await
        })
    }).collect();
    
    // All should succeed
    for handle in handles {
        assert!(handle.await.unwrap().is_ok());
    }
}
```

## Production Deployment Considerations

When deploying BitCraps:

1. **Storage**: Use SSD for database files
2. **Backup**: Configure offsite backup replication
3. **Monitoring**: Set up alerts for:
   - Connection pool exhaustion
   - Transaction failures
   - Backup failures
   - Disk space usage

4. **Tuning**: Adjust based on workload:
   - Increase cache for read-heavy loads
   - Tune checkpoint interval for write-heavy loads
   - Adjust connection pool size for concurrency

## Exercise: Implement Query Optimization

Create an efficient query system with caching:

```rust
pub struct QueryCache {
    cache: LruCache<String, Vec<u8>>,
}

impl QueryCache {
    pub async fn get_or_compute<F>(&mut self, key: &str, f: F) -> Result<Vec<u8>>
    where F: FnOnce() -> Result<Vec<u8>>
    {
        // TODO: Check cache first
        // TODO: Compute if miss
        // TODO: Store in cache
        // TODO: Handle cache invalidation
    }
}
```

## Key Takeaways

1. **Connection Pooling**: Reuse connections for efficiency
2. **WAL Mode**: Enable concurrent reads and writes
3. **Transactions**: Ensure data consistency
4. **Backups**: Automate for disaster recovery
5. **Monitoring**: Detect issues before they become critical
6. **Migration**: Evolve schema safely
7. **Optimization**: Tune for your specific workload

Our production database layer provides the foundation for a reliable, scalable distributed gaming system. In the next chapter, we'll explore advanced protocol features that build on this solid data foundation.

Remember: A distributed system is only as reliable as its weakest component. Our database ensures that component isn't data persistence!