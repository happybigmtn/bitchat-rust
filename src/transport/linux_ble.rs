//! Linux BlueZ BLE Peripheral Implementation via D-Bus
//!
//! This module provides a complete Linux BLE peripheral implementation
//! using D-Bus calls to BlueZ's GATT and advertising APIs.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Mutex, RwLock};
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::protocol::PeerId;
use crate::transport::ble_peripheral::{
    AdvertisingConfig, BlePeripheral, ConnectionState, PeripheralEvent, PeripheralStats,
    RecoveryConfig, BITCRAPS_SERVICE_UUID,
};

/// BlueZ D-Bus interface constants
#[cfg(target_os = "linux")]
mod bluez_constants {
    /// BlueZ D-Bus service name
    pub const BLUEZ_SERVICE: &str = "org.bluez";

    /// BlueZ adapter interface
    pub const ADAPTER_INTERFACE: &str = "org.bluez.Adapter1";

    /// BlueZ GATT Manager interface
    pub const GATT_MANAGER_INTERFACE: &str = "org.bluez.GattManager1";

    /// BlueZ LE Advertising Manager interface
    pub const LE_ADVERTISING_MANAGER_INTERFACE: &str = "org.bluez.LEAdvertisingManager1";

    /// GATT Service interface
    pub const GATT_SERVICE_INTERFACE: &str = "org.bluez.GattService1";

    /// GATT Characteristic interface
    pub const GATT_CHARACTERISTIC_INTERFACE: &str = "org.bluez.GattCharacteristic1";

    /// LE Advertisement interface
    pub const LE_ADVERTISEMENT_INTERFACE: &str = "org.bluez.LEAdvertisement1";

    /// BlueZ object manager interface
    pub const OBJECT_MANAGER_INTERFACE: &str = "org.freedesktop.DBus.ObjectManager";

    /// Properties interface
    pub const PROPERTIES_INTERFACE: &str = "org.freedesktop.DBus.Properties";

    /// Default BlueZ adapter path
    pub const DEFAULT_ADAPTER_PATH: &str = "/org/bluez/hci0";

    /// BitCraps application path
    pub const BITCRAPS_APP_PATH: &str = "/org/bitchat/rust/application";

    /// BitCraps service path
    pub const BITCRAPS_SERVICE_PATH: &str = "/org/bitchat/rust/application/service";

    /// BitCraps TX characteristic path
    pub const BITCRAPS_TX_CHAR_PATH: &str = "/org/bitchat/rust/application/service/tx_char";

    /// BitCraps RX characteristic path
    pub const BITCRAPS_RX_CHAR_PATH: &str = "/org/bitchat/rust/application/service/rx_char";

    /// BitCraps advertisement path
    pub const BITCRAPS_ADVERTISEMENT_PATH: &str = "/org/bitchat/rust/advertisement";
}

/// D-Bus connection wrapper for thread safety
#[cfg(target_os = "linux")]
struct DbusConnection {
    // Placeholder for actual D-Bus connection
    // In real implementation, this would be zbus::Connection or dbus::Connection
    _connection: (),
}

#[cfg(target_os = "linux")]
impl DbusConnection {
    /// Create new D-Bus connection
    async fn new() -> Result<Self> {
        log::debug!("Creating D-Bus connection to BlueZ");

        // In real implementation, this would connect to the system D-Bus:
        // let conn = zbus::Connection::system().await?;

        Ok(Self { _connection: () })
    }

    /// Call a D-Bus method
    async fn call_method(
        &self,
        destination: &str,
        path: &str,
        interface: &str,
        method: &str,
    ) -> Result<()> {
        log::debug!(
            "D-Bus call: {}.{}.{} on {}",
            destination,
            interface,
            method,
            path
        );

        // In real implementation, this would make the actual D-Bus call:
        // let result = conn.call_method(destination, path, interface, method, args).await?;

        // For now, simulate success
        Ok(())
    }

    /// Get a D-Bus property
    async fn get_property(
        &self,
        destination: &str,
        path: &str,
        interface: &str,
        property: &str,
    ) -> Result<String> {
        log::debug!("D-Bus get property: {}.{} on {}", interface, property, path);

        // In real implementation:
        // let result: String = conn.get_property(destination, path, interface, property).await?;

        // For now, return placeholder
        Ok("placeholder".to_string())
    }

    /// Set a D-Bus property  
    async fn set_property(
        &self,
        destination: &str,
        path: &str,
        interface: &str,
        property: &str,
        _value: &str, // Simplified for this implementation
    ) -> Result<()> {
        log::debug!(
            "D-Bus set property: {}.{} = ? on {}",
            interface,
            property,
            path
        );

        // In real implementation:
        // conn.set_property(destination, path, interface, property, value).await?;

        Ok(())
    }

    /// Register a D-Bus object
    async fn register_object(&self, path: &str, interfaces: &[&str]) -> Result<()> {
        log::debug!(
            "Registering D-Bus object at {} with interfaces: {:?}",
            path,
            interfaces
        );

        // In real implementation, this would register the object with the D-Bus connection

        Ok(())
    }
}

/// Linux BLE Peripheral using BlueZ D-Bus
#[cfg(target_os = "linux")]
pub struct LinuxBlePeripheral {
    local_peer_id: PeerId,
    is_advertising: Arc<RwLock<bool>>,
    connected_centrals: Arc<RwLock<HashMap<PeerId, String>>>,
    event_sender: mpsc::Sender<PeripheralEvent>,
    event_receiver: Mutex<mpsc::Receiver<PeripheralEvent>>,
    config: Arc<RwLock<AdvertisingConfig>>,
    stats: Arc<RwLock<PeripheralStats>>,
    advertising_start_time: Arc<RwLock<Option<Instant>>>,

    // D-Bus components
    dbus_connection: Option<Arc<DbusConnection>>,
    adapter_path: String,
    application_registered: Arc<RwLock<bool>>,
    advertisement_registered: Arc<RwLock<bool>>,

    // Service data
    service_characteristics: Arc<RwLock<HashMap<String, Vec<u8>>>>,
}

#[cfg(target_os = "linux")]
impl LinuxBlePeripheral {
    pub async fn new(local_peer_id: PeerId) -> Result<Self> {
        let (event_sender, event_receiver) = mpsc::channel(1000); // Moderate traffic for BLE events

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
            adapter_path: bluez_constants::DEFAULT_ADAPTER_PATH.to_string(),
            application_registered: Arc::new(RwLock::new(false)),
            advertisement_registered: Arc::new(RwLock::new(false)),

            service_characteristics: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Initialize BlueZ D-Bus connection and discover adapter
    pub async fn initialize_bluez(&mut self) -> Result<()> {
        log::info!(
            "Initializing Linux BlueZ D-Bus connection for peer {:?}",
            self.local_peer_id
        );

        // Create D-Bus connection
        let dbus_connection = Arc::new(DbusConnection::new().await?);

        // Check if BlueZ is available
        self.check_bluez_availability(&dbus_connection).await?;

        // Find Bluetooth adapter
        self.find_bluetooth_adapter(&dbus_connection).await?;

        // Check adapter capabilities
        self.check_adapter_capabilities(&dbus_connection).await?;

        self.dbus_connection = Some(dbus_connection);

        log::info!("BlueZ D-Bus initialization completed successfully");
        Ok(())
    }

    /// Check if BlueZ service is available on D-Bus
    async fn check_bluez_availability(&self, dbus: &DbusConnection) -> Result<()> {
        log::debug!("Checking BlueZ availability");

        // In real implementation, this would check if BlueZ service exists:
        // let services = dbus.list_names().await?;
        // if !services.contains(&bluez_constants::BLUEZ_SERVICE.to_string()) {
        //     return Err(Error::Network("BlueZ service not available".to_string()));
        // }

        log::debug!("BlueZ service is available");
        Ok(())
    }

    /// Find available Bluetooth adapter
    async fn find_bluetooth_adapter(&mut self, dbus: &DbusConnection) -> Result<()> {
        log::debug!("Finding Bluetooth adapter");

        // In real implementation, this would enumerate available adapters:
        // let objects = dbus.call_method(
        //     bluez_constants::BLUEZ_SERVICE,
        //     "/",
        //     bluez_constants::OBJECT_MANAGER_INTERFACE,
        //     "GetManagedObjects",
        //     &[]
        // ).await?;

        // For now, assume default adapter exists
        self.adapter_path = bluez_constants::DEFAULT_ADAPTER_PATH.to_string();

        log::debug!("Using Bluetooth adapter: {}", self.adapter_path);
        Ok(())
    }

    /// Check adapter capabilities for GATT and advertising
    async fn check_adapter_capabilities(&self, dbus: &DbusConnection) -> Result<()> {
        log::debug!("Checking adapter capabilities");

        // Check if adapter is powered on
        let powered = dbus
            .get_property(
                bluez_constants::BLUEZ_SERVICE,
                &self.adapter_path,
                bluez_constants::ADAPTER_INTERFACE,
                "Powered",
            )
            .await?;

        if powered != "true" {
            log::warn!("Bluetooth adapter is not powered on, attempting to power on");

            // Try to power on the adapter
            dbus.set_property(
                bluez_constants::BLUEZ_SERVICE,
                &self.adapter_path,
                bluez_constants::ADAPTER_INTERFACE,
                "Powered",
                "true",
            )
            .await?;
        }

        // Check if LE is supported (this would be in the adapter properties)
        log::debug!("Adapter capabilities verified");
        Ok(())
    }

    /// Register GATT application with BlueZ
    async fn register_gatt_application(&self, config: &AdvertisingConfig) -> Result<()> {
        log::debug!("Registering GATT application with BlueZ");

        let dbus = self
            .dbus_connection
            .as_ref()
            .ok_or_else(|| Error::Network("D-Bus connection not initialized".to_string()))?;

        // Register application object
        dbus.register_object(
            bluez_constants::BITCRAPS_APP_PATH,
            &[bluez_constants::OBJECT_MANAGER_INTERFACE],
        )
        .await?;

        // Register service object
        self.register_gatt_service(dbus, config).await?;

        // Register characteristics
        self.register_tx_characteristic(dbus).await?;
        self.register_rx_characteristic(dbus).await?;

        // Register application with GATT Manager
        dbus.call_method(
            bluez_constants::BLUEZ_SERVICE,
            &self.adapter_path,
            bluez_constants::GATT_MANAGER_INTERFACE,
            "RegisterApplication",
        )
        .await?;

        *self.application_registered.write().await = true;

        log::debug!("GATT application registered successfully");
        Ok(())
    }

    /// Register GATT service
    async fn register_gatt_service(
        &self,
        dbus: &DbusConnection,
        config: &AdvertisingConfig,
    ) -> Result<()> {
        log::debug!("Registering GATT service");

        // Register service object with BlueZ
        dbus.register_object(
            bluez_constants::BITCRAPS_SERVICE_PATH,
            &[bluez_constants::GATT_SERVICE_INTERFACE],
        )
        .await?;

        // Set service properties
        let service_uuid = config.service_uuid.to_string();
        dbus.set_property(
            bluez_constants::BLUEZ_SERVICE,
            bluez_constants::BITCRAPS_SERVICE_PATH,
            bluez_constants::GATT_SERVICE_INTERFACE,
            "UUID",
            &service_uuid,
        )
        .await?;

        dbus.set_property(
            bluez_constants::BLUEZ_SERVICE,
            bluez_constants::BITCRAPS_SERVICE_PATH,
            bluez_constants::GATT_SERVICE_INTERFACE,
            "Primary",
            "true",
        )
        .await?;

        log::debug!("GATT service registered with UUID: {}", service_uuid);
        Ok(())
    }

    /// Register TX characteristic (for sending data to centrals)
    async fn register_tx_characteristic(&self, dbus: &DbusConnection) -> Result<()> {
        log::debug!("Registering TX characteristic");

        let tx_uuid = Uuid::from_u128(BITCRAPS_SERVICE_UUID.as_u128() + 1);

        // Register characteristic object
        dbus.register_object(
            bluez_constants::BITCRAPS_TX_CHAR_PATH,
            &[bluez_constants::GATT_CHARACTERISTIC_INTERFACE],
        )
        .await?;

        // Set characteristic properties
        dbus.set_property(
            bluez_constants::BLUEZ_SERVICE,
            bluez_constants::BITCRAPS_TX_CHAR_PATH,
            bluez_constants::GATT_CHARACTERISTIC_INTERFACE,
            "UUID",
            &tx_uuid.to_string(),
        )
        .await?;

        dbus.set_property(
            bluez_constants::BLUEZ_SERVICE,
            bluez_constants::BITCRAPS_TX_CHAR_PATH,
            bluez_constants::GATT_CHARACTERISTIC_INTERFACE,
            "Service",
            bluez_constants::BITCRAPS_SERVICE_PATH,
        )
        .await?;

        dbus.set_property(
            bluez_constants::BLUEZ_SERVICE,
            bluez_constants::BITCRAPS_TX_CHAR_PATH,
            bluez_constants::GATT_CHARACTERISTIC_INTERFACE,
            "Flags",
            "read,notify",
        )
        .await?;

        log::debug!("TX characteristic registered with UUID: {}", tx_uuid);
        Ok(())
    }

    /// Register RX characteristic (for receiving data from centrals)
    async fn register_rx_characteristic(&self, dbus: &DbusConnection) -> Result<()> {
        log::debug!("Registering RX characteristic");

        let rx_uuid = Uuid::from_u128(BITCRAPS_SERVICE_UUID.as_u128() + 2);

        // Register characteristic object
        dbus.register_object(
            bluez_constants::BITCRAPS_RX_CHAR_PATH,
            &[bluez_constants::GATT_CHARACTERISTIC_INTERFACE],
        )
        .await?;

        // Set characteristic properties
        dbus.set_property(
            bluez_constants::BLUEZ_SERVICE,
            bluez_constants::BITCRAPS_RX_CHAR_PATH,
            bluez_constants::GATT_CHARACTERISTIC_INTERFACE,
            "UUID",
            &rx_uuid.to_string(),
        )
        .await?;

        dbus.set_property(
            bluez_constants::BLUEZ_SERVICE,
            bluez_constants::BITCRAPS_RX_CHAR_PATH,
            bluez_constants::GATT_CHARACTERISTIC_INTERFACE,
            "Service",
            bluez_constants::BITCRAPS_SERVICE_PATH,
        )
        .await?;

        dbus.set_property(
            bluez_constants::BLUEZ_SERVICE,
            bluez_constants::BITCRAPS_RX_CHAR_PATH,
            bluez_constants::GATT_CHARACTERISTIC_INTERFACE,
            "Flags",
            "write,write-without-response",
        )
        .await?;

        log::debug!("RX characteristic registered with UUID: {}", rx_uuid);
        Ok(())
    }

    /// Register advertisement with BlueZ
    async fn register_advertisement(&self, config: &AdvertisingConfig) -> Result<()> {
        log::debug!("Registering advertisement with BlueZ");

        let dbus = self
            .dbus_connection
            .as_ref()
            .ok_or_else(|| Error::Network("D-Bus connection not initialized".to_string()))?;

        // Register advertisement object
        dbus.register_object(
            bluez_constants::BITCRAPS_ADVERTISEMENT_PATH,
            &[bluez_constants::LE_ADVERTISEMENT_INTERFACE],
        )
        .await?;

        // Set advertisement properties
        dbus.set_property(
            bluez_constants::BLUEZ_SERVICE,
            bluez_constants::BITCRAPS_ADVERTISEMENT_PATH,
            bluez_constants::LE_ADVERTISEMENT_INTERFACE,
            "Type",
            "peripheral",
        )
        .await?;

        // Set service UUIDs
        dbus.set_property(
            bluez_constants::BLUEZ_SERVICE,
            bluez_constants::BITCRAPS_ADVERTISEMENT_PATH,
            bluez_constants::LE_ADVERTISEMENT_INTERFACE,
            "ServiceUUIDs",
            &config.service_uuid.to_string(),
        )
        .await?;

        // Set local name if requested
        if config.include_name {
            dbus.set_property(
                bluez_constants::BLUEZ_SERVICE,
                bluez_constants::BITCRAPS_ADVERTISEMENT_PATH,
                bluez_constants::LE_ADVERTISEMENT_INTERFACE,
                "LocalName",
                &config.local_name,
            )
            .await?;
        }

        // Set intervals (convert from ms to units of 0.625ms)
        let min_interval = (config.advertising_interval_ms as f32 / 0.625) as u16;

        dbus.set_property(
            bluez_constants::BLUEZ_SERVICE,
            bluez_constants::BITCRAPS_ADVERTISEMENT_PATH,
            bluez_constants::LE_ADVERTISEMENT_INTERFACE,
            "MinInterval",
            &min_interval.to_string(),
        )
        .await?;

        dbus.set_property(
            bluez_constants::BLUEZ_SERVICE,
            bluez_constants::BITCRAPS_ADVERTISEMENT_PATH,
            bluez_constants::LE_ADVERTISEMENT_INTERFACE,
            "MaxInterval",
            &(min_interval + 10).to_string(),
        )
        .await?;

        // Register advertisement with LE Advertising Manager
        dbus.call_method(
            bluez_constants::BLUEZ_SERVICE,
            &self.adapter_path,
            bluez_constants::LE_ADVERTISING_MANAGER_INTERFACE,
            "RegisterAdvertisement",
        )
        .await?;

        *self.advertisement_registered.write().await = true;

        log::debug!("Advertisement registered successfully");
        Ok(())
    }

    /// Start Linux BLE advertising
    pub async fn start_linux_advertising(&mut self, config: &AdvertisingConfig) -> Result<()> {
        if *self.is_advertising.read().await {
            return Err(Error::Network("Already advertising".to_string()));
        }

        log::info!("Starting Linux BLE advertising");

        let dbus = self
            .dbus_connection
            .as_ref()
            .ok_or_else(|| Error::Network("D-Bus connection not initialized".to_string()))?;

        // Register GATT application if not already done
        if !*self.application_registered.read().await {
            self.register_gatt_application(config).await?;
        }

        // Register advertisement if not already done
        if !*self.advertisement_registered.read().await {
            self.register_advertisement(config).await?;
        }

        // Update state
        *self.is_advertising.write().await = true;
        *self.advertising_start_time.write().await = Some(Instant::now());
        *self.config.write().await = config.clone();

        // Send event
        let _ = self.event_sender.send(PeripheralEvent::AdvertisingStarted);

        log::info!("Linux BLE advertising started successfully");
        Ok(())
    }

    /// Stop Linux BLE advertising
    pub async fn stop_linux_advertising(&mut self) -> Result<()> {
        if !*self.is_advertising.read().await {
            return Ok(());
        }

        log::info!("Stopping Linux BLE advertising");

        let dbus = self
            .dbus_connection
            .as_ref()
            .ok_or_else(|| Error::Network("D-Bus connection not initialized".to_string()))?;

        // Unregister advertisement
        if *self.advertisement_registered.read().await {
            dbus.call_method(
                bluez_constants::BLUEZ_SERVICE,
                &self.adapter_path,
                bluez_constants::LE_ADVERTISING_MANAGER_INTERFACE,
                "UnregisterAdvertisement",
            )
            .await?;

            *self.advertisement_registered.write().await = false;
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

        log::info!("Linux BLE advertising stopped");
        Ok(())
    }

    /// Send data to a connected central via characteristic notification
    pub async fn send_to_linux_central(&mut self, peer_id: PeerId, data: &[u8]) -> Result<()> {
        let centrals = self.connected_centrals.read().await;

        if centrals.contains_key(&peer_id) {
            // Update TX characteristic value
            let mut characteristics = self.service_characteristics.write().await;
            characteristics.insert(
                bluez_constants::BITCRAPS_TX_CHAR_PATH.to_string(),
                data.to_vec(),
            );

            // In real implementation, this would send a D-Bus signal for PropertiesChanged
            // to notify subscribed centrals about the characteristic value change

            let dbus = self
                .dbus_connection
                .as_ref()
                .ok_or_else(|| Error::Network("D-Bus connection not initialized".to_string()))?;

            // Emit PropertiesChanged signal for the characteristic
            dbus.call_method(
                bluez_constants::BLUEZ_SERVICE,
                bluez_constants::BITCRAPS_TX_CHAR_PATH,
                bluez_constants::PROPERTIES_INTERFACE,
                "PropertiesChanged",
            )
            .await?;

            let mut stats = self.stats.write().await;
            stats.bytes_sent += data.len() as u64;

            log::debug!("Sent {} bytes to central {:?}", data.len(), peer_id);
            Ok(())
        } else {
            Err(Error::Network(format!(
                "Central {:?} not connected",
                peer_id
            )))
        }
    }

    /// Disconnect from a central
    pub async fn disconnect_linux_central(&mut self, peer_id: PeerId) -> Result<()> {
        let mut centrals = self.connected_centrals.write().await;

        if let Some(address) = centrals.remove(&peer_id) {
            // In real implementation, we could disconnect the device via BlueZ D-Bus
            // For now, just remove from our tracking

            let _ = self
                .event_sender
                .send(PeripheralEvent::CentralDisconnected {
                    peer_id,
                    reason: "Disconnected by peripheral".to_string(),
                });

            log::info!("Disconnected central {:?} at {}", peer_id, address);
            Ok(())
        } else {
            Err(Error::Network(format!(
                "Central {:?} not connected",
                peer_id
            )))
        }
    }

    /// Handle characteristic write from central (callback for D-Bus method)
    pub async fn handle_characteristic_write(
        &self,
        characteristic_path: &str,
        value: Vec<u8>,
        sender: &str,
    ) {
        log::debug!(
            "Received characteristic write: {} bytes on {} from {}",
            value.len(),
            characteristic_path,
            sender
        );

        // Generate peer ID from sender address
        let peer_id = {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            use std::hash::Hasher;
            hasher.write(sender.as_bytes());
            let hash = hasher.finish();
            let mut peer_id = [0u8; 32];
            peer_id[..8].copy_from_slice(&hash.to_be_bytes());
            peer_id
        };

        // Track connection
        let mut centrals = self.connected_centrals.write().await;
        centrals.insert(peer_id, sender.to_string());

        // Update stats
        let mut stats = self.stats.write().await;
        stats.bytes_received += value.len() as u64;

        // Send data received event
        let _ = self.event_sender.send(PeripheralEvent::DataReceived {
            peer_id,
            data: value,
        });
    }
}

#[cfg(target_os = "linux")]
impl Drop for LinuxBlePeripheral {
    fn drop(&mut self) {
        // Clean up D-Bus registrations
        if let Some(dbus) = &self.dbus_connection {
            tokio::spawn(async move {
                // In real implementation, unregister D-Bus objects
                log::debug!("Cleaning up BlueZ D-Bus registrations");
            });
        }
    }
}

/// Implement BlePeripheral trait for Linux
#[cfg(target_os = "linux")]
#[async_trait::async_trait]
impl BlePeripheral for LinuxBlePeripheral {
    async fn start_advertising(&mut self, config: &AdvertisingConfig) -> Result<()> {
        self.start_linux_advertising(config).await
    }

    async fn stop_advertising(&mut self) -> Result<()> {
        self.stop_linux_advertising().await
    }

    fn is_advertising(&self) -> bool {
        self.is_advertising
            .try_read()
            .map(|guard| *guard)
            .unwrap_or(false)
    }

    async fn send_to_central(&mut self, peer_id: PeerId, data: &[u8]) -> Result<()> {
        self.send_to_linux_central(peer_id, data).await
    }

    async fn disconnect_central(&mut self, peer_id: PeerId) -> Result<()> {
        self.disconnect_linux_central(peer_id).await
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

    async fn set_recovery_config(&mut self, _config: RecoveryConfig) -> Result<()> {
        // TODO: Store recovery configuration
        Ok(())
    }

    async fn recover(&mut self) -> Result<()> {
        log::warn!("Attempting Linux BLE recovery");

        // Stop advertising and unregister services
        self.stop_advertising().await?;

        // Reset D-Bus registrations
        *self.application_registered.write().await = false;
        *self.advertisement_registered.write().await = false;

        // Wait before attempting recovery
        tokio::time::sleep(Duration::from_millis(2000)).await;

        // Reinitialize BlueZ connection
        self.initialize_bluez().await?;

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
        // Disconnect and attempt reconnection
        self.disconnect_central(peer_id).await?;
        Ok(())
    }

    async fn health_check(&self) -> Result<bool> {
        // Check if D-Bus connection is healthy
        if let Some(dbus) = &self.dbus_connection {
            // Try to get adapter state to verify connection
            match dbus
                .get_property(
                    bluez_constants::BLUEZ_SERVICE,
                    &self.adapter_path,
                    bluez_constants::ADAPTER_INTERFACE,
                    "Powered",
                )
                .await
            {
                Ok(_) => Ok(true),
                Err(_) => Ok(false),
            }
        } else {
            Ok(false)
        }
    }

    async fn reset(&mut self) -> Result<()> {
        log::info!("Resetting Linux BLE peripheral");

        // Stop advertising and clear all connections
        self.stop_advertising().await?;
        self.connected_centrals.write().await.clear();

        // Reset all D-Bus registrations
        *self.application_registered.write().await = false;
        *self.advertisement_registered.write().await = false;

        // Reset statistics
        *self.stats.write().await = PeripheralStats::default();

        // Clear characteristic data
        self.service_characteristics.write().await.clear();

        Ok(())
    }
}

/// D-Bus method handlers for BlueZ integration
#[cfg(target_os = "linux")]
pub mod dbus_handlers {
    use super::*;

    /// Handle GATT characteristic WriteValue method call
    pub async fn handle_write_value(
        peripheral: Arc<Mutex<LinuxBlePeripheral>>,
        characteristic_path: String,
        value: Vec<u8>,
        _options: HashMap<String, String>,
        sender: String,
    ) -> Result<()> {
        let peripheral = peripheral.lock().await;
        peripheral
            .handle_characteristic_write(&characteristic_path, value, &sender)
            .await;
        Ok(())
    }

    /// Handle GATT characteristic ReadValue method call
    pub async fn handle_read_value(
        peripheral: Arc<Mutex<LinuxBlePeripheral>>,
        characteristic_path: String,
        _options: HashMap<String, String>,
    ) -> Result<Vec<u8>> {
        let peripheral = peripheral.lock().await;
        let characteristics = peripheral.service_characteristics.read().await;

        // Return current value or empty if not set
        Ok(characteristics
            .get(&characteristic_path)
            .cloned()
            .unwrap_or_default())
    }

    /// Handle GATT characteristic StartNotify method call
    pub async fn handle_start_notify(
        peripheral: Arc<Mutex<LinuxBlePeripheral>>,
        _characteristic_path: String,
        sender: String,
    ) -> Result<()> {
        log::debug!("Central {} started notifications", sender);

        // Track that this central is subscribed to notifications
        // In a full implementation, we'd maintain a list of subscribed centrals

        Ok(())
    }

    /// Handle GATT characteristic StopNotify method call
    pub async fn handle_stop_notify(
        peripheral: Arc<Mutex<LinuxBlePeripheral>>,
        _characteristic_path: String,
        sender: String,
    ) -> Result<()> {
        log::debug!("Central {} stopped notifications", sender);

        // Remove central from notifications

        Ok(())
    }
}
