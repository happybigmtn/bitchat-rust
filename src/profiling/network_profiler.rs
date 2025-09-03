use parking_lot::RwLock;
use rustc_hash::FxHashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::interval;

use crate::error::BitCrapsError;
use crate::protocol::PeerId;

/// Network performance profiler with latency and throughput tracking
pub struct NetworkProfiler {
    metrics: Arc<RwLock<NetworkMetrics>>,
    latency_tracker: Arc<RwLock<LatencyTracker>>,
    throughput_tracker: Arc<RwLock<ThroughputTracker>>,
    profiling_active: Arc<RwLock<bool>>,
    sample_interval: Duration,
}

impl NetworkProfiler {
    pub fn new() -> Result<Self, BitCrapsError> {
        Ok(Self {
            metrics: Arc::new(RwLock::new(NetworkMetrics::new())),
            latency_tracker: Arc::new(RwLock::new(LatencyTracker::new())),
            throughput_tracker: Arc::new(RwLock::new(ThroughputTracker::new())),
            profiling_active: Arc::new(RwLock::new(false)),
            sample_interval: Duration::from_millis(500),
        })
    }

    pub async fn start(&mut self) -> Result<(), BitCrapsError> {
        *self.profiling_active.write() = true;

        let metrics = Arc::clone(&self.metrics);
        let profiling_active = Arc::clone(&self.profiling_active);
        let sample_interval = self.sample_interval;

        tokio::spawn(async move {
            let mut interval = interval(sample_interval);

            while *profiling_active.read() {
                interval.tick().await;

                // Update metrics periodically
                let mut metrics = metrics.write();
                metrics.update_timestamp();
            }
        });

        tracing::debug!("Network profiling started");
        Ok(())
    }

    pub async fn stop(&mut self) -> Result<NetworkProfile, BitCrapsError> {
        *self.profiling_active.write() = false;

        tokio::time::sleep(Duration::from_millis(600)).await;

        let metrics = self.metrics.read().clone();
        let latency_stats = self.latency_tracker.read().get_statistics();
        let throughput_stats = self.throughput_tracker.read().get_statistics();

        // Reset for next session
        {
            let mut metrics_guard = self.metrics.write();
            *metrics_guard = NetworkMetrics::new();
        }
        self.latency_tracker.write().reset();
        self.throughput_tracker.write().reset();

        Ok(NetworkProfile {
            total_connections: metrics.total_connections,
            active_connections: metrics.active_connections,
            total_messages_sent: throughput_stats.total_messages_sent,
            total_messages_received: throughput_stats.total_messages_received,
            total_bytes_sent: throughput_stats.total_bytes_sent,
            total_bytes_received: throughput_stats.total_bytes_received,
            average_latency: latency_stats.average_latency,
            p95_latency: latency_stats.p95_latency,
            p99_latency: latency_stats.p99_latency,
            packet_loss_rate: metrics.calculate_packet_loss_rate(),
            throughput_mbps: throughput_stats.calculate_throughput_mbps(),
            connection_errors: metrics.connection_errors,
            timeout_errors: metrics.timeout_errors,
            peer_statistics: latency_stats.peer_statistics,
            profiling_duration: metrics.profiling_duration(),
        })
    }

    pub async fn current_metrics(&self) -> Result<NetworkMetrics, BitCrapsError> {
        Ok(self.metrics.read().clone())
    }

    /// Record a connection event
    pub fn record_connection(&self, peer_id: PeerId, success: bool) {
        let mut metrics = self.metrics.write();

        if success {
            metrics.total_connections += 1;
            metrics.active_connections += 1;
        } else {
            metrics.connection_errors += 1;
        }

        metrics.connection_events.push(ConnectionEvent {
            peer_id,
            event_type: if success {
                ConnectionEventType::Connected
            } else {
                ConnectionEventType::Failed
            },
            timestamp: Instant::now(),
        });

        // Keep only recent events
        if metrics.connection_events.len() > 1000 {
            metrics.connection_events.remove(0);
        }
    }

    /// Record a disconnection event
    pub fn record_disconnection(&self, peer_id: PeerId) {
        let mut metrics = self.metrics.write();

        if metrics.active_connections > 0 {
            metrics.active_connections -= 1;
        }

        metrics.connection_events.push(ConnectionEvent {
            peer_id,
            event_type: ConnectionEventType::Disconnected,
            timestamp: Instant::now(),
        });
    }

    /// Record a message send event
    pub fn record_message_sent(&self, peer_id: PeerId, size_bytes: usize, success: bool) {
        self.throughput_tracker
            .write()
            .record_sent(size_bytes, success);

        if !success {
            let mut metrics = self.metrics.write();
            metrics.send_errors += 1;
        }

        // Track per-peer statistics
        self.latency_tracker.write().update_peer_activity(peer_id);
    }

    /// Record a message receive event
    pub fn record_message_received(&self, peer_id: PeerId, size_bytes: usize) {
        self.throughput_tracker.write().record_received(size_bytes);
        self.latency_tracker.write().update_peer_activity(peer_id);
    }

    /// Record round-trip time for a message
    pub fn record_round_trip_time(&self, peer_id: PeerId, rtt: Duration) {
        self.latency_tracker.write().record_latency(peer_id, rtt);
    }

    /// Record a timeout event
    pub fn record_timeout(&self, peer_id: PeerId) {
        let mut metrics = self.metrics.write();
        metrics.timeout_errors += 1;

        self.latency_tracker.write().record_timeout(peer_id);
    }

    /// Profile network operation
    pub async fn profile_operation<F, R>(
        &mut self,
        peer_id: PeerId,
        operation_name: &str,
        operation: F,
    ) -> Result<(R, NetworkOperationProfile), BitCrapsError>
    where
        F: std::future::Future<Output = R>,
    {
        let start_time = Instant::now();

        let result = operation.await;

        let duration = start_time.elapsed();

        let profile = NetworkOperationProfile {
            peer_id,
            operation_name: operation_name.to_string(),
            duration,
            success: true, // Simplified - in real implementation, would detect failures
        };

        // Update latency tracker
        self.latency_tracker
            .write()
            .record_latency(peer_id, duration);

        Ok((result, profile))
    }
}

/// Network performance metrics
#[derive(Debug, Clone)]
pub struct NetworkMetrics {
    pub total_connections: u64,
    pub active_connections: u64,
    pub connection_errors: u64,
    pub send_errors: u64,
    pub timeout_errors: u64,
    pub connection_events: Vec<ConnectionEvent>,
    pub start_time: Option<Instant>,
    pub last_update: Option<Instant>,
}

impl NetworkMetrics {
    pub fn new() -> Self {
        Self {
            total_connections: 0,
            active_connections: 0,
            connection_errors: 0,
            send_errors: 0,
            timeout_errors: 0,
            connection_events: Vec::new(),
            start_time: Some(Instant::now()),
            last_update: Some(Instant::now()),
        }
    }

    pub fn update_timestamp(&mut self) {
        self.last_update = Some(Instant::now());
    }

    pub fn calculate_packet_loss_rate(&self) -> f64 {
        if self.total_connections == 0 {
            return 0.0;
        }

        let total_errors = self.connection_errors + self.send_errors + self.timeout_errors;
        total_errors as f64 / (self.total_connections as f64 + total_errors as f64)
    }

    pub fn profiling_duration(&self) -> Duration {
        if let (Some(start), Some(last)) = (self.start_time, self.last_update) {
            last - start
        } else {
            Duration::from_nanos(0)
        }
    }
}

/// Connection event tracking
#[derive(Debug, Clone)]
pub struct ConnectionEvent {
    pub peer_id: PeerId,
    pub event_type: ConnectionEventType,
    pub timestamp: Instant,
}

#[derive(Debug, Clone, Copy)]
pub enum ConnectionEventType {
    Connected,
    Disconnected,
    Failed,
}

/// Latency tracking and analysis
pub struct LatencyTracker {
    peer_latencies: FxHashMap<PeerId, Vec<Duration>>,
    peer_timeouts: FxHashMap<PeerId, u32>,
    peer_last_activity: FxHashMap<PeerId, Instant>,
    global_latencies: Vec<Duration>,
}

impl LatencyTracker {
    pub fn new() -> Self {
        Self {
            peer_latencies: FxHashMap::default(),
            peer_timeouts: FxHashMap::default(),
            peer_last_activity: FxHashMap::default(),
            global_latencies: Vec::new(),
        }
    }

    pub fn record_latency(&mut self, peer_id: PeerId, latency: Duration) {
        // Record per-peer latency with memory limit
        let peer_latencies = self
            .peer_latencies
            .entry(peer_id)
            .or_insert_with(|| Vec::with_capacity(100));
        peer_latencies.push(latency);

        // Keep only last 100 entries per peer (was 1000)
        if peer_latencies.len() > 100 {
            // More efficient: use drain to remove first half instead of single element
            peer_latencies.drain(..50);
        }

        // Record global latency with reduced memory footprint
        self.global_latencies.push(latency);
        if self.global_latencies.len() > 5000 {
            // Reduced from 10000
            // More efficient batch removal
            self.global_latencies.drain(..2500);
        }

        // Update activity timestamp
        self.peer_last_activity.insert(peer_id, Instant::now());

        // Cleanup inactive peers periodically (every 1000 entries)
        if self.global_latencies.len() % 1000 == 0 {
            self.cleanup_inactive_peers();
        }
    }

    pub fn record_timeout(&mut self, peer_id: PeerId) {
        *self.peer_timeouts.entry(peer_id).or_insert(0) += 1;
    }

    pub fn update_peer_activity(&mut self, peer_id: PeerId) {
        self.peer_last_activity.insert(peer_id, Instant::now());
    }

    pub fn get_statistics(&self) -> LatencyStatistics {
        let average_latency = if self.global_latencies.is_empty() {
            Duration::from_nanos(0)
        } else {
            let total_nanos: u64 = self
                .global_latencies
                .iter()
                .map(|d| d.as_nanos() as u64)
                .sum();
            Duration::from_nanos(total_nanos / self.global_latencies.len() as u64)
        };

        // Calculate percentiles
        let mut sorted_latencies = self.global_latencies.clone();
        sorted_latencies.sort();

        let p95_latency = if sorted_latencies.is_empty() {
            Duration::from_nanos(0)
        } else {
            let index = (sorted_latencies.len() as f64 * 0.95) as usize;
            sorted_latencies
                .get(index)
                .copied()
                .unwrap_or(Duration::from_nanos(0))
        };

        let p99_latency = if sorted_latencies.is_empty() {
            Duration::from_nanos(0)
        } else {
            let index = (sorted_latencies.len() as f64 * 0.99) as usize;
            sorted_latencies
                .get(index)
                .copied()
                .unwrap_or(Duration::from_nanos(0))
        };

        // Calculate per-peer statistics
        let peer_statistics = self
            .peer_latencies
            .iter()
            .map(|(peer_id, latencies)| {
                let peer_average = if latencies.is_empty() {
                    Duration::from_nanos(0)
                } else {
                    let total_nanos: u64 = latencies.iter().map(|d| d.as_nanos() as u64).sum();
                    Duration::from_nanos(total_nanos / latencies.len() as u64)
                };

                let timeouts = self.peer_timeouts.get(peer_id).copied().unwrap_or(0);
                let last_activity = self.peer_last_activity.get(peer_id).copied();

                PeerLatencyStatistics {
                    peer_id: *peer_id,
                    average_latency: peer_average,
                    sample_count: latencies.len(),
                    timeout_count: timeouts,
                    last_activity,
                }
            })
            .collect();

        LatencyStatistics {
            average_latency,
            p95_latency,
            p99_latency,
            total_samples: self.global_latencies.len(),
            peer_statistics,
        }
    }

    pub fn reset(&mut self) {
        self.peer_latencies.clear();
        self.peer_timeouts.clear();
        self.peer_last_activity.clear();
        self.global_latencies.clear();
    }

    /// Cleanup inactive peers to prevent memory leaks
    fn cleanup_inactive_peers(&mut self) {
        let cutoff = Instant::now() - Duration::from_secs(300); // 5 minutes
        let mut inactive_peers = Vec::new();

        // Find inactive peers
        for (peer_id, last_activity) in &self.peer_last_activity {
            if *last_activity < cutoff {
                inactive_peers.push(*peer_id);
            }
        }

        // Remove inactive peer data
        for peer_id in inactive_peers {
            self.peer_latencies.remove(&peer_id);
            self.peer_timeouts.remove(&peer_id);
            self.peer_last_activity.remove(&peer_id);
        }
    }
}

/// Throughput tracking and analysis
pub struct ThroughputTracker {
    total_bytes_sent: u64,
    total_bytes_received: u64,
    total_messages_sent: u64,
    total_messages_received: u64,
    send_failures: u64,
    start_time: Instant,
    throughput_samples: Vec<ThroughputSample>,
}

impl ThroughputTracker {
    pub fn new() -> Self {
        Self {
            total_bytes_sent: 0,
            total_bytes_received: 0,
            total_messages_sent: 0,
            total_messages_received: 0,
            send_failures: 0,
            start_time: Instant::now(),
            throughput_samples: Vec::new(),
        }
    }

    pub fn record_sent(&mut self, size_bytes: usize, success: bool) {
        if success {
            self.total_bytes_sent += size_bytes as u64;
            self.total_messages_sent += 1;
        } else {
            self.send_failures += 1;
        }

        // Record throughput sample
        self.throughput_samples.push(ThroughputSample {
            bytes: size_bytes,
            direction: ThroughputDirection::Sent,
            success,
            timestamp: Instant::now(),
        });

        // Keep only recent samples with efficient memory management
        if self.throughput_samples.len() > 1000 {
            // Reduced from 5000
            // More efficient batch removal
            self.throughput_samples.drain(..500);
        }
    }

    pub fn record_received(&mut self, size_bytes: usize) {
        self.total_bytes_received += size_bytes as u64;
        self.total_messages_received += 1;

        self.throughput_samples.push(ThroughputSample {
            bytes: size_bytes,
            direction: ThroughputDirection::Received,
            success: true,
            timestamp: Instant::now(),
        });

        // Also apply memory management for received samples
        if self.throughput_samples.len() > 1000 {
            self.throughput_samples.drain(..500);
        }
    }

    pub fn get_statistics(&self) -> ThroughputStatistics {
        let duration = self.start_time.elapsed();

        ThroughputStatistics {
            total_messages_sent: self.total_messages_sent,
            total_messages_received: self.total_messages_received,
            total_bytes_sent: self.total_bytes_sent,
            total_bytes_received: self.total_bytes_received,
            send_failures: self.send_failures,
            duration,
        }
    }

    pub fn reset(&mut self) {
        self.total_bytes_sent = 0;
        self.total_bytes_received = 0;
        self.total_messages_sent = 0;
        self.total_messages_received = 0;
        self.send_failures = 0;
        self.start_time = Instant::now();
        self.throughput_samples.clear();
    }
}

#[derive(Debug, Clone)]
struct ThroughputSample {
    bytes: usize,
    direction: ThroughputDirection,
    success: bool,
    timestamp: Instant,
}

#[derive(Debug, Clone, Copy)]
enum ThroughputDirection {
    Sent,
    Received,
}

/// Complete network profiling results
#[derive(Debug, Clone)]
pub struct NetworkProfile {
    pub total_connections: u64,
    pub active_connections: u64,
    pub total_messages_sent: u64,
    pub total_messages_received: u64,
    pub total_bytes_sent: u64,
    pub total_bytes_received: u64,
    pub average_latency: Duration,
    pub p95_latency: Duration,
    pub p99_latency: Duration,
    pub packet_loss_rate: f64,
    pub throughput_mbps: f64,
    pub connection_errors: u64,
    pub timeout_errors: u64,
    pub peer_statistics: Vec<PeerLatencyStatistics>,
    pub profiling_duration: Duration,
}

/// Latency statistics
#[derive(Debug, Clone)]
pub struct LatencyStatistics {
    pub average_latency: Duration,
    pub p95_latency: Duration,
    pub p99_latency: Duration,
    pub total_samples: usize,
    pub peer_statistics: Vec<PeerLatencyStatistics>,
}

/// Per-peer latency statistics
#[derive(Debug, Clone)]
pub struct PeerLatencyStatistics {
    pub peer_id: PeerId,
    pub average_latency: Duration,
    pub sample_count: usize,
    pub timeout_count: u32,
    pub last_activity: Option<Instant>,
}

/// Throughput statistics
#[derive(Debug, Clone)]
pub struct ThroughputStatistics {
    pub total_messages_sent: u64,
    pub total_messages_received: u64,
    pub total_bytes_sent: u64,
    pub total_bytes_received: u64,
    pub send_failures: u64,
    pub duration: Duration,
}

impl ThroughputStatistics {
    pub fn calculate_throughput_mbps(&self) -> f64 {
        if self.duration.as_secs_f64() == 0.0 {
            return 0.0;
        }

        let total_bytes = self.total_bytes_sent + self.total_bytes_received;
        let bits = (total_bytes * 8) as f64;
        let megabits = bits / 1_000_000.0;

        megabits / self.duration.as_secs_f64()
    }
}

/// Network operation profile
#[derive(Debug, Clone)]
pub struct NetworkOperationProfile {
    pub peer_id: PeerId,
    pub operation_name: String,
    pub duration: Duration,
    pub success: bool,
}
