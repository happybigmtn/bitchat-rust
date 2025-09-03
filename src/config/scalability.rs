//! Scalability configuration module for BitCraps
//!
//! This module provides comprehensive configuration for all scalability-related limits
//! and parameters, making them runtime-configurable and adaptive to system conditions.

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;

/// Platform types for optimization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlatformType {
    /// Mobile device (phone/tablet) - battery optimized
    Mobile,
    /// Desktop/laptop - balanced performance
    Desktop,
    /// Server - maximum performance
    Server,
    /// Embedded - minimal resources
    Embedded,
}

/// Performance profiles that adjust multiple parameters together
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PerformanceProfile {
    /// Battery saver mode
    PowerSaver,
    /// Balanced performance and efficiency
    Balanced,
    /// Maximum performance
    HighPerformance,
    /// Custom profile defined by user
    Custom,
}

/// Comprehensive scalability configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScalabilityConfig {
    /// Platform type for optimization
    pub platform: PlatformType,

    /// Performance profile
    pub profile: PerformanceProfile,

    /// Network and transport limits
    pub network: NetworkLimits,

    /// Memory and cache limits
    pub memory: MemoryLimits,

    /// CPU and threading limits
    pub cpu: CpuLimits,

    /// Database connection and cache limits
    pub database: DatabaseLimits,

    /// Bluetooth/BLE specific limits
    pub bluetooth: BluetoothLimits,

    /// Gaming and consensus limits
    pub gaming: GamingLimits,

    /// Security and validation limits
    pub security: SecurityLimits,

    /// Timeout configurations
    pub timeouts: TimeoutLimits,

    /// Enable adaptive algorithms
    pub adaptive: AdaptiveConfig,
}

/// Network and transport scalability limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkLimits {
    /// Maximum concurrent connections
    pub max_connections: usize,

    /// Maximum message size in bytes
    pub max_message_size: usize,

    /// Network buffer size
    pub buffer_size: usize,

    /// Message batch size for efficiency
    pub message_batch_size: usize,

    /// Connection pool size
    pub connection_pool_size: usize,

    /// Maximum MTU size (adaptive based on transport)
    pub max_mtu_size: usize,

    /// BLE payload size limit
    pub ble_max_payload_size: usize,

    /// Enable compression for messages above threshold
    pub compression_threshold: usize,

    /// Maximum retry attempts
    pub max_retries: u32,

    /// Keep-alive interval
    pub keepalive_interval: Duration,

    /// Connection timeout
    pub connection_timeout: Duration,
}

/// Memory and cache scalability limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryLimits {
    /// Maximum heap size in MB
    pub max_heap_mb: usize,

    /// Cache size in MB
    pub cache_size_mb: usize,

    /// Buffer pool configuration
    pub buffer_pools: BufferPoolConfig,

    /// Object pool sizes
    pub object_pools: ObjectPoolConfig,

    /// Memory pressure threshold (percentage)
    pub pressure_threshold: f64,

    /// Enable memory compression
    pub enable_compression: bool,

    /// Garbage collection interval
    pub gc_interval: Duration,

    /// Pre-allocate buffers on startup
    pub preallocate_buffers: bool,
}

/// Buffer pool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferPoolConfig {
    /// Small buffer pool (256 bytes)
    pub small_pool_size: usize,
    pub small_buffer_size: usize,

    /// Medium buffer pool (1KB)
    pub medium_pool_size: usize,
    pub medium_buffer_size: usize,

    /// Large buffer pool (4KB)
    pub large_pool_size: usize,
    pub large_buffer_size: usize,

    /// Packet buffer pool (MTU size)
    pub packet_pool_size: usize,
    pub packet_buffer_size: usize,
}

/// Object pool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectPoolConfig {
    /// Message object pool
    pub message_pool_size: usize,

    /// Connection object pool
    pub connection_pool_size: usize,

    /// Transaction object pool
    pub transaction_pool_size: usize,

    /// String pool size
    pub string_pool_size: usize,

    /// Vector pool size
    pub vec_pool_size: usize,
}

/// CPU and threading limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuLimits {
    /// Worker thread pool size
    pub worker_threads: usize,

    /// Async runtime threads
    pub async_threads: usize,

    /// Game processing threads
    pub game_threads: usize,

    /// Network processing threads
    pub network_threads: usize,

    /// Crypto processing threads
    pub crypto_threads: usize,

    /// Target CPU utilization percentage
    pub target_utilization: f64,

    /// Task batch size for efficiency
    pub task_batch_size: usize,

    /// Enable CPU affinity
    pub enable_affinity: bool,

    /// Enable priority scheduling
    pub enable_priority_scheduling: bool,

    /// Idle sleep duration when no work
    pub idle_sleep_duration: Duration,
}

/// Database connection and cache limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseLimits {
    /// Connection pool size
    pub connection_pool_size: usize,

    /// Query cache size
    pub query_cache_size: usize,

    /// Index cache size
    pub index_cache_size: usize,

    /// Page cache size in KB
    pub page_cache_size_kb: usize,

    /// Memory-mapped I/O size in MB
    pub mmap_size_mb: usize,

    /// Transaction batch size
    pub transaction_batch_size: usize,

    /// WAL checkpoint interval
    pub wal_checkpoint_interval: Duration,

    /// Vacuum interval
    pub vacuum_interval: Duration,

    /// Enable query optimization
    pub enable_query_optimization: bool,
}

/// Bluetooth/BLE specific limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BluetoothLimits {
    /// BLE MTU size (adaptive)
    pub mtu_size: u16,

    /// Maximum concurrent connections
    pub max_concurrent_connections: usize,

    /// Scan interval
    pub scan_interval: Duration,

    /// Scan window
    pub scan_window: Duration,

    /// Advertisement interval
    pub advertisement_interval: Duration,

    /// Connection interval range
    pub connection_interval_min: Duration,
    pub connection_interval_max: Duration,

    /// Connection latency (skip intervals)
    pub connection_latency: u16,

    /// Supervision timeout
    pub supervision_timeout: Duration,

    /// TX power level (0-100)
    pub tx_power_level: u8,

    /// Enable adaptive scanning
    pub adaptive_scanning: bool,
}

/// Gaming and consensus limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GamingLimits {
    /// Maximum concurrent games
    pub max_concurrent_games: usize,

    /// Maximum players per game
    pub max_players_per_game: usize,

    /// Maximum games per player
    pub max_games_per_player: usize,

    /// Maximum bets per player per game
    pub max_bets_per_player: usize,

    /// Consensus batch size
    pub consensus_batch_size: usize,

    /// Vote cache size
    pub vote_cache_size: usize,

    /// Signature validation threads
    pub validation_threads: usize,

    /// Enable parallel validation
    pub parallel_validation: bool,

    /// Fast-path for unanimous decisions
    pub enable_fast_path: bool,

    /// Fork detection interval
    pub fork_detection_interval: Duration,
}

/// Security and validation limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityLimits {
    /// Maximum string length
    pub max_string_length: usize,

    /// Maximum array length
    pub max_array_length: usize,

    /// Maximum packet size
    pub max_packet_size: usize,

    /// Key rotation interval
    pub key_rotation_interval: Duration,

    /// Maximum message age
    pub max_message_age: Duration,

    /// Rate limiting: requests per second
    pub rate_limit_rps: u32,

    /// Rate limiting: burst size
    pub rate_limit_burst: u32,

    /// Hash verification difficulty
    pub hash_difficulty: u32,

    /// Signature verification timeout
    pub signature_timeout: Duration,
}

/// Timeout configurations for various operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeoutLimits {
    /// Database operation timeout
    pub database: Duration,

    /// Network request timeout
    pub network: Duration,

    /// Consensus operation timeout
    pub consensus: Duration,

    /// File I/O timeout
    pub file_io: Duration,

    /// Lock acquisition timeout
    pub lock: Duration,

    /// Channel operation timeout
    pub channel: Duration,

    /// Service startup/shutdown timeout
    pub service: Duration,

    /// Critical fast operations timeout
    pub critical_fast: Duration,
}

/// Adaptive algorithm configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptiveConfig {
    /// Enable adaptive algorithms
    pub enabled: bool,

    /// Adaptation interval
    pub adaptation_interval: Duration,

    /// Minimum adaptation threshold (percentage change)
    pub min_threshold: f64,

    /// Maximum adaptation step (percentage)
    pub max_step: f64,

    /// Adaptation history window
    pub history_window: usize,

    /// Enable MTU auto-discovery
    pub adaptive_mtu: bool,

    /// Enable connection pool auto-sizing
    pub adaptive_connections: bool,

    /// Enable cache size auto-tuning
    pub adaptive_cache: bool,

    /// Enable thread pool auto-sizing
    pub adaptive_threads: bool,

    /// Enable memory pressure adaptation
    pub adaptive_memory: bool,
}

impl Default for ScalabilityConfig {
    fn default() -> Self {
        Self::for_platform(PlatformType::Desktop, PerformanceProfile::Balanced)
    }
}

impl ScalabilityConfig {
    /// Create configuration for specific platform and profile
    pub fn for_platform(platform: PlatformType, profile: PerformanceProfile) -> Self {
        match platform {
            PlatformType::Mobile => Self::mobile_config(profile),
            PlatformType::Desktop => Self::desktop_config(profile),
            PlatformType::Server => Self::server_config(profile),
            PlatformType::Embedded => Self::embedded_config(profile),
        }
    }

    /// Mobile platform configuration
    fn mobile_config(profile: PerformanceProfile) -> Self {
        let (multiplier, battery_optimized) = match profile {
            PerformanceProfile::PowerSaver => (0.5, true),
            PerformanceProfile::Balanced => (0.75, true),
            PerformanceProfile::HighPerformance => (1.0, false),
            PerformanceProfile::Custom => (1.0, true),
        };

        Self {
            platform: PlatformType::Mobile,
            profile,
            network: NetworkLimits {
                max_connections: (50.0f64 * multiplier) as usize,
                max_message_size: 32 * 1024, // 32KB
                buffer_size: (8192.0f64 * multiplier) as usize,
                message_batch_size: (20.0f64 * multiplier) as usize,
                connection_pool_size: (5.0f64 * multiplier) as usize,
                max_mtu_size: 1500,
                ble_max_payload_size: 244, // Will be adaptive
                compression_threshold: 1024,
                max_retries: 3,
                keepalive_interval: if battery_optimized {
                    Duration::from_secs(120)
                } else {
                    Duration::from_secs(60)
                },
                connection_timeout: Duration::from_secs(30),
            },
            memory: MemoryLimits {
                max_heap_mb: (256.0f64 * multiplier) as usize,
                cache_size_mb: (64.0f64 * multiplier) as usize,
                buffer_pools: BufferPoolConfig {
                    small_pool_size: (100.0f64 * multiplier) as usize,
                    small_buffer_size: 256,
                    medium_pool_size: (50.0f64 * multiplier) as usize,
                    medium_buffer_size: 1024,
                    large_pool_size: (20.0f64 * multiplier) as usize,
                    large_buffer_size: 4096,
                    packet_pool_size: (30.0f64 * multiplier) as usize,
                    packet_buffer_size: 1500, // MTU size
                },
                object_pools: ObjectPoolConfig {
                    message_pool_size: (200.0f64 * multiplier) as usize,
                    connection_pool_size: (20.0f64 * multiplier) as usize,
                    transaction_pool_size: (50.0f64 * multiplier) as usize,
                    string_pool_size: (100.0f64 * multiplier) as usize,
                    vec_pool_size: (100.0f64 * multiplier) as usize,
                },
                pressure_threshold: 80.0,
                enable_compression: true,
                gc_interval: if battery_optimized {
                    Duration::from_secs(300)
                } else {
                    Duration::from_secs(120)
                },
                preallocate_buffers: false,
            },
            cpu: CpuLimits {
                worker_threads: (2.0f64 * multiplier).max(1.0f64) as usize,
                async_threads: (2.0f64 * multiplier).max(1.0f64) as usize,
                game_threads: 1,
                network_threads: 1,
                crypto_threads: 1,
                target_utilization: if battery_optimized { 30.0 } else { 60.0 },
                task_batch_size: (10.0f64 * multiplier) as usize,
                enable_affinity: false,
                enable_priority_scheduling: true,
                idle_sleep_duration: Duration::from_millis(if battery_optimized {
                    100
                } else {
                    50
                }),
            },
            database: DatabaseLimits {
                connection_pool_size: (3.0f64 * multiplier).max(1.0f64) as usize,
                query_cache_size: (500.0f64 * multiplier) as usize,
                index_cache_size: (1000.0f64 * multiplier) as usize,
                page_cache_size_kb: (16.0 * 1024.0 * multiplier) as usize, // 16MB
                mmap_size_mb: (128.0 * multiplier) as usize,
                transaction_batch_size: (20.0f64 * multiplier) as usize,
                wal_checkpoint_interval: Duration::from_secs(300),
                vacuum_interval: Duration::from_secs(86400), // Daily
                enable_query_optimization: true,
            },
            bluetooth: BluetoothLimits {
                mtu_size: 185, // Conservative for battery
                max_concurrent_connections: (8.0f64 * multiplier) as usize,
                scan_interval: Duration::from_millis(if battery_optimized { 5000 } else { 2000 }),
                scan_window: Duration::from_millis(if battery_optimized { 1000 } else { 500 }),
                advertisement_interval: Duration::from_millis(if battery_optimized {
                    2000
                } else {
                    1000
                }),
                connection_interval_min: Duration::from_millis(100),
                connection_interval_max: Duration::from_millis(200),
                connection_latency: if battery_optimized { 4 } else { 2 },
                supervision_timeout: Duration::from_secs(10),
                tx_power_level: if battery_optimized { 50 } else { 75 },
                adaptive_scanning: true,
            },
            gaming: GamingLimits {
                max_concurrent_games: (10.0f64 * multiplier) as usize,
                max_players_per_game: 8,
                max_games_per_player: 3,
                max_bets_per_player: (20.0f64 * multiplier) as usize,
                consensus_batch_size: (20.0f64 * multiplier) as usize,
                vote_cache_size: (200.0f64 * multiplier) as usize,
                validation_threads: 1,
                parallel_validation: multiplier > 0.7,
                enable_fast_path: true,
                fork_detection_interval: Duration::from_secs(60),
            },
            security: SecurityLimits {
                max_string_length: 4096,
                max_array_length: 1000,
                max_packet_size: 32 * 1024, // 32KB
                key_rotation_interval: Duration::from_secs(24 * 60 * 60), // 24 hours, will be adaptive
                max_message_age: Duration::from_secs(5 * 60), // 5 minutes, will be adaptive
                rate_limit_rps: (100.0f64 * multiplier) as u32,
                rate_limit_burst: (10.0f64 * multiplier) as u32,
                hash_difficulty: 16,
                signature_timeout: Duration::from_secs(5),
            },
            timeouts: TimeoutLimits {
                database: Duration::from_secs(5),
                network: Duration::from_secs(if battery_optimized { 15 } else { 10 }),
                consensus: Duration::from_secs(if battery_optimized { 45 } else { 30 }),
                file_io: Duration::from_secs(3),
                lock: Duration::from_secs(1),
                channel: Duration::from_millis(500),
                service: Duration::from_secs(60),
                critical_fast: Duration::from_millis(100),
            },
            adaptive: AdaptiveConfig {
                enabled: true,
                adaptation_interval: Duration::from_secs(300), // 5 minutes
                min_threshold: 0.1,                            // 10% change required
                max_step: 0.2,                                 // 20% max adjustment per step
                history_window: 10,
                adaptive_mtu: true,
                adaptive_connections: true,
                adaptive_cache: true,
                adaptive_threads: false, // Limited on mobile
                adaptive_memory: true,
            },
        }
    }

    /// Desktop platform configuration
    fn desktop_config(profile: PerformanceProfile) -> Self {
        let multiplier = match profile {
            PerformanceProfile::PowerSaver => 0.6,
            PerformanceProfile::Balanced => 1.0,
            PerformanceProfile::HighPerformance => 1.5,
            PerformanceProfile::Custom => 1.0,
        };

        Self {
            platform: PlatformType::Desktop,
            profile,
            network: NetworkLimits {
                max_connections: (200.0f64 * multiplier) as usize,
                max_message_size: 64 * 1024, // 64KB
                buffer_size: (16384.0 * multiplier) as usize,
                message_batch_size: (100.0f64 * multiplier) as usize,
                connection_pool_size: (10.0f64 * multiplier) as usize,
                max_mtu_size: 9000, // Jumbo frames support
                ble_max_payload_size: 244,
                compression_threshold: 2048,
                max_retries: 5,
                keepalive_interval: Duration::from_secs(60),
                connection_timeout: Duration::from_secs(20),
            },
            memory: MemoryLimits {
                max_heap_mb: (1024.0f64 * multiplier) as usize,
                cache_size_mb: (256.0f64 * multiplier) as usize,
                buffer_pools: BufferPoolConfig {
                    small_pool_size: (500.0f64 * multiplier) as usize,
                    small_buffer_size: 256,
                    medium_pool_size: (200.0f64 * multiplier) as usize,
                    medium_buffer_size: 1024,
                    large_pool_size: (100.0f64 * multiplier) as usize,
                    large_buffer_size: 4096,
                    packet_pool_size: (150.0 * multiplier) as usize,
                    packet_buffer_size: 9000, // Jumbo frame size
                },
                object_pools: ObjectPoolConfig {
                    message_pool_size: (1000.0f64 * multiplier) as usize,
                    connection_pool_size: (100.0f64 * multiplier) as usize,
                    transaction_pool_size: (200.0f64 * multiplier) as usize,
                    string_pool_size: (500.0f64 * multiplier) as usize,
                    vec_pool_size: (500.0f64 * multiplier) as usize,
                },
                pressure_threshold: 75.0,
                enable_compression: false,
                gc_interval: Duration::from_secs(120),
                preallocate_buffers: true,
            },
            cpu: CpuLimits {
                worker_threads: (num_cpus::get() as f64 * multiplier).max(2.0f64) as usize,
                async_threads: (num_cpus::get() as f64 * multiplier).max(2.0f64) as usize,
                game_threads: ((num_cpus::get() / 2) as f64 * multiplier).max(1.0f64) as usize,
                network_threads: ((num_cpus::get() / 4) as f64 * multiplier).max(1.0f64) as usize,
                crypto_threads: ((num_cpus::get() / 4) as f64 * multiplier).max(1.0f64) as usize,
                target_utilization: 70.0,
                task_batch_size: (50.0f64 * multiplier) as usize,
                enable_affinity: true,
                enable_priority_scheduling: true,
                idle_sleep_duration: Duration::from_millis(25),
            },
            database: DatabaseLimits {
                connection_pool_size: (10.0f64 * multiplier) as usize,
                query_cache_size: (2000.0 * multiplier) as usize,
                index_cache_size: (5000.0 * multiplier) as usize,
                page_cache_size_kb: (64.0 * 1024.0 * multiplier) as usize, // 64MB
                mmap_size_mb: (512.0 * multiplier) as usize,
                transaction_batch_size: (100.0f64 * multiplier) as usize,
                wal_checkpoint_interval: Duration::from_secs(60),
                vacuum_interval: Duration::from_secs(43200), // 12 hours
                enable_query_optimization: true,
            },
            bluetooth: BluetoothLimits {
                mtu_size: 251,
                max_concurrent_connections: (20.0f64 * multiplier) as usize,
                scan_interval: Duration::from_millis(2000),
                scan_window: Duration::from_millis(500),
                advertisement_interval: Duration::from_millis(1000),
                connection_interval_min: Duration::from_millis(50),
                connection_interval_max: Duration::from_millis(100),
                connection_latency: 2,
                supervision_timeout: Duration::from_secs(5),
                tx_power_level: 75,
                adaptive_scanning: true,
            },
            gaming: GamingLimits {
                max_concurrent_games: (50.0f64 * multiplier) as usize,
                max_players_per_game: 12,
                max_games_per_player: 10,
                max_bets_per_player: (100.0f64 * multiplier) as usize,
                consensus_batch_size: (100.0f64 * multiplier) as usize,
                vote_cache_size: (1000.0f64 * multiplier) as usize,
                validation_threads: ((num_cpus::get() / 2) as f64 * multiplier).max(2.0f64)
                    as usize,
                parallel_validation: true,
                enable_fast_path: true,
                fork_detection_interval: Duration::from_secs(30),
            },
            security: SecurityLimits {
                max_string_length: 8192,
                max_array_length: 5000,
                max_packet_size: 64 * 1024, // 64KB
                key_rotation_interval: Duration::from_secs(24 * 60 * 60),
                max_message_age: Duration::from_secs(5 * 60),
                rate_limit_rps: (500.0f64 * multiplier) as u32,
                rate_limit_burst: (50.0f64 * multiplier) as u32,
                hash_difficulty: 20,
                signature_timeout: Duration::from_secs(10),
            },
            timeouts: TimeoutLimits {
                database: Duration::from_secs(5),
                network: Duration::from_secs(10),
                consensus: Duration::from_secs(30),
                file_io: Duration::from_secs(3),
                lock: Duration::from_secs(1),
                channel: Duration::from_millis(500),
                service: Duration::from_secs(60),
                critical_fast: Duration::from_millis(100),
            },
            adaptive: AdaptiveConfig {
                enabled: true,
                adaptation_interval: Duration::from_secs(60), // 1 minute
                min_threshold: 0.05,                          // 5% change required
                max_step: 0.25,                               // 25% max adjustment per step
                history_window: 20,
                adaptive_mtu: true,
                adaptive_connections: true,
                adaptive_cache: true,
                adaptive_threads: true,
                adaptive_memory: true,
            },
        }
    }

    /// Server platform configuration
    fn server_config(profile: PerformanceProfile) -> Self {
        let multiplier = match profile {
            PerformanceProfile::PowerSaver => 1.0, // Servers don't usually power save
            PerformanceProfile::Balanced => 1.5,
            PerformanceProfile::HighPerformance => 2.0,
            PerformanceProfile::Custom => 1.5,
        };

        Self {
            platform: PlatformType::Server,
            profile,
            network: NetworkLimits {
                max_connections: (1000.0f64 * multiplier) as usize,
                max_message_size: 1024 * 1024, // 1MB
                buffer_size: (65536.0 * multiplier) as usize,
                message_batch_size: (500.0f64 * multiplier) as usize,
                connection_pool_size: (50.0f64 * multiplier) as usize,
                max_mtu_size: 9000,
                ble_max_payload_size: 244,
                compression_threshold: 8192,
                max_retries: 10,
                keepalive_interval: Duration::from_secs(30),
                connection_timeout: Duration::from_secs(10),
            },
            memory: MemoryLimits {
                max_heap_mb: (4096.0f64 * multiplier) as usize,
                cache_size_mb: (1024.0f64 * multiplier) as usize,
                buffer_pools: BufferPoolConfig {
                    small_pool_size: (2000.0 * multiplier) as usize,
                    small_buffer_size: 256,
                    medium_pool_size: (1000.0f64 * multiplier) as usize,
                    medium_buffer_size: 1024,
                    large_pool_size: (500.0f64 * multiplier) as usize,
                    large_buffer_size: 4096,
                    packet_pool_size: (500.0f64 * multiplier) as usize,
                    packet_buffer_size: 9000,
                },
                object_pools: ObjectPoolConfig {
                    message_pool_size: (5000.0 * multiplier) as usize,
                    connection_pool_size: (500.0f64 * multiplier) as usize,
                    transaction_pool_size: (1000.0f64 * multiplier) as usize,
                    string_pool_size: (2000.0 * multiplier) as usize,
                    vec_pool_size: (2000.0 * multiplier) as usize,
                },
                pressure_threshold: 90.0,
                enable_compression: false,
                gc_interval: Duration::from_secs(60),
                preallocate_buffers: true,
            },
            cpu: CpuLimits {
                worker_threads: num_cpus::get(),
                async_threads: num_cpus::get(),
                game_threads: num_cpus::get() / 2,
                network_threads: num_cpus::get() / 4,
                crypto_threads: num_cpus::get() / 2,
                target_utilization: 85.0,
                task_batch_size: (200.0f64 * multiplier) as usize,
                enable_affinity: true,
                enable_priority_scheduling: true,
                idle_sleep_duration: Duration::from_millis(10),
            },
            database: DatabaseLimits {
                connection_pool_size: (50.0f64 * multiplier) as usize,
                query_cache_size: (10000.0 * multiplier) as usize,
                index_cache_size: (50000.0 * multiplier) as usize,
                page_cache_size_kb: (256.0 * 1024.0 * multiplier) as usize, // 256MB
                mmap_size_mb: (2048.0f64 * multiplier) as usize,
                transaction_batch_size: (500.0f64 * multiplier) as usize,
                wal_checkpoint_interval: Duration::from_secs(30),
                vacuum_interval: Duration::from_secs(21600), // 6 hours
                enable_query_optimization: true,
            },
            bluetooth: BluetoothLimits {
                mtu_size: 512,
                max_concurrent_connections: (100.0f64 * multiplier) as usize,
                scan_interval: Duration::from_millis(500),
                scan_window: Duration::from_millis(250),
                advertisement_interval: Duration::from_millis(200),
                connection_interval_min: Duration::from_millis(20),
                connection_interval_max: Duration::from_millis(40),
                connection_latency: 0,
                supervision_timeout: Duration::from_secs(2),
                tx_power_level: 100,
                adaptive_scanning: false,
            },
            gaming: GamingLimits {
                max_concurrent_games: (500.0f64 * multiplier) as usize,
                max_players_per_game: 20,
                max_games_per_player: 50,
                max_bets_per_player: (500.0f64 * multiplier) as usize,
                consensus_batch_size: (500.0f64 * multiplier) as usize,
                vote_cache_size: (10000.0 * multiplier) as usize,
                validation_threads: num_cpus::get() / 2,
                parallel_validation: true,
                enable_fast_path: true,
                fork_detection_interval: Duration::from_secs(10),
            },
            security: SecurityLimits {
                max_string_length: 32768,
                max_array_length: 50000,
                max_packet_size: 1024 * 1024, // 1MB
                key_rotation_interval: Duration::from_secs(12 * 60 * 60), // 12 hours
                max_message_age: Duration::from_secs(10 * 60), // 10 minutes
                rate_limit_rps: (2000.0 * multiplier) as u32,
                rate_limit_burst: (200.0f64 * multiplier) as u32,
                hash_difficulty: 24,
                signature_timeout: Duration::from_secs(30),
            },
            timeouts: TimeoutLimits {
                database: Duration::from_secs(10),
                network: Duration::from_secs(20),
                consensus: Duration::from_secs(60),
                file_io: Duration::from_secs(10),
                lock: Duration::from_secs(5),
                channel: Duration::from_secs(1),
                service: Duration::from_secs(120),
                critical_fast: Duration::from_millis(200),
            },
            adaptive: AdaptiveConfig {
                enabled: true,
                adaptation_interval: Duration::from_secs(30), // 30 seconds
                min_threshold: 0.02,                          // 2% change required
                max_step: 0.30,                               // 30% max adjustment per step
                history_window: 50,
                adaptive_mtu: true,
                adaptive_connections: true,
                adaptive_cache: true,
                adaptive_threads: true,
                adaptive_memory: true,
            },
        }
    }

    /// Embedded platform configuration
    fn embedded_config(profile: PerformanceProfile) -> Self {
        let multiplier = match profile {
            PerformanceProfile::PowerSaver => 0.3,
            PerformanceProfile::Balanced => 0.5,
            PerformanceProfile::HighPerformance => 0.8,
            PerformanceProfile::Custom => 0.5,
        };

        Self {
            platform: PlatformType::Embedded,
            profile,
            network: NetworkLimits {
                max_connections: (10.0f64 * multiplier).max(5.0f64) as usize,
                max_message_size: 8 * 1024, // 8KB
                buffer_size: (2048.0f64 * multiplier).max(1024.0f64) as usize,
                message_batch_size: (5.0f64 * multiplier).max(1.0f64) as usize,
                connection_pool_size: (2.0f64 * multiplier).max(1.0f64) as usize,
                max_mtu_size: 1500,
                ble_max_payload_size: 185, // Conservative
                compression_threshold: 512,
                max_retries: 2,
                keepalive_interval: Duration::from_secs(300),
                connection_timeout: Duration::from_secs(60),
            },
            memory: MemoryLimits {
                max_heap_mb: (32.0f64 * multiplier).max(16.0f64) as usize,
                cache_size_mb: (8.0f64 * multiplier).max(4.0f64) as usize,
                buffer_pools: BufferPoolConfig {
                    small_pool_size: (20.0f64 * multiplier).max(10.0f64) as usize,
                    small_buffer_size: 256,
                    medium_pool_size: (10.0f64 * multiplier).max(5.0f64) as usize,
                    medium_buffer_size: 1024,
                    large_pool_size: (5.0f64 * multiplier).max(2.0f64) as usize,
                    large_buffer_size: 2048, // Smaller for embedded
                    packet_pool_size: (10.0f64 * multiplier).max(5.0f64) as usize,
                    packet_buffer_size: 1500,
                },
                object_pools: ObjectPoolConfig {
                    message_pool_size: (50.0f64 * multiplier).max(20.0f64) as usize,
                    connection_pool_size: (10.0f64 * multiplier).max(5.0f64) as usize,
                    transaction_pool_size: (20.0f64 * multiplier).max(10.0f64) as usize,
                    string_pool_size: (30.0f64 * multiplier).max(10.0f64) as usize,
                    vec_pool_size: (30.0f64 * multiplier).max(10.0f64) as usize,
                },
                pressure_threshold: 70.0,
                enable_compression: true,
                gc_interval: Duration::from_secs(600), // 10 minutes
                preallocate_buffers: false,
            },
            cpu: CpuLimits {
                worker_threads: 1,
                async_threads: 1,
                game_threads: 1,
                network_threads: 1,
                crypto_threads: 1,
                target_utilization: 20.0,
                task_batch_size: (5.0f64 * multiplier).max(1.0f64) as usize,
                enable_affinity: false,
                enable_priority_scheduling: false,
                idle_sleep_duration: Duration::from_millis(200),
            },
            database: DatabaseLimits {
                connection_pool_size: 1,
                query_cache_size: (100.0f64 * multiplier).max(50.0f64) as usize,
                index_cache_size: (200.0f64 * multiplier).max(100.0f64) as usize,
                page_cache_size_kb: (2.0f64 * 1024.0f64 * multiplier).max(1024.0f64) as usize, // 2MB
                mmap_size_mb: (16.0f64 * multiplier).max(8.0f64) as usize,
                transaction_batch_size: (5.0f64 * multiplier).max(1.0f64) as usize,
                wal_checkpoint_interval: Duration::from_secs(600),
                vacuum_interval: Duration::from_secs(172800), // 48 hours
                enable_query_optimization: false,             // Too expensive
            },
            bluetooth: BluetoothLimits {
                mtu_size: 185,
                max_concurrent_connections: (3.0f64 * multiplier).max(2.0f64) as usize,
                scan_interval: Duration::from_millis(10000),
                scan_window: Duration::from_millis(2000),
                advertisement_interval: Duration::from_millis(5000),
                connection_interval_min: Duration::from_millis(200),
                connection_interval_max: Duration::from_millis(400),
                connection_latency: 6,
                supervision_timeout: Duration::from_secs(20),
                tx_power_level: 25,
                adaptive_scanning: true,
            },
            gaming: GamingLimits {
                max_concurrent_games: (3.0f64 * multiplier).max(1.0f64) as usize,
                max_players_per_game: 4,
                max_games_per_player: 1,
                max_bets_per_player: (5.0f64 * multiplier).max(1.0f64) as usize,
                consensus_batch_size: (5.0f64 * multiplier).max(1.0f64) as usize,
                vote_cache_size: (50.0f64 * multiplier).max(20.0f64) as usize,
                validation_threads: 1,
                parallel_validation: false,
                enable_fast_path: false,
                fork_detection_interval: Duration::from_secs(300),
            },
            security: SecurityLimits {
                max_string_length: 1024,
                max_array_length: 100,
                max_packet_size: 8 * 1024, // 8KB
                key_rotation_interval: Duration::from_secs(48 * 60 * 60), // 48 hours
                max_message_age: Duration::from_secs(15 * 60), // 15 minutes
                rate_limit_rps: (10.0f64 * multiplier).max(5.0f64) as u32,
                rate_limit_burst: (2.0f64 * multiplier).max(1.0f64) as u32,
                hash_difficulty: 12,
                signature_timeout: Duration::from_secs(30),
            },
            timeouts: TimeoutLimits {
                database: Duration::from_secs(10),
                network: Duration::from_secs(30),
                consensus: Duration::from_secs(120),
                file_io: Duration::from_secs(10),
                lock: Duration::from_secs(5),
                channel: Duration::from_secs(2),
                service: Duration::from_secs(300),
                critical_fast: Duration::from_millis(500),
            },
            adaptive: AdaptiveConfig {
                enabled: false, // Too expensive for embedded
                adaptation_interval: Duration::from_secs(600),
                min_threshold: 0.2,
                max_step: 0.1,
                history_window: 5,
                adaptive_mtu: false,
                adaptive_connections: false,
                adaptive_cache: false,
                adaptive_threads: false,
                adaptive_memory: true, // Only memory adaptation
            },
        }
    }
}

/// Runtime scalability manager that can update configuration dynamically
pub struct ScalabilityManager {
    config: Arc<RwLock<ScalabilityConfig>>,
    metrics_history: Arc<RwLock<Vec<crate::performance::PerformanceMetrics>>>,
    last_adaptation: Arc<RwLock<std::time::Instant>>,
}

impl ScalabilityManager {
    /// Create a new scalability manager
    pub fn new(config: ScalabilityConfig) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
            metrics_history: Arc::new(RwLock::new(Vec::new())),
            last_adaptation: Arc::new(RwLock::new(std::time::Instant::now())),
        }
    }

    /// Get current configuration
    pub fn get_config(&self) -> ScalabilityConfig {
        self.config.read().clone()
    }

    /// Update configuration
    pub fn update_config(&self, config: ScalabilityConfig) {
        *self.config.write() = config;
    }

    /// Adapt configuration based on current metrics
    pub fn adapt_to_metrics(&self, metrics: &crate::performance::PerformanceMetrics) {
        let config_read = self.config.read();
        if !config_read.adaptive.enabled {
            return;
        }

        let now = std::time::Instant::now();
        let last_adaptation = *self.last_adaptation.read();

        if now.duration_since(last_adaptation) < config_read.adaptive.adaptation_interval {
            return;
        }
        drop(config_read);

        // Add metrics to history
        {
            let mut history = self.metrics_history.write();
            history.push(metrics.clone());

            let max_history = self.config.read().adaptive.history_window;
            if history.len() > max_history {
                history.remove(0);
            }
        }

        // Perform adaptations
        self.adapt_network_limits(metrics);
        self.adapt_memory_limits(metrics);
        self.adapt_cpu_limits(metrics);
        self.adapt_database_limits(metrics);
        self.adapt_gaming_limits(metrics);

        *self.last_adaptation.write() = now;
    }

    /// Adapt network limits based on performance
    fn adapt_network_limits(&self, metrics: &crate::performance::PerformanceMetrics) {
        let mut config = self.config.write();
        if !config.adaptive.adaptive_connections {
            return;
        }

        let network_utilization = metrics.network_latency.p95_ms / 1000.0; // Convert to seconds
        let connection_efficiency = metrics.consensus_performance.throughput_ops_per_sec;

        // Adapt connection pool size
        if connection_efficiency < 10.0 && config.network.max_connections > 10 {
            let reduction = (config.network.max_connections as f64 * 0.1).max(1.0f64) as usize;
            config.network.max_connections = (config.network.max_connections - reduction).max(5);
            log::info!(
                "Reduced max connections to {}",
                config.network.max_connections
            );
        } else if connection_efficiency > 100.0 && network_utilization < 0.1 {
            let increase = (config.network.max_connections as f64 * 0.1).max(1.0f64) as usize;
            let platform_max = match config.platform {
                PlatformType::Mobile => 100,
                PlatformType::Desktop => 500,
                PlatformType::Server => 2000,
                PlatformType::Embedded => 20,
            };
            config.network.max_connections =
                (config.network.max_connections + increase).min(platform_max);
            log::info!(
                "Increased max connections to {}",
                config.network.max_connections
            );
        }

        // Adapt MTU size based on success rate
        if config.adaptive.adaptive_mtu {
            let success_rate = 1.0 - (metrics.mesh_performance.packet_loss_rate * 100.0);
            if success_rate < 0.8 && config.network.ble_max_payload_size > 128 {
                config.network.ble_max_payload_size =
                    (config.network.ble_max_payload_size - 20).max(128);
                log::info!("Reduced BLE MTU to {}", config.network.ble_max_payload_size);
            } else if success_rate > 0.95 && config.network.ble_max_payload_size < 512 {
                config.network.ble_max_payload_size =
                    (config.network.ble_max_payload_size + 20).min(512);
                log::info!(
                    "Increased BLE MTU to {}",
                    config.network.ble_max_payload_size
                );
            }
        }
    }

    /// Adapt memory limits based on usage
    fn adapt_memory_limits(&self, metrics: &crate::performance::PerformanceMetrics) {
        let mut config = self.config.write();
        if !config.adaptive.adaptive_memory {
            return;
        }

        let memory_pressure =
            (metrics.memory_usage.heap_used_mb / config.memory.max_heap_mb as f64) * 100.0;

        if memory_pressure > config.memory.pressure_threshold {
            // Reduce cache sizes
            config.memory.cache_size_mb = (config.memory.cache_size_mb / 2).max(4);

            // Reduce pool sizes
            config.memory.object_pools.message_pool_size =
                (config.memory.object_pools.message_pool_size / 2).max(10);
            config.memory.object_pools.vec_pool_size =
                (config.memory.buffer_pools.medium_pool_size / 2).max(5);

            // More aggressive GC
            config.memory.gc_interval = Duration::from_secs(30);

            log::warn!(
                "High memory pressure ({}%), reduced cache and pool sizes",
                memory_pressure
            );
        } else if memory_pressure < config.memory.pressure_threshold - 20.0 {
            // Increase cache sizes if we have room
            let platform_max_cache = match config.platform {
                PlatformType::Mobile => 128,
                PlatformType::Desktop => 512,
                PlatformType::Server => 2048,
                PlatformType::Embedded => 16,
            };

            if config.memory.cache_size_mb < platform_max_cache {
                config.memory.cache_size_mb =
                    (config.memory.cache_size_mb * 3 / 2).min(platform_max_cache);
                log::info!(
                    "Low memory pressure ({}%), increased cache size to {} MB",
                    memory_pressure,
                    config.memory.cache_size_mb
                );
            }
        }
    }

    /// Adapt CPU limits based on utilization
    fn adapt_cpu_limits(&self, metrics: &crate::performance::PerformanceMetrics) {
        let mut config = self.config.write();
        if !config.adaptive.adaptive_threads {
            return;
        }

        let cpu_utilization = metrics.cpu_usage.utilization_percent;

        if cpu_utilization > config.cpu.target_utilization + 20.0 {
            // Reduce batch sizes to spread load
            config.cpu.task_batch_size = (config.cpu.task_batch_size / 2).max(1);
            config.gaming.consensus_batch_size = (config.gaming.consensus_batch_size / 2).max(1);

            log::info!("High CPU usage ({}%), reduced batch sizes", cpu_utilization);
        } else if cpu_utilization < config.cpu.target_utilization - 10.0 {
            // Increase batch sizes for efficiency
            let platform_max_batch = match config.platform {
                PlatformType::Mobile => 50,
                PlatformType::Desktop => 200,
                PlatformType::Server => 1000,
                PlatformType::Embedded => 10,
            };

            config.cpu.task_batch_size =
                (config.cpu.task_batch_size * 3 / 2).min(platform_max_batch);
            config.gaming.consensus_batch_size =
                (config.gaming.consensus_batch_size * 3 / 2).min(platform_max_batch);

            log::info!(
                "Low CPU usage ({}%), increased batch sizes",
                cpu_utilization
            );
        }
    }

    /// Adapt database limits based on performance
    fn adapt_database_limits(&self, metrics: &crate::performance::PerformanceMetrics) {
        let mut config = self.config.write();
        if !config.adaptive.adaptive_cache {
            return;
        }

        // Use network latency as a proxy for database performance
        let db_latency = metrics.network_latency.p50_ms; // Use median as proxy

        if db_latency > 100.0 {
            // Increase database cache sizes
            let platform_max = match config.platform {
                PlatformType::Mobile => 2000,
                PlatformType::Desktop => 10000,
                PlatformType::Server => 100000,
                PlatformType::Embedded => 500,
            };

            config.database.query_cache_size =
                (config.database.query_cache_size * 3 / 2).min(platform_max);
            config.database.index_cache_size =
                (config.database.index_cache_size * 3 / 2).min(platform_max * 2);

            log::info!("High DB latency ({}ms), increased cache sizes", db_latency);
        }
    }

    /// Adapt gaming limits based on consensus performance
    fn adapt_gaming_limits(&self, metrics: &crate::performance::PerformanceMetrics) {
        let mut config = self.config.write();

        let consensus_throughput = metrics.consensus_performance.throughput_ops_per_sec;

        if consensus_throughput < 10.0 {
            // Reduce concurrent games to improve performance
            config.gaming.max_concurrent_games =
                (config.gaming.max_concurrent_games * 2 / 3).max(1);
            log::info!(
                "Low consensus throughput ({}), reduced max games to {}",
                consensus_throughput,
                config.gaming.max_concurrent_games
            );
        } else if consensus_throughput > 100.0 {
            // Increase concurrent games if performance is good
            let platform_max = match config.platform {
                PlatformType::Mobile => 20,
                PlatformType::Desktop => 100,
                PlatformType::Server => 1000,
                PlatformType::Embedded => 5,
            };

            config.gaming.max_concurrent_games =
                (config.gaming.max_concurrent_games * 4 / 3).min(platform_max);
            log::info!(
                "High consensus throughput ({}), increased max games to {}",
                consensus_throughput,
                config.gaming.max_concurrent_games
            );
        }
    }
}

impl Default for ScalabilityManager {
    fn default() -> Self {
        Self::new(ScalabilityConfig::default())
    }
}
