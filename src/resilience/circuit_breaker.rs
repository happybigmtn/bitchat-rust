//! Circuit Breaker Pattern for External Service Calls
//!
//! Prevents cascading failures by stopping calls to failing services
//! and allowing them time to recover.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CircuitBreakerError {
    #[error("Circuit breaker is open")]
    CircuitOpen,
    #[error("Service call failed: {0}")]
    ServiceError(String),
}

#[derive(Debug, Clone, Copy)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

/// Configuration for circuit breaker behavior
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of failures before opening circuit
    pub failure_threshold: u32,
    /// Success threshold to close circuit from half-open
    pub success_threshold: u32,
    /// Time window for counting failures
    pub failure_window: Duration,
    /// How long to keep circuit open before trying half-open
    pub timeout_duration: Duration,
    /// Maximum concurrent requests in half-open state
    pub half_open_max_requests: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 3,
            failure_window: Duration::from_secs(60),
            timeout_duration: Duration::from_secs(30),
            half_open_max_requests: 3,
        }
    }
}

/// Circuit breaker for protecting external service calls
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    state: Arc<RwLock<CircuitState>>,
    failure_count: AtomicU32,
    success_count: AtomicU32,
    half_open_requests: AtomicU32,
    last_failure_time: Arc<RwLock<Option<Instant>>>,
    last_state_change: Arc<RwLock<Instant>>,
    total_requests: AtomicU64,
    total_failures: AtomicU64,
    circuit_opened_count: AtomicU32,
}

impl CircuitBreaker {
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            state: Arc::new(RwLock::new(CircuitState::Closed)),
            failure_count: AtomicU32::new(0),
            success_count: AtomicU32::new(0),
            half_open_requests: AtomicU32::new(0),
            last_failure_time: Arc::new(RwLock::new(None)),
            last_state_change: Arc::new(RwLock::new(Instant::now())),
            total_requests: AtomicU64::new(0),
            total_failures: AtomicU64::new(0),
            circuit_opened_count: AtomicU32::new(0),
        }
    }

    /// Execute a function with circuit breaker protection
    pub async fn call<F, T, E>(&self, f: F) -> Result<T, CircuitBreakerError>
    where
        F: FnOnce() -> futures::future::BoxFuture<'static, Result<T, E>>,
        E: std::fmt::Display,
    {
        self.total_requests.fetch_add(1, Ordering::Relaxed);

        // Check if we should attempt the call
        if !self.should_attempt_call().await {
            return Err(CircuitBreakerError::CircuitOpen);
        }

        // Execute the function
        let result = f().await;

        // Update circuit breaker state based on result
        match result {
            Ok(value) => {
                self.record_success().await;
                Ok(value)
            }
            Err(e) => {
                self.record_failure().await;
                self.total_failures.fetch_add(1, Ordering::Relaxed);
                Err(CircuitBreakerError::ServiceError(e.to_string()))
            }
        }
    }

    /// Check if a call should be attempted
    async fn should_attempt_call(&self) -> bool {
        let state = *self.state.read().await;

        match state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                // Check if timeout has passed to transition to half-open
                let last_change = *self.last_state_change.read().await;
                if last_change.elapsed() >= self.config.timeout_duration {
                    self.transition_to_half_open().await;
                    self.try_half_open_request()
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => self.try_half_open_request(),
        }
    }

    /// Try to make a request in half-open state
    fn try_half_open_request(&self) -> bool {
        let current = self.half_open_requests.fetch_add(1, Ordering::AcqRel);
        if current < self.config.half_open_max_requests {
            true
        } else {
            self.half_open_requests.fetch_sub(1, Ordering::AcqRel);
            false
        }
    }

    /// Record a successful call
    async fn record_success(&self) {
        let state = *self.state.read().await;

        match state {
            CircuitState::Closed => {
                // Reset failure count on success in closed state
                self.failure_count.store(0, Ordering::Release);
            }
            CircuitState::HalfOpen => {
                let success_count = self.success_count.fetch_add(1, Ordering::AcqRel) + 1;
                self.half_open_requests.fetch_sub(1, Ordering::AcqRel);

                if success_count >= self.config.success_threshold {
                    self.transition_to_closed().await;
                }
            }
            CircuitState::Open => {
                // Shouldn't happen, but handle gracefully
                log::warn!("Success recorded in open state");
            }
        }
    }

    /// Record a failed call
    async fn record_failure(&self) {
        let state = *self.state.read().await;

        match state {
            CircuitState::Closed => {
                // Check if failure is within time window
                let mut last_failure = self.last_failure_time.write().await;
                let now = Instant::now();

                let should_count = match *last_failure {
                    Some(last) if now.duration_since(last) <= self.config.failure_window => true,
                    _ => {
                        // Reset counter for new window
                        self.failure_count.store(0, Ordering::Relaxed);
                        true
                    }
                };

                if should_count {
                    *last_failure = Some(now);
                    let failure_count = self.failure_count.fetch_add(1, Ordering::AcqRel) + 1;

                    if failure_count >= self.config.failure_threshold {
                        drop(last_failure); // Release lock before transition
                        self.transition_to_open().await;
                    }
                }
            }
            CircuitState::HalfOpen => {
                self.half_open_requests.fetch_sub(1, Ordering::AcqRel);
                self.transition_to_open().await;
            }
            CircuitState::Open => {
                // Already open, nothing to do
            }
        }
    }

    /// Transition to open state
    async fn transition_to_open(&self) {
        let mut state = self.state.write().await;
        if !matches!(*state, CircuitState::Open) {
            *state = CircuitState::Open;
            *self.last_state_change.write().await = Instant::now();
            self.circuit_opened_count.fetch_add(1, Ordering::AcqRel);
            self.success_count.store(0, Ordering::Release);
            log::warn!("Circuit breaker opened after {} failures", self.config.failure_threshold);
        }
    }

    /// Transition to half-open state
    async fn transition_to_half_open(&self) {
        let mut state = self.state.write().await;
        if matches!(*state, CircuitState::Open) {
            *state = CircuitState::HalfOpen;
            *self.last_state_change.write().await = Instant::now();
            self.success_count.store(0, Ordering::Relaxed);
            self.half_open_requests.store(0, Ordering::Relaxed);
            log::info!("Circuit breaker transitioned to half-open");
        }
    }

    /// Transition to closed state
    async fn transition_to_closed(&self) {
        let mut state = self.state.write().await;
        if !matches!(*state, CircuitState::Closed) {
            *state = CircuitState::Closed;
            *self.last_state_change.write().await = Instant::now();
            self.failure_count.store(0, Ordering::Relaxed);
            self.success_count.store(0, Ordering::Relaxed);
            self.half_open_requests.store(0, Ordering::Relaxed);
            log::info!("Circuit breaker closed after successful recovery");
        }
    }

    /// Get current circuit state
    pub async fn get_state(&self) -> CircuitState {
        *self.state.read().await
    }

    /// Get circuit breaker statistics
    pub fn get_stats(&self) -> CircuitBreakerStats {
        CircuitBreakerStats {
            total_requests: self.total_requests.load(Ordering::Relaxed),
            total_failures: self.total_failures.load(Ordering::Relaxed),
            current_failure_count: self.failure_count.load(Ordering::Relaxed),
            circuit_opened_count: self.circuit_opened_count.load(Ordering::Relaxed),
        }
    }

    /// Reset circuit breaker to closed state
    pub async fn reset(&self) {
        let mut state = self.state.write().await;
        *state = CircuitState::Closed;
        *self.last_state_change.write().await = Instant::now();
        self.failure_count.store(0, Ordering::Relaxed);
        self.success_count.store(0, Ordering::Relaxed);
        self.half_open_requests.store(0, Ordering::Relaxed);
        *self.last_failure_time.write().await = None;
        log::info!("Circuit breaker manually reset");
    }
}

/// Statistics for circuit breaker monitoring
#[derive(Debug, Clone)]
pub struct CircuitBreakerStats {
    pub total_requests: u64,
    pub total_failures: u64,
    pub current_failure_count: u32,
    pub circuit_opened_count: u32,
}

/// Manager for multiple circuit breakers
pub struct CircuitBreakerManager {
    breakers: dashmap::DashMap<String, Arc<CircuitBreaker>>,
    default_config: CircuitBreakerConfig,
}

impl CircuitBreakerManager {
    pub fn new(default_config: CircuitBreakerConfig) -> Self {
        Self {
            breakers: dashmap::DashMap::new(),
            default_config,
        }
    }

    /// Get or create a circuit breaker for a service
    pub fn get_breaker(&self, service_name: &str) -> Arc<CircuitBreaker> {
        self.breakers
            .entry(service_name.to_string())
            .or_insert_with(|| Arc::new(CircuitBreaker::new(self.default_config.clone())))
            .clone()
    }

    /// Get statistics for all circuit breakers
    pub fn get_all_stats(&self) -> Vec<(String, CircuitBreakerStats)> {
        self.breakers
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().get_stats()))
            .collect()
    }

    /// Reset all circuit breakers
    pub async fn reset_all(&self) {
        for entry in self.breakers.iter() {
            entry.value().reset().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_circuit_breaker_opens_on_failures() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            ..Default::default()
        };
        let breaker = CircuitBreaker::new(config);

        // Simulate failures
        for _ in 0..3 {
            let _ = breaker.call(|| {
                Box::pin(async { Result::<(), &str>::Err("Service error") })
            }).await;
        }

        // Circuit should be open
        assert!(matches!(breaker.get_state().await, CircuitState::Open));

        // Next call should fail immediately
        let result = breaker.call(|| {
            Box::pin(async { Result::<(), &str>::Ok(()) })
        }).await;
        assert!(matches!(result, Err(CircuitBreakerError::CircuitOpen)));
    }

    #[tokio::test]
    async fn test_circuit_breaker_recovers() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            success_threshold: 2,
            timeout_duration: Duration::from_millis(100),
            ..Default::default()
        };
        let breaker = CircuitBreaker::new(config);

        // Open the circuit
        for _ in 0..2 {
            let _ = breaker.call(|| {
                Box::pin(async { Result::<(), &str>::Err("Error") })
            }).await;
        }
        assert!(matches!(breaker.get_state().await, CircuitState::Open));

        // Wait for timeout
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Should transition to half-open and allow limited requests
        for _ in 0..2 {
            let _ = breaker.call(|| {
                Box::pin(async { Result::<(), &str>::Ok(()) })
            }).await;
        }

        // Circuit should be closed after successful requests
        assert!(matches!(breaker.get_state().await, CircuitState::Closed));
    }
}