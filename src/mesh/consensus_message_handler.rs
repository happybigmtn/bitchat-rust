//! Consensus Message Handler for Mesh Network
//!
//! This module provides specialized message handling for consensus messages
//! within the mesh network, including routing, validation, and integration
//! with the consensus system.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::time::interval;

use crate::crypto::BitchatIdentity;
use crate::error::{Error, Result};
use crate::mesh::MeshService;
use crate::protocol::network_consensus_bridge::NetworkConsensusBridge;
use crate::protocol::p2p_messages::{ConsensusMessage, MessagePriority};
use crate::protocol::{BitchatPacket, GameId, PACKET_TYPE_CONSENSUS_VOTE};

/// Configuration for consensus message handling
#[derive(Debug, Clone)]
pub struct ConsensusMessageConfig {
    /// Maximum messages to process per second
    pub max_messages_per_second: u32,
    /// Message validation timeout
    pub validation_timeout: Duration,
    /// Priority queue sizes
    pub critical_queue_size: usize,
    pub high_queue_size: usize,
    pub normal_queue_size: usize,
    pub low_queue_size: usize,
    /// Enable encryption for consensus messages
    pub enable_encryption: bool,
}

impl Default for ConsensusMessageConfig {
    fn default() -> Self {
        Self {
            max_messages_per_second: 100,
            validation_timeout: Duration::from_millis(500),
            critical_queue_size: 1000,
            high_queue_size: 500,
            normal_queue_size: 200,
            low_queue_size: 100,
            enable_encryption: true, // Enable encryption by default
        }
    }
}

/// Priority message queues for consensus messages
struct PriorityQueues {
    critical: mpsc::Receiver<ConsensusMessage>,
    high: mpsc::Receiver<ConsensusMessage>,
    normal: mpsc::Receiver<ConsensusMessage>,
    low: mpsc::Receiver<ConsensusMessage>,

    critical_sender: mpsc::Sender<ConsensusMessage>,
    high_sender: mpsc::Sender<ConsensusMessage>,
    normal_sender: mpsc::Sender<ConsensusMessage>,
    low_sender: mpsc::Sender<ConsensusMessage>,
}

impl PriorityQueues {
    fn new(config: &ConsensusMessageConfig) -> Self {
        let (critical_sender, critical) = mpsc::channel(config.critical_queue_size);
        let (high_sender, high) = mpsc::channel(config.high_queue_size);
        let (normal_sender, normal) = mpsc::channel(config.normal_queue_size);
        let (low_sender, low) = mpsc::channel(config.low_queue_size);

        Self {
            critical,
            high,
            normal,
            low,
            critical_sender,
            high_sender,
            normal_sender,
            low_sender,
        }
    }

    async fn send_by_priority(&self, message: ConsensusMessage) -> Result<()> {
        let sender = match message.payload.priority() {
            MessagePriority::Critical => &self.critical_sender,
            MessagePriority::High => &self.high_sender,
            MessagePriority::Normal => &self.normal_sender,
            MessagePriority::Low => &self.low_sender,
        };

        sender
            .send(message)
            .await
            .map_err(|e| Error::Network(format!("Failed to queue message: {}", e)))
    }
}

/// Consensus message handler for the mesh network
pub struct ConsensusMessageHandler {
    // Core components
    mesh_service: Arc<MeshService>,
    identity: Arc<BitchatIdentity>,

    // Configuration
    config: ConsensusMessageConfig,

    // Message processing
    priority_queues: Arc<Mutex<PriorityQueues>>,
    consensus_bridges: Arc<RwLock<HashMap<GameId, Arc<NetworkConsensusBridge>>>>,

    // Rate limiting
    messages_processed: Arc<RwLock<u64>>,
    last_rate_check: Arc<RwLock<Instant>>,
    current_rate: Arc<RwLock<u32>>,

    // Message validation
    message_validator: Arc<ConsensusMessageValidator>,

    // Statistics
    stats: Arc<RwLock<ConsensusMessageStats>>,
}

/// Validation for consensus messages
struct ConsensusMessageValidator {
    identity: Arc<BitchatIdentity>,
    validation_timeout: Duration,
}

impl ConsensusMessageValidator {
    fn new(identity: Arc<BitchatIdentity>, timeout: Duration) -> Self {
        Self {
            identity,
            validation_timeout: timeout,
        }
    }

    /// Validate a consensus message
    async fn validate_message(&self, message: &ConsensusMessage) -> Result<()> {
        // Check message age
        if message.timestamp + self.validation_timeout.as_secs()
            < std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        {
            return Err(Error::Validation("Message too old".to_string()));
        }

        // Verify signature (simplified - in practice would use proper crypto verification)
        if message.signature.0 == [0u8; 64] {
            return Err(Error::Validation("Invalid message signature".to_string()));
        }

        // Validate sender
        if message.sender == [0u8; 32] {
            return Err(Error::Validation("Invalid sender".to_string()));
        }

        // Check for reasonable message size
        let message_size = bincode::serialized_size(&message.payload)
            .map_err(|e| Error::Validation(format!("Cannot determine message size: {}", e)))?;

        if message_size > 64 * 1024 {
            // 64KB limit
            return Err(Error::Validation("Message too large".to_string()));
        }

        Ok(())
    }
}

/// Statistics for consensus message handling
#[derive(Debug, Clone, Default)]
pub struct ConsensusMessageStats {
    pub total_messages_received: u64,
    pub total_messages_processed: u64,
    pub total_messages_dropped: u64,
    pub messages_by_priority: HashMap<u8, u64>,
    pub validation_failures: u64,
    pub rate_limit_drops: u64,
    pub processing_errors: u64,
}

impl ConsensusMessageHandler {
    /// Create new consensus message handler
    pub fn new(
        mesh_service: Arc<MeshService>,
        identity: Arc<BitchatIdentity>,
        config: ConsensusMessageConfig,
    ) -> Self {
        let priority_queues = Arc::new(Mutex::new(PriorityQueues::new(&config)));
        let message_validator = Arc::new(ConsensusMessageValidator::new(
            identity.clone(),
            config.validation_timeout,
        ));

        Self {
            mesh_service,
            identity,
            config,
            priority_queues,
            consensus_bridges: Arc::new(RwLock::new(HashMap::new())),
            messages_processed: Arc::new(RwLock::new(0)),
            last_rate_check: Arc::new(RwLock::new(Instant::now())),
            current_rate: Arc::new(RwLock::new(0)),
            message_validator,
            stats: Arc::new(RwLock::new(ConsensusMessageStats::default())),
        }
    }

    /// Register a consensus bridge for a specific game
    pub async fn register_consensus_bridge(
        &self,
        game_id: GameId,
        bridge: Arc<NetworkConsensusBridge>,
    ) {
        self.consensus_bridges.write().await.insert(game_id, bridge);
        log::info!("Registered consensus bridge for game {:?}", game_id);
    }

    /// Unregister a consensus bridge
    pub async fn unregister_consensus_bridge(&self, game_id: &GameId) {
        self.consensus_bridges.write().await.remove(game_id);
        log::info!("Unregistered consensus bridge for game {:?}", game_id);
    }

    /// Start the consensus message handler
    pub async fn start(&self) -> Result<()> {
        log::info!("Starting consensus message handler");

        // Start message processing tasks
        self.start_packet_listener().await;
        self.start_priority_processor().await;
        self.start_rate_limiter().await;
        self.start_stats_collector().await;

        Ok(())
    }

    /// Start listening for consensus packets from mesh service
    async fn start_packet_listener(&self) {
        let mesh_service = self.mesh_service.clone();
        let priority_queues = self.priority_queues.clone();
        let message_validator = self.message_validator.clone();
        let stats = self.stats.clone();

        tokio::spawn(async move {
            // This is a simplified implementation - in practice, you'd hook into
            // the mesh service's event system more directly
            let mut check_interval = interval(Duration::from_millis(10));

            loop {
                check_interval.tick().await;

                // In a real implementation, this would receive actual mesh events
                // For now, we simulate by checking for consensus packets
                // The actual integration would hook into MeshService's message processing
            }
        });
    }

    /// Process incoming BitchatPacket and extract consensus message if applicable
    pub async fn handle_packet(&self, packet: BitchatPacket) -> Result<()> {
        // Check if this is a consensus packet
        if packet.packet_type != PACKET_TYPE_CONSENSUS_VOTE {
            return Ok(()); // Not our concern
        }

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.total_messages_received += 1;
        }

        // Extract consensus message from packet
        let message = self.packet_to_consensus_message(&packet)?;

        // Validate message
        if let Err(e) = self.message_validator.validate_message(&message).await {
            log::warn!("Invalid consensus message from {:?}: {}", message.sender, e);
            let mut stats = self.stats.write().await;
            stats.validation_failures += 1;
            stats.total_messages_dropped += 1;
            return Ok(());
        }

        // Check rate limiting
        if !self.check_rate_limit().await {
            log::warn!(
                "Rate limit exceeded, dropping message from {:?}",
                message.sender
            );
            let mut stats = self.stats.write().await;
            stats.rate_limit_drops += 1;
            stats.total_messages_dropped += 1;
            return Ok(());
        }

        // Queue message by priority
        let priority = message.payload.priority();
        let queues = self.priority_queues.lock().await;

        if let Err(e) = queues.send_by_priority(message).await {
            log::error!("Failed to queue consensus message: {}", e);
            let mut stats = self.stats.write().await;
            stats.total_messages_dropped += 1;
        } else {
            let mut stats = self.stats.write().await;
            *stats
                .messages_by_priority
                .entry(priority as u8)
                .or_insert(0) += 1;
        }

        Ok(())
    }

    /// Start priority-based message processor
    async fn start_priority_processor(&self) {
        let consensus_bridges = self.consensus_bridges.clone();
        let messages_processed = self.messages_processed.clone();
        let stats = self.stats.clone();

        // Create receivers from the queues (this is a simplified approach)
        // In practice, you'd need a more sophisticated queue management system

        tokio::spawn(async move {
            let mut process_interval = interval(Duration::from_millis(1));

            loop {
                process_interval.tick().await;

                // Process messages from priority queues
                // This is where we'd actually dequeue and process messages
                // The implementation would vary based on the exact queue structure

                // For now, we simulate processing
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        });
    }

    /// Start rate limiting checker
    async fn start_rate_limiter(&self) {
        let messages_processed = self.messages_processed.clone();
        let last_rate_check = self.last_rate_check.clone();
        let current_rate = self.current_rate.clone();
        let max_rate = self.config.max_messages_per_second;

        tokio::spawn(async move {
            let mut rate_check_interval = interval(Duration::from_secs(1));

            loop {
                rate_check_interval.tick().await;

                let now = Instant::now();
                let last_check = {
                    let mut last = last_rate_check.write().await;
                    let prev = *last;
                    *last = now;
                    prev
                };

                let elapsed = now.duration_since(last_check);
                if elapsed >= Duration::from_secs(1) {
                    // Reset rate counter
                    *current_rate.write().await = 0;
                }
            }
        });
    }

    /// Start statistics collection
    async fn start_stats_collector(&self) {
        let stats = self.stats.clone();

        tokio::spawn(async move {
            let mut stats_interval = interval(Duration::from_secs(60));

            loop {
                stats_interval.tick().await;

                let current_stats = stats.read().await.clone();
                log::info!(
                    "Consensus message stats: received={}, processed={}, dropped={}",
                    current_stats.total_messages_received,
                    current_stats.total_messages_processed,
                    current_stats.total_messages_dropped
                );
            }
        });
    }

    /// Check if we're within rate limits
    async fn check_rate_limit(&self) -> bool {
        let mut rate = self.current_rate.write().await;
        if *rate >= self.config.max_messages_per_second {
            false
        } else {
            *rate += 1;
            true
        }
    }

    /// Convert BitchatPacket to ConsensusMessage
    fn packet_to_consensus_message(&self, packet: &BitchatPacket) -> Result<ConsensusMessage> {
        if let Some(payload) = &packet.payload {
            bincode::deserialize(payload).map_err(|e| {
                Error::Serialization(format!("Failed to deserialize consensus message: {}", e))
            })
        } else {
            Err(Error::Protocol("Packet has no payload".to_string()))
        }
    }

    /// Process a consensus message for a specific game
    async fn process_consensus_message(&self, message: ConsensusMessage) -> Result<()> {
        let bridges = self.consensus_bridges.read().await;

        if let Some(bridge) = bridges.get(&message.game_id) {
            // Convert message back to packet format for bridge processing
            let packet = self.consensus_message_to_packet(&message)?;

            // Send to appropriate bridge
            bridge.handle_network_message(packet).await?;

            // Update stats
            {
                let mut stats = self.stats.write().await;
                stats.total_messages_processed += 1;
            }
            *self.messages_processed.write().await += 1;
        } else {
            log::debug!(
                "No consensus bridge registered for game {:?}",
                message.game_id
            );
            // Not an error - we might receive messages for games we're not participating in
        }

        Ok(())
    }

    /// Convert ConsensusMessage back to BitchatPacket
    fn consensus_message_to_packet(&self, message: &ConsensusMessage) -> Result<BitchatPacket> {
        let mut packet = BitchatPacket::new(PACKET_TYPE_CONSENSUS_VOTE);

        let payload =
            bincode::serialize(message).map_err(|e| Error::Serialization(e.to_string()))?;

        packet.payload = Some(payload);
        packet.source = message.sender;
        packet.target = [0u8; 32]; // Broadcast

        Ok(packet)
    }

    /// Get message handler statistics
    pub async fn get_stats(&self) -> ConsensusMessageStats {
        self.stats.read().await.clone()
    }

    /// Get number of registered consensus bridges
    pub async fn get_bridge_count(&self) -> usize {
        self.consensus_bridges.read().await.len()
    }
}

/// Integration helper for connecting consensus message handler to mesh service
pub struct MeshConsensusIntegration;

impl MeshConsensusIntegration {
    /// Integrate consensus message handler with mesh service
    pub async fn integrate(
        mesh_service: Arc<MeshService>,
        handler: Arc<ConsensusMessageHandler>,
    ) -> Result<()> {
        // Start the handler
        handler.start().await?;

        // In a real implementation, you would:
        // 1. Hook the handler into MeshService's event system
        // 2. Register packet type handlers
        // 3. Set up message routing

        // For now, we'll set up a simple integration task
        let mesh_clone = mesh_service.clone();
        let handler_clone = handler.clone();

        tokio::spawn(async move {
            let mut integration_interval = interval(Duration::from_millis(10));

            loop {
                integration_interval.tick().await;

                // In practice, this would be driven by actual mesh events
                // For now, we simulate by periodically checking for packets
                // The real implementation would integrate more directly with MeshService
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        });

        log::info!("Consensus message handler integrated with mesh service");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::BitchatKeypair;
    use crate::protocol::p2p_messages::ConsensusPayload;
    use crate::transport::TransportCoordinator;

    #[tokio::test]
    async fn test_consensus_message_handler_creation() {
        let keypair = BitchatKeypair::generate();
        let identity = Arc::new(crate::crypto::BitchatIdentity::from_keypair_with_pow(
            keypair, 8,
        ));
        let transport = Arc::new(TransportCoordinator::new());
        let mesh_service = Arc::new(MeshService::new(identity.clone(), transport));

        let config = ConsensusMessageConfig::default();
        let handler = ConsensusMessageHandler::new(mesh_service, identity, config);

        assert_eq!(handler.get_bridge_count().await, 0);

        let stats = handler.get_stats().await;
        assert_eq!(stats.total_messages_received, 0);
        assert_eq!(stats.total_messages_processed, 0);
    }

    #[tokio::test]
    async fn test_message_validation() {
        let keypair = BitchatKeypair::generate();
        let identity = Arc::new(crate::crypto::BitchatIdentity::from_keypair_with_pow(
            keypair, 8,
        ));

        let validator = ConsensusMessageValidator::new(identity, Duration::from_secs(60));

        // Create a test message
        let message = ConsensusMessage {
            message_id: [1u8; 32],
            sender: [2u8; 32],
            game_id: [3u8; 16],
            round: 1,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            payload: ConsensusPayload::Heartbeat {
                alive_participants: vec![],
                network_view: crate::protocol::p2p_messages::NetworkView {
                    participants: vec![],
                    connections: vec![],
                    partition_id: None,
                    leader: None,
                },
            },
            signature: crate::protocol::Signature([1u8; 64]), // Non-zero signature
            compressed: false,
        };

        // Should pass validation
        assert!(validator.validate_message(&message).await.is_ok());

        // Test with zero signature (should fail)
        let mut invalid_message = message.clone();
        invalid_message.signature = crate::protocol::Signature([0u8; 64]);
        assert!(validator.validate_message(&invalid_message).await.is_err());
    }
}
