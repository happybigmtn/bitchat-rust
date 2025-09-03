//! Database migration management for BitCraps
//!
//! Features:
//! - Cross-database migration support (SQLite, PostgreSQL)
//! - Atomic migrations with rollback support
//! - Migration validation and verification
//! - Version tracking and history
//! - Schema drift detection

use crate::database::abstractions::*;
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Migration definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Migration {
    pub version: String,
    pub name: String,
    pub description: String,
    pub up_sql: String,
    pub down_sql: Option<String>,
    pub backend: DatabaseBackend,
    pub checksum: String,
    pub created_at: u64,
}

/// Migration status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MigrationStatus {
    Pending,
    Applied,
    Failed,
    Rolled_back,
}

/// Applied migration record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppliedMigration {
    pub version: String,
    pub name: String,
    pub checksum: String,
    pub applied_at: u64,
    pub execution_time_ms: u64,
    pub status: MigrationStatus,
}

/// Migration manager for handling database schema changes
pub struct MigrationManager {
    backend: Box<dyn DatabaseBackendTrait>,
    migrations_dir: PathBuf,
    available_migrations: HashMap<String, Migration>,
}

impl MigrationManager {
    /// Create a new migration manager
    pub async fn new(
        backend: Box<dyn DatabaseBackendTrait>,
        migrations_dir: impl AsRef<Path>,
    ) -> Result<Self> {
        let migrations_dir = migrations_dir.as_ref().to_path_buf();
        let mut manager = Self {
            backend,
            migrations_dir,
            available_migrations: HashMap::new(),
        };
        
        // Initialize migration tracking table
        manager.initialize_migration_table().await?;
        
        // Load available migrations
        manager.load_available_migrations().await?;
        
        Ok(manager)
    }
    
    /// Initialize the migrations tracking table
    async fn initialize_migration_table(&self) -> Result<()> {
        let create_table_sql = r#"
            CREATE TABLE IF NOT EXISTS schema_migrations (
                version VARCHAR(255) PRIMARY KEY,
                name VARCHAR(255) NOT NULL,
                checksum VARCHAR(64) NOT NULL,
                applied_at BIGINT NOT NULL,
                execution_time_ms BIGINT NOT NULL,
                status VARCHAR(20) NOT NULL DEFAULT 'applied'
            )
        "#;
        
        self.backend.execute(create_table_sql, &[]).await?;
        Ok(())
    }
    
    /// Load available migrations from the migrations directory
    async fn load_available_migrations(&mut self) -> Result<()> {
        self.available_migrations.clear();
        
        // Determine backend-specific directory
        let backend_dir = match self.determine_backend().await? {
            DatabaseBackend::SQLite => self.migrations_dir.join("sqlite"),
            DatabaseBackend::PostgreSQL => self.migrations_dir.join("postgresql"),
        };
        
        if !backend_dir.exists() {
            return Ok(()); // No migrations directory for this backend
        }
        
        // Read migration files
        let entries = fs::read_dir(&backend_dir)
            .map_err(|e| Error::Database(format!("Failed to read migrations directory: {}", e)))?;
        
        for entry in entries {
            let entry = entry
                .map_err(|e| Error::Database(format!("Failed to read directory entry: {}", e)))?;
            
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("sql") {
                if let Some(migration) = self.parse_migration_file(&path).await? {
                    self.available_migrations.insert(migration.version.clone(), migration);
                }
            }
        }
        
        Ok(())
    }
    
    /// Parse a migration file
    async fn parse_migration_file(&self, path: &Path) -> Result<Option<Migration>> {
        let content = fs::read_to_string(path)
            .map_err(|e| Error::Database(format!("Failed to read migration file: {}", e)))?;
        
        let filename = path.file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| Error::Database("Invalid migration filename".to_string()))?;
        
        // Parse version and name from filename (e.g., "001_initial_schema.sql")
        let parts: Vec<&str> = filename.splitn(2, '_').collect();
        if parts.len() != 2 {
            return Ok(None); // Skip invalid filenames
        }
        
        let version = parts[0].to_string();
        let name = parts[1].replace('_', " ");
        
        // Calculate checksum
        let checksum = blake3::hash(content.as_bytes()).to_hex().to_string();
        
        let backend = self.determine_backend().await?;
        
        Ok(Some(Migration {
            version,
            name,
            description: format!("Migration from {}", filename),
            up_sql: content,
            down_sql: None, // TODO: Support down migrations
            backend,
            checksum,
            created_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        }))
    }
    
    /// Determine the database backend type
    async fn determine_backend(&self) -> Result<DatabaseBackend> {
        // Try to detect backend by querying for specific system functions
        let pg_result = self.backend.query("SELECT version()", &[]).await;
        if pg_result.is_ok() {
            let rows = pg_result.unwrap();
            if !rows.rows.is_empty() {
                if let Ok(version) = rows.rows[0].get::<String>("version") {
                    if version.to_lowercase().contains("postgresql") {
                        return Ok(DatabaseBackend::PostgreSQL);
                    }
                }
            }
        }
        
        // Default to SQLite
        Ok(DatabaseBackend::SQLite)
    }
    
    /// Get pending migrations
    pub async fn get_pending_migrations(&self) -> Result<Vec<Migration>> {
        let applied = self.get_applied_migrations().await?;
        let applied_versions: std::collections::HashSet<String> = applied
            .into_iter()
            .map(|m| m.version)
            .collect();
        
        let mut pending: Vec<Migration> = self
            .available_migrations
            .values()
            .filter(|m| !applied_versions.contains(&m.version))
            .cloned()
            .collect();
        
        // Sort by version
        pending.sort_by(|a, b| a.version.cmp(&b.version));
        
        Ok(pending)
    }
    
    /// Get applied migrations
    pub async fn get_applied_migrations(&self) -> Result<Vec<AppliedMigration>> {
        let result = self.backend.query(
            "SELECT version, name, checksum, applied_at, execution_time_ms, status 
             FROM schema_migrations ORDER BY version",
            &[]
        ).await?;
        
        let mut migrations = Vec::new();
        for row in result.rows {
            migrations.push(AppliedMigration {
                version: row.get("version")?,
                name: row.get("name")?,
                checksum: row.get("checksum")?,
                applied_at: row.get::<i64>("applied_at")? as u64,
                execution_time_ms: row.get::<i64>("execution_time_ms")? as u64,
                status: match row.get::<String>("status")?.as_str() {
                    "applied" => MigrationStatus::Applied,
                    "failed" => MigrationStatus::Failed,
                    "rolled_back" => MigrationStatus::Rolled_back,
                    _ => MigrationStatus::Pending,
                },
            });
        }
        
        Ok(migrations)
    }
    
    /// Apply all pending migrations
    pub async fn migrate_up(&self) -> Result<MigrationReport> {
        let pending = self.get_pending_migrations().await?;
        let mut report = MigrationReport {
            total_migrations: pending.len(),
            successful: 0,
            failed: Vec::new(),
            execution_time_ms: 0,
        };
        
        let start_time = std::time::Instant::now();
        
        for migration in pending {
            match self.apply_migration(&migration).await {
                Ok(execution_time) => {
                    report.successful += 1;
                    log::info!("Applied migration {}: {}", migration.version, migration.name);
                    
                    // Record successful migration
                    self.record_migration(&migration, execution_time, MigrationStatus::Applied).await?;
                }
                Err(e) => {
                    log::error!("Failed to apply migration {}: {}", migration.version, e);
                    report.failed.push(format!("{}: {}", migration.version, e));
                    
                    // Record failed migration
                    self.record_migration(&migration, 0, MigrationStatus::Failed).await?;
                    break; // Stop on first failure
                }
            }
        }
        
        report.execution_time_ms = start_time.elapsed().as_millis() as u64;
        Ok(report)
    }
    
    /// Apply a single migration
    async fn apply_migration(&self, migration: &Migration) -> Result<u64> {
        let start_time = std::time::Instant::now();
        
        // Validate checksum
        let current_checksum = blake3::hash(migration.up_sql.as_bytes()).to_hex().to_string();
        if current_checksum != migration.checksum {
            return Err(Error::Database(
                "Migration checksum mismatch - file may have been modified".to_string()
            ));
        }
        
        // Start transaction
        let tx = self.backend.begin_transaction().await?;
        
        // Execute migration SQL
        let result = tx.execute(&migration.up_sql, &[]).await;
        
        match result {
            Ok(_) => {
                tx.commit().await?;
                Ok(start_time.elapsed().as_millis() as u64)
            }
            Err(e) => {
                tx.rollback().await?;
                Err(e)
            }
        }
    }
    
    /// Record migration application
    async fn record_migration(
        &self,
        migration: &Migration,
        execution_time_ms: u64,
        status: MigrationStatus,
    ) -> Result<()> {
        let status_str = match status {
            MigrationStatus::Applied => "applied",
            MigrationStatus::Failed => "failed",
            MigrationStatus::Rolled_back => "rolled_back",
            MigrationStatus::Pending => "pending",
        };
        
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        
        self.backend.execute(
            "INSERT INTO schema_migrations (version, name, checksum, applied_at, execution_time_ms, status)
             VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT (version) DO UPDATE SET
             applied_at = ?, execution_time_ms = ?, status = ?",
            &[
                &migration.version as &dyn SqlParameter,
                &migration.name as &dyn SqlParameter,
                &migration.checksum as &dyn SqlParameter,
                &(now as i64) as &dyn SqlParameter,
                &(execution_time_ms as i64) as &dyn SqlParameter,
                &status_str.to_string() as &dyn SqlParameter,
                &(now as i64) as &dyn SqlParameter,
                &(execution_time_ms as i64) as &dyn SqlParameter,
                &status_str.to_string() as &dyn SqlParameter,
            ]
        ).await?;
        
        Ok(())
    }
    
    /// Validate database schema integrity
    pub async fn validate_schema(&self) -> Result<SchemaValidationReport> {
        let mut report = SchemaValidationReport {
            is_valid: true,
            issues: Vec::new(),
        };
        
        // Check for applied migrations that don't exist in files
        let applied = self.get_applied_migrations().await?;
        for applied_migration in applied {
            if !self.available_migrations.contains_key(&applied_migration.version) {
                report.is_valid = false;
                report.issues.push(format!(
                    "Migration {} is applied but file is missing",
                    applied_migration.version
                ));
            } else {
                // Check checksum
                let available = &self.available_migrations[&applied_migration.version];
                if available.checksum != applied_migration.checksum {
                    report.is_valid = false;
                    report.issues.push(format!(
                        "Migration {} checksum mismatch - file may have been modified",
                        applied_migration.version
                    ));
                }
            }
        }
        
        Ok(report)
    }
    
    /// Get migration status summary
    pub async fn get_status(&self) -> Result<MigrationStatus> {
        let pending = self.get_pending_migrations().await?;
        let applied = self.get_applied_migrations().await?;
        
        let failed = applied.iter().any(|m| m.status == MigrationStatus::Failed);
        
        if failed {
            Ok(MigrationStatus::Failed)
        } else if pending.is_empty() {
            Ok(MigrationStatus::Applied)
        } else {
            Ok(MigrationStatus::Pending)
        }
    }
}

/// Migration execution report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationReport {
    pub total_migrations: usize,
    pub successful: usize,
    pub failed: Vec<String>,
    pub execution_time_ms: u64,
}

/// Schema validation report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaValidationReport {
    pub is_valid: bool,
    pub issues: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::sqlite_backend::SqliteBackend;
    use tempfile::TempDir;
    use std::fs;
    
    #[tokio::test]
    async fn test_migration_manager() {
        let temp_dir = TempDir::new().unwrap();
        
        // Create SQLite backend
        let config = DatabaseConnection {
            backend: DatabaseBackend::SQLite,
            connection_string: ":memory:".to_string(),
            pool_config: PoolConfiguration::default(),
        };
        let backend = SqliteBackend::new(&config).await.unwrap();
        
        // Create migrations directory
        let migrations_dir = temp_dir.path().join("migrations");
        let sqlite_dir = migrations_dir.join("sqlite");
        fs::create_dir_all(&sqlite_dir).unwrap();
        
        // Create test migration
        let migration_sql = r#"
            CREATE TABLE test_migration (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL
            );
            
            INSERT INTO test_migration (name) VALUES ('test');
        "#;
        
        fs::write(sqlite_dir.join("001_test_migration.sql"), migration_sql).unwrap();
        
        // Create migration manager
        let mut manager = MigrationManager::new(
            Box::new(backend),
            migrations_dir,
        ).await.unwrap();
        
        // Check pending migrations
        let pending = manager.get_pending_migrations().await.unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].version, "001");
        assert_eq!(pending[0].name, "test migration");
        
        // Apply migrations
        let report = manager.migrate_up().await.unwrap();
        assert_eq!(report.successful, 1);
        assert_eq!(report.failed.len(), 0);
        
        // Check no pending migrations
        let pending = manager.get_pending_migrations().await.unwrap();
        assert_eq!(pending.len(), 0);
        
        // Check applied migrations
        let applied = manager.get_applied_migrations().await.unwrap();
        assert_eq!(applied.len(), 1);
        assert_eq!(applied[0].version, "001");
        assert_eq!(applied[0].status, MigrationStatus::Applied);
        
        // Validate schema
        let validation = manager.validate_schema().await.unwrap();
        assert!(validation.is_valid);
        assert_eq!(validation.issues.len(), 0);
    }
}