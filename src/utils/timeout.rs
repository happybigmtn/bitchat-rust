//! Timeout utilities for async operations
//!
//! Provides consistent timeout handling across the codebase to prevent
//! indefinite hangs and resource exhaustion.

use crate::config::scalability::ScalabilityConfig;
use std::future::Future;
use std::sync::OnceLock;
use std::time::Duration;
use thiserror::Error;
use tokio::time::{timeout, Timeout};

#[derive(Debug, Error)]
pub enum TimeoutError {
    #[error("Operation timed out after {0:?}")]
    Timeout(Duration),
    #[error("Operation failed: {0}")]
    OperationError(String),
}

/// Global configurable timeout settings
static TIMEOUT_CONFIG: OnceLock<TimeoutConfig> = OnceLock::new();

/// Configurable timeout durations
#[derive(Debug, Clone)]
pub struct TimeoutConfig {
    pub database: Duration,
    pub network: Duration,
    pub consensus: Duration,
    pub file_io: Duration,
    pub lock: Duration,
    pub channel: Duration,
    pub service: Duration,
    pub critical_fast: Duration,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            database: Duration::from_secs(5),
            network: Duration::from_secs(10),
            consensus: Duration::from_secs(30),
            file_io: Duration::from_secs(3),
            lock: Duration::from_secs(1),
            channel: Duration::from_millis(500),
            service: Duration::from_secs(60),
            critical_fast: Duration::from_millis(100),
        }
    }
}

impl TimeoutConfig {
    /// Create from scalability configuration
    pub fn from_scalability_config(config: &ScalabilityConfig) -> Self {
        Self {
            database: config.timeouts.database,
            network: config.timeouts.network,
            consensus: config.timeouts.consensus,
            file_io: config.timeouts.file_io,
            lock: config.timeouts.lock,
            channel: config.timeouts.channel,
            service: config.timeouts.service,
            critical_fast: config.timeouts.critical_fast,
        }
    }

    /// Initialize global timeout configuration
    pub fn init_global(config: Self) {
        let _ = TIMEOUT_CONFIG.set(config);
    }

    /// Get global timeout configuration
    pub fn global() -> &'static TimeoutConfig {
        TIMEOUT_CONFIG.get_or_init(|| TimeoutConfig::default())
    }
}

/// Default timeout durations for different operation types (backwards compatibility)
pub struct TimeoutDefaults;

impl TimeoutDefaults {
    /// Database operations (configurable)
    pub fn database() -> Duration {
        TimeoutConfig::global().database
    }

    /// Network requests (configurable)
    pub fn network() -> Duration {
        TimeoutConfig::global().network
    }

    /// Consensus operations (configurable)
    pub fn consensus() -> Duration {
        TimeoutConfig::global().consensus
    }

    /// File I/O operations (configurable)
    pub fn file_io() -> Duration {
        TimeoutConfig::global().file_io
    }

    /// Lock acquisition (configurable)
    pub fn lock() -> Duration {
        TimeoutConfig::global().lock
    }

    /// Channel operations (configurable)
    pub fn channel() -> Duration {
        TimeoutConfig::global().channel
    }

    /// Service startup/shutdown (configurable)
    pub fn service() -> Duration {
        TimeoutConfig::global().service
    }

    /// Critical operations that should be fast (configurable)
    pub fn critical_fast() -> Duration {
        TimeoutConfig::global().critical_fast
    }

    // Backwards compatibility constants (deprecated)
    #[deprecated(note = "Use TimeoutDefaults::database() instead")]
    pub const DATABASE: Duration = Duration::from_secs(5);

    #[deprecated(note = "Use TimeoutDefaults::network() instead")]
    pub const NETWORK: Duration = Duration::from_secs(10);

    #[deprecated(note = "Use TimeoutDefaults::consensus() instead")]
    pub const CONSENSUS: Duration = Duration::from_secs(30);

    #[deprecated(note = "Use TimeoutDefaults::file_io() instead")]
    pub const FILE_IO: Duration = Duration::from_secs(3);

    #[deprecated(note = "Use TimeoutDefaults::lock() instead")]
    pub const LOCK: Duration = Duration::from_secs(1);

    #[deprecated(note = "Use TimeoutDefaults::channel() instead")]
    pub const CHANNEL: Duration = Duration::from_millis(500);

    #[deprecated(note = "Use TimeoutDefaults::service() instead")]
    pub const SERVICE: Duration = Duration::from_secs(60);

    #[deprecated(note = "Use TimeoutDefaults::critical_fast() instead")]
    pub const CRITICAL_FAST: Duration = Duration::from_millis(100);
}

/// Extension trait for adding timeout to futures
pub trait TimeoutExt: Future {
    /// Add a timeout to this future
    fn with_timeout(self, duration: Duration) -> Timeout<Self>
    where
        Self: Sized,
    {
        timeout(duration, self)
    }

    /// Add a database timeout (configurable)
    fn with_db_timeout(self) -> Timeout<Self>
    where
        Self: Sized,
    {
        self.with_timeout(TimeoutDefaults::database())
    }

    /// Add a network timeout (configurable)
    fn with_network_timeout(self) -> Timeout<Self>
    where
        Self: Sized,
    {
        self.with_timeout(TimeoutDefaults::network())
    }

    /// Add a consensus timeout (configurable)
    fn with_consensus_timeout(self) -> Timeout<Self>
    where
        Self: Sized,
    {
        self.with_timeout(TimeoutDefaults::consensus())
    }

    /// Add a lock timeout (configurable)
    fn with_lock_timeout(self) -> Timeout<Self>
    where
        Self: Sized,
    {
        self.with_timeout(TimeoutDefaults::lock())
    }
}

// Implement for all futures
impl<T: Future> TimeoutExt for T {}

/// Wrapper for operations that should have timeouts
pub struct TimeoutGuard<T> {
    result: Option<T>,
    operation: String,
    started_at: std::time::Instant,
    timeout_duration: Duration,
}

impl<T> TimeoutGuard<T> {
    pub fn new(operation: impl Into<String>, timeout_duration: Duration) -> Self {
        Self {
            result: None,
            operation: operation.into(),
            started_at: std::time::Instant::now(),
            timeout_duration,
        }
    }

    pub async fn execute<F, E>(mut self, future: F) -> Result<T, TimeoutError>
    where
        F: Future<Output = Result<T, E>>,
        E: std::fmt::Display,
    {
        match timeout(self.timeout_duration, future).await {
            Ok(Ok(value)) => {
                self.result = Some(value);
                Ok(self.result.take().unwrap())
            }
            Ok(Err(e)) => Err(TimeoutError::OperationError(e.to_string())),
            Err(_) => {
                log::error!(
                    "Operation '{}' timed out after {:?}",
                    self.operation,
                    self.timeout_duration
                );
                Err(TimeoutError::Timeout(self.timeout_duration))
            }
        }
    }
}

impl<T> Drop for TimeoutGuard<T> {
    fn drop(&mut self) {
        let elapsed = self.started_at.elapsed();
        if elapsed > self.timeout_duration {
            log::warn!(
                "Operation '{}' took {:?} (timeout was {:?})",
                self.operation,
                elapsed,
                self.timeout_duration
            );
        }
    }
}

/// Macro for wrapping async operations with timeout
#[macro_export]
macro_rules! with_timeout {
    ($duration:expr, $op:expr) => {
        tokio::time::timeout($duration, $op)
            .await
            .map_err(|_| $crate::utils::timeout::TimeoutError::Timeout($duration))?
    };

    (db: $op:expr) => {
        with_timeout!($crate::utils::timeout::TimeoutDefaults::database(), $op)
    };

    (network: $op:expr) => {
        with_timeout!($crate::utils::timeout::TimeoutDefaults::network(), $op)
    };

    (consensus: $op:expr) => {
        with_timeout!($crate::utils::timeout::TimeoutDefaults::consensus(), $op)
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_timeout_guard() {
        let guard = TimeoutGuard::<String>::new("test_operation", Duration::from_millis(100));

        let result = guard
            .execute::<_, String>(async {
                tokio::time::sleep(Duration::from_millis(50)).await;
                Ok("success".to_string())
            })
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_timeout_guard_timeout() {
        let guard = TimeoutGuard::<String>::new("slow_operation", Duration::from_millis(50));

        let result = guard
            .execute(async {
                tokio::time::sleep(Duration::from_millis(100)).await;
                Ok::<_, String>("should_timeout".to_string())
            })
            .await;

        assert!(matches!(result, Err(TimeoutError::Timeout(_))));
    }

    #[tokio::test]
    async fn test_timeout_extension() {
        use TimeoutExt;

        let future = async {
            tokio::time::sleep(Duration::from_millis(10)).await;
            42
        };

        let result = future.with_timeout(Duration::from_millis(100)).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }
}
