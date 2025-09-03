//! Networking API
//!
//! High-level API for peer-to-peer networking, connection management,
//! and network topology operations.

use crate::sdk_v2::{
    error::{SDKError, SDKResult},
    types::*,
    rest::RestClient,
    SDKContext,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;
use std::net::SocketAddr;

/// Network management API
#[derive(Debug)]
pub struct NetworkAPI {
    context: Arc<SDKContext>,
    rest_client: RestClient,
    connected_peers: Arc<RwLock<HashMap<PeerId, PeerConnection>>>,
    network_stats: Arc<RwLock<NetworkStatistics>>,
}

/// Peer connection information
#[derive(Debug, Clone)]
struct PeerConnection {
    pub peer_info: PeerInfo,
    pub connection_time: chrono::DateTime<chrono::Utc>,
    pub last_activity: chrono::DateTime<chrono::Utc>,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub message_count: u64,
}

impl NetworkAPI {
    /// Create a new network API instance
    pub fn new(context: Arc<SDKContext>) -> Self {
        let rest_client = RestClient::new(&context.config)
            .expect("Failed to create REST client");
        
        Self {
            context,
            rest_client,
            connected_peers: Arc::new(RwLock::new(HashMap::new())),
            network_stats: Arc::new(RwLock::new(NetworkStatistics::default())),
        }
    }
    
    /// Get list of all peers in the network
    pub async fn get_peers(&self) -> SDKResult<Vec<PeerInfo>> {
        let peers: Vec<PeerInfo> = self.rest_client
            .get("network/peers")
            .await?;
        
        Ok(peers)
    }
    
    /// Get list of connected peers
    pub async fn get_connected_peers(&self) -> Vec<PeerInfo> {
        let peers = self.connected_peers.read().await;
        peers.values().map(|conn| conn.peer_info.clone()).collect()
    }
    
    /// Connect to a specific peer
    pub async fn connect(&self, peer_address: &str) -> SDKResult<PeerId> {
        let request = ConnectPeerRequest {
            address: peer_address.to_string(),
            timeout_seconds: 30,
        };
        
        let response: ConnectPeerResponse = self.rest_client
            .post("network/peers/connect", request)
            .await?;
        
        // Update local peer list
        if response.success {
            let peer_connection = PeerConnection {
                peer_info: response.peer_info.clone(),
                connection_time: chrono::Utc::now(),
                last_activity: chrono::Utc::now(),
                bytes_sent: 0,
                bytes_received: 0,
                message_count: 0,
            };
            
            let mut peers = self.connected_peers.write().await;
            peers.insert(response.peer_info.peer_id.clone(), peer_connection);
        }
        
        Ok(response.peer_info.peer_id)
    }
    
    /// Disconnect from a specific peer
    pub async fn disconnect(&self, peer_id: &PeerId) -> SDKResult<()> {
        let _: serde_json::Value = self.rest_client
            .delete(&format!("network/peers/{}", peer_id))
            .await?;
        
        // Remove from local peer list
        {
            let mut peers = self.connected_peers.write().await;
            peers.remove(peer_id);
        }
        
        Ok(())
    }
    
    /// Get detailed peer information
    pub async fn get_peer_info(&self, peer_id: &PeerId) -> SDKResult<DetailedPeerInfo> {
        let peer_info: DetailedPeerInfo = self.rest_client
            .get(&format!("network/peers/{}", peer_id))
            .await?;
        
        Ok(peer_info)
    }
    
    /// Send a direct message to a peer
    pub async fn send_message(&self, peer_id: &PeerId, message: PeerMessage) -> SDKResult<()> {
        let request = SendMessageRequest {
            recipient: peer_id.clone(),
            message,
            priority: MessagePriority::Normal,
        };
        
        let _: serde_json::Value = self.rest_client
            .post(&format!("network/peers/{}/messages", peer_id), request)
            .await?;
        
        // Update local statistics
        {
            let mut peers = self.connected_peers.write().await;
            if let Some(peer) = peers.get_mut(peer_id) {
                peer.message_count += 1;
                peer.last_activity = chrono::Utc::now();
            }
        }
        
        Ok(())
    }
    
    /// Broadcast a message to all connected peers
    pub async fn broadcast_message(&self, message: PeerMessage) -> SDKResult<BroadcastResult> {
        let request = BroadcastMessageRequest {
            message,
            exclude_peers: None,
            priority: MessagePriority::Normal,
        };
        
        let result: BroadcastResult = self.rest_client
            .post("network/broadcast", request)
            .await?;
        
        Ok(result)
    }
    
    /// Get network topology information
    pub async fn get_network_topology(&self) -> SDKResult<NetworkTopology> {
        let topology: NetworkTopology = self.rest_client
            .get("network/topology")
            .await?;
        
        Ok(topology)
    }
    
    /// Get network statistics
    pub async fn get_network_statistics(&self) -> NetworkStatistics {
        self.network_stats.read().await.clone()
    }
    
    /// Discover peers on the local network
    pub async fn discover_local_peers(&self) -> SDKResult<Vec<PeerInfo>> {
        let peers: Vec<PeerInfo> = self.rest_client
            .post("network/discover", DiscoverPeersRequest {
                discovery_type: DiscoveryType::Local,
                timeout_seconds: 10,
            })
            .await?;
        
        Ok(peers)
    }
    
    /// Join a specific network/mesh
    pub async fn join_network(&self, network_id: &str) -> SDKResult<NetworkJoinResult> {
        let request = JoinNetworkRequest {
            network_id: network_id.to_string(),
            capabilities: vec![
                "craps_game".to_string(),
                "consensus_voting".to_string(),
                "peer_discovery".to_string(),
            ],
        };
        
        let result: NetworkJoinResult = self.rest_client
            .post("network/join", request)
            .await?;
        
        Ok(result)
    }
    
    /// Leave a network/mesh
    pub async fn leave_network(&self, network_id: &str) -> SDKResult<()> {
        let _: serde_json::Value = self.rest_client
            .delete(&format!("network/{}", network_id))
            .await?;
        
        Ok(())
    }
    
    /// Get connection quality metrics
    pub async fn get_connection_quality(&self, peer_id: &PeerId) -> SDKResult<ConnectionQuality> {
        let quality: ConnectionQuality = self.rest_client
            .get(&format!("network/peers/{}/quality", peer_id))
            .await?;
        
        Ok(quality)
    }
    
    /// Test connection to a peer
    pub async fn ping_peer(&self, peer_id: &PeerId) -> SDKResult<PingResult> {
        let result: PingResult = self.rest_client
            .post(&format!("network/peers/{}/ping", peer_id), serde_json::json!({}))
            .await?;
        
        Ok(result)
    }
    
    /// Get NAT traversal information
    pub async fn get_nat_info(&self) -> SDKResult<NATInfo> {
        let nat_info: NATInfo = self.rest_client
            .get("network/nat")
            .await?;
        
        Ok(nat_info)
    }
    
    /// Configure network settings
    pub async fn configure_network(&self, config: NetworkConfiguration) -> SDKResult<()> {
        let _: serde_json::Value = self.rest_client
            .put("network/config", config)
            .await?;
        
        Ok(())
    }
    
    /// Get network events (recent connections, disconnections, etc.)
    pub async fn get_network_events(&self, limit: Option<u32>) -> SDKResult<Vec<NetworkEvent>> {
        let path = if let Some(limit) = limit {
            format!("network/events?limit={}", limit)
        } else {
            "network/events".to_string()
        };
        
        let events: Vec<NetworkEvent> = self.rest_client
            .get(&path)
            .await?;
        
        Ok(events)
    }
}

/// Request/Response structures
#[derive(Debug, Serialize)]
struct ConnectPeerRequest {
    address: String,
    timeout_seconds: u64,
}

#[derive(Debug, Deserialize)]
struct ConnectPeerResponse {
    success: bool,
    peer_info: PeerInfo,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct SendMessageRequest {
    recipient: PeerId,
    message: PeerMessage,
    priority: MessagePriority,
}

#[derive(Debug, Serialize)]
struct BroadcastMessageRequest {
    message: PeerMessage,
    exclude_peers: Option<Vec<PeerId>>,
    priority: MessagePriority,
}

#[derive(Debug, Serialize)]
struct DiscoverPeersRequest {
    discovery_type: DiscoveryType,
    timeout_seconds: u64,
}

#[derive(Debug, Serialize)]
struct JoinNetworkRequest {
    network_id: String,
    capabilities: Vec<String>,
}

/// Network-related types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailedPeerInfo {
    #[serde(flatten)]
    pub basic_info: PeerInfo,
    pub connection_history: Vec<ConnectionHistoryEntry>,
    pub game_participation: Vec<GameParticipation>,
    pub reputation_metrics: ReputationMetrics,
    pub technical_capabilities: TechnicalCapabilities,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionHistoryEntry {
    pub connected_at: chrono::DateTime<chrono::Utc>,
    pub disconnected_at: Option<chrono::DateTime<chrono::Utc>>,
    pub duration_seconds: Option<u64>,
    pub disconnect_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameParticipation {
    pub game_id: GameId,
    pub joined_at: chrono::DateTime<chrono::Utc>,
    pub left_at: Option<chrono::DateTime<chrono::Utc>>,
    pub role: String,
    pub performance_score: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReputationMetrics {
    pub trust_score: f64,
    pub reliability_score: f64,
    pub consensus_participation: f64,
    pub cheating_reports: u32,
    pub positive_feedback: u32,
    pub negative_feedback: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TechnicalCapabilities {
    pub supported_protocols: Vec<String>,
    pub max_bandwidth: Option<u64>,
    pub nat_type: Option<String>,
    pub encryption_support: Vec<String>,
    pub platform: String,
    pub version: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum MessagePriority {
    Low,
    Normal,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PeerMessage {
    GameInvite { game_id: GameId, message: String },
    ChatMessage { content: String },
    GameUpdate { game_id: GameId, update: serde_json::Value },
    ConsensusRequest { proposal_id: ProposalId },
    SystemMessage { message_type: String, payload: serde_json::Value },
    Custom { message_type: String, data: serde_json::Value },
}

#[derive(Debug, Deserialize)]
pub struct BroadcastResult {
    pub total_peers: u32,
    pub successful_sends: u32,
    pub failed_sends: u32,
    pub errors: Vec<BroadcastError>,
}

#[derive(Debug, Deserialize)]
pub struct BroadcastError {
    pub peer_id: PeerId,
    pub error: String,
}

#[derive(Debug, Deserialize)]
pub struct NetworkTopology {
    pub total_peers: u32,
    pub connected_peers: u32,
    pub network_diameter: u32,
    pub clustering_coefficient: f64,
    pub peer_connections: Vec<PeerConnection>,
    pub network_partitions: Vec<NetworkPartition>,
}

#[derive(Debug, Deserialize)]
pub struct NetworkPartition {
    pub partition_id: String,
    pub peer_count: u32,
    pub peers: Vec<PeerId>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetworkStatistics {
    pub total_connections: u64,
    pub active_connections: u32,
    pub messages_sent: u64,
    pub messages_received: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub connection_uptime: f64,
    pub average_latency: f64,
    pub packet_loss_rate: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum DiscoveryType {
    Local,
    DHT,
    Bootstrap,
    Broadcast,
}

#[derive(Debug, Deserialize)]
pub struct NetworkJoinResult {
    pub success: bool,
    pub network_id: String,
    pub assigned_peer_id: PeerId,
    pub bootstrap_peers: Vec<PeerInfo>,
    pub network_config: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct ConnectionQuality {
    pub latency_ms: f64,
    pub packet_loss_percent: f64,
    pub bandwidth_mbps: f64,
    pub jitter_ms: f64,
    pub quality_score: f64, // 0.0 to 1.0
    pub is_stable: bool,
}

#[derive(Debug, Deserialize)]
pub struct PingResult {
    pub success: bool,
    pub round_trip_time_ms: f64,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub peer_response: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct NATInfo {
    pub nat_type: NATType,
    pub public_address: Option<SocketAddr>,
    pub local_address: SocketAddr,
    pub port_mapping_supported: bool,
    pub upnp_available: bool,
    pub stun_servers: Vec<String>,
    pub turn_servers: Vec<String>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub enum NATType {
    OpenInternet,
    FullCone,
    RestrictedCone,
    PortRestrictedCone,
    Symmetric,
    Blocked,
    Unknown,
}

#[derive(Debug, Serialize)]
pub struct NetworkConfiguration {
    pub max_connections: u32,
    pub connection_timeout: u64,
    pub heartbeat_interval: u64,
    pub enable_upnp: bool,
    pub preferred_protocols: Vec<String>,
    pub bandwidth_limit: Option<u64>,
    pub discovery_settings: DiscoverySettings,
}

#[derive(Debug, Serialize)]
pub struct DiscoverySettings {
    pub enable_local_discovery: bool,
    pub enable_dht: bool,
    pub bootstrap_nodes: Vec<String>,
    pub discovery_interval: u64,
}

#[derive(Debug, Deserialize)]
pub struct NetworkEvent {
    pub event_type: NetworkEventType,
    pub peer_id: Option<PeerId>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub details: serde_json::Value,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub enum NetworkEventType {
    PeerConnected,
    PeerDisconnected,
    MessageReceived,
    MessageSent,
    NetworkJoined,
    NetworkLeft,
    TopologyChanged,
    QualityChanged,
}

/// Network utility functions
pub struct NetworkUtils;

impl NetworkUtils {
    /// Calculate network health score based on various metrics
    pub fn calculate_network_health(stats: &NetworkStatistics) -> f64 {
        let latency_score = if stats.average_latency < 50.0 { 1.0 } else { (100.0 - stats.average_latency).max(0.0) / 100.0 };
        let packet_loss_score = (1.0 - stats.packet_loss_rate).max(0.0);
        let uptime_score = (stats.connection_uptime / 100.0).min(1.0);
        
        (latency_score + packet_loss_score + uptime_score) / 3.0
    }
    
    /// Recommend optimal peers based on quality metrics
    pub fn recommend_peers(peers: &[DetailedPeerInfo], count: usize) -> Vec<PeerId> {
        let mut peer_scores: Vec<(PeerId, f64)> = peers
            .iter()
            .map(|peer| {
                let reputation_score = peer.reputation_metrics.trust_score * 0.4
                    + peer.reputation_metrics.reliability_score * 0.3
                    + peer.reputation_metrics.consensus_participation * 0.3;
                (peer.basic_info.peer_id.clone(), reputation_score)
            })
            .collect();
        
        peer_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        peer_scores.into_iter().take(count).map(|(id, _)| id).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sdk_v2::{config::{Config, Environment}, init};
    
    #[tokio::test]
    async fn test_network_api_creation() {
        let config = Config::builder()
            .api_key("test-key")
            .environment(Environment::Testing)
            .build()
            .unwrap();
            
        let context = init(config).await.unwrap();
        let network_api = NetworkAPI::new(context);
        
        // Test that the API was created successfully
        assert_eq!(network_api.get_connected_peers().await.len(), 0);
    }
    
    #[test]
    fn test_network_health_calculation() {
        let stats = NetworkStatistics {
            average_latency: 25.0,
            packet_loss_rate: 0.01,
            connection_uptime: 95.0,
            ..Default::default()
        };
        
        let health = NetworkUtils::calculate_network_health(&stats);
        assert!(health > 0.9); // Should be high quality
        
        let poor_stats = NetworkStatistics {
            average_latency: 200.0,
            packet_loss_rate: 0.15,
            connection_uptime: 60.0,
            ..Default::default()
        };
        
        let poor_health = NetworkUtils::calculate_network_health(&poor_stats);
        assert!(poor_health < 0.7); // Should be poor quality
    }
    
    #[test]
    fn test_peer_recommendations() {
        let peers = vec![
            DetailedPeerInfo {
                basic_info: PeerInfo {
                    peer_id: "peer1".to_string(),
                    address: "127.0.0.1:8080".to_string(),
                    status: PeerStatus::Connected,
                    last_seen: chrono::Utc::now(),
                    version: "1.0.0".to_string(),
                    capabilities: vec![],
                    latency: Some(50),
                },
                connection_history: vec![],
                game_participation: vec![],
                reputation_metrics: ReputationMetrics {
                    trust_score: 0.9,
                    reliability_score: 0.95,
                    consensus_participation: 0.8,
                    cheating_reports: 0,
                    positive_feedback: 10,
                    negative_feedback: 1,
                },
                technical_capabilities: TechnicalCapabilities {
                    supported_protocols: vec!["TCP".to_string()],
                    max_bandwidth: Some(1000),
                    nat_type: Some("OpenInternet".to_string()),
                    encryption_support: vec!["TLS".to_string()],
                    platform: "Linux".to_string(),
                    version: "1.0.0".to_string(),
                },
            }
        ];
        
        let recommendations = NetworkUtils::recommend_peers(&peers, 1);
        assert_eq!(recommendations.len(), 1);
        assert_eq!(recommendations[0], "peer1");
    }
}