use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use parking_lot::{Mutex, RwLock};
use rustc_hash::FxHashMap;
use tokio::sync::Semaphore;
use bytes::Bytes;
use lz4_flex::{compress_prepend_size, decompress_size_prepended};
use zstd;
use brotli;

use crate::protocol::{PeerId, P2PMessage};
use crate::transport::TransportError;

/// Network optimization manager with adaptive protocols
pub struct NetworkOptimizer {
    /// Connection quality metrics per peer
    peer_metrics: Arc<RwLock<FxHashMap<PeerId, PeerMetrics>>>,
    /// Message batching queues per peer
    batch_queues: Arc<RwLock<FxHashMap<PeerId, BatchQueue>>>,
    /// Compression algorithms by effectiveness
    compression_stats: Arc<RwLock<CompressionStats>>,
    /// Rate limiting
    rate_limiter: Arc<Semaphore>,
    /// Network congestion detector
    congestion_detector: CongestionDetector,
    /// Configuration
    config: NetworkOptimizerConfig,
}

#[derive(Debug, Clone)]
pub struct NetworkOptimizerConfig {
    /// Maximum batch size before forced send
    pub max_batch_size: usize,
    /// Maximum batch wait time before forced send
    pub max_batch_delay: Duration,
    /// Minimum message size for compression
    pub compression_threshold: usize,
    /// Maximum concurrent connections
    pub max_connections: usize,
    /// Connection timeout
    pub connection_timeout: Duration,
    /// Retry configuration
    pub max_retries: u32,
    pub retry_backoff: Duration,
}

impl Default for NetworkOptimizerConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 10,
            max_batch_delay: Duration::from_millis(50),
            compression_threshold: 1024,
            max_connections: 100,
            connection_timeout: Duration::from_secs(30),
            max_retries: 3,
            retry_backoff: Duration::from_millis(100),
        }
    }
}

impl NetworkOptimizer {
    pub fn new(config: NetworkOptimizerConfig) -> Self {
        let rate_limiter = Arc::new(Semaphore::new(config.max_connections));
        
        Self {
            peer_metrics: Arc::new(RwLock::new(FxHashMap::default())),
            batch_queues: Arc::new(RwLock::new(FxHashMap::default())),
            compression_stats: Arc::new(RwLock::new(CompressionStats::new())),
            rate_limiter,
            congestion_detector: CongestionDetector::new(),
            config,
        }
    }
    
    /// Optimize message for transmission to specific peer
    pub async fn optimize_message(&self, peer_id: &PeerId, message: P2PMessage) -> OptimizedMessage {
        // Get peer metrics for optimization decisions
        let peer_metrics = self.get_peer_metrics(peer_id);
        
        // Serialize payload for optimization
        let payload_bytes = bincode::serialize(&message.payload).unwrap_or_default();
        
        // Decide on compression based on peer connection quality and message size
        let optimized_payload = self.optimize_payload(&payload_bytes, &peer_metrics).await;
        
        // Apply batching if beneficial
        let should_batch = self.should_batch_message(&peer_metrics, &message);
        
        let compression_type = optimized_payload.compression_type;
        let transmission_time = self.estimate_transmission_time(&peer_metrics, &optimized_payload);
        
        OptimizedMessage {
            peer_id: *peer_id,
            original_message: message,
            optimized_payload,
            should_batch,
            compression_used: compression_type,
            estimated_transmission_time: transmission_time,
        }
    }
    
    /// Optimize payload based on peer connection quality
    async fn optimize_payload(&self, payload: &[u8], peer_metrics: &PeerMetrics) -> OptimizedPayload {
        if payload.len() < self.config.compression_threshold {
            return OptimizedPayload {
                data: Bytes::copy_from_slice(payload),
                compression_type: CompressionType::None,
                original_size: payload.len(),
                compressed_size: payload.len(),
                compression_ratio: 1.0,
            };
        }
        
        // Choose compression algorithm based on peer connection quality
        let compression_type = self.choose_compression_algorithm(peer_metrics, payload.len());
        
        match compression_type {
            CompressionType::None => OptimizedPayload {
                data: Bytes::copy_from_slice(payload),
                compression_type: CompressionType::None,
                original_size: payload.len(),
                compressed_size: payload.len(),
                compression_ratio: 1.0,
            },
            CompressionType::Lz4 => self.compress_lz4(payload),
            CompressionType::Zstd => self.compress_zstd(payload).await,
            CompressionType::Brotli => self.compress_brotli(payload).await,
        }
    }
    
    /// Choose optimal compression algorithm
    fn choose_compression_algorithm(&self, peer_metrics: &PeerMetrics, payload_size: usize) -> CompressionType {
        let compression_stats = self.compression_stats.read();
        
        // For fast connections with low latency, use better compression
        if peer_metrics.bandwidth_mbps > 10.0 && peer_metrics.latency < Duration::from_millis(50) {
            if payload_size > 10_000 {
                compression_stats.best_algorithm_for_size(payload_size)
            } else {
                CompressionType::Lz4 // Fast for smaller payloads
            }
        } else if peer_metrics.bandwidth_mbps > 1.0 {
            CompressionType::Zstd // Good balance
        } else {
            CompressionType::Lz4 // Fastest for slow connections
        }
    }
    
    /// LZ4 compression (fastest)
    fn compress_lz4(&self, payload: &[u8]) -> OptimizedPayload {
        let compressed = compress_prepend_size(payload);
        let compressed_len = compressed.len();
        let compression_ratio = payload.len() as f32 / compressed_len as f32;
        
        // Update stats
        {
            let mut stats = self.compression_stats.write();
            stats.update_lz4_stats(payload.len(), compressed_len);
        }
        
        OptimizedPayload {
            data: Bytes::from(compressed),
            compression_type: CompressionType::Lz4,
            original_size: payload.len(),
            compressed_size: compressed_len,
            compression_ratio,
        }
    }
    
    /// Zstd compression (good balance)
    async fn compress_zstd(&self, payload: &[u8]) -> OptimizedPayload {
        let compressed = tokio::task::spawn_blocking({
            let payload = payload.to_vec();
            move || zstd::encode_all(payload.as_slice(), 3)
        }).await.unwrap_or_else(|_| Ok(payload.to_vec())).unwrap_or_else(|_| payload.to_vec());
        
        let compression_ratio = payload.len() as f32 / compressed.len() as f32;
        
        // Update stats
        {
            let mut stats = self.compression_stats.write();
            stats.update_zstd_stats(payload.len(), compressed.len());
        }
        
        let compressed_size = compressed.len();
        
        OptimizedPayload {
            data: Bytes::from(compressed),
            compression_type: CompressionType::Zstd,
            original_size: payload.len(),
            compressed_size,
            compression_ratio,
        }
    }
    
    /// Brotli compression (best ratio)
    async fn compress_brotli(&self, payload: &[u8]) -> OptimizedPayload {
        let compressed = tokio::task::spawn_blocking({
            let payload = payload.to_vec();
            move || {
                let mut output = Vec::new();
                let mut reader = payload.as_slice();
                brotli::BrotliCompress(&mut reader, &mut output, &brotli::enc::BrotliEncoderParams::default())
                    .map(|_| output)
                    .unwrap_or_else(|_| payload)
            }
        }).await.unwrap_or_else(|_| payload.to_vec());
        
        let compression_ratio = payload.len() as f32 / compressed.len() as f32;
        
        let compressed_size = compressed.len();
        
        // Update stats
        {
            let mut stats = self.compression_stats.write();
            stats.update_brotli_stats(payload.len(), compressed_size);
        }
        
        OptimizedPayload {
            data: Bytes::from(compressed),
            compression_type: CompressionType::Brotli,
            original_size: payload.len(),
            compressed_size,
            compression_ratio,
        }
    }
    
    /// Decompress payload based on compression type
    pub async fn decompress_payload(&self, optimized: &OptimizedPayload) -> Result<Bytes, TransportError> {
        match optimized.compression_type {
            CompressionType::None => Ok(optimized.data.clone()),
            CompressionType::Lz4 => {
                let decompressed = decompress_size_prepended(&optimized.data)
                    .map_err(|e| TransportError::CompressionError(e.to_string()))?;
                Ok(Bytes::from(decompressed))
            },
            CompressionType::Zstd => {
                let decompressed = tokio::task::spawn_blocking({
                    let data = optimized.data.clone();
                    move || zstd::decode_all(data.as_ref())
                }).await
                    .map_err(|e| TransportError::CompressionError(e.to_string()))?
                    .map_err(|e| TransportError::CompressionError(e.to_string()))?;
                Ok(Bytes::from(decompressed))
            },
            CompressionType::Brotli => {
                let decompressed = tokio::task::spawn_blocking({
                    let data = optimized.data.clone();
                    move || {
                        let mut output = Vec::new();
                        let mut reader = data.as_ref();
                        brotli::BrotliDecompress(&mut reader, &mut output)
                            .map(|_| output)
                    }
                }).await
                    .map_err(|e| TransportError::CompressionError(e.to_string()))?
                    .map_err(|e| TransportError::CompressionError(e.to_string()))?;
                Ok(Bytes::from(decompressed))
            }
        }
    }
    
    /// Add message to batch queue for peer
    pub async fn add_to_batch(&self, peer_id: &PeerId, message: P2PMessage) -> Option<Vec<P2PMessage>> {
        let mut queues = self.batch_queues.write();
        let queue = queues.entry(*peer_id).or_insert_with(|| BatchQueue::new(self.config.clone()));
        
        queue.add_message(message);
        
        // Check if batch should be sent
        if queue.should_send() {
            Some(queue.drain_messages())
        } else {
            None
        }
    }
    
    /// Force send all batched messages for peer
    pub async fn flush_batches(&self, peer_id: &PeerId) -> Vec<P2PMessage> {
        let mut queues = self.batch_queues.write();
        if let Some(queue) = queues.get_mut(peer_id) {
            queue.drain_messages()
        } else {
            Vec::new()
        }
    }
    
    /// Update peer metrics based on transmission results
    pub async fn update_peer_metrics(&self, peer_id: &PeerId, transmission_result: TransmissionResult) {
        let mut metrics = self.peer_metrics.write();
        let peer_metrics = metrics.entry(*peer_id).or_insert_with(PeerMetrics::new);
        
        peer_metrics.update(transmission_result);
        self.congestion_detector.update_peer_metrics(peer_id, peer_metrics);
    }
    
    /// Get current peer metrics
    fn get_peer_metrics(&self, peer_id: &PeerId) -> PeerMetrics {
        self.peer_metrics.read()
            .get(peer_id)
            .cloned()
            .unwrap_or_else(PeerMetrics::new)
    }
    
    /// Determine if message should be batched
    fn should_batch_message(&self, peer_metrics: &PeerMetrics, message: &P2PMessage) -> bool {
        // Don't batch urgent messages - TODO: implement priority checking based on payload
        // if message.priority > 200 {
        //     return false;
        // }
        
        // Don't batch if connection is very fast
        if peer_metrics.bandwidth_mbps > 50.0 && peer_metrics.latency < Duration::from_millis(10) {
            return false;
        }
        
        // Batch for slower connections to improve efficiency
        peer_metrics.bandwidth_mbps < 10.0 || peer_metrics.latency > Duration::from_millis(100)
    }
    
    /// Estimate transmission time for optimized payload
    fn estimate_transmission_time(&self, peer_metrics: &PeerMetrics, payload: &OptimizedPayload) -> Duration {
        if peer_metrics.bandwidth_mbps <= 0.0 {
            return Duration::from_secs(1); // Default estimate
        }
        
        let bits = (payload.compressed_size * 8) as f64;
        let bandwidth_bps = peer_metrics.bandwidth_mbps * 1_000_000.0;
        let transmission_time = Duration::from_secs_f64(bits / bandwidth_bps);
        
        // Add latency and some overhead
        transmission_time + peer_metrics.latency + Duration::from_millis(10)
    }
    
    /// Get network optimization statistics
    pub fn get_stats(&self) -> NetworkOptimizerStats {
        let peer_metrics = self.peer_metrics.read();
        let compression_stats = self.compression_stats.read();
        let batch_queues = self.batch_queues.read();
        
        let total_peers = peer_metrics.len();
        let average_bandwidth = if total_peers > 0 {
            peer_metrics.values().map(|m| m.bandwidth_mbps).sum::<f64>() / total_peers as f64
        } else {
            0.0
        };
        
        let total_batched_messages: usize = batch_queues.values()
            .map(|queue| queue.pending_count())
            .sum();
        
        NetworkOptimizerStats {
            total_peers,
            average_bandwidth_mbps: average_bandwidth,
            compression_stats: compression_stats.clone(),
            total_batched_messages,
            congestion_level: self.congestion_detector.current_level(),
        }
    }
}

/// Metrics for a specific peer connection
#[derive(Debug, Clone)]
pub struct PeerMetrics {
    pub bandwidth_mbps: f64,
    pub latency: Duration,
    pub packet_loss_rate: f64,
    pub successful_transmissions: u64,
    pub failed_transmissions: u64,
    pub last_updated: Instant,
    pub connection_quality: ConnectionQuality,
}

impl PeerMetrics {
    pub fn new() -> Self {
        Self {
            bandwidth_mbps: 1.0, // Conservative default
            latency: Duration::from_millis(100),
            packet_loss_rate: 0.0,
            successful_transmissions: 0,
            failed_transmissions: 0,
            last_updated: Instant::now(),
            connection_quality: ConnectionQuality::Unknown,
        }
    }
    
    pub fn update(&mut self, result: TransmissionResult) {
        self.last_updated = Instant::now();
        
        match result {
            TransmissionResult::Success { duration, bytes_sent } => {
                self.successful_transmissions += 1;
                
                // Update bandwidth estimate (exponential moving average)
                let new_bandwidth = (bytes_sent * 8) as f64 / duration.as_secs_f64() / 1_000_000.0;
                self.bandwidth_mbps = 0.8 * self.bandwidth_mbps + 0.2 * new_bandwidth;
                
                // Update latency (exponential moving average)
                self.latency = Duration::from_nanos(
                    (0.8 * self.latency.as_nanos() as f64 + 0.2 * duration.as_nanos() as f64) as u64
                );
            },
            TransmissionResult::Failure { error: _ } => {
                self.failed_transmissions += 1;
            },
            TransmissionResult::Timeout => {
                self.failed_transmissions += 1;
                // Increase latency estimate for timeouts
                self.latency = self.latency.saturating_mul(2);
            },
        }
        
        // Update packet loss rate
        let total = self.successful_transmissions + self.failed_transmissions;
        if total > 0 {
            self.packet_loss_rate = self.failed_transmissions as f64 / total as f64;
        }
        
        // Update connection quality
        self.connection_quality = self.calculate_quality();
    }
    
    fn calculate_quality(&self) -> ConnectionQuality {
        if self.packet_loss_rate > 0.1 || self.bandwidth_mbps < 0.1 {
            ConnectionQuality::Poor
        } else if self.packet_loss_rate > 0.05 || self.bandwidth_mbps < 1.0 || self.latency > Duration::from_millis(500) {
            ConnectionQuality::Fair
        } else if self.packet_loss_rate < 0.01 && self.bandwidth_mbps > 10.0 && self.latency < Duration::from_millis(100) {
            ConnectionQuality::Excellent
        } else {
            ConnectionQuality::Good
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConnectionQuality {
    Unknown,
    Poor,
    Fair,
    Good,
    Excellent,
}

/// Message batching queue for efficient transmission
struct BatchQueue {
    messages: VecDeque<P2PMessage>,
    first_message_time: Option<Instant>,
    config: NetworkOptimizerConfig,
}

impl BatchQueue {
    fn new(config: NetworkOptimizerConfig) -> Self {
        Self {
            messages: VecDeque::new(),
            first_message_time: None,
            config,
        }
    }
    
    fn add_message(&mut self, message: P2PMessage) {
        if self.messages.is_empty() {
            self.first_message_time = Some(Instant::now());
        }
        self.messages.push_back(message);
    }
    
    fn should_send(&self) -> bool {
        if self.messages.is_empty() {
            return false;
        }
        
        // Send if batch is full
        if self.messages.len() >= self.config.max_batch_size {
            return true;
        }
        
        // Send if batch has been waiting too long
        if let Some(first_time) = self.first_message_time {
            if first_time.elapsed() >= self.config.max_batch_delay {
                return true;
            }
        }
        
        false
    }
    
    fn drain_messages(&mut self) -> Vec<P2PMessage> {
        self.first_message_time = None;
        self.messages.drain(..).collect()
    }
    
    fn pending_count(&self) -> usize {
        self.messages.len()
    }
}

/// Network congestion detection and management
pub struct CongestionDetector {
    peer_metrics: Arc<Mutex<FxHashMap<PeerId, PeerMetrics>>>,
    global_metrics: Arc<Mutex<GlobalNetworkMetrics>>,
}

impl CongestionDetector {
    pub fn new() -> Self {
        Self {
            peer_metrics: Arc::new(Mutex::new(FxHashMap::default())),
            global_metrics: Arc::new(Mutex::new(GlobalNetworkMetrics::new())),
        }
    }
    
    pub fn update_peer_metrics(&self, peer_id: &PeerId, metrics: &PeerMetrics) {
        {
            let mut peer_metrics = self.peer_metrics.lock();
            peer_metrics.insert(*peer_id, metrics.clone());
        }
        
        // Update global metrics
        {
            let mut global = self.global_metrics.lock();
            global.update_from_peer_metrics(metrics);
        }
    }
    
    pub fn current_level(&self) -> CongestionLevel {
        let global = self.global_metrics.lock();
        global.congestion_level()
    }
}

#[derive(Debug, Clone)]
struct GlobalNetworkMetrics {
    average_latency: Duration,
    average_bandwidth: f64,
    average_packet_loss: f64,
    last_updated: Instant,
}

impl GlobalNetworkMetrics {
    fn new() -> Self {
        Self {
            average_latency: Duration::from_millis(100),
            average_bandwidth: 1.0,
            average_packet_loss: 0.0,
            last_updated: Instant::now(),
        }
    }
    
    fn update_from_peer_metrics(&mut self, peer_metrics: &PeerMetrics) {
        // Exponential moving average
        self.average_latency = Duration::from_nanos(
            (0.9 * self.average_latency.as_nanos() as f64 + 0.1 * peer_metrics.latency.as_nanos() as f64) as u64
        );
        self.average_bandwidth = 0.9 * self.average_bandwidth + 0.1 * peer_metrics.bandwidth_mbps;
        self.average_packet_loss = 0.9 * self.average_packet_loss + 0.1 * peer_metrics.packet_loss_rate;
        self.last_updated = Instant::now();
    }
    
    fn congestion_level(&self) -> CongestionLevel {
        if self.average_packet_loss > 0.1 || self.average_latency > Duration::from_secs(1) {
            CongestionLevel::High
        } else if self.average_packet_loss > 0.05 || self.average_latency > Duration::from_millis(500) {
            CongestionLevel::Medium
        } else {
            CongestionLevel::Low
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CongestionLevel {
    Low,
    Medium,
    High,
}

/// Compression algorithm statistics
#[derive(Debug, Clone)]
pub struct CompressionStats {
    pub lz4_stats: CompressionAlgorithmStats,
    pub zstd_stats: CompressionAlgorithmStats,
    pub brotli_stats: CompressionAlgorithmStats,
}

impl CompressionStats {
    pub fn new() -> Self {
        Self {
            lz4_stats: CompressionAlgorithmStats::new("LZ4"),
            zstd_stats: CompressionAlgorithmStats::new("Zstd"),
            brotli_stats: CompressionAlgorithmStats::new("Brotli"),
        }
    }
    
    pub fn update_lz4_stats(&mut self, original_size: usize, compressed_size: usize) {
        self.lz4_stats.update(original_size, compressed_size);
    }
    
    pub fn update_zstd_stats(&mut self, original_size: usize, compressed_size: usize) {
        self.zstd_stats.update(original_size, compressed_size);
    }
    
    pub fn update_brotli_stats(&mut self, original_size: usize, compressed_size: usize) {
        self.brotli_stats.update(original_size, compressed_size);
    }
    
    pub fn best_algorithm_for_size(&self, size: usize) -> CompressionType {
        // Choose based on size and historical performance
        if size < 1024 {
            CompressionType::Lz4 // Fast for small payloads
        } else if size < 10_000 {
            // Choose based on best compression ratio for medium sizes
            if self.zstd_stats.average_ratio > self.lz4_stats.average_ratio {
                CompressionType::Zstd
            } else {
                CompressionType::Lz4
            }
        } else {
            // For large payloads, use best compression
            let ratios = [
                (CompressionType::Lz4, self.lz4_stats.average_ratio),
                (CompressionType::Zstd, self.zstd_stats.average_ratio),
                (CompressionType::Brotli, self.brotli_stats.average_ratio),
            ];
            
            ratios.iter()
                .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(algo, _)| *algo)
                .unwrap_or(CompressionType::Zstd)
        }
    }
}

#[derive(Debug, Clone)]
pub struct CompressionAlgorithmStats {
    pub name: String,
    pub uses: u64,
    pub total_original_bytes: u64,
    pub total_compressed_bytes: u64,
    pub average_ratio: f64,
}

impl CompressionAlgorithmStats {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            uses: 0,
            total_original_bytes: 0,
            total_compressed_bytes: 0,
            average_ratio: 1.0,
        }
    }
    
    pub fn update(&mut self, original_size: usize, compressed_size: usize) {
        self.uses += 1;
        self.total_original_bytes += original_size as u64;
        self.total_compressed_bytes += compressed_size as u64;
        
        if self.total_compressed_bytes > 0 {
            self.average_ratio = self.total_original_bytes as f64 / self.total_compressed_bytes as f64;
        }
    }
}

// Supporting types and enums

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CompressionType {
    None,
    Lz4,
    Zstd,
    Brotli,
}

#[derive(Debug, Clone)]
pub struct OptimizedMessage {
    pub peer_id: PeerId,
    pub original_message: P2PMessage,
    pub optimized_payload: OptimizedPayload,
    pub should_batch: bool,
    pub compression_used: CompressionType,
    pub estimated_transmission_time: Duration,
}

#[derive(Debug, Clone)]
pub struct OptimizedPayload {
    pub data: Bytes,
    pub compression_type: CompressionType,
    pub original_size: usize,
    pub compressed_size: usize,
    pub compression_ratio: f32,
}

#[derive(Debug, Clone)]
pub enum TransmissionResult {
    Success {
        duration: Duration,
        bytes_sent: usize,
    },
    Failure {
        error: String,
    },
    Timeout,
}

#[derive(Debug)]
pub struct NetworkOptimizerStats {
    pub total_peers: usize,
    pub average_bandwidth_mbps: f64,
    pub compression_stats: CompressionStats,
    pub total_batched_messages: usize,
    pub congestion_level: CongestionLevel,
}