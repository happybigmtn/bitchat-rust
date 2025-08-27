//! Performance tuning configuration for BitCraps
//! 
//! Provides runtime-adjustable performance parameters optimized for different scenarios

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Performance profile for different deployment scenarios
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PerformanceProfile {
    /// Optimized for battery life on mobile devices
    PowerSaving,
    /// Balanced performance and battery usage
    Balanced,
    /// Maximum performance, higher battery drain
    HighPerformance,
    /// Custom profile with specific parameters
    Custom(PerformanceConfig),
}

/// Comprehensive performance configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Network performance settings
    pub network: NetworkPerformanceConfig,
    
    /// Consensus performance settings
    pub consensus: ConsensusPerformanceConfig,
    
    /// Database performance settings
    pub database: DatabasePerformanceConfig,
    
    /// Bluetooth/BLE performance settings
    pub bluetooth: BluetoothPerformanceConfig,
    
    /// CPU utilization settings
    pub cpu: CpuPerformanceConfig,
    
    /// Memory management settings
    pub memory: MemoryPerformanceConfig,
    
    /// Battery optimization settings
    pub battery: BatteryOptimizationConfig,
}

/// Network performance configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPerformanceConfig {
    /// Maximum concurrent connections
    pub max_connections: usize,
    
    /// Connection timeout
    pub connection_timeout: Duration,
    
    /// Message batch size for efficiency
    pub message_batch_size: usize,
    
    /// Enable message compression for large payloads
    pub enable_compression: bool,
    
    /// Compression threshold in bytes
    pub compression_threshold: usize,
    
    /// Network buffer size
    pub buffer_size: usize,
    
    /// Keepalive interval
    pub keepalive_interval: Duration,
    
    /// Maximum retry attempts
    pub max_retries: u32,
    
    /// Retry backoff multiplier
    pub retry_backoff_multiplier: f64,
}

/// Consensus performance configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusPerformanceConfig {
    /// Consensus round timeout
    pub round_timeout: Duration,
    
    /// Vote collection timeout
    pub vote_timeout: Duration,
    
    /// Maximum operations per batch
    pub batch_size: usize,
    
    /// Enable parallel signature validation
    pub parallel_validation: bool,
    
    /// Signature validation thread pool size
    pub validation_threads: usize,
    
    /// Vote cache size
    pub vote_cache_size: usize,
    
    /// Enable fast-path for unanimous decisions
    pub enable_fast_path: bool,
    
    /// Quorum calculation optimization
    pub optimized_quorum: bool,
    
    /// Fork detection interval
    pub fork_detection_interval: Duration,
}

/// Database performance configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabasePerformanceConfig {
    /// Connection pool size
    pub connection_pool_size: usize,
    
    /// Cache size in MB
    pub cache_size_mb: usize,
    
    /// Memory-mapped I/O size in MB
    pub mmap_size_mb: usize,
    
    /// Write-ahead log auto-checkpoint interval
    pub wal_checkpoint_interval: Duration,
    
    /// Transaction batch size
    pub transaction_batch_size: usize,
    
    /// Enable query optimization
    pub enable_query_optimization: bool,
    
    /// Index cache size
    pub index_cache_size: usize,
    
    /// Vacuum schedule interval
    pub vacuum_interval: Duration,
}

/// Bluetooth/BLE performance configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BluetoothPerformanceConfig {
    /// BLE scan interval
    pub scan_interval: Duration,
    
    /// BLE scan window
    pub scan_window: Duration,
    
    /// Advertisement interval
    pub advertisement_interval: Duration,
    
    /// Connection interval min
    pub connection_interval_min: Duration,
    
    /// Connection interval max
    pub connection_interval_max: Duration,
    
    /// Connection latency (number of intervals that can be skipped)
    pub connection_latency: u16,
    
    /// Supervision timeout
    pub supervision_timeout: Duration,
    
    /// MTU size for BLE connections
    pub mtu_size: u16,
    
    /// Enable adaptive scanning
    pub adaptive_scanning: bool,
    
    /// Power level (0-100)
    pub tx_power_level: u8,
}

/// CPU performance configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuPerformanceConfig {
    /// Target CPU utilization percentage
    pub target_utilization: f64,
    
    /// Worker thread pool size
    pub worker_threads: usize,
    
    /// Async runtime threads
    pub async_threads: usize,
    
    /// Enable CPU affinity
    pub enable_affinity: bool,
    
    /// Task priority levels
    pub enable_priority_scheduling: bool,
    
    /// Batch processing threshold
    pub batch_threshold: usize,
    
    /// Sleep duration when idle
    pub idle_sleep_duration: Duration,
}

/// Memory performance configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryPerformanceConfig {
    /// Maximum heap size in MB
    pub max_heap_mb: usize,
    
    /// Object pool sizes
    pub pool_sizes: ObjectPoolConfig,
    
    /// Cache eviction policy
    pub eviction_policy: EvictionPolicy,
    
    /// Memory pressure threshold (percentage)
    pub pressure_threshold: f64,
    
    /// Enable memory compression
    pub enable_compression: bool,
    
    /// Garbage collection interval
    pub gc_interval: Duration,
    
    /// Pre-allocation sizes
    pub preallocate_buffers: bool,
}

/// Object pool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectPoolConfig {
    /// Message pool size
    pub message_pool: usize,
    
    /// Connection pool size
    pub connection_pool: usize,
    
    /// Buffer pool size
    pub buffer_pool: usize,
    
    /// Transaction pool size
    pub transaction_pool: usize,
}

/// Cache eviction policies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EvictionPolicy {
    /// Least Recently Used
    LRU,
    /// Least Frequently Used
    LFU,
    /// First In First Out
    FIFO,
    /// Adaptive Replacement Cache
    ARC,
}

/// Battery optimization configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatteryOptimizationConfig {
    /// Enable battery optimization
    pub enabled: bool,
    
    /// Low battery threshold (percentage)
    pub low_battery_threshold: u8,
    
    /// Critical battery threshold (percentage)
    pub critical_battery_threshold: u8,
    
    /// Reduce activity when on battery
    pub reduce_on_battery: bool,
    
    /// Suspend threshold (percentage)
    pub suspend_threshold: u8,
    
    /// Wake lock strategy
    pub wake_lock_strategy: WakeLockStrategy,
    
    /// Doze mode compatibility
    pub doze_compatible: bool,
    
    /// Background execution limits
    pub background_limits: BackgroundLimits,
}

/// Wake lock strategies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WakeLockStrategy {
    /// No wake locks
    None,
    /// Partial wake lock (CPU only)
    Partial,
    /// Full wake lock (CPU + Screen)
    Full,
    /// Adaptive based on activity
    Adaptive,
}

/// Background execution limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackgroundLimits {
    /// Maximum background CPU time per hour (seconds)
    pub max_cpu_seconds: u32,
    
    /// Maximum network requests per hour
    pub max_network_requests: u32,
    
    /// Maximum database operations per hour
    pub max_db_operations: u32,
}

impl Default for PerformanceProfile {
    fn default() -> Self {
        Self::Balanced
    }
}

impl PerformanceProfile {
    /// Get the performance configuration for this profile
    pub fn get_config(&self) -> PerformanceConfig {
        match self {
            Self::PowerSaving => Self::power_saving_config(),
            Self::Balanced => Self::balanced_config(),
            Self::HighPerformance => Self::high_performance_config(),
            Self::Custom(config) => config.clone(),
        }
    }
    
    /// Power saving configuration for mobile devices
    fn power_saving_config() -> PerformanceConfig {
        PerformanceConfig {
            network: NetworkPerformanceConfig {
                max_connections: 10,
                connection_timeout: Duration::from_secs(30),
                message_batch_size: 50,
                enable_compression: true,
                compression_threshold: 1024,
                buffer_size: 4096,
                keepalive_interval: Duration::from_secs(120),
                max_retries: 3,
                retry_backoff_multiplier: 2.0,
            },
            consensus: ConsensusPerformanceConfig {
                round_timeout: Duration::from_secs(10),
                vote_timeout: Duration::from_secs(5),
                batch_size: 20,
                parallel_validation: false,
                validation_threads: 1,
                vote_cache_size: 100,
                enable_fast_path: true,
                optimized_quorum: true,
                fork_detection_interval: Duration::from_secs(60),
            },
            database: DatabasePerformanceConfig {
                connection_pool_size: 2,
                cache_size_mb: 16,
                mmap_size_mb: 64,
                wal_checkpoint_interval: Duration::from_secs(300),
                transaction_batch_size: 10,
                enable_query_optimization: true,
                index_cache_size: 1000,
                vacuum_interval: Duration::from_secs(86400),
            },
            bluetooth: BluetoothPerformanceConfig {
                scan_interval: Duration::from_millis(5000),
                scan_window: Duration::from_millis(1000),
                advertisement_interval: Duration::from_millis(2000),
                connection_interval_min: Duration::from_millis(100),
                connection_interval_max: Duration::from_millis(200),
                connection_latency: 4,
                supervision_timeout: Duration::from_secs(10),
                mtu_size: 185,
                adaptive_scanning: true,
                tx_power_level: 50,
            },
            cpu: CpuPerformanceConfig {
                target_utilization: 30.0,
                worker_threads: 2,
                async_threads: 2,
                enable_affinity: false,
                enable_priority_scheduling: true,
                batch_threshold: 20,
                idle_sleep_duration: Duration::from_millis(100),
            },
            memory: MemoryPerformanceConfig {
                max_heap_mb: 128,
                pool_sizes: ObjectPoolConfig {
                    message_pool: 100,
                    connection_pool: 10,
                    buffer_pool: 50,
                    transaction_pool: 20,
                },
                eviction_policy: EvictionPolicy::LRU,
                pressure_threshold: 80.0,
                enable_compression: true,
                gc_interval: Duration::from_secs(300),
                preallocate_buffers: false,
            },
            battery: BatteryOptimizationConfig {
                enabled: true,
                low_battery_threshold: 20,
                critical_battery_threshold: 5,
                reduce_on_battery: true,
                suspend_threshold: 3,
                wake_lock_strategy: WakeLockStrategy::Adaptive,
                doze_compatible: true,
                background_limits: BackgroundLimits {
                    max_cpu_seconds: 60,
                    max_network_requests: 100,
                    max_db_operations: 50,
                },
            },
        }
    }
    
    /// Balanced configuration for general use
    fn balanced_config() -> PerformanceConfig {
        PerformanceConfig {
            network: NetworkPerformanceConfig {
                max_connections: 50,
                connection_timeout: Duration::from_secs(20),
                message_batch_size: 100,
                enable_compression: true,
                compression_threshold: 2048,
                buffer_size: 8192,
                keepalive_interval: Duration::from_secs(60),
                max_retries: 5,
                retry_backoff_multiplier: 1.5,
            },
            consensus: ConsensusPerformanceConfig {
                round_timeout: Duration::from_secs(5),
                vote_timeout: Duration::from_secs(3),
                batch_size: 50,
                parallel_validation: true,
                validation_threads: 2,
                vote_cache_size: 500,
                enable_fast_path: true,
                optimized_quorum: true,
                fork_detection_interval: Duration::from_secs(30),
            },
            database: DatabasePerformanceConfig {
                connection_pool_size: 5,
                cache_size_mb: 64,
                mmap_size_mb: 256,
                wal_checkpoint_interval: Duration::from_secs(60),
                transaction_batch_size: 50,
                enable_query_optimization: true,
                index_cache_size: 5000,
                vacuum_interval: Duration::from_secs(43200),
            },
            bluetooth: BluetoothPerformanceConfig {
                scan_interval: Duration::from_millis(2000),
                scan_window: Duration::from_millis(500),
                advertisement_interval: Duration::from_millis(1000),
                connection_interval_min: Duration::from_millis(50),
                connection_interval_max: Duration::from_millis(100),
                connection_latency: 2,
                supervision_timeout: Duration::from_secs(5),
                mtu_size: 251,
                adaptive_scanning: true,
                tx_power_level: 75,
            },
            cpu: CpuPerformanceConfig {
                target_utilization: 60.0,
                worker_threads: 4,
                async_threads: 4,
                enable_affinity: true,
                enable_priority_scheduling: true,
                batch_threshold: 50,
                idle_sleep_duration: Duration::from_millis(50),
            },
            memory: MemoryPerformanceConfig {
                max_heap_mb: 512,
                pool_sizes: ObjectPoolConfig {
                    message_pool: 500,
                    connection_pool: 50,
                    buffer_pool: 200,
                    transaction_pool: 100,
                },
                eviction_policy: EvictionPolicy::ARC,
                pressure_threshold: 75.0,
                enable_compression: false,
                gc_interval: Duration::from_secs(120),
                preallocate_buffers: true,
            },
            battery: BatteryOptimizationConfig {
                enabled: true,
                low_battery_threshold: 15,
                critical_battery_threshold: 5,
                reduce_on_battery: false,
                suspend_threshold: 2,
                wake_lock_strategy: WakeLockStrategy::Partial,
                doze_compatible: true,
                background_limits: BackgroundLimits {
                    max_cpu_seconds: 300,
                    max_network_requests: 500,
                    max_db_operations: 200,
                },
            },
        }
    }
    
    /// High performance configuration for powerful devices
    fn high_performance_config() -> PerformanceConfig {
        PerformanceConfig {
            network: NetworkPerformanceConfig {
                max_connections: 200,
                connection_timeout: Duration::from_secs(10),
                message_batch_size: 500,
                enable_compression: false,
                compression_threshold: 10240,
                buffer_size: 65536,
                keepalive_interval: Duration::from_secs(30),
                max_retries: 10,
                retry_backoff_multiplier: 1.2,
            },
            consensus: ConsensusPerformanceConfig {
                round_timeout: Duration::from_secs(2),
                vote_timeout: Duration::from_secs(1),
                batch_size: 200,
                parallel_validation: true,
                validation_threads: 8,
                vote_cache_size: 2000,
                enable_fast_path: true,
                optimized_quorum: true,
                fork_detection_interval: Duration::from_secs(10),
            },
            database: DatabasePerformanceConfig {
                connection_pool_size: 20,
                cache_size_mb: 256,
                mmap_size_mb: 1024,
                wal_checkpoint_interval: Duration::from_secs(30),
                transaction_batch_size: 200,
                enable_query_optimization: true,
                index_cache_size: 20000,
                vacuum_interval: Duration::from_secs(21600),
            },
            bluetooth: BluetoothPerformanceConfig {
                scan_interval: Duration::from_millis(500),
                scan_window: Duration::from_millis(250),
                advertisement_interval: Duration::from_millis(200),
                connection_interval_min: Duration::from_millis(20),
                connection_interval_max: Duration::from_millis(40),
                connection_latency: 0,
                supervision_timeout: Duration::from_secs(2),
                mtu_size: 512,
                adaptive_scanning: false,
                tx_power_level: 100,
            },
            cpu: CpuPerformanceConfig {
                target_utilization: 90.0,
                worker_threads: num_cpus::get(),
                async_threads: num_cpus::get(),
                enable_affinity: true,
                enable_priority_scheduling: true,
                batch_threshold: 200,
                idle_sleep_duration: Duration::from_millis(10),
            },
            memory: MemoryPerformanceConfig {
                max_heap_mb: 2048,
                pool_sizes: ObjectPoolConfig {
                    message_pool: 2000,
                    connection_pool: 200,
                    buffer_pool: 1000,
                    transaction_pool: 500,
                },
                eviction_policy: EvictionPolicy::ARC,
                pressure_threshold: 90.0,
                enable_compression: false,
                gc_interval: Duration::from_secs(60),
                preallocate_buffers: true,
            },
            battery: BatteryOptimizationConfig {
                enabled: false,
                low_battery_threshold: 10,
                critical_battery_threshold: 3,
                reduce_on_battery: false,
                suspend_threshold: 1,
                wake_lock_strategy: WakeLockStrategy::Full,
                doze_compatible: false,
                background_limits: BackgroundLimits {
                    max_cpu_seconds: u32::MAX,
                    max_network_requests: u32::MAX,
                    max_db_operations: u32::MAX,
                },
            },
        }
    }
}

/// Runtime performance tuner that adjusts parameters based on system state
pub struct PerformanceTuner {
    profile: PerformanceProfile,
    config: PerformanceConfig,
    auto_tune: bool,
}

impl PerformanceTuner {
    /// Create a new performance tuner with the specified profile
    pub fn new(profile: PerformanceProfile) -> Self {
        let config = profile.get_config();
        Self {
            profile,
            config,
            auto_tune: true,
        }
    }
    
    /// Enable or disable auto-tuning
    pub fn set_auto_tune(&mut self, enabled: bool) {
        self.auto_tune = enabled;
    }
    
    /// Get the current configuration
    pub fn get_config(&self) -> &PerformanceConfig {
        &self.config
    }
    
    /// Update configuration based on system metrics
    pub fn tune(&mut self, metrics: &crate::performance::PerformanceMetrics) {
        if !self.auto_tune {
            return;
        }
        
        // Adjust network parameters based on latency
        if metrics.network_latency.p95_ms > 500.0 {
            self.config.network.enable_compression = true;
            self.config.network.message_batch_size = 
                (self.config.network.message_batch_size * 2).min(1000);
        }
        
        // Adjust consensus parameters based on throughput
        if metrics.consensus_performance.throughput_ops_per_sec < 50.0 {
            self.config.consensus.batch_size = 
                (self.config.consensus.batch_size * 2).min(500);
            self.config.consensus.parallel_validation = true;
        }
        
        // Adjust memory parameters based on usage
        if metrics.memory_usage.heap_used_mb > self.config.memory.max_heap_mb as f64 * 0.9 {
            // Trigger more aggressive GC
            self.config.memory.gc_interval = Duration::from_secs(30);
            // Reduce pool sizes
            self.config.memory.pool_sizes.message_pool /= 2;
            self.config.memory.pool_sizes.buffer_pool /= 2;
        }
        
        // Adjust CPU parameters based on utilization
        if metrics.cpu_usage.utilization_percent > self.config.cpu.target_utilization {
            // Reduce activity
            self.config.cpu.idle_sleep_duration = Duration::from_millis(100);
            self.config.cpu.batch_threshold *= 2;
        }
    }
}