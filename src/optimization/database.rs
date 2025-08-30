use lru::LruCache;
use parking_lot::{Mutex, RwLock};
use rustc_hash::FxHashMap;
use std::collections::VecDeque;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;

use crate::error::BitCrapsError;
use crate::protocol::TransactionId;

/// Database query optimization and caching layer
pub struct DatabaseOptimizer {
    /// Query result cache with automatic expiration
    query_cache: Arc<RwLock<LruCache<QueryKey, CachedResult>>>,
    /// Query performance statistics
    query_stats: Arc<RwLock<FxHashMap<String, QueryStats>>>,
    /// Connection pool optimization
    connection_pool: Arc<ConnectionPoolOptimizer>,
    /// Prepared statement cache
    prepared_statements: Arc<RwLock<FxHashMap<String, PreparedStatementInfo>>>,
    /// Transaction batching
    transaction_batcher: Arc<Mutex<TransactionBatcher>>,
    /// Configuration
    config: DatabaseOptimizerConfig,
}

#[derive(Debug, Clone)]
pub struct DatabaseOptimizerConfig {
    /// Maximum entries in query cache
    pub cache_size: usize,
    /// Cache TTL for different query types
    pub cache_ttl_read: Duration,
    pub cache_ttl_write: Duration,
    /// Batch transaction settings
    pub batch_size: usize,
    pub batch_timeout: Duration,
    /// Connection pool settings
    pub max_connections: usize,
    pub connection_timeout: Duration,
    /// Query optimization settings
    pub slow_query_threshold: Duration,
    pub enable_query_logging: bool,
}

impl Default for DatabaseOptimizerConfig {
    fn default() -> Self {
        Self {
            cache_size: 1000,
            cache_ttl_read: Duration::from_secs(300), // 5 minutes
            cache_ttl_write: Duration::from_secs(60), // 1 minute
            batch_size: 100,
            batch_timeout: Duration::from_millis(100),
            max_connections: 20,
            connection_timeout: Duration::from_secs(30),
            slow_query_threshold: Duration::from_millis(1000),
            enable_query_logging: true,
        }
    }
}

impl DatabaseOptimizer {
    pub fn new(config: DatabaseOptimizerConfig) -> Self {
        let cache_size =
            NonZeroUsize::new(config.cache_size).unwrap_or(NonZeroUsize::new(1000).unwrap());

        Self {
            query_cache: Arc::new(RwLock::new(LruCache::new(cache_size))),
            query_stats: Arc::new(RwLock::new(FxHashMap::default())),
            connection_pool: Arc::new(ConnectionPoolOptimizer::new(&config)),
            prepared_statements: Arc::new(RwLock::new(FxHashMap::default())),
            transaction_batcher: Arc::new(Mutex::new(TransactionBatcher::new(&config))),
            config,
        }
    }

    /// Execute optimized query with caching
    pub async fn execute_query<T>(&self, query: &DatabaseQuery) -> Result<T, BitCrapsError>
    where
        T: Clone + serde::Serialize + for<'de> serde::Deserialize<'de> + Send + 'static + Default,
    {
        let query_key = QueryKey::from_query(query);
        let start_time = Instant::now();

        // Try cache first for read queries
        if query.is_read_only() {
            if let Some(cached) = self.get_cached_result::<T>(&query_key) {
                self.update_query_stats(&query.query_type, start_time, true);
                return Ok(cached);
            }
        }

        // Execute query with connection pool optimization
        let connection = self.connection_pool.acquire_connection().await?;
        let result = self.execute_raw_query::<T>(connection, query).await?;

        let duration = start_time.elapsed();
        self.update_query_stats(&query.query_type, start_time, false);

        // Cache result if it's a read query
        if query.is_read_only() {
            self.cache_result(&query_key, &result, self.config.cache_ttl_read);
        }

        // Log slow queries
        if duration > self.config.slow_query_threshold && self.config.enable_query_logging {
            tracing::warn!(
                "Slow query detected: {} took {:?}ms",
                query.query_type,
                duration.as_millis()
            );
        }

        Ok(result)
    }

    /// Execute query with prepared statement optimization
    pub async fn execute_prepared_query<T>(&self, query: &PreparedQuery) -> Result<T, BitCrapsError>
    where
        T: Clone + serde::Serialize + for<'de> serde::Deserialize<'de> + Send + 'static + Default,
    {
        let start_time = Instant::now();

        // Check if we have a prepared statement for this query
        let statement_info = {
            let mut statements = self.prepared_statements.write();
            statements
                .entry(query.statement_id.clone())
                .or_insert_with(|| PreparedStatementInfo::new(&query.sql))
                .clone()
        };

        // Execute using prepared statement
        let connection = self.connection_pool.acquire_connection().await?;
        let result = self
            .execute_prepared_raw::<T>(connection, query, &statement_info)
            .await?;

        // Update statement usage statistics
        {
            let mut statements = self.prepared_statements.write();
            if let Some(info) = statements.get_mut(&query.statement_id) {
                info.execution_count += 1;
                info.total_execution_time += start_time.elapsed();
            }
        }

        Ok(result)
    }

    /// Batch multiple transactions for better performance
    pub async fn batch_transaction(
        &self,
        transaction: DatabaseTransaction,
    ) -> Result<TransactionId, BitCrapsError> {
        let mut batcher = self.transaction_batcher.lock();

        let transaction_id = batcher.add_transaction(transaction);

        // Check if batch should be executed
        if batcher.should_execute_batch() {
            let batch = batcher.take_batch();
            drop(batcher); // Release lock before async operation

            // Execute batch in background
            let optimizer = Arc::new(self.clone());
            tokio::spawn(async move {
                if let Err(e) = optimizer.execute_batch(batch).await {
                    tracing::error!("Failed to execute transaction batch: {:?}", e);
                }
            });
        }

        Ok(transaction_id)
    }

    /// Execute a batch of transactions atomically
    async fn execute_batch(&self, batch: TransactionBatch) -> Result<(), BitCrapsError> {
        let connection = self.connection_pool.acquire_connection().await?;
        let start_time = Instant::now();

        // Begin transaction
        self.begin_transaction(connection.clone()).await?;

        let mut results = Vec::with_capacity(batch.transactions.len());
        let mut success = true;

        // Execute all transactions in the batch
        for transaction in &batch.transactions {
            match self
                .execute_transaction_raw(connection.clone(), transaction)
                .await
            {
                Ok(result) => results.push(result),
                Err(e) => {
                    tracing::error!("Transaction failed in batch: {:?}", e);
                    success = false;
                    break;
                }
            }
        }

        // Commit or rollback based on success
        if success {
            self.commit_transaction(connection).await?;
            tracing::debug!(
                "Successfully executed batch of {} transactions in {:?}",
                batch.transactions.len(),
                start_time.elapsed()
            );
        } else {
            self.rollback_transaction(connection).await?;
            return Err(BitCrapsError::Database(
                "Batch transaction failed".to_string(),
            ));
        }

        Ok(())
    }

    /// Optimize database schema based on query patterns
    pub async fn optimize_schema(&self) -> Result<Vec<OptimizationSuggestion>, BitCrapsError> {
        let query_stats = self.query_stats.read();
        let mut suggestions = Vec::new();

        // Analyze query patterns
        for (query_type, stats) in query_stats.iter() {
            // Suggest indexes for slow, frequent queries
            if stats.average_duration() > self.config.slow_query_threshold
                && stats.execution_count > 100
            {
                suggestions.push(OptimizationSuggestion::AddIndex {
                    table: self.extract_table_name(query_type),
                    columns: self.extract_frequent_columns(query_type, stats),
                    reason: format!(
                        "Query type {} is slow ({:?}) and frequent ({})",
                        query_type,
                        stats.average_duration(),
                        stats.execution_count
                    ),
                });
            }

            // Suggest partitioning for very large tables
            if stats.execution_count > 10000 && stats.cache_hit_rate() < 0.3 {
                suggestions.push(OptimizationSuggestion::PartitionTable {
                    table: self.extract_table_name(query_type),
                    strategy: PartitionStrategy::ByDate,
                    reason: "High query volume with low cache hit rate suggests large table"
                        .to_string(),
                });
            }
        }

        // Analyze prepared statement usage
        let prepared_stats = self.prepared_statements.read();
        for (statement_id, info) in prepared_stats.iter() {
            if info.execution_count > 1000
                && info.average_execution_time() > Duration::from_millis(500)
            {
                suggestions.push(OptimizationSuggestion::OptimizeQuery {
                    query: statement_id.clone(),
                    current_performance: info.average_execution_time(),
                    reason: "Frequently executed prepared statement with poor performance"
                        .to_string(),
                });
            }
        }

        Ok(suggestions)
    }

    /// Get comprehensive database performance statistics
    pub fn get_performance_stats(&self) -> DatabasePerformanceStats {
        let query_stats = self.query_stats.read();
        let connection_stats = self.connection_pool.get_stats();
        let prepared_stats = self.prepared_statements.read();

        let total_queries: u64 = query_stats.values().map(|s| s.execution_count).sum();
        let total_cache_hits: u64 = query_stats.values().map(|s| s.cache_hits).sum();
        let cache_hit_rate = if total_queries > 0 {
            total_cache_hits as f64 / total_queries as f64
        } else {
            0.0
        };

        let slow_queries = query_stats
            .iter()
            .filter(|(_, stats)| stats.average_duration() > self.config.slow_query_threshold)
            .count();

        DatabasePerformanceStats {
            total_queries,
            cache_hit_rate,
            slow_queries,
            average_query_duration: self.calculate_average_query_duration(&query_stats),
            connection_pool_stats: connection_stats,
            prepared_statement_count: prepared_stats.len(),
            most_frequent_queries: self.get_top_queries(&query_stats, 10),
        }
    }

    /// Clear query cache (useful for testing or manual optimization)
    pub fn clear_cache(&self) {
        let mut cache = self.query_cache.write();
        cache.clear();
    }

    /// Force execute all pending batched transactions
    pub async fn flush_pending_transactions(&self) -> Result<(), BitCrapsError> {
        let batch = {
            let mut batcher = self.transaction_batcher.lock();
            batcher.take_batch()
        };

        if !batch.transactions.is_empty() {
            self.execute_batch(batch).await?;
        }

        Ok(())
    }

    // Private helper methods

    fn get_cached_result<T>(&self, key: &QueryKey) -> Option<T>
    where
        T: Clone + for<'de> serde::Deserialize<'de>,
    {
        let mut cache = self.query_cache.write();

        if let Some(cached) = cache.get(key) {
            if !cached.is_expired() {
                if let Ok(result) = serde_json::from_str(&cached.data) {
                    return Some(result);
                }
            } else {
                // Remove expired entry
                cache.pop(key);
            }
        }

        None
    }

    fn cache_result<T>(&self, key: &QueryKey, result: &T, ttl: Duration)
    where
        T: serde::Serialize,
    {
        if let Ok(serialized) = serde_json::to_string(result) {
            let cached_result = CachedResult {
                data: serialized,
                created_at: Instant::now(),
                ttl,
            };

            let mut cache = self.query_cache.write();
            cache.put(key.clone(), cached_result);
        }
    }

    fn update_query_stats(&self, query_type: &str, start_time: Instant, from_cache: bool) {
        let duration = start_time.elapsed();
        let mut stats = self.query_stats.write();

        let query_stats = stats
            .entry(query_type.to_string())
            .or_insert_with(QueryStats::new);

        query_stats.execution_count += 1;
        query_stats.total_duration += duration;

        if from_cache {
            query_stats.cache_hits += 1;
        }
    }

    fn calculate_average_query_duration(&self, stats: &FxHashMap<String, QueryStats>) -> Duration {
        let (total_duration, total_count) = stats.values().fold(
            (Duration::from_nanos(0), 0u64),
            |(dur_acc, count_acc), stats| {
                (
                    dur_acc + stats.total_duration,
                    count_acc + stats.execution_count,
                )
            },
        );

        if total_count > 0 {
            total_duration / total_count as u32
        } else {
            Duration::from_nanos(0)
        }
    }

    fn get_top_queries(
        &self,
        stats: &FxHashMap<String, QueryStats>,
        limit: usize,
    ) -> Vec<(String, u64)> {
        let mut queries: Vec<_> = stats
            .iter()
            .map(|(query, stats)| (query.clone(), stats.execution_count))
            .collect();

        queries.sort_by(|a, b| b.1.cmp(&a.1));
        queries.truncate(limit);
        queries
    }

    fn extract_table_name(&self, query_type: &str) -> String {
        // Simplified table extraction - in practice, this would parse the SQL
        if query_type.contains("game_states") {
            "game_states".to_string()
        } else if query_type.contains("players") {
            "players".to_string()
        } else if query_type.contains("transactions") {
            "transactions".to_string()
        } else {
            "unknown".to_string()
        }
    }

    fn extract_frequent_columns(&self, _query_type: &str, _stats: &QueryStats) -> Vec<String> {
        // Simplified column extraction - in practice, this would analyze query patterns
        vec!["id".to_string(), "created_at".to_string()]
    }

    // Placeholder methods for actual database operations
    async fn execute_raw_query<T>(
        &self,
        _connection: DatabaseConnection,
        _query: &DatabaseQuery,
    ) -> Result<T, BitCrapsError>
    where
        T: Clone + for<'de> serde::Deserialize<'de> + Default,
    {
        // Placeholder implementation
        Ok(T::default())
    }

    async fn execute_prepared_raw<T>(
        &self,
        _connection: DatabaseConnection,
        _query: &PreparedQuery,
        _info: &PreparedStatementInfo,
    ) -> Result<T, BitCrapsError>
    where
        T: Clone + for<'de> serde::Deserialize<'de> + Default,
    {
        // Placeholder implementation
        Ok(T::default())
    }

    async fn execute_transaction_raw(
        &self,
        _connection: DatabaseConnection,
        _transaction: &DatabaseTransaction,
    ) -> Result<String, BitCrapsError> {
        // Placeholder implementation
        Ok("success".to_string())
    }

    async fn begin_transaction(
        &self,
        _connection: DatabaseConnection,
    ) -> Result<(), BitCrapsError> {
        Ok(())
    }

    async fn commit_transaction(
        &self,
        _connection: DatabaseConnection,
    ) -> Result<(), BitCrapsError> {
        Ok(())
    }

    async fn rollback_transaction(
        &self,
        _connection: DatabaseConnection,
    ) -> Result<(), BitCrapsError> {
        Ok(())
    }
}

impl Clone for DatabaseOptimizer {
    fn clone(&self) -> Self {
        Self {
            query_cache: Arc::clone(&self.query_cache),
            query_stats: Arc::clone(&self.query_stats),
            connection_pool: Arc::clone(&self.connection_pool),
            prepared_statements: Arc::clone(&self.prepared_statements),
            transaction_batcher: Arc::clone(&self.transaction_batcher),
            config: self.config.clone(),
        }
    }
}

/// Connection pool optimization
pub struct ConnectionPoolOptimizer {
    available_connections: Arc<Semaphore>,
    connection_stats: Arc<RwLock<ConnectionPoolStats>>,
    config: DatabaseOptimizerConfig,
}

impl ConnectionPoolOptimizer {
    pub fn new(config: &DatabaseOptimizerConfig) -> Self {
        Self {
            available_connections: Arc::new(Semaphore::new(config.max_connections)),
            connection_stats: Arc::new(RwLock::new(ConnectionPoolStats::new())),
            config: config.clone(),
        }
    }

    pub async fn acquire_connection(&self) -> Result<DatabaseConnection, BitCrapsError> {
        let start_time = Instant::now();

        // Acquire permit from semaphore
        let _permit = self
            .available_connections
            .acquire()
            .await
            .map_err(|_| BitCrapsError::Database("Failed to acquire connection".to_string()))?;

        let wait_time = start_time.elapsed();

        // Update statistics
        {
            let mut stats = self.connection_stats.write();
            stats.total_acquisitions += 1;
            stats.total_wait_time += wait_time;
            stats.active_connections += 1;
        }

        Ok(DatabaseConnection {
            id: uuid::Uuid::new_v4(),
            acquired_at: Instant::now(),
        })
    }

    pub fn get_stats(&self) -> ConnectionPoolStats {
        self.connection_stats.read().clone()
    }
}

/// Transaction batching for improved performance
pub struct TransactionBatcher {
    pending_transactions: VecDeque<DatabaseTransaction>,
    batch_start_time: Option<Instant>,
    next_transaction_id: u64,
    config: DatabaseOptimizerConfig,
}

impl TransactionBatcher {
    pub fn new(config: &DatabaseOptimizerConfig) -> Self {
        Self {
            pending_transactions: VecDeque::new(),
            batch_start_time: None,
            next_transaction_id: 1,
            config: config.clone(),
        }
    }

    pub fn add_transaction(&mut self, transaction: DatabaseTransaction) -> TransactionId {
        if self.pending_transactions.is_empty() {
            self.batch_start_time = Some(Instant::now());
        }

        let mut transaction_id = [0u8; 32];
        transaction_id[..8].copy_from_slice(&self.next_transaction_id.to_le_bytes());
        self.next_transaction_id += 1;

        self.pending_transactions.push_back(transaction);

        transaction_id
    }

    pub fn should_execute_batch(&self) -> bool {
        if self.pending_transactions.is_empty() {
            return false;
        }

        // Execute if batch is full
        if self.pending_transactions.len() >= self.config.batch_size {
            return true;
        }

        // Execute if batch timeout reached
        if let Some(start_time) = self.batch_start_time {
            if start_time.elapsed() >= self.config.batch_timeout {
                return true;
            }
        }

        false
    }

    pub fn take_batch(&mut self) -> TransactionBatch {
        let transactions = self.pending_transactions.drain(..).collect();
        self.batch_start_time = None;

        TransactionBatch {
            transactions,
            created_at: Instant::now(),
        }
    }
}

// Supporting types and structures

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct QueryKey {
    pub query_hash: u64,
    pub parameters_hash: u64,
}

impl QueryKey {
    pub fn from_query(query: &DatabaseQuery) -> Self {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        query.sql.hash(&mut hasher);
        let query_hash = hasher.finish();

        let mut param_hasher = DefaultHasher::new();
        for param in &query.parameters {
            param.hash(&mut param_hasher);
        }
        let parameters_hash = param_hasher.finish();

        Self {
            query_hash,
            parameters_hash,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CachedResult {
    pub data: String,
    pub created_at: Instant,
    pub ttl: Duration,
}

impl CachedResult {
    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed() > self.ttl
    }
}

#[derive(Debug, Clone)]
pub struct QueryStats {
    pub execution_count: u64,
    pub cache_hits: u64,
    pub total_duration: Duration,
    pub last_executed: Option<Instant>,
}

impl QueryStats {
    pub fn new() -> Self {
        Self {
            execution_count: 0,
            cache_hits: 0,
            total_duration: Duration::from_nanos(0),
            last_executed: None,
        }
    }

    pub fn cache_hit_rate(&self) -> f64 {
        if self.execution_count > 0 {
            self.cache_hits as f64 / self.execution_count as f64
        } else {
            0.0
        }
    }

    pub fn average_duration(&self) -> Duration {
        if self.execution_count > 0 {
            self.total_duration / self.execution_count as u32
        } else {
            Duration::from_nanos(0)
        }
    }
}

#[derive(Debug, Clone)]
pub struct PreparedStatementInfo {
    pub sql: String,
    pub created_at: Instant,
    pub execution_count: u64,
    pub total_execution_time: Duration,
}

impl PreparedStatementInfo {
    pub fn new(sql: &str) -> Self {
        Self {
            sql: sql.to_string(),
            created_at: Instant::now(),
            execution_count: 0,
            total_execution_time: Duration::from_nanos(0),
        }
    }

    pub fn average_execution_time(&self) -> Duration {
        if self.execution_count > 0 {
            self.total_execution_time / self.execution_count as u32
        } else {
            Duration::from_nanos(0)
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConnectionPoolStats {
    pub total_acquisitions: u64,
    pub total_wait_time: Duration,
    pub active_connections: usize,
    pub peak_connections: usize,
}

impl ConnectionPoolStats {
    pub fn new() -> Self {
        Self {
            total_acquisitions: 0,
            total_wait_time: Duration::from_nanos(0),
            active_connections: 0,
            peak_connections: 0,
        }
    }

    pub fn average_wait_time(&self) -> Duration {
        if self.total_acquisitions > 0 {
            self.total_wait_time / self.total_acquisitions as u32
        } else {
            Duration::from_nanos(0)
        }
    }
}

#[derive(Debug, Clone)]
pub struct DatabaseConnection {
    pub id: uuid::Uuid,
    pub acquired_at: Instant,
}

#[derive(Debug, Clone)]
pub struct DatabaseQuery {
    pub sql: String,
    pub parameters: Vec<String>,
    pub query_type: String,
}

impl DatabaseQuery {
    pub fn is_read_only(&self) -> bool {
        let sql_lower = self.sql.to_lowercase();
        sql_lower.trim_start().starts_with("select")
            || sql_lower.trim_start().starts_with("with")
            || sql_lower.contains("explain")
    }
}

#[derive(Debug, Clone)]
pub struct PreparedQuery {
    pub statement_id: String,
    pub sql: String,
    pub parameters: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct DatabaseTransaction {
    pub queries: Vec<DatabaseQuery>,
    pub transaction_type: TransactionType,
    pub priority: TransactionPriority,
}

#[derive(Debug, Clone, Copy)]
pub enum TransactionType {
    Read,
    Write,
    ReadWrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TransactionPriority {
    Low = 1,
    Normal = 2,
    High = 3,
    Critical = 4,
}

#[derive(Debug)]
pub struct TransactionBatch {
    pub transactions: Vec<DatabaseTransaction>,
    pub created_at: Instant,
}

#[derive(Debug)]
pub enum OptimizationSuggestion {
    AddIndex {
        table: String,
        columns: Vec<String>,
        reason: String,
    },
    PartitionTable {
        table: String,
        strategy: PartitionStrategy,
        reason: String,
    },
    OptimizeQuery {
        query: String,
        current_performance: Duration,
        reason: String,
    },
}

#[derive(Debug)]
pub enum PartitionStrategy {
    ByDate,
    ByHash,
    ByRange,
}

#[derive(Debug)]
pub struct DatabasePerformanceStats {
    pub total_queries: u64,
    pub cache_hit_rate: f64,
    pub slow_queries: usize,
    pub average_query_duration: Duration,
    pub connection_pool_stats: ConnectionPoolStats,
    pub prepared_statement_count: usize,
    pub most_frequent_queries: Vec<(String, u64)>,
}
