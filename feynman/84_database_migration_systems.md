# Chapter 84: Database Migration Systems - Evolution Without Revolution

## A Primer on Schema Evolution: From ALTER TABLE to Zero-Downtime Deployments

In 1970, Edgar Codd published "A Relational Model of Data for Large Shared Data Banks," introducing the concept of schema - a formal description of database structure. But Codd's model had a fatal flaw: it assumed schemas were immutable. Once you defined your tables, that was it. Reality, as always, proved messier. Requirements change, understanding deepens, mistakes need correction. The schema that seemed perfect on Monday looks naive by Friday.

The first database migration was probably an ALTER TABLE command typed directly into a production database at 2 AM by a sleep-deprived developer. We've all been there - SSH into production, cross your fingers, run the SQL. What could go wrong? Everything, as it turns out. One typo could destroy millions of records. One missing WHERE clause could update every row. One locked table could bring down the entire application.

Database migrations are about managing change safely. They're version control for your database schema, allowing you to evolve your data model while preserving data integrity. Think of them like Git for databases - track changes, review history, rollback mistakes. Without migrations, database changes are like editing code without version control: possible, but terrifying.

The challenge is that databases are stateful. When you deploy new code, you can simply replace the old version. But you can't replace a database - it contains irreplaceable business data. You must transform it in place, like performing surgery on a patient who must stay conscious. One wrong cut, and you've lost customer orders, user accounts, financial records.

Modern applications make this even harder. In the age of continuous deployment, you might deploy code dozens of times per day. But database migrations can take hours on large tables. You can't take your application offline for six hours while you add an index to a billion-row table. Users expect 24/7 availability. This creates the central tension of database migrations: the need for change versus the demand for stability.

The evolution of migration tools mirrors the evolution of deployment practices. Early tools like Rails migrations (2005) assumed single-server deployments with maintenance windows. Modern tools like gh-ost (GitHub's online schema migration tool) can alter massive tables without blocking writes. The progression from "take the site down for maintenance" to "users won't even notice" represents one of operations engineering's greatest achievements.

Consider what happens during a migration in a distributed system like BitCraps. Nodes might be running different code versions during deployment. Some nodes expect the old schema, others the new. The database must serve both during the transition. This requires careful orchestration - migrations must be backward compatible, applied atomically, and reversible if things go wrong.

## The Anatomy of a Migration System

A migration system consists of several key components working in concert:

```rust
use std::collections::BTreeMap;
use chrono::{DateTime, Utc};
use sha2::{Sha256, Digest};

/// A single migration representing a schema change
#[derive(Debug, Clone)]
pub struct Migration {
    /// Unique identifier (usually timestamp-based)
    pub id: String,
    
    /// Human-readable description
    pub description: String,
    
    /// SQL to apply the migration
    pub up: String,
    
    /// SQL to reverse the migration
    pub down: String,
    
    /// SHA-256 hash of the migration content
    pub checksum: String,
    
    /// Author information
    pub author: String,
    
    /// Safety level of this migration
    pub safety: MigrationSafety,
    
    /// Execution strategy
    pub strategy: ExecutionStrategy,
}

#[derive(Debug, Clone)]
pub enum MigrationSafety {
    /// Safe to run without downtime
    Safe,
    
    /// Requires careful coordination but possible without downtime
    SafeWithCare {
        warnings: Vec<String>,
    },
    
    /// Requires downtime or maintenance window
    RequiresDowntime {
        estimated_duration: Duration,
        reason: String,
    },
    
    /// Destructive operation that loses data
    Destructive {
        data_loss_description: String,
        requires_confirmation: bool,
    },
}

#[derive(Debug, Clone)]
pub enum ExecutionStrategy {
    /// Run as a single transaction
    Transactional,
    
    /// Run without transaction (for DDL that can't be transactional)
    NonTransactional,
    
    /// Run in batches to avoid long locks
    Batched {
        batch_size: usize,
        sleep_between: Duration,
    },
    
    /// Online migration that doesn't block reads/writes
    Online {
        method: OnlineMethod,
    },
}

#[derive(Debug, Clone)]
pub enum OnlineMethod {
    /// Create shadow table, copy data, swap
    ShadowTable,
    
    /// Use triggers to keep tables in sync
    TriggerBased,
    
    /// Database-specific online DDL
    NativeOnline,
    
    /// Custom strategy
    Custom(String),
}
```

## The Migration Engine

The migration engine orchestrates the execution of migrations:

```rust
pub struct MigrationEngine {
    /// Database connection
    connection: Arc<DatabaseConnection>,
    
    /// Migration history table
    history_table: String,
    
    /// Available migrations
    migrations: BTreeMap<String, Migration>,
    
    /// Configuration
    config: MigrationConfig,
    
    /// Hooks for monitoring
    hooks: Vec<Box<dyn MigrationHook>>,
}

pub struct MigrationConfig {
    /// Allow destructive migrations
    allow_destructive: bool,
    
    /// Automatically rollback on failure
    auto_rollback: bool,
    
    /// Maximum time for a single migration
    timeout: Duration,
    
    /// Lock timeout for DDL operations
    lock_timeout: Duration,
    
    /// Statement timeout for long queries
    statement_timeout: Duration,
    
    /// Dry run mode
    dry_run: bool,
}

impl MigrationEngine {
    /// Initialize the migration system
    pub async fn initialize(&self) -> Result<()> {
        // Create migration history table if it doesn't exist
        let create_table = format!(
            "CREATE TABLE IF NOT EXISTS {} (
                id VARCHAR(255) PRIMARY KEY,
                description TEXT NOT NULL,
                checksum VARCHAR(64) NOT NULL,
                executed_at TIMESTAMP NOT NULL,
                execution_time_ms BIGINT NOT NULL,
                applied_by VARCHAR(255) NOT NULL,
                success BOOLEAN NOT NULL,
                error_message TEXT,
                rolled_back BOOLEAN DEFAULT FALSE,
                rolled_back_at TIMESTAMP
            )",
            self.history_table
        );
        
        self.connection.execute(&create_table).await?;
        
        // Create index for faster history queries
        let create_index = format!(
            "CREATE INDEX IF NOT EXISTS idx_{}_executed_at 
             ON {} (executed_at DESC)",
            self.history_table, self.history_table
        );
        
        self.connection.execute(&create_index).await?;
        
        Ok(())
    }
    
    /// Get list of pending migrations
    pub async fn pending_migrations(&self) -> Result<Vec<&Migration>> {
        let applied = self.get_applied_migrations().await?;
        
        let mut pending = Vec::new();
        for (id, migration) in &self.migrations {
            if !applied.contains(id) {
                // Verify migration dependencies
                if self.check_dependencies(migration).await? {
                    pending.push(migration);
                }
            }
        }
        
        Ok(pending)
    }
    
    /// Apply all pending migrations
    pub async fn migrate(&mut self) -> Result<MigrationReport> {
        let pending = self.pending_migrations().await?;
        
        if pending.is_empty() {
            return Ok(MigrationReport {
                migrations_run: 0,
                success: true,
                details: vec![],
            });
        }
        
        info!("Found {} pending migrations", pending.len());
        
        let mut report = MigrationReport::default();
        
        for migration in pending {
            let result = self.apply_single_migration(migration).await;
            
            match result {
                Ok(details) => {
                    report.migrations_run += 1;
                    report.details.push(details);
                }
                Err(e) => {
                    error!("Migration {} failed: {}", migration.id, e);
                    report.success = false;
                    
                    if self.config.auto_rollback {
                        warn!("Attempting automatic rollback");
                        self.rollback_to(migration.id).await?;
                    }
                    
                    return Err(e);
                }
            }
        }
        
        Ok(report)
    }
    
    /// Apply a single migration
    async fn apply_single_migration(&mut self, migration: &Migration) -> Result<MigrationDetails> {
        let start = Instant::now();
        
        // Validate migration checksum
        self.validate_checksum(migration)?;
        
        // Check safety level
        self.check_safety(migration).await?;
        
        // Acquire migration lock
        let _lock = self.acquire_lock().await?;
        
        // Call pre-migration hooks
        for hook in &self.hooks {
            hook.before_migration(migration).await?;
        }
        
        // Execute based on strategy
        let result = match &migration.strategy {
            ExecutionStrategy::Transactional => {
                self.apply_transactional(migration).await
            }
            ExecutionStrategy::NonTransactional => {
                self.apply_non_transactional(migration).await
            }
            ExecutionStrategy::Batched { batch_size, sleep_between } => {
                self.apply_batched(migration, *batch_size, *sleep_between).await
            }
            ExecutionStrategy::Online { method } => {
                self.apply_online(migration, method).await
            }
        };
        
        let execution_time = start.elapsed();
        
        // Record in history
        self.record_migration(migration, result.is_ok(), execution_time).await?;
        
        // Call post-migration hooks
        for hook in &self.hooks {
            hook.after_migration(migration, &result).await?;
        }
        
        result
    }
}
```

## Advanced Migration Strategies

### 1. Zero-Downtime Migrations

For 24/7 applications, migrations must not interrupt service:

```rust
pub struct ZeroDowntimeMigration {
    engine: MigrationEngine,
    strategy: ZeroDowntimeStrategy,
}

pub enum ZeroDowntimeStrategy {
    /// Expand and Contract pattern
    ExpandContract {
        expansion_phase: Vec<Migration>,
        migration_phase: Vec<Migration>,
        contraction_phase: Vec<Migration>,
    },
    
    /// Blue-Green deployment with schema versioning
    BlueGreen {
        blue_schema: String,
        green_schema: String,
        switch_strategy: SwitchStrategy,
    },
    
    /// Dual writes to old and new schema
    DualWrite {
        transition_period: Duration,
        verification_queries: Vec<String>,
    },
}

impl ZeroDowntimeMigration {
    /// Add a non-nullable column without downtime
    pub async fn add_non_nullable_column(
        &mut self,
        table: &str,
        column: &str,
        column_type: &str,
        default_value: &str,
    ) -> Result<()> {
        // Phase 1: Add nullable column
        let phase1 = Migration {
            id: format!("add_nullable_{}_to_{}", column, table),
            description: format!("Add nullable {} column", column),
            up: format!("ALTER TABLE {} ADD COLUMN {} {} NULL", table, column, column_type),
            down: format!("ALTER TABLE {} DROP COLUMN {}", table, column),
            safety: MigrationSafety::Safe,
            strategy: ExecutionStrategy::NonTransactional,
            ..Default::default()
        };
        
        self.engine.apply_single_migration(&phase1).await?;
        
        // Phase 2: Backfill in batches
        let phase2 = Migration {
            id: format!("backfill_{}_in_{}", column, table),
            description: format!("Backfill {} with default value", column),
            up: format!(
                "UPDATE {} SET {} = {} WHERE {} IS NULL",
                table, column, default_value, column
            ),
            down: format!("UPDATE {} SET {} = NULL", table, column),
            safety: MigrationSafety::SafeWithCare {
                warnings: vec!["May take time on large tables".to_string()],
            },
            strategy: ExecutionStrategy::Batched {
                batch_size: 1000,
                sleep_between: Duration::from_millis(100),
            },
            ..Default::default()
        };
        
        self.engine.apply_single_migration(&phase2).await?;
        
        // Phase 3: Add NOT NULL constraint
        let phase3 = Migration {
            id: format!("add_not_null_constraint_{}_to_{}", column, table),
            description: format!("Add NOT NULL constraint to {}", column),
            up: format!("ALTER TABLE {} ALTER COLUMN {} SET NOT NULL", table, column),
            down: format!("ALTER TABLE {} ALTER COLUMN {} DROP NOT NULL", table, column),
            safety: MigrationSafety::Safe,
            strategy: ExecutionStrategy::NonTransactional,
            ..Default::default()
        };
        
        self.engine.apply_single_migration(&phase3).await?;
        
        Ok(())
    }
    
    /// Rename a column without downtime using dual writes
    pub async fn rename_column_dual_write(
        &mut self,
        table: &str,
        old_name: &str,
        new_name: &str,
    ) -> Result<()> {
        // Phase 1: Add new column
        let add_column = format!(
            "ALTER TABLE {} ADD COLUMN {} 
             (SELECT column_type FROM information_schema.columns 
              WHERE table_name = '{}' AND column_name = '{}')",
            table, new_name, table, old_name
        );
        
        self.engine.connection.execute(&add_column).await?;
        
        // Phase 2: Create trigger for dual writes
        let create_trigger = format!(
            "CREATE TRIGGER migrate_{}_to_{}
             BEFORE INSERT OR UPDATE ON {}
             FOR EACH ROW
             BEGIN
                 NEW.{} = COALESCE(NEW.{}, NEW.{});
                 NEW.{} = NEW.{};
             END",
            old_name, new_name, table,
            new_name, new_name, old_name,
            old_name, new_name
        );
        
        self.engine.connection.execute(&create_trigger).await?;
        
        // Phase 3: Backfill existing data
        let backfill = format!(
            "UPDATE {} SET {} = {} WHERE {} IS NULL",
            table, new_name, old_name, new_name
        );
        
        self.engine.connection.execute(&backfill).await?;
        
        // Phase 4: Switch application to use new column
        // (This happens in application deployment)
        
        // Phase 5: Drop trigger and old column (after deployment is complete)
        let cleanup = format!(
            "DROP TRIGGER migrate_{}_to_{};
             ALTER TABLE {} DROP COLUMN {}",
            old_name, new_name, table, old_name
        );
        
        self.engine.connection.execute(&cleanup).await?;
        
        Ok(())
    }
}
```

### 2. Online Index Building

Creating indexes on large tables can lock them for hours:

```rust
pub struct OnlineIndexBuilder {
    connection: Arc<DatabaseConnection>,
    config: IndexConfig,
}

pub struct IndexConfig {
    /// Maximum time to hold locks
    max_lock_time: Duration,
    
    /// Number of rows to process at once
    batch_size: usize,
    
    /// Sleep between batches
    batch_delay: Duration,
    
    /// Use CREATE INDEX CONCURRENTLY if available
    use_concurrent: bool,
}

impl OnlineIndexBuilder {
    /// Build index without blocking writes
    pub async fn create_index_online(
        &self,
        table: &str,
        index_name: &str,
        columns: &[&str],
        unique: bool,
    ) -> Result<()> {
        if self.config.use_concurrent && self.supports_concurrent_index().await? {
            // PostgreSQL supports CREATE INDEX CONCURRENTLY
            self.create_index_concurrently(table, index_name, columns, unique).await
        } else {
            // Manual online index building
            self.create_index_manually(table, index_name, columns, unique).await
        }
    }
    
    /// PostgreSQL's CREATE INDEX CONCURRENTLY
    async fn create_index_concurrently(
        &self,
        table: &str,
        index_name: &str,
        columns: &[&str],
        unique: bool,
    ) -> Result<()> {
        let unique_clause = if unique { "UNIQUE" } else { "" };
        let columns_list = columns.join(", ");
        
        let sql = format!(
            "CREATE {} INDEX CONCURRENTLY {} ON {} ({})",
            unique_clause, index_name, table, columns_list
        );
        
        // Note: CREATE INDEX CONCURRENTLY cannot run in a transaction
        self.connection.execute_non_transactional(&sql).await?;
        
        // Validate index is valid (concurrent builds can fail silently)
        let check_valid = format!(
            "SELECT indisvalid FROM pg_index 
             WHERE indexrelid = '{}'::regclass",
            index_name
        );
        
        let is_valid: bool = self.connection.query_one(&check_valid).await?;
        
        if !is_valid {
            // Index creation failed, drop and retry
            warn!("Index {} creation failed, dropping and retrying", index_name);
            self.connection.execute(&format!("DROP INDEX CONCURRENTLY IF EXISTS {}", index_name)).await?;
            
            // Retry with exponential backoff
            self.create_index_concurrently(table, index_name, columns, unique).await?;
        }
        
        Ok(())
    }
    
    /// Manual online index building for databases without CONCURRENTLY
    async fn create_index_manually(
        &self,
        table: &str,
        index_name: &str,
        columns: &[&str],
        unique: bool,
    ) -> Result<()> {
        // Create shadow table with index
        let shadow_table = format!("{}_shadow", table);
        let create_shadow = format!(
            "CREATE TABLE {} (LIKE {} INCLUDING ALL)",
            shadow_table, table
        );
        self.connection.execute(&create_shadow).await?;
        
        // Create index on shadow table (empty, so fast)
        let create_index = format!(
            "CREATE {} INDEX {} ON {} ({})",
            if unique { "UNIQUE" } else { "" },
            index_name,
            shadow_table,
            columns.join(", ")
        );
        self.connection.execute(&create_index).await?;
        
        // Copy data in batches
        let mut offset = 0;
        loop {
            let copy_batch = format!(
                "INSERT INTO {} 
                 SELECT * FROM {} 
                 ORDER BY ctid 
                 LIMIT {} OFFSET {}
                 ON CONFLICT DO NOTHING",
                shadow_table, table, self.config.batch_size, offset
            );
            
            let rows_copied = self.connection.execute(&copy_batch).await?;
            
            if rows_copied == 0 {
                break;
            }
            
            offset += self.config.batch_size;
            
            // Sleep to reduce load
            tokio::time::sleep(self.config.batch_delay).await;
        }
        
        // Set up triggers to keep tables in sync
        let sync_trigger = format!(
            "CREATE TRIGGER sync_{}_to_shadow
             AFTER INSERT OR UPDATE OR DELETE ON {}
             FOR EACH ROW EXECUTE FUNCTION sync_to_shadow()",
            table, table
        );
        self.connection.execute(&sync_trigger).await?;
        
        // Final sync
        let final_sync = format!(
            "INSERT INTO {} 
             SELECT * FROM {} 
             WHERE ctid > (SELECT MAX(ctid) FROM {})
             ON CONFLICT DO NOTHING",
            shadow_table, table, shadow_table
        );
        self.connection.execute(&final_sync).await?;
        
        // Atomic swap
        let swap = format!(
            "BEGIN;
             ALTER TABLE {} RENAME TO {}_old;
             ALTER TABLE {} RENAME TO {};
             DROP TABLE {}_old CASCADE;
             COMMIT;",
            table, table, shadow_table, table, table
        );
        self.connection.execute(&swap).await?;
        
        Ok(())
    }
}
```

### 3. Data Migration Patterns

Migrations often need to transform existing data:

```rust
pub struct DataMigration {
    connection: Arc<DatabaseConnection>,
    config: DataMigrationConfig,
}

pub struct DataMigrationConfig {
    /// Process in parallel
    parallelism: usize,
    
    /// Batch size for updates
    batch_size: usize,
    
    /// Progress reporting
    progress_callback: Option<Box<dyn Fn(f64) + Send + Sync>>,
    
    /// Validation after migration
    validate: bool,
}

impl DataMigration {
    /// Migrate data from old schema to new schema
    pub async fn migrate_data<F>(
        &self,
        source_query: &str,
        transform: F,
        target_table: &str,
    ) -> Result<MigrationStats>
    where
        F: Fn(Row) -> Result<TransformedRow> + Send + Sync + Clone + 'static,
    {
        let total_rows = self.count_rows(source_query).await?;
        let mut stats = MigrationStats::default();
        
        // Create channels for parallel processing
        let (tx, rx) = mpsc::channel(self.config.parallelism * 2);
        let (result_tx, result_rx) = mpsc::channel(self.config.parallelism * 2);
        
        // Spawn reader task
        let connection = self.connection.clone();
        let source_query = source_query.to_string();
        let batch_size = self.config.batch_size;
        
        tokio::spawn(async move {
            let mut offset = 0;
            loop {
                let query = format!("{} LIMIT {} OFFSET {}", source_query, batch_size, offset);
                let rows = connection.query(&query).await?;
                
                if rows.is_empty() {
                    break;
                }
                
                for row in rows {
                    tx.send(row).await?;
                }
                
                offset += batch_size;
            }
            Ok::<(), Error>(())
        });
        
        // Spawn transformer tasks
        for _ in 0..self.config.parallelism {
            let rx = rx.clone();
            let result_tx = result_tx.clone();
            let transform = transform.clone();
            
            tokio::spawn(async move {
                while let Some(row) = rx.recv().await {
                    match transform(row) {
                        Ok(transformed) => {
                            result_tx.send(Ok(transformed)).await?;
                        }
                        Err(e) => {
                            result_tx.send(Err(e)).await?;
                        }
                    }
                }
                Ok::<(), Error>(())
            });
        }
        
        // Writer task
        let connection = self.connection.clone();
        let target_table = target_table.to_string();
        let progress_callback = self.config.progress_callback.clone();
        
        tokio::spawn(async move {
            let mut batch = Vec::new();
            let mut processed = 0;
            
            while let Some(result) = result_rx.recv().await {
                match result {
                    Ok(transformed) => {
                        batch.push(transformed);
                        stats.successful_rows += 1;
                    }
                    Err(e) => {
                        stats.failed_rows += 1;
                        stats.errors.push(e.to_string());
                    }
                }
                
                if batch.len() >= batch_size {
                    self.write_batch(&target_table, &batch).await?;
                    batch.clear();
                    
                    processed += batch_size;
                    if let Some(ref callback) = progress_callback {
                        callback(processed as f64 / total_rows as f64);
                    }
                }
            }
            
            // Write remaining batch
            if !batch.is_empty() {
                self.write_batch(&target_table, &batch).await?;
            }
            
            Ok::<(), Error>(())
        }).await??;
        
        // Validation phase
        if self.config.validate {
            stats.validation_result = self.validate_migration(&target_table, total_rows).await?;
        }
        
        Ok(stats)
    }
    
    /// Split a single column into multiple columns
    pub async fn split_column(
        &self,
        table: &str,
        source_column: &str,
        target_columns: &[(&str, Box<dyn Fn(&str) -> String>)],
    ) -> Result<()> {
        // Add new columns
        for (column_name, _) in target_columns {
            let add_column = format!(
                "ALTER TABLE {} ADD COLUMN IF NOT EXISTS {} TEXT",
                table, column_name
            );
            self.connection.execute(&add_column).await?;
        }
        
        // Update in batches
        let mut offset = 0;
        loop {
            // Start transaction for batch
            self.connection.execute("BEGIN").await?;
            
            // Select batch
            let select = format!(
                "SELECT id, {} FROM {} 
                 LIMIT {} OFFSET {} 
                 FOR UPDATE",
                source_column, table, self.config.batch_size, offset
            );
            
            let rows = self.connection.query(&select).await?;
            
            if rows.is_empty() {
                self.connection.execute("COMMIT").await?;
                break;
            }
            
            // Transform and update
            for row in rows {
                let id: i64 = row.get("id");
                let source_value: String = row.get(source_column);
                
                let mut update_parts = vec![];
                for (column_name, transform_fn) in target_columns {
                    let new_value = transform_fn(&source_value);
                    update_parts.push(format!("{} = '{}'", column_name, new_value));
                }
                
                let update = format!(
                    "UPDATE {} SET {} WHERE id = {}",
                    table,
                    update_parts.join(", "),
                    id
                );
                
                self.connection.execute(&update).await?;
            }
            
            self.connection.execute("COMMIT").await?;
            
            offset += self.config.batch_size;
            
            // Progress callback
            if let Some(ref callback) = self.config.progress_callback {
                let progress = offset as f64 / self.count_rows(&format!("SELECT COUNT(*) FROM {}", table)).await? as f64;
                callback(progress.min(1.0));
            }
        }
        
        Ok(())
    }
}
```

## BitCraps Migration Implementation

Let's examine how BitCraps handles database migrations:

```rust
// From src/database/migrations.rs

pub struct BitCrapsMigrations {
    engine: MigrationEngine,
    validator: MigrationValidator,
}

impl BitCrapsMigrations {
    /// Load all migrations from the migrations directory
    pub fn load_migrations() -> Result<Vec<Migration>> {
        let mut migrations = Vec::new();
        
        // Initial schema
        migrations.push(Migration {
            id: "20240101_000000_initial_schema".to_string(),
            description: "Create initial BitCraps schema".to_string(),
            up: include_str!("../../migrations/20240101_000000_initial_schema.up.sql").to_string(),
            down: include_str!("../../migrations/20240101_000000_initial_schema.down.sql").to_string(),
            checksum: Self::calculate_checksum(include_str!("../../migrations/20240101_000000_initial_schema.up.sql")),
            author: "BitCraps Team".to_string(),
            safety: MigrationSafety::Safe,
            strategy: ExecutionStrategy::Transactional,
        });
        
        // Add game statistics table
        migrations.push(Migration {
            id: "20240115_000000_add_game_statistics".to_string(),
            description: "Add table for game statistics and analytics".to_string(),
            up: r#"
                CREATE TABLE game_statistics (
                    id SERIAL PRIMARY KEY,
                    game_id UUID NOT NULL REFERENCES games(id),
                    player_id UUID NOT NULL REFERENCES players(id),
                    metric_name VARCHAR(100) NOT NULL,
                    metric_value JSONB NOT NULL,
                    recorded_at TIMESTAMP NOT NULL DEFAULT NOW(),
                    
                    INDEX idx_game_stats_game (game_id),
                    INDEX idx_game_stats_player (player_id),
                    INDEX idx_game_stats_time (recorded_at)
                );
            "#.to_string(),
            down: "DROP TABLE game_statistics;".to_string(),
            safety: MigrationSafety::Safe,
            strategy: ExecutionStrategy::Transactional,
            ..Default::default()
        });
        
        // Add consensus metrics
        migrations.push(Migration {
            id: "20240201_000000_consensus_metrics".to_string(),
            description: "Add consensus performance metrics".to_string(),
            up: r#"
                -- Add metrics columns to consensus_rounds table
                ALTER TABLE consensus_rounds 
                ADD COLUMN IF NOT EXISTS round_duration_ms INTEGER,
                ADD COLUMN IF NOT EXISTS vote_count INTEGER,
                ADD COLUMN IF NOT EXISTS byzantine_nodes TEXT[],
                ADD COLUMN IF NOT EXISTS network_partition BOOLEAN DEFAULT FALSE;
                
                -- Create index for performance queries
                CREATE INDEX CONCURRENTLY IF NOT EXISTS 
                idx_consensus_performance 
                ON consensus_rounds (round_duration_ms) 
                WHERE round_duration_ms IS NOT NULL;
            "#.to_string(),
            down: r#"
                ALTER TABLE consensus_rounds 
                DROP COLUMN IF EXISTS round_duration_ms,
                DROP COLUMN IF EXISTS vote_count,
                DROP COLUMN IF EXISTS byzantine_nodes,
                DROP COLUMN IF EXISTS network_partition;
                
                DROP INDEX IF EXISTS idx_consensus_performance;
            "#.to_string(),
            safety: MigrationSafety::Safe,
            strategy: ExecutionStrategy::NonTransactional, // Due to CONCURRENTLY
            ..Default::default()
        });
        
        // Partition large tables for performance
        migrations.push(Migration {
            id: "20240301_000000_partition_game_events".to_string(),
            description: "Partition game_events table by month for better performance".to_string(),
            up: r#"
                -- Convert to partitioned table
                ALTER TABLE game_events RENAME TO game_events_old;
                
                CREATE TABLE game_events (
                    LIKE game_events_old INCLUDING ALL
                ) PARTITION BY RANGE (created_at);
                
                -- Create partitions for past and future months
                CREATE TABLE game_events_2024_01 
                PARTITION OF game_events 
                FOR VALUES FROM ('2024-01-01') TO ('2024-02-01');
                
                CREATE TABLE game_events_2024_02 
                PARTITION OF game_events 
                FOR VALUES FROM ('2024-02-01') TO ('2024-03-01');
                
                -- Add more partitions as needed...
                
                -- Copy data
                INSERT INTO game_events SELECT * FROM game_events_old;
                
                -- Drop old table
                DROP TABLE game_events_old;
            "#.to_string(),
            down: r#"
                -- Reverse partitioning (warning: expensive operation)
                CREATE TABLE game_events_unpartitioned AS 
                SELECT * FROM game_events;
                
                DROP TABLE game_events CASCADE;
                
                ALTER TABLE game_events_unpartitioned 
                RENAME TO game_events;
            "#.to_string(),
            safety: MigrationSafety::RequiresDowntime {
                estimated_duration: Duration::from_secs(300),
                reason: "Table partitioning requires full table rewrite".to_string(),
            },
            strategy: ExecutionStrategy::NonTransactional,
            ..Default::default()
        });
        
        Ok(migrations)
    }
    
    /// Validate migrations before running
    pub async fn validate_migrations(&self) -> Result<ValidationReport> {
        let mut report = ValidationReport::default();
        
        for migration in self.engine.migrations.values() {
            // Check SQL syntax
            if let Err(e) = self.validator.validate_sql(&migration.up).await {
                report.errors.push(format!(
                    "Migration {} has invalid UP SQL: {}",
                    migration.id, e
                ));
            }
            
            if let Err(e) = self.validator.validate_sql(&migration.down).await {
                report.errors.push(format!(
                    "Migration {} has invalid DOWN SQL: {}",
                    migration.id, e
                ));
            }
            
            // Check if migration is reversible
            if !self.validator.is_reversible(migration).await? {
                report.warnings.push(format!(
                    "Migration {} may not be fully reversible",
                    migration.id
                ));
            }
            
            // Estimate execution time
            let estimate = self.validator.estimate_duration(migration).await?;
            if estimate > Duration::from_secs(60) {
                report.warnings.push(format!(
                    "Migration {} estimated to take {:?}",
                    migration.id, estimate
                ));
            }
        }
        
        Ok(report)
    }
}

/// Custom migration for BitCraps-specific operations
pub struct BitCrapsCustomMigrations;

impl BitCrapsCustomMigrations {
    /// Migrate from proof-of-work to proof-of-stake
    pub async fn migrate_consensus_mechanism(
        connection: &DatabaseConnection,
    ) -> Result<()> {
        // This is a complex migration that changes the consensus mechanism
        
        // Phase 1: Add new columns for proof-of-stake
        connection.execute(r#"
            ALTER TABLE validators 
            ADD COLUMN stake_amount BIGINT DEFAULT 0,
            ADD COLUMN delegation_pool BIGINT DEFAULT 0,
            ADD COLUMN last_block_proposed TIMESTAMP,
            ADD COLUMN slashing_history JSONB DEFAULT '[]'::jsonb;
        "#).await?;
        
        // Phase 2: Migrate existing validators
        connection.execute(r#"
            UPDATE validators 
            SET stake_amount = 
                CASE 
                    WHEN reputation_score > 0.9 THEN 1000000
                    WHEN reputation_score > 0.7 THEN 500000
                    ELSE 100000
                END
            WHERE active = true;
        "#).await?;
        
        // Phase 3: Create staking history table
        connection.execute(r#"
            CREATE TABLE staking_history (
                id SERIAL PRIMARY KEY,
                validator_id UUID REFERENCES validators(id),
                action VARCHAR(50) NOT NULL,
                amount BIGINT NOT NULL,
                timestamp TIMESTAMP NOT NULL DEFAULT NOW(),
                block_height BIGINT NOT NULL,
                transaction_hash VARCHAR(64) NOT NULL
            );
        "#).await?;
        
        // Phase 4: Update consensus parameters
        connection.execute(r#"
            UPDATE consensus_config 
            SET 
                mechanism = 'proof-of-stake',
                min_stake = 100000,
                max_validators = 100,
                unbonding_period = INTERVAL '21 days'
            WHERE active = true;
        "#).await?;
        
        Ok(())
    }
}
```

## Migration Testing Strategies

Testing migrations is crucial but often overlooked:

```rust
pub struct MigrationTester {
    test_database: TestDatabase,
    production_snapshot: DatabaseSnapshot,
}

impl MigrationTester {
    /// Test migration forward and backward
    pub async fn test_migration_reversibility(&self, migration: &Migration) -> Result<()> {
        // Apply migration
        self.test_database.execute(&migration.up).await?;
        
        // Verify schema after up migration
        let schema_after_up = self.test_database.get_schema().await?;
        
        // Apply down migration
        self.test_database.execute(&migration.down).await?;
        
        // Verify schema is restored
        let schema_after_down = self.test_database.get_schema().await?;
        
        // Compare schemas
        assert_eq!(
            self.test_database.initial_schema,
            schema_after_down,
            "Schema not properly restored after down migration"
        );
        
        Ok(())
    }
    
    /// Test migration with production data
    pub async fn test_with_production_data(&self, migration: &Migration) -> Result<()> {
        // Restore production snapshot to test database
        self.test_database.restore_snapshot(&self.production_snapshot).await?;
        
        // Measure performance
        let start = Instant::now();
        
        // Apply migration
        self.test_database.execute(&migration.up).await?;
        
        let duration = start.elapsed();
        
        // Verify data integrity
        let validation_queries = self.generate_validation_queries(migration);
        for query in validation_queries {
            let result = self.test_database.query(&query).await?;
            assert!(result.is_valid(), "Data integrity check failed");
        }
        
        // Check performance
        assert!(
            duration < Duration::from_secs(300),
            "Migration took too long: {:?}",
            duration
        );
        
        Ok(())
    }
    
    /// Test concurrent migrations
    pub async fn test_concurrent_safety(&self, migration: &Migration) -> Result<()> {
        let mut handles = vec![];
        
        // Spawn multiple migration attempts
        for i in 0..5 {
            let migration = migration.clone();
            let db = self.test_database.clone();
            
            let handle = tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(i * 100)).await;
                db.execute(&migration.up).await
            });
            
            handles.push(handle);
        }
        
        // Wait for all to complete
        let results = futures::future::join_all(handles).await;
        
        // Exactly one should succeed, others should fail gracefully
        let successes = results.iter().filter(|r| r.is_ok()).count();
        assert_eq!(successes, 1, "Multiple migrations succeeded concurrently!");
        
        Ok(())
    }
}
```

## Exercises

### Exercise 1: Implement Schema Versioning

Create a system to track and enforce schema versions:

```rust
pub struct SchemaVersion {
    major: u32,
    minor: u32,
    patch: u32,
}

pub trait SchemaVersioning {
    async fn get_current_version(&self) -> Result<SchemaVersion>;
    async fn validate_compatibility(&self, required: SchemaVersion) -> Result<bool>;
    async fn upgrade_to_version(&self, target: SchemaVersion) -> Result<()>;
}

// TODO: Implement version tracking and compatibility checking
```

### Exercise 2: Build Migration Conflict Detector

Detect when migrations might conflict:

```rust
pub struct ConflictDetector;

impl ConflictDetector {
    pub fn detect_conflicts(migrations: &[Migration]) -> Vec<Conflict> {
        // TODO: Detect migrations that:
        // 1. Modify the same table
        // 2. Create conflicting constraints
        // 3. Have incompatible data transformations
    }
}
```

### Exercise 3: Create Migration Performance Predictor

Predict how long migrations will take:

```rust
pub struct PerformancePredictor;

impl PerformancePredictor {
    pub async fn predict_duration(&self, migration: &Migration) -> Duration {
        // TODO: Analyze migration and predict duration based on:
        // 1. Operation type (ALTER, INDEX, UPDATE)
        // 2. Table size
        // 3. Index complexity
        // 4. Historical migration performance
    }
}
```

## Common Pitfalls and Solutions

### Pitfall 1: Not Testing Rollbacks
**Problem**: Down migrations fail when you need them most
**Solution**: Test both up and down migrations in CI

### Pitfall 2: Mixing DDL and DML in Transactions
**Problem**: Some DDL statements can't be rolled back
**Solution**: Separate schema changes from data changes

### Pitfall 3: Not Considering Application Deployment
**Problem**: New code expects new schema before migration runs
**Solution**: Make migrations backward compatible

### Pitfall 4: Ignoring Lock Impact
**Problem**: Migrations lock tables, causing application timeouts
**Solution**: Use online migration strategies for large tables

## Summary

Database migrations are essential for evolving applications safely. The key insights:

1. **Migrations are code**: Version control, review, and test them
2. **Plan for zero downtime**: Use expand-contract patterns
3. **Test with production data**: Migrations behave differently at scale
4. **Monitor and measure**: Track migration performance and impact
5. **Automate everything**: Manual migrations are error-prone
6. **Design for rollback**: Always have an escape plan

Migrations are the bridge between your application's past and future. Master them, and you can evolve your system fearlessly while keeping your data safe.

## References

- "Refactoring Databases" by Ambler & Sadalage
- "Zero Downtime Database Migrations" by Bryzek
- GitHub's gh-ost documentation
- Flyway and Liquibase migration tools
- "Database Reliability Engineering" by Campbell & Majors

---

*Next Chapter: [Chapter 85: CLI Design and Architecture](./85_cli_design_and_architecture.md)*

*Previous Chapter: [Chapter 83: MTU Discovery and Optimization](./83_mtu_discovery_and_optimization.md)*