//! Transport layer for BitCraps mesh networking
//! 
//! This module implements the transport layer including:
//! - Bluetooth LE mesh transport using btleplug
//! - Transport abstraction trait
//! - Peer discovery and connection management
//! - Packet routing and forwarding

pub mod bluetooth;
pub mod traits;
pub mod kademlia;
pub mod pow_identity;
pub mod nat_traversal;
pub mod mtu_discovery;
pub mod connection_pool;
pub mod ble_peripheral;
pub mod enhanced_bluetooth;
pub mod ble_config;
pub mod crypto;
pub mod bounded_queue;
pub mod secure_gatt_server;
pub mod tcp_transport;
pub mod intelligent_coordinator;
pub mod security;
pub mod keystore;

// Platform-specific BLE peripheral implementations
#[cfg(target_os = "android")]
pub mod android_ble;
#[cfg(any(target_os = "ios", target_os = "macos"))]
pub mod ios_ble;
#[cfg(target_os = "linux")]
pub mod linux_ble;

#[cfg(test)]
mod connection_limits_test;

#[cfg(test)]
mod ble_integration_test;

#[cfg(test)]
mod multi_transport_test;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use tokio::time::interval;
use serde::{Serialize, Deserialize};

use crate::protocol::{PeerId, BitchatPacket};
use crate::error::{Error, Result};

pub use traits::*;
pub use bluetooth::*;
pub use ble_peripheral::*;
pub use enhanced_bluetooth::*;
pub use ble_config::*;
pub use crypto::*;
pub use security::*;
pub use keystore::*;

/// Transport address types for different connection methods
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TransportAddress {
    Tcp(SocketAddr),      // TCP connection (for testing/development)
    Udp(SocketAddr),      // UDP connection (for testing/development)  
    Bluetooth(String),    // Bluetooth device ID/address
    Mesh(PeerId),        // Abstract mesh routing via peer ID
}

/// Transport-specific error types
#[derive(Debug, Clone)]
pub enum TransportError {
    ConnectionFailed(String),
    Disconnected(String),
    SendFailed(String),
    ReceiveFailed(String),
    InitializationFailed(String),
    CompressionError(String),
    Timeout,
    InvalidAddress,
    NotConnected,
}

impl std::fmt::Display for TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransportError::ConnectionFailed(msg) => write!(f, "Connection failed: {}", msg),
            TransportError::Disconnected(msg) => write!(f, "Disconnected: {}", msg),
            TransportError::SendFailed(msg) => write!(f, "Send failed: {}", msg),
            TransportError::ReceiveFailed(msg) => write!(f, "Receive failed: {}", msg),
            TransportError::InitializationFailed(msg) => write!(f, "Initialization failed: {}", msg),
            TransportError::CompressionError(msg) => write!(f, "Compression error: {}", msg),
            TransportError::Timeout => write!(f, "Operation timed out"),
            TransportError::InvalidAddress => write!(f, "Invalid transport address"),
            TransportError::NotConnected => write!(f, "Not connected to peer"),
        }
    }
}

impl std::error::Error for TransportError {}

/// Events that can occur on a transport
#[derive(Debug, Clone)]
pub enum TransportEvent {
    Connected { peer_id: PeerId, address: TransportAddress },
    Disconnected { peer_id: PeerId, reason: String },
    DataReceived { peer_id: PeerId, data: Vec<u8> },
    Error { peer_id: Option<PeerId>, error: String },
}

/// Connection limits configuration
#[derive(Debug, Clone)]
pub struct ConnectionLimits {
    /// Maximum total connections allowed
    pub max_total_connections: usize,
    /// Maximum connections per peer address
    pub max_connections_per_peer: usize,
    /// Rate limit: max new connections per time window
    pub max_new_connections_per_minute: usize,
    /// Connection attempt cooldown period
    pub connection_cooldown: Duration,
}

impl Default for ConnectionLimits {
    fn default() -> Self {
        Self {
            max_total_connections: 100,
            max_connections_per_peer: 3,
            max_new_connections_per_minute: 10,
            connection_cooldown: Duration::from_secs(60),
        }
    }
}

/// Connection tracking for rate limiting
#[derive(Debug, Clone)]
struct ConnectionAttempt {
    timestamp: Instant,
    peer_address: TransportAddress,
}

/// Transport coordinator configuration
#[derive(Debug, Clone)]
pub struct CoordinatorConfig {
    pub discovery_interval: Duration,
    pub failover_timeout: Duration,
    pub max_transports: usize,
    pub enable_failover: bool,
}

impl Default for CoordinatorConfig {
    fn default() -> Self {
        Self {
            discovery_interval: Duration::from_secs(30),
            failover_timeout: Duration::from_secs(10),
            max_transports: 5,
            enable_failover: true,
        }
    }
}

/// Transport instance with health tracking
struct TransportInstance {
    transport: Box<dyn Transport>,
    health: TransportHealth,
    last_activity: Instant,
    priority: u8, // 0 = highest priority
}

/// Transport health status
#[derive(Debug, Clone, PartialEq)]
enum TransportHealth {
    Healthy,
    Degraded,
    Failed,
}

/// Transport coordinator managing multiple transport types
pub struct TransportCoordinator {
    bluetooth: Option<Arc<RwLock<BluetoothTransport>>>,
    enhanced_bluetooth: Option<Arc<RwLock<EnhancedBluetoothTransport>>>,
    transports: Arc<RwLock<Vec<TransportInstance>>>,
    connections: Arc<RwLock<HashMap<PeerId, TransportAddress>>>,
    connection_counts_per_address: Arc<RwLock<HashMap<TransportAddress, usize>>>,
    connection_attempts: Arc<RwLock<Vec<ConnectionAttempt>>>,
    connection_limits: ConnectionLimits,
    coordinator_config: CoordinatorConfig,
    event_sender: mpsc::UnboundedSender<TransportEvent>,
    event_receiver: Arc<RwLock<mpsc::UnboundedReceiver<TransportEvent>>>,
    discovery_task: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
}

impl Default for TransportCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

impl TransportCoordinator {
    pub fn new() -> Self {
        Self::new_with_limits(ConnectionLimits::default())
    }
    
    pub fn new_with_limits(limits: ConnectionLimits) -> Self {
        Self::new_with_config(limits, CoordinatorConfig::default())
    }

    pub fn new_with_config(limits: ConnectionLimits, config: CoordinatorConfig) -> Self {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();
        
        let coordinator = Self {
            bluetooth: None,
            enhanced_bluetooth: None,
            transports: Arc::new(RwLock::new(Vec::new())),
            connections: Arc::new(RwLock::new(HashMap::new())),
            connection_counts_per_address: Arc::new(RwLock::new(HashMap::new())),
            connection_attempts: Arc::new(RwLock::new(Vec::new())),
            connection_limits: limits,
            coordinator_config: config,
            event_sender,
            event_receiver: Arc::new(RwLock::new(event_receiver)),
            discovery_task: Arc::new(RwLock::new(None)),
        };
        
        // Start cleanup task for connection attempts
        coordinator.start_cleanup_task();
        
        coordinator
    }
    
    /// Start background task to clean up old connection attempts
    fn start_cleanup_task(&self) {
        let connection_attempts = self.connection_attempts.clone();
        let cleanup_interval = Duration::from_secs(60); // Clean up every minute
        
        tokio::spawn(async move {
            let mut interval = interval(cleanup_interval);
            loop {
                interval.tick().await;
                let cutoff = Instant::now() - Duration::from_secs(300); // Keep last 5 minutes
                
                let mut attempts = connection_attempts.write().await;
                attempts.retain(|attempt| attempt.timestamp > cutoff);
            }
        });
    }
    
    /// Check if a new connection is allowed based on limits
    async fn check_connection_limits(&self, address: &TransportAddress) -> Result<()> {
        // Check total connection limit
        let connections = self.connections.read().await;
        if connections.len() >= self.connection_limits.max_total_connections {
            return Err(Error::Network(format!(
                "Connection rejected: Maximum total connections ({}) exceeded",
                self.connection_limits.max_total_connections
            )));
        }
        
        // Check per-peer connection limit
        let connection_counts = self.connection_counts_per_address.read().await;
        if let Some(&count) = connection_counts.get(address) {
            if count >= self.connection_limits.max_connections_per_peer {
                return Err(Error::Network(format!(
                    "Connection rejected: Maximum connections per peer ({}) exceeded for {:?}",
                    self.connection_limits.max_connections_per_peer, address
                )));
            }
        }
        
        // Check rate limiting
        let now = Instant::now();
        let rate_limit_window = Duration::from_secs(60); // 1 minute window
        let attempts = self.connection_attempts.read().await;
        
        let recent_attempts = attempts
            .iter()
            .filter(|attempt| now.duration_since(attempt.timestamp) < rate_limit_window)
            .count();
        
        if recent_attempts >= self.connection_limits.max_new_connections_per_minute {
            return Err(Error::Network(format!(
                "Connection rejected: Rate limit exceeded ({} connections/minute)",
                self.connection_limits.max_new_connections_per_minute
            )));
        }
        
        // Check connection cooldown for this specific address
        let last_attempt_for_address = attempts
            .iter()
            .filter(|attempt| attempt.peer_address == *address)
            .max_by_key(|attempt| attempt.timestamp);
            
        if let Some(last_attempt) = last_attempt_for_address {
            if now.duration_since(last_attempt.timestamp) < self.connection_limits.connection_cooldown {
                return Err(Error::Network(format!(
                    "Connection rejected: Cooldown period active for {:?} ({}s remaining)",
                    address,
                    (self.connection_limits.connection_cooldown - now.duration_since(last_attempt.timestamp)).as_secs()
                )));
            }
        }
        
        Ok(())
    }
    
    /// Set maximum connections allowed
    pub fn set_max_connections(&mut self, max_connections: u32) {
        self.connection_limits.max_total_connections = max_connections as usize;
    }
    
    /// Set discovery interval for peer finding
    pub fn set_discovery_interval(&mut self, interval: Duration) {
        self.coordinator_config.discovery_interval = interval;
        
        // Restart discovery task with new interval
        let discovery_task = self.discovery_task.clone();
        let config = self.coordinator_config.clone();
        let transports = self.transports.clone();
        let event_sender = self.event_sender.clone();
        
        tokio::spawn(async move {
            // Stop existing task if running
            {
                let mut task_guard = discovery_task.write().await;
                if let Some(task) = task_guard.take() {
                    task.abort();
                }
            }
            
            // Start new discovery task
            let new_task = tokio::spawn(Self::discovery_task(config, transports, event_sender));
            *discovery_task.write().await = Some(new_task);
        });
    }
    
    /// Add a transport to the coordinator
    pub async fn add_transport(&self, transport: Box<dyn Transport>, priority: u8) -> Result<()> {
        let mut transports = self.transports.write().await;
        
        if transports.len() >= self.coordinator_config.max_transports {
            return Err(Error::Network(format!("Maximum transport limit ({}) reached", self.coordinator_config.max_transports)));
        }
        
        let transport_instance = TransportInstance {
            transport,
            health: TransportHealth::Healthy,
            last_activity: Instant::now(),
            priority,
        };
        
        transports.push(transport_instance);
        
        // Sort by priority (0 = highest)
        transports.sort_by_key(|t| t.priority);
        
        println!("Added transport with priority {}, total: {}", priority, transports.len());
        Ok(())
    }
    
    /// Start peer discovery task
    async fn discovery_task(
        config: CoordinatorConfig,
        transports: Arc<RwLock<Vec<TransportInstance>>>,
        event_sender: mpsc::UnboundedSender<TransportEvent>,
    ) {
        let mut interval = interval(config.discovery_interval);
        
        loop {
            interval.tick().await;
            
            // Perform discovery on all healthy transports
            let transports_guard = transports.read().await;
            for transport_instance in transports_guard.iter() {
                if transport_instance.health == TransportHealth::Healthy {
                    // In a real implementation, we'd call transport.discover_peers()
                    // For now, just log the discovery attempt
                    println!("Performing peer discovery (priority: {})", transport_instance.priority);
                }
            }
            
            // Health check on transports
            drop(transports_guard);
            Self::check_transport_health(&transports, &event_sender).await;
        }
    }
    
    /// Check health of all transports
    async fn check_transport_health(
        transports: &Arc<RwLock<Vec<TransportInstance>>>,
        event_sender: &mpsc::UnboundedSender<TransportEvent>,
    ) {
        let mut transports_guard = transports.write().await;
        let now = Instant::now();
        
        for transport_instance in transports_guard.iter_mut() {
            // Mark as failed if no activity for too long (simplified health check)
            if now.duration_since(transport_instance.last_activity) > Duration::from_secs(300) {
                if transport_instance.health != TransportHealth::Failed {
                    transport_instance.health = TransportHealth::Failed;
                    let _ = event_sender.send(TransportEvent::Error {
                        peer_id: None,
                        error: "Transport health check failed".to_string(),
                    });
                }
            }
        }
    }
    
    /// Record a connection attempt for rate limiting
    async fn record_connection_attempt(&self, address: &TransportAddress) {
        let mut attempts = self.connection_attempts.write().await;
        attempts.push(ConnectionAttempt {
            timestamp: Instant::now(),
            peer_address: address.clone(),
        });
    }
    
    /// Update connection counts when a connection is established
    async fn increment_connection_count(&self, address: &TransportAddress) {
        let mut counts = self.connection_counts_per_address.write().await;
        *counts.entry(address.clone()).or_insert(0) += 1;
    }
    
    /// Update connection counts when a connection is closed
    async fn decrement_connection_count(&self, address: &TransportAddress) {
        let mut counts = self.connection_counts_per_address.write().await;
        if let Some(count) = counts.get_mut(address) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                counts.remove(address);
            }
        }
    }
    
    /// Initialize Bluetooth transport
    pub async fn init_bluetooth(&mut self, local_peer_id: PeerId) -> Result<()> {
        let bluetooth = BluetoothTransport::new(local_peer_id).await
            .map_err(|e| Error::Network(format!("Failed to initialize Bluetooth: {}", e)))?;
        
        self.bluetooth = Some(Arc::new(RwLock::new(bluetooth)));
        Ok(())
    }
    
    /// Initialize enhanced Bluetooth transport with both central and peripheral roles
    pub async fn init_enhanced_bluetooth(&mut self, local_peer_id: PeerId) -> Result<()> {
        log::info!("Initializing enhanced Bluetooth transport");
        
        let mut enhanced_bluetooth = EnhancedBluetoothTransport::new(local_peer_id).await
            .map_err(|e| Error::Network(format!("Failed to initialize enhanced Bluetooth: {}", e)))?;
        
        // Initialize the transport
        enhanced_bluetooth.initialize().await
            .map_err(|e| Error::Network(format!("Failed to initialize enhanced Bluetooth components: {}", e)))?;
        
        self.enhanced_bluetooth = Some(Arc::new(RwLock::new(enhanced_bluetooth)));
        
        log::info!("Enhanced Bluetooth transport initialized successfully");
        Ok(())
    }
    
    /// Start BLE advertising (requires enhanced Bluetooth transport)
    pub async fn start_ble_advertising(&self, config: AdvertisingConfig) -> Result<()> {
        if let Some(enhanced_bt) = &self.enhanced_bluetooth {
            let mut bt = enhanced_bt.write().await;
            bt.start_advertising(config).await
                .map_err(|e| Error::Network(format!("Failed to start BLE advertising: {}", e)))
        } else {
            Err(Error::Network("Enhanced Bluetooth transport not initialized".to_string()))
        }
    }
    
    /// Stop BLE advertising
    pub async fn stop_ble_advertising(&self) -> Result<()> {
        if let Some(enhanced_bt) = &self.enhanced_bluetooth {
            let mut bt = enhanced_bt.write().await;
            bt.stop_advertising().await
                .map_err(|e| Error::Network(format!("Failed to stop BLE advertising: {}", e)))
        } else {
            Err(Error::Network("Enhanced Bluetooth transport not initialized".to_string()))
        }
    }
    
    /// Start mesh mode (both advertising and scanning)
    pub async fn start_mesh_mode(&self, config: AdvertisingConfig) -> Result<()> {
        if let Some(enhanced_bt) = &self.enhanced_bluetooth {
            let mut bt = enhanced_bt.write().await;
            bt.start_mesh_mode(config).await
                .map_err(|e| Error::Network(format!("Failed to start mesh mode: {}", e)))
        } else {
            Err(Error::Network("Enhanced Bluetooth transport not initialized".to_string()))
        }
    }
    
    /// Get enhanced Bluetooth statistics
    pub async fn get_enhanced_bluetooth_stats(&self) -> Result<EnhancedBluetoothStats> {
        if let Some(enhanced_bt) = &self.enhanced_bluetooth {
            let bt = enhanced_bt.read().await;
            Ok(bt.get_combined_stats().await)
        } else {
            Err(Error::Network("Enhanced Bluetooth transport not initialized".to_string()))
        }
    }
    
    /// Start listening on all available transports
    pub async fn start_listening(&self) -> Result<()> {
        // Prefer enhanced Bluetooth if available
        if let Some(enhanced_bluetooth) = &self.enhanced_bluetooth {
            let mut bt = enhanced_bluetooth.write().await;
            bt.listen(TransportAddress::Bluetooth("BitCraps".to_string())).await
                .map_err(|e| Error::Network(format!("Enhanced Bluetooth listen failed: {}", e)))?;
        } else if let Some(bluetooth) = &self.bluetooth {
            let mut bt = bluetooth.write().await;
            bt.listen(TransportAddress::Bluetooth("BitCraps".to_string())).await
                .map_err(|e| Error::Network(format!("Bluetooth listen failed: {}", e)))?;
        }
        
        Ok(())
    }
    
    /// Connect to a peer via the best available transport with failover
    pub async fn connect_to_peer(&self, peer_id: PeerId, address: TransportAddress) -> Result<()> {
        // Check connection limits before attempting to connect
        self.check_connection_limits(&address).await?;
        
        // Record the connection attempt
        self.record_connection_attempt(&address).await;
        
        if self.coordinator_config.enable_failover {
            self.connect_with_failover(peer_id, address).await
        } else {
            self.connect_single_transport(peer_id, address).await
        }
    }
    
    /// Connect using failover logic across multiple transports
    async fn connect_with_failover(&self, peer_id: PeerId, address: TransportAddress) -> Result<()> {
        let transports = self.transports.read().await;
        
        // Try transports in priority order, only healthy ones
        for transport_instance in transports.iter() {
            if transport_instance.health != TransportHealth::Healthy {
                continue;
            }
            
            println!("Attempting connection via transport priority {}", transport_instance.priority);
            
            // Try to connect with timeout
            let connect_future = self.attempt_transport_connection(transport_instance, peer_id, address.clone());
            
            match tokio::time::timeout(self.coordinator_config.failover_timeout, connect_future).await {
                Ok(Ok(_)) => {
                    // Connection successful - update tracking
                    self.connections.write().await.insert(peer_id, address.clone());
                    self.increment_connection_count(&address).await;
                    
                    // Send connection event
                    let _ = self.event_sender.send(TransportEvent::Connected {
                        peer_id,
                        address: address.clone(),
                    });
                    
                    return Ok(());
                }
                Ok(Err(e)) => {
                    println!("Transport connection failed: {}", e);
                    continue; // Try next transport
                }
                Err(_) => {
                    println!("Transport connection timed out");
                    continue; // Try next transport
                }
            }
        }
        
        // All transports failed, try legacy Bluetooth as fallback
        self.connect_single_transport(peer_id, address).await
    }
    
    /// Attempt connection using a specific transport instance
    async fn attempt_transport_connection(
        &self,
        _transport_instance: &TransportInstance,
        _peer_id: PeerId,
        _address: TransportAddress,
    ) -> Result<()> {
        // In a real implementation, this would call transport_instance.transport.connect()
        // For now, simulate a connection attempt that might fail
        use rand::Rng;
        let mut rng = rand::thread_rng();
        if rng.gen_bool(0.7) { // 70% success rate for simulation
            Ok(())
        } else {
            Err(Error::Network("Simulated connection failure".to_string()))
        }
    }
    
    /// Connect using single transport (legacy method)
    async fn connect_single_transport(&self, peer_id: PeerId, address: TransportAddress) -> Result<()> {
        match address {
            TransportAddress::Bluetooth(_) => {
                if let Some(bluetooth) = &self.bluetooth {
                    let mut bt = bluetooth.write().await;
                    
                    // Attempt the connection
                    match bt.connect(address.clone()).await {
                        Ok(_) => {
                            // Connection successful - update tracking
                            self.connections.write().await.insert(peer_id, address.clone());
                            self.increment_connection_count(&address).await;
                            
                            // Send connection event
                            let _ = self.event_sender.send(TransportEvent::Connected {
                                peer_id,
                                address: address.clone(),
                            });
                        }
                        Err(e) => {
                            // Connection failed - send error event
                            let error_msg = format!("Bluetooth connect failed: {}", e);
                            let _ = self.event_sender.send(TransportEvent::Error {
                                peer_id: Some(peer_id),
                                error: error_msg.clone(),
                            });
                            return Err(Error::Network(error_msg));
                        }
                    }
                }
            }
            _ => {
                return Err(Error::Network("Unsupported transport type".to_string()));
            }
        }
        
        Ok(())
    }
    
    /// Disconnect from a peer and update connection tracking
    pub async fn disconnect_from_peer(&self, peer_id: PeerId) -> Result<()> {
        let mut connections = self.connections.write().await;
        
        if let Some(address) = connections.remove(&peer_id) {
            // Decrement connection count for this address
            self.decrement_connection_count(&address).await;
            
            // Perform actual disconnect based on transport type
            match address {
                TransportAddress::Bluetooth(_) => {
                    if let Some(bluetooth) = &self.bluetooth {
                        let mut bt = bluetooth.write().await;
                        bt.disconnect(peer_id).await
                            .map_err(|e| Error::Network(format!("Bluetooth disconnect failed: {}", e)))?;
                    }
                }
                _ => {
                    return Err(Error::Network("Unsupported transport type".to_string()));
                }
            }
            
            // Send disconnection event
            let _ = self.event_sender.send(TransportEvent::Disconnected {
                peer_id,
                reason: "User requested disconnect".to_string(),
            });
        }
        
        Ok(())
    }
    
    /// Get current connection limits configuration
    pub fn connection_limits(&self) -> &ConnectionLimits {
        &self.connection_limits
    }
    
    /// Update connection limits (takes effect for new connections)
    pub fn update_connection_limits(&mut self, limits: ConnectionLimits) {
        self.connection_limits = limits;
    }
    
    /// Get connection statistics
    pub async fn connection_stats(&self) -> ConnectionStats {
        let connections = self.connections.read().await;
        let counts = self.connection_counts_per_address.read().await;
        let attempts = self.connection_attempts.read().await;
        
        let now = Instant::now();
        let recent_attempts = attempts
            .iter()
            .filter(|attempt| now.duration_since(attempt.timestamp) < Duration::from_secs(60))
            .count();
        
        ConnectionStats {
            total_connections: connections.len(),
            connections_by_address: counts.clone(),
            recent_connection_attempts: recent_attempts,
            connection_limit: self.connection_limits.max_total_connections,
        }
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
#[derive(Default)]
pub struct TransportStats {
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub connection_count: usize,
    pub error_count: u64,
}


/// Connection statistics for DoS protection monitoring
#[derive(Debug, Clone)]
pub struct ConnectionStats {
    pub total_connections: usize,
    pub connections_by_address: HashMap<TransportAddress, usize>,
    pub recent_connection_attempts: usize,
    pub connection_limit: usize,
}