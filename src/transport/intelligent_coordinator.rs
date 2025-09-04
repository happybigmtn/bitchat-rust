//! Intelligent Transport Coordinator
//!
//! This module provides intelligent transport selection and failover:
//! - Multi-transport coordination (BLE, UDP, TCP, TLS)
//! - NAT traversal integration
//! - Automatic failover with health monitoring
//! - Load balancing across transports
//! - Support for 8+ concurrent players

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Mutex, RwLock};

use crate::error::{Error, Result};
use crate::protocol::PeerId;
#[cfg(feature = "nat-traversal")]
use crate::transport::nat_traversal::NetworkHandler;
use crate::transport::{Transport, TransportAddress, TransportEvent};

/// Transport priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TransportPriority {
    Critical = 0, // Reserved for essential communications
    High = 1,     // Game-critical data
    Normal = 2,   // Regular game traffic
    Low = 3,      // Background/maintenance traffic
}

/// Transport performance metrics
#[derive(Debug, Clone)]
pub struct TransportMetrics {
    pub latency: Duration,
    pub packet_loss: f32,
    pub throughput: u64,
    pub reliability: f32,
    pub last_updated: Instant,
}

impl Default for TransportMetrics {
    fn default() -> Self {
        Self {
            latency: Duration::from_millis(100),
            packet_loss: 0.0,
            throughput: 0,
            reliability: 1.0,
            last_updated: Instant::now(),
        }
    }
}

/// Transport capability flags
#[derive(Debug, Clone)]
pub struct TransportCapabilities {
    pub supports_broadcast: bool,
    pub supports_multicast: bool,
    pub max_message_size: usize,
    pub max_connections: usize,
    pub requires_pairing: bool,
    pub encryption_available: bool,
}

/// Managed transport instance
pub struct ManagedTransport {
    pub transport_id: String,
    pub transport_type: TransportType,
    pub transport: Box<dyn Transport>,
    pub metrics: TransportMetrics,
    pub capabilities: TransportCapabilities,
    pub health_status: TransportHealth,
    pub active_connections: HashMap<PeerId, ConnectionInfo>,
    pub last_health_check: Instant,
    pub priority_score: f32,
}

/// Transport types supported by the system
#[derive(Debug, Clone, PartialEq)]
pub enum TransportType {
    Ble,
    Udp,
    UdpWithNatTraversal,
    Tcp,
    TcpTls,
    TurnRelay,
}

/// Transport health status
#[derive(Debug, Clone, PartialEq)]
pub enum TransportHealth {
    Optimal,
    Good,
    Degraded,
    Critical,
    Failed,
}

/// Connection information for each peer
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub peer_id: PeerId,
    pub address: TransportAddress,
    pub established_at: Instant,
    pub last_activity: Instant,
    pub messages_sent: u64,
    pub messages_received: u64,
    pub error_count: u64,
}

/// Intelligent transport coordinator configuration
#[derive(Debug, Clone)]
pub struct IntelligentCoordinatorConfig {
    pub max_transports_per_peer: usize,
    pub health_check_interval: Duration,
    pub failover_timeout: Duration,
    pub metric_update_interval: Duration,
    pub load_balance_threshold: f32,
    pub enable_adaptive_routing: bool,
    pub enable_predictive_failover: bool,
}

impl Default for IntelligentCoordinatorConfig {
    fn default() -> Self {
        Self {
            max_transports_per_peer: 3,
            health_check_interval: Duration::from_secs(10),
            failover_timeout: Duration::from_secs(5),
            metric_update_interval: Duration::from_secs(1),
            load_balance_threshold: 0.8,
            enable_adaptive_routing: true,
            enable_predictive_failover: true,
        }
    }
}

/// Intelligent transport coordinator
pub struct IntelligentTransportCoordinator {
    config: IntelligentCoordinatorConfig,
    transports: Arc<RwLock<HashMap<String, ManagedTransport>>>,
    peer_connections: Arc<RwLock<HashMap<PeerId, Vec<String>>>>, // peer_id -> transport_ids
    nat_handler: Arc<NetworkHandler>,
    event_sender: mpsc::Sender<TransportEvent>,
    event_receiver: Arc<Mutex<mpsc::Receiver<TransportEvent>>>,
    routing_table: Arc<RwLock<HashMap<PeerId, String>>>, // peer_id -> preferred_transport_id
    performance_history: Arc<RwLock<HashMap<String, Vec<TransportMetrics>>>>,
}

impl IntelligentTransportCoordinator {
    /// Create new intelligent transport coordinator
    pub fn new(config: IntelligentCoordinatorConfig, nat_handler: NetworkHandler) -> Self {
        let (event_sender, event_receiver) = mpsc::channel(10000); // Critical path: high-capacity for coordination events

        Self {
            config,
            transports: Arc::new(RwLock::new(HashMap::new())),
            peer_connections: Arc::new(RwLock::new(HashMap::new())),
            nat_handler: Arc::new(nat_handler),
            event_sender,
            event_receiver: Arc::new(Mutex::new(event_receiver)),
            routing_table: Arc::new(RwLock::new(HashMap::new())),
            performance_history: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add a transport to the coordinator
    pub async fn add_transport(
        &self,
        transport_id: String,
        transport_type: TransportType,
        transport: Box<dyn Transport>,
        capabilities: TransportCapabilities,
    ) -> Result<()> {
        let managed_transport = ManagedTransport {
            transport_id: transport_id.clone(),
            transport_type,
            transport,
            metrics: TransportMetrics::default(),
            capabilities,
            health_status: TransportHealth::Good,
            active_connections: HashMap::new(),
            last_health_check: Instant::now(),
            priority_score: 0.5, // Start with neutral score
        };

        {
            let mut transports = self.transports.write().await;
            transports.insert(transport_id.clone(), managed_transport);
        }

        println!("Added transport: {}", transport_id);
        self.start_transport_monitoring(transport_id).await;

        Ok(())
    }

    /// Intelligently select best transport for a peer
    pub async fn select_optimal_transport(
        &self,
        peer_id: PeerId,
        message_priority: TransportPriority,
    ) -> Result<String> {
        let transports = self.transports.read().await;
        let routing_table = self.routing_table.read().await;

        // Check if we have a preferred transport for this peer
        if let Some(preferred_transport_id) = routing_table.get(&peer_id) {
            if let Some(transport) = transports.get(preferred_transport_id) {
                if transport.health_status != TransportHealth::Failed
                    && transport.health_status != TransportHealth::Critical
                {
                    return Ok(preferred_transport_id.clone());
                }
            }
        }

        // Find optimal transport based on multiple criteria
        let mut best_transport_id = None;
        let mut best_score = f32::MIN;

        for (transport_id, transport) in transports.iter() {
            if transport.health_status == TransportHealth::Failed {
                continue;
            }

            let score = self
                .calculate_transport_score(transport, message_priority)
                .await;

            if score > best_score {
                best_score = score;
                best_transport_id = Some(transport_id.clone());
            }
        }

        best_transport_id
            .ok_or_else(|| Error::Network("No suitable transport available".to_string()))
    }

    /// Calculate transport selection score
    async fn calculate_transport_score(
        &self,
        transport: &ManagedTransport,
        priority: TransportPriority,
    ) -> f32 {
        let mut score = 0.0;

        // Health status weight (40%)
        let health_weight = match transport.health_status {
            TransportHealth::Optimal => 1.0,
            TransportHealth::Good => 0.8,
            TransportHealth::Degraded => 0.5,
            TransportHealth::Critical => 0.2,
            TransportHealth::Failed => 0.0,
        };
        score += health_weight * 0.4;

        // Performance metrics weight (30%)
        let latency_score = 1.0 - (transport.metrics.latency.as_millis() as f32 / 1000.0).min(1.0);
        let loss_score = 1.0 - transport.metrics.packet_loss.min(1.0);
        let reliability_score = transport.metrics.reliability;

        let performance_score = (latency_score + loss_score + reliability_score) / 3.0;
        score += performance_score * 0.3;

        // Load balancing weight (20%)
        let load_factor = transport.active_connections.len() as f32
            / transport.capabilities.max_connections as f32;
        let load_score = 1.0 - load_factor.min(1.0);
        score += load_score * 0.2;

        // Priority matching weight (10%)
        let priority_score = match (transport.transport_type.clone(), priority) {
            (TransportType::TcpTls, TransportPriority::Critical) => 1.0,
            (TransportType::Tcp, TransportPriority::High) => 0.9,
            (TransportType::UdpWithNatTraversal, TransportPriority::Normal) => 0.8,
            (TransportType::Udp, TransportPriority::Normal) => 0.7,
            (TransportType::Ble, TransportPriority::Low) => 0.9,
            _ => 0.5,
        };
        score += priority_score * 0.1;

        score
    }

    /// Connect to a peer using optimal transport with failover
    pub async fn connect_with_failover(
        &self,
        peer_id: PeerId,
        preferred_address: Option<TransportAddress>,
    ) -> Result<()> {
        // Try preferred address first if provided
        if let Some(address) = preferred_address {
            if let Ok(transport_id) = self.connect_via_address(peer_id, address).await {
                return Ok(());
            }
        }

        // Use intelligent selection
        let transport_priorities = [
            TransportPriority::High,
            TransportPriority::Normal,
            TransportPriority::Low,
        ];

        for priority in &transport_priorities {
            match self.select_optimal_transport(peer_id, *priority).await {
                Ok(transport_id) => {
                    if self
                        .attempt_connection(peer_id, &transport_id)
                        .await
                        .is_ok()
                    {
                        return Ok(());
                    }
                }
                Err(_) => continue,
            }
        }

        // Last resort: try NAT traversal
        self.attempt_nat_traversal_connection(peer_id).await
    }

    /// Attempt connection via specific address
    async fn connect_via_address(
        &self,
        peer_id: PeerId,
        address: TransportAddress,
    ) -> Result<String> {
        let transport_id = self.find_transport_for_address(&address).await?;
        self.attempt_connection(peer_id, &transport_id).await?;
        Ok(transport_id)
    }

    /// Find transport that can handle specific address type
    async fn find_transport_for_address(&self, address: &TransportAddress) -> Result<String> {
        let transports = self.transports.read().await;

        for (transport_id, transport) in transports.iter() {
            let compatible = match (address, &transport.transport_type) {
                (TransportAddress::Tcp(_), TransportType::Tcp) => true,
                (TransportAddress::Tcp(_), TransportType::TcpTls) => true,
                (TransportAddress::Udp(_), TransportType::Udp) => true,
                (TransportAddress::Udp(_), TransportType::UdpWithNatTraversal) => true,
                (TransportAddress::Bluetooth(_), TransportType::Ble) => true,
                _ => false,
            };

            if compatible && transport.health_status != TransportHealth::Failed {
                return Ok(transport_id.clone());
            }
        }

        Err(Error::Network(format!(
            "No transport available for address: {:?}",
            address
        )))
    }

    /// Attempt connection using specific transport
    async fn attempt_connection(&self, peer_id: PeerId, transport_id: &str) -> Result<()> {
        let mut transports = self.transports.write().await;

        if let Some(transport) = transports.get_mut(transport_id) {
            // This is a simplified version - in reality we'd need the actual address
            let dummy_address = match transport.transport_type {
                TransportType::Tcp | TransportType::TcpTls => {
                    TransportAddress::Tcp("127.0.0.1:8080".parse().unwrap())
                }
                TransportType::Udp | TransportType::UdpWithNatTraversal => {
                    TransportAddress::Udp("127.0.0.1:8080".parse().unwrap())
                }
                TransportType::Ble => TransportAddress::Bluetooth("dummy".to_string()),
                TransportType::TurnRelay => {
                    TransportAddress::Udp("127.0.0.1:8080".parse().unwrap())
                }
            };

            match transport.transport.connect(dummy_address.clone()).await {
                Ok(connected_peer_id) => {
                    // Record connection
                    let connection_info = ConnectionInfo {
                        peer_id,
                        address: dummy_address,
                        established_at: Instant::now(),
                        last_activity: Instant::now(),
                        messages_sent: 0,
                        messages_received: 0,
                        error_count: 0,
                    };

                    transport
                        .active_connections
                        .insert(peer_id, connection_info);

                    // Update routing table
                    {
                        let mut routing_table = self.routing_table.write().await;
                        routing_table.insert(peer_id, transport_id.to_string());
                    }

                    // Update peer connections
                    {
                        let mut peer_connections = self.peer_connections.write().await;
                        peer_connections
                            .entry(peer_id)
                            .or_insert_with(Vec::new)
                            .push(transport_id.to_string());
                    }

                    println!(
                        "Connected to peer {:?} via transport: {}",
                        peer_id, transport_id
                    );
                    Ok(())
                }
                Err(e) => {
                    transport.health_status = TransportHealth::Degraded;
                    Err(Error::Network(format!(
                        "Connection failed via {}: {}",
                        transport_id, e
                    )))
                }
            }
        } else {
            Err(Error::Network(format!(
                "Transport not found: {}",
                transport_id
            )))
        }
    }

    /// Attempt NAT traversal connection
    async fn attempt_nat_traversal_connection(&self, peer_id: PeerId) -> Result<()> {
        println!("Attempting NAT traversal for peer: {:?}", peer_id);

        // Use dummy address for demonstration
        let target_address = "192.168.1.100:8080".parse().unwrap();

        match self.nat_handler.setup_nat_traversal().await {
            Ok(_) => {
                println!(
                    "NAT traversal successful: {:?} -> {}",
                    peer_id, target_address
                );

                // Create UDP transport connection through NAT traversal
                self.connect_via_address(peer_id, TransportAddress::Udp(target_address))
                    .await?;
                Ok(())
            }
            Err(e) => Err(Error::Network(format!("NAT traversal failed: {}", e))),
        }
    }

    /// Send message with intelligent routing
    pub async fn send_intelligent(
        &self,
        peer_id: PeerId,
        data: Vec<u8>,
        priority: TransportPriority,
    ) -> Result<()> {
        // Select best transport for this message
        let transport_id = self.select_optimal_transport(peer_id, priority).await?;

        // Attempt to send
        let send_success = {
            let mut transports = self.transports.write().await;
            if let Some(transport) = transports.get_mut(&transport_id) {
                transport
                    .transport
                    .send(peer_id, data.clone())
                    .await
                    .is_ok()
            } else {
                return Err(Error::Network(format!(
                    "Transport not found: {}",
                    transport_id
                )));
            }
        };

        if send_success {
            // Update metrics
            self.update_transport_success(&transport_id, peer_id).await;
            Ok(())
        } else {
            // Update metrics and attempt failover
            self.update_transport_failure(&transport_id, peer_id).await;

            if self.config.enable_adaptive_routing {
                self.attempt_failover_send(peer_id, data, priority).await
            } else {
                Err(Error::Network("Send failed".to_string()))
            }
        }
    }

    /// Attempt failover send using alternative transport
    async fn attempt_failover_send(
        &self,
        peer_id: PeerId,
        data: Vec<u8>,
        priority: TransportPriority,
    ) -> Result<()> {
        // Get alternative transports for this peer
        let peer_connections = self.peer_connections.read().await;
        let transport_ids = peer_connections.get(&peer_id).cloned().unwrap_or_default();

        for transport_id in transport_ids {
            let send_success = {
                let mut transports = self.transports.write().await;
                if let Some(transport) = transports.get_mut(&transport_id) {
                    if transport.health_status != TransportHealth::Failed {
                        transport
                            .transport
                            .send(peer_id, data.clone())
                            .await
                            .is_ok()
                    } else {
                        continue;
                    }
                } else {
                    continue;
                }
            };

            if send_success {
                // Update routing table to prefer this transport
                {
                    let mut routing_table = self.routing_table.write().await;
                    routing_table.insert(peer_id, transport_id.clone());
                }

                println!("Failover successful via transport: {}", transport_id);
                return Ok(());
            }
        }

        Err(Error::Network("All failover attempts failed".to_string()))
    }

    /// Start monitoring a transport
    async fn start_transport_monitoring(&self, transport_id: String) {
        let transports = Arc::clone(&self.transports);
        let config = self.config.clone();
        let performance_history = Arc::clone(&self.performance_history);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(config.health_check_interval);

            loop {
                interval.tick().await;

                // Perform health check
                let health_status = {
                    let transports_guard = transports.read().await;
                    if let Some(transport) = transports_guard.get(&transport_id) {
                        Self::assess_transport_health(transport).await
                    } else {
                        break; // Transport removed
                    }
                };

                // Update transport health
                {
                    let mut transports_guard = transports.write().await;
                    if let Some(transport) = transports_guard.get_mut(&transport_id) {
                        transport.health_status = health_status.clone();
                        transport.last_health_check = Instant::now();

                        // Update performance history
                        let mut history = performance_history.write().await;
                        let transport_history =
                            history.entry(transport_id.clone()).or_insert_with(Vec::new);
                        transport_history.push(transport.metrics.clone());

                        // Keep only recent history (last 100 entries)
                        if transport_history.len() > 100 {
                            transport_history.drain(0..50);
                        }
                    }
                }

                if health_status == TransportHealth::Failed {
                    println!("Transport {} marked as failed", transport_id);
                }
            }
        });
    }

    /// Assess transport health based on metrics
    async fn assess_transport_health(transport: &ManagedTransport) -> TransportHealth {
        let metrics = &transport.metrics;
        let connection_count = transport.active_connections.len();
        let max_connections = transport.capabilities.max_connections;

        // Calculate health score
        let mut score = 1.0;

        // Latency impact
        if metrics.latency > Duration::from_millis(1000) {
            score -= 0.3;
        } else if metrics.latency > Duration::from_millis(500) {
            score -= 0.1;
        }

        // Packet loss impact
        score -= metrics.packet_loss * 0.4;

        // Reliability impact
        score -= (1.0 - metrics.reliability) * 0.3;

        // Connection load impact
        let load_factor = connection_count as f32 / max_connections as f32;
        if load_factor > 0.9 {
            score -= 0.2;
        } else if load_factor > 0.7 {
            score -= 0.1;
        }

        // Convert score to health status
        match score {
            s if s >= 0.8 => TransportHealth::Optimal,
            s if s >= 0.6 => TransportHealth::Good,
            s if s >= 0.4 => TransportHealth::Degraded,
            s if s >= 0.2 => TransportHealth::Critical,
            _ => TransportHealth::Failed,
        }
    }

    /// Update transport metrics on success
    async fn update_transport_success(&self, transport_id: &str, peer_id: PeerId) {
        let mut transports = self.transports.write().await;
        if let Some(transport) = transports.get_mut(transport_id) {
            if let Some(connection) = transport.active_connections.get_mut(&peer_id) {
                connection.messages_sent += 1;
                connection.last_activity = Instant::now();
            }

            // Update reliability (simple moving average)
            transport.metrics.reliability = (transport.metrics.reliability * 0.9) + 0.1;
        }
    }

    /// Update transport metrics on failure
    async fn update_transport_failure(&self, transport_id: &str, peer_id: PeerId) {
        let mut transports = self.transports.write().await;
        if let Some(transport) = transports.get_mut(transport_id) {
            if let Some(connection) = transport.active_connections.get_mut(&peer_id) {
                connection.error_count += 1;
                connection.last_activity = Instant::now();
            }

            // Update reliability (penalize failures)
            transport.metrics.reliability = (transport.metrics.reliability * 0.9) + 0.0;

            // Update packet loss estimate
            transport.metrics.packet_loss = (transport.metrics.packet_loss * 0.9) + 0.1;
        }
    }

    /// Get comprehensive transport statistics
    pub async fn get_transport_statistics(&self) -> HashMap<String, TransportStatistics> {
        let transports = self.transports.read().await;
        let mut stats = HashMap::new();

        for (transport_id, transport) in transports.iter() {
            let transport_stats = TransportStatistics {
                transport_type: transport.transport_type.clone(),
                health_status: transport.health_status.clone(),
                active_connections: transport.active_connections.len(),
                max_connections: transport.capabilities.max_connections,
                metrics: transport.metrics.clone(),
                total_messages_sent: transport
                    .active_connections
                    .values()
                    .map(|c| c.messages_sent)
                    .sum(),
                total_messages_received: transport
                    .active_connections
                    .values()
                    .map(|c| c.messages_received)
                    .sum(),
                total_errors: transport
                    .active_connections
                    .values()
                    .map(|c| c.error_count)
                    .sum(),
            };

            stats.insert(transport_id.clone(), transport_stats);
        }

        stats
    }

    /// Broadcast message to all connected peers with transport optimization
    pub async fn broadcast_optimized(
        &self,
        data: Vec<u8>,
        priority: TransportPriority,
    ) -> Result<u32> {
        let peer_connections = self.peer_connections.read().await;
        let mut successful_sends = 0u32;

        for peer_id in peer_connections.keys() {
            if self
                .send_intelligent(*peer_id, data.clone(), priority)
                .await
                .is_ok()
            {
                successful_sends += 1;
            }
        }

        Ok(successful_sends)
    }
}

/// Transport statistics for monitoring
#[derive(Debug, Clone)]
pub struct TransportStatistics {
    pub transport_type: TransportType,
    pub health_status: TransportHealth,
    pub active_connections: usize,
    pub max_connections: usize,
    pub metrics: TransportMetrics,
    pub total_messages_sent: u64,
    pub total_messages_received: u64,
    pub total_errors: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::nat_traversal::NetworkHandler;
    use tokio::net::UdpSocket;

    #[tokio::test]
    async fn test_transport_score_calculation() {
        let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let nat_handler = NetworkHandler::new(socket, None, "127.0.0.1:0".parse().unwrap());
        let coordinator = IntelligentTransportCoordinator::new(
            IntelligentCoordinatorConfig::default(),
            nat_handler,
        );

        let capabilities = TransportCapabilities {
            supports_broadcast: true,
            supports_multicast: false,
            max_message_size: 1024,
            max_connections: 100,
            requires_pairing: false,
            encryption_available: true,
        };

        let transport = ManagedTransport {
            transport_id: "test".to_string(),
            transport_type: TransportType::TcpTls,
            transport: Box::new(crate::transport::tcp_transport::TcpTransport::new(
                crate::transport::tcp_transport::TcpTransportConfig::default(),
            )),
            metrics: TransportMetrics {
                latency: Duration::from_millis(50),
                packet_loss: 0.01,
                throughput: 1000000,
                reliability: 0.99,
                last_updated: Instant::now(),
            },
            capabilities,
            health_status: TransportHealth::Optimal,
            active_connections: HashMap::new(),
            last_health_check: Instant::now(),
            priority_score: 0.0,
        };

        let score = coordinator
            .calculate_transport_score(&transport, TransportPriority::Critical)
            .await;

        // Should be high score for optimal transport with critical priority
        assert!(score > 0.8, "Score should be high: {}", score);
    }

    #[tokio::test]
    async fn test_health_assessment() {
        let capabilities = TransportCapabilities {
            supports_broadcast: true,
            supports_multicast: false,
            max_message_size: 1024,
            max_connections: 100,
            requires_pairing: false,
            encryption_available: true,
        };

        let transport = ManagedTransport {
            transport_id: "test".to_string(),
            transport_type: TransportType::Udp,
            transport: Box::new(crate::transport::tcp_transport::TcpTransport::new(
                crate::transport::tcp_transport::TcpTransportConfig::default(),
            )),
            metrics: TransportMetrics {
                latency: Duration::from_millis(2000), // High latency
                packet_loss: 0.5,                     // High packet loss
                throughput: 1000,
                reliability: 0.3, // Low reliability
                last_updated: Instant::now(),
            },
            capabilities,
            health_status: TransportHealth::Good,
            active_connections: HashMap::new(),
            last_health_check: Instant::now(),
            priority_score: 0.0,
        };

        let health = IntelligentTransportCoordinator::assess_transport_health(&transport).await;

        // Should be degraded or failed due to poor metrics
        assert!(matches!(
            health,
            TransportHealth::Failed | TransportHealth::Critical
        ));
    }
}
