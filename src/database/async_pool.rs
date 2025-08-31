//! Async database pool wrapper to eliminate synchronous bottlenecks
//!
//! This module provides a fully async interface to SQLite using tokio's
//! blocking thread pool to handle the synchronous rusqlite operations.

use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Semaphore};
use tokio::task;

use crate::error::{Error, Result};

/// Async database pool configuration
#[derive(Debug, Clone)]
pub struct AsyncDbConfig {
    pub path: String,
    pub max_connections: usize,
    pub connection_timeout: Duration,
    pub enable_wal: bool,
    pub checkpoint_interval: Duration,
    pub busy_timeout: Duration,
}

impl Default for AsyncDbConfig {
    fn default() -> Self {
        Self {
            path: "data/bitcraps.db".to_string(),
            max_connections: 10,
            connection_timeout: Duration::from_secs(5),
            enable_wal: true,
            checkpoint_interval: Duration::from_secs(300),
            busy_timeout: Duration::from_secs(5),
        }
    }
}

/// Async database connection pool
pub struct AsyncDatabasePool {
    config: AsyncDbConfig,
    connections: Arc<RwLock<Vec<AsyncConnection>>>,
    semaphore: Arc<Semaphore>,
    stats: Arc<RwLock<PoolStats>>,
}

/// Wrapper for async connection handling
struct AsyncConnection {
    id: usize,
    path: String,
    in_use: bool,
    created_at: Instant,
    last_used: Instant,
    total_queries: u64,
}

/// Pool statistics
#[derive(Debug, Clone, Default)]
pub struct PoolStats {
    pub total_connections: usize,
    pub active_connections: usize,
    pub total_queries: u64,
    pub total_wait_time_ms: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub corrupted: bool,
    pub total_transactions: u64,
}

impl AsyncDatabasePool {
    /// Create a new async database pool
    pub async fn new(config: AsyncDbConfig) -> Result<Self> {
        // Ensure database directory exists
        if let Some(parent) = Path::new(&config.path).parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| Error::Io(e))?;
        }

        // Initialize schema
        let path = config.path.clone();
        let enable_wal = config.enable_wal;
        let busy_timeout = config.busy_timeout.as_millis() as i32;

        task::spawn_blocking(move || -> Result<()> {
            let conn = Connection::open(&path).map_err(|e| Error::Database(e.to_string()))?;

            // Enable WAL mode
            if enable_wal {
                let _: String = conn.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))
                    .map_err(|e| Error::Database(e.to_string()))?;
            }

            // Set busy timeout
            conn.busy_timeout(Duration::from_millis(busy_timeout as u64))
                .map_err(|e| Error::Database(e.to_string()))?;

            // Initialize schema
            conn.execute_batch(
                "
                CREATE TABLE IF NOT EXISTS peers (
                    peer_id BLOB PRIMARY KEY,
                    public_key BLOB NOT NULL,
                    address TEXT,
                    last_seen INTEGER NOT NULL,
                    reputation INTEGER DEFAULT 0,
                    data BLOB
                );
                
                CREATE TABLE IF NOT EXISTS game_state (
                    game_id BLOB PRIMARY KEY,
                    state BLOB NOT NULL,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                );
                
                CREATE TABLE IF NOT EXISTS transactions (
                    tx_id BLOB PRIMARY KEY,
                    from_peer BLOB NOT NULL,
                    to_peer BLOB NOT NULL,
                    amount INTEGER NOT NULL,
                    timestamp INTEGER NOT NULL,
                    signature BLOB NOT NULL
                );
                
                CREATE INDEX IF NOT EXISTS idx_peers_last_seen ON peers(last_seen);
                CREATE INDEX IF NOT EXISTS idx_transactions_timestamp ON transactions(timestamp);
                CREATE INDEX IF NOT EXISTS idx_game_state_updated ON game_state(updated_at);
                ",
            )
            .map_err(|e| Error::Database(e.to_string()))?;

            Ok(())
        })
        .await
        .map_err(|e| Error::Database(format!("Task join error: {}", e)))??;

        // Create connection pool
        let mut connections = Vec::with_capacity(config.max_connections);
        for i in 0..config.max_connections {
            connections.push(AsyncConnection {
                id: i,
                path: config.path.clone(),
                in_use: false,
                created_at: Instant::now(),
                last_used: Instant::now(),
                total_queries: 0,
            });
        }

        let pool = Self {
            config: config.clone(),
            connections: Arc::new(RwLock::new(connections)),
            semaphore: Arc::new(Semaphore::new(config.max_connections)),
            stats: Arc::new(RwLock::new(PoolStats::default())),
        };

        // Start background maintenance
        pool.start_maintenance_tasks();

        Ok(pool)
    }

    /// Execute a query with a callback
    pub async fn execute<F, R>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&Connection) -> Result<R> + Send + 'static,
        R: Send + 'static,
    {
        let start = Instant::now();

        // Acquire semaphore permit
        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|_| Error::Database("Failed to acquire connection permit".to_string()))?;

        // Get available connection
        let conn_info = {
            let mut connections = self.connections.write().await;
            connections.iter_mut().find(|c| !c.in_use).map(|c| {
                c.in_use = true;
                c.last_used = Instant::now();
                c.total_queries += 1;
                (c.id, c.path.clone())
            })
        };

        let (conn_id, path) =
            conn_info.ok_or_else(|| Error::Database("No available connections".to_string()))?;

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.active_connections += 1;
            stats.total_queries += 1;
            stats.total_wait_time_ms += start.elapsed().as_millis() as u64;
        }

        // Execute in blocking thread
        let result = task::spawn_blocking(move || -> Result<R> {
            let conn = Connection::open(&path).map_err(|e| Error::Database(e.to_string()))?;

            // Set pragmas
            let _: String = conn.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))
                .map_err(|e| Error::Database(e.to_string()))?;
            conn.busy_timeout(Duration::from_secs(5))
                .map_err(|e| Error::Database(e.to_string()))?;

            // Execute user function
            f(&conn)
        })
        .await
        .map_err(|e| Error::Database(format!("Task join error: {}", e)))?;

        // Release connection
        {
            let mut connections = self.connections.write().await;
            if let Some(conn) = connections.iter_mut().find(|c| c.id == conn_id) {
                conn.in_use = false;
            }

            let mut stats = self.stats.write().await;
            stats.active_connections -= 1;
        }

        result
    }

    /// Execute a transaction
    pub async fn transaction<F, R>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&rusqlite::Transaction) -> Result<R> + Send + 'static,
        R: Send + 'static,
    {
        let start = Instant::now();

        // Acquire semaphore permit
        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|_| Error::Database("Failed to acquire connection permit".to_string()))?;

        // Get available connection
        let conn_info = {
            let mut connections = self.connections.write().await;
            connections.iter_mut().find(|c| !c.in_use).map(|c| {
                c.in_use = true;
                c.last_used = Instant::now();
                c.total_queries += 1;
                (c.id, c.path.clone())
            })
        };

        let (conn_id, path) =
            conn_info.ok_or_else(|| Error::Database("No available connections".to_string()))?;

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.active_connections += 1;
            stats.total_queries += 1;
            stats.total_wait_time_ms += start.elapsed().as_millis() as u64;
        }

        // Execute in blocking thread
        let result = task::spawn_blocking(move || -> Result<R> {
            let mut conn = Connection::open(&path).map_err(|e| Error::Database(e.to_string()))?;

            // Set pragmas
            let _: String = conn.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))
                .map_err(|e| Error::Database(e.to_string()))?;
            conn.busy_timeout(Duration::from_secs(5))
                .map_err(|e| Error::Database(e.to_string()))?;

            // Create and use transaction
            let tx = conn
                .transaction()
                .map_err(|e| Error::Database(e.to_string()))?;

            match f(&tx) {
                Ok(result) => {
                    tx.commit().map_err(|e| Error::Database(e.to_string()))?;
                    Ok(result)
                }
                Err(e) => {
                    // Transaction automatically rolls back on drop
                    Err(e)
                }
            }
        })
        .await
        .map_err(|e| Error::Database(format!("Task join error: {}", e)))?;

        // Release connection
        {
            let mut connections = self.connections.write().await;
            if let Some(conn) = connections.iter_mut().find(|c| c.id == conn_id) {
                conn.in_use = false;
            }

            let mut stats = self.stats.write().await;
            stats.active_connections -= 1;
        }

        result
    }

    /// Query with results
    pub async fn query<T, F>(&self, sql: &str, params: Vec<String>, f: F) -> Result<Vec<T>>
    where
        F: Fn(&rusqlite::Row) -> rusqlite::Result<T> + Clone + Send + 'static,
        T: Send + 'static,
    {
        let sql = sql.to_string();

        self.execute(move |conn| {
            let mut stmt = conn
                .prepare(&sql)
                .map_err(|e| Error::Database(e.to_string()))?;

            let param_refs: Vec<&dyn rusqlite::ToSql> =
                params.iter().map(|p| p as &dyn rusqlite::ToSql).collect();

            let results = stmt
                .query_map(&param_refs[..], f)
                .map_err(|e| Error::Database(e.to_string()))?
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|e| Error::Database(e.to_string()))?;

            Ok(results)
        })
        .await
    }

    /// Simple execute with no results
    pub async fn execute_sql(&self, sql: &str, params: Vec<String>) -> Result<usize> {
        let sql = sql.to_string();

        self.execute(move |conn| {
            let param_refs: Vec<&dyn rusqlite::ToSql> =
                params.iter().map(|p| p as &dyn rusqlite::ToSql).collect();

            conn.execute(&sql, &param_refs[..])
                .map_err(|e| Error::Database(e.to_string()))
        })
        .await
    }

    /// Get pool statistics
    pub async fn get_stats(&self) -> PoolStats {
        self.stats.read().await.clone()
    }

    /// Start background maintenance tasks
    fn start_maintenance_tasks(&self) {
        let stats = self.stats.clone();
        let interval_duration = self.config.checkpoint_interval;
        let path = self.config.path.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(interval_duration);

            loop {
                interval.tick().await;

                // Run checkpoint
                let checkpoint_path = path.clone();
                if let Ok(()) = task::spawn_blocking(move || -> Result<()> {
                    let conn = Connection::open(&checkpoint_path)
                        .map_err(|e| Error::Database(e.to_string()))?;

                    let _: (i64, i64) = conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
                        Ok((row.get(0)?, row.get(1)?))
                    })
                    .map_err(|e| Error::Database(e.to_string()))?;

                    Ok(())
                })
                .await
                .unwrap_or_else(|e| Err(Error::Database(format!("Checkpoint failed: {}", e))))
                {
                    log::debug!("Database checkpoint completed");
                }

                // Log stats periodically
                let current_stats = stats.read().await;
                log::debug!(
                    "Database pool stats: active={}/{}, queries={}",
                    current_stats.active_connections,
                    current_stats.total_connections,
                    current_stats.total_queries
                );
            }
        });
    }
}

/// Example high-level async operations
impl AsyncDatabasePool {
    /// Store peer information
    pub async fn store_peer(
        &self,
        peer_id: &[u8; 32],
        public_key: &[u8],
        address: Option<String>,
    ) -> Result<()> {
        let peer_id = peer_id.to_vec();
        let public_key = public_key.to_vec();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        self.execute(move |conn| {
            conn.execute(
                "INSERT OR REPLACE INTO peers (peer_id, public_key, address, last_seen) 
                 VALUES (?1, ?2, ?3, ?4)",
                params![peer_id, public_key, address, timestamp],
            )
            .map_err(|e| Error::Database(e.to_string()))?;
            Ok(())
        })
        .await
    }

    /// Get peer information
    pub async fn get_peer(&self, peer_id: &[u8; 32]) -> Result<Option<PeerInfo>> {
        let peer_id = peer_id.to_vec();

        self.execute(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT public_key, address, last_seen, reputation FROM peers WHERE peer_id = ?1"
            ).map_err(|e| Error::Database(e.to_string()))?;

            let peer = match stmt.query_row(params![peer_id], |row| {
                Ok(PeerInfo {
                    public_key: row.get(0)?,
                    address: row.get(1)?,
                    last_seen: row.get(2)?,
                    reputation: row.get(3)?,
                })
            }) {
                Ok(p) => Some(p),
                Err(rusqlite::Error::QueryReturnedNoRows) => None,
                Err(e) => return Err(Error::Database(e.to_string())),
            };

            Ok(peer)
        })
        .await
    }
}

#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub public_key: Vec<u8>,
    pub address: Option<String>,
    pub last_seen: i64,
    pub reputation: i32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_async_pool() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let config = AsyncDbConfig {
            path: db_path.to_str().unwrap().to_string(),
            max_connections: 5,
            ..Default::default()
        };

        let pool = AsyncDatabasePool::new(config).await.unwrap();

        // Test storing and retrieving peer
        let peer_id = [1u8; 32];
        let public_key = [2u8; 32];

        pool.store_peer(&peer_id, &public_key, Some("127.0.0.1:8333".to_string()))
            .await
            .unwrap();

        let peer_info = pool.get_peer(&peer_id).await.unwrap();
        assert!(peer_info.is_some());

        let info = peer_info.unwrap();
        assert_eq!(info.public_key, public_key.to_vec());
        assert_eq!(info.address, Some("127.0.0.1:8333".to_string()));
    }

    #[tokio::test]
    async fn test_concurrent_access() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let config = AsyncDbConfig {
            path: db_path.to_str().unwrap().to_string(),
            max_connections: 3,
            ..Default::default()
        };

        let pool = Arc::new(AsyncDatabasePool::new(config).await.unwrap());

        // Spawn multiple concurrent tasks
        let mut handles = vec![];

        for i in 0..10 {
            let pool_clone = pool.clone();
            let handle = tokio::spawn(async move {
                let peer_id = [i as u8; 32];
                let public_key = [(i + 1) as u8; 32];

                pool_clone.store_peer(&peer_id, &public_key, None).await
            });
            handles.push(handle);
        }

        // Wait for all tasks
        for handle in handles {
            handle.await.unwrap().unwrap();
        }

        // Verify stats
        let stats = pool.get_stats().await;
        assert_eq!(stats.total_queries, 10);
    }
}
