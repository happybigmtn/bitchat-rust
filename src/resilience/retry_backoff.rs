//! Exponential Backoff and Retry Strategies
//!
//! Provides configurable retry mechanisms with exponential backoff,
//! jitter, and circuit breaker integration for resilient network operations.

use std::time::Duration;
use rand::Rng;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RetryError {
    #[error("Maximum retries exceeded ({0} attempts)")]
    MaxRetriesExceeded(u32),
    #[error("Operation timeout after {0:?}")]
    Timeout(Duration),
    #[error("Operation cancelled")]
    Cancelled,
}

/// Backoff strategy for retry operations
#[derive(Debug, Clone)]
pub enum BackoffStrategy {
    /// Fixed delay between retries
    Fixed(Duration),
    /// Linear increase in delay (base * attempt_number)
    Linear { base: Duration, max: Duration },
    /// Exponential increase in delay (base * 2^attempt)
    Exponential {
        base: Duration,
        max: Duration,
        multiplier: f64,
    },
    /// Fibonacci sequence backoff
    Fibonacci { base: Duration, max: Duration },
    /// Custom backoff function
    Custom(fn(u32) -> Duration),
}

/// Configuration for retry operations
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_attempts: u32,
    /// Backoff strategy to use
    pub backoff_strategy: BackoffStrategy,
    /// Maximum total time for all retries
    pub max_total_duration: Option<Duration>,
    /// Whether to add jitter to backoff delays
    pub jitter: bool,
    /// Jitter factor (0.0 to 1.0)
    pub jitter_factor: f64,
    /// Conditions that should trigger retry
    pub retry_on: Vec<String>,
    /// Conditions that should not trigger retry
    pub dont_retry_on: Vec<String>,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            backoff_strategy: BackoffStrategy::Exponential {
                base: Duration::from_millis(100),
                max: Duration::from_secs(30),
                multiplier: 2.0,
            },
            max_total_duration: Some(Duration::from_secs(60)),
            jitter: true,
            jitter_factor: 0.3,
            retry_on: vec![],
            dont_retry_on: vec![],
        }
    }
}

/// Retry executor with backoff
pub struct RetryExecutor {
    config: RetryConfig,
}

impl RetryExecutor {
    pub fn new(config: RetryConfig) -> Self {
        Self { config }
    }

    /// Execute an async operation with retry logic
    pub async fn execute<F, T, E>(&self, mut operation: F) -> Result<T, RetryError>
    where
        F: FnMut() -> futures::future::BoxFuture<'static, Result<T, E>>,
        E: std::fmt::Display,
    {
        let start_time = std::time::Instant::now();
        let mut attempt = 0;
        let mut last_error = None;

        while attempt < self.config.max_attempts {
            // Check total duration timeout
            if let Some(max_duration) = self.config.max_total_duration {
                if start_time.elapsed() > max_duration {
                    return Err(RetryError::Timeout(max_duration));
                }
            }

            // Execute the operation
            match operation().await {
                Ok(value) => return Ok(value),
                Err(e) => {
                    let error_str = e.to_string();

                    // Check if we should retry this error
                    if !self.should_retry(&error_str) {
                        last_error = Some(error_str);
                        break;
                    }

                    last_error = Some(error_str);
                    attempt += 1;

                    if attempt < self.config.max_attempts {
                        // Calculate backoff delay
                        let delay = self.calculate_backoff(attempt);

                        // Add jitter if configured
                        let delay = if self.config.jitter {
                            self.add_jitter(delay)
                        } else {
                            delay
                        };

                        log::debug!(
                            "Retry attempt {} after {:?} delay (error: {})",
                            attempt, delay, e
                        );

                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        log::error!(
            "Max retries exceeded after {} attempts. Last error: {:?}",
            attempt, last_error
        );
        Err(RetryError::MaxRetriesExceeded(attempt))
    }

    /// Calculate backoff delay based on strategy
    fn calculate_backoff(&self, attempt: u32) -> Duration {
        match &self.config.backoff_strategy {
            BackoffStrategy::Fixed(delay) => *delay,

            BackoffStrategy::Linear { base, max } => {
                let delay = base.saturating_mul(attempt);
                std::cmp::min(delay, *max)
            }

            BackoffStrategy::Exponential { base, max, multiplier } => {
                let multiplier = multiplier.powi(attempt as i32 - 1);
                let delay_ms = (base.as_millis() as f64 * multiplier) as u64;
                let delay = Duration::from_millis(delay_ms);
                std::cmp::min(delay, *max)
            }

            BackoffStrategy::Fibonacci { base, max } => {
                let fib = self.fibonacci(attempt);
                let delay = base.saturating_mul(fib);
                std::cmp::min(delay, *max)
            }

            BackoffStrategy::Custom(f) => f(attempt),
        }
    }

    /// Calculate fibonacci number
    fn fibonacci(&self, n: u32) -> u32 {
        match n {
            0 => 0,
            1 => 1,
            _ => {
                let mut a = 0;
                let mut b = 1;
                for _ in 2..=n {
                    let temp = a + b;
                    a = b;
                    b = temp;
                }
                b
            }
        }
    }

    /// Add jitter to delay
    fn add_jitter(&self, delay: Duration) -> Duration {
        use rand::rngs::OsRng;
        let mut rng = OsRng;
        let jitter_range = (delay.as_millis() as f64 * self.config.jitter_factor) as u64;
        let jitter = rng.gen_range(0..=jitter_range);
        let jittered = if rng.gen_bool(0.5) {
            delay.saturating_add(Duration::from_millis(jitter))
        } else {
            delay.saturating_sub(Duration::from_millis(jitter))
        };
        jittered
    }

    /// Check if error should trigger retry
    fn should_retry(&self, error: &str) -> bool {
        // Check dont_retry_on list first (takes precedence)
        for pattern in &self.config.dont_retry_on {
            if error.contains(pattern) {
                return false;
            }
        }

        // If retry_on is empty, retry all errors
        if self.config.retry_on.is_empty() {
            return true;
        }

        // Check retry_on list
        for pattern in &self.config.retry_on {
            if error.contains(pattern) {
                return true;
            }
        }

        false
    }
}

/// Builder for RetryConfig
pub struct RetryConfigBuilder {
    config: RetryConfig,
}

impl RetryConfigBuilder {
    pub fn new() -> Self {
        Self {
            config: RetryConfig::default(),
        }
    }

    pub fn max_attempts(mut self, attempts: u32) -> Self {
        self.config.max_attempts = attempts;
        self
    }

    pub fn exponential_backoff(mut self, base: Duration, max: Duration) -> Self {
        self.config.backoff_strategy = BackoffStrategy::Exponential {
            base,
            max,
            multiplier: 2.0,
        };
        self
    }

    pub fn linear_backoff(mut self, base: Duration, max: Duration) -> Self {
        self.config.backoff_strategy = BackoffStrategy::Linear { base, max };
        self
    }

    pub fn fixed_backoff(mut self, delay: Duration) -> Self {
        self.config.backoff_strategy = BackoffStrategy::Fixed(delay);
        self
    }

    pub fn with_jitter(mut self, factor: f64) -> Self {
        self.config.jitter = true;
        self.config.jitter_factor = factor.clamp(0.0, 1.0);
        self
    }

    pub fn max_duration(mut self, duration: Duration) -> Self {
        self.config.max_total_duration = Some(duration);
        self
    }

    pub fn retry_on(mut self, patterns: Vec<String>) -> Self {
        self.config.retry_on = patterns;
        self
    }

    pub fn dont_retry_on(mut self, patterns: Vec<String>) -> Self {
        self.config.dont_retry_on = patterns;
        self
    }

    pub fn build(self) -> RetryConfig {
        self.config
    }
}

/// Convenience function for simple retry with exponential backoff
pub async fn retry_with_backoff<F, T, E>(
    operation: F,
    max_attempts: u32,
) -> Result<T, RetryError>
where
    F: FnMut() -> futures::future::BoxFuture<'static, Result<T, E>>,
    E: std::fmt::Display,
{
    let config = RetryConfigBuilder::new()
        .max_attempts(max_attempts)
        .exponential_backoff(Duration::from_millis(100), Duration::from_secs(10))
        .with_jitter(0.3)
        .build();

    let executor = RetryExecutor::new(config);
    executor.execute(operation).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_exponential_backoff() {
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_clone = attempts.clone();

        let config = RetryConfigBuilder::new()
            .max_attempts(3)
            .exponential_backoff(Duration::from_millis(10), Duration::from_secs(1))
            .build();

        let executor = RetryExecutor::new(config);

        let result = executor.execute(|| {
            let attempts = attempts_clone.clone();
            Box::pin(async move {
                let attempt = attempts.fetch_add(1, Ordering::SeqCst);
                if attempt < 2 {
                    Err("Temporary error")
                } else {
                    Ok("Success")
                }
            })
        }).await;

        assert!(result.is_ok());
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_max_retries_exceeded() {
        let config = RetryConfigBuilder::new()
            .max_attempts(2)
            .fixed_backoff(Duration::from_millis(1))
            .build();

        let executor = RetryExecutor::new(config);

        let result: Result<(), RetryError> = executor.execute(|| {
            Box::pin(async { Err("Always fails") })
        }).await;

        assert!(matches!(result, Err(RetryError::MaxRetriesExceeded(2))));
    }

    #[tokio::test]
    async fn test_dont_retry_on_pattern() {
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_clone = attempts.clone();

        let config = RetryConfigBuilder::new()
            .max_attempts(3)
            .dont_retry_on(vec!["Fatal".to_string()])
            .build();

        let executor = RetryExecutor::new(config);

        let result: Result<(), RetryError> = executor.execute(|| {
            let attempts = attempts_clone.clone();
            Box::pin(async move {
                attempts.fetch_add(1, Ordering::SeqCst);
                Err("Fatal error")
            })
        }).await;

        // Should only try once due to "Fatal" in error
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
        assert!(result.is_err());
    }
}