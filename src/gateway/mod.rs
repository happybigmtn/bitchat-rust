//! Gateway Node Implementation for Internet Bridging
//! 
//! This module provides gateway nodes that bridge local BLE mesh networks
//! to the internet, enabling global BitCraps gameplay while maintaining
//! the efficiency and privacy of local mesh networks.

pub mod core;
pub mod bridge;
pub mod discovery;
pub mod load_balancer;
pub mod failover;

pub use core::{
    GatewayNode, GatewayConfig, GatewayInterface, GatewayProtocol,
    GatewayStats, GatewayEvent, BandwidthUsage, GatewayNodeError
};
pub use bridge::{
    BridgeProtocol, ProtocolTranslator, MessageRouter, NATTraversal
};
pub use discovery::{
    GatewayDiscovery, GatewaySelectionCriteria, GatewayPreference,
    GatewayAdvertisement, HealthMonitor
};
pub use load_balancer::{
    LoadBalancer, LoadBalancingPolicy, GatewayMetrics
};
pub use failover::{
    FailoverManager, RedundancyConfig, FailoverEvent
};

use std::sync::Arc;
use tokio::sync::RwLock;
use crate::protocol::PeerId;
use crate::error::Result;

/// Gateway node factory for creating and managing gateway instances
pub struct GatewayFactory {
    gateways: Arc<RwLock<Vec<Arc<GatewayNode>>>>,
}

impl Default for GatewayFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl GatewayFactory {
    pub fn new() -> Self {
        Self {
            gateways: Arc::new(RwLock::new(Vec::new())),
        }
    }
    
    /// Create a new gateway node with the specified configuration
    pub async fn create_gateway(
        &self,
        config: GatewayConfig,
        identity: Arc<crate::crypto::BitchatIdentity>,
        mesh_service: Arc<crate::mesh::MeshService>,
    ) -> Result<Arc<GatewayNode>> {
        let gateway = Arc::new(GatewayNode::new(identity, config, mesh_service)?);
        
        let mut gateways = self.gateways.write().await;
        gateways.push(gateway.clone());
        
        Ok(gateway)
    }
    
    /// Get all managed gateway nodes
    pub async fn get_gateways(&self) -> Vec<Arc<GatewayNode>> {
        self.gateways.read().await.clone()
    }
    
    /// Remove a gateway node
    pub async fn remove_gateway(&self, peer_id: PeerId) -> bool {
        let mut gateways = self.gateways.write().await;
        let initial_len = gateways.len();
        gateways.retain(|gateway| {
            // Compare peer IDs if accessible
            true // Simplified for now
        });
        gateways.len() < initial_len
    }
    
    /// Start all gateway nodes
    pub async fn start_all(&self) -> Result<()> {
        let gateways = self.gateways.read().await;
        
        for gateway in gateways.iter() {
            gateway.start().await?;
        }
        
        Ok(())
    }
    
    /// Stop all gateway nodes
    pub async fn stop_all(&self) {
        let gateways = self.gateways.read().await;
        
        for gateway in gateways.iter() {
            gateway.stop().await;
        }
    }
}

/// Gateway node type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GatewayType {
    /// Full gateway with both local and internet interfaces
    Full,
    /// Local-only gateway for mesh bridging
    LocalBridge,
    /// Internet-only gateway for external connectivity
    InternetBridge,
    /// Relay gateway for message forwarding
    Relay,
}

/// Gateway capability flags
#[derive(Debug, Clone)]
pub struct GatewayCapabilities {
    pub supports_tcp: bool,
    pub supports_udp: bool,
    pub supports_websocket: bool,
    pub supports_quic: bool,
    pub supports_ble_bridging: bool,
    pub supports_nat_traversal: bool,
    pub supports_load_balancing: bool,
    pub max_concurrent_connections: usize,
    pub max_bandwidth_mbps: f64,
}

impl Default for GatewayCapabilities {
    fn default() -> Self {
        Self {
            supports_tcp: true,
            supports_udp: true,
            supports_websocket: false,
            supports_quic: false,
            supports_ble_bridging: true,
            supports_nat_traversal: true,
            supports_load_balancing: true,
            max_concurrent_connections: 1000,
            max_bandwidth_mbps: 100.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::{BitchatKeypair, BitchatIdentity};
    use crate::transport::TransportCoordinator;
    
    #[tokio::test]
    async fn test_gateway_factory() {
        let factory = GatewayFactory::new();
        
        // Initially no gateways
        let gateways = factory.get_gateways().await;
        assert_eq!(gateways.len(), 0);
        
        // Create a gateway
        let keypair = BitchatKeypair::generate();
        let identity = Arc::new(BitchatIdentity::from_keypair_with_pow(keypair, 8));
        let transport = Arc::new(TransportCoordinator::new());
        let mesh = Arc::new(crate::mesh::MeshService::new(identity.clone(), transport));
        let config = GatewayConfig::default();
        
        let gateway = factory.create_gateway(config, identity, mesh).await.unwrap();
        
        // Should now have one gateway
        let gateways = factory.get_gateways().await;
        assert_eq!(gateways.len(), 1);
    }
}