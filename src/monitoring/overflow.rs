//! Overflow handling for broadcast channels and queues
//!
//! Provides utilities to handle channel overflow gracefully rather than
//! dropping messages silently or blocking indefinitely.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, mpsc};
use tokio::time::interval;

/// Overflow handling strategy for channels
#[derive(Debug, Clone, Copy)]
pub enum OverflowStrategy {
    /// Drop oldest messages when full
    DropOldest,
    /// Block until space is available
    Block,
    /// Return an error when full
    Error,
    /// Drop newest (incoming) message
    DropNewest,
}

/// Statistics for overflow monitoring
#[derive(Debug, Clone)]
pub struct OverflowStats {
    pub total_sent: u64,
    pub total_dropped: u64,
    pub total_blocked: u64,
    pub current_size: usize,
    pub max_size: usize,
    pub last_overflow: Option<Instant>,
}

/// Wrapper around broadcast::Sender with overflow handling
pub struct BoundedBroadcaster<T> {
    sender: broadcast::Sender<T>,
    strategy: OverflowStrategy,
    stats: Arc<AtomicU64>, // Simple counter for dropped messages
    max_size: usize,
}

impl<T: Clone> BoundedBroadcaster<T> {
    pub fn new(capacity: usize, strategy: OverflowStrategy) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self {
            sender,
            strategy,
            stats: Arc::new(AtomicU64::new(0)),
            max_size: capacity,
        }
    }

    /// Send a message with overflow handling
    pub fn send(&self, message: T) -> Result<usize, BroadcastError<T>> {
        match self.sender.send(message) {
            Ok(receiver_count) => Ok(receiver_count),
            Err(broadcast::error::SendError(msg)) => {
                match self.strategy {
                    OverflowStrategy::DropOldest => {
                        // For broadcast channels, we can't easily drop oldest
                        // This is a limitation of tokio's broadcast channel
                        self.stats.fetch_add(1, Ordering::Relaxed);
                        log::warn!("Broadcast channel full, dropping message");
                        Err(BroadcastError::Dropped(msg))
                    }
                    OverflowStrategy::Block => {
                        // Can't block in broadcast channels - they don't have backpressure
                        log::error!("Block strategy not supported for broadcast channels");
                        Err(BroadcastError::Dropped(msg))
                    }
                    OverflowStrategy::Error => {
                        Err(BroadcastError::Full(msg))
                    }
                    OverflowStrategy::DropNewest => {
                        self.stats.fetch_add(1, Ordering::Relaxed);
                        log::debug!("Dropping newest message due to full channel");
                        Err(BroadcastError::Dropped(msg))
                    }
                }
            }
        }
    }

    /// Try to send without blocking
    pub fn try_send(&self, message: T) -> Result<usize, BroadcastError<T>> {
        self.send(message)
    }

    /// Get a receiver for this broadcaster
    pub fn subscribe(&self) -> broadcast::Receiver<T> {
        self.sender.subscribe()
    }

    /// Get current statistics
    pub fn stats(&self) -> OverflowStats {
        OverflowStats {
            total_sent: 0, // broadcast::Sender doesn't track this
            total_dropped: self.stats.load(Ordering::Relaxed),
            total_blocked: 0,
            current_size: 0, // broadcast::Sender doesn't expose current size
            max_size: self.max_size,
            last_overflow: None, // Would need to track this separately
        }
    }
}

/// Error types for broadcast operations
#[derive(Debug)]
pub enum BroadcastError<T> {
    /// Channel is full
    Full(T),
    /// Message was dropped due to overflow
    Dropped(T),
    /// No receivers available
    NoReceivers,
}

/// Overflow-aware MPSC sender wrapper
pub struct BoundedMpsc<T> {
    sender: mpsc::Sender<T>,
    strategy: OverflowStrategy,
    stats: OverflowCounter,
}

struct OverflowCounter {
    sent: AtomicU64,
    dropped: AtomicU64,
    blocked: AtomicU64,
}

impl<T> BoundedMpsc<T> {
    pub fn new(capacity: usize, strategy: OverflowStrategy) -> (Self, mpsc::Receiver<T>) {
        let (sender, receiver) = mpsc::channel(capacity);
        let bounded = Self {
            sender,
            strategy,
            stats: OverflowCounter {
                sent: AtomicU64::new(0),
                dropped: AtomicU64::new(0),
                blocked: AtomicU64::new(0),
            },
        };
        (bounded, receiver)
    }

    /// Send a message with overflow handling
    pub async fn send(&self, message: T) -> Result<(), MpscError<T>> {
        match self.strategy {
            OverflowStrategy::Block => {
                match self.sender.send(message).await {
                    Ok(()) => {
                        self.stats.sent.fetch_add(1, Ordering::Relaxed);
                        Ok(())
                    }
                    Err(mpsc::error::SendError(msg)) => {
                        Err(MpscError::Closed(msg))
                    }
                }
            }
            _ => {
                match self.sender.try_send(message) {
                    Ok(()) => {
                        self.stats.sent.fetch_add(1, Ordering::Relaxed);
                        Ok(())
                    }
                    Err(mpsc::error::TrySendError::Full(msg)) => {
                        match self.strategy {
                            OverflowStrategy::DropNewest => {
                                self.stats.dropped.fetch_add(1, Ordering::Relaxed);
                                log::debug!("Dropping message due to full MPSC channel");
                                Err(MpscError::Dropped(msg))
                            }
                            OverflowStrategy::Error => {
                                Err(MpscError::Full(msg))
                            }
                            _ => {
                                // DropOldest not easily supported in MPSC
                                self.stats.dropped.fetch_add(1, Ordering::Relaxed);
                                Err(MpscError::Dropped(msg))
                            }
                        }
                    }
                    Err(mpsc::error::TrySendError::Closed(msg)) => {
                        Err(MpscError::Closed(msg))
                    }
                }
            }
        }
    }

    /// Try to send without waiting
    pub fn try_send(&self, message: T) -> Result<(), MpscError<T>> {
        match self.sender.try_send(message) {
            Ok(()) => {
                self.stats.sent.fetch_add(1, Ordering::Relaxed);
                Ok(())
            }
            Err(mpsc::error::TrySendError::Full(msg)) => {
                match self.strategy {
                    OverflowStrategy::DropNewest => {
                        self.stats.dropped.fetch_add(1, Ordering::Relaxed);
                        Err(MpscError::Dropped(msg))
                    }
                    _ => Err(MpscError::Full(msg))
                }
            }
            Err(mpsc::error::TrySendError::Closed(msg)) => {
                Err(MpscError::Closed(msg))
            }
        }
    }

    /// Get current statistics
    pub fn stats(&self) -> OverflowStats {
        OverflowStats {
            total_sent: self.stats.sent.load(Ordering::Relaxed),
            total_dropped: self.stats.dropped.load(Ordering::Relaxed),
            total_blocked: self.stats.blocked.load(Ordering::Relaxed),
            current_size: 0, // mpsc::Sender doesn't expose this
            max_size: 0, // Would need to store separately
            last_overflow: None, // Would need to track separately
        }
    }
}

/// Error types for MPSC operations
#[derive(Debug)]
pub enum MpscError<T> {
    /// Channel is full
    Full(T),
    /// Message was dropped due to overflow
    Dropped(T),
    /// Channel is closed
    Closed(T),
}

/// Overflow monitor that can watch multiple channels
pub struct OverflowMonitor {
    broadcasters: Vec<Arc<dyn OverflowStatsProvider>>,
    alert_threshold: f64, // Percentage of capacity
}

pub trait OverflowStatsProvider: Send + Sync {
    fn get_stats(&self) -> OverflowStats;
    fn name(&self) -> &str;
}

impl OverflowMonitor {
    pub fn new(alert_threshold: f64) -> Self {
        Self {
            broadcasters: Vec::new(),
            alert_threshold,
        }
    }

    pub fn add_channel<T: OverflowStatsProvider + 'static>(&mut self, channel: Arc<T>) {
        self.broadcasters.push(channel);
    }

    /// Start monitoring channels for overflow
    pub async fn start_monitoring(&self) -> tokio::task::JoinHandle<()> {
        let broadcasters = self.broadcasters.clone();
        let threshold = self.alert_threshold;

        spawn_tracked("task", TaskType::Maintenance, async move {
            let mut interval = interval(Duration::from_secs(10));

            loop {
                interval.tick().await;

                for broadcaster in &broadcasters {
                    let stats = broadcaster.get_stats();

                    // Check for high drop rate
                    if stats.total_sent > 0 {
                        let drop_rate = stats.total_dropped as f64 / stats.total_sent as f64;
                        if drop_rate > threshold {
                            log::warn!(
                                "High message drop rate in channel '{}': {:.2}% ({} dropped / {} sent)",
                                broadcaster.name(),
                                drop_rate * 100.0,
                                stats.total_dropped,
                                stats.total_sent
                            );
                        }
                    }

                    // Log current stats periodically
                    if stats.total_dropped > 0 {
                        log::info!(
                            "Channel '{}' stats: {} sent, {} dropped",
                            broadcaster.name(),
                            stats.total_sent,
                            stats.total_dropped
                        );
                    }
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;
use crate::utils::task_tracker::{spawn_tracked, TaskType};

    #[tokio::test]
    async fn test_bounded_broadcaster() {
        let broadcaster = BoundedBroadcaster::new(2, OverflowStrategy::DropNewest);
        let mut rx1 = broadcaster.subscribe();
        let mut rx2 = broadcaster.subscribe();

        // Send within capacity
        assert!(broadcaster.send("msg1".to_string()).is_ok());
        assert!(broadcaster.send("msg2".to_string()).is_ok());

        // This should drop due to full channel
        let result = broadcaster.send("msg3".to_string());
        assert!(matches!(result, Err(BroadcastError::Dropped(_))));

        // Receivers should still get the first messages
        assert_eq!(rx1.recv().await.unwrap(), "msg1");
        assert_eq!(rx2.recv().await.unwrap(), "msg1");
    }

    #[tokio::test]
    async fn test_bounded_mpsc() {
        let (sender, mut receiver) = BoundedMpsc::new(2, OverflowStrategy::DropNewest);

        // Send within capacity
        assert!(sender.try_send("msg1".to_string()).is_ok());
        assert!(sender.try_send("msg2".to_string()).is_ok());

        // This should drop
        let result = sender.try_send("msg3".to_string());
        assert!(matches!(result, Err(MpscError::Dropped(_))));

        // Receive messages
        assert_eq!(receiver.recv().await.unwrap(), "msg1");
        assert_eq!(receiver.recv().await.unwrap(), "msg2");

        // No third message should be available
        sleep(Duration::from_millis(10)).await;
        assert!(receiver.try_recv().is_err());
    }
}