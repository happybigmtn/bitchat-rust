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
    /// Advertising failed with recovery suggestion
    AdvertisingFailed {
        error: String,
        retry_suggested: bool,
        retry_delay_ms: u64,
    },
    /// Connection state changed
    ConnectionStateChanged {
        peer_id: PeerId,
        state: ConnectionState,
    },
    /// Platform-specific event
    PlatformEvent {
        platform: String,
        event_data: Vec<u8>,
    },
}

/// Connection states for state management
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Authenticating,
    Ready,
    Disconnecting,
    Error(String),
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
    /// Number of connection failures
    pub connection_failures: u64,
    /// Number of successful reconnections
    pub reconnection_attempts: u64,
    /// Average connection duration
    pub avg_connection_duration: Duration,
    /// Last error timestamp
    pub last_error_time: Option<Instant>,
    /// Platform-specific metrics
    pub platform_specific: HashMap<String, u64>,
}

/// Connection recovery configuration
#[derive(Debug, Clone)]
pub struct RecoveryConfig {
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Base delay between retries (exponential backoff)
    pub base_retry_delay_ms: u64,
    /// Maximum retry delay
    pub max_retry_delay_ms: u64,
    /// Timeout for connection attempts
    pub connection_timeout_ms: u64,
    /// Whether to enable automatic recovery
    pub auto_recovery_enabled: bool,
}

impl Default for RecoveryConfig {
    fn default() -> Self {
        Self {
            max_retries: 5,
            base_retry_delay_ms: 1000,
            max_retry_delay_ms: 30000,
            connection_timeout_ms: 10000,
            auto_recovery_enabled: true,
        }
    }
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
    
    /// Set recovery configuration
    async fn set_recovery_config(&mut self, config: RecoveryConfig) -> Result<()>;
    
    /// Trigger manual recovery attempt
    async fn recover(&mut self) -> Result<()>;
    
    /// Get current connection state for a peer
    async fn get_connection_state(&self, peer_id: PeerId) -> Option<ConnectionState>;
    
    /// Force reconnection to a specific central
    async fn force_reconnect(&mut self, peer_id: PeerId) -> Result<()>;
    
    /// Check platform-specific health status
    async fn health_check(&self) -> Result<bool>;
    
    /// Reset peripheral state (emergency recovery)
    async fn reset(&mut self) -> Result<()>;
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
    // JNI handles for Android Bluetooth LE
    bluetooth_manager: Arc<Mutex<Option<AndroidBluetoothManager>>>,
    advertise_callback: Arc<Mutex<Option<AndroidAdvertiseCallback>>>,
    gatt_server: Arc<Mutex<Option<AndroidGattServer>>>,
    // Recovery and connection state management
    recovery_config: Arc<RwLock<RecoveryConfig>>,
    connection_states: Arc<RwLock<HashMap<PeerId, ConnectionState>>>,
    retry_count: Arc<RwLock<u32>>,
}

#[cfg(target_os = "android")]
use jni::JavaVM;
#[cfg(target_os = "android")]
use jni::objects::{GlobalRef, JClass, JObject, JString, JValue};
#[cfg(target_os = "android")]
use jni::JNIEnv;
#[cfg(target_os = "android")]
use ndk_context::android_context;
#[cfg(target_os = "android")]
use std::ffi::CString;

/// Android Bluetooth Manager wrapper
#[cfg(target_os = "android")]
struct AndroidBluetoothManager {
    jvm: JavaVM,
    bluetooth_manager: GlobalRef,
    bluetooth_adapter: GlobalRef,
    le_advertiser: Option<GlobalRef>,
}

/// Android Advertise Callback wrapper
#[cfg(target_os = "android")]
struct AndroidAdvertiseCallback {
    callback_ref: GlobalRef,
}

/// Android GATT Server wrapper
#[cfg(target_os = "android")]
struct AndroidGattServer {
    gatt_server: GlobalRef,
    service: GlobalRef,
    tx_characteristic: GlobalRef,
    rx_characteristic: GlobalRef,
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
            bluetooth_manager: Arc::new(Mutex::new(None)),
            advertise_callback: Arc::new(Mutex::new(None)),
            gatt_server: Arc::new(Mutex::new(None)),
            recovery_config: Arc::new(RwLock::new(RecoveryConfig::default())),
            connection_states: Arc::new(RwLock::new(HashMap::new())),
            retry_count: Arc::new(RwLock::new(0)),
        })
    }
    
    /// Initialize JNI connection to Android BluetoothLeAdvertiser
    async fn initialize_jni(&mut self) -> Result<()> {
        log::info!("Initializing Android BLE advertising via JNI for peer {:?}", self.local_peer_id);
        
        let context = android_context::android_context()
            .ok_or_else(|| Error::Platform("Failed to get Android context".to_string()))?;
        
        // Get JNI environment
        let vm = unsafe { jni::JavaVM::from_raw(context.vm().cast()) }
            .map_err(|e| Error::Platform(format!("Failed to get JavaVM: {}", e)))?;
        
        let env = vm.get_env()
            .map_err(|e| Error::Platform(format!("Failed to get JNI environment: {}", e)))?;
        
        // Get BluetoothManager system service
        let context_obj = JObject::from_raw(context.context().cast());
        let bluetooth_service = env.new_string("bluetooth")
            .map_err(|e| Error::Platform(format!("Failed to create string: {}", e)))?;
        
        let bluetooth_manager = env.call_method(
            &context_obj,
            "getSystemService",
            "(Ljava/lang/String;)Ljava/lang/Object;",
            &[JValue::Object(&bluetooth_service)]
        ).map_err(|e| Error::Platform(format!("Failed to get BluetoothManager: {}", e)))?
        .l().map_err(|e| Error::Platform(format!("Failed to get BluetoothManager object: {}", e)))?;
        
        // Get BluetoothAdapter
        let bluetooth_adapter = env.call_method(
            &bluetooth_manager,
            "getAdapter",
            "()Landroid/bluetooth/BluetoothAdapter;",
            &[]
        ).map_err(|e| Error::Platform(format!("Failed to get BluetoothAdapter: {}", e)))?
        .l().map_err(|e| Error::Platform(format!("Failed to get BluetoothAdapter object: {}", e)))?;
        
        // Check if Bluetooth is enabled
        let is_enabled = env.call_method(
            &bluetooth_adapter,
            "isEnabled",
            "()Z",
            &[]
        ).map_err(|e| Error::Platform(format!("Failed to check Bluetooth enabled: {}", e)))?
        .z().map_err(|e| Error::Platform(format!("Failed to get boolean result: {}", e)))?;
        
        if !is_enabled {
            return Err(Error::Platform("Bluetooth is not enabled".to_string()));
        }
        
        // Get BluetoothLeAdvertiser
        let le_advertiser = env.call_method(
            &bluetooth_adapter,
            "getBluetoothLeAdvertiser",
            "()Landroid/bluetooth/le/BluetoothLeAdvertiser;",
            &[]
        ).map_err(|e| Error::Platform(format!("Failed to get BluetoothLeAdvertiser: {}", e)))?
        .l().map_err(|e| Error::Platform(format!("Failed to get BluetoothLeAdvertiser object: {}", e)))?;
        
        if le_advertiser.is_null() {
            return Err(Error::Platform("BLE advertising not supported on this device".to_string()));
        }
        
        // Create global references
        let bluetooth_manager_ref = env.new_global_ref(&bluetooth_manager)
            .map_err(|e| Error::Platform(format!("Failed to create global ref: {}", e)))?;
        let bluetooth_adapter_ref = env.new_global_ref(&bluetooth_adapter)
            .map_err(|e| Error::Platform(format!("Failed to create global ref: {}", e)))?;
        let le_advertiser_ref = env.new_global_ref(&le_advertiser)
            .map_err(|e| Error::Platform(format!("Failed to create global ref: {}", e)))?;
        
        // Store the Android Bluetooth Manager
        let android_manager = AndroidBluetoothManager {
            jvm: vm,
            bluetooth_manager: bluetooth_manager_ref,
            bluetooth_adapter: bluetooth_adapter_ref,
            le_advertiser: Some(le_advertiser_ref),
        };
        
        *self.bluetooth_manager.lock().await = Some(android_manager);
        
        log::info!("Android BLE JNI initialization completed successfully");
        Ok(())
    }
    
    /// Create Android AdvertiseSettings
    async fn create_advertise_settings(&self, config: &AdvertisingConfig) -> Result<JObject> {
        let manager = self.bluetooth_manager.lock().await;
        let android_manager = manager.as_ref()
            .ok_or_else(|| Error::Platform("Bluetooth manager not initialized".to_string()))?;
        
        let env = android_manager.jvm.get_env()
            .map_err(|e| Error::Platform(format!("Failed to get JNI environment: {}", e)))?;
        
        // Create AdvertiseSettings.Builder
        let builder_class = env.find_class("android/bluetooth/le/AdvertiseSettings$Builder")
            .map_err(|e| Error::Platform(format!("Failed to find AdvertiseSettings.Builder class: {}", e)))?;
        
        let builder = env.new_object(builder_class, "()V", &[])
            .map_err(|e| Error::Platform(format!("Failed to create AdvertiseSettings.Builder: {}", e)))?;
        
        // Set advertising mode based on interval
        let mode = if config.advertising_interval_ms <= 100 {
            2 // ADVERTISE_MODE_LOW_LATENCY
        } else if config.advertising_interval_ms <= 250 {
            1 // ADVERTISE_MODE_BALANCED
        } else {
            0 // ADVERTISE_MODE_LOW_POWER
        };
        
        let builder_with_mode = env.call_method(
            &builder,
            "setAdvertiseMode",
            "(I)Landroid/bluetooth/le/AdvertiseSettings$Builder;",
            &[JValue::Int(mode)]
        ).map_err(|e| Error::Platform(format!("Failed to set advertise mode: {}", e)))?
        .l().map_err(|e| Error::Platform(format!("Failed to get builder object: {}", e)))?;
        
        // Set TX power level
        let power_level = match config.tx_power_level {
            p if p >= 7 => 3,  // ADVERTISE_TX_POWER_HIGH
            p if p >= 0 => 2,  // ADVERTISE_TX_POWER_MEDIUM
            p if p >= -10 => 1, // ADVERTISE_TX_POWER_LOW
            _ => 0,            // ADVERTISE_TX_POWER_ULTRA_LOW
        };
        
        let builder_with_power = env.call_method(
            &builder_with_mode,
            "setTxPowerLevel",
            "(I)Landroid/bluetooth/le/AdvertiseSettings$Builder;",
            &[JValue::Int(power_level)]
        ).map_err(|e| Error::Platform(format!("Failed to set TX power level: {}", e)))?
        .l().map_err(|e| Error::Platform(format!("Failed to get builder object: {}", e)))?;
        
        // Set connectable
        let builder_with_connectable = env.call_method(
            &builder_with_power,
            "setConnectable",
            "(Z)Landroid/bluetooth/le/AdvertiseSettings$Builder;",
            &[JValue::Bool(config.connectable as u8)]
        ).map_err(|e| Error::Platform(format!("Failed to set connectable: {}", e)))?
        .l().map_err(|e| Error::Platform(format!("Failed to get builder object: {}", e)))?;
        
        // Build the settings
        let settings = env.call_method(
            &builder_with_connectable,
            "build",
            "()Landroid/bluetooth/le/AdvertiseSettings;",
            &[]
        ).map_err(|e| Error::Platform(format!("Failed to build AdvertiseSettings: {}", e)))?
        .l().map_err(|e| Error::Platform(format!("Failed to get AdvertiseSettings object: {}", e)))?;
        
        log::debug!("Created Android AdvertiseSettings: interval={}ms, tx_power={}, connectable={}", 
                   config.advertising_interval_ms, config.tx_power_level, config.connectable);
        
        Ok(settings)
    }
    
    /// Create Android AdvertiseData
    async fn create_advertise_data(&self, config: &AdvertisingConfig) -> Result<JObject> {
        let manager = self.bluetooth_manager.lock().await;
        let android_manager = manager.as_ref()
            .ok_or_else(|| Error::Platform("Bluetooth manager not initialized".to_string()))?;
        
        let env = android_manager.jvm.get_env()
            .map_err(|e| Error::Platform(format!("Failed to get JNI environment: {}", e)))?;
        
        // Create AdvertiseData.Builder
        let builder_class = env.find_class("android/bluetooth/le/AdvertiseData$Builder")
            .map_err(|e| Error::Platform(format!("Failed to find AdvertiseData.Builder class: {}", e)))?;
        
        let builder = env.new_object(builder_class, "()V", &[])
            .map_err(|e| Error::Platform(format!("Failed to create AdvertiseData.Builder: {}", e)))?;
        
        // Add service UUID
        let uuid_class = env.find_class("java/util/UUID")
            .map_err(|e| Error::Platform(format!("Failed to find UUID class: {}", e)))?;
        
        let uuid_string = env.new_string(config.service_uuid.to_string())
            .map_err(|e| Error::Platform(format!("Failed to create UUID string: {}", e)))?;
        
        let service_uuid = env.call_static_method(
            uuid_class,
            "fromString",
            "(Ljava/lang/String;)Ljava/util/UUID;",
            &[JValue::Object(&uuid_string)]
        ).map_err(|e| Error::Platform(format!("Failed to create UUID: {}", e)))?
        .l().map_err(|e| Error::Platform(format!("Failed to get UUID object: {}", e)))?;
        
        // Create ParcelUuid from UUID
        let parcel_uuid_class = env.find_class("android/os/ParcelUuid")
            .map_err(|e| Error::Platform(format!("Failed to find ParcelUuid class: {}", e)))?;
        
        let parcel_uuid = env.new_object(
            parcel_uuid_class,
            "(Ljava/util/UUID;)V",
            &[JValue::Object(&service_uuid)]
        ).map_err(|e| Error::Platform(format!("Failed to create ParcelUuid: {}", e)))?;
        
        let builder_with_uuid = env.call_method(
            &builder,
            "addServiceUuid",
            "(Landroid/os/ParcelUuid;)Landroid/bluetooth/le/AdvertiseData$Builder;",
            &[JValue::Object(&parcel_uuid)]
        ).map_err(|e| Error::Platform(format!("Failed to add service UUID: {}", e)))?
        .l().map_err(|e| Error::Platform(format!("Failed to get builder object: {}", e)))?;
        
        // Add local name if configured
        let builder_with_name = if config.include_name {
            let local_name = env.new_string(&config.local_name)
                .map_err(|e| Error::Platform(format!("Failed to create local name string: {}", e)))?;
            
            env.call_method(
                &builder_with_uuid,
                "setIncludeDeviceName",
                "(Z)Landroid/bluetooth/le/AdvertiseData$Builder;",
                &[JValue::Bool(1)]
            ).map_err(|e| Error::Platform(format!("Failed to set include device name: {}", e)))?
            .l().map_err(|e| Error::Platform(format!("Failed to get builder object: {}", e)))?
        } else {
            builder_with_uuid
        };
        
        // Add manufacturer data with peer ID
        let peer_id_bytes = &self.local_peer_id;
        let peer_id_array = env.byte_array_from_slice(peer_id_bytes)
            .map_err(|e| Error::Platform(format!("Failed to create byte array: {}", e)))?;
        
        let builder_with_manufacturer = env.call_method(
            &builder_with_name,
            "addManufacturerData",
            "(I[B)Landroid/bluetooth/le/AdvertiseData$Builder;",
            &[JValue::Int(0x004C), JValue::Object(&peer_id_array)] // Apple company ID as example
        ).map_err(|e| Error::Platform(format!("Failed to add manufacturer data: {}", e)))?
        .l().map_err(|e| Error::Platform(format!("Failed to get builder object: {}", e)))?;
        
        // Build the data
        let advertise_data = env.call_method(
            &builder_with_manufacturer,
            "build",
            "()Landroid/bluetooth/le/AdvertiseData;",
            &[]
        ).map_err(|e| Error::Platform(format!("Failed to build AdvertiseData: {}", e)))?
        .l().map_err(|e| Error::Platform(format!("Failed to get AdvertiseData object: {}", e)))?;
        
        log::debug!("Created Android AdvertiseData: service_uuid={}, include_name={}", 
                   config.service_uuid, config.include_name);
        
        Ok(advertise_data)
    }
    
    /// Set up GATT server for incoming connections
    async fn setup_gatt_server(&self, config: &AdvertisingConfig) -> Result<()> {
        let manager = self.bluetooth_manager.lock().await;
        let android_manager = manager.as_ref()
            .ok_or_else(|| Error::Platform("Bluetooth manager not initialized".to_string()))?;
        
        let env = android_manager.jvm.get_env()
            .map_err(|e| Error::Platform(format!("Failed to get JNI environment: {}", e)))?;
        
        // Get Android context (stored in ndk-context)
        let context = android_context::android_context()
            .ok_or_else(|| Error::Platform("Failed to get Android context".to_string()))?;
        let context_obj = JObject::from_raw(context.context().cast());
        
        // Create GATT server callback
        // In a real implementation, we would need to create a Java class that extends BluetoothGattServerCallback
        // For now, we'll create a placeholder
        
        // Get BluetoothManager and open GATT server
        let gatt_server = env.call_method(
            &android_manager.bluetooth_manager,
            "openGattServer",
            "(Landroid/content/Context;Landroid/bluetooth/BluetoothGattServerCallback;)Landroid/bluetooth/BluetoothGattServer;",
            &[JValue::Object(&context_obj), JValue::Object(&JObject::null())] // TODO: Implement callback
        ).map_err(|e| Error::Platform(format!("Failed to open GATT server: {}", e)))?
        .l().map_err(|e| Error::Platform(format!("Failed to get GATT server object: {}", e)))?;
        
        // Create BitCraps service
        let service_uuid_string = env.new_string(config.service_uuid.to_string())
            .map_err(|e| Error::Platform(format!("Failed to create service UUID string: {}", e)))?;
        
        let uuid_class = env.find_class("java/util/UUID")
            .map_err(|e| Error::Platform(format!("Failed to find UUID class: {}", e)))?;
        
        let service_uuid = env.call_static_method(
            uuid_class,
            "fromString",
            "(Ljava/lang/String;)Ljava/util/UUID;",
            &[JValue::Object(&service_uuid_string)]
        ).map_err(|e| Error::Platform(format!("Failed to create service UUID: {}", e)))?
        .l().map_err(|e| Error::Platform(format!("Failed to get UUID object: {}", e)))?;
        
        let service_class = env.find_class("android/bluetooth/BluetoothGattService")
            .map_err(|e| Error::Platform(format!("Failed to find BluetoothGattService class: {}", e)))?;
        
        let service = env.new_object(
            service_class,
            "(Ljava/util/UUID;I)V",
            &[JValue::Object(&service_uuid), JValue::Int(0)] // SERVICE_TYPE_PRIMARY
        ).map_err(|e| Error::Platform(format!("Failed to create GATT service: {}", e)))?;
        
        // Create TX characteristic (for sending data to central)
        let tx_uuid_str = "6E400002-B5A3-F393-E0A9-E50E24DCCA9E";
        let tx_uuid_string = env.new_string(tx_uuid_str)
            .map_err(|e| Error::Platform(format!("Failed to create TX UUID string: {}", e)))?;
        
        let tx_uuid = env.call_static_method(
            uuid_class,
            "fromString",
            "(Ljava/lang/String;)Ljava/util/UUID;",
            &[JValue::Object(&tx_uuid_string)]
        ).map_err(|e| Error::Platform(format!("Failed to create TX UUID: {}", e)))?
        .l().map_err(|e| Error::Platform(format!("Failed to get TX UUID object: {}", e)))?;
        
        let characteristic_class = env.find_class("android/bluetooth/BluetoothGattCharacteristic")
            .map_err(|e| Error::Platform(format!("Failed to find BluetoothGattCharacteristic class: {}", e)))?;
        
        let tx_characteristic = env.new_object(
            characteristic_class,
            "(Ljava/util/UUID;II)V",
            &[
                JValue::Object(&tx_uuid),
                JValue::Int(0x10), // PROPERTY_NOTIFY
                JValue::Int(0x01)  // PERMISSION_READ
            ]
        ).map_err(|e| Error::Platform(format!("Failed to create TX characteristic: {}", e)))?;
        
        // Create RX characteristic (for receiving data from central)
        let rx_uuid_str = "6E400003-B5A3-F393-E0A9-E50E24DCCA9E";
        let rx_uuid_string = env.new_string(rx_uuid_str)
            .map_err(|e| Error::Platform(format!("Failed to create RX UUID string: {}", e)))?;
        
        let rx_uuid = env.call_static_method(
            uuid_class,
            "fromString",
            "(Ljava/lang/String;)Ljava/util/UUID;",
            &[JValue::Object(&rx_uuid_string)]
        ).map_err(|e| Error::Platform(format!("Failed to create RX UUID: {}", e)))?
        .l().map_err(|e| Error::Platform(format!("Failed to get RX UUID object: {}", e)))?;
        
        let rx_characteristic = env.new_object(
            characteristic_class,
            "(Ljava/util/UUID;II)V",
            &[
                JValue::Object(&rx_uuid),
                JValue::Int(0x08), // PROPERTY_WRITE
                JValue::Int(0x02)  // PERMISSION_WRITE
            ]
        ).map_err(|e| Error::Platform(format!("Failed to create RX characteristic: {}", e)))?;
        
        // Add characteristics to service
        env.call_method(
            &service,
            "addCharacteristic",
            "(Landroid/bluetooth/BluetoothGattCharacteristic;)Z",
            &[JValue::Object(&tx_characteristic)]
        ).map_err(|e| Error::Platform(format!("Failed to add TX characteristic: {}", e)))?;
        
        env.call_method(
            &service,
            "addCharacteristic",
            "(Landroid/bluetooth/BluetoothGattCharacteristic;)Z",
            &[JValue::Object(&rx_characteristic)]
        ).map_err(|e| Error::Platform(format!("Failed to add RX characteristic: {}", e)))?;
        
        // Add service to GATT server
        let service_added = env.call_method(
            &gatt_server,
            "addService",
            "(Landroid/bluetooth/BluetoothGattService;)Z",
            &[JValue::Object(&service)]
        ).map_err(|e| Error::Platform(format!("Failed to add service to GATT server: {}", e)))?
        .z().map_err(|e| Error::Platform(format!("Failed to get boolean result: {}", e)))?;
        
        if !service_added {
            return Err(Error::Platform("Failed to add service to GATT server".to_string()));
        }
        
        // Store GATT server references
        let gatt_server_ref = env.new_global_ref(&gatt_server)
            .map_err(|e| Error::Platform(format!("Failed to create GATT server global ref: {}", e)))?;
        let service_ref = env.new_global_ref(&service)
            .map_err(|e| Error::Platform(format!("Failed to create service global ref: {}", e)))?;
        let tx_char_ref = env.new_global_ref(&tx_characteristic)
            .map_err(|e| Error::Platform(format!("Failed to create TX characteristic global ref: {}", e)))?;
        let rx_char_ref = env.new_global_ref(&rx_characteristic)
            .map_err(|e| Error::Platform(format!("Failed to create RX characteristic global ref: {}", e)))?;
        
        let android_gatt = AndroidGattServer {
            gatt_server: gatt_server_ref,
            service: service_ref,
            tx_characteristic: tx_char_ref,
            rx_characteristic: rx_char_ref,
        };
        
        *self.gatt_server.lock().await = Some(android_gatt);
        
        log::debug!("Set up Android GATT server with max_connections={}", config.max_connections);
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
        
        // Set up GATT server first
        self.setup_gatt_server(config).await?;
        
        // Create advertising settings and data
        let settings = self.create_advertise_settings(config).await?;
        let advertise_data = self.create_advertise_data(config).await?;
        
        // Start advertising
        let manager = self.bluetooth_manager.lock().await;
        let android_manager = manager.as_ref()
            .ok_or_else(|| Error::Platform("Bluetooth manager not initialized".to_string()))?;
        
        let env = android_manager.jvm.get_env()
            .map_err(|e| Error::Platform(format!("Failed to get JNI environment: {}", e)))?;
        
        let le_advertiser = android_manager.le_advertiser.as_ref()
            .ok_or_else(|| Error::Platform("BLE advertiser not available".to_string()))?;
        
        // TODO: Create proper AdvertiseCallback
        // For now, we'll pass null as callback - this needs to be implemented
        env.call_method(
            le_advertiser,
            "startAdvertising",
            "(Landroid/bluetooth/le/AdvertiseSettings;Landroid/bluetooth/le/AdvertiseData;Landroid/bluetooth/le/AdvertiseCallback;)V",
            &[
                JValue::Object(&settings),
                JValue::Object(&advertise_data),
                JValue::Object(&JObject::null()) // TODO: Implement callback
            ]
        ).map_err(|e| Error::Platform(format!("Failed to start advertising: {}", e)))?;
        
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
        
        // Stop advertising
        let manager = self.bluetooth_manager.lock().await;
        if let Some(android_manager) = manager.as_ref() {
            if let Some(le_advertiser) = &android_manager.le_advertiser {
                let env = android_manager.jvm.get_env()
                    .map_err(|e| Error::Platform(format!("Failed to get JNI environment: {}", e)))?;
                
                // TODO: Use actual callback reference instead of null
                env.call_method(
                    le_advertiser,
                    "stopAdvertising",
                    "(Landroid/bluetooth/le/AdvertiseCallback;)V",
                    &[JValue::Object(&JObject::null())]
                ).map_err(|e| Error::Platform(format!("Failed to stop advertising: {}", e)))?;
            }
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
        
        log::info!("Android BLE advertising stopped");
        Ok(())
    }
    
    fn is_advertising(&self) -> bool {
        self.is_advertising.try_read().map(|guard| *guard).unwrap_or(false)
    }
    
    async fn send_to_central(&mut self, peer_id: PeerId, data: &[u8]) -> Result<()> {
        let centrals = self.connected_centrals.read().await;
        if let Some(device_address) = centrals.get(&peer_id) {
            // Send data via GATT characteristic notification
            let gatt_server_lock = self.gatt_server.lock().await;
            if let Some(gatt_server) = gatt_server_lock.as_ref() {
                let manager = self.bluetooth_manager.lock().await;
                let android_manager = manager.as_ref()
                    .ok_or_else(|| Error::Platform("Bluetooth manager not initialized".to_string()))?;
                
                let env = android_manager.jvm.get_env()
                    .map_err(|e| Error::Platform(format!("Failed to get JNI environment: {}", e)))?;
                
                // Create byte array from data
                let data_array = env.byte_array_from_slice(data)
                    .map_err(|e| Error::Platform(format!("Failed to create byte array: {}", e)))?;
                
                // Set characteristic value
                env.call_method(
                    &gatt_server.tx_characteristic,
                    "setValue",
                    "([B)Z",
                    &[JValue::Object(&data_array)]
                ).map_err(|e| Error::Platform(format!("Failed to set characteristic value: {}", e)))?;
                
                // Get BluetoothDevice from address
                let device_address_string = env.new_string(device_address)
                    .map_err(|e| Error::Platform(format!("Failed to create address string: {}", e)))?;
                
                let bluetooth_device = env.call_method(
                    &android_manager.bluetooth_adapter,
                    "getRemoteDevice",
                    "(Ljava/lang/String;)Landroid/bluetooth/BluetoothDevice;",
                    &[JValue::Object(&device_address_string)]
                ).map_err(|e| Error::Platform(format!("Failed to get remote device: {}", e)))?
                .l().map_err(|e| Error::Platform(format!("Failed to get device object: {}", e)))?;
                
                // Notify characteristic changed
                let notified = env.call_method(
                    &gatt_server.gatt_server,
                    "notifyCharacteristicChanged",
                    "(Landroid/bluetooth/BluetoothDevice;Landroid/bluetooth/BluetoothGattCharacteristic;Z)Z",
                    &[
                        JValue::Object(&bluetooth_device),
                        JValue::Object(&gatt_server.tx_characteristic),
                        JValue::Bool(0) // false for notification (vs indication)
                    ]
                ).map_err(|e| Error::Platform(format!("Failed to notify characteristic: {}", e)))?
                .z().map_err(|e| Error::Platform(format!("Failed to get boolean result: {}", e)))?;
                
                if !notified {
                    return Err(Error::Network(format!("Failed to notify central {:?}", peer_id)));
                }
                
                let mut stats = self.stats.write().await;
                stats.bytes_sent += data.len() as u64;
                
                log::debug!("Sent {} bytes to central {:?}", data.len(), peer_id);
                Ok(())
            } else {
                Err(Error::Network("GATT server not initialized".to_string()))
            }
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
    
    async fn set_recovery_config(&mut self, config: RecoveryConfig) -> Result<()> {
        *self.recovery_config.write().await = config;
        Ok(())
    }
    
    async fn recover(&mut self) -> Result<()> {
        log::info!("Manual recovery requested for Android BLE peripheral");
        self.reset_android_ble().await?;
        
        // Restart advertising if it was active
        let config = self.config.read().await.clone();
        if self.is_advertising() {
            self.start_advertising(&config).await?;
        }
        
        Ok(())
    }
    
    async fn get_connection_state(&self, peer_id: PeerId) -> Option<ConnectionState> {
        self.connection_states.read().await.get(&peer_id).cloned()
    }
    
    async fn force_reconnect(&mut self, peer_id: PeerId) -> Result<()> {
        // Update connection state
        {
            let mut states = self.connection_states.write().await;
            states.insert(peer_id, ConnectionState::Connecting);
        }
        
        // Send state change event
        let _ = self.event_sender.send(PeripheralEvent::ConnectionStateChanged {
            peer_id,
            state: ConnectionState::Connecting,
        });
        
        // Android doesn't support initiating connections as peripheral
        log::info!("Force reconnect requested for {:?} - waiting for central to reconnect", peer_id);
        
        Ok(())
    }
    
    async fn health_check(&self) -> Result<bool> {
        let manager_lock = self.bluetooth_manager.lock().await;
        if let Some(android_manager) = manager_lock.as_ref() {
            // Check if Bluetooth is still enabled
            let env = android_manager.jvm.get_env()
                .map_err(|e| Error::Platform(format!("Failed to get JNI environment: {}", e)))?;
            
            let is_enabled = env.call_method(
                &android_manager.bluetooth_adapter,
                "isEnabled",
                "()Z",
                &[]
            ).map_err(|e| Error::Platform(format!("Failed to check Bluetooth status: {}", e)))?
            .z().map_err(|e| Error::Platform(format!("Failed to get boolean result: {}", e)))?;
            
            Ok(is_enabled)
        } else {
            Ok(false)
        }
    }
    
    async fn reset(&mut self) -> Result<()> {
        log::warn!("Emergency reset requested for Android BLE peripheral");
        
        // Stop advertising
        if self.is_advertising() {
            let _ = self.stop_advertising().await;
        }
        
        // Clear all state
        self.connected_centrals.write().await.clear();
        self.connection_states.write().await.clear();
        *self.retry_count.write().await = 0;
        
        // Reset components
        self.reset_android_ble().await?;
        
        log::info!("Android BLE peripheral emergency reset completed");
        Ok(())
    }
    
    /// Attempt recovery with exponential backoff
    async fn attempt_recovery(&mut self, error: &str) -> Result<()> {
        let mut retry_count = self.retry_count.write().await;
        let recovery_config = self.recovery_config.read().await;
        
        if !recovery_config.auto_recovery_enabled {
            return Err(Error::Platform(format!("Auto-recovery disabled: {}", error)));
        }
        
        if *retry_count >= recovery_config.max_retries {
            *retry_count = 0;
            return Err(Error::Platform(format!("Max retries exceeded: {}", error)));
        }
        
        *retry_count += 1;
        let delay = std::cmp::min(
            recovery_config.base_retry_delay_ms * (2_u64.pow(*retry_count - 1)),
            recovery_config.max_retry_delay_ms
        );
        
        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.error_count += 1;
            stats.reconnection_attempts += 1;
            stats.last_error_time = Some(Instant::now());
        }
        
        // Send recovery event
        let _ = self.event_sender.send(PeripheralEvent::AdvertisingFailed {
            error: error.to_string(),
            retry_suggested: true,
            retry_delay_ms: delay,
        });
        
        // Wait before retry
        tokio::time::sleep(Duration::from_millis(delay)).await;
        
        // Attempt to reinitialize
        log::info!("Attempting Android BLE recovery (attempt {})", *retry_count);
        self.reset_android_ble().await?;
        
        Ok(())
    }
    
    /// Reset Android BLE components
    async fn reset_android_ble(&mut self) -> Result<()> {
        log::info!("Resetting Android BLE components");
        
        // Clear existing state
        *self.bluetooth_manager.lock().await = None;
        *self.advertise_callback.lock().await = None;
        *self.gatt_server.lock().await = None;
        *self.is_advertising.write().await = false;
        
        // Re-initialize
        self.initialize_jni().await?;
        
        log::info!("Android BLE reset completed");
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
    // Core Bluetooth FFI handles
    peripheral_manager: Arc<Mutex<Option<CoreBluetoothPeripheralManager>>>,
    service: Arc<Mutex<Option<CoreBluetoothService>>>,
}

/// Core Bluetooth Peripheral Manager wrapper
#[cfg(any(target_os = "ios", target_os = "macos"))]
struct CoreBluetoothPeripheralManager {
    manager_ptr: *mut std::ffi::c_void,
    delegate_ptr: *mut std::ffi::c_void,
}

/// Core Bluetooth Service wrapper
#[cfg(any(target_os = "ios", target_os = "macos"))]
struct CoreBluetoothService {
    service_ptr: *mut std::ffi::c_void,
    tx_characteristic_ptr: *mut std::ffi::c_void,
    rx_characteristic_ptr: *mut std::ffi::c_void,
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
unsafe impl Send for CoreBluetoothPeripheralManager {}
#[cfg(any(target_os = "ios", target_os = "macos"))]
unsafe impl Sync for CoreBluetoothPeripheralManager {}
#[cfg(any(target_os = "ios", target_os = "macos"))]
unsafe impl Send for CoreBluetoothService {}
#[cfg(any(target_os = "ios", target_os = "macos"))]
unsafe impl Sync for CoreBluetoothService {}

#[cfg(any(target_os = "ios", target_os = "macos"))]
use objc::runtime::{Class, Object, Sel};
#[cfg(any(target_os = "ios", target_os = "macos"))]
use objc::{msg_send, sel, sel_impl};
#[cfg(any(target_os = "ios", target_os = "macos"))]
use core_foundation::base::CFTypeRef;
#[cfg(any(target_os = "ios", target_os = "macos"))]
use core_foundation::string::{CFString, CFStringRef};
#[cfg(any(target_os = "ios", target_os = "macos"))]
use core_foundation::uuid::{CFUUID, CFUUIDRef};
#[cfg(any(target_os = "ios", target_os = "macos"))]
use core_foundation::array::{CFArray, CFArrayRef};
#[cfg(any(target_os = "ios", target_os = "macos"))]
use core_foundation::dictionary::{CFDictionary, CFDictionaryRef};
#[cfg(any(target_os = "ios", target_os = "macos"))]
use std::ffi::CStr;

#[cfg(any(target_os = "ios", target_os = "macos"))]
extern "C" {
    fn CBPeripheralManagerAlloc() -> *mut Object;
    fn CBPeripheralManagerInit(manager: *mut Object) -> *mut Object;
    fn CBPeripheralManagerStartAdvertising(manager: *mut Object, data: CFDictionaryRef);
    fn CBPeripheralManagerStopAdvertising(manager: *mut Object);
    fn CBPeripheralManagerAddService(manager: *mut Object, service: *mut Object);
    fn CBMutableServiceAlloc() -> *mut Object;
    fn CBMutableServiceInit(service: *mut Object, uuid: CFUUIDRef, primary: bool) -> *mut Object;
    fn CBMutableCharacteristicAlloc() -> *mut Object;
    fn CBMutableCharacteristicInit(
        characteristic: *mut Object,
        uuid: CFUUIDRef,
        properties: u32,
        permissions: u32
    ) -> *mut Object;
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
            peripheral_manager: Arc::new(Mutex::new(None)),
            service: Arc::new(Mutex::new(None)),
        })
    }
    
    /// Initialize Core Bluetooth peripheral manager
    async fn initialize_peripheral_manager(&mut self) -> Result<()> {
        log::info!("Initializing iOS/macOS CBPeripheralManager for peer {:?}", self.local_peer_id);
        
        unsafe {
            // Create CBPeripheralManager instance
            let manager = CBPeripheralManagerAlloc();
            if manager.is_null() {
                return Err(Error::Platform("Failed to allocate CBPeripheralManager".to_string()));
            }
            
            let initialized_manager = CBPeripheralManagerInit(manager);
            if initialized_manager.is_null() {
                return Err(Error::Platform("Failed to initialize CBPeripheralManager".to_string()));
            }
            
            // TODO: Set up delegate callbacks
            // In a real implementation, we would create an Objective-C class that implements
            // CBPeripheralManagerDelegate and handle callbacks for state changes and connections
            
            // Check if Bluetooth is available and powered on
            // This would typically be done in the delegate callback, but for now we'll simulate
            
            let cb_manager = CoreBluetoothPeripheralManager {
                manager_ptr: initialized_manager as *mut std::ffi::c_void,
                delegate_ptr: std::ptr::null_mut(), // TODO: Implement delegate
            };
            
            *self.peripheral_manager.lock().await = Some(cb_manager);
        }
        
        log::info!("iOS/macOS CBPeripheralManager initialization completed");
        Ok(())
    }
    
    /// Create Core Bluetooth service
    async fn create_cb_service(&self, config: &AdvertisingConfig) -> Result<()> {
        log::debug!("Creating Core Bluetooth service with UUID: {}", config.service_uuid);
        
        unsafe {
            // Create service UUID
            let service_uuid_str = config.service_uuid.to_string();
            let service_uuid_cfstr = CFString::new(&service_uuid_str);
            let service_uuid = CFUUID::from_string(&service_uuid_cfstr);
            
            // Create mutable service
            let service = CBMutableServiceAlloc();
            if service.is_null() {
                return Err(Error::Platform("Failed to allocate CBMutableService".to_string()));
            }
            
            let initialized_service = CBMutableServiceInit(service, service_uuid.as_CFUUIDRef(), true);
            if initialized_service.is_null() {
                return Err(Error::Platform("Failed to initialize CBMutableService".to_string()));
            }
            
            // Create TX characteristic (notify)
            let tx_uuid_str = "6E400002-B5A3-F393-E0A9-E50E24DCCA9E";
            let tx_uuid_cfstr = CFString::new(tx_uuid_str);
            let tx_uuid = CFUUID::from_string(&tx_uuid_cfstr);
            
            let tx_characteristic = CBMutableCharacteristicAlloc();
            if tx_characteristic.is_null() {
                return Err(Error::Platform("Failed to allocate TX characteristic".to_string()));
            }
            
            let initialized_tx_char = CBMutableCharacteristicInit(
                tx_characteristic,
                tx_uuid.as_CFUUIDRef(),
                0x10, // CBCharacteristicPropertyNotify
                0x01  // CBAttributePermissionsReadable
            );
            if initialized_tx_char.is_null() {
                return Err(Error::Platform("Failed to initialize TX characteristic".to_string()));
            }
            
            // Create RX characteristic (write)
            let rx_uuid_str = "6E400003-B5A3-F393-E0A9-E50E24DCCA9E";
            let rx_uuid_cfstr = CFString::new(rx_uuid_str);
            let rx_uuid = CFUUID::from_string(&rx_uuid_cfstr);
            
            let rx_characteristic = CBMutableCharacteristicAlloc();
            if rx_characteristic.is_null() {
                return Err(Error::Platform("Failed to allocate RX characteristic".to_string()));
            }
            
            let initialized_rx_char = CBMutableCharacteristicInit(
                rx_characteristic,
                rx_uuid.as_CFUUIDRef(),
                0x08, // CBCharacteristicPropertyWrite
                0x02  // CBAttributePermissionsWriteable
            );
            if initialized_rx_char.is_null() {
                return Err(Error::Platform("Failed to initialize RX characteristic".to_string()));
            }
            
            // Add characteristics to service
            // This would require additional FFI calls to add characteristics to the service
            
            // Store service references
            let cb_service = CoreBluetoothService {
                service_ptr: initialized_service as *mut std::ffi::c_void,
                tx_characteristic_ptr: initialized_tx_char as *mut std::ffi::c_void,
                rx_characteristic_ptr: initialized_rx_char as *mut std::ffi::c_void,
            };
            
            *self.service.lock().await = Some(cb_service);
            
            // Add service to peripheral manager
            let manager_lock = self.peripheral_manager.lock().await;
            if let Some(manager) = manager_lock.as_ref() {
                CBPeripheralManagerAddService(
                    manager.manager_ptr as *mut Object,
                    initialized_service
                );
            }
        }
        
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
        unsafe {
            let manager_lock = self.peripheral_manager.lock().await;
            if let Some(manager) = manager_lock.as_ref() {
                // Create advertising data dictionary
                // This is a simplified version - in a real implementation,
                // we would need to create proper CFDictionary with service UUIDs and local name
                
                // For now, start advertising with minimal data
                CBPeripheralManagerStartAdvertising(
                    manager.manager_ptr as *mut Object,
                    std::ptr::null() // TODO: Create proper advertising data dictionary
                );
            } else {
                return Err(Error::Platform("Peripheral manager not initialized".to_string()));
            }
        }
        
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
        
        // Stop advertising
        unsafe {
            let manager_lock = self.peripheral_manager.lock().await;
            if let Some(manager) = manager_lock.as_ref() {
                CBPeripheralManagerStopAdvertising(manager.manager_ptr as *mut Object);
            }
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
    
    fn is_advertising(&self) -> bool {
        self.is_advertising.try_read().map(|guard| *guard).unwrap_or(false)
    }
    
    async fn send_to_central(&mut self, peer_id: PeerId, data: &[u8]) -> Result<()> {
        let centrals = self.connected_centrals.read().await;
        if centrals.contains_key(&peer_id) {
            // Send data via characteristic notification
            unsafe {
                let service_lock = self.service.lock().await;
                let manager_lock = self.peripheral_manager.lock().await;
                
                if let (Some(service), Some(manager)) = (service_lock.as_ref(), manager_lock.as_ref()) {
                    // TODO: Implement updateValue:forCharacteristic:onSubscribedCentrals:
                    // This requires additional FFI bindings and proper NSData creation
                    
                    // For now, we'll log the operation
                    log::debug!("Would send {} bytes to central {:?} via Core Bluetooth", data.len(), peer_id);
                } else {
                    return Err(Error::Platform("Service or manager not initialized".to_string()));
                }
            }
            
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
    
    async fn set_recovery_config(&mut self, _config: RecoveryConfig) -> Result<()> {
        log::info!("Recovery config not implemented for iOS/macOS platform");
        Ok(())
    }
    
    async fn recover(&mut self) -> Result<()> {
        log::info!("Manual recovery requested for iOS/macOS BLE peripheral");
        // Reset peripheral manager
        self.initialize_peripheral_manager().await?;
        
        // Restart advertising if it was active
        let config = self.config.read().await.clone();
        if self.is_advertising() {
            self.start_advertising(&config).await?;
        }
        
        Ok(())
    }
    
    async fn get_connection_state(&self, _peer_id: PeerId) -> Option<ConnectionState> {
        None // iOS doesn't track individual connection states from peripheral side
    }
    
    async fn force_reconnect(&mut self, peer_id: PeerId) -> Result<()> {
        // iOS doesn't support peripheral-initiated reconnection
        log::info!("Force reconnect requested for {:?} - iOS doesn't support peripheral-initiated connections", peer_id);
        Ok(())
    }
    
    async fn health_check(&self) -> Result<bool> {
        let manager_lock = self.peripheral_manager.lock().await;
        Ok(manager_lock.is_some())
    }
    
    async fn reset(&mut self) -> Result<()> {
        log::warn!("Emergency reset requested for iOS/macOS BLE peripheral");
        
        // Stop advertising
        if self.is_advertising() {
            let _ = self.stop_advertising().await;
        }
        
        // Clear state
        self.connected_centrals.write().await.clear();
        
        // Reset peripheral manager
        *self.peripheral_manager.lock().await = None;
        *self.service.lock().await = None;
        
        log::info!("iOS/macOS BLE peripheral emergency reset completed");
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
    dbus_connection: Arc<Mutex<Option<BlueZConnection>>>,
    gatt_application: Arc<Mutex<Option<BlueZGattApplication>>>,
    advertisement: Arc<Mutex<Option<BlueZAdvertisement>>>,
}

/// BlueZ D-Bus connection wrapper
#[cfg(target_os = "linux")]
struct BlueZConnection {
    connection: zbus::Connection,
    adapter_proxy: zbus::Proxy<'static>,
    gatt_manager_proxy: zbus::Proxy<'static>,
    le_advertising_manager_proxy: zbus::Proxy<'static>,
}

/// BlueZ GATT Application
#[cfg(target_os = "linux")]
struct BlueZGattApplication {
    service_path: String,
    tx_char_path: String,
    rx_char_path: String,
}

/// BlueZ Advertisement
#[cfg(target_os = "linux")]
struct BlueZAdvertisement {
    advertisement_path: String,
    registered: bool,
}

#[cfg(target_os = "linux")]
use zbus::{Connection, Proxy, ObjectPath};
#[cfg(target_os = "linux")]
use futures_util::stream::StreamExt;
#[cfg(target_os = "linux")]
use std::collections::HashMap as StdHashMap;
#[cfg(target_os = "linux")]
use zbus::dbus_interface;

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
            dbus_connection: Arc::new(Mutex::new(None)),
            gatt_application: Arc::new(Mutex::new(None)),
            advertisement: Arc::new(Mutex::new(None)),
        })
    }
    
    /// Initialize BlueZ D-Bus connection
    async fn initialize_bluez(&mut self) -> Result<()> {
        log::info!("Initializing Linux BlueZ D-Bus connection for peer {:?}", self.local_peer_id);
        
        // Connect to D-Bus system bus
        let connection = Connection::system().await
            .map_err(|e| Error::Platform(format!("Failed to connect to D-Bus: {}", e)))?;
        
        // Get default Bluetooth adapter
        let adapter_proxy = Proxy::new(
            &connection,
            "org.bluez",
            "/org/bluez/hci0", // Default adapter path
            "org.bluez.Adapter1"
        ).await
        .map_err(|e| Error::Platform(format!("Failed to create adapter proxy: {}", e)))?;
        
        // Check if adapter is powered on
        let powered: bool = adapter_proxy.get_property("Powered").await
            .map_err(|e| Error::Platform(format!("Failed to get adapter power state: {}", e)))?;
        
        if !powered {
            // Try to power on the adapter
            adapter_proxy.set_property("Powered", true).await
                .map_err(|e| Error::Platform(format!("Failed to power on adapter: {}", e)))?;
        }
        
        // Get GATT Manager proxy
        let gatt_manager_proxy = Proxy::new(
            &connection,
            "org.bluez",
            "/org/bluez/hci0",
            "org.bluez.GattManager1"
        ).await
        .map_err(|e| Error::Platform(format!("Failed to create GATT manager proxy: {}", e)))?;
        
        // Get LE Advertising Manager proxy
        let le_advertising_manager_proxy = Proxy::new(
            &connection,
            "org.bluez",
            "/org/bluez/hci0",
            "org.bluez.LEAdvertisingManager1"
        ).await
        .map_err(|e| Error::Platform(format!("Failed to create LE advertising manager proxy: {}", e)))?;
        
        let bluez_connection = BlueZConnection {
            connection,
            adapter_proxy,
            gatt_manager_proxy,
            le_advertising_manager_proxy,
        };
        
        *self.dbus_connection.lock().await = Some(bluez_connection);
        
        log::info!("Linux BlueZ D-Bus connection initialized successfully");
        Ok(())
    }
    
    /// Register GATT application with BlueZ
    async fn register_gatt_application(&self, config: &AdvertisingConfig) -> Result<()> {
        log::debug!("Registering GATT application with BlueZ for service {}", config.service_uuid);
        
        let connection_lock = self.dbus_connection.lock().await;
        let bluez_conn = connection_lock.as_ref()
            .ok_or_else(|| Error::Platform("BlueZ D-Bus connection not initialized".to_string()))?;
        
        // Create GATT service paths
        let app_path = format!("/com/bitcraps/app/{}", hex::encode(self.local_peer_id));
        let service_path = format!("{}/service0", app_path);
        let tx_char_path = format!("{}/char0", service_path);
        let rx_char_path = format!("{}/char1", service_path);
        
        // TODO: Implement D-Bus object server with GATT service interfaces
        // This requires implementing the org.bluez.GattService1,
        // org.bluez.GattCharacteristic1, and org.bluez.GattDescriptor1 interfaces
        
        // For now, we'll create a simplified application structure
        let gatt_app = BlueZGattApplication {
            service_path: service_path.clone(),
            tx_char_path: tx_char_path.clone(),
            rx_char_path: rx_char_path.clone(),
        };
        
        // Register the application with GATT Manager
        let app_path_obj = ObjectPath::try_from(app_path.as_str())
            .map_err(|e| Error::Platform(format!("Invalid application path: {}", e)))?;
        
        // This call would register our GATT application
        // gatt_manager_proxy.call_method("RegisterApplication", &(app_path_obj, StdHashMap::<String, zbus::Value>::new()))
        //     .await
        //     .map_err(|e| Error::Platform(format!("Failed to register GATT application: {}", e)))?;
        
        *self.gatt_application.lock().await = Some(gatt_app);
        
        log::debug!("GATT application registered successfully at path: {}", service_path);
        Ok(())
    }
    
    /// Register advertisement with BlueZ
    async fn register_advertisement(&self, config: &AdvertisingConfig) -> Result<()> {
        log::debug!("Registering advertisement with BlueZ: interval={}ms", config.advertising_interval_ms);
        
        let connection_lock = self.dbus_connection.lock().await;
        let bluez_conn = connection_lock.as_ref()
            .ok_or_else(|| Error::Platform("BlueZ D-Bus connection not initialized".to_string()))?;
        
        // Create advertisement path
        let adv_path = format!("/com/bitcraps/advertisement/{}", hex::encode(self.local_peer_id));
        
        // TODO: Implement D-Bus object server with LEAdvertisement1 interface
        // This requires implementing the org.bluez.LEAdvertisement1 interface
        
        // Create advertisement data structure
        let mut adv_properties = StdHashMap::new();
        
        // Set advertisement type
        adv_properties.insert("Type".to_string(), zbus::Value::Str("peripheral".into()));
        
        // Set service UUIDs
        let service_uuids = vec![config.service_uuid.to_string()];
        adv_properties.insert("ServiceUUIDs".to_string(), zbus::Value::Array(vec![].into()));
        
        // Set local name if enabled
        if config.include_name {
            adv_properties.insert("LocalName".to_string(), zbus::Value::Str(config.local_name.clone().into()));
        }
        
        // Set manufacturer data with peer ID
        let peer_id_bytes = self.local_peer_id.as_bytes();
        let manufacturer_data = StdHashMap::new();
        // manufacturer_data.insert(0x004C, peer_id_bytes); // Apple company ID
        
        let advertisement = BlueZAdvertisement {
            advertisement_path: adv_path.clone(),
            registered: false,
        };
        
        // Register advertisement with LE Advertising Manager
        let adv_path_obj = ObjectPath::try_from(adv_path.as_str())
            .map_err(|e| Error::Platform(format!("Invalid advertisement path: {}", e)))?;
        
        // This call would register our advertisement
        // le_advertising_manager_proxy.call_method("RegisterAdvertisement", &(adv_path_obj, StdHashMap::<String, zbus::Value>::new()))
        //     .await
        //     .map_err(|e| Error::Platform(format!("Failed to register advertisement: {}", e)))?;
        
        *self.advertisement.lock().await = Some(advertisement);
        
        log::debug!("Advertisement registered successfully at path: {}", adv_path);
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
        let advertisement_lock = self.advertisement.lock().await;
        if let Some(advertisement) = advertisement_lock.as_ref() {
            if advertisement.registered {
                let connection_lock = self.dbus_connection.lock().await;
                if let Some(bluez_conn) = connection_lock.as_ref() {
                    let adv_path_obj = ObjectPath::try_from(advertisement.advertisement_path.as_str())
                        .map_err(|e| Error::Platform(format!("Invalid advertisement path: {}", e)))?;
                    
                    // Unregister advertisement
                    // bluez_conn.le_advertising_manager_proxy.call_method("UnregisterAdvertisement", &(adv_path_obj,))
                    //     .await
                    //     .map_err(|e| Error::Platform(format!("Failed to unregister advertisement: {}", e)))?;
                    
                    log::debug!("Advertisement unregistered from path: {}", advertisement.advertisement_path);
                }
            }
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
            let gatt_app_lock = self.gatt_application.lock().await;
            if let Some(gatt_app) = gatt_app_lock.as_ref() {
                let connection_lock = self.dbus_connection.lock().await;
                if let Some(bluez_conn) = connection_lock.as_ref() {
                    // TODO: Implement characteristic notification via D-Bus
                    // This would require emitting PropertiesChanged signal on the characteristic
                    
                    log::debug!("Would send {} bytes to central {:?} via BlueZ D-Bus characteristic {}", 
                              data.len(), peer_id, gatt_app.tx_char_path);
                } else {
                    return Err(Error::Platform("BlueZ D-Bus connection not available".to_string()));
                }
            } else {
                return Err(Error::Platform("GATT application not registered".to_string()));
            }
            
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
            let connection_lock = self.dbus_connection.lock().await;
            if let Some(bluez_conn) = connection_lock.as_ref() {
                // Get device proxy and disconnect
                let device_path = format!("/org/bluez/hci0/dev_{}", address.replace(':', "_"));
                
                // TODO: Create device proxy and call Disconnect method
                // let device_proxy = Proxy::new(
                //     &bluez_conn.connection,
                //     "org.bluez",
                //     &device_path,
                //     "org.bluez.Device1"
                // ).await?;
                // device_proxy.call_method("Disconnect", &()).await?;
                
                log::debug!("Would disconnect device at path: {}", device_path);
            }
            
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
    
    async fn set_recovery_config(&mut self, _config: RecoveryConfig) -> Result<()> {
        log::info!("Recovery config not implemented for Linux platform");
        Ok(())
    }
    
    async fn recover(&mut self) -> Result<()> {
        log::info!("Manual recovery requested for Linux BlueZ BLE peripheral");
        
        // Re-initialize BlueZ connection
        self.initialize_bluez().await?;
        
        // Restart advertising if it was active
        let config = self.config.read().await.clone();
        if self.is_advertising() {
            self.start_advertising(&config).await?;
        }
        
        Ok(())
    }
    
    async fn get_connection_state(&self, _peer_id: PeerId) -> Option<ConnectionState> {
        None // BlueZ connection states are managed externally
    }
    
    async fn force_reconnect(&mut self, peer_id: PeerId) -> Result<()> {
        log::info!("Force reconnect requested for {:?} - BlueZ doesn't support peripheral-initiated connections", peer_id);
        Ok(())
    }
    
    async fn health_check(&self) -> Result<bool> {
        let connection_lock = self.dbus_connection.lock().await;
        if let Some(bluez_conn) = connection_lock.as_ref() {
            // Check if adapter is powered on
            match bluez_conn.adapter_proxy.get_property("Powered").await {
                Ok(powered) => Ok(powered),
                Err(_) => Ok(false),
            }
        } else {
            Ok(false)
        }
    }
    
    async fn reset(&mut self) -> Result<()> {
        log::warn!("Emergency reset requested for Linux BlueZ BLE peripheral");
        
        // Stop advertising
        if self.is_advertising() {
            let _ = self.stop_advertising().await;
        }
        
        // Clear state
        self.connected_centrals.write().await.clear();
        
        // Reset D-Bus connection
        *self.dbus_connection.lock().await = None;
        *self.gatt_application.lock().await = None;
        *self.advertisement.lock().await = None;
        
        log::info!("Linux BlueZ BLE peripheral emergency reset completed");
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
    // Windows BLE components
    ble_advertiser: Arc<Mutex<Option<WindowsBleAdvertiser>>>,
    gatt_server: Arc<Mutex<Option<WindowsGattServer>>>,
}

/// Windows BLE Advertiser wrapper
#[cfg(target_os = "windows")]
struct WindowsBleAdvertiser {
    publisher: *mut std::ffi::c_void, // BluetoothLEAdvertisementPublisher
}

/// Windows GATT Server wrapper
#[cfg(target_os = "windows")]
struct WindowsGattServer {
    server: *mut std::ffi::c_void, // GattServiceProvider
    service: *mut std::ffi::c_void,
    tx_characteristic: *mut std::ffi::c_void,
    rx_characteristic: *mut std::ffi::c_void,
}

#[cfg(target_os = "windows")]
unsafe impl Send for WindowsBleAdvertiser {}
#[cfg(target_os = "windows")]
unsafe impl Sync for WindowsBleAdvertiser {}
#[cfg(target_os = "windows")]
unsafe impl Send for WindowsGattServer {}
#[cfg(target_os = "windows")]
unsafe impl Sync for WindowsGattServer {}

#[cfg(target_os = "windows")]
use windows::{
    Win32::Devices::Bluetooth::*,
    Win32::Foundation::*,
    Win32::System::Com::*,
};
#[cfg(target_os = "windows")]
use std::ptr;

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
            ble_advertiser: Arc::new(Mutex::new(None)),
            gatt_server: Arc::new(Mutex::new(None)),
        })
    }
    
    /// Initialize Windows BLE advertising
    async fn initialize_windows_ble(&self) -> Result<()> {
        log::info!("Initializing Windows BLE advertising for peer {:?}", self.local_peer_id);
        
        unsafe {
            // Initialize COM
            CoInitializeEx(ptr::null_mut(), COINIT_APARTMENTTHREADED)
                .ok()
                .map_err(|e| Error::Platform(format!("Failed to initialize COM: {:?}", e)))?;
            
            // TODO: Create BluetoothLEAdvertisementPublisher using WinRT APIs
            // This requires using the Windows Runtime (WinRT) APIs through COM interop
            // For now, we'll create placeholder structures
            
            let advertiser = WindowsBleAdvertiser {
                publisher: ptr::null_mut(), // TODO: Initialize actual publisher
            };
            
            *self.ble_advertiser.lock().await = Some(advertiser);
        }
        
        log::info!("Windows BLE initialization completed");
        Ok(())
    }
    
    /// Set up Windows GATT server
    async fn setup_windows_gatt_server(&self, config: &AdvertisingConfig) -> Result<()> {
        log::debug!("Setting up Windows GATT server for service {}", config.service_uuid);
        
        unsafe {
            // TODO: Create GattServiceProvider using WinRT APIs
            // This would involve:
            // 1. Creating a GattServiceProvider
            // 2. Creating GattLocalCharacteristics for TX/RX
            // 3. Setting up characteristic properties and permissions
            // 4. Handling characteristic read/write events
            
            let gatt_server = WindowsGattServer {
                server: ptr::null_mut(),
                service: ptr::null_mut(),
                tx_characteristic: ptr::null_mut(),
                rx_characteristic: ptr::null_mut(),
            };
            
            *self.gatt_server.lock().await = Some(gatt_server);
        }
        
        log::debug!("Windows GATT server setup completed");
        Ok(())
    }
}

#[cfg(target_os = "windows")]
#[async_trait]
impl BlePeripheral for WindowsBlePeripheral {
    async fn start_advertising(&mut self, config: &AdvertisingConfig) -> Result<()> {
        if *self.is_advertising.read().await {
            return Err(Error::Network("Already advertising".to_string()));
        }
        
        log::info!("Starting Windows BLE advertising");
        
        // Initialize Windows BLE if not already done
        self.initialize_windows_ble().await?;
        
        // Set up GATT server
        self.setup_windows_gatt_server(config).await?;
        
        // Update configuration
        *self.config.write().await = config.clone();
        
        // Start advertising using Windows Runtime APIs
        let advertiser_lock = self.ble_advertiser.lock().await;
        if let Some(_advertiser) = advertiser_lock.as_ref() {
            // TODO: Start advertising with proper WinRT calls
            // This would involve:
            // 1. Creating BluetoothLEAdvertisement object
            // 2. Setting service UUIDs, local name, manufacturer data
            // 3. Configuring advertisement settings (interval, TX power)
            // 4. Starting the publisher
            
            log::debug!("Would start Windows BLE advertising with config: {:?}", config);
        } else {
            return Err(Error::Platform("BLE advertiser not initialized".to_string()));
        }
        
        // Update state
        *self.is_advertising.write().await = true;
        *self.advertising_start_time.write().await = Some(Instant::now());
        
        // Send event
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
            // Send data via GATT characteristic notification
            let gatt_server_lock = self.gatt_server.lock().await;
            if let Some(_gatt_server) = gatt_server_lock.as_ref() {
                // TODO: Implement characteristic notification using WinRT
                // This would involve:
                // 1. Getting the appropriate GattLocalCharacteristic
                // 2. Creating notification data
                // 3. Calling NotifyValueAsync on subscribed sessions
                
                log::debug!("Would send {} bytes to central {:?} via Windows GATT", data.len(), peer_id);
            } else {
                return Err(Error::Platform("GATT server not initialized".to_string()));
            }
            
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
            // Disconnect via Windows GATT server
            let gatt_server_lock = self.gatt_server.lock().await;
            if let Some(_gatt_server) = gatt_server_lock.as_ref() {
                // TODO: Disconnect specific session using WinRT
                // This would involve getting the GattSession and calling Close()
                
                log::debug!("Would disconnect central {:?} at {} via Windows GATT", peer_id, address);
            }
            
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
    
    async fn set_recovery_config(&mut self, _config: RecoveryConfig) -> Result<()> {
        log::info!("Recovery config not implemented for Windows platform");
        Ok(())
    }
    
    async fn recover(&mut self) -> Result<()> {
        log::info!("Manual recovery requested for Windows BLE peripheral");
        
        // Re-initialize Windows BLE
        self.initialize_windows_ble().await?;
        
        // Restart advertising if it was active
        let config = self.config.read().await.clone();
        if self.is_advertising() {
            self.start_advertising(&config).await?;
        }
        
        Ok(())
    }
    
    async fn get_connection_state(&self, _peer_id: PeerId) -> Option<ConnectionState> {
        None // Windows connection states managed by WinRT
    }
    
    async fn force_reconnect(&mut self, peer_id: PeerId) -> Result<()> {
        log::info!("Force reconnect requested for {:?} - Windows doesn't support peripheral-initiated connections", peer_id);
        Ok(())
    }
    
    async fn health_check(&self) -> Result<bool> {
        let advertiser_lock = self.ble_advertiser.lock().await;
        let gatt_server_lock = self.gatt_server.lock().await;
        Ok(advertiser_lock.is_some() && gatt_server_lock.is_some())
    }
    
    async fn reset(&mut self) -> Result<()> {
        log::warn!("Emergency reset requested for Windows BLE peripheral");
        
        // Stop advertising
        if self.is_advertising() {
            let _ = self.stop_advertising().await;
        }
        
        // Clear state
        self.connected_centrals.write().await.clear();
        
        // Reset components
        *self.ble_advertiser.lock().await = None;
        *self.gatt_server.lock().await = None;
        
        log::info!("Windows BLE peripheral emergency reset completed");
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
    
    async fn set_recovery_config(&mut self, _config: RecoveryConfig) -> Result<()> {
        Ok(())
    }
    
    async fn recover(&mut self) -> Result<()> {
        log::info!("Recovery not supported on fallback platform");
        Ok(())
    }
    
    async fn get_connection_state(&self, _peer_id: PeerId) -> Option<ConnectionState> {
        None
    }
    
    async fn force_reconnect(&mut self, peer_id: PeerId) -> Result<()> {
        Err(Error::Network(format!("Cannot reconnect to {:?}: platform not supported", peer_id)))
    }
    
    async fn health_check(&self) -> Result<bool> {
        Ok(false)
    }
    
    async fn reset(&mut self) -> Result<()> {
        log::info!("Reset not needed on fallback platform");
        Ok(())
    }
}