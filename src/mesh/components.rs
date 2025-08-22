use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use async_trait::async_trait;
use crate::protocol::{PeerId, BitchatPacket};
use crate::error::Result;

/// Component types in the mesh service
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ComponentType {
    Transport,
    Routing,
    Discovery,
    Storage,
    Gaming,
    Treasury,
    AntiCheat,
    Consensus,
}

/// Trait for mesh service components
#[async_trait]
pub trait MeshComponent: Send + Sync {
    /// Get the component type
    fn component_type(&self) -> ComponentType;
    
    /// Start the component
    async fn start(&mut self) -> Result<()>;
    
    /// Stop the component
    async fn stop(&mut self) -> Result<()>;
    
    /// Process a packet
    async fn process_packet(&self, packet: BitchatPacket) -> Result<()>;
    
    /// Handle peer connection
    async fn on_peer_connected(&self, peer_id: PeerId);
    
    /// Handle peer disconnection
    async fn on_peer_disconnected(&self, peer_id: PeerId);
    
    /// Get component health status
    async fn health_check(&self) -> ComponentHealth;
}

/// Health status of a component
#[derive(Debug, Clone)]
pub struct ComponentHealth {
    pub healthy: bool,
    pub message: String,
    pub metrics: HashMap<String, f64>,
}

/// Manager for all mesh components
pub struct ComponentManager {
    components: Arc<RwLock<HashMap<ComponentType, Box<dyn MeshComponent>>>>,
}

impl ComponentManager {
    /// Create a new component manager
    pub fn new() -> Self {
        Self {
            components: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Register a component
    pub async fn register(&self, component: Box<dyn MeshComponent>) -> Result<()> {
        let component_type = component.component_type();
        self.components.write().await.insert(component_type, component);
        Ok(())
    }
    
    /// Start all components
    pub async fn start_all(&self) -> Result<()> {
        for component in self.components.write().await.values_mut() {
            component.start().await?;
        }
        Ok(())
    }
    
    /// Stop all components
    pub async fn stop_all(&self) -> Result<()> {
        for component in self.components.write().await.values_mut() {
            component.stop().await?;
        }
        Ok(())
    }
    
    /// Process a packet with the appropriate component
    pub async fn process_packet(&self, packet: BitchatPacket) -> Result<()> {
        // Route to appropriate component based on packet type
        // For now, broadcast to all components
        for component in self.components.read().await.values() {
            component.process_packet(packet.clone()).await?;
        }
        Ok(())
    }
    
    /// Notify all components of peer connection
    pub async fn notify_peer_connected(&self, peer_id: PeerId) {
        for component in self.components.read().await.values() {
            component.on_peer_connected(peer_id).await;
        }
    }
    
    /// Notify all components of peer disconnection
    pub async fn notify_peer_disconnected(&self, peer_id: PeerId) {
        for component in self.components.read().await.values() {
            component.on_peer_disconnected(peer_id).await;
        }
    }
    
    /// Check health of all components
    pub async fn health_check_all(&self) -> HashMap<ComponentType, ComponentHealth> {
        let mut health_results = HashMap::new();
        
        for (component_type, component) in self.components.read().await.iter() {
            health_results.insert(*component_type, component.health_check().await);
        }
        
        health_results
    }
}