//! Transport layer for BitCraps mesh networking
//!
//! This module implements the transport layer including:
//! - Bluetooth LE mesh transport using btleplug
//! - Transport abstraction trait
//! - Peer discovery and connection management
//! - Packet routing and forwarding

pub mod ble_config;
pub mod ble_peripheral;
pub mod bluetooth;
pub mod bounded_queue;
pub mod connection_pool;
pub mod crypto;
pub mod enhanced_bluetooth;
pub mod intelligent_coordinator;
pub mod kademlia;
pub mod keystore;
pub mod mtu_discovery;
// NAT traversal support gated by feature flag
#[cfg(feature = "nat-traversal")]
pub mod nat_traversal;
pub mod pow_identity;
// Secure GATT server (requires BLE support)
#[cfg(feature = "bluetooth")]
pub mod secure_gatt_server;
pub mod security;
// TCP transport with optional TLS support
pub mod tcp_transport;
pub mod traits;

// Platform-specific BLE peripheral implementations
// Android BLE support requires both target platform and android feature
#[cfg(all(target_os = "android", feature = "android"))]
pub mod android_ble;
// iOS/macOS BLE support with platform detection
#[cfg(any(target_os = "ios", target_os = "macos"))]
pub mod ios_ble;
// Linux BLE support via BlueZ
#[cfg(target_os = "linux")]
pub mod linux_ble;

// Test modules organized by speed
#[cfg(test)]
mod connection_limits_test; // Fast unit test

#[cfg(all(test, feature = "physical_device_tests"))]
mod ble_integration_test; // Slow integration test

#[cfg(test)]
mod multi_transport_test; // Medium speed test

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use tokio::time::interval;

use crate::error::{Error, Result};
use crate::memory_pool::GameMemoryPools;
use crate::protocol::{BitchatPacket, PeerId};
use bounded_queue::{BoundedTransportEventQueue, OverflowBehavior, QueueConfig};

// Specific re-exports to avoid ambiguous glob conflicts
pub use ble_config::{BleConfigBuilder, BleTransportConfig};
pub use ble_peripheral::AdvertisingConfig;
pub use ble_peripheral::{BlePeripheral, PeripheralEvent};
pub use bluetooth::{BluetoothStats, BluetoothTransport};
pub use crypto::TransportCrypto; // CryptoEvent not implemented yet
pub use enhanced_bluetooth::{EnhancedBluetoothStats, EnhancedBluetoothTransport};
pub use keystore::{KeystoreConfig, SecureTransportKeystore};
pub use security::{BleSecurityConfig, EnhancedTransportSecurity};
pub use traits::Transport;

// NAT traversal re-exports (conditional)
#[cfg(feature = "nat-traversal")]
pub use nat_traversal::{NatType, NetworkHandler, TransportMode};

// Conditional re-exports based on features
#[cfg(feature = "bluetooth")]
pub use secure_gatt_server::{GattService, SecureGattServer};

/// Transport address types for different connection methods
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TransportAddress {
    Tcp(SocketAddr),   // TCP connection (for testing/development)
    Udp(SocketAddr),   // UDP connection (for testing/development)
    Bluetooth(String), // Bluetooth device ID/address
    Mesh(PeerId),      // Abstract mesh routing via peer ID
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
            TransportError::InitializationFailed(msg) => {
                write!(f, "Initialization failed: {}", msg)
            }
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
    Connected {
        peer_id: PeerId,
        address: TransportAddress,
    },
    Disconnected {
        peer_id: PeerId,
        reason: String,
    },
    DataReceived {
        peer_id: PeerId,
        data: Vec<u8>,
    },
    Error {
        peer_id: Option<PeerId>,
        error: String,
    },
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

/// Connection metadata for tracking and LRU eviction
#[derive(Debug, Clone)]
struct ConnectionMetadata {
    address: TransportAddress,
    established_at: Instant,
}

/// Pending message for backpressure queue
#[derive(Debug, Clone)]
struct PendingMessage {
    peer_id: PeerId,
    data: Vec<u8>,
    timestamp: Instant,
    retry_count: u8,
    priority: MessagePriority,
}

/// Message priority for backpressure handling
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum MessagePriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

/// Transport coordinator managing multiple transport types with backpressure
pub struct TransportCoordinator {
    bluetooth: Option<Arc<RwLock<BluetoothTransport>>>,
    enhanced_bluetooth: Option<Arc<RwLock<EnhancedBluetoothTransport>>>,
    tcp_transport: Option<Arc<RwLock<tcp_transport::TcpTransport>>>,
    transports: Arc<RwLock<Vec<TransportInstance>>>,
    connections: Arc<DashMap<PeerId, ConnectionMetadata>>,
    connection_counts_per_address: Arc<DashMap<TransportAddress, usize>>,
    connection_attempts: Arc<RwLock<Vec<ConnectionAttempt>>>,
    connection_limits: ConnectionLimits,
    coordinator_config: CoordinatorConfig,
    event_sender: mpsc::Sender<TransportEvent>,
    event_receiver: Arc<RwLock<mpsc::Receiver<TransportEvent>>>,
    discovery_task: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
    active_transports: Arc<DashMap<String, TransportType>>,
    failover_enabled: bool,
    /// Bounded event queue with backpressure
    bounded_event_queue: BoundedTransportEventQueue,
    /// Message send queue with backpressure
    send_queue: bounded_queue::BoundedEventQueue<PendingMessage>,
    /// Memory pools for packet buffer optimization
    memory_pools: Arc<GameMemoryPools>,
}

#[derive(Debug, Clone)]
enum TransportType {
    Bluetooth,
    EnhancedBluetooth,
    Tcp,
    Udp,
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
        let (event_sender, event_receiver) = mpsc::channel(1000); // Legacy unbounded channel

        // Create bounded event queue with backpressure for high-load scenarios
        let event_queue_config = QueueConfig {
            max_size: 10_000,
            overflow_behavior: match config.enable_failover {
                true => OverflowBehavior::DropOldest, // Prioritize new events
                false => OverflowBehavior::Backpressure, // Apply backpressure
            },
            backpressure_timeout: Duration::from_millis(100),
            enable_metrics: true,
        };
        let bounded_event_queue = BoundedTransportEventQueue::with_config(event_queue_config);

        // Create send queue for message backpressure
        let send_queue_config = QueueConfig {
            max_size: 5_000,
            overflow_behavior: OverflowBehavior::Backpressure,
            backpressure_timeout: Duration::from_millis(50),
            enable_metrics: true,
        };
        let send_queue = bounded_queue::BoundedEventQueue::with_config(send_queue_config);

        let memory_pools = Arc::new(GameMemoryPools::new());

        let coordinator = Self {
            bluetooth: None,
            enhanced_bluetooth: None,
            tcp_transport: None,
            transports: Arc::new(RwLock::new(Vec::new())),
            connections: Arc::new(DashMap::new()),
            connection_counts_per_address: Arc::new(DashMap::new()),
            connection_attempts: Arc::new(RwLock::new(Vec::new())),
            connection_limits: limits,
            coordinator_config: config.clone(),
            event_sender,
            event_receiver: Arc::new(RwLock::new(event_receiver)),
            discovery_task: Arc::new(RwLock::new(None)),
            active_transports: Arc::new(DashMap::new()),
            failover_enabled: config.enable_failover,
            bounded_event_queue,
            send_queue,
            memory_pools,
        };

        // Start cleanup task for connection attempts
        coordinator.start_cleanup_task();

        coordinator
    }

    /// Get a pooled Vec<u8> buffer for temporary use
    pub async fn get_buffer(&self) -> crate::memory_pool::PooledObject<Vec<u8>> {
        self.memory_pools.vec_u8_pool.get().await
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

    /// Enforce connection capacity limits by evicting oldest connections if needed
    async fn enforce_connection_capacity(&self) {
        let max_connections = self.connection_limits.max_total_connections;

        // Check if we need to evict connections
        while self.connections.len() >= max_connections {
            // Find the oldest connection
            let oldest = self
                .connections
                .iter()
                .min_by_key(|entry| entry.value().established_at)
                .map(|entry| (*entry.key(), entry.value().clone()));

            if let Some((peer_id, metadata)) = oldest {
                // Remove the oldest connection
                self.connections.remove(&peer_id);

                // Decrement the connection count for this address
                if let Some(mut count) = self
                    .connection_counts_per_address
                    .get_mut(&metadata.address)
                {
                    *count = count.saturating_sub(1);
                    if *count == 0 {
                        drop(count);
                        self.connection_counts_per_address.remove(&metadata.address);
                    }
                }

                log::warn!(
                    "Evicted oldest connection to peer {:?} (established at {:?}) to enforce capacity limit of {}",
                    peer_id,
                    metadata.established_at,
                    max_connections
                );

                // Send disconnection event
                let _ = self
                    .event_sender
                    .send(TransportEvent::Disconnected {
                        peer_id,
                        reason: "Connection evicted due to capacity limit".to_string(),
                    })
                    .await;
            } else {
                // No connections to evict, break the loop
                break;
            }
        }
    }

    /// Check if a new connection is allowed based on limits
    async fn check_connection_limits(&self, address: &TransportAddress) -> Result<()> {
        // Note: We no longer reject connections due to total limit.
        // Instead, we'll evict old connections to make room in enforce_connection_capacity().
        // This check is now for rate limiting and per-peer limits only.

        // Check per-peer connection limit
        if let Some(count_ref) = self.connection_counts_per_address.get(address) {
            let count = *count_ref.value();
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
            if now.duration_since(last_attempt.timestamp)
                < self.connection_limits.connection_cooldown
            {
                return Err(Error::Network(format!(
                    "Connection rejected: Cooldown period active for {:?} ({}s remaining)",
                    address,
                    (self.connection_limits.connection_cooldown
                        - now.duration_since(last_attempt.timestamp))
                    .as_secs()
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
            return Err(Error::Network(format!(
                "Maximum transport limit ({}) reached",
                self.coordinator_config.max_transports
            )));
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

        println!(
            "Added transport with priority {}, total: {}",
            priority,
            transports.len()
        );
        Ok(())
    }

    /// Start peer discovery task
    async fn discovery_task(
        config: CoordinatorConfig,
        transports: Arc<RwLock<Vec<TransportInstance>>>,
        event_sender: mpsc::Sender<TransportEvent>,
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
                    println!(
                        "Performing peer discovery (priority: {})",
                        transport_instance.priority
                    );
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
        event_sender: &mpsc::Sender<TransportEvent>,
    ) {
        let mut transports_guard = transports.write().await;
        let now = Instant::now();

        for transport_instance in transports_guard.iter_mut() {
            // Mark as failed if no activity for too long (simplified health check)
            if now.duration_since(transport_instance.last_activity) > Duration::from_secs(300)
                && transport_instance.health != TransportHealth::Failed
            {
                transport_instance.health = TransportHealth::Failed;
                let _ = event_sender
                    .send(TransportEvent::Error {
                        peer_id: None,
                        error: "Transport health check failed".to_string(),
                    })
                    .await;
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
        // DashMap provides thread-safe operations
        let mut entry = self
            .connection_counts_per_address
            .entry(address.clone())
            .or_insert(0);
        *entry += 1;
    }

    /// Update connection counts when a connection is closed
    async fn decrement_connection_count(&self, address: &TransportAddress) {
        // DashMap provides thread-safe operations
        if let Some(mut count) = self.connection_counts_per_address.get_mut(address) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                self.connection_counts_per_address.remove(address);
            }
        }
    }

    /// Initialize Bluetooth transport
    pub async fn init_bluetooth(&mut self, local_peer_id: PeerId) -> Result<()> {
        let bluetooth = BluetoothTransport::new(local_peer_id)
            .await
            .map_err(|e| Error::Network(format!("Failed to initialize Bluetooth: {}", e)))?;

        self.bluetooth = Some(Arc::new(RwLock::new(bluetooth)));
        Ok(())
    }

    /// Initialize enhanced Bluetooth transport with both central and peripheral roles
    pub async fn init_enhanced_bluetooth(&mut self, local_peer_id: PeerId) -> Result<()> {
        log::info!("Initializing enhanced Bluetooth transport");

        let mut enhanced_bluetooth = EnhancedBluetoothTransport::new(local_peer_id)
            .await
            .map_err(|e| {
                Error::Network(format!("Failed to initialize enhanced Bluetooth: {}", e))
            })?;

        // Initialize the transport
        enhanced_bluetooth.initialize().await.map_err(|e| {
            Error::Network(format!(
                "Failed to initialize enhanced Bluetooth components: {}",
                e
            ))
        })?;

        self.enhanced_bluetooth = Some(Arc::new(RwLock::new(enhanced_bluetooth)));

        // Register as active transport
        self.active_transports.insert(
            "enhanced_bluetooth".to_string(),
            TransportType::EnhancedBluetooth,
        );

        log::info!("Enhanced Bluetooth transport initialized successfully");
        Ok(())
    }

    /// Initialize TCP transport for WiFi/Internet connectivity
    pub async fn init_tcp_transport(
        &mut self,
        config: tcp_transport::TcpTransportConfig,
        local_peer_id: PeerId,
    ) -> Result<()> {
        log::info!("Initializing TCP transport");

        let tcp_transport = tcp_transport::TcpTransport::new_with_sender(
            config,
            local_peer_id,
            self.event_sender.clone(),
        );
        self.tcp_transport = Some(Arc::new(RwLock::new(tcp_transport)));

        // Register as active transport
        self.active_transports
            .insert("tcp".to_string(), TransportType::Tcp);

        log::info!("TCP transport initialized successfully");
        Ok(())
    }

    /// Enable TCP transport with default config on specified port
    pub async fn enable_tcp(&mut self, _port: u16, local_peer_id: PeerId) -> Result<()> {
        let config = tcp_transport::TcpTransportConfig {
            max_connections: 100,
            connection_timeout: Duration::from_secs(10),
            keepalive_interval: Duration::from_secs(30),
            max_message_size: 1024 * 1024, // 1MB
            enable_tls: true,              // Enable encryption by default
            connection_pool_size: 20,
        };

        self.init_tcp_transport(config, local_peer_id).await
    }

    /// Listen on a TCP address using the TCP transport
    pub async fn listen_tcp(&self, addr: std::net::SocketAddr) -> Result<()> {
        if let Some(tcp) = &self.tcp_transport {
            let mut tcp = tcp.write().await;
            tcp.listen(TransportAddress::Tcp(addr))
                .await
                .map_err(|e| Error::Network(format!("TCP listen failed: {}", e)))?;
            Ok(())
        } else {
            Err(Error::Network("TCP transport not initialized".to_string()))
        }
    }

    /// Connect to a TCP peer
    pub async fn connect_tcp(&self, addr: std::net::SocketAddr) -> Result<PeerId> {
        if let Some(tcp) = &self.tcp_transport {
            let mut tcp = tcp.write().await;
            tcp.connect(TransportAddress::Tcp(addr))
                .await
                .map_err(|e| Error::Network(format!("TCP connect failed: {}", e)))
        } else {
            Err(Error::Network("TCP transport not initialized".to_string()))
        }
    }

    /// Enable concurrent operation of multiple transports
    pub async fn enable_multi_transport_mode(&mut self) -> Result<()> {
        log::info!("Enabling multi-transport mode");

        let transport_count = self.active_transports.len();

        if transport_count < 2 {
            return Err(Error::Network(
                "Need at least 2 transports for multi-transport mode".to_string(),
            ));
        }

        log::info!(
            "Multi-transport mode enabled with {} transports",
            transport_count
        );

        // Start transport monitoring task
        self.start_transport_monitoring().await;

        Ok(())
    }

    /// Start background task to monitor transport health and performance
    async fn start_transport_monitoring(&self) {
        let active_transports = self.active_transports.clone();
        let bluetooth = self.bluetooth.clone();
        let enhanced_bluetooth = self.enhanced_bluetooth.clone();
        let tcp_transport = self.tcp_transport.clone();
        let event_sender = self.event_sender.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));

            loop {
                interval.tick().await;

                log::debug!("Monitoring {} active transports", active_transports.len());

                // Check health of each transport
                for entry in active_transports.iter() {
                    let (name, transport_type) = (entry.key(), entry.value());
                    let is_healthy = match transport_type {
                        TransportType::Bluetooth => {
                            if let Some(bt) = &bluetooth {
                                // Check Bluetooth health
                                let stats = bt.read().await.bluetooth_stats().await;
                                stats.active_connections > 0 || stats.recent_connection_attempts > 0
                            } else {
                                false
                            }
                        }
                        TransportType::EnhancedBluetooth => {
                            if let Some(ebt) = &enhanced_bluetooth {
                                // Check enhanced Bluetooth health
                                let stats = ebt.read().await.get_combined_stats().await;
                                stats.total_connections > 0
                            } else {
                                false
                            }
                        }
                        TransportType::Tcp => {
                            if let Some(tcp) = &tcp_transport {
                                // Check TCP health
                                let stats = tcp.read().await.connection_stats().await;
                                stats.healthy_connections > 0 || stats.total_connections > 0
                            } else {
                                false
                            }
                        }
                        TransportType::Udp => {
                            // UDP transport not implemented yet
                            false
                        }
                    };

                    if !is_healthy {
                        log::debug!("Transport {} is unhealthy", name);
                        let _ = event_sender
                            .send(TransportEvent::Error {
                                peer_id: None,
                                error: format!("Transport {} health check failed", name),
                            })
                            .await;
                    } else {
                        log::debug!("Transport {} is healthy", name);
                    }
                }
            }
        });
    }

    /// Start BLE advertising (requires enhanced Bluetooth transport)
    pub async fn start_ble_advertising(&self, config: AdvertisingConfig) -> Result<()> {
        if let Some(enhanced_bt) = &self.enhanced_bluetooth {
            let mut bt = enhanced_bt.write().await;
            bt.start_advertising(config)
                .await
                .map_err(|e| Error::Network(format!("Failed to start BLE advertising: {}", e)))
        } else {
            Err(Error::Network(
                "Enhanced Bluetooth transport not initialized".to_string(),
            ))
        }
    }

    /// Stop BLE advertising
    pub async fn stop_ble_advertising(&self) -> Result<()> {
        if let Some(enhanced_bt) = &self.enhanced_bluetooth {
            let mut bt = enhanced_bt.write().await;
            bt.stop_advertising()
                .await
                .map_err(|e| Error::Network(format!("Failed to stop BLE advertising: {}", e)))
        } else {
            Err(Error::Network(
                "Enhanced Bluetooth transport not initialized".to_string(),
            ))
        }
    }

    /// Start mesh mode (both advertising and scanning)
    pub async fn start_mesh_mode(&self, config: AdvertisingConfig) -> Result<()> {
        if let Some(enhanced_bt) = &self.enhanced_bluetooth {
            let mut bt = enhanced_bt.write().await;
            bt.start_mesh_mode(config)
                .await
                .map_err(|e| Error::Network(format!("Failed to start mesh mode: {}", e)))
        } else {
            Err(Error::Network(
                "Enhanced Bluetooth transport not initialized".to_string(),
            ))
        }
    }

    /// Get enhanced Bluetooth statistics
    pub async fn get_enhanced_bluetooth_stats(&self) -> Result<EnhancedBluetoothStats> {
        if let Some(enhanced_bt) = &self.enhanced_bluetooth {
            let bt = enhanced_bt.read().await;
            Ok(bt.get_combined_stats().await)
        } else {
            Err(Error::Network(
                "Enhanced Bluetooth transport not initialized".to_string(),
            ))
        }
    }

    /// Start listening on all available transports
    pub async fn start_listening(&self) -> Result<()> {
        // Prefer enhanced Bluetooth if available
        if let Some(enhanced_bluetooth) = &self.enhanced_bluetooth {
            let mut bt = enhanced_bluetooth.write().await;
            bt.listen(TransportAddress::Bluetooth("BitCraps".to_string()))
                .await
                .map_err(|e| Error::Network(format!("Enhanced Bluetooth listen failed: {}", e)))?;
        } else if let Some(bluetooth) = &self.bluetooth {
            let mut bt = bluetooth.write().await;
            bt.listen(TransportAddress::Bluetooth("BitCraps".to_string()))
                .await
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

    /// Connect using intelligent failover logic across multiple transports
    async fn connect_with_failover(
        &self,
        peer_id: PeerId,
        address: TransportAddress,
    ) -> Result<()> {
        log::info!(
            "Attempting connection with failover to peer {:?} at {:?}",
            peer_id,
            address
        );

        // Try transports in order of preference based on address type and availability
        let transport_attempts = self.get_transport_priority_for_address(&address).await;

        for (transport_name, attempt_func) in transport_attempts {
            log::debug!("Attempting connection via {} transport", transport_name);

            let start_time = std::time::Instant::now();

            match tokio::time::timeout(self.coordinator_config.failover_timeout, attempt_func).await
            {
                Ok(Ok(_)) => {
                    let elapsed = start_time.elapsed();
                    log::info!(
                        "Successfully connected via {} in {:?}",
                        transport_name,
                        elapsed
                    );

                    // Enforce capacity limits before adding new connection
                    self.enforce_connection_capacity().await;

                    // Update connection tracking with metadata
                    let metadata = ConnectionMetadata {
                        address: address.clone(),
                        established_at: Instant::now(),
                    };
                    self.connections.insert(peer_id, metadata);
                    self.increment_connection_count(&address).await;

                    // Send connection event
                    let _ = self.event_sender.send(TransportEvent::Connected {
                        peer_id,
                        address: address.clone(),
                    });
                    
                    // Record network metric
                    crate::monitoring::record_network_event("peer_connected", Some(&format!("{:?}", peer_id)));

                    return Ok(());
                }
                Ok(Err(e)) => {
                    log::debug!("Connection via {} failed: {}", transport_name, e);
                    continue; // Try next transport
                }
                Err(_) => {
                    log::debug!("Connection via {} timed out", transport_name);
                    continue; // Try next transport
                }
            }
        }

        // All modern transports failed - log comprehensive failure
        log::error!(
            "All transport failover attempts failed for peer {:?}",
            peer_id
        );
        Err(Error::Network(format!(
            "Failed to connect to peer {:?}: all transports exhausted",
            peer_id
        )))
    }

    /// Get prioritized list of transport connection functions for a given address
    async fn get_transport_priority_for_address(
        &self,
        address: &TransportAddress,
    ) -> Vec<(
        String,
        std::pin::Pin<std::boxed::Box<dyn std::future::Future<Output = Result<()>> + Send>>,
    )> {
        // Use pooled Vec<u8> and then into_inner for reuse, though we need Vec<(String, Future)>
        // For this specific case, we'll use a regular Vec since the type is complex and usage is infrequent
        let mut attempts = Vec::new();

        match address {
            TransportAddress::Bluetooth(_) => {
                // For Bluetooth addresses, prefer enhanced BT over legacy
                if let Some(enhanced_bt) = &self.enhanced_bluetooth {
                    let enhanced_bt = enhanced_bt.clone();
                    let address = address.clone();
                    attempts.push((
                        "enhanced_bluetooth".to_string(),
                        Box::pin(async move {
                            let mut transport = enhanced_bt.write().await;
                            match transport.connect(address).await {
                                Ok(_) => Ok(()),
                                Err(e) => Err(Error::Network(e.to_string())),
                            }
                        })
                            as std::pin::Pin<
                                std::boxed::Box<
                                    dyn std::future::Future<Output = Result<()>> + Send,
                                >,
                            >,
                    ));
                }

                if let Some(bluetooth) = &self.bluetooth {
                    let bluetooth = bluetooth.clone();
                    let address = address.clone();
                    attempts.push((
                        "bluetooth".to_string(),
                        Box::pin(async move {
                            let mut transport = bluetooth.write().await;
                            match transport.connect(address).await {
                                Ok(_) => Ok(()),
                                Err(e) => Err(Error::Network(e.to_string())),
                            }
                        }),
                    ));
                }
            }
            TransportAddress::Tcp(_) => {
                // For TCP addresses, use TCP transport
                if let Some(tcp) = &self.tcp_transport {
                    let tcp = tcp.clone();
                    let address = address.clone();
                    attempts.push((
                        "tcp".to_string(),
                        Box::pin(async move {
                            let mut transport = tcp.write().await;
                            match transport.connect(address).await {
                                Ok(_) => Ok(()),
                                Err(e) => Err(Error::Network(e.to_string())),
                            }
                        }),
                    ));
                }
            }
            TransportAddress::Udp(_) => {
                // UDP transport not yet implemented
                log::debug!("UDP transport requested but not implemented");
            }
            TransportAddress::Mesh(_) => {
                // For mesh addresses, try all available transports
                if let Some(enhanced_bt) = &self.enhanced_bluetooth {
                    let enhanced_bt = enhanced_bt.clone();
                    let mesh_address = TransportAddress::Bluetooth("mesh".to_string());
                    attempts.push((
                        "enhanced_bluetooth_mesh".to_string(),
                        Box::pin(async move {
                            let mut transport = enhanced_bt.write().await;
                            match transport.connect(mesh_address).await {
                                Ok(_) => Ok(()),
                                Err(e) => Err(Error::Network(e.to_string())),
                            }
                        }),
                    ));
                }

                if let Some(tcp) = &self.tcp_transport {
                    let tcp = tcp.clone();
                    let tcp_address = TransportAddress::Tcp("127.0.0.1:8000".parse().unwrap()); // Default mesh TCP
                    attempts.push((
                        "tcp_mesh".to_string(),
                        Box::pin(async move {
                            let mut transport = tcp.write().await;
                            match transport.connect(tcp_address).await {
                                Ok(_) => Ok(()),
                                Err(e) => Err(Error::Network(e.to_string())),
                            }
                        }),
                    ));
                }
            }
        }

        attempts
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
        use rand::{rngs::OsRng, Rng};
        let mut rng = OsRng;
        if rng.gen_bool(0.7) {
            // 70% success rate for simulation
            Ok(())
        } else {
            Err(Error::Network("Simulated connection failure".to_string()))
        }
    }

    /// Connect using single transport (legacy method)
    async fn connect_single_transport(
        &self,
        peer_id: PeerId,
        address: TransportAddress,
    ) -> Result<()> {
        match address {
            TransportAddress::Bluetooth(_) => {
                if let Some(bluetooth) = &self.bluetooth {
                    let mut bt = bluetooth.write().await;

                    // Attempt the connection
                    match bt.connect(address.clone()).await {
                        Ok(_) => {
                            // Enforce capacity limits before adding new connection
                            self.enforce_connection_capacity().await;

                            // Connection successful - update tracking with metadata
                            let metadata = ConnectionMetadata {
                                address: address.clone(),
                                established_at: Instant::now(),
                            };
                            self.connections.insert(peer_id, metadata);
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
        // DashMap provides thread-safe operations
        if let Some((_, metadata)) = self.connections.remove(&peer_id) {
            // Decrement connection count for this address
            self.decrement_connection_count(&metadata.address.clone())
                .await;

            // Perform actual disconnect based on transport type
            match metadata.address {
                TransportAddress::Bluetooth(_) => {
                    if let Some(bluetooth) = &self.bluetooth {
                        let mut bt = bluetooth.write().await;
                        bt.disconnect(peer_id).await.map_err(|e| {
                            Error::Network(format!("Bluetooth disconnect failed: {}", e))
                        })?;
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
            
            // Record network metric
            crate::monitoring::record_network_event("peer_disconnected", Some(&format!("{:?}", peer_id)));
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
        let connections_len = self.connections.len();
        let total_connections_by_address: usize = self
            .connection_counts_per_address
            .iter()
            .map(|entry| *entry.value())
            .sum();
        let attempts = self.connection_attempts.read().await;

        let now = Instant::now();
        let recent_attempts = attempts
            .iter()
            .filter(|attempt| now.duration_since(attempt.timestamp) < Duration::from_secs(60))
            .count();

        ConnectionStats {
            total_connections: connections_len,
            connections_by_address: self
                .connection_counts_per_address
                .iter()
                .map(|entry| (entry.key().clone(), *entry.value()))
                .collect(),
            recent_connection_attempts: recent_attempts,
            connection_limit: self.connection_limits.max_total_connections,
        }
    }

    /// Send data to a peer using best available transport with automatic failover
    pub async fn send_to_peer(&self, peer_id: PeerId, data: Vec<u8>) -> Result<()> {
        if let Some(metadata) = self.connections.get(&peer_id) {
            let address = metadata.address.clone(); // Extract address from metadata
                                                    // Try sending based on the known connection address
            let primary_result = self.send_via_address(peer_id, &address, &data).await;

            if primary_result.is_ok() {
                return Ok(());
            }

            log::debug!(
                "Primary transport failed for peer {:?}, trying failover",
                peer_id
            );

            // If primary transport failed and failover is enabled, try other transports
            if self.failover_enabled {
                // No need to drop connections as DashMap doesn't use locks
                return self.send_with_transport_failover(peer_id, data).await;
            }

            primary_result
        } else {
            Err(Error::Network(format!("Peer {:?} not connected", peer_id)))
        }
    }

    /// Send data via a specific transport address
    async fn send_via_address(
        &self,
        peer_id: PeerId,
        address: &TransportAddress,
        data: &[u8],
    ) -> Result<()> {
        match address {
            TransportAddress::Bluetooth(_) => {
                // Try enhanced Bluetooth first, then fall back to regular Bluetooth
                if let Some(enhanced_bt) = &self.enhanced_bluetooth {
                    if let Ok(result) = enhanced_bt
                        .read()
                        .await
                        .send_to_peer(peer_id, data.to_vec())
                        .await
                    {
                        return Ok(result);
                    }
                }

                if let Some(bluetooth) = &self.bluetooth {
                    let mut bt = bluetooth.write().await;
                    bt.send(peer_id, data.to_vec())
                        .await
                        .map_err(|e| Error::Network(format!("Bluetooth send failed: {}", e)))?;
                }
            }
            TransportAddress::Tcp(_) => {
                if let Some(tcp) = &self.tcp_transport {
                    let mut transport = tcp.write().await;
                    transport
                        .send(peer_id, data.to_vec())
                        .await
                        .map_err(|e| Error::Network(format!("TCP send failed: {}", e)))?;
                }
            }
            TransportAddress::Udp(_) => {
                return Err(Error::Network(
                    "UDP transport not yet implemented".to_string(),
                ));
            }
            TransportAddress::Mesh(_) => {
                // For mesh addresses, try all available transports
                return self
                    .send_with_transport_failover(peer_id, data.to_vec())
                    .await;
            }
        }

        Ok(())
    }

    /// Send with automatic transport failover
    async fn send_with_transport_failover(&self, peer_id: PeerId, data: Vec<u8>) -> Result<()> {
        let mut last_error = Error::Network("No transports available".to_string());

        // Try enhanced Bluetooth
        if let Some(enhanced_bt) = &self.enhanced_bluetooth {
            match enhanced_bt
                .read()
                .await
                .send_to_peer(peer_id, data.clone())
                .await
            {
                Ok(_) => {
                    log::debug!(
                        "Successfully sent via enhanced Bluetooth to peer {:?}",
                        peer_id
                    );
                    return Ok(());
                }
                Err(e) => {
                    log::debug!(
                        "Enhanced Bluetooth send failed to peer {:?}: {}",
                        peer_id,
                        e
                    );
                    last_error = e;
                }
            }
        }

        // Try TCP transport
        if let Some(tcp) = &self.tcp_transport {
            match tcp.write().await.send(peer_id, data.clone()).await {
                Ok(_) => {
                    log::debug!("Successfully sent via TCP to peer {:?}", peer_id);
                    return Ok(());
                }
                Err(e) => {
                    log::debug!("TCP send failed to peer {:?}: {}", peer_id, e);
                    last_error = Error::Network(e.to_string());
                }
            }
        }

        // Try regular Bluetooth as last resort
        if let Some(bluetooth) = &self.bluetooth {
            match bluetooth.write().await.send(peer_id, data).await {
                Ok(_) => {
                    log::debug!("Successfully sent via Bluetooth to peer {:?}", peer_id);
                    return Ok(());
                }
                Err(e) => {
                    log::debug!("Bluetooth send failed to peer {:?}: {}", peer_id, e);
                    last_error = Error::Network(e.to_string());
                }
            }
        }

        log::error!("All transport send attempts failed for peer {:?}", peer_id);
        Err(last_error)
    }

    /// Broadcast packet to all connected peers
    pub async fn broadcast_packet(&self, packet: BitchatPacket) -> Result<()> {
        let data = bincode::serialize(&packet)
            .map_err(|e| Error::Protocol(format!("Packet serialization failed: {}", e)))?;

        // Use DashMap iter() directly instead of read().await
        for peer_ref in self.connections.iter() {
            let peer_id = *peer_ref.key();
            if let Err(e) = self.send_to_peer(peer_id, data.clone()).await {
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
        self.connections.iter().map(|entry| *entry.key()).collect()
    }
}

/// Transport statistics for monitoring
#[derive(Debug, Clone, Default)]
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
