//! Production Storage Module for BitCraps
//!
//! This module provides comprehensive storage solutions for production deployment:
//!
//! ## Features
//! - **High Performance**: Optimized database operations with connection pooling
//! - **Data Integrity**: ACID compliance with automatic backup and recovery
//! - **Space Efficiency**: Intelligent compression and deduplication
//! - **Scalability**: Horizontal scaling support with sharding capabilities
//! - **Monitoring**: Comprehensive metrics and statistics
//! - **Reliability**: Automatic failover and disaster recovery
//!
//! ## Components
//! - `PersistentStorageManager`: Main storage interface
//! - `DatabasePool`: High-performance connection management
//! - `StorageCache`: Multi-tier caching for performance
//! - `BackupManager`: Automated backup and recovery
//! - `CompressionEngine`: Data compression and decompression

pub mod persistent_storage;
pub mod encryption;
pub mod postgresql_backend;

pub use persistent_storage::{
    PersistentStorageManager,
    StorageConfig,
    StorageError,
    StorageStatistics,
    DatabaseStatistics,
    CacheStatistics,
    MaintenanceReport,
    CompressionLevel,
};

pub use encryption::{
    EncryptionEngine,
    KeyManager,
    FileKeyManager,
    EncryptedData,
    KeyDerivationParams,
    calculate_integrity_hash,
};

pub use postgresql_backend::{
    PostgresBackend,
    PostgresConfig,
    PoolStatistics,
};

// Re-export commonly used types
pub use persistent_storage::{
    StorageRecord,
    BackupInfo,
    StorageStats,
};