# Chapter 18: Caching and Performance Optimization

Implementation Status: Partial
- Lines of code analyzed: to be confirmed
- Key files: see references within chapter
- Gaps/Future Work: clarifications pending

## Walking Through `src/cache/multi_tier.rs`

*Part of the comprehensive BitCraps curriculum - the final deep dive into high-performance systems*

---

## Part I: Caching and Performance Optimization for Complete Beginners

Imagine you're a librarian in a massive library with millions of books. A student asks for a popular book. You could walk to the exact shelf, find the book, and bring it back - but that takes 15 minutes. Or you could keep the most popular books at the front desk for instant access. That's caching in a nutshell.

In computing, this "walk to the shelf" represents slow operations: disk reads, network requests, database queries, complex calculations. The "front desk" represents fast storage: CPU registers, L1 cache, RAM. The art of performance optimization is predicting what data will be needed and keeping it in fast storage.

But here's the challenge: fast storage is expensive and limited. Your CPU has maybe 32KB of L1 cache, while your disk might have 2TB. You must choose wisely what to keep where.

### The Fundamental Speed Hierarchy

Modern computer systems have a clear speed hierarchy, with each level being orders of magnitude faster or slower than the next:

**CPU Registers (1 cycle = ~0.3 nanoseconds)**
- Fastest possible access
- Only ~32-64 bytes total
- Directly accessed by CPU instructions

**L1 Cache (~1 nanosecond)**
- On-CPU cache, separate for instructions and data
- ~32KB typical size
- 99%+ hit rate required for good performance

**L2 Cache (~3 nanoseconds)**
- Larger on-CPU or near-CPU cache
- ~256KB-1MB typical size
- Shared between CPU cores

**L3 Cache (~12 nanoseconds)**
- Shared between all cores
- ~8-32MB typical size
- Last chance before main memory

**RAM (~100 nanoseconds)**
- Main system memory
- 8-64GB typical size
- 100x slower than L1 cache

**SSD (~100,000 nanoseconds = 0.1 milliseconds)**
- Solid state storage
- 256GB-2TB typical size
- 1,000x slower than RAM

**HDD (~10,000,000 nanoseconds = 10 milliseconds)**
- Mechanical hard drive
- 1-10TB typical size
- 100x slower than SSD

**Network (~50,000,000-200,000,000 nanoseconds = 50-200 milliseconds)**
- Internet or WAN access
- Essentially infinite size
- 500-2000x slower than local disk

### The Human Perspective

To put these numbers in human terms, if accessing L1 cache took 1 second:

- L2 cache would take 3 seconds
- L3 cache would take 12 seconds  
- RAM would take 1.7 minutes
- SSD would take 2.8 hours
- HDD would take 11.6 days
- Network would take 2-6 years

This is why cache misses are so catastrophic for performance!

### Famous Performance Disasters and Their Lessons

**The First Caching Insight: IBM System/360 (1964)**

IBM discovered that programs exhibit "locality of reference" - they tend to access the same data repeatedly and access nearby data in sequence. This insight led to the first computer caches and revolutionized computer architecture.

Lesson: *Most data access patterns are predictable.*

**The Pentium FDIV Bug (1994) - When Caches Store Wrong Data**

Intel's Pentium processor had a bug in its floating-point division table (essentially a cache of division results). For specific input combinations, it returned incorrect results. This cost Intel $475 million in replacement processors.

Lesson: *Cache correctness is more important than cache speed.*

**The Cloudflare Outage (2017) - Cache Stampedes**

When Cloudflare's cache servers went down, all requests simultaneously hit the origin servers. The sudden load caused a cascade failure that took down much of the internet for several hours. This is called a "cache stampede."

Lesson: *Cache failures can amplify load rather than reduce it.*

**The Meltdown and Spectre Vulnerabilities (2018) - When Caches Leak Secrets**

These CPU vulnerabilities exploited speculative execution and caches to read protected memory. Attackers could measure cache timing to infer sensitive data like passwords and encryption keys.

Lesson: *Performance optimizations can create security vulnerabilities.*

**Netflix's Billion-Dollar Caching Strategy**

Netflix spends over $1 billion annually on content delivery networks (CDNs) - essentially massive global caches. They pre-position popular content in servers near users, reducing streaming latency and bandwidth costs.

Lesson: *Caching can be the difference between business success and failure.*

### The Psychology of Performance

**Human Performance Perception:**
- **Instant** (0-100ms): Feels instantaneous
- **Fast** (100ms-1s): Feels responsive  
- **Acceptable** (1-10s): Usable but noticeable delay
- **Slow** (10s+): Frustrating, users abandon tasks

**The 10x Rule:**
Each layer of the storage hierarchy is roughly 10x slower than the previous. This means:
- Cache miss = 10x performance penalty
- Two cache misses = 100x penalty
- Three cache misses = 1000x penalty

**Performance vs. Correctness Trade-off:**
Caches introduce complexity and potential inconsistency. Sometimes "slower but correct" is better than "faster but wrong."

### Core Caching Concepts

**1. Cache Hit vs. Cache Miss**

**Cache Hit**: Data is found in the cache (fast path)
**Cache Miss**: Data is not in cache, must be fetched from slower storage (slow path)

Hit rate = Cache Hits / (Cache Hits + Cache Misses)

A 90% hit rate means 10% of requests take the slow path. For a 100x speed difference, average performance is:
0.9 × 1 + 0.1 × 100 = 10.9 (nearly 11x slower than perfect caching)

**2. Cache Replacement Policies**

When the cache is full, which item should be evicted?

**LRU (Least Recently Used)**: Remove the item accessed longest ago
- Good for temporal locality
- Complex to implement efficiently

**FIFO (First In, First Out)**: Remove the oldest item  
- Simple to implement
- Ignores access patterns

**LFU (Least Frequently Used)**: Remove the item accessed least often
- Good for skewed access patterns
- Can suffer from "aging" problems

**Random**: Remove a random item
- Simplest implementation
- Surprisingly effective in practice

**2. Temporal vs. Spatial Locality**

**Temporal Locality**: Data accessed recently is likely to be accessed again soon
Example: Loop variables, frequently called functions

**Spatial Locality**: Data near recently accessed data is likely to be accessed soon  
Example: Array elements, sequential file reads

Effective caches exploit both types of locality.

### Cache Coherence: The Consistency Problem

In systems with multiple caches (multiple CPU cores, distributed systems), keeping caches consistent is challenging:

**Problem**: CPU 1 caches variable X=5, CPU 2 modifies X=10. CPU 1's cache now has stale data.

**Solutions:**

**Write-Through**: All writes go to both cache and backing store
- Slow writes, fast reads
- Always consistent

**Write-Back**: Writes only go to cache initially, written to backing store later  
- Fast writes, but complex consistency

**Cache Invalidation**: When data changes, invalidate all cached copies
- Simple but causes cache misses

**MESI Protocol**: Modified, Exclusive, Shared, Invalid states for cache lines
- Complex but efficient

### Multi-Level Caching Strategy

Modern systems use multiple cache levels with different characteristics:

**L1**: Very fast, very small, very expensive
**L2**: Somewhat fast, somewhat large, somewhat expensive  
**L3**: Moderately fast, moderately large, moderately expensive

**Inclusive Caches**: Higher levels contain all data from lower levels
- Simple consistency model
- Wastes space with duplication

**Exclusive Caches**: Each level contains different data
- More efficient use of total cache space
- More complex consistency

### Cache-Aware Algorithms

Traditional algorithms don't consider cache behavior. Cache-aware algorithms are designed for modern memory hierarchies:

**Matrix Multiplication:**
- Naive: A[i][j] × B[i][j] (poor cache locality)
- Blocked: Process small sub-matrices that fit in cache (good locality)
- Result: 10-100x performance improvement

**Sorting:**
- Quicksort: Good average case, poor cache behavior
- Merge sort: Predictable cache access patterns
- Cache-oblivious algorithms: Good performance regardless of cache sizes

**Tree Traversals:**
- Depth-first: Good spatial locality (siblings accessed together)
- Breadth-first: Poor spatial locality (nodes scattered across memory)
- B-trees: Designed to minimize disk accesses (cache-friendly)

### Caching in Distributed Systems

Distributed systems introduce additional caching challenges:

**1. Cache Stampedes**
When cached data expires, all nodes simultaneously request fresh data, overwhelming the backend.

**Solution**: Staggered expiration times, probabilistic early expiration

**2. Cache Warm-up**
New nodes start with empty caches, causing poor performance until caches fill.

**Solution**: Preload critical data, gradual traffic ramp-up

**3. Distributed Cache Invalidation**
When data changes, all nodes' caches must be updated.

**Solutions**: 
- Event-driven invalidation (publish/subscribe)
- Time-based expiration (eventual consistency)
- Version-based validation (compare timestamps)

### Advanced Caching Patterns

**1. Read-Through Cache**
```
if (data = cache.get(key)) != null:
    return data
else:
    data = database.get(key)
    cache.put(key, data)
    return data
```

**2. Write-Through Cache**
```
cache.put(key, data)
database.put(key, data)
return success
```

**3. Write-Behind Cache**
```
cache.put(key, data)
queue.push(WriteRequest(key, data))  // Async
return success
```

**4. Cache-Aside Pattern**
```
// Application manages cache explicitly
data = cache.get(key)
if data == null:
    data = database.get(key)
    cache.put(key, data)
return data
```

### Performance Measurement and Optimization

**Key Metrics:**

**Hit Rate**: Percentage of requests served from cache
**Miss Rate**: Percentage of requests that require slow path  
**Throughput**: Requests handled per second
**Latency**: Time to complete individual requests
**P99/P95 Latency**: 99th/95th percentile response times (tail latency)

**Common Optimization Techniques:**

**Prefetching**: Load data before it's requested
**Batch Loading**: Fetch multiple items in one request
**Compression**: Store more data in same cache space
**TTL Optimization**: Balance freshness vs. hit rate
**Cache Partitioning**: Separate hot and cold data

### The Economics of Caching

**Cost-Benefit Analysis:**

**Benefits:**
- Reduced server load
- Lower bandwidth usage  
- Improved user experience
- Higher system capacity

**Costs:**
- Memory/storage for cache
- CPU overhead for cache management
- Complexity in development and operations
- Potential consistency issues

**Rule of Thumb**: If hit rate > 80% and speed improvement > 5x, caching usually pays off.

### Modern Caching Technologies

**Redis**: In-memory key-value store with persistence
**Memcached**: Simple, fast in-memory cache
**CDNs**: Geographic distribution of cached content
**Browser Caches**: Client-side caching for web applications
**Application-Level Caching**: Custom caching within applications

### Cache Security Considerations

**1. Cache Poisoning**
Attackers store malicious data in cache, served to legitimate users.

**Prevention**: Validate all cached data, use secure cache keys

**2. Timing Attacks**
Attackers infer sensitive data by measuring cache access times.

**Prevention**: Constant-time operations, cache obfuscation

**3. Cache Side-Channels**
Information leakage through cache state changes.

**Prevention**: Cache partitioning, randomized eviction

### The Future of Caching

**Machine Learning-Driven Caching**: AI predicts what to cache based on access patterns
**Persistent Memory**: Blur the line between memory and storage
**Quantum Caching**: Theoretical but potentially revolutionary approach
**Edge Computing**: Move caches closer to users geographically

### Common Caching Anti-Patterns

**1. Cache Everything**
Caching data that's rarely accessed wastes memory and complicates invalidation.

**2. Infinite TTL**  
Data becomes stale, leading to inconsistent behavior.

**3. Synchronous Cache Updates**
Blocking operations waiting for cache updates hurts performance.

**4. Ignoring Cache Warm-up**
New deployments suffer from cold cache performance.

**5. Complex Cache Keys**
Expensive key generation can negate caching benefits.

---

Now that you understand the theoretical foundations and practical challenges of high-performance caching, let's examine how BitCraps implements a sophisticated multi-tier caching system designed for real-time gaming applications.

---

## Part II: BitCraps Multi-Tier Caching Implementation Deep Dive

The BitCraps caching system implements a sophisticated three-tier cache hierarchy optimized for the unique requirements of real-time distributed gaming: microsecond access times for hot data, persistent storage for session continuity, and automatic promotion/demotion based on access patterns.

### Module Architecture: `src/cache/multi_tier.rs`

The caching system establishes a comprehensive performance optimization framework through carefully orchestrated cache tiers:

**Lines 1-14: System Overview**
```rust
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
```

The import choices reveal the sophisticated approach:
- **DashMap**: Lock-free concurrent HashMap for L1 cache
- **LruCache**: Efficient LRU implementation for L2 cache  
- **memmap2**: Memory-mapped files for L3 cache
- **parking_lot**: High-performance RwLock alternative

### Cache Entry Metadata: Rich Performance Tracking

**Lines 15-41: Comprehensive Entry Tracking**
```rust
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
```

This entry design enables sophisticated cache management policies:

**Temporal Tracking**: `inserted_at` and `last_accessed` enable LRU eviction
**Frequency Tracking**: `access_count` enables LFU eviction and promotion decisions
**Size Tracking**: `size_bytes` enables memory-based eviction policies
**Atomic Updates**: `touch()` method ensures consistent metadata updates

The rich metadata enables advanced cache replacement policies beyond simple LRU.

### Performance Monitoring: Comprehensive Cache Analytics

**Lines 43-83: Multi-Tier Statistics Tracking**
```rust
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
```

The statistics enable comprehensive performance analysis:

**Per-Tier Metrics**: Track hit/miss rates for each cache level
**Promotion Tracking**: Monitor data movement between cache tiers
**Overall Performance**: Calculate aggregate system performance metrics

This data is crucial for tuning cache sizes, eviction policies, and promotion thresholds in production.

### L1 Cache: Lock-Free High-Performance Tier

**Lines 85-170: Concurrent High-Speed Cache**
```rust
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
}
```

**Key Design Decisions:**

**DashMap for Concurrency**: The L1 cache uses DashMap, a lock-free concurrent HashMap that provides excellent performance under high contention. This is crucial for gaming applications where multiple threads might access the cache simultaneously.

**Dual Size Constraints**: Both entry count and memory usage are limited, preventing resource exhaustion from either too many small entries or too many large entries.

**Immediate Metadata Updates**: The `touch()` method updates access time and count immediately, ensuring LRU calculations are always current.

**Size-Aware Eviction**: The cache considers both memory usage and entry counts when deciding whether to evict, optimizing for the most constrained resource.

### L2 Cache: Balanced Performance and Capacity

**Lines 172-232: Structured LRU Cache**
```rust
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
    pub fn insert(&self, key: K, value: V, size_bytes: usize) -> Option<V> {
        let mut cache = self.cache.write();
        let mut current_size = self.current_size_bytes.write();
        
        // Check size constraint
        while *current_size + size_bytes > self.max_size_bytes && !cache.is_empty() {
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
}
```

**L2 Cache Characteristics:**

**True LRU Ordering**: Uses the `lru` crate for proper LRU semantics, ensuring the least recently used items are evicted first.

**Automatic Eviction Loop**: The while loop ensures sufficient space is available, even if multiple items must be evicted.

**Write Lock Coordination**: Uses RwLock to coordinate between readers and the insertion process, balancing concurrency with consistency.

The L2 cache serves as a middle ground between L1's extreme performance and L3's extreme capacity.

### L3 Cache: Persistent Memory-Mapped Storage

**Lines 234-332: File-Based Persistent Cache**
```rust
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
    pub fn get(&self, key: &str) -> Result<Vec<u8>> {
        let mut index = self.index.write();
        
        if let Some(entry) = index.get_mut(key) {
            entry.last_accessed = Instant::now();
            
            // Memory map the file
            let file = File::open(&entry.file_path)
                .map_err(crate::error::Error::Io)?;
            
            let mmap = unsafe {
                MmapOptions::new()
                    .offset(entry.offset as u64)
                    .len(entry.size)
                    .map(&file)
                    .map_err(crate::error::Error::Io)?
            };
            
            Ok(mmap[..].to_vec())
        } else {
            Err(crate::error::Error::InvalidData("Key not found in L3 cache".to_string()))
        }
    }
}
```

**L3 Cache Innovations:**

**Memory-Mapped File I/O**: Uses mmap for efficient file access, allowing the OS to optimize memory usage and page management.

**Per-Entry Files**: Each cache entry gets its own file, simplifying management and allowing parallel access to different entries.

**Offset Support**: The L3Entry structure includes offset information, enabling future optimizations like file compaction or shared files.

**OS-Managed Persistence**: Files persist across application restarts, providing session continuity for gaming applications.

The unsafe block is necessary for memory mapping but is well-contained and follows standard mmap patterns.

### Multi-Tier Orchestration: Intelligent Cache Management

**Lines 334-425: Coordinated Multi-Level Cache**
```rust
pub struct MultiTierCache<K, V>
where
    K: Eq + std::hash::Hash + Clone + ToString,
    V: Clone + Serialize + for<'de> Deserialize<'de>,
{
    l1: L1Cache<K, V>,
    l2: L2Cache<K, V>,
    l3: L3Cache,
    stats: Arc<RwLock<CacheStats>>,
    _promotion_threshold: u64,  // Access count for promotion
}

impl<K, V> MultiTierCache<K, V> {
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
}
```

**Multi-Tier Intelligence:**

**Hierarchical Lookup**: Searches from fastest to slowest cache, ensuring minimum possible latency.

**Automatic Promotion**: Data found in lower tiers is automatically promoted to higher tiers for future fast access.

**Statistical Tracking**: Every cache access updates detailed statistics for performance monitoring.

**Serialization Bridge**: Handles the serialization required for L3 storage while maintaining type safety at higher levels.

**Smart Insertion Strategy**:
```rust
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
```

New data goes directly into both L1 (for immediate performance) and L3 (for persistence), skipping L2 initially. This reflects the assumption that newly inserted data is likely to be accessed soon (temporal locality).

### Advanced Cache Replacement: LRU Implementation

**Lines 138-156: Sophisticated Eviction Logic**
```rust
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
```

This LRU implementation demonstrates several important concepts:

**Accurate Time Tracking**: Uses `Instant` for precise timing measurements
**Atomic Eviction**: The remove operation is atomic, preventing partial state updates
**Memory Accounting**: Immediately updates size tracking when items are removed
**Graceful Handling**: Returns silently if no items exist to evict

### Performance Testing and Validation

**Lines 427-476: Comprehensive Test Suite**
```rust
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
```

The test validates several critical behaviors:

**Basic Functionality**: Insert and retrieve operations work correctly
**Statistics Accuracy**: Hit counts are properly tracked
**Persistence**: L3 cache survives L1/L2 clearing
**Promotion Logic**: Data is automatically promoted from L3 to L2

### Memory Management and Resource Control

The cache system implements sophisticated resource management:

**Size-Based Eviction**: All cache tiers respect both entry count and memory size limits
**Immediate Cleanup**: Resources are freed immediately when entries are evicted
**Controlled Growth**: Maximum sizes prevent unbounded memory usage
**Efficient Storage**: Binary serialization minimizes storage overhead

### Configuration and Tuning

**Lines 352-360: Performance-Tuned Defaults**
```rust
pub fn new(cache_dir: PathBuf) -> Result<Self> {
    Ok(Self {
        l1: L1Cache::new(1000, 64),      // 64MB L1
        l2: L2Cache::new(10000, 512),    // 512MB L2
        l3: L3Cache::new(cache_dir, 4096)?,  // 4GB L3
        stats: Arc::new(RwLock::new(CacheStats::default())),
        _promotion_threshold: 3,
    })
}
```

The default configuration reflects gaming application requirements:

**L1 Cache**: 1,000 entries, 64MB - optimized for frequently accessed small objects
**L2 Cache**: 10,000 entries, 512MB - intermediate storage for moderately hot data
**L3 Cache**: 4GB - large persistent storage for session data and game state

The 1:10:64 size ratio (64MB:512MB:4GB) follows cache hierarchy principles from CPU design.

### Production Performance Characteristics

**Expected Performance (on modern hardware):**

**L1 Cache**: ~100 nanoseconds access time
- DashMap lookup + clone operation
- No I/O or blocking operations

**L2 Cache**: ~1 microsecond access time  
- RwLock acquisition + LRU update + clone
- Plus promotion to L1

**L3 Cache**: ~100 microseconds access time
- File open + mmap + deserialization  
- Plus promotion to L2

**Cache Miss**: Variable (depends on backing store)
- Could be database query, network request, computation

### Key Design Principles Applied

**1. Locality of Reference**
The multi-tier design assumes temporal and spatial locality, keeping frequently accessed data in faster tiers.

**2. Write-Once, Read-Many**
The cache is optimized for read-heavy workloads typical in gaming applications.

**3. Graceful Degradation**
Cache failures don't break the system - they just reduce performance.

**4. Observability**
Comprehensive statistics enable performance monitoring and tuning.

**5. Resource Bounded**
All cache tiers have clear resource limits preventing system resource exhaustion.

### Gaming-Specific Optimizations

**Session Persistence**: L3 cache survives application restarts, maintaining game state across sessions.

**Real-Time Performance**: L1 cache provides microsecond access times required for real-time gaming.

**Memory Efficiency**: Size-aware eviction prevents memory exhaustion on resource-constrained mobile devices.

**Concurrent Access**: Lock-free L1 cache handles multiple game threads without blocking.

The BitCraps caching system demonstrates how sophisticated caching theory translates into practical, high-performance code that can support the demanding requirements of real-time distributed gaming while providing the observability and resource management needed for production deployment.
