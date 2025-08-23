# BitCraps Mobile Expansion Plan

## Executive Summary

This document outlines a comprehensive plan to expand BitCraps to Android and iOS platforms while maintaining an all-Rust core architecture with minimal platform-specific code. The plan leverages UniFFI for automatic binding generation, btleplug for cross-platform Bluetooth support, and native UI frameworks for optimal user experience.

**Timeline**: 16-20 weeks total  
**Approach**: Shared Rust library with native UI layers  
**Target**: Full protocol compatibility between all platforms  

---

## Table of Contents

1. [Architecture Overview](#1-architecture-overview)
2. [Android Integration Architecture](#2-android-integration-architecture)
3. [iOS Integration Architecture](#3-ios-integration-architecture)
4. [Shared Rust Library Design](#4-shared-rust-library-design)
5. [Build System and Toolchain Setup](#5-build-system-and-toolchain-setup)
6. [Testing Strategy](#6-testing-strategy)
7. [Timeline and Milestones](#7-timeline-and-milestones)
8. [Risk Mitigation Strategies](#8-risk-mitigation-strategies)
9. [Implementation Details](#9-implementation-details)
10. [Quality Assurance](#10-quality-assurance)

---

## 1. Architecture Overview

### 1.1 Core Principles

- **Rust Core**: All business logic, networking, cryptography, and consensus remain in Rust
- **Native UI**: Platform-specific UI using Kotlin/Jetpack Compose (Android) and Swift/SwiftUI (iOS)
- **Minimal Duplication**: Share 95%+ of code across platforms
- **Protocol Compatibility**: Seamless interoperability between desktop, Android, and iOS clients

### 1.2 High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     Platform UI Layer                          │
├─────────────────┬───────────────────────┬─────────────────────┤
│   Android       │     Shared Rust       │        iOS          │
│   (Kotlin)      │       Core             │      (Swift)        │
│                 │                        │                     │
│ • Jetpack       │ • Bluetooth Transport  │ • SwiftUI           │
│   Compose       │ • Consensus Engine     │ • CoreBluetooth     │
│ • Material3     │ • Cryptography         │ • Combine           │
│ • BLE Manager   │ • Game Logic           │ • Metal (future)    │
├─────────────────┼───────────────────────┼─────────────────────┤
│                 │     UniFFI Bindings    │                     │
├─────────────────┼───────────────────────┼─────────────────────┤
│                 │      Rust FFI API      │                     │
│                 │  (C-compatible ABI)    │                     │
└─────────────────┴───────────────────────┴─────────────────────┘
```

### 1.3 Key Components

**Shared Components (Rust)**:
- `bitcraps-core`: Main game logic and consensus
- `bitcraps-transport`: Bluetooth mesh networking
- `bitcraps-crypto`: Cryptographic operations
- `bitcraps-mobile`: Mobile-specific adaptations

**Platform Components**:
- Android: APK with native UI calling Rust via JNI
- iOS: App with native UI calling Rust via C FFI

---

## 2. Android Integration Architecture

### 2.1 Technical Approach

**Build Target**: `aarch64-linux-android`, `armv7-linux-androideabi`, `x86_64-linux-android`  
**Integration Method**: JNI with UniFFI-generated bindings  
**UI Framework**: Jetpack Compose with Material 3  
**Bluetooth**: btleplug with Android BluetoothManager backend  

### 2.2 Project Structure

```
android/
├── app/
│   ├── src/main/
│   │   ├── java/com/bitcraps/
│   │   │   ├── BitcrapsApplication.kt
│   │   │   ├── MainActivity.kt
│   │   │   ├── ui/
│   │   │   │   ├── game/GameScreen.kt
│   │   │   │   ├── lobby/LobbyScreen.kt
│   │   │   │   └── components/DiceView.kt
│   │   │   └── service/
│   │   │       ├── BluetoothService.kt
│   │   │       └── GameService.kt
│   │   ├── jniLibs/
│   │   │   ├── arm64-v8a/libbitcraps.so
│   │   │   ├── armeabi-v7a/libbitcraps.so
│   │   │   └── x86_64/libbitcraps.so
│   │   └── AndroidManifest.xml
│   └── build.gradle
├── build.gradle
└── settings.gradle
```

### 2.3 Key Implementation Details

**Bluetooth Permissions** (AndroidManifest.xml):
```xml
<uses-permission android:name="android.permission.BLUETOOTH" />
<uses-permission android:name="android.permission.BLUETOOTH_ADMIN" />
<uses-permission android:name="android.permission.BLUETOOTH_SCAN" />
<uses-permission android:name="android.permission.BLUETOOTH_CONNECT" />
<uses-permission android:name="android.permission.ACCESS_FINE_LOCATION" />
<uses-permission android:name="android.permission.ACCESS_COARSE_LOCATION" />
```

**JNI Integration**:
- Use cargo-ndk for cross-compilation
- UniFFI generates Kotlin bindings automatically
- Handle Android lifecycle events (pause/resume)
- Manage background Bluetooth scanning permissions

### 2.4 UI Components

**Core Screens**:
- `LobbyScreen`: Peer discovery and game creation
- `GameScreen`: Craps table interface with animated dice
- `WalletScreen`: CRAP token balance and transactions
- `SettingsScreen`: Network configuration and preferences

**Key Features**:
- Smooth dice roll animations using Jetpack Compose
- Real-time peer list updates
- Toast notifications for game events
- Material 3 theming with dark mode support

---

## 3. iOS Integration Architecture

### 3.1 Technical Approach

**Build Target**: `aarch64-apple-ios`, `x86_64-apple-ios` (simulator)  
**Integration Method**: C FFI with Swift bridging  
**UI Framework**: SwiftUI with Combine for reactive updates  
**Bluetooth**: btleplug with CoreBluetooth backend  

### 3.2 Project Structure

```
ios/
├── BitCraps/
│   ├── BitCraps.xcodeproj
│   ├── BitCraps/
│   │   ├── App/
│   │   │   ├── BitCrapsApp.swift
│   │   │   └── ContentView.swift
│   │   ├── Views/
│   │   │   ├── GameView.swift
│   │   │   ├── LobbyView.swift
│   │   │   └── Components/
│   │   │       ├── DiceView.swift
│   │   │       └── BettingView.swift
│   │   ├── Services/
│   │   │   ├── BluetoothService.swift
│   │   │   └── GameService.swift
│   │   └── Bridge/
│   │       ├── BitcrapsFFI.swift
│   │       └── bitcraps.h
│   └── Frameworks/
│       └── libbitcraps.xcframework
```

### 3.3 Key Implementation Details

**Info.plist Configuration**:
```xml
<key>NSBluetoothAlwaysUsageDescription</key>
<string>BitCraps uses Bluetooth to connect with nearby players</string>
<key>NSBluetoothPeripheralUsageDescription</key>
<string>BitCraps needs Bluetooth to host games</string>
```

**Swift Integration**:
- Use cbindgen to generate C headers from Rust
- Create Swift wrapper classes for ergonomic API
- Handle iOS app lifecycle (background/foreground)
- Integrate with iOS notification system

### 3.4 UI Components

**Core Views**:
- `LobbyView`: Game discovery and creation interface
- `GameView`: Interactive craps table with Metal-accelerated animations
- `WalletView`: Token management and transaction history
- `SettingsView`: Bluetooth and network configuration

**Key Features**:
- Smooth 60fps dice animations using Metal (future enhancement)
- Haptic feedback for dice rolls and bets
- iOS-native notifications for game events
- Dark mode support following iOS guidelines

---

## 4. Shared Rust Library Design

### 4.1 Crate Structure

```rust
bitcraps-mobile/
├── Cargo.toml
├── src/
│   ├── lib.rs              // Main mobile library entry point
│   ├── mobile_api.rs       // UniFFI interface definitions
│   ├── events.rs           // Event system for UI updates
│   ├── bluetooth_mobile.rs // Mobile-specific Bluetooth adaptations
│   └── platform/
│       ├── android.rs      // Android-specific code
│       └── ios.rs          // iOS-specific code
└── uniffi.toml             // UniFFI configuration
```

### 4.2 UniFFI Interface Design

**Core API** (mobile_api.rs):
```rust
use uniffi::export;

#[export]
pub struct BitcrapsNode {
    // Internal implementation hidden
}

#[export]
impl BitcrapsNode {
    #[uniffi::constructor]
    pub fn new(config: NodeConfig) -> Result<BitcrapsNode, BitcrapsError>;
    
    pub fn start_discovery(&self) -> Result<(), BitcrapsError>;
    pub fn create_game(&self, game_name: String) -> Result<GameId, BitcrapsError>;
    pub fn join_game(&self, game_id: GameId) -> Result<(), BitcrapsError>;
    pub fn place_bet(&self, bet: BetRequest) -> Result<(), BitcrapsError>;
    pub fn get_balance(&self) -> u64;
    pub fn get_peers(&self) -> Vec<PeerInfo>;
}

#[export]
pub struct GameEvent {
    pub event_type: EventType,
    pub data: String,
    pub timestamp: u64,
}

#[export]
pub enum EventType {
    PeerDiscovered,
    PeerLost,
    GameCreated,
    GameJoined,
    BetPlaced,
    DiceRolled,
    GameFinished,
}
```

### 4.3 Event System

**Reactive Updates**:
- Event-driven architecture for UI updates
- Platform-specific event delivery (Android: callbacks, iOS: notifications)
- Buffered events for offline/background scenarios

**Implementation**:
```rust
pub struct EventBus {
    listeners: Arc<Mutex<Vec<Box<dyn EventListener>>>>,
}

pub trait EventListener: Send + Sync {
    fn on_event(&self, event: GameEvent);
}

// Platform-specific implementations
#[cfg(target_os = "android")]
impl EventListener for AndroidEventListener { /* ... */ }

#[cfg(target_os = "ios")]
impl EventListener for IOSEventListener { /* ... */ }
```

---

## 5. Build System and Toolchain Setup

### 5.1 Cross-Compilation Setup

**Rust Targets**:
```bash
# Android targets
rustup target add aarch64-linux-android
rustup target add armv7-linux-androideabi
rustup target add x86_64-linux-android

# iOS targets
rustup target add aarch64-apple-ios
rustup target add x86_64-apple-ios
rustup target add aarch64-apple-ios-sim
```

**Required Tools**:
```bash
# Android
cargo install cargo-ndk
# iOS
cargo install cargo-lipo
cargo install cbindgen
# UniFFI
cargo install uniffi-bindgen
```

### 5.2 Build Scripts

**Android Build** (scripts/build-android.sh):
```bash
#!/bin/bash
set -e

# Build Rust library for Android
cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 build --release

# Generate UniFFI bindings
uniffi-bindgen generate src/mobile_api.udl --language kotlin \
    --out-dir android/app/src/main/kotlin/com/bitcraps/generated

# Copy native libraries
cp target/aarch64-linux-android/release/libbitcraps.so \
   android/app/src/main/jniLibs/arm64-v8a/
cp target/armv7-linux-androideabi/release/libbitcraps.so \
   android/app/src/main/jniLibs/armeabi-v7a/
cp target/x86_64-linux-android/release/libbitcraps.so \
   android/app/src/main/jniLibs/x86_64/

# Build Android APK
cd android && ./gradlew assembleRelease
```

**iOS Build** (scripts/build-ios.sh):
```bash
#!/bin/bash
set -e

# Build universal static library
cargo lipo --release --targets \
    aarch64-apple-ios,x86_64-apple-ios,aarch64-apple-ios-sim

# Generate C header
cbindgen --config cbindgen.toml --crate bitcraps-mobile \
    --output ios/BitCraps/Bridge/bitcraps.h

# Generate Swift bindings
uniffi-bindgen generate src/mobile_api.udl --language swift \
    --out-dir ios/BitCraps/Generated

# Create XCFramework
xcodebuild -create-xcframework \
    -library target/universal/release/libbitcraps.a \
    -headers ios/BitCraps/Bridge/ \
    -output ios/BitCraps/Frameworks/libbitcraps.xcframework
```

### 5.3 Continuous Integration

**GitHub Actions** (.github/workflows/mobile.yml):
```yaml
name: Mobile Build

on: [push, pull_request]

jobs:
  android:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - name: Install Android targets
        run: rustup target add aarch64-linux-android armv7-linux-androideabi
      - name: Setup Android SDK
        uses: android-actions/setup-android@v2
      - name: Install cargo-ndk
        run: cargo install cargo-ndk
      - name: Build Android
        run: ./scripts/build-android.sh
      - name: Run Android tests
        run: cd android && ./gradlew test

  ios:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - name: Install iOS targets
        run: rustup target add aarch64-apple-ios x86_64-apple-ios
      - name: Install tools
        run: cargo install cargo-lipo cbindgen uniffi-bindgen
      - name: Build iOS
        run: ./scripts/build-ios.sh
      - name: Build Xcode project
        run: cd ios && xcodebuild -project BitCraps.xcodeproj -scheme BitCraps
```

---

## 6. Testing Strategy

### 6.1 Testing Pyramid

```
        ┌─────────────────────────────────┐
        │         E2E Tests               │ <- 5%
        │    (Physical Devices)           │
        ├─────────────────────────────────┤
        │      Integration Tests          │ <- 25%
        │   (Platform + Rust Core)        │
        ├─────────────────────────────────┤
        │        Unit Tests               │ <- 70%
        │    (Rust Core Logic)            │
        └─────────────────────────────────┘
```

### 6.2 Unit Testing (Rust Core)

**Existing Test Suite**: Leverage current comprehensive test coverage
- `tests/unit_tests/`: Core protocol and crypto tests
- `tests/gaming/`: Game logic and fairness tests
- `tests/security/`: Security and vulnerability tests
- `tests/integration/`: Multi-peer scenario tests

**Mobile-Specific Tests**:
```rust
#[cfg(test)]
mod mobile_tests {
    use super::*;

    #[test]
    fn test_mobile_api_initialization() {
        let config = NodeConfig::default();
        let node = BitcrapsNode::new(config).unwrap();
        assert!(node.get_balance() == 0);
    }

    #[test]
    fn test_event_system() {
        let mut events = Vec::new();
        let listener = TestEventListener::new(&mut events);
        
        // Test event delivery
        let event = GameEvent {
            event_type: EventType::PeerDiscovered,
            data: "peer_id_123".to_string(),
            timestamp: 1234567890,
        };
        
        listener.on_event(event);
        assert_eq!(events.len(), 1);
    }
}
```

### 6.3 Integration Testing

**Android Integration Tests**:
```kotlin
@RunWith(AndroidJUnit4::class)
class BitcrapsIntegrationTest {
    
    @Test
    fun testNodeInitialization() {
        val config = NodeConfig()
        val node = BitcrapsNode(config)
        
        assertEquals(0, node.getBalance())
        assertNotNull(node.getPeers())
    }
    
    @Test
    fun testBluetoothPermissions() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val permission = ContextCompat.checkSelfPermission(
            context, 
            Manifest.permission.BLUETOOTH_SCAN
        )
        // Test permission handling
    }
}
```

**iOS Integration Tests**:
```swift
import XCTest
@testable import BitCraps

class BitcrapsIntegrationTests: XCTestCase {
    
    func testNodeInitialization() {
        let config = NodeConfig()
        let node = try! BitcrapsNode(config: config)
        
        XCTAssertEqual(node.getBalance(), 0)
        XCTAssertNotNil(node.getPeers())
    }
    
    func testBluetoothCapability() {
        XCTAssertTrue(CBCentralManager().state == .poweredOn)
    }
}
```

### 6.4 Device Testing

**Testing Matrix**:
| Platform | Device Types | OS Versions | Test Scenarios |
|----------|--------------|-------------|----------------|
| Android | Phone, Tablet | API 26-34 | Discovery, Gaming, Background |
| iOS | iPhone, iPad | iOS 14-17 | Discovery, Gaming, Background |

**Test Scenarios**:
1. **Peer Discovery**: Multiple devices finding each other
2. **Cross-Platform Gaming**: Android ↔ iOS ↔ Desktop interoperability
3. **Connection Stability**: Maintain connections during gameplay
4. **Background Behavior**: App backgrounding and foregrounding
5. **Battery Impact**: Extended gameplay sessions
6. **Memory Usage**: Long-running stability tests

### 6.5 Automated Testing Infrastructure

**Device Farm Integration**:
- AWS Device Farm for Android testing
- Xcode Cloud for iOS testing
- Firebase Test Lab for additional Android coverage

**Test Automation**:
```bash
# Automated device testing script
./scripts/run-device-tests.sh --platform android --devices "pixel6,galaxy-s21"
./scripts/run-device-tests.sh --platform ios --devices "iphone13,ipad-pro"
```

---

## 7. Timeline and Milestones

### 7.1 Phase 1: Foundation (Weeks 1-4)

**Week 1-2: Development Environment Setup**
- [ ] Install and configure cross-compilation toolchains
- [ ] Set up Android development environment (Android Studio, SDKs)
- [ ] Set up iOS development environment (Xcode, certificates)
- [ ] Create build scripts for both platforms
- [ ] Establish CI/CD pipelines

**Week 3-4: Shared Library Architecture**
- [ ] Create `bitcraps-mobile` crate structure
- [ ] Design and implement UniFFI interface
- [ ] Create mobile-specific adaptations of core components
- [ ] Implement event system for UI updates
- [ ] Basic cross-compilation testing

**Deliverables**:
- Working build system for both platforms
- Initial mobile library with basic API
- Documentation for development setup

### 7.2 Phase 2: Android Implementation (Weeks 5-8)

**Week 5: Android Project Setup**
- [ ] Create Android project structure
- [ ] Implement JNI bindings integration
- [ ] Configure btleplug for Android
- [ ] Set up basic UI framework (Jetpack Compose)

**Week 6-7: Core Android Features**
- [ ] Implement Bluetooth permission handling
- [ ] Create lobby screen for peer discovery
- [ ] Implement game screen with basic UI
- [ ] Add wallet/balance functionality
- [ ] Integrate with Rust core via UniFFI

**Week 8: Android Polish and Testing**
- [ ] Add animations and improved UI
- [ ] Implement Android-specific features (notifications, background handling)
- [ ] Write Android integration tests
- [ ] Performance optimization and debugging

**Deliverables**:
- Functional Android app with full game capabilities
- Android integration test suite
- Performance benchmarks

### 7.3 Phase 3: iOS Implementation (Weeks 9-12)

**Week 9: iOS Project Setup**
- [ ] Create iOS project structure
- [ ] Implement C FFI bridge
- [ ] Configure btleplug for iOS
- [ ] Set up SwiftUI interface

**Week 10-11: Core iOS Features**
- [ ] Implement CoreBluetooth integration
- [ ] Create lobby view for peer discovery
- [ ] Implement game view with SwiftUI
- [ ] Add wallet functionality
- [ ] Integrate with Rust core

**Week 12: iOS Polish and Testing**
- [ ] Add iOS-specific animations and haptics
- [ ] Implement iOS features (background modes, notifications)
- [ ] Write iOS integration tests
- [ ] App Store preparation (if applicable)

**Deliverables**:
- Functional iOS app with full game capabilities
- iOS integration test suite
- App Store-ready build (optional)

### 7.4 Phase 4: Cross-Platform Integration (Weeks 13-16)

**Week 13-14: Interoperability Testing**
- [ ] Multi-device testing (Android ↔ iOS ↔ Desktop)
- [ ] Protocol compatibility verification
- [ ] Performance testing across platforms
- [ ] Bug fixes and optimizations

**Week 15: Advanced Features**
- [ ] Background Bluetooth handling
- [ ] Push notifications integration
- [ ] Advanced UI features (animations, effects)
- [ ] Battery optimization

**Week 16: Final Polish**
- [ ] User experience improvements
- [ ] Documentation completion
- [ ] Deployment preparation
- [ ] Security audit preparation

**Deliverables**:
- Fully tested cross-platform application
- Complete documentation
- Deployment-ready builds

### 7.5 Phase 5: Production Preparation (Weeks 17-20)

**Week 17-18: Beta Testing**
- [ ] Internal testing with multiple devices
- [ ] External beta testing program
- [ ] Performance monitoring and optimization
- [ ] Bug fixes from beta feedback

**Week 19-20: Production Release**
- [ ] Security audit and fixes
- [ ] Store submission preparation
- [ ] Marketing materials and documentation
- [ ] Release and monitoring

**Final Deliverables**:
- Production-ready Android and iOS applications
- Comprehensive test suite with >95% coverage
- Complete user and developer documentation
- Deployment and maintenance procedures

---

## 8. Risk Mitigation Strategies

### 8.1 Technical Risks

**Risk**: Bluetooth compatibility issues across devices
- **Likelihood**: Medium
- **Impact**: High
- **Mitigation**: 
  - Extensive device testing matrix
  - Fallback mechanisms for connection issues
  - Comprehensive BLE adapter abstraction
  - Community testing program

**Risk**: UniFFI binding generation complexity
- **Likelihood**: Low
- **Impact**: Medium
- **Mitigation**:
  - Prototype early with simple APIs
  - Fallback to manual FFI if needed
  - Extensive testing of generated bindings
  - Regular updates to latest UniFFI version

**Risk**: Platform-specific Bluetooth API limitations
- **Likelihood**: Medium
- **Impact**: Medium
- **Mitigation**:
  - Research platform limitations early
  - Design flexible protocol that works within constraints
  - Platform-specific optimizations where needed
  - Regular testing on real hardware

### 8.2 Development Risks

**Risk**: Cross-compilation toolchain issues
- **Likelihood**: Medium
- **Impact**: Medium
- **Mitigation**:
  - Use proven toolchains (cargo-ndk, cargo-lipo)
  - Docker-based build environments for consistency
  - Regular CI builds to catch issues early
  - Team training on toolchain usage

**Risk**: Team expertise in mobile development
- **Likelihood**: Low
- **Impact**: Medium
- **Mitigation**:
  - Invest in team training early
  - Bring in mobile development consultants
  - Start with simple implementations
  - Leverage existing mobile expertise in community

**Risk**: Integration complexity with existing codebase
- **Likelihood**: Low
- **Impact**: High
- **Mitigation**:
  - Comprehensive integration testing
  - Gradual migration approach
  - Maintain backward compatibility
  - Extensive code review process

### 8.3 Business Risks

**Risk**: App store approval issues
- **Likelihood**: Low
- **Impact**: High
- **Mitigation**:
  - Review app store guidelines early
  - Implement required privacy and security features
  - Prepare for submission process
  - Have alternative distribution methods ready

**Risk**: Performance issues on mobile devices
- **Likelihood**: Medium
- **Impact**: Medium
- **Mitigation**:
  - Early performance testing on low-end devices
  - Memory and battery optimization focus
  - Configurable performance settings
  - Gradual feature rollout

### 8.4 Security Risks

**Risk**: Mobile-specific attack vectors
- **Likelihood**: Medium
- **Impact**: High
- **Mitigation**:
  - Security review of mobile-specific code
  - Secure storage of cryptographic keys
  - Regular security audits
  - Secure communication protocols

**Risk**: Key management on mobile platforms
- **Likelihood**: Medium
- **Impact**: High
- **Mitigation**:
  - Use platform secure storage (Keychain/Keystore)
  - Implement proper key rotation
  - Secure backup and recovery mechanisms
  - Regular security testing

---

## 9. Implementation Details

### 9.1 Bluetooth Stack Integration

**btleplug Configuration**:
```rust
// Mobile-specific Bluetooth configuration
pub struct MobileBluetoothConfig {
    pub scan_window: Duration,
    pub scan_interval: Duration,
    pub advertising_interval: Duration,
    pub connection_timeout: Duration,
    pub max_connections: usize,
}

impl Default for MobileBluetoothConfig {
    fn default() -> Self {
        Self {
            scan_window: Duration::from_millis(100),
            scan_interval: Duration::from_millis(200),
            advertising_interval: Duration::from_millis(1000),
            connection_timeout: Duration::from_secs(30),
            max_connections: 8,
        }
    }
}
```

**Power Management**:
```rust
pub struct PowerManager {
    config: MobileBluetoothConfig,
    state: PowerState,
}

impl PowerManager {
    pub fn adjust_for_battery_level(&mut self, battery_level: f32) {
        if battery_level < 0.2 {
            // Reduce scanning frequency when battery is low
            self.config.scan_interval = Duration::from_millis(1000);
            self.config.advertising_interval = Duration::from_millis(2000);
        }
    }
    
    pub fn handle_background_mode(&mut self, is_background: bool) {
        if is_background {
            // Reduce activity when app is in background
            self.config.scan_window = Duration::from_millis(50);
            self.config.max_connections = 4;
        }
    }
}
```

### 9.2 State Management

**Event-Driven Architecture**:
```rust
#[derive(Debug, Clone)]
pub enum MobileEvent {
    BluetoothStateChanged(BluetoothState),
    PeerDiscovered(PeerInfo),
    PeerConnected(PeerId),
    PeerDisconnected(PeerId),
    GameStateChanged(GameState),
    DiceRolled(DiceResult),
    BetPlaced(BetInfo),
    BalanceUpdated(u64),
    Error(String),
}

pub trait MobileEventHandler: Send + Sync {
    fn handle_event(&self, event: MobileEvent);
}

// Platform-specific implementations
#[cfg(target_os = "android")]
pub struct AndroidEventHandler {
    jni_env: Arc<Mutex<JNIEnv<'static>>>,
    callback_object: GlobalRef,
}

#[cfg(target_os = "android")]
impl MobileEventHandler for AndroidEventHandler {
    fn handle_event(&self, event: MobileEvent) {
        if let Ok(env) = self.jni_env.try_lock() {
            match event {
                MobileEvent::DiceRolled(result) => {
                    env.call_method(
                        self.callback_object.as_obj(),
                        "onDiceRolled",
                        "(II)V",
                        &[JValue::Int(result.die1 as i32), JValue::Int(result.die2 as i32)]
                    ).ok();
                }
                // Handle other events...
                _ => {}
            }
        }
    }
}
```

### 9.3 Memory Management

**Memory Pool for Mobile**:
```rust
use std::sync::Arc;
use crossbeam_epoch::{Guard, Owned, Shared};

pub struct MobileMemoryPool {
    small_blocks: Arc<BlockPool<1024>>,
    medium_blocks: Arc<BlockPool<4096>>,
    large_blocks: Arc<BlockPool<16384>>,
}

impl MobileMemoryPool {
    pub fn new() -> Self {
        Self {
            small_blocks: Arc::new(BlockPool::new(100)),  // Pool of 100 1KB blocks
            medium_blocks: Arc::new(BlockPool::new(50)),   // Pool of 50 4KB blocks  
            large_blocks: Arc::new(BlockPool::new(10)),    // Pool of 10 16KB blocks
        }
    }
    
    pub fn allocate(&self, size: usize) -> Option<Box<[u8]>> {
        match size {
            0..=1024 => self.small_blocks.get(),
            1025..=4096 => self.medium_blocks.get(),
            4097..=16384 => self.large_blocks.get(),
            _ => None, // Allocate directly for very large requests
        }
    }
}
```

### 9.4 Security Considerations

**Secure Key Storage**:
```rust
#[cfg(target_os = "android")]
pub struct AndroidKeystore {
    // Uses Android Keystore system
}

#[cfg(target_os = "android")]
impl AndroidKeystore {
    pub fn store_key(&self, alias: &str, key: &[u8]) -> Result<(), KeystoreError> {
        // Use Android Keystore API via JNI
        // Keys are hardware-backed when available
    }
    
    pub fn retrieve_key(&self, alias: &str) -> Result<Vec<u8>, KeystoreError> {
        // Retrieve from secure storage
    }
}

#[cfg(target_os = "ios")]
pub struct IOSKeychain {
    // Uses iOS Keychain Services
}

#[cfg(target_os = "ios")]
impl IOSKeychain {
    pub fn store_key(&self, service: &str, key: &[u8]) -> Result<(), KeychainError> {
        // Use Keychain Services API
        // Keys stored in Secure Enclave when available
    }
}
```

---

## 10. Quality Assurance

### 10.1 Code Quality Metrics

**Target Metrics**:
- Test Coverage: >95% for Rust core, >85% for platform code
- Code Duplication: <5% between platforms
- Build Time: <5 minutes for full build
- App Size: <50MB for release builds
- Memory Usage: <200MB peak during gameplay
- Battery Impact: <5% per hour of gameplay

### 10.2 Performance Benchmarks

**Mobile Performance Targets**:
```rust
#[cfg(test)]
mod mobile_benchmarks {
    use criterion::{black_box, criterion_group, criterion_main, Criterion};
    
    fn bench_peer_discovery(c: &mut Criterion) {
        c.bench_function("peer_discovery_mobile", |b| {
            b.iter(|| {
                // Should complete within 5 seconds on mobile
                discover_peers_mobile(black_box(Duration::from_secs(5)))
            });
        });
    }
    
    fn bench_game_state_sync(c: &mut Criterion) {
        c.bench_function("game_state_sync_mobile", |b| {
            b.iter(|| {
                // Should sync game state within 1 second
                sync_game_state_mobile(black_box(sample_game_state()))
            });
        });
    }
    
    criterion_group!(mobile_benches, bench_peer_discovery, bench_game_state_sync);
    criterion_main!(mobile_benches);
}
```

### 10.3 User Experience Validation

**UX Testing Checklist**:
- [ ] App launches within 3 seconds
- [ ] Peer discovery completes within 10 seconds
- [ ] Dice animations are smooth (60fps)
- [ ] UI remains responsive during network operations
- [ ] Battery usage remains reasonable during extended play
- [ ] App handles network interruptions gracefully
- [ ] Permissions are requested at appropriate times
- [ ] Error messages are user-friendly and actionable

### 10.4 Security Validation

**Security Testing Protocol**:
1. **Static Analysis**: Use cargo-audit and platform-specific security scanners
2. **Dynamic Analysis**: Runtime security testing with network monitoring
3. **Penetration Testing**: Test for common mobile vulnerabilities
4. **Code Review**: Security-focused review of all mobile-specific code
5. **Third-party Audit**: External security audit before production release

---

## Conclusion

This mobile expansion plan provides a comprehensive roadmap for bringing BitCraps to Android and iOS platforms while maintaining the all-Rust core architecture. The approach minimizes platform-specific code duplication, ensures full protocol compatibility, and leverages proven tools and frameworks.

**Key Success Factors**:
1. **Early and frequent testing** on real devices
2. **Strong cross-platform architecture** with clear separation of concerns
3. **Comprehensive quality assurance** throughout the development process
4. **Proactive risk mitigation** for known technical challenges
5. **Community involvement** in testing and feedback

The timeline of 16-20 weeks is aggressive but achievable with proper planning and execution. The result will be a truly cross-platform decentralized casino experience that works seamlessly across desktop, Android, and iOS devices.

**Next Steps**:
1. Team review and approval of this plan
2. Resource allocation and team assignments
3. Development environment setup (Week 1)
4. Begin Phase 1 implementation

This plan positions BitCraps to be the first fully decentralized, cross-platform casino protocol with native mobile support, opening up massive market opportunities and demonstrating the power of Rust for mobile development.