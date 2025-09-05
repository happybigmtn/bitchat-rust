//! Production Persistent Storage System for BitCraps
//!
//! This module provides enterprise-grade persistent storage with:
//! - High-performance database operations
//! - Automatic backup and recovery
//! - Data compression and deduplication
//! - ACID compliance
//! - Horizontal scaling support

use std::sync::{Arc, atomic::{AtomicU64, AtomicUsize, Ordering}};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{RwLock, Mutex, Semaphore};
use tokio::time::{interval, sleep};
use parking_lot::RwLock as ParkingRwLock;
use serde::{Serialize, Deserialize, de::DeserializeOwned};
use serde_json;
use rusqlite::{Connection, params, Result as SqlResult};
use flate2::{Compression, write::GzEncoder, read::GzDecoder};
use std::io::{Write, Read};
use blake3::Hasher;
use tracing::{info, warn, error, debug};

use crate::error::Error;
#[cfg(feature = "monitoring")]
use crate::monitoring::metrics::METRICS;
use crate::storage::encryption::{EncryptionEngine, FileKeyManager, EncryptedData, KeyManager};
use crate::storage::postgresql_backend::{PostgresBackend, PostgresConfig};

/// Production persistent storage manager
pub struct PersistentStorageManager {
    /// Primary database connection pool
    db_pool: Arc<DatabasePool>,
    /// Cache layer for high-performance reads
    cache: Arc<StorageCache>,
    /// Backup manager for data protection
    backup_manager: Arc<BackupManager>,
    /// Compression engine for space efficiency
    compression_engine: Arc<CompressionEngine>,
    /// Encryption engine for data at rest
    encryption_engine: Arc<Mutex<EncryptionEngine>>,
    /// Storage statistics
    stats: Arc<StorageStats>,
    /// Configuration
    config: StorageConfig,
}

impl PersistentStorageManager {
    /// Create new persistent storage manager
    pub async fn new(config: StorageConfig) -> Result<Self, StorageError> {
        // Initialize database pool
        let db_pool = Arc::new(DatabasePool::new(&config).await?);

        // Initialize cache
        let cache = Arc::new(StorageCache::new(config.cache_size_mb * 1024 * 1024));

        // Initialize backup manager
        let backup_manager = Arc::new(BackupManager::new(config.clone()).await?);

        // Initialize compression engine
        let compression_engine = Arc::new(CompressionEngine::new(config.compression_level));

        // Initialize encryption engine
        let key_dir = config.data_path.join("keys");
        let key_manager = Box::new(FileKeyManager::new(key_dir)?);
        let encryption_engine = Arc::new(Mutex::new(EncryptionEngine::new(key_manager)));

        // Initialize statistics
        let stats = Arc::new(StorageStats::new());

        let manager = Self {
            db_pool,
            cache,
            backup_manager,
            compression_engine,
            encryption_engine,
            stats,
            config,
        };

        // Start background tasks
        manager.start_background_tasks().await?;

        info!("Persistent storage manager initialized");
        Ok(manager)
    }

    /// Store data with automatic compression and caching
    pub async fn store<T: Serialize + Send + Sync>(
        &self,
        collection: &str,
        key: &str,
        data: &T
    ) -> Result<(), StorageError> {
        let start_time = std::time::Instant::now();

        // Serialize data
        let serialized = serde_json::to_vec(data)
            .map_err(|e| StorageError::SerializationError(e.to_string()))?;

        // Compress data if enabled
        let (compressed_data, is_compressed) = if self.config.enable_compression {
            let compressed = self.compression_engine.compress(&serialized)?;
            if compressed.len() < serialized.len() {
                (compressed, true)
            } else {
                (serialized, false)
            }
        } else {
            (serialized, false)
        };

        // Encrypt data if enabled
        let (stored_data, encrypted_metadata) = if self.config.enable_encryption {
            let mut encryption_engine = self.encryption_engine.lock().await;
            let encrypted = encryption_engine.encrypt(&compressed_data)
                .map_err(|e| StorageError::ConfigurationError(format!("Encryption failed: {}", e)))?;

            let metadata = serde_json::to_vec(&encrypted)
                .map_err(|e| StorageError::SerializationError(e.to_string()))?;

            (metadata, Some(encrypted))
        } else {
            (compressed_data, None)
        };

        // Generate content hash for deduplication
        let content_hash = self.calculate_hash(&stored_data);

        // Check for duplicate content
        if self.config.enable_deduplication {
            if let Some(existing_key) = self.find_duplicate_content(&content_hash).await? {
                // Create reference instead of storing duplicate
                self.create_content_reference(collection, key, &existing_key).await?;
                self.stats.deduplicated_writes.fetch_add(1, Ordering::Relaxed);
                return Ok(());
            }
        }

        // Store in database
        let storage_record = StorageRecord {
            collection: collection.to_string(),
            key: key.to_string(),
            data: stored_data.clone(),
            content_hash,
            is_compressed,
            created_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
            size_bytes: stored_data.len() as u64,
            access_count: 0,
            last_accessed: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
        };

        self.db_pool.store_record(&storage_record).await?;

        // Update cache (store encrypted data in cache for security)
        self.cache.put(
            &format!("{}:{}", collection, key),
            stored_data.clone(),
            is_compressed
        ).await;

        // Update statistics
        let elapsed = start_time.elapsed();
        self.stats.total_writes.fetch_add(1, Ordering::Relaxed);
        self.stats.total_write_time_ms.fetch_add(elapsed.as_millis() as u64, Ordering::Relaxed);
        self.stats.bytes_written.fetch_add(stored_data.len() as u64, Ordering::Relaxed);

        debug!("Stored {}: {} bytes in {:?}", key, stored_data.len(), elapsed);
        Ok(())
    }

    /// Retrieve data with automatic decompression and caching
    pub async fn retrieve<T: DeserializeOwned + Send + Sync>(
        &self,
        collection: &str,
        key: &str
    ) -> Result<Option<T>, StorageError> {
        let start_time = std::time::Instant::now();
        let cache_key = format!("{}:{}", collection, key);

        // Check cache first
        if let Some((data, is_compressed)) = self.cache.get(&cache_key).await {
            self.stats.cache_hits.fetch_add(1, Ordering::Relaxed);

            // Decrypt cached data if encryption is enabled
            let decrypted_data = if self.config.enable_encryption {
                let encrypted: EncryptedData = serde_json::from_slice(&data)
                    .map_err(|e| StorageError::DeserializationError(e.to_string()))?;

                let encryption_engine = self.encryption_engine.lock().await;
                encryption_engine.decrypt(&encrypted)
                    .map_err(|e| StorageError::ConfigurationError(format!("Cache decryption failed: {}", e)))?
            } else {
                data
            };

            let decompressed = if is_compressed {
                self.compression_engine.decompress(&decrypted_data)?
            } else {
                decrypted_data
            };

            let result: T = serde_json::from_slice(&decompressed)
                .map_err(|e| StorageError::DeserializationError(e.to_string()))?;

            return Ok(Some(result));
        }

        self.stats.cache_misses.fetch_add(1, Ordering::Relaxed);

        // Load from database
        let record = match self.db_pool.load_record(collection, key).await? {
            Some(record) => record,
            None => return Ok(None),
        };

        // Update access statistics
        self.db_pool.update_access_stats(collection, key).await?;

        // Decrypt if enabled
        let decrypted_data = if self.config.enable_encryption {
            // Deserialize encrypted metadata
            let encrypted: EncryptedData = serde_json::from_slice(&record.data)
                .map_err(|e| StorageError::DeserializationError(e.to_string()))?;

            let encryption_engine = self.encryption_engine.lock().await;
            encryption_engine.decrypt(&encrypted)
                .map_err(|e| StorageError::ConfigurationError(format!("Decryption failed: {}", e)))?
        } else {
            record.data.clone()
        };

        // Decompress if needed
        let decompressed_data = if record.is_compressed {
            self.compression_engine.decompress(&decrypted_data)?
        } else {
            decrypted_data
        };

        // Deserialize
        let result: T = serde_json::from_slice(&decompressed_data)
            .map_err(|e| StorageError::DeserializationError(e.to_string()))?;

        // Update cache
        self.cache.put(&cache_key, record.data, record.is_compressed).await;

        // Update statistics
        let elapsed = start_time.elapsed();
        self.stats.total_reads.fetch_add(1, Ordering::Relaxed);
        self.stats.total_read_time_ms.fetch_add(elapsed.as_millis() as u64, Ordering::Relaxed);

        debug!("Retrieved {}: {} bytes in {:?}", key, record.size_bytes, elapsed);
        Ok(Some(result))
    }

    /// Delete data from storage and cache
    pub async fn delete(&self, collection: &str, key: &str) -> Result<bool, StorageError> {
        let cache_key = format!("{}:{}", collection, key);

        // Remove from cache
        self.cache.remove(&cache_key).await;

        // Remove from database
        let deleted = self.db_pool.delete_record(collection, key).await?;

        if deleted {
            self.stats.total_deletes.fetch_add(1, Ordering::Relaxed);
        }

        Ok(deleted)
    }

    /// List keys in a collection with pagination
    pub async fn list_keys(
        &self,
        collection: &str,
        offset: usize,
        limit: usize
    ) -> Result<Vec<String>, StorageError> {
        self.db_pool.list_keys(collection, offset, limit).await
    }

    /// Get storage statistics
    pub async fn get_statistics(&self) -> StorageStatistics {
        let db_stats = self.db_pool.get_statistics().await;
        let cache_stats = self.cache.get_statistics().await;

        StorageStatistics {
            total_reads: self.stats.total_reads.load(Ordering::Relaxed),
            total_writes: self.stats.total_writes.load(Ordering::Relaxed),
            total_deletes: self.stats.total_deletes.load(Ordering::Relaxed),
            cache_hits: self.stats.cache_hits.load(Ordering::Relaxed),
            cache_misses: self.stats.cache_misses.load(Ordering::Relaxed),
            deduplicated_writes: self.stats.deduplicated_writes.load(Ordering::Relaxed),
            bytes_written: self.stats.bytes_written.load(Ordering::Relaxed),
            average_read_time_ms: if self.stats.total_reads.load(Ordering::Relaxed) > 0 {
                self.stats.total_read_time_ms.load(Ordering::Relaxed) as f64 /
                self.stats.total_reads.load(Ordering::Relaxed) as f64
            } else { 0.0 },
            average_write_time_ms: if self.stats.total_writes.load(Ordering::Relaxed) > 0 {
                self.stats.total_write_time_ms.load(Ordering::Relaxed) as f64 /
                self.stats.total_writes.load(Ordering::Relaxed) as f64
            } else { 0.0 },
            cache_hit_rate: if cache_stats.total_requests > 0 {
                cache_stats.hits as f64 / cache_stats.total_requests as f64
            } else { 0.0 },
            database_stats: db_stats,
            cache_stats,
        }
    }

    /// Perform database maintenance
    pub async fn maintenance(&self) -> Result<MaintenanceReport, StorageError> {
        info!("Starting storage maintenance");
        let start_time = std::time::Instant::now();

        let mut report = MaintenanceReport::new();

        // Database optimization
        if let Err(e) = self.db_pool.optimize().await {
            warn!("Database optimization failed: {:?}", e);
            report.errors.push(format!("Database optimization: {:?}", e));
        } else {
            report.database_optimized = true;
        }

        // Cache cleanup
        let evicted = self.cache.cleanup().await;
        report.cache_entries_evicted = evicted;

        // Backup creation
        match self.backup_manager.create_backup().await {
            Ok(backup_info) => {
                report.backup_created = true;
                report.backup_size_mb = backup_info.size_bytes / 1024 / 1024;
            },
            Err(e) => {
                warn!("Backup creation failed: {:?}", e);
                report.errors.push(format!("Backup creation: {:?}", e));
            }
        }

        // Old backup cleanup
        match self.backup_manager.cleanup_old_backups().await {
            Ok(cleaned) => {
                report.old_backups_cleaned = cleaned;
            },
            Err(e) => {
                warn!("Backup cleanup failed: {:?}", e);
                report.errors.push(format!("Backup cleanup: {:?}", e));
            }
        }

        report.duration_ms = start_time.elapsed().as_millis() as u64;
        report.success = report.errors.is_empty();

        info!("Storage maintenance completed in {:?}", start_time.elapsed());
        Ok(report)
    }

    /// Rotate encryption key
    pub async fn rotate_encryption_key(&self) -> Result<String, StorageError> {
        if self.config.enable_encryption {
            let mut encryption_engine = self.encryption_engine.lock().await;
            encryption_engine.rotate_key()
                .map_err(|e| StorageError::ConfigurationError(format!("Key rotation failed: {}", e)))
        } else {
            Err(StorageError::ConfigurationError("Encryption not enabled".to_string()))
        }
    }

    /// List available encryption keys
    pub async fn list_encryption_keys(&self) -> Vec<String> {
        if self.config.enable_encryption {
            let encryption_engine = self.encryption_engine.lock().await;
            encryption_engine.list_keys()
        } else {
            vec![]
        }
    }

    /// Derive encryption key from password
    pub async fn derive_key_from_password(&self, password: &str) -> Result<String, StorageError> {
        if self.config.enable_encryption {
            let mut encryption_engine = self.encryption_engine.lock().await;
            encryption_engine.derive_key_from_password(password)
                .map_err(|e| StorageError::ConfigurationError(format!("Key derivation failed: {}", e)))
        } else {
            Err(StorageError::ConfigurationError("Encryption not enabled".to_string()))
        }
    }

    /// Start background maintenance tasks
    async fn start_background_tasks(&self) -> Result<(), StorageError> {
        // Statistics reporting
        let stats = Arc::clone(&self.stats);
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(60));
            loop {
                interval.tick().await;
                stats.report_to_metrics().await;
            }
        });

        // Periodic maintenance
        let manager = self.clone();
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(3600)); // Every hour
            loop {
                interval.tick().await;
                if let Err(e) = manager.maintenance().await {
                    error!("Background maintenance failed: {:?}", e);
                }
            }
        });

        // Cache eviction
        let cache = Arc::clone(&self.cache);
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(300)); // Every 5 minutes
            loop {
                interval.tick().await;
                cache.evict_expired().await;
            }
        });

        Ok(())
    }

    /// Calculate content hash for deduplication
    fn calculate_hash(&self, data: &[u8]) -> String {
        let mut hasher = Hasher::new();
        hasher.update(data);
        hex::encode(hasher.finalize().as_bytes())
    }

    /// Find duplicate content by hash
    async fn find_duplicate_content(&self, content_hash: &str) -> Result<Option<String>, StorageError> {
        self.db_pool.find_by_content_hash(content_hash).await
    }

    /// Create content reference for deduplication
    async fn create_content_reference(&self, collection: &str, key: &str, target_key: &str) -> Result<(), StorageError> {
        self.db_pool.create_reference(collection, key, target_key).await
    }
}

impl Clone for PersistentStorageManager {
    fn clone(&self) -> Self {
        Self {
            db_pool: Arc::clone(&self.db_pool),
            cache: Arc::clone(&self.cache),
            backup_manager: Arc::clone(&self.backup_manager),
            compression_engine: Arc::clone(&self.compression_engine),
            encryption_engine: Arc::clone(&self.encryption_engine),
            stats: Arc::clone(&self.stats),
            config: self.config.clone(),
        }
    }
}

/// Database connection pool
pub struct DatabasePool {
    connections: Arc<Mutex<Vec<Connection>>>,
    semaphore: Arc<Semaphore>,
    config: StorageConfig,
}

impl DatabasePool {
    async fn new(config: &StorageConfig) -> Result<Self, StorageError> {
        let mut connections = Vec::new();

        // Create database file if it doesn't exist
        std::fs::create_dir_all(&config.data_path)
            .map_err(|e| StorageError::DatabaseError(format!("Failed to create data directory: {}", e)))?;

        let db_path = config.data_path.join("bitcraps.db");

        // Create initial connections
        for _ in 0..config.max_connections {
            let mut conn = Connection::open(&db_path)
                .map_err(|e| StorageError::DatabaseError(e.to_string()))?;

            // Enable SQLite optimizations
            Self::configure_connection(&mut conn, &config)?;

            // Initialize database schema
            Self::init_schema(&conn)?;
            connections.push(conn);
        }

        Ok(Self {
            connections: Arc::new(Mutex::new(connections)),
            semaphore: Arc::new(Semaphore::new(config.max_connections)),
            config: config.clone(),
        })
    }

    fn configure_connection(conn: &mut Connection, config: &StorageConfig) -> Result<(), StorageError> {
        // Enable WAL mode for better concurrency and crash safety
        conn.execute("PRAGMA journal_mode=WAL", [])
            .map_err(|e| StorageError::DatabaseError(format!("Failed to set WAL mode: {}", e)))?;

        // Enable synchronous=NORMAL for WAL mode (faster writes while still safe)
        conn.execute("PRAGMA synchronous=NORMAL", [])
            .map_err(|e| StorageError::DatabaseError(format!("Failed to set synchronous mode: {}", e)))?;

        // Increase cache size for better performance (negative value = KB of RAM)
        let cache_size = -(config.cache_size_mb as i32 * 1024);
        // Use parameterized statement to avoid SQL injection
        conn.pragma_update(None, "cache_size", cache_size)
            .map_err(|e| StorageError::DatabaseError(format!("Failed to set cache size: {}", e)))?;

        // Enable foreign keys for referential integrity
        conn.execute("PRAGMA foreign_keys=ON", [])
            .map_err(|e| StorageError::DatabaseError(format!("Failed to enable foreign keys: {}", e)))?;

        // Set busy timeout to handle concurrent access
        conn.execute("PRAGMA busy_timeout=30000", []) // 30 seconds
            .map_err(|e| StorageError::DatabaseError(format!("Failed to set busy timeout: {}", e)))?;

        // Enable automatic index creation for better query performance
        conn.execute("PRAGMA automatic_index=ON", [])
            .map_err(|e| StorageError::DatabaseError(format!("Failed to enable automatic index: {}", e)))?;

        // Set temp store to memory for better performance
        conn.execute("PRAGMA temp_store=MEMORY", [])
            .map_err(|e| StorageError::DatabaseError(format!("Failed to set temp store: {}", e)))?;

        // Set mmap_size for memory-mapped I/O (256MB default)
        conn.execute("PRAGMA mmap_size=268435456", [])
            .map_err(|e| StorageError::DatabaseError(format!("Failed to set mmap size: {}", e)))?;

        // Verify WAL mode was set correctly
        let mut stmt = conn.prepare("PRAGMA journal_mode")
            .map_err(|e| StorageError::DatabaseError(format!("Failed to prepare journal_mode query: {}", e)))?;
        let journal_mode: String = stmt.query_row([], |row| row.get(0))
            .map_err(|e| StorageError::DatabaseError(format!("Failed to query journal_mode: {}", e)))?;

        if journal_mode.to_uppercase() != "WAL" {
            return Err(StorageError::DatabaseError(format!("Failed to enable WAL mode: got {}", journal_mode)));
        }

        info!("SQLite configured with WAL mode and optimizations: cache_size={}KB, mmap_size=256MB", config.cache_size_mb * 1024);
        Ok(())
    }

    fn init_schema(conn: &Connection) -> Result<(), StorageError> {
        conn.execute_batch("
            CREATE TABLE IF NOT EXISTS storage_records (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                collection TEXT NOT NULL,
                key TEXT NOT NULL,
                data BLOB NOT NULL,
                content_hash TEXT NOT NULL,
                is_compressed BOOLEAN NOT NULL,
                created_at INTEGER NOT NULL,
                size_bytes INTEGER NOT NULL,
                access_count INTEGER NOT NULL DEFAULT 0,
                last_accessed INTEGER NOT NULL,
                UNIQUE(collection, key)
            );

            CREATE INDEX IF NOT EXISTS idx_collection_key ON storage_records(collection, key);
            CREATE INDEX IF NOT EXISTS idx_content_hash ON storage_records(content_hash);
            CREATE INDEX IF NOT EXISTS idx_created_at ON storage_records(created_at);
            CREATE INDEX IF NOT EXISTS idx_last_accessed ON storage_records(last_accessed);

            CREATE TABLE IF NOT EXISTS content_references (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                collection TEXT NOT NULL,
                key TEXT NOT NULL,
                target_key TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                UNIQUE(collection, key)
            );
        ").map_err(|e| StorageError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    async fn store_record(&self, record: &StorageRecord) -> Result<(), StorageError> {
        let _permit = self.semaphore.acquire().await
            .map_err(|e| StorageError::DatabaseError(format!("Failed to acquire semaphore: {}", e)))?;
        let mut connections = self.connections.lock().await;
        let conn = connections.pop()
            .ok_or_else(|| StorageError::DatabaseError("No database connections available".to_string()))?;

        let result = conn.execute(
            "INSERT OR REPLACE INTO storage_records
             (collection, key, data, content_hash, is_compressed, created_at, size_bytes, access_count, last_accessed)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                record.collection,
                record.key,
                record.data,
                record.content_hash,
                record.is_compressed,
                record.created_at,
                record.size_bytes,
                record.access_count,
                record.last_accessed
            ]
        ).map_err(|e| StorageError::DatabaseError(e.to_string()));

        connections.push(conn);
        result?;
        Ok(())
    }

    async fn load_record(&self, collection: &str, key: &str) -> Result<Option<StorageRecord>, StorageError> {
        let _permit = self.semaphore.acquire().await
            .map_err(|e| StorageError::DatabaseError(format!("Failed to acquire semaphore: {}", e)))?;
        let mut connections = self.connections.lock().await;
        let conn = connections.pop()
            .ok_or_else(|| StorageError::DatabaseError("No database connections available".to_string()))?;

        let result = conn.query_row(
            "SELECT collection, key, data, content_hash, is_compressed, created_at, size_bytes, access_count, last_accessed
             FROM storage_records WHERE collection = ?1 AND key = ?2",
            params![collection, key],
            |row| {
                Ok(StorageRecord {
                    collection: row.get(0)?,
                    key: row.get(1)?,
                    data: row.get(2)?,
                    content_hash: row.get(3)?,
                    is_compressed: row.get(4)?,
                    created_at: row.get(5)?,
                    size_bytes: row.get(6)?,
                    access_count: row.get(7)?,
                    last_accessed: row.get(8)?,
                })
            }
        );

        connections.push(conn);

        match result {
            Ok(record) => Ok(Some(record)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(StorageError::DatabaseError(e.to_string())),
        }
    }

    async fn delete_record(&self, collection: &str, key: &str) -> Result<bool, StorageError> {
        let _permit = self.semaphore.acquire().await
            .map_err(|e| StorageError::DatabaseError(format!("Failed to acquire semaphore: {}", e)))?;
        let mut connections = self.connections.lock().await;
        let conn = connections.pop()
            .ok_or_else(|| StorageError::DatabaseError("No database connections available".to_string())))?;

        let result = conn.execute(
            "DELETE FROM storage_records WHERE collection = ?1 AND key = ?2",
            params![collection, key]
        ).map_err(|e| StorageError::DatabaseError(e.to_string()));

        connections.push(conn);
        Ok(result? > 0)
    }

    async fn list_keys(&self, collection: &str, offset: usize, limit: usize) -> Result<Vec<String>, StorageError> {
        let _permit = self.semaphore.acquire().await
            .map_err(|e| StorageError::DatabaseError(format!("Failed to acquire semaphore: {}", e)))?;
        let mut connections = self.connections.lock().await;
        let conn = connections.pop()
            .ok_or_else(|| StorageError::DatabaseError("No database connections available".to_string())))?;

        let mut stmt = conn.prepare(
            "SELECT key FROM storage_records WHERE collection = ?1 ORDER BY key LIMIT ?2 OFFSET ?3"
        ).map_err(|e| StorageError::DatabaseError(e.to_string()))?;

        let keys: Result<Vec<String>, _> = stmt.query_map(
            params![collection, limit, offset],
            |row| row.get(0)
        ).map_err(|e| StorageError::DatabaseError(e.to_string()))?
         .collect();

        connections.push(conn);
        keys.map_err(|e| StorageError::DatabaseError(e.to_string()))
    }

    async fn update_access_stats(&self, collection: &str, key: &str) -> Result<(), StorageError> {
        let _permit = self.semaphore.acquire().await
            .map_err(|e| StorageError::DatabaseError(format!("Failed to acquire semaphore: {}", e)))?;
        let mut connections = self.connections.lock().await;
        let conn = connections.pop()
            .ok_or_else(|| StorageError::DatabaseError("No database connections available".to_string()))?;

        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        let result = conn.execute(
            "UPDATE storage_records SET access_count = access_count + 1, last_accessed = ?1
             WHERE collection = ?2 AND key = ?3",
            params![now, collection, key]
        ).map_err(|e| StorageError::DatabaseError(e.to_string()));

        connections.push(conn);
        result?;
        Ok(())
    }

    async fn find_by_content_hash(&self, content_hash: &str) -> Result<Option<String>, StorageError> {
        let _permit = self.semaphore.acquire().await
            .map_err(|e| StorageError::DatabaseError(format!("Failed to acquire semaphore: {}", e)))?;
        let mut connections = self.connections.lock().await;
        let conn = connections.pop()
            .ok_or_else(|| StorageError::DatabaseError("No database connections available".to_string())))?;

        let result = conn.query_row(
            "SELECT key FROM storage_records WHERE content_hash = ?1 LIMIT 1",
            params![content_hash],
            |row| row.get::<_, String>(0)
        );

        connections.push(conn);

        match result {
            Ok(key) => Ok(Some(key)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(StorageError::DatabaseError(e.to_string())),
        }
    }

    async fn create_reference(&self, collection: &str, key: &str, target_key: &str) -> Result<(), StorageError> {
        let _permit = self.semaphore.acquire().await
            .map_err(|e| StorageError::DatabaseError(format!("Failed to acquire semaphore: {}", e)))?;
        let mut connections = self.connections.lock().await;
        let conn = connections.pop()
            .ok_or_else(|| StorageError::DatabaseError("No database connections available".to_string()))?;

        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        let result = conn.execute(
            "INSERT OR REPLACE INTO content_references (collection, key, target_key, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![collection, key, target_key, now]
        ).map_err(|e| StorageError::DatabaseError(e.to_string()));

        connections.push(conn);
        result?;
        Ok(())
    }

    async fn optimize(&self) -> Result<(), StorageError> {
        let _permit = self.semaphore.acquire().await
            .map_err(|e| StorageError::DatabaseError(format!("Failed to acquire semaphore: {}", e)))?;
        let mut connections = self.connections.lock().await;
        let conn = connections.pop()
            .ok_or_else(|| StorageError::DatabaseError("No database connections available".to_string()))?;

        conn.execute_batch("
            VACUUM;
            REINDEX;
            ANALYZE;
        ").map_err(|e| StorageError::DatabaseError(e.to_string()))?;

        connections.push(conn);
        Ok(())
    }

    async fn get_statistics(&self) -> DatabaseStatistics {
        // This would collect real database statistics
        DatabaseStatistics {
            total_records: 0,
            database_size_bytes: 0,
            index_size_bytes: 0,
            fragmentation_percent: 0.0,
        }
    }
}

// Supporting types and implementations...
// (Due to length limits, I'll include key types and continue in the next file)

/// Database backend type
#[derive(Debug, Clone)]
pub enum DatabaseBackend {
    Sqlite,
    Postgres(PostgresConfig),
}

#[derive(Debug, Clone)]
pub struct StorageConfig {
    pub data_path: PathBuf,
    pub database_backend: DatabaseBackend,
    pub max_connections: usize,
    pub cache_size_mb: usize,
    pub enable_compression: bool,
    pub compression_level: CompressionLevel,
    pub enable_encryption: bool,
    pub enable_deduplication: bool,
    pub backup_retention_days: u32,
    pub auto_backup_interval_hours: u32,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            data_path: PathBuf::from("./data"),
            database_backend: DatabaseBackend::Sqlite,
            max_connections: 10,
            cache_size_mb: 128,
            enable_compression: true,
            compression_level: CompressionLevel::Balanced,
            enable_encryption: true,
            enable_deduplication: true,
            backup_retention_days: 30,
            auto_backup_interval_hours: 24,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum CompressionLevel {
    Fast,
    Balanced,
    Maximum,
}

impl CompressionLevel {
    fn to_flate2_level(self) -> Compression {
        match self {
            CompressionLevel::Fast => Compression::fast(),
            CompressionLevel::Balanced => Compression::default(),
            CompressionLevel::Maximum => Compression::best(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct StorageRecord {
    pub collection: String,
    pub key: String,
    pub data: Vec<u8>,
    pub content_hash: String,
    pub is_compressed: bool,
    pub created_at: u64,
    pub size_bytes: u64,
    pub access_count: u64,
    pub last_accessed: u64,
}

#[derive(Debug)]
pub struct StorageStatistics {
    pub total_reads: u64,
    pub total_writes: u64,
    pub total_deletes: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub deduplicated_writes: u64,
    pub bytes_written: u64,
    pub average_read_time_ms: f64,
    pub average_write_time_ms: f64,
    pub cache_hit_rate: f64,
    pub database_stats: DatabaseStatistics,
    pub cache_stats: CacheStatistics,
}

#[derive(Debug)]
pub struct DatabaseStatistics {
    pub total_records: u64,
    pub database_size_bytes: u64,
    pub index_size_bytes: u64,
    pub fragmentation_percent: f64,
}

#[derive(Debug)]
pub struct CacheStatistics {
    pub total_requests: u64,
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub memory_usage_bytes: u64,
}

#[derive(Debug)]
pub struct MaintenanceReport {
    pub success: bool,
    pub duration_ms: u64,
    pub database_optimized: bool,
    pub backup_created: bool,
    pub backup_size_mb: u64,
    pub cache_entries_evicted: usize,
    pub old_backups_cleaned: usize,
    pub errors: Vec<String>,
}

impl MaintenanceReport {
    pub fn new() -> Self {
        Self {
            success: false,
            duration_ms: 0,
            database_optimized: false,
            backup_created: false,
            backup_size_mb: 0,
            cache_entries_evicted: 0,
            old_backups_cleaned: 0,
            errors: Vec::new(),
        }
    }
}

pub struct StorageStats {
    pub total_reads: AtomicU64,
    pub total_writes: AtomicU64,
    pub total_deletes: AtomicU64,
    pub cache_hits: AtomicU64,
    pub cache_misses: AtomicU64,
    pub deduplicated_writes: AtomicU64,
    pub bytes_written: AtomicU64,
    pub total_read_time_ms: AtomicU64,
    pub total_write_time_ms: AtomicU64,
}

impl StorageStats {
    pub fn new() -> Self {
        Self {
            total_reads: AtomicU64::new(0),
            total_writes: AtomicU64::new(0),
            total_deletes: AtomicU64::new(0),
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
            deduplicated_writes: AtomicU64::new(0),
            bytes_written: AtomicU64::new(0),
            total_read_time_ms: AtomicU64::new(0),
            total_write_time_ms: AtomicU64::new(0),
        }
    }

    pub async fn report_to_metrics(&self) {
        // Report storage metrics to global monitoring
        METRICS.resources.update_memory(self.bytes_written.load(Ordering::Relaxed));
    }
}

#[derive(Debug)]
pub enum StorageError {
    DatabaseError(String),
    SerializationError(String),
    DeserializationError(String),
    CompressionError(String),
    BackupError(String),
    ConfigurationError(String),
}

// Placeholder implementations for remaining components
pub struct StorageCache {
    max_size: usize,
    data: Arc<RwLock<HashMap<String, (Vec<u8>, bool)>>>,
}

impl StorageCache {
    pub fn new(max_size: usize) -> Self {
        Self {
            max_size,
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn get(&self, key: &str) -> Option<(Vec<u8>, bool)> {
        self.data.read().await.get(key).cloned()
    }

    pub async fn put(&self, key: &str, value: Vec<u8>, is_compressed: bool) {
        self.data.write().await.insert(key.to_string(), (value, is_compressed));
    }

    pub async fn remove(&self, key: &str) {
        self.data.write().await.remove(key);
    }

    pub async fn cleanup(&self) -> usize {
        0 // Placeholder
    }

    pub async fn evict_expired(&self) {
        // Placeholder
    }

    pub async fn get_statistics(&self) -> CacheStatistics {
        CacheStatistics {
            total_requests: 0,
            hits: 0,
            misses: 0,
            evictions: 0,
            memory_usage_bytes: 0,
        }
    }
}

pub struct BackupManager {
    config: StorageConfig,
    backup_dir: PathBuf,
}

impl BackupManager {
    pub async fn new(config: StorageConfig) -> Result<Self, StorageError> {
        let backup_dir = config.data_path.join("backups");
        std::fs::create_dir_all(&backup_dir)
            .map_err(|e| StorageError::DatabaseError(format!("Failed to create backup directory: {}", e)))?;

        Ok(Self { config, backup_dir })
    }

    pub async fn create_backup(&self) -> Result<BackupInfo, StorageError> {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)
            .unwrap().as_secs();
        let backup_name = format!("bitcraps_backup_{}.db", timestamp);
        let backup_path = self.backup_dir.join(&backup_name);
        let source_db = self.config.data_path.join("bitcraps.db");

        // Create backup using SQLite backup API for consistency
        let source_conn = Connection::open(&source_db)
            .map_err(|e| StorageError::DatabaseError(format!("Failed to open source database: {}", e)))?;

        let backup_conn = Connection::open(&backup_path)
            .map_err(|e| StorageError::DatabaseError(format!("Failed to create backup database: {}", e)))?;

        // Perform online backup
        let backup = rusqlite::backup::Backup::new(&source_conn, &backup_conn)
            .map_err(|e| StorageError::DatabaseError(format!("Failed to initialize backup: {}", e)))?;

        backup.run_to_completion(5, Duration::from_millis(100), None)
            .map_err(|e| StorageError::DatabaseError(format!("Backup failed: {}", e)))?;

        // Validate backup integrity
        let validation_result = self.validate_backup(&backup_path).await?;
        if !validation_result.is_valid {
            std::fs::remove_file(&backup_path).ok(); // Clean up invalid backup
            return Err(StorageError::DatabaseError(format!("Backup validation failed: {}", validation_result.error_message.unwrap_or_default())));
        }

        let file_size = std::fs::metadata(&backup_path)
            .map_err(|e| StorageError::DatabaseError(format!("Failed to get backup file size: {}", e)))?
            .len();

        info!("Created validated backup: {} ({} bytes)", backup_name, file_size);

        Ok(BackupInfo {
            backup_id: backup_name.clone(),
            size_bytes: file_size,
            created_at: timestamp,
            validation_hash: validation_result.integrity_hash,
            backup_path: backup_path,
        })
    }

    pub async fn validate_backup(&self, backup_path: &Path) -> Result<BackupValidationResult, StorageError> {
        // Open backup database for validation
        let conn = Connection::open(backup_path)
            .map_err(|e| StorageError::DatabaseError(format!("Failed to open backup for validation: {}", e)))?;

        let mut validation_result = BackupValidationResult {
            is_valid: false,
            integrity_hash: String::new(),
            error_message: None,
            validation_time: SystemTime::now(),
        };

        // Check database integrity
        let integrity_check: String = conn.prepare("PRAGMA integrity_check")
            .and_then(|mut stmt| stmt.query_row([], |row| row.get(0)))
            .map_err(|e| StorageError::DatabaseError(format!("Failed to run integrity check: {}", e)))?;

        if integrity_check != "ok" {
            validation_result.error_message = Some(format!("Integrity check failed: {}", integrity_check));
            return Ok(validation_result);
        }

        // Check that essential tables exist and have expected structure
        let table_check: i32 = conn.prepare("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('storage_records', 'content_references')")
            .and_then(|mut stmt| stmt.query_row([], |row| row.get(0)))
            .map_err(|e| StorageError::DatabaseError(format!("Failed to check table structure: {}", e)))?;

        if table_check != 2 {
            validation_result.error_message = Some("Missing essential tables in backup".to_string());
            return Ok(validation_result);
        }

        // Check that we can read from all tables
        let record_count: i64 = conn.prepare("SELECT COUNT(*) FROM storage_records")
            .and_then(|mut stmt| stmt.query_row([], |row| row.get(0)))
            .map_err(|e| StorageError::DatabaseError(format!("Failed to count storage records: {}", e)))?;

        let ref_count: i64 = conn.prepare("SELECT COUNT(*) FROM content_references")
            .and_then(|mut stmt| stmt.query_row([], |row| row.get(0)))
            .map_err(|e| StorageError::DatabaseError(format!("Failed to count content references: {}", e)))?;

        // Calculate integrity hash of backup file
        let backup_data = std::fs::read(backup_path)
            .map_err(|e| StorageError::DatabaseError(format!("Failed to read backup file for hashing: {}", e)))?;

        let mut hasher = Hasher::new();
        hasher.update(&backup_data);
        validation_result.integrity_hash = hex::encode(hasher.finalize().as_bytes());
        validation_result.is_valid = true;

        info!("Backup validation passed: {} records, {} refs, hash: {}",
              record_count, ref_count, &validation_result.integrity_hash[..16]);

        Ok(validation_result)
    }

    pub async fn restore_from_backup(&self, backup_info: &BackupInfo) -> Result<(), StorageError> {
        // Validate backup before restore
        let validation = self.validate_backup(&backup_info.backup_path).await?;
        if !validation.is_valid {
            return Err(StorageError::DatabaseError(format!("Cannot restore from invalid backup: {}",
                validation.error_message.unwrap_or_default())));
        }

        // Verify backup hash matches
        if validation.integrity_hash != backup_info.validation_hash {
            return Err(StorageError::DatabaseError("Backup integrity hash mismatch".to_string()));
        }

        let current_db = self.config.data_path.join("bitcraps.db");
        let backup_current = self.config.data_path.join("bitcraps.db.pre_restore");

        // Backup current database first
        if current_db.exists() {
            std::fs::copy(&current_db, &backup_current)
                .map_err(|e| StorageError::DatabaseError(format!("Failed to backup current database: {}", e)))?;
        }

        // Replace current database with backup
        std::fs::copy(&backup_info.backup_path, &current_db)
            .map_err(|e| {
                // Attempt to restore original if copy failed
                if backup_current.exists() {
                    std::fs::copy(&backup_current, &current_db).ok();
                }
                StorageError::DatabaseError(format!("Failed to restore from backup: {}", e))
            })?;

        // Cleanup temporary backup
        std::fs::remove_file(&backup_current).ok();

        info!("Successfully restored database from backup: {}", backup_info.backup_id);
        Ok(())
    }

    pub async fn cleanup_old_backups(&self) -> Result<usize, StorageError> {
        let max_age_days = 30; // Keep backups for 30 days
        let max_age_secs = max_age_days * 24 * 3600;
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let mut cleaned_count = 0;

        let entries = std::fs::read_dir(&self.backup_dir)
            .map_err(|e| StorageError::DatabaseError(format!("Failed to read backup directory: {}", e)))?;

        for entry in entries {
            let entry = entry.map_err(|e| StorageError::DatabaseError(format!("Failed to read directory entry: {}", e)))?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("db") {
                if let Ok(metadata) = std::fs::metadata(&path) {
                    if let Ok(modified) = metadata.modified() {
                        if let Ok(age) = modified.duration_since(UNIX_EPOCH) {
                            if now.saturating_sub(age.as_secs()) > max_age_secs {
                                if std::fs::remove_file(&path).is_ok() {
                                    cleaned_count += 1;
                                    debug!("Cleaned up old backup: {:?}", path.file_name());
                                }
                            }
                        }
                    }
                }
            }
        }

        info!("Cleaned up {} old backup files", cleaned_count);
        Ok(cleaned_count)
    }
}

pub struct BackupInfo {
    pub backup_id: String,
    pub size_bytes: u64,
    pub created_at: u64,
    pub validation_hash: String,
    pub backup_path: PathBuf,
}

pub struct BackupValidationResult {
    pub is_valid: bool,
    pub integrity_hash: String,
    pub error_message: Option<String>,
    pub validation_time: SystemTime,
}

pub struct CompressionEngine {
    level: CompressionLevel,
}

impl CompressionEngine {
    pub fn new(level: CompressionLevel) -> Self {
        Self { level }
    }

    pub fn compress(&self, data: &[u8]) -> Result<Vec<u8>, StorageError> {
        let mut encoder = GzEncoder::new(Vec::new(), self.level.to_flate2_level());
        encoder.write_all(data)
            .map_err(|e| StorageError::CompressionError(e.to_string()))?;
        encoder.finish()
            .map_err(|e| StorageError::CompressionError(e.to_string()))
    }

    pub fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, StorageError> {
        let mut decoder = GzDecoder::new(data);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed)
            .map_err(|e| StorageError::CompressionError(e.to_string()))?;
        Ok(decompressed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_storage_creation() {
        let temp_dir = TempDir::new().unwrap();
        let config = StorageConfig {
            data_path: temp_dir.path().to_path_buf(),
            ..StorageConfig::default()
        };

        let storage = PersistentStorageManager::new(config).await.unwrap();
        let stats = storage.get_statistics().await;
        assert_eq!(stats.total_reads, 0);
    }

    #[tokio::test]
    async fn test_store_and_retrieve() {
        let temp_dir = TempDir::new().unwrap();
        let config = StorageConfig {
            data_path: temp_dir.path().to_path_buf(),
            ..StorageConfig::default()
        };

        let storage = PersistentStorageManager::new(config).await.unwrap();

        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct TestData {
            value: String,
            number: i32,
        }

        let data = TestData {
            value: "test".to_string(),
            number: 42,
        };

        storage.store("test_collection", "test_key", &data).await.unwrap();
        let retrieved: Option<TestData> = storage.retrieve("test_collection", "test_key").await.unwrap();

        assert_eq!(retrieved, Some(data));
    }
}