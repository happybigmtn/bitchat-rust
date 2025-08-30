//! Transport trait definitions

use crate::protocol::PeerId;
use crate::transport::{TransportAddress, TransportEvent};
use async_trait::async_trait;

/// Core transport trait - defines what any transport must do
#[async_trait]
pub trait Transport: Send + Sync {
    /// Start listening on the specified address
    async fn listen(&mut self, address: TransportAddress)
        -> Result<(), Box<dyn std::error::Error>>;

    /// Connect to a peer at the specified address
    async fn connect(
        &mut self,
        address: TransportAddress,
    ) -> Result<PeerId, Box<dyn std::error::Error>>;

    /// Send data to a connected peer
    async fn send(
        &mut self,
        peer_id: PeerId,
        data: Vec<u8>,
    ) -> Result<(), Box<dyn std::error::Error>>;

    /// Disconnect from a peer
    async fn disconnect(&mut self, peer_id: PeerId) -> Result<(), Box<dyn std::error::Error>>;

    /// Check if connected to a peer
    fn is_connected(&self, peer_id: &PeerId) -> bool;

    /// Get list of connected peers
    fn connected_peers(&self) -> Vec<PeerId>;

    /// Receive the next transport event
    async fn next_event(&mut self) -> Option<TransportEvent>;
}
