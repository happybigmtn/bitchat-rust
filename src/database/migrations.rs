//! Database migration system for schema management

use rusqlite::Connection;
use std::collections::HashMap;
use chrono::Utc;
use sha2::{Sha256, Digest};
use crate::error::{Error, Result};

/// Migration manager for database schema versioning
pub struct MigrationManager {
    migrations: Vec<Migration>,
    connection: Option<Connection>,
}

/// Individual migration definition
#[derive(Clone)]
pub struct Migration {
    pub version: u32,
    pub name: String,
    pub up_sql: String,
    pub down_sql: Option<String>,
    pub checksum: String,
}

impl Migration {
    pub fn new(version: u32, name: impl Into<String>, up_sql: impl Into<String>) -> Self {
        let up = up_sql.into();
        let checksum = Self::calculate_checksum(&up);
        
        Self {
            version,
            name: name.into(),
            up_sql: up,
            down_sql: None,
            checksum,
        }
    }

    pub fn with_down(mut self, down_sql: impl Into<String>) -> Self {
        self.down_sql = Some(down_sql.into());
        self
    }

    fn calculate_checksum(sql: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(sql.as_bytes());
        format!("{:x}", hasher.finalize())
    }
}

impl Default for MigrationManager {
    fn default() -> Self {
        Self::new()
    }
}

impl MigrationManager {
    pub fn new() -> Self {
        Self {
            migrations: Self::load_migrations(),
            connection: None,
        }
    }

    /// Load all migration definitions
    fn load_migrations() -> Vec<Migration> {
        vec![
            // V1: Initial schema
            Migration::new(
                1,
                "initial_schema",
                r#"
                -- Users table
                CREATE TABLE IF NOT EXISTS users (
                    id TEXT PRIMARY KEY,
                    username TEXT UNIQUE NOT NULL,
                    public_key BLOB NOT NULL,
                    reputation REAL DEFAULT 0.0,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                );
                
                -- Games table
                CREATE TABLE IF NOT EXISTS games (
                    id TEXT PRIMARY KEY,
                    state TEXT NOT NULL,
                    pot_size INTEGER DEFAULT 0,
                    phase TEXT NOT NULL,
                    created_at INTEGER NOT NULL,
                    completed_at INTEGER,
                    winner_id TEXT REFERENCES users(id)
                );
                
                -- Bets table
                CREATE TABLE IF NOT EXISTS bets (
                    id BLOB PRIMARY KEY,
                    game_id TEXT NOT NULL REFERENCES games(id),
                    player_id TEXT NOT NULL REFERENCES users(id),
                    bet_type TEXT NOT NULL,
                    amount INTEGER NOT NULL,
                    outcome TEXT,
                    created_at INTEGER NOT NULL,
                    resolved_at INTEGER
                );
                
                -- Transactions table
                CREATE TABLE IF NOT EXISTS transactions (
                    id TEXT PRIMARY KEY,
                    from_user_id TEXT REFERENCES users(id),
                    to_user_id TEXT REFERENCES users(id),
                    amount INTEGER NOT NULL,
                    transaction_type TEXT NOT NULL,
                    status TEXT NOT NULL,
                    created_at INTEGER NOT NULL,
                    confirmed_at INTEGER
                );
                
                -- Indices for performance
                CREATE INDEX IF NOT EXISTS idx_games_created_at ON games(created_at);
                CREATE INDEX IF NOT EXISTS idx_bets_game_id ON bets(game_id);
                CREATE INDEX IF NOT EXISTS idx_bets_player_id ON bets(player_id);
                CREATE INDEX IF NOT EXISTS idx_transactions_from_user ON transactions(from_user_id);
                CREATE INDEX IF NOT EXISTS idx_transactions_to_user ON transactions(to_user_id);
                "#
            ).with_down(r#"
                DROP TABLE IF EXISTS transactions;
                DROP TABLE IF EXISTS bets;
                DROP TABLE IF EXISTS games;
                DROP TABLE IF EXISTS users;
            "#),

            // V2: Add peer connections tracking
            Migration::new(
                2,
                "add_peer_connections",
                r#"
                CREATE TABLE IF NOT EXISTS peer_connections (
                    id TEXT PRIMARY KEY,
                    peer_id TEXT NOT NULL,
                    connection_type TEXT NOT NULL,
                    signal_strength INTEGER,
                    latency_ms INTEGER,
                    connected_at INTEGER NOT NULL,
                    disconnected_at INTEGER,
                    data_sent_bytes INTEGER DEFAULT 0,
                    data_received_bytes INTEGER DEFAULT 0
                );
                
                CREATE INDEX IF NOT EXISTS idx_peer_connections_peer_id ON peer_connections(peer_id);
                CREATE INDEX IF NOT EXISTS idx_peer_connections_connected_at ON peer_connections(connected_at);
                "#
            ).with_down("DROP TABLE IF EXISTS peer_connections;"),

            // V3: Add consensus tracking
            Migration::new(
                3,
                "add_consensus_tracking",
                r#"
                CREATE TABLE IF NOT EXISTS consensus_rounds (
                    round_number INTEGER PRIMARY KEY,
                    game_id TEXT REFERENCES games(id),
                    proposer_id TEXT NOT NULL,
                    proposal_hash BLOB NOT NULL,
                    vote_count INTEGER DEFAULT 0,
                    finalized INTEGER DEFAULT 0,
                    created_at INTEGER NOT NULL,
                    finalized_at INTEGER
                );
                
                CREATE TABLE IF NOT EXISTS consensus_votes (
                    id TEXT PRIMARY KEY,
                    round_number INTEGER REFERENCES consensus_rounds(round_number),
                    voter_id TEXT NOT NULL,
                    vote_hash BLOB NOT NULL,
                    signature BLOB NOT NULL,
                    created_at INTEGER NOT NULL
                );
                
                CREATE INDEX IF NOT EXISTS idx_consensus_rounds_game_id ON consensus_rounds(game_id);
                CREATE INDEX IF NOT EXISTS idx_consensus_votes_round ON consensus_votes(round_number);
                "#
            ).with_down(r#"
                DROP TABLE IF EXISTS consensus_votes;
                DROP TABLE IF EXISTS consensus_rounds;
            "#),

            // V4: Add game statistics
            Migration::new(
                4,
                "add_game_statistics",
                r#"
                CREATE TABLE IF NOT EXISTS game_statistics (
                    game_id TEXT PRIMARY KEY REFERENCES games(id),
                    total_bets INTEGER DEFAULT 0,
                    total_wagered INTEGER DEFAULT 0,
                    total_won INTEGER DEFAULT 0,
                    house_edge REAL,
                    duration_seconds INTEGER,
                    player_count INTEGER DEFAULT 0,
                    max_pot_size INTEGER DEFAULT 0,
                    created_at INTEGER NOT NULL
                );
                
                CREATE TABLE IF NOT EXISTS player_statistics (
                    player_id TEXT PRIMARY KEY REFERENCES users(id),
                    games_played INTEGER DEFAULT 0,
                    games_won INTEGER DEFAULT 0,
                    total_wagered INTEGER DEFAULT 0,
                    total_won INTEGER DEFAULT 0,
                    win_rate REAL DEFAULT 0.0,
                    avg_bet_size INTEGER DEFAULT 0,
                    biggest_win INTEGER DEFAULT 0,
                    longest_streak INTEGER DEFAULT 0,
                    updated_at INTEGER NOT NULL
                );
                
                CREATE INDEX IF NOT EXISTS idx_game_statistics_created_at ON game_statistics(created_at);
                CREATE INDEX IF NOT EXISTS idx_player_statistics_win_rate ON player_statistics(win_rate);
                "#
            ).with_down(r#"
                DROP TABLE IF EXISTS player_statistics;
                DROP TABLE IF EXISTS game_statistics;
            "#),

            // V5: Add audit logging
            Migration::new(
                5,
                "add_audit_logging",
                r#"
                CREATE TABLE IF NOT EXISTS audit_log (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    event_type TEXT NOT NULL,
                    entity_type TEXT,
                    entity_id TEXT,
                    user_id TEXT,
                    old_value TEXT,
                    new_value TEXT,
                    metadata TEXT,
                    ip_address TEXT,
                    user_agent TEXT,
                    created_at INTEGER NOT NULL
                );
                
                CREATE INDEX IF NOT EXISTS idx_audit_log_event_type ON audit_log(event_type);
                CREATE INDEX IF NOT EXISTS idx_audit_log_entity ON audit_log(entity_type, entity_id);
                CREATE INDEX IF NOT EXISTS idx_audit_log_user_id ON audit_log(user_id);
                CREATE INDEX IF NOT EXISTS idx_audit_log_created_at ON audit_log(created_at);
                "#
            ).with_down("DROP TABLE IF EXISTS audit_log;"),

            // V6: Add token economy tables
            Migration::new(
                6,
                "add_token_economy",
                r#"
                CREATE TABLE IF NOT EXISTS token_balances (
                    user_id TEXT PRIMARY KEY REFERENCES users(id),
                    balance INTEGER NOT NULL DEFAULT 0,
                    locked_balance INTEGER DEFAULT 0,
                    staked_balance INTEGER DEFAULT 0,
                    last_updated INTEGER NOT NULL
                );
                
                CREATE TABLE IF NOT EXISTS token_transfers (
                    id TEXT PRIMARY KEY,
                    from_address TEXT NOT NULL,
                    to_address TEXT NOT NULL,
                    amount INTEGER NOT NULL,
                    fee INTEGER DEFAULT 0,
                    memo TEXT,
                    status TEXT NOT NULL,
                    block_height INTEGER,
                    created_at INTEGER NOT NULL,
                    confirmed_at INTEGER
                );
                
                CREATE TABLE IF NOT EXISTS staking_positions (
                    id TEXT PRIMARY KEY,
                    user_id TEXT NOT NULL REFERENCES users(id),
                    amount INTEGER NOT NULL,
                    lock_period_days INTEGER NOT NULL,
                    reward_rate REAL NOT NULL,
                    started_at INTEGER NOT NULL,
                    ends_at INTEGER NOT NULL,
                    claimed_rewards INTEGER DEFAULT 0,
                    is_active INTEGER DEFAULT 1
                );
                
                CREATE INDEX IF NOT EXISTS idx_token_transfers_from ON token_transfers(from_address);
                CREATE INDEX IF NOT EXISTS idx_token_transfers_to ON token_transfers(to_address);
                CREATE INDEX IF NOT EXISTS idx_staking_positions_user ON staking_positions(user_id);
                CREATE INDEX IF NOT EXISTS idx_staking_positions_active ON staking_positions(is_active);
                "#
            ).with_down(r#"
                DROP TABLE IF EXISTS staking_positions;
                DROP TABLE IF EXISTS token_transfers;
                DROP TABLE IF EXISTS token_balances;
            "#),

            // V7: Add performance metrics
            Migration::new(
                7,
                "add_performance_metrics",
                r#"
                CREATE TABLE IF NOT EXISTS performance_metrics (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    metric_name TEXT NOT NULL,
                    metric_value REAL NOT NULL,
                    metric_unit TEXT,
                    component TEXT NOT NULL,
                    tags TEXT,
                    created_at INTEGER NOT NULL
                );
                
                CREATE TABLE IF NOT EXISTS system_health (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    cpu_usage REAL,
                    memory_usage REAL,
                    disk_usage REAL,
                    network_in_bytes INTEGER,
                    network_out_bytes INTEGER,
                    active_connections INTEGER,
                    error_count INTEGER DEFAULT 0,
                    created_at INTEGER NOT NULL
                );
                
                CREATE INDEX IF NOT EXISTS idx_performance_metrics_name ON performance_metrics(metric_name);
                CREATE INDEX IF NOT EXISTS idx_performance_metrics_component ON performance_metrics(component);
                CREATE INDEX IF NOT EXISTS idx_performance_metrics_created_at ON performance_metrics(created_at);
                CREATE INDEX IF NOT EXISTS idx_system_health_created_at ON system_health(created_at);
                "#
            ).with_down(r#"
                DROP TABLE IF EXISTS system_health;
                DROP TABLE IF EXISTS performance_metrics;
            "#),
        ]
    }

    /// Initialize connection for migrations
    pub fn with_connection(mut self, conn: Connection) -> Self {
        self.connection = Some(conn);
        self
    }

    /// Run all pending migrations
    pub fn migrate(&mut self) -> Result<MigrationReport> {
        // Take the connection out temporarily to avoid borrow issues
        let mut conn = self.connection.take()
            .ok_or_else(|| Error::Database("No connection available".into()))?;

        // Create migrations table if it doesn't exist
        Self::create_migrations_table_static(&conn)?;

        let mut report = MigrationReport::new();
        let current_version = Self::get_current_version_static(&conn)?;

        let migrations = self.migrations.clone();
        for migration in &migrations {
            if migration.version > current_version {
                match Self::run_migration_static(&mut conn, migration) {
                    Ok(_) => {
                        report.successful.push(migration.version);
                        tracing::info!("Applied migration v{}: {}", migration.version, migration.name);
                    }
                    Err(e) => {
                        report.failed.push((migration.version, e.to_string()));
                        tracing::error!("Failed migration v{}: {}", migration.version, e);
                        // Stop on first failure
                        break;
                    }
                }
            } else {
                report.skipped.push(migration.version);
            }
        }

        report.final_version = Self::get_current_version_static(&conn)?;
        
        // Put the connection back
        self.connection = Some(conn);
        
        Ok(report)
    }

    /// Rollback to a specific version
    pub fn rollback_to(&mut self, target_version: u32) -> Result<MigrationReport> {
        // Take the connection out temporarily to avoid borrow issues
        let mut conn = self.connection.take()
            .ok_or_else(|| Error::Database("No connection available".into()))?;

        let mut report = MigrationReport::new();
        let current_version = Self::get_current_version_static(&conn)?;

        if target_version >= current_version {
            self.connection = Some(conn); // Put connection back before returning
            return Err(Error::Database(
                format!("Cannot rollback to version {} from current version {}", 
                    target_version, current_version)
            ));
        }

        // Rollback migrations in reverse order
        let migrations = self.migrations.clone();
        for migration in migrations.iter().rev() {
            if migration.version > target_version && migration.version <= current_version {
                match Self::rollback_migration_static(&mut conn, migration) {
                    Ok(_) => {
                        report.successful.push(migration.version);
                        tracing::info!("Rolled back migration v{}: {}", migration.version, migration.name);
                    }
                    Err(e) => {
                        report.failed.push((migration.version, e.to_string()));
                        tracing::error!("Failed to rollback migration v{}: {}", migration.version, e);
                        break;
                    }
                }
            }
        }

        report.final_version = Self::get_current_version_static(&conn)?;
        
        // Put the connection back
        self.connection = Some(conn);
        
        Ok(report)
    }

    /// Create migrations tracking table (static version)
    fn create_migrations_table_static(conn: &Connection) -> Result<()> {
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                checksum TEXT NOT NULL,
                applied_at TEXT NOT NULL
            )
            "#,
            [],
        ).map_err(|e| Error::Database(e.to_string()))?;
        Ok(())
    }

    /// Create migrations tracking table (non-static compatibility)
    fn create_migrations_table(&self, conn: &Connection) -> Result<()> {
        Self::create_migrations_table_static(conn)
    }

    /// Get current schema version (static version)
    fn get_current_version_static(conn: &Connection) -> Result<u32> {
        let version: Option<u32> = conn
            .query_row(
                "SELECT MAX(version) FROM schema_migrations",
                [],
                |row| row.get(0),
            )
            .unwrap_or(Some(0));
        Ok(version.unwrap_or(0))
    }

    /// Get current schema version (non-static compatibility)
    fn get_current_version(&self, conn: &Connection) -> Result<u32> {
        Self::get_current_version_static(conn)
    }

    /// Run a single migration (static version)
    fn run_migration_static(conn: &mut Connection, migration: &Migration) -> Result<()> {
        let tx = conn.transaction()
            .map_err(|e| Error::Database(e.to_string()))?;

        // Execute migration SQL
        tx.execute_batch(&migration.up_sql)
            .map_err(|e| Error::Database(format!("Migration failed: {}", e)))?;

        // Record migration
        tx.execute(
            "INSERT INTO schema_migrations (version, name, checksum, applied_at) VALUES (?, ?, ?, ?)",
            rusqlite::params![
                migration.version,
                &migration.name,
                &migration.checksum,
                Utc::now().to_rfc3339(),
            ],
        ).map_err(|e| Error::Database(e.to_string()))?;

        tx.commit()
            .map_err(|e| Error::Database(e.to_string()))?;

        Ok(())
    }

    /// Run a single migration (non-static compatibility)
    fn run_migration(&self, conn: &mut Connection, migration: &Migration) -> Result<()> {
        Self::run_migration_static(conn, migration)
    }

    /// Rollback a single migration (static version)
    fn rollback_migration_static(conn: &mut Connection, migration: &Migration) -> Result<()> {
        let down_sql = migration.down_sql.as_ref()
            .ok_or_else(|| Error::Database(format!("No rollback SQL for migration v{}", migration.version)))?;

        let tx = conn.transaction()
            .map_err(|e| Error::Database(e.to_string()))?;

        // Execute rollback SQL
        tx.execute_batch(down_sql)
            .map_err(|e| Error::Database(format!("Rollback failed: {}", e)))?;

        // Remove migration record
        tx.execute(
            "DELETE FROM schema_migrations WHERE version = ?",
            [migration.version],
        ).map_err(|e| Error::Database(e.to_string()))?;

        tx.commit()
            .map_err(|e| Error::Database(e.to_string()))?;

        Ok(())
    }

    /// Rollback a single migration (non-static compatibility)
    fn rollback_migration(&self, conn: &mut Connection, migration: &Migration) -> Result<()> {
        Self::rollback_migration_static(conn, migration)
    }

    /// Validate all migrations
    pub fn validate(&self, conn: &Connection) -> Result<ValidationReport> {
        let mut report = ValidationReport::new();

        // Check for duplicate versions
        let mut versions = HashMap::new();
        for migration in &self.migrations {
            if let Some(existing) = versions.insert(migration.version, &migration.name) {
                report.errors.push(format!(
                    "Duplicate version {}: '{}' and '{}'",
                    migration.version, existing, migration.name
                ));
            }
        }

        // Check for gaps in versions
        let mut sorted_versions: Vec<_> = versions.keys().copied().collect();
        sorted_versions.sort();
        for window in sorted_versions.windows(2) {
            if window[1] - window[0] > 1 {
                report.warnings.push(format!(
                    "Gap in migration versions between {} and {}",
                    window[0], window[1]
                ));
            }
        }

        // Verify applied migrations haven't changed
        if let Ok(applied) = self.get_applied_migrations(conn) {
            for (version, checksum) in applied {
                if let Some(migration) = self.migrations.iter().find(|m| m.version == version) {
                    if migration.checksum != checksum {
                        report.errors.push(format!(
                            "Migration v{} checksum mismatch. File changed after being applied!",
                            version
                        ));
                    }
                }
            }
        }

        report.is_valid = report.errors.is_empty();
        Ok(report)
    }

    /// Get list of applied migrations
    fn get_applied_migrations(&self, conn: &Connection) -> Result<Vec<(u32, String)>> {
        let mut stmt = conn.prepare("SELECT version, checksum FROM schema_migrations")
            .map_err(|e| Error::Database(e.to_string()))?;

        let migrations = stmt.query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?))
        }).map_err(|e| Error::Database(e.to_string()))?;

        migrations.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| Error::Database(e.to_string()))
    }

    /// Get migration status
    pub fn status(&self, conn: &Connection) -> Result<MigrationStatus> {
        let current_version = self.get_current_version(conn)?;
        let latest_version = self.migrations.iter()
            .map(|m| m.version)
            .max()
            .unwrap_or(0);

        let applied = self.get_applied_migrations(conn)?
            .into_iter()
            .map(|(v, _)| v)
            .collect();

        let pending: Vec<u32> = self.migrations.iter()
            .filter(|m| m.version > current_version)
            .map(|m| m.version)
            .collect();

        Ok(MigrationStatus {
            current_version,
            latest_version,
            applied,
            pending,
            is_up_to_date: current_version == latest_version,
        })
    }
}

/// Migration execution report
#[derive(Debug)]
pub struct MigrationReport {
    pub successful: Vec<u32>,
    pub failed: Vec<(u32, String)>,
    pub skipped: Vec<u32>,
    pub final_version: u32,
}

impl MigrationReport {
    fn new() -> Self {
        Self {
            successful: Vec::new(),
            failed: Vec::new(),
            skipped: Vec::new(),
            final_version: 0,
        }
    }

    pub fn is_success(&self) -> bool {
        self.failed.is_empty()
    }
}

/// Migration validation report
#[derive(Debug)]
pub struct ValidationReport {
    pub is_valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl ValidationReport {
    fn new() -> Self {
        Self {
            is_valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }
}

/// Current migration status
#[derive(Debug)]
pub struct MigrationStatus {
    pub current_version: u32,
    pub latest_version: u32,
    pub applied: Vec<u32>,
    pub pending: Vec<u32>,
    pub is_up_to_date: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn test_migration_manager() {
        let conn = Connection::open_in_memory().unwrap();
        
        // Create the migrations table first
        conn.execute(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at INTEGER NOT NULL
            )",
            [],
        ).unwrap();
        
        let mut manager = MigrationManager::new().with_connection(conn);
        
        // Run migrations
        let report = manager.migrate().unwrap();
        assert!(report.is_success());
        assert!(!report.successful.is_empty());
    }
}