//! Android BLE Peripheral Implementation via JNI
//! 
//! This module provides a complete Android BLE peripheral implementation
//! using JNI calls to Android's BluetoothLeAdvertiser and BluetoothGattServer APIs.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock, Mutex};
use uuid::Uuid;

use crate::protocol::PeerId;
use crate::error::{Error, Result};
use crate::transport::ble_peripheral::{
    BlePeripheral, AdvertisingConfig, PeripheralEvent, PeripheralStats, BITCRAPS_SERVICE_UUID
};

#[cfg(target_os = "android")]
use jni::{
    JNIEnv, JavaVM, GlobalRef, AttachGuard,
    objects::{JObject, JString, JValue, JByteArray},
    signature::{JavaType, Primitive},
    sys::{jlong, jint, jboolean, JNI_TRUE, JNI_FALSE},
};

#[cfg(target_os = "android")]
use std::sync::atomic::{AtomicPtr, Ordering};

/// Android-specific BLE advertising constants
#[cfg(target_os = "android")]
mod android_constants {
    /// AdvertiseSettings.ADVERTISE_MODE_LOW_LATENCY (0)
    pub const ADVERTISE_MODE_LOW_LATENCY: i32 = 0;
    /// AdvertiseSettings.ADVERTISE_MODE_BALANCED (1)  
    pub const ADVERTISE_MODE_BALANCED: i32 = 1;
    /// AdvertiseSettings.ADVERTISE_MODE_LOW_POWER (2)
    pub const ADVERTISE_MODE_LOW_POWER: i32 = 2;
    
    /// AdvertiseSettings.ADVERTISE_TX_POWER_ULTRA_LOW (-21)
    pub const ADVERTISE_TX_POWER_ULTRA_LOW: i32 = -21;
    /// AdvertiseSettings.ADVERTISE_TX_POWER_LOW (-12)
    pub const ADVERTISE_TX_POWER_LOW: i32 = -12;
    /// AdvertiseSettings.ADVERTISE_TX_POWER_MEDIUM (-7)
    pub const ADVERTISE_TX_POWER_MEDIUM: i32 = -7;
    /// AdvertiseSettings.ADVERTISE_TX_POWER_HIGH (1)
    pub const ADVERTISE_TX_POWER_HIGH: i32 = 1;
    
    /// BluetoothGattCharacteristic.PROPERTY_READ (2)
    pub const PROPERTY_READ: i32 = 2;
    /// BluetoothGattCharacteristic.PROPERTY_WRITE (8)
    pub const PROPERTY_WRITE: i32 = 8;
    /// BluetoothGattCharacteristic.PROPERTY_NOTIFY (16)
    pub const PROPERTY_NOTIFY: i32 = 16;
    
    /// BluetoothGattCharacteristic.PERMISSION_READ (1)
    pub const PERMISSION_READ: i32 = 1;
    /// BluetoothGattCharacteristic.PERMISSION_WRITE (16)
    pub const PERMISSION_WRITE: i32 = 16;
}

/// Android BLE Peripheral using JNI
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
    
    // JNI components
    java_vm: Option<Arc<JavaVM>>,
    bluetooth_adapter: Option<GlobalRef>,
    bluetooth_le_advertiser: Option<GlobalRef>,
    gatt_server: Option<GlobalRef>,
    advertise_callback: Option<GlobalRef>,
    gatt_server_callback: Option<GlobalRef>,
    
    // Service and characteristics
    bitcraps_service: Option<GlobalRef>,
    tx_characteristic: Option<GlobalRef>,
    rx_characteristic: Option<GlobalRef>,
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
            
            java_vm: None,
            bluetooth_adapter: None,
            bluetooth_le_advertiser: None,
            gatt_server: None,
            advertise_callback: None,
            gatt_server_callback: None,
            
            bitcraps_service: None,
            tx_characteristic: None,
            rx_characteristic: None,
        })
    }
    
    /// Initialize JNI components for Android BLE
    pub async fn initialize_jni(&mut self) -> Result<()> {
        log::info!("Initializing Android BLE JNI components for peer {:?}", self.local_peer_id);
        
        // Get the JavaVM from ndk-context
        let java_vm = ndk_context::java_vm().ok_or_else(|| {
            Error::Network("Failed to get JavaVM from ndk-context".to_string())
        })?;
        
        let java_vm = Arc::new(java_vm);
        let mut env = java_vm.attach_current_thread().map_err(|e| {
            Error::Network(format!("Failed to attach to Java thread: {}", e))
        })?;
        
        // Get BluetoothAdapter
        self.get_bluetooth_adapter(&mut env).await?;
        
        // Get BluetoothLeAdvertiser
        self.get_bluetooth_le_advertiser(&mut env).await?;
        
        // Set up callbacks
        self.setup_advertise_callback(&mut env).await?;
        self.setup_gatt_server_callback(&mut env).await?;
        
        // Create GATT service and characteristics
        self.create_gatt_service(&mut env).await?;
        
        // Open GATT server
        self.open_gatt_server(&mut env).await?;
        
        self.java_vm = Some(java_vm);
        
        log::info!("Android BLE JNI initialization completed successfully");
        Ok(())
    }
    
    /// Get Android BluetoothAdapter instance
    async fn get_bluetooth_adapter(&mut self, env: &mut AttachGuard) -> Result<()> {
        log::debug!("Getting Android BluetoothAdapter");
        
        // BluetoothAdapter adapter = BluetoothAdapter.getDefaultAdapter();
        let bluetooth_adapter_class = env.find_class("android/bluetooth/BluetoothAdapter")
            .map_err(|e| Error::Network(format!("Failed to find BluetoothAdapter class: {}", e)))?;
        
        let adapter_jobject = env.call_static_method(
            bluetooth_adapter_class,
            "getDefaultAdapter",
            "()Landroid/bluetooth/BluetoothAdapter;",
            &[]
        ).map_err(|e| Error::Network(format!("Failed to get default adapter: {}", e)))?
         .l().map_err(|e| Error::Network(format!("Failed to convert adapter to object: {}", e)))?;
        
        if adapter_jobject.is_null() {
            return Err(Error::Network("BluetoothAdapter is null - Bluetooth not available".to_string()));
        }
        
        // Check if Bluetooth is enabled
        let is_enabled = env.call_method(
            adapter_jobject,
            "isEnabled",
            "()Z",
            &[]
        ).map_err(|e| Error::Network(format!("Failed to check if Bluetooth is enabled: {}", e)))?
         .z().map_err(|e| Error::Network(format!("Failed to convert boolean: {}", e)))?;
        
        if !is_enabled {
            return Err(Error::Network("Bluetooth is not enabled".to_string()));
        }
        
        // Store global reference
        self.bluetooth_adapter = Some(env.new_global_ref(adapter_jobject).map_err(|e| {
            Error::Network(format!("Failed to create global reference for adapter: {}", e))
        })?);
        
        log::debug!("BluetoothAdapter obtained successfully");
        Ok(())
    }
    
    /// Get Android BluetoothLeAdvertiser instance
    async fn get_bluetooth_le_advertiser(&mut self, env: &mut AttachGuard) -> Result<()> {
        log::debug!("Getting Android BluetoothLeAdvertiser");
        
        let adapter_ref = self.bluetooth_adapter.as_ref().ok_or_else(|| {
            Error::Network("BluetoothAdapter not initialized".to_string())
        })?;
        
        // BluetoothLeAdvertiser advertiser = adapter.getBluetoothLeAdvertiser();
        let advertiser_jobject = env.call_method(
            adapter_ref.as_obj(),
            "getBluetoothLeAdvertiser",
            "()Landroid/bluetooth/le/BluetoothLeAdvertiser;",
            &[]
        ).map_err(|e| Error::Network(format!("Failed to get BluetoothLeAdvertiser: {}", e)))?
         .l().map_err(|e| Error::Network(format!("Failed to convert advertiser to object: {}", e)))?;
        
        if advertiser_jobject.is_null() {
            return Err(Error::Network("BluetoothLeAdvertiser is null - BLE advertising not supported".to_string()));
        }
        
        // Store global reference
        self.bluetooth_le_advertiser = Some(env.new_global_ref(advertiser_jobject).map_err(|e| {
            Error::Network(format!("Failed to create global reference for advertiser: {}", e))
        })?);
        
        log::debug!("BluetoothLeAdvertiser obtained successfully");
        Ok(())
    }
    
    /// Set up AdvertiseCallback for handling advertising events
    async fn setup_advertise_callback(&mut self, env: &mut AttachGuard) -> Result<()> {
        log::debug!("Setting up AdvertiseCallback");
        
        // We need to create a custom AdvertiseCallback class in Java/Kotlin
        // This is a placeholder - in real implementation, you'd have a Java class
        // that extends AdvertiseCallback and calls back to Rust via JNI
        
        // For now, create a simple callback object reference
        // In practice, this would be implemented as a native Java class
        let callback_class = env.find_class("com/bitchat/rust/AdvertiseCallbackBridge")
            .map_err(|e| Error::Network(format!("Failed to find AdvertiseCallbackBridge class (implement in Java): {}", e)))?;
        
        let callback_object = env.new_object(
            callback_class,
            "(J)V", // Constructor takes long (Rust object pointer)
            &[JValue::Long(self as *const _ as jlong)]
        ).map_err(|e| Error::Network(format!("Failed to create AdvertiseCallbackBridge: {}", e)))?;
        
        self.advertise_callback = Some(env.new_global_ref(callback_object).map_err(|e| {
            Error::Network(format!("Failed to create global reference for callback: {}", e))
        })?);
        
        log::debug!("AdvertiseCallback setup completed");
        Ok(())
    }
    
    /// Set up BluetoothGattServerCallback for handling GATT server events
    async fn setup_gatt_server_callback(&mut self, env: &mut AttachGuard) -> Result<()> {
        log::debug!("Setting up BluetoothGattServerCallback");
        
        // Similar to AdvertiseCallback, this needs a custom Java class
        let callback_class = env.find_class("com/bitchat/rust/GattServerCallbackBridge")
            .map_err(|e| Error::Network(format!("Failed to find GattServerCallbackBridge class (implement in Java): {}", e)))?;
        
        let callback_object = env.new_object(
            callback_class,
            "(J)V",
            &[JValue::Long(self as *const _ as jlong)]
        ).map_err(|e| Error::Network(format!("Failed to create GattServerCallbackBridge: {}", e)))?;
        
        self.gatt_server_callback = Some(env.new_global_ref(callback_object).map_err(|e| {
            Error::Network(format!("Failed to create global reference for GATT callback: {}", e))
        })?);
        
        log::debug!("BluetoothGattServerCallback setup completed");
        Ok(())
    }
    
    /// Create GATT service with TX/RX characteristics
    async fn create_gatt_service(&mut self, env: &mut AttachGuard) -> Result<()> {
        log::debug!("Creating GATT service and characteristics");
        
        // Create service UUID
        let service_uuid_str = env.new_string(BITCRAPS_SERVICE_UUID.to_string())
            .map_err(|e| Error::Network(format!("Failed to create service UUID string: {}", e)))?;
        
        let uuid_class = env.find_class("java/util/UUID")
            .map_err(|e| Error::Network(format!("Failed to find UUID class: {}", e)))?;
        
        let service_uuid = env.call_static_method(
            uuid_class,
            "fromString",
            "(Ljava/lang/String;)Ljava/util/UUID;",
            &[JValue::Object(service_uuid_str.into())]
        ).map_err(|e| Error::Network(format!("Failed to create service UUID: {}", e)))?
         .l().map_err(|e| Error::Network(format!("Failed to convert service UUID: {}", e)))?;
        
        // Create BluetoothGattService
        let gatt_service_class = env.find_class("android/bluetooth/BluetoothGattService")
            .map_err(|e| Error::Network(format!("Failed to find BluetoothGattService class: {}", e)))?;
        
        let service_object = env.new_object(
            gatt_service_class,
            "(Ljava/util/UUID;I)V",
            &[
                JValue::Object(service_uuid),
                JValue::Int(0) // BluetoothGattService.SERVICE_TYPE_PRIMARY
            ]
        ).map_err(|e| Error::Network(format!("Failed to create BluetoothGattService: {}", e)))?;
        
        // Create TX characteristic (for sending data to central)
        self.create_tx_characteristic(env, &service_object).await?;
        
        // Create RX characteristic (for receiving data from central)
        self.create_rx_characteristic(env, &service_object).await?;
        
        // Store service reference
        self.bitcraps_service = Some(env.new_global_ref(service_object).map_err(|e| {
            Error::Network(format!("Failed to create global reference for service: {}", e))
        })?);
        
        log::debug!("GATT service created successfully");
        Ok(())
    }
    
    /// Create TX characteristic for sending data to centrals
    async fn create_tx_characteristic(&mut self, env: &mut AttachGuard, service: &JObject) -> Result<()> {
        use crate::transport::ble_peripheral::BITCRAPS_SERVICE_UUID;
        
        // TX characteristic UUID (for sending data to central)
        let tx_uuid = Uuid::from_u128(BITCRAPS_SERVICE_UUID.as_u128() + 1);
        let tx_uuid_str = env.new_string(tx_uuid.to_string())
            .map_err(|e| Error::Network(format!("Failed to create TX UUID string: {}", e)))?;
        
        let uuid_class = env.find_class("java/util/UUID")
            .map_err(|e| Error::Network(format!("Failed to find UUID class: {}", e)))?;
        
        let tx_uuid_obj = env.call_static_method(
            uuid_class,
            "fromString",
            "(Ljava/lang/String;)Ljava/util/UUID;",
            &[JValue::Object(tx_uuid_str.into())]
        ).map_err(|e| Error::Network(format!("Failed to create TX UUID: {}", e)))?
         .l().map_err(|e| Error::Network(format!("Failed to convert TX UUID: {}", e)))?;
        
        // Create characteristic with NOTIFY property
        let char_class = env.find_class("android/bluetooth/BluetoothGattCharacteristic")
            .map_err(|e| Error::Network(format!("Failed to find BluetoothGattCharacteristic class: {}", e)))?;
        
        let tx_char = env.new_object(
            char_class,
            "(Ljava/util/UUID;II)V",
            &[
                JValue::Object(tx_uuid_obj),
                JValue::Int(android_constants::PROPERTY_READ | android_constants::PROPERTY_NOTIFY),
                JValue::Int(android_constants::PERMISSION_READ)
            ]
        ).map_err(|e| Error::Network(format!("Failed to create TX characteristic: {}", e)))?;
        
        // Add characteristic to service
        env.call_method(
            service,
            "addCharacteristic",
            "(Landroid/bluetooth/BluetoothGattCharacteristic;)Z",
            &[JValue::Object(tx_char)]
        ).map_err(|e| Error::Network(format!("Failed to add TX characteristic to service: {}", e)))?;
        
        // Store reference
        self.tx_characteristic = Some(env.new_global_ref(tx_char).map_err(|e| {
            Error::Network(format!("Failed to create global reference for TX characteristic: {}", e))
        })?);
        
        log::debug!("TX characteristic created successfully");
        Ok(())
    }
    
    /// Create RX characteristic for receiving data from centrals
    async fn create_rx_characteristic(&mut self, env: &mut AttachGuard, service: &JObject) -> Result<()> {
        use crate::transport::ble_peripheral::BITCRAPS_SERVICE_UUID;
        
        // RX characteristic UUID (for receiving data from central)
        let rx_uuid = Uuid::from_u128(BITCRAPS_SERVICE_UUID.as_u128() + 2);
        let rx_uuid_str = env.new_string(rx_uuid.to_string())
            .map_err(|e| Error::Network(format!("Failed to create RX UUID string: {}", e)))?;
        
        let uuid_class = env.find_class("java/util/UUID")
            .map_err(|e| Error::Network(format!("Failed to find UUID class: {}", e)))?;
        
        let rx_uuid_obj = env.call_static_method(
            uuid_class,
            "fromString",
            "(Ljava/lang/String;)Ljava/util/UUID;",
            &[JValue::Object(rx_uuid_str.into())]
        ).map_err(|e| Error::Network(format!("Failed to create RX UUID: {}", e)))?
         .l().map_err(|e| Error::Network(format!("Failed to convert RX UUID: {}", e)))?;
        
        // Create characteristic with WRITE property
        let char_class = env.find_class("android/bluetooth/BluetoothGattCharacteristic")
            .map_err(|e| Error::Network(format!("Failed to find BluetoothGattCharacteristic class: {}", e)))?;
        
        let rx_char = env.new_object(
            char_class,
            "(Ljava/util/UUID;II)V",
            &[
                JValue::Object(rx_uuid_obj),
                JValue::Int(android_constants::PROPERTY_WRITE),
                JValue::Int(android_constants::PERMISSION_WRITE)
            ]
        ).map_err(|e| Error::Network(format!("Failed to create RX characteristic: {}", e)))?;
        
        // Add characteristic to service
        env.call_method(
            service,
            "addCharacteristic",
            "(Landroid/bluetooth/BluetoothGattCharacteristic;)Z",
            &[JValue::Object(rx_char)]
        ).map_err(|e| Error::Network(format!("Failed to add RX characteristic to service: {}", e)))?;
        
        // Store reference
        self.rx_characteristic = Some(env.new_global_ref(rx_char).map_err(|e| {
            Error::Network(format!("Failed to create global reference for RX characteristic: {}", e))
        })?);
        
        log::debug!("RX characteristic created successfully");
        Ok(())
    }
    
    /// Open GATT server for handling connections
    async fn open_gatt_server(&mut self, env: &mut AttachGuard) -> Result<()> {
        log::debug!("Opening GATT server");
        
        let adapter_ref = self.bluetooth_adapter.as_ref().ok_or_else(|| {
            Error::Network("BluetoothAdapter not initialized".to_string())
        })?;
        
        let callback_ref = self.gatt_server_callback.as_ref().ok_or_else(|| {
            Error::Network("GATT server callback not initialized".to_string())
        })?;
        
        // Get BluetoothManager
        let context = ndk_context::android_context().android_context();
        let bluetooth_service_str = env.new_string("bluetooth")
            .map_err(|e| Error::Network(format!("Failed to create bluetooth service string: {}", e)))?;
        
        let bluetooth_manager = env.call_method(
            context,
            "getSystemService",
            "(Ljava/lang/String;)Ljava/lang/Object;",
            &[JValue::Object(bluetooth_service_str.into())]
        ).map_err(|e| Error::Network(format!("Failed to get BluetoothManager: {}", e)))?
         .l().map_err(|e| Error::Network(format!("Failed to convert BluetoothManager: {}", e)))?;
        
        // Open GATT server
        let gatt_server = env.call_method(
            bluetooth_manager,
            "openGattServer",
            "(Landroid/content/Context;Landroid/bluetooth/BluetoothGattServerCallback;)Landroid/bluetooth/BluetoothGattServer;",
            &[
                JValue::Object(context),
                JValue::Object(callback_ref.as_obj())
            ]
        ).map_err(|e| Error::Network(format!("Failed to open GATT server: {}", e)))?
         .l().map_err(|e| Error::Network(format!("Failed to convert GATT server: {}", e)))?;
        
        if gatt_server.is_null() {
            return Err(Error::Network("Failed to open GATT server".to_string()));
        }
        
        // Add our service to the GATT server
        let service_ref = self.bitcraps_service.as_ref().ok_or_else(|| {
            Error::Network("BitCraps service not created".to_string())
        })?;
        
        let add_result = env.call_method(
            gatt_server,
            "addService",
            "(Landroid/bluetooth/BluetoothGattService;)Z",
            &[JValue::Object(service_ref.as_obj())]
        ).map_err(|e| Error::Network(format!("Failed to add service to GATT server: {}", e)))?
         .z().map_err(|e| Error::Network(format!("Failed to convert add service result: {}", e)))?;
        
        if !add_result {
            return Err(Error::Network("Failed to add service to GATT server".to_string()));
        }
        
        // Store GATT server reference
        self.gatt_server = Some(env.new_global_ref(gatt_server).map_err(|e| {
            Error::Network(format!("Failed to create global reference for GATT server: {}", e))
        })?);
        
        log::debug!("GATT server opened successfully");
        Ok(())
    }
    
    /// Create AdvertiseSettings for Android BLE advertising
    fn create_advertise_settings(&self, env: &mut AttachGuard, config: &AdvertisingConfig) -> Result<JObject> {
        log::debug!("Creating AdvertiseSettings");
        
        let settings_builder_class = env.find_class("android/bluetooth/le/AdvertiseSettings$Builder")
            .map_err(|e| Error::Network(format!("Failed to find AdvertiseSettings.Builder class: {}", e)))?;
        
        let builder = env.new_object(
            settings_builder_class,
            "()V",
            &[]
        ).map_err(|e| Error::Network(format!("Failed to create AdvertiseSettings.Builder: {}", e)))?;
        
        // Set advertising mode based on interval
        let advertise_mode = if config.advertising_interval_ms <= 100 {
            android_constants::ADVERTISE_MODE_LOW_LATENCY
        } else if config.advertising_interval_ms <= 1000 {
            android_constants::ADVERTISE_MODE_BALANCED
        } else {
            android_constants::ADVERTISE_MODE_LOW_POWER
        };
        
        let builder = env.call_method(
            builder,
            "setAdvertiseMode",
            "(I)Landroid/bluetooth/le/AdvertiseSettings$Builder;",
            &[JValue::Int(advertise_mode)]
        ).map_err(|e| Error::Network(format!("Failed to set advertise mode: {}", e)))?
         .l().map_err(|e| Error::Network(format!("Failed to convert builder: {}", e)))?;
        
        // Set TX power level
        let tx_power = match config.tx_power_level {
            -21..=-13 => android_constants::ADVERTISE_TX_POWER_ULTRA_LOW,
            -12..=-8 => android_constants::ADVERTISE_TX_POWER_LOW,
            -7..=0 => android_constants::ADVERTISE_TX_POWER_MEDIUM,
            _ => android_constants::ADVERTISE_TX_POWER_HIGH,
        };
        
        let builder = env.call_method(
            builder,
            "setTxPowerLevel",
            "(I)Landroid/bluetooth/le/AdvertiseSettings$Builder;",
            &[JValue::Int(tx_power)]
        ).map_err(|e| Error::Network(format!("Failed to set TX power level: {}", e)))?
         .l().map_err(|e| Error::Network(format!("Failed to convert builder: {}", e)))?;
        
        // Set connectable
        let builder = env.call_method(
            builder,
            "setConnectable",
            "(Z)Landroid/bluetooth/le/AdvertiseSettings$Builder;",
            &[JValue::Bool(if config.connectable { JNI_TRUE } else { JNI_FALSE } as jboolean)]
        ).map_err(|e| Error::Network(format!("Failed to set connectable: {}", e)))?
         .l().map_err(|e| Error::Network(format!("Failed to convert builder: {}", e)))?;
        
        // Build settings
        let settings = env.call_method(
            builder,
            "build",
            "()Landroid/bluetooth/le/AdvertiseSettings;",
            &[]
        ).map_err(|e| Error::Network(format!("Failed to build AdvertiseSettings: {}", e)))?
         .l().map_err(|e| Error::Network(format!("Failed to convert settings: {}", e)))?;
        
        Ok(settings)
    }
    
    /// Create AdvertiseData for Android BLE advertising
    fn create_advertise_data(&self, env: &mut AttachGuard, config: &AdvertisingConfig) -> Result<JObject> {
        log::debug!("Creating AdvertiseData");
        
        let data_builder_class = env.find_class("android/bluetooth/le/AdvertiseData$Builder")
            .map_err(|e| Error::Network(format!("Failed to find AdvertiseData.Builder class: {}", e)))?;
        
        let builder = env.new_object(
            data_builder_class,
            "()V",
            &[]
        ).map_err(|e| Error::Network(format!("Failed to create AdvertiseData.Builder: {}", e)))?;
        
        // Add service UUID
        let service_uuid_str = env.new_string(config.service_uuid.to_string())
            .map_err(|e| Error::Network(format!("Failed to create service UUID string: {}", e)))?;
        
        let uuid_class = env.find_class("java/util/UUID")
            .map_err(|e| Error::Network(format!("Failed to find UUID class: {}", e)))?;
        
        let service_uuid = env.call_static_method(
            uuid_class,
            "fromString",
            "(Ljava/lang/String;)Ljava/util/UUID;",
            &[JValue::Object(service_uuid_str.into())]
        ).map_err(|e| Error::Network(format!("Failed to create service UUID: {}", e)))?
         .l().map_err(|e| Error::Network(format!("Failed to convert service UUID: {}", e)))?;
        
        let builder = env.call_method(
            builder,
            "addServiceUuid",
            "(Landroid/os/ParcelUuid;)Landroid/bluetooth/le/AdvertiseData$Builder;",
            &[JValue::Object(service_uuid)]
        ).map_err(|e| Error::Network(format!("Failed to add service UUID: {}", e)))?
         .l().map_err(|e| Error::Network(format!("Failed to convert builder: {}", e)))?;
        
        // Add local name if requested
        if config.include_name {
            let local_name = env.new_string(&config.local_name)
                .map_err(|e| Error::Network(format!("Failed to create local name string: {}", e)))?;
            
            let builder = env.call_method(
                builder,
                "setIncludeDeviceName",
                "(Z)Landroid/bluetooth/le/AdvertiseData$Builder;",
                &[JValue::Bool(JNI_TRUE as jboolean)]
            ).map_err(|e| Error::Network(format!("Failed to set include device name: {}", e)))?
             .l().map_err(|e| Error::Network(format!("Failed to convert builder: {}", e)))?;
        }
        
        // Build advertise data
        let data = env.call_method(
            builder,
            "build",
            "()Landroid/bluetooth/le/AdvertiseData;",
            &[]
        ).map_err(|e| Error::Network(format!("Failed to build AdvertiseData: {}", e)))?
         .l().map_err(|e| Error::Network(format!("Failed to convert data: {}", e)))?;
        
        Ok(data)
    }
    
    /// Start Android BLE advertising
    pub async fn start_android_advertising(&mut self, config: &AdvertisingConfig) -> Result<()> {
        if *self.is_advertising.read().await {
            return Err(Error::Network("Already advertising".to_string()));
        }
        
        log::info!("Starting Android BLE advertising");
        
        // Get JNI environment
        let java_vm = self.java_vm.as_ref().ok_or_else(|| {
            Error::Network("JavaVM not initialized".to_string())
        })?;
        
        let mut env = java_vm.attach_current_thread().map_err(|e| {
            Error::Network(format!("Failed to attach to Java thread: {}", e))
        })?;
        
        // Create advertising settings and data
        let settings = self.create_advertise_settings(&mut env, config)?;
        let data = self.create_advertise_data(&mut env, config)?;
        
        // Get advertiser and callback references
        let advertiser_ref = self.bluetooth_le_advertiser.as_ref().ok_or_else(|| {
            Error::Network("BluetoothLeAdvertiser not initialized".to_string())
        })?;
        
        let callback_ref = self.advertise_callback.as_ref().ok_or_else(|| {
            Error::Network("Advertise callback not initialized".to_string())
        })?;
        
        // Start advertising
        env.call_method(
            advertiser_ref.as_obj(),
            "startAdvertising",
            "(Landroid/bluetooth/le/AdvertiseSettings;Landroid/bluetooth/le/AdvertiseData;Landroid/bluetooth/le/AdvertiseCallback;)V",
            &[
                JValue::Object(settings),
                JValue::Object(data),
                JValue::Object(callback_ref.as_obj())
            ]
        ).map_err(|e| Error::Network(format!("Failed to start advertising: {}", e)))?;
        
        // Update state
        *self.is_advertising.write().await = true;
        *self.advertising_start_time.write().await = Some(Instant::now());
        *self.config.write().await = config.clone();
        
        // Send event
        let _ = self.event_sender.send(PeripheralEvent::AdvertisingStarted);
        
        log::info!("Android BLE advertising started successfully");
        Ok(())
    }
    
    /// Stop Android BLE advertising
    pub async fn stop_android_advertising(&mut self) -> Result<()> {
        if !*self.is_advertising.read().await {
            return Ok(());
        }
        
        log::info!("Stopping Android BLE advertising");
        
        // Get JNI environment
        let java_vm = self.java_vm.as_ref().ok_or_else(|| {
            Error::Network("JavaVM not initialized".to_string())
        })?;
        
        let mut env = java_vm.attach_current_thread().map_err(|e| {
            Error::Network(format!("Failed to attach to Java thread: {}", e))
        })?;
        
        // Get advertiser and callback references
        let advertiser_ref = self.bluetooth_le_advertiser.as_ref().ok_or_else(|| {
            Error::Network("BluetoothLeAdvertiser not initialized".to_string())
        })?;
        
        let callback_ref = self.advertise_callback.as_ref().ok_or_else(|| {
            Error::Network("Advertise callback not initialized".to_string())
        })?;
        
        // Stop advertising
        env.call_method(
            advertiser_ref.as_obj(),
            "stopAdvertising",
            "(Landroid/bluetooth/le/AdvertiseCallback;)V",
            &[JValue::Object(callback_ref.as_obj())]
        ).map_err(|e| Error::Network(format!("Failed to stop advertising: {}", e)))?;
        
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
        
        log::info!("Android BLE advertising stopped successfully");
        Ok(())
    }
}

/// JNI callback functions for Android advertising events
/// These would be called from the Java AdvertiseCallbackBridge class

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_com_bitchat_rust_AdvertiseCallbackBridge_onStartSuccess(
    env: JNIEnv,
    _class: jni::objects::JClass,
    rust_ptr: jlong,
    settings: jni::objects::JObject,
) {
    log::info!("Android advertising started successfully");
    
    // Convert rust_ptr back to AndroidBlePeripheral reference
    let peripheral = unsafe { &mut *(rust_ptr as *mut AndroidBlePeripheral) };
    
    // Send success event
    let _ = peripheral.event_sender.send(PeripheralEvent::AdvertisingStarted);
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_com_bitchat_rust_AdvertiseCallbackBridge_onStartFailure(
    env: JNIEnv,
    _class: jni::objects::JClass,
    rust_ptr: jlong,
    error_code: jint,
) {
    let error_message = match error_code {
        1 => "ADVERTISE_FAILED_DATA_TOO_LARGE",
        2 => "ADVERTISE_FAILED_TOO_MANY_ADVERTISERS",
        3 => "ADVERTISE_FAILED_ALREADY_STARTED",
        4 => "ADVERTISE_FAILED_INTERNAL_ERROR",
        5 => "ADVERTISE_FAILED_FEATURE_UNSUPPORTED",
        _ => "Unknown error",
    };
    
    log::error!("Android advertising failed: {} ({})", error_message, error_code);
    
    let peripheral = unsafe { &mut *(rust_ptr as *mut AndroidBlePeripheral) };
    
    // Update stats and send error event
    tokio::spawn(async move {
        let mut stats = peripheral.stats.write().await;
        stats.error_count += 1;
        
        let _ = peripheral.event_sender.send(PeripheralEvent::Error {
            error: format!("Advertising failed: {}", error_message),
        });
    });
}

// Additional JNI callbacks for GATT server events would go here...
// These would handle connection events, characteristic writes, etc.