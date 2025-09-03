//! SQLite backend implementation for backward compatibility
//!
//! Features:
//! - WAL mode for better concurrency
//! - Connection pooling with tokio-rusqlite
//! - Automatic schema migrations
//! - In-memory caching for performance
//! - Backup and recovery support

use crate::database::abstractions::*;
use crate::error::{Error, Result};
use async_trait::async_trait;
use rusqlite::{Connection as SqliteConnection, Transaction as SqliteTransaction, Statement as SqliteStatement};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};
use tokio_rusqlite::{Connection as AsyncConnection};
use uuid::Uuid;

/// SQLite backend configuration
#[derive(Debug, Clone)]
pub struct SqliteConfig {
    pub database_path: String,
    pub enable_wal: bool,
    pub cache_size: i64,
    pub temp_store: TempStore,
    pub synchronous: Synchronous,
    pub journal_mode: JournalMode,
    pub foreign_keys: bool,
    pub busy_timeout: Duration,
    pub connection_pool: PoolConfiguration,
}

#[derive(Debug, Clone)]
pub enum TempStore {
    Default,
    File,
    Memory,
}

#[derive(Debug, Clone)]
pub enum Synchronous {
    Off,
    Normal,
    Full,
    Extra,
}

#[derive(Debug, Clone)]
pub enum JournalMode {
    Delete,
    Truncate,
    Persist,
    Memory,
    Wal,
    Off,
}

impl Default for SqliteConfig {
    fn default() -> Self {
        Self {
            database_path: ":memory:".to_string(),
            enable_wal: true,
            cache_size: -64000, // 64MB
            temp_store: TempStore::Memory,
            synchronous: Synchronous::Normal,
            journal_mode: JournalMode::Wal,
            foreign_keys: true,
            busy_timeout: Duration::from_secs(30),
            connection_pool: PoolConfiguration::default(),
        }
    }
}

/// SQLite backend implementation
pub struct SqliteBackend {
    connection_pool: Arc<SqliteConnectionPool>,
    config: SqliteConfig,
    statement_cache: Arc<RwLock<HashMap<String, CachedSqliteStatement>>>,
    metrics: Arc<RwLock<SqliteMetrics>>,
}

struct SqliteConnectionPool {
    connections: Mutex<Vec<AsyncConnection>>,
    config: SqliteConfig,
}

struct CachedSqliteStatement {
    sql: String,
    use_count: u32,
    last_used: Instant,
}

#[derive(Default)]
struct SqliteMetrics {
    total_queries: u64,
    total_transactions: u64,
    failed_queries: u64,
    avg_query_time_ms: f64,
    cache_hits: u64,
    cache_misses: u64,
}

impl SqliteBackend {
    pub async fn new(config: &DatabaseConnection) -> Result<Self> {
        let sqlite_config = Self::extract_sqlite_config(config)?;
        let connection_pool = Arc::new(SqliteConnectionPool::new(sqlite_config.clone()).await?);
        
        let backend = Self {
            connection_pool,
            config: sqlite_config,
            statement_cache: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(SqliteMetrics::default())),
        };
        
        // Initialize schema
        backend.initialize_schema().await?;
        
        Ok(backend)
    }
    
    fn extract_sqlite_config(config: &DatabaseConnection) -> Result<SqliteConfig> {
        let connection_string = &config.connection_string;
        
        let database_path = if connection_string.starts_with("sqlite://") {
            connection_string.strip_prefix("sqlite://").unwrap().to_string()
        } else if connection_string.starts_with("sqlite:") {
            connection_string.strip_prefix("sqlite:").unwrap().to_string()
        } else if connection_string.ends_with(".db") || connection_string == ":memory:" {
            connection_string.clone()
        } else {
            return Err(Error::Database("Invalid SQLite connection string".to_string()));
        };
        
        Ok(SqliteConfig {
            database_path,
            connection_pool: config.pool_config.clone(),
            ..SqliteConfig::default()
        })
    }
    
    async fn initialize_schema(&self) -> Result<()> {
        let schema_sql = include_str!("../migrations/sqlite/001_initial_schema.sql");
        self.execute(schema_sql, &[]).await?;
        Ok(())
    }
    
    async fn get_connection(&self) -> Result<AsyncConnection> {
        self.connection_pool.get_connection().await
    }
}

impl SqliteConnectionPool {
    async fn new(config: SqliteConfig) -> Result<Self> {
        let pool = Self {
            connections: Mutex::new(Vec::new()),
            config,
        };
        
        // Pre-create minimum connections
        let mut connections = pool.connections.lock().await;
        for _ in 0..pool.config.connection_pool.min_connections {
            let conn = pool.create_connection().await?;
            connections.push(conn);
        }
        
        Ok(pool)
    }
    
    async fn create_connection(&self) -> Result<AsyncConnection> {
        let conn = if self.config.database_path == ":memory:" {
            AsyncConnection::open_in_memory().await
        } else {
            AsyncConnection::open(&self.config.database_path).await
        }.map_err(|e| Error::Database(format!("Failed to open SQLite connection: {}", e)))?;
        
        // Configure connection
        self.configure_connection(&conn).await?;
        
        Ok(conn)
    }
    
    async fn configure_connection(&self, conn: &AsyncConnection) -> Result<()> {
        // Enable WAL mode
        if self.config.enable_wal {
            conn.call(|conn| {
                conn.pragma_update(None, "journal_mode", "WAL")?;
                Ok(())
            }).await.map_err(|e: rusqlite::Error| Error::Database(format!("Failed to enable WAL: {}", e)))?;
        }
        
        // Set cache size
        conn.call(move |conn| {
            conn.pragma_update(None, "cache_size", self.config.cache_size)?;
            Ok(())
        }).await.map_err(|e: rusqlite::Error| Error::Database(format!("Failed to set cache size: {}", e)))?;
        
        // Set temp store
        let temp_store_value = match self.config.temp_store {
            TempStore::Default => 0,
            TempStore::File => 1,
            TempStore::Memory => 2,
        };
        
        conn.call(move |conn| {
            conn.pragma_update(None, "temp_store", temp_store_value)?;
            Ok(())
        }).await.map_err(|e: rusqlite::Error| Error::Database(format!("Failed to set temp store: {}", e)))?;
        
        // Set synchronous mode
        let sync_value = match self.config.synchronous {
            Synchronous::Off => "OFF",
            Synchronous::Normal => "NORMAL",
            Synchronous::Full => "FULL",
            Synchronous::Extra => "EXTRA",
        };
        
        conn.call(move |conn| {
            conn.pragma_update(None, "synchronous", sync_value)?;
            Ok(())
        }).await.map_err(|e: rusqlite::Error| Error::Database(format!("Failed to set synchronous: {}", e)))?;
        
        // Enable foreign keys
        if self.config.foreign_keys {
            conn.call(|conn| {
                conn.pragma_update(None, "foreign_keys", true)?;
                Ok(())
            }).await.map_err(|e: rusqlite::Error| Error::Database(format!("Failed to enable foreign keys: {}", e)))?;
        }
        
        // Set busy timeout
        let timeout_ms = self.config.busy_timeout.as_millis() as i32;
        conn.call(move |conn| {
            conn.busy_timeout(std::time::Duration::from_millis(timeout_ms as u64))?;
            Ok(())
        }).await.map_err(|e: rusqlite::Error| Error::Database(format!("Failed to set busy timeout: {}", e)))?;
        
        Ok(())
    }
    
    async fn get_connection(&self) -> Result<AsyncConnection> {
        let mut connections = self.connections.lock().await;
        
        if let Some(conn) = connections.pop() {
            Ok(conn)
        } else if connections.len() < self.config.connection_pool.max_connections as usize {
            drop(connections); // Release lock before creating connection
            self.create_connection().await
        } else {
            // Wait for a connection to become available
            drop(connections);
            tokio::time::sleep(Duration::from_millis(10)).await;
            self.get_connection().await
        }
    }
    
    async fn return_connection(&self, conn: AsyncConnection) {
        let mut connections = self.connections.lock().await;
        if connections.len() < self.config.connection_pool.max_connections as usize {
            connections.push(conn);
        }
        // If pool is full, just drop the connection
    }
}

#[async_trait]
impl DatabaseBackendTrait for SqliteBackend {
    async fn initialize(&self, _config: &DatabaseConnection) -> Result<()> {
        // Already initialized in new()
        Ok(())
    }
    
    async fn begin_transaction(&self) -> Result<Box<dyn TransactionTrait>> {
        let connection = self.get_connection().await?;
        
        // Start transaction
        connection.call(|conn| {
            conn.execute("BEGIN IMMEDIATE", []).map_err(|e| {
                Error::Database(format!("Failed to begin transaction: {}", e))
            })?;
            Ok(())
        }).await?;
        
        Ok(Box::new(SqliteTransactionWrapper {
            connection: Some(connection),
            backend: self.connection_pool.clone(),
        }))
    }
    
    async fn query(&self, sql: &str, params: &[&dyn SqlParameter]) -> Result<QueryResult> {
        let start_time = Instant::now();
        let connection = self.get_connection().await?;
        
        let sql_owned = sql.to_string();
        let params_owned: Vec<SqlValue> = params.iter().map(|p| p.as_sql_value()).collect();
        
        let result = connection.call(move |conn| {
            let mut stmt = conn.prepare(&sql_owned)?;
            
            // Convert parameters to rusqlite format
            let rusqlite_params: Vec<Box<dyn rusqlite::ToSql>> = params_owned.iter()
                .map(|p| convert_to_rusqlite_param(p))
                .collect();
            
            let param_refs: Vec<&dyn rusqlite::ToSql> = rusqlite_params.iter()
                .map(|b| b.as_ref())
                .collect();
            
            let rows = stmt.query_map(&param_refs[..], |row| {
                convert_sqlite_row(row)
            })?;
            
            let mut db_rows = Vec::new();
            for row_result in rows {
                db_rows.push(row_result?);
            }
            
            Ok::<Vec<DatabaseRow>, rusqlite::Error>(db_rows)
        }).await.map_err(|e| Error::Database(format!("Query failed: {}", e)))?;
        
        self.connection_pool.return_connection(connection).await;
        
        // Update metrics
        let duration = start_time.elapsed();
        let mut metrics = self.metrics.write().await;
        metrics.total_queries += 1;
        metrics.avg_query_time_ms = (metrics.avg_query_time_ms + duration.as_millis() as f64) / 2.0;
        
        Ok(QueryResult {
            rows: result,
            affected_rows: 0,
        })
    }
    
    async fn execute(&self, sql: &str, params: &[&dyn SqlParameter]) -> Result<u64> {
        let start_time = Instant::now();
        let connection = self.get_connection().await?;
        
        let sql_owned = sql.to_string();
        let params_owned: Vec<SqlValue> = params.iter().map(|p| p.as_sql_value()).collect();
        
        let result = connection.call(move |conn| {
            let rusqlite_params: Vec<Box<dyn rusqlite::ToSql>> = params_owned.iter()
                .map(|p| convert_to_rusqlite_param(p))
                .collect();
            
            let param_refs: Vec<&dyn rusqlite::ToSql> = rusqlite_params.iter()
                .map(|b| b.as_ref())
                .collect();
            
            let affected = conn.execute(&sql_owned, &param_refs[..])?;
            Ok::<usize, rusqlite::Error>(affected)
        }).await.map_err(|e| Error::Database(format!("Execute failed: {}", e)))?;
        
        self.connection_pool.return_connection(connection).await;
        
        // Update metrics
        let duration = start_time.elapsed();
        let mut metrics = self.metrics.write().await;
        metrics.total_queries += 1;
        metrics.avg_query_time_ms = (metrics.avg_query_time_ms + duration.as_millis() as f64) / 2.0;
        
        Ok(result as u64)
    }
    
    async fn prepare(&self, sql: &str) -> Result<Box<dyn PreparedStatementTrait>> {
        // Check cache first
        let cache_key = sql.to_string();
        {
            let cache = self.statement_cache.read().await;
            if cache.contains_key(&cache_key) {
                let mut metrics = self.metrics.write().await;
                metrics.cache_hits += 1;
                return Ok(Box::new(SqlitePreparedStatement {
                    sql: sql.to_string(),
                    backend: self.connection_pool.clone(),
                }));
            }
        }
        
        // Add to cache
        let mut cache = self.statement_cache.write().await;
        cache.insert(cache_key, CachedSqliteStatement {
            sql: sql.to_string(),
            use_count: 1,
            last_used: Instant::now(),
        });
        
        let mut metrics = self.metrics.write().await;
        metrics.cache_misses += 1;
        
        Ok(Box::new(SqlitePreparedStatement {
            sql: sql.to_string(),
            backend: self.connection_pool.clone(),
        }))
    }
    
    async fn health_check(&self) -> Result<DatabaseHealth> {
        let start_time = Instant::now();
        
        let health_result = self.execute("SELECT 1", &[]).await;
        let response_time = start_time.elapsed().as_millis() as u64;
        
        let is_healthy = health_result.is_ok();
        let metrics = self.metrics.read().await;
        
        Ok(DatabaseHealth {
            is_healthy,
            response_time_ms: response_time,
            active_connections: 0, // SQLite doesn't have traditional connections
            error_rate: if metrics.total_queries > 0 {
                metrics.failed_queries as f32 / metrics.total_queries as f32
            } else {
                0.0
            },
            last_check: chrono::Utc::now(),
        })
    }
    
    fn pool_stats(&self) -> PoolStats {
        // SQLite connection pool stats are basic
        PoolStats {
            active_connections: 1, // SQLite uses single connection effectively
            idle_connections: 0,
            total_connections: 1,
            max_connections: 1,
            pending_requests: 0,
        }
    }
}

// Transaction wrapper for SQLite
struct SqliteTransactionWrapper {
    connection: Option<AsyncConnection>,
    backend: Arc<SqliteConnectionPool>,
}

#[async_trait]
impl TransactionTrait for SqliteTransactionWrapper {
    async fn query(&self, sql: &str, params: &[&dyn SqlParameter]) -> Result<QueryResult> {
        let connection = self.connection.as_ref()
            .ok_or_else(|| Error::Database("Transaction already completed".to_string()))?;
        
        let sql_owned = sql.to_string();
        let params_owned: Vec<SqlValue> = params.iter().map(|p| p.as_sql_value()).collect();
        
        let result = connection.call(move |conn| {
            let mut stmt = conn.prepare(&sql_owned)?;
            
            let rusqlite_params: Vec<&dyn rusqlite::ToSql> = params_owned.iter()
                .map(|p| convert_to_rusqlite_param(p))
                .collect();
            
            let rows = stmt.query_map(&rusqlite_params[..], |row| {
                convert_sqlite_row(row)
            })?;
            
            let mut db_rows = Vec::new();
            for row_result in rows {
                db_rows.push(row_result?);
            }
            
            Ok::<Vec<DatabaseRow>, rusqlite::Error>(db_rows)
        }).await.map_err(|e| Error::Database(format!("Transaction query failed: {}", e)))?;
        
        Ok(QueryResult {
            rows: result,
            affected_rows: 0,
        })
    }
    
    async fn execute(&self, sql: &str, params: &[&dyn SqlParameter]) -> Result<u64> {
        let connection = self.connection.as_ref()
            .ok_or_else(|| Error::Database("Transaction already completed".to_string()))?;
        
        let sql_owned = sql.to_string();
        let params_owned: Vec<SqlValue> = params.iter().map(|p| p.as_sql_value()).collect();
        
        let result = connection.call(move |conn| {
            let rusqlite_params: Vec<Box<dyn rusqlite::ToSql>> = params_owned.iter()
                .map(|p| convert_to_rusqlite_param(p))
                .collect();
            
            let param_refs: Vec<&dyn rusqlite::ToSql> = rusqlite_params.iter()
                .map(|b| b.as_ref())
                .collect();
            
            let affected = conn.execute(&sql_owned, &param_refs[..])?;
            Ok::<usize, rusqlite::Error>(affected)
        }).await.map_err(|e| Error::Database(format!("Transaction execute failed: {}", e)))?;
        
        Ok(result as u64)
    }
    
    async fn commit(mut self: Box<Self>) -> Result<()> {
        let connection = self.connection.take()
            .ok_or_else(|| Error::Database("Transaction already completed".to_string()))?;
        
        connection.call(|conn| {
            conn.execute("COMMIT", [])?;
            Ok::<(), rusqlite::Error>(())
        }).await.map_err(|e| Error::Database(format!("Transaction commit failed: {}", e)))?;
        
        self.backend.return_connection(connection).await;
        Ok(())
    }
    
    async fn rollback(mut self: Box<Self>) -> Result<()> {
        let connection = self.connection.take()
            .ok_or_else(|| Error::Database("Transaction already completed".to_string()))?;
        
        connection.call(|conn| {
            conn.execute("ROLLBACK", [])?;
            Ok::<(), rusqlite::Error>(())
        }).await.map_err(|e| Error::Database(format!("Transaction rollback failed: {}", e)))?;
        
        self.backend.return_connection(connection).await;
        Ok(())
    }
}

// Prepared statement implementation
struct SqlitePreparedStatement {
    sql: String,
    backend: Arc<SqliteConnectionPool>,
}

#[async_trait]
impl PreparedStatementTrait for SqlitePreparedStatement {
    async fn execute(&self, params: &[&dyn SqlParameter]) -> Result<u64> {
        let connection = self.backend.get_connection().await?;
        
        let sql_owned = self.sql.clone();
        let params_owned: Vec<SqlValue> = params.iter().map(|p| p.as_sql_value()).collect();
        
        let result = connection.call(move |conn| {
            let rusqlite_params: Vec<Box<dyn rusqlite::ToSql>> = params_owned.iter()
                .map(|p| convert_to_rusqlite_param(p))
                .collect();
            
            let param_refs: Vec<&dyn rusqlite::ToSql> = rusqlite_params.iter()
                .map(|b| b.as_ref())
                .collect();
            
            let affected = conn.execute(&sql_owned, &param_refs[..])?;
            Ok::<usize, rusqlite::Error>(affected)
        }).await.map_err(|e| Error::Database(format!("Prepared execute failed: {}", e)))?;
        
        self.backend.return_connection(connection).await;
        Ok(result as u64)
    }
    
    async fn query(&self, params: &[&dyn SqlParameter]) -> Result<QueryResult> {
        let connection = self.backend.get_connection().await?;
        
        let sql_owned = self.sql.clone();
        let params_owned: Vec<SqlValue> = params.iter().map(|p| p.as_sql_value()).collect();
        
        let result = connection.call(move |conn| {
            let mut stmt = conn.prepare(&sql_owned)?;
            
            let rusqlite_params: Vec<&dyn rusqlite::ToSql> = params_owned.iter()
                .map(|p| convert_to_rusqlite_param(p))
                .collect();
            
            let rows = stmt.query_map(&rusqlite_params[..], |row| {
                convert_sqlite_row(row)
            })?;
            
            let mut db_rows = Vec::new();
            for row_result in rows {
                db_rows.push(row_result?);
            }
            
            Ok::<Vec<DatabaseRow>, rusqlite::Error>(db_rows)
        }).await.map_err(|e| Error::Database(format!("Prepared query failed: {}", e)))?;
        
        self.backend.return_connection(connection).await;
        
        Ok(QueryResult {
            rows: result,
            affected_rows: 0,
        })
    }
}

// Helper functions for converting between types
fn convert_to_rusqlite_param(value: &SqlValue) -> Box<dyn rusqlite::ToSql> {
    match value {
        SqlValue::Null => Box::new(rusqlite::types::Null),
        SqlValue::Bool(v) => Box::new(*v),
        SqlValue::I32(v) => Box::new(*v),
        SqlValue::I64(v) => Box::new(*v),
        SqlValue::F64(v) => Box::new(*v),
        SqlValue::Text(v) => Box::new(v.clone()),
        SqlValue::Bytes(v) => Box::new(v.clone()),
        SqlValue::Uuid(v) => Box::new(v.to_string()),
        SqlValue::Timestamp(v) => Box::new(v.timestamp()),
    }
}

fn convert_sqlite_row(row: &rusqlite::Row) -> Result<DatabaseRow, rusqlite::Error> {
    let mut columns = HashMap::new();
    
    for (i, column_name) in row.as_ref().column_names().iter().enumerate() {
        let value = match row.get_ref(i)? {
            rusqlite::types::ValueRef::Null => SqlValue::Null,
            rusqlite::types::ValueRef::Integer(v) => SqlValue::I64(v),
            rusqlite::types::ValueRef::Real(v) => SqlValue::F64(v),
            rusqlite::types::ValueRef::Text(v) => {
                SqlValue::Text(String::from_utf8_lossy(v).to_string())
            }
            rusqlite::types::ValueRef::Blob(v) => SqlValue::Bytes(v.to_vec()),
        };
        
        columns.insert(column_name.to_string(), value);
    }
    
    Ok(DatabaseRow { columns })
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_sqlite_backend() {
        let config = DatabaseConnection {
            backend: DatabaseBackend::SQLite,
            connection_string: ":memory:".to_string(),
            pool_config: PoolConfiguration::default(),
        };
        
        let backend = SqliteBackend::new(&config).await.unwrap();
        
        // Test basic query
        let result = backend.execute(
            "CREATE TABLE test (id INTEGER PRIMARY KEY, name TEXT)", 
            &[]
        ).await.unwrap();
        
        assert_eq!(result, 0); // CREATE returns 0 affected rows
        
        // Test insert
        let name = "test_name".to_string();
        let result = backend.execute(
            "INSERT INTO test (name) VALUES (?)", 
            &[&name as &dyn SqlParameter]
        ).await.unwrap();
        
        assert_eq!(result, 1);
        
        // Test query
        let results = backend.query(
            "SELECT id, name FROM test WHERE name = ?",
            &[&name as &dyn SqlParameter]
        ).await.unwrap();
        
        assert_eq!(results.rows.len(), 1);
        let row = &results.rows[0];
        let retrieved_name: String = row.get("name").unwrap();
        assert_eq!(retrieved_name, "test_name");
    }
    
    #[tokio::test]
    async fn test_sqlite_transaction() {
        let config = DatabaseConnection {
            backend: DatabaseBackend::SQLite,
            connection_string: ":memory:".to_string(),
            pool_config: PoolConfiguration::default(),
        };
        
        let backend = SqliteBackend::new(&config).await.unwrap();
        
        // Create table
        backend.execute(
            "CREATE TABLE test (id INTEGER PRIMARY KEY, value INTEGER)", 
            &[]
        ).await.unwrap();
        
        // Test successful transaction
        {
            let tx = backend.begin_transaction().await.unwrap();
            tx.execute("INSERT INTO test (value) VALUES (?)", &[&42i32 as &dyn SqlParameter]).await.unwrap();
            tx.execute("INSERT INTO test (value) VALUES (?)", &[&43i32 as &dyn SqlParameter]).await.unwrap();
            tx.commit().await.unwrap();
        }
        
        // Verify data is committed
        let results = backend.query("SELECT COUNT(*) as count FROM test", &[]).await.unwrap();
        let count: i64 = results.rows[0].get("count").unwrap();
        assert_eq!(count, 2);
        
        // Test rollback transaction
        {
            let tx = backend.begin_transaction().await.unwrap();
            tx.execute("INSERT INTO test (value) VALUES (?)", &[&44i32 as &dyn SqlParameter]).await.unwrap();
            tx.rollback().await.unwrap();
        }
        
        // Verify rollback worked
        let results = backend.query("SELECT COUNT(*) as count FROM test", &[]).await.unwrap();
        let count: i64 = results.rows[0].get("count").unwrap();
        assert_eq!(count, 2); // Should still be 2
    }
}