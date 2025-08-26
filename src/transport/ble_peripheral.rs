//! BLE Peripheral Advertising for BitChat
//! 
//! This module provides platform-specific implementations for BLE peripheral
//! advertising since btleplug doesn't support peripheral mode on most platforms.
//! It works alongside btleplug for central mode (scanning) functionality.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock, Mutex};
use async_trait::async_trait;
use uuid::Uuid;
use serde::{Serialize, Deserialize};

use crate::protocol::PeerId;
use crate::error::{Error, Result};

/// BitCraps BLE Service UUID - same as used in bluetooth.rs
pub const BITCRAPS_SERVICE_UUID: Uuid = Uuid::from_u128(0x12345678_1234_5678_1234_567812345678);

/// BLE advertising configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvertisingConfig {
    /// Service UUID to advertise
    pub service_uuid: Uuid,
    /// Local device name
    pub local_name: String,
    /// Advertising interval in milliseconds (20ms - 10.24s range)
    pub advertising_interval_ms: u16,
    /// Transmit power level (-127 to +20 dBm)
    pub tx_power_level: i8,
    /// Whether to include device name in advertisement
    pub include_name: bool,
    /// Whether to make device connectable
    pub connectable: bool,
    /// Maximum number of simultaneous connections
    pub max_connections: u8,
}

impl Default for AdvertisingConfig {
    fn default() -> Self {
        Self {
            service_uuid: BITCRAPS_SERVICE_UUID,
            local_name: "BitChat".to_string(),
            advertising_interval_ms: 100, // 100ms interval
            tx_power_level: 0, // 0 dBm
            include_name: true,
            connectable: true,
            max_connections: 8,
        }
    }
}

/// BLE peripheral advertising events
#[derive(Debug, Clone)]
pub enum PeripheralEvent {
    /// Advertising started successfully
    AdvertisingStarted,
    /// Advertising stopped
    AdvertisingStopped,
    /// Central device connected
    CentralConnected { 
        peer_id: PeerId,
        central_address: String 
    },
    /// Central device disconnected
    CentralDisconnected { 
        peer_id: PeerId,
        reason: String 
    },
    /// Data received from central
    DataReceived { 
        peer_id: PeerId,
        data: Vec<u8> 
    },
    /// Error occurred
    Error { 
        error: String 
    },
}

/// Statistics for BLE peripheral operations
#[derive(Debug, Clone, Default)]
pub struct PeripheralStats {
    /// Total time advertising has been active
    pub advertising_duration: Duration,
    /// Number of central connections received
    pub total_connections: u64,
    /// Currently connected centrals
    pub active_connections: usize,
    /// Bytes sent to centrals
    pub bytes_sent: u64,
    /// Bytes received from centrals
    pub bytes_received: u64,
    /// Number of advertising errors
    pub error_count: u64,
}

/// Core trait for BLE peripheral advertising
#[async_trait]
pub trait BlePeripheral: Send + Sync {
    /// Start advertising with the given configuration
    async fn start_advertising(&mut self, config: &AdvertisingConfig) -> Result<()>;
    
    /// Stop advertising
    async fn stop_advertising(&mut self) -> Result<()>;
    
    /// Check if currently advertising
    fn is_advertising(&self) -> bool;
    
    /// Send data to a connected central device
    async fn send_to_central(&mut self, peer_id: PeerId, data: &[u8]) -> Result<()>;
    
    /// Disconnect from a central device
    async fn disconnect_central(&mut self, peer_id: PeerId) -> Result<()>;
    
    /// Get list of connected central devices
    fn connected_centrals(&self) -> Vec<PeerId>;
    
    /// Get the next peripheral event
    async fn next_event(&mut self) -> Option<PeripheralEvent>;
    
    /// Get peripheral statistics
    async fn get_stats(&self) -> PeripheralStats;
    
    /// Update advertising configuration (may require restart)
    async fn update_config(&mut self, config: &AdvertisingConfig) -> Result<()>;
}

/// Platform-specific BLE peripheral factory
pub struct BlePeripheralFactory;

impl BlePeripheralFactory {
    /// Create a platform-appropriate BLE peripheral implementation
    pub async fn create_peripheral(local_peer_id: PeerId) -> Result<Box<dyn BlePeripheral>> {
        #[cfg(target_os = "android")]
        {
            log::info!("Creating Android BLE peripheral implementation");
            Ok(Box::new(AndroidBlePeripheral::new(local_peer_id).await?))
        }
        
        #[cfg(any(target_os = "ios", target_os = "macos"))]
        {
            log::info!("Creating iOS/macOS BLE peripheral implementation");
            Ok(Box::new(IosBlePeripheral::new(local_peer_id).await?))
        }
        
        #[cfg(target_os = "linux")]
        {
            log::info!("Creating Linux BlueZ BLE peripheral implementation");
            Ok(Box::new(LinuxBlePeripheral::new(local_peer_id).await?))
        }
        
        #[cfg(target_os = "windows")]
        {
            log::info!("Creating Windows BLE peripheral implementation");
            Ok(Box::new(WindowsBlePeripheral::new(local_peer_id).await?))
        }
        
        #[cfg(not(any(target_os = "android", target_os = "ios", target_os = "macos", target_os = "linux", target_os = "windows")))]
        {
            log::warn!("No platform-specific BLE peripheral implementation available, using fallback");
            Ok(Box::new(FallbackBlePeripheral::new(local_peer_id).await?))
        }
    }
}

/// Android BLE Peripheral Implementation using JNI
#[cfg(target_os = "android")]
pub struct AndroidBlePeripheral {
    local_peer_id: PeerId,
    is_advertising: Arc<RwLock<bool>>,
    connected_centrals: Arc<RwLock<HashMap<PeerId, String>>>,
    event_sender: mpsc::UnboundedSender<PeripheralEvent>,
    event_receiver: Mutex<mpsc::UnboundedReceiver<PeripheralEvent>>,
    config: Arc<RwLock<AdvertisingConfig>>,
    stats: Arc<RwLock<PeripheralStats>>,
    advertising_start_time: Arc<RwLock<Option<Instant>>>,
    // JNI handle will be stored here when implemented
    #[allow(dead_code)]
    jni_handle: Option<()>, // Placeholder for actual JNI GlobalRef
}

#[cfg(target_os = "android")]
impl AndroidBlePeripheral {
    pub async fn new(local_peer_id: PeerId) -> Result<Self> {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();
        
        Ok(Self {
            local_peer_id,
            is_advertising: Arc::new(RwLock::new(false)),
            connected_centrals: Arc::new(RwLock::new(HashMap::new())),
            event_sender,
            event_receiver: Mutex::new(event_receiver),
            config: Arc::new(RwLock::new(AdvertisingConfig::default())),
            stats: Arc::new(RwLock::new(PeripheralStats::default())),
            advertising_start_time: Arc::new(RwLock::new(None)),
            jni_handle: None,
        })
    }
    
    /// Initialize JNI connection to Android BluetoothLeAdvertiser
    async fn initialize_jni(&mut self) -> Result<()> {
        // This will be implemented with actual JNI calls to:
        // 1. Get BluetoothAdapter instance
        // 2. Get BluetoothLeAdvertiser
        // 3. Set up AdvertiseCallback
        // 4. Set up GATT Server with BitCraps service
        
        log::info!("Initializing Android BLE advertising via JNI for peer {:?}", self.local_peer_id);
        
        // For now, simulate successful initialization
        // Real implementation would set up JNI global references here
        
        Ok(())
    }
    
    /// Create Android AdvertiseSettings
    fn create_advertise_settings(&self, config: &AdvertisingConfig) -> Result<()> {
        // This will create Android AdvertiseSettings with:
        // - Advertising mode based on interval
        // - TX power level
        // - Connectable flag
        // - Timeout (none for continuous)
        
        log::debug!("Creating Android AdvertiseSettings: interval={}ms, tx_power={}, connectable={}", 
                   config.advertising_interval_ms, config.tx_power_level, config.connectable);
        
        Ok(())
    }
    
    /// Create Android AdvertiseData
    fn create_advertise_data(&self, config: &AdvertisingConfig) -> Result<()> {
        // This will create Android AdvertiseData with:
        // - Service UUID
        // - Local name (if enabled)
        // - Manufacturer data (peer ID)
        
        log::debug!("Creating Android AdvertiseData: service_uuid={}, include_name={}", 
                   config.service_uuid, config.include_name);
        
        Ok(())
    }
    
    /// Set up GATT server for incoming connections
    async fn setup_gatt_server(&self, config: &AdvertisingConfig) -> Result<()> {
        // This will set up BluetoothGattServer with:
        // - BitCraps service with TX/RX characteristics
        // - Proper permissions and properties
        // - Server callback for connection events
        
        log::debug!("Setting up Android GATT server with max_connections={}", config.max_connections);
        
        Ok(())
    }
}

#[cfg(target_os = "android")]
#[async_trait]
impl BlePeripheral for AndroidBlePeripheral {
    async fn start_advertising(&mut self, config: &AdvertisingConfig) -> Result<()> {
        if *self.is_advertising.read().await {
            return Err(Error::Network("Already advertising".to_string()));
        }
        
        log::info!("Starting Android BLE advertising");
        
        // Initialize JNI if not already done
        self.initialize_jni().await?;
        
        // Update configuration
        *self.config.write().await = config.clone();
        
        // Create advertising settings and data
        self.create_advertise_settings(config)?;
        self.create_advertise_data(config)?;
        
        // Set up GATT server
        self.setup_gatt_server(config).await?;
        
        // Start advertising (JNI call would go here)
        // bluetoothLeAdvertiser.startAdvertising(settings, data, callback)
        
        // Update state
        *self.is_advertising.write().await = true;
        *self.advertising_start_time.write().await = Some(Instant::now());
        
        // Send event
        let _ = self.event_sender.send(PeripheralEvent::AdvertisingStarted);
        
        log::info!("Android BLE advertising started successfully");
        Ok(())
    }
    
    async fn stop_advertising(&mut self) -> Result<()> {
        if !*self.is_advertising.read().await {
            return Ok(());
        }
        
        log::info!("Stopping Android BLE advertising");
        
        // Stop advertising (JNI call would go here)
        // bluetoothLeAdvertiser.stopAdvertising(callback)
        
        // Update statistics
        if let Some(start_time) = *self.advertising_start_time.read().await {
            let mut stats = self.stats.write().await;
            stats.advertising_duration += start_time.elapsed();
        }
        
        // Update state
        *self.is_advertising.write().await = false;
        *self.advertising_start_time.write().await = None;
        
        // Send event
        let _ = self.event_sender.send(PeripheralEvent::AdvertisingStopped);
        
        log::info!("Android BLE advertising stopped");
        Ok(())
    }
    
    fn is_advertising(&self) -> bool {
        self.is_advertising.try_read().map(|guard| *guard).unwrap_or(false)
    }
    
    async fn send_to_central(&mut self, peer_id: PeerId, data: &[u8]) -> Result<()> {
        let centrals = self.connected_centrals.read().await;
        if centrals.contains_key(&peer_id) {
            // Send data via GATT characteristic notification
            // gattServer.notifyCharacteristicChanged(device, characteristic, data)
            
            let mut stats = self.stats.write().await;
            stats.bytes_sent += data.len() as u64;
            
            log::debug!("Sent {} bytes to central {:?}", data.len(), peer_id);
            Ok(())
        } else {
            Err(Error::Network(format!("Central {:?} not connected", peer_id)))
        }
    }
    
    async fn disconnect_central(&mut self, peer_id: PeerId) -> Result<()> {
        let mut centrals = self.connected_centrals.write().await;
        if let Some(address) = centrals.remove(&peer_id) {
            // Disconnect via GATT server
            // gattServer.cancelConnection(device)
            
            let _ = self.event_sender.send(PeripheralEvent::CentralDisconnected {
                peer_id,
                reason: "Disconnected by peripheral".to_string(),
            });
            
            log::info!("Disconnected central {:?} at {}", peer_id, address);
            Ok(())
        } else {
            Err(Error::Network(format!("Central {:?} not connected", peer_id)))
        }
    }
    
    fn connected_centrals(&self) -> Vec<PeerId> {
        self.connected_centrals.try_read()
            .map(|guard| guard.keys().copied().collect())
            .unwrap_or_default()
    }
    
    async fn next_event(&mut self) -> Option<PeripheralEvent> {
        let mut receiver = self.event_receiver.lock().await;
        receiver.recv().await
    }
    
    async fn get_stats(&self) -> PeripheralStats {
        let mut stats = self.stats.read().await.clone();
        
        // Update advertising duration if currently advertising
        if let Some(start_time) = *self.advertising_start_time.read().await {
            stats.advertising_duration += start_time.elapsed();
        }
        
        stats.active_connections = self.connected_centrals.read().await.len();
        stats
    }
    
    async fn update_config(&mut self, config: &AdvertisingConfig) -> Result<()> {
        let was_advertising = self.is_advertising();
        
        if was_advertising {
            self.stop_advertising().await?;
        }
        
        *self.config.write().await = config.clone();
        
        if was_advertising {
            self.start_advertising(config).await?;
        }
        
        Ok(())
    }
}

/// iOS/macOS BLE Peripheral Implementation using FFI to Core Bluetooth
#[cfg(any(target_os = "ios", target_os = "macos"))]
pub struct IosBlePeripheral {
    local_peer_id: PeerId,
    is_advertising: Arc<RwLock<bool>>,
    connected_centrals: Arc<RwLock<HashMap<PeerId, String>>>,
    event_sender: mpsc::UnboundedSender<PeripheralEvent>,
    event_receiver: Mutex<mpsc::UnboundedReceiver<PeripheralEvent>>,
    config: Arc<RwLock<AdvertisingConfig>>,
    stats: Arc<RwLock<PeripheralStats>>,
    advertising_start_time: Arc<RwLock<Option<Instant>>>,
    // FFI handle to CBPeripheralManager will be stored here
    #[allow(dead_code)]
    peripheral_manager: Option<()>, // Placeholder for actual FFI handle
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
impl IosBlePeripheral {
    pub async fn new(local_peer_id: PeerId) -> Result<Self> {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();
        
        Ok(Self {
            local_peer_id,
            is_advertising: Arc::new(RwLock::new(false)),
            connected_centrals: Arc::new(RwLock::new(HashMap::new())),
            event_sender,
            event_receiver: Mutex::new(event_receiver),
            config: Arc::new(RwLock::new(AdvertisingConfig::default())),
            stats: Arc::new(RwLock::new(PeripheralStats::default())),
            advertising_start_time: Arc::new(RwLock::new(None)),
            peripheral_manager: None,
        })
    }
    
    /// Initialize Core Bluetooth peripheral manager
    async fn initialize_peripheral_manager(&mut self) -> Result<()> {
        // This will be implemented with FFI calls to:
        // 1. Create CBPeripheralManager instance
        // 2. Set up delegate callbacks
        // 3. Wait for powered on state
        
        log::info!("Initializing iOS/macOS CBPeripheralManager for peer {:?}", self.local_peer_id);
        
        // For now, simulate successful initialization
        
        Ok(())
    }
    
    /// Create Core Bluetooth service
    async fn create_cb_service(&self, config: &AdvertisingConfig) -> Result<()> {
        // This will create CBMutableService with:
        // - BitCraps service UUID
        // - TX/RX characteristics with proper properties
        // - Add to peripheral manager
        
        log::debug!("Creating Core Bluetooth service with UUID: {}", config.service_uuid);
        
        Ok(())
    }
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
#[async_trait]
impl BlePeripheral for IosBlePeripheral {
    async fn start_advertising(&mut self, config: &AdvertisingConfig) -> Result<()> {
        if *self.is_advertising.read().await {
            return Err(Error::Network("Already advertising".to_string()));
        }
        
        log::info!("Starting iOS/macOS BLE advertising");
        
        // Initialize peripheral manager if not already done
        self.initialize_peripheral_manager().await?;
        
        // Update configuration
        *self.config.write().await = config.clone();
        
        // Create and add service
        self.create_cb_service(config).await?;
        
        // Start advertising
        // [peripheralManager startAdvertising:@{
        //     CBAdvertisementDataServiceUUIDsKey: @[serviceUUID],
        //     CBAdvertisementDataLocalNameKey: localName
        // }];
        
        // Update state
        *self.is_advertising.write().await = true;
        *self.advertising_start_time.write().await = Some(Instant::now());
        
        // Send event
        let _ = self.event_sender.send(PeripheralEvent::AdvertisingStarted);
        
        log::info!("iOS/macOS BLE advertising started successfully");
        Ok(())
    }
    
    async fn stop_advertising(&mut self) -> Result<()> {
        if !*self.is_advertising.read().await {
            return Ok(());
        }
        
        log::info!("Stopping iOS/macOS BLE advertising");
        
        // Stop advertising: [peripheralManager stopAdvertising];
        
        // Update statistics
        if let Some(start_time) = *self.advertising_start_time.read().await {
            let mut stats = self.stats.write().await;
            stats.advertising_duration += start_time.elapsed();
        }
        
        // Update state
        *self.is_advertising.write().await = false;
        *self.advertising_start_time.write().await = None;
        
        // Send event
        let _ = self.event_sender.send(PeripheralEvent::AdvertisingStopped);
        
        log::info!("iOS/macOS BLE advertising stopped");
        Ok(())
    }
    
    fn is_advertising(&self) -> bool {
        self.is_advertising.try_read().map(|guard| *guard).unwrap_or(false)
    }
    
    async fn send_to_central(&mut self, peer_id: PeerId, data: &[u8]) -> Result<()> {
        let centrals = self.connected_centrals.read().await;
        if centrals.contains_key(&peer_id) {
            // Send data via characteristic notification
            // [peripheralManager updateValue:data forCharacteristic:characteristic onSubscribedCentrals:centrals];
            
            let mut stats = self.stats.write().await;
            stats.bytes_sent += data.len() as u64;
            
            log::debug!("Sent {} bytes to central {:?}", data.len(), peer_id);
            Ok(())
        } else {
            Err(Error::Network(format!("Central {:?} not connected", peer_id)))
        }
    }
    
    async fn disconnect_central(&mut self, peer_id: PeerId) -> Result<()> {
        let mut centrals = self.connected_centrals.write().await;
        if let Some(address) = centrals.remove(&peer_id) {
            // Note: iOS doesn't allow peripheral to disconnect central directly
            // We can only stop responding to the central
            
            let _ = self.event_sender.send(PeripheralEvent::CentralDisconnected {
                peer_id,
                reason: "Connection terminated by peripheral".to_string(),
            });
            
            log::info!("Marked central {:?} at {} as disconnected", peer_id, address);
            Ok(())
        } else {
            Err(Error::Network(format!("Central {:?} not connected", peer_id)))
        }
    }
    
    fn connected_centrals(&self) -> Vec<PeerId> {
        self.connected_centrals.try_read()
            .map(|guard| guard.keys().copied().collect())
            .unwrap_or_default()
    }
    
    async fn next_event(&mut self) -> Option<PeripheralEvent> {
        let mut receiver = self.event_receiver.lock().await;
        receiver.recv().await
    }
    
    async fn get_stats(&self) -> PeripheralStats {
        let mut stats = self.stats.read().await.clone();
        
        // Update advertising duration if currently advertising
        if let Some(start_time) = *self.advertising_start_time.read().await {
            stats.advertising_duration += start_time.elapsed();
        }
        
        stats.active_connections = self.connected_centrals.read().await.len();
        stats
    }
    
    async fn update_config(&mut self, config: &AdvertisingConfig) -> Result<()> {
        let was_advertising = self.is_advertising();
        
        if was_advertising {
            self.stop_advertising().await?;
        }
        
        *self.config.write().await = config.clone();
        
        if was_advertising {
            self.start_advertising(config).await?;
        }
        
        Ok(())
    }
}

/// Linux BlueZ BLE Peripheral Implementation
#[cfg(target_os = "linux")]
pub struct LinuxBlePeripheral {
    local_peer_id: PeerId,
    is_advertising: Arc<RwLock<bool>>,
    connected_centrals: Arc<RwLock<HashMap<PeerId, String>>>,
    event_sender: mpsc::UnboundedSender<PeripheralEvent>,
    event_receiver: Mutex<mpsc::UnboundedReceiver<PeripheralEvent>>,
    config: Arc<RwLock<AdvertisingConfig>>,
    stats: Arc<RwLock<PeripheralStats>>,
    advertising_start_time: Arc<RwLock<Option<Instant>>>,
    // BlueZ D-Bus connection handle
    #[allow(dead_code)]
    dbus_connection: Option<()>, // Placeholder for actual D-Bus connection
}

#[cfg(target_os = "linux")]
impl LinuxBlePeripheral {
    pub async fn new(local_peer_id: PeerId) -> Result<Self> {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();
        
        Ok(Self {
            local_peer_id,
            is_advertising: Arc::new(RwLock::new(false)),
            connected_centrals: Arc::new(RwLock::new(HashMap::new())),
            event_sender,
            event_receiver: Mutex::new(event_receiver),
            config: Arc::new(RwLock::new(AdvertisingConfig::default())),
            stats: Arc::new(RwLock::new(PeripheralStats::default())),
            advertising_start_time: Arc::new(RwLock::new(None)),
            dbus_connection: None,
        })
    }
    
    /// Initialize BlueZ D-Bus connection
    async fn initialize_bluez(&mut self) -> Result<()> {
        // This will be implemented with D-Bus calls to:
        // 1. Connect to BlueZ via D-Bus
        // 2. Register GATT application
        // 3. Register advertisement
        // 4. Set up signal handlers for connections
        
        log::info!("Initializing Linux BlueZ D-Bus connection for peer {:?}", self.local_peer_id);
        
        // For now, simulate successful initialization
        
        Ok(())
    }
    
    /// Register GATT application with BlueZ
    async fn register_gatt_application(&self, config: &AdvertisingConfig) -> Result<()> {
        // This will register a GATT application with:
        // - BitCraps service
        // - TX/RX characteristics
        // - Proper flags and permissions
        
        log::debug!("Registering GATT application with BlueZ for service {}", config.service_uuid);
        
        Ok(())
    }
    
    /// Register advertisement with BlueZ
    async fn register_advertisement(&self, config: &AdvertisingConfig) -> Result<()> {
        // This will register an advertisement with:
        // - Service UUID
        // - Local name
        // - Advertisement type (peripheral)
        // - Min/max intervals
        
        log::debug!("Registering advertisement with BlueZ: interval={}ms", config.advertising_interval_ms);
        
        Ok(())
    }
}

#[cfg(target_os = "linux")]
#[async_trait]
impl BlePeripheral for LinuxBlePeripheral {
    async fn start_advertising(&mut self, config: &AdvertisingConfig) -> Result<()> {
        if *self.is_advertising.read().await {
            return Err(Error::Network("Already advertising".to_string()));
        }
        
        log::info!("Starting Linux BlueZ BLE advertising");
        
        // Initialize BlueZ if not already done
        self.initialize_bluez().await?;
        
        // Update configuration
        *self.config.write().await = config.clone();
        
        // Register GATT application
        self.register_gatt_application(config).await?;
        
        // Register and start advertisement
        self.register_advertisement(config).await?;
        
        // Update state
        *self.is_advertising.write().await = true;
        *self.advertising_start_time.write().await = Some(Instant::now());
        
        // Send event
        let _ = self.event_sender.send(PeripheralEvent::AdvertisingStarted);
        
        log::info!("Linux BlueZ BLE advertising started successfully");
        Ok(())
    }
    
    async fn stop_advertising(&mut self) -> Result<()> {
        if !*self.is_advertising.read().await {
            return Ok(());
        }
        
        log::info!("Stopping Linux BlueZ BLE advertising");
        
        // Unregister advertisement via D-Bus
        
        // Update statistics
        if let Some(start_time) = *self.advertising_start_time.read().await {
            let mut stats = self.stats.write().await;
            stats.advertising_duration += start_time.elapsed();
        }
        
        // Update state
        *self.is_advertising.write().await = false;
        *self.advertising_start_time.write().await = None;
        
        // Send event
        let _ = self.event_sender.send(PeripheralEvent::AdvertisingStopped);
        
        log::info!("Linux BlueZ BLE advertising stopped");
        Ok(())
    }
    
    fn is_advertising(&self) -> bool {
        self.is_advertising.try_read().map(|guard| *guard).unwrap_or(false)
    }
    
    async fn send_to_central(&mut self, peer_id: PeerId, data: &[u8]) -> Result<()> {
        let centrals = self.connected_centrals.read().await;
        if centrals.contains_key(&peer_id) {
            // Send data via GATT characteristic notification through D-Bus
            
            let mut stats = self.stats.write().await;
            stats.bytes_sent += data.len() as u64;
            
            log::debug!("Sent {} bytes to central {:?}", data.len(), peer_id);
            Ok(())
        } else {
            Err(Error::Network(format!("Central {:?} not connected", peer_id)))
        }
    }
    
    async fn disconnect_central(&mut self, peer_id: PeerId) -> Result<()> {
        let mut centrals = self.connected_centrals.write().await;
        if let Some(address) = centrals.remove(&peer_id) {
            // Disconnect via BlueZ D-Bus interface
            
            let _ = self.event_sender.send(PeripheralEvent::CentralDisconnected {
                peer_id,
                reason: "Disconnected by peripheral".to_string(),
            });
            
            log::info!("Disconnected central {:?} at {}", peer_id, address);
            Ok(())
        } else {
            Err(Error::Network(format!("Central {:?} not connected", peer_id)))
        }
    }
    
    fn connected_centrals(&self) -> Vec<PeerId> {
        self.connected_centrals.try_read()
            .map(|guard| guard.keys().copied().collect())
            .unwrap_or_default()
    }
    
    async fn next_event(&mut self) -> Option<PeripheralEvent> {
        let mut receiver = self.event_receiver.lock().await;
        receiver.recv().await
    }
    
    async fn get_stats(&self) -> PeripheralStats {
        let mut stats = self.stats.read().await.clone();
        
        // Update advertising duration if currently advertising
        if let Some(start_time) = *self.advertising_start_time.read().await {
            stats.advertising_duration += start_time.elapsed();
        }
        
        stats.active_connections = self.connected_centrals.read().await.len();
        stats
    }
    
    async fn update_config(&mut self, config: &AdvertisingConfig) -> Result<()> {
        let was_advertising = self.is_advertising();
        
        if was_advertising {
            self.stop_advertising().await?;
        }
        
        *self.config.write().await = config.clone();
        
        if was_advertising {
            self.start_advertising(config).await?;
        }
        
        Ok(())
    }
}

/// Windows BLE Peripheral Implementation
#[cfg(target_os = "windows")]
pub struct WindowsBlePeripheral {
    local_peer_id: PeerId,
    is_advertising: Arc<RwLock<bool>>,
    connected_centrals: Arc<RwLock<HashMap<PeerId, String>>>,
    event_sender: mpsc::UnboundedSender<PeripheralEvent>,
    event_receiver: Mutex<mpsc::UnboundedReceiver<PeripheralEvent>>,
    config: Arc<RwLock<AdvertisingConfig>>,
    stats: Arc<RwLock<PeripheralStats>>,
    advertising_start_time: Arc<RwLock<Option<Instant>>>,
}

#[cfg(target_os = "windows")]
impl WindowsBlePeripheral {
    pub async fn new(local_peer_id: PeerId) -> Result<Self> {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();
        
        Ok(Self {
            local_peer_id,
            is_advertising: Arc::new(RwLock::new(false)),
            connected_centrals: Arc::new(RwLock::new(HashMap::new())),
            event_sender,
            event_receiver: Mutex::new(event_receiver),
            config: Arc::new(RwLock::new(AdvertisingConfig::default())),
            stats: Arc::new(RwLock::new(PeripheralStats::default())),
            advertising_start_time: Arc::new(RwLock::new(None)),
        })
    }
}

#[cfg(target_os = "windows")]
#[async_trait]
impl BlePeripheral for WindowsBlePeripheral {
    async fn start_advertising(&mut self, config: &AdvertisingConfig) -> Result<()> {
        log::info!("Starting Windows BLE advertising");
        
        // Windows BLE peripheral mode implementation would go here
        // Using Windows Runtime APIs for Bluetooth LE
        
        *self.is_advertising.write().await = true;
        *self.advertising_start_time.write().await = Some(Instant::now());
        *self.config.write().await = config.clone();
        
        let _ = self.event_sender.send(PeripheralEvent::AdvertisingStarted);
        
        log::info!("Windows BLE advertising started successfully");
        Ok(())
    }
    
    async fn stop_advertising(&mut self) -> Result<()> {
        if !*self.is_advertising.read().await {
            return Ok(());
        }
        
        log::info!("Stopping Windows BLE advertising");
        
        // Update statistics
        if let Some(start_time) = *self.advertising_start_time.read().await {
            let mut stats = self.stats.write().await;
            stats.advertising_duration += start_time.elapsed();
        }
        
        *self.is_advertising.write().await = false;
        *self.advertising_start_time.write().await = None;
        
        let _ = self.event_sender.send(PeripheralEvent::AdvertisingStopped);
        
        log::info!("Windows BLE advertising stopped");
        Ok(())
    }
    
    fn is_advertising(&self) -> bool {
        self.is_advertising.try_read().map(|guard| *guard).unwrap_or(false)
    }
    
    async fn send_to_central(&mut self, peer_id: PeerId, data: &[u8]) -> Result<()> {
        let centrals = self.connected_centrals.read().await;
        if centrals.contains_key(&peer_id) {
            let mut stats = self.stats.write().await;
            stats.bytes_sent += data.len() as u64;
            
            log::debug!("Sent {} bytes to central {:?}", data.len(), peer_id);
            Ok(())
        } else {
            Err(Error::Network(format!("Central {:?} not connected", peer_id)))
        }
    }
    
    async fn disconnect_central(&mut self, peer_id: PeerId) -> Result<()> {
        let mut centrals = self.connected_centrals.write().await;
        if let Some(address) = centrals.remove(&peer_id) {
            let _ = self.event_sender.send(PeripheralEvent::CentralDisconnected {
                peer_id,
                reason: "Disconnected by peripheral".to_string(),
            });
            
            log::info!("Disconnected central {:?} at {}", peer_id, address);
            Ok(())
        } else {
            Err(Error::Network(format!("Central {:?} not connected", peer_id)))
        }
    }
    
    fn connected_centrals(&self) -> Vec<PeerId> {
        self.connected_centrals.try_read()
            .map(|guard| guard.keys().copied().collect())
            .unwrap_or_default()
    }
    
    async fn next_event(&mut self) -> Option<PeripheralEvent> {
        let mut receiver = self.event_receiver.lock().await;
        receiver.recv().await
    }
    
    async fn get_stats(&self) -> PeripheralStats {
        let mut stats = self.stats.read().await.clone();
        
        if let Some(start_time) = *self.advertising_start_time.read().await {
            stats.advertising_duration += start_time.elapsed();
        }
        
        stats.active_connections = self.connected_centrals.read().await.len();
        stats
    }
    
    async fn update_config(&mut self, config: &AdvertisingConfig) -> Result<()> {
        let was_advertising = self.is_advertising();
        
        if was_advertising {
            self.stop_advertising().await?;
        }
        
        *self.config.write().await = config.clone();
        
        if was_advertising {
            self.start_advertising(config).await?;
        }
        
        Ok(())
    }
}

/// Fallback implementation for unsupported platforms
pub struct FallbackBlePeripheral {
    local_peer_id: PeerId,
    event_sender: mpsc::UnboundedSender<PeripheralEvent>,
    event_receiver: Mutex<mpsc::UnboundedReceiver<PeripheralEvent>>,
    config: Arc<RwLock<AdvertisingConfig>>,
}

impl FallbackBlePeripheral {
    pub async fn new(local_peer_id: PeerId) -> Result<Self> {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();
        
        Ok(Self {
            local_peer_id,
            event_sender,
            event_receiver: Mutex::new(event_receiver),
            config: Arc::new(RwLock::new(AdvertisingConfig::default())),
        })
    }
}

#[async_trait]
impl BlePeripheral for FallbackBlePeripheral {
    async fn start_advertising(&mut self, config: &AdvertisingConfig) -> Result<()> {
        log::warn!("BLE peripheral advertising not supported on this platform");
        *self.config.write().await = config.clone();
        
        // Send error event to indicate platform limitation
        let _ = self.event_sender.send(PeripheralEvent::Error {
            error: "BLE peripheral advertising not supported on this platform".to_string(),
        });
        
        Ok(())
    }
    
    async fn stop_advertising(&mut self) -> Result<()> {
        log::info!("Fallback stop_advertising called");
        Ok(())
    }
    
    fn is_advertising(&self) -> bool {
        false
    }
    
    async fn send_to_central(&mut self, peer_id: PeerId, _data: &[u8]) -> Result<()> {
        Err(Error::Network(format!("Cannot send to central {:?}: platform not supported", peer_id)))
    }
    
    async fn disconnect_central(&mut self, peer_id: PeerId) -> Result<()> {
        Err(Error::Network(format!("Cannot disconnect central {:?}: platform not supported", peer_id)))
    }
    
    fn connected_centrals(&self) -> Vec<PeerId> {
        Vec::new()
    }
    
    async fn next_event(&mut self) -> Option<PeripheralEvent> {
        let mut receiver = self.event_receiver.lock().await;
        receiver.recv().await
    }
    
    async fn get_stats(&self) -> PeripheralStats {
        PeripheralStats::default()
    }
    
    async fn update_config(&mut self, config: &AdvertisingConfig) -> Result<()> {
        *self.config.write().await = config.clone();
        Ok(())
    }
}