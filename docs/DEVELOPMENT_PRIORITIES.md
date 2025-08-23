# BitCraps Development Priorities
## Master Priority List with Critical Corrections

This document provides the definitive, prioritized list of development tasks incorporating ALL feedback from engineering reviews and senior engineer corrections.

---

## 🔴 Priority 0: IMMEDIATE FIXES (Week 1)
**These MUST be fixed before any other development proceeds**

### Android Critical Fixes
1. **Add BLUETOOTH_ADVERTISE permission** - App will fail without this
2. **Implement Foreground Service with type** - Required for Android 14+
3. **Fix JNI thread safety** - Store JavaVM, not JNIEnv
4. **Include btleplug Java module** - Required for BLE to work
5. **Runtime permissions for API 31+** - "Nearby devices" group

### iOS Critical Fixes
1. **Remove deprecated Info.plist keys** - Will fail App Review
2. **Fix background BLE assumptions** - No local names, service UUID overflow
3. **Implement service UUID filtering** - Required for background discovery

### Architecture Fixes
1. **UniFFI: Use UDL only** - No mixed patterns, causes confusion
2. **Replace callback interfaces** - Use async polling/streams
3. **One Tokio runtime per process** - Not per-thread
4. **Add protocol versioning** - u16 in handshake for compatibility

---

## 🟠 Priority 1: SECURITY & COMPLIANCE (Weeks 2-6)

### Security Infrastructure
1. **Formal Security Audit**
   - Third-party penetration testing
   - Threat modeling with STRIDE methodology
   - Byzantine fault tolerance validation (>33% malicious nodes)
   - Smart contract audit (if applicable)

2. **Cryptographic Hardening**
   - Android: Ed25519 in Keystore with AES fallback wrapper
   - iOS: CryptoKit + Keychain, Secure Enclave when available
   - Hardware security module (HSM) integration
   - Key rotation automation

3. **Compliance & Privacy**
   - GDPR/CCPA compliance implementation
   - Privacy policy generation
   - Data retention policies
   - Right to deletion implementation

### Testing Infrastructure
1. **Physical Device Lab** (NOT cloud farms for BLE)
   - 5+ Android devices (API 29-34)
   - 4+ iOS devices (iOS 14-17)
   - Automated test orchestration
   - Cross-platform pairing tests

2. **Chaos Engineering**
   - Network partition simulation
   - Byzantine node injection
   - Message delay/drop testing
   - Clock skew simulation

---

## 🟡 Priority 2: MOBILE IMPLEMENTATION (Weeks 7-12)

### Android Implementation
1. **Core Integration**
   - Foreground Service with persistent notification
   - Doze mode handling
   - Battery optimization exemption request
   - OEM-specific workarounds (Samsung, Xiaomi)

2. **UI/UX Polish**
   - Material 3 design system
   - 60fps dice animations
   - Haptic feedback
   - Landscape/portrait support

### iOS Implementation
1. **Core Integration**
   - Background mode constraints handling
   - State restoration support
   - Energy efficiency optimization
   - TestFlight beta distribution

2. **UI/UX Polish**
   - SwiftUI with Metal rendering
   - Native iOS animations
   - Dynamic Type support
   - Dark mode support

### Cross-Platform
1. **Interoperability Testing**
   - Android ↔ iOS gameplay validation
   - Desktop ↔ Mobile compatibility
   - Protocol version negotiation
   - Mixed network scenarios

---

## 🟢 Priority 3: PERFORMANCE & SCALABILITY (Weeks 13-18)

### Performance Optimization
1. **Battery Optimization**
   - Target: <5% drain per hour active
   - Adaptive BLE scanning intervals
   - Duty cycling implementation
   - Power-aware connection management

2. **Memory Optimization**
   - Target: <150MB baseline
   - Per-connection memory pools (not global)
   - Efficient caching strategies
   - Memory pressure handling

3. **Network Optimization**
   - Enhanced connection pooling (10x capacity)
   - DHT optimization for mobile constraints
   - Adaptive MTU discovery
   - Message compression (60-80% reduction)

### Scalability Features
1. **Persistent Storage**
   - Crash-safe token ledger
   - Delta-compressed game history
   - Merkle tree verification
   - Peer synchronization protocol

2. **Network Extensions**
   - Internet gateway/bridge nodes
   - Multi-hop routing optimization
   - Sharding for large networks
   - Federation protocol design

---

## 🔵 Priority 4: PLATFORM & EXTENSIBILITY (Weeks 19-24)

### Production Infrastructure
1. **Monitoring & Observability**
   - Prometheus + Grafana dashboards
   - Distributed tracing (Jaeger)
   - Custom alerting rules
   - SLA monitoring (99.9% target)

2. **Deployment Automation**
   - CI/CD pipeline with GitHub Actions
   - Blue-green deployments
   - Automated rollback
   - Feature flag system

### Platform Features
1. **Multi-Game Framework**
   - Abstract game interface
   - Pluggable game modules
   - Shared consensus layer
   - Game editor tools

2. **Developer Experience**
   - Comprehensive API documentation
   - SDK for third-party integration
   - Example applications
   - Developer portal

---

## ⚪ Priority 5: LAUNCH PREPARATION (Weeks 25-28)

### App Store Preparation
1. **Google Play**
   - Store listing optimization
   - Foreground Service justification
   - Pre-registration campaign
   - A/B testing setup

2. **Apple App Store**
   - App Review preparation
   - TestFlight beta program
   - Marketing materials
   - Press kit

### Launch Support
1. **Operations**
   - 24/7 monitoring
   - Incident response procedures
   - Customer support system
   - Bug tracking workflow

2. **Marketing**
   - Launch campaign
   - Community building
   - Influencer outreach
   - Analytics tracking

---

## 📊 Success Metrics & KPIs

### Technical Metrics
| Metric | Target | Priority |
|--------|--------|----------|
| Security vulnerabilities | 0 critical | P0 |
| Crash rate | <0.1% | P1 |
| Battery drain (active) | <5%/hour | P2 |
| Memory usage | <150MB | P2 |
| Consensus latency | <500ms | P3 |
| Test coverage | >95% core | P3 |

### Business Metrics
| Metric | Target | Priority |
|--------|--------|----------|
| Time to market | 24-28 weeks | P1 |
| App store rating | >4.5 stars | P2 |
| User retention | >40% DAU/MAU | P3 |
| Platform parity | 100% | P1 |

---

## ⚠️ Critical Dependencies

### Must Have Before Launch
1. ✅ All P0 fixes implemented
2. ✅ Security audit passed
3. ✅ Physical device testing complete
4. ✅ App store compliance verified
5. ✅ Privacy policy approved

### Technical Dependencies
1. **btleplug stability** - Core BLE functionality
2. **UniFFI compatibility** - Cross-platform bindings
3. **Tokio runtime** - Async operations
4. **Device availability** - Physical test devices

---

## 🚧 Known Risks & Mitigations

### High Risk Items
1. **BLE Fragmentation**
   - Risk: Different behavior across devices
   - Mitigation: Extensive physical testing, fallback protocols

2. **App Store Rejection**
   - Risk: Gambling regulations, technical issues
   - Mitigation: Early review, legal consultation, compliance focus

3. **Battery Complaints**
   - Risk: User uninstalls due to drain
   - Mitigation: Aggressive optimization, user controls, clear communication

### Medium Risk Items
1. **Network Scalability**
   - Risk: Performance degradation at scale
   - Mitigation: Load testing, sharding, gateway nodes

2. **Cross-Platform Bugs**
   - Risk: Platform-specific issues
   - Mitigation: Comprehensive testing matrix, CI/CD

---

## 📅 Sprint Planning

### 2-Week Sprint Cycles

**Sprint 1-2**: Platform Spikes & Fixes (P0)
**Sprint 3-6**: Security & Core Development (P1)
**Sprint 7-10**: Mobile Implementation (P2)
**Sprint 11-14**: Performance & Testing (P3)
**Sprint 15-18**: Platform Features (P4)
**Sprint 19-20**: Launch Preparation (P5)

### Daily Standup Focus Areas
1. Blockers from P0 fixes
2. Security audit findings
3. Device testing results
4. Performance metrics
5. User feedback

---

## 📝 Documentation Requirements

### Technical Documentation
1. **Architecture diagrams** - System design
2. **API reference** - Complete endpoint docs
3. **Protocol specification** - Wire format
4. **Security model** - Threat analysis
5. **Operations runbook** - Deployment & monitoring

### User Documentation
1. **User guide** - Getting started
2. **FAQ** - Common issues
3. **Troubleshooting** - Problem resolution
4. **Privacy policy** - Data handling
5. **Terms of service** - Legal framework

---

## ✅ Definition of Done

### Feature Complete Criteria
- [ ] All P0 fixes implemented and tested
- [ ] Security audit passed with no criticals
- [ ] Physical device testing on 10+ devices
- [ ] Performance targets met
- [ ] Documentation complete
- [ ] App store approval obtained

### Production Ready Criteria
- [ ] 99.9% uptime achieved in staging
- [ ] Monitoring and alerting operational
- [ ] Disaster recovery tested
- [ ] Support procedures documented
- [ ] Legal review complete
- [ ] Launch plan approved

---

## 🎯 Next Actions (Immediate)

1. **TODAY**: Fix Android permissions and Foreground Service
2. **TODAY**: Fix iOS Info.plist and background assumptions
3. **TOMORROW**: Complete platform spikes on physical devices
4. **THIS WEEK**: Finalize UniFFI UDL contract
5. **THIS WEEK**: Set up physical device test bench

---

*Document Version: 1.0*  
*Last Updated: 2025-08-23*  
*Status: Active Development*  
*Owner: BitCraps Development Team*