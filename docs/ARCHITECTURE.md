# BitCraps System Architecture

## Executive Summary

BitCraps is a decentralized, peer-to-peer gaming platform that enables trustless craps games over Bluetooth mesh networks. The system combines Byzantine fault-tolerant consensus, cryptographic security, and mobile optimization to create a production-ready distributed casino.

## System Overview

```
┌─────────────────────────────────────────────────────────────┐
│                     User Interface Layer                      │
├─────────────┬──────────────┬──────────────┬─────────────────┤
│     CLI     │     TUI      │   Mobile     │    Web API      │
└─────────────┴──────────────┴──────────────┴─────────────────┘
                              │
┌─────────────────────────────────────────────────────────────┐
│                     Gaming Framework                          │
├─────────────┬──────────────┬──────────────┬─────────────────┤
│    Craps    │   Blackjack  │    Poker     │   Extensions    │
└─────────────┴──────────────┴──────────────┴─────────────────┘
                              │
┌─────────────────────────────────────────────────────────────┐
│                    Consensus Engine                           │
├─────────────┬──────────────┬──────────────┬─────────────────┤
│   Byzantine │  Commit-     │   State      │   Anti-Cheat    │
│   Consensus │  Reveal      │   Sync       │   Detection     │
└─────────────┴──────────────┴──────────────┴─────────────────┘
                              │
┌─────────────────────────────────────────────────────────────┐
│                      Mesh Network                             │
├─────────────┬──────────────┬──────────────┬─────────────────┤
│   Routing   │   Discovery  │  Reputation  │   Resilience    │
└─────────────┴──────────────┴──────────────┴─────────────────┘
                              │
┌─────────────────────────────────────────────────────────────┐
│                   Transport Layer                             │
├─────────────┬──────────────┬──────────────┬─────────────────┤
│  Bluetooth  │     TCP      │     UDP      │    WebSocket    │
└─────────────┴──────────────┴──────────────┴─────────────────┘
                              │
┌─────────────────────────────────────────────────────────────┐
│                  Platform Adaptation                          │
├─────────────┬──────────────┬──────────────┬─────────────────┤
│   Android   │     iOS      │    Linux     │    Windows      │
└─────────────┴──────────────┴──────────────┴─────────────────┘
```

## Core Components

### 1. Transport Layer (`src/transport/`)

The transport layer provides a unified abstraction over multiple network protocols:

#### Key Components:
- **Transport Trait**: Defines common interface for all transports
- **Bluetooth Module**: BLE implementation for local mesh networking
- **Connection Pool**: Manages connection lifecycle and reuse
- **MTU Discovery**: Optimizes packet sizes for each transport

#### Architecture Pattern:
```rust
pub trait Transport: Send + Sync {
    async fn send(&self, peer: PeerId, data: Vec<u8>) -> Result<()>;
    async fn recv(&self) -> Result<(PeerId, Vec<u8>)>;
    async fn broadcast(&self, data: Vec<u8>) -> Result<()>;
}
```

### 2. Mesh Network Layer (`src/mesh/`)

Implements a self-organizing peer-to-peer network with Byzantine fault tolerance:

#### Key Components:
- **MeshService**: Core routing and message handling
- **Advanced Routing**: Implements TTL-based flooding with deduplication
- **Gateway Nodes**: Bridge between mesh and external networks
- **Resilience Module**: Handles network partitions and failures

#### Design Decisions:
- TTL-based message propagation prevents infinite loops
- LRU cache (10,000 messages) prevents duplicate processing
- Reputation scoring identifies and isolates malicious nodes

### 3. Consensus Engine (`src/protocol/consensus/`)

Provides Byzantine fault-tolerant agreement on game state:

#### Key Components:
- **Byzantine Engine**: Implements PBFT-style consensus
- **Commit-Reveal**: Ensures fair random number generation
- **State Validation**: Verifies all state transitions
- **Lock-Free Engine**: High-performance consensus using atomics

#### Consensus Flow:
1. **Proposal Phase**: Node proposes game operation
2. **Voting Phase**: Nodes vote on proposal validity
3. **Commit Phase**: Execute if 2/3+ agreement reached
4. **State Update**: Apply changes to local state

### 4. Gaming Framework (`src/gaming/`)

Extensible framework supporting multiple games:

#### Key Components:
- **Game Engine Trait**: Common interface for all games
- **Craps Implementation**: Complete 64-bet-type craps
- **Multi-Game Manager**: Handles concurrent game sessions
- **Treasury Management**: Manages house funds and payouts

#### Extensibility:
```rust
pub trait GameEngine {
    type State: GameState;
    type Action: GameAction;
    type Result: GameResult;
    
    async fn process_action(&self, state: &Self::State, action: Self::Action) 
        -> Result<(Self::State, Self::Result)>;
}
```

### 5. Security Layer (`src/crypto/`)

Comprehensive cryptographic protection:

#### Key Components:
- **Ed25519 Signatures**: Identity and message authentication
- **X25519 Key Exchange**: Secure session establishment
- **ChaCha20-Poly1305**: Authenticated encryption
- **Secure Keystore**: Protected key storage with Argon2

#### Security Properties:
- Forward secrecy through ephemeral keys
- Proof-of-work identity to prevent Sybil attacks
- Zero-knowledge proofs for private validation

### 6. Anti-Cheat System (`src/protocol/anti_cheat.rs`)

Multi-layered cheat detection and prevention:

#### Detection Methods:
- **Statistical Analysis**: Chi-square tests for randomness
- **Behavioral Profiling**: Track suspicious patterns
- **Time Validation**: Prevent timestamp manipulation
- **Consensus Verification**: Require 2/3 agreement on violations

### 7. Mobile Platform (`src/mobile/`)

Optimizations for resource-constrained devices:

#### Key Components:
- **CPU Optimizer**: Adaptive task scheduling
- **Memory Manager**: Strict budget enforcement (150MB target)
- **BLE Optimizer**: Duty cycling and power management
- **Battery Manager**: Target <5% drain per hour

## Data Flow

### Game Action Flow

```
User Input → UI Layer → Game Engine → Consensus Engine → 
    → Mesh Broadcast → Peer Validation → State Update → UI Update
```

### Message Propagation

```
Sender → Serialize → Compress → Encrypt → Transport Send →
    → Network → Transport Receive → Decrypt → Decompress → 
    → Deserialize → Message Handler
```

## Performance Characteristics

### Targets
- **Consensus Latency**: <500ms for 8 players
- **Message Propagation**: <200ms local mesh
- **Memory Usage**: <150MB baseline
- **Battery Drain**: <5% per hour active play
- **CPU Usage**: <20% average

### Optimization Strategies
- Lock-free data structures for hot paths
- Adaptive compression based on content type
- Connection pooling to reduce handshake overhead
- Batch processing for consensus operations
- Lazy evaluation for expensive computations

## Scalability

### Horizontal Scaling
- Each mesh network operates independently
- Gateway nodes bridge isolated meshes
- No central point of failure

### Vertical Scaling
- Multi-threaded consensus processing
- Parallel message validation
- Concurrent game sessions

### Limits
- **Max Players per Game**: 8 (Byzantine fault tolerance limit)
- **Max Concurrent Games**: Limited by memory (est. 100)
- **Max Mesh Size**: ~100 nodes (message propagation overhead)

## Failure Handling

### Network Failures
- **Partition Tolerance**: Games continue in majority partition
- **Reconnection**: Automatic state synchronization on rejoin
- **Message Retries**: Exponential backoff with jitter

### Node Failures
- **Crash Recovery**: Persistent state allows restart
- **Byzantine Tolerance**: System tolerates up to 33% malicious nodes
- **Reputation System**: Gradual isolation of misbehaving nodes

## Security Model

### Threat Model
- **Byzantine Adversaries**: Up to 1/3 of nodes may be malicious
- **Network Adversaries**: May drop, delay, or reorder messages
- **Sybil Attacks**: Mitigated through proof-of-work identity

### Security Guarantees
- **Consensus Safety**: No conflicting decisions with honest majority
- **Liveness**: Progress with 2/3+ honest nodes
- **Fairness**: Commit-reveal prevents manipulation
- **Privacy**: Zero-knowledge proofs for sensitive operations

## Deployment Architecture

### Mobile Deployment
```
Mobile App
    ├── UniFFI Bindings
    ├── Platform UI (SwiftUI/Jetpack Compose)
    └── Rust Core Library
```

### Server Deployment
```
Gateway Node
    ├── REST API
    ├── WebSocket Server
    ├── Mesh Bridge
    └── Monitoring
```

## Monitoring and Observability

### Metrics Collection
- **Prometheus**: Time-series metrics
- **Grafana**: Visualization dashboards
- **Custom Alerts**: Threshold-based alerting

### Key Metrics
- Consensus round latency
- Message propagation time
- Active player count
- Treasury balance
- Cheat detection rate

## Configuration Management

### Layered Configuration
1. Default values (compiled in)
2. Configuration files (`/etc/bitcraps/config.toml`)
3. Environment variables (`BITCRAPS_*`)
4. Command-line arguments
5. Runtime updates (hot reload)

### Key Parameters
```toml
[network]
max_connections = 100
connection_timeout = 30

[consensus]
round_timeout = 5
min_validators = 3

[anti_cheat]
suspicion_threshold = 3
evidence_retention = 3600
```

## Testing Strategy

### Unit Tests
- Component-level validation
- Mock dependencies
- Property-based testing

### Integration Tests
- Multi-component interaction
- Real network communication
- Consensus scenarios

### Chaos Testing
- Random failure injection
- Network partition simulation
- Byzantine behavior testing

## Future Enhancements

### Planned Features
1. **Quantum-Resistant Cryptography**: Post-quantum key exchange
2. **Layer 2 Scaling**: Off-chain state channels
3. **Cross-Chain Integration**: Bridge to blockchain networks
4. **AI Anti-Cheat**: Machine learning for pattern detection

### Research Areas
- Zero-knowledge proof optimization
- Homomorphic encryption for private games
- Formal verification of consensus algorithm
- Hardware security module integration

## Conclusion

BitCraps represents a production-ready implementation of a distributed gaming system with strong security guarantees, excellent performance characteristics, and comprehensive failure handling. The modular architecture enables easy extension while maintaining the core security and consensus properties.

The system successfully demonstrates that trustless, decentralized gaming is not only possible but practical, even on resource-constrained mobile devices over local mesh networks.