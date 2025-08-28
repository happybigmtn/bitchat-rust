# BitCraps Visual Architecture Diagrams

This document provides visual representations of the BitCraps distributed gaming system architecture using Mermaid diagrams. These diagrams complement the Feynman curriculum chapters by providing visual learning aids.

## 1. System Architecture Overview

```mermaid
graph TB
    subgraph "Client Layer"
        MW[Mobile Wallet]
        TUI[Terminal UI]
        SDK[SDK/API]
    end
    
    subgraph "Transport Layer"
        BLE[Bluetooth LE]
        TCP[TCP/IP]
        KAD[Kademlia DHT]
    end
    
    subgraph "Protocol Layer"
        CE[Consensus Engine]
        VM[Validation Module]
        TM[Treasury Manager]
    end
    
    subgraph "Game Layer"
        GR[Game Registry]
        CR[Craps Engine]
        PK[Poker Engine]
        BJ[Blackjack Engine]
    end
    
    subgraph "Storage Layer"
        DB[(SQLite)]
        MC[Merkle Cache]
        KS[Keystore]
    end
    
    subgraph "Monitoring Layer"
        MT[Metrics]
        AL[Alerts]
        LOG[Logging]
    end
    
    MW --> BLE
    TUI --> TCP
    SDK --> KAD
    
    BLE --> CE
    TCP --> CE
    KAD --> CE
    
    CE --> VM
    VM --> TM
    CE --> GR
    
    GR --> CR
    GR --> PK
    GR --> BJ
    
    CE --> DB
    CE --> MC
    TM --> KS
    
    CE --> MT
    VM --> AL
    GR --> LOG
```

## 2. Consensus State Machine

```mermaid
stateDiagram-v2
    [*] --> Initializing
    Initializing --> WaitingForPeers
    WaitingForPeers --> ProposingBlock: Have Quorum
    ProposingBlock --> ValidatingBlock
    ValidatingBlock --> VotingPhase
    VotingPhase --> Committed: 2/3+ Votes
    VotingPhase --> Rejected: >1/3 Reject
    Committed --> ProposingBlock: Next Round
    Rejected --> ProposingBlock: Retry
    
    ProposingBlock --> ViewChange: Timeout
    ValidatingBlock --> ViewChange: Invalid
    VotingPhase --> ViewChange: No Consensus
    ViewChange --> ProposingBlock: New Leader
    
    state ViewChange {
        [*] --> ElectingLeader
        ElectingLeader --> BroadcastingView
        BroadcastingView --> ConfirmingView
        ConfirmingView --> [*]
    }
```

## 3. Network Topology - Mesh Network Formation

```mermaid
graph LR
    subgraph "Physical Layer"
        P1[Peer 1<br/>Android]
        P2[Peer 2<br/>iOS]
        P3[Peer 3<br/>Desktop]
        P4[Peer 4<br/>Android]
        P5[Peer 5<br/>iOS]
    end
    
    P1 -.->|BLE| P2
    P1 -.->|BLE| P4
    P2 -.->|BLE| P3
    P2 -.->|BLE| P5
    P3 -->|TCP| P4
    P3 -->|TCP| P5
    P4 -.->|BLE| P5
    
    style P1 fill:#90EE90
    style P2 fill:#87CEEB
    style P3 fill:#FFB6C1
    style P4 fill:#90EE90
    style P5 fill:#87CEEB
```

## 4. Message Flow - Distributed Dice Roll

```mermaid
sequenceDiagram
    participant P1 as Player 1
    participant P2 as Player 2
    participant P3 as Player 3
    participant CE as Consensus Engine
    participant BC as Blockchain
    
    P1->>CE: ProposeRoll(seed1)
    P2->>CE: ProposeRoll(seed2)
    P3->>CE: ProposeRoll(seed3)
    
    CE->>CE: CombineSeeds(seed1, seed2, seed3)
    CE->>CE: GenerateDiceValue(combined)
    
    CE->>P1: BroadcastResult(dice=7)
    CE->>P2: BroadcastResult(dice=7)
    CE->>P3: BroadcastResult(dice=7)
    
    P1->>CE: SignResult(signature1)
    P2->>CE: SignResult(signature2)
    P3->>CE: SignResult(signature3)
    
    CE->>BC: CommitBlock(result, signatures)
    BC->>P1: Confirmed
    BC->>P2: Confirmed
    BC->>P3: Confirmed
```

## 5. Byzantine Fault Tolerance Scenario

```mermaid
graph TB
    subgraph "Honest Nodes (67%)"
        H1[Node 1<br/>Value: A]
        H2[Node 2<br/>Value: A]
        H3[Node 3<br/>Value: A]
        H4[Node 4<br/>Value: A]
    end
    
    subgraph "Byzantine Nodes (33%)"
        B1[Node 5<br/>Value: B to some<br/>Value: A to others]
        B2[Node 6<br/>Silent/Crashed]
    end
    
    subgraph "Consensus Result"
        R[Final Value: A<br/>≥2/3 Agreement]
    end
    
    H1 --> R
    H2 --> R
    H3 --> R
    H4 --> R
    B1 -.->|Equivocating| R
    B2 -.->|No Response| R
    
    style B1 fill:#FF6B6B
    style B2 fill:#FF6B6B
    style H1 fill:#90EE90
    style H2 fill:#90EE90
    style H3 fill:#90EE90
    style H4 fill:#90EE90
    style R fill:#FFD700
```

## 6. Database Transaction Lifecycle

```mermaid
stateDiagram-v2
    [*] --> Begin
    Begin --> ReadPhase
    ReadPhase --> ValidationPhase
    ValidationPhase --> WritePhase: Valid
    ValidationPhase --> Rollback: Invalid
    WritePhase --> PreCommit
    PreCommit --> Commit: Success
    PreCommit --> Rollback: Failure
    Commit --> [*]
    Rollback --> [*]
    
    state WritePhase {
        [*] --> AcquireLocks
        AcquireLocks --> WriteWAL
        WriteWAL --> UpdatePages
        UpdatePages --> [*]
    }
    
    state Rollback {
        [*] --> ReleaseLocks
        ReleaseLocks --> RestoreState
        RestoreState --> [*]
    }
```

## 7. Cryptographic Operations Flow

```mermaid
graph LR
    subgraph "Key Generation"
        ENT[Entropy Source] --> KDF[Key Derivation]
        KDF --> PK[Public Key]
        KDF --> SK[Private Key]
    end
    
    subgraph "Signing"
        MSG[Message] --> HASH[SHA-256]
        HASH --> SIGN[Ed25519 Sign]
        SK --> SIGN
        SIGN --> SIG[Signature]
    end
    
    subgraph "Verification"
        MSG2[Message] --> HASH2[SHA-256]
        HASH2 --> VER[Ed25519 Verify]
        PK --> VER
        SIG --> VER
        VER --> RES{Valid?}
    end
    
    subgraph "Encryption"
        PT[Plaintext] --> ENC[ChaCha20-Poly1305]
        KEY[Shared Secret] --> ENC
        ENC --> CT[Ciphertext]
    end
```

## 8. Mobile Platform Architecture Comparison

```mermaid
graph TB
    subgraph "iOS Architecture"
        IUI[SwiftUI] --> IVM[View Model]
        IVM --> IFF[UniFFI Bridge]
        IFF --> RUST1[Rust Core]
    end
    
    subgraph "Android Architecture"
        AUI[Jetpack Compose] --> AVM[ViewModel]
        AVM --> JNI[JNI Bridge]
        JNI --> RUST2[Rust Core]
    end
    
    subgraph "Shared Rust Core"
        RUST1 --> CORE[BitCraps Engine]
        RUST2 --> CORE
        CORE --> BLE[Bluetooth]
        CORE --> NET[Networking]
        CORE --> CONS[Consensus]
        CORE --> STOR[Storage]
    end
    
    style IUI fill:#87CEEB
    style AUI fill:#90EE90
    style CORE fill:#FFD700
```

## 9. Performance Optimization Pipeline

```mermaid
graph LR
    subgraph "Input"
        REQ[Request]
    end
    
    subgraph "Optimization Layers"
        L1[L1 Cache<br/>Hot Data]
        L2[L2 Cache<br/>Warm Data]
        L3[Connection Pool]
        L4[Batch Processor]
        L5[Async Runtime]
    end
    
    subgraph "Storage"
        DB[(Database)]
    end
    
    REQ --> L1
    L1 -->|Hit| RESP[Response]
    L1 -->|Miss| L2
    L2 -->|Hit| L1
    L2 -->|Miss| L3
    L3 --> L4
    L4 --> L5
    L5 --> DB
    DB --> L5
    L5 --> L4
    L4 --> L3
    L3 --> L2
    L2 --> L1
    L1 --> RESP
```

## 10. Deployment Architecture

```mermaid
graph TB
    subgraph "Development"
        DEV[Local Dev] --> TEST[Unit Tests]
        TEST --> INT[Integration Tests]
    end
    
    subgraph "CI/CD Pipeline"
        INT --> GH[GitHub Actions]
        GH --> BUILD[Build Artifacts]
        BUILD --> SEC[Security Scan]
        SEC --> PERF[Performance Tests]
    end
    
    subgraph "Deployment Targets"
        PERF --> STAGE[Staging]
        STAGE --> PROD[Production]
        
        subgraph "Production"
            AND[Play Store]
            IOS[App Store]
            DKR[Docker Hub]
            GIT[GitHub Releases]
        end
        
        PROD --> AND
        PROD --> IOS
        PROD --> DKR
        PROD --> GIT
    end
    
    subgraph "Monitoring"
        PROD --> PROM[Prometheus]
        PROM --> GRAF[Grafana]
        PROD --> SENT[Sentry]
        PROD --> LOKI[Loki Logs]
    end
```

## 11. Game Session Lifecycle

```mermaid
stateDiagram-v2
    [*] --> Creating: Player initiates
    Creating --> WaitingForPlayers: Session created
    WaitingForPlayers --> Starting: Min players joined
    Starting --> Active: Game initialized
    Active --> Paused: Player timeout
    Paused --> Active: Player returns
    Active --> Finalizing: Game ends
    Finalizing --> Complete: Results calculated
    Complete --> [*]
    
    WaitingForPlayers --> Cancelled: Timeout
    Paused --> Abandoned: Timeout
    Cancelled --> [*]
    Abandoned --> [*]
```

## 12. Testing Strategy Pyramid

```mermaid
graph TB
    subgraph "Test Pyramid"
        E2E[End-to-End Tests<br/>5%]
        INT[Integration Tests<br/>20%]
        UNIT[Unit Tests<br/>75%]
    end
    
    subgraph "Test Types"
        E2E --> CHAOS[Chaos Engineering]
        E2E --> LOAD[Load Testing]
        INT --> API[API Tests]
        INT --> DB[Database Tests]
        UNIT --> LOGIC[Business Logic]
        UNIT --> CRYPTO[Cryptography]
    end
    
    style E2E fill:#FF6B6B
    style INT fill:#FFD700
    style UNIT fill:#90EE90
```

## Usage in Curriculum

These diagrams should be embedded within the relevant Feynman curriculum chapters:

- **Diagram 1**: Chapter 3 (Library Architecture)
- **Diagram 2**: Chapter 19 (Consensus Engine)
- **Diagram 3**: Chapter 13 (Mesh Networking)
- **Diagram 4**: Chapter 14 (Consensus Algorithms)
- **Diagram 5**: Chapter 48 (Byzantine Fault Tolerance)
- **Diagram 6**: Chapter 11 (Database Systems)
- **Diagram 7**: Chapter 4-9 (Cryptography Modules)
- **Diagram 8**: Chapter 68 (Mobile Interface Design)
- **Diagram 9**: Chapter 38 (Performance Optimization)
- **Diagram 10**: Chapter 99 (Deployment Strategies)
- **Diagram 11**: Chapter 30 (Multi-Game Framework)
- **Diagram 12**: Chapters 46-55 (Testing Chapters)

## Rendering Instructions

To render these diagrams:

1. **Markdown Viewers**: Most modern markdown viewers support Mermaid natively
2. **GitHub**: Automatically renders Mermaid blocks in .md files
3. **VS Code**: Install the Mermaid extension for preview
4. **Export**: Use Mermaid CLI to export as SVG/PNG for presentations
5. **Interactive**: Use Mermaid Live Editor for customization

## Future Diagram Additions

Priority diagrams to add:

1. Merkle Tree Structure visualization
2. Network Partition Recovery scenarios
3. Token Economics flow chart
4. Security Threat Model diagram
5. Mobile Battery Optimization states
6. Cross-platform Message Format
7. Peer Discovery Mechanism
8. State Synchronization Protocol
9. Circuit Breaker Pattern
10. Event Sourcing Architecture

---

*These visual aids complement the text-based Feynman curriculum, providing visual learners with alternative ways to understand complex distributed systems concepts.*