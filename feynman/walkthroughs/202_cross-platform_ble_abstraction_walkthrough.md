# Chapter 91: Cross-Platform BLE Abstraction - One API, Many Platforms

Implementation Status: Partial
- Lines of code analyzed: to be confirmed
- Key files: see references within chapter
- Gaps/Future Work: clarifications pending


## Introduction: The Tower of Babel Problem

In the biblical story of the Tower of Babel, humanity spoke one language until they tried to build a tower to heaven. As punishment, their language was confused, and they could no longer understand each other. Modern Bluetooth Low Energy development feels similar - iOS speaks CoreBluetooth, Android speaks its Bluetooth API, Windows speaks WinRT, and Linux speaks BlueZ. Each platform has its own way of doing the exact same thing.

Cross-platform BLE abstraction is the translator that lets your code speak to all these platforms in one language. It's the difference between writing your BLE logic once or rewriting it for every platform. In BitCraps, where we need to run on phones, tablets, laptops, and embedded devices, a good abstraction layer isn't just convenient - it's essential for maintainability and sanity.

This chapter explores how to build a unified BLE API that works everywhere while still exposing platform-specific capabilities when needed. We'll cover the art of finding common ground between wildly different APIs, handling platform limitations gracefully, and building abstractions that don't leak.

## The Platform Landscape: Understanding the Differences

Before we can abstract, we need to understand what we're abstracting:

### iOS/macOS: CoreBluetooth
- Objective-C/Swift API
- Central and Peripheral roles
- Background execution with restrictions
- Requires Info.plist configuration

### Android: Android Bluetooth
- Java/Kotlin API
- Supports all roles
- Complex permission model
- Manufacturer-specific quirks

### Windows: WinRT Bluetooth
- C++/C# API
- Limited peripheral support
- UWP sandboxing
- Requires package manifest

### Linux: BlueZ
- D-Bus API
- Most feature-complete
- Root/sudo often required
- Different versions have different capabilities

## The Abstraction Layer Architecture

Here's how to build a cross-platform BLE abstraction:

```rust
// Platform-agnostic BLE interface
pub trait BluetoothAdapter: Send + Sync {
    type Central: BluetoothCentral;
    type Peripheral: BluetoothPeripheral;
    type Error: std::error::Error;
    
    /// Initialize the adapter
    async fn initialize(&mut self) -> Result<(), Self::Error>;
    
    /// Check if Bluetooth is available and powered on
    async fn is_available(&self) -> Result<bool, Self::Error>;
    
    /// Get adapter state
    async fn state(&self) -> Result<AdapterState, Self::Error>;
    
    /// Request necessary permissions (mobile platforms)
    async fn request_permissions(&self) -> Result<PermissionStatus, Self::Error>;
    
    /// Create a central manager for scanning/connecting
    async fn create_central(&self) -> Result<Self::Central, Self::Error>;
    
    /// Create a peripheral manager for advertising
    async fn create_peripheral(&self) -> Result<Self::Peripheral, Self::Error>;
    
    /// Platform-specific capabilities
    fn capabilities(&self) -> PlatformCapabilities;
}

#[derive(Debug, Clone)]
pub struct PlatformCapabilities {
    pub central_role: bool,
    pub peripheral_role: bool,
    pub simultaneous_roles: bool,
    pub extended_advertising: bool,
    pub long_range: bool,
    pub coded_phy: bool,
    pub max_connections: usize,
    pub background_execution: BackgroundSupport,
}

#[derive(Debug, Clone)]
pub enum BackgroundSupport {
    Full,
    Limited { restrictions: Vec<String> },
    None,
}

// Platform detection and factory
pub struct BluetoothManager {
    #[cfg(target_os = "ios")]
    adapter: IosBleAdapter,
    
    #[cfg(target_os = "macos")]
    adapter: MacOsBleAdapter,
    
    #[cfg(target_os = "android")]
    adapter: AndroidBleAdapter,
    
    #[cfg(target_os = "windows")]
    adapter: WindowsBleAdapter,
    
    #[cfg(target_os = "linux")]
    adapter: LinuxBleAdapter,
    
    #[cfg(not(any(
        target_os = "ios",
        target_os = "macos", 
        target_os = "android",
        target_os = "windows",
        target_os = "linux"
    )))]
    adapter: MockBleAdapter, // For testing
}

impl BluetoothManager {
    pub fn new() -> Result<Self, BleError> {
        #[cfg(target_os = "ios")]
        let adapter = IosBleAdapter::new()?;
        
        #[cfg(target_os = "android")]
        let adapter = AndroidBleAdapter::new()?;
        
        // ... other platforms
        
        Ok(Self { adapter })
    }
    
    pub fn adapter(&self) -> &dyn BluetoothAdapter {
        &self.adapter
    }
}
```

## iOS Implementation: CoreBluetooth Bridge

Bridging to iOS requires careful handling of Objective-C:

```rust
#[cfg(target_os = "ios")]
mod ios {
    use objc::{msg_send, sel, sel_impl};
    use objc::runtime::{Object, Class};
    use objc_foundation::{INSObject, NSObject, NSString};
    use objc_id::{Id, Owned};
    use core_bluetooth_sys::*;
    
    pub struct IosBleAdapter {
        central_manager: Id<CBCentralManager>,
        peripheral_manager: Id<CBPeripheralManager>,
        delegate: Id<BleDelegate>,
        state: Arc<Mutex<AdapterState>>,
    }
    
    impl IosBleAdapter {
        pub fn new() -> Result<Self, BleError> {
            unsafe {
                // Create delegate for callbacks
                let delegate_class = create_delegate_class();
                let delegate: Id<BleDelegate> = msg_send![delegate_class, new];
                
                // Create central manager
                let central_manager: Id<CBCentralManager> = msg_send![
                    class!(CBCentralManager),
                    alloc
                ];
                let central_manager: Id<CBCentralManager> = msg_send![
                    central_manager,
                    initWithDelegate:delegate.clone()
                    queue:dispatch_get_main_queue()
                    options:nil
                ];
                
                // Create peripheral manager
                let peripheral_manager: Id<CBPeripheralManager> = msg_send![
                    class!(CBPeripheralManager),
                    alloc
                ];
                let peripheral_manager: Id<CBPeripheralManager> = msg_send![
                    peripheral_manager,
                    initWithDelegate:delegate.clone()
                    queue:dispatch_get_main_queue()
                    options:nil
                ];
                
                Ok(Self {
                    central_manager,
                    peripheral_manager,
                    delegate,
                    state: Arc::new(Mutex::new(AdapterState::Unknown)),
                })
            }
        }
        
        fn handle_state_update(&self, state: CBManagerState) {
            let adapter_state = match state {
                CBManagerStatePoweredOn => AdapterState::PoweredOn,
                CBManagerStatePoweredOff => AdapterState::PoweredOff,
                CBManagerStateUnauthorized => AdapterState::Unauthorized,
                CBManagerStateUnsupported => AdapterState::Unsupported,
                _ => AdapterState::Unknown,
            };
            
            *self.state.lock().unwrap() = adapter_state;
        }
    }
    
    impl BluetoothAdapter for IosBleAdapter {
        type Central = IosCentral;
        type Peripheral = IosPeripheral;
        type Error = IosError;
        
        async fn is_available(&self) -> Result<bool, Self::Error> {
            let state = unsafe {
                msg_send![self.central_manager, state]
            };
            
            Ok(state == CBManagerStatePoweredOn)
        }
        
        fn capabilities(&self) -> PlatformCapabilities {
            PlatformCapabilities {
                central_role: true,
                peripheral_role: true,
                simultaneous_roles: true,
                extended_advertising: false, // iOS doesn't support
                long_range: false,
                coded_phy: false,
                max_connections: 8, // iOS limit
                background_execution: BackgroundSupport::Limited {
                    restrictions: vec![
                        "Must have specific background mode".to_string(),
                        "Limited to specific services in background".to_string(),
                        "No local name in background advertisements".to_string(),
                    ],
                },
            }
        }
    }
}
```

## Android Implementation: JNI Bridge

Android requires JNI to bridge between Rust and Java:

```rust
#[cfg(target_os = "android")]
mod android {
    use jni::{JNIEnv, JavaVM, objects::{JObject, JClass, JString}};
    use jni::sys::{jint, jboolean};
    
    pub struct AndroidBleAdapter {
        vm: JavaVM,
        adapter_obj: JObject<'static>,
        context: JObject<'static>,
    }
    
    impl AndroidBleAdapter {
        pub fn new() -> Result<Self, BleError> {
            let vm = get_java_vm()?;
            let env = vm.attach_current_thread()?;
            
            // Get application context
            let context = get_application_context(&env)?;
            
            // Get Bluetooth adapter
            let adapter_class = env.find_class("android/bluetooth/BluetoothAdapter")?;
            let get_default = env.get_static_method_id(
                adapter_class,
                "getDefaultAdapter",
                "()Landroid/bluetooth/BluetoothAdapter;"
            )?;
            
            let adapter_obj = env.call_static_method_unchecked(
                adapter_class,
                get_default,
                ReturnType::Object,
                &[]
            )?.l()?;
            
            Ok(Self {
                vm,
                adapter_obj,
                context,
            })
        }
        
        fn check_permissions(&self) -> Result<bool, BleError> {
            let env = self.vm.attach_current_thread()?;
            
            // Check for BLUETOOTH_CONNECT permission (Android 12+)
            let permission_class = env.find_class("android/Manifest$permission")?;
            let bluetooth_connect = env.get_static_field(
                permission_class,
                "BLUETOOTH_CONNECT",
                "Ljava/lang/String;"
            )?.l()?;
            
            let check_self_permission = env.get_method_id(
                self.context.class(),
                "checkSelfPermission",
                "(Ljava/lang/String;)I"
            )?;
            
            let result: jint = env.call_method(
                self.context,
                check_self_permission,
                ReturnType::Int,
                &[JValue::Object(bluetooth_connect)]
            )?.i()?;
            
            Ok(result == 0) // PERMISSION_GRANTED = 0
        }
    }
    
    impl BluetoothAdapter for AndroidBleAdapter {
        type Central = AndroidCentral;
        type Peripheral = AndroidPeripheral;
        type Error = AndroidError;
        
        async fn request_permissions(&self) -> Result<PermissionStatus, Self::Error> {
            if self.check_permissions()? {
                return Ok(PermissionStatus::Granted);
            }
            
            // Request permissions through activity
            let env = self.vm.attach_current_thread()?;
            
            // This needs to be called on the UI thread
            request_permissions_on_ui_thread(&env, &self.context).await
        }
        
        fn capabilities(&self) -> PlatformCapabilities {
            let env = self.vm.attach_current_thread().unwrap();
            
            // Check for peripheral mode support
            let is_multiple_advertisement_supported: jboolean = env.call_method(
                self.adapter_obj,
                "isMultipleAdvertisementSupported",
                "()Z",
                &[]
            ).unwrap().z().unwrap();
            
            PlatformCapabilities {
                central_role: true,
                peripheral_role: is_multiple_advertisement_supported != 0,
                simultaneous_roles: true,
                extended_advertising: self.check_extended_advertising_support(),
                long_range: true, // Bluetooth 5.0+
                coded_phy: true,
                max_connections: 32, // Android allows more connections
                background_execution: BackgroundSupport::Full,
            }
        }
    }
}
```

## Windows Implementation: WinRT Bridge

Windows requires WinRT API access:

```rust
#[cfg(target_os = "windows")]
mod windows {
    use windows::{
        Devices::Bluetooth::*,
        Devices::Bluetooth::Advertisement::*,
        Devices::Bluetooth::GenericAttributeProfile::*,
        Foundation::*,
        Storage::Streams::*,
    };
    
    pub struct WindowsBleAdapter {
        radio: BluetoothAdapter,
        watcher: BluetoothLEAdvertisementWatcher,
        publisher: BluetoothLEAdvertisementPublisher,
    }
    
    impl WindowsBleAdapter {
        pub async fn new() -> Result<Self, BleError> {
            // Get default Bluetooth adapter
            let radio = BluetoothAdapter::GetDefaultAsync()?.await?
                .ok_or(BleError::NoAdapter)?;
            
            // Check radio state
            if !radio.IsLowEnergySupported()? {
                return Err(BleError::BleNotSupported);
            }
            
            // Create advertisement watcher for scanning
            let watcher = BluetoothLEAdvertisementWatcher::new()?;
            watcher.SetScanningMode(BluetoothLEScanningMode::Active)?;
            
            // Create publisher for advertising
            let publisher = BluetoothLEAdvertisementPublisher::new()?;
            
            Ok(Self {
                radio,
                watcher,
                publisher,
            })
        }
    }
    
    impl BluetoothAdapter for WindowsBleAdapter {
        type Central = WindowsCentral;
        type Peripheral = WindowsPeripheral;
        type Error = WindowsError;
        
        async fn is_available(&self) -> Result<bool, Self::Error> {
            let state = self.radio.GetRadioAsync()?.await?;
            Ok(state?.State()? == RadioState::On)
        }
        
        fn capabilities(&self) -> PlatformCapabilities {
            PlatformCapabilities {
                central_role: true,
                peripheral_role: true, // Limited support
                simultaneous_roles: true,
                extended_advertising: false,
                long_range: false,
                coded_phy: false,
                max_connections: 16,
                background_execution: BackgroundSupport::Limited {
                    restrictions: vec![
                        "Requires foreground app".to_string(),
                        "Background tasks have time limits".to_string(),
                    ],
                },
            }
        }
    }
}
```

## Linux Implementation: BlueZ D-Bus

Linux uses D-Bus to communicate with BlueZ:

```rust
#[cfg(target_os = "linux")]
mod linux {
    use dbus::{Connection, BusType, Message};
    use dbus::arg::{Variant, Dict};
    
    pub struct LinuxBleAdapter {
        connection: Connection,
        adapter_path: String,
        object_manager: ObjectManager,
    }
    
    impl LinuxBleAdapter {
        pub fn new() -> Result<Self, BleError> {
            // Connect to system D-Bus
            let connection = Connection::get_private(BusType::System)?;
            
            // Find Bluetooth adapter
            let adapter_path = Self::find_adapter(&connection)?;
            
            // Create object manager for monitoring
            let object_manager = ObjectManager::new(&connection)?;
            
            Ok(Self {
                connection,
                adapter_path,
                object_manager,
            })
        }
        
        fn find_adapter(conn: &Connection) -> Result<String, BleError> {
            let msg = Message::new_method_call(
                "org.bluez",
                "/",
                "org.freedesktop.DBus.ObjectManager",
                "GetManagedObjects"
            )?;
            
            let reply = conn.send_with_reply_and_block(msg, 5000)?;
            
            // Parse reply to find adapter
            let objects: HashMap<String, HashMap<String, Variant>> = reply.read1()?;
            
            for (path, interfaces) in objects {
                if interfaces.contains_key("org.bluez.Adapter1") {
                    return Ok(path);
                }
            }
            
            Err(BleError::NoAdapter)
        }
        
        fn set_powered(&self, powered: bool) -> Result<(), BleError> {
            let msg = Message::new_method_call(
                "org.bluez",
                &self.adapter_path,
                "org.freedesktop.DBus.Properties",
                "Set"
            )?
            .append3("org.bluez.Adapter1", "Powered", Variant(powered));
            
            self.connection.send_with_reply_and_block(msg, 5000)?;
            Ok(())
        }
    }
    
    impl BluetoothAdapter for LinuxBleAdapter {
        type Central = LinuxCentral;
        type Peripheral = LinuxPeripheral;
        type Error = LinuxError;
        
        async fn initialize(&mut self) -> Result<(), Self::Error> {
            // Power on adapter
            self.set_powered(true)?;
            
            // Register with BlueZ
            self.register_application().await?;
            
            Ok(())
        }
        
        fn capabilities(&self) -> PlatformCapabilities {
            // Linux/BlueZ generally has the most complete BLE support
            PlatformCapabilities {
                central_role: true,
                peripheral_role: true,
                simultaneous_roles: true,
                extended_advertising: true,
                long_range: true,
                coded_phy: true,
                max_connections: 64, // Kernel dependent
                background_execution: BackgroundSupport::Full,
            }
        }
    }
}
```

## Unified Scanning and Discovery

Create a unified scanning API across platforms:

```rust
pub trait BluetoothCentral: Send + Sync {
    type Peripheral: RemotePeripheral;
    type Error: std::error::Error;
    
    /// Start scanning for peripherals
    async fn start_scan(&self, filter: Option<ScanFilter>) -> Result<(), Self::Error>;
    
    /// Stop scanning
    async fn stop_scan(&self) -> Result<(), Self::Error>;
    
    /// Get discovered peripherals
    async fn discovered_peripherals(&self) -> Result<Vec<Self::Peripheral>, Self::Error>;
    
    /// Connect to a peripheral
    async fn connect(&self, peripheral: &Self::Peripheral) -> Result<Connection, Self::Error>;
}

#[derive(Debug, Clone)]
pub struct ScanFilter {
    pub service_uuids: Vec<Uuid>,
    pub name_prefix: Option<String>,
    pub manufacturer_data: Option<(u16, Vec<u8>)>,
    pub rssi_threshold: Option<i8>,
}

// Cross-platform peripheral representation
pub trait RemotePeripheral: Send + Sync {
    fn id(&self) -> PeripheralId;
    fn name(&self) -> Option<String>;
    fn rssi(&self) -> Option<i8>;
    fn is_connectable(&self) -> bool;
    fn advertisement_data(&self) -> AdvertisementData;
    fn platform_specific(&self) -> PlatformSpecificData;
}

// Platform-specific data that might be needed
#[derive(Debug, Clone)]
pub enum PlatformSpecificData {
    #[cfg(target_os = "ios")]
    Ios {
        identifier: String,
        is_peripheral: bool,
    },
    
    #[cfg(target_os = "android")]
    Android {
        address: String,
        bond_state: BondState,
        device_type: DeviceType,
    },
    
    #[cfg(target_os = "windows")]
    Windows {
        bluetooth_address: u64,
        is_classic_supported: bool,
    },
    
    #[cfg(target_os = "linux")]
    Linux {
        dbus_path: String,
        adapter: String,
    },
    
    #[cfg(not(any(target_os = "ios", target_os = "android", target_os = "windows", target_os = "linux")))]
    Mock,
}
```

## Unified GATT Operations

Abstract GATT operations across platforms:

```rust
pub trait GattConnection: Send + Sync {
    type Error: std::error::Error;
    
    /// Discover all services
    async fn discover_services(&self) -> Result<Vec<GattService>, Self::Error>;
    
    /// Read a characteristic
    async fn read_characteristic(&self, char_id: CharacteristicId) -> Result<Vec<u8>, Self::Error>;
    
    /// Write to a characteristic
    async fn write_characteristic(
        &self,
        char_id: CharacteristicId,
        value: &[u8],
        write_type: WriteType,
    ) -> Result<(), Self::Error>;
    
    /// Subscribe to notifications
    async fn subscribe(&self, char_id: CharacteristicId) -> Result<NotificationStream, Self::Error>;
    
    /// Request MTU change
    async fn request_mtu(&self, mtu: u16) -> Result<u16, Self::Error>;
    
    /// Get connection parameters
    fn connection_parameters(&self) -> ConnectionParameters;
}

#[derive(Debug, Clone)]
pub enum WriteType {
    WithResponse,
    WithoutResponse,
    Signed,
}

pub struct NotificationStream {
    receiver: mpsc::UnboundedReceiver<Vec<u8>>,
    unsubscribe: Box<dyn FnOnce() + Send>,
}

impl Stream for NotificationStream {
    type Item = Vec<u8>;
    
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        self.receiver.poll_recv(cx)
    }
}

impl Drop for NotificationStream {
    fn drop(&mut self) {
        (self.unsubscribe)();
    }
}
```

## Error Handling Across Platforms

Unified error handling is crucial:

```rust
#[derive(Debug, thiserror::Error)]
pub enum BleError {
    #[error("Bluetooth not available")]
    NotAvailable,
    
    #[error("No Bluetooth adapter found")]
    NoAdapter,
    
    #[error("Bluetooth LE not supported")]
    BleNotSupported,
    
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    
    #[error("Operation not supported on this platform")]
    NotSupported,
    
    #[error("Platform error: {0}")]
    Platform(Box<dyn std::error::Error + Send + Sync>),
}

// Platform-specific error conversion
impl From<IosError> for BleError {
    fn from(err: IosError) -> Self {
        match err {
            IosError::Unauthorized => BleError::PermissionDenied("iOS Bluetooth unauthorized".into()),
            IosError::PoweredOff => BleError::NotAvailable,
            _ => BleError::Platform(Box::new(err)),
        }
    }
}

impl From<AndroidError> for BleError {
    fn from(err: AndroidError) -> Self {
        match err {
            AndroidError::MissingPermission(perm) => BleError::PermissionDenied(perm),
            AndroidError::BluetoothDisabled => BleError::NotAvailable,
            _ => BleError::Platform(Box::new(err)),
        }
    }
}
```

## Testing Across Platforms

Build a mock implementation for testing:

```rust
#[cfg(test)]
mod mock {
    use super::*;
    
    pub struct MockBleAdapter {
        state: Arc<Mutex<MockState>>,
        peripherals: Arc<Mutex<Vec<MockPeripheral>>>,
    }
    
    #[derive(Default)]
    struct MockState {
        powered_on: bool,
        scanning: bool,
        advertising: bool,
    }
    
    impl MockBleAdapter {
        pub fn new() -> Self {
            Self {
                state: Arc::new(Mutex::new(MockState::default())),
                peripherals: Arc::new(Mutex::new(Vec::new())),
            }
        }
        
        pub fn add_mock_peripheral(&self, peripheral: MockPeripheral) {
            self.peripherals.lock().unwrap().push(peripheral);
        }
        
        pub fn set_powered(&self, powered: bool) {
            self.state.lock().unwrap().powered_on = powered;
        }
    }
    
    impl BluetoothAdapter for MockBleAdapter {
        type Central = MockCentral;
        type Peripheral = MockPeripheral;
        type Error = MockError;
        
        async fn is_available(&self) -> Result<bool, Self::Error> {
            Ok(self.state.lock().unwrap().powered_on)
        }
        
        fn capabilities(&self) -> PlatformCapabilities {
            PlatformCapabilities {
                central_role: true,
                peripheral_role: true,
                simultaneous_roles: true,
                extended_advertising: true,
                long_range: true,
                coded_phy: true,
                max_connections: 100,
                background_execution: BackgroundSupport::Full,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_cross_platform_scan() {
        let adapter = MockBleAdapter::new();
        adapter.set_powered(true);
        
        // Add mock peripherals
        adapter.add_mock_peripheral(MockPeripheral {
            id: "test-device-1".into(),
            name: Some("BitCraps Node".into()),
            rssi: Some(-50),
            services: vec![GAME_SERVICE_UUID],
        });
        
        let central = adapter.create_central().await.unwrap();
        
        // Start scanning
        central.start_scan(Some(ScanFilter {
            service_uuids: vec![GAME_SERVICE_UUID],
            name_prefix: Some("BitCraps".into()),
            rssi_threshold: Some(-70),
            manufacturer_data: None,
        })).await.unwrap();
        
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        let peripherals = central.discovered_peripherals().await.unwrap();
        assert_eq!(peripherals.len(), 1);
        assert_eq!(peripherals[0].name(), Some("BitCraps Node".into()));
    }
}
```

## Platform Capability Detection and Fallbacks

Handle platform limitations gracefully:

```rust
pub struct CapabilityManager {
    capabilities: PlatformCapabilities,
    fallback_strategies: HashMap<Capability, FallbackStrategy>,
}

impl CapabilityManager {
    pub fn new(adapter: &dyn BluetoothAdapter) -> Self {
        let capabilities = adapter.capabilities();
        let mut fallback_strategies = HashMap::new();
        
        // Define fallbacks for missing capabilities
        if !capabilities.peripheral_role {
            fallback_strategies.insert(
                Capability::PeripheralRole,
                FallbackStrategy::UseWifiDirect,
            );
        }
        
        if !capabilities.extended_advertising {
            fallback_strategies.insert(
                Capability::ExtendedAdvertising,
                FallbackStrategy::UseMultipleAdvertisements,
            );
        }
        
        if matches!(capabilities.background_execution, BackgroundSupport::None) {
            fallback_strategies.insert(
                Capability::BackgroundExecution,
                FallbackStrategy::UseForegroundService,
            );
        }
        
        Self {
            capabilities,
            fallback_strategies,
        }
    }
    
    pub fn can_do(&self, capability: Capability) -> bool {
        match capability {
            Capability::PeripheralRole => self.capabilities.peripheral_role,
            Capability::ExtendedAdvertising => self.capabilities.extended_advertising,
            Capability::SimultaneousRoles => self.capabilities.simultaneous_roles,
            _ => false,
        }
    }
    
    pub fn get_fallback(&self, capability: Capability) -> Option<&FallbackStrategy> {
        self.fallback_strategies.get(&capability)
    }
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum Capability {
    PeripheralRole,
    ExtendedAdvertising,
    BackgroundExecution,
    SimultaneousRoles,
    LongRange,
}

#[derive(Debug, Clone)]
pub enum FallbackStrategy {
    UseWifiDirect,
    UseMultipleAdvertisements,
    UseForegroundService,
    Disable,
}
```

## Practical Exercises

### Exercise 1: Implement Platform Feature Detection
Build runtime feature detection:

```rust
pub struct FeatureDetector {
    // Your implementation
}

impl FeatureDetector {
    pub async fn detect_features(&self) -> PlatformFeatures {
        // Your task: Detect available BLE features at runtime
        // Check OS version
        // Test capabilities
        // Return comprehensive feature set
        todo!("Implement feature detection")
    }
}
```

### Exercise 2: Build Connection Retry Logic
Implement cross-platform connection management:

```rust
pub struct ConnectionManager {
    // Your implementation
}

impl ConnectionManager {
    pub async fn connect_with_retry(&self, peripheral_id: &str) -> Result<Connection, Error> {
        // Your task: Implement connection with:
        // - Platform-specific retry strategies
        // - Exponential backoff
        // - Connection parameter negotiation
        todo!("Implement connection retry")
    }
}
```

### Exercise 3: Create Platform Shim
Build a shim for platform differences:

```rust
pub trait PlatformShim {
    fn normalize_uuid(&self, uuid: &str) -> Uuid;
    fn normalize_address(&self, address: &str) -> String;
    fn convert_rssi(&self, raw_rssi: i32) -> i8;
}

impl PlatformShim for IosShim {
    // Your task: Implement iOS-specific normalizations
    todo!("Implement iOS shim")
}
```

## Common Pitfalls and Solutions

### 1. Platform API Changes
APIs change between OS versions:

```rust
// Bad: Assume API availability
let extended_adv = adapter.create_extended_advertisement();

// Good: Check capability first
if adapter.capabilities().extended_advertising {
    let extended_adv = adapter.create_extended_advertisement();
} else {
    // Use fallback
    let regular_adv = adapter.create_advertisement();
}
```

### 2. Threading Models Differ
Each platform has different threading requirements:

```rust
// iOS: Must use main queue for UI operations
#[cfg(target_os = "ios")]
dispatch_async(dispatch_get_main_queue(), || {
    // UI updates
});

// Android: UI thread for permissions
#[cfg(target_os = "android")]
run_on_ui_thread(|| {
    // Permission requests
});
```

### 3. Permission Timing
Permission requests must be handled correctly:

```rust
// Bad: Assume permissions granted
adapter.start_scan();

// Good: Request and verify
match adapter.request_permissions().await {
    Ok(PermissionStatus::Granted) => adapter.start_scan(),
    Ok(PermissionStatus::Denied) => show_permission_rationale(),
    Err(e) => handle_permission_error(e),
}
```

## Conclusion: Write Once, Run Everywhere (Almost)

Cross-platform BLE abstraction is about finding the common ground between wildly different platforms while still exposing their unique capabilities. It's the art of building APIs that feel native on every platform while hiding the complexity of platform-specific implementations.

In BitCraps, this abstraction layer is what allows us to focus on game logic rather than wrestling with platform differences. Whether running on an iPhone in California or a Linux server in Tokyo, the same BLE code just works.

Key principles to remember:

1. **Abstract the common, expose the specific** - Hide what's the same, make differences explicit
2. **Fail gracefully** - Not all platforms support all features
3. **Test on real devices** - Simulators lie about BLE capabilities
4. **Version defensively** - APIs change, be prepared
5. **Document platform quirks** - Future you will thank present you

The perfect abstraction doesn't hide differences - it makes them manageable.

## Additional Resources

- **UniFFI** - Mozilla's tool for building cross-language interfaces
- **Flutter Platform Channels** - How Flutter handles platform differences
- **React Native Modules** - Cross-platform native module development
- **rust-windowing** - Example of successful cross-platform abstraction

Remember: The best cross-platform code is platform-aware code that chooses not to care about the differences.
