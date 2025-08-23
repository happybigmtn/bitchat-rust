//! Multi-tier caching system for high-performance data access

use std::sync::Arc;
use std::time::Instant;
use lru::LruCache;
use parking_lot::RwLock;
use memmap2::MmapOptions;
use serde::{Serialize, Deserialize};
use std::fs::File;
use std::path::PathBuf;
use std::collections::HashMap;
use dashmap::DashMap;
use crate::error::Result;

/// Cache entry with metadata
#[derive(Debug, Clone)]
pub struct CacheEntry<T> {
    pub value: T,
    pub inserted_at: Instant,
    pub last_accessed: Instant,
    pub access_count: u64,
    pub size_bytes: usize,
}

impl<T> CacheEntry<T> {
    fn new(value: T, size_bytes: usize) -> Self {
        let now = Instant::now();
        Self {
            value,
            inserted_at: now,
            last_accessed: now,
            access_count: 1,
            size_bytes,
        }
    }
    
    fn touch(&mut self) {
        self.last_accessed = Instant::now();
        self.access_count += 1;
    }
}

/// Cache statistics for monitoring
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    pub l1_hits: u64,
    pub l1_misses: u64,
    pub l2_hits: u64,
    pub l2_misses: u64,
    pub l3_hits: u64,
    pub l3_misses: u64,
    pub total_evictions: u64,
    pub promotions: u64,
    pub demotions: u64,
}

impl CacheStats {
    pub fn l1_hit_rate(&self) -> f64 {
        if self.l1_hits + self.l1_misses == 0 {
            0.0
        } else {
            self.l1_hits as f64 / (self.l1_hits + self.l1_misses) as f64
        }
    }
    
    pub fn l2_hit_rate(&self) -> f64 {
        if self.l2_hits + self.l2_misses == 0 {
            0.0
        } else {
            self.l2_hits as f64 / (self.l2_hits + self.l2_misses) as f64
        }
    }
    
    pub fn overall_hit_rate(&self) -> f64 {
        let total_hits = self.l1_hits + self.l2_hits + self.l3_hits;
        let total_misses = self.l1_misses + self.l2_misses + self.l3_misses;
        if total_hits + total_misses == 0 {
            0.0
        } else {
            total_hits as f64 / (total_hits + total_misses) as f64
        }
    }
}

/// L1 cache - In-memory, lock-free, fastest
pub struct L1Cache<K, V> 
where
    K: Eq + std::hash::Hash + Clone,
    V: Clone,
{
    cache: Arc<DashMap<K, CacheEntry<V>>>,
    max_entries: usize,
    max_size_bytes: usize,
    current_size_bytes: Arc<RwLock<usize>>,
}

impl<K, V> L1Cache<K, V>
where
    K: Eq + std::hash::Hash + Clone,
    V: Clone,
{
    pub fn new(max_entries: usize, max_size_mb: usize) -> Self {
        Self {
            cache: Arc::new(DashMap::with_capacity(max_entries)),
            max_entries,
            max_size_bytes: max_size_mb * 1024 * 1024,
            current_size_bytes: Arc::new(RwLock::new(0)),
        }
    }
    
    pub fn get(&self, key: &K) -> Option<V> {
        self.cache.get_mut(key).map(|mut entry| {
            entry.touch();
            entry.value.clone()
        })
    }
    
    pub fn insert(&self, key: K, value: V, size_bytes: usize) -> Option<V> {
        // Check size constraints
        let mut current_size = self.current_size_bytes.write();
        
        if self.cache.len() >= self.max_entries || *current_size + size_bytes > self.max_size_bytes {
            // Evict least recently used
            self.evict_lru();
        }
        
        let entry = CacheEntry::new(value.clone(), size_bytes);
        let old = self.cache.insert(key, entry);
        
        if let Some(ref old_entry) = old {
            *current_size -= old_entry.size_bytes;
        }
        *current_size += size_bytes;
        
        old.map(|e| e.value)
    }
    
    fn evict_lru(&self) {
        // Find least recently used entry
        let mut oldest_key = None;
        let mut oldest_time = Instant::now();
        
        for entry in self.cache.iter() {
            if entry.value().last_accessed < oldest_time {
                oldest_time = entry.value().last_accessed;
                oldest_key = Some(entry.key().clone());
            }
        }
        
        if let Some(key) = oldest_key {
            if let Some((_, entry)) = self.cache.remove(&key) {
                let mut current_size = self.current_size_bytes.write();
                *current_size -= entry.size_bytes;
            }
        }
    }
    
    pub fn clear(&self) {
        self.cache.clear();
        *self.current_size_bytes.write() = 0;
    }
    
    pub fn len(&self) -> usize {
        self.cache.len()
    }
    
    pub fn size_bytes(&self) -> usize {
        *self.current_size_bytes.read()
    }
}

/// L2 cache - In-memory LRU, slightly slower but larger
pub struct L2Cache<K, V>
where
    K: Eq + std::hash::Hash + Clone,
    V: Clone,
{
    cache: Arc<RwLock<LruCache<K, CacheEntry<V>>>>,
    max_size_bytes: usize,
    current_size_bytes: Arc<RwLock<usize>>,
}

impl<K, V> L2Cache<K, V>
where
    K: Eq + std::hash::Hash + Clone,
    V: Clone,
{
    pub fn new(max_entries: usize, max_size_mb: usize) -> Self {
        let cache = LruCache::new(std::num::NonZeroUsize::new(max_entries).unwrap());
        Self {
            cache: Arc::new(RwLock::new(cache)),
            max_size_bytes: max_size_mb * 1024 * 1024,
            current_size_bytes: Arc::new(RwLock::new(0)),
        }
    }
    
    pub fn get(&self, key: &K) -> Option<V> {
        let mut cache = self.cache.write();
        cache.get_mut(key).map(|entry| {
            entry.touch();
            entry.value.clone()
        })
    }
    
    pub fn insert(&self, key: K, value: V, size_bytes: usize) -> Option<V> {
        let mut cache = self.cache.write();
        let mut current_size = self.current_size_bytes.write();
        
        // Check size constraint
        while *current_size + size_bytes > self.max_size_bytes && cache.len() > 0 {
            // LRU eviction happens automatically
            if let Some((_, evicted)) = cache.pop_lru() {
                *current_size -= evicted.size_bytes;
            }
        }
        
        let entry = CacheEntry::new(value.clone(), size_bytes);
        let old = cache.put(key, entry);
        
        if let Some(ref old_entry) = old {
            *current_size -= old_entry.size_bytes;
        }
        *current_size += size_bytes;
        
        old.map(|e| e.value)
    }
    
    pub fn clear(&self) {
        self.cache.write().clear();
        *self.current_size_bytes.write() = 0;
    }
}

/// L3 cache - Memory-mapped file, persistent but slower
pub struct L3Cache {
    cache_dir: PathBuf,
    index: Arc<RwLock<HashMap<String, L3Entry>>>,
    max_size_bytes: usize,
    current_size_bytes: Arc<RwLock<usize>>,
}

#[derive(Debug, Clone)]
struct L3Entry {
    file_path: PathBuf,
    offset: usize,
    size: usize,
    last_accessed: Instant,
}

impl L3Cache {
    pub fn new(cache_dir: PathBuf, max_size_mb: usize) -> Result<Self> {
        std::fs::create_dir_all(&cache_dir)
            .map_err(|e| crate::error::Error::Io(e))?;
        
        Ok(Self {
            cache_dir,
            index: Arc::new(RwLock::new(HashMap::new())),
            max_size_bytes: max_size_mb * 1024 * 1024,
            current_size_bytes: Arc::new(RwLock::new(0)),
        })
    }
    
    pub fn get(&self, key: &str) -> Result<Vec<u8>> {
        let mut index = self.index.write();
        
        if let Some(entry) = index.get_mut(key) {
            entry.last_accessed = Instant::now();
            
            // Memory map the file
            let file = File::open(&entry.file_path)
                .map_err(|e| crate::error::Error::Io(e))?;
            
            let mmap = unsafe {
                MmapOptions::new()
                    .offset(entry.offset as u64)
                    .len(entry.size)
                    .map(&file)
                    .map_err(|e| crate::error::Error::Io(e))?
            };
            
            Ok(mmap[..].to_vec())
        } else {
            Err(crate::error::Error::InvalidData("Key not found in L3 cache".to_string()))
        }
    }
    
    pub fn insert(&self, key: String, value: &[u8]) -> Result<()> {
        let mut index = self.index.write();
        let mut current_size = self.current_size_bytes.write();
        
        // Check size constraint
        if *current_size + value.len() > self.max_size_bytes {
            self.evict_lru(&mut index, &mut current_size)?;
        }
        
        // Write to file
        let file_path = self.cache_dir.join(format!("{}.cache", key));
        std::fs::write(&file_path, value)
            .map_err(|e| crate::error::Error::Io(e))?;
        
        let entry = L3Entry {
            file_path,
            offset: 0,
            size: value.len(),
            last_accessed: Instant::now(),
        };
        
        if let Some(old_entry) = index.insert(key, entry) {
            *current_size -= old_entry.size;
            // Clean up old file
            let _ = std::fs::remove_file(old_entry.file_path);
        }
        
        *current_size += value.len();
        
        Ok(())
    }
    
    fn evict_lru(&self, index: &mut HashMap<String, L3Entry>, current_size: &mut usize) -> Result<()> {
        // Find least recently used
        if let Some((key, _)) = index.iter()
            .min_by_key(|(_, entry)| entry.last_accessed)
            .map(|(k, v)| (k.clone(), v.clone()))
        {
            if let Some(entry) = index.remove(&key) {
                *current_size -= entry.size;
                let _ = std::fs::remove_file(entry.file_path);
            }
        }
        Ok(())
    }
}

/// Multi-tier cache orchestrator
pub struct MultiTierCache<K, V>
where
    K: Eq + std::hash::Hash + Clone + ToString,
    V: Clone + Serialize + for<'de> Deserialize<'de>,
{
    l1: L1Cache<K, V>,
    l2: L2Cache<K, V>,
    l3: L3Cache,
    stats: Arc<RwLock<CacheStats>>,
    promotion_threshold: u64,  // Access count for promotion
}

impl<K, V> MultiTierCache<K, V>
where
    K: Eq + std::hash::Hash + Clone + ToString,
    V: Clone + Serialize + for<'de> Deserialize<'de>,
{
    pub fn new(cache_dir: PathBuf) -> Result<Self> {
        Ok(Self {
            l1: L1Cache::new(1000, 64),      // 64MB L1
            l2: L2Cache::new(10000, 512),    // 512MB L2
            l3: L3Cache::new(cache_dir, 4096)?,  // 4GB L3
            stats: Arc::new(RwLock::new(CacheStats::default())),
            promotion_threshold: 3,
        })
    }
    
    pub fn get(&self, key: &K) -> Option<V> {
        let mut stats = self.stats.write();
        
        // Check L1
        if let Some(value) = self.l1.get(key) {
            stats.l1_hits += 1;
            return Some(value);
        }
        stats.l1_misses += 1;
        
        // Check L2
        if let Some(value) = self.l2.get(key) {
            stats.l2_hits += 1;
            
            // Promote to L1 if accessed frequently
            let size = bincode::serialize(&value).unwrap_or_default().len();
            self.l1.insert(key.clone(), value.clone(), size);
            stats.promotions += 1;
            
            return Some(value);
        }
        stats.l2_misses += 1;
        
        // Check L3
        if let Ok(bytes) = self.l3.get(&key.to_string()) {
            if let Ok(value) = bincode::deserialize::<V>(&bytes) {
                stats.l3_hits += 1;
                
                // Promote to L2
                self.l2.insert(key.clone(), value.clone(), bytes.len());
                stats.promotions += 1;
                
                return Some(value);
            }
        }
        stats.l3_misses += 1;
        
        None
    }
    
    pub fn insert(&self, key: K, value: V) -> Result<()> {
        let bytes = bincode::serialize(&value)
            .map_err(|e| crate::error::Error::Serialization(e.to_string()))?;
        let size = bytes.len();
        
        // Insert into L1 first (hot data)
        self.l1.insert(key.clone(), value.clone(), size);
        
        // Also insert into L3 for persistence
        self.l3.insert(key.to_string(), &bytes)?;
        
        Ok(())
    }
    
    pub fn get_stats(&self) -> CacheStats {
        self.stats.read().clone()
    }
    
    pub fn clear_all(&self) {
        self.l1.clear();
        self.l2.clear();
        // L3 persists
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestValue {
        data: String,
    }
    
    #[test]
    fn test_l1_cache() {
        let cache: L1Cache<String, TestValue> = L1Cache::new(10, 1);
        
        let key = "test".to_string();
        let value = TestValue { data: "data".to_string() };
        
        cache.insert(key.clone(), value.clone(), 100);
        assert_eq!(cache.get(&key), Some(value));
        assert_eq!(cache.len(), 1);
    }
    
    #[test]
    fn test_multi_tier_cache() {
        let temp_dir = TempDir::new().unwrap();
        let cache: MultiTierCache<String, TestValue> = 
            MultiTierCache::new(temp_dir.path().to_path_buf()).unwrap();
        
        let key = "test".to_string();
        let value = TestValue { data: "data".to_string() };
        
        // Insert and retrieve
        cache.insert(key.clone(), value.clone()).unwrap();
        assert_eq!(cache.get(&key), Some(value.clone()));
        
        // Check stats
        let stats = cache.get_stats();
        assert_eq!(stats.l1_hits, 1);
        
        // Clear L1 and L2
        cache.clear_all();
        
        // Should still find in L3
        assert_eq!(cache.get(&key), Some(value));
        
        let stats = cache.get_stats();
        assert_eq!(stats.l3_hits, 1);
        assert_eq!(stats.promotions, 1);  // Promoted from L3 to L2
    }
}