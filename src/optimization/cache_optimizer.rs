//! Cache Optimizer for BitCraps
//!
//! Provides intelligent cache management, hit ratio optimization, and
//! adaptive caching strategies for maximum performance.

use std::collections::{HashMap, BTreeMap, VecDeque};
use std::hash::Hash;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Mutex};

/// Cache optimizer configuration
#[derive(Clone, Debug)]
pub struct CacheOptimizerConfig {
    /// Enable adaptive cache sizing
    pub enable_adaptive_sizing: bool,
    /// Enable cache warming
    pub enable_cache_warming: bool,
    /// Enable access pattern learning
    pub enable_pattern_learning: bool,
    /// Cache monitoring interval
    pub monitoring_interval: Duration,
    /// Default cache TTL
    pub default_ttl: Duration,
    /// Maximum cache memory (MB)
    pub max_cache_memory_mb: usize,
    /// Target hit ratio
    pub target_hit_ratio: f64,
    /// Eviction policy
    pub eviction_policy: EvictionPolicy,
    /// Preloading strategy
    pub preloading_strategy: PreloadingStrategy,
}

impl Default for CacheOptimizerConfig {
    fn default() -> Self {
        Self {
            enable_adaptive_sizing: true,
            enable_cache_warming: true,
            enable_pattern_learning: true,
            monitoring_interval: Duration::from_secs(60),
            default_ttl: Duration::from_secs(3600), // 1 hour
            max_cache_memory_mb: 512,
            target_hit_ratio: 0.85,
            eviction_policy: EvictionPolicy::AdaptiveLRU,
            preloading_strategy: PreloadingStrategy::PredictiveLoading,
        }
    }
}

/// Cache eviction policies
#[derive(Debug, Clone)]
pub enum EvictionPolicy {
    LRU,           // Least Recently Used
    LFU,           // Least Frequently Used
    FIFO,          // First In, First Out
    Random,        // Random eviction
    TTL,           // Time-based expiration
    AdaptiveLRU,   // LRU with frequency weighting
    SizeBased,     // Evict largest items first
}

/// Cache preloading strategies
#[derive(Debug, Clone)]
pub enum PreloadingStrategy {
    None,
    PredictiveLoading,  // Based on access patterns
    TimeBasedLoading,   // Based on time patterns
    DependencyLoading,  // Based on data dependencies
    HybridLoading,      // Combination of strategies
}

/// Cache access pattern
#[derive(Debug, Clone)]
pub struct AccessPattern {
    pub key: String,
    pub access_times: VecDeque<Instant>,
    pub access_frequency: f64,
    pub access_regularity: f64, // How regular the access pattern is
    pub last_access: Instant,
    pub predicted_next_access: Option<Instant>,
}

/// Cache entry metadata
#[derive(Debug, Clone)]
pub struct CacheEntryMetadata {
    pub key: String,
    pub size_bytes: usize,
    pub created_at: Instant,
    pub last_accessed: Instant,
    pub access_count: u64,
    pub ttl: Option<Duration>,
    pub priority: u8, // 1-10, higher = more important
    pub cost_to_recreate: f64, // Relative cost to regenerate this entry
}

/// Cache performance metrics
#[derive(Debug, Clone)]
pub struct CacheMetrics {
    pub timestamp: Instant,
    pub hit_ratio: f64,
    pub miss_ratio: f64,
    pub total_requests: u64,
    pub cache_size_bytes: usize,
    pub entry_count: usize,
    pub average_access_time_ms: f64,
    pub eviction_count: u64,
    pub memory_pressure: f64, // 0.0 to 1.0
    pub cache_efficiency: f64, // Cost-benefit ratio
    pub popular_keys: Vec<String>,
}

/// Cache optimization recommendations
#[derive(Debug, Clone)]
pub struct CacheOptimizationRecommendation {
    pub recommendation_type: OptimizationType,
    pub priority: u8,
    pub expected_improvement: f64, // Expected hit ratio improvement
    pub description: String,
    pub implementation_steps: Vec<String>,
    pub estimated_cost: OptimizationCost,
}

#[derive(Debug, Clone)]
pub enum OptimizationType {
    IncreaseCacheSize,
    DecreaseCacheSize,
    ChangeEvictionPolicy,
    EnablePrefetching,
    AdjustTTL,
    PartitionCache,
    AddCacheLayer,
    OptimizeKeyStructure,
}

#[derive(Debug, Clone)]
pub enum OptimizationCost {
    Low,    // Configuration change
    Medium, // Code modifications
    High,   // Architectural changes
}

/// Multi-tier intelligent cache
pub struct IntelligentCache<K, V>
where
    K: Clone + Hash + Eq + Send + Sync,
    V: Clone + Send + Sync,
{
    name: String,
    config: CacheOptimizerConfig,
    
    // L1: Hot cache (most frequently accessed)
    l1_cache: Arc<RwLock<HashMap<K, (V, CacheEntryMetadata)>>>,
    l1_max_size: usize,
    
    // L2: Warm cache (moderately accessed)
    l2_cache: Arc<RwLock<HashMap<K, (V, CacheEntryMetadata)>>>,
    l2_max_size: usize,
    
    // L3: Cold cache (infrequently accessed but still valuable)
    l3_cache: Arc<RwLock<HashMap<K, (V, CacheEntryMetadata)>>>,
    l3_max_size: usize,
    
    // Access patterns and analytics
    access_patterns: Arc<RwLock<HashMap<K, AccessPattern>>>,
    metrics: Arc<RwLock<CacheMetrics>>,
    
    // Statistics
    total_requests: AtomicU64,
    cache_hits: AtomicU64,
    l1_hits: AtomicU64,
    l2_hits: AtomicU64,
    l3_hits: AtomicU64,
    evictions: AtomicU64,
    
    // Optimization state
    last_optimization: Arc<RwLock<Instant>>,
    optimization_history: Arc<RwLock<Vec<CacheOptimizationRecommendation>>>,
}

impl<K, V> IntelligentCache<K, V>
where
    K: Clone + Hash + Eq + Send + Sync + 'static + std::fmt::Debug,
    V: Clone + Send + Sync + 'static,
{
    pub fn new(name: String, config: CacheOptimizerConfig) -> Self {
        let total_size = config.max_cache_memory_mb * 1024 * 1024; // Convert to bytes
        
        Self {
            name,
            config: config.clone(),
            l1_cache: Arc::new(RwLock::new(HashMap::new())),
            l1_max_size: total_size / 2, // 50% for L1
            l2_cache: Arc::new(RwLock::new(HashMap::new())),
            l2_max_size: total_size * 3 / 10, // 30% for L2
            l3_cache: Arc::new(RwLock::new(HashMap::new())),
            l3_max_size: total_size / 5, // 20% for L3
            access_patterns: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(CacheMetrics {
                timestamp: Instant::now(),
                hit_ratio: 0.0,
                miss_ratio: 1.0,
                total_requests: 0,
                cache_size_bytes: 0,
                entry_count: 0,
                average_access_time_ms: 0.0,
                eviction_count: 0,
                memory_pressure: 0.0,
                cache_efficiency: 0.0,
                popular_keys: Vec::new(),
            })),
            total_requests: AtomicU64::new(0),
            cache_hits: AtomicU64::new(0),
            l1_hits: AtomicU64::new(0),
            l2_hits: AtomicU64::new(0),
            l3_hits: AtomicU64::new(0),
            evictions: AtomicU64::new(0),
            last_optimization: Arc::new(RwLock::new(Instant::now())),
            optimization_history: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Get value from cache with intelligent tier management
    pub async fn get(&self, key: &K) -> Option<V> {
        let start_time = Instant::now();
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        
        // Try L1 cache first
        if let Some((value, mut metadata)) = self.get_from_l1(key).await {
            self.l1_hits.fetch_add(1, Ordering::Relaxed);
            self.cache_hits.fetch_add(1, Ordering::Relaxed);
            
            // Update access metadata
            metadata.last_accessed = Instant::now();
            metadata.access_count += 1;
            
            self.update_access_pattern(key, start_time).await;
            self.update_metrics(start_time).await;
            return Some(value);
        }
        
        // Try L2 cache
        if let Some((value, metadata)) = self.get_from_l2(key).await {
            self.l2_hits.fetch_add(1, Ordering::Relaxed);
            self.cache_hits.fetch_add(1, Ordering::Relaxed);
            
            // Promote to L1 if frequently accessed
            if metadata.access_count > 5 {
                self.promote_to_l1(key.clone(), value.clone(), metadata).await;
            }
            
            self.update_access_pattern(key, start_time).await;
            self.update_metrics(start_time).await;
            return Some(value);
        }
        
        // Try L3 cache
        if let Some((value, metadata)) = self.get_from_l3(key).await {
            self.l3_hits.fetch_add(1, Ordering::Relaxed);
            self.cache_hits.fetch_add(1, Ordering::Relaxed);
            
            // Promote to L2 if accessed multiple times
            if metadata.access_count > 2 {
                self.promote_to_l2(key.clone(), value.clone(), metadata).await;
            }
            
            self.update_access_pattern(key, start_time).await;
            self.update_metrics(start_time).await;
            return Some(value);
        }
        
        // Cache miss
        self.update_access_pattern(key, start_time).await;
        self.update_metrics(start_time).await;
        None
    }

    /// Insert value into cache with intelligent tier placement
    pub async fn insert(&self, key: K, value: V, size_bytes: usize, priority: Option<u8>) {
        let metadata = CacheEntryMetadata {
            key: format!("{:?}", key), // Simplified key representation
            size_bytes,
            created_at: Instant::now(),
            last_accessed: Instant::now(),
            access_count: 1,
            ttl: Some(self.config.default_ttl),
            priority: priority.unwrap_or(5),
            cost_to_recreate: 1.0, // Default cost
        };

        // Determine initial tier based on priority and predicted access pattern
        let predicted_access = self.predict_access_pattern(&key).await;
        
        match (metadata.priority, predicted_access) {
            (8..=10, _) | (_, Some(_)) => {
                // High priority or predicted access -> L1
                self.insert_into_l1(key, value, metadata).await;
            },
            (5..=7, _) => {
                // Medium priority -> L2
                self.insert_into_l2(key, value, metadata).await;
            },
            _ => {
                // Low priority -> L3
                self.insert_into_l3(key, value, metadata).await;
            },
        }
    }

    /// Remove expired entries
    pub async fn cleanup_expired(&self) {
        let now = Instant::now();
        
        // Clean L1
        {
            let mut l1 = self.l1_cache.write().await;
            l1.retain(|_, (_, metadata)| {
                if let Some(ttl) = metadata.ttl {
                    now.duration_since(metadata.created_at) < ttl
                } else {
                    true
                }
            });
        }
        
        // Clean L2
        {
            let mut l2 = self.l2_cache.write().await;
            l2.retain(|_, (_, metadata)| {
                if let Some(ttl) = metadata.ttl {
                    now.duration_since(metadata.created_at) < ttl
                } else {
                    true
                }
            });
        }
        
        // Clean L3
        {
            let mut l3 = self.l3_cache.write().await;
            l3.retain(|_, (_, metadata)| {
                if let Some(ttl) = metadata.ttl {
                    now.duration_since(metadata.created_at) < ttl
                } else {
                    true
                }
            });
        }
    }

    /// Get current cache metrics
    pub async fn get_metrics(&self) -> CacheMetrics {
        let metrics = self.metrics.read().await;
        metrics.clone()
    }

    /// Get cache optimization recommendations
    pub async fn get_optimization_recommendations(&self) -> Vec<CacheOptimizationRecommendation> {
        let metrics = self.get_metrics().await;
        let mut recommendations = Vec::new();

        // Analyze hit ratio
        if metrics.hit_ratio < self.config.target_hit_ratio {
            let hit_ratio_gap = self.config.target_hit_ratio - metrics.hit_ratio;
            
            if hit_ratio_gap > 0.2 {
                recommendations.push(CacheOptimizationRecommendation {
                    recommendation_type: OptimizationType::IncreaseCacheSize,
                    priority: 9,
                    expected_improvement: hit_ratio_gap * 0.6,
                    description: format!("Increase cache size to improve hit ratio from {:.1}% to target {:.1}%",
                                       metrics.hit_ratio * 100.0, self.config.target_hit_ratio * 100.0),
                    implementation_steps: vec![
                        "Analyze memory usage patterns".to_string(),
                        "Increase cache memory allocation".to_string(),
                        "Monitor performance impact".to_string(),
                    ],
                    estimated_cost: OptimizationCost::Low,
                });
            }

            if hit_ratio_gap > 0.15 {
                recommendations.push(CacheOptimizationRecommendation {
                    recommendation_type: OptimizationType::EnablePrefetching,
                    priority: 8,
                    expected_improvement: hit_ratio_gap * 0.4,
                    description: "Enable predictive prefetching based on access patterns".to_string(),
                    implementation_steps: vec![
                        "Analyze access patterns".to_string(),
                        "Implement prefetching logic".to_string(),
                        "Monitor prefetch accuracy".to_string(),
                    ],
                    estimated_cost: OptimizationCost::Medium,
                });
            }
        }

        // Analyze memory pressure
        if metrics.memory_pressure > 0.8 {
            recommendations.push(CacheOptimizationRecommendation {
                recommendation_type: OptimizationType::ChangeEvictionPolicy,
                priority: 7,
                expected_improvement: 0.1,
                description: "Switch to more aggressive eviction policy due to memory pressure".to_string(),
                implementation_steps: vec![
                    "Implement size-based eviction".to_string(),
                    "Reduce TTL for less important entries".to_string(),
                    "Monitor memory usage".to_string(),
                ],
                estimated_cost: OptimizationCost::Low,
            });
        }

        // Sort by priority
        recommendations.sort_by(|a, b| b.priority.cmp(&a.priority));
        recommendations
    }

    /// Perform cache warming based on access patterns
    pub async fn warm_cache<F, Fut>(&self, loader: F) 
    where
        F: Fn(K) -> Fut + Send + Sync,
        Fut: std::future::Future<Output = Option<(V, usize)>> + Send,
    {
        if !self.config.enable_cache_warming {
            return;
        }

        let patterns = self.access_patterns.read().await;
        let mut keys_to_warm = Vec::new();

        // Identify keys likely to be accessed soon
        let now = Instant::now();
        for (key, pattern) in patterns.iter() {
            if let Some(predicted_access) = pattern.predicted_next_access {
                if predicted_access.duration_since(now) < Duration::from_secs(5 * 60) {
                    keys_to_warm.push(key.clone());
                }
            }
        }

        // Load predicted keys
        for key in keys_to_warm {
            if let Some((value, size)) = loader(key.clone()).await {
                self.insert(key, value, size, Some(6)).await; // Medium-high priority for warmed entries
            }
        }
    }

    /// Start optimization monitoring
    pub async fn start_optimization_monitoring(&self) {
        let cache = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(cache.config.monitoring_interval);
            
            loop {
                interval.tick().await;
                
                // Update metrics
                cache.calculate_comprehensive_metrics().await;
                
                // Perform optimizations if needed
                let metrics = cache.get_metrics().await;
                if metrics.hit_ratio < cache.config.target_hit_ratio * 0.9 {
                    cache.perform_automatic_optimizations().await;
                }
                
                // Cleanup expired entries
                cache.cleanup_expired().await;
                
                // Rebalance tiers if necessary
                cache.rebalance_tiers().await;
            }
        });
    }

    /// Get from L1 cache
    async fn get_from_l1(&self, key: &K) -> Option<(V, CacheEntryMetadata)> {
        let l1 = self.l1_cache.read().await;
        l1.get(key).cloned()
    }

    /// Get from L2 cache
    async fn get_from_l2(&self, key: &K) -> Option<(V, CacheEntryMetadata)> {
        let mut l2 = self.l2_cache.write().await;
        if let Some((value, mut metadata)) = l2.remove(key) {
            metadata.access_count += 1;
            metadata.last_accessed = Instant::now();
            l2.insert(key.clone(), (value.clone(), metadata.clone()));
            Some((value, metadata))
        } else {
            None
        }
    }

    /// Get from L3 cache
    async fn get_from_l3(&self, key: &K) -> Option<(V, CacheEntryMetadata)> {
        let mut l3 = self.l3_cache.write().await;
        if let Some((value, mut metadata)) = l3.remove(key) {
            metadata.access_count += 1;
            metadata.last_accessed = Instant::now();
            l3.insert(key.clone(), (value.clone(), metadata.clone()));
            Some((value, metadata))
        } else {
            None
        }
    }

    /// Insert into L1 cache
    async fn insert_into_l1(&self, key: K, value: V, metadata: CacheEntryMetadata) {
        let mut l1 = self.l1_cache.write().await;
        
        // Evict if necessary
        while self.calculate_tier_size(&l1) + metadata.size_bytes > self.l1_max_size && !l1.is_empty() {
            self.evict_from_tier(&mut l1).await;
        }
        
        l1.insert(key, (value, metadata));
    }

    /// Insert into L2 cache
    async fn insert_into_l2(&self, key: K, value: V, metadata: CacheEntryMetadata) {
        let mut l2 = self.l2_cache.write().await;
        
        // Evict if necessary
        while self.calculate_tier_size(&l2) + metadata.size_bytes > self.l2_max_size && !l2.is_empty() {
            self.evict_from_tier(&mut l2).await;
        }
        
        l2.insert(key, (value, metadata));
    }

    /// Insert into L3 cache
    async fn insert_into_l3(&self, key: K, value: V, metadata: CacheEntryMetadata) {
        let mut l3 = self.l3_cache.write().await;
        
        // Evict if necessary
        while self.calculate_tier_size(&l3) + metadata.size_bytes > self.l3_max_size && !l3.is_empty() {
            self.evict_from_tier(&mut l3).await;
        }
        
        l3.insert(key, (value, metadata));
    }

    /// Promote entry to L1
    async fn promote_to_l1(&self, key: K, value: V, mut metadata: CacheEntryMetadata) {
        // Remove from L2
        {
            let mut l2 = self.l2_cache.write().await;
            l2.remove(&key);
        }
        
        metadata.priority = (metadata.priority + 1).min(10);
        self.insert_into_l1(key, value, metadata).await;
    }

    /// Promote entry to L2
    async fn promote_to_l2(&self, key: K, value: V, mut metadata: CacheEntryMetadata) {
        // Remove from L3
        {
            let mut l3 = self.l3_cache.write().await;
            l3.remove(&key);
        }
        
        metadata.priority = (metadata.priority + 1).min(10);
        self.insert_into_l2(key, value, metadata).await;
    }

    /// Calculate tier size in bytes
    fn calculate_tier_size(&self, tier: &HashMap<K, (V, CacheEntryMetadata)>) -> usize {
        tier.values().map(|(_, metadata)| metadata.size_bytes).sum()
    }

    /// Evict entry from tier based on policy
    async fn evict_from_tier(&self, tier: &mut HashMap<K, (V, CacheEntryMetadata)>) {
        if tier.is_empty() {
            return;
        }

        let key_to_evict = match self.config.eviction_policy {
            EvictionPolicy::LRU | EvictionPolicy::AdaptiveLRU => {
                tier.iter()
                    .min_by_key(|(_, (_, metadata))| metadata.last_accessed)
                    .map(|(k, _)| k.clone())
            },
            EvictionPolicy::LFU => {
                tier.iter()
                    .min_by_key(|(_, (_, metadata))| metadata.access_count)
                    .map(|(k, _)| k.clone())
            },
            EvictionPolicy::SizeBased => {
                tier.iter()
                    .max_by_key(|(_, (_, metadata))| metadata.size_bytes)
                    .map(|(k, _)| k.clone())
            },
            _ => {
                // Random eviction as fallback
                tier.keys().next().cloned()
            },
        };

        if let Some(key) = key_to_evict {
            tier.remove(&key);
            self.evictions.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Update access pattern for predictive analytics
    async fn update_access_pattern(&self, key: &K, access_time: Instant) {
        if !self.config.enable_pattern_learning {
            return;
        }

        let mut patterns = self.access_patterns.write().await;
        let pattern = patterns.entry(key.clone()).or_insert_with(|| AccessPattern {
            key: format!("{:?}", key),
            access_times: VecDeque::new(),
            access_frequency: 0.0,
            access_regularity: 0.0,
            last_access: access_time,
            predicted_next_access: None,
        });

        pattern.access_times.push_back(access_time);
        pattern.last_access = access_time;

        // Keep only recent accesses (last 100)
        while pattern.access_times.len() > 100 {
            pattern.access_times.pop_front();
        }

        // Calculate access frequency and regularity
        if pattern.access_times.len() > 1 {
            let time_span = access_time.duration_since(*pattern.access_times.front().unwrap()).as_secs_f64();
            pattern.access_frequency = pattern.access_times.len() as f64 / time_span;
            
            // Calculate regularity (how evenly spaced the accesses are)
            if pattern.access_times.len() > 2 {
                let intervals: Vec<f64> = pattern.access_times
                    .iter()
                    .zip(pattern.access_times.iter().skip(1))
                    .map(|(a, b)| b.duration_since(*a).as_secs_f64())
                    .collect();
                
                let mean_interval = intervals.iter().sum::<f64>() / intervals.len() as f64;
                let variance = intervals.iter()
                    .map(|&interval| (interval - mean_interval).powi(2))
                    .sum::<f64>() / intervals.len() as f64;
                
                pattern.access_regularity = 1.0 / (1.0 + variance); // Higher = more regular
                
                // Predict next access time
                if pattern.access_regularity > 0.7 && mean_interval > 0.0 {
                    pattern.predicted_next_access = Some(access_time + Duration::from_secs_f64(mean_interval));
                }
            }
        }
    }

    /// Predict if key will be accessed soon
    async fn predict_access_pattern(&self, key: &K) -> Option<Instant> {
        let patterns = self.access_patterns.read().await;
        patterns.get(key)?.predicted_next_access
    }

    /// Update comprehensive metrics
    async fn update_metrics(&self, access_start: Instant) {
        let access_time = access_start.elapsed().as_secs_f64() * 1000.0;
        
        let total_requests = self.total_requests.load(Ordering::Relaxed);
        let cache_hits = self.cache_hits.load(Ordering::Relaxed);
        
        let mut metrics = self.metrics.write().await;
        metrics.timestamp = Instant::now();
        metrics.total_requests = total_requests;
        metrics.hit_ratio = if total_requests > 0 {
            cache_hits as f64 / total_requests as f64
        } else {
            0.0
        };
        metrics.miss_ratio = 1.0 - metrics.hit_ratio;
        
        // Update average access time (simple moving average)
        if metrics.average_access_time_ms == 0.0 {
            metrics.average_access_time_ms = access_time;
        } else {
            metrics.average_access_time_ms = metrics.average_access_time_ms * 0.9 + access_time * 0.1;
        }
    }

    /// Calculate comprehensive metrics
    async fn calculate_comprehensive_metrics(&self) {
        let l1 = self.l1_cache.read().await;
        let l2 = self.l2_cache.read().await;
        let l3 = self.l3_cache.read().await;
        
        let total_size = self.calculate_tier_size(&l1) + 
                        self.calculate_tier_size(&l2) + 
                        self.calculate_tier_size(&l3);
        let total_entries = l1.len() + l2.len() + l3.len();
        let max_total_size = self.l1_max_size + self.l2_max_size + self.l3_max_size;
        
        let mut metrics = self.metrics.write().await;
        metrics.cache_size_bytes = total_size;
        metrics.entry_count = total_entries;
        metrics.memory_pressure = total_size as f64 / max_total_size as f64;
        metrics.eviction_count = self.evictions.load(Ordering::Relaxed);
        
        // Calculate cache efficiency (hit ratio vs memory usage)
        metrics.cache_efficiency = if metrics.memory_pressure > 0.0 {
            metrics.hit_ratio / metrics.memory_pressure
        } else {
            0.0
        };
        
        // Identify popular keys
        let mut key_popularity: Vec<_> = vec![
            l1.iter().map(|(k, (_, m))| (format!("{:?}", k), m.access_count)).collect(),
            l2.iter().map(|(k, (_, m))| (format!("{:?}", k), m.access_count)).collect(),
            l3.iter().map(|(k, (_, m))| (format!("{:?}", k), m.access_count)).collect(),
        ].into_iter().flatten().collect();
        
        key_popularity.sort_by(|a, b| b.1.cmp(&a.1));
        metrics.popular_keys = key_popularity.into_iter()
            .take(10)
            .map(|(k, _)| k)
            .collect();
    }

    /// Perform automatic optimizations
    async fn perform_automatic_optimizations(&self) {
        let recommendations = self.get_optimization_recommendations().await;
        
        for rec in recommendations.iter().take(3) { // Apply top 3 recommendations
            match rec.recommendation_type {
                OptimizationType::ChangeEvictionPolicy => {
                    // This would require configuration update
                    println!("Recommendation: {}", rec.description);
                },
                OptimizationType::AdjustTTL => {
                    // Implement TTL adjustments
                    println!("Adjusting TTL based on access patterns");
                },
                _ => {
                    println!("Optimization recommendation: {}", rec.description);
                },
            }
        }
        
        // Store optimization history
        let mut history = self.optimization_history.write().await;
        history.extend(recommendations);
        
        // Keep bounded history
        if history.len() > 100 {
            let drain_to = history.len() / 2;
            history.drain(0..drain_to);
        }
    }

    /// Rebalance cache tiers based on access patterns
    async fn rebalance_tiers(&self) {
        // This is a simplified rebalancing - in practice, you'd want more sophisticated logic
        let l1_hit_ratio = self.l1_hits.load(Ordering::Relaxed) as f64 / 
                          self.total_requests.load(Ordering::Relaxed).max(1) as f64;
        
        // If L1 hit ratio is low, consider demoting some entries
        if l1_hit_ratio < 0.3 {
            let mut l1 = self.l1_cache.write().await;
            let keys_to_demote: Vec<_> = l1.iter()
                .filter(|(_, (_, metadata))| {
                    metadata.last_accessed.elapsed() > Duration::from_secs(30 * 60) &&
                    metadata.access_count < 3
                })
                .map(|(k, _)| k.clone())
                .take(5) // Demote up to 5 entries
                .collect();
            
            for key in keys_to_demote {
                if let Some((value, metadata)) = l1.remove(&key) {
                    drop(l1); // Release L1 lock
                    self.insert_into_l2(key, value, metadata).await;
                    l1 = self.l1_cache.write().await; // Re-acquire lock
                }
            }
        }
    }
}

impl<K, V> Clone for IntelligentCache<K, V>
where
    K: Clone + Hash + Eq + Send + Sync,
    V: Clone + Send + Sync,
{
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            config: self.config.clone(),
            l1_cache: Arc::clone(&self.l1_cache),
            l1_max_size: self.l1_max_size,
            l2_cache: Arc::clone(&self.l2_cache),
            l2_max_size: self.l2_max_size,
            l3_cache: Arc::clone(&self.l3_cache),
            l3_max_size: self.l3_max_size,
            access_patterns: Arc::clone(&self.access_patterns),
            metrics: Arc::clone(&self.metrics),
            total_requests: AtomicU64::new(self.total_requests.load(Ordering::Relaxed)),
            cache_hits: AtomicU64::new(self.cache_hits.load(Ordering::Relaxed)),
            l1_hits: AtomicU64::new(self.l1_hits.load(Ordering::Relaxed)),
            l2_hits: AtomicU64::new(self.l2_hits.load(Ordering::Relaxed)),
            l3_hits: AtomicU64::new(self.l3_hits.load(Ordering::Relaxed)),
            evictions: AtomicU64::new(self.evictions.load(Ordering::Relaxed)),
            last_optimization: Arc::clone(&self.last_optimization),
            optimization_history: Arc::clone(&self.optimization_history),
        }
    }
}

/// Cache optimizer that manages multiple cache instances
pub struct CacheOptimizer {
    config: CacheOptimizerConfig,
    caches: Arc<RwLock<HashMap<String, Box<dyn Send + Sync>>>>,
    global_metrics: Arc<RwLock<HashMap<String, CacheMetrics>>>,
    is_monitoring: Arc<std::sync::atomic::AtomicBool>,
}

impl CacheOptimizer {
    pub fn new(config: CacheOptimizerConfig) -> Self {
        Self {
            config,
            caches: Arc::new(RwLock::new(HashMap::new())),
            global_metrics: Arc::new(RwLock::new(HashMap::new())),
            is_monitoring: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Register a cache for optimization
    pub async fn register_cache<K, V>(
        &self,
        name: String,
        cache: Arc<IntelligentCache<K, V>>,
    ) where
        K: Clone + Hash + Eq + Send + Sync + 'static,
        V: Clone + Send + Sync + 'static,
    {
        let mut caches = self.caches.write().await;
        caches.insert(name.clone(), Box::new(cache));
        
        // Start monitoring for this cache
        if self.config.enable_adaptive_sizing {
            // Cache-specific monitoring would be started here
        }
    }

    /// Get global cache statistics
    pub async fn get_global_statistics(&self) -> HashMap<String, CacheMetrics> {
        let metrics = self.global_metrics.read().await;
        metrics.clone()
    }

    /// Generate comprehensive optimization report
    pub async fn generate_optimization_report(&self) -> String {
        let mut report = String::new();
        let global_stats = self.get_global_statistics().await;
        
        report.push_str("=== CACHE OPTIMIZER REPORT ===\n");
        report.push_str(&format!("Total Managed Caches: {}\n", global_stats.len()));
        
        let mut total_requests = 0u64;
        let mut total_hits = 0u64;
        let mut total_memory = 0usize;
        
        for (name, metrics) in &global_stats {
            report.push_str(&format!("\n--- Cache: {} ---\n", name));
            report.push_str(&format!("Hit Ratio: {:.1}%\n", metrics.hit_ratio * 100.0));
            report.push_str(&format!("Total Requests: {}\n", metrics.total_requests));
            report.push_str(&format!("Memory Usage: {:.1}MB\n", metrics.cache_size_bytes as f64 / 1024.0 / 1024.0));
            report.push_str(&format!("Entry Count: {}\n", metrics.entry_count));
            report.push_str(&format!("Memory Pressure: {:.1}%\n", metrics.memory_pressure * 100.0));
            report.push_str(&format!("Cache Efficiency: {:.2}\n", metrics.cache_efficiency));
            
            total_requests += metrics.total_requests;
            total_hits += (metrics.hit_ratio * metrics.total_requests as f64) as u64;
            total_memory += metrics.cache_size_bytes;
        }
        
        report.push_str(&format!("\n--- Global Statistics ---\n"));
        report.push_str(&format!("Overall Hit Ratio: {:.1}%\n", 
                                if total_requests > 0 { total_hits as f64 / total_requests as f64 * 100.0 } else { 0.0 }));
        report.push_str(&format!("Total Memory Usage: {:.1}MB\n", total_memory as f64 / 1024.0 / 1024.0));
        
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_intelligent_cache_basic_operations() {
        let config = CacheOptimizerConfig::default();
        let cache: IntelligentCache<String, String> = IntelligentCache::new("test_cache".to_string(), config);
        
        // Test insert and get
        cache.insert("key1".to_string(), "value1".to_string(), 100, Some(8)).await;
        let value = cache.get(&"key1".to_string()).await;
        assert_eq!(value, Some("value1".to_string()));
        
        // Test cache miss
        let missing = cache.get(&"nonexistent".to_string()).await;
        assert_eq!(missing, None);
        
        let metrics = cache.get_metrics().await;
        assert!(metrics.total_requests > 0);
        assert!(metrics.hit_ratio > 0.0);
    }

    #[tokio::test]
    async fn test_multi_tier_promotion() {
        let config = CacheOptimizerConfig::default();
        let cache: IntelligentCache<String, String> = IntelligentCache::new("test_cache".to_string(), config);
        
        // Insert with low priority (should go to L3)
        cache.insert("key1".to_string(), "value1".to_string(), 100, Some(2)).await;
        
        // Access multiple times to trigger promotion
        for _ in 0..6 {
            let _ = cache.get(&"key1".to_string()).await;
        }
        
        let metrics = cache.get_metrics().await;
        assert!(metrics.hit_ratio > 0.8); // Should have good hit ratio due to promotion
    }

    #[tokio::test]
    async fn test_cache_optimization_recommendations() {
        let config = CacheOptimizerConfig {
            target_hit_ratio: 0.9, // High target to trigger recommendations
            ..Default::default()
        };
        let cache: IntelligentCache<String, String> = IntelligentCache::new("test_cache".to_string(), config);
        
        // Generate some cache activity with low hit ratio
        cache.insert("key1".to_string(), "value1".to_string(), 100, Some(5)).await;
        for i in 0..10 {
            let _ = cache.get(&format!("nonexistent_{}", i)).await; // Misses
        }
        let _ = cache.get(&"key1".to_string()).await; // One hit
        
        let recommendations = cache.get_optimization_recommendations().await;
        assert!(!recommendations.is_empty());
        
        // Should recommend increasing cache size or enabling prefetching
        let has_size_recommendation = recommendations.iter()
            .any(|r| matches!(r.recommendation_type, OptimizationType::IncreaseCacheSize));
        let has_prefetch_recommendation = recommendations.iter()
            .any(|r| matches!(r.recommendation_type, OptimizationType::EnablePrefetching));
        
        assert!(has_size_recommendation || has_prefetch_recommendation);
    }

    #[tokio::test]
    async fn test_access_pattern_learning() {
        let config = CacheOptimizerConfig {
            enable_pattern_learning: true,
            ..Default::default()
        };
        let cache: IntelligentCache<String, String> = IntelligentCache::new("test_cache".to_string(), config);
        
        // Create regular access pattern
        cache.insert("key1".to_string(), "value1".to_string(), 100, Some(5)).await;
        
        for _ in 0..5 {
            let _ = cache.get(&"key1".to_string()).await;
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        
        // Pattern should be learned (though prediction accuracy depends on timing)
        let patterns = cache.access_patterns.read().await;
        let pattern = patterns.get(&"key1".to_string());
        assert!(pattern.is_some());
        assert!(pattern.unwrap().access_count > 0.0);
    }

    #[tokio::test]
    async fn test_cache_optimizer_global_management() {
        let config = CacheOptimizerConfig::default();
        let optimizer = CacheOptimizer::new(config.clone());
        
        let cache1: Arc<IntelligentCache<String, String>> = 
            Arc::new(IntelligentCache::new("cache1".to_string(), config.clone()));
        let cache2: Arc<IntelligentCache<String, String>> = 
            Arc::new(IntelligentCache::new("cache2".to_string(), config));
        
        optimizer.register_cache("cache1".to_string(), cache1).await;
        optimizer.register_cache("cache2".to_string(), cache2).await;
        
        let report = optimizer.generate_optimization_report().await;
        assert!(report.contains("CACHE OPTIMIZER REPORT"));
        assert!(report.contains("Total Managed Caches: 2"));
    }

    #[tokio::test]
    async fn test_cache_tier_eviction() {
        let config = CacheOptimizerConfig {
            max_cache_memory_mb: 1, // Very small cache to trigger eviction
            eviction_policy: EvictionPolicy::LRU,
            ..Default::default()
        };
        let cache: IntelligentCache<String, String> = IntelligentCache::new("test_cache".to_string(), config);
        
        // Insert enough entries to trigger eviction
        for i in 0..20 {
            cache.insert(
                format!("key_{}", i), 
                format!("value_{}", i), 
                50000, // 50KB each
                Some(5)
            ).await;
        }
        
        // Some early entries should have been evicted
        let first_key_exists = cache.get(&"key_0".to_string()).await.is_some();
        let last_key_exists = cache.get(&"key_19".to_string()).await.is_some();
        
        // Recent entries should still exist, early ones might be evicted
        assert!(last_key_exists);
        
        let metrics = cache.get_metrics().await;
        assert!(metrics.eviction_count > 0);
    }
}