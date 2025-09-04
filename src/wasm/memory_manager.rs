//! WASM memory management system
//!
//! This module provides secure memory management for WASM instances including:
//! - Memory allocation and deallocation
//! - Memory limits and quotas
//! - Garbage collection and cleanup
//! - Memory isolation between instances
//! - Protection against memory-based attacks

use crate::error::{Error, Result};
use dashmap::DashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Semaphore};
use uuid::Uuid;

/// WASM memory manager
pub struct WasmMemoryManager {
    /// Maximum total memory across all instances
    max_total_memory: usize,
    /// Currently allocated memory
    allocated_memory: AtomicUsize,
    /// Memory allocations per instance
    allocations: Arc<DashMap<Uuid, MemoryAllocation>>,
    /// Memory limits per instance
    instance_limits: Arc<DashMap<Uuid, usize>>,
    /// Memory pools for efficient allocation
    memory_pools: Arc<RwLock<MemoryPools>>,
    /// Semaphore for controlling total allocations
    allocation_semaphore: Arc<Semaphore>,
    /// Garbage collection settings
    gc_config: GarbageCollectionConfig,
}

/// Memory allocation information
#[derive(Debug, Clone)]
pub struct MemoryAllocation {
    /// Instance ID
    pub instance_id: Uuid,
    /// Allocated memory size
    pub size: usize,
    /// Allocation time
    pub allocated_at: Instant,
    /// Last access time
    pub last_accessed: Instant,
    /// Number of accesses
    pub access_count: u64,
    /// Memory protection flags
    pub protection: MemoryProtection,
}

/// Memory protection flags
#[derive(Debug, Clone, Copy)]
pub struct MemoryProtection {
    /// Memory is readable
    pub readable: bool,
    /// Memory is writable
    pub writable: bool,
    /// Memory is executable (should be false for WASM data)
    pub executable: bool,
}

impl Default for MemoryProtection {
    fn default() -> Self {
        Self {
            readable: true,
            writable: true,
            executable: false, // Never allow execution of data memory
        }
    }
}

/// Memory pools for efficient allocation
#[derive(Debug)]
pub struct MemoryPools {
    /// Small allocations (< 4KB)
    small_pool: Vec<Vec<u8>>,
    /// Medium allocations (4KB - 64KB)
    medium_pool: Vec<Vec<u8>>,
    /// Large allocations (64KB - 1MB)
    large_pool: Vec<Vec<u8>>,
    /// Pool statistics
    stats: MemoryPoolStats,
}

/// Memory pool statistics
#[derive(Debug, Clone, Default)]
pub struct MemoryPoolStats {
    pub small_pool_size: usize,
    pub medium_pool_size: usize,
    pub large_pool_size: usize,
    pub pool_hits: u64,
    pub pool_misses: u64,
    pub total_allocations: u64,
    pub total_deallocations: u64,
}

/// Garbage collection configuration
#[derive(Debug, Clone)]
pub struct GarbageCollectionConfig {
    /// Enable automatic garbage collection
    pub enabled: bool,
    /// GC interval
    pub gc_interval: Duration,
    /// Maximum unused memory before triggering GC
    pub max_unused_memory: usize,
    /// Memory age threshold for collection
    pub memory_age_threshold: Duration,
}

impl Default for GarbageCollectionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            gc_interval: Duration::from_secs(60),
            max_unused_memory: 64 * 1024 * 1024, // 64MB
            memory_age_threshold: Duration::from_secs(300), // 5 minutes
        }
    }
}

/// Memory usage statistics
#[derive(Debug, Clone, Default)]
pub struct MemoryStats {
    pub total_allocated: usize,
    pub peak_allocation: usize,
    pub current_instances: usize,
    pub allocation_count: u64,
    pub deallocation_count: u64,
    pub gc_runs: u64,
    pub memory_fragmentation: f64,
    pub pool_stats: MemoryPoolStats,
}

impl WasmMemoryManager {
    /// Create a new memory manager
    pub fn new(max_total_memory: usize) -> Self {
        let max_permits = (max_total_memory / 4096).max(1); // Allow at least 1 permit
        
        Self {
            max_total_memory,
            allocated_memory: AtomicUsize::new(0),
            allocations: Arc::new(DashMap::new()),
            instance_limits: Arc::new(DashMap::new()),
            memory_pools: Arc::new(RwLock::new(MemoryPools::new())),
            allocation_semaphore: Arc::new(Semaphore::new(max_permits)),
            gc_config: GarbageCollectionConfig::default(),
        }
    }

    /// Allocate memory for a WASM instance
    pub async fn allocate_memory(&self, instance_id: Uuid, size: usize) -> Result<Vec<u8>> {
        // Check if instance already has memory allocated
        if self.allocations.contains_key(&instance_id) {
            return Err(Error::Wasm("Memory already allocated for instance".to_string()));
        }

        // Check total memory limit
        let current_allocated = self.allocated_memory.load(Ordering::Relaxed);
        if current_allocated + size > self.max_total_memory {
            return Err(Error::Wasm("Total memory limit exceeded".to_string()));
        }

        // Check instance-specific limit
        if let Some(limit) = self.instance_limits.get(&instance_id) {
            if size > *limit.value() {
                return Err(Error::Wasm("Instance memory limit exceeded".to_string()));
            }
        }

        // Acquire allocation permit
        let _permit = self.allocation_semaphore
            .acquire()
            .await
            .map_err(|_| Error::Wasm("Failed to acquire allocation permit".to_string()))?;

        // Try to get memory from pool first
        let memory = self.allocate_from_pool(size).await.unwrap_or_else(|| {
            // Allocate new memory if pool allocation fails
            vec![0u8; size]
        });

        // Update allocation tracking
        self.allocated_memory.fetch_add(size, Ordering::Relaxed);

        let allocation = MemoryAllocation {
            instance_id,
            size,
            allocated_at: Instant::now(),
            last_accessed: Instant::now(),
            access_count: 0,
            protection: MemoryProtection::default(),
        };

        self.allocations.insert(instance_id, allocation);

        // Update pool stats
        {
            let mut pools = self.memory_pools.write().await;
            pools.stats.total_allocations += 1;
        }

        log::debug!("Allocated {} bytes for instance {}", size, instance_id);
        Ok(memory)
    }

    /// Free memory for a WASM instance
    pub async fn free_memory(&self, instance_id: Uuid) -> Result<()> {
        let allocation = self.allocations.remove(&instance_id)
            .ok_or_else(|| Error::Wasm("No memory allocation found for instance".to_string()))?
            .1;

        // Update total allocated memory
        self.allocated_memory.fetch_sub(allocation.size, Ordering::Relaxed);

        // Return memory to pool if possible
        self.return_to_pool(allocation.size).await;

        // Update pool stats
        {
            let mut pools = self.memory_pools.write().await;
            pools.stats.total_deallocations += 1;
        }

        log::debug!("Freed {} bytes for instance {}", allocation.size, instance_id);
        Ok(())
    }

    /// Set memory limit for a specific instance
    pub async fn set_instance_limit(&self, instance_id: Uuid, limit: usize) {
        self.instance_limits.insert(instance_id, limit);
    }

    /// Remove memory limit for an instance
    pub async fn remove_instance_limit(&self, instance_id: Uuid) {
        self.instance_limits.remove(&instance_id);
    }

    /// Get current memory usage for an instance
    pub async fn get_instance_usage(&self, instance_id: Uuid) -> Option<usize> {
        self.allocations.get(&instance_id).map(|allocation| allocation.size)
    }

    /// Record memory access for an instance
    pub async fn record_access(&self, instance_id: Uuid) {
        if let Some(mut allocation) = self.allocations.get_mut(&instance_id) {
            allocation.last_accessed = Instant::now();
            allocation.access_count += 1;
        }
    }

    /// Get total allocated memory
    pub async fn get_total_usage(&self) -> usize {
        self.allocated_memory.load(Ordering::Relaxed)
    }

    /// Get memory statistics
    pub async fn get_stats(&self) -> MemoryStats {
        let pools = self.memory_pools.read().await;
        
        MemoryStats {
            total_allocated: self.allocated_memory.load(Ordering::Relaxed),
            peak_allocation: self.get_peak_allocation().await,
            current_instances: self.allocations.len(),
            allocation_count: pools.stats.total_allocations,
            deallocation_count: pools.stats.total_deallocations,
            gc_runs: 0, // Would be tracked in real GC implementation
            memory_fragmentation: self.calculate_fragmentation().await,
            pool_stats: pools.stats.clone(),
        }
    }

    /// Start garbage collection task
    pub async fn start_gc_task(&self) {
        if !self.gc_config.enabled {
            return;
        }

        let allocations = self.allocations.clone();
        let memory_pools = self.memory_pools.clone();
        let allocated_memory = AtomicUsize::new(self.allocated_memory.load(Ordering::Relaxed));
        let gc_config = self.gc_config.clone();

        crate::utils::spawn_tracked("wasm_memory_gc", crate::utils::TaskType::Background, async move {
            let mut interval = tokio::time::interval(gc_config.gc_interval);

            loop {
                interval.tick().await;

                // Find instances with old, unused memory
                let mut to_collect = Vec::new();
                let now = Instant::now();

                for entry in allocations.iter() {
                    let allocation = entry.value();
                    
                    // Check if memory is old and unused
                    if now.duration_since(allocation.last_accessed) > gc_config.memory_age_threshold {
                        to_collect.push(allocation.instance_id);
                    }
                }

                // Perform garbage collection
                if !to_collect.is_empty() {
                    log::debug!("Garbage collecting {} memory allocations", to_collect.len());

                    for instance_id in to_collect {
                        if let Some((_, allocation)) = allocations.remove(&instance_id) {
                            allocated_memory.fetch_sub(allocation.size, Ordering::Relaxed);
                            
                            // Return memory to pool
                            let mut pools = memory_pools.write().await;
                            Self::return_memory_to_pool(&mut pools, allocation.size);
                        }
                    }
                }

                // Trim memory pools if too large
                {
                    let mut pools = memory_pools.write().await;
                    let current_pool_size = pools.small_pool.len() * 4096 + 
                                          pools.medium_pool.len() * 65536 +
                                          pools.large_pool.len() * 1048576;
                    
                    if current_pool_size > gc_config.max_unused_memory {
                        Self::trim_memory_pools(&mut pools);
                    }
                }
            }
        }).await;
    }

    /// Allocate memory from pool
    async fn allocate_from_pool(&self, size: usize) -> Option<Vec<u8>> {
        let mut pools = self.memory_pools.write().await;

        let memory = if size <= 4096 && !pools.small_pool.is_empty() {
            pools.stats.pool_hits += 1;
            Some(pools.small_pool.pop().unwrap())
        } else if size <= 65536 && !pools.medium_pool.is_empty() {
            pools.stats.pool_hits += 1;
            Some(pools.medium_pool.pop().unwrap())
        } else if size <= 1048576 && !pools.large_pool.is_empty() {
            pools.stats.pool_hits += 1;
            Some(pools.large_pool.pop().unwrap())
        } else {
            pools.stats.pool_misses += 1;
            None
        };

        // Resize memory if necessary
        memory.map(|mut mem| {
            mem.resize(size, 0);
            mem
        })
    }

    /// Return memory to pool
    async fn return_to_pool(&self, size: usize) {
        let mut pools = self.memory_pools.write().await;
        Self::return_memory_to_pool(&mut pools, size);
    }

    /// Return memory to pool (internal)
    fn return_memory_to_pool(pools: &mut MemoryPools, size: usize) {
        // Only return memory to pool if it's not too large
        const MAX_POOL_SIZE: usize = 100; // Maximum items per pool

        if size <= 4096 && pools.small_pool.len() < MAX_POOL_SIZE {
            pools.small_pool.push(vec![0u8; 4096]);
            pools.stats.small_pool_size = pools.small_pool.len();
        } else if size <= 65536 && pools.medium_pool.len() < MAX_POOL_SIZE {
            pools.medium_pool.push(vec![0u8; 65536]);
            pools.stats.medium_pool_size = pools.medium_pool.len();
        } else if size <= 1048576 && pools.large_pool.len() < MAX_POOL_SIZE {
            pools.large_pool.push(vec![0u8; 1048576]);
            pools.stats.large_pool_size = pools.large_pool.len();
        }
    }

    /// Trim memory pools when they get too large
    fn trim_memory_pools(pools: &mut MemoryPools) {
        const TARGET_POOL_SIZE: usize = 50;

        // Trim small pool
        if pools.small_pool.len() > TARGET_POOL_SIZE {
            pools.small_pool.truncate(TARGET_POOL_SIZE);
            pools.stats.small_pool_size = pools.small_pool.len();
        }

        // Trim medium pool
        if pools.medium_pool.len() > TARGET_POOL_SIZE {
            pools.medium_pool.truncate(TARGET_POOL_SIZE);
            pools.stats.medium_pool_size = pools.medium_pool.len();
        }

        // Trim large pool
        if pools.large_pool.len() > TARGET_POOL_SIZE {
            pools.large_pool.truncate(TARGET_POOL_SIZE);
            pools.stats.large_pool_size = pools.large_pool.len();
        }

        log::debug!("Trimmed memory pools to reduce memory usage");
    }

    /// Get peak allocation (placeholder implementation)
    async fn get_peak_allocation(&self) -> usize {
        // In a real implementation, this would track the maximum allocation
        self.allocated_memory.load(Ordering::Relaxed)
    }

    /// Calculate memory fragmentation
    async fn calculate_fragmentation(&self) -> f64 {
        // Simplified fragmentation calculation
        let pools = self.memory_pools.read().await;
        let total_pool_memory = pools.small_pool.len() * 4096 + 
                               pools.medium_pool.len() * 65536 +
                               pools.large_pool.len() * 1048576;
        
        let allocated = self.allocated_memory.load(Ordering::Relaxed);
        
        if allocated > 0 {
            total_pool_memory as f64 / allocated as f64
        } else {
            0.0
        }
    }

    /// Check if instance has valid memory allocation
    pub fn has_allocation(&self, instance_id: Uuid) -> bool {
        self.allocations.contains_key(&instance_id)
    }

    /// Get all allocated instances
    pub fn get_allocated_instances(&self) -> Vec<Uuid> {
        self.allocations.iter().map(|entry| *entry.key()).collect()
    }

    /// Force garbage collection
    pub async fn force_gc(&self) -> Result<usize> {
        let mut collected = 0;
        let now = Instant::now();

        // Collect allocations older than threshold
        let to_collect: Vec<Uuid> = self.allocations
            .iter()
            .filter(|entry| {
                now.duration_since(entry.value().last_accessed) > self.gc_config.memory_age_threshold
            })
            .map(|entry| *entry.key())
            .collect();

        for instance_id in to_collect {
            if let Some((_, allocation)) = self.allocations.remove(&instance_id) {
                self.allocated_memory.fetch_sub(allocation.size, Ordering::Relaxed);
                collected += allocation.size;
            }
        }

        // Trim memory pools
        {
            let mut pools = self.memory_pools.write().await;
            Self::trim_memory_pools(&mut pools);
        }

        log::info!("Force GC collected {} bytes from {} instances", collected, to_collect.len());
        Ok(collected)
    }
}

impl MemoryPools {
    fn new() -> Self {
        Self {
            small_pool: Vec::new(),
            medium_pool: Vec::new(),
            large_pool: Vec::new(),
            stats: MemoryPoolStats::default(),
        }
    }
}

/// Memory manager builder for configuration
pub struct WasmMemoryManagerBuilder {
    max_total_memory: usize,
    gc_config: GarbageCollectionConfig,
}

impl WasmMemoryManagerBuilder {
    pub fn new() -> Self {
        Self {
            max_total_memory: 256 * 1024 * 1024, // 256MB default
            gc_config: GarbageCollectionConfig::default(),
        }
    }

    pub fn max_total_memory(mut self, bytes: usize) -> Self {
        self.max_total_memory = bytes;
        self
    }

    pub fn gc_interval(mut self, interval: Duration) -> Self {
        self.gc_config.gc_interval = interval;
        self
    }

    pub fn gc_memory_threshold(mut self, threshold: usize) -> Self {
        self.gc_config.max_unused_memory = threshold;
        self
    }

    pub fn gc_age_threshold(mut self, threshold: Duration) -> Self {
        self.gc_config.memory_age_threshold = threshold;
        self
    }

    pub fn disable_gc(mut self) -> Self {
        self.gc_config.enabled = false;
        self
    }

    pub fn build(self) -> WasmMemoryManager {
        let mut manager = WasmMemoryManager::new(self.max_total_memory);
        manager.gc_config = self.gc_config;
        manager
    }
}

impl Default for WasmMemoryManagerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_manager_creation() {
        let manager = WasmMemoryManager::new(64 * 1024 * 1024);
        
        assert_eq!(manager.get_total_usage().await, 0);
        let stats = manager.get_stats().await;
        assert_eq!(stats.total_allocated, 0);
        assert_eq!(stats.current_instances, 0);
    }

    #[tokio::test]
    async fn test_memory_allocation() {
        let manager = WasmMemoryManager::new(64 * 1024 * 1024);
        let instance_id = Uuid::new_v4();
        let size = 4096;

        let memory = manager.allocate_memory(instance_id, size).await.unwrap();
        
        assert_eq!(memory.len(), size);
        assert_eq!(manager.get_total_usage().await, size);
        assert_eq!(manager.get_instance_usage(instance_id).await, Some(size));
        assert!(manager.has_allocation(instance_id));
    }

    #[tokio::test]
    async fn test_memory_deallocation() {
        let manager = WasmMemoryManager::new(64 * 1024 * 1024);
        let instance_id = Uuid::new_v4();
        let size = 4096;

        let _memory = manager.allocate_memory(instance_id, size).await.unwrap();
        assert_eq!(manager.get_total_usage().await, size);

        manager.free_memory(instance_id).await.unwrap();
        assert_eq!(manager.get_total_usage().await, 0);
        assert!(!manager.has_allocation(instance_id));
    }

    #[tokio::test]
    async fn test_memory_limits() {
        let manager = WasmMemoryManager::new(8192); // Small total limit
        let instance_id = Uuid::new_v4();

        // Set instance limit
        manager.set_instance_limit(instance_id, 4096).await;

        // Should succeed within instance limit
        let _memory1 = manager.allocate_memory(instance_id, 4096).await.unwrap();

        // Should fail - instance limit exceeded
        let instance_id2 = Uuid::new_v4();
        manager.set_instance_limit(instance_id2, 2048).await;
        let result = manager.allocate_memory(instance_id2, 4096).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_memory_access_tracking() {
        let manager = WasmMemoryManager::new(64 * 1024 * 1024);
        let instance_id = Uuid::new_v4();

        let _memory = manager.allocate_memory(instance_id, 4096).await.unwrap();
        
        // Record access
        manager.record_access(instance_id).await;

        // Check that access was recorded
        if let Some(allocation) = manager.allocations.get(&instance_id) {
            assert_eq!(allocation.access_count, 1);
        }
    }

    #[tokio::test]
    async fn test_memory_protection() {
        let protection = MemoryProtection::default();
        
        assert!(protection.readable);
        assert!(protection.writable);
        assert!(!protection.executable); // Should never allow execution
    }

    #[tokio::test]
    async fn test_memory_pools() {
        let pools = MemoryPools::new();
        
        assert_eq!(pools.small_pool.len(), 0);
        assert_eq!(pools.medium_pool.len(), 0);
        assert_eq!(pools.large_pool.len(), 0);
        assert_eq!(pools.stats.total_allocations, 0);
    }

    #[tokio::test]
    async fn test_gc_config() {
        let config = GarbageCollectionConfig::default();
        
        assert!(config.enabled);
        assert_eq!(config.gc_interval, Duration::from_secs(60));
        assert_eq!(config.max_unused_memory, 64 * 1024 * 1024);
    }

    #[tokio::test]
    async fn test_memory_manager_builder() {
        let manager = WasmMemoryManagerBuilder::new()
            .max_total_memory(128 * 1024 * 1024)
            .gc_interval(Duration::from_secs(30))
            .gc_memory_threshold(32 * 1024 * 1024)
            .build();

        assert_eq!(manager.max_total_memory, 128 * 1024 * 1024);
        assert_eq!(manager.gc_config.gc_interval, Duration::from_secs(30));
        assert_eq!(manager.gc_config.max_unused_memory, 32 * 1024 * 1024);
    }
}