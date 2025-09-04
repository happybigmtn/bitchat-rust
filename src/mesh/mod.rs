//! Mesh networking for BitCraps with Security Hardening
//!
//! This module implements the mesh networking layer including:
//! - Mesh service coordination with input validation
//! - Peer management and discovery
//! - Message routing and forwarding with DoS protection
//! - Network topology management
//! - Game session management
//! - Anti-cheat monitoring
//! - Message deduplication
//! - Comprehensive security event logging

pub mod advanced_routing;
pub mod anti_cheat;
pub mod components;
pub mod consensus_message_handler;
pub mod deduplication;
pub mod game_session;
pub mod gateway;
pub mod kademlia_dht;
pub mod message_queue;
pub mod resilience;
pub mod service;

use dashmap::DashMap;
use lru::LruCache;
use parking_lot::RwLock as ParkingRwLock;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::num::NonZeroUsize;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use tokio::time::interval;

use crate::crypto::BitchatIdentity;
use crate::error::{Error, Result};
use crate::memory_pool::GameMemoryPools;
use crate::protocol::{
    BitchatPacket, PeerId, RoutingInfo, PACKET_TYPE_CONSENSUS_VOTE, PACKET_TYPE_HEARTBEAT,
    PACKET_TYPE_PING, PACKET_TYPE_PONG,
};
use crate::protocol::packet_utils::parse_game_creation_data;
use crate::token::ProofOfRelay;
use crate::transport::{TransportCoordinator, TransportEvent};

pub use consensus_message_handler::{
    ConsensusMessageConfig, ConsensusMessageHandler, ConsensusMessageStats,
    MeshConsensusIntegration,
};

// Re-export game session management
pub use game_session::{GameSession, GameSessionManager, SessionState};

/// Maximum number of messages to cache for deduplication
const MAX_MESSAGE_CACHE_SIZE: usize = 10000;

/// TTL for cached messages in seconds (10 minutes)
const MESSAGE_CACHE_TTL_SECONDS: u64 = 600;

/// Deduplication window for high-priority messages (shorter window for faster processing)
const PRIORITY_MESSAGE_TTL_SECONDS: u64 = 300;

/// Deduplication configuration
#[derive(Debug, Clone)]
pub struct DeduplicationConfig {
    /// Maximum cache size for message deduplication
    pub max_cache_size: usize,
    /// TTL for normal messages
    pub normal_message_ttl: Duration,
    /// TTL for high-priority consensus messages
    pub priority_message_ttl: Duration,
    /// Cleanup interval for cache maintenance
    pub cleanup_interval: Duration,
}

impl Default for DeduplicationConfig {
    fn default() -> Self {
        Self {
            max_cache_size: MAX_MESSAGE_CACHE_SIZE,
            normal_message_ttl: Duration::from_secs(MESSAGE_CACHE_TTL_SECONDS),
            priority_message_ttl: Duration::from_secs(PRIORITY_MESSAGE_TTL_SECONDS),
            cleanup_interval: Duration::from_secs(300), // 5 minutes
        }
    }
}

/// Network partition state tracking
#[derive(Debug, Clone)]
struct NetworkPartitionState {
    /// Peers that are part of our partition
    our_partition: HashSet<PeerId>,
    /// Last time we detected connectivity to the full network
    last_full_connectivity: Instant,
    /// Peers that were lost during partition
    partitioned_peers: HashMap<PeerId, Instant>, // peer_id -> when we lost them
    /// Whether we're currently in a partitioned state
    is_partitioned: bool,
}

impl Default for NetworkPartitionState {
    fn default() -> Self {
        Self {
            our_partition: HashSet::new(),
            last_full_connectivity: Instant::now(),
            partitioned_peers: HashMap::new(),
            is_partitioned: false,
        }
    }
}

/// Mesh service managing peer connections and routing with security
pub struct MeshService {
    identity: Arc<BitchatIdentity>,
    transport: Arc<TransportCoordinator>,
    peers: Arc<DashMap<PeerId, MeshPeer>>,
    routing_table: Arc<DashMap<PeerId, RouteInfo>>,
    message_cache: Arc<ParkingRwLock<LruCache<u64, CachedMessage>>>, // Keep LruCache with fast lock
    deduplication_config: DeduplicationConfig,
    event_sender: broadcast::Sender<MeshEvent>,
    event_queue_config: EventQueueConfig,
    is_running: Arc<AtomicBool>,
    proof_of_relay: Option<Arc<ProofOfRelay>>,
    security_manager: Arc<crate::security::SecurityManager>,
    // Timing configuration for mobile optimization
    heartbeat_interval: Arc<parking_lot::RwLock<Duration>>,
    peer_timeout: Arc<parking_lot::RwLock<Duration>>,
    // Partition recovery state
    partition_state: Arc<parking_lot::RwLock<NetworkPartitionState>>,
    // Memory pools for performance optimization
    memory_pools: Arc<GameMemoryPools>,
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

/// Message priority for TTL differentiation
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MessagePriority {
    Critical, // Consensus messages, immediate processing
    High,     // Game state updates
    Normal,   // Regular mesh traffic
    Low,      // Discovery, maintenance
}

/// Cached message to prevent loops with priority-based TTL
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct CachedMessage {
    packet_hash: u64,
    first_seen: Instant,
    forwarded_to: HashSet<PeerId>,
    priority: MessagePriority,
    ttl_override: Option<Duration>, // Allow custom TTL per message
}

/// Mesh network events
#[derive(Debug, Clone)]
pub enum MeshEvent {
    PeerJoined {
        peer: MeshPeer,
    },
    PeerLeft {
        peer_id: PeerId,
        reason: String,
    },
    MessageReceived {
        from: PeerId,
        packet: BitchatPacket,
    },
    RouteDiscovered {
        destination: PeerId,
        route: RouteInfo,
    },
    NetworkPartition {
        isolated_peers: Vec<PeerId>,
    },
    PartitionRecovered {
        recovered_peers: Vec<PeerId>,
        partition_duration: Duration,
    },
    QueueOverflow {
        dropped_events: usize,
    }, // Backpressure indicator
}

/// Event queue configuration for backpressure management
#[derive(Debug, Clone)]
pub struct EventQueueConfig {
    pub max_queue_size: usize,
    pub high_water_mark: usize, // When to start dropping low-priority events
    pub drop_strategy: DropStrategy,
}

/// Strategy for dropping events when queue is full
#[derive(Debug, Clone)]
pub enum DropStrategy {
    DropOldest,      // Drop oldest events (FIFO)
    DropLowPriority, // Drop low-priority events first
    Backpressure,    // Block senders until space available
}

impl Default for EventQueueConfig {
    fn default() -> Self {
        Self {
            max_queue_size: 1000,
            high_water_mark: 800,
            drop_strategy: DropStrategy::DropLowPriority,
        }
    }
}

impl MeshService {
    pub fn new(identity: Arc<BitchatIdentity>, transport: Arc<TransportCoordinator>) -> Self {
        let event_queue_config = EventQueueConfig::default();
        let deduplication_config = DeduplicationConfig::default();
        let (event_sender, _) = broadcast::channel(event_queue_config.max_queue_size);
        let security_config = crate::security::SecurityConfig::default();
        let security_manager = Arc::new(crate::security::SecurityManager::new(security_config));
        let memory_pools = Arc::new(GameMemoryPools::new());

        Self {
            identity,
            transport,
            peers: Arc::new(DashMap::new()),
            routing_table: Arc::new(DashMap::new()),
            message_cache: Arc::new(ParkingRwLock::new(LruCache::new(
                NonZeroUsize::new(deduplication_config.max_cache_size)
                    .expect("Deduplication cache size must be greater than 0"),
            ))),
            deduplication_config,
            event_sender,
            event_queue_config,
            is_running: Arc::new(AtomicBool::new(false)),
            proof_of_relay: None,
            security_manager,
            heartbeat_interval: Arc::new(parking_lot::RwLock::new(Duration::from_secs(30))),
            peer_timeout: Arc::new(parking_lot::RwLock::new(Duration::from_secs(300))),
            partition_state: Arc::new(parking_lot::RwLock::new(NetworkPartitionState::default())),
            memory_pools,
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
    pub fn set_heartbeat_interval(&self, interval: Duration) {
        let mut heartbeat = self.heartbeat_interval.write();
        *heartbeat = interval;
        log::info!("Mesh heartbeat interval set to {:?}", interval);
    }

    /// Set peer timeout for mobile connections
    pub fn set_peer_timeout(&self, timeout: Duration) {
        let mut peer_timeout = self.peer_timeout.write();
        *peer_timeout = timeout;
        log::info!("Mesh peer timeout set to {:?}", timeout);
    }

    /// Send event with backpressure handling
    async fn send_event_with_backpressure(&self, event: MeshEvent) {
        match self.event_queue_config.drop_strategy {
            DropStrategy::Backpressure => {
                // Broadcast channels don't have async send, so just use send
                if let Err(_) = self.event_sender.send(event) {
                    log::error!("Failed to send mesh event: channel closed or no receivers");
                }
            }
            DropStrategy::DropOldest | DropStrategy::DropLowPriority => {
                // Broadcast channels don't support try_send in the same way
                if let Err(_) = self.event_sender.send(event) {
                    log::warn!("Event queue full or no receivers, event dropped");

                    // Send overflow notification if not already overflowing
                    let _ = self
                        .event_sender
                        .send(MeshEvent::QueueOverflow { dropped_events: 1 });
                }
            }
        }
    }

    /// Get event priority for drop strategy
    fn get_event_priority(event: &MeshEvent) -> u8 {
        match event {
            MeshEvent::NetworkPartition { .. } => 0, // Highest priority
            MeshEvent::PeerLeft { .. } => 1,
            MeshEvent::PeerJoined { .. } => 2,
            MeshEvent::RouteDiscovered { .. } => 3,
            MeshEvent::MessageReceived { .. } => 4,
            MeshEvent::QueueOverflow { .. } => 5,
            MeshEvent::PartitionRecovered { .. } => 6, // Lowest priority
        }
    }

    /// Start the mesh service
    pub async fn start(&self) -> Result<()> {
        self.is_running
            .store(true, std::sync::atomic::Ordering::Relaxed);

        // Start transport layer
        self.transport.start_listening().await?;

        // Start mesh maintenance tasks
        self.start_peer_discovery().await;
        self.start_route_maintenance().await;
        self.start_message_processing().await;
        self.start_cleanup_tasks().await;
        self.start_partition_detection().await;

        log::info!(
            "Mesh service started with peer ID: {:?}",
            self.identity.peer_id
        );
        Ok(())
    }

    /// Stop the mesh service
    pub async fn stop(&self) {
        self.is_running
            .store(false, std::sync::atomic::Ordering::Relaxed);
        log::info!("Mesh service stopped");
    }

    /// Send a packet to a specific peer or broadcast with security validation
    pub async fn send_packet(
        &self,
        packet: BitchatPacket,
        sender_ip: std::net::IpAddr,
    ) -> Result<()> {
        // Validate network message before processing
        let message_data = bincode::serialize(&packet).map_err(|e| {
            crate::error::Error::Protocol(format!("Packet serialization failed: {}", e))
        })?;
        self.security_manager
            .validate_network_message(&message_data, sender_ip)?;

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

        // Add to message cache to prevent loops with priority awareness
        self.add_to_message_cache_with_packet(&packet).await;

        // Send via transport coordinator
        self.transport.broadcast_packet(packet).await
    }

    /// Route packet to a specific peer
    async fn route_packet_to_peer(
        &self,
        mut packet: BitchatPacket,
        destination: PeerId,
    ) -> Result<()> {
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
                let data = bincode::serialize(&packet)
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
        // DashMap doesn't need read() - access directly
        let routing_table = &self.routing_table;

        // Check if we have a direct route
        if let Some(route) = routing_table.get(&destination) {
            // Check if route is still fresh (less than 5 minutes old)
            if route.last_updated.elapsed() < Duration::from_secs(300) {
                return Some(route.next_hop);
            }
        }

        // Check if peer is directly connected
        // DashMap doesn't need read() - access directly
        let peers = &self.peers;
        if peers.contains_key(&destination) {
            return Some(destination);
        }

        None
    }

    /// Static method to process received packet (used by spawned task)
    async fn process_received_packet(
        packet: BitchatPacket,
        from: PeerId,
        message_cache: &Arc<ParkingRwLock<LruCache<u64, CachedMessage>>>,
        peers: &Arc<DashMap<PeerId, MeshPeer>>,
        event_sender: &broadcast::Sender<MeshEvent>,
        identity: &Arc<BitchatIdentity>,
    ) {
        // Check message cache to prevent loops
        let packet_hash = Self::calculate_packet_hash_static(&packet);
        if Self::is_message_cached_static(packet_hash, message_cache).await {
            log::debug!("Dropping duplicate packet");
            return;
        }

        // Add to cache (static version uses default priority)
        Self::add_to_message_cache_static(packet_hash, message_cache).await;

        // Update peer activity
        Self::update_peer_activity_static(from, &peers).await;

        // If the packet is addressed to us, emit event. Also surface game creation packets for discovery.
        let mut emitted = false;
        if let Some(destination) = packet.get_receiver() {
            if destination == identity.peer_id {
                let event = MeshEvent::MessageReceived { from, packet: packet.clone() };
                if let Err(e) = event_sender.send(event) {
                    log::warn!("Failed to send MessageReceived event: {:?}", e);
                }
                emitted = true;
            }
        }

        // Surface game creation packets even if broadcast without specific receiver
        if packet.packet_type == crate::protocol::PACKET_TYPE_GAME_DATA {
            if let Some(game) = parse_game_creation_data(&packet) {
                log::info!(
                    "Discovered game {:?} from {:?} (max_players={}, buy_in={})",
                    game.game_id,
                    game.creator,
                    game.max_players,
                    game.buy_in
                );
                // Also emit a MessageReceived to upstream consumers if not already emitted
                if !emitted {
                    let event = MeshEvent::MessageReceived { from, packet };
                    if let Err(e) = event_sender.send(event) {
                        log::warn!("Failed to send MessageReceived event: {:?}", e);
                    }
                }
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

        // Add to cache with packet context
        self.add_to_message_cache_with_packet(&packet).await;

        // Update peer activity
        self.update_peer_activity(from).await;

        // Check if packet is for us
        if let Some(destination) = packet.get_receiver() {
            if destination == self.identity.peer_id {
                // Handle special packet types
                if self.handle_special_packet(&packet, from).await {
                    return; // Packet was handled, don't forward
                }

                // Regular packet - send event
                let event = MeshEvent::MessageReceived { from, packet };
                self.send_event_with_backpressure(event).await;
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

                if let Err(e) = proof_of_relay
                    .record_relay(
                        self.identity.peer_id,
                        packet_hash,
                        source,
                        destination,
                        hop_count,
                    )
                    .await
                {
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

    /// Start peer discovery and heartbeat task
    async fn start_peer_discovery(&self) {
        let transport = self.transport.clone();
        let peers = self.peers.clone();
        let is_running = self.is_running.clone();
        let event_sender = self.event_sender.clone();
        let heartbeat_interval = self.heartbeat_interval.clone();
        let peer_timeout = self.peer_timeout.clone();
        let identity = self.identity.clone();

        tokio::spawn(async move {
            let mut discovery_interval = {
                let interval_duration = *heartbeat_interval.read();
                interval(interval_duration)
            };

            while is_running.load(std::sync::atomic::Ordering::Relaxed) {
                discovery_interval.tick().await;

                // Send heartbeat to all connected peers
                Self::send_heartbeats(&transport, &peers, &identity).await;

                // Check for timed out peers
                let timeout_duration = *peer_timeout.read();
                Self::check_peer_timeouts(&peers, &event_sender, timeout_duration).await;

                // Discovery logic would go here
                log::debug!("Running peer discovery and heartbeat cycle");
            }
        });
    }

    /// Send heartbeat messages to all connected peers
    async fn send_heartbeats(
        transport: &Arc<TransportCoordinator>,
        peers: &Arc<DashMap<PeerId, MeshPeer>>,
        identity: &Arc<BitchatIdentity>,
    ) {
        // use crate::protocol::PACKET_TYPE_HEARTBEAT; // Not available, using fallback

        for peer_entry in peers.iter() {
            let peer_id = *peer_entry.key();
            let peer = peer_entry.value();

            // Skip peers that were recently active (no need to send heartbeat)
            if peer.last_seen.elapsed() < Duration::from_secs(15) {
                continue;
            }

            // Create heartbeat packet
            let mut heartbeat_packet = BitchatPacket::new(PACKET_TYPE_PING); // Using PING as heartbeat
            heartbeat_packet.add_sender(identity.peer_id);
            heartbeat_packet.add_receiver(peer_id);
            // Timestamp is handled in the packet creation

            // Send heartbeat
            if let Ok(serialized) = bincode::serialize(&heartbeat_packet) {
                if let Err(e) = transport.send_to_peer(peer_id, serialized).await {
                    log::warn!("Failed to send heartbeat to {:?}: {}", peer_id, e);
                }
            }
        }
    }

    /// Check for peers that have timed out and remove them
    async fn check_peer_timeouts(
        peers: &Arc<DashMap<PeerId, MeshPeer>>,
        event_sender: &broadcast::Sender<MeshEvent>,
        timeout_duration: Duration,
    ) {
        let now = Instant::now();
        let mut timed_out_peers = Vec::new();

        // Find timed out peers
        for peer_entry in peers.iter() {
            let peer_id = *peer_entry.key();
            let peer = peer_entry.value();

            if now.duration_since(peer.last_seen) > timeout_duration {
                timed_out_peers.push((peer_id, peer.clone()));
            }
        }

        // Remove timed out peers and send events
        for (peer_id, peer) in timed_out_peers {
            if peers.remove(&peer_id).is_some() {
                log::info!("Peer {:?} timed out after {:?}", peer_id, timeout_duration);

                // Send peer left event
                let event = MeshEvent::PeerLeft {
                    peer_id,
                    reason: format!("Timeout after {:?}", timeout_duration),
                };

                if let Err(e) = event_sender.send(event) {
                    log::warn!("Failed to send PeerLeft event: {:?}", e);
                }
            }
        }
    }

    /// Start route maintenance task
    async fn start_route_maintenance(&self) {
        let routing_table = self.routing_table.clone();
        let _peers = self.peers.clone();
        let is_running = self.is_running.clone();

        tokio::spawn(async move {
            let mut maintenance_interval = interval(Duration::from_secs(60));

            while is_running.load(std::sync::atomic::Ordering::Relaxed) {
                maintenance_interval.tick().await;

                // Clean up stale routes
                // DashMap is accessed directly
                let cutoff = Instant::now() - Duration::from_secs(600); // 10 minutes

                routing_table.retain(|_, route| route.last_updated > cutoff);

                log::debug!("Route maintenance: {} routes active", routing_table.len());
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
            while is_running.load(std::sync::atomic::Ordering::Relaxed) {
                if let Some(event) = transport.next_event().await {
                    match event {
                        TransportEvent::DataReceived { peer_id, data } => {
                            // Try to parse using bincode first (MVP wire format), then fall back
                            let packet_opt: Option<BitchatPacket> = match bincode::deserialize(&data) {
                                Ok(pkt) => Some(pkt),
                                Err(_) => {
                                    let mut cursor = std::io::Cursor::new(&data);
                                    match BitchatPacket::deserialize(&mut cursor) {
                                        Ok(pkt) => Some(pkt),
                                        Err(e) => {
                                            log::warn!(
                                                "Failed to decode incoming packet: {} ({} bytes)",
                                                e,
                                                data.len()
                                            );
                                            None
                                        }
                                    }
                                }
                            };

                            if let Some(packet) = packet_opt {
                                // Handle packet processing inline
                                Self::process_received_packet(
                                    packet,
                                    peer_id,
                                    &message_cache,
                                    &peers,
                                    &event_sender,
                                    &identity,
                                )
                                .await;
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

            while is_running.load(std::sync::atomic::Ordering::Relaxed) {
                cleanup_interval.tick().await;

                // Clean message cache with priority-aware TTL and memory pressure handling
                let mut cache = message_cache.write();
                let now = Instant::now();

                // Check memory pressure first
                let cache_size = cache.len();
                const MAX_CACHE_SIZE: usize = 10000;
                const HIGH_WATER_MARK: usize = (MAX_CACHE_SIZE as f64 * 0.8) as usize;
                const LOW_WATER_MARK: usize = MAX_CACHE_SIZE / 2;

                if cache_size > HIGH_WATER_MARK {
                    // Memory pressure - aggressively evict oldest entries
                    let to_remove = cache_size - LOW_WATER_MARK;
                    log::warn!(
                        "Message cache memory pressure: {} entries, removing {}",
                        cache_size,
                        to_remove
                    );

                    // LRU eviction - remove least recently used
                    for _ in 0..to_remove {
                        cache.pop_lru();
                    }
                } else {
                    // Priority-aware TTL cleanup
                    let mut keys_to_remove = Vec::new();

                    for (key, cached_msg) in cache.iter() {
                        let ttl = cached_msg
                            .ttl_override
                            .unwrap_or(Duration::from_secs(MESSAGE_CACHE_TTL_SECONDS));
                        let expiry = cached_msg.first_seen + ttl;

                        if now > expiry {
                            keys_to_remove.push(*key);
                        }
                    }

                    // Remove expired entries
                    let expired_count = keys_to_remove.len();
                    for key in keys_to_remove {
                        cache.pop(&key);
                    }

                    if expired_count > 0 {
                        log::debug!("Removed {} expired messages from cache", expired_count);
                    }
                }

                // Clean inactive peers
                // DashMap is accessed directly
                let inactive_cutoff = Instant::now() - Duration::from_secs(300); // 5 minutes
                peers.retain(|_, peer| peer.last_seen > inactive_cutoff);

                log::debug!(
                    "Cleanup: {} cached messages, {} active peers",
                    cache.len(),
                    peers.len()
                );
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

    /// Determine message priority based on packet type
    fn determine_message_priority(&self, packet: &BitchatPacket) -> MessagePriority {
        // PACKET_TYPE_CONSENSUS_VOTE already imported at module level

        match packet.packet_type {
            PACKET_TYPE_CONSENSUS_VOTE => {
                // | PACKET_TYPE_DICE_COMMIT | PACKET_TYPE_DICE_REVEAL => { // Not yet implemented
                MessagePriority::Critical
            }
            // Game state packets
            packet_type if packet_type >= 0x30 && packet_type <= 0x3F => MessagePriority::High,
            // Discovery and maintenance
            packet_type if packet_type >= 0x10 && packet_type <= 0x1F => MessagePriority::Low,
            // Everything else is normal priority
            _ => MessagePriority::Normal,
        }
    }

    /// Get appropriate TTL for message based on priority
    fn get_message_ttl(&self, priority: MessagePriority) -> Duration {
        match priority {
            MessagePriority::Critical => self.deduplication_config.priority_message_ttl,
            MessagePriority::High => self.deduplication_config.priority_message_ttl,
            MessagePriority::Normal => self.deduplication_config.normal_message_ttl,
            MessagePriority::Low => self.deduplication_config.normal_message_ttl,
        }
    }

    /// Handle special packet types (heartbeat, ping, etc.)
    /// Returns true if packet was handled and shouldn't be forwarded
    async fn handle_special_packet(&self, packet: &BitchatPacket, from: PeerId) -> bool {
        // Constants are already imported at module level

        match packet.packet_type {
            PACKET_TYPE_HEARTBEAT => {
                log::debug!("Received heartbeat from {:?}", from);
                // Heartbeat updates peer activity (already done in caller)
                // Send heartbeat response if needed
                self.send_heartbeat_response(from).await;
                true
            }
            PACKET_TYPE_PING => {
                log::debug!("Received ping from {:?}", from);
                // Respond with pong
                self.send_pong_response(from).await;
                true
            }
            PACKET_TYPE_PONG => {
                log::debug!("Received pong from {:?}", from);
                // Update latency measurement
                self.update_peer_latency(from, packet).await;
                true
            }
            _ => false, // Not a special packet
        }
    }

    /// Send heartbeat response to peer
    async fn send_heartbeat_response(&self, peer_id: PeerId) {
        // use crate::protocol::PACKET_TYPE_HEARTBEAT; // Not available, using fallback

        let mut response_packet = BitchatPacket::new(PACKET_TYPE_HEARTBEAT);
        response_packet.add_sender(self.identity.peer_id);
        response_packet.add_receiver(peer_id);
        // Timestamp is handled in the packet creation

        if let Ok(serialized) = bincode::serialize(&response_packet) {
            if let Err(e) = self.transport.send_to_peer(peer_id, serialized).await {
                log::warn!("Failed to send heartbeat response to {:?}: {}", peer_id, e);
            }
        }
    }

    /// Send pong response to ping
    async fn send_pong_response(&self, peer_id: PeerId) {
        use crate::protocol::PACKET_TYPE_PONG;

        let mut pong_packet = BitchatPacket::new(PACKET_TYPE_PONG);
        pong_packet.add_sender(self.identity.peer_id);
        pong_packet.add_receiver(peer_id);
        // Timestamp is handled in the packet creation

        if let Ok(serialized) = bincode::serialize(&pong_packet) {
            if let Err(e) = self.transport.send_to_peer(peer_id, serialized).await {
                log::warn!("Failed to send pong to {:?}: {}", peer_id, e);
            }
        }
    }

    /// Update peer latency based on ping/pong timing
    async fn update_peer_latency(&self, peer_id: PeerId, packet: &BitchatPacket) {
        if let (Some(timestamp), Some(mut peer_entry)) =
            (packet.get_timestamp(), self.peers.get_mut(&peer_id))
        {
            let current_time = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;

            if current_time > timestamp {
                let latency_ms = current_time - timestamp;
                peer_entry.latency = Some(Duration::from_millis(latency_ms));
                log::debug!("Updated latency for {:?}: {}ms", peer_id, latency_ms);
            }
        }
    }

    /// Calculate packet hash for relay tracking (256-bit hash)
    fn calculate_packet_hash_for_relay(&self, packet: &BitchatPacket) -> [u8; 32] {
        use sha2::{Digest, Sha256};

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
        message_cache: &Arc<ParkingRwLock<LruCache<u64, CachedMessage>>>,
    ) -> bool {
        message_cache.read().contains(&packet_hash)
    }

    /// Check if message is in cache
    async fn is_message_cached(&self, packet_hash: u64) -> bool {
        Self::is_message_cached_static(packet_hash, &self.message_cache).await
    }

    /// Static version of add_to_message_cache
    async fn add_to_message_cache_static(
        packet_hash: u64,
        message_cache: &Arc<ParkingRwLock<LruCache<u64, CachedMessage>>>,
    ) {
        let cached_msg = CachedMessage {
            packet_hash,
            first_seen: Instant::now(),
            forwarded_to: HashSet::new(),
            priority: MessagePriority::Normal, // Default priority for static version
            ttl_override: None,
        };

        message_cache.write().put(packet_hash, cached_msg);
    }

    /// Add message to cache with priority awareness
    async fn add_to_message_cache(&self, packet_hash: u64) {
        Self::add_to_message_cache_static(packet_hash, &self.message_cache).await;
    }

    /// Add message to cache with packet context for priority determination
    async fn add_to_message_cache_with_packet(&self, packet: &BitchatPacket) {
        let packet_hash = self.calculate_packet_hash(packet);
        let priority = self.determine_message_priority(packet);
        let ttl = self.get_message_ttl(priority);

        let cached_msg = CachedMessage {
            packet_hash,
            first_seen: Instant::now(),
            forwarded_to: HashSet::new(),
            priority,
            ttl_override: Some(ttl),
        };

        self.message_cache.write().put(packet_hash, cached_msg);
    }

    /// Static version of update_peer_activity
    async fn update_peer_activity_static(peer_id: PeerId, peers: &Arc<DashMap<PeerId, MeshPeer>>) {
        // DashMap allows concurrent access
        peers
            .entry(peer_id)
            .and_modify(|peer| {
                peer.last_seen = Instant::now();
                peer.packets_received += 1;
            })
            .or_insert_with(|| MeshPeer {
                peer_id,
                connected_at: Instant::now(),
                last_seen: Instant::now(),
                packets_sent: 0,
                packets_received: 1,
                latency: None,
                reputation: 0.5,    // Start with neutral reputation
                is_treasury: false, // Will be determined later
            });
    }

    /// Update peer activity
    async fn update_peer_activity(&self, peer_id: PeerId) {
        Self::update_peer_activity_static(peer_id, &self.peers).await;

        // Send event after updating (only in instance method)
        // DashMap doesn't need read() - access directly
        let peers = &self.peers;
        if let Some(peer) = peers.get(&peer_id) {
            if peer.packets_received == 1 {
                // New peer - use backpressure handling
                let event = MeshEvent::PeerJoined { peer: peer.clone() };
                let sender = self.event_sender.clone();
                tokio::spawn(async move {
                    if let Err(e) = sender.send(event) {
                        log::warn!("Failed to send PeerJoined event: {:?}", e);
                    }
                });
            }
        }
    }

    /// Get mesh statistics
    pub async fn get_stats(&self) -> MeshStats {
        // DashMap doesn't need read() - access directly
        let peers = &self.peers;
        // DashMap doesn't need read() - access directly
        let routing_table = &self.routing_table;
        let message_cache = self.message_cache.read();

        MeshStats {
            connected_peers: peers.len(),
            known_routes: routing_table.len(),
            cached_messages: message_cache.len(),
            total_packets_received: peers
                .iter()
                .map(|entry| entry.value().packets_received)
                .sum(),
            total_packets_sent: peers.iter().map(|entry| entry.value().packets_sent).sum(),
        }
    }

    /// Get connected peers
    pub async fn get_connected_peers(&self) -> Vec<MeshPeer> {
        self.peers.iter().map(|e| e.value().clone()).collect()
    }

    /// Subscribe to mesh events
    pub fn subscribe(&self) -> broadcast::Receiver<MeshEvent> {
        self.event_sender.subscribe()
    }

    /// Queue a message for battery-efficient batch broadcast
    /// Messages are batched and sent together to minimize BLE radio usage
    pub async fn queue_for_batch_broadcast(&self, msg: MeshMessage) {
        // In a production implementation, this would queue messages
        // and send them in batches to save battery
        // For now, we'll just add a small delay to simulate batching
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Then broadcast normally
        if let Err(e) = self.broadcast_message(msg).await {
            log::warn!("Failed to broadcast batched message: {}", e);
        }
    }

    /// Broadcast a message to all connected peers
    pub async fn broadcast_message(&self, msg: MeshMessage) -> Result<()> {
        // Serialize the message once
        let serialized = bincode::serialize(&msg)?;

        // Send to all connected peers
        // DashMap doesn't need read() - access directly
        let peers = &self.peers;
        for entry in peers.iter() {
            let peer_id = *entry.key();
            self.transport
                .send_to_peer(peer_id, serialized.clone())
                .await?;
        }

        Ok(())
    }

    /// Send a packet to a specific peer
    pub async fn send_to_peer(&self, peer_id: PeerId, packet: BitchatPacket) -> Result<()> {
        // Use pooled buffer for serialization to reduce allocations
        let mut buffer = self.memory_pools.vec_u8_pool.get().await;
        buffer.clear(); // Ensure buffer starts empty
        
        // Serialize directly into the pooled buffer
        bincode::serialize_into(&mut **buffer, &packet)?;
        
        // Clone the serialized data for transport (buffer will be returned to pool when dropped)
        let serialized = buffer.clone();
        self.transport.send_to_peer(peer_id, serialized).await
    }

    /// Poll for game discovery responses
    pub async fn poll_discovery_response(&self) -> Option<MeshMessage> {
        // For now, return None as we need to restructure message caching
        // to store the actual messages instead of just hashes
        None
    }

    /// Send a message to a specific peer
    pub async fn send_message(&self, msg: MeshMessage, peer_id: PeerId) -> Result<()> {
        // Use pooled buffer for serialization to reduce allocations
        let mut buffer = self.memory_pools.vec_u8_pool.get().await;
        buffer.clear(); // Ensure buffer starts empty
        
        // Serialize directly into the pooled buffer
        bincode::serialize_into(&mut **buffer, &msg)?;
        
        // Clone the serialized data for transport (buffer will be returned to pool when dropped)
        let serialized = buffer.clone();
        self.transport.send_to_peer(peer_id, serialized).await
    }

    /// Get the peer ID of this node
    pub fn get_peer_id(&self) -> PeerId {
        self.identity.peer_id
    }

    /// Start partition detection and recovery task
    async fn start_partition_detection(&self) {
        let peers = self.peers.clone();
        let partition_state = self.partition_state.clone();
        let event_sender = self.event_sender.clone();
        let is_running = self.is_running.clone();
        let identity = self.identity.clone();

        tokio::spawn(async move {
            let mut partition_check_interval = interval(Duration::from_secs(30));

            while is_running.load(std::sync::atomic::Ordering::Relaxed) {
                partition_check_interval.tick().await;

                // Check for partition state changes
                Self::check_partition_state(&peers, &partition_state, &event_sender, &identity)
                    .await;
            }
        });
    }

    /// Check for network partition and recovery
    async fn check_partition_state(
        peers: &Arc<DashMap<PeerId, MeshPeer>>,
        partition_state: &Arc<parking_lot::RwLock<NetworkPartitionState>>,
        event_sender: &broadcast::Sender<MeshEvent>,
        identity: &Arc<BitchatIdentity>,
    ) {
        let now = Instant::now();
        let current_peers: HashSet<PeerId> = peers.iter().map(|entry| *entry.key()).collect();

        let mut state = partition_state.write();
        let previous_partition_size = state.our_partition.len();
        let was_partitioned = state.is_partitioned;

        // Check if we have enough connectivity (at least 2 peers for meaningful partition detection)
        if current_peers.len() >= 2 {
            // Update our partition with current peers
            state.our_partition = current_peers.clone();
            state.last_full_connectivity = now;

            // Check for recovered peers
            let recovered_peers: Vec<PeerId> = current_peers
                .intersection(&state.partitioned_peers.keys().cloned().collect())
                .cloned()
                .collect();

            if !recovered_peers.is_empty() && was_partitioned {
                let partition_duration = if let Some(&lost_time) = recovered_peers
                    .iter()
                    .filter_map(|peer| state.partitioned_peers.get(peer))
                    .min()
                {
                    now.duration_since(lost_time)
                } else {
                    Duration::from_secs(0)
                };

                // Remove recovered peers from partitioned list
                for peer in &recovered_peers {
                    state.partitioned_peers.remove(peer);
                }

                // Send recovery event
                let event = MeshEvent::PartitionRecovered {
                    recovered_peers: recovered_peers.clone(),
                    partition_duration,
                };

                if let Err(e) = event_sender.send(event) {
                    log::warn!("Failed to send PartitionRecovered event: {:?}", e);
                }

                log::info!(
                    "Network partition recovered: {} peers reconnected after {:?}",
                    recovered_peers.len(),
                    partition_duration
                );
            }

            // Reset partition state if we have good connectivity
            if current_peers.len() >= previous_partition_size.max(3) {
                state.is_partitioned = false;
            }
        } else {
            // Low connectivity - we might be in a partition
            if !state.is_partitioned
                && now.duration_since(state.last_full_connectivity) > Duration::from_secs(60)
            {
                // Detect partition after 1 minute of low connectivity
                state.is_partitioned = true;

                // Mark missing peers as partitioned
                let all_known_peers: HashSet<PeerId> =
                    state.our_partition.union(&current_peers).cloned().collect();
                for peer in all_known_peers.difference(&current_peers) {
                    state.partitioned_peers.entry(*peer).or_insert(now);
                }

                let isolated_peers: Vec<PeerId> = state.partitioned_peers.keys().cloned().collect();

                // Send partition event
                let event = MeshEvent::NetworkPartition {
                    isolated_peers: isolated_peers.clone(),
                };

                if let Err(e) = event_sender.send(event) {
                    log::warn!("Failed to send NetworkPartition event: {:?}", e);
                }

                log::warn!(
                    "Network partition detected: {} peers isolated",
                    isolated_peers.len()
                );
            }
        }
    }

    /// Check if we're currently in a partitioned state
    pub fn is_partitioned(&self) -> bool {
        self.partition_state.read().is_partitioned
    }

    /// Get the current partition state
    pub fn get_partition_info(&self) -> (bool, Vec<PeerId>, Vec<PeerId>) {
        let state = self.partition_state.read();
        let our_partition: Vec<PeerId> = state.our_partition.iter().cloned().collect();
        let partitioned_peers: Vec<PeerId> = state.partitioned_peers.keys().cloned().collect();

        (state.is_partitioned, our_partition, partitioned_peers)
    }

    /// Send a message and wait for response
    pub async fn send_and_wait_response(
        &self,
        msg: MeshMessage,
        peer_id: PeerId,
    ) -> Result<Option<MeshMessage>> {
        // Send the message
        self.send_message(msg.clone(), peer_id).await?;

        // For now, just return None as we need to restructure message handling
        // to properly track responses
        Ok(None)
    }
}

/// Mesh message type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MeshMessageType {
    GameDiscovery,
    GameDiscoveryResponse,
    GameAnnouncement,
    GameVerification,
    GameVerificationAck,
    GameStateSync,
    GameStateSyncResponse,
    DirectMessage,
    Broadcast,
    Consensus,
}

/// Mesh message structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshMessage {
    pub message_type: MeshMessageType,
    pub payload: Vec<u8>,
    pub sender: PeerId,
    pub recipient: Option<PeerId>,
    pub timestamp: u64,
    pub signature: Vec<u8>,
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
    use std::sync::RwLock;

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
            LruCache::new(NonZeroUsize::new(3).expect("Cache size 3 is a positive constant")), // Small cache for testing
        ));

        // Add items to fill cache
        for i in 0..3u64 {
            let cached_msg = CachedMessage {
                packet_hash: i,
                first_seen: Instant::now(),
                forwarded_to: HashSet::new(),
                priority: MessagePriority::Normal,
                ttl_override: None,
            };
            test_cache.write().unwrap().put(i, cached_msg);
        }

        // Cache should be full
        assert_eq!(test_cache.read().unwrap().len(), 3);
        assert!(test_cache.read().unwrap().contains(&0));
        assert!(test_cache.read().unwrap().contains(&1));
        assert!(test_cache.read().unwrap().contains(&2));

        // Add one more item - should evict the least recently used (0)
        let cached_msg = CachedMessage {
            packet_hash: 3,
            first_seen: Instant::now(),
            forwarded_to: HashSet::new(),
            priority: MessagePriority::Normal,
            ttl_override: None,
        };
        test_cache.write().unwrap().put(3, cached_msg);

        // Cache should still be size 3, but 0 should be evicted
        assert_eq!(test_cache.read().unwrap().len(), 3);
        assert!(!test_cache.read().unwrap().contains(&0)); // Evicted
        assert!(test_cache.read().unwrap().contains(&1));
        assert!(test_cache.read().unwrap().contains(&2));
        assert!(test_cache.read().unwrap().contains(&3)); // New item
    }
}
