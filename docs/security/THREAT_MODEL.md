# BitCraps STRIDE Threat Model

## Executive Summary

This document presents a comprehensive threat model for the BitCraps decentralized casino platform using the STRIDE methodology. The analysis identifies potential security threats across the mesh networking, consensus, and gaming layers, with specific focus on BLE/P2P attack vectors.

**Document Version**: 1.0  
**Date**: 2025-08-24  
**Classification**: Security Sensitive  
**Review Cycle**: Quarterly

---

## System Overview

### Architecture Components

```
┌─────────────────────────────────────────────────────┐
│                   Mobile Client                      │
│  ┌─────────────┐  ┌──────────┐  ┌──────────────┐  │
│  │   UI Layer  │  │   Game    │  │   Wallet     │  │
│  │  (Android/  │  │   Logic   │  │  (Key Mgmt)  │  │
│  │    iOS)     │  │           │  │              │  │
│  └──────┬──────┘  └─────┬─────┘  └──────┬───────┘  │
│         └────────────────┼───────────────┘          │
│                          │                          │
│  ┌───────────────────────▼───────────────────────┐  │
│  │           BitCraps Core (Rust)                │  │
│  │  ┌─────────┐  ┌───────────┐  ┌────────────┐ │  │
│  │  │Consensus│  │  Protocol │  │   Crypto   │ │  │
│  │  │ Engine  │  │  Handler  │  │  Module    │ │  │
│  │  └─────────┘  └───────────┘  └────────────┘ │  │
│  └───────────────────────┬───────────────────────┘  │
│                          │                          │
│  ┌───────────────────────▼───────────────────────┐  │
│  │            Transport Layer                     │  │
│  │  ┌──────────┐  ┌──────────┐  ┌────────────┐ │  │
│  │  │   BLE    │  │  TCP/IP  │  │    Mesh    │ │  │
│  │  │ Transport│  │ Transport│  │  Routing   │ │  │
│  │  └──────────┘  └──────────┘  └────────────┘ │  │
│  └────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────┘
```

### Trust Boundaries

1. **Device Boundary**: Between user device and external world
2. **Process Boundary**: Between UI and core Rust library
3. **Network Boundary**: Between local device and peer devices
4. **Consensus Boundary**: Between honest and potentially malicious nodes

---

## STRIDE Analysis

## S - Spoofing Identity

### Threat S1: Peer Identity Spoofing
**Component**: Mesh Network Discovery  
**Attack Vector**: Attacker creates fake peer identity without PoW  
**Impact**: High - Can join network without proper authorization  
**Likelihood**: Medium  
**Current Mitigation**: 
- Ed25519 signature verification
- Proof-of-work identity generation (difficulty: 10)
- Peer ID validation

**Additional Mitigations Needed**:
- [ ] Increase PoW difficulty for production (20+)
- [ ] Implement identity revocation mechanism
- [ ] Add reputation scoring system

### Threat S2: Game State Spoofing
**Component**: Consensus Engine  
**Attack Vector**: Malicious node claims false game state  
**Impact**: Critical - Could manipulate game outcomes  
**Likelihood**: Low  
**Current Mitigation**:
- 2/3 consensus threshold
- Merkle tree state verification
- Signature verification on all state updates

**Risk Level**: **LOW** (Well mitigated)

### Threat S3: BLE Device Spoofing
**Component**: Bluetooth Transport  
**Attack Vector**: Attacker spoofs legitimate device MAC address  
**Impact**: Medium - Could intercept or redirect connections  
**Likelihood**: Medium  
**Current Mitigation**:
- Service UUID verification
- Post-connection cryptographic handshake
- Session encryption

**Additional Mitigations Needed**:
- [ ] Implement device fingerprinting
- [ ] Add connection rate limiting per MAC

---

## T - Tampering with Data

### Threat T1: Message Tampering in Transit
**Component**: Network Transport  
**Attack Vector**: Man-in-the-middle modifies packets  
**Impact**: High - Could alter game commands or outcomes  
**Likelihood**: Low  
**Current Mitigation**:
- Ed25519 signature on all messages
- HMAC integrity verification
- Session encryption (planned)

**Risk Level**: **LOW** (Well mitigated)

### Threat T2: Consensus Message Manipulation
**Component**: Consensus Protocol  
**Attack Vector**: Byzantine node sends conflicting messages  
**Impact**: High - Could cause consensus failure  
**Likelihood**: Medium  
**Current Mitigation**:
- Signature verification
- Message ordering enforcement
- Timeout mechanisms

**Additional Mitigations Needed**:
- [ ] Implement view change protocol
- [ ] Add equivocation detection and slashing

### Threat T3: Local Storage Tampering
**Component**: Database/Keystore  
**Attack Vector**: Malicious app or root access modifies local data  
**Impact**: Critical - Could steal funds or manipulate state  
**Likelihood**: Low  
**Current Mitigation**:
- Encrypted key storage (ChaCha20Poly1305)
- File permission restrictions (0600)
- Integrity checks on database

**Risk Level**: **MEDIUM** (Mobile platforms vulnerable to jailbreak/root)

---

## R - Repudiation

### Threat R1: Bet Repudiation
**Component**: Game Protocol  
**Attack Vector**: Player denies placing losing bet  
**Impact**: High - Loss of game integrity  
**Likelihood**: Low  
**Current Mitigation**:
- Signed bet commitments
- Consensus agreement on all bets
- Immutable game history in Merkle tree

**Risk Level**: **LOW** (Well mitigated)

### Threat R2: Consensus Vote Repudiation
**Component**: Consensus Engine  
**Attack Vector**: Node denies sending specific vote  
**Impact**: Medium - Could delay consensus  
**Likelihood**: Low  
**Current Mitigation**:
- All votes are signed
- Votes included in consensus record
- Penalty system for non-participation

**Additional Mitigations Needed**:
- [ ] Implement persistent vote log
- [ ] Add vote aggregation signatures

---

## I - Information Disclosure

### Threat I1: Dice Roll Prediction
**Component**: Random Number Generation  
**Attack Vector**: Attacker predicts future dice rolls  
**Impact**: Critical - Complete game compromise  
**Likelihood**: Very Low  
**Current Mitigation**:
- Commit-reveal scheme
- Collective randomness generation
- ChaCha20 CSPRNG

**Risk Level**: **LOW** (Strong cryptographic protection)

### Threat I2: Private Key Exposure
**Component**: Keystore  
**Attack Vector**: Memory dump, side-channel, or malware  
**Impact**: Critical - Complete account compromise  
**Likelihood**: Low  
**Current Mitigation**:
- Key encryption at rest
- Zeroization on drop
- Memory protection

**Additional Mitigations Needed**:
- [ ] Implement secure enclave support (iOS)
- [ ] Add Android Keystore integration
- [ ] Implement key derivation from biometrics

### Threat I3: Network Topology Disclosure
**Component**: Mesh Routing  
**Attack Vector**: Traffic analysis reveals network structure  
**Impact**: Low - Privacy concern  
**Likelihood**: High  
**Current Mitigation**:
- Epidemic routing obscures paths
- No persistent peer relationships

**Risk Level**: **MEDIUM** (Privacy implications)

---

## D - Denial of Service

### Threat D1: Connection Exhaustion
**Component**: Transport Layer  
**Attack Vector**: Attacker opens many connections  
**Impact**: High - Service unavailable  
**Likelihood**: High  
**Current Mitigation**:
- Connection limits (100 total, 3 per peer)
- Rate limiting (10 new connections/minute)
- Connection cooldown (60 seconds)

**Risk Level**: **LOW** (Well mitigated)

### Threat D2: Consensus Stalling
**Component**: Consensus Engine  
**Attack Vector**: Byzantine nodes refuse to participate  
**Impact**: High - Game cannot progress  
**Likelihood**: Medium  
**Current Mitigation**:
- Timeout mechanisms (30 seconds)
- Force settlement after timeout
- Penalty system for non-participation

**Additional Mitigations Needed**:
- [ ] Implement leader election
- [ ] Add fast recovery protocol

### Threat D3: BLE Jamming
**Component**: Bluetooth Transport  
**Attack Vector**: Radio frequency interference  
**Impact**: High - Complete communication failure  
**Likelihood**: Low (requires physical proximity)  
**Current Mitigation**:
- Fallback to TCP/IP transport
- Automatic reconnection
- Mesh routing alternatives

**Risk Level**: **MEDIUM** (Physical attack vector)

### Threat D4: Memory Exhaustion
**Component**: Message Processing  
**Attack Vector**: Send large/complex messages  
**Impact**: High - Application crash  
**Likelihood**: Medium  
**Current Mitigation**:
- Message size limits (65KB)
- Memory pooling and recycling
- Garbage collection

**Risk Level**: **LOW** (Well mitigated)

---

## E - Elevation of Privilege

### Threat E1: Consensus Authority Hijacking
**Component**: Consensus Engine  
**Attack Vector**: Attacker gains >33% control  
**Impact**: Critical - Can manipulate all games  
**Likelihood**: Very Low  
**Current Mitigation**:
- Byzantine fault tolerance (up to 33%)
- Sybil resistance via PoW
- Peer diversity requirements

**Additional Mitigations Needed**:
- [ ] Implement stake-based voting weight
- [ ] Add geographic distribution requirements

### Threat E2: Admin Function Access
**Component**: Game Protocol  
**Attack Vector**: Exploit to access privileged functions  
**Impact**: Critical - Could mint tokens or modify rules  
**Likelihood**: Very Low  
**Current Mitigation**:
- No admin functions in protocol
- Decentralized governance model
- Immutable smart contract design

**Risk Level**: **LOW** (By design)

### Threat E3: Mobile Platform Privilege Escalation
**Component**: Android/iOS App  
**Attack Vector**: Exploit OS vulnerability  
**Impact**: High - Access to keystore  
**Likelihood**: Low  
**Current Mitigation**:
- Minimal permission requirements
- Sandboxed execution
- No root/jailbreak detection (privacy-preserving)

**Additional Mitigations Needed**:
- [ ] Implement app attestation
- [ ] Add runtime integrity checks

---

## Attack Vectors Specific to BLE/P2P

### 1. BlueBorne-Style Attacks
**Threat**: Remote code execution via BLE  
**Mitigation**: 
- Input validation on all BLE data
- Minimal BLE stack exposure
- Regular security updates

### 2. BLE Tracking
**Threat**: Device tracking via BLE advertisements  
**Mitigation**:
- MAC address randomization
- Rotating service UUIDs
- Minimal advertisement data

### 3. Mesh Partitioning
**Threat**: Isolate nodes from network  
**Mitigation**:
- Multiple transport options
- Mesh healing algorithms
- Gateway nodes for internet bridge

### 4. Eclipse Attacks
**Threat**: Surround node with malicious peers  
**Mitigation**:
- Peer diversity requirements
- Random peer selection
- Reputation system

---

## Risk Matrix

| Threat | Impact | Likelihood | Risk Level | Status |
|--------|--------|------------|------------|---------|
| Dice Roll Prediction | Critical | Very Low | LOW | ✅ Mitigated |
| Private Key Exposure | Critical | Low | MEDIUM | ⚠️ Partial |
| Consensus Authority Hijacking | Critical | Very Low | LOW | ✅ Mitigated |
| Game State Spoofing | Critical | Low | LOW | ✅ Mitigated |
| Local Storage Tampering | Critical | Low | MEDIUM | ⚠️ Partial |
| Message Tampering | High | Low | LOW | ✅ Mitigated |
| Connection Exhaustion | High | High | LOW | ✅ Mitigated |
| Consensus Stalling | High | Medium | MEDIUM | ⚠️ Partial |
| BLE Jamming | High | Low | MEDIUM | ⚠️ Partial |
| Network Topology Disclosure | Low | High | MEDIUM | ⚠️ Partial |

---

## Security Controls Summary

### Implemented Controls ✅
1. **Cryptographic Protection**
   - Ed25519 signatures
   - ChaCha20Poly1305 encryption
   - SHA256 commitments
   - HMAC integrity

2. **Access Control**
   - Proof-of-work identity
   - Connection limits
   - Rate limiting
   - Permission model

3. **Byzantine Fault Tolerance**
   - 2/3 consensus threshold
   - Timeout mechanisms
   - Penalty system
   - Merkle tree verification

4. **Input Validation**
   - Message size limits
   - Type checking
   - Sanitization
   - Binary validation

### Planned Controls 🔄
1. **Enhanced Authentication**
   - Biometric integration
   - Hardware security module
   - Multi-factor authentication

2. **Advanced Consensus**
   - View change protocol
   - Equivocation slashing
   - Leader election

3. **Monitoring & Detection**
   - Anomaly detection
   - Security event logging
   - Intrusion detection

---

## Recommendations

### Critical Priority (Week 2)
1. Implement secure enclave integration for key storage
2. Add equivocation detection in consensus
3. Increase PoW difficulty for mainnet

### High Priority (Week 3)
1. Implement stake-based voting weights
2. Add runtime integrity checking
3. Deploy monitoring and alerting

### Medium Priority (Week 4+)
1. Implement privacy-preserving features
2. Add formal verification of consensus
3. Conduct penetration testing

---

## Compliance Mapping

### OWASP Mobile Top 10
- M1: Improper Platform Usage ✅ Mitigated
- M2: Insecure Data Storage ⚠️ Partial (needs secure enclave)
- M3: Insecure Communication ✅ Mitigated
- M4: Insecure Authentication ✅ Mitigated
- M5: Insufficient Cryptography ✅ Mitigated
- M6: Insecure Authorization ✅ Mitigated
- M7: Client Code Quality ✅ Mitigated
- M8: Code Tampering ⚠️ Partial (needs attestation)
- M9: Reverse Engineering ⚠️ Acceptable risk
- M10: Extraneous Functionality ✅ Mitigated

### GDPR/Privacy
- Data minimization ✅
- Encryption at rest ✅
- Right to erasure ✅
- Privacy by design ✅

---

## Conclusion

The BitCraps platform demonstrates strong security architecture with comprehensive protection against most STRIDE categories. The primary areas requiring attention are:

1. **Mobile platform security** - Integration with hardware security features
2. **Advanced consensus** - Additional Byzantine fault tolerance mechanisms
3. **Privacy enhancements** - Protection against traffic analysis

Overall risk level: **LOW to MEDIUM** with clear mitigation paths for all identified threats.

---

## Appendix A: Attack Tree

```
Root: Compromise BitCraps Game
├── Manipulate Game Outcome
│   ├── Predict Dice Rolls [MITIGATED]
│   ├── Control Consensus [MITIGATED]
│   └── Tamper with State [MITIGATED]
├── Steal User Funds
│   ├── Extract Private Keys [PARTIAL]
│   ├── Replay Transactions [MITIGATED]
│   └── Social Engineering [OUT OF SCOPE]
└── Disrupt Service
    ├── DoS Attack [MITIGATED]
    ├── Consensus Stalling [PARTIAL]
    └── Network Partitioning [PARTIAL]
```

---

## Appendix B: Data Flow Diagram

```
User Input → Validation → Game Logic → Consensus → State Update
     ↓           ↓            ↓            ↓            ↓
  Keystore    Sanitize    Sign Msg    Broadcast    Merkle Tree
     ↓           ↓            ↓            ↓            ↓
  Sign Tx    Rate Limit   Transport    Verify      Database
```

---

*End of Threat Model Document*