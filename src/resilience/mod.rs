//! Network resilience and fault tolerance mechanisms
//! 
//! Provides automatic reconnection, circuit breakers, retry logic,
//! and failover capabilities for production reliability.

use std::sync::Arc;
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Mutex};
use tokio::time::{interval, sleep};
use crate::error::{Error, Result};
use crate::protocol::PeerId;
use crate::transport::TransportAddress;

/// Connection state for resilience tracking
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    Connected,
    Disconnected,
    Reconnecting,
    Failed,
}

/// Circuit breaker states
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CircuitState {
    Closed,     // Normal operation
    Open,       // Failing, reject requests
    HalfOpen,   // Testing if service recovered
}

/// Network resilience manager
pub struct ResilienceManager {
    connections: Arc<RwLock<HashMap<PeerId, ConnectionInfo>>>,
    circuit_breakers: Arc<RwLock<HashMap<String, CircuitBreaker>>>,
    retry_policies: Arc<RwLock<HashMap<String, RetryPolicy>>>,
    _health_checker: Arc<HealthChecker>,
    reconnect_scheduler: Arc<ReconnectScheduler>,
}

/// Connection information with health tracking
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub peer_id: PeerId,
    pub address: TransportAddress,
    pub state: ConnectionState,
    pub last_seen: Instant,
    pub reconnect_attempts: u32,
    pub consecutive_failures: u32,
    pub latency_ms: Option<u32>,
    pub packet_loss_rate: f32,
}

/// Circuit breaker for preventing cascading failures
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    name: String,
    state: CircuitState,
    failure_count: u32,
    success_count: u32,
    last_failure_time: Option<Instant>,
    last_state_change: Instant,
    config: CircuitBreakerConfig,
}

/// Circuit breaker configuration
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    pub failure_threshold: u32,
    pub success_threshold: u32,
    pub timeout: Duration,
    pub half_open_requests: u32,
}

/// Retry policy for failed operations
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub exponential_base: f32,
    pub jitter: bool,
}

/// Health checker for connections
pub struct HealthChecker {
    _check_interval: Duration,
    _timeout: Duration,
    _unhealthy_threshold: u32,
    _healthy_threshold: u32,
}

/// Reconnection scheduler with backoff
pub struct ReconnectScheduler {
    queue: Arc<Mutex<VecDeque<ReconnectTask>>>,
    _max_concurrent: usize,
    base_delay: Duration,
    max_delay: Duration,
}

/// Reconnection task
#[derive(Debug, Clone)]
struct ReconnectTask {
    peer_id: PeerId,
    _address: TransportAddress,
    attempt: u32,
    scheduled_at: Instant,
}

impl Default for ResilienceManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ResilienceManager {
    /// Create a new resilience manager
    pub fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            circuit_breakers: Arc::new(RwLock::new(HashMap::new())),
            retry_policies: Arc::new(RwLock::new(HashMap::new())),
            _health_checker: Arc::new(HealthChecker::new()),
            reconnect_scheduler: Arc::new(ReconnectScheduler::new()),
        }
    }
    
    /// Register a connection for monitoring
    pub async fn register_connection(&self, peer_id: PeerId, address: TransportAddress) {
        let mut connections = self.connections.write().await;
        connections.insert(peer_id, ConnectionInfo {
            peer_id,
            address,
            state: ConnectionState::Connected,
            last_seen: Instant::now(),
            reconnect_attempts: 0,
            consecutive_failures: 0,
            latency_ms: None,
            packet_loss_rate: 0.0,
        });
    }
    
    /// Handle connection failure with automatic recovery
    pub async fn handle_failure(&self, peer_id: PeerId) -> Result<()> {
        let (address, reconnect_attempts, should_reconnect) = {
            let mut connections = self.connections.write().await;
            
            if let Some(conn) = connections.get_mut(&peer_id) {
                conn.consecutive_failures += 1;
                conn.state = ConnectionState::Disconnected;
                
                // Check if we should attempt reconnection
                if conn.consecutive_failures < 10 {
                    conn.state = ConnectionState::Reconnecting;
                    (conn.address.clone(), conn.reconnect_attempts, true)
                } else {
                    conn.state = ConnectionState::Failed;
                    (conn.address.clone(), conn.reconnect_attempts, false)
                }
            } else {
                return Err(Error::Network("Unknown peer".to_string()));
            }
        };
        
        if should_reconnect {
            // Schedule reconnection with exponential backoff
            self.reconnect_scheduler.schedule(
                peer_id,
                address,
                reconnect_attempts,
            ).await;
            Ok(())
        } else {
            Err(Error::Network(format!("Connection to {:?} permanently failed", peer_id)))
        }
    }
    
    /// Execute operation with retry policy
    pub async fn with_retry<F, Fut, T>(
        &self,
        policy_name: &str,
        mut operation: F,
    ) -> Result<T>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let policies = self.retry_policies.read().await;
        let policy = policies.get(policy_name)
            .cloned()
            .unwrap_or_else(RetryPolicy::default);
        drop(policies);
        
        let mut attempt = 0;
        let mut delay = policy.initial_delay;
        
        loop {
            attempt += 1;
            
            match operation().await {
                Ok(result) => return Ok(result),
                Err(e) if attempt >= policy.max_attempts => {
                    return Err(Error::Network(format!(
                        "Operation failed after {} attempts: {}",
                        attempt, e
                    )));
                }
                Err(_) => {
                    // Apply jitter if enabled
                    let mut actual_delay = delay;
                    if policy.jitter {
                        let jitter = rand::random::<f32>() * 0.3;
                        actual_delay = delay.mul_f32(1.0 + jitter - 0.15);
                    }
                    
                    sleep(actual_delay).await;
                    
                    // Exponential backoff
                    let delay_secs = (delay.as_secs_f32() * policy.exponential_base)
                        .min(policy.max_delay.as_secs_f32());
                    delay = Duration::from_secs_f32(delay_secs);
                }
            }
        }
    }
    
    /// Check circuit breaker state
    pub async fn check_circuit(&self, name: &str) -> Result<()> {
        let mut breakers = self.circuit_breakers.write().await;
        let breaker = breakers.entry(name.to_string())
            .or_insert_with(|| CircuitBreaker::new(name.to_string()));
        
        match breaker.state {
            CircuitState::Open => {
                // Check if timeout has passed
                if let Some(last_failure) = breaker.last_failure_time {
                    if last_failure.elapsed() > breaker.config.timeout {
                        breaker.transition_to(CircuitState::HalfOpen);
                    } else {
                        return Err(Error::Network(format!(
                            "Circuit breaker '{}' is open",
                            name
                        )));
                    }
                } else {
                    return Err(Error::Network(format!(
                        "Circuit breaker '{}' is open",
                        name
                    )));
                }
            }
            CircuitState::HalfOpen => {
                // Allow limited requests through
                if breaker.success_count >= breaker.config.half_open_requests {
                    return Err(Error::Network(format!(
                        "Circuit breaker '{}' is testing",
                        name
                    )));
                }
            }
            CircuitState::Closed => {
                // Normal operation
            }
        }
        
        Ok(())
    }
    
    /// Record success for circuit breaker
    pub async fn record_success(&self, name: &str) {
        let mut breakers = self.circuit_breakers.write().await;
        if let Some(breaker) = breakers.get_mut(name) {
            breaker.record_success();
        }
    }
    
    /// Record failure for circuit breaker
    pub async fn record_failure(&self, name: &str) {
        let mut breakers = self.circuit_breakers.write().await;
        if let Some(breaker) = breakers.get_mut(name) {
            breaker.record_failure();
        }
    }
    
    /// Update connection metrics
    pub async fn update_metrics(
        &self,
        peer_id: PeerId,
        latency_ms: u32,
        packet_loss: f32,
    ) {
        let mut connections = self.connections.write().await;
        if let Some(conn) = connections.get_mut(&peer_id) {
            conn.latency_ms = Some(latency_ms);
            conn.packet_loss_rate = packet_loss;
            conn.last_seen = Instant::now();
            
            // Reset failure count on successful communication
            if conn.state == ConnectionState::Connected {
                conn.consecutive_failures = 0;
            }
        }
    }
    
    /// Start background health monitoring
    pub fn start_monitoring(self: Arc<Self>) {
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(10));
            
            loop {
                interval.tick().await;
                
                // Check connection health
                let connections = self.connections.read().await.clone();
                for (peer_id, conn) in connections {
                    if conn.last_seen.elapsed() > Duration::from_secs(30) {
                        // Connection seems dead, trigger reconnection
                        if conn.state == ConnectionState::Connected {
                            let _ = self.handle_failure(peer_id).await;
                        }
                    }
                }
                
                // Process reconnection queue
                self.reconnect_scheduler.process().await;
            }
        });
    }
    
    /// Get connection statistics
    pub async fn get_stats(&self) -> NetworkStats {
        let connections = self.connections.read().await;
        let breakers = self.circuit_breakers.read().await;
        
        let total_connections = connections.len();
        let connected = connections.values()
            .filter(|c| c.state == ConnectionState::Connected)
            .count();
        let reconnecting = connections.values()
            .filter(|c| c.state == ConnectionState::Reconnecting)
            .count();
        let failed = connections.values()
            .filter(|c| c.state == ConnectionState::Failed)
            .count();
        
        let open_circuits = breakers.values()
            .filter(|b| b.state == CircuitState::Open)
            .count();
        
        let avg_latency = connections.values()
            .filter_map(|c| c.latency_ms)
            .sum::<u32>() as f32 / connected.max(1) as f32;
        
        let avg_packet_loss = connections.values()
            .map(|c| c.packet_loss_rate)
            .sum::<f32>() / total_connections.max(1) as f32;
        
        NetworkStats {
            total_connections,
            connected,
            reconnecting,
            failed,
            open_circuits,
            avg_latency_ms: avg_latency,
            avg_packet_loss,
        }
    }
}

impl CircuitBreaker {
    fn new(name: String) -> Self {
        Self {
            name,
            state: CircuitState::Closed,
            failure_count: 0,
            success_count: 0,
            last_failure_time: None,
            last_state_change: Instant::now(),
            config: CircuitBreakerConfig::default(),
        }
    }
    
    fn record_success(&mut self) {
        match self.state {
            CircuitState::HalfOpen => {
                self.success_count += 1;
                if self.success_count >= self.config.success_threshold {
                    self.transition_to(CircuitState::Closed);
                }
            }
            CircuitState::Closed => {
                self.failure_count = 0;
            }
            _ => {}
        }
    }
    
    fn record_failure(&mut self) {
        self.last_failure_time = Some(Instant::now());
        
        match self.state {
            CircuitState::Closed => {
                self.failure_count += 1;
                if self.failure_count >= self.config.failure_threshold {
                    self.transition_to(CircuitState::Open);
                }
            }
            CircuitState::HalfOpen => {
                self.transition_to(CircuitState::Open);
            }
            _ => {}
        }
    }
    
    fn transition_to(&mut self, new_state: CircuitState) {
        log::info!("Circuit breaker '{}' transitioning from {:?} to {:?}",
                  self.name, self.state, new_state);
        
        self.state = new_state;
        self.last_state_change = Instant::now();
        
        match new_state {
            CircuitState::Closed => {
                self.failure_count = 0;
                self.success_count = 0;
            }
            CircuitState::HalfOpen => {
                self.success_count = 0;
            }
            _ => {}
        }
    }
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 3,
            timeout: Duration::from_secs(60),
            half_open_requests: 3,
        }
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            exponential_base: 2.0,
            jitter: true,
        }
    }
}

impl HealthChecker {
    fn new() -> Self {
        Self {
            _check_interval: Duration::from_secs(10),
            _timeout: Duration::from_secs(5),
            _unhealthy_threshold: 3,
            _healthy_threshold: 2,
        }
    }
}

impl ReconnectScheduler {
    fn new() -> Self {
        Self {
            queue: Arc::new(Mutex::new(VecDeque::new())),
            _max_concurrent: 5,
            base_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(300),
        }
    }
    
    async fn schedule(&self, peer_id: PeerId, address: TransportAddress, attempt: u32) {
        let delay = self.calculate_delay(attempt);
        let task = ReconnectTask {
            peer_id,
            _address: address,
            attempt: attempt + 1,
            scheduled_at: Instant::now() + delay,
        };
        
        let mut queue = self.queue.lock().await;
        queue.push_back(task);
    }
    
    async fn process(&self) {
        let mut queue = self.queue.lock().await;
        let now = Instant::now();
        
        while let Some(task) = queue.front() {
            if task.scheduled_at > now {
                break;
            }
            
            if let Some(task) = queue.pop_front() {
                // Trigger reconnection
                log::info!("Attempting reconnection to {:?} (attempt {})",
                          task.peer_id, task.attempt);
                
                // Actual reconnection would happen here
                // For now, just log the attempt
            }
        }
    }
    
    fn calculate_delay(&self, attempt: u32) -> Duration {
        let delay = self.base_delay * 2u32.pow(attempt.min(10));
        delay.min(self.max_delay)
    }
}

/// Network statistics
#[derive(Debug, Clone)]
pub struct NetworkStats {
    pub total_connections: usize,
    pub connected: usize,
    pub reconnecting: usize,
    pub failed: usize,
    pub open_circuits: usize,
    pub avg_latency_ms: f32,
    pub avg_packet_loss: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_circuit_breaker() {
        let manager = Arc::new(ResilienceManager::new());
        
        // Record failures to open circuit
        for _ in 0..5 {
            manager.record_failure("test").await;
        }
        
        // Circuit should be open
        assert!(manager.check_circuit("test").await.is_err());
        
        // Wait for timeout
        tokio::time::sleep(Duration::from_secs(61)).await;
        
        // Circuit should be half-open
        assert!(manager.check_circuit("test").await.is_ok());
    }
    
    #[tokio::test]
    async fn test_retry_policy() {
        let manager = Arc::new(ResilienceManager::new());
        
        let attempts = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let attempts_clone = attempts.clone();
        
        let result = manager.with_retry("test", move || {
            let attempts = attempts_clone.clone();
            async move {
                let count = attempts.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if count < 2 {
                    Err(Error::Network("Temporary failure".to_string()))
                } else {
                    Ok(42)
                }
            }
        }).await;
        
        assert_eq!(result.unwrap(), 42);
        assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 3);
    }
}