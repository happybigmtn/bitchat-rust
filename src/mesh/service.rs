use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, mpsc, broadcast};
use tokio::time::interval;
use serde::{Serialize, Deserialize};
use crate::protocol::{PeerId, GameId, BitchatPacket};
use crate::transport::TransportEvent;
use crate::error::Result;
use super::components::ComponentManager;
use super::deduplication::MessageDeduplicator;
use super::message_queue::MessageQueue;
use super::game_session::GameSessionManager;
use super::anti_cheat::AntiCheatMonitor;
use crate::token::ProofOfRelay;

/// Configuration for the mesh service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshConfig {
    pub peer_id: PeerId,
    pub message_ttl: u8,
    pub max_message_size: usize,
    pub dedup_window: Duration,
    pub heartbeat_interval: Duration,
    pub session_timeout: Duration,
    pub max_peers: usize,
    pub enable_anti_cheat: bool,
    pub treasury_participation: bool,
}

impl Default for MeshConfig {
    fn default() -> Self {
        Self {
            peer_id: [0u8; 32],
            message_ttl: 64,
            max_message_size: 65536,
            dedup_window: Duration::from_secs(300),
            heartbeat_interval: Duration::from_secs(30),
            session_timeout: Duration::from_secs(120),
            max_peers: 1000,
            enable_anti_cheat: true,
            treasury_participation: true,
        }
    }
}

/// Core mesh service orchestrator
/// 
/// Feynman: Think of this as the "control tower" at an airport.
/// It coordinates all the different services (baggage, fuel, catering)
/// to ensure planes (messages) get where they need to go safely and efficiently.
pub struct MeshService {
    config: MeshConfig,
    components: Arc<ComponentManager>,
    deduplicator: Arc<MessageDeduplicator>,
    message_queue: Arc<MessageQueue>,
    game_sessions: Arc<RwLock<GameSessionManager>>,
    anti_cheat: Arc<AntiCheatMonitor>,
    proof_of_relay: Option<Arc<ProofOfRelay>>,
    
    // Peer management
    peers: Arc<RwLock<HashMap<PeerId, PeerInfo>>>,
    routing_table: Arc<RwLock<RoutingTable>>,
    
    // Event channels
    event_tx: broadcast::Sender<MeshEvent>,
    command_rx: mpsc::Receiver<MeshCommand>,
    
    // Service state
    is_running: Arc<RwLock<bool>>,
    start_time: Instant,
    stats: Arc<RwLock<MeshStatistics>>,
}

/// Information about a connected peer
#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub peer_id: PeerId,
    pub connected_at: Instant,
    pub last_seen: Instant,
    pub latency: Option<Duration>,
    pub capabilities: PeerCapabilities,
    pub trust_score: i32,
    pub active_games: Vec<GameId>,
}

/// Capabilities advertised by a peer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerCapabilities {
    pub supports_compression: bool,
    pub supports_encryption: bool,
    pub max_packet_size: usize,
    pub protocol_version: u16,
    pub is_treasury_node: bool,
    pub gaming_enabled: bool,
}

/// Routing table for efficient message forwarding
pub struct RoutingTable {
    routes: HashMap<PeerId, Vec<Route>>,
    direct_peers: HashMap<PeerId, DirectConnection>,
}

#[derive(Clone)]
struct Route {
    next_hop: PeerId,
    distance: u32,
    last_updated: Instant,
}

#[derive(Clone)]
struct DirectConnection {
    peer_id: PeerId,
    latency: Duration,
    reliability: f32,
}

/// Events emitted by the mesh service
#[derive(Debug, Clone)]
pub enum MeshEvent {
    PeerConnected { peer_id: PeerId, capabilities: PeerCapabilities },
    PeerDisconnected { peer_id: PeerId, reason: String },
    MessageReceived { from: PeerId, packet: BitchatPacket },
    GameSessionStarted { game_id: GameId, participants: Vec<PeerId> },
    GameSessionEnded { game_id: GameId, reason: String },
    AntiCheatAlert { peer_id: PeerId, violation: String },
    TreasuryUpdate { balance: u64, active_games: usize },
}

/// Commands that can be sent to the mesh service
#[derive(Debug)]
pub enum MeshCommand {
    SendMessage { to: PeerId, packet: BitchatPacket },
    BroadcastMessage { packet: BitchatPacket },
    CreateGameSession { game_id: GameId, participants: Vec<PeerId> },
    EndGameSession { game_id: GameId },
    BanPeer { peer_id: PeerId, duration: Duration },
    UpdateTrustScore { peer_id: PeerId, delta: i32 },
}

/// Statistics tracked by the mesh service
#[derive(Debug, Default, Clone)]
pub struct MeshStatistics {
    pub messages_sent: u64,
    pub messages_received: u64,
    pub messages_forwarded: u64,
    pub messages_dropped: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub active_peers: usize,
    pub active_games: usize,
    pub treasury_balance: u64,
    pub anti_cheat_violations: u64,
}

impl MeshService {
    /// Create a new mesh service
    pub fn new(config: MeshConfig) -> (Self, mpsc::Sender<MeshCommand>) {
        let (event_tx, _) = broadcast::channel(1000);
        let (command_tx, command_rx) = mpsc::channel(100);
        
        let components = Arc::new(ComponentManager::new());
        let deduplicator = Arc::new(MessageDeduplicator::new(config.dedup_window));
        let message_queue = Arc::new(MessageQueue::new(1000));
        let game_sessions = Arc::new(RwLock::new(GameSessionManager::new(
            config.peer_id,
            config.treasury_participation,
        )));
        let anti_cheat = Arc::new(AntiCheatMonitor::new(config.enable_anti_cheat));
        
        let service = Self {
            config,
            components,
            deduplicator,
            message_queue,
            game_sessions,
            anti_cheat,
            proof_of_relay: None, // Will be set later via set_proof_of_relay
            peers: Arc::new(RwLock::new(HashMap::new())),
            routing_table: Arc::new(RwLock::new(RoutingTable {
                routes: HashMap::new(),
                direct_peers: HashMap::new(),
            })),
            event_tx,
            command_rx,
            is_running: Arc::new(RwLock::new(false)),
            start_time: Instant::now(),
            stats: Arc::new(RwLock::new(MeshStatistics::default())),
        };
        
        (service, command_tx)
    }
    
    /// Set the proof of relay system for mining rewards
    pub fn set_proof_of_relay(&mut self, proof_of_relay: Arc<ProofOfRelay>) {
        self.proof_of_relay = Some(proof_of_relay);
    }
    
    /// Start the mesh service
    pub async fn start(&mut self) -> Result<()> {
        *self.is_running.write().await = true;
        
        // Start all components
        self.components.start_all().await?;
        
        // Start background tasks
        self.start_heartbeat_task().await;
        self.start_maintenance_task().await;
        self.start_message_processor().await;
        self.start_command_processor().await;
        
        // Start game session manager
        if self.config.treasury_participation {
            self.game_sessions.write().await.start_treasury_bot().await?;
        }
        
        Ok(())
    }
    
    /// Stop the mesh service
    pub async fn stop(&mut self) -> Result<()> {
        *self.is_running.write().await = false;
        
        // Stop all components
        self.components.stop_all().await?;
        
        // Clean up game sessions
        self.game_sessions.write().await.cleanup_all_sessions().await;
        
        Ok(())
    }
    
    /// Process an incoming transport event
    pub async fn handle_transport_event(&self, event: TransportEvent) -> Result<()> {
        match event {
            TransportEvent::Connected { peer_id, .. } => {
                self.handle_peer_connected(peer_id).await?;
            }
            TransportEvent::Disconnected { peer_id, reason } => {
                self.handle_peer_disconnected(peer_id, reason).await?;
            }
            TransportEvent::DataReceived { peer_id, data } => {
                self.handle_data_received(peer_id, data).await?;
            }
            TransportEvent::Error { peer_id, error } => {
                if let Some(peer) = peer_id {
                    self.handle_peer_error(peer, error).await?;
                }
            }
        }
        Ok(())
    }
    
    /// Handle a new peer connection
    async fn handle_peer_connected(&self, peer_id: PeerId) -> Result<()> {
        let peer_info = PeerInfo {
            peer_id,
            connected_at: Instant::now(),
            last_seen: Instant::now(),
            latency: None,
            capabilities: PeerCapabilities {
                supports_compression: true,
                supports_encryption: true,
                max_packet_size: 65536,
                protocol_version: 1,
                is_treasury_node: false,
                gaming_enabled: true,
            },
            trust_score: 0,
            active_games: Vec::new(),
        };
        
        self.peers.write().await.insert(peer_id, peer_info.clone());
        
        // Notify components
        self.components.notify_peer_connected(peer_id).await;
        
        // Emit event
        let _ = self.event_tx.send(MeshEvent::PeerConnected {
            peer_id,
            capabilities: peer_info.capabilities,
        });
        
        // Update stats
        self.stats.write().await.active_peers += 1;
        
        Ok(())
    }
    
    /// Handle a peer disconnection
    async fn handle_peer_disconnected(&self, peer_id: PeerId, reason: String) -> Result<()> {
        // Remove from peers
        if let Some(peer_info) = self.peers.write().await.remove(&peer_id) {
            // Clean up game sessions
            for game_id in &peer_info.active_games {
                self.game_sessions.write().await
                    .handle_player_disconnect(*game_id, peer_id).await;
            }
        }
        
        // Notify components
        self.components.notify_peer_disconnected(peer_id).await;
        
        // Emit event
        let _ = self.event_tx.send(MeshEvent::PeerDisconnected { peer_id, reason });
        
        // Update stats
        let mut stats = self.stats.write().await;
        if stats.active_peers > 0 {
            stats.active_peers -= 1;
        }
        
        Ok(())
    }
    
    /// Handle received data from a peer
    async fn handle_data_received(&self, peer_id: PeerId, data: Vec<u8>) -> Result<()> {
        // Track data size before moving
        let data_len = data.len();
        
        // Deserialize packet
        let mut cursor = std::io::Cursor::new(data);
        let packet = BitchatPacket::deserialize(&mut cursor)?;
        
        // Check for duplicates
        if self.deduplicator.is_duplicate(&packet).await {
            self.stats.write().await.messages_dropped += 1;
            return Ok(());
        }
        
        // Anti-cheat analysis
        if self.config.enable_anti_cheat {
            if let Some(violation) = self.anti_cheat.analyze_packet(&packet, peer_id).await {
                let _ = self.event_tx.send(MeshEvent::AntiCheatAlert {
                    peer_id,
                    violation: violation.clone(),
                });
                self.stats.write().await.anti_cheat_violations += 1;
                
                // Adjust trust score
                self.update_peer_trust(peer_id, -10).await;
                
                // Severe violations result in immediate ban
                if violation.contains("severe") {
                    self.ban_peer(peer_id, Duration::from_secs(3600)).await?;
                    return Ok(());
                }
            }
        }
        
        // Update peer last seen
        if let Some(peer) = self.peers.write().await.get_mut(&peer_id) {
            peer.last_seen = Instant::now();
        }
        
        // Process packet based on type
        match packet.packet_type {
            0x20 | 0x21 | 0x22 | 0x23 => { // Game packets
                self.handle_game_packet(peer_id, packet).await?;
            }
            0x10 => { // Message packet
                self.handle_message_packet(peer_id, packet).await?;
            }
            0x01 => { // Heartbeat
                self.handle_heartbeat(peer_id, packet).await?;
            }
            _ => {
                // Queue for processing by components
                if let Err(e) = self.message_queue.enqueue(packet) {
                    log::warn!("Failed to enqueue message: {}", e);
                }
            }
        }
        
        // Update stats
        self.stats.write().await.messages_received += 1;
        self.stats.write().await.bytes_received += data_len as u64;
        
        Ok(())
    }
    
    /// Handle game-related packets
    async fn handle_game_packet(&self, from: PeerId, packet: BitchatPacket) -> Result<()> {
        self.game_sessions.write().await
            .process_game_packet(from, packet).await?;
        Ok(())
    }
    
    /// Handle regular message packets
    async fn handle_message_packet(&self, from: PeerId, packet: BitchatPacket) -> Result<()> {
        // Emit event for application layer
        let _ = self.event_tx.send(MeshEvent::MessageReceived {
            from,
            packet: packet.clone(),
        });
        
        // Forward if necessary based on TTL
        if packet.ttl > 1 {
            self.forward_packet(packet).await?;
        }
        
        Ok(())
    }
    
    /// Handle heartbeat packets
    async fn handle_heartbeat(&self, from: PeerId, _packet: BitchatPacket) -> Result<()> {
        if let Some(peer) = self.peers.write().await.get_mut(&from) {
            peer.last_seen = Instant::now();
            // Could calculate latency here if packet includes timestamp
        }
        Ok(())
    }
    
    /// Forward a packet to its destination
    async fn forward_packet(&self, mut packet: BitchatPacket) -> Result<()> {
        packet.ttl -= 1;
        
        // Extract routing information
        let source = packet.get_sender().unwrap_or([0u8; 32]);
        let destination = packet.get_receiver().unwrap_or([0u8; 32]);
        let hop_count = 8 - packet.ttl; // Calculate how many hops so far
        
        // Generate packet hash for relay tracking
        let packet_hash = self.calculate_packet_hash(&packet);
        
        // Record relay event for mining rewards
        if let Some(proof_of_relay) = &self.proof_of_relay {
            if let Err(e) = proof_of_relay.record_relay(
                self.config.peer_id,
                packet_hash,
                source,
                destination,
                hop_count,
            ).await {
                log::warn!("Failed to record relay for mining: {}", e);
            }
        }
        
        // Extract target from TLV data if present
        // For now, just count as forwarded
        // Full routing logic would examine TLV fields
        self.stats.write().await.messages_forwarded += 1;
        
        Ok(())
    }
    
    /// Calculate packet hash for relay tracking
    fn calculate_packet_hash(&self, packet: &BitchatPacket) -> [u8; 32] {
        use sha2::{Sha256, Digest};
        
        let mut hasher = Sha256::new();
        hasher.update(&[packet.version, packet.packet_type, packet.flags, packet.ttl]);
        hasher.update(&packet.total_length.to_be_bytes());
        hasher.update(&packet.sequence.to_be_bytes());
        
        // Add TLV data to hash
        for tlv in &packet.tlv_data {
            hasher.update(&[tlv.field_type]);
            hasher.update(&tlv.length.to_be_bytes());
            hasher.update(&tlv.value);
        }
        
        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        hash
    }
    
    /// Handle peer errors
    async fn handle_peer_error(&self, peer_id: PeerId, error: String) -> Result<()> {
        log::error!("Peer {} error: {}", hex::encode(peer_id), error);
        
        // Decrease trust score
        self.update_peer_trust(peer_id, -5).await;
        
        Ok(())
    }
    
    /// Update a peer's trust score
    async fn update_peer_trust(&self, peer_id: PeerId, delta: i32) {
        if let Some(peer) = self.peers.write().await.get_mut(&peer_id) {
            peer.trust_score = (peer.trust_score + delta).max(-100).min(100);
        }
    }
    
    /// Ban a peer for a specified duration
    async fn ban_peer(&self, peer_id: PeerId, duration: Duration) -> Result<()> {
        // Remove from peers
        self.peers.write().await.remove(&peer_id);
        
        // Add to ban list (would be implemented in anti-cheat module)
        self.anti_cheat.ban_peer(peer_id, duration).await;
        
        // Disconnect at transport level
        // transport.disconnect(peer_id).await?;
        
        Ok(())
    }
    
    /// Start heartbeat task
    async fn start_heartbeat_task(&self) {
        let interval_duration = self.config.heartbeat_interval;
        let is_running = self.is_running.clone();
        
        tokio::spawn(async move {
            let mut heartbeat_interval = interval(interval_duration);
            
            while *is_running.read().await {
                heartbeat_interval.tick().await;
                // Send heartbeats to all peers
                // This would be implemented with transport layer
            }
        });
    }
    
    /// Start maintenance task
    async fn start_maintenance_task(&self) {
        let is_running = self.is_running.clone();
        let peers = self.peers.clone();
        let session_timeout = self.config.session_timeout;
        let stats = self.stats.clone();
        
        tokio::spawn(async move {
            let mut maintenance_interval = interval(Duration::from_secs(60));
            
            while *is_running.read().await {
                maintenance_interval.tick().await;
                
                // Clean up timed-out peers
                let now = Instant::now();
                let mut peers_write = peers.write().await;
                let timed_out: Vec<PeerId> = peers_write
                    .iter()
                    .filter(|(_, info)| now - info.last_seen > session_timeout)
                    .map(|(id, _)| *id)
                    .collect();
                
                for peer_id in timed_out {
                    peers_write.remove(&peer_id);
                    if stats.read().await.active_peers > 0 {
                        stats.write().await.active_peers -= 1;
                    }
                }
            }
        });
    }
    
    /// Start message processor task
    async fn start_message_processor(&self) {
        let is_running = self.is_running.clone();
        let message_queue = self.message_queue.clone();
        let components = self.components.clone();
        
        tokio::spawn(async move {
            while *is_running.read().await {
                if let Some(packet) = message_queue.dequeue() {
                    // Process with appropriate component
                    let _ = components.process_packet(packet).await;
                } else {
                    // No messages available, sleep briefly to avoid busy waiting
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            }
        });
    }
    
    /// Start command processor task
    async fn start_command_processor(&mut self) {
        let is_running = self.is_running.clone();
        
        while *is_running.read().await {
            if let Some(command) = self.command_rx.recv().await {
                match command {
                    MeshCommand::SendMessage { to, packet } => {
                        // Implement send logic
                        let _ = to;
                        let _ = packet;
                    }
                    MeshCommand::BroadcastMessage { packet } => {
                        // Implement broadcast logic
                        let _ = packet;
                    }
                    MeshCommand::CreateGameSession { game_id, participants } => {
                        self.game_sessions.write().await
                            .create_session(game_id, participants).await;
                    }
                    MeshCommand::EndGameSession { game_id } => {
                        self.game_sessions.write().await
                            .end_session(game_id).await;
                    }
                    MeshCommand::BanPeer { peer_id, duration } => {
                        let _ = self.ban_peer(peer_id, duration).await;
                    }
                    MeshCommand::UpdateTrustScore { peer_id, delta } => {
                        self.update_peer_trust(peer_id, delta).await;
                    }
                }
            }
        }
    }
    
    /// Get a subscription to mesh events
    pub fn subscribe(&self) -> broadcast::Receiver<MeshEvent> {
        self.event_tx.subscribe()
    }
    
    /// Get current statistics
    pub async fn get_stats(&self) -> MeshStatistics {
        self.stats.read().await.clone()
    }
}