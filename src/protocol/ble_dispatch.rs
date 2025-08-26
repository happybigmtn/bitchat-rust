//! BLE-Optimized Message Dispatch System
//! 
//! This module provides bandwidth-efficient message dispatch optimized for
//! Bluetooth Low Energy constraints with compression, fragmentation, and
//! intelligent bandwidth management.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time::interval;
use serde::{Deserialize, Serialize};

use crate::protocol::PeerId;
use crate::protocol::p2p_messages::{ConsensusMessage, MessagePriority};
use crate::error::{Error, Result};

/// BLE constraints and configuration
#[derive(Debug, Clone)]
pub struct BleConfig {
    /// Maximum Transmission Unit (MTU) for BLE
    pub mtu_size: usize,
    /// Available bandwidth in bytes per second
    pub bandwidth_bps: usize,
    /// Maximum concurrent connections
    pub max_connections: usize,
    /// Connection interval in milliseconds
    pub connection_interval_ms: u16,
    /// Message timeout before retry
    pub message_timeout: Duration,
    /// Maximum retransmission attempts
    pub max_retries: u32,
    /// Adaptive compression threshold
    pub compression_threshold: usize,
}

impl Default for BleConfig {
    fn default() -> Self {
        Self {
            mtu_size: 244,                // BLE 4.2+ MTU minus headers
            bandwidth_bps: 125_000,       // ~1 Mbps theoretical / 8
            max_connections: 8,           // Typical BLE peripheral limit
            connection_interval_ms: 30,   // 30ms connection interval
            message_timeout: Duration::from_secs(5),
            max_retries: 3,
            compression_threshold: 64,    // Compress messages >64 bytes
        }
    }
}

/// Message fragment for large message transmission
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageFragment {
    /// Original message ID
    pub message_id: [u8; 16],
    /// Fragment sequence number
    pub fragment_id: u16,
    /// Total number of fragments
    pub total_fragments: u16,
    /// Fragment payload
    pub data: Vec<u8>,
    /// Fragment checksum
    pub checksum: u32,
}

/// Pending message for transmission
#[derive(Debug, Clone)]
struct PendingMessage {
    message: ConsensusMessage,
    priority: MessagePriority,
    target_peers: Vec<PeerId>,
    created_at: Instant,
    retry_count: u32,
    fragments: Option<Vec<MessageFragment>>,
    bytes_sent: usize,
}

/// Fragment reassembly state
#[derive(Debug)]
struct FragmentAssembly {
    message_id: [u8; 16],
    expected_fragments: u16,
    received_fragments: HashMap<u16, MessageFragment>,
    first_fragment_time: Instant,
}

/// Per-peer bandwidth tracking
#[derive(Debug, Clone)]
struct PeerBandwidth {
    peer_id: PeerId,
    bytes_sent: u64,
    bytes_received: u64,
    last_activity: Instant,
    connection_quality: f64, // 0.0 to 1.0
    estimated_rtt: Duration,
    packet_loss_rate: f64,
}

/// BLE-optimized message dispatcher
pub struct BleMessageDispatcher {
    config: BleConfig,
    
    // Message queues by priority
    critical_queue: Arc<RwLock<VecDeque<PendingMessage>>>,
    high_queue: Arc<RwLock<VecDeque<PendingMessage>>>,
    normal_queue: Arc<RwLock<VecDeque<PendingMessage>>>,
    low_queue: Arc<RwLock<VecDeque<PendingMessage>>>,
    
    // Fragment reassembly
    fragment_assembly: Arc<RwLock<HashMap<[u8; 16], FragmentAssembly>>>,
    
    // Bandwidth management
    peer_bandwidth: Arc<RwLock<HashMap<PeerId, PeerBandwidth>>>,
    total_bandwidth_used: Arc<RwLock<u64>>,
    bandwidth_window_start: Arc<RwLock<Instant>>,
    
    // Statistics
    messages_queued: Arc<RwLock<u64>>,
    messages_sent: Arc<RwLock<u64>>,
    messages_dropped: Arc<RwLock<u64>>,
    bytes_compressed: Arc<RwLock<u64>>,
    compression_ratio: Arc<RwLock<f64>>,
}

impl BleMessageDispatcher {
    /// Create new BLE message dispatcher
    pub fn new(config: BleConfig) -> Self {
        Self {
            config,
            critical_queue: Arc::new(RwLock::new(VecDeque::new())),
            high_queue: Arc::new(RwLock::new(VecDeque::new())),
            normal_queue: Arc::new(RwLock::new(VecDeque::new())),
            low_queue: Arc::new(RwLock::new(VecDeque::new())),
            fragment_assembly: Arc::new(RwLock::new(HashMap::new())),
            peer_bandwidth: Arc::new(RwLock::new(HashMap::new())),
            total_bandwidth_used: Arc::new(RwLock::new(0)),
            bandwidth_window_start: Arc::new(RwLock::new(Instant::now())),
            messages_queued: Arc::new(RwLock::new(0)),
            messages_sent: Arc::new(RwLock::new(0)),
            messages_dropped: Arc::new(RwLock::new(0)),
            bytes_compressed: Arc::new(RwLock::new(0)),
            compression_ratio: Arc::new(RwLock::new(1.0)),
        }
    }
    
    /// Start the dispatcher background tasks
    pub async fn start(&self) {
        self.start_dispatch_task().await;
        self.start_bandwidth_reset_task().await;
        self.start_fragment_cleanup_task().await;
        self.start_retry_task().await;
    }
    
    /// Queue message for transmission
    pub async fn queue_message(
        &self, 
        message: ConsensusMessage, 
        target_peers: Vec<PeerId>
    ) -> Result<()> {
        let priority = message.payload.priority();
        let message_size = message.payload_size();
        
        // Check if we should compress
        let mut optimized_message = message;
        if message_size >= self.config.compression_threshold {
            optimized_message.compress()?;
            
            let compressed_size = optimized_message.payload_size();
            let ratio = compressed_size as f64 / message_size as f64;
            
            // Update compression statistics
            *self.bytes_compressed.write().await += (message_size - compressed_size) as u64;
            let mut current_ratio = self.compression_ratio.write().await;
            *current_ratio = (*current_ratio * 0.9) + (ratio * 0.1); // Exponential moving average
            
            log::debug!("Compressed message from {} to {} bytes (ratio: {:.2})", 
                       message_size, compressed_size, ratio);
        }
        
        let pending = PendingMessage {
            message: optimized_message,
            priority,
            target_peers,
            created_at: Instant::now(),
            retry_count: 0,
            fragments: None,
            bytes_sent: 0,
        };
        
        // Queue in appropriate priority queue
        match priority {
            MessagePriority::Critical => {
                self.critical_queue.write().await.push_back(pending);
            }
            MessagePriority::High => {
                self.high_queue.write().await.push_back(pending);
            }
            MessagePriority::Normal => {
                self.normal_queue.write().await.push_back(pending);
            }
            MessagePriority::Low => {
                self.low_queue.write().await.push_back(pending);
            }
        }
        
        *self.messages_queued.write().await += 1;
        Ok(())
    }
    
    /// Process incoming message fragment
    pub async fn handle_fragment(&self, fragment: MessageFragment) -> Result<Option<ConsensusMessage>> {
        let mut assembly_map = self.fragment_assembly.write().await;
        
        // Get or create fragment assembly state
        let assembly = assembly_map.entry(fragment.message_id).or_insert_with(|| {
            FragmentAssembly {
                message_id: fragment.message_id,
                expected_fragments: fragment.total_fragments,
                received_fragments: HashMap::new(),
                first_fragment_time: Instant::now(),
            }
        });
        
        // Verify fragment checksum
        let expected_checksum = crc32fast::hash(&fragment.data);
        if fragment.checksum != expected_checksum {
            log::warn!("Fragment checksum mismatch for message {:?}, fragment {}", 
                      fragment.message_id, fragment.fragment_id);
            return Err(Error::InvalidData("Fragment checksum mismatch".to_string()));
        }
        
        // Add fragment
        assembly.received_fragments.insert(fragment.fragment_id, fragment);
        
        // Check if we have all fragments
        let complete = assembly.received_fragments.len() == assembly.expected_fragments as usize;
        if complete {
            // Reassemble message
            let message_id = assembly.message_id;
            let message = self.reassemble_message(assembly)?;
            
            // Clean up assembly state
            assembly_map.remove(&message_id);
            
            Ok(Some(message))
        } else {
            Ok(None)
        }
    }
    
    /// Start message dispatch task
    async fn start_dispatch_task(&self) {
        let critical_queue = self.critical_queue.clone();
        let high_queue = self.high_queue.clone();
        let normal_queue = self.normal_queue.clone();
        let low_queue = self.low_queue.clone();
        let peer_bandwidth = self.peer_bandwidth.clone();
        let config = self.config.clone();
        let messages_sent = self.messages_sent.clone();
        let messages_dropped = self.messages_dropped.clone();
        
        tokio::spawn(async move {
            let mut dispatch_interval = interval(Duration::from_millis(config.connection_interval_ms as u64));
            
            loop {
                dispatch_interval.tick().await;
                
                // Calculate available bandwidth for this interval
                let interval_bandwidth = (config.bandwidth_bps as u64 * config.connection_interval_ms as u64) / 1000;
                let mut bytes_remaining = interval_bandwidth;
                
                // Process queues by priority (critical first, then high, normal, low)
                bytes_remaining = Self::process_queue_static(&critical_queue, bytes_remaining, &peer_bandwidth, &config, &messages_sent, &messages_dropped).await;
                bytes_remaining = Self::process_queue_static(&high_queue, bytes_remaining, &peer_bandwidth, &config, &messages_sent, &messages_dropped).await;
                bytes_remaining = Self::process_queue_static(&normal_queue, bytes_remaining, &peer_bandwidth, &config, &messages_sent, &messages_dropped).await;
                let _ = Self::process_queue_static(&low_queue, bytes_remaining, &peer_bandwidth, &config, &messages_sent, &messages_dropped).await;
            }
        });
    }
    
    /// Process a priority queue (static method for tokio::spawn)
    async fn process_queue_static(
        queue: &Arc<RwLock<VecDeque<PendingMessage>>>,
        mut bytes_available: u64,
        peer_bandwidth: &Arc<RwLock<HashMap<PeerId, PeerBandwidth>>>,
        config: &BleConfig,
        messages_sent: &Arc<RwLock<u64>>,
        messages_dropped: &Arc<RwLock<u64>>,
    ) -> u64 {
        let mut queue_guard = queue.write().await;
        let mut messages_to_requeue = Vec::new();
        
        while let Some(mut pending) = queue_guard.pop_front() {
            if bytes_available == 0 {
                // No bandwidth left - requeue message
                messages_to_requeue.push(pending);
                continue;
            }
            
            // Check if message has timed out
            if pending.created_at.elapsed() > config.message_timeout {
                if pending.retry_count >= config.max_retries {
                    log::warn!("Dropping message after {} retries", pending.retry_count);
                    *messages_dropped.write().await += 1;
                    continue;
                } else {
                    pending.retry_count += 1;
                    pending.created_at = Instant::now();
                }
            }
            
            // Serialize message
            let message_bytes = match bincode::serialize(&pending.message) {
                Ok(bytes) => bytes,
                Err(e) => {
                    log::error!("Failed to serialize message: {}", e);
                    *messages_dropped.write().await += 1;
                    continue;
                }
            };
            
            // Fragment if necessary
            if message_bytes.len() > config.mtu_size {
                if pending.fragments.is_none() {
                    pending.fragments = Some(Self::fragment_message(message_bytes.clone(), config.mtu_size));
                }
            }
            
            // Send message or fragments
            let bytes_to_send = if let Some(ref fragments) = pending.fragments {
                // Send next fragment
                if let Some(fragment) = fragments.get(pending.bytes_sent / config.mtu_size) {
                    let fragment_bytes = match bincode::serialize(fragment) {
                        Ok(bytes) => bytes,
                        Err(e) => {
                            log::error!("Failed to serialize fragment: {}", e);
                            *messages_dropped.write().await += 1;
                            continue;
                        }
                    };
                    
                    if fragment_bytes.len() as u64 <= bytes_available {
                        // TODO: Actually send the fragment via transport
                        // transport.send_fragment(fragment_bytes).await;
                        
                        pending.bytes_sent += config.mtu_size;
                        fragment_bytes.len() as u64
                    } else {
                        // Not enough bandwidth - requeue
                        messages_to_requeue.push(pending);
                        continue;
                    }
                } else {
                    // All fragments sent
                    *messages_sent.write().await += 1;
                    continue;
                }
            } else {
                // Send complete message
                if message_bytes.len() as u64 <= bytes_available {
                    // TODO: Actually send the message via transport
                    // transport.send_message(message_bytes).await;
                    
                    *messages_sent.write().await += 1;
                    message_bytes.len() as u64
                } else {
                    // Not enough bandwidth - requeue
                    messages_to_requeue.push(pending);
                    continue;
                }
            };
            
            bytes_available = bytes_available.saturating_sub(bytes_to_send);
            
            // Update peer bandwidth tracking
            let mut bandwidth_map = peer_bandwidth.write().await;
            for peer_id in &pending.target_peers {
                let peer_stats = bandwidth_map.entry(*peer_id).or_insert_with(|| {
                    PeerBandwidth {
                        peer_id: *peer_id,
                        bytes_sent: 0,
                        bytes_received: 0,
                        last_activity: Instant::now(),
                        connection_quality: 1.0,
                        estimated_rtt: Duration::from_millis(100),
                        packet_loss_rate: 0.0,
                    }
                });
                peer_stats.bytes_sent += bytes_to_send / pending.target_peers.len() as u64;
                peer_stats.last_activity = Instant::now();
            }
            
            // Requeue if not fully sent
            if pending.fragments.is_some() && pending.bytes_sent < message_bytes.len() {
                messages_to_requeue.push(pending);
            }
        }
        
        // Requeue messages that couldn't be sent
        for message in messages_to_requeue {
            queue_guard.push_back(message);
        }
        
        bytes_available
    }
    
    /// Fragment a large message into MTU-sized pieces
    fn fragment_message(message_bytes: Vec<u8>, mtu_size: usize) -> Vec<MessageFragment> {
        let payload_size = mtu_size - 32; // Reserve space for fragment header
        let total_fragments = (message_bytes.len() + payload_size - 1) / payload_size;
        
        let mut message_id = [0u8; 16];
        use rand::{RngCore, rngs::OsRng};
        let mut secure_rng = OsRng;
        secure_rng.fill_bytes(&mut message_id);
        
        let mut fragments = Vec::new();
        
        for (i, chunk) in message_bytes.chunks(payload_size).enumerate() {
            let fragment = MessageFragment {
                message_id,
                fragment_id: i as u16,
                total_fragments: total_fragments as u16,
                data: chunk.to_vec(),
                checksum: crc32fast::hash(chunk),
            };
            
            fragments.push(fragment);
        }
        
        fragments
    }
    
    /// Reassemble message from fragments
    fn reassemble_message(&self, assembly: &FragmentAssembly) -> Result<ConsensusMessage> {
        let mut message_bytes = Vec::new();
        
        // Sort fragments by ID and concatenate
        let mut sorted_fragments: Vec<_> = assembly.received_fragments.values().collect();
        sorted_fragments.sort_by_key(|f| f.fragment_id);
        
        for fragment in sorted_fragments {
            message_bytes.extend_from_slice(&fragment.data);
        }
        
        // Deserialize the complete message
        bincode::deserialize(&message_bytes)
            .map_err(|e| Error::Serialization(e.to_string()))
    }
    
    /// Start bandwidth reset task (resets bandwidth counters periodically)
    async fn start_bandwidth_reset_task(&self) {
        let bandwidth_window_start = self.bandwidth_window_start.clone();
        let total_bandwidth_used = self.total_bandwidth_used.clone();
        
        tokio::spawn(async move {
            let mut reset_interval = interval(Duration::from_secs(1)); // Reset every second
            
            loop {
                reset_interval.tick().await;
                
                *bandwidth_window_start.write().await = Instant::now();
                *total_bandwidth_used.write().await = 0;
            }
        });
    }
    
    /// Start fragment cleanup task (removes stale fragment assemblies)
    async fn start_fragment_cleanup_task(&self) {
        let fragment_assembly = self.fragment_assembly.clone();
        
        tokio::spawn(async move {
            let mut cleanup_interval = interval(Duration::from_secs(30)); // Cleanup every 30 seconds
            
            loop {
                cleanup_interval.tick().await;
                
                let mut assembly_map = fragment_assembly.write().await;
                let cutoff_time = Instant::now() - Duration::from_secs(60); // 1 minute timeout
                
                assembly_map.retain(|_, assembly| assembly.first_fragment_time > cutoff_time);
            }
        });
    }
    
    /// Start retry task for failed messages
    async fn start_retry_task(&self) {
        // TODO: Implement retry logic for failed transmissions
    }
    
    /// Update peer connection quality based on transmission success
    pub async fn update_peer_quality(&self, peer_id: PeerId, success: bool, rtt: Duration) {
        let mut bandwidth_map = self.peer_bandwidth.write().await;
        
        if let Some(peer_stats) = bandwidth_map.get_mut(&peer_id) {
            peer_stats.estimated_rtt = rtt;
            peer_stats.last_activity = Instant::now();
            
            // Update connection quality with exponential moving average
            let new_quality = if success { 1.0 } else { 0.0 };
            peer_stats.connection_quality = (peer_stats.connection_quality * 0.9) + (new_quality * 0.1);
            
            // Update packet loss rate
            let loss_update = if success { 0.0 } else { 1.0 };
            peer_stats.packet_loss_rate = (peer_stats.packet_loss_rate * 0.95) + (loss_update * 0.05);
        }
    }
    
    /// Get current BLE dispatch statistics
    pub async fn get_stats(&self) -> BleDispatchStats {
        BleDispatchStats {
            messages_queued: *self.messages_queued.read().await,
            messages_sent: *self.messages_sent.read().await,
            messages_dropped: *self.messages_dropped.read().await,
            bytes_compressed: *self.bytes_compressed.read().await,
            compression_ratio: *self.compression_ratio.read().await,
            critical_queue_size: self.critical_queue.read().await.len(),
            high_queue_size: self.high_queue.read().await.len(),
            normal_queue_size: self.normal_queue.read().await.len(),
            low_queue_size: self.low_queue.read().await.len(),
            active_fragments: self.fragment_assembly.read().await.len(),
            connected_peers: self.peer_bandwidth.read().await.len(),
            total_bandwidth_used: *self.total_bandwidth_used.read().await,
        }
    }
    
    /// Get bandwidth utilization percentage
    pub async fn get_bandwidth_utilization(&self) -> f64 {
        let window_duration = self.bandwidth_window_start.read().await.elapsed();
        if window_duration.is_zero() {
            return 0.0;
        }
        
        let bytes_used = *self.total_bandwidth_used.read().await;
        let available_bytes = (self.config.bandwidth_bps as f64 * window_duration.as_secs_f64()) as u64;
        
        if available_bytes == 0 {
            0.0
        } else {
            (bytes_used as f64 / available_bytes as f64) * 100.0
        }
    }
    
    /// Adaptive compression - adjust compression threshold based on bandwidth usage
    pub async fn adapt_compression_threshold(&mut self) {
        let utilization = self.get_bandwidth_utilization().await;
        
        // Increase compression aggressiveness when bandwidth is high
        if utilization > 80.0 {
            self.config.compression_threshold = 32; // Compress smaller messages
        } else if utilization < 40.0 {
            self.config.compression_threshold = 128; // Less aggressive compression
        } else {
            self.config.compression_threshold = 64; // Default threshold
        }
    }
}

/// BLE dispatcher statistics
#[derive(Debug, Clone)]
pub struct BleDispatchStats {
    pub messages_queued: u64,
    pub messages_sent: u64,
    pub messages_dropped: u64,
    pub bytes_compressed: u64,
    pub compression_ratio: f64,
    pub critical_queue_size: usize,
    pub high_queue_size: usize,
    pub normal_queue_size: usize,
    pub low_queue_size: usize,
    pub active_fragments: usize,
    pub connected_peers: usize,
    pub total_bandwidth_used: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::p2p_messages::ConsensusPayload;
    
    #[tokio::test]
    async fn test_message_fragmentation() {
        let large_data = vec![0u8; 1000]; // 1KB message
        let mtu_size = 244;
        
        let fragments = BleMessageDispatcher::fragment_message(large_data.clone(), mtu_size);
        
        assert!(fragments.len() > 1);
        assert_eq!(fragments[0].message_id, fragments[1].message_id);
        assert_eq!(fragments[0].total_fragments as usize, fragments.len());
        
        // Verify total size
        let total_fragment_data: usize = fragments.iter().map(|f| f.data.len()).sum();
        assert_eq!(total_fragment_data, large_data.len());
    }
    
    #[tokio::test]
    async fn test_bandwidth_management() {
        let config = BleConfig::default();
        let dispatcher = BleMessageDispatcher::new(config);
        dispatcher.start().await;
        
        // Test initial stats
        let stats = dispatcher.get_stats().await;
        assert_eq!(stats.messages_queued, 0);
        assert_eq!(stats.messages_sent, 0);
        assert_eq!(stats.connected_peers, 0);
    }
}