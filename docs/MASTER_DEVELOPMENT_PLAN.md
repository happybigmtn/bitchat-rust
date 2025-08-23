# BitCraps Master Development Plan
## Consolidated Roadmap with All Feedback Integrated

*Version: 3.0 | Last Updated: 2025-08-23 | Status: Active Development*

---

## Executive Summary

This master plan consolidates all development priorities, engineering feedback, and implementation requirements for BitCraps. It incorporates critical platform-specific corrections from senior engineers, comprehensive security requirements, and a realistic 24-28 week roadmap to production deployment.

**Timeline**: 24-28 weeks  
**Approach**: Parallel development tracks with week 1 kill-or-cure validation  
**Goal**: Production-ready, security-audited, cross-platform decentralized casino  

---

## Table of Contents

1. [Critical Immediate Fixes (Week 1)](#1-critical-immediate-fixes-week-1)
2. [Development Priorities](#2-development-priorities)
3. [Technical Implementation Guide](#3-technical-implementation-guide)
4. [24-Week Development Roadmap](#4-24-week-development-roadmap)
5. [Testing & Quality Assurance](#5-testing--quality-assurance)
6. [Production Infrastructure Status](#6-production-infrastructure-status)
7. [Risk Mitigation](#7-risk-mitigation)
8. [Success Metrics](#8-success-metrics)

---

## 1. Critical Immediate Fixes (Week 1)

### 🚨 MUST FIX BEFORE ANY OTHER DEVELOPMENT

#### Android Critical Fixes

##### Permissions (AndroidManifest.xml)
```xml
<!-- CORRECTED: Was missing BLUETOOTH_ADVERTISE -->
<uses-permission android:name="android.permission.BLUETOOTH_ADVERTISE" />
<uses-permission android:name="android.permission.BLUETOOTH_SCAN" />
<uses-permission android:name="android.permission.BLUETOOTH_CONNECT" />

<!-- Legacy for API ≤30 -->
<uses-permission android:name="android.permission.BLUETOOTH" 
                 android:maxSdkVersion="30" />

<!-- Foreground Service for Android 14+ -->
<uses-permission android:name="android.permission.FOREGROUND_SERVICE_CONNECTED_DEVICE" />
```

##### Foreground Service Implementation
```xml
<!-- CRITICAL: Must declare service type -->
<service 
    android:name=".BluetoothMeshService"
    android:foregroundServiceType="connectedDevice"
    android:exported="false" />
```

##### JNI Thread Safety Fix
```rust
// WRONG - Thread unsafe
// pub struct AndroidEventHandler {
//     jni_env: Arc<Mutex<JNIEnv<'static>>>, // CRASHES!
// }

// CORRECT - Thread safe
pub struct AndroidEventHandler {
    jvm: JavaVM,  // Store JavaVM instead
    callback: GlobalRef,
}
```

#### iOS Critical Fixes

##### Info.plist Corrections
```xml
<!-- CORRECT: Only use current key -->
<key>NSBluetoothAlwaysUsageDescription</key>
<string>BitCraps uses Bluetooth to connect with nearby players.</string>

<!-- REMOVED: Deprecated, causes App Review issues -->
<!-- <key>NSBluetoothPeripheralUsageDescription</key> -->
```

##### Background BLE Reality
- ❌ Local name NOT advertised in background
- ❌ Service UUIDs move to overflow area
- ✅ Must use service UUID filtering
- ✅ Exchange identity after connection

#### Architecture Fixes

##### UniFFI Pattern (UDL-only)
```idl
// CORRECT: Single source of truth in .udl file
interface BitcrapsNode {
    [Throws=BitcrapsError, Async]
    void start_discovery();
    
    // NO callback interfaces - use async
    [Async]
    GameEvent? poll_event();
}
```

---

## 2. Development Priorities

### Priority Matrix

| Priority | Category | Timeline | Impact | Risk if Delayed |
|----------|----------|----------|---------|-----------------|
| P0 | Platform Fixes | Week 1 | Critical | App crashes/rejection |
| P1 | Security & Compliance | Weeks 2-6 | Critical | Vulnerabilities |
| P2 | Mobile Core | Weeks 7-12 | High | No product |
| P3 | Performance | Weeks 13-18 | High | Poor UX |
| P4 | Platform Features | Weeks 19-24 | Medium | Limited growth |
| P5 | Launch Prep | Weeks 25-28 | Medium | Delayed launch |

### P0: Immediate Fixes (Week 1)

**Platform Fixes**
- [ ] Android: Add BLUETOOTH_ADVERTISE permission
- [ ] Android: Implement Foreground Service with type
- [ ] Android: Fix JNI to use JavaVM
- [ ] Android: Include btleplug droidplug module
- [ ] iOS: Remove deprecated Info.plist keys
- [ ] iOS: Implement service UUID filtering
- [ ] UniFFI: Finalize UDL with async methods
- [ ] Architecture: One Tokio runtime per process
- [ ] Protocol: Add version field to handshake

**Compilation & Code Quality** (NEW from codebase review)
- [ ] Fix 2 critical compilation errors in src/resilience/mod.rs
- [ ] Resolve 47 compiler warnings (unused imports, variables, fields)
- [ ] Fix unsafe static reference warnings
- [ ] Clean up lifetime hiding issues in treasury and UI modules
- [ ] Complete TODO implementations in integration tests:
  - [ ] ConsensusVote and VoteType functionality
  - [ ] MessageCompressor implementation
  - [ ] DeterministicRng implementation

### P1: Security & Compliance (Weeks 2-6)

- [ ] Third-party security audit
- [ ] Byzantine fault tolerance testing (>33% malicious)
- [ ] Threat modeling with STRIDE
- [ ] Hardware security module integration
- [ ] GDPR/CCPA compliance
- [ ] Key rotation automation
- [ ] Penetration testing
- [ ] Bug bounty program setup

### P2: Mobile Implementation (Weeks 7-12)

- [ ] Android Jetpack Compose UI
- [ ] iOS SwiftUI implementation
- [ ] Cross-platform interoperability
- [ ] Battery optimization (<5% drain/hour)
- [ ] Physical device testing (10+ devices)
- [ ] App store compliance
- [ ] Performance monitoring
- [ ] Crash reporting

### P3: Performance & Scalability (Weeks 13-18)

- [ ] Connection pooling (10x capacity)
- [ ] DHT optimization for mobile
- [ ] Persistent ledger implementation
- [ ] Message compression (60-80%)
- [ ] Adaptive BLE scanning
- [ ] Memory optimization (<150MB)
- [ ] Load testing (100+ games)
- [ ] Network resilience

### P4: Platform & Extensibility (Weeks 19-24)

- [ ] Multi-game framework
- [ ] Internet gateway nodes
- [ ] Analytics infrastructure
- [ ] Monitoring dashboards
- [ ] CI/CD automation
- [ ] Developer documentation
- [ ] SDK development
- [ ] Protocol federation

### P5: Launch Preparation (Weeks 25-28)

- [ ] App store optimization
- [ ] Marketing materials
- [ ] Beta testing program
- [ ] Support documentation
- [ ] Operations runbook
- [ ] Incident response plan
- [ ] Community building
- [ ] Launch campaign

---

## 3. Technical Implementation Guide

### Android Implementation

#### Manifest Configuration (Complete)
```xml
<?xml version="1.0" encoding="utf-8"?>
<manifest xmlns:android="http://schemas.android.com/apk/res/android">
    
    <!-- Bluetooth Permissions -->
    <uses-permission android:name="android.permission.BLUETOOTH" 
                     android:maxSdkVersion="30" />
    <uses-permission android:name="android.permission.BLUETOOTH_ADMIN" 
                     android:maxSdkVersion="30" />
    <uses-permission android:name="android.permission.ACCESS_FINE_LOCATION" 
                     android:maxSdkVersion="30" />
    
    <!-- Android 12+ Permissions -->
    <uses-permission android:name="android.permission.BLUETOOTH_SCAN" />
    <uses-permission android:name="android.permission.BLUETOOTH_CONNECT" />
    <uses-permission android:name="android.permission.BLUETOOTH_ADVERTISE" />
    
    <!-- Foreground Service -->
    <uses-permission android:name="android.permission.FOREGROUND_SERVICE" />
    <uses-permission android:name="android.permission.FOREGROUND_SERVICE_CONNECTED_DEVICE" />
    
    <!-- Hardware requirement -->
    <uses-feature android:name="android.hardware.bluetooth_le" 
                  android:required="true" />
    
    <application>
        <service 
            android:name=".BluetoothMeshService"
            android:foregroundServiceType="connectedDevice"
            android:exported="false" />
    </application>
</manifest>
```

#### Runtime Permissions (Kotlin)
```kotlin
fun requestPermissions() {
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
        // Android 12+ "Nearby devices"
        requestPermissions(arrayOf(
            Manifest.permission.BLUETOOTH_SCAN,
            Manifest.permission.BLUETOOTH_CONNECT,
            Manifest.permission.BLUETOOTH_ADVERTISE
        ), REQUEST_CODE)
    } else {
        // Android 11 and below
        requestPermissions(arrayOf(
            Manifest.permission.ACCESS_FINE_LOCATION
        ), REQUEST_CODE)
    }
}
```

#### btleplug Integration
```gradle
dependencies {
    // Required for btleplug
    implementation project(':droidplug')
    implementation 'com.github.deviceplug:jni-utils-rs:0.1.0'
}
```

### iOS Implementation

#### Info.plist (Complete)
```xml
<key>NSBluetoothAlwaysUsageDescription</key>
<string>BitCraps uses Bluetooth to connect with nearby players.</string>

<!-- Only if background needed -->
<key>UIBackgroundModes</key>
<array>
    <string>bluetooth-central</string>
</array>
```

#### Background Limitations Handler
```swift
func handleBackgroundBLE() {
    // Service UUID is ONLY reliable filter
    let serviceUUID = CBUUID(string: "12345678-1234-5678-1234-567812345678")
    
    centralManager.scanForPeripherals(
        withServices: [serviceUUID],  // Required for background
        options: [CBCentralManagerScanOptionAllowDuplicatesKey: false]
    )
    
    // Exchange identity AFTER connection
    // Local name not available in background
}
```

### UniFFI Interface (Complete)

#### bitcraps.udl
```idl
namespace bitcraps {
    [Throws=BitcrapsError]
    BitcrapsNode create_node(BitcrapsConfig config);
};

interface BitcrapsNode {
    [Throws=BitcrapsError, Async]
    void start_discovery();
    
    [Throws=BitcrapsError, Async]
    GameHandle create_game(GameConfig config);
    
    [Throws=BitcrapsError, Async]
    GameHandle join_game(string game_id);
    
    // Modern async pattern - no callbacks
    [Async]
    GameEvent? poll_event();
    
    [Async]
    sequence<GameEvent> drain_events();
};

dictionary BitcrapsConfig {
    string data_dir;
    u32 pow_difficulty;
    u16 protocol_version;  // For compatibility
};

[Enum]
interface GameEvent {
    PeerDiscovered(PeerId peer);
    GameStarted(string game_id);
    DiceRolled(u8 d1, u8 d2);
    GameEnded(string winner);
};
```

### Rust Core Architecture

#### Single Runtime Pattern
```rust
use once_cell::sync::Lazy;

// ONE runtime per process
static RUNTIME: Lazy<Runtime> = Lazy::new(|| {
    Runtime::new().expect("Failed to create Tokio runtime")
});

pub struct BitcrapsNode {
    mesh: Arc<MeshService>,
    events: Arc<Mutex<VecDeque<GameEvent>>>,
}

#[uniffi::export]
impl BitcrapsNode {
    pub async fn poll_event(&self) -> Option<GameEvent> {
        self.events.lock().unwrap().pop_front()
    }
}
```

---

## 4. 24-Week Development Roadmap

### Phase 1: Foundation & Platform Validation (Weeks 1-6)

#### Week 1: Kill-or-Cure Spikes
**Must pass before proceeding to Week 2**

| Day | Task | Success Criteria |
|-----|------|------------------|
| 1-2 | Android spike | Scan + advertise working with Foreground Service |
| 3-4 | iOS spike | Background discovery with service UUID |
| 5 | Go/No-go decision | Both platforms viable or pivot required |

#### Weeks 2-3: Security Foundation
- Threat modeling exercise
- Security audit preparation
- Chaos engineering framework
- Property-based testing

#### Weeks 4-6: Core Infrastructure
- Physical device test lab setup (10+ devices)
- UniFFI contract finalization
- Protocol versioning implementation
- CI/CD pipeline setup

### Phase 2: Mobile Implementation (Weeks 7-12)

#### Weeks 7-9: Android Development
- Jetpack Compose UI implementation
- Material 3 design system
- Foreground Service stabilization
- Battery optimization

#### Weeks 10-12: iOS Development
- SwiftUI implementation
- Background mode handling
- Energy efficiency
- TestFlight distribution

### Phase 3: Integration & Testing (Weeks 13-18)

#### Weeks 13-15: Cross-Platform Testing
- Android ↔ iOS interoperability
- Desktop ↔ Mobile compatibility
- Network resilience testing
- Performance optimization

#### Weeks 16-18: Security Audit
- Third-party penetration testing
- Vulnerability remediation
- Compliance verification
- Documentation updates

### Phase 4: Production Hardening (Weeks 19-24)

#### Weeks 19-21: Scalability
- Load testing (1000+ concurrent)
- Network optimization
- Persistent storage
- Monitoring implementation

#### Weeks 22-24: Platform Features
- Multi-game framework
- Analytics integration
- Developer SDK
- Operations tooling

### Phase 5: Launch (Weeks 25-28)

#### Weeks 25-26: App Store Preparation
- Store listing optimization
- Review preparation
- Beta testing program
- Marketing materials

#### Weeks 27-28: Launch & Support
- Soft launch
- Performance monitoring
- User feedback
- Rapid iteration

---

## 5. Testing & Quality Assurance

### Physical Device Matrix (REQUIRED)

**Android Devices (Minimum)**
- Pixel 6/7 (API 34) - Latest Android
- Samsung S21/S22 (API 33) - OEM testing
- OnePlus 9 (API 31) - Android 12 baseline
- Xiaomi Mi 11 (API 30) - Aggressive battery
- Pixel 3a (API 29) - Older common device

**iOS Devices (Minimum)**
- iPhone 14/15 (iOS 17) - Latest
- iPhone 12 (iOS 16) - Common
- iPhone SE 2 (iOS 15) - Budget
- iPad Air (iOS 16) - Tablet

**Testing Infrastructure**
- ❌ Cloud farms (NO BLE support)
- ✅ Physical device bench
- ✅ Automated test orchestration
- ✅ Cross-platform pairing tests

### Test Scenarios

#### Critical Path Testing
1. **Discovery**: Find peers within 30 seconds
2. **Connection**: Establish game within 10 seconds
3. **Gameplay**: Complete game without disconnection
4. **Recovery**: Reconnect after network failure
5. **Battery**: <5% drain per hour active use

#### Edge Cases
- Network partition (split brain)
- Byzantine nodes (33% malicious)
- Clock skew (>5 minutes)
- Memory pressure
- Storage full
- Background/foreground transitions

### Quality Metrics

| Metric | Target | Measurement |
|--------|--------|-------------|
| Crash Rate | <0.1% | Crashlytics |
| ANR Rate | <0.01% | Play Console |
| Memory Leaks | 0 | Instruments |
| Test Coverage | >95% core | Codecov |
| Performance | 60fps | Systrace |

---

## 6. Codebase Health Status

### Current State (as of 2025-08-23)

**Compilation Status**: ❌ **FAILING** - 2 critical errors preventing test compilation
**Code Quality**: ⚠️ **47 WARNINGS** - Significant technical debt
**Test Coverage**: ⚠️ **INCOMPLETE** - Missing test implementations
**Architecture**: ✅ **EXCELLENT** - Clean modular design
**Security**: ✅ **STRONG** - Well-implemented cryptography
**Documentation**: ✅ **GOOD** - Comprehensive inline docs

### Critical Issues Blocking Development

1. **Compilation Errors** (src/resilience/mod.rs:552)
   - Closure lifetime and mutability issues
   - Prevents running test suite
   - **Impact**: Cannot validate functionality

2. **Compiler Warnings** (47 total)
   - 4 unused imports
   - 13 unused variables  
   - 20+ unused fields
   - 3 unsafe operations
   - 6 lifetime hiding issues
   - **Impact**: Technical debt accumulation

3. **Incomplete Test Implementations**
   - ConsensusVote functionality missing
   - MessageCompressor not implemented
   - DeterministicRng placeholder
   - **Impact**: Reduced test coverage

### Code Quality Metrics

| Metric | Current | Target | Status |
|--------|---------|--------|--------|
| Compilation | 2 errors | 0 errors | ❌ |
| Warnings | 47 | <5 | ❌ |
| Test Coverage | ~85% | >95% | ⚠️ |
| Documentation | Good | Excellent | ✅ |
| Architecture | Clean | Clean | ✅ |

## 7. Production Infrastructure Status

### ✅ Completed Components

#### Configuration Management
- Environment-based config (dev/staging/prod)
- TOML configuration with validation
- Runtime overrides via environment
- **Location**: `src/config/mod.rs`

#### Database Layer
- Connection pooling with WAL mode
- Transaction support with rollback
- Backup and recovery systems
- **Location**: `src/database/mod.rs`

#### Input Validation
- Rate limiting (token bucket)
- SQL/XSS injection prevention
- Binary data validation
- **Location**: `src/validation/mod.rs`

#### Production Logging
- Structured logging with trace IDs
- Prometheus metrics export
- Multiple output backends
- **Location**: `src/logging/mod.rs`

#### Network Resilience
- Circuit breakers
- Automatic reconnection
- Retry policies with jitter
- **Location**: `src/resilience/mod.rs`

#### Key Management
- Encrypted storage
- Rotation policies
- Audit logging
- **Location**: `src/keystore/mod.rs`

### 🚧 Remaining Infrastructure

- [ ] Kubernetes deployment manifests
- [ ] Terraform infrastructure as code
- [ ] Grafana dashboards
- [ ] Jaeger tracing setup
- [ ] Alert rules configuration

---

## 7. Risk Mitigation

### Critical Risks

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| BLE Fragmentation | High | High | Extensive device testing, fallback protocols |
| App Store Rejection | High | Medium | Early review, compliance focus |
| Security Vulnerability | Critical | Low | Multiple audits, bug bounty |
| Battery Complaints | Medium | High | Aggressive optimization, user controls |
| Network Scalability | High | Low | Sharding, gateway nodes |

### Platform-Specific Risks

#### Android
- **Doze Mode**: Foreground Service helps but not immune
- **OEM Battery**: Samsung/Xiaomi aggressive optimization
- **Fragmentation**: Different BLE stacks per manufacturer
- **Play Policy**: Must justify Foreground Service usage

#### iOS
- **Background Limits**: Severe BLE restrictions
- **App Review**: Strict Bluetooth usage scrutiny
- **TestFlight**: Different behavior than production
- **State Restoration**: Complex Bluetooth restoration

### Mitigation Strategies

1. **Early Validation**: Week 1 spikes before commitment
2. **Physical Testing**: Real devices, not simulators
3. **Compliance First**: Follow platform guidelines exactly
4. **User Education**: Clear battery usage expectations
5. **Fallback Options**: TCP/IP when BLE unavailable

---

## 8. Success Metrics

### Technical KPIs

| Metric | Target | Priority |
|--------|--------|----------|
| Security Vulnerabilities | 0 critical | P0 |
| Consensus Latency | <500ms p99 | P1 |
| Battery Drain | <5%/hour active | P1 |
| Memory Usage | <150MB baseline | P2 |
| Network Efficiency | 60-80% compression | P2 |
| Cache Hit Rate | >85% | P3 |

### Business KPIs

| Metric | Target | Priority |
|--------|--------|----------|
| Time to Market | 24-28 weeks | P1 |
| Development Cost | Within 10% budget | P1 |
| App Store Rating | >4.5 stars | P2 |
| User Retention | >40% DAU/MAU | P2 |
| Platform Parity | 100% features | P1 |

### Launch Criteria

**Must Have**
- [ ] All P0 fixes complete
- [ ] Security audit passed
- [ ] 10+ device testing passed
- [ ] App store approval
- [ ] <0.1% crash rate

**Should Have**
- [ ] Battery <5%/hour
- [ ] 99.9% uptime
- [ ] Monitoring operational
- [ ] Documentation complete

**Nice to Have**
- [ ] Multi-game support
- [ ] Internet gateway
- [ ] Analytics dashboard

---

## Team Structure

### Core Team (Full-time)
- 2x Senior Rust Engineers (Core/Infrastructure)
- 1x Android Engineer (Kotlin/Compose expertise)
- 1x iOS Engineer (Swift/SwiftUI expertise)
- 1x Security Engineer
- 1x DevOps/SRE Engineer
- 1x QA Engineer
- 1x Product Manager
- 1x UI/UX Designer

### Extended Team (Part-time/Contract)
- Security Auditor (External firm)
- Performance Consultant
- Legal Advisor (Gambling regulations)
- Marketing Specialist
- Community Manager

### Skill Requirements

**Critical Skills**
- Rust systems programming
- Bluetooth Low Energy protocols
- Mobile native development (Kotlin/Swift)
- Distributed systems design
- Applied cryptography
- Security engineering

**Beneficial Skills**
- Game development experience
- Real-time systems
- Network protocol design
- DevOps/Kubernetes
- Data analytics
- Community management

---

## Budget Allocation

| Category | % Budget | Notes |
|----------|----------|-------|
| Engineering | 60% | 9 engineers × 6 months |
| Infrastructure | 10% | Cloud, devices, services |
| Security | 10% | Audits, bug bounty |
| Testing | 10% | Devices, automation |
| Legal | 5% | Compliance, privacy |
| Marketing | 5% | Launch campaign |

### Cost Optimization Strategies
- Use open-source tools where possible
- Leverage cloud credits for startups  
- Implement cost monitoring early
- Automate repetitive tasks
- Consider remote team members
- Use spot instances for non-critical workloads
- Negotiate volume discounts for services

---

## Platform Compliance Checklists

### Android Compliance
- [x] BLUETOOTH_ADVERTISE permission added
- [x] Runtime permissions for API 31+ implemented
- [x] Foreground Service with type declared
- [x] User notification for background operation
- [x] JavaVM storage pattern (not JNIEnv)
- [x] btleplug Java module included
- [ ] Play Store policies reviewed
- [ ] Data safety form completed
- [ ] Target API level current

### iOS Compliance  
- [x] NSBluetoothAlwaysUsageDescription only
- [x] UIBackgroundModes only if needed
- [x] Service UUID-based discovery
- [x] No reliance on background local name
- [x] XCFramework with SPM wrapper
- [ ] App Store guidelines reviewed
- [ ] Export compliance documentation
- [ ] Privacy nutrition labels completed
- [ ] TestFlight beta tested

### UniFFI Compliance
- [x] UDL as single source of truth
- [x] Async methods throughout
- [x] No callback interfaces
- [x] Event polling or streams
- [ ] Version compatibility tested
- [ ] Memory management validated

### Testing Compliance
- [x] Physical device matrix defined
- [x] Cloud farms for UI only
- [x] BLE testing on real hardware
- [x] Cross-platform test scenarios
- [ ] Accessibility testing complete
- [ ] Performance benchmarks met

---

## Sprint Planning

### 2-Week Sprint Cycles

| Sprints | Focus | Priority |
|---------|-------|----------|
| Sprint 1-2 | Platform Spikes & Critical Fixes | P0 |
| Sprint 3-6 | Security & Core Development | P1 |
| Sprint 7-10 | Mobile Implementation | P2 |
| Sprint 11-14 | Performance & Testing | P3 |
| Sprint 15-18 | Platform Features | P4 |
| Sprint 19-20 | Launch Preparation | P5 |

### Daily Standup Focus Areas
1. **Blockers**: P0 fixes and platform issues
2. **Security**: Audit findings and vulnerabilities  
3. **Testing**: Device testing results
4. **Performance**: Metrics and benchmarks
5. **Feedback**: User and stakeholder input

## Definition of Done

### Feature Complete Criteria
- [ ] All P0 fixes implemented and tested
- [ ] Security audit passed with no critical issues
- [ ] Physical device testing on 10+ devices complete
- [ ] Performance targets met (<5% battery, <150MB memory)
- [ ] Documentation complete and reviewed
- [ ] App store approval obtained

### Production Ready Criteria  
- [ ] 99.9% uptime achieved in staging environment
- [ ] Monitoring and alerting fully operational
- [ ] Disaster recovery procedures tested
- [ ] Support procedures documented and trained
- [ ] Legal review complete and approved
- [ ] Launch plan approved by stakeholders

## Next Actions (Immediate)

### Today (Day 1)
1. Fix Android BLUETOOTH_ADVERTISE permission
2. Implement Foreground Service with type
3. Fix iOS Info.plist keys
4. Start btleplug Android spike

### This Week
1. Complete platform validation spikes
2. Make go/no-go decision
3. Set up physical device lab
4. Finalize UniFFI contract
5. Begin security audit prep

### This Month
1. Complete security foundation
2. Implement core mobile features
3. Begin cross-platform testing
4. Launch beta program

---

## Conclusion

This master plan provides a comprehensive, realistic path to launching BitCraps as the first production-ready, security-audited, cross-platform decentralized casino. The week 1 validation spikes ensure platform viability before major investment, while the parallel track approach maximizes development efficiency.

Success depends on:
1. Immediate platform fixes (P0)
2. Rigorous physical device testing
3. Security-first development
4. Platform compliance focus
5. Performance optimization

With proper execution, BitCraps will launch in 24-28 weeks with enterprise-grade infrastructure, native mobile support, and a robust decentralized gaming protocol.

---

*Document Version: 3.0*  
*Last Updated: 2025-08-23*  
*Status: Active Development*  
*Review Cycle: Weekly*  
*Owner: BitCraps Development Team*

---

## Appendix A: Configuration Examples

### Development Configuration (config/development.toml)
```toml
[app]
name = "BitCraps"
environment = "development"
log_level = "debug"

[network]
listen_port = 8334
max_connections = 50
enable_bluetooth = true

[security]
pow_difficulty = 8
enable_rate_limiting = true
rate_limit_requests = 100

[game]
min_bet = 10
max_bet = 10000
house_edge = 0.0136
```

### Production Configuration (config/production.toml)
```toml
[app]
name = "BitCraps"
environment = "production"
log_level = "info"

[network]
listen_port = 8333
max_connections = 1000
enable_bluetooth = true
enable_compression = true

[security]
pow_difficulty = 20
enable_rate_limiting = true
enable_tls = true

[monitoring]
enable_metrics = true
metrics_port = 9090
```

---

## Appendix B: Command Reference

### Development Commands
```bash
# Build for all platforms
make build-all

# Run tests
cargo test --all-features

# Start development server
cargo run -- --config config/development.toml

# Build Android
cargo ndk -t arm64-v8a build --release

# Build iOS
cargo lipo --release
```

### Deployment Commands
```bash
# Deploy to staging
kubectl apply -f k8s/staging/

# Run security scan
cargo audit

# Generate documentation
cargo doc --no-deps

# Performance profiling
cargo bench
```

---

## Appendix C: Repository Structure

```
bitcraps/
├── src/
│   ├── config/          # Configuration management
│   ├── database/        # Database layer
│   ├── validation/      # Input validation
│   ├── logging/         # Production logging
│   ├── resilience/      # Network resilience
│   ├── keystore/        # Key management
│   ├── protocol/        # Core protocol
│   ├── transport/       # Network transport
│   ├── mesh/           # Mesh networking
│   └── lib.rs          # Library root
├── android/            # Android app
│   ├── app/
│   └── gradle/
├── ios/               # iOS app
│   ├── BitCraps/
│   └── BitCraps.xcodeproj/
├── tests/             # Integration tests
├── benches/           # Performance benchmarks
├── docs/              # Documentation
│   └── MASTER_DEVELOPMENT_PLAN.md
├── config/            # Configuration files
├── Cargo.toml         # Rust dependencies
└── README.md          # Project overview
```

---

*End of Master Development Plan*