//! Circuit Breaker Implementation
//!
//! Implements the circuit breaker pattern to prevent cascading failures.

use super::CircuitBreakerConfig;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Circuit breaker states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitBreakerState {
    Closed,   // Normal operation
    Open,     // Failing fast
    HalfOpen, // Testing recovery
}

/// Circuit breaker implementation
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    state: Arc<RwLock<CircuitBreakerState>>,
    failure_count: Arc<AtomicU32>,
    success_count: Arc<AtomicU32>,
    last_failure_time: Arc<RwLock<Option<Instant>>>,
    request_count: Arc<AtomicU64>,
    last_state_change: Arc<RwLock<Instant>>,
}

impl CircuitBreaker {
    /// Create a new circuit breaker
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            state: Arc::new(RwLock::new(CircuitBreakerState::Closed)),
            failure_count: Arc::new(AtomicU32::new(0)),
            success_count: Arc::new(AtomicU32::new(0)),
            last_failure_time: Arc::new(RwLock::new(None)),
            request_count: Arc::new(AtomicU64::new(0)),
            last_state_change: Arc::new(RwLock::new(Instant::now())),
        }
    }
    
    /// Check if the circuit breaker allows execution
    pub async fn can_execute(&self) -> bool {
        if !self.config.enabled {
            return true;
        }
        
        let state = *self.state.read().await;
        
        match state {
            CircuitBreakerState::Closed => true,
            CircuitBreakerState::Open => {
                // Check if recovery timeout has passed
                let last_failure = self.last_failure_time.read().await;
                if let Some(failure_time) = *last_failure {
                    if failure_time.elapsed() >= self.config.recovery_timeout {
                        // Transition to half-open
                        self.transition_to_half_open().await;
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            },
            CircuitBreakerState::HalfOpen => {
                // Allow limited requests through to test recovery
                true
            }
        }
    }
    
    /// Record a successful operation
    pub async fn record_success(&self) {
        if !self.config.enabled {
            return;
        }
        
        let state = *self.state.read().await;
        self.request_count.fetch_add(1, Ordering::Relaxed);
        
        match state {
            CircuitBreakerState::Closed => {
                // Reset failure count on success
                self.failure_count.store(0, Ordering::Relaxed);
            },
            CircuitBreakerState::HalfOpen => {
                let success_count = self.success_count.fetch_add(1, Ordering::Relaxed) + 1;
                
                if success_count >= self.config.success_threshold {
                    // Enough successful requests, transition to closed
                    self.transition_to_closed().await;
                }
            },
            CircuitBreakerState::Open => {
                // This shouldn't happen if can_execute is used properly
                log::warn!("Recorded success while circuit breaker is open");
            }
        }
    }
    
    /// Record a failed operation
    pub async fn record_failure(&self) {
        if !self.config.enabled {
            return;
        }
        
        let state = *self.state.read().await;
        self.request_count.fetch_add(1, Ordering::Relaxed);
        
        let failure_count = self.failure_count.fetch_add(1, Ordering::Relaxed) + 1;
        *self.last_failure_time.write().await = Some(Instant::now());
        
        match state {
            CircuitBreakerState::Closed => {
                if failure_count >= self.config.failure_threshold {
                    self.transition_to_open().await;
                }
            },
            CircuitBreakerState::HalfOpen => {
                // Any failure in half-open state transitions back to open
                self.transition_to_open().await;
            },
            CircuitBreakerState::Open => {
                // Already open, just update failure time
            }
        }
    }
    
    /// Get the current state of the circuit breaker
    pub async fn get_state(&self) -> CircuitBreakerState {
        *self.state.read().await
    }
    
    /// Get circuit breaker metrics
    pub async fn get_metrics(&self) -> CircuitBreakerMetrics {
        let state = *self.state.read().await;
        let failure_count = self.failure_count.load(Ordering::Relaxed);
        let success_count = self.success_count.load(Ordering::Relaxed);
        let request_count = self.request_count.load(Ordering::Relaxed);
        let last_state_change = *self.last_state_change.read().await;
        
        let failure_rate = if request_count > 0 {
            failure_count as f64 / request_count as f64
        } else {
            0.0
        };
        
        CircuitBreakerMetrics {
            state,
            failure_count,
            success_count,
            request_count,
            failure_rate,
            time_in_current_state: last_state_change.elapsed(),
        }
    }
    
    // Private helper methods
    
    async fn transition_to_open(&self) {
        log::info!("Circuit breaker transitioning to OPEN state");
        *self.state.write().await = CircuitBreakerState::Open;
        *self.last_state_change.write().await = Instant::now();
    }
    
    async fn transition_to_half_open(&self) {
        log::info!("Circuit breaker transitioning to HALF-OPEN state");
        *self.state.write().await = CircuitBreakerState::HalfOpen;
        self.success_count.store(0, Ordering::Relaxed);
        *self.last_state_change.write().await = Instant::now();
    }
    
    async fn transition_to_closed(&self) {
        log::info!("Circuit breaker transitioning to CLOSED state");
        *self.state.write().await = CircuitBreakerState::Closed;
        self.failure_count.store(0, Ordering::Relaxed);
        self.success_count.store(0, Ordering::Relaxed);
        *self.last_state_change.write().await = Instant::now();
    }
}

/// Circuit breaker metrics
#[derive(Debug, Clone)]
pub struct CircuitBreakerMetrics {
    pub state: CircuitBreakerState,
    pub failure_count: u32,
    pub success_count: u32,
    pub request_count: u64,
    pub failure_rate: f64,
    pub time_in_current_state: Duration,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    
    #[tokio::test]
    async fn test_circuit_breaker_closed_to_open() {
        let config = CircuitBreakerConfig {
            enabled: true,
            failure_threshold: 3,
            recovery_timeout: Duration::from_secs(60),
            success_threshold: 2,
        };
        
        let cb = CircuitBreaker::new(config);
        
        // Initially closed and allows requests
        assert_eq!(cb.get_state().await, CircuitBreakerState::Closed);
        assert!(cb.can_execute().await);
        
        // Record failures to trigger open state
        for _ in 0..3 {
            cb.record_failure().await;
        }
        
        // Should now be open and block requests
        assert_eq!(cb.get_state().await, CircuitBreakerState::Open);
        assert!(!cb.can_execute().await);
    }
    
    #[tokio::test]
    async fn test_circuit_breaker_recovery() {
        let config = CircuitBreakerConfig {
            enabled: true,
            failure_threshold: 2,
            recovery_timeout: Duration::from_millis(10), // Very short for testing
            success_threshold: 2,
        };
        
        let cb = CircuitBreaker::new(config);
        
        // Trigger open state
        cb.record_failure().await;
        cb.record_failure().await;
        assert_eq!(cb.get_state().await, CircuitBreakerState::Open);
        
        // Wait for recovery timeout
        tokio::time::sleep(Duration::from_millis(20)).await;
        
        // Should now allow requests (transitioning to half-open)
        assert!(cb.can_execute().await);
        assert_eq!(cb.get_state().await, CircuitBreakerState::HalfOpen);
        
        // Record successful requests to close circuit
        cb.record_success().await;
        cb.record_success().await;
        
        // Should now be closed
        assert_eq!(cb.get_state().await, CircuitBreakerState::Closed);
    }
    
    #[tokio::test]
    async fn test_circuit_breaker_half_open_failure() {
        let config = CircuitBreakerConfig {
            enabled: true,
            failure_threshold: 1,
            recovery_timeout: Duration::from_millis(10),
            success_threshold: 2,
        };
        
        let cb = CircuitBreaker::new(config);
        
        // Trigger open state
        cb.record_failure().await;
        assert_eq!(cb.get_state().await, CircuitBreakerState::Open);
        
        // Wait for recovery timeout
        tokio::time::sleep(Duration::from_millis(20)).await;
        
        // Transition to half-open
        assert!(cb.can_execute().await);
        assert_eq!(cb.get_state().await, CircuitBreakerState::HalfOpen);
        
        // Record failure in half-open state
        cb.record_failure().await;
        
        // Should transition back to open
        assert_eq!(cb.get_state().await, CircuitBreakerState::Open);
        assert!(!cb.can_execute().await);
    }
    
    #[tokio::test]
    async fn test_circuit_breaker_metrics() {
        let config = CircuitBreakerConfig {
            enabled: true,
            failure_threshold: 5,
            recovery_timeout: Duration::from_secs(60),
            success_threshold: 3,
        };
        
        let cb = CircuitBreaker::new(config);
        
        // Record some operations
        cb.record_success().await;
        cb.record_success().await;
        cb.record_failure().await;
        
        let metrics = cb.get_metrics().await;
        assert_eq!(metrics.success_count, 2);
        assert_eq!(metrics.failure_count, 1);
        assert_eq!(metrics.request_count, 3);
        assert_eq!(metrics.state, CircuitBreakerState::Closed);
        assert!((metrics.failure_rate - (1.0 / 3.0)).abs() < f64::EPSILON);
    }
    
    #[tokio::test]
    async fn test_circuit_breaker_disabled() {
        let config = CircuitBreakerConfig {
            enabled: false,
            failure_threshold: 1,
            recovery_timeout: Duration::from_secs(60),
            success_threshold: 1,
        };
        
        let cb = CircuitBreaker::new(config);
        
        // Even after failures, should still allow requests when disabled
        cb.record_failure().await;
        cb.record_failure().await;
        cb.record_failure().await;
        
        assert!(cb.can_execute().await);
    }
}