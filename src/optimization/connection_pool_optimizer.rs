#![cfg(feature = "optimization")]

//! Connection Pool Optimizer for BitCraps
//!
//! Provides intelligent connection pool management with adaptive sizing,
//! health monitoring, and performance optimization.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock, Semaphore};
use tokio::time::timeout;
use uuid::Uuid;

/// Connection pool configuration
#[derive(Clone, Debug)]
pub struct ConnectionPoolConfig {
    /// Minimum pool size
    pub min_connections: usize,
    /// Maximum pool size
    pub max_connections: usize,
    /// Connection idle timeout
    pub idle_timeout: Duration,
    /// Connection validation timeout
    pub validation_timeout: Duration,
    /// Health check interval
    pub health_check_interval: Duration,
    /// Pool expansion rate (connections per second)
    pub expansion_rate: f64,
    /// Pool contraction rate (connections per second)
    pub contraction_rate: f64,
    /// Enable adaptive sizing
    pub enable_adaptive_sizing: bool,
    /// Connection retry attempts
    pub max_retry_attempts: usize,
    /// Load balancing strategy
    pub load_balancing: LoadBalancingStrategy,
}

impl Default for ConnectionPoolConfig {
    fn default() -> Self {
        Self {
            min_connections: 5,
            max_connections: 100,
            idle_timeout: Duration::from_secs(300), // 5 minutes
            validation_timeout: Duration::from_secs(5),
            health_check_interval: Duration::from_secs(30),
            expansion_rate: 2.0, // 2 connections per second
            contraction_rate: 1.0, // 1 connection per second
            enable_adaptive_sizing: true,
            max_retry_attempts: 3,
            load_balancing: LoadBalancingStrategy::RoundRobin,
        }
    }
}

/// Load balancing strategies
#[derive(Debug, Clone)]
pub enum LoadBalancingStrategy {
    RoundRobin,
    LeastConnections,
    WeightedRandom,
    ResponseTime,
    HealthBased,
}

/// Connection wrapper with metadata
#[derive(Debug)]
pub struct PooledConnection<T> {
    pub connection: T,
    pub id: Uuid,
    pub created_at: Instant,
    pub last_used: Instant,
    pub use_count: u64,
    pub is_healthy: AtomicBool,
    pub response_time_ms: Arc<RwLock<f64>>,
    pub error_count: AtomicU64,
}

impl<T> PooledConnection<T> {
    pub fn new(connection: T) -> Self {
        let now = Instant::now();
        Self {
            connection,
            id: Uuid::new_v4(),
            created_at: now,
            last_used: now,
            use_count: 0,
            is_healthy: AtomicBool::new(true),
            response_time_ms: Arc::new(RwLock::new(0.0)),
            error_count: AtomicU64::new(0),
        }
    }

    pub fn mark_used(&mut self) {
        self.last_used = Instant::now();
        self.use_count += 1;
    }

    pub fn is_idle(&self, idle_timeout: Duration) -> bool {
        self.last_used.elapsed() > idle_timeout
    }

    pub fn is_healthy(&self) -> bool {
        self.is_healthy.load(Ordering::Relaxed)
    }

    pub fn set_healthy(&self, healthy: bool) {
        self.is_healthy.store(healthy, Ordering::Relaxed);
    }

    pub async fn update_response_time(&self, response_time_ms: f64) {
        let mut rt = self.response_time_ms.write().await;
        *rt = (*rt * 0.9) + (response_time_ms * 0.1); // Exponential moving average
    }

    pub async fn get_response_time(&self) -> f64 {
        let rt = self.response_time_ms.read().await;
        *rt
    }

    pub fn increment_error_count(&self) {
        self.error_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn get_error_count(&self) -> u64 {
        self.error_count.load(Ordering::Relaxed)
    }
}

/// Pool statistics
#[derive(Debug, Clone)]
pub struct PoolStatistics {
    pub total_connections: usize,
    pub active_connections: usize,
    pub idle_connections: usize,
    pub healthy_connections: usize,
    pub unhealthy_connections: usize,
    pub connections_created: u64,
    pub connections_destroyed: u64,
    pub connection_requests: u64,
    pub connection_wait_time_ms: f64,
    pub average_response_time_ms: f64,
    pub pool_utilization_percent: f64,
    pub error_rate_percent: f64,
}

/// Adaptive connection pool
pub struct AdaptiveConnectionPool<T, F, Fut>
where
    T: Send + Sync + 'static,
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<T, Box<dyn std::error::Error + Send + Sync>>> + Send + 'static,
{
    config: ConnectionPoolConfig,
    connections: Arc<RwLock<Vec<Arc<Mutex<PooledConnection<T>>>>>>,
    available_connections: Arc<Mutex<VecDeque<Arc<Mutex<PooledConnection<T>>>>>>,
    active_connections: Arc<Mutex<HashMap<Uuid, Arc<Mutex<PooledConnection<T>>>>>>,
    connection_factory: Arc<F>,
    semaphore: Arc<Semaphore>,
    
    // Statistics
    connections_created: AtomicU64,
    connections_destroyed: AtomicU64,
    connection_requests: AtomicU64,
    total_wait_time_ms: AtomicU64,
    
    // Adaptive sizing state
    last_expansion: Arc<RwLock<Instant>>,
    last_contraction: Arc<RwLock<Instant>>,
    demand_history: Arc<RwLock<VecDeque<(Instant, f64)>>>,
    
    // Health monitoring
    is_monitoring: AtomicBool,
    next_connection_index: AtomicUsize, // For round-robin
}

impl<T, F, Fut> AdaptiveConnectionPool<T, F, Fut>
where
    T: Send + Sync + 'static,
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<T, Box<dyn std::error::Error + Send + Sync>>> + Send + 'static,
{
    pub async fn new(config: ConnectionPoolConfig, connection_factory: F) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let pool = Self {
            semaphore: Arc::new(Semaphore::new(config.max_connections)),
            connections: Arc::new(RwLock::new(Vec::new())),
            available_connections: Arc::new(Mutex::new(VecDeque::new())),
            active_connections: Arc::new(Mutex::new(HashMap::new())),
            connection_factory: Arc::new(connection_factory),
            connections_created: AtomicU64::new(0),
            connections_destroyed: AtomicU64::new(0),
            connection_requests: AtomicU64::new(0),
            total_wait_time_ms: AtomicU64::new(0),
            last_expansion: Arc::new(RwLock::new(Instant::now())),
            last_contraction: Arc::new(RwLock::new(Instant::now())),
            demand_history: Arc::new(RwLock::new(VecDeque::new())),
            is_monitoring: AtomicBool::new(false),
            next_connection_index: AtomicUsize::new(0),
            config,
        };

        // Initialize with minimum connections
        pool.initialize_pool().await?;
        
        // Start monitoring if enabled
        if pool.config.enable_adaptive_sizing {
            pool.start_monitoring().await;
        }

        Ok(pool)
    }

    /// Get a connection from the pool
    pub async fn get_connection(&self) -> Result<ConnectionHandle<T>, Box<dyn std::error::Error + Send + Sync>> {
        let start_time = Instant::now();
        self.connection_requests.fetch_add(1, Ordering::Relaxed);

        // Try to get an available connection first
        if let Some(conn) = self.try_get_available_connection().await {
            let wait_time = start_time.elapsed().as_millis() as u64;
            self.total_wait_time_ms.fetch_add(wait_time, Ordering::Relaxed);
            return Ok(ConnectionHandle::new(conn, Arc::new(self.clone())));
        }

        // If no available connections, try to create a new one
        let permit = timeout(
            Duration::from_secs(10),
            self.semaphore.clone().acquire_owned()
        ).await
        .map_err(|_| "Connection pool acquisition timeout")?
        .map_err(|e| format!("Failed to acquire connection permit: {}", e))?;

        let connection = self.create_connection().await?;
        let pooled_conn = Arc::new(Mutex::new(PooledConnection::new(connection)));

        // Add to active connections
        {
            let mut active = self.active_connections.lock().await;
            let conn_guard = pooled_conn.lock().await;
            active.insert(conn_guard.id, Arc::clone(&pooled_conn));
        }

        let wait_time = start_time.elapsed().as_millis() as u64;
        self.total_wait_time_ms.fetch_add(wait_time, Ordering::Relaxed);

        Ok(ConnectionHandle::new_with_permit(pooled_conn, Arc::new(self.clone()), permit))
    }

    /// Get pool statistics
    pub async fn get_statistics(&self) -> PoolStatistics {
        let connections = self.connections.read().await;
        let active = self.active_connections.lock().await;
        let available = self.available_connections.lock().await;

        let total_connections = connections.len();
        let active_connections = active.len();
        let idle_connections = available.len();

        let mut healthy_connections = 0;
        let mut total_response_time = 0.0;
        let mut total_errors = 0u64;

        for conn_arc in connections.iter() {
            let conn = conn_arc.lock().await;
            if conn.is_healthy() {
                healthy_connections += 1;
            }
            total_response_time += conn.get_response_time().await;
            total_errors += conn.get_error_count();
        }

        let unhealthy_connections = total_connections - healthy_connections;
        let average_response_time = if total_connections > 0 {
            total_response_time / total_connections as f64
        } else {
            0.0
        };

        let pool_utilization = if self.config.max_connections > 0 {
            (active_connections as f64 / self.config.max_connections as f64) * 100.0
        } else {
            0.0
        };

        let total_requests = self.connection_requests.load(Ordering::Relaxed);
        let error_rate = if total_requests > 0 {
            (total_errors as f64 / total_requests as f64) * 100.0
        } else {
            0.0
        };

        let total_wait_time = self.total_wait_time_ms.load(Ordering::Relaxed);
        let average_wait_time = if total_requests > 0 {
            total_wait_time as f64 / total_requests as f64
        } else {
            0.0
        };

        PoolStatistics {
            total_connections,
            active_connections,
            idle_connections,
            healthy_connections,
            unhealthy_connections,
            connections_created: self.connections_created.load(Ordering::Relaxed),
            connections_destroyed: self.connections_destroyed.load(Ordering::Relaxed),
            connection_requests: total_requests,
            connection_wait_time_ms: average_wait_time,
            average_response_time_ms: average_response_time,
            pool_utilization_percent: pool_utilization,
            error_rate_percent: error_rate,
        }
    }

    /// Return connection to pool
    pub(crate) async fn return_connection(&self, conn_arc: Arc<Mutex<PooledConnection<T>>>) {
        // Remove from active connections
        {
            let mut active = self.active_connections.lock().await;
            let conn = conn_arc.lock().await;
            active.remove(&conn.id);
        }

        // Check if connection is still healthy
        let is_healthy = {
            let conn = conn_arc.lock().await;
            conn.is_healthy() && !conn.is_idle(self.config.idle_timeout)
        };

        if is_healthy {
            // Return to available pool
            let mut available = self.available_connections.lock().await;
            available.push_back(conn_arc);
        } else {
            // Connection is unhealthy or expired, destroy it
            self.destroy_connection(conn_arc).await;
        }
    }

    /// Try to get an available connection using load balancing strategy
    async fn try_get_available_connection(&self) -> Option<Arc<Mutex<PooledConnection<T>>>> {
        match self.config.load_balancing {
            LoadBalancingStrategy::RoundRobin => self.get_round_robin_connection().await,
            LoadBalancingStrategy::LeastConnections => self.get_least_connections_connection().await,
            LoadBalancingStrategy::ResponseTime => self.get_fastest_connection().await,
            LoadBalancingStrategy::HealthBased => self.get_healthiest_connection().await,
            LoadBalancingStrategy::WeightedRandom => self.get_weighted_random_connection().await,
        }
    }

    /// Round-robin connection selection
    async fn get_round_robin_connection(&self) -> Option<Arc<Mutex<PooledConnection<T>>>> {
        let mut available = self.available_connections.lock().await;
        if !available.is_empty() {
            // Simple round-robin for available connections
            available.pop_front()
        } else {
            None
        }
    }

    /// Least connections selection
    async fn get_least_connections_connection(&self) -> Option<Arc<Mutex<PooledConnection<T>>>> {
        let mut available = self.available_connections.lock().await;
        if available.is_empty() {
            return None;
        }

        // Find connection with least usage
        let mut best_index = 0;
        let mut min_use_count = u64::MAX;

        for (i, conn_arc) in available.iter().enumerate() {
            let conn = conn_arc.lock().await;
            if conn.use_count < min_use_count {
                min_use_count = conn.use_count;
                best_index = i;
            }
        }

        available.remove(best_index)
    }

    /// Fastest response time selection
    async fn get_fastest_connection(&self) -> Option<Arc<Mutex<PooledConnection<T>>>> {
        let mut available = self.available_connections.lock().await;
        if available.is_empty() {
            return None;
        }

        // Find connection with best response time
        let mut best_index = 0;
        let mut best_response_time = f64::MAX;

        for (i, conn_arc) in available.iter().enumerate() {
            let conn = conn_arc.lock().await;
            let response_time = conn.get_response_time().await;
            if response_time < best_response_time {
                best_response_time = response_time;
                best_index = i;
            }
        }

        available.remove(best_index)
    }

    /// Healthiest connection selection
    async fn get_healthiest_connection(&self) -> Option<Arc<Mutex<PooledConnection<T>>>> {
        let mut available = self.available_connections.lock().await;
        if available.is_empty() {
            return None;
        }

        // Find healthiest connection (lowest error count)
        let mut best_index = 0;
        let mut min_error_count = u64::MAX;

        for (i, conn_arc) in available.iter().enumerate() {
            let conn = conn_arc.lock().await;
            if conn.is_healthy() {
                let error_count = conn.get_error_count();
                if error_count < min_error_count {
                    min_error_count = error_count;
                    best_index = i;
                }
            }
        }

        available.remove(best_index)
    }

    /// Weighted random selection
    async fn get_weighted_random_connection(&self) -> Option<Arc<Mutex<PooledConnection<T>>>> {
        let mut available = self.available_connections.lock().await;
        if available.is_empty() {
            return None;
        }

        // Simple random selection (could be enhanced with actual weighting)
        use rand::{Rng, SeedableRng};
        use rand::rngs::StdRng;
        use std::time::SystemTime;
        let seed = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default().as_nanos() as u64;
        let mut rng = StdRng::seed_from_u64(seed);
        let index = rng.gen_range(0..available.len());
        available.remove(index)
    }

    /// Initialize pool with minimum connections
    async fn initialize_pool(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut connections = Vec::new();
        let mut available = self.available_connections.lock().await;

        for _ in 0..self.config.min_connections {
            let connection = self.create_connection().await?;
            let pooled_conn = Arc::new(Mutex::new(PooledConnection::new(connection)));
            available.push_back(Arc::clone(&pooled_conn));
            connections.push(pooled_conn);
        }

        let mut pool_connections = self.connections.write().await;
        *pool_connections = connections;

        Ok(())
    }

    /// Create a new connection
    async fn create_connection(&self) -> Result<T, Box<dyn std::error::Error + Send + Sync>> {
        let factory = Arc::clone(&self.connection_factory);
        let connection = factory().await?;
        self.connections_created.fetch_add(1, Ordering::Relaxed);
        Ok(connection)
    }

    /// Destroy a connection
    async fn destroy_connection(&self, conn_arc: Arc<Mutex<PooledConnection<T>>>) {
        // Remove from all collections
        {
            let mut connections = self.connections.write().await;
            let conn = conn_arc.lock().await;
            connections.retain(|c| {
                if let Ok(existing) = c.try_lock() {
                    existing.id != conn.id
                } else {
                    true
                }
            });
        }

        self.connections_destroyed.fetch_add(1, Ordering::Relaxed);
    }

    /// Start health monitoring and adaptive sizing
    async fn start_monitoring(&self) {
        if self.is_monitoring.swap(true, Ordering::Relaxed) {
            return; // Already monitoring
        }

        let pool = Arc::new(self.clone());
        
        // Health check task
        let health_pool = Arc::clone(&pool);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(health_pool.config.health_check_interval);
            
            while health_pool.is_monitoring.load(Ordering::Relaxed) {
                interval.tick().await;
                health_pool.perform_health_checks().await;
            }
        });

        // Adaptive sizing task
        let sizing_pool = Arc::clone(&pool);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));
            
            while sizing_pool.is_monitoring.load(Ordering::Relaxed) {
                interval.tick().await;
                sizing_pool.adjust_pool_size().await;
            }
        });

        // Cleanup task
        let cleanup_pool = Arc::clone(&pool);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            
            while cleanup_pool.is_monitoring.load(Ordering::Relaxed) {
                interval.tick().await;
                cleanup_pool.cleanup_idle_connections().await;
            }
        });
    }

    /// Perform health checks on all connections
    async fn perform_health_checks(&self) {
        let connections = self.connections.read().await;
        
        for conn_arc in connections.iter() {
            let mut conn = conn_arc.lock().await;
            
            // Simple health check - in practice, you'd validate the actual connection
            let is_healthy = !conn.is_idle(self.config.idle_timeout) && 
                           conn.get_error_count() < 10; // Arbitrary error threshold
            
            conn.set_healthy(is_healthy);
            
            if !is_healthy {
                println!("Connection {} marked as unhealthy", conn.id);
            }
        }
    }

    /// Adjust pool size based on demand
    async fn adjust_pool_size(&self) {
        let stats = self.get_statistics().await;
        
        // Record current demand
        {
            let mut history = self.demand_history.write().await;
            history.push_back((Instant::now(), stats.pool_utilization_percent));
            
            // Keep only recent history (last 10 minutes)
            let cutoff = Instant::now() - Duration::from_secs(600);
            while let Some((timestamp, _)) = history.front() {
                if *timestamp < cutoff {
                    history.pop_front();
                } else {
                    break;
                }
            }
        }

        // Decide on scaling action
        if stats.pool_utilization_percent > 80.0 && stats.total_connections < self.config.max_connections {
            self.expand_pool().await;
        } else if stats.pool_utilization_percent < 30.0 && stats.total_connections > self.config.min_connections {
            self.contract_pool().await;
        }
    }

    /// Expand the pool
    async fn expand_pool(&self) {
        let mut last_expansion = self.last_expansion.write().await;
        let now = Instant::now();
        let time_since_expansion = now.duration_since(*last_expansion).as_secs_f64();
        
        if time_since_expansion < (1.0 / self.config.expansion_rate) {
            return; // Too soon to expand again
        }

        match self.create_connection().await {
            Ok(connection) => {
                let pooled_conn = Arc::new(Mutex::new(PooledConnection::new(connection)));
                
                {
                    let mut connections = self.connections.write().await;
                    connections.push(Arc::clone(&pooled_conn));
                }
                
                {
                    let mut available = self.available_connections.lock().await;
                    available.push_back(pooled_conn);
                }
                
                *last_expansion = now;
                println!("Pool expanded - new size: {}", self.connections.read().await.len());
            },
            Err(e) => {
                eprintln!("Failed to expand pool: {}", e);
            }
        }
    }

    /// Contract the pool
    async fn contract_pool(&self) {
        let mut last_contraction = self.last_contraction.write().await;
        let now = Instant::now();
        let time_since_contraction = now.duration_since(*last_contraction).as_secs_f64();
        
        if time_since_contraction < (1.0 / self.config.contraction_rate) {
            return; // Too soon to contract again
        }

        // Remove least used idle connection
        let conn_to_remove = {
            let mut available = self.available_connections.lock().await;
            if let Some(conn_arc) = available.pop_back() {
                Some(conn_arc)
            } else {
                None
            }
        };

        if let Some(conn_arc) = conn_to_remove {
            self.destroy_connection(conn_arc).await;
            *last_contraction = now;
            println!("Pool contracted - new size: {}", self.connections.read().await.len());
        }
    }

    /// Cleanup idle connections
    async fn cleanup_idle_connections(&self) {
        let idle_connections = {
            let mut available = self.available_connections.lock().await;
            let mut idle_conns = Vec::new();
            
            available.retain(|conn_arc| {
                if let Ok(conn) = conn_arc.try_lock() {
                    if conn.is_idle(self.config.idle_timeout) {
                        idle_conns.push(Arc::clone(conn_arc));
                        false
                    } else {
                        true
                    }
                } else {
                    true // Keep if can't check (actively used)
                }
            });
            
            idle_conns
        };

        for conn_arc in idle_connections {
            self.destroy_connection(conn_arc).await;
        }
    }
}

impl<T, F, Fut> Clone for AdaptiveConnectionPool<T, F, Fut>
where
    T: Send + Sync + 'static,
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<T, Box<dyn std::error::Error + Send + Sync>>> + Send + 'static,
{
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            connections: Arc::clone(&self.connections),
            available_connections: Arc::clone(&self.available_connections),
            active_connections: Arc::clone(&self.active_connections),
            connection_factory: Arc::clone(&self.connection_factory),
            semaphore: Arc::clone(&self.semaphore),
            connections_created: AtomicU64::new(self.connections_created.load(Ordering::Relaxed)),
            connections_destroyed: AtomicU64::new(self.connections_destroyed.load(Ordering::Relaxed)),
            connection_requests: AtomicU64::new(self.connection_requests.load(Ordering::Relaxed)),
            total_wait_time_ms: AtomicU64::new(self.total_wait_time_ms.load(Ordering::Relaxed)),
            last_expansion: Arc::clone(&self.last_expansion),
            last_contraction: Arc::clone(&self.last_contraction),
            demand_history: Arc::clone(&self.demand_history),
            is_monitoring: AtomicBool::new(self.is_monitoring.load(Ordering::Relaxed)),
            next_connection_index: AtomicUsize::new(self.next_connection_index.load(Ordering::Relaxed)),
        }
    }
}

/// Connection handle that automatically returns connection to pool when dropped
pub struct ConnectionHandle<T> {
    connection: Option<Arc<Mutex<PooledConnection<T>>>>,
    pool: Arc<dyn ConnectionPool<T> + Send + Sync>,
    _permit: Option<tokio::sync::OwnedSemaphorePermit>,
}

impl<T> ConnectionHandle<T> {
    pub fn new(
        connection: Arc<Mutex<PooledConnection<T>>>,
        pool: Arc<dyn ConnectionPool<T> + Send + Sync>,
    ) -> Self {
        Self {
            connection: Some(connection),
            pool,
            _permit: None,
        }
    }

    pub fn new_with_permit(
        connection: Arc<Mutex<PooledConnection<T>>>,
        pool: Arc<dyn ConnectionPool<T> + Send + Sync>,
        permit: tokio::sync::OwnedSemaphorePermit,
    ) -> Self {
        Self {
            connection: Some(connection),
            pool,
            _permit: Some(permit),
        }
    }

    pub async fn execute<R, F, Fut>(&mut self, f: F) -> Result<R, Box<dyn std::error::Error + Send + Sync>>
    where
        F: FnOnce(&mut T) -> Fut,
        Fut: std::future::Future<Output = Result<R, Box<dyn std::error::Error + Send + Sync>>>,
    {
        if let Some(conn_arc) = &self.connection {
            let start_time = Instant::now();
            let mut conn = conn_arc.lock().await;
            conn.mark_used();
            
            let result = f(&mut conn.connection).await;
            
            let response_time = start_time.elapsed().as_millis() as f64;
            conn.update_response_time(response_time).await;
            
            match &result {
                Ok(_) => {},
                Err(_) => {
                    conn.increment_error_count();
                    conn.set_healthy(false);
                },
            }
            
            result
        } else {
            Err("Connection handle is empty".into())
        }
    }
}

impl<T> Drop for ConnectionHandle<T> {
    fn drop(&mut self) {
        if let Some(connection) = self.connection.take() {
            let pool = Arc::clone(&self.pool);
            tokio::spawn(async move {
                pool.return_connection(connection).await;
            });
        }
    }
}

/// Connection pool trait for polymorphism
#[async_trait::async_trait]
pub trait ConnectionPool<T> {
    async fn return_connection(&self, connection: Arc<Mutex<PooledConnection<T>>>);
}

#[async_trait::async_trait]
impl<T, F, Fut> ConnectionPool<T> for AdaptiveConnectionPool<T, F, Fut>
where
    T: Send + Sync + 'static,
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<T, Box<dyn std::error::Error + Send + Sync>>> + Send + 'static,
{
    async fn return_connection(&self, connection: Arc<Mutex<PooledConnection<T>>>) {
        self.return_connection(connection).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    // Mock connection for testing
    #[derive(Debug)]
    struct MockConnection {
        id: u32,
    }

    static CONNECTION_COUNTER: AtomicU32 = AtomicU32::new(0);

    async fn create_mock_connection() -> Result<MockConnection, Box<dyn std::error::Error + Send + Sync>> {
        let id = CONNECTION_COUNTER.fetch_add(1, Ordering::Relaxed);
        Ok(MockConnection { id })
    }

    #[tokio::test]
    async fn test_pool_creation() {
        let config = ConnectionPoolConfig {
            min_connections: 2,
            max_connections: 5,
            ..Default::default()
        };

        let pool = AdaptiveConnectionPool::new(config, create_mock_connection).await.unwrap();
        let stats = pool.get_statistics().await;

        assert_eq!(stats.total_connections, 2);
        assert_eq!(stats.idle_connections, 2);
        assert_eq!(stats.active_connections, 0);
    }

    #[tokio::test]
    async fn test_connection_acquisition() {
        let config = ConnectionPoolConfig {
            min_connections: 1,
            max_connections: 3,
            ..Default::default()
        };

        let pool = AdaptiveConnectionPool::new(config, create_mock_connection).await.unwrap();
        let mut handle = pool.get_connection().await.unwrap();

        let result = handle.execute(|conn| async move {
            Ok(conn.id)
        }).await.unwrap();

        assert!(result < 10); // Should be a reasonable ID
    }

    #[tokio::test]
    async fn test_pool_statistics() {
        let config = ConnectionPoolConfig {
            min_connections: 2,
            max_connections: 5,
            ..Default::default()
        };

        let pool = AdaptiveConnectionPool::new(config, create_mock_connection).await.unwrap();
        
        // Get a connection to make it active
        let _handle = pool.get_connection().await.unwrap();
        
        let stats = pool.get_statistics().await;
        assert_eq!(stats.active_connections, 1);
        assert!(stats.connection_requests > 0);
    }

    #[tokio::test]
    async fn test_load_balancing_strategies() {
        let config = ConnectionPoolConfig {
            min_connections: 3,
            max_connections: 5,
            load_balancing: LoadBalancingStrategy::LeastConnections,
            ..Default::default()
        };

        let pool = AdaptiveConnectionPool::new(config, create_mock_connection).await.unwrap();
        
        // Test that we can get connections with different strategies
        let _handle1 = pool.get_connection().await.unwrap();
        let _handle2 = pool.get_connection().await.unwrap();
        
        let stats = pool.get_statistics().await;
        assert_eq!(stats.active_connections, 2);
    }

    #[tokio::test]
    async fn test_connection_health_tracking() {
        let config = ConnectionPoolConfig {
            min_connections: 1,
            max_connections: 2,
            ..Default::default()
        };

        let pool = AdaptiveConnectionPool::new(config, create_mock_connection).await.unwrap();
        let mut handle = pool.get_connection().await.unwrap();

        // Simulate a failed operation
        let _ = handle.execute(|_conn| async move {
            Err::<(), _>("Simulated error".into())
        }).await;

        // Check that error was tracked
        if let Some(conn_arc) = &handle.connection {
            let conn = conn_arc.lock().await;
            assert!(conn.get_error_count() > 0);
        }
    }
}