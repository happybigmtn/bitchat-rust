# BitCraps Mobile Implementation Guide v2.0
## Incorporating Senior Engineer Feedback

This guide addresses all critical platform-specific requirements and corrections identified in the senior engineering review. It serves as the technical implementation reference for Android and iOS development.

---

## Critical Changes from Review

### ⚠️ MUST FIX IMMEDIATELY

1. **Android Permissions** - Missing BLUETOOTH_ADVERTISE, incorrect permission model
2. **Android Foreground Services** - Missing service type declaration (crash/rejection risk)
3. **iOS Info.plist** - Using deprecated keys
4. **JNI Pattern** - Thread-unsafe JNIEnv storage
5. **UniFFI Pattern** - Mixed approaches causing confusion
6. **Testing Strategy** - Unrealistic device farm expectations

---

## Android Implementation (Corrected)

### 1. Permissions Configuration

#### AndroidManifest.xml (CORRECTED)
```xml
<!-- Legacy permissions for Android 11 and below (API ≤30) -->
<uses-permission android:name="android.permission.BLUETOOTH" 
                 android:maxSdkVersion="30" />
<uses-permission android:name="android.permission.BLUETOOTH_ADMIN" 
                 android:maxSdkVersion="30" />

<!-- Location for BLE scanning on older devices -->
<uses-permission android:name="android.permission.ACCESS_FINE_LOCATION" 
                 android:maxSdkVersion="30" />

<!-- Modern permissions for Android 12+ (API 31+) -->
<!-- These are runtime permissions under "Nearby devices" -->
<uses-permission android:name="android.permission.BLUETOOTH_SCAN" />
<uses-permission android:name="android.permission.BLUETOOTH_CONNECT" />
<uses-permission android:name="android.permission.BLUETOOTH_ADVERTISE" /> <!-- CRITICAL: Was missing -->

<!-- Foreground Service permissions for Android 14+ -->
<uses-permission android:name="android.permission.FOREGROUND_SERVICE" />
<uses-permission android:name="android.permission.FOREGROUND_SERVICE_CONNECTED_DEVICE" />

<!-- Declare BLE hardware requirement -->
<uses-feature android:name="android.hardware.bluetooth_le" 
              android:required="true" />
```

#### Runtime Permission Request (Kotlin)
```kotlin
class PermissionManager(private val activity: Activity) {
    
    fun requestBlePermissions() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            // Android 12+ - Request "Nearby devices" permissions
            val permissions = arrayOf(
                Manifest.permission.BLUETOOTH_SCAN,
                Manifest.permission.BLUETOOTH_CONNECT,
                Manifest.permission.BLUETOOTH_ADVERTISE  // CRITICAL: Include advertising
            )
            
            // Check and request as a group
            if (!hasAllPermissions(permissions)) {
                activity.requestPermissions(permissions, REQUEST_BLE_PERMISSIONS)
            }
        } else {
            // Android 11 and below - Request location
            val permission = Manifest.permission.ACCESS_FINE_LOCATION
            if (!hasPermission(permission)) {
                activity.requestPermissions(arrayOf(permission), REQUEST_LOCATION)
            }
        }
    }
}
```

### 2. Foreground Service Implementation (CRITICAL)

#### Service Declaration in AndroidManifest.xml
```xml
<service 
    android:name=".BluetoothMeshService"
    android:foregroundServiceType="connectedDevice"
    android:exported="false">
    <!-- CRITICAL: foregroundServiceType is mandatory on Android 14+ -->
</service>
```

#### BluetoothMeshService.kt (CORRECTED)
```kotlin
class BluetoothMeshService : Service() {
    
    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        // CRITICAL: Must start foreground within 5 seconds on Android 12+
        startForegroundService()
        return START_STICKY
    }
    
    private fun startForegroundService() {
        val notification = createNotification()
        
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            // Android 14+ requires service type
            startForeground(
                NOTIFICATION_ID, 
                notification,
                ServiceInfo.FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE
            )
        } else {
            startForeground(NOTIFICATION_ID, notification)
        }
    }
    
    private fun createNotification(): Notification {
        // User-visible notification is MANDATORY for foreground service
        return NotificationCompat.Builder(this, CHANNEL_ID)
            .setContentTitle("BitCraps Mesh Active")
            .setContentText("Connected to nearby players")
            .setSmallIcon(R.drawable.ic_dice)
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .setOngoing(true)  // Cannot be dismissed
            .build()
    }
}
```

### 3. JNI Pattern (THREAD-SAFE)

#### CORRECTED Rust Implementation
```rust
// WRONG - DO NOT STORE JNIEnv
// pub struct AndroidEventHandler {
//     jni_env: Arc<Mutex<JNIEnv<'static>>>, // UNSAFE!
// }

// CORRECT - Store JavaVM instead
pub struct AndroidEventHandler {
    jvm: JavaVM,
    callback: GlobalRef,
}

impl AndroidEventHandler {
    pub fn new(env: &JNIEnv, callback: JObject) -> Result<Self> {
        let jvm = env.get_java_vm()?;
        let callback = env.new_global_ref(callback)?;
        Ok(Self { jvm, callback })
    }
    
    pub fn handle_event(&self, event: GameEvent) -> Result<()> {
        // CORRECT: Attach to current thread for each callback
        let mut env = self.jvm.attach_current_thread()?;
        
        // Now safe to use env for this callback
        let event_json = serde_json::to_string(&event)?;
        let jstring = env.new_string(event_json)?;
        
        env.call_method(
            &self.callback,
            "onGameEvent",
            "(Ljava/lang/String;)V",
            &[jstring.into()],
        )?;
        
        Ok(())
        // env automatically detaches when dropped
    }
}
```

### 4. btleplug Android Integration

#### Gradle Dependencies (build.gradle)
```gradle
dependencies {
    // btleplug Java components - REQUIRED
    implementation project(':droidplug')
    implementation 'com.github.deviceplug:jni-utils-rs:0.1.0'
    
    // Your other dependencies
    implementation 'androidx.compose.ui:ui:1.5.4'
    // ...
}
```

#### Build Steps
```bash
# 1. Build btleplug Java module
cd btleplug/java
./gradlew assembleRelease

# 2. Copy to Android project
cp build/outputs/aar/droidplug-release.aar ../android/app/libs/

# 3. Build Rust library with cargo-ndk
cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 \
    -o ../android/app/src/main/jniLibs build --release
```

---

## iOS Implementation (Corrected)

### 1. Info.plist Configuration (CORRECTED)

```xml
<!-- CORRECT: Only use the current key -->
<key>NSBluetoothAlwaysUsageDescription</key>
<string>BitCraps uses Bluetooth to connect with nearby players for games.</string>

<!-- REMOVED: This is deprecated, do not include -->
<!-- <key>NSBluetoothPeripheralUsageDescription</key> -->

<!-- Only include if you ACTUALLY need background operation -->
<!-- Most games should work foreground-only -->
<key>UIBackgroundModes</key>
<array>
    <!-- Only if scanning in background -->
    <string>bluetooth-central</string>
    <!-- Only if advertising in background (rare) -->
    <!-- <string>bluetooth-peripheral</string> -->
</array>
```

### 2. Background BLE Constraints

#### Critical iOS Background Limitations
```swift
// REALITY CHECK: iOS background BLE is heavily restricted

class iOSBluetoothManager {
    
    func configureForBackground() {
        // 1. Local name is NOT advertised in background
        // 2. Service UUIDs move to "overflow area"
        // 3. Scan results are coalesced (no duplicate callbacks)
        
        // SOLUTION: Use service UUID filtering
        let serviceUUID = CBUUID(string: "12345678-1234-5678-1234-567812345678")
        
        // Scanning: MUST specify service UUIDs for background
        centralManager.scanForPeripherals(
            withServices: [serviceUUID],  // REQUIRED for background
            options: [
                CBCentralManagerScanOptionAllowDuplicatesKey: false
                // Duplicates are always coalesced in background anyway
            ]
        )
        
        // Advertising: Service UUIDs go to overflow
        let advertisementData = [
            CBAdvertisementDataServiceUUIDsKey: [serviceUUID],
            // Local name won't work in background
            // CBAdvertisementDataLocalNameKey: "BitCraps" // IGNORED
        ]
    }
    
    func handleBackgroundDiscovery() {
        // Design around these constraints:
        // - No local names visible
        // - Must connect to get full info
        // - Service UUID is your only filter
        
        // Exchange identity AFTER connection
        peripheral.discoverServices([serviceUUID])
        // Then read characteristics for player info
    }
}
```

### 3. Swift Package Manager Distribution

#### Package.swift for XCFramework
```swift
// swift-tools-version:5.7
import PackageDescription

let package = Package(
    name: "BitcrapsCore",
    platforms: [.iOS(.v14)],
    products: [
        .library(
            name: "BitcrapsCore",
            targets: ["BitcrapsCore", "BitcrapsFramework"]
        ),
    ],
    targets: [
        .target(
            name: "BitcrapsCore",
            dependencies: ["BitcrapsFramework"]
        ),
        .binaryTarget(
            name: "BitcrapsFramework",
            path: "BitcrapsCore.xcframework"
        )
    ]
)
```

---

## UniFFI Implementation (CORRECTED)

### Use UDL as Single Source of Truth

#### bitcraps.udl (DEFINITIVE)
```idl
namespace bitcraps {
    // Initialize the Rust runtime (call once)
    [Throws=BitcrapsError]
    BitcrapsNode create_node(BitcrapsConfig config);
};

// Main node interface
interface BitcrapsNode {
    // Async methods for proper threading
    [Throws=BitcrapsError, Async]
    void start_discovery();
    
    [Throws=BitcrapsError, Async]
    GameHandle create_game(GameConfig config);
    
    [Throws=BitcrapsError, Async]
    GameHandle join_game(string game_id);
    
    // Modern event pattern - NO callback interfaces
    [Async]
    GameEvent? poll_event();  // Returns null if no events
    
    // Or use async stream (better)
    [Async]
    sequence<GameEvent> get_event_stream();
};

// Game handle for active games
interface GameHandle {
    [Throws=BitcrapsError, Async]
    void place_bet(BetType bet_type, u64 amount);
    
    [Throws=BitcrapsError, Async]
    void roll_dice();
    
    [Async]
    GameState get_state();
};

// Data types
dictionary BitcrapsConfig {
    string data_dir;
    u32 pow_difficulty;
    boolean enable_mesh;
};

[Enum]
interface GameEvent {
    PeerDiscovered(PeerId peer_id);
    GameStarted(string game_id);
    DiceRolled(u8 dice1, u8 dice2);
    BetPlaced(PeerId player, u64 amount);
    GameEnded(string winner_id);
};
```

#### Rust Implementation (NO mixed patterns)
```rust
// CORRECT: Implement the UDL interface exactly
pub struct BitcrapsNode {
    runtime: Arc<Runtime>,
    mesh: Arc<MeshService>,
    events: Arc<Mutex<VecDeque<GameEvent>>>,
}

#[uniffi::export]
impl BitcrapsNode {
    // Async method implementation
    pub async fn start_discovery(&self) -> Result<(), BitcrapsError> {
        self.mesh.start_discovery().await
            .map_err(|e| BitcrapsError::Network(e.to_string()))
    }
    
    // Modern event pattern - polling
    pub async fn poll_event(&self) -> Option<GameEvent> {
        self.events.lock().unwrap().pop_front()
    }
    
    // Or async stream (better for UI)
    pub async fn get_event_stream(&self) -> Vec<GameEvent> {
        // In practice, implement as async stream
        // UniFFI will convert to Flow (Kotlin) / AsyncSequence (Swift)
        self.events.lock().unwrap().drain(..).collect()
    }
}

// DO NOT mix with manual #[export] or callback traits
```

---

## Testing Strategy (REALITY-BASED)

### Physical Device Testing Matrix

#### On-Bench Hardware Setup (REQUIRED)
```yaml
# Real devices needed for BLE testing
android_devices:
  - Pixel 6 (API 34)      # Latest Android
  - Samsung S21 (API 33)  # Popular OEM
  - OnePlus 8 (API 31)    # Android 12 baseline
  - Xiaomi Mi 11 (API 30) # Pre-12 permissions
  - Pixel 3a (API 29)     # Older but common

ios_devices:
  - iPhone 14 (iOS 17)    # Latest
  - iPhone 12 (iOS 16)    # Common
  - iPhone SE 2 (iOS 15)  # Budget/older
  - iPad Air (iOS 16)     # Tablet

# Cloud device farms - UI ONLY (no BLE)
ui_testing:
  - AWS Device Farm       # UI automation only
  - Firebase Test Lab     # UI and performance
  - BrowserStack          # Manual UI testing
```

#### Testing Scripts
```bash
#!/bin/bash
# Physical device test orchestration

# 1. Install on all devices
adb devices | while read device; do
    adb -s $device install app-debug.apk
done

# 2. Run BLE discovery test
./run_discovery_test.sh

# 3. Cross-platform pairing
# - Start iOS app in peripheral mode
# - Android scans and connects
# - Verify bidirectional communication

# 4. Background behavior
# - iOS: Background with bluetooth-central
# - Android: Foreground Service active
# - Verify discovery works
```

---

## Two-Week Sprint Plan (CORRECTED)

### Week 1: Platform Spikes

#### Day 1-3: Android Spike
- [ ] Fix AndroidManifest.xml permissions (add BLUETOOTH_ADVERTISE)
- [ ] Implement Foreground Service with connectedDevice type
- [ ] Fix JNI to use JavaVM, not JNIEnv
- [ ] Build btleplug with droidplug module
- [ ] Test on Android 14 device (API 34)
- [ ] Verify scan + advertise with runtime permissions

#### Day 4-5: iOS Spike
- [ ] Fix Info.plist (remove deprecated key)
- [ ] Test background limitations
- [ ] Verify service UUID overflow behavior
- [ ] Implement connection-based identity exchange
- [ ] Test on physical iPhone with iOS 17

### Week 2: UniFFI & Integration

#### Day 6-8: UniFFI Contract
- [ ] Finalize bitcraps.udl as single source
- [ ] Remove all mixed #[export] patterns
- [ ] Implement async methods properly
- [ ] Replace callbacks with polling/streams
- [ ] Generate bindings for both platforms

#### Day 9-10: Integration Testing
- [ ] Cross-platform discovery test
- [ ] iOS background → Android foreground
- [ ] Verify protocol version negotiation
- [ ] Test with 4+ devices simultaneously
- [ ] Document platform-specific limitations

---

## Platform-Specific Gotchas

### Android Gotchas
1. **Doze Mode**: Foreground Service helps but isn't immune
2. **OEM variations**: Samsung/Xiaomi have aggressive battery optimization
3. **Play Store**: Must justify Foreground Service usage
4. **Permissions**: "Nearby devices" group can be denied entirely

### iOS Gotchas
1. **App Review**: Will scrutinize Bluetooth usage description
2. **Background**: Only bluetooth-central is reliable; peripheral is flaky
3. **TestFlight**: BLE works differently than App Store builds
4. **State Restoration**: Must handle Bluetooth state restoration

---

## Architecture Decisions

### Runtime Management
```rust
// CORRECT: One Tokio runtime per process
lazy_static! {
    static ref RUNTIME: Runtime = Runtime::new().unwrap();
}

pub fn initialize_bitcraps() -> BitcrapsNode {
    // Use the single runtime for all async operations
    RUNTIME.block_on(async {
        BitcrapsNode::new().await
    })
}
```

### Protocol Versioning
```rust
// Add to initial handshake
pub struct HandshakePacket {
    pub proto_version: u16,  // Start at 1
    pub min_supported: u16,  // Oldest compatible
    pub peer_id: PeerId,
    pub capabilities: u32,   // Feature flags
}
```

---

## Performance Targets (Mobile-Adjusted)

| Metric | Target | Measurement |
|--------|--------|-------------|
| Battery (Active) | <5% per hour | Battery Historian / Instruments |
| Battery (Background) | <1% per hour | Energy Logs |
| Memory | <150MB baseline | Memory Profiler |
| BLE Latency | <200ms discovery | Custom instrumentation |
| UI Frame Rate | 60fps consistent | Systrace / Instruments |

---

## Compliance Checklist

### Android
- [x] BLUETOOTH_ADVERTISE permission added
- [x] Runtime permissions for API 31+
- [x] Foreground Service with type declared
- [x] User notification for background operation
- [x] JavaVM storage pattern (not JNIEnv)
- [x] btleplug Java module included

### iOS
- [x] NSBluetoothAlwaysUsageDescription only
- [x] UIBackgroundModes only if needed
- [x] Service UUID-based discovery
- [x] No reliance on background local name
- [x] XCFramework with SPM wrapper

### UniFFI
- [x] UDL as single source of truth
- [x] Async methods throughout
- [x] No callback interfaces
- [x] Event polling or streams

### Testing
- [x] Physical device matrix defined
- [x] Cloud farms for UI only
- [x] BLE testing on real hardware
- [x] Cross-platform test scenarios

---

*Document Version: 2.0*  
*Last Updated: 2025-08-23*  
*Status: Critical Corrections Applied*