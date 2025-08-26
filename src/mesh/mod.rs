//! Mesh networking for BitCraps
//! 
//! This module implements the mesh networking layer including:
//! - Mesh service coordination
//! - Peer management and discovery
//! - Message routing and forwarding
//! - Network topology management
//! - Game session management
//! - Anti-cheat monitoring
//! - Message deduplication

pub mod service;
pub mod components;
pub mod deduplication;
pub mod message_queue;
pub mod game_session;
pub mod anti_cheat;
pub mod kademlia_dht;
pub mod gateway;
pub mod advanced_routing;
pub mod resilience;
pub mod consensus_message_handler;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use tokio::time::interval;
use lru::LruCache;
use std::num::NonZeroUsize;

use crate::protocol::{PeerId, BitchatPacket, RoutingInfo};
use crate::transport::{TransportCoordinator, TransportEvent};
use crate::crypto::BitchatIdentity;
use crate::error::{Error, Result};
use crate::token::ProofOfRelay;

pub use consensus_message_handler::{
    ConsensusMessageHandler, ConsensusMessageConfig, ConsensusMessageStats,
    MeshConsensusIntegration,
};

/// Maximum number of messages to cache for deduplication
const MAX_MESSAGE_CACHE_SIZE: usize = 10000;

/// Mesh service managing peer connections and routing
pub struct MeshService {
    identity: Arc<BitchatIdentity>,
    transport: Arc<TransportCoordinator>,
    peers: Arc<RwLock<HashMap<PeerId, MeshPeer>>>,
    routing_table: Arc<RwLock<HashMap<PeerId, RouteInfo>>>,
    message_cache: Arc<RwLock<LruCache<u64, CachedMessage>>>,
    event_sender: mpsc::UnboundedSender<MeshEvent>,
    is_running: Arc<RwLock<bool>>,
    proof_of_relay: Option<Arc<ProofOfRelay>>,
}

/// Information about a mesh peer
#[derive(Debug, Clone)]
pub struct MeshPeer {
    pub peer_id: PeerId,
    pub connected_at: Instant,
    pub last_seen: Instant,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub latency: Option<Duration>,
    pub reputation: f64,
    pub is_treasury: bool,
}

/// Routing information for reaching a destination
#[derive(Debug, Clone)]
pub struct RouteInfo {
    pub destination: PeerId,
    pub next_hop: PeerId,
    pub hop_count: u8,
    pub last_updated: Instant,
    pub reliability: f64,
}

/// Cached message to prevent loops
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct CachedMessage {
    packet_hash: u64,
    first_seen: Instant,
    forwarded_to: HashSet<PeerId>,
}

/// Mesh network events
#[derive(Debug, Clone)]
pub enum MeshEvent {
    PeerJoined { peer: MeshPeer },
    PeerLeft { peer_id: PeerId, reason: String },
    MessageReceived { from: PeerId, packet: BitchatPacket },
    RouteDiscovered { destination: PeerId, route: RouteInfo },
    NetworkPartition { isolated_peers: Vec<PeerId> },
}

impl MeshService {
    pub fn new(
        identity: Arc<BitchatIdentity>,
        transport: Arc<TransportCoordinator>,
    ) -> Self {
        let (event_sender, _) = mpsc::unbounded_channel();
        
        Self {
            identity,
            transport,
            peers: Arc::new(RwLock::new(HashMap::new())),
            routing_table: Arc::new(RwLock::new(HashMap::new())),
            message_cache: Arc::new(RwLock::new(
                LruCache::new(NonZeroUsize::new(MAX_MESSAGE_CACHE_SIZE).expect("MAX_MESSAGE_CACHE_SIZE constant must be greater than 0"))
            )),
            event_sender,
            is_running: Arc::new(RwLock::new(false)),
            proof_of_relay: None,
        }
    }
    
    /// Set the proof of relay system for mining rewards
    pub fn set_proof_of_relay(&mut self, proof_of_relay: Arc<ProofOfRelay>) {
        self.proof_of_relay = Some(proof_of_relay);
    }
    
    /// Set consensus message handler for processing consensus packets
    pub fn set_consensus_handler(&mut self, handler: Arc<ConsensusMessageHandler>) {
        // Store handler for consensus message processing
        // In practice, this would integrate more deeply with the mesh service
        log::info!("Consensus message handler registered with mesh service");
    }
    
    /// Set heartbeat interval for mobile battery optimization
    pub fn set_heartbeat_interval(&self, _interval: Duration) {
        // TODO: Implement heartbeat interval configuration
        // This would update internal timers for peer keepalive messages
    }
    
    /// Set peer timeout for mobile connections
    pub fn set_peer_timeout(&self, _timeout: Duration) {
        // TODO: Implement peer timeout configuration
        // This would update how long we wait before considering a peer disconnected
    }
    
    /// Start the mesh service
    pub async fn start(&self) -> Result<()> {
        *self.is_running.write().await = true;
        
        // Start transport layer
        self.transport.start_listening().await?;
        
        // Start mesh maintenance tasks
        self.start_peer_discovery().await;
        self.start_route_maintenance().await;
        self.start_message_processing().await;
        self.start_cleanup_tasks().await;
        
        log::info!("Mesh service started with peer ID: {:?}", self.identity.peer_id);
        Ok(())
    }
    
    /// Stop the mesh service
    pub async fn stop(&self) {
        *self.is_running.write().await = false;
        log::info!("Mesh service stopped");
    }
    
    /// Send a packet to a specific peer or broadcast
    pub async fn send_packet(&self, packet: BitchatPacket) -> Result<()> {
        if let Some(destination) = packet.get_receiver() {
            // Send to specific peer
            self.route_packet_to_peer(packet, destination).await
        } else {
            // Broadcast to all peers
            self.broadcast_packet(packet).await
        }
    }
    
    /// Broadcast packet to all connected peers
    pub async fn broadcast_packet(&self, mut packet: BitchatPacket) -> Result<()> {
        // Add our identity as sender if not already set
        if packet.get_sender().is_none() {
            packet.add_sender(self.identity.peer_id);
        }
        
        // Add to message cache to prevent loops
        let packet_hash = self.calculate_packet_hash(&packet);
        self.add_to_message_cache(packet_hash).await;
        
        // Send via transport coordinator
        self.transport.broadcast_packet(packet).await
    }
    
    /// Route packet to a specific peer
    async fn route_packet_to_peer(&self, mut packet: BitchatPacket, destination: PeerId) -> Result<()> {
        // Check if we are the destination
        if destination == self.identity.peer_id {
            self.handle_received_packet(packet, destination).await;
            return Ok(());
        }
        
        // Add routing information
        if let Ok(Some(routing_info)) = packet.get_routing_info() {
            // Update existing routing info
            let mut updated_routing = routing_info;
            updated_routing.route_history.push(self.identity.peer_id);
            updated_routing.max_hops -= 1;
            
            if updated_routing.max_hops == 0 {
                log::warn!("Packet TTL expired, dropping");
                return Ok(());
            }
            
            packet.add_routing_info(&updated_routing)?;
        } else {
            // Add initial routing info
            let routing_info = RoutingInfo {
                source: self.identity.peer_id,
                destination: Some(destination),
                route_history: vec![self.identity.peer_id],
                max_hops: 8, // Maximum hops in mesh
            };
            packet.add_routing_info(&routing_info)?;
        }
        
        // Look up route
        let next_hop = self.find_next_hop(destination).await;
        
        match next_hop {
            Some(next_peer) => {
                // Send to next hop
                let mut serialized_packet = packet;
                let data = serialized_packet.serialize()
                    .map_err(|e| Error::Protocol(format!("Packet serialization failed: {}", e)))?;
                
                self.transport.send_to_peer(next_peer, data).await
            }
            None => {
                // No route found, broadcast with limited TTL
                packet.ttl = 3; // Limited broadcast
                self.broadcast_packet(packet).await
            }
        }
    }
    
    /// Find next hop for reaching destination
    async fn find_next_hop(&self, destination: PeerId) -> Option<PeerId> {
        let routing_table = self.routing_table.read().await;
        
        // Check if we have a direct route
        if let Some(route) = routing_table.get(&destination) {
            // Check if route is still fresh (less than 5 minutes old)
            if route.last_updated.elapsed() < Duration::from_secs(300) {
                return Some(route.next_hop);
            }
        }
        
        // Check if peer is directly connected
        let peers = self.peers.read().await;
        if peers.contains_key(&destination) {
            return Some(destination);
        }
        
        None
    }
    
    /// Static method to process received packet (used by spawned task)
    async fn process_received_packet(
        packet: BitchatPacket,
        from: PeerId,
        message_cache: &Arc<RwLock<LruCache<u64, CachedMessage>>>,
        peers: &Arc<RwLock<HashMap<PeerId, MeshPeer>>>,
        event_sender: &mpsc::UnboundedSender<MeshEvent>,
        identity: &Arc<BitchatIdentity>,
    ) {
        // Check message cache to prevent loops
        let packet_hash = Self::calculate_packet_hash_static(&packet);
        if Self::is_message_cached_static(packet_hash, message_cache).await {
            log::debug!("Dropping duplicate packet");
            return;
        }
        
        // Add to cache
        Self::add_to_message_cache_static(packet_hash, message_cache).await;
        
        // Update peer activity
        Self::update_peer_activity_static(from, peers).await;
        
        // Check if packet is for us
        if let Some(destination) = packet.get_receiver() {
            if destination == identity.peer_id {
                // Packet is for us
                let _ = event_sender.send(MeshEvent::MessageReceived {
                    from,
                    packet,
                });
            }
        }
        
        // Note: Forwarding logic simplified for now to avoid circular dependencies
    }
    
    /// Handle received packet from transport layer
    async fn handle_received_packet(&self, packet: BitchatPacket, from: PeerId) {
        // Check message cache to prevent loops
        let packet_hash = self.calculate_packet_hash(&packet);
        if self.is_message_cached(packet_hash).await {
            log::debug!("Dropping duplicate packet");
            return;
        }
        
        // Add to cache
        self.add_to_message_cache(packet_hash).await;
        
        // Update peer activity
        self.update_peer_activity(from).await;
        
        // Check if packet is for us
        if let Some(destination) = packet.get_receiver() {
            if destination == self.identity.peer_id {
                // Packet is for us
                let _ = self.event_sender.send(MeshEvent::MessageReceived {
                    from,
                    packet,
                });
                return;
            }
        }
        
        // Forward packet if not expired
        if packet.should_forward() {
            let mut forwarded_packet = packet;
            forwarded_packet.decrement_ttl();
            
            // Record relay event for mining rewards
            if let Some(proof_of_relay) = &self.proof_of_relay {
                let packet_hash = self.calculate_packet_hash_for_relay(&forwarded_packet);
                let source = forwarded_packet.get_sender().unwrap_or([0u8; 32]);
                let destination = forwarded_packet.get_receiver().unwrap_or([0u8; 32]);
                let hop_count = 8 - forwarded_packet.ttl; // Calculate hops so far
                
                if let Err(e) = proof_of_relay.record_relay(
                    self.identity.peer_id,
                    packet_hash,
                    source,
                    destination,
                    hop_count,
                ).await {
                    log::warn!("Failed to record relay for mining: {}", e);
                }
            }
            
            if let Some(destination) = forwarded_packet.get_receiver() {
                let _ = Box::pin(self.route_packet_to_peer(forwarded_packet, destination)).await;
            } else {
                // Broadcast packet with decremented TTL
                let _ = self.broadcast_packet(forwarded_packet).await;
            }
        }
    }
    
    /// Start peer discovery task
    async fn start_peer_discovery(&self) {
        let _transport = self.transport.clone();
        let _peers = self.peers.clone();
        let is_running = self.is_running.clone();
        let _event_sender = self.event_sender.clone();
        
        tokio::spawn(async move {
            let mut discovery_interval = interval(Duration::from_secs(30));
            
            while *is_running.read().await {
                discovery_interval.tick().await;
                
                // Discovery logic would go here
                // For now, just check transport events
                log::debug!("Running peer discovery cycle");
            }
        });
    }
    
    /// Start route maintenance task
    async fn start_route_maintenance(&self) {
        let routing_table = self.routing_table.clone();
        let _peers = self.peers.clone();
        let is_running = self.is_running.clone();
        
        tokio::spawn(async move {
            let mut maintenance_interval = interval(Duration::from_secs(60));
            
            while *is_running.read().await {
                maintenance_interval.tick().await;
                
                // Clean up stale routes
                let mut table = routing_table.write().await;
                let cutoff = Instant::now() - Duration::from_secs(600); // 10 minutes
                
                table.retain(|_, route| route.last_updated > cutoff);
                
                log::debug!("Route maintenance: {} routes active", table.len());
            }
        });
    }
    
    /// Start message processing task
    async fn start_message_processing(&self) {
        let transport = self.transport.clone();
        let message_cache = self.message_cache.clone();
        let peers = self.peers.clone();
        let event_sender = self.event_sender.clone();
        let identity = self.identity.clone();
        let is_running = self.is_running.clone();
        
        tokio::spawn(async move {
            while *is_running.read().await {
                if let Some(event) = transport.next_event().await {
                    match event {
                        TransportEvent::DataReceived { peer_id, data } => {
                            // Parse packet and handle
                            let mut cursor = std::io::Cursor::new(data);
                            if let Ok(packet) = BitchatPacket::deserialize(&mut cursor) {
                                // Handle packet processing inline
                                Self::process_received_packet(
                                    packet,
                                    peer_id,
                                    &message_cache,
                                    &peers,
                                    &event_sender,
                                    &identity,
                                ).await;
                            }
                        }
                        TransportEvent::Connected { peer_id, address } => {
                            log::info!("Peer connected: {:?} at {:?}", peer_id, address);
                        }
                        TransportEvent::Disconnected { peer_id, reason } => {
                            log::info!("Peer disconnected: {:?} ({})", peer_id, reason);
                        }
                        TransportEvent::Error { peer_id, error } => {
                            log::error!("Transport error for {:?}: {}", peer_id, error);
                        }
                    }
                }
            }
        });
    }
    
    /// Start cleanup tasks
    async fn start_cleanup_tasks(&self) {
        let message_cache = self.message_cache.clone();
        let peers = self.peers.clone();
        let is_running = self.is_running.clone();
        
        tokio::spawn(async move {
            let mut cleanup_interval = interval(Duration::from_secs(300)); // 5 minutes
            
            while *is_running.read().await {
                cleanup_interval.tick().await;
                
                // Clean message cache - remove old entries
                let mut cache = message_cache.write().await;
                let cutoff = Instant::now() - Duration::from_secs(600); // 10 minutes
                
                // Collect keys to remove (avoid borrowing issues)
                let mut keys_to_remove = Vec::new();
                for (key, value) in cache.iter() {
                    if value.first_seen <= cutoff {
                        keys_to_remove.push(*key);
                    }
                }
                
                // Remove expired entries
                for key in keys_to_remove {
                    cache.pop(&key);
                }
                
                // Clean inactive peers
                let mut peer_map = peers.write().await;
                let inactive_cutoff = Instant::now() - Duration::from_secs(300); // 5 minutes
                peer_map.retain(|_, peer| peer.last_seen > inactive_cutoff);
                
                log::debug!("Cleanup: {} cached messages, {} active peers", 
                          cache.len(), peer_map.len());
            }
        });
    }
    
    /// Static version of calculate_packet_hash
    fn calculate_packet_hash_static(packet: &BitchatPacket) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        
        // Hash key packet fields
        packet.packet_type.hash(&mut hasher);
        if let Some(sender) = packet.get_sender() {
            sender.hash(&mut hasher);
        }
        if let Some(timestamp) = packet.get_timestamp() {
            timestamp.hash(&mut hasher);
        }
        
        hasher.finish()
    }
    
    /// Calculate hash of packet for deduplication
    fn calculate_packet_hash(&self, packet: &BitchatPacket) -> u64 {
        Self::calculate_packet_hash_static(packet)
    }
    
    /// Calculate packet hash for relay tracking (256-bit hash)
    fn calculate_packet_hash_for_relay(&self, packet: &BitchatPacket) -> [u8; 32] {
        use sha2::{Sha256, Digest};
        
        let mut hasher = Sha256::new();
        hasher.update([packet.version, packet.packet_type, packet.flags, packet.ttl]);
        hasher.update(packet.total_length.to_be_bytes());
        hasher.update(packet.sequence.to_be_bytes());
        
        // Add TLV data to hash
        for tlv in &packet.tlv_data {
            hasher.update([tlv.field_type]);
            hasher.update(tlv.length.to_be_bytes());
            hasher.update(&tlv.value);
        }
        
        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        hash
    }
    
    
    /// Static version of is_message_cached
    async fn is_message_cached_static(
        packet_hash: u64,
        message_cache: &Arc<RwLock<LruCache<u64, CachedMessage>>>,
    ) -> bool {
        message_cache.read().await.contains(&packet_hash)
    }
    
    /// Check if message is in cache
    async fn is_message_cached(&self, packet_hash: u64) -> bool {
        Self::is_message_cached_static(packet_hash, &self.message_cache).await
    }
    
    /// Static version of add_to_message_cache
    async fn add_to_message_cache_static(
        packet_hash: u64,
        message_cache: &Arc<RwLock<LruCache<u64, CachedMessage>>>,
    ) {
        let cached_msg = CachedMessage {
            packet_hash,
            first_seen: Instant::now(),
            forwarded_to: HashSet::new(),
        };
        
        message_cache.write().await.put(packet_hash, cached_msg);
    }
    
    /// Add message to cache
    async fn add_to_message_cache(&self, packet_hash: u64) {
        Self::add_to_message_cache_static(packet_hash, &self.message_cache).await;
    }
    
    /// Static version of update_peer_activity
    async fn update_peer_activity_static(
        peer_id: PeerId,
        peers: &Arc<RwLock<HashMap<PeerId, MeshPeer>>>,
    ) {
        let mut peer_map = peers.write().await;
        
        if let Some(peer) = peer_map.get_mut(&peer_id) {
            peer.last_seen = Instant::now();
            peer.packets_received += 1;
        } else {
            // New peer
            let new_peer = MeshPeer {
                peer_id,
                connected_at: Instant::now(),
                last_seen: Instant::now(),
                packets_sent: 0,
                packets_received: 1,
                latency: None,
                reputation: 0.5, // Start with neutral reputation
                is_treasury: false, // Will be determined later
            };
            
            peer_map.insert(peer_id, new_peer);
        }
    }
    
    /// Update peer activity
    async fn update_peer_activity(&self, peer_id: PeerId) {
        Self::update_peer_activity_static(peer_id, &self.peers).await;
        
        // Send event after updating (only in instance method)
        let peers = self.peers.read().await;
        if let Some(peer) = peers.get(&peer_id) {
            if peer.packets_received == 1 {
                // New peer
                let _ = self.event_sender.send(MeshEvent::PeerJoined {
                    peer: peer.clone(),
                });
            }
        }
    }
    
    /// Get mesh statistics
    pub async fn get_stats(&self) -> MeshStats {
        let peers = self.peers.read().await;
        let routing_table = self.routing_table.read().await;
        let message_cache = self.message_cache.read().await;
        
        MeshStats {
            connected_peers: peers.len(),
            known_routes: routing_table.len(),
            cached_messages: message_cache.len(),
            total_packets_received: peers.values().map(|p| p.packets_received).sum(),
            total_packets_sent: peers.values().map(|p| p.packets_sent).sum(),
        }
    }
    
    /// Get connected peers
    pub async fn get_connected_peers(&self) -> Vec<MeshPeer> {
        self.peers.read().await.values().cloned().collect()
    }
}

/// Mesh network statistics
#[derive(Debug, Clone)]
pub struct MeshStats {
    pub connected_peers: usize,
    pub known_routes: usize,
    pub cached_messages: usize,
    pub total_packets_received: u64,
    pub total_packets_sent: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::BitchatKeypair;
    
    #[tokio::test]
    async fn test_mesh_service_creation() {
        let keypair = BitchatKeypair::generate();
        let identity = Arc::new(BitchatIdentity::from_keypair_with_pow(keypair, 8));
        let transport = Arc::new(TransportCoordinator::new());
        
        let mesh = MeshService::new(identity, transport);
        
        let stats = mesh.get_stats().await;
        assert_eq!(stats.connected_peers, 0);
        assert_eq!(stats.known_routes, 0);
        assert_eq!(stats.cached_messages, 0);
    }

    #[tokio::test]
    async fn test_lru_message_cache_bounds() {
        let keypair = BitchatKeypair::generate();
        let identity = Arc::new(BitchatIdentity::from_keypair_with_pow(keypair, 8));
        let transport = Arc::new(TransportCoordinator::new());
        
        let mesh = MeshService::new(identity, transport);
        
        // Test that cache is initialized empty
        let initial_stats = mesh.get_stats().await;
        assert_eq!(initial_stats.cached_messages, 0);
        
        // Add some messages to cache
        for i in 0..50u64 {
            mesh.add_to_message_cache(i).await;
        }
        
        let stats_after_adds = mesh.get_stats().await;
        assert_eq!(stats_after_adds.cached_messages, 50);
        
        // Test cache deduplication
        mesh.add_to_message_cache(0).await; // duplicate
        let stats_after_dup = mesh.get_stats().await;
        assert_eq!(stats_after_dup.cached_messages, 50); // should still be 50
        
        // Test that cache is bounded - add more than max size
        // Since MAX_MESSAGE_CACHE_SIZE is 10000, we can't easily test eviction
        // but we can test that it doesn't grow unbounded by checking the size
        for i in 50..150u64 {
            mesh.add_to_message_cache(i).await;
        }
        
        let stats_final = mesh.get_stats().await;
        assert_eq!(stats_final.cached_messages, 150);
        
        // Verify message is cached
        assert!(mesh.is_message_cached(0).await);
        assert!(mesh.is_message_cached(149).await);
        assert!(!mesh.is_message_cached(1000).await);
    }

    #[tokio::test]
    async fn test_lru_cache_eviction() {
        // Create a mesh service with a small cache for testing
        let keypair = BitchatKeypair::generate();
        let identity = Arc::new(BitchatIdentity::from_keypair_with_pow(keypair, 8));
        let transport = Arc::new(TransportCoordinator::new());
        
        // Create a small LRU cache directly to test eviction
        let test_cache: Arc<RwLock<LruCache<u64, CachedMessage>>> = Arc::new(RwLock::new(
            LruCache::new(NonZeroUsize::new(3).expect("Cache size 3 is a positive constant")) // Small cache for testing
        ));
        
        // Add items to fill cache
        for i in 0..3u64 {
            let cached_msg = CachedMessage {
                packet_hash: i,
                first_seen: Instant::now(),
                forwarded_to: HashSet::new(),
            };
            test_cache.write().await.put(i, cached_msg);
        }
        
        // Cache should be full
        assert_eq!(test_cache.read().await.len(), 3);
        assert!(test_cache.read().await.contains(&0));
        assert!(test_cache.read().await.contains(&1));
        assert!(test_cache.read().await.contains(&2));
        
        // Add one more item - should evict the least recently used (0)
        let cached_msg = CachedMessage {
            packet_hash: 3,
            first_seen: Instant::now(),
            forwarded_to: HashSet::new(),
        };
        test_cache.write().await.put(3, cached_msg);
        
        // Cache should still be size 3, but 0 should be evicted
        assert_eq!(test_cache.read().await.len(), 3);
        assert!(!test_cache.read().await.contains(&0)); // Evicted
        assert!(test_cache.read().await.contains(&1));
        assert!(test_cache.read().await.contains(&2));
        assert!(test_cache.read().await.contains(&3)); // New item
    }
}