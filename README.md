# BitCraps: Decentralized Casino Protocol in Rust 🎲

## Learning Journey with Feynman-Style Explanations

This repository contains a complete 6-week implementation guide for building BitCraps, a decentralized peer-to-peer casino protocol. Each week's documentation includes detailed Feynman-style explanations that break down complex concepts into simple, intuitive understanding.

## What is the Feynman Technique?

Richard Feynman's learning method: **"If you can't explain it simply, you don't understand it well enough."**

Throughout this codebase, you'll find explanations that:
1. Use simple analogies (e.g., "XOR distance is like measuring how different two phone numbers are")
2. Build from first principles (start with bytes, build up to protocols)
3. Connect abstract concepts to real-world examples (DHT routing = city highway system)
4. Explain the "why" not just the "how"

## 🎯 Project Overview

BitCraps transforms the BitChat mesh networking protocol into a decentralized casino featuring:
- **64 Bet Types**: Complete craps implementation from Hackathon contracts
- **CRAP Tokens**: Bitcoin-style tokenomics with Proof-of-Relay mining
- **Mesh Networking**: P2P communication without central servers
- **Cryptographic Security**: Noise Protocol, PBFT consensus, VDF randomness
- **100+ Player Support**: Hierarchical sharding for massive multiplayer games

## 📚 Weekly Implementation Guide

### Week 1: Cryptographic Foundations & Protocol
**File**: `docs/week1.md`
- Noise Protocol Framework (XX handshake pattern)
- Binary serialization for network efficiency
- All 64 craps bet types with exact payouts
- CRAP token primitives

**Feynman Focus**: "How do we pack complex data into simple bytes and ensure everyone speaks the same language?"

### Week 2: Transport Layer & Mesh Networking
**File**: `docs/week2.md`
- Transport trait abstraction (TCP/UDP/Bluetooth)
- Kademlia DHT for O(log n) routing
- Eclipse attack prevention
- Proof-of-Work identity generation

**Feynman Focus**: "How do we build efficient 'highways' for messages without connecting everyone to everyone?"

### Week 3: Gaming Protocol & Mesh Services
**File**: `docs/week3.md`
- Complete craps game logic
- Table management for 2-100 players
- Bet resolution and payout calculations
- Anti-cheat mechanisms

**Feynman Focus**: "How do we ensure fair dice rolls when no one trusts anyone?"

### Week 4: Token Economics
**File**: `docs/week4.md`
- CRAP token implementation
- Proof-of-Relay consensus
- Mining and distribution
- Staking mechanisms

**Feynman Focus**: "How do we create digital money that rewards network participation?"

### Week 5: Security Enhancements
**File**: `docs/week5.md`
- PBFT consensus for Byzantine fault tolerance
- VDF for verifiable randomness
- Zero-knowledge proofs for privacy
- Collusion detection

**Feynman Focus**: "How do we prevent cheating in a system with no central authority?"

### Week 6: Production Features
**File**: `docs/week6.md`
- Terminal UI implementation
- Performance optimization
- Monitoring and metrics
- Deployment strategies

**Feynman Focus**: "How do we make a complex system simple to use?"

## 🚀 Getting Started

### Prerequisites
```bash
# Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Required dependencies (in Cargo.toml)
tokio = { version = "1.47", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
snow = "0.9"  # Noise Protocol
ed25519-dalek = "2.0"
sha2 = "0.10"
```

### Learning Path

1. **Read the Overview**: Start with `docs/bitcraps.md` for the complete vision
2. **Week-by-Week Implementation**: Follow each week's guide in order
3. **Type the Code**: Don't copy-paste! Type each implementation to build muscle memory
4. **Understand the Feynman Explanations**: For each function, understand the analogy
5. **Run Tests**: Each week has test cases to verify your implementation
6. **Experiment**: Modify parameters, add features, break things and fix them

### Building Your Implementation

```bash
# Start with Week 1
cd bitchat-rust
cargo build

# As you implement each week, uncomment the modules in src/lib.rs
# Test your implementation
cargo test

# Run the application
cargo run
```

## 🎲 The 64 Bet Types

All bet types from the Hackathon contracts are implemented:

| Category | Bet Types | Count |
|----------|-----------|-------|
| Core Line Bets | Pass, Don't Pass, Come, Don't Come | 4 |
| Field Bet | Standard field | 1 |
| YES Bets | Number before 7 (2-12 except 7) | 10 |
| NO Bets | 7 before number (2-12 except 7) | 10 |
| Hardways | Hard 4, 6, 8, 10 | 4 |
| Odds Bets | Pass/Don't/Come/Don't Come Odds | 4 |
| Special Bets | Fire, Hot Roller, Muggsy, etc. | 10 |
| NEXT Bets | Single roll propositions | 11 |
| Repeater Bets | Number must repeat N times | 10 |
| **Total** | | **64** |

## 💡 Key Concepts Explained

### Kademlia DHT
**Feynman**: "Like a phone book where you only need to know a few neighbors to find anyone in the world"

### Noise Protocol
**Feynman**: "A choreographed dance where two strangers become trusted friends through precise steps"

### PBFT Consensus
**Feynman**: "Like a jury that needs 2/3 agreement even if 1/3 are liars"

### VDF (Verifiable Delay Function)
**Feynman**: "A mathematical puzzle that MUST take time to solve - no shortcuts allowed"

### Zero-Knowledge Proofs
**Feynman**: "Proving you know a secret without revealing the secret itself"

## 🔧 Architecture

```
BitCraps Protocol Stack
├── Application Layer
│   ├── Gaming Logic (Craps rules, payouts)
│   └── Token System (CRAP tokens)
├── Consensus Layer
│   ├── PBFT (Byzantine agreement)
│   └── VDF (Randomness generation)
├── Network Layer
│   ├── Kademlia DHT (Peer discovery)
│   └── Transport (TCP/UDP/Bluetooth)
└── Cryptographic Layer
    ├── Noise Protocol (Encryption)
    └── Ed25519/Curve25519 (Signatures)
```

## 📈 Performance Targets

- **Routing**: O(log n) with Kademlia DHT
- **Consensus**: 2f+1 fault tolerance with PBFT
- **Throughput**: 1000+ bets/second per table
- **Latency**: <100ms for local mesh
- **Scale**: 100+ concurrent players per table

## 🛡️ Security Features

- **Sybil Resistance**: Proof-of-Work identities
- **Eclipse Prevention**: Redundant routing paths
- **Collusion Detection**: Statistical analysis of betting patterns
- **Fair Randomness**: VDF + commit-reveal + BLS threshold signatures
- **Privacy**: Zero-knowledge proofs for sensitive operations

## 📝 License

MIT OR Apache-2.0 (dual licensed)

## 🎓 Learning Resources

- [Noise Protocol Framework](http://noiseprotocol.org/)
- [Kademlia Paper](https://pdos.csail.mit.edu/~petar/papers/maymounkov-kademlia-lncs.pdf)
- [PBFT Original Paper](https://pmg.csail.mit.edu/papers/osdi99.pdf)
- [VDF Introduction](https://eprint.iacr.org/2018/601.pdf)

## 🚧 Development Status

This is a learning implementation. Each week's code is designed to be:
1. **Educational**: Clear over clever
2. **Incremental**: Builds on previous weeks
3. **Testable**: Includes test cases
4. **Production-Ready**: Could be deployed with minor modifications

## 💬 Support

For questions about the implementation:
1. Review the Feynman explanations in each file
2. Check the test cases for usage examples
3. Experiment with modifications to understand behavior
4. Remember: The best way to learn is by doing!

---

**Start your journey**: Open `docs/week1.md` and begin building your decentralized casino! 🎰