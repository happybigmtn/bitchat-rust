# BitCraps Development Memory

## Session: 2025-08-22

### Project Context
- **Repository**: BitCraps - Decentralized Craps Casino Protocol
- **Language**: Rust
- **Current Branch**: master
- **Status**: Clean working directory

### Project Overview
A peer-to-peer Bluetooth mesh casino protocol with CRAP tokens, implementing:
- Bluetooth mesh networking for offline P2P gaming
- Noise Protocol encryption with forward secrecy
- Proof-of-Relay mining system
- Decentralized craps gaming with cryptographic fairness
- CRAP token economy

### Architecture
- `protocol/`: Wire protocol & message types
- `transport/`: Bluetooth & networking layer
- `mesh/`: Mesh routing & game sessions
- `session/`: Noise protocol & encryption
- `gaming/`: Craps game logic (consolidated into protocol)
- `token/`: CRAP token ledger
- `ui/`: CLI & TUI interfaces

### Recent Commits
- e4b3619: Complete BitCraps implementation with Week 7-8 and critical security fixes
- a681917: Add Week 6 testing infrastructure
- 1ebbbe1: Consolidate gaming module into protocol module

### Session Goals
*To be defined by user*

### Session Notes
*Will be updated during development*

---

## Commands Reference
```bash
# Build
cargo build --release

# Test
cargo test

# Run
cargo run -- start
```