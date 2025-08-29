# BitCraps System Architecture

## High-Level Architecture Overview

```mermaid
graph TB
    subgraph "Mobile Clients"
        IOS[iOS App]
        AND[Android App]
    end
    
    subgraph "P2P Network Layer"
        BLE[BLE Mesh Network]
        TCP[TCP/Internet]
        TURN[TURN Relay Servers]
    end
    
    subgraph "Core Services"
        CONSENSUS[Byzantine Consensus Engine]
        GAME[Game Orchestrator]
        TOKEN[Token Economics]
        TREASURY[Treasury Management]
    end
    
    subgraph "Infrastructure"
        GATEWAY[Gateway Nodes]
        MONITOR[Monitoring Stack]
        DB[(Database)]
    end
    
    IOS --> BLE
    AND --> BLE
    BLE --> GATEWAY
    TCP --> GATEWAY
    GATEWAY --> CONSENSUS
    CONSENSUS --> GAME
    GAME --> TOKEN
    TOKEN --> TREASURY
    CONSENSUS --> DB
    MONITOR --> CONSENSUS
```

## Detailed Component Architecture

### 1. Mobile Layer Architecture

```mermaid
graph LR
    subgraph "iOS Application"
        SWIFT[Swift UI Layer]
        SDK_IOS[BitCraps SDK]
        UNIFFI_IOS[UniFFI Bridge]
        RUST_IOS[Rust Core Library]
    end
    
    subgraph "Android Application"
        KOTLIN[Kotlin UI Layer]
        SDK_AND[BitCraps SDK]
        JNI[JNI Bridge]
        RUST_AND[Rust Core Library]
    end
    
    SWIFT --> SDK_IOS
    SDK_IOS --> UNIFFI_IOS
    UNIFFI_IOS --> RUST_IOS
    
    KOTLIN --> SDK_AND
    SDK_AND --> JNI
    JNI --> RUST_AND
```

### 2. Network Transport Architecture

```mermaid
graph TD
    subgraph "Multi-Transport System"
        TC[Transport Coordinator]
        BT[BLE Transport]
        UT[UDP Transport]
        TT[TCP Transport]
        NAT[NAT Traversal]
    end
    
    subgraph "Protocol Stack"
        MSG[Message Router]
        FRAG[Fragmentation Layer]
        CRYPTO[Encryption Layer]
        AUTH[Authentication]
    end
    
    TC --> BT
    TC --> UT
    TC --> TT
    TC --> NAT
    
    BT --> MSG
    UT --> MSG
    TT --> MSG
    MSG --> FRAG
    FRAG --> CRYPTO
    CRYPTO --> AUTH
```

### 3. Consensus & Gaming Architecture

```mermaid
graph TB
    subgraph "Consensus Layer"
        BFT[Byzantine Fault Tolerant Consensus]
        VOTE[Voting Mechanism]
        FORK[Fork Resolution]
        FINAL[Finality Engine]
    end
    
    subgraph "Gaming Layer"
        ORCH[Game Orchestrator]
        PAYOUT[Payout Engine]
        DICE[Dice Roll Protocol]
        BET[Bet Validation]
    end
    
    subgraph "State Management"
        STATE[Game State]
        LEDGER[Token Ledger]
        HISTORY[Transaction History]
    end
    
    BFT --> VOTE
    VOTE --> FORK
    FORK --> FINAL
    FINAL --> ORCH
    ORCH --> PAYOUT
    ORCH --> DICE
    ORCH --> BET
    PAYOUT --> STATE
    STATE --> LEDGER
    LEDGER --> HISTORY
```

### 4. Security Architecture

```mermaid
graph LR
    subgraph "Security Layers"
        TLS[TLS 1.3]
        ECDH[ECDH Key Exchange]
        AES[AES-256-GCM]
        HMAC[HMAC-SHA256]
    end
    
    subgraph "Key Management"
        KS[Secure Keystore]
        ROT[Key Rotation]
        HSM[HSM Support]
        DERIVE[Key Derivation]
    end
    
    subgraph "Access Control"
        AUTH2[Authentication]
        RBAC[Role-Based Access]
        AUDIT[Audit Logging]
        MONITOR2[Security Monitoring]
    end
    
    TLS --> ECDH
    ECDH --> AES
    AES --> HMAC
    
    KS --> ROT
    ROT --> HSM
    HSM --> DERIVE
    
    AUTH2 --> RBAC
    RBAC --> AUDIT
    AUDIT --> MONITOR2
```

## Data Flow Architecture

### Game Flow Sequence

```mermaid
sequenceDiagram
    participant Player
    participant Mobile App
    participant BLE Mesh
    participant Gateway
    participant Consensus
    participant Game Engine
    participant Treasury
    
    Player->>Mobile App: Start Game
    Mobile App->>BLE Mesh: Broadcast Game Creation
    BLE Mesh->>Gateway: Relay to Network
    Gateway->>Consensus: Submit Transaction
    Consensus->>Consensus: Byzantine Agreement
    Consensus->>Game Engine: Execute Game Logic
    Game Engine->>Treasury: Process Payouts
    Treasury->>Game Engine: Confirm Transaction
    Game Engine->>Consensus: Update State
    Consensus->>Gateway: Broadcast Result
    Gateway->>BLE Mesh: Relay Result
    BLE Mesh->>Mobile App: Update UI
    Mobile App->>Player: Show Result
```

## Deployment Architecture

### Kubernetes Deployment

```yaml
Deployment Structure:
├── Namespaces
│   ├── bitcraps-production
│   ├── bitcraps-staging
│   └── bitcraps-monitoring
├── Core Services
│   ├── gateway-nodes (3-10 replicas)
│   ├── consensus-nodes (5-15 replicas)
│   └── turn-servers (2-4 replicas)
├── Data Layer
│   ├── postgresql-primary
│   ├── postgresql-replicas (2-3)
│   └── redis-cluster (3 nodes)
├── Monitoring
│   ├── prometheus
│   ├── grafana
│   └── alertmanager
└── Ingress
    ├── nginx-ingress
    └── cert-manager
```

### Multi-Region Architecture

```mermaid
graph TB
    subgraph "Region 1 (Primary)"
        K8S1[Kubernetes Cluster]
        DB1[(PostgreSQL Primary)]
        CACHE1[Redis Cluster]
    end
    
    subgraph "Region 2 (Secondary)"
        K8S2[Kubernetes Cluster]
        DB2[(PostgreSQL Replica)]
        CACHE2[Redis Cluster]
    end
    
    subgraph "Region 3 (DR)"
        K8S3[Kubernetes Cluster]
        DB3[(PostgreSQL Replica)]
        CACHE3[Redis Cluster]
    end
    
    subgraph "Global Services"
        CDN[CloudFlare CDN]
        DNS[Route53 DNS]
        S3[S3 Backups]
    end
    
    K8S1 -.->|Replication| K8S2
    K8S2 -.->|Replication| K8S3
    DB1 -->|Streaming Replication| DB2
    DB1 -->|Streaming Replication| DB3
    
    CDN --> K8S1
    CDN --> K8S2
    DNS --> CDN
    
    K8S1 --> S3
    K8S2 --> S3
    K8S3 --> S3
```

## Technology Stack

### Core Technologies

| Layer | Technology | Purpose |
|-------|------------|---------|
| **Language** | Rust | Core implementation |
| **Mobile** | Swift/Kotlin | Native mobile apps |
| **Networking** | Tokio | Async runtime |
| **Database** | PostgreSQL/SQLite | Data persistence |
| **Cache** | Redis | Session/game state |
| **Consensus** | Custom BFT | Byzantine fault tolerance |
| **Cryptography** | Ring/Snow | Encryption & signatures |
| **Monitoring** | Prometheus/Grafana | Metrics & visualization |
| **Container** | Docker | Containerization |
| **Orchestration** | Kubernetes | Container orchestration |
| **CI/CD** | GitHub Actions | Automation |

### Protocol Stack

```
Application Layer
├── Game Protocol (Craps rules, betting)
├── Token Economics (Staking, rewards)
└── Chat/Social Features

Consensus Layer
├── Byzantine Fault Tolerant Consensus
├── Fork Resolution
└── Finality Guarantees

Network Layer
├── P2P Mesh Networking
├── Multi-Transport (BLE/TCP/UDP)
├── NAT Traversal
└── Gateway Bridging

Security Layer
├── End-to-End Encryption
├── Digital Signatures
├── Key Management
└── Access Control

Physical Layer
├── Bluetooth Low Energy
├── Internet (TCP/IP)
└── TURN Relay Servers
```

## Performance Architecture

### Optimization Strategies

```mermaid
graph LR
    subgraph "CPU Optimization"
        SIMD[SIMD Instructions]
        PARALLEL[Parallel Processing]
        ASYNC[Async I/O]
    end
    
    subgraph "Memory Optimization"
        POOL[Object Pooling]
        CACHE[Multi-tier Cache]
        COMPRESS[Compression]
    end
    
    subgraph "Network Optimization"
        BATCH[Message Batching]
        FRAGMENT[Smart Fragmentation]
        PRIORITY[QoS Priority]
    end
    
    subgraph "Mobile Optimization"
        BATTERY[Battery Management]
        THERMAL[Thermal Control]
        BACKGROUND[Background Tasks]
    end
```

### Scalability Metrics

| Component | Capacity | Latency | Throughput |
|-----------|----------|---------|------------|
| **BLE Mesh** | 50 peers/node | <500ms | 100 msg/s |
| **Gateway** | 1000 connections | <100ms | 10K msg/s |
| **Consensus** | 100 nodes | <2s finality | 1000 tx/s |
| **Database** | 10M records | <10ms read | 5000 ops/s |
| **Game Engine** | 1000 games | <50ms | 100 games/s |

## Security Architecture

### Defense in Depth

```
Layer 1: Network Security
├── DDoS Protection (CloudFlare)
├── WAF (Web Application Firewall)
├── Rate Limiting
└── IP Whitelisting

Layer 2: Application Security
├── Input Validation
├── SQL Injection Prevention
├── XSS Protection
└── CSRF Tokens

Layer 3: Data Security
├── Encryption at Rest (AES-256)
├── Encryption in Transit (TLS 1.3)
├── Key Management (HSM ready)
└── Data Masking

Layer 4: Access Control
├── Multi-factor Authentication
├── Role-Based Access Control
├── API Key Management
└── Session Management

Layer 5: Monitoring & Response
├── Security Event Logging
├── Anomaly Detection
├── Incident Response
└── Forensics Capability
```

## Monitoring & Observability

### Metrics Collection

```mermaid
graph TD
    subgraph "Application Metrics"
        APP[App Metrics]
        CUSTOM[Custom Metrics]
        BUSINESS[Business KPIs]
    end
    
    subgraph "Infrastructure Metrics"
        CPU[CPU Usage]
        MEM[Memory Usage]
        NET[Network I/O]
        DISK[Disk I/O]
    end
    
    subgraph "Aggregation"
        PROM[Prometheus]
        GRAF[Grafana]
        ALERT[AlertManager]
    end
    
    subgraph "Notification"
        SLACK[Slack]
        PAGER[PagerDuty]
        EMAIL[Email]
    end
    
    APP --> PROM
    CUSTOM --> PROM
    BUSINESS --> PROM
    CPU --> PROM
    MEM --> PROM
    NET --> PROM
    DISK --> PROM
    
    PROM --> GRAF
    PROM --> ALERT
    
    ALERT --> SLACK
    ALERT --> PAGER
    ALERT --> EMAIL
```

## Disaster Recovery Architecture

### Backup Strategy

```
Backup Types:
├── Database Backups
│   ├── Full backup (daily)
│   ├── Incremental (hourly)
│   └── Transaction logs (continuous)
├── Configuration Backups
│   ├── Kubernetes manifests
│   ├── Helm values
│   └── Secrets (encrypted)
└── State Backups
    ├── Game state snapshots
    ├── Token ledger
    └── User data

Recovery Targets:
├── RTO (Recovery Time Objective): 1 hour
├── RPO (Recovery Point Objective): 15 minutes
└── Backup Retention: 30 days
```

## Future Architecture Considerations

### Planned Enhancements

1. **Multi-Chain Support**
   - Ethereum integration
   - Polygon/Arbitrum L2
   - Cross-chain bridges

2. **Advanced Features**
   - AI-powered anti-cheat
   - Machine learning for fraud detection
   - Predictive scaling

3. **Geographic Expansion**
   - Regional gateway nodes
   - Localized content delivery
   - Compliance automation

4. **Performance Improvements**
   - WebAssembly runtime
   - GPU acceleration
   - Edge computing nodes

---

*This architecture document represents the current state of the BitCraps system and will evolve as the platform grows and requirements change.*