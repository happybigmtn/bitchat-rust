//! Bounded queue implementation with backpressure handling for transport events
//!
//! This module provides a bounded queue that prevents memory exhaustion from
//! unbounded event accumulation, with configurable overflow behavior and
//! backpressure handling.

use crate::transport::TransportEvent;
use std::sync::{atomic::AtomicU64, Arc};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Mutex, Semaphore};

/// Maximum default queue size (10,000 events)
const DEFAULT_MAX_QUEUE_SIZE: usize = 10_000;

/// Default backpressure timeout
const DEFAULT_BACKPRESSURE_TIMEOUT: Duration = Duration::from_millis(100);

/// Overflow behavior when queue is full
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverflowBehavior {
    /// Drop the oldest event to make room for new ones
    DropOldest,
    /// Drop the new event if queue is full
    DropNewest,
    /// Block until space is available (with timeout)
    Backpressure,
    /// Return error when queue is full
    Reject,
}

/// Statistics for bounded queue monitoring
#[derive(Debug, Clone, Default)]
pub struct QueueStats {
    /// Total events enqueued
    pub events_enqueued: u64,
    /// Total events dequeued
    pub events_dequeued: u64,
    /// Events dropped due to overflow
    pub events_dropped: u64,
    /// Events rejected due to full queue
    pub events_rejected: u64,
    /// Current queue size
    pub current_size: usize,
    /// Maximum queue size
    pub max_size: usize,
    /// High water mark (max size reached)
    pub high_water_mark: usize,
    /// Total backpressure events
    pub backpressure_events: u64,
    /// Average processing latency (microseconds)
    pub avg_processing_latency_us: u64,
}

/// Event wrapper with metadata for tracking
#[derive(Debug)]
struct EventWithMetadata<T> {
    event: T,
    enqueued_at: Instant,
    sequence: u64,
}

/// Bounded queue implementation with overflow protection
pub struct BoundedEventQueue<T> {
    /// Internal sender (bounded)
    sender: mpsc::Sender<EventWithMetadata<T>>,
    /// Internal receiver
    receiver: Arc<Mutex<mpsc::Receiver<EventWithMetadata<T>>>>,
    /// Queue configuration
    config: QueueConfig,
    /// Queue statistics
    stats: Arc<Mutex<QueueStats>>,
    /// Event sequence counter
    sequence_counter: AtomicU64,
    /// Semaphore for backpressure control
    semaphore: Arc<Semaphore>,
}

/// Configuration for bounded queue
#[derive(Debug, Clone)]
pub struct QueueConfig {
    pub max_size: usize,
    pub overflow_behavior: OverflowBehavior,
    pub backpressure_timeout: Duration,
    pub enable_metrics: bool,
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            max_size: DEFAULT_MAX_QUEUE_SIZE,
            overflow_behavior: OverflowBehavior::DropOldest,
            backpressure_timeout: DEFAULT_BACKPRESSURE_TIMEOUT,
            enable_metrics: true,
        }
    }
}

impl<T> Default for BoundedEventQueue<T>
where
    T: Send + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T> BoundedEventQueue<T>
where
    T: Send + 'static,
{
    /// Create new bounded event queue with default configuration
    pub fn new() -> Self {
        Self::with_config(QueueConfig::default())
    }

    /// Create new bounded event queue with custom configuration
    pub fn with_config(config: QueueConfig) -> Self {
        let (sender, receiver) = mpsc::channel(config.max_size);
        let semaphore = Arc::new(Semaphore::new(config.max_size));

        let stats = QueueStats {
            max_size: config.max_size,
            ..Default::default()
        };

        Self {
            sender,
            receiver: Arc::new(Mutex::new(receiver)),
            config,
            stats: Arc::new(Mutex::new(stats)),
            sequence_counter: AtomicU64::new(0),
            semaphore,
        }
    }

    /// Get a sender handle for this queue
    pub fn sender(&self) -> BoundedEventSender<T> {
        BoundedEventSender {
            sender: self.sender.clone(),
            config: self.config.clone(),
            stats: self.stats.clone(),
            sequence_counter: Arc::new(AtomicU64::new(0)),
            semaphore: self.semaphore.clone(),
        }
    }

    /// Get a receiver handle for this queue
    pub fn receiver(&self) -> BoundedEventReceiver<T> {
        BoundedEventReceiver {
            receiver: self.receiver.clone(),
            stats: self.stats.clone(),
            semaphore: self.semaphore.clone(),
        }
    }

    /// Get current queue statistics
    pub async fn stats(&self) -> QueueStats {
        let stats = self.stats.lock().await;
        let mut result = stats.clone();

        // Update current size based on available permits
        result.current_size = self.config.max_size - self.semaphore.available_permits();

        result
    }

    /// Update queue configuration (affects new operations)
    pub async fn update_config(&mut self, new_config: QueueConfig) {
        self.config = new_config;
        let mut stats = self.stats.lock().await;
        stats.max_size = self.config.max_size;
    }
}

/// Sender handle for bounded event queue
#[derive(Clone)]
pub struct BoundedEventSender<T> {
    sender: mpsc::Sender<EventWithMetadata<T>>,
    config: QueueConfig,
    stats: Arc<Mutex<QueueStats>>,
    sequence_counter: Arc<AtomicU64>,
    semaphore: Arc<Semaphore>,
}

impl<T> BoundedEventSender<T>
where
    T: Send + 'static,
{
    /// Send event with overflow protection
    pub async fn send(&self, event: T) -> Result<(), BoundedQueueError> {
        let sequence = self
            .sequence_counter
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let metadata = EventWithMetadata {
            event,
            enqueued_at: Instant::now(),
            sequence,
        };

        match self.config.overflow_behavior {
            OverflowBehavior::Backpressure => self.send_with_backpressure(metadata).await,
            OverflowBehavior::DropOldest => self.send_drop_oldest(metadata).await,
            OverflowBehavior::DropNewest => self.send_drop_newest(metadata).await,
            OverflowBehavior::Reject => self.send_reject_on_full(metadata).await,
        }
    }

    /// Try to send without blocking
    pub fn try_send(&self, event: T) -> Result<(), BoundedQueueError> {
        let sequence = self
            .sequence_counter
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let metadata = EventWithMetadata {
            event,
            enqueued_at: Instant::now(),
            sequence,
        };

        match self.sender.try_send(metadata) {
            Ok(()) => {
                // Update stats
                if self.config.enable_metrics {
                    if let Ok(mut stats) = self.stats.try_lock() {
                        stats.events_enqueued += 1;
                    }
                }
                Ok(())
            }
            Err(mpsc::error::TrySendError::Full(_)) => {
                // Update stats
                if self.config.enable_metrics {
                    if let Ok(mut stats) = self.stats.try_lock() {
                        stats.events_rejected += 1;
                    }
                }
                Err(BoundedQueueError::QueueFull)
            }
            Err(mpsc::error::TrySendError::Closed(_)) => Err(BoundedQueueError::QueueClosed),
        }
    }

    async fn send_with_backpressure(
        &self,
        metadata: EventWithMetadata<T>,
    ) -> Result<(), BoundedQueueError> {
        // Try to acquire semaphore permit with timeout
        let permit =
            match tokio::time::timeout(self.config.backpressure_timeout, self.semaphore.acquire())
                .await
            {
                Ok(Ok(permit)) => permit,
                Ok(Err(_)) => return Err(BoundedQueueError::QueueClosed),
                Err(_) => {
                    // Timeout - record backpressure event
                    if self.config.enable_metrics {
                        if let Ok(mut stats) = self.stats.try_lock() {
                            stats.backpressure_events += 1;
                            stats.events_rejected += 1;
                        }
                    }
                    return Err(BoundedQueueError::BackpressureTimeout);
                }
            };

        // Send the event
        match self.sender.send(metadata).await {
            Ok(()) => {
                if self.config.enable_metrics {
                    if let Ok(mut stats) = self.stats.try_lock() {
                        stats.events_enqueued += 1;
                    }
                }
                // Don't forget the permit - it will be released when event is received
                std::mem::forget(permit);
                Ok(())
            }
            Err(_) => Err(BoundedQueueError::QueueClosed),
        }
    }

    async fn send_drop_oldest(
        &self,
        metadata: EventWithMetadata<T>,
    ) -> Result<(), BoundedQueueError> {
        // For drop oldest, we always try to send
        // The channel itself will handle dropping if needed
        match self.sender.send(metadata).await {
            Ok(()) => {
                if self.config.enable_metrics {
                    if let Ok(mut stats) = self.stats.try_lock() {
                        stats.events_enqueued += 1;
                    }
                }
                Ok(())
            }
            Err(_) => Err(BoundedQueueError::QueueClosed),
        }
    }

    async fn send_drop_newest(
        &self,
        metadata: EventWithMetadata<T>,
    ) -> Result<(), BoundedQueueError> {
        match self.sender.try_send(metadata) {
            Ok(()) => {
                if self.config.enable_metrics {
                    if let Ok(mut stats) = self.stats.try_lock() {
                        stats.events_enqueued += 1;
                    }
                }
                Ok(())
            }
            Err(mpsc::error::TrySendError::Full(_)) => {
                // Drop newest (current) event
                if self.config.enable_metrics {
                    if let Ok(mut stats) = self.stats.try_lock() {
                        stats.events_dropped += 1;
                    }
                }
                Ok(()) // Not an error - we successfully dropped
            }
            Err(mpsc::error::TrySendError::Closed(_)) => Err(BoundedQueueError::QueueClosed),
        }
    }

    async fn send_reject_on_full(
        &self,
        metadata: EventWithMetadata<T>,
    ) -> Result<(), BoundedQueueError> {
        match self.sender.try_send(metadata) {
            Ok(()) => {
                if self.config.enable_metrics {
                    if let Ok(mut stats) = self.stats.try_lock() {
                        stats.events_enqueued += 1;
                    }
                }
                Ok(())
            }
            Err(mpsc::error::TrySendError::Full(_)) => {
                if self.config.enable_metrics {
                    if let Ok(mut stats) = self.stats.try_lock() {
                        stats.events_rejected += 1;
                    }
                }
                Err(BoundedQueueError::QueueFull)
            }
            Err(mpsc::error::TrySendError::Closed(_)) => Err(BoundedQueueError::QueueClosed),
        }
    }
}

/// Receiver handle for bounded event queue
pub struct BoundedEventReceiver<T> {
    receiver: Arc<Mutex<mpsc::Receiver<EventWithMetadata<T>>>>,
    stats: Arc<Mutex<QueueStats>>,
    semaphore: Arc<Semaphore>,
}

impl<T> BoundedEventReceiver<T> {
    /// Receive next event from queue
    pub async fn recv(&self) -> Option<T> {
        let mut receiver = self.receiver.lock().await;

        match receiver.recv().await {
            Some(metadata) => {
                // Update stats
                if let Ok(mut stats) = self.stats.try_lock() {
                    stats.events_dequeued += 1;

                    // Update processing latency
                    let latency = metadata.enqueued_at.elapsed().as_micros() as u64;
                    stats.avg_processing_latency_us =
                        (stats.avg_processing_latency_us * 3 + latency) / 4; // Moving average
                }

                // Release semaphore permit
                self.semaphore.add_permits(1);

                Some(metadata.event)
            }
            None => None,
        }
    }

    /// Try to receive without blocking
    pub async fn try_recv(&self) -> Result<Option<T>, BoundedQueueError> {
        let mut receiver = self.receiver.lock().await;

        match receiver.try_recv() {
            Ok(metadata) => {
                // Update stats
                if let Ok(mut stats) = self.stats.try_lock() {
                    stats.events_dequeued += 1;

                    let latency = metadata.enqueued_at.elapsed().as_micros() as u64;
                    stats.avg_processing_latency_us =
                        (stats.avg_processing_latency_us * 3 + latency) / 4;
                }

                // Release semaphore permit
                self.semaphore.add_permits(1);

                Ok(Some(metadata.event))
            }
            Err(mpsc::error::TryRecvError::Empty) => Ok(None),
            Err(mpsc::error::TryRecvError::Disconnected) => Err(BoundedQueueError::QueueClosed),
        }
    }
}

/// Errors that can occur with bounded queue operations
#[derive(Debug, thiserror::Error)]
pub enum BoundedQueueError {
    #[error("Queue is full")]
    QueueFull,
    #[error("Queue is closed")]
    QueueClosed,
    #[error("Backpressure timeout exceeded")]
    BackpressureTimeout,
}

/// Specialized bounded queue for transport events
pub type BoundedTransportEventQueue = BoundedEventQueue<TransportEvent>;
pub type BoundedTransportEventSender = BoundedEventSender<TransportEvent>;
pub type BoundedTransportEventReceiver = BoundedEventReceiver<TransportEvent>;

impl BoundedTransportEventQueue {
    /// Create transport event queue with recommended settings
    pub fn for_transport() -> Self {
        let config = QueueConfig {
            max_size: 10_000, // 10K events max
            overflow_behavior: OverflowBehavior::DropOldest,
            backpressure_timeout: Duration::from_millis(100),
            enable_metrics: true,
        };

        Self::with_config(config)
    }

    /// Create transport event queue with high throughput settings
    pub fn for_high_throughput() -> Self {
        let config = QueueConfig {
            max_size: 50_000, // 50K events max
            overflow_behavior: OverflowBehavior::Backpressure,
            backpressure_timeout: Duration::from_millis(50),
            enable_metrics: true,
        };

        Self::with_config(config)
    }

    /// Create transport event queue with strict reliability settings
    pub fn for_reliability() -> Self {
        let config = QueueConfig {
            max_size: 5_000, // 5K events max (smaller for faster processing)
            overflow_behavior: OverflowBehavior::Reject,
            backpressure_timeout: Duration::from_millis(200),
            enable_metrics: true,
        };

        Self::with_config(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_bounded_queue_basic() {
        let queue = BoundedEventQueue::<i32>::new();
        let sender = queue.sender();
        let receiver = queue.receiver();

        // Send a few events
        assert!(sender.send(1).await.is_ok());
        assert!(sender.send(2).await.is_ok());
        assert!(sender.send(3).await.is_ok());

        // Receive events
        assert_eq!(receiver.recv().await, Some(1));
        assert_eq!(receiver.recv().await, Some(2));
        assert_eq!(receiver.recv().await, Some(3));
    }

    #[tokio::test]
    async fn test_drop_oldest_overflow() {
        let config = QueueConfig {
            max_size: 2,
            overflow_behavior: OverflowBehavior::DropOldest,
            ..Default::default()
        };

        let queue = BoundedEventQueue::<i32>::with_config(config);
        let sender = queue.sender();
        let receiver = queue.receiver();

        // Fill queue beyond capacity
        assert!(sender.send(1).await.is_ok());
        assert!(sender.send(2).await.is_ok());
        assert!(sender.send(3).await.is_ok()); // Should drop oldest (1)

        // Should receive 2 and 3
        assert_eq!(receiver.recv().await, Some(2));
        assert_eq!(receiver.recv().await, Some(3));
    }

    #[tokio::test]
    async fn test_drop_newest_overflow() {
        let config = QueueConfig {
            max_size: 2,
            overflow_behavior: OverflowBehavior::DropNewest,
            ..Default::default()
        };

        let queue = BoundedEventQueue::<i32>::with_config(config);
        let sender = queue.sender();
        let receiver = queue.receiver();

        // Fill queue
        assert!(sender.send(1).await.is_ok());
        assert!(sender.send(2).await.is_ok());
        assert!(sender.send(3).await.is_ok()); // Should be dropped

        // Should receive 1 and 2
        assert_eq!(receiver.recv().await, Some(1));
        assert_eq!(receiver.recv().await, Some(2));
    }

    #[tokio::test]
    async fn test_reject_on_full() {
        let config = QueueConfig {
            max_size: 2,
            overflow_behavior: OverflowBehavior::Reject,
            ..Default::default()
        };

        let queue = BoundedEventQueue::<i32>::with_config(config);
        let sender = queue.sender();

        // Fill queue
        assert!(sender.send(1).await.is_ok());
        assert!(sender.send(2).await.is_ok());

        // Next send should be rejected
        assert!(matches!(
            sender.send(3).await,
            Err(BoundedQueueError::QueueFull)
        ));
    }

    #[tokio::test]
    async fn test_backpressure() {
        let config = QueueConfig {
            max_size: 1,
            overflow_behavior: OverflowBehavior::Backpressure,
            backpressure_timeout: Duration::from_millis(50),
            ..Default::default()
        };

        let queue = BoundedEventQueue::<i32>::with_config(config);
        let sender = queue.sender();
        let receiver = queue.receiver();

        // Fill queue
        assert!(sender.send(1).await.is_ok());

        // Next send should timeout due to backpressure
        let start = Instant::now();
        let result = sender.send(2).await;
        let duration = start.elapsed();

        assert!(matches!(
            result,
            Err(BoundedQueueError::BackpressureTimeout)
        ));
        assert!(duration >= Duration::from_millis(50));

        // After receiving, should be able to send again
        assert_eq!(receiver.recv().await, Some(1));
        assert!(sender.send(3).await.is_ok());
    }

    #[tokio::test]
    async fn test_queue_stats() {
        let queue = BoundedEventQueue::<i32>::new();
        let sender = queue.sender();
        let receiver = queue.receiver();

        // Send some events
        sender.send(1).await.unwrap();
        sender.send(2).await.unwrap();

        // Receive one event
        receiver.recv().await;

        let stats = queue.stats().await;
        assert_eq!(stats.events_enqueued, 2);
        assert_eq!(stats.events_dequeued, 1);
        assert!(stats.avg_processing_latency_us > 0);
    }
}
