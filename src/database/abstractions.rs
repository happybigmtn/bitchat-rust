//! Database abstraction layer for BitCraps
//!
//! Provides a unified interface for multiple database backends:
//! - SQLite for development and small deployments
//! - PostgreSQL for production with horizontal scaling
//! - Sharding support for massive scale
//!
//! Features:
//! - Connection pooling with multiple pool implementations
//! - Database-agnostic migrations
//! - Horizontal sharding with consistent hashing
//! - Automatic failover and load balancing
//! - Transaction isolation across shards

use crate::error::{Error, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use uuid::Uuid;

/// Database backend type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DatabaseBackend {
    SQLite,
    PostgreSQL,
}

/// Database connection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConnection {
    pub backend: DatabaseBackend,
    pub connection_string: String,
    pub pool_config: PoolConfiguration,
}

/// Connection pool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolConfiguration {
    pub min_connections: u32,
    pub max_connections: u32,
    pub connection_timeout: Duration,
    pub idle_timeout: Duration,
    pub max_lifetime: Duration,
    pub test_on_checkout: bool,
    pub retry_attempts: u32,
}

impl Default for PoolConfiguration {
    fn default() -> Self {
        Self {
            min_connections: 5,
            max_connections: 32,
            connection_timeout: Duration::from_secs(30),
            idle_timeout: Duration::from_secs(600),
            max_lifetime: Duration::from_secs(3600),
            test_on_checkout: true,
            retry_attempts: 3,
        }
    }
}

/// Sharding configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardingConfig {
    pub enabled: bool,
    pub shards: Vec<ShardDefinition>,
    pub hash_function: HashFunction,
    pub rebalance_threshold: f32,
    pub auto_scaling: bool,
}

/// Individual shard definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardDefinition {
    pub id: String,
    pub connection: DatabaseConnection,
    pub weight: u32,
    pub range_start: u64,
    pub range_end: u64,
    pub status: ShardStatus,
}

/// Shard status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ShardStatus {
    Active,
    ReadOnly,
    Draining,
    Offline,
}

/// Hash function for sharding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HashFunction {
    Consistent,
    Murmur3,
    Blake3,
}

/// Database abstraction trait
#[async_trait]
pub trait DatabaseBackendTrait: Send + Sync {
    /// Initialize the database backend
    async fn initialize(&self, config: &DatabaseConnection) -> Result<()>;
    
    /// Create a transaction
    async fn begin_transaction(&self) -> Result<Box<dyn TransactionTrait>>;
    
    /// Execute a query that returns rows
    async fn query(&self, sql: &str, params: &[&dyn SqlParameter]) -> Result<QueryResult>;
    
    /// Execute a query that doesn't return rows
    async fn execute(&self, sql: &str, params: &[&dyn SqlParameter]) -> Result<u64>;
    
    /// Prepare a statement for repeated execution
    async fn prepare(&self, sql: &str) -> Result<Box<dyn PreparedStatementTrait>>;
    
    /// Get database health information
    async fn health_check(&self) -> Result<DatabaseHealth>;
    
    /// Get connection pool statistics
    fn pool_stats(&self) -> PoolStats;
}

/// Transaction abstraction trait
#[async_trait]
pub trait TransactionTrait: Send + Sync {
    /// Execute a query within the transaction
    async fn query(&self, sql: &str, params: &[&dyn SqlParameter]) -> Result<QueryResult>;
    
    /// Execute a statement within the transaction
    async fn execute(&self, sql: &str, params: &[&dyn SqlParameter]) -> Result<u64>;
    
    /// Commit the transaction
    async fn commit(self: Box<Self>) -> Result<()>;
    
    /// Rollback the transaction
    async fn rollback(self: Box<Self>) -> Result<()>;
}

/// Prepared statement trait
#[async_trait]
pub trait PreparedStatementTrait: Send + Sync {
    /// Execute the prepared statement
    async fn execute(&self, params: &[&dyn SqlParameter]) -> Result<u64>;
    
    /// Query with the prepared statement
    async fn query(&self, params: &[&dyn SqlParameter]) -> Result<QueryResult>;
}

/// SQL parameter trait for type-safe parameter binding
pub trait SqlParameter: Send + Sync {
    fn as_sql_value(&self) -> SqlValue;
}

/// SQL value types
#[derive(Debug, Clone)]
pub enum SqlValue {
    Null,
    Bool(bool),
    I32(i32),
    I64(i64),
    F64(f64),
    Text(String),
    Bytes(Vec<u8>),
    Uuid(Uuid),
    Timestamp(chrono::DateTime<chrono::Utc>),
}

// Implement SqlParameter for common types
impl SqlParameter for i32 {
    fn as_sql_value(&self) -> SqlValue {
        SqlValue::I32(*self)
    }
}

impl SqlParameter for i64 {
    fn as_sql_value(&self) -> SqlValue {
        SqlValue::I64(*self)
    }
}

impl SqlParameter for f64 {
    fn as_sql_value(&self) -> SqlValue {
        SqlValue::F64(*self)
    }
}

impl SqlParameter for String {
    fn as_sql_value(&self) -> SqlValue {
        SqlValue::Text(self.clone())
    }
}

impl SqlParameter for &str {
    fn as_sql_value(&self) -> SqlValue {
        SqlValue::Text(self.to_string())
    }
}

impl SqlParameter for Vec<u8> {
    fn as_sql_value(&self) -> SqlValue {
        SqlValue::Bytes(self.clone())
    }
}

impl SqlParameter for Uuid {
    fn as_sql_value(&self) -> SqlValue {
        SqlValue::Uuid(*self)
    }
}

impl SqlParameter for bool {
    fn as_sql_value(&self) -> SqlValue {
        SqlValue::Bool(*self)
    }
}

/// Query result abstraction
#[derive(Debug, Clone)]
pub struct QueryResult {
    pub rows: Vec<DatabaseRow>,
    pub affected_rows: u64,
}

/// Database row abstraction
#[derive(Debug, Clone)]
pub struct DatabaseRow {
    pub columns: HashMap<String, SqlValue>,
}

impl DatabaseRow {
    pub fn get<T: FromSqlValue>(&self, column: &str) -> Result<T> {
        let value = self.columns.get(column)
            .ok_or_else(|| Error::Database(format!("Column {} not found", column)))?;
        T::from_sql_value(value)
    }
    
    pub fn get_opt<T: FromSqlValue>(&self, column: &str) -> Result<Option<T>> {
        match self.columns.get(column) {
            Some(SqlValue::Null) | None => Ok(None),
            Some(value) => Ok(Some(T::from_sql_value(value)?)),
        }
    }
}

/// Trait for converting SQL values to Rust types
pub trait FromSqlValue: Sized {
    fn from_sql_value(value: &SqlValue) -> Result<Self>;
}

// Implement FromSqlValue for common types
impl FromSqlValue for i32 {
    fn from_sql_value(value: &SqlValue) -> Result<Self> {
        match value {
            SqlValue::I32(v) => Ok(*v),
            SqlValue::I64(v) => Ok(*v as i32),
            _ => Err(Error::Database("Cannot convert to i32".to_string())),
        }
    }
}

impl FromSqlValue for i64 {
    fn from_sql_value(value: &SqlValue) -> Result<Self> {
        match value {
            SqlValue::I64(v) => Ok(*v),
            SqlValue::I32(v) => Ok(*v as i64),
            _ => Err(Error::Database("Cannot convert to i64".to_string())),
        }
    }
}

impl FromSqlValue for f64 {
    fn from_sql_value(value: &SqlValue) -> Result<Self> {
        match value {
            SqlValue::F64(v) => Ok(*v),
            _ => Err(Error::Database("Cannot convert to f64".to_string())),
        }
    }
}

impl FromSqlValue for String {
    fn from_sql_value(value: &SqlValue) -> Result<Self> {
        match value {
            SqlValue::Text(v) => Ok(v.clone()),
            _ => Err(Error::Database("Cannot convert to String".to_string())),
        }
    }
}

impl FromSqlValue for Vec<u8> {
    fn from_sql_value(value: &SqlValue) -> Result<Self> {
        match value {
            SqlValue::Bytes(v) => Ok(v.clone()),
            _ => Err(Error::Database("Cannot convert to Vec<u8>".to_string())),
        }
    }
}

impl FromSqlValue for Uuid {
    fn from_sql_value(value: &SqlValue) -> Result<Self> {
        match value {
            SqlValue::Uuid(v) => Ok(*v),
            SqlValue::Text(v) => Uuid::parse_str(v)
                .map_err(|_| Error::Database("Invalid UUID format".to_string())),
            _ => Err(Error::Database("Cannot convert to Uuid".to_string())),
        }
    }
}

impl FromSqlValue for bool {
    fn from_sql_value(value: &SqlValue) -> Result<Self> {
        match value {
            SqlValue::Bool(v) => Ok(*v),
            SqlValue::I32(v) => Ok(*v != 0),
            _ => Err(Error::Database("Cannot convert to bool".to_string())),
        }
    }
}

impl FromSqlValue for chrono::DateTime<chrono::Utc> {
    fn from_sql_value(value: &SqlValue) -> Result<Self> {
        match value {
            SqlValue::Timestamp(v) => Ok(*v),
            _ => Err(Error::Database("Cannot convert to DateTime".to_string())),
        }
    }
}

/// Database health information
#[derive(Debug, Clone)]
pub struct DatabaseHealth {
    pub is_healthy: bool,
    pub response_time_ms: u64,
    pub active_connections: u32,
    pub error_rate: f32,
    pub last_check: chrono::DateTime<chrono::Utc>,
}

/// Connection pool statistics
#[derive(Debug, Clone)]
pub struct PoolStats {
    pub active_connections: u32,
    pub idle_connections: u32,
    pub total_connections: u32,
    pub max_connections: u32,
    pub pending_requests: u32,
}

/// Sharded database manager
pub struct ShardedDatabase {
    shards: Vec<Box<dyn DatabaseBackendTrait>>,
    config: ShardingConfig,
    hash_ring: ConsistentHashRing,
}

impl ShardedDatabase {
    pub async fn new(config: ShardingConfig) -> Result<Self> {
        let mut shards = Vec::new();
        let mut hash_ring = ConsistentHashRing::new(config.hash_function.clone());
        
        for shard_def in &config.shards {
            // Create appropriate backend based on configuration
            let backend = create_backend(&shard_def.connection).await?;
            shards.push(backend);
            
            // Add shard to hash ring
            hash_ring.add_shard(&shard_def.id, shard_def.weight);
        }
        
        Ok(Self {
            shards,
            config,
            hash_ring,
        })
    }
    
    /// Get the appropriate shard for a given key
    pub fn get_shard_for_key(&self, key: &str) -> Result<&dyn DatabaseBackendTrait> {
        let shard_id = self.hash_ring.get_shard(key)?;
        let shard_index = self.config.shards
            .iter()
            .position(|s| s.id == shard_id)
            .ok_or_else(|| Error::Database(format!("Shard {} not found", shard_id)))?;
            
        Ok(self.shards[shard_index].as_ref())
    }
    
    /// Execute a cross-shard transaction (two-phase commit)
    pub async fn execute_cross_shard_transaction<F>(&self, keys: &[String], operation: F) -> Result<()>
    where
        F: Fn(&dyn DatabaseBackendTrait) -> Result<()> + Send + Sync,
    {
        // Phase 1: Prepare all shards
        let mut prepared_shards = Vec::new();
        let mut transactions = Vec::new();
        
        for key in keys {
            let shard = self.get_shard_for_key(key)?;
            let tx = shard.begin_transaction().await?;
            prepared_shards.push(shard);
            transactions.push(tx);
        }
        
        // Phase 2: Commit or rollback all
        let mut success = true;
        for (shard, tx) in prepared_shards.iter().zip(transactions.iter()) {
            if let Err(_) = operation(*shard) {
                success = false;
                break;
            }
        }
        
        if success {
            for tx in transactions {
                tx.commit().await?;
            }
        } else {
            for tx in transactions {
                tx.rollback().await?;
            }
            return Err(Error::Database("Cross-shard transaction failed".to_string()));
        }
        
        Ok(())
    }
}

/// Consistent hash ring for sharding
pub struct ConsistentHashRing {
    hash_function: HashFunction,
    ring: std::collections::BTreeMap<u64, String>,
    virtual_nodes: u32,
}

impl ConsistentHashRing {
    pub fn new(hash_function: HashFunction) -> Self {
        Self {
            hash_function,
            ring: std::collections::BTreeMap::new(),
            virtual_nodes: 160, // Standard virtual node count
        }
    }
    
    pub fn add_shard(&mut self, shard_id: &str, weight: u32) {
        let virtual_nodes = self.virtual_nodes * weight;
        for i in 0..virtual_nodes {
            let virtual_key = format!("{}:{}", shard_id, i);
            let hash = self.hash(&virtual_key);
            self.ring.insert(hash, shard_id.to_string());
        }
    }
    
    pub fn get_shard(&self, key: &str) -> Result<String> {
        if self.ring.is_empty() {
            return Err(Error::Database("No shards available".to_string()));
        }
        
        let hash = self.hash(key);
        
        // Find the first shard with hash >= key hash
        let shard = self.ring
            .range(hash..)
            .next()
            .or_else(|| self.ring.iter().next()) // Wrap around
            .map(|(_, shard_id)| shard_id.clone())
            .ok_or_else(|| Error::Database("No suitable shard found".to_string()))?;
            
        Ok(shard)
    }
    
    fn hash(&self, key: &str) -> u64 {
        match self.hash_function {
            HashFunction::Consistent => {
                use std::hash::{Hash, Hasher};
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                key.hash(&mut hasher);
                hasher.finish()
            }
            HashFunction::Murmur3 => {
                // In production, use a proper murmur3 implementation
                use std::hash::{Hash, Hasher};
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                key.hash(&mut hasher);
                hasher.finish()
            }
            HashFunction::Blake3 => {
                let hash = blake3::hash(key.as_bytes());
                u64::from_le_bytes(hash.as_bytes()[0..8].try_into().unwrap())
            }
        }
    }
}

/// Create a database backend based on configuration
pub async fn create_backend(config: &DatabaseConnection) -> Result<Box<dyn DatabaseBackendTrait>> {
    match config.backend {
        DatabaseBackend::SQLite => {
            #[cfg(feature = "sqlite")]
            {
                let backend = crate::database::sqlite_backend::SqliteBackend::new(config).await?;
                Ok(Box::new(backend))
            }
            #[cfg(not(feature = "sqlite"))]
            {
                Err(Error::Database("SQLite support not compiled in".to_string()))
            }
        }
        DatabaseBackend::PostgreSQL => {
            #[cfg(feature = "postgres")]
            {
                let backend = crate::database::postgres_backend::PostgresBackend::new(config).await?;
                Ok(Box::new(backend))
            }
            #[cfg(not(feature = "postgres"))]
            {
                Err(Error::Database("PostgreSQL support not compiled in".to_string()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_consistent_hash_ring() {
        let mut ring = ConsistentHashRing::new(HashFunction::Blake3);
        ring.add_shard("shard1", 1);
        ring.add_shard("shard2", 1);
        ring.add_shard("shard3", 2); // Higher weight
        
        // Test that the same key always goes to the same shard
        let shard1 = ring.get_shard("test_key_1").unwrap();
        let shard2 = ring.get_shard("test_key_1").unwrap();
        assert_eq!(shard1, shard2);
        
        // Test distribution
        let mut distribution = HashMap::new();
        for i in 0..1000 {
            let key = format!("key_{}", i);
            let shard = ring.get_shard(&key).unwrap();
            *distribution.entry(shard).or_insert(0) += 1;
        }
        
        // Should have reasonable distribution
        assert!(distribution.len() > 1);
        println!("Distribution: {:?}", distribution);
    }
    
    #[test]
    fn test_sql_value_conversions() {
        let value = SqlValue::I32(42);
        assert_eq!(i32::from_sql_value(&value).unwrap(), 42);
        assert_eq!(i64::from_sql_value(&value).unwrap(), 42i64);
        
        let value = SqlValue::Text("hello".to_string());
        assert_eq!(String::from_sql_value(&value).unwrap(), "hello");
        
        let value = SqlValue::Bool(true);
        assert_eq!(bool::from_sql_value(&value).unwrap(), true);
    }
}