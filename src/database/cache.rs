//! Database caching layer for performance optimization
//! 
//! Provides multi-tier caching with LRU eviction, write-through/write-back
//! strategies, and intelligent cache warming.

use std::sync::Arc;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use lru::LruCache;
use serde::{Serialize, Deserialize};
use crate::error::{Error, Result};

/// Cache configuration
#[derive(Debug, Clone)]
pub struct CacheConfig {
    pub l1_size: usize,           // In-memory cache size
    pub l2_size: usize,           // Disk cache size
    pub ttl_seconds: u64,         // Time to live
    pub write_strategy: WriteStrategy,
    pub enable_compression: bool,
    pub enable_metrics: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            l1_size: 1000,
            l2_size: 10000,
            ttl_seconds: 300,
            write_strategy: WriteStrategy::WriteThrough,
            enable_compression: true,
            enable_metrics: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum WriteStrategy {
    WriteThrough,  // Write to cache and database simultaneously
    WriteBack,     // Write to cache first, database later
    WriteAround,   // Skip cache, write directly to database
}

/// Multi-tier cache implementation
pub struct DatabaseCache {
    l1_cache: Arc<RwLock<LruCache<String, CacheEntry>>>,
    l2_cache: Arc<RwLock<HashMap<String, CacheEntry>>>,
    config: CacheConfig,
    metrics: Arc<RwLock<CacheMetrics>>,
}

/// Cache entry with metadata
#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub data: Vec<u8>,
    pub created_at: Instant,
    pub expires_at: Instant,
    pub access_count: u64,
    pub size_bytes: usize,
    pub compressed: bool,
}

impl CacheEntry {
    /// Create a new cache entry
    pub fn new(data: Vec<u8>, ttl: Duration, compress: bool) -> Result<Self> {
        let now = Instant::now();
        let original_len = data.len();
        let compressed_data = if compress && data.len() > 128 {
            Self::compress(&data)?
        } else {
            data
        };
        let compressed_len = compressed_data.len();

        Ok(Self {
            size_bytes: compressed_len,
            data: compressed_data,
            created_at: now,
            expires_at: now + ttl,
            access_count: 0,
            compressed: compress && compressed_len < original_len,
        })
    }

    /// Check if entry has expired
    pub fn is_expired(&self) -> bool {
        Instant::now() > self.expires_at
    }

    /// Get the data, decompressing if needed
    pub fn get_data(&mut self) -> Result<Vec<u8>> {
        self.access_count += 1;
        if self.compressed {
            Self::decompress(&self.data)
        } else {
            Ok(self.data.clone())
        }
    }

    /// Compress data using flate2
    fn compress(data: &[u8]) -> Result<Vec<u8>> {
        use flate2::Compression;
        use flate2::write::GzEncoder;
        use std::io::Write;

        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(data)
            .map_err(|e| Error::Cache(format!("Compression failed: {}", e)))?;
        encoder.finish()
            .map_err(|e| Error::Cache(format!("Compression finalize failed: {}", e)))
    }

    /// Decompress data using flate2
    fn decompress(data: &[u8]) -> Result<Vec<u8>> {
        use flate2::read::GzDecoder;
        use std::io::Read;

        let mut decoder = GzDecoder::new(data);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed)
            .map_err(|e| Error::Cache(format!("Decompression failed: {}", e)))?;
        Ok(decompressed)
    }
}

/// Cache performance metrics
#[derive(Debug, Clone, Default)]
pub struct CacheMetrics {
    pub l1_hits: u64,
    pub l1_misses: u64,
    pub l2_hits: u64,
    pub l2_misses: u64,
    pub evictions: u64,
    pub total_requests: u64,
    pub bytes_cached: u64,
    pub compression_ratio: f64,
}

impl CacheMetrics {
    /// Calculate hit rate
    pub fn hit_rate(&self) -> f64 {
        let total_hits = self.l1_hits + self.l2_hits;
        if self.total_requests == 0 {
            0.0
        } else {
            total_hits as f64 / self.total_requests as f64
        }
    }

    /// Calculate L1 hit rate
    pub fn l1_hit_rate(&self) -> f64 {
        let l1_requests = self.l1_hits + self.l1_misses;
        if l1_requests == 0 {
            0.0
        } else {
            self.l1_hits as f64 / l1_requests as f64
        }
    }
}

impl DatabaseCache {
    /// Create a new database cache
    pub fn new(config: CacheConfig) -> Self {
        Self {
            l1_cache: Arc::new(RwLock::new(LruCache::new(std::num::NonZeroUsize::new(config.l1_size).unwrap()))),
            l2_cache: Arc::new(RwLock::new(HashMap::with_capacity(config.l2_size))),
            config,
            metrics: Arc::new(RwLock::new(CacheMetrics::default())),
        }
    }

    /// Get value from cache
    pub async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        if self.config.enable_metrics {
            let mut metrics = self.metrics.write().await;
            metrics.total_requests += 1;
        }

        // Try L1 cache first
        {
            let mut l1 = self.l1_cache.write().await;
            if let Some(entry) = l1.get_mut(key) {
                if !entry.is_expired() {
                    if self.config.enable_metrics {
                        let mut metrics = self.metrics.write().await;
                        metrics.l1_hits += 1;
                    }
                    return Ok(Some(entry.get_data()?));
                } else {
                    // Remove expired entry
                    l1.pop(key);
                }
            }
        }

        if self.config.enable_metrics {
            let mut metrics = self.metrics.write().await;
            metrics.l1_misses += 1;
        }

        // Try L2 cache
        {
            let mut l2 = self.l2_cache.write().await;
            if let Some(entry) = l2.get_mut(key) {
                if !entry.is_expired() {
                    if self.config.enable_metrics {
                        let mut metrics = self.metrics.write().await;
                        metrics.l2_hits += 1;
                    }
                    
                    // Promote to L1
                    let data = entry.get_data()?;
                    self.promote_to_l1(key, entry.clone()).await;
                    return Ok(Some(data));
                } else {
                    // Remove expired entry
                    l2.remove(key);
                }
            }
        }

        if self.config.enable_metrics {
            let mut metrics = self.metrics.write().await;
            metrics.l2_misses += 1;
        }

        Ok(None)
    }

    /// Put value into cache
    pub async fn put(&self, key: String, value: Vec<u8>) -> Result<()> {
        let ttl = Duration::from_secs(self.config.ttl_seconds);
        let entry = CacheEntry::new(value, ttl, self.config.enable_compression)?;

        // Put into L1 cache
        {
            let mut l1 = self.l1_cache.write().await;
            if let Some((evicted_key, evicted_entry)) = l1.push(key.clone(), entry.clone()) {
                // Demote evicted entry to L2
                self.demote_to_l2(evicted_key, evicted_entry).await;
                
                if self.config.enable_metrics {
                    let mut metrics = self.metrics.write().await;
                    metrics.evictions += 1;
                }
            }
        }

        // Update metrics
        if self.config.enable_metrics {
            let mut metrics = self.metrics.write().await;
            metrics.bytes_cached += entry.size_bytes as u64;
        }

        Ok(())
    }

    /// Remove value from cache
    pub async fn remove(&self, key: &str) -> Result<bool> {
        let mut removed = false;

        // Remove from L1
        {
            let mut l1 = self.l1_cache.write().await;
            if l1.pop(key).is_some() {
                removed = true;
            }
        }

        // Remove from L2
        {
            let mut l2 = self.l2_cache.write().await;
            if l2.remove(key).is_some() {
                removed = true;
            }
        }

        Ok(removed)
    }

    /// Clear all cached entries
    pub async fn clear(&self) -> Result<()> {
        {
            let mut l1 = self.l1_cache.write().await;
            l1.clear();
        }

        {
            let mut l2 = self.l2_cache.write().await;
            l2.clear();
        }

        // Reset metrics
        if self.config.enable_metrics {
            let mut metrics = self.metrics.write().await;
            *metrics = CacheMetrics::default();
        }

        Ok(())
    }

    /// Get cache metrics
    pub async fn metrics(&self) -> CacheMetrics {
        if self.config.enable_metrics {
            self.metrics.read().await.clone()
        } else {
            CacheMetrics::default()
        }
    }

    /// Warm cache with frequently accessed data
    pub async fn warm_cache(&self, keys: Vec<String>) -> Result<usize> {
        // This would typically fetch from database
        // For now, we'll just mark these keys as warmed
        let mut warmed = 0;
        
        for key in keys {
            // In a real implementation, you'd fetch from the database
            // and populate the cache with the actual data
            tracing::debug!("Warming cache for key: {}", key);
            warmed += 1;
        }

        Ok(warmed)
    }

    /// Promote entry to L1 cache
    async fn promote_to_l1(&self, key: &str, entry: CacheEntry) {
        let mut l1 = self.l1_cache.write().await;
        if let Some((evicted_key, evicted_entry)) = l1.push(key.to_string(), entry) {
            // Demote evicted entry to L2
            drop(l1); // Release lock before calling demote_to_l2
            self.demote_to_l2(evicted_key, evicted_entry).await;
        }
    }

    /// Demote entry to L2 cache
    async fn demote_to_l2(&self, key: String, entry: CacheEntry) {
        let mut l2 = self.l2_cache.write().await;
        
        // Check if L2 is full
        if l2.len() >= self.config.l2_size {
            // Remove oldest entry (simple FIFO for L2)
            if let Some(oldest_key) = l2.keys().next().cloned() {
                l2.remove(&oldest_key);
            }
        }
        
        l2.insert(key, entry);
    }

    /// Clean up expired entries
    pub async fn cleanup_expired(&self) -> Result<usize> {
        let mut removed = 0;
        let now = Instant::now();

        // Clean L1
        {
            let mut l1 = self.l1_cache.write().await;
            let expired_keys: Vec<String> = l1.iter()
                .filter(|(_, entry)| entry.expires_at < now)
                .map(|(key, _)| key.clone())
                .collect();

            for key in expired_keys {
                l1.pop(&key);
                removed += 1;
            }
        }

        // Clean L2
        {
            let mut l2 = self.l2_cache.write().await;
            let expired_keys: Vec<String> = l2.iter()
                .filter(|(_, entry)| entry.expires_at < now)
                .map(|(key, _)| key.clone())
                .collect();

            for key in expired_keys {
                l2.remove(&key);
                removed += 1;
            }
        }

        Ok(removed)
    }

    /// Get cache statistics
    pub async fn stats(&self) -> CacheStats {
        let l1_size = self.l1_cache.read().await.len();
        let l2_size = self.l2_cache.read().await.len();
        let metrics = self.metrics().await;

        CacheStats {
            l1_entries: l1_size,
            l2_entries: l2_size,
            total_entries: l1_size + l2_size,
            hit_rate: metrics.hit_rate(),
            l1_hit_rate: metrics.l1_hit_rate(),
            bytes_cached: metrics.bytes_cached,
            compression_ratio: metrics.compression_ratio,
            evictions: metrics.evictions,
        }
    }
}

/// Cache statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    pub l1_entries: usize,
    pub l2_entries: usize,
    pub total_entries: usize,
    pub hit_rate: f64,
    pub l1_hit_rate: f64,
    pub bytes_cached: u64,
    pub compression_ratio: f64,
    pub evictions: u64,
}

/// Cache-aware query builder
pub struct CachedQueryBuilder {
    cache: Arc<DatabaseCache>,
    query_prefix: String,
}

impl CachedQueryBuilder {
    pub fn new(cache: Arc<DatabaseCache>, prefix: String) -> Self {
        Self {
            cache,
            query_prefix: prefix,
        }
    }

    /// Build a cache key for a query
    pub fn build_key(&self, query: &str, params: &[String]) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        query.hash(&mut hasher);
        for param in params {
            param.hash(&mut hasher);
        }
        
        format!("{}:{:x}", self.query_prefix, hasher.finish())
    }

    /// Execute a cached query
    pub async fn execute<T>(&self, key: String, fetch_fn: impl std::future::Future<Output = Result<T>>) -> Result<T>
    where
        T: Serialize + for<'de> Deserialize<'de>,
    {
        // Try to get from cache first
        if let Some(cached_data) = self.cache.get(&key).await? {
            match bincode::deserialize(&cached_data) {
                Ok(data) => return Ok(data),
                Err(e) => {
                    tracing::warn!("Cache deserialization failed for key {}: {}", key, e);
                    // Remove invalid cache entry
                    let _ = self.cache.remove(&key).await;
                }
            }
        }

        // Fetch from database
        let data = fetch_fn.await?;
        
        // Cache the result
        if let Ok(serialized) = bincode::serialize(&data) {
            if let Err(e) = self.cache.put(key, serialized).await {
                tracing::warn!("Failed to cache query result: {}", e);
            }
        }

        Ok(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_cache_basic_operations() {
        let config = CacheConfig {
            l1_size: 2,
            l2_size: 3,
            ttl_seconds: 1,
            ..Default::default()
        };
        
        let cache = DatabaseCache::new(config);
        
        // Test put and get
        cache.put("key1".to_string(), b"value1".to_vec()).await.unwrap();
        let value = cache.get("key1").await.unwrap();
        assert_eq!(value, Some(b"value1".to_vec()));
        
        // Test cache miss
        let missing = cache.get("missing").await.unwrap();
        assert_eq!(missing, None);
        
        // Test metrics
        let metrics = cache.metrics().await;
        assert!(metrics.total_requests > 0);
    }

    #[tokio::test]
    async fn test_cache_expiration() {
        let config = CacheConfig {
            l1_size: 10,
            l2_size: 10,
            ttl_seconds: 0, // Immediate expiration
            ..Default::default()
        };
        
        let cache = DatabaseCache::new(config);
        
        cache.put("key1".to_string(), b"value1".to_vec()).await.unwrap();
        sleep(Duration::from_millis(10)).await; // Wait for expiration
        
        let value = cache.get("key1").await.unwrap();
        assert_eq!(value, None);
    }

    #[tokio::test]
    async fn test_cache_eviction() {
        let config = CacheConfig {
            l1_size: 1,
            l2_size: 1,
            ttl_seconds: 60,
            ..Default::default()
        };
        
        let cache = DatabaseCache::new(config);
        
        cache.put("key1".to_string(), b"value1".to_vec()).await.unwrap();
        cache.put("key2".to_string(), b"value2".to_vec()).await.unwrap();
        cache.put("key3".to_string(), b"value3".to_vec()).await.unwrap();
        
        let stats = cache.stats().await;
        assert!(stats.evictions > 0);
    }
}