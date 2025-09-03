//! Bridge Protocol Implementation
//!
//! This module implements protocol translation between BLE mesh networks
//! and internet protocols (TCP/UDP), enabling seamless communication
//! across different transport layers.

pub mod protocol;
pub mod translator;
pub mod router;
pub mod nat;

pub use protocol::{BridgeProtocol, BridgeMessage, BridgeHeader, ProtocolType};
pub use translator::{ProtocolTranslator, TranslationRule, TranslationError};
pub use router::{MessageRouter, RoutingTable, RoutingDecision, RouteMetrics};
pub use nat::{NATTraversal, NATPunching, STUNClient, TURNRelay};

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};

use crate::protocol::PeerId;
use crate::error::Result;

/// Bridge configuration for protocol translation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeConfig {
    /// Enable protocol translation
    pub enable_translation: bool,
    /// Enable message compression
    pub enable_compression: bool,
    /// Enable encryption for bridged messages
    pub enable_encryption: bool,
    /// Maximum message size for bridging
    pub max_message_size: usize,
    /// Timeout for bridge operations
    pub bridge_timeout: std::time::Duration,
    /// NAT traversal configuration
    pub nat_config: NATConfig,
    /// Routing configuration
    pub routing_config: RoutingConfig,
}

/// NAT traversal configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NATConfig {
    /// Enable NAT traversal
    pub enable_nat_traversal: bool,
    /// STUN server addresses
    pub stun_servers: Vec<String>,
    /// TURN server configuration
    pub turn_servers: Vec<TURNServerConfig>,
    /// NAT detection timeout
    pub detection_timeout: std::time::Duration,
    /// Keepalive interval for NAT holes
    pub keepalive_interval: std::time::Duration,
}

/// TURN server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TURNServerConfig {
    pub address: String,
    pub username: String,
    pub password: String,
    pub realm: String,
}

/// Routing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingConfig {
    /// Enable adaptive routing
    pub enable_adaptive_routing: bool,
    /// Route update interval
    pub update_interval: std::time::Duration,
    /// Maximum route hops
    pub max_hops: u8,
    /// Route timeout
    pub route_timeout: std::time::Duration,
    /// Load balancing algorithm
    pub load_balancing: LoadBalancingAlgorithm,
}

/// Load balancing algorithms for routing
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum LoadBalancingAlgorithm {
    RoundRobin,
    LeastConnections,
    WeightedRoundRobin,
    LeastLatency,
    Random,
}

impl Default for BridgeConfig {
    fn default() -> Self {
        Self {
            enable_translation: true,
            enable_compression: true,
            enable_encryption: true,
            max_message_size: 64 * 1024, // 64KB
            bridge_timeout: std::time::Duration::from_secs(30),
            nat_config: NATConfig {
                enable_nat_traversal: true,
                stun_servers: vec![
                    "stun:stun.l.google.com:19302".to_string(),
                    "stun:stun1.l.google.com:19302".to_string(),
                ],
                turn_servers: Vec::new(),
                detection_timeout: std::time::Duration::from_secs(10),
                keepalive_interval: std::time::Duration::from_secs(25),
            },
            routing_config: RoutingConfig {
                enable_adaptive_routing: true,
                update_interval: std::time::Duration::from_secs(30),
                max_hops: 10,
                route_timeout: std::time::Duration::from_secs(300),
                load_balancing: LoadBalancingAlgorithm::LeastLatency,
            },
        }
    }
}

/// Bridge manager that coordinates all bridging operations
pub struct BridgeManager {
    config: BridgeConfig,
    protocol_translator: Arc<ProtocolTranslator>,
    message_router: Arc<MessageRouter>,
    nat_traversal: Arc<NATTraversal>,

    // Bridge state
    active_bridges: Arc<RwLock<HashMap<PeerId, BridgeConnection>>>,
    routing_table: Arc<RwLock<RoutingTable>>,
    nat_mappings: Arc<RwLock<HashMap<SocketAddr, SocketAddr>>>,

    // Statistics
    bridge_stats: Arc<RwLock<BridgeStatistics>>,
}

/// Bridge connection information
#[derive(Debug, Clone)]
pub struct BridgeConnection {
    pub peer_id: PeerId,
    pub local_address: SocketAddr,
    pub remote_address: SocketAddr,
    pub protocol_type: ProtocolType,
    pub established_at: std::time::Instant,
    pub bytes_transferred: u64,
    pub last_activity: std::time::Instant,
    pub connection_quality: ConnectionQuality,
}

/// Connection quality metrics
#[derive(Debug, Clone)]
pub struct ConnectionQuality {
    pub latency: std::time::Duration,
    pub jitter: std::time::Duration,
    pub packet_loss: f64,
    pub throughput: f64,
}

/// Bridge statistics
#[derive(Debug, Default, Clone)]
pub struct BridgeStatistics {
    pub messages_bridged: u64,
    pub bytes_bridged: u64,
    pub active_bridges: usize,
    pub successful_translations: u64,
    pub failed_translations: u64,
    pub nat_successes: u64,
    pub nat_failures: u64,
    pub average_latency: std::time::Duration,
}

impl BridgeManager {
    /// Create new bridge manager
    pub fn new(config: BridgeConfig) -> Self {
        Self {
            protocol_translator: Arc::new(ProtocolTranslator::new(config.clone())),
            message_router: Arc::new(MessageRouter::new(config.routing_config.clone())),
            nat_traversal: Arc::new(NATTraversal::new(config.nat_config.clone())),
            active_bridges: Arc::new(RwLock::new(HashMap::new())),
            routing_table: Arc::new(RwLock::new(RoutingTable::new())),
            nat_mappings: Arc::new(RwLock::new(HashMap::new())),
            bridge_stats: Arc::new(RwLock::new(BridgeStatistics::default())),
            config,
        }
    }

    /// Start bridge manager
    pub async fn start(&self) -> Result<()> {
        // Initialize NAT traversal
        if self.config.nat_config.enable_nat_traversal {
            self.nat_traversal.initialize().await?;
        }

        // Start routing table updates
        if self.config.routing_config.enable_adaptive_routing {
            self.start_routing_updates().await?;
        }

        log::info!("Bridge manager started");
        Ok(())
    }

    /// Stop bridge manager
    pub async fn stop(&self) {
        // Close all active bridges
        let mut bridges = self.active_bridges.write().await;
        bridges.clear();

        log::info!("Bridge manager stopped");
    }

    /// Create bridge between BLE mesh and internet peer
    pub async fn create_bridge(
        &self,
        local_peer: PeerId,
        remote_address: SocketAddr,
        protocol: ProtocolType,
    ) -> Result<()> {
        // Perform NAT traversal if needed
        let final_address = if self.config.nat_config.enable_nat_traversal {
            self.nat_traversal.establish_connection(remote_address).await?
        } else {
            remote_address
        };

        // Create bridge connection
        let bridge = BridgeConnection {
            peer_id: local_peer,
            local_address: "0.0.0.0:0".parse().unwrap(), // Will be updated
            remote_address: final_address,
            protocol_type: protocol,
            established_at: std::time::Instant::now(),
            bytes_transferred: 0,
            last_activity: std::time::Instant::now(),
            connection_quality: ConnectionQuality {
                latency: std::time::Duration::ZERO,
                jitter: std::time::Duration::ZERO,
                packet_loss: 0.0,
                throughput: 0.0,
            },
        };

        // Add to active bridges
        self.active_bridges.write().await.insert(local_peer, bridge);

        // Update statistics
        {
            let mut stats = self.bridge_stats.write().await;
            stats.active_bridges = self.active_bridges.read().await.len();
        }

        log::info!("Bridge created: {} -> {}",
                  hex::encode(&local_peer[..8]), final_address);

        Ok(())
    }

    /// Remove bridge
    pub async fn remove_bridge(&self, local_peer: PeerId) -> Result<()> {
        let removed = self.active_bridges.write().await.remove(&local_peer).is_some();

        if removed {
            // Update statistics
            let mut stats = self.bridge_stats.write().await;
            stats.active_bridges = self.active_bridges.read().await.len();

            log::info!("Bridge removed: {}", hex::encode(&local_peer[..8]));
        }

        Ok(())
    }

    /// Bridge message from BLE to internet
    pub async fn bridge_to_internet(
        &self,
        from_peer: PeerId,
        message: Vec<u8>,
    ) -> Result<()> {
        let bridges = self.active_bridges.read().await;

        if let Some(bridge) = bridges.get(&from_peer) {
            // Translate BLE message to internet protocol
            let translated = self.protocol_translator
                .translate_ble_to_internet(message, bridge.protocol_type)
                .await?;

            // Route message to destination
            self.message_router
                .route_to_internet(translated, bridge.remote_address)
                .await?;

            // Update statistics
            let mut stats = self.bridge_stats.write().await;
            stats.messages_bridged += 1;
            stats.bytes_bridged += translated.len() as u64;
            stats.successful_translations += 1;

            Ok(())
        } else {
            Err(crate::error::Error::Network(
                format!("No bridge found for peer {}", hex::encode(&from_peer[..8]))
            ))
        }
    }

    /// Bridge message from internet to BLE
    pub async fn bridge_to_ble(
        &self,
        to_peer: PeerId,
        message: Vec<u8>,
        source_protocol: ProtocolType,
    ) -> Result<()> {
        // Translate internet message to BLE format
        let translated = self.protocol_translator
            .translate_internet_to_ble(message, source_protocol)
            .await?;

        // Route message to BLE mesh
        self.message_router
            .route_to_ble(translated, to_peer)
            .await?;

        // Update statistics
        let mut stats = self.bridge_stats.write().await;
        stats.messages_bridged += 1;
        stats.bytes_bridged += translated.len() as u64;
        stats.successful_translations += 1;

        Ok(())
    }

    /// Get bridge statistics
    pub async fn get_statistics(&self) -> BridgeStatistics {
        let stats = self.bridge_stats.read().await;
        let mut result = stats.clone();
        result.active_bridges = self.active_bridges.read().await.len();
        result
    }

    /// Update bridge quality metrics
    pub async fn update_bridge_quality(
        &self,
        peer_id: PeerId,
        quality: ConnectionQuality,
    ) -> Result<()> {
        let mut bridges = self.active_bridges.write().await;

        if let Some(bridge) = bridges.get_mut(&peer_id) {
            bridge.connection_quality = quality;
            bridge.last_activity = std::time::Instant::now();
        }

        Ok(())
    }

    /// Get active bridges
    pub async fn get_active_bridges(&self) -> Vec<BridgeConnection> {
        self.active_bridges.read().await.values().cloned().collect()
    }

    // Private helper methods
    async fn start_routing_updates(&self) -> Result<()> {
        let router = self.message_router.clone();
        let interval = self.config.routing_config.update_interval;

        tokio::spawn(async move {
            let mut update_interval = tokio::time::interval(interval);

            loop {
                update_interval.tick().await;

                if let Err(e) = router.update_routing_table().await {
                    log::error!("Failed to update routing table: {}", e);
                }
            }
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_bridge_manager_creation() {
        let config = BridgeConfig::default();
        let manager = BridgeManager::new(config);

        let stats = manager.get_statistics().await;
        assert_eq!(stats.active_bridges, 0);
        assert_eq!(stats.messages_bridged, 0);
    }

    #[tokio::test]
    async fn test_bridge_creation() {
        let config = BridgeConfig::default();
        let manager = BridgeManager::new(config);

        let peer_id = [0u8; 32];
        let remote_addr = "127.0.0.1:8080".parse().unwrap();

        let result = manager.create_bridge(
            peer_id,
            remote_addr,
            ProtocolType::Tcp,
        ).await;

        assert!(result.is_ok());

        let stats = manager.get_statistics().await;
        assert_eq!(stats.active_bridges, 1);
    }
}