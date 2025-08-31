//! Memory Pool for Performance Optimization
//!
//! This module provides memory pooling for frequently allocated objects
//! to reduce allocation overhead and garbage collection pressure.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

/// Generic memory pool for reusable objects
pub struct MemoryPool<T> {
    pool: Arc<Mutex<VecDeque<T>>>,
    factory: Box<dyn Fn() -> T + Send + Sync>,
    max_size: usize,
    stats: Arc<Mutex<PoolStats>>,
}

/// Statistics for memory pool usage
#[derive(Debug, Clone)]
pub struct PoolStats {
    pub allocations: u64,
    pub deallocations: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub current_size: usize,
    pub max_size_reached: usize,
    pub last_reset: Instant,
}

impl Default for PoolStats {
    fn default() -> Self {
        Self {
            allocations: 0,
            deallocations: 0,
            cache_hits: 0,
            cache_misses: 0,
            current_size: 0,
            max_size_reached: 0,
            last_reset: Instant::now(),
        }
    }
}

/// A pooled object that returns to the pool when dropped
pub struct PooledObject<T: Send + 'static> {
    object: Option<T>,
    pool: Arc<Mutex<VecDeque<T>>>,
    stats: Arc<Mutex<PoolStats>>,
}

impl<T> MemoryPool<T>
where
    T: Default + Send + 'static,
{
    /// Create a new memory pool with default factory
    pub fn new(max_size: usize) -> Self {
        Self::with_factory(max_size, Box::new(|| T::default()))
    }
}

impl<T> MemoryPool<T>
where
    T: Send + 'static,
{
    /// Create a new memory pool with custom factory function
    pub fn with_factory<F>(max_size: usize, factory: F) -> Self
    where
        F: Fn() -> T + Send + Sync + 'static,
    {
        let mut stats = PoolStats::default();
        stats.last_reset = Instant::now();

        Self {
            pool: Arc::new(Mutex::new(VecDeque::with_capacity(max_size))),
            factory: Box::new(factory),
            max_size,
            stats: Arc::new(Mutex::new(stats)),
        }
    }

    /// Get an object from the pool, creating one if the pool is empty
    pub async fn get(&self) -> PooledObject<T> {
        let mut pool = self.pool.lock().await;
        let mut stats = self.stats.lock().await;

        stats.allocations += 1;

        if let Some(object) = pool.pop_front() {
            stats.cache_hits += 1;
            stats.current_size = pool.len();

            drop(pool);
            drop(stats);

            PooledObject {
                object: Some(object),
                pool: self.pool.clone(),
                stats: self.stats.clone(),
            }
        } else {
            stats.cache_misses += 1;

            drop(pool);
            drop(stats);

            let object = (self.factory)();

            PooledObject {
                object: Some(object),
                pool: self.pool.clone(),
                stats: self.stats.clone(),
            }
        }
    }

    /// Pre-populate the pool with objects
    pub async fn warmup(&self, count: usize) {
        let mut pool = self.pool.lock().await;
        let target_count = std::cmp::min(count, self.max_size);

        while pool.len() < target_count {
            pool.push_back((self.factory)());
        }

        let mut stats = self.stats.lock().await;
        stats.current_size = pool.len();
        stats.max_size_reached = std::cmp::max(stats.max_size_reached, pool.len());
    }

    /// Get current pool statistics
    pub async fn stats(&self) -> PoolStats {
        self.stats.lock().await.clone()
    }

    /// Reset pool statistics
    pub async fn reset_stats(&self) {
        let mut stats = self.stats.lock().await;
        *stats = PoolStats {
            last_reset: Instant::now(),
            current_size: stats.current_size,
            ..Default::default()
        };
    }

    /// Clear all objects from the pool
    pub async fn clear(&self) {
        let mut pool = self.pool.lock().await;
        pool.clear();

        let mut stats = self.stats.lock().await;
        stats.current_size = 0;
    }
}

impl<T: Send + 'static> PooledObject<T> {
    /// Get a reference to the pooled object
    pub fn as_ref(&self) -> &T {
        self.object
            .as_ref()
            .unwrap_or_else(|| {
                // This should never happen due to pool invariants
                eprintln!("CRITICAL: PooledObject accessed after object was taken");
                std::process::exit(1);
            })
    }

    /// Get a mutable reference to the pooled object
    pub fn as_mut(&mut self) -> &mut T {
        self.object
            .as_mut()
            .unwrap_or_else(|| {
                // This should never happen due to pool invariants
                eprintln!("CRITICAL: PooledObject accessed after object was taken");
                std::process::exit(1);
            })
    }

    /// Take ownership of the object, preventing it from returning to the pool
    pub fn into_inner(mut self) -> T {
        self.object
            .take()
            .unwrap_or_else(|| {
                // This should never happen due to pool invariants
                eprintln!("CRITICAL: PooledObject accessed after object was already taken");
                std::process::exit(1);
            })
    }
}

impl<T: Send + 'static> Drop for PooledObject<T> {
    fn drop(&mut self) {
        if let Some(object) = self.object.take() {
            // Return object to pool asynchronously
            let pool = self.pool.clone();
            let stats = self.stats.clone();

            tokio::spawn(async move {
                let mut pool_guard = pool.lock().await;
                let mut stats_guard = stats.lock().await;

                // Only add back to pool if it's not full
                if pool_guard.len() < pool_guard.capacity() {
                    pool_guard.push_back(object);
                    stats_guard.current_size = pool_guard.len();
                    stats_guard.max_size_reached =
                        std::cmp::max(stats_guard.max_size_reached, pool_guard.len());
                }

                stats_guard.deallocations += 1;
            });
        }
    }
}

impl<T: Send + 'static> std::ops::Deref for PooledObject<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl<T: Send + 'static> std::ops::DerefMut for PooledObject<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut()
    }
}

/// Specialized memory pools for common objects
pub struct GameMemoryPools {
    pub vec_u8_pool: MemoryPool<Vec<u8>>,
    pub string_pool: MemoryPool<String>,
    pub hashmap_pool: MemoryPool<std::collections::HashMap<String, String>>,
}

impl GameMemoryPools {
    /// Create a new set of game memory pools
    pub fn new() -> Self {
        Self {
            vec_u8_pool: MemoryPool::with_factory(100, || Vec::with_capacity(1024)),
            string_pool: MemoryPool::with_factory(50, || String::with_capacity(256)),
            hashmap_pool: MemoryPool::with_factory(25, || {
                std::collections::HashMap::with_capacity(16)
            }),
        }
    }

    /// Create memory pools from application configuration
    pub fn from_app_config(config: &crate::app::ApplicationConfig) -> Self {
        let vec_pool_size = config.vec_pool_size;
        let vec_pool_capacity = config.vec_pool_capacity;
        let string_pool_size = config.string_pool_size;
        let string_pool_capacity = config.string_pool_capacity;
        
        Self {
            vec_u8_pool: MemoryPool::with_factory(vec_pool_size, move || {
                Vec::with_capacity(vec_pool_capacity)
            }),
            string_pool: MemoryPool::with_factory(string_pool_size, move || {
                String::with_capacity(string_pool_capacity)
            }),
            hashmap_pool: MemoryPool::with_factory(25, || {
                std::collections::HashMap::with_capacity(16)
            }),
        }
    }

    /// Warmup all pools
    pub async fn warmup(&self) {
        tokio::join!(
            self.vec_u8_pool.warmup(50),
            self.string_pool.warmup(25),
            self.hashmap_pool.warmup(10),
        );
    }

    /// Get combined statistics for all pools
    pub async fn combined_stats(&self) -> CombinedPoolStats {
        let (vec_stats, string_stats, hashmap_stats) = tokio::join!(
            self.vec_u8_pool.stats(),
            self.string_pool.stats(),
            self.hashmap_pool.stats(),
        );

        CombinedPoolStats {
            vec_u8_stats: vec_stats,
            string_stats,
            hashmap_stats,
            total_allocations: 0, // Will be calculated
        }
    }
}

impl Default for GameMemoryPools {
    fn default() -> Self {
        Self::new()
    }
}

/// Combined statistics for all game memory pools
#[derive(Debug, Clone)]
pub struct CombinedPoolStats {
    pub vec_u8_stats: PoolStats,
    pub string_stats: PoolStats,
    pub hashmap_stats: PoolStats,
    pub total_allocations: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_memory_pool_basic() {
        let pool = MemoryPool::<Vec<u8>>::new(10);

        // Test getting an object from empty pool
        let obj1 = pool.get().await;
        assert_eq!(obj1.len(), 0);

        // Modify the object
        {
            let mut obj_mut = obj1;
            obj_mut.push(42);
            assert_eq!(obj_mut[0], 42);
        }

        // Object should return to pool when dropped
        sleep(Duration::from_millis(10)).await;

        let stats = pool.stats().await;
        assert_eq!(stats.allocations, 1);
        assert_eq!(stats.cache_misses, 1);
    }

    #[tokio::test]
    async fn test_pool_reuse() {
        let pool = MemoryPool::<Vec<u8>>::new(10);

        // Pre-populate pool
        pool.warmup(5).await;

        let obj = pool.get().await;
        let stats_before = pool.stats().await;
        drop(obj);

        // Allow time for async drop
        sleep(Duration::from_millis(10)).await;

        let obj2 = pool.get().await;
        let stats_after = pool.stats().await;

        assert!(stats_after.cache_hits > stats_before.cache_hits);
        drop(obj2);
    }

    #[tokio::test]
    async fn test_game_memory_pools() {
        let pools = GameMemoryPools::new();
        pools.warmup().await;

        let vec_obj = pools.vec_u8_pool.get().await;
        let string_obj = pools.string_pool.get().await;
        let hashmap_obj = pools.hashmap_pool.get().await;

        // Verify objects are properly initialized
        assert!(vec_obj.capacity() >= 1024);
        assert!(string_obj.capacity() >= 256);
        assert!(hashmap_obj.capacity() >= 16);

        drop(vec_obj);
        drop(string_obj);
        drop(hashmap_obj);

        let combined_stats = pools.combined_stats().await;
        assert!(combined_stats.vec_u8_stats.allocations > 0);
    }

    #[tokio::test]
    async fn test_pooled_object_into_inner() {
        let pool = MemoryPool::<Vec<u8>>::new(10);
        let mut obj = pool.get().await;
        obj.push(123);

        let inner = obj.into_inner();
        assert_eq!(inner[0], 123);

        // Object should not return to pool
        sleep(Duration::from_millis(10)).await;
        let stats = pool.stats().await;
        assert_eq!(stats.deallocations, 0);
    }
}
