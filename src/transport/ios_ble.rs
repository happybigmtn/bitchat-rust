//! iOS/macOS BLE Peripheral Implementation via FFI to Core Bluetooth
//!
//! This module provides a complete iOS/macOS BLE peripheral implementation
//! using FFI calls to Core Bluetooth's CBPeripheralManager and related APIs.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Mutex, RwLock};
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::protocol::PeerId;
use crate::transport::ble_peripheral::{
    AdvertisingConfig, BlePeripheral, PeripheralEvent, PeripheralStats, BITCRAPS_SERVICE_UUID,
};

/// iOS/macOS Core Bluetooth FFI declarations
#[cfg(any(target_os = "ios", target_os = "macos"))]
mod core_bluetooth_ffi {
    use std::os::raw::{c_char, c_void};

    /// Opaque pointer to CBPeripheralManager
    #[repr(C)]
    pub struct CBPeripheralManager(pub *mut c_void);

    /// Opaque pointer to CBMutableService
    #[repr(C)]
    pub struct CBMutableService(pub *mut c_void);

    /// Opaque pointer to CBMutableCharacteristic
    #[repr(C)]
    pub struct CBMutableCharacteristic(pub *mut c_void);

    /// Opaque pointer to CBCentral
    #[repr(C)]
    pub struct CBCentral(pub *mut c_void);

    /// Core Bluetooth Manager State
    #[repr(C)]
    pub enum CBManagerState {
        Unknown = 0,
        Resetting = 1,
        Unsupported = 2,
        Unauthorized = 3,
        PoweredOff = 4,
        PoweredOn = 5,
    }

    /// Characteristic Properties
    #[repr(C)]
    pub enum CBCharacteristicProperties {
        Broadcast = 0x01,
        Read = 0x02,
        WriteWithoutResponse = 0x04,
        Write = 0x08,
        Notify = 0x10,
        Indicate = 0x20,
        AuthenticatedSignedWrites = 0x40,
        ExtendedProperties = 0x80,
        NotifyEncryptionRequired = 0x100,
        IndicateEncryptionRequired = 0x200,
    }

    /// Characteristic Permissions
    #[repr(C)]
    pub enum CBAttributePermissions {
        Readable = 0x01,
        Writeable = 0x02,
        ReadEncryptionRequired = 0x04,
        WriteEncryptionRequired = 0x08,
    }

    /// Callback function types
    pub type PeripheralManagerStateCallback = extern "C" fn(*mut c_void, CBManagerState);
    pub type PeripheralManagerAdvertisingCallback = extern "C" fn(*mut c_void, bool);
    pub type PeripheralManagerConnectionCallback = extern "C" fn(*mut c_void, *mut CBCentral);
    pub type PeripheralManagerCharacteristicCallback =
        extern "C" fn(*mut c_void, *mut CBCentral, *mut CBMutableCharacteristic, *const u8, usize);

    extern "C" {
        /// Create new CBPeripheralManager
        pub fn cb_peripheral_manager_new(
            user_data: *mut c_void,
            state_callback: PeripheralManagerStateCallback,
            advertising_callback: PeripheralManagerAdvertisingCallback,
            connection_callback: PeripheralManagerConnectionCallback,
            characteristic_callback: PeripheralManagerCharacteristicCallback,
        ) -> *mut CBPeripheralManager;

        /// Release CBPeripheralManager
        pub fn cb_peripheral_manager_release(manager: *mut CBPeripheralManager);

        /// Get peripheral manager state
        pub fn cb_peripheral_manager_state(manager: *mut CBPeripheralManager) -> CBManagerState;

        /// Create new mutable service
        pub fn cb_mutable_service_new(
            service_uuid: *const c_char,
            is_primary: bool,
        ) -> *mut CBMutableService;

        /// Release mutable service
        pub fn cb_mutable_service_release(service: *mut CBMutableService);

        /// Create new mutable characteristic
        pub fn cb_mutable_characteristic_new(
            characteristic_uuid: *const c_char,
            properties: u32,
            permissions: u32,
            value: *const u8,
            value_length: usize,
        ) -> *mut CBMutableCharacteristic;

        /// Release mutable characteristic
        pub fn cb_mutable_characteristic_release(characteristic: *mut CBMutableCharacteristic);

        /// Add characteristic to service
        pub fn cb_mutable_service_add_characteristic(
            service: *mut CBMutableService,
            characteristic: *mut CBMutableCharacteristic,
        );

        /// Add service to peripheral manager
        pub fn cb_peripheral_manager_add_service(
            manager: *mut CBPeripheralManager,
            service: *mut CBMutableService,
        );

        /// Start advertising
        pub fn cb_peripheral_manager_start_advertising(
            manager: *mut CBPeripheralManager,
            service_uuid: *const c_char,
            local_name: *const c_char,
        );

        /// Stop advertising
        pub fn cb_peripheral_manager_stop_advertising(manager: *mut CBPeripheralManager);

        /// Check if advertising
        pub fn cb_peripheral_manager_is_advertising(manager: *mut CBPeripheralManager) -> bool;

        /// Update characteristic value
        pub fn cb_peripheral_manager_update_value(
            manager: *mut CBPeripheralManager,
            characteristic: *mut CBMutableCharacteristic,
            value: *const u8,
            value_length: usize,
            central: *mut CBCentral,
        ) -> bool;

        /// Get central identifier
        pub fn cb_central_identifier(central: *mut CBCentral) -> *const c_char;

        /// Release central
        pub fn cb_central_release(central: *mut CBCentral);
    }
}

/// iOS/macOS BLE Peripheral using Core Bluetooth FFI
#[cfg(any(target_os = "ios", target_os = "macos"))]
pub struct IosBlePeripheral {
    local_peer_id: PeerId,
    is_advertising: Arc<RwLock<bool>>,
    connected_centrals: Arc<RwLock<HashMap<PeerId, (*mut core_bluetooth_ffi::CBCentral, String)>>>,
    event_sender: mpsc::Sender<PeripheralEvent>,
    event_receiver: Mutex<mpsc::Receiver<PeripheralEvent>>,
    config: Arc<RwLock<AdvertisingConfig>>,
    stats: Arc<RwLock<PeripheralStats>>,
    advertising_start_time: Arc<RwLock<Option<Instant>>>,

    // Core Bluetooth components
    peripheral_manager: Option<*mut core_bluetooth_ffi::CBPeripheralManager>,
    bitcraps_service: Option<*mut core_bluetooth_ffi::CBMutableService>,
    tx_characteristic: Option<*mut core_bluetooth_ffi::CBMutableCharacteristic>,
    rx_characteristic: Option<*mut core_bluetooth_ffi::CBMutableCharacteristic>,

    // State management
    manager_state: Arc<RwLock<core_bluetooth_ffi::CBManagerState>>,
    initialization_complete: Arc<RwLock<bool>>,

    // Recovery configuration
    recovery_config: Arc<RwLock<Option<RecoveryConfig>>>,
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
impl IosBlePeripheral {
    pub async fn new(local_peer_id: PeerId) -> Result<Self> {
        let (event_sender, event_receiver) = mpsc::channel(1000); // Bounded channel for backpressure

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
            bitcraps_service: None,
            tx_characteristic: None,
            rx_characteristic: None,

            manager_state: Arc::new(RwLock::new(core_bluetooth_ffi::CBManagerState::Unknown)),
            initialization_complete: Arc::new(RwLock::new(false)),
            recovery_config: Arc::new(RwLock::new(None)),
        })
    }

    /// Initialize Core Bluetooth peripheral manager
    pub async fn initialize_core_bluetooth(&mut self) -> Result<()> {
        log::info!(
            "Initializing iOS/macOS Core Bluetooth for peer {:?}",
            self.local_peer_id
        );

        // Create peripheral manager with callbacks
        let user_data = self as *mut Self as *mut std::os::raw::c_void;

        let peripheral_manager = unsafe {
            core_bluetooth_ffi::cb_peripheral_manager_new(
                user_data,
                Self::peripheral_manager_state_callback,
                Self::peripheral_manager_advertising_callback,
                Self::peripheral_manager_connection_callback,
                Self::peripheral_manager_characteristic_callback,
            )
        };

        if peripheral_manager.is_null() {
            return Err(Error::Network(
                "Failed to create CBPeripheralManager".to_string(),
            ));
        }

        self.peripheral_manager = Some(peripheral_manager);

        // Wait for peripheral manager to be ready
        self.wait_for_powered_on().await?;

        // Create service and characteristics
        self.create_core_bluetooth_service().await?;

        // Add service to peripheral manager
        if let (Some(manager), Some(service)) = (self.peripheral_manager, self.bitcraps_service) {
            unsafe {
                core_bluetooth_ffi::cb_peripheral_manager_add_service(manager, service);
            }
        }

        *self.initialization_complete.write().await = true;

        log::info!("iOS/macOS Core Bluetooth initialization completed successfully");
        Ok(())
    }

    /// Wait for peripheral manager to reach powered on state
    async fn wait_for_powered_on(&self) -> Result<()> {
        let mut attempts = 0;
        const MAX_ATTEMPTS: u32 = 50; // 5 seconds with 100ms intervals

        while attempts < MAX_ATTEMPTS {
            let state = *self.manager_state.read().await;

            match state {
                core_bluetooth_ffi::CBManagerState::PoweredOn => {
                    log::info!("Core Bluetooth peripheral manager is powered on");
                    return Ok(());
                }
                core_bluetooth_ffi::CBManagerState::PoweredOff => {
                    return Err(Error::Network("Bluetooth is powered off".to_string()));
                }
                core_bluetooth_ffi::CBManagerState::Unsupported => {
                    return Err(Error::Network(
                        "Bluetooth Low Energy is not supported on this device".to_string(),
                    ));
                }
                core_bluetooth_ffi::CBManagerState::Unauthorized => {
                    return Err(Error::Network(
                        "App is not authorized to use Bluetooth Low Energy".to_string(),
                    ));
                }
                _ => {
                    // Still initializing, wait a bit
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    attempts += 1;
                }
            }
        }

        Err(Error::Network(
            "Timeout waiting for Bluetooth to become ready".to_string(),
        ))
    }

    /// Create Core Bluetooth service and characteristics
    async fn create_core_bluetooth_service(&mut self) -> Result<()> {
        log::debug!("Creating Core Bluetooth service and characteristics");

        // Create service
        let service_uuid_cstr = std::ffi::CString::new(BITCRAPS_SERVICE_UUID.to_string())
            .map_err(|e| Error::Network(format!("Failed to create service UUID CString: {}", e)))?;

        let service =
            unsafe { core_bluetooth_ffi::cb_mutable_service_new(service_uuid_cstr.as_ptr(), true) };

        if service.is_null() {
            return Err(Error::Network(
                "Failed to create CBMutableService".to_string(),
            ));
        }

        self.bitcraps_service = Some(service);

        // Create TX characteristic (for sending data to centrals)
        self.create_tx_characteristic().await?;

        // Create RX characteristic (for receiving data from centrals)
        self.create_rx_characteristic().await?;

        log::debug!("Core Bluetooth service created successfully");
        Ok(())
    }

    /// Create TX characteristic for sending data to centrals
    async fn create_tx_characteristic(&mut self) -> Result<()> {
        let tx_uuid = Uuid::from_u128(BITCRAPS_SERVICE_UUID.as_u128() + 1);
        let tx_uuid_cstr = std::ffi::CString::new(tx_uuid.to_string())
            .map_err(|e| Error::Network(format!("Failed to create TX UUID CString: {}", e)))?;

        let properties = core_bluetooth_ffi::CBCharacteristicProperties::Read as u32
            | core_bluetooth_ffi::CBCharacteristicProperties::Notify as u32;

        let permissions = core_bluetooth_ffi::CBAttributePermissions::Readable as u32;

        let tx_characteristic = unsafe {
            core_bluetooth_ffi::cb_mutable_characteristic_new(
                tx_uuid_cstr.as_ptr(),
                properties,
                permissions,
                std::ptr::null(),
                0,
            )
        };

        if tx_characteristic.is_null() {
            return Err(Error::Network(
                "Failed to create TX characteristic".to_string(),
            ));
        }

        // Add characteristic to service
        if let Some(service) = self.bitcraps_service {
            unsafe {
                core_bluetooth_ffi::cb_mutable_service_add_characteristic(
                    service,
                    tx_characteristic,
                );
            }
        }

        self.tx_characteristic = Some(tx_characteristic);

        log::debug!("TX characteristic created successfully");
        Ok(())
    }

    /// Create RX characteristic for receiving data from centrals
    async fn create_rx_characteristic(&mut self) -> Result<()> {
        let rx_uuid = Uuid::from_u128(BITCRAPS_SERVICE_UUID.as_u128() + 2);
        let rx_uuid_cstr = std::ffi::CString::new(rx_uuid.to_string())
            .map_err(|e| Error::Network(format!("Failed to create RX UUID CString: {}", e)))?;

        let properties = core_bluetooth_ffi::CBCharacteristicProperties::Write as u32
            | core_bluetooth_ffi::CBCharacteristicProperties::WriteWithoutResponse as u32;

        let permissions = core_bluetooth_ffi::CBAttributePermissions::Writeable as u32;

        let rx_characteristic = unsafe {
            core_bluetooth_ffi::cb_mutable_characteristic_new(
                rx_uuid_cstr.as_ptr(),
                properties,
                permissions,
                std::ptr::null(),
                0,
            )
        };

        if rx_characteristic.is_null() {
            return Err(Error::Network(
                "Failed to create RX characteristic".to_string(),
            ));
        }

        // Add characteristic to service
        if let Some(service) = self.bitcraps_service {
            unsafe {
                core_bluetooth_ffi::cb_mutable_service_add_characteristic(
                    service,
                    rx_characteristic,
                );
            }
        }

        self.rx_characteristic = Some(rx_characteristic);

        log::debug!("RX characteristic created successfully");
        Ok(())
    }

    /// Start iOS/macOS BLE advertising
    pub async fn start_ios_advertising(&mut self, config: &AdvertisingConfig) -> Result<()> {
        if *self.is_advertising.read().await {
            return Err(Error::Network("Already advertising".to_string()));
        }

        if !*self.initialization_complete.read().await {
            return Err(Error::Network("Core Bluetooth not initialized".to_string()));
        }

        log::info!("Starting iOS/macOS BLE advertising");

        let peripheral_manager = self
            .peripheral_manager
            .ok_or_else(|| Error::Network("Peripheral manager not initialized".to_string()))?;

        // Create CStrings for FFI
        let service_uuid_cstr = std::ffi::CString::new(config.service_uuid.to_string())
            .map_err(|e| Error::Network(format!("Failed to create service UUID CString: {}", e)))?;

        let local_name_cstr = if config.include_name {
            Some(
                std::ffi::CString::new(config.local_name.clone()).map_err(|e| {
                    Error::Network(format!("Failed to create local name CString: {}", e))
                })?,
            )
        } else {
            None
        };

        // Start advertising
        unsafe {
            core_bluetooth_ffi::cb_peripheral_manager_start_advertising(
                peripheral_manager,
                service_uuid_cstr.as_ptr(),
                local_name_cstr
                    .as_ref()
                    .map_or(std::ptr::null(), |s| s.as_ptr()),
            );
        }

        // Update state
        *self.is_advertising.write().await = true;
        *self.advertising_start_time.write().await = Some(Instant::now());
        *self.config.write().await = config.clone();

        // Note: The advertising started event will be sent via callback

        log::info!("iOS/macOS BLE advertising start initiated");
        Ok(())
    }

    /// Stop iOS/macOS BLE advertising
    pub async fn stop_ios_advertising(&mut self) -> Result<()> {
        if !*self.is_advertising.read().await {
            return Ok(());
        }

        log::info!("Stopping iOS/macOS BLE advertising");

        let peripheral_manager = self
            .peripheral_manager
            .ok_or_else(|| Error::Network("Peripheral manager not initialized".to_string()))?;

        // Stop advertising
        unsafe {
            core_bluetooth_ffi::cb_peripheral_manager_stop_advertising(peripheral_manager);
        }

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

    /// Send data to a connected central via characteristic notification
    pub async fn send_to_ios_central(&mut self, peer_id: PeerId, data: &[u8]) -> Result<()> {
        let centrals = self.connected_centrals.read().await;

        if let Some((central_ptr, _address)) = centrals.get(&peer_id) {
            let tx_characteristic = self
                .tx_characteristic
                .ok_or_else(|| Error::Network("TX characteristic not available".to_string()))?;

            let peripheral_manager = self
                .peripheral_manager
                .ok_or_else(|| Error::Network("Peripheral manager not initialized".to_string()))?;

            // Update characteristic value and notify central
            let success = unsafe {
                core_bluetooth_ffi::cb_peripheral_manager_update_value(
                    peripheral_manager,
                    tx_characteristic,
                    data.as_ptr(),
                    data.len(),
                    *central_ptr,
                )
            };

            if success {
                let mut stats = self.stats.write().await;
                stats.bytes_sent += data.len() as u64;

                log::debug!("Sent {} bytes to central {:?}", data.len(), peer_id);
                Ok(())
            } else {
                Err(Error::Network(format!(
                    "Failed to send data to central {:?}",
                    peer_id
                )))
            }
        } else {
            Err(Error::Network(format!(
                "Central {:?} not connected",
                peer_id
            )))
        }
    }

    /// Disconnect from a central (iOS doesn't allow peripheral to disconnect directly)
    pub async fn disconnect_ios_central(&mut self, peer_id: PeerId) -> Result<()> {
        let mut centrals = self.connected_centrals.write().await;

        if let Some((_central_ptr, address)) = centrals.remove(&peer_id) {
            // Note: iOS doesn't allow peripheral to actively disconnect central
            // We can only stop responding to requests

            let _ = self
                .event_sender
                .send(PeripheralEvent::CentralDisconnected {
                    peer_id,
                    reason: "Connection terminated by peripheral".to_string(),
                });

            log::info!(
                "Marked central {:?} at {} as disconnected",
                peer_id,
                address
            );
            Ok(())
        } else {
            Err(Error::Network(format!(
                "Central {:?} not connected",
                peer_id
            )))
        }
    }

    /// Core Bluetooth callbacks
    extern "C" fn peripheral_manager_state_callback(
        user_data: *mut std::os::raw::c_void,
        state: core_bluetooth_ffi::CBManagerState,
    ) {
        if user_data.is_null() {
            return;
        }

        let peripheral = unsafe { &mut *(user_data as *mut IosBlePeripheral) };

        log::debug!("Peripheral manager state changed to: {:?}", state);

        // Update state in a thread-safe way
        tokio::spawn(async move {
            *peripheral.manager_state.write().await = state;
        });
    }

    extern "C" fn peripheral_manager_advertising_callback(
        user_data: *mut std::os::raw::c_void,
        success: bool,
    ) {
        if user_data.is_null() {
            return;
        }

        let peripheral = unsafe { &mut *(user_data as *mut IosBlePeripheral) };

        if success {
            log::info!("iOS/macOS BLE advertising started successfully");
            let _ = peripheral
                .event_sender
                .send(PeripheralEvent::AdvertisingStarted);
        } else {
            log::error!("iOS/macOS BLE advertising failed to start");

            // Update state and stats
            tokio::spawn(async move {
                *peripheral.is_advertising.write().await = false;
                *peripheral.advertising_start_time.write().await = None;

                let mut stats = peripheral.stats.write().await;
                stats.error_count += 1;

                let _ = peripheral.event_sender.send(PeripheralEvent::Error {
                    error: "Failed to start advertising".to_string(),
                });
            });
        }
    }

    extern "C" fn peripheral_manager_connection_callback(
        user_data: *mut std::os::raw::c_void,
        central: *mut core_bluetooth_ffi::CBCentral,
    ) {
        if user_data.is_null() || central.is_null() {
            return;
        }

        let peripheral = unsafe { &mut *(user_data as *mut IosBlePeripheral) };

        // Get central identifier
        let central_id_cstr = unsafe { core_bluetooth_ffi::cb_central_identifier(central) };

        if central_id_cstr.is_null() {
            return;
        }

        let central_id = unsafe {
            std::ffi::CStr::from_ptr(central_id_cstr)
                .to_string_lossy()
                .into_owned()
        };

        // Generate peer ID from central identifier
        let peer_id = {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            use std::hash::Hasher;
            hasher.write(central_id.as_bytes());
            let hash = hasher.finish();
            let mut peer_id = [0u8; 32];
            peer_id[..8].copy_from_slice(&hash.to_be_bytes());
            peer_id
        };

        log::info!("Central connected: {} -> {:?}", central_id, peer_id);

        // Store connection
        tokio::spawn(async move {
            let mut centrals = peripheral.connected_centrals.write().await;
            centrals.insert(peer_id, (central, central_id.clone()));

            let mut stats = peripheral.stats.write().await;
            stats.total_connections += 1;

            let _ = peripheral
                .event_sender
                .send(PeripheralEvent::CentralConnected {
                    peer_id,
                    central_address: central_id,
                });
        });
    }

    extern "C" fn peripheral_manager_characteristic_callback(
        user_data: *mut std::os::raw::c_void,
        central: *mut core_bluetooth_ffi::CBCentral,
        _characteristic: *mut core_bluetooth_ffi::CBMutableCharacteristic,
        data: *const u8,
        data_length: usize,
    ) {
        if user_data.is_null() || central.is_null() || data.is_null() || data_length == 0 {
            return;
        }

        let peripheral = unsafe { &mut *(user_data as *mut IosBlePeripheral) };

        // Get central identifier and find peer ID
        let central_id_cstr = unsafe { core_bluetooth_ffi::cb_central_identifier(central) };

        if central_id_cstr.is_null() {
            return;
        }

        let central_id = unsafe {
            std::ffi::CStr::from_ptr(central_id_cstr)
                .to_string_lossy()
                .into_owned()
        };

        // Convert data
        let received_data = unsafe { std::slice::from_raw_parts(data, data_length).to_vec() };

        log::debug!(
            "Received {} bytes from central {}",
            received_data.len(),
            central_id
        );

        // Find peer ID and send event
        tokio::spawn(async move {
            let centrals = peripheral.connected_centrals.read().await;

            // Find peer ID by central address
            for (peer_id, (_ptr, address)) in centrals.iter() {
                if address == &central_id {
                    let mut stats = peripheral.stats.write().await;
                    stats.bytes_received += received_data.len() as u64;

                    let _ = peripheral.event_sender.send(PeripheralEvent::DataReceived {
                        peer_id: *peer_id,
                        data: received_data,
                    });
                    break;
                }
            }
        });
    }
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
impl Drop for IosBlePeripheral {
    fn drop(&mut self) {
        // Clean up Core Bluetooth resources
        if let Some(manager) = self.peripheral_manager {
            unsafe {
                core_bluetooth_ffi::cb_peripheral_manager_release(manager);
            }
        }

        if let Some(service) = self.bitcraps_service {
            unsafe {
                core_bluetooth_ffi::cb_mutable_service_release(service);
            }
        }

        if let Some(characteristic) = self.tx_characteristic {
            unsafe {
                core_bluetooth_ffi::cb_mutable_characteristic_release(characteristic);
            }
        }

        if let Some(characteristic) = self.rx_characteristic {
            unsafe {
                core_bluetooth_ffi::cb_mutable_characteristic_release(characteristic);
            }
        }

        // Release central references
        if let Ok(centrals) = self.connected_centrals.try_read() {
            for (_peer_id, (central_ptr, _address)) in centrals.iter() {
                unsafe {
                    core_bluetooth_ffi::cb_central_release(*central_ptr);
                }
            }
        }
    }
}

/// Implement BlePeripheral trait for iOS
#[cfg(any(target_os = "ios", target_os = "macos"))]
#[async_trait::async_trait]
impl BlePeripheral for IosBlePeripheral {
    async fn start_advertising(&mut self, config: &AdvertisingConfig) -> Result<()> {
        self.start_ios_advertising(config).await
    }

    async fn stop_advertising(&mut self) -> Result<()> {
        self.stop_ios_advertising().await
    }

    fn is_advertising(&self) -> bool {
        self.is_advertising
            .try_read()
            .map(|guard| *guard)
            .unwrap_or(false)
    }

    async fn send_to_central(&mut self, peer_id: PeerId, data: &[u8]) -> Result<()> {
        self.send_to_ios_central(peer_id, data).await
    }

    async fn disconnect_central(&mut self, peer_id: PeerId) -> Result<()> {
        self.disconnect_ios_central(peer_id).await
    }

    fn connected_centrals(&self) -> Vec<PeerId> {
        self.connected_centrals
            .try_read()
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

    async fn set_recovery_config(&mut self, config: RecoveryConfig) -> Result<()> {
        // Store recovery configuration
        let mut recovery_config = self.recovery_config.write().await;
        *recovery_config = Some(config);
        log::debug!("Recovery configuration updated for iOS BLE");
        Ok(())
    }

    async fn recover(&mut self) -> Result<()> {
        log::warn!("Attempting iOS BLE recovery");

        // Stop advertising and reinitialize Core Bluetooth
        self.stop_advertising().await?;
        tokio::time::sleep(Duration::from_millis(2000)).await;

        // Wait for powered on state again
        self.wait_for_powered_on().await?;

        // Restart with current config
        let config = self.config.read().await.clone();
        self.start_advertising(&config).await
    }

    async fn get_connection_state(&self, peer_id: PeerId) -> Option<ConnectionState> {
        self.connected_centrals
            .read()
            .await
            .get(&peer_id)
            .map(|_| ConnectionState::Connected)
    }

    async fn force_reconnect(&mut self, peer_id: PeerId) -> Result<()> {
        // iOS doesn't allow peripheral to force reconnection
        // We can only disconnect and wait for central to reconnect
        self.disconnect_central(peer_id).await?;
        Ok(())
    }

    async fn health_check(&self) -> Result<bool> {
        // Check if peripheral manager is still valid and powered on
        let state = *self.manager_state.read().await;
        match state {
            core_bluetooth_ffi::CBManagerState::PoweredOn => Ok(true),
            _ => Ok(false),
        }
    }

    async fn reset(&mut self) -> Result<()> {
        log::info!("Resetting iOS BLE peripheral");

        // Stop advertising and clear all connections
        self.stop_advertising().await?;
        self.connected_centrals.write().await.clear();

        // Reset statistics
        *self.stats.write().await = PeripheralStats::default();
        *self.initialization_complete.write().await = false;

        // Reinitialize Core Bluetooth
        self.initialize_core_bluetooth().await
    }
}
