//! Adaptive Interval Utility for Battery-Efficient Polling
//!
//! This module provides an adaptive polling mechanism that automatically
//! adjusts its interval based on activity levels to minimize battery drain
//! while maintaining responsiveness during active periods.

use std::time::{Duration, Instant};
use tokio::time::{interval, Interval};

/// Configuration for adaptive interval behavior
#[derive(Debug, Clone)]
pub struct AdaptiveIntervalConfig {
    /// Minimum interval (fastest polling rate)
    pub min_interval: Duration,
    /// Maximum interval (slowest polling rate when idle)
    pub max_interval: Duration,
    /// Time to wait without activity before backing off
    pub backoff_threshold: Duration,
    /// Multiplier for exponential backoff
    pub backoff_multiplier: f64,
    /// Time to consider as "recent activity"
    pub activity_window: Duration,
}

impl Default for AdaptiveIntervalConfig {
    fn default() -> Self {
        Self {
            min_interval: Duration::from_millis(100), // Never faster than 100ms
            max_interval: Duration::from_secs(5),     // Slowest is 5 seconds
            backoff_threshold: Duration::from_secs(1), // Start backing off after 1s
            backoff_multiplier: 2.0,                  // Double interval each step
            activity_window: Duration::from_secs(10), // Consider last 10s for activity
        }
    }
}

/// Adaptive interval that adjusts polling rate based on activity
pub struct AdaptiveInterval {
    config: AdaptiveIntervalConfig,
    interval: Interval,
    current_interval: Duration,
    last_activity: Option<Instant>,
    last_backoff_check: Instant,
}

impl AdaptiveInterval {
    /// Create a new adaptive interval with default configuration
    pub fn new() -> Self {
        Self::with_config(AdaptiveIntervalConfig::default())
    }

    /// Create a new adaptive interval with custom configuration
    pub fn with_config(config: AdaptiveIntervalConfig) -> Self {
        let interval = interval(config.min_interval);
        Self {
            current_interval: config.min_interval,
            config,
            interval,
            last_activity: None,
            last_backoff_check: Instant::now(),
        }
    }

    /// Create adaptive interval optimized for consensus operations
    pub fn for_consensus() -> Self {
        Self::with_config(AdaptiveIntervalConfig {
            min_interval: Duration::from_millis(100),
            max_interval: Duration::from_secs(2),
            backoff_threshold: Duration::from_millis(500),
            backoff_multiplier: 1.5,
            activity_window: Duration::from_secs(5),
        })
    }

    /// Create adaptive interval optimized for network operations
    pub fn for_network() -> Self {
        Self::with_config(AdaptiveIntervalConfig {
            min_interval: Duration::from_millis(250),
            max_interval: Duration::from_secs(5),
            backoff_threshold: Duration::from_secs(2),
            backoff_multiplier: 2.0,
            activity_window: Duration::from_secs(15),
        })
    }

    /// Wait for the next tick, automatically adjusting interval based on activity
    pub async fn tick(&mut self) {
        self.interval.tick().await;
        self.adjust_interval();
    }

    /// Signal that activity has occurred, resetting to fast polling
    pub fn signal_activity(&mut self) {
        let now = Instant::now();
        self.last_activity = Some(now);

        // Reset to minimum interval if we're currently slower
        if self.current_interval > self.config.min_interval {
            self.current_interval = self.config.min_interval;
            self.interval = interval(self.current_interval);
            log::debug!(
                "Activity detected - reset to fast polling: {:?}",
                self.current_interval
            );
        }
    }

    /// Check if we should back off and adjust interval accordingly
    fn adjust_interval(&mut self) {
        let now = Instant::now();

        // Only check for backoff periodically to avoid constant adjustments
        if now.duration_since(self.last_backoff_check) < self.config.backoff_threshold {
            return;
        }

        self.last_backoff_check = now;

        // Determine if we should back off based on recent activity
        let should_backoff = match self.last_activity {
            Some(last_activity) => {
                // No recent activity - back off
                now.duration_since(last_activity) > self.config.activity_window
            }
            None => {
                // No activity ever recorded - back off
                true
            }
        };

        if should_backoff && self.current_interval < self.config.max_interval {
            // Apply exponential backoff
            let new_interval = Duration::from_millis(
                (self.current_interval.as_millis() as f64 * self.config.backoff_multiplier) as u64,
            )
            .min(self.config.max_interval);

            if new_interval != self.current_interval {
                log::debug!(
                    "Backing off polling interval: {:?} -> {:?}",
                    self.current_interval,
                    new_interval
                );
                self.current_interval = new_interval;
                self.interval = interval(self.current_interval);
            }
        }
    }

    /// Get the current polling interval
    pub fn current_interval(&self) -> Duration {
        self.current_interval
    }

    /// Get the time since last activity
    pub fn time_since_activity(&self) -> Option<Duration> {
        self.last_activity
            .map(|last| Instant::now().duration_since(last))
    }

    /// Reset the interval to minimum (useful for manual override)
    pub fn reset_to_fast(&mut self) {
        if self.current_interval > self.config.min_interval {
            log::debug!("Manually reset to fast polling");
            self.current_interval = self.config.min_interval;
            self.interval = interval(self.current_interval);
        }
    }

    /// Force interval to maximum (useful for manual override)
    pub fn force_slow(&mut self) {
        if self.current_interval < self.config.max_interval {
            log::debug!("Manually forced to slow polling");
            self.current_interval = self.config.max_interval;
            self.interval = interval(self.current_interval);
        }
    }
}

impl Default for AdaptiveInterval {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_adaptive_interval_starts_fast() {
        let interval = AdaptiveInterval::new();
        assert_eq!(interval.current_interval(), Duration::from_millis(100));
    }

    #[tokio::test]
    async fn test_activity_resets_to_fast() {
        let mut interval = AdaptiveInterval::with_config(AdaptiveIntervalConfig {
            min_interval: Duration::from_millis(100),
            max_interval: Duration::from_secs(1),
            backoff_threshold: Duration::from_millis(100),
            backoff_multiplier: 2.0,
            activity_window: Duration::from_millis(200),
        });

        // Let it back off by waiting without activity
        sleep(Duration::from_millis(500)).await;
        interval.tick().await;

        // Should be slower than minimum
        assert!(interval.current_interval() > Duration::from_millis(100));

        // Signal activity - should reset to fast
        interval.signal_activity();
        assert_eq!(interval.current_interval(), Duration::from_millis(100));
    }

    #[tokio::test]
    async fn test_exponential_backoff() {
        let mut interval = AdaptiveInterval::with_config(AdaptiveIntervalConfig {
            min_interval: Duration::from_millis(100),
            max_interval: Duration::from_secs(2),
            backoff_threshold: Duration::from_millis(50),
            backoff_multiplier: 2.0,
            activity_window: Duration::from_millis(100),
        });

        let initial = interval.current_interval();

        // Wait for backoff to kick in
        sleep(Duration::from_millis(200)).await;
        interval.tick().await;
        sleep(Duration::from_millis(100)).await;
        interval.tick().await;

        // Should have backed off
        assert!(interval.current_interval() > initial);
    }

    #[tokio::test]
    async fn test_consensus_optimized_config() {
        let interval = AdaptiveInterval::for_consensus();
        assert_eq!(interval.current_interval(), Duration::from_millis(100));
        assert_eq!(interval.config.max_interval, Duration::from_secs(2));
    }

    #[tokio::test]
    async fn test_network_optimized_config() {
        let interval = AdaptiveInterval::for_network();
        assert_eq!(interval.current_interval(), Duration::from_millis(250));
        assert_eq!(interval.config.max_interval, Duration::from_secs(5));
    }
}
