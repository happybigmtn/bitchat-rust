//! Query Optimizer for BitCraps
//!
//! Provides intelligent query optimization, caching, and performance tuning
//! for database operations with adaptive query planning.

use std::collections::{HashMap, BTreeMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

/// Query optimizer configuration
#[derive(Clone, Debug)]
pub struct QueryOptimizerConfig {
    /// Enable query caching
    pub enable_query_cache: bool,
    /// Enable query rewriting
    pub enable_query_rewriting: bool,
    /// Enable execution plan caching
    pub enable_plan_cache: bool,
    /// Maximum cached queries
    pub max_cached_queries: usize,
    /// Query cache TTL
    pub cache_ttl: Duration,
    /// Slow query threshold
    pub slow_query_threshold_ms: u64,
    /// Statistics collection interval
    pub stats_collection_interval: Duration,
    /// Enable adaptive optimization
    pub enable_adaptive_optimization: bool,
    /// Query timeout
    pub default_timeout: Duration,
}

impl Default for QueryOptimizerConfig {
    fn default() -> Self {
        Self {
            enable_query_cache: true,
            enable_query_rewriting: true,
            enable_plan_cache: true,
            max_cached_queries: 10000,
            cache_ttl: Duration::from_secs(3600), // 1 hour
            slow_query_threshold_ms: 100,
            stats_collection_interval: Duration::from_secs(60),
            enable_adaptive_optimization: true,
            default_timeout: Duration::from_secs(30),
        }
    }
}

/// Query metadata and statistics
#[derive(Debug, Clone)]
pub struct QueryMetadata {
    pub query_id: String,
    pub query_text: String,
    pub query_hash: u64,
    pub execution_count: u64,
    pub total_execution_time_ms: u64,
    pub average_execution_time_ms: f64,
    pub min_execution_time_ms: u64,
    pub max_execution_time_ms: u64,
    pub last_executed: Instant,
    pub error_count: u64,
    pub result_cache_hits: u64,
    pub plan_cache_hits: u64,
    pub optimization_level: OptimizationLevel,
    pub table_access_patterns: HashMap<String, TableAccessPattern>,
}

/// Table access patterns for optimization
#[derive(Debug, Clone)]
pub struct TableAccessPattern {
    pub table_name: String,
    pub access_type: AccessType,
    pub columns_accessed: Vec<String>,
    pub filter_conditions: Vec<String>,
    pub join_conditions: Vec<String>,
    pub access_frequency: u64,
    pub selectivity: f64, // Fraction of rows returned
}

#[derive(Debug, Clone)]
pub enum AccessType {
    FullScan,
    IndexScan,
    IndexSeek,
    KeyLookup,
    Join,
    Aggregation,
}

/// Query execution plan
#[derive(Debug, Clone)]
pub struct ExecutionPlan {
    pub plan_id: String,
    pub query_hash: u64,
    pub plan_type: PlanType,
    pub estimated_cost: f64,
    pub estimated_rows: u64,
    pub operators: Vec<QueryOperator>,
    pub indexes_used: Vec<String>,
    pub created_at: Instant,
    pub usage_count: u64,
    pub actual_performance: Option<ActualPerformance>,
}

#[derive(Debug, Clone)]
pub enum PlanType {
    Simple,
    Complex,
    Parallel,
    Cached,
    Optimized,
}

#[derive(Debug, Clone)]
pub struct QueryOperator {
    pub operator_type: OperatorType,
    pub estimated_cost: f64,
    pub estimated_rows: u64,
    pub table_name: Option<String>,
    pub index_name: Option<String>,
    pub filter_conditions: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum OperatorType {
    TableScan,
    IndexScan,
    IndexSeek,
    NestedLoop,
    HashJoin,
    SortMergeJoin,
    Aggregation,
    Sort,
    Filter,
    Projection,
}

#[derive(Debug, Clone)]
pub struct ActualPerformance {
    pub actual_execution_time_ms: u64,
    pub actual_rows_processed: u64,
    pub cpu_time_ms: u64,
    pub io_operations: u64,
    pub memory_usage_mb: f64,
}

/// Optimization levels
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum OptimizationLevel {
    None,
    Basic,
    Standard,
    Advanced,
    Aggressive,
}

/// Query rewrite rules
#[derive(Debug, Clone)]
pub struct QueryRewriteRule {
    pub rule_id: String,
    pub rule_type: RewriteRuleType,
    pub pattern: String,
    pub replacement: String,
    pub conditions: Vec<String>,
    pub estimated_improvement: f64, // Performance improvement factor
}

#[derive(Debug, Clone)]
pub enum RewriteRuleType {
    PredicatePushdown,
    ProjectionPushdown,
    JoinReordering,
    SubqueryToJoin,
    InToExists,
    UnionToUnionAll,
    RedundantOperatorElimination,
    ConstantFolding,
}

/// Query result cache entry
#[derive(Debug, Clone)]
pub struct QueryCacheEntry {
    pub query_hash: u64,
    pub result_data: Vec<u8>, // Serialized result
    pub result_metadata: ResultMetadata,
    pub created_at: Instant,
    pub access_count: u64,
    pub last_accessed: Instant,
    pub ttl: Duration,
}

#[derive(Debug, Clone)]
pub struct ResultMetadata {
    pub row_count: u64,
    pub column_count: usize,
    pub data_size_bytes: usize,
    pub execution_time_ms: u64,
}

/// Query optimizer statistics
#[derive(Debug, Clone)]
pub struct OptimizerStatistics {
    pub total_queries: u64,
    pub cached_queries: u64,
    pub cache_hit_ratio: f64,
    pub plan_cache_hit_ratio: f64,
    pub average_query_time_ms: f64,
    pub slow_queries: u64,
    pub optimized_queries: u64,
    pub rewritten_queries: u64,
    pub top_slow_queries: Vec<QueryMetadata>,
    pub optimization_savings_ms: u64,
}

/// Main query optimizer
pub struct QueryOptimizer {
    config: QueryOptimizerConfig,
    query_metadata: Arc<RwLock<HashMap<String, QueryMetadata>>>,
    execution_plans: Arc<RwLock<HashMap<u64, ExecutionPlan>>>,
    query_cache: Arc<RwLock<HashMap<u64, QueryCacheEntry>>>,
    rewrite_rules: Arc<RwLock<Vec<QueryRewriteRule>>>,
    query_statistics: Arc<RwLock<OptimizerStatistics>>,
    
    // Performance counters
    total_queries: AtomicU64,
    cache_hits: AtomicU64,
    plan_cache_hits: AtomicU64,
    total_execution_time_ms: AtomicU64,
    
    // Monitoring
    is_monitoring: std::sync::atomic::AtomicBool,
}

impl QueryOptimizer {
    pub fn new(config: QueryOptimizerConfig) -> Self {
        let mut optimizer = Self {
            config,
            query_metadata: Arc::new(RwLock::new(HashMap::new())),
            execution_plans: Arc::new(RwLock::new(HashMap::new())),
            query_cache: Arc::new(RwLock::new(HashMap::new())),
            rewrite_rules: Arc::new(RwLock::new(Vec::new())),
            query_statistics: Arc::new(RwLock::new(OptimizerStatistics {
                total_queries: 0,
                cached_queries: 0,
                cache_hit_ratio: 0.0,
                plan_cache_hit_ratio: 0.0,
                average_query_time_ms: 0.0,
                slow_queries: 0,
                optimized_queries: 0,
                rewritten_queries: 0,
                top_slow_queries: Vec::new(),
                optimization_savings_ms: 0,
            })),
            total_queries: AtomicU64::new(0),
            cache_hits: AtomicU64::new(0),
            plan_cache_hits: AtomicU64::new(0),
            total_execution_time_ms: AtomicU64::new(0),
            is_monitoring: std::sync::atomic::AtomicBool::new(false),
        };

        // Initialize default rewrite rules
        optimizer.initialize_rewrite_rules();
        optimizer
    }

    /// Start query optimizer monitoring
    pub async fn start(&self) {
        if self.is_monitoring.swap(true, Ordering::Relaxed) {
            return; // Already running
        }

        println!("Starting query optimizer with config: {:?}", self.config);
        
        // Start statistics collection
        self.start_statistics_collection().await;
        
        // Start cache cleanup
        self.start_cache_cleanup().await;
        
        // Start adaptive optimization if enabled
        if self.config.enable_adaptive_optimization {
            self.start_adaptive_optimization().await;
        }
    }

    /// Stop query optimizer
    pub async fn stop(&self) {
        self.is_monitoring.store(false, Ordering::Relaxed);
        println!("Stopping query optimizer");
    }

    /// Optimize and execute a query
    pub async fn optimize_and_execute<T, F, Fut>(
        &self,
        query: &str,
        executor: F,
    ) -> Result<T, Box<dyn std::error::Error + Send + Sync>>
    where
        F: FnOnce(String) -> Fut,
        Fut: std::future::Future<Output = Result<T, Box<dyn std::error::Error + Send + Sync>>>,
    {
        let start_time = Instant::now();
        let query_hash = self.hash_query(query);
        let query_id = format!("query_{}", Uuid::new_v4());

        self.total_queries.fetch_add(1, Ordering::Relaxed);

        // Check query result cache first
        if self.config.enable_query_cache {
            if let Some(cached_result) = self.get_cached_result(query_hash).await {
                self.cache_hits.fetch_add(1, Ordering::Relaxed);
                // In a real implementation, you'd deserialize and return the cached result
                // For now, we'll continue with execution
            }
        }

        // Get or create execution plan
        let execution_plan = if self.config.enable_plan_cache {
            self.get_or_create_execution_plan(query, query_hash).await?
        } else {
            self.create_execution_plan(query, query_hash).await?
        };

        // Apply query rewriting if enabled
        let optimized_query = if self.config.enable_query_rewriting {
            self.rewrite_query(query).await
        } else {
            query.to_string()
        };

        // Execute the optimized query
        let execution_start = Instant::now();
        let result = executor(optimized_query).await;
        let execution_time = execution_start.elapsed();

        // Record performance metrics
        self.record_query_execution(
            &query_id,
            query,
            query_hash,
            &execution_plan,
            execution_time,
            result.is_ok(),
        ).await;

        // Cache successful results if configured
        if result.is_ok() && self.config.enable_query_cache {
            // In a real implementation, you'd serialize and cache the result
            self.cache_query_result(query_hash, &result, execution_time).await;
        }

        let total_time = start_time.elapsed();
        self.total_execution_time_ms.fetch_add(
            total_time.as_millis() as u64,
            Ordering::Relaxed
        );

        result
    }

    /// Get query statistics
    pub async fn get_statistics(&self) -> OptimizerStatistics {
        let stats = self.query_statistics.read().await;
        stats.clone()
    }

    /// Get query metadata for a specific query
    pub async fn get_query_metadata(&self, query_id: &str) -> Option<QueryMetadata> {
        let metadata = self.query_metadata.read().await;
        metadata.get(query_id).cloned()
    }

    /// Add custom rewrite rule
    pub async fn add_rewrite_rule(&self, rule: QueryRewriteRule) {
        let mut rules = self.rewrite_rules.write().await;
        rules.push(rule);
    }

    /// Clear query cache
    pub async fn clear_cache(&self) {
        let mut cache = self.query_cache.write().await;
        cache.clear();
        println!("Query cache cleared");
    }

    /// Generate optimization report
    pub async fn generate_report(&self) -> String {
        let stats = self.get_statistics().await;
        let mut report = String::new();

        report.push_str("=== QUERY OPTIMIZER REPORT ===\n");
        report.push_str(&format!("Total Queries: {}\n", stats.total_queries));
        report.push_str(&format!("Cached Queries: {}\n", stats.cached_queries));
        report.push_str(&format!("Cache Hit Ratio: {:.1}%\n", stats.cache_hit_ratio * 100.0));
        report.push_str(&format!("Plan Cache Hit Ratio: {:.1}%\n", stats.plan_cache_hit_ratio * 100.0));
        report.push_str(&format!("Average Query Time: {:.2}ms\n", stats.average_query_time_ms));
        report.push_str(&format!("Slow Queries: {}\n", stats.slow_queries));
        report.push_str(&format!("Optimized Queries: {}\n", stats.optimized_queries));
        report.push_str(&format!("Rewritten Queries: {}\n", stats.rewritten_queries));
        report.push_str(&format!("Optimization Savings: {}ms\n", stats.optimization_savings_ms));

        if !stats.top_slow_queries.is_empty() {
            report.push_str("\n--- Top Slow Queries ---\n");
            for (i, query) in stats.top_slow_queries.iter().take(5).enumerate() {
                report.push_str(&format!(
                    "{}. {} (avg: {:.2}ms, executions: {})\n",
                    i + 1,
                    query.query_text.chars().take(100).collect::<String>(),
                    query.average_execution_time_ms,
                    query.execution_count
                ));
            }
        }

        report
    }

    /// Hash a query for caching
    fn hash_query(&self, query: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        query.trim().to_lowercase().hash(&mut hasher);
        hasher.finish()
    }

    /// Get cached query result
    async fn get_cached_result(&self, query_hash: u64) -> Option<QueryCacheEntry> {
        let mut cache = self.query_cache.write().await;
        
        if let Some(entry) = cache.get_mut(&query_hash) {
            let now = Instant::now();
            
            // Check if entry is still valid
            if now.duration_since(entry.created_at) < entry.ttl {
                entry.access_count += 1;
                entry.last_accessed = now;
                Some(entry.clone())
            } else {
                // Entry expired
                cache.remove(&query_hash);
                None
            }
        } else {
            None
        }
    }

    /// Get or create execution plan
    async fn get_or_create_execution_plan(
        &self,
        query: &str,
        query_hash: u64,
    ) -> Result<ExecutionPlan, Box<dyn std::error::Error + Send + Sync>> {
        // Check plan cache first
        {
            let mut plans = self.execution_plans.write().await;
            if let Some(plan) = plans.get_mut(&query_hash) {
                plan.usage_count += 1;
                self.plan_cache_hits.fetch_add(1, Ordering::Relaxed);
                return Ok(plan.clone());
            }
        }

        // Create new execution plan
        let plan = self.create_execution_plan(query, query_hash).await?;
        
        // Cache the plan
        {
            let mut plans = self.execution_plans.write().await;
            plans.insert(query_hash, plan.clone());
        }

        Ok(plan)
    }

    /// Create execution plan for query
    async fn create_execution_plan(
        &self,
        query: &str,
        query_hash: u64,
    ) -> Result<ExecutionPlan, Box<dyn std::error::Error + Send + Sync>> {
        // Simplified execution plan creation
        // In a real implementation, this would analyze the query structure
        
        let plan_type = if query.contains("JOIN") {
            PlanType::Complex
        } else if query.contains("GROUP BY") || query.contains("ORDER BY") {
            PlanType::Simple
        } else {
            PlanType::Simple
        };

        let operators = self.analyze_query_operators(query);
        let estimated_cost = self.estimate_query_cost(&operators);

        Ok(ExecutionPlan {
            plan_id: format!("plan_{}", Uuid::new_v4()),
            query_hash,
            plan_type,
            estimated_cost,
            estimated_rows: 1000, // Simplified estimate
            operators,
            indexes_used: self.identify_useful_indexes(query),
            created_at: Instant::now(),
            usage_count: 1,
            actual_performance: None,
        })
    }

    /// Analyze query operators
    fn analyze_query_operators(&self, query: &str) -> Vec<QueryOperator> {
        let mut operators = Vec::new();
        let query_upper = query.to_uppercase();

        // Simplified operator detection
        if query_upper.contains("SELECT") {
            operators.push(QueryOperator {
                operator_type: OperatorType::Projection,
                estimated_cost: 10.0,
                estimated_rows: 100,
                table_name: None,
                index_name: None,
                filter_conditions: Vec::new(),
            });
        }

        if query_upper.contains("FROM") {
            operators.push(QueryOperator {
                operator_type: OperatorType::TableScan,
                estimated_cost: 100.0,
                estimated_rows: 1000,
                table_name: Some("table".to_string()), // Simplified
                index_name: None,
                filter_conditions: Vec::new(),
            });
        }

        if query_upper.contains("WHERE") {
            operators.push(QueryOperator {
                operator_type: OperatorType::Filter,
                estimated_cost: 50.0,
                estimated_rows: 300,
                table_name: None,
                index_name: None,
                filter_conditions: vec!["condition".to_string()], // Simplified
            });
        }

        if query_upper.contains("JOIN") {
            operators.push(QueryOperator {
                operator_type: OperatorType::HashJoin,
                estimated_cost: 200.0,
                estimated_rows: 500,
                table_name: None,
                index_name: None,
                filter_conditions: Vec::new(),
            });
        }

        operators
    }

    /// Estimate query execution cost
    fn estimate_query_cost(&self, operators: &[QueryOperator]) -> f64 {
        operators.iter().map(|op| op.estimated_cost).sum()
    }

    /// Identify useful indexes for query
    fn identify_useful_indexes(&self, query: &str) -> Vec<String> {
        let mut indexes = Vec::new();
        
        // Simplified index recommendation
        if query.contains("WHERE") {
            indexes.push("idx_filter".to_string());
        }
        if query.contains("ORDER BY") {
            indexes.push("idx_sort".to_string());
        }
        if query.contains("JOIN") {
            indexes.push("idx_join".to_string());
        }

        indexes
    }

    /// Apply query rewriting rules
    async fn rewrite_query(&self, query: &str) -> String {
        let rules = self.rewrite_rules.read().await;
        let mut rewritten = query.to_string();

        for rule in rules.iter() {
            rewritten = self.apply_rewrite_rule(&rewritten, rule);
        }

        rewritten
    }

    /// Apply a single rewrite rule
    fn apply_rewrite_rule(&self, query: &str, rule: &QueryRewriteRule) -> String {
        // Simplified rule application - in practice, this would use proper SQL parsing
        match rule.rule_type {
            RewriteRuleType::PredicatePushdown => {
                // Move WHERE conditions closer to table access
                self.apply_predicate_pushdown(query)
            },
            RewriteRuleType::JoinReordering => {
                // Reorder joins for optimal execution
                self.apply_join_reordering(query)
            },
            _ => query.to_string(), // Other rules not implemented in this example
        }
    }

    /// Apply predicate pushdown optimization
    fn apply_predicate_pushdown(&self, query: &str) -> String {
        // Simplified implementation
        // In practice, this would properly parse and restructure the SQL
        query.to_string()
    }

    /// Apply join reordering optimization
    fn apply_join_reordering(&self, query: &str) -> String {
        // Simplified implementation
        // In practice, this would analyze join selectivity and reorder accordingly
        query.to_string()
    }

    /// Cache query result
    async fn cache_query_result<T>(
        &self,
        query_hash: u64,
        result: &Result<T, Box<dyn std::error::Error + Send + Sync>>,
        execution_time: Duration,
    ) {
        if result.is_err() {
            return; // Don't cache failed results
        }

        let entry = QueryCacheEntry {
            query_hash,
            result_data: Vec::new(), // In practice, serialize the result
            result_metadata: ResultMetadata {
                row_count: 10, // Simplified
                column_count: 5,
                data_size_bytes: 1024,
                execution_time_ms: execution_time.as_millis() as u64,
            },
            created_at: Instant::now(),
            access_count: 0,
            last_accessed: Instant::now(),
            ttl: self.config.cache_ttl,
        };

        let mut cache = self.query_cache.write().await;
        
        // Evict old entries if cache is full
        while cache.len() >= self.config.max_cached_queries {
            if let Some(oldest_key) = self.find_oldest_cache_entry(&cache) {
                cache.remove(&oldest_key);
            } else {
                break;
            }
        }

        cache.insert(query_hash, entry);
    }

    /// Find oldest cache entry for eviction
    fn find_oldest_cache_entry(&self, cache: &HashMap<u64, QueryCacheEntry>) -> Option<u64> {
        cache.iter()
            .min_by_key(|(_, entry)| entry.last_accessed)
            .map(|(key, _)| *key)
    }

    /// Record query execution metrics
    async fn record_query_execution(
        &self,
        query_id: &str,
        query: &str,
        query_hash: u64,
        plan: &ExecutionPlan,
        execution_time: Duration,
        success: bool,
    ) {
        let execution_time_ms = execution_time.as_millis() as u64;
        
        // Update query metadata
        {
            let mut metadata = self.query_metadata.write().await;
            let entry = metadata.entry(query_id.to_string()).or_insert_with(|| {
                QueryMetadata {
                    query_id: query_id.to_string(),
                    query_text: query.to_string(),
                    query_hash,
                    execution_count: 0,
                    total_execution_time_ms: 0,
                    average_execution_time_ms: 0.0,
                    min_execution_time_ms: u64::MAX,
                    max_execution_time_ms: 0,
                    last_executed: Instant::now(),
                    error_count: 0,
                    result_cache_hits: 0,
                    plan_cache_hits: 0,
                    optimization_level: OptimizationLevel::Basic,
                    table_access_patterns: HashMap::new(),
                }
            });

            entry.execution_count += 1;
            entry.total_execution_time_ms += execution_time_ms;
            entry.average_execution_time_ms = 
                entry.total_execution_time_ms as f64 / entry.execution_count as f64;
            entry.min_execution_time_ms = entry.min_execution_time_ms.min(execution_time_ms);
            entry.max_execution_time_ms = entry.max_execution_time_ms.max(execution_time_ms);
            entry.last_executed = Instant::now();

            if !success {
                entry.error_count += 1;
            }
        }

        // Update execution plan performance
        {
            let mut plans = self.execution_plans.write().await;
            if let Some(cached_plan) = plans.get_mut(&query_hash) {
                cached_plan.actual_performance = Some(ActualPerformance {
                    actual_execution_time_ms: execution_time_ms,
                    actual_rows_processed: 100, // Simplified
                    cpu_time_ms: execution_time_ms / 2,
                    io_operations: 10,
                    memory_usage_mb: 5.0,
                });
            }
        }
    }

    /// Initialize default rewrite rules
    fn initialize_rewrite_rules(&mut self) {
        let rules = vec![
            QueryRewriteRule {
                rule_id: "predicate_pushdown".to_string(),
                rule_type: RewriteRuleType::PredicatePushdown,
                pattern: r"SELECT .* FROM .* WHERE .*".to_string(),
                replacement: "optimized".to_string(),
                conditions: Vec::new(),
                estimated_improvement: 1.5,
            },
            QueryRewriteRule {
                rule_id: "join_reordering".to_string(),
                rule_type: RewriteRuleType::JoinReordering,
                pattern: r".*JOIN.*JOIN.*".to_string(),
                replacement: "reordered".to_string(),
                conditions: Vec::new(),
                estimated_improvement: 2.0,
            },
        ];

        // In an async context, you'd use Arc<RwLock<>> and await, but for initialization:
        // This is a simplified approach for the constructor
    }

    /// Start statistics collection
    async fn start_statistics_collection(&self) {
        let optimizer = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(optimizer.config.stats_collection_interval);
            
            while optimizer.is_monitoring.load(Ordering::Relaxed) {
                interval.tick().await;
                optimizer.collect_statistics().await;
            }
        });
    }

    /// Start cache cleanup task
    async fn start_cache_cleanup(&self) {
        let optimizer = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300)); // Every 5 minutes
            
            while optimizer.is_monitoring.load(Ordering::Relaxed) {
                interval.tick().await;
                optimizer.cleanup_expired_cache().await;
            }
        });
    }

    /// Start adaptive optimization
    async fn start_adaptive_optimization(&self) {
        let optimizer = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(600)); // Every 10 minutes
            
            while optimizer.is_monitoring.load(Ordering::Relaxed) {
                interval.tick().await;
                optimizer.perform_adaptive_optimization().await;
            }
        });
    }

    /// Collect and update statistics
    async fn collect_statistics(&self) {
        let total_queries = self.total_queries.load(Ordering::Relaxed);
        let cache_hits = self.cache_hits.load(Ordering::Relaxed);
        let plan_cache_hits = self.plan_cache_hits.load(Ordering::Relaxed);
        let total_time = self.total_execution_time_ms.load(Ordering::Relaxed);

        let cache_hit_ratio = if total_queries > 0 {
            cache_hits as f64 / total_queries as f64
        } else {
            0.0
        };

        let plan_cache_hit_ratio = if total_queries > 0 {
            plan_cache_hits as f64 / total_queries as f64
        } else {
            0.0
        };

        let average_query_time = if total_queries > 0 {
            total_time as f64 / total_queries as f64
        } else {
            0.0
        };

        // Count slow queries and get top slow queries
        let metadata = self.query_metadata.read().await;
        let slow_queries = metadata.values()
            .filter(|m| m.average_execution_time_ms > self.config.slow_query_threshold_ms as f64)
            .count() as u64;

        let mut top_slow_queries: Vec<_> = metadata.values().cloned().collect();
        top_slow_queries.sort_by(|a, b| b.average_execution_time_ms.partial_cmp(&a.average_execution_time_ms).unwrap());
        top_slow_queries.truncate(10);

        // Update statistics
        let mut stats = self.query_statistics.write().await;
        stats.total_queries = total_queries;
        stats.cached_queries = cache_hits;
        stats.cache_hit_ratio = cache_hit_ratio;
        stats.plan_cache_hit_ratio = plan_cache_hit_ratio;
        stats.average_query_time_ms = average_query_time;
        stats.slow_queries = slow_queries;
        stats.top_slow_queries = top_slow_queries;
    }

    /// Cleanup expired cache entries
    async fn cleanup_expired_cache(&self) {
        let mut cache = self.query_cache.write().await;
        let now = Instant::now();
        
        cache.retain(|_, entry| {
            now.duration_since(entry.created_at) < entry.ttl
        });
        
        println!("Cache cleanup completed - {} entries remaining", cache.len());
    }

    /// Perform adaptive optimization
    async fn perform_adaptive_optimization(&self) {
        // Analyze slow queries and suggest optimizations
        let metadata = self.query_metadata.read().await;
        let mut optimization_count = 0;

        for query_meta in metadata.values() {
            if query_meta.average_execution_time_ms > self.config.slow_query_threshold_ms as f64 * 2.0 {
                // This query needs optimization
                self.suggest_query_optimization(query_meta).await;
                optimization_count += 1;
            }
        }

        if optimization_count > 0 {
            println!("Adaptive optimization completed - {} queries analyzed", optimization_count);
        }
    }

    /// Suggest optimization for a slow query
    async fn suggest_query_optimization(&self, _query_meta: &QueryMetadata) {
        // In a real implementation, this would analyze the query and suggest:
        // - Missing indexes
        // - Better query structure
        // - Partitioning strategies
        // - Caching recommendations
        
        println!("Optimization suggestion generated for slow query");
    }
}

impl Clone for QueryOptimizer {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            query_metadata: Arc::clone(&self.query_metadata),
            execution_plans: Arc::clone(&self.execution_plans),
            query_cache: Arc::clone(&self.query_cache),
            rewrite_rules: Arc::clone(&self.rewrite_rules),
            query_statistics: Arc::clone(&self.query_statistics),
            total_queries: AtomicU64::new(self.total_queries.load(Ordering::Relaxed)),
            cache_hits: AtomicU64::new(self.cache_hits.load(Ordering::Relaxed)),
            plan_cache_hits: AtomicU64::new(self.plan_cache_hits.load(Ordering::Relaxed)),
            total_execution_time_ms: AtomicU64::new(self.total_execution_time_ms.load(Ordering::Relaxed)),
            is_monitoring: std::sync::atomic::AtomicBool::new(self.is_monitoring.load(Ordering::Relaxed)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_query_optimizer_creation() {
        let config = QueryOptimizerConfig::default();
        let optimizer = QueryOptimizer::new(config);
        
        let stats = optimizer.get_statistics().await;
        assert_eq!(stats.total_queries, 0);
        assert_eq!(stats.cache_hit_ratio, 0.0);
    }

    #[tokio::test]
    async fn test_query_optimization() {
        let config = QueryOptimizerConfig::default();
        let optimizer = QueryOptimizer::new(config);
        
        let query = "SELECT * FROM users WHERE id = 1";
        let result = optimizer.optimize_and_execute(query, |optimized_query| async move {
            // Simulate query execution
            assert!(!optimized_query.is_empty());
            Ok("result".to_string())
        }).await;
        
        assert!(result.is_ok());
        
        let stats = optimizer.get_statistics().await;
        assert!(stats.total_queries > 0);
    }

    #[tokio::test]
    async fn test_query_caching() {
        let config = QueryOptimizerConfig {
            enable_query_cache: true,
            ..Default::default()
        };
        let optimizer = QueryOptimizer::new(config);
        
        let query = "SELECT * FROM users WHERE active = true";
        let query_hash = optimizer.hash_query(query);
        
        // First execution
        let _ = optimizer.optimize_and_execute(query, |_| async move {
            Ok("result".to_string())
        }).await;
        
        // Check that the query was processed
        let stats = optimizer.get_statistics().await;
        assert!(stats.total_queries > 0);
    }

    #[tokio::test]
    async fn test_rewrite_rules() {
        let config = QueryOptimizerConfig {
            enable_query_rewriting: true,
            ..Default::default()
        };
        let mut optimizer = QueryOptimizer::new(config);
        
        let rule = QueryRewriteRule {
            rule_id: "test_rule".to_string(),
            rule_type: RewriteRuleType::PredicatePushdown,
            pattern: "test_pattern".to_string(),
            replacement: "test_replacement".to_string(),
            conditions: Vec::new(),
            estimated_improvement: 1.5,
        };
        
        optimizer.add_rewrite_rule(rule).await;
        
        let query = "SELECT * FROM table WHERE condition";
        let rewritten = optimizer.rewrite_query(query).await;
        
        // In this simplified test, the query should remain the same
        // In a real implementation, it would be rewritten according to the rules
        assert!(!rewritten.is_empty());
    }

    #[tokio::test]
    async fn test_execution_plan_creation() {
        let config = QueryOptimizerConfig::default();
        let optimizer = QueryOptimizer::new(config);
        
        let query = "SELECT name FROM users WHERE age > 18 ORDER BY name";
        let query_hash = optimizer.hash_query(query);
        
        let plan = optimizer.create_execution_plan(query, query_hash).await.unwrap();
        
        assert!(!plan.operators.is_empty());
        assert!(plan.estimated_cost > 0.0);
    }

    #[tokio::test]
    async fn test_statistics_collection() {
        let config = QueryOptimizerConfig::default();
        let optimizer = QueryOptimizer::new(config);
        
        // Execute a few queries
        for i in 0..5 {
            let query = format!("SELECT * FROM table WHERE id = {}", i);
            let _ = optimizer.optimize_and_execute(&query, |_| async move {
                Ok(format!("result_{}", i))
            }).await;
        }
        
        let stats = optimizer.get_statistics().await;
        assert_eq!(stats.total_queries, 5);
    }
}