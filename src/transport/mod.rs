//! Transport layer for BitCraps mesh networking
//! 
//! This module implements the transport layer including:
//! - Bluetooth LE mesh transport using btleplug
//! - Transport abstraction trait
//! - Peer discovery and connection management
//! - Packet routing and forwarding

pub mod bluetooth;
pub mod traits;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use async_trait::async_trait;
use serde::{Serialize, Deserialize};

use crate::protocol::{PeerId, BitchatPacket};
use crate::error::{Error, Result};

pub use traits::*;
pub use bluetooth::*;

/// Transport address types for different connection methods
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TransportAddress {
    Tcp(SocketAddr),      // TCP connection (for testing/development)
    Udp(SocketAddr),      // UDP connection (for testing/development)  
    Bluetooth(String),    // Bluetooth device ID/address
    Mesh(PeerId),        // Abstract mesh routing via peer ID
}

/// Events that can occur on a transport
#[derive(Debug, Clone)]
pub enum TransportEvent {
    Connected { peer_id: PeerId, address: TransportAddress },
    Disconnected { peer_id: PeerId, reason: String },
    DataReceived { peer_id: PeerId, data: Vec<u8> },
    Error { peer_id: Option<PeerId>, error: String },
}

/// Transport coordinator managing multiple transport types
pub struct TransportCoordinator {
    bluetooth: Option<Arc<RwLock<BluetoothTransport>>>,
    connections: Arc<RwLock<HashMap<PeerId, TransportAddress>>>,
    event_sender: mpsc::UnboundedSender<TransportEvent>,
    event_receiver: Arc<RwLock<mpsc::UnboundedReceiver<TransportEvent>>>,
}

impl TransportCoordinator {
    pub fn new() -> Self {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();
        
        Self {
            bluetooth: None,
            connections: Arc::new(RwLock::new(HashMap::new())),
            event_sender,
            event_receiver: Arc::new(RwLock::new(event_receiver)),
        }
    }
    
    /// Initialize Bluetooth transport
    pub async fn init_bluetooth(&mut self, local_peer_id: PeerId) -> Result<()> {
        let bluetooth = BluetoothTransport::new(local_peer_id).await
            .map_err(|e| Error::Network(format!("Failed to initialize Bluetooth: {}", e)))?;
        
        self.bluetooth = Some(Arc::new(RwLock::new(bluetooth)));
        Ok(())
    }
    
    /// Start listening on all available transports
    pub async fn start_listening(&self) -> Result<()> {
        if let Some(bluetooth) = &self.bluetooth {
            let mut bt = bluetooth.write().await;
            bt.listen(TransportAddress::Bluetooth("BitCraps".to_string())).await
                .map_err(|e| Error::Network(format!("Bluetooth listen failed: {}", e)))?;
        }
        
        Ok(())
    }
    
    /// Connect to a peer via the best available transport
    pub async fn connect_to_peer(&self, peer_id: PeerId, address: TransportAddress) -> Result<()> {
        match address {
            TransportAddress::Bluetooth(_) => {
                if let Some(bluetooth) = &self.bluetooth {
                    let mut bt = bluetooth.write().await;
                    bt.connect(address.clone()).await
                        .map_err(|e| Error::Network(format!("Bluetooth connect failed: {}", e)))?;
                    
                    self.connections.write().await.insert(peer_id, address);
                }
            }
            _ => {
                return Err(Error::Network("Unsupported transport type".to_string()));
            }
        }
        
        Ok(())
    }
    
    /// Send data to a peer
    pub async fn send_to_peer(&self, peer_id: PeerId, data: Vec<u8>) -> Result<()> {
        let connections = self.connections.read().await;
        
        if let Some(address) = connections.get(&peer_id) {
            match address {
                TransportAddress::Bluetooth(_) => {
                    if let Some(bluetooth) = &self.bluetooth {
                        let mut bt = bluetooth.write().await;
                        bt.send(peer_id, data).await
                            .map_err(|e| Error::Network(format!("Bluetooth send failed: {}", e)))?;
                    }
                }
                _ => {
                    return Err(Error::Network("Unsupported transport type".to_string()));
                }
            }
        } else {
            return Err(Error::Network("Peer not connected".to_string()));
        }
        
        Ok(())
    }
    
    /// Broadcast packet to all connected peers
    pub async fn broadcast_packet(&self, packet: BitchatPacket) -> Result<()> {
        let mut serialized_packet = packet.clone();
        let data = serialized_packet.serialize()
            .map_err(|e| Error::Protocol(format!("Packet serialization failed: {}", e)))?;
        
        let connections = self.connections.read().await;
        
        for peer_id in connections.keys() {
            if let Err(e) = self.send_to_peer(*peer_id, data.clone()).await {
                log::warn!("Failed to broadcast to peer {:?}: {}", peer_id, e);
            }
        }
        
        Ok(())
    }
    
    /// Get next transport event
    pub async fn next_event(&self) -> Option<TransportEvent> {
        let mut receiver = self.event_receiver.write().await;
        receiver.recv().await
    }
    
    /// Get list of connected peers
    pub async fn connected_peers(&self) -> Vec<PeerId> {
        self.connections.read().await.keys().copied().collect()
    }
}

/// Transport statistics for monitoring
#[derive(Debug, Clone)]
pub struct TransportStats {
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub connection_count: usize,
    pub error_count: u64,
}

impl Default for TransportStats {
    fn default() -> Self {
        Self {
            bytes_sent: 0,
            bytes_received: 0,
            packets_sent: 0,
            packets_received: 0,
            connection_count: 0,
            error_count: 0,
        }
    }
}