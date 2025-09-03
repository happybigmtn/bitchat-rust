//! Loop Budget Utility for Resource-Bounded Loops
//!
//! This module provides utilities to prevent unbounded resource consumption
//! in infinite loops by implementing iteration limits, backpressure, and
//! load shedding mechanisms.

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::time::sleep;

/// Resource budget for controlling loop iterations
#[derive(Debug, Clone)]
pub struct LoopBudget {
    /// Maximum iterations per time window
    max_iterations_per_window: u64,
    /// Time window duration
    window_duration: Duration,
    /// Current iteration count in window
    current_iterations: Arc<AtomicU64>,
    /// Window start time
    window_start: Arc<std::sync::Mutex<Instant>>,
    /// Backoff configuration
    backoff: BackoffConfig,
}

/// Backoff configuration for when budget is exhausted
#[derive(Debug, Clone)]
pub struct BackoffConfig {
    /// Initial backoff duration
    pub initial_backoff: Duration,
    /// Maximum backoff duration
    pub max_backoff: Duration,
    /// Backoff multiplier
    pub multiplier: f64,
    /// Current backoff duration
    current_backoff: Arc<std::sync::Mutex<Duration>>,
}

impl Default for BackoffConfig {
    fn default() -> Self {
        Self {
            initial_backoff: Duration::from_millis(10),
            max_backoff: Duration::from_secs(1),
            multiplier: 1.5,
            current_backoff: Arc::new(std::sync::Mutex::new(Duration::from_millis(10))),
        }
    }
}

impl LoopBudget {
    /// Create new loop budget with specified limits
    pub fn new(max_iterations_per_second: u64) -> Self {
        Self {
            max_iterations_per_window: max_iterations_per_second,
            window_duration: Duration::from_secs(1),
            current_iterations: Arc::new(AtomicU64::new(0)),
            window_start: Arc::new(std::sync::Mutex::new(Instant::now())),
            backoff: BackoffConfig::default(),
        }
    }

    /// Create budget for high-frequency loops (e.g., network operations)
    pub fn for_network() -> Self {
        Self::new(1000) // 1000 iterations per second
    }

    /// Create budget for medium-frequency loops (e.g., consensus operations)
    pub fn for_consensus() -> Self {
        Self::new(500) // 500 iterations per second
    }

    /// Create budget for low-frequency loops (e.g., cleanup operations)
    pub fn for_maintenance() -> Self {
        Self::new(100) // 100 iterations per second
    }

    /// Create budget for discovery operations
    pub fn for_discovery() -> Self {
        Self::new(200) // 200 iterations per second
    }

    /// Check if we can proceed with the next iteration
    pub fn can_proceed(&self) -> bool {
        self.reset_window_if_needed();

        let current = self.current_iterations.load(Ordering::Relaxed);
        current < self.max_iterations_per_window
    }

    /// Consume budget for one iteration
    pub fn consume(&self, count: u64) {
        self.current_iterations.fetch_add(count, Ordering::Relaxed);

        // Reset backoff on successful iteration
        if let Ok(mut backoff) = self.backoff.current_backoff.lock() {
            *backoff = self.backoff.initial_backoff;
        }
    }

    /// Get backoff duration when budget is exhausted
    pub async fn backoff(&self) {
        let backoff_duration = {
            // Handle poisoned mutex by using the poisoned data
            let mut current = match self.backoff.current_backoff.lock() {
                Ok(guard) => guard,
                Err(poisoned) => {
                    log::error!("Backoff mutex poisoned, recovering");
                    poisoned.into_inner()
                }
            };
            let duration = *current;

            // Increase backoff for next time
            let next = Duration::from_millis(
                (current.as_millis() as f64 * self.backoff.multiplier) as u64,
            );
            *current = next.min(self.backoff.max_backoff);

            duration
        };

        sleep(backoff_duration).await;
    }

    /// Get current utilization percentage (0.0 to 100.0)
    pub fn utilization(&self) -> f64 {
        self.reset_window_if_needed();
        let current = self.current_iterations.load(Ordering::Relaxed);
        (current as f64 / self.max_iterations_per_window as f64) * 100.0
    }

    /// Reset window if time window has passed
    fn reset_window_if_needed(&self) {
        if let Ok(mut window_start) = self.window_start.lock() {
            if window_start.elapsed() >= self.window_duration {
                *window_start = Instant::now();
                self.current_iterations.store(0, Ordering::Relaxed);
            }
        }
    }
}

/// Bounded channel wrapper with overflow handling
pub struct BoundedLoop<T> {
    receiver: mpsc::Receiver<T>,
    budget: LoopBudget,
    overflow_handler: OverflowHandler<T>,
    stats: Arc<LoopStats>,
}

/// Overflow handling strategies
pub enum OverflowHandler<T> {
    /// Drop oldest messages when full
    DropOldest,
    /// Drop newest messages when full
    DropNewest,
    /// Custom handler function
    Custom(Box<dyn Fn(T) + Send + Sync>),
}

/// Loop statistics
#[derive(Debug, Default)]
pub struct LoopStats {
    pub iterations: AtomicU64,
    pub budget_exceeded: AtomicU64,
    pub messages_dropped: AtomicU64,
    pub backoff_count: AtomicU64,
}

impl<T> BoundedLoop<T> {
    /// Create new bounded loop with receiver and budget
    pub fn new(
        receiver: mpsc::Receiver<T>,
        budget: LoopBudget,
        overflow_handler: OverflowHandler<T>,
    ) -> Self {
        Self {
            receiver,
            budget,
            overflow_handler,
            stats: Arc::new(LoopStats::default()),
        }
    }

    /// Process messages with budget control
    pub async fn process_with_budget<F, Fut>(&mut self, mut handler: F)
    where
        F: FnMut(T) -> Fut,
        Fut: std::future::Future<Output = ()>,
    {
        loop {
            // Check budget before processing
            if !self.budget.can_proceed() {
                self.stats.budget_exceeded.fetch_add(1, Ordering::Relaxed);
                self.budget.backoff().await;
                continue;
            }

            // Try to receive message with timeout
            match tokio::time::timeout(Duration::from_millis(100), self.receiver.recv()).await {
                Ok(Some(message)) => {
                    // Process message
                    handler(message).await;
                    self.budget.consume(1);
                    self.stats.iterations.fetch_add(1, Ordering::Relaxed);
                }
                Ok(None) => {
                    // Channel closed
                    break;
                }
                Err(_) => {
                    // Timeout - give up CPU briefly
                    sleep(Duration::from_millis(1)).await;
                }
            }
        }
    }

    /// Get loop statistics
    pub fn stats(&self) -> Arc<LoopStats> {
        self.stats.clone()
    }
}

/// Circuit breaker for preventing cascade failures
#[derive(Debug)]
pub struct CircuitBreaker {
    failure_threshold: usize,
    recovery_timeout: Duration,
    current_failures: AtomicUsize,
    state: Arc<std::sync::Mutex<CircuitState>>,
    last_failure: Arc<std::sync::Mutex<Option<Instant>>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CircuitState {
    Closed,   // Normal operation
    Open,     // Failing, rejecting requests
    HalfOpen, // Testing if recovered
}

impl CircuitBreaker {
    /// Create new circuit breaker
    pub fn new(failure_threshold: usize, recovery_timeout: Duration) -> Self {
        Self {
            failure_threshold,
            recovery_timeout,
            current_failures: AtomicUsize::new(0),
            state: Arc::new(std::sync::Mutex::new(CircuitState::Closed)),
            last_failure: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    /// Check if requests should be allowed through
    pub fn allow_request(&self) -> bool {
        let mut state = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                log::error!("Circuit state mutex poisoned in allow_request, recovering");
                poisoned.into_inner()
            }
        };

        match *state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                // Check if we should transition to half-open
                if let Ok(last_failure) = self.last_failure.lock() {
                    if let Some(last) = *last_failure {
                        if last.elapsed() >= self.recovery_timeout {
                            *state = CircuitState::HalfOpen;
                            return true;
                        }
                    }
                }
                false
            }
            CircuitState::HalfOpen => true,
        }
    }

    /// Record a successful operation
    pub fn record_success(&self) {
        self.current_failures.store(0, Ordering::Relaxed);
        let mut state = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                log::error!("Circuit state mutex poisoned in record_success, recovering");
                poisoned.into_inner()
            }
        };
        *state = CircuitState::Closed;
    }

    /// Record a failed operation
    pub fn record_failure(&self) {
        let failures = self.current_failures.fetch_add(1, Ordering::Relaxed) + 1;

        if failures >= self.failure_threshold {
            let mut state = match self.state.lock() {
                Ok(guard) => guard,
                Err(poisoned) => {
                    log::error!("Circuit state mutex poisoned in record_failure, recovering");
                    poisoned.into_inner()
                }
            };
            *state = CircuitState::Open;

            let mut last_failure = match self.last_failure.lock() {
                Ok(guard) => guard,
                Err(poisoned) => {
                    log::error!("Last failure mutex poisoned, recovering");
                    poisoned.into_inner()
                }
            };
            *last_failure = Some(Instant::now());
        }
    }

    /// Get current state
    pub fn state(&self) -> CircuitState {
        match self.state.lock() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => {
                log::error!("Circuit state mutex poisoned in getter, recovering");
                poisoned.into_inner().clone()
            }
        }
    }
}

/// Load shedding utility for dropping work when overloaded
pub struct LoadShedder {
    /// Maximum queue size before shedding
    max_queue_size: usize,
    /// Current queue size estimate
    current_queue_size: AtomicUsize,
    /// Shed probability (0.0 to 1.0)
    shed_probability: Arc<std::sync::Mutex<f64>>,
    /// Statistics
    shed_count: AtomicU64,
}

impl LoadShedder {
    /// Create new load shedder
    pub fn new(max_queue_size: usize) -> Self {
        Self {
            max_queue_size,
            current_queue_size: AtomicUsize::new(0),
            shed_probability: Arc::new(std::sync::Mutex::new(0.0)),
            shed_count: AtomicU64::new(0),
        }
    }

    /// Check if we should shed this request
    pub fn should_shed(&self) -> bool {
        let queue_size = self.current_queue_size.load(Ordering::Relaxed);

        if queue_size >= self.max_queue_size {
            // Update shed probability based on overload
            let overload_factor = (queue_size as f64) / (self.max_queue_size as f64);
            let shed_prob = (overload_factor - 1.0).max(0.0).min(1.0);

            if let Ok(mut prob) = self.shed_probability.lock() {
                *prob = shed_prob;
            }

            // Probabilistic shedding
            if fastrand::f64() < shed_prob {
                self.shed_count.fetch_add(1, Ordering::Relaxed);
                return true;
            }
        }

        false
    }

    /// Update queue size estimate
    pub fn update_queue_size(&self, size: usize) {
        self.current_queue_size.store(size, Ordering::Relaxed);
    }

    /// Get shed statistics
    pub fn shed_count(&self) -> u64 {
        self.shed_count.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{advance, pause};

    #[tokio::test]
    async fn test_loop_budget_basic() {
        let budget = LoopBudget::new(10); // 10 per second

        // Should allow first 10 iterations
        for _ in 0..10 {
            assert!(budget.can_proceed());
            budget.consume(1);
        }

        // Should block 11th iteration
        assert!(!budget.can_proceed());
    }

    #[tokio::test]
    async fn test_loop_budget_window_reset() {
        pause();
        let budget = LoopBudget::new(5);

        // Consume budget
        for _ in 0..5 {
            budget.consume(1);
        }
        assert!(!budget.can_proceed());

        // Advance time and check reset
        advance(Duration::from_secs(1)).await;
        assert!(budget.can_proceed());
    }

    #[tokio::test]
    async fn test_circuit_breaker() {
        let breaker = CircuitBreaker::new(3, Duration::from_secs(5));

        // Should start closed
        assert!(breaker.allow_request());
        assert_eq!(breaker.state(), CircuitState::Closed);

        // Record failures
        for _ in 0..3 {
            breaker.record_failure();
        }

        // Should be open now
        assert!(!breaker.allow_request());
        assert_eq!(breaker.state(), CircuitState::Open);

        // Record success should close it
        breaker.record_success();
        assert!(breaker.allow_request());
        assert_eq!(breaker.state(), CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_load_shedder() {
        let shedder = LoadShedder::new(100);

        // Should not shed when under capacity
        shedder.update_queue_size(50);
        assert!(!shedder.should_shed());

        // Should shed when over capacity
        shedder.update_queue_size(150);
        // Note: probabilistic, so we can't guarantee it will shed on first try
        let mut shed_count = 0;
        for _ in 0..100 {
            if shedder.should_shed() {
                shed_count += 1;
            }
        }
        assert!(shed_count > 0); // Should have shed some requests
    }
}
