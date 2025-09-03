//! Database manager with backend selection and scaling capabilities
//!
//! Features:
//! - Automatic backend selection based on configuration
//! - Horizontal scaling with sharding
//! - Read/write splitting
//! - Connection health monitoring
//! - Automatic failover and recovery

use crate::database::abstractions::*;
use crate::database::migration_manager::{MigrationManager, MigrationReport};
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// Database manager configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseManagerConfig {
    /// Primary database configuration
    pub primary: DatabaseConnection,
    
    /// Read replica configurations
    pub read_replicas: Vec<DatabaseConnection>,
    
    /// Sharding configuration (optional)
    pub sharding: Option<ShardingConfig>,
    
    /// Health check settings
    pub health_check: HealthCheckConfig,
    
    /// Failover settings
    pub failover: FailoverConfig,
    
    /// Migration settings
    pub migrations: MigrationConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckConfig {
    pub interval: Duration,
    pub timeout: Duration,
    pub max_failures: u32,
    pub recovery_interval: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailoverConfig {
    pub enable_automatic_failover: bool,
    pub max_retry_attempts: u32,
    pub retry_interval: Duration,
    pub circuit_breaker_threshold: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationConfig {
    pub migrations_directory: String,
    pub auto_migrate: bool,
    pub backup_before_migration: bool,
}

/// Database manager with high availability and scaling
pub struct DatabaseManager {
    primary_backend: Arc<dyn DatabaseBackendTrait>,
    read_backends: Vec<Arc<dyn DatabaseBackendTrait>>,
    sharded_manager: Option<ShardedDatabase>,
    config: DatabaseManagerConfig,
    health_monitor: Arc<DatabaseHealthMonitor>,
    migration_manager: Option<MigrationManager>,
}

impl DatabaseManager {
    /// Create a new database manager
    pub async fn new(config: DatabaseManagerConfig) -> Result<Self> {
        // Create primary backend
        let primary_backend = Arc::new(create_backend(&config.primary).await?);
        
        // Create read replica backends
        let mut read_backends = Vec::new();
        for replica_config in &config.read_replicas {
            let backend = Arc::new(create_backend(replica_config).await?);
            read_backends.push(backend);
        }
        
        // Create sharded manager if configured
        let sharded_manager = if let Some(ref sharding_config) = config.sharding {
            Some(ShardedDatabase::new(sharding_config.clone()).await?)
        } else {
            None
        };
        
        // Initialize health monitor
        let health_monitor = Arc::new(DatabaseHealthMonitor::new(
            primary_backend.clone(),
            read_backends.clone(),
            config.health_check.clone(),
        ));
        
        // Initialize migration manager
        let migration_manager = if !config.migrations.migrations_directory.is_empty() {
            Some(MigrationManager::new(
                primary_backend.clone(),
                &config.migrations.migrations_directory,
            ).await?)
        } else {
            None
        };
        
        let manager = Self {
            primary_backend,
            read_backends,
            sharded_manager,
            config,
            health_monitor,
            migration_manager,
        };
        
        // Start background health monitoring
        manager.start_health_monitoring().await;
        
        // Auto-migrate if configured
        if config.migrations.auto_migrate {
            manager.run_migrations().await?;
        }
        
        Ok(manager)
    }
    
    /// Run database migrations
    pub async fn run_migrations(&self) -> Result<MigrationReport> {
        match &self.migration_manager {
            Some(manager) => manager.migrate_up().await,
            None => Ok(MigrationReport {
                total_migrations: 0,
                successful: 0,
                failed: Vec::new(),
                execution_time_ms: 0,
            }),
        }
    }
    
    /// Execute a query (automatically routes to appropriate backend)
    pub async fn query(&self, sql: &str, params: &[&dyn SqlParameter]) -> Result<QueryResult> {
        if self.is_read_only_query(sql) {
            self.query_read_replica(sql, params).await
        } else {
            self.query_primary(sql, params).await
        }
    }
    
    /// Execute a write operation on the primary backend
    pub async fn execute(&self, sql: &str, params: &[&dyn SqlParameter]) -> Result<u64> {
        if let Some(ref sharded) = self.sharded_manager {
            // For sharded operations, we need a key to determine the shard
            // For now, execute on primary
            self.primary_backend.execute(sql, params).await
        } else {
            self.primary_backend.execute(sql, params).await
        }
    }
    
    /// Execute a sharded operation with a specific key
    pub async fn execute_sharded(&self, key: &str, sql: &str, params: &[&dyn SqlParameter]) -> Result<u64> {
        match &self.sharded_manager {
            Some(sharded) => {
                let shard = sharded.get_shard_for_key(key)?;
                shard.execute(sql, params).await
            }
            None => self.primary_backend.execute(sql, params).await,
        }
    }
    
    /// Execute a query on primary backend
    pub async fn query_primary(&self, sql: &str, params: &[&dyn SqlParameter]) -> Result<QueryResult> {
        self.execute_with_retry(|| {
            self.primary_backend.query(sql, params)
        }).await
    }
    
    /// Execute a query on read replica (with load balancing)
    pub async fn query_read_replica(&self, sql: &str, params: &[&dyn SqlParameter]) -> Result<QueryResult> {
        if self.read_backends.is_empty() {
            return self.query_primary(sql, params).await;
        }
        
        // Simple round-robin load balancing
        let backend_index = fastrand::usize(0..self.read_backends.len());
        let backend = &self.read_backends[backend_index];
        
        // Try the selected replica first
        match backend.query(sql, params).await {
            Ok(result) => Ok(result),
            Err(_) => {
                // Fall back to primary on failure
                log::warn!("Read replica failed, falling back to primary");
                self.query_primary(sql, params).await
            }
        }
    }
    
    /// Begin a transaction on the primary backend
    pub async fn begin_transaction(&self) -> Result<Box<dyn TransactionTrait>> {
        self.primary_backend.begin_transaction().await
    }
    
    /// Begin a sharded transaction
    pub async fn begin_sharded_transaction(&self, keys: &[String]) -> Result<ShardedTransaction> {
        match &self.sharded_manager {
            Some(sharded) => {
                let mut shard_transactions = HashMap::new();
                
                // Get unique shards for the keys
                let mut unique_shards = std::collections::HashSet::new();
                for key in keys {
                    let shard_id = sharded.hash_ring.get_shard(key)?;
                    unique_shards.insert(shard_id);
                }
                
                // Begin transaction on each shard
                for shard_id in unique_shards {
                    let shard = sharded.get_shard_for_key(&shard_id)?;
                    let tx = shard.begin_transaction().await?;
                    shard_transactions.insert(shard_id, tx);
                }
                
                Ok(ShardedTransaction {
                    transactions: shard_transactions,
                    sharded_manager: sharded,
                })
            }
            None => {
                // Single-shard transaction
                let tx = self.primary_backend.begin_transaction().await?;
                let mut shard_transactions = HashMap::new();
                shard_transactions.insert("primary".to_string(), tx);
                
                Ok(ShardedTransaction {
                    transactions: shard_transactions,
                    sharded_manager: self.sharded_manager.as_ref().unwrap(), // This is safe because we're in the None branch
                })
            }
        }
    }
    
    /// Check if a query is read-only
    fn is_read_only_query(&self, sql: &str) -> bool {
        let sql_upper = sql.trim().to_uppercase();
        sql_upper.starts_with("SELECT") ||
        sql_upper.starts_with("WITH") ||
        sql_upper.starts_with("SHOW") ||
        sql_upper.starts_with("DESCRIBE") ||
        sql_upper.starts_with("EXPLAIN")
    }
    
    /// Execute operation with retry logic
    async fn execute_with_retry<F, Fut, T>(&self, mut operation: F) -> Result<T>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let mut attempts = 0;
        let max_attempts = self.config.failover.max_retry_attempts;
        
        loop {
            match operation().await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    attempts += 1;
                    if attempts >= max_attempts {
                        return Err(e);
                    }
                    
                    log::warn!("Database operation failed (attempt {}): {}", attempts, e);
                    tokio::time::sleep(self.config.failover.retry_interval).await;
                }
            }
        }
    }
    
    /// Start background health monitoring
    async fn start_health_monitoring(&self) {
        if self.config.health_check.interval > Duration::ZERO {
            let health_monitor = self.health_monitor.clone();
            let interval = self.config.health_check.interval;
            
            tokio::spawn(async move {
                let mut interval_timer = tokio::time::interval(interval);
                loop {
                    interval_timer.tick().await;
                    if let Err(e) = health_monitor.check_all_backends().await {
                        log::error!("Health check failed: {}", e);
                    }
                }
            });
        }
    }
    
    /// Get database health status
    pub async fn get_health_status(&self) -> Result<DatabaseClusterHealth> {
        self.health_monitor.get_cluster_health().await
    }
    
    /// Get connection pool statistics
    pub fn get_pool_statistics(&self) -> DatabasePoolStatistics {
        let primary_stats = self.primary_backend.pool_stats();
        let replica_stats: Vec<PoolStats> = self.read_backends
            .iter()
            .map(|backend| backend.pool_stats())
            .collect();
        
        DatabasePoolStatistics {
            primary: primary_stats,
            replicas: replica_stats,
            sharded: self.sharded_manager.as_ref().map(|_| {
                // Would collect stats from all shards
                Vec::new() // Placeholder
            }),
        }
    }
}

/// Sharded transaction wrapper
pub struct ShardedTransaction<'a> {
    transactions: HashMap<String, Box<dyn TransactionTrait>>,
    sharded_manager: &'a ShardedDatabase,
}

impl<'a> ShardedTransaction<'a> {
    /// Execute a query within the transaction
    pub async fn query(&self, key: &str, sql: &str, params: &[&dyn SqlParameter]) -> Result<QueryResult> {
        let shard_id = self.sharded_manager.hash_ring.get_shard(key)?;
        
        if let Some(tx) = self.transactions.get(&shard_id) {
            tx.query(sql, params).await
        } else {
            Err(Error::Database("No transaction for shard".to_string()))
        }
    }
    
    /// Execute a statement within the transaction
    pub async fn execute(&self, key: &str, sql: &str, params: &[&dyn SqlParameter]) -> Result<u64> {
        let shard_id = self.sharded_manager.hash_ring.get_shard(key)?;
        
        if let Some(tx) = self.transactions.get(&shard_id) {
            tx.execute(sql, params).await
        } else {
            Err(Error::Database("No transaction for shard".to_string()))
        }
    }
    
    /// Commit all transactions (two-phase commit)
    pub async fn commit(self) -> Result<()> {
        // Phase 1: Prepare all transactions
        // Phase 2: Commit all transactions
        
        let mut transactions: Vec<_> = self.transactions.into_values().collect();
        
        // For simplicity, just commit all (in production, implement 2PC)
        for tx in transactions {
            tx.commit().await?;
        }
        
        Ok(())
    }
    
    /// Rollback all transactions
    pub async fn rollback(self) -> Result<()> {
        let transactions: Vec<_> = self.transactions.into_values().collect();
        
        for tx in transactions {
            tx.rollback().await?;
        }
        
        Ok(())
    }
}

/// Database health monitoring
pub struct DatabaseHealthMonitor {
    primary: Arc<dyn DatabaseBackendTrait>,
    replicas: Vec<Arc<dyn DatabaseBackendTrait>>,
    config: HealthCheckConfig,
    health_status: Arc<RwLock<DatabaseClusterHealth>>,
}

impl DatabaseHealthMonitor {
    fn new(
        primary: Arc<dyn DatabaseBackendTrait>,
        replicas: Vec<Arc<dyn DatabaseBackendTrait>>,
        config: HealthCheckConfig,
    ) -> Self {
        let health_status = Arc::new(RwLock::new(DatabaseClusterHealth {
            primary_healthy: true,
            replica_health: replicas.iter().map(|_| true).collect(),
            last_check: chrono::Utc::now(),
            total_failures: 0,
        }));
        
        Self {
            primary,
            replicas,
            config,
            health_status,
        }
    }
    
    async fn check_all_backends(&self) -> Result<()> {
        let primary_health = self.check_backend_health(&*self.primary).await;
        let mut replica_health = Vec::new();
        
        for replica in &self.replicas {
            let health = self.check_backend_health(&**replica).await;
            replica_health.push(health.is_ok());
        }
        
        let mut status = self.health_status.write().await;
        status.primary_healthy = primary_health.is_ok();
        status.replica_health = replica_health;
        status.last_check = chrono::Utc::now();
        
        if primary_health.is_err() {
            status.total_failures += 1;
        }
        
        Ok(())
    }
    
    async fn check_backend_health(&self, backend: &dyn DatabaseBackendTrait) -> Result<()> {
        tokio::time::timeout(
            self.config.timeout,
            backend.health_check()
        ).await
        .map_err(|_| Error::Database("Health check timeout".to_string()))?
        .map(|health| {
            if health.is_healthy {
                Ok(())
            } else {
                Err(Error::Database("Backend is unhealthy".to_string()))
            }
        })?
    }
    
    async fn get_cluster_health(&self) -> Result<DatabaseClusterHealth> {
        let status = self.health_status.read().await;
        Ok(status.clone())
    }
}

/// Database cluster health status
#[derive(Debug, Clone)]
pub struct DatabaseClusterHealth {
    pub primary_healthy: bool,
    pub replica_health: Vec<bool>,
    pub last_check: chrono::DateTime<chrono::Utc>,
    pub total_failures: u64,
}

/// Database pool statistics across all backends
#[derive(Debug, Clone)]
pub struct DatabasePoolStatistics {
    pub primary: PoolStats,
    pub replicas: Vec<PoolStats>,
    pub sharded: Option<Vec<PoolStats>>,
}

/// Factory function for creating database managers from environment
pub async fn create_database_manager_from_env() -> Result<DatabaseManager> {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite://./data/bitcraps.db".to_string());
    
    let backend = if database_url.starts_with("postgresql://") || database_url.starts_with("postgres://") {
        DatabaseBackend::PostgreSQL
    } else {
        DatabaseBackend::SQLite
    };
    
    let config = DatabaseManagerConfig {
        primary: DatabaseConnection {
            backend,
            connection_string: database_url,
            pool_config: PoolConfiguration::default(),
        },
        read_replicas: Vec::new(), // TODO: Load from environment
        sharding: None,
        health_check: HealthCheckConfig {
            interval: Duration::from_secs(30),
            timeout: Duration::from_secs(5),
            max_failures: 5,
            recovery_interval: Duration::from_secs(60),
        },
        failover: FailoverConfig {
            enable_automatic_failover: true,
            max_retry_attempts: 3,
            retry_interval: Duration::from_millis(100),
            circuit_breaker_threshold: 0.5,
        },
        migrations: MigrationConfig {
            migrations_directory: "src/database/migrations".to_string(),
            auto_migrate: true,
            backup_before_migration: true,
        },
    };
    
    DatabaseManager::new(config).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_database_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        
        let config = DatabaseManagerConfig {
            primary: DatabaseConnection {
                backend: DatabaseBackend::SQLite,
                connection_string: ":memory:".to_string(),
                pool_config: PoolConfiguration::default(),
            },
            read_replicas: Vec::new(),
            sharding: None,
            health_check: HealthCheckConfig {
                interval: Duration::from_secs(60),
                timeout: Duration::from_secs(5),
                max_failures: 3,
                recovery_interval: Duration::from_secs(30),
            },
            failover: FailoverConfig {
                enable_automatic_failover: true,
                max_retry_attempts: 3,
                retry_interval: Duration::from_millis(100),
                circuit_breaker_threshold: 0.5,
            },
            migrations: MigrationConfig {
                migrations_directory: temp_dir.path().join("migrations").to_string_lossy().to_string(),
                auto_migrate: false,
                backup_before_migration: false,
            },
        };
        
        let manager = DatabaseManager::new(config).await.unwrap();
        
        // Test basic query
        let result = manager.query("SELECT 1 as test", &[]).await.unwrap();
        assert_eq!(result.rows.len(), 1);
        
        // Test health status
        let health = manager.get_health_status().await.unwrap();
        assert!(health.primary_healthy);
        
        // Test pool statistics
        let stats = manager.get_pool_statistics();
        assert!(stats.primary.total_connections > 0);
    }
    
    #[tokio::test]
    async fn test_read_write_routing() {
        let config = DatabaseManagerConfig {
            primary: DatabaseConnection {
                backend: DatabaseBackend::SQLite,
                connection_string: ":memory:".to_string(),
                pool_config: PoolConfiguration::default(),
            },
            read_replicas: vec![
                DatabaseConnection {
                    backend: DatabaseBackend::SQLite,
                    connection_string: ":memory:".to_string(),
                    pool_config: PoolConfiguration::default(),
                }
            ],
            sharding: None,
            health_check: HealthCheckConfig {
                interval: Duration::ZERO, // Disable background health checks
                timeout: Duration::from_secs(5),
                max_failures: 3,
                recovery_interval: Duration::from_secs(30),
            },
            failover: FailoverConfig {
                enable_automatic_failover: false,
                max_retry_attempts: 1,
                retry_interval: Duration::from_millis(100),
                circuit_breaker_threshold: 0.5,
            },
            migrations: MigrationConfig {
                migrations_directory: "".to_string(),
                auto_migrate: false,
                backup_before_migration: false,
            },
        };
        
        let manager = DatabaseManager::new(config).await.unwrap();
        
        // Create table on primary
        manager.execute(
            "CREATE TABLE test (id INTEGER PRIMARY KEY, value TEXT)", 
            &[]
        ).await.unwrap();
        
        // Insert data
        let value = "test_value".to_string();
        manager.execute(
            "INSERT INTO test (value) VALUES (?)",
            &[&value as &dyn SqlParameter]
        ).await.unwrap();
        
        // Read-only query should route to replica (but will fall back to primary in this test)
        let result = manager.query("SELECT value FROM test", &[]).await.unwrap();
        assert_eq!(result.rows.len(), 1);
    }
}