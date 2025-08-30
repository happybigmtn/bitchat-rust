//! Enhanced Bluetooth Transport with BLE Peripheral Advertising
//!
//! This module combines the existing BluetoothTransport (for central/scanning functionality)
//! with the new BLE peripheral advertising capabilities to create a fully bidirectional
//! Bluetooth mesh transport.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

use crate::error::{Error, Result};
use crate::protocol::PeerId;
use crate::transport::{
    ble_peripheral::ConnectionState,
    bounded_queue::{
        BoundedTransportEventQueue, BoundedTransportEventReceiver, BoundedTransportEventSender,
    },
    crypto::{ConnectionPriority, TransportCrypto},
    AdvertisingConfig, BlePeripheral, BlePeripheralFactory, BluetoothTransport, PeripheralEvent,
    PeripheralStats, Transport, TransportAddress, TransportEvent,
};

/// Enhanced Bluetooth transport combining central and peripheral roles
pub struct EnhancedBluetoothTransport {
    /// Existing central transport (scanning and connecting)
    central_transport: Arc<RwLock<BluetoothTransport>>,

    /// BLE peripheral for advertising and accepting connections
    peripheral: Arc<Mutex<Box<dyn BlePeripheral>>>,

    /// Local peer ID
    local_peer_id: PeerId,

    /// Bounded event queue for overflow protection
    event_queue: Arc<BoundedTransportEventQueue>,
    event_sender: BoundedTransportEventSender,
    event_receiver: Arc<Mutex<BoundedTransportEventReceiver>>,

    /// Peripheral event processing task
    peripheral_event_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,

    /// Configuration
    advertising_config: Arc<RwLock<AdvertisingConfig>>,

    /// Statistics combining both central and peripheral
    combined_stats: Arc<RwLock<EnhancedBluetoothStats>>,

    /// Role management
    is_advertising: Arc<RwLock<bool>>,
    is_scanning: Arc<RwLock<bool>>,

    /// Connection tracking
    peripheral_connections: Arc<RwLock<HashMap<PeerId, String>>>,

    /// State management
    initialization_complete: Arc<RwLock<bool>>,

    /// Transport-layer encryption
    transport_crypto: Arc<TransportCrypto>,

    /// Connection prioritization enabled
    prioritization_enabled: Arc<RwLock<bool>>,
}

/// Combined statistics for enhanced Bluetooth transport
#[derive(Debug, Clone, Default)]
pub struct EnhancedBluetoothStats {
    /// Central transport stats
    pub central_connections: usize,
    pub central_bytes_sent: u64,
    pub central_bytes_received: u64,

    /// Peripheral transport stats
    pub peripheral_stats: PeripheralStats,

    /// Combined metrics
    pub total_connections: usize,
    pub total_bytes_sent: u64,
    pub total_bytes_received: u64,

    /// Discovery metrics
    pub peers_discovered: u64,
    pub connection_attempts: u64,
    pub successful_connections: u64,
    pub failed_connections: u64,
}

impl EnhancedBluetoothTransport {
    /// Create new enhanced Bluetooth transport
    pub async fn new(local_peer_id: PeerId) -> Result<Self> {
        log::info!(
            "Creating enhanced Bluetooth transport for peer {:?}",
            local_peer_id
        );

        // Create central transport
        let central_transport = Arc::new(RwLock::new(
            BluetoothTransport::new(local_peer_id).await.map_err(|e| {
                Error::Network(format!("Failed to create central transport: {}", e))
            })?,
        ));

        // Create platform-specific peripheral
        let peripheral = Arc::new(Mutex::new(
            BlePeripheralFactory::create_peripheral(local_peer_id).await?,
        ));

        // Create bounded event queue for overflow protection
        let event_queue = Arc::new(BoundedTransportEventQueue::for_transport());
        let event_sender = event_queue.sender();
        let event_receiver = event_queue.receiver();

        // Create transport crypto for security
        let transport_crypto = Arc::new(TransportCrypto::new());

        let transport = Self {
            central_transport,
            peripheral,
            local_peer_id,
            event_queue: event_queue.clone(),
            event_sender,
            event_receiver: Arc::new(Mutex::new(event_receiver)),
            peripheral_event_task: Arc::new(Mutex::new(None)),
            advertising_config: Arc::new(RwLock::new(AdvertisingConfig::default())),
            combined_stats: Arc::new(RwLock::new(EnhancedBluetoothStats::default())),
            is_advertising: Arc::new(RwLock::new(false)),
            is_scanning: Arc::new(RwLock::new(false)),
            peripheral_connections: Arc::new(RwLock::new(HashMap::new())),
            initialization_complete: Arc::new(RwLock::new(false)),
            transport_crypto,
            prioritization_enabled: Arc::new(RwLock::new(true)),
        };

        // Start peripheral event processing
        transport.start_peripheral_event_processing().await;

        *transport.initialization_complete.write().await = true;

        log::info!("Enhanced Bluetooth transport created successfully");
        Ok(transport)
    }

    /// Initialize the enhanced transport with both roles
    pub async fn initialize(&mut self) -> Result<()> {
        log::info!("Initializing enhanced Bluetooth transport");

        // Platform-specific initialization is handled internally by the factory
        // The platform-specific implementations handle their own initialization
        log::debug!("Platform-specific peripheral initialization handled by factory");

        log::info!("Enhanced Bluetooth transport initialization complete");
        Ok(())
    }

    /// Start BLE advertising with the given configuration
    pub async fn start_advertising(&mut self, config: AdvertisingConfig) -> Result<()> {
        log::info!("Starting BLE advertising");

        // Update configuration
        *self.advertising_config.write().await = config.clone();

        // Start advertising on peripheral
        {
            let mut peripheral = self.peripheral.lock().await;
            peripheral.start_advertising(&config).await?;
        }

        *self.is_advertising.write().await = true;

        log::info!("BLE advertising started successfully");
        Ok(())
    }

    /// Stop BLE advertising
    pub async fn stop_advertising(&mut self) -> Result<()> {
        if !*self.is_advertising.read().await {
            return Ok(());
        }

        log::info!("Stopping BLE advertising");

        {
            let mut peripheral = self.peripheral.lock().await;
            peripheral.stop_advertising().await?;
        }

        *self.is_advertising.write().await = false;

        log::info!("BLE advertising stopped");
        Ok(())
    }

    /// Start scanning for peers
    pub async fn start_scanning(&mut self) -> Result<()> {
        log::info!("Starting BLE scanning");

        {
            let central = self.central_transport.write().await;
            central
                .scan_for_peers()
                .await
                .map_err(|e| Error::Network(format!("Failed to start scanning: {}", e)))?;
        }

        *self.is_scanning.write().await = true;

        log::info!("BLE scanning started successfully");
        Ok(())
    }

    /// Stop scanning for peers
    pub async fn stop_scanning(&mut self) -> Result<()> {
        if !*self.is_scanning.read().await {
            return Ok(());
        }

        log::info!("Stopping BLE scanning");

        // Note: BluetoothTransport doesn't expose a stop_scanning method
        // In practice, this would stop the scanning task

        *self.is_scanning.write().await = false;

        log::info!("BLE scanning stopped");
        Ok(())
    }

    /// Start both advertising and scanning for full mesh functionality
    pub async fn start_mesh_mode(&mut self, config: AdvertisingConfig) -> Result<()> {
        log::info!("Starting full mesh mode (advertising + scanning)");

        // Start advertising first
        self.start_advertising(config).await?;

        // Then start scanning
        self.start_scanning().await?;

        log::info!("Full mesh mode started successfully");
        Ok(())
    }

    /// Stop mesh mode
    pub async fn stop_mesh_mode(&mut self) -> Result<()> {
        log::info!("Stopping mesh mode");

        // Stop both advertising and scanning
        self.stop_advertising().await?;
        self.stop_scanning().await?;

        log::info!("Mesh mode stopped");
        Ok(())
    }

    /// Send data to a peer (tries both central and peripheral connections)
    /// Data is encrypted using transport-layer encryption
    pub async fn send_to_peer(&self, peer_id: PeerId, data: Vec<u8>) -> Result<()> {
        use std::time::Instant;
        let start_time = Instant::now();

        // Encrypt the data using transport crypto
        let encrypted_data = match self.transport_crypto.encrypt_message(peer_id, &data).await {
            Ok(encrypted) => encrypted,
            Err(e) => {
                log::warn!("Failed to encrypt data for peer {:?}: {}", peer_id, e);
                // Update metrics with failure
                let _ = self
                    .transport_crypto
                    .update_connection_metrics(peer_id, u32::MAX, false)
                    .await;
                return Err(e);
            }
        };

        let mut success = false;

        // Try peripheral connection first (if peer connected as central to us)
        {
            let peripheral_connections = self.peripheral_connections.read().await;
            if peripheral_connections.contains_key(&peer_id) {
                let mut peripheral = self.peripheral.lock().await;
                match peripheral.send_to_central(peer_id, &encrypted_data).await {
                    Ok(()) => {
                        success = true;
                        log::debug!(
                            "Sent {} encrypted bytes to peer {:?} via peripheral connection",
                            encrypted_data.len(),
                            peer_id
                        );
                    }
                    Err(e) => {
                        log::debug!("Failed to send via peripheral connection: {}", e);
                        // Fall through to try central connection
                    }
                }
            }
        }

        // Try central connection if peripheral failed
        if !success {
            let mut central = self.central_transport.write().await;
            match central.send(peer_id, encrypted_data.clone()).await {
                Ok(()) => {
                    success = true;
                    log::debug!(
                        "Sent {} encrypted bytes to peer {:?} via central connection",
                        encrypted_data.len(),
                        peer_id
                    );
                }
                Err(e) => {
                    log::debug!("Failed to send via central connection: {}", e);
                }
            }
        }

        // Update connection metrics
        let latency_ms = start_time.elapsed().as_millis() as u32;
        let _ = self
            .transport_crypto
            .update_connection_metrics(peer_id, latency_ms, success)
            .await;

        if success {
            Ok(())
        } else {
            Err(Error::Network(format!(
                "Failed to send to peer {:?}: no active connection",
                peer_id
            )))
        }
    }

    /// Get list of all connected peers (both central and peripheral)
    /// Sorted by connection priority if prioritization is enabled
    pub async fn get_all_connected_peers(&self) -> Vec<PeerId> {
        let mut peers = Vec::new();

        // Add central connections
        {
            let central = self.central_transport.read().await;
            peers.extend(central.connected_peers());
        }

        // Add peripheral connections
        {
            let peripheral_connections = self.peripheral_connections.read().await;
            peers.extend(peripheral_connections.keys().copied());
        }

        // Remove duplicates
        peers.sort();
        peers.dedup();

        // Sort by priority if enabled
        if *self.prioritization_enabled.read().await {
            let peer_priorities = self.transport_crypto.get_peers_by_priority().await;
            let priority_map: HashMap<PeerId, f32> = peer_priorities.into_iter().collect();

            peers.sort_by(|a, b| {
                let priority_a = priority_map.get(a).copied().unwrap_or(0.0);
                let priority_b = priority_map.get(b).copied().unwrap_or(0.0);
                priority_b
                    .partial_cmp(&priority_a)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }

        peers
    }

    /// Get combined statistics
    pub async fn get_combined_stats(&self) -> EnhancedBluetoothStats {
        let mut combined = self.combined_stats.read().await.clone();

        // Update peripheral stats
        {
            let peripheral = self.peripheral.lock().await;
            combined.peripheral_stats = peripheral.get_stats().await;
        }

        // Update central stats
        {
            let central = self.central_transport.read().await;
            let central_stats = central.bluetooth_stats().await;
            combined.central_connections = central_stats.active_connections;
        }

        // Update combined metrics
        combined.total_connections =
            combined.central_connections + combined.peripheral_stats.active_connections;
        combined.total_bytes_sent =
            combined.central_bytes_sent + combined.peripheral_stats.bytes_sent;
        combined.total_bytes_received =
            combined.central_bytes_received + combined.peripheral_stats.bytes_received;

        combined
    }

    /// Start peripheral event processing task
    async fn start_peripheral_event_processing(&self) {
        let peripheral = self.peripheral.clone();
        let event_sender = self.event_sender.clone();
        let peripheral_connections = self.peripheral_connections.clone();
        let combined_stats = self.combined_stats.clone();
        let transport_crypto = self.transport_crypto.clone();

        let task = tokio::spawn(async move {
            log::debug!("Starting peripheral event processing");

            loop {
                let event = {
                    let mut p = peripheral.lock().await;
                    p.next_event().await
                };

                match event {
                    Some(PeripheralEvent::AdvertisingStarted) => {
                        log::info!("Peripheral advertising started");
                        let _ = event_sender
                            .send(TransportEvent::Connected {
                                peer_id: [0u8; 32], // Placeholder for advertising started
                                address: TransportAddress::Bluetooth("advertising".to_string()),
                            })
                            .await;
                    }

                    Some(PeripheralEvent::CentralConnected {
                        peer_id,
                        central_address,
                    }) => {
                        log::info!(
                            "Central connected as peripheral: {:?} at {}",
                            peer_id,
                            central_address
                        );

                        // Track connection
                        peripheral_connections
                            .write()
                            .await
                            .insert(peer_id, central_address.clone());

                        // Update stats
                        {
                            let mut stats = combined_stats.write().await;
                            stats.successful_connections += 1;
                        }

                        // Perform key exchange for this peer
                        let crypto_public_key = transport_crypto.public_key();
                        // In a real implementation, we would exchange keys here
                        // For now, we'll use a mock key exchange
                        let _ = transport_crypto
                            .perform_key_exchange(peer_id, crypto_public_key)
                            .await;

                        // Send transport event
                        let _ = event_sender
                            .send(TransportEvent::Connected {
                                peer_id,
                                address: TransportAddress::Bluetooth(central_address),
                            })
                            .await;
                    }

                    Some(PeripheralEvent::CentralDisconnected { peer_id, reason }) => {
                        log::info!(
                            "Central disconnected from peripheral: {:?} ({})",
                            peer_id,
                            reason
                        );

                        // Remove connection
                        peripheral_connections.write().await.remove(&peer_id);

                        // Remove crypto state for disconnected peer
                        transport_crypto.remove_peer(peer_id).await;

                        // Send transport event
                        let _ = event_sender
                            .send(TransportEvent::Disconnected { peer_id, reason })
                            .await;
                    }

                    Some(PeripheralEvent::DataReceived { peer_id, data }) => {
                        log::debug!(
                            "Received {} encrypted bytes from central {:?}",
                            data.len(),
                            peer_id
                        );

                        // Decrypt the data
                        match transport_crypto.decrypt_message(peer_id, &data).await {
                            Ok(decrypted_data) => {
                                log::debug!(
                                    "Successfully decrypted {} bytes from peer {:?}",
                                    decrypted_data.len(),
                                    peer_id
                                );

                                // Update stats
                                {
                                    let mut stats = combined_stats.write().await;
                                    stats.total_bytes_received += decrypted_data.len() as u64;
                                }

                                // Send transport event with decrypted data
                                let _ = event_sender
                                    .send(TransportEvent::DataReceived {
                                        peer_id,
                                        data: decrypted_data,
                                    })
                                    .await;
                            }
                            Err(e) => {
                                log::error!(
                                    "Failed to decrypt message from peer {:?}: {}",
                                    peer_id,
                                    e
                                );
                                let _ = event_sender
                                    .send(TransportEvent::Error {
                                        peer_id: Some(peer_id),
                                        error: format!("Decryption failed: {}", e),
                                    })
                                    .await;
                            }
                        }
                    }

                    Some(PeripheralEvent::Error { error }) => {
                        log::error!("Peripheral error: {}", error);
                        let _ = event_sender
                            .send(TransportEvent::Error {
                                peer_id: None,
                                error,
                            })
                            .await;
                    }

                    Some(PeripheralEvent::AdvertisingStopped) => {
                        log::info!("Peripheral advertising stopped");
                    }

                    Some(PeripheralEvent::AdvertisingFailed {
                        error,
                        retry_suggested,
                        retry_delay_ms,
                    }) => {
                        log::error!(
                            "Peripheral advertising failed: {} (retry: {}, delay: {}ms)",
                            error,
                            retry_suggested,
                            retry_delay_ms
                        );
                        let _ = event_sender
                            .send(TransportEvent::Error {
                                peer_id: None,
                                error: format!("BLE advertising failed: {}", error),
                            })
                            .await;
                    }

                    Some(PeripheralEvent::ConnectionStateChanged { peer_id, state }) => match state
                    {
                        ConnectionState::Connected => {
                            log::info!("Peripheral connected to peer: {:?}", peer_id);
                            let _ = event_sender
                                .send(TransportEvent::Connected {
                                    peer_id,
                                    address: TransportAddress::Bluetooth("peripheral".to_string()),
                                })
                                .await;
                        }
                        ConnectionState::Disconnected => {
                            log::info!("Peripheral disconnected from peer: {:?}", peer_id);
                            let _ = event_sender
                                .send(TransportEvent::Disconnected {
                                    peer_id,
                                    reason: "BLE peripheral connection closed".to_string(),
                                })
                                .await;
                        }
                        ConnectionState::Connecting => {
                            log::debug!("Peripheral connecting to peer: {:?}", peer_id);
                        }
                        ConnectionState::Authenticating => {
                            log::debug!("Peripheral authenticating with peer: {:?}", peer_id);
                        }
                        ConnectionState::Ready => {
                            log::debug!(
                                "Peripheral ready for communication with peer: {:?}",
                                peer_id
                            );
                        }
                        ConnectionState::Disconnecting => {
                            log::debug!("Peripheral disconnecting from peer: {:?}", peer_id);
                        }
                        ConnectionState::Error(error) => {
                            log::error!(
                                "Peripheral connection error with peer {:?}: {}",
                                peer_id,
                                error
                            );
                            let _ = event_sender
                                .send(TransportEvent::Error {
                                    peer_id: Some(peer_id),
                                    error: format!("BLE peripheral connection error: {}", error),
                                })
                                .await;
                        }
                    },

                    Some(PeripheralEvent::PlatformEvent {
                        platform,
                        event_data,
                    }) => {
                        log::debug!(
                            "Platform-specific peripheral event on {}: {:?}",
                            platform,
                            event_data
                        );
                    }

                    None => {
                        // Channel closed, exit
                        log::debug!("Peripheral event channel closed");
                        break;
                    }
                }
            }
        });

        *self.peripheral_event_task.lock().await = Some(task);
    }

    /// Update advertising configuration
    pub async fn update_advertising_config(&mut self, config: AdvertisingConfig) -> Result<()> {
        let was_advertising = *self.is_advertising.read().await;

        if was_advertising {
            self.stop_advertising().await?;
        }

        *self.advertising_config.write().await = config.clone();

        if was_advertising {
            self.start_advertising(config).await?;
        }

        Ok(())
    }

    /// Set connection priority for a peer
    pub async fn set_peer_priority(
        &self,
        peer_id: PeerId,
        priority: ConnectionPriority,
    ) -> Result<()> {
        self.transport_crypto
            .set_connection_priority(peer_id, priority)
            .await
    }

    /// Get connection priority for a peer
    pub async fn get_peer_priority(&self, peer_id: PeerId) -> Option<f32> {
        let priorities = self.transport_crypto.get_peers_by_priority().await;
        priorities
            .iter()
            .find(|(id, _)| *id == peer_id)
            .map(|(_, priority)| *priority)
    }

    /// Enable or disable connection prioritization
    pub async fn set_prioritization_enabled(&self, enabled: bool) {
        *self.prioritization_enabled.write().await = enabled;
    }

    /// Get queue statistics for monitoring
    pub async fn get_queue_stats(&self) -> crate::transport::bounded_queue::QueueStats {
        self.event_queue.stats().await
    }

    /// Get transport crypto statistics
    pub async fn get_crypto_stats(&self) -> crate::transport::crypto::CryptoStats {
        self.transport_crypto.get_crypto_stats().await
    }
}

/// Implement the Transport trait for EnhancedBluetoothTransport
#[async_trait]
impl Transport for EnhancedBluetoothTransport {
    async fn listen(
        &mut self,
        address: TransportAddress,
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        match address {
            TransportAddress::Bluetooth(name) => {
                log::info!(
                    "Starting enhanced Bluetooth transport listening as: {}",
                    name
                );

                // Create advertising config from name
                let mut config = self.advertising_config.read().await.clone();
                config.local_name = name;

                // Start mesh mode (both advertising and scanning)
                self.start_mesh_mode(config).await?;

                Ok(())
            }
            _ => Err(Error::Network(
                "Invalid address type for enhanced Bluetooth transport".to_string(),
            )
            .into()),
        }
    }

    async fn connect(
        &mut self,
        address: TransportAddress,
    ) -> std::result::Result<PeerId, Box<dyn std::error::Error>> {
        // Delegate to central transport for outgoing connections
        let mut central = self.central_transport.write().await;
        match central.connect(address).await {
            Ok(peer_id) => Ok(peer_id),
            Err(e) => Err(Box::new(Error::Transport(e.to_string()))),
        }
    }

    async fn send(
        &mut self,
        peer_id: PeerId,
        data: Vec<u8>,
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        // Try central transport first
        let mut central = self.central_transport.write().await;
        match central.send(peer_id, data).await {
            Ok(()) => Ok(()),
            Err(e) => Err(Box::new(Error::Transport(e.to_string()))),
        }
    }

    async fn disconnect(
        &mut self,
        peer_id: PeerId,
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        // Try to disconnect from both central and peripheral connections
        let mut errors = Vec::new();

        // Try peripheral disconnect
        {
            let peripheral_connections = self.peripheral_connections.read().await;
            if peripheral_connections.contains_key(&peer_id) {
                let mut peripheral = self.peripheral.lock().await;
                if let Err(e) = peripheral.disconnect_central(peer_id).await {
                    errors.push(format!("Peripheral disconnect failed: {}", e));
                }
            }
        }

        // Try central disconnect
        {
            let mut central = self.central_transport.write().await;
            if central.is_connected(&peer_id) {
                if let Err(e) = central.disconnect(peer_id).await {
                    errors.push(format!("Central disconnect failed: {}", e));
                }
            }
        }

        if !errors.is_empty() {
            Err(Box::new(Error::Network(errors.join("; "))))
        } else {
            Ok(())
        }
    }

    fn is_connected(&self, peer_id: &PeerId) -> bool {
        // Check both central and peripheral connections
        let central_connected = {
            let central = match self.central_transport.try_read() {
                Ok(central) => central,
                Err(_) => return false,
            };
            central.is_connected(peer_id)
        };

        let peripheral_connected = {
            let peripheral_connections = match self.peripheral_connections.try_read() {
                Ok(connections) => connections,
                Err(_) => return false,
            };
            peripheral_connections.contains_key(peer_id)
        };

        central_connected || peripheral_connected
    }

    fn connected_peers(&self) -> Vec<PeerId> {
        // This is a synchronous method, so we can't use async
        // We'll provide a synchronous version that may be less accurate
        let mut peers = Vec::new();

        // Add central peers
        if let Ok(central) = self.central_transport.try_read() {
            peers.extend(central.connected_peers());
        }

        // Add peripheral peers
        if let Ok(peripheral_connections) = self.peripheral_connections.try_read() {
            peers.extend(peripheral_connections.keys().copied());
        }

        // Remove duplicates
        peers.sort();
        peers.dedup();

        peers
    }

    async fn next_event(&mut self) -> Option<TransportEvent> {
        // First check our bounded event queue
        {
            let receiver = self.event_receiver.lock().await;
            if let Ok(Some(event)) = receiver.try_recv().await {
                return Some(event);
            }
        }

        // Then check central transport
        {
            let mut central = self.central_transport.write().await;
            if let Some(event) = central.next_event().await {
                return Some(event);
            }
        }

        // Wait for next event from our bounded queue
        let receiver = self.event_receiver.lock().await;
        receiver.recv().await
    }
}

impl Drop for EnhancedBluetoothTransport {
    fn drop(&mut self) {
        // Clean up the peripheral event processing task
        if let Ok(mut task_guard) = self.peripheral_event_task.try_lock() {
            if let Some(task) = task_guard.take() {
                task.abort();
            }
        }
    }
}
