//! PostgreSQL backend implementation for BitCraps storage
//!
//! This module provides a PostgreSQL backend as an alternative to SQLite
//! for production deployments requiring better scalability and concurrent access.

#[cfg(feature = "postgres")]
use deadpool_postgres::{Config, Pool, Runtime};
#[cfg(feature = "postgres")]
use tokio_postgres::{NoTls, Error as PgError, Row};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Serialize, Deserialize};
use crate::error::{Error, Result};
use crate::storage::{StorageRecord, StorageError, DatabaseStatistics};

/// PostgreSQL configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostgresConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub database: String,
    pub max_connections: usize,
    pub min_connections: usize,
    pub connection_timeout_seconds: u64,
    pub idle_timeout_seconds: u64,
}

impl Default for PostgresConfig {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 5432,
            user: "bitcraps".to_string(),
            password: "bitcraps".to_string(),
            database: "bitcraps".to_string(),
            max_connections: 20,
            min_connections: 5,
            connection_timeout_seconds: 30,
            idle_timeout_seconds: 600,
        }
    }
}

/// PostgreSQL database backend
#[cfg(feature = "postgres")]
pub struct PostgresBackend {
    pool: Pool,
    config: PostgresConfig,
}

#[cfg(feature = "postgres")]
impl PostgresBackend {
    /// Create new PostgreSQL backend
    pub async fn new(config: PostgresConfig) -> Result<Self> {
        let mut pg_config = Config::new();
        pg_config.host = Some(config.host.clone());
        pg_config.port = Some(config.port);
        pg_config.user = Some(config.user.clone());
        pg_config.password = Some(config.password.clone());
        pg_config.dbname = Some(config.database.clone());
        
        // Connection pool settings
        pg_config.pool = Some(deadpool_postgres::PoolConfig::new(config.max_connections));
        
        let pool = pg_config.create_pool(Some(Runtime::Tokio1), NoTls)
            .map_err(|e| Error::Database(format!("Failed to create PostgreSQL pool: {}", e)))?;

        let backend = Self { pool, config };

        // Initialize schema
        backend.init_schema().await?;

        Ok(backend)
    }

    /// Initialize database schema
    async fn init_schema(&self) -> Result<()> {
        let client = self.pool.get().await
            .map_err(|e| Error::Database(format!("Failed to get connection: {}", e)))?;

        let schema_sql = r#"
            -- Enable UUID extension
            CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
            
            -- Storage records table
            CREATE TABLE IF NOT EXISTS storage_records (
                id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
                collection TEXT NOT NULL,
                key TEXT NOT NULL,
                data BYTEA NOT NULL,
                content_hash TEXT NOT NULL,
                is_compressed BOOLEAN NOT NULL DEFAULT FALSE,
                created_at BIGINT NOT NULL,
                size_bytes BIGINT NOT NULL,
                access_count BIGINT NOT NULL DEFAULT 0,
                last_accessed BIGINT NOT NULL,
                UNIQUE(collection, key)
            );

            -- Indices for performance
            CREATE INDEX IF NOT EXISTS idx_storage_collection_key ON storage_records(collection, key);
            CREATE INDEX IF NOT EXISTS idx_storage_content_hash ON storage_records(content_hash);
            CREATE INDEX IF NOT EXISTS idx_storage_created_at ON storage_records(created_at);
            CREATE INDEX IF NOT EXISTS idx_storage_last_accessed ON storage_records(last_accessed);

            -- Content references for deduplication
            CREATE TABLE IF NOT EXISTS content_references (
                id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
                collection TEXT NOT NULL,
                key TEXT NOT NULL,
                target_key TEXT NOT NULL,
                created_at BIGINT NOT NULL,
                UNIQUE(collection, key)
            );

            CREATE INDEX IF NOT EXISTS idx_content_refs_collection_key ON content_references(collection, key);

            -- Performance metrics
            CREATE TABLE IF NOT EXISTS performance_metrics (
                id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
                metric_name TEXT NOT NULL,
                metric_value DOUBLE PRECISION NOT NULL,
                metric_unit TEXT,
                component TEXT NOT NULL,
                tags JSONB,
                created_at BIGINT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_perf_metrics_name ON performance_metrics(metric_name);
            CREATE INDEX IF NOT EXISTS idx_perf_metrics_component ON performance_metrics(component);
            CREATE INDEX IF NOT EXISTS idx_perf_metrics_created_at ON performance_metrics(created_at);

            -- System health tracking
            CREATE TABLE IF NOT EXISTS system_health (
                id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
                cpu_usage DOUBLE PRECISION,
                memory_usage DOUBLE PRECISION,
                disk_usage DOUBLE PRECISION,
                network_in_bytes BIGINT,
                network_out_bytes BIGINT,
                active_connections INTEGER,
                error_count INTEGER DEFAULT 0,
                created_at BIGINT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_system_health_created_at ON system_health(created_at);
        "#;

        client.batch_execute(schema_sql).await
            .map_err(|e| Error::Database(format!("Failed to initialize schema: {}", e)))?;

        println!("PostgreSQL schema initialized successfully");
        Ok(())
    }

    /// Store a record
    pub async fn store_record(&self, record: &StorageRecord) -> Result<()> {
        let client = self.pool.get().await
            .map_err(|e| Error::Database(format!("Failed to get connection: {}", e)))?;

        let stmt = client.prepare(
            "INSERT INTO storage_records 
             (collection, key, data, content_hash, is_compressed, created_at, size_bytes, access_count, last_accessed)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
             ON CONFLICT (collection, key) DO UPDATE SET
             data = EXCLUDED.data,
             content_hash = EXCLUDED.content_hash,
             is_compressed = EXCLUDED.is_compressed,
             size_bytes = EXCLUDED.size_bytes,
             last_accessed = EXCLUDED.last_accessed"
        ).await
        .map_err(|e| Error::Database(format!("Failed to prepare statement: {}", e)))?;

        client.execute(&stmt, &[
            &record.collection,
            &record.key,
            &record.data,
            &record.content_hash,
            &record.is_compressed,
            &(record.created_at as i64),
            &(record.size_bytes as i64),
            &(record.access_count as i64),
            &(record.last_accessed as i64),
        ]).await
        .map_err(|e| Error::Database(format!("Failed to execute statement: {}", e)))?;

        Ok(())
    }

    /// Load a record
    pub async fn load_record(&self, collection: &str, key: &str) -> Result<Option<StorageRecord>> {
        let client = self.pool.get().await
            .map_err(|e| Error::Database(format!("Failed to get connection: {}", e)))?;

        let stmt = client.prepare(
            "SELECT collection, key, data, content_hash, is_compressed, created_at, size_bytes, access_count, last_accessed
             FROM storage_records WHERE collection = $1 AND key = $2"
        ).await
        .map_err(|e| Error::Database(format!("Failed to prepare statement: {}", e)))?;

        let rows = client.query(&stmt, &[&collection, &key]).await
            .map_err(|e| Error::Database(format!("Failed to execute query: {}", e)))?;

        if let Some(row) = rows.first() {
            let record = StorageRecord {
                collection: row.get(0),
                key: row.get(1),
                data: row.get(2),
                content_hash: row.get(3),
                is_compressed: row.get(4),
                created_at: row.get::<_, i64>(5) as u64,
                size_bytes: row.get::<_, i64>(6) as u64,
                access_count: row.get::<_, i64>(7) as u64,
                last_accessed: row.get::<_, i64>(8) as u64,
            };
            Ok(Some(record))
        } else {
            Ok(None)
        }
    }

    /// Delete a record
    pub async fn delete_record(&self, collection: &str, key: &str) -> Result<bool> {
        let client = self.pool.get().await
            .map_err(|e| Error::Database(format!("Failed to get connection: {}", e)))?;

        let stmt = client.prepare("DELETE FROM storage_records WHERE collection = $1 AND key = $2").await
            .map_err(|e| Error::Database(format!("Failed to prepare statement: {}", e)))?;

        let rows_affected = client.execute(&stmt, &[&collection, &key]).await
            .map_err(|e| Error::Database(format!("Failed to execute statement: {}", e)))?;

        Ok(rows_affected > 0)
    }

    /// List keys in a collection with pagination
    pub async fn list_keys(&self, collection: &str, offset: usize, limit: usize) -> Result<Vec<String>> {
        let client = self.pool.get().await
            .map_err(|e| Error::Database(format!("Failed to get connection: {}", e)))?;

        let stmt = client.prepare(
            "SELECT key FROM storage_records WHERE collection = $1 ORDER BY key LIMIT $2 OFFSET $3"
        ).await
        .map_err(|e| Error::Database(format!("Failed to prepare statement: {}", e)))?;

        let rows = client.query(&stmt, &[&collection, &(limit as i64), &(offset as i64)]).await
            .map_err(|e| Error::Database(format!("Failed to execute query: {}", e)))?;

        let keys: Result<Vec<String>> = rows.iter()
            .map(|row| Ok(row.get(0)))
            .collect();

        keys
    }

    /// Update access statistics
    pub async fn update_access_stats(&self, collection: &str, key: &str) -> Result<()> {
        let client = self.pool.get().await
            .map_err(|e| Error::Database(format!("Failed to get connection: {}", e)))?;

        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
        let stmt = client.prepare(
            "UPDATE storage_records SET access_count = access_count + 1, last_accessed = $1 
             WHERE collection = $2 AND key = $3"
        ).await
        .map_err(|e| Error::Database(format!("Failed to prepare statement: {}", e)))?;

        client.execute(&stmt, &[&now, &collection, &key]).await
            .map_err(|e| Error::Database(format!("Failed to execute statement: {}", e)))?;

        Ok(())
    }

    /// Find record by content hash
    pub async fn find_by_content_hash(&self, content_hash: &str) -> Result<Option<String>> {
        let client = self.pool.get().await
            .map_err(|e| Error::Database(format!("Failed to get connection: {}", e)))?;

        let stmt = client.prepare("SELECT key FROM storage_records WHERE content_hash = $1 LIMIT 1").await
            .map_err(|e| Error::Database(format!("Failed to prepare statement: {}", e)))?;

        let rows = client.query(&stmt, &[&content_hash]).await
            .map_err(|e| Error::Database(format!("Failed to execute query: {}", e)))?;

        if let Some(row) = rows.first() {
            Ok(Some(row.get(0)))
        } else {
            Ok(None)
        }
    }

    /// Create content reference for deduplication
    pub async fn create_reference(&self, collection: &str, key: &str, target_key: &str) -> Result<()> {
        let client = self.pool.get().await
            .map_err(|e| Error::Database(format!("Failed to get connection: {}", e)))?;

        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
        let stmt = client.prepare(
            "INSERT INTO content_references (collection, key, target_key, created_at) 
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (collection, key) DO UPDATE SET target_key = EXCLUDED.target_key"
        ).await
        .map_err(|e| Error::Database(format!("Failed to prepare statement: {}", e)))?;

        client.execute(&stmt, &[&collection, &key, &target_key, &now]).await
            .map_err(|e| Error::Database(format!("Failed to execute statement: {}", e)))?;

        Ok(())
    }

    /// Optimize database performance
    pub async fn optimize(&self) -> Result<()> {
        let client = self.pool.get().await
            .map_err(|e| Error::Database(format!("Failed to get connection: {}", e)))?;

        // Run PostgreSQL maintenance commands
        let maintenance_sql = r#"
            -- Update table statistics
            ANALYZE storage_records;
            ANALYZE content_references;
            ANALYZE performance_metrics;
            ANALYZE system_health;
            
            -- Reindex if needed (commented out as it can be expensive)
            -- REINDEX TABLE storage_records;
        "#;

        client.batch_execute(maintenance_sql).await
            .map_err(|e| Error::Database(format!("Failed to optimize database: {}", e)))?;

        Ok(())
    }

    /// Get database statistics
    pub async fn get_statistics(&self) -> Result<DatabaseStatistics> {
        let client = self.pool.get().await
            .map_err(|e| Error::Database(format!("Failed to get connection: {}", e)))?;

        // Get record count
        let count_row = client.query_one("SELECT COUNT(*) FROM storage_records", &[]).await
            .map_err(|e| Error::Database(format!("Failed to get record count: {}", e)))?;
        let total_records: i64 = count_row.get(0);

        // Get database size (approximate)
        let size_query = r#"
            SELECT pg_size_pretty(pg_total_relation_size('storage_records')) as table_size,
                   pg_size_pretty(pg_indexes_size('storage_records')) as index_size
        "#;
        
        let size_row = client.query_one(size_query, &[]).await.unwrap_or_else(|_| {
            // Fallback if pg_size_pretty is not available
            client.query_one("SELECT '0 bytes' as table_size, '0 bytes' as index_size", &[])
                .unwrap()
        });

        // Parse sizes (simplified - in production would parse the formatted strings)
        let database_size_bytes = 0u64; // Would parse from pg_size_pretty result
        let index_size_bytes = 0u64;   // Would parse from pg_size_pretty result

        Ok(DatabaseStatistics {
            total_records: total_records as u64,
            database_size_bytes,
            index_size_bytes,
            fragmentation_percent: 0.0, // PostgreSQL handles fragmentation differently
        })
    }

    /// Get connection pool statistics
    pub fn get_pool_statistics(&self) -> PoolStatistics {
        let status = self.pool.status();
        PoolStatistics {
            size: status.size,
            available: status.available,
            waiting: status.waiting,
            max_size: status.max_size,
        }
    }
}

/// PostgreSQL connection pool statistics
#[derive(Debug, Clone)]
pub struct PoolStatistics {
    pub size: usize,
    pub available: usize,
    pub waiting: usize,
    pub max_size: usize,
}

// Stub implementation when postgres feature is not enabled
#[cfg(not(feature = "postgres"))]
pub struct PostgresBackend;

#[cfg(not(feature = "postgres"))]
impl PostgresBackend {
    pub async fn new(_config: PostgresConfig) -> Result<Self> {
        Err(Error::Database("PostgreSQL support not compiled in. Enable 'postgres' feature.".to_string()))
    }

    pub async fn store_record(&self, _record: &StorageRecord) -> Result<()> {
        Err(Error::Database("PostgreSQL support not available".to_string()))
    }

    pub async fn load_record(&self, _collection: &str, _key: &str) -> Result<Option<StorageRecord>> {
        Err(Error::Database("PostgreSQL support not available".to_string()))
    }

    pub async fn delete_record(&self, _collection: &str, _key: &str) -> Result<bool> {
        Err(Error::Database("PostgreSQL support not available".to_string()))
    }

    pub async fn list_keys(&self, _collection: &str, _offset: usize, _limit: usize) -> Result<Vec<String>> {
        Err(Error::Database("PostgreSQL support not available".to_string()))
    }

    pub async fn update_access_stats(&self, _collection: &str, _key: &str) -> Result<()> {
        Err(Error::Database("PostgreSQL support not available".to_string()))
    }

    pub async fn find_by_content_hash(&self, _content_hash: &str) -> Result<Option<String>> {
        Err(Error::Database("PostgreSQL support not available".to_string()))
    }

    pub async fn create_reference(&self, _collection: &str, _key: &str, _target_key: &str) -> Result<()> {
        Err(Error::Database("PostgreSQL support not available".to_string()))
    }

    pub async fn optimize(&self) -> Result<()> {
        Err(Error::Database("PostgreSQL support not available".to_string()))
    }

    pub async fn get_statistics(&self) -> Result<DatabaseStatistics> {
        Err(Error::Database("PostgreSQL support not available".to_string()))
    }

    pub fn get_pool_statistics(&self) -> PoolStatistics {
        PoolStatistics { size: 0, available: 0, waiting: 0, max_size: 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    #[cfg(feature = "postgres")]
    async fn test_postgres_config() {
        let config = PostgresConfig::default();
        assert_eq!(config.host, "localhost");
        assert_eq!(config.port, 5432);
        assert_eq!(config.max_connections, 20);
    }

    #[test]
    #[cfg(not(feature = "postgres"))]
    fn test_postgres_not_available() {
        // Test that the stub implementation correctly reports unavailability
        let config = PostgresConfig::default();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        
        let result = runtime.block_on(async {
            PostgresBackend::new(config).await
        });
        
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not compiled in"));
    }
}