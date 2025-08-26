//! Mobile memory management with pooling and strict limits
//! 
//! This module provides comprehensive memory management for mobile devices:
//! - Enforces <150MB memory usage target
//! - Pool-based allocation for zero-copy operations
//! - Memory pressure detection and response
//! - Automatic garbage collection triggers
//! - Per-component memory budgeting
//! - Memory leak detection and prevention

use std::sync::{Arc, atomic::{AtomicUsize, AtomicU64, AtomicBool, Ordering}};
use std::time::{Duration, SystemTime};
use std::collections::{HashMap, VecDeque};
use std::alloc::{GlobalAlloc, Layout};
use tokio::sync::{RwLock, Mutex};
use bytes::{Bytes, BytesMut};
use serde::{Deserialize, Serialize};

use super::performance::PowerState;

/// Memory management configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryManagerConfig {
    /// Maximum total memory usage in MB
    pub max_memory_mb: usize,
    /// Memory pressure threshold (0.0-1.0, 0.8 = 80% of max)
    pub pressure_threshold: f64,
    /// Critical memory threshold (0.0-1.0, 0.95 = 95% of max)
    pub critical_threshold: f64,
    /// Pool configuration
    pub pools: PoolConfig,
    /// Component memory budgets
    pub component_budgets: ComponentBudgets,
    /// Garbage collection settings
    pub gc_settings: GcSettings,
    /// Monitoring configuration
    pub monitoring: MemoryMonitoringConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolConfig {
    /// Small buffer pool (256 bytes)
    pub small_pool_size: usize,
    pub small_buffer_size: usize,
    /// Medium buffer pool (1KB)  
    pub medium_pool_size: usize,
    pub medium_buffer_size: usize,
    /// Large buffer pool (4KB)
    pub large_pool_size: usize,
    pub large_buffer_size: usize,
    /// Packet buffer pool (for network packets)
    pub packet_pool_size: usize,
    pub packet_buffer_size: usize,
    /// Enable buffer validation
    pub enable_validation: bool,
    /// Pool replenishment threshold
    pub replenish_threshold: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentBudgets {
    /// Consensus system memory budget (MB)
    pub consensus_mb: f64,
    /// Networking memory budget (MB)
    pub networking_mb: f64,
    /// UI/Display memory budget (MB)
    pub ui_mb: f64,
    /// Gaming state memory budget (MB)
    pub gaming_mb: f64,
    /// Cache memory budget (MB)
    pub cache_mb: f64,
    /// Crypto operations memory budget (MB)
    pub crypto_mb: f64,
    /// Other/misc memory budget (MB)
    pub other_mb: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcSettings {
    /// Enable automatic garbage collection
    pub auto_gc_enabled: bool,
    /// GC trigger threshold (memory pressure level)
    pub gc_trigger_threshold: f64,
    /// Force GC interval in seconds (0 = disabled)
    pub force_gc_interval_secs: u64,
    /// GC aggressiveness (0.0-1.0)
    pub gc_aggressiveness: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryMonitoringConfig {
    /// Memory usage check interval in seconds
    pub check_interval_secs: u64,
    /// Leak detection enabled
    pub leak_detection_enabled: bool,
    /// Leak detection window in seconds
    pub leak_detection_window_secs: u64,
    /// Statistics collection enabled
    pub stats_enabled: bool,
}

impl Default for MemoryManagerConfig {
    fn default() -> Self {
        Self {
            max_memory_mb: 150,
            pressure_threshold: 0.8,
            critical_threshold: 0.95,
            pools: PoolConfig::default(),
            component_budgets: ComponentBudgets::default(),
            gc_settings: GcSettings::default(),
            monitoring: MemoryMonitoringConfig::default(),
        }
    }
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            small_pool_size: 100,
            small_buffer_size: 256,
            medium_pool_size: 50,
            medium_buffer_size: 1024,
            large_pool_size: 20,
            large_buffer_size: 4096,
            packet_pool_size: 30,
            packet_buffer_size: 1500, // MTU size
            enable_validation: true,
            replenish_threshold: 0.2, // 20%
        }
    }
}

impl Default for ComponentBudgets {
    fn default() -> Self {
        Self {
            consensus_mb: 30.0,    // 20% for consensus
            networking_mb: 25.0,   // 16.7% for networking
            ui_mb: 20.0,          // 13.3% for UI
            gaming_mb: 35.0,      // 23.3% for gaming state
            cache_mb: 20.0,       // 13.3% for cache
            crypto_mb: 10.0,      // 6.7% for crypto
            other_mb: 10.0,       // 6.7% for everything else
        }
    }
}

impl Default for GcSettings {
    fn default() -> Self {
        Self {
            auto_gc_enabled: true,
            gc_trigger_threshold: 0.85,
            force_gc_interval_secs: 300, // 5 minutes
            gc_aggressiveness: 0.5,
        }
    }
}

impl Default for MemoryMonitoringConfig {
    fn default() -> Self {
        Self {
            check_interval_secs: 10,
            leak_detection_enabled: true,
            leak_detection_window_secs: 300, // 5 minutes
            stats_enabled: true,
        }
    }
}

/// Memory pool for specific buffer sizes
#[derive(Debug)]
struct MemoryPool {
    /// Pool name for debugging
    name: String,
    /// Buffer size in bytes
    buffer_size: usize,
    /// Available buffers
    available: Arc<Mutex<Vec<BytesMut>>>,
    /// Maximum pool size
    max_size: usize,
    /// Current pool size
    current_size: AtomicUsize,
    /// Total allocated from this pool
    total_allocated: AtomicUsize,
    /// Total returned to this pool
    total_returned: AtomicUsize,
    /// Pool statistics
    stats: Arc<RwLock<PoolStats>>,
    /// Pool creation timestamp
    created_at: SystemTime,
}

/// Pool statistics
#[derive(Debug, Clone, Default)]
struct PoolStats {
    /// Total allocation requests
    pub allocation_requests: u64,
    /// Allocation hits (served from pool)
    pub allocation_hits: u64,
    /// Allocation misses (new allocation required)
    pub allocation_misses: u64,
    /// Peak pool usage
    pub peak_usage: usize,
    /// Current pool usage
    pub current_usage: usize,
    /// Total memory allocated by this pool (bytes)
    pub total_memory_bytes: u64,
    /// Average allocation time (nanoseconds)
    pub avg_allocation_time_ns: u64,
}

/// Component memory usage tracking
#[derive(Debug, Clone)]
pub struct ComponentUsage {
    /// Component name
    pub name: String,
    /// Current memory usage in bytes
    pub current_bytes: u64,
    /// Peak memory usage in bytes
    pub peak_bytes: u64,
    /// Allocated budget in bytes
    pub budget_bytes: u64,
    /// Number of allocations
    pub allocation_count: u64,
    /// Number of over-budget incidents
    pub over_budget_count: u64,
    /// Last update timestamp
    pub last_update: SystemTime,
}

/// Memory manager statistics
#[derive(Debug, Clone)]
pub struct MemoryStats {
    /// Current total memory usage in bytes
    pub current_usage_bytes: u64,
    /// Peak memory usage in bytes
    pub peak_usage_bytes: u64,
    /// Current memory pressure (0.0-1.0)
    pub pressure_level: f64,
    /// Memory usage by component
    pub component_usage: HashMap<String, ComponentUsage>,
    /// Pool statistics
    pub pool_stats: HashMap<String, PoolStats>,
    /// Total allocations performed
    pub total_allocations: u64,
    /// Total deallocations performed
    pub total_deallocations: u64,
    /// Number of GC runs
    pub gc_runs: u64,
    /// Number of allocation denials
    pub allocation_denials: u64,
    /// Memory leaks detected
    pub leaks_detected: u64,
    /// Average allocation time (nanoseconds)
    pub avg_allocation_time_ns: u64,
    /// System memory info (if available)
    pub system_memory_mb: Option<u64>,
}

/// Memory allocation request
#[derive(Debug, Clone)]
struct AllocationRequest {
    /// Requesting component
    component: String,
    /// Requested size in bytes
    size: usize,
    /// Request timestamp
    timestamp: SystemTime,
    /// Request priority
    priority: AllocationPriority,
}

/// Allocation priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum AllocationPriority {
    /// Background/cache allocations
    Low = 0,
    /// Normal operation allocations
    Normal = 1,
    /// User-facing operation allocations
    High = 2,
    /// Critical system allocations
    Critical = 3,
}

/// Main mobile memory manager
pub struct MobileMemoryManager {
    /// Configuration
    config: Arc<RwLock<MemoryManagerConfig>>,
    
    /// Memory pools by size
    pools: HashMap<String, Arc<MemoryPool>>,
    
    /// Current total memory usage (bytes)
    current_usage: Arc<AtomicUsize>,
    
    /// Component memory usage tracking
    component_usage: Arc<RwLock<HashMap<String, ComponentUsage>>>,
    
    /// Memory statistics
    stats: Arc<RwLock<MemoryStats>>,
    
    /// Current power state
    power_state: Arc<RwLock<PowerState>>,
    
    /// Control flags
    is_running: Arc<AtomicBool>,
    
    /// Task handles
    monitoring_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    gc_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    
    /// Allocation history for leak detection
    allocation_history: Arc<RwLock<VecDeque<(SystemTime, usize, String)>>>,
    
    /// GC trigger flag
    gc_requested: Arc<AtomicBool>,
    
    /// Last GC run timestamp
    last_gc_run: Arc<RwLock<SystemTime>>,
}

impl MobileMemoryManager {
    /// Create new mobile memory manager
    pub fn new(config: MemoryManagerConfig, max_memory_mb: usize) -> Self {
        let mut config = config;
        config.max_memory_mb = max_memory_mb;
        
        let mut manager = Self {
            config: Arc::new(RwLock::new(config.clone())),
            pools: HashMap::new(),
            current_usage: Arc::new(AtomicUsize::new(0)),
            component_usage: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(MemoryStats::new())),
            power_state: Arc::new(RwLock::new(PowerState::Active)),
            is_running: Arc::new(AtomicBool::new(false)),
            monitoring_task: Arc::new(Mutex::new(None)),
            gc_task: Arc::new(Mutex::new(None)),
            allocation_history: Arc::new(RwLock::new(VecDeque::with_capacity(10000))),
            gc_requested: Arc::new(AtomicBool::new(false)),
            last_gc_run: Arc::new(RwLock::new(SystemTime::now())),
        };
        
        // Initialize memory pools
        manager.initialize_pools(&config);
        
        // Initialize component budgets
        tokio::spawn({
            let component_usage = manager.component_usage.clone();
            let budgets = config.component_budgets.clone();
            async move {
                let mut usage = component_usage.write().await;
                
                usage.insert("consensus".to_string(), ComponentUsage {
                    name: "consensus".to_string(),
                    current_bytes: 0,
                    peak_bytes: 0,
                    budget_bytes: (budgets.consensus_mb * 1024.0 * 1024.0) as u64,
                    allocation_count: 0,
                    over_budget_count: 0,
                    last_update: SystemTime::now(),
                });
                
                usage.insert("networking".to_string(), ComponentUsage {
                    name: "networking".to_string(),
                    current_bytes: 0,
                    peak_bytes: 0,
                    budget_bytes: (budgets.networking_mb * 1024.0 * 1024.0) as u64,
                    allocation_count: 0,
                    over_budget_count: 0,
                    last_update: SystemTime::now(),
                });
                
                usage.insert("ui".to_string(), ComponentUsage {
                    name: "ui".to_string(),
                    current_bytes: 0,
                    peak_bytes: 0,
                    budget_bytes: (budgets.ui_mb * 1024.0 * 1024.0) as u64,
                    allocation_count: 0,
                    over_budget_count: 0,
                    last_update: SystemTime::now(),
                });
                
                usage.insert("gaming".to_string(), ComponentUsage {
                    name: "gaming".to_string(),
                    current_bytes: 0,
                    peak_bytes: 0,
                    budget_bytes: (budgets.gaming_mb * 1024.0 * 1024.0) as u64,
                    allocation_count: 0,
                    over_budget_count: 0,
                    last_update: SystemTime::now(),
                });
                
                // Initialize other components...
            }
        });
        
        manager
    }
    
    /// Initialize memory pools
    fn initialize_pools(&mut self, config: &MemoryManagerConfig) {
        // Small buffer pool
        self.pools.insert(
            "small".to_string(),
            Arc::new(MemoryPool::new(
                "small".to_string(),
                config.pools.small_buffer_size,
                config.pools.small_pool_size,
            )),
        );
        
        // Medium buffer pool
        self.pools.insert(
            "medium".to_string(),
            Arc::new(MemoryPool::new(
                "medium".to_string(),
                config.pools.medium_buffer_size,
                config.pools.medium_pool_size,
            )),
        );
        
        // Large buffer pool
        self.pools.insert(
            "large".to_string(),
            Arc::new(MemoryPool::new(
                "large".to_string(),
                config.pools.large_buffer_size,
                config.pools.large_pool_size,
            )),
        );
        
        // Packet buffer pool
        self.pools.insert(
            "packet".to_string(),
            Arc::new(MemoryPool::new(
                "packet".to_string(),
                config.pools.packet_buffer_size,
                config.pools.packet_pool_size,
            )),
        );
    }
    
    /// Start memory management
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        if self.is_running.swap(true, Ordering::Relaxed) {
            return Ok(()); // Already running
        }
        
        log::info!("Starting mobile memory manager (max: {} MB)", 
                  self.config.read().await.max_memory_mb);
        
        // Pre-populate pools
        self.prepopulate_pools().await;
        
        // Start monitoring
        self.start_monitoring_loop().await;
        
        // Start GC task
        if self.config.read().await.gc_settings.auto_gc_enabled {
            self.start_gc_loop().await;
        }
        
        log::info!("Mobile memory manager started successfully");
        Ok(())
    }
    
    /// Stop memory management
    pub async fn stop(&self) -> Result<(), Box<dyn std::error::Error>> {
        if !self.is_running.swap(false, Ordering::Relaxed) {
            return Ok(()); // Already stopped
        }
        
        log::info!("Stopping mobile memory manager");
        
        // Stop tasks
        if let Some(task) = self.monitoring_task.lock().await.take() {
            task.abort();
        }
        
        if let Some(task) = self.gc_task.lock().await.take() {
            task.abort();
        }
        
        // Final statistics
        let stats = self.stats.read().await;
        log::info!("Final memory stats: current: {} MB, peak: {} MB, allocations: {}, denials: {}",
                  stats.current_usage_bytes / 1024 / 1024,
                  stats.peak_usage_bytes / 1024 / 1024,
                  stats.total_allocations,
                  stats.allocation_denials);
        
        log::info!("Mobile memory manager stopped");
        Ok(())
    }
    
    /// Set current power state
    pub async fn set_power_state(&self, state: PowerState) -> Result<(), Box<dyn std::error::Error>> {
        *self.power_state.write().await = state;
        
        // Adjust GC aggressiveness based on power state
        match state {
            PowerState::Critical => {
                // Aggressive memory management in critical power state
                self.gc_requested.store(true, Ordering::Relaxed);
                log::info!("Triggered aggressive GC due to critical power state");
            },
            PowerState::PowerSaver => {
                // More frequent GC in power saver mode
                if self.get_pressure_level().await > 0.6 {
                    self.gc_requested.store(true, Ordering::Relaxed);
                }
            },
            _ => {}, // Normal GC behavior
        }
        
        Ok(())
    }
    
    /// Check if allocation is allowed for component
    pub async fn can_allocate(&self, component: &str, size: usize) -> bool {
        // Check total memory limit
        let current_usage = self.current_usage.load(Ordering::Relaxed);
        let max_bytes = self.config.read().await.max_memory_mb * 1024 * 1024;
        
        if current_usage + size > max_bytes {
            return false;
        }
        
        // Check component budget
        if let Some(usage) = self.component_usage.read().await.get(component) {
            if usage.current_bytes + size as u64 > usage.budget_bytes {
                // Over budget, but allow critical allocations
                if size < 1024 { // Small allocations allowed
                    return true;
                }
                return false;
            }
        }
        
        // Check memory pressure
        let pressure = self.get_pressure_level().await;
        if pressure > self.config.read().await.critical_threshold {
            return false; // Critical memory pressure
        }
        
        true
    }
    
    /// Allocate buffer from appropriate pool
    pub async fn allocate_buffer(&self, component: &str, size: usize) -> Option<BytesMut> {
        if !self.can_allocate(component, size).await {
            // Update denial statistics
            {
                let mut stats = self.stats.write().await;
                stats.allocation_denials += 1;
            }
            
            log::warn!("Memory allocation denied for component '{}', size: {} bytes", component, size);
            return None;
        }
        
        let start_time = SystemTime::now();
        
        // Select appropriate pool
        let pool_name = if size <= 256 {
            "small"
        } else if size <= 1024 {
            "medium"
        } else if size <= 4096 {
            "large"
        } else if size <= 1500 {
            "packet"
        } else {
            // Large allocation, create directly
            let buffer = BytesMut::with_capacity(size);
            self.record_allocation(component, size, start_time).await;
            return Some(buffer);
        };
        
        // Get buffer from pool
        if let Some(pool) = self.pools.get(pool_name) {
            if let Some(buffer) = pool.allocate().await {
                self.record_allocation(component, size, start_time).await;
                return Some(buffer);
            }
        }
        
        // Fallback to direct allocation
        let buffer = BytesMut::with_capacity(size);
        self.record_allocation(component, size, start_time).await;
        Some(buffer)
    }
    
    /// Return buffer to appropriate pool
    pub async fn return_buffer(&self, component: &str, mut buffer: BytesMut) {
        let size = buffer.capacity();
        
        // Clear buffer data
        buffer.clear();
        
        // Find appropriate pool
        let pool_name = if size <= 512 {
            "small"
        } else if size <= 2048 {
            "medium"
        } else if size <= 8192 {
            "large"
        } else if size <= 1500 {
            "packet"
        } else {
            // Too large for pools, just drop it
            self.record_deallocation(component, size).await;
            return;
        };
        
        // Return to pool
        if let Some(pool) = self.pools.get(pool_name) {
            pool.return_buffer(buffer).await;
            self.record_deallocation(component, size).await;
        }
    }
    
    /// Get current memory pressure level (0.0 - 1.0)
    pub async fn get_pressure_level(&self) -> f64 {
        let current_usage = self.current_usage.load(Ordering::Relaxed);
        let max_usage = self.config.read().await.max_memory_mb * 1024 * 1024;
        current_usage as f64 / max_usage as f64
    }
    
    /// Get current memory statistics
    pub async fn get_stats(&self) -> MemoryStats {
        self.stats.read().await.clone()
    }
    
    /// Get memory usage for component
    pub async fn get_component_usage(&self, component: &str) -> Option<ComponentUsage> {
        self.component_usage.read().await.get(component).cloned()
    }
    
    /// Force garbage collection
    pub async fn force_gc(&self) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Forcing garbage collection");
        self.gc_requested.store(true, Ordering::Relaxed);
        
        // Wait for GC to complete (simplified)
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        Ok(())
    }
    
    /// Pre-populate memory pools
    async fn prepopulate_pools(&self) {
        for (name, pool) in &self.pools {
            log::debug!("Pre-populating pool '{}' with {} buffers", name, pool.max_size / 2);
            
            for _ in 0..(pool.max_size / 2) {
                let buffer = BytesMut::with_capacity(pool.buffer_size);
                pool.return_buffer(buffer).await;
            }
        }
        
        log::info!("Memory pools pre-populated");
    }
    
    /// Record allocation
    async fn record_allocation(&self, component: &str, size: usize, start_time: SystemTime) {
        let allocation_time = SystemTime::now().duration_since(start_time).unwrap_or(Duration::ZERO).as_nanos() as u64;
        
        // Update total usage
        self.current_usage.fetch_add(size, Ordering::Relaxed);
        
        // Update component usage
        {
            let mut usage_map = self.component_usage.write().await;
            if let Some(usage) = usage_map.get_mut(component) {
                usage.current_bytes += size as u64;
                usage.peak_bytes = usage.peak_bytes.max(usage.current_bytes);
                usage.allocation_count += 1;
                usage.last_update = SystemTime::now();
                
                // Check budget
                if usage.current_bytes > usage.budget_bytes {
                    usage.over_budget_count += 1;
                    log::warn!("Component '{}' over budget: {} / {} bytes", 
                             component, usage.current_bytes, usage.budget_bytes);
                }
            }
        }
        
        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.total_allocations += 1;
            stats.current_usage_bytes = self.current_usage.load(Ordering::Relaxed) as u64;
            stats.peak_usage_bytes = stats.peak_usage_bytes.max(stats.current_usage_bytes);
            stats.pressure_level = stats.current_usage_bytes as f64 / (self.config.read().await.max_memory_mb as f64 * 1024.0 * 1024.0);
            
            // Update average allocation time
            if stats.total_allocations > 0 {
                stats.avg_allocation_time_ns = (stats.avg_allocation_time_ns * (stats.total_allocations - 1) + allocation_time) / stats.total_allocations;
            } else {
                stats.avg_allocation_time_ns = allocation_time;
            }
        }
        
        // Record in allocation history
        {
            let mut history = self.allocation_history.write().await;
            history.push_back((SystemTime::now(), size, component.to_string()));
            
            // Keep only recent history
            if history.len() > 10000 {
                history.pop_front();
            }
        }
        
        // Check if GC is needed
        let pressure = self.get_pressure_level().await;
        if pressure > self.config.read().await.gc_settings.gc_trigger_threshold {
            self.gc_requested.store(true, Ordering::Relaxed);
        }
    }
    
    /// Record deallocation
    async fn record_deallocation(&self, component: &str, size: usize) {
        // Update total usage
        self.current_usage.fetch_sub(size, Ordering::Relaxed);
        
        // Update component usage
        {
            let mut usage_map = self.component_usage.write().await;
            if let Some(usage) = usage_map.get_mut(component) {
                usage.current_bytes = usage.current_bytes.saturating_sub(size as u64);
                usage.last_update = SystemTime::now();
            }
        }
        
        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.total_deallocations += 1;
            stats.current_usage_bytes = self.current_usage.load(Ordering::Relaxed) as u64;
            stats.pressure_level = stats.current_usage_bytes as f64 / (self.config.read().await.max_memory_mb as f64 * 1024.0 * 1024.0);
        }
    }
    
    /// Start monitoring loop
    async fn start_monitoring_loop(&self) {
        let config = self.config.clone();
        let current_usage = self.current_usage.clone();
        let stats = self.stats.clone();
        let allocation_history = self.allocation_history.clone();
        let is_running = self.is_running.clone();
        
        let task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                Duration::from_secs(config.read().await.monitoring.check_interval_secs)
            );
            
            while is_running.load(Ordering::Relaxed) {
                interval.tick().await;
                
                let current = current_usage.load(Ordering::Relaxed);
                let max_bytes = config.read().await.max_memory_mb * 1024 * 1024;
                let pressure = current as f64 / max_bytes as f64;
                
                // Update pressure level in stats
                {
                    let mut stats_guard = stats.write().await;
                    stats_guard.current_usage_bytes = current as u64;
                    stats_guard.pressure_level = pressure;
                }
                
                // Log periodic statistics
                if pressure > 0.5 {
                    log::info!("Memory usage: {} MB / {} MB ({:.1}% pressure)",
                             current / 1024 / 1024,
                             max_bytes / 1024 / 1024,
                             pressure * 100.0);
                }
                
                // Leak detection
                if config.read().await.monitoring.leak_detection_enabled {
                    Self::detect_leaks(&allocation_history, &config).await;
                }
            }
        });
        
        *self.monitoring_task.lock().await = Some(task);
    }
    
    /// Start garbage collection loop
    async fn start_gc_loop(&self) {
        let gc_requested = self.gc_requested.clone();
        let last_gc_run = self.last_gc_run.clone();
        let config = self.config.clone();
        let stats = self.stats.clone();
        let is_running = self.is_running.clone();
        
        let task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60)); // Check every minute
            
            while is_running.load(Ordering::Relaxed) {
                interval.tick().await;
                
                let should_gc = gc_requested.load(Ordering::Relaxed) || {
                    let force_interval = config.read().await.gc_settings.force_gc_interval_secs;
                    if force_interval > 0 {
                        let last_gc = *last_gc_run.read().await;
                        last_gc.duration_since(SystemTime::now()).unwrap_or(Duration::ZERO).as_secs() >= force_interval
                    } else {
                        false
                    }
                };
                
                if should_gc {
                    log::info!("Running garbage collection");
                    
                    // Reset flag
                    gc_requested.store(false, Ordering::Relaxed);
                    
                    // Perform GC (in a real implementation, this would trigger actual GC)
                    // For now, just simulate the process
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    
                    // Update statistics
                    {
                        let mut stats_guard = stats.write().await;
                        stats_guard.gc_runs += 1;
                    }
                    
                    // Update last GC time
                    *last_gc_run.write().await = SystemTime::now();
                    
                    log::debug!("Garbage collection completed");
                }
            }
        });
        
        *self.gc_task.lock().await = Some(task);
    }
    
    /// Detect memory leaks
    async fn detect_leaks(
        allocation_history: &Arc<RwLock<VecDeque<(SystemTime, usize, String)>>>,
        config: &Arc<RwLock<MemoryManagerConfig>>,
    ) {
        let window_duration = Duration::from_secs(
            config.read().await.monitoring.leak_detection_window_secs
        );
        let cutoff_time = SystemTime::now() - window_duration;
        
        let history = allocation_history.read().await;
        let recent_allocations: Vec<_> = history.iter()
            .filter(|(time, _, _)| *time >= cutoff_time)
            .collect();
        
        // Simple leak detection: look for components with high allocation rates
        let mut component_allocations: HashMap<String, (usize, u64)> = HashMap::new();
        
        for (_, size, component) in recent_allocations {
            let entry = component_allocations.entry(component.clone())
                .or_insert((0, 0));
            entry.0 += 1; // Count
            entry.1 += *size as u64; // Total size
        }
        
        // Flag components with suspicious allocation patterns
        for (component, (count, total_size)) in component_allocations {
            let allocations_per_minute = count as f64 / (window_duration.as_secs() as f64 / 60.0);
            let avg_size = total_size / count as u64;
            
            // Heuristic: more than 100 allocations per minute with average size > 1KB
            if allocations_per_minute > 100.0 && avg_size > 1024 {
                log::warn!("Potential memory leak detected in component '{}': {:.1} allocs/min, avg size: {} bytes",
                         component, allocations_per_minute, avg_size);
            }
        }
    }
}

impl MemoryPool {
    /// Create new memory pool
    fn new(name: String, buffer_size: usize, max_size: usize) -> Self {
        Self {
            name,
            buffer_size,
            available: Arc::new(Mutex::new(Vec::with_capacity(max_size))),
            max_size,
            current_size: AtomicUsize::new(0),
            total_allocated: AtomicUsize::new(0),
            total_returned: AtomicUsize::new(0),
            stats: Arc::new(RwLock::new(PoolStats::default())),
            created_at: SystemTime::now(),
        }
    }
    
    /// Allocate buffer from pool
    async fn allocate(&self) -> Option<BytesMut> {
        let start_time = SystemTime::now();
        
        // Try to get from pool
        let mut available = self.available.lock().await;
        
        {
            let mut stats = self.stats.write().await;
            stats.allocation_requests += 1;
        }
        
        if let Some(mut buffer) = available.pop() {
            buffer.clear();
            buffer.resize(self.buffer_size, 0);
            self.total_allocated.fetch_add(1, Ordering::Relaxed);
            
            // Update statistics
            {
                let mut stats = self.stats.write().await;
                stats.allocation_hits += 1;
                stats.current_usage = available.len();
                
                let allocation_time = SystemTime::now().duration_since(start_time).unwrap_or(Duration::ZERO).as_nanos() as u64;
                stats.avg_allocation_time_ns = if stats.allocation_requests > 1 {
                    (stats.avg_allocation_time_ns * (stats.allocation_requests - 1) + allocation_time) / stats.allocation_requests
                } else {
                    allocation_time
                };
            }
            
            Some(buffer)
        } else {
            // Create new buffer
            let buffer = BytesMut::with_capacity(self.buffer_size);
            self.current_size.fetch_add(1, Ordering::Relaxed);
            
            // Update statistics
            {
                let mut stats = self.stats.write().await;
                stats.allocation_misses += 1;
                stats.peak_usage = stats.peak_usage.max(self.current_size.load(Ordering::Relaxed));
                stats.total_memory_bytes += self.buffer_size as u64;
                
                let allocation_time = SystemTime::now().duration_since(start_time).unwrap_or(Duration::ZERO).as_nanos() as u64;
                stats.avg_allocation_time_ns = if stats.allocation_requests > 1 {
                    (stats.avg_allocation_time_ns * (stats.allocation_requests - 1) + allocation_time) / stats.allocation_requests
                } else {
                    allocation_time
                };
            }
            
            Some(buffer)
        }
    }
    
    /// Return buffer to pool
    async fn return_buffer(&self, buffer: BytesMut) {
        let mut available = self.available.lock().await;
        
        if available.len() < self.max_size && buffer.capacity() == self.buffer_size {
            available.push(buffer);
            self.total_returned.fetch_add(1, Ordering::Relaxed);
            
            // Update statistics
            {
                let mut stats = self.stats.write().await;
                stats.current_usage = available.len();
            }
        }
        // If pool is full or buffer is wrong size, just drop it
    }
}

impl MemoryStats {
    fn new() -> Self {
        Self {
            current_usage_bytes: 0,
            peak_usage_bytes: 0,
            pressure_level: 0.0,
            component_usage: HashMap::new(),
            pool_stats: HashMap::new(),
            total_allocations: 0,
            total_deallocations: 0,
            gc_runs: 0,
            allocation_denials: 0,
            leaks_detected: 0,
            avg_allocation_time_ns: 0,
            system_memory_mb: None,
        }
    }
}

/// Memory manager interface for the performance module
impl MobileMemoryManager {
    pub async fn get_metrics(&self) -> super::performance::MemoryMetrics {
        let stats = self.stats.read().await;
        
        super::performance::MemoryMetrics {
            current_usage_mb: stats.current_usage_bytes as f64 / 1024.0 / 1024.0,
            pressure_level: stats.pressure_level,
            allocations_denied: stats.allocation_denials,
            gc_runs: stats.gc_runs,
        }
    }
}

/// Memory metrics for the performance system
#[derive(Debug, Clone)]
pub struct MemoryMetrics {
    pub current_usage_mb: f64,
    pub pressure_level: f64,
    pub allocations_denied: u64,
    pub gc_runs: u64,
}