# API Documentation
## BitCraps Decentralized Casino Platform

*Version: 1.0 | Created: 2025-08-29*

---

## Executive Summary

This document provides comprehensive API documentation for the BitCraps decentralized casino platform, covering mobile SDK interfaces, P2P protocol specifications, WebSocket APIs, and REST endpoints. All APIs are designed for production use with comprehensive error handling, authentication, and rate limiting.

---

## Table of Contents

1. [Mobile SDK API](#1-mobile-sdk-api)
2. [P2P Protocol API](#2-p2p-protocol-api)
3. [WebSocket Real-time API](#3-websocket-real-time-api)
4. [REST API Endpoints](#4-rest-api-endpoints)
5. [Authentication Methods](#5-authentication-methods)
6. [Rate Limiting](#6-rate-limiting)
7. [Error Handling](#7-error-handling)
8. [SDK Integration Examples](#8-sdk-integration-examples)

---

## 1. Mobile SDK API

### 1.1 Core SDK Interface (UniFFI)

The mobile SDK is generated using Mozilla UniFFI and provides identical APIs for iOS (Swift) and Android (Kotlin).

#### 1.1.1 BitCrapsNode - Main SDK Entry Point

```rust
// Rust Interface Definition
namespace bitcraps {
    [Throws=BitCrapsError]
    BitCrapsNode create_node(BitCrapsConfig config);
    
    [Throws=BitCrapsError]
    sequence<string> get_available_bluetooth_adapters();
};

interface BitCrapsNode {
    // Network Discovery
    [Throws=BitCrapsError, Async]
    void start_discovery();
    
    [Throws=BitCrapsError, Async]
    void stop_discovery();
    
    // Game Management
    [Throws=BitCrapsError, Async]
    GameHandle create_game(GameConfig config);
    
    [Throws=BitCrapsError, Async]
    GameHandle join_game(string game_id);
    
    [Throws=BitCrapsError, Async]
    void leave_game();
    
    // Event System
    [Async]
    GameEvent? poll_event();
    
    [Async]
    sequence<GameEvent> drain_events();
    
    // Status and Monitoring
    NodeStatus get_status();
    sequence<PeerInfo> get_connected_peers();
    NetworkStats get_network_stats();
    
    // Power Management
    [Throws=BitCrapsError]
    void set_power_mode(PowerMode mode);
    
    [Throws=BitCrapsError]
    void set_scan_interval(u32 milliseconds);
    
    [Throws=BitCrapsError]
    void configure_for_platform(PlatformConfig config);
};
```

#### Swift Usage Example
```swift
import BitCraps

class GameManager: ObservableObject {
    private var node: BitCrapsNode?
    
    func initializeNode() {
        do {
            let config = BitCrapsConfig(
                bluetoothName: "Player1",
                enableBatteryOptimization: true,
                maxPeers: 8,
                discoveryTimeoutSeconds: 30
            )
            
            node = try createNode(config: config)
            try await node?.startDiscovery()
        } catch let error as BitCrapsError {
            handleError(error)
        }
    }
    
    func pollEvents() async {
        guard let node = node else { return }
        
        while let event = await node.pollEvent() {
            await handleGameEvent(event)
        }
    }
}
```

#### Kotlin Usage Example
```kotlin
import com.bitcraps.uniffi.bitcraps.*

class GameManager {
    private var node: BitCrapsNode? = null
    
    suspend fun initializeNode() {
        try {
            val config = BitCrapsConfig(
                bluetoothName = "Player1",
                enableBatteryOptimization = true,
                maxPeers = 8u,
                discoveryTimeoutSeconds = 30u
            )
            
            node = createNode(config)
            node?.startDiscovery()
        } catch (error: BitCrapsException) {
            handleError(error)
        }
    }
    
    suspend fun pollEvents() {
        val events = node?.drainEvents() ?: return
        events.forEach { event ->
            handleGameEvent(event)
        }
    }
}
```

### 1.2 Game Management API

#### 1.2.1 GameHandle Interface

```rust
interface GameHandle {
    // Game Information
    string get_game_id();
    GameState get_state();
    sequence<GameEvent> get_game_history();
    
    // Game Actions
    [Throws=BitCrapsError, Async]
    void place_bet(BetType bet_type, u64 amount);
    
    [Throws=BitCrapsError, Async]
    void roll_dice();
    
    // Game State
    [Async]
    DiceRoll? get_last_roll();
};
```

#### 1.2.2 Betting System API

```rust
[Enum]
interface BetType {
    Pass();
    DontPass();
    Field();
    Any7();
    AnyCraps();
    Hardway(u8 number);          // 4, 6, 8, or 10
    PlaceBet(u8 number);         // 4, 5, 6, 8, 9, or 10
    ComeBet();
    DontComeBet();
    PassLineBet();
    DontPassLineBet();
    // Additional 50+ bet types available
};
```

### 1.3 Event System

#### 1.3.1 Game Events

```rust
[Enum]
interface GameEvent {
    // Networking Events
    PeerDiscovered(PeerInfo peer);
    PeerConnected(string peer_id);
    PeerDisconnected(string peer_id);
    NetworkStateChanged(NetworkState new_state);
    
    // Game Events
    GameCreated(string game_id);
    GameJoined(string game_id, string peer_id);
    GameLeft(string game_id, string peer_id);
    GameStarted(string game_id);
    GameEnded(string game_id, string? winner_id, u64 payout);
    
    // Game Actions
    BetPlaced(string peer_id, BetType bet_type, u64 amount);
    DiceRolled(DiceRoll roll);
    
    // System Events
    ErrorOccurred(BitCrapsError error);
    BatteryOptimizationDetected(string reason);
};
```

### 1.4 Configuration and Platform Optimization

#### 1.4.1 Platform Configuration

```rust
dictionary PlatformConfig {
    PlatformType platform;          // Android, iOS, Desktop, Web
    boolean background_scanning;     // Enable background BLE scanning
    u32 scan_window_ms;             // BLE scan window (10-10240ms)
    u32 scan_interval_ms;           // BLE scan interval (2.5-40960ms)
    boolean low_power_mode;         // Enable power optimization
    sequence<string> service_uuids; // Custom service UUIDs
};

[Enum]
interface PowerMode {
    HighPerformance();    // Maximum performance, high power usage
    Balanced();           // Balance performance and battery
    BatterySaver();       // Conserve battery, reduced performance
    UltraLowPower();      // Minimal power usage, basic functionality
};
```

#### 1.4.2 Battery Optimization Configuration

```swift
// iOS Battery Optimization
let platformConfig = PlatformConfig(
    platform: .iOS,
    backgroundScanning: true,
    scanWindowMs: 30,      // 30ms scan window
    scanIntervalMs: 1875,  // 1.875s scan interval (iOS optimized)
    lowPowerMode: true,
    serviceUuids: ["12345678-1234-1234-1234-123456789abc"]
)

try node.configureForPlatform(config: platformConfig)
```

```kotlin
// Android Battery Optimization  
val platformConfig = PlatformConfig(
    platform = PlatformType.ANDROID,
    backgroundScanning = true,
    scanWindowMs = 500u,     // 500ms scan window
    scanIntervalMs = 5000u,  // 5s scan interval
    lowPowerMode = true,
    serviceUuids = listOf("12345678-1234-1234-1234-123456789abc")
)

node.configureForPlatform(platformConfig)
```

---

## 2. P2P Protocol API

### 2.1 Message Protocol Specification

#### 2.1.1 Core Message Structure

```rust
// Binary Protocol Message Format
pub struct P2PMessage {
    pub version: u8,                    // Protocol version (current: 1)
    pub message_type: MessageType,      // Message type identifier
    pub message_id: [u8; 32],          // Unique message ID (Blake3 hash)
    pub sender: PeerId,                 // Sender peer ID (32 bytes)
    pub recipient: Option<PeerId>,      // Target peer (None = broadcast)
    pub game_id: Option<GameId>,        // Game session ID
    pub timestamp: u64,                 // Unix timestamp (milliseconds)
    pub ttl: u8,                       // Time-to-live for routing
    pub payload: Vec<u8>,              // Message payload (compressed)
    pub signature: [u8; 64],           // Ed25519 signature
}
```

#### 2.1.2 Message Types

```rust
#[repr(u8)]
pub enum MessageType {
    // Discovery and Connection
    PeerDiscovery = 0x01,
    PeerResponse = 0x02,
    ConnectionRequest = 0x03,
    ConnectionAccept = 0x04,
    ConnectionReject = 0x05,
    Disconnect = 0x06,
    
    // Game Management
    GameCreate = 0x10,
    GameJoin = 0x11,
    GameLeave = 0x12,
    GameStart = 0x13,
    GameEnd = 0x14,
    
    // Game Actions
    PlaceBet = 0x20,
    RollDice = 0x21,
    GameStateUpdate = 0x22,
    
    // Consensus Protocol
    ConsensusProposal = 0x30,
    ConsensusVote = 0x31,
    ConsensusCommit = 0x32,
    StateSync = 0x33,
    
    // Anti-Cheat
    CheatEvidence = 0x40,
    ReputationUpdate = 0x41,
    DisputeResolution = 0x42,
    
    // Network Management  
    Heartbeat = 0x50,
    NetworkTopology = 0x51,
    PartitionRecovery = 0x52,
    
    // Error and Status
    Error = 0xFE,
    Status = 0xFF,
}
```

### 2.2 Consensus Protocol API

#### 2.2.1 Byzantine Consensus Messages

```rust
// Consensus Proposal Message
pub struct ConsensusProposal {
    pub round: u64,                    // Consensus round number
    pub proposer: PeerId,              // Proposing peer
    pub game_action: GameAction,       // Proposed game action
    pub previous_state_hash: [u8; 32], // Previous game state hash
    pub proposed_state_hash: [u8; 32], // Proposed new state hash
    pub evidence: Vec<u8>,             // Supporting evidence
    pub nonce: u64,                    // Anti-replay nonce
}

// Consensus Vote Message
pub struct ConsensusVote {
    pub round: u64,                    // Voting round
    pub voter: PeerId,                 // Voting peer
    pub proposal_hash: [u8; 32],       // Hash of proposal being voted on
    pub vote: VoteType,                // Accept or Reject
    pub justification: Option<String>, // Reason for vote (optional)
    pub validator_signature: [u8; 64], // Vote signature
}

#[repr(u8)]
pub enum VoteType {
    Accept = 0x01,
    Reject = 0x02,
    Abstain = 0x03,
}
```

#### 2.2.2 State Synchronization API

```rust
// State Synchronization Request
pub struct StateSyncRequest {
    pub requesting_peer: PeerId,
    pub last_known_state_hash: [u8; 32],
    pub last_known_round: u64,
    pub checkpoint_hash: Option<[u8; 32]>,
}

// State Synchronization Response
pub struct StateSyncResponse {
    pub responding_peer: PeerId,
    pub state_updates: Vec<StateUpdate>,
    pub merkle_proof: MerkleProof,
    pub is_complete: bool,
    pub next_checkpoint: Option<[u8; 32]>,
}
```

### 2.3 Anti-Cheat Protocol

#### 2.3.1 Cheat Detection Messages

```rust
// Cheat Evidence Report
pub struct CheatEvidence {
    pub reporter: PeerId,              // Reporting peer
    pub accused: PeerId,               // Accused peer
    pub evidence_type: CheatType,      // Type of cheating detected
    pub game_round: u64,               // Round when cheat occurred
    pub evidence_data: Vec<u8>,        // Supporting evidence
    pub confidence_score: f64,         // Detection confidence (0.0-1.0)
    pub timestamp: u64,                // Evidence timestamp
}

#[repr(u8)]
pub enum CheatType {
    StatisticalAnomaly = 0x01,      // Unusual betting patterns
    TimestampManipulation = 0x02,    // Clock manipulation
    StateManipulation = 0x03,        // Game state tampering
    ConsensusManipulation = 0x04,    // Consensus protocol abuse
    IdentitySpoofing = 0x05,        // Fake identity
    NetworkManipulation = 0x06,      // Network-level attacks
    CryptographicViolation = 0x07,   // Cryptographic protocol violations
}
```

---

## 3. WebSocket Real-time API

### 3.1 WebSocket Connection

#### 3.1.1 Connection Endpoint

```
wss://api.bitcraps.com/v1/websocket
```

**Authentication**: JWT token in query parameter or Authorization header

```javascript
const ws = new WebSocket('wss://api.bitcraps.com/v1/websocket?token=<jwt_token>');
```

#### 3.1.2 Message Format

All WebSocket messages use JSON format:

```typescript
interface WebSocketMessage {
    type: string;           // Message type
    id?: string;           // Request ID for correlation
    timestamp: number;     // Unix timestamp
    data: any;            // Message payload
}
```

### 3.2 Real-time Game Events

#### 3.2.1 Game State Updates

```typescript
// Subscribe to game events
{
    "type": "subscribe",
    "id": "req-001", 
    "data": {
        "channel": "game",
        "game_id": "game-12345"
    }
}

// Game state update
{
    "type": "game_state_update",
    "timestamp": 1693250400000,
    "data": {
        "game_id": "game-12345",
        "state": "point",
        "point": 6,
        "players": [
            {
                "peer_id": "peer-001",
                "name": "Alice",
                "balance": 1000,
                "bets": [
                    {
                        "type": "pass_line",
                        "amount": 50,
                        "placed_at": 1693250350000
                    }
                ]
            }
        ],
        "last_roll": {
            "die1": 3,
            "die2": 3,
            "total": 6,
            "roller": "peer-001",
            "timestamp": 1693250400000
        }
    }
}
```

#### 3.2.2 Peer Connection Events

```typescript
// Peer connected
{
    "type": "peer_connected",
    "timestamp": 1693250400000,
    "data": {
        "peer_id": "peer-002",
        "display_name": "Bob",
        "signal_strength": -45,
        "connection_type": "bluetooth_le"
    }
}

// Peer disconnected
{
    "type": "peer_disconnected", 
    "timestamp": 1693250500000,
    "data": {
        "peer_id": "peer-002",
        "reason": "timeout",
        "last_seen": 1693250450000
    }
}
```

### 3.3 Network Monitoring

#### 3.3.1 Network Statistics

```typescript
// Subscribe to network stats
{
    "type": "subscribe",
    "id": "req-002",
    "data": {
        "channel": "network_stats",
        "interval_ms": 5000
    }
}

// Network statistics update
{
    "type": "network_stats",
    "timestamp": 1693250400000,
    "data": {
        "peers_discovered": 12,
        "active_connections": 5,
        "bytes_sent": 1048576,
        "bytes_received": 2097152,
        "packets_dropped": 3,
        "average_latency_ms": 45.6,
        "consensus_rounds_completed": 156,
        "consensus_success_rate": 0.987
    }
}
```

---

## 4. REST API Endpoints

### 4.1 Authentication Endpoints

#### 4.1.1 Device Registration

```http
POST /api/v1/auth/register
Content-Type: application/json

{
    "device_id": "device-12345",
    "platform": "ios",
    "app_version": "1.0.0",
    "public_key": "0x1234...abcd",
    "proof_of_work": {
        "nonce": 12345678,
        "difficulty": 20,
        "hash": "0x0000abcd..."
    }
}
```

**Response:**
```http
HTTP/1.1 201 Created
Content-Type: application/json

{
    "success": true,
    "data": {
        "peer_id": "peer-12345",
        "access_token": "eyJhbGciOiJFZDI1NTE5In0...",
        "refresh_token": "refresh_token_here",
        "expires_in": 3600,
        "device_certificate": "-----BEGIN CERTIFICATE-----..."
    }
}
```

#### 4.1.2 Token Refresh

```http
POST /api/v1/auth/refresh
Content-Type: application/json

{
    "refresh_token": "refresh_token_here"
}
```

### 4.2 Game Management Endpoints

#### 4.2.1 List Available Games

```http
GET /api/v1/games?status=active&limit=20&offset=0
Authorization: Bearer <access_token>
```

**Response:**
```http
HTTP/1.1 200 OK
Content-Type: application/json

{
    "success": true,
    "data": {
        "games": [
            {
                "game_id": "game-12345",
                "name": "High Stakes Craps",
                "state": "waiting",
                "min_bet": 10,
                "max_bet": 1000,
                "max_players": 8,
                "current_players": 3,
                "created_at": "2025-08-29T10:00:00Z",
                "creator": {
                    "peer_id": "peer-001",
                    "display_name": "Alice"
                }
            }
        ],
        "pagination": {
            "total": 45,
            "limit": 20,
            "offset": 0,
            "has_more": true
        }
    }
}
```

#### 4.2.2 Game Statistics

```http
GET /api/v1/games/{game_id}/stats
Authorization: Bearer <access_token>
```

**Response:**
```http
HTTP/1.1 200 OK
Content-Type: application/json

{
    "success": true,
    "data": {
        "game_id": "game-12345",
        "total_rounds": 247,
        "total_bets": 1543,
        "total_wagered": 155430,
        "house_edge": 0.014,
        "average_round_duration": 45.7,
        "player_statistics": [
            {
                "peer_id": "peer-001",
                "display_name": "Alice",
                "rounds_played": 89,
                "total_wagered": 4520,
                "net_winnings": -230,
                "win_rate": 0.47
            }
        ]
    }
}
```

### 4.3 Network Monitoring Endpoints

#### 4.3.1 Network Health

```http
GET /api/v1/network/health
Authorization: Bearer <access_token>
```

**Response:**
```http
HTTP/1.1 200 OK
Content-Type: application/json

{
    "success": true,
    "data": {
        "network_status": "healthy",
        "total_peers": 1247,
        "active_peers": 892,
        "consensus_health": {
            "success_rate": 0.991,
            "average_round_time": 1.85,
            "byzantine_tolerance": 0.33
        },
        "anti_cheat": {
            "total_reports": 23,
            "confirmed_violations": 3,
            "false_positive_rate": 0.001
        },
        "performance_metrics": {
            "average_latency_ms": 67.3,
            "message_throughput": 12450,
            "error_rate": 0.002
        }
    }
}
```

---

## 5. Authentication Methods

### 5.1 Proof-of-Work Identity

BitCraps uses proof-of-work for Sybil attack prevention:

```rust
pub struct ProofOfWork {
    pub public_key: [u8; 32],     // Ed25519 public key
    pub nonce: u64,               // Proof-of-work nonce
    pub difficulty: u8,           // Required difficulty level
    pub timestamp: u64,           // PoW generation time
}

impl ProofOfWork {
    pub fn verify(&self) -> bool {
        let hash = blake3::hash(&[
            &self.public_key[..],
            &self.nonce.to_le_bytes(),
            &self.timestamp.to_le_bytes(),
        ].concat());
        
        // Check leading zeros match difficulty
        let leading_zeros = hash.as_bytes()
            .iter()
            .take_while(|&&b| b == 0)
            .count() * 8;
            
        leading_zeros >= self.difficulty as usize
    }
}
```

### 5.2 JWT Token Structure

```json
{
    "header": {
        "alg": "Ed25519",
        "typ": "JWT"
    },
    "payload": {
        "peer_id": "peer-12345",
        "device_id": "device-12345", 
        "platform": "ios",
        "iat": 1693250400,
        "exp": 1693254000,
        "permissions": ["game:join", "game:create", "network:read"]
    }
}
```

---

## 6. Rate Limiting

### 6.1 Rate Limiting Rules

| Endpoint Category | Rate Limit | Window | Burst Limit |
|------------------|------------|---------|-------------|
| Authentication | 10 requests | 1 minute | 20 |
| Game Actions | 100 requests | 1 minute | 150 |
| Network Monitoring | 1000 requests | 1 minute | 1500 |
| WebSocket Messages | 500 messages | 1 minute | 750 |

### 6.2 Rate Limiting Headers

```http
HTTP/1.1 200 OK
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 95
X-RateLimit-Reset: 1693250460
X-RateLimit-Window: 60
```

### 6.3 Rate Limit Exceeded Response

```http
HTTP/1.1 429 Too Many Requests
Content-Type: application/json
Retry-After: 30

{
    "success": false,
    "error": {
        "code": "RATE_LIMIT_EXCEEDED",
        "message": "Rate limit exceeded. Try again in 30 seconds.",
        "details": {
            "limit": 100,
            "window_seconds": 60,
            "retry_after_seconds": 30
        }
    }
}
```

---

## 7. Error Handling

### 7.1 Standard Error Response Format

```json
{
    "success": false,
    "error": {
        "code": "ERROR_CODE",
        "message": "Human-readable error message",
        "details": {
            "field": "Additional context",
            "timestamp": "2025-08-29T10:00:00Z",
            "request_id": "req-12345"
        }
    }
}
```

### 7.2 Error Codes

#### Authentication Errors (1000-1099)
- `1000` - `INVALID_CREDENTIALS` - Invalid username/password
- `1001` - `TOKEN_EXPIRED` - Authentication token expired
- `1002` - `INVALID_TOKEN` - Malformed or invalid token
- `1003` - `PROOF_OF_WORK_INVALID` - Invalid proof-of-work
- `1004` - `DEVICE_NOT_REGISTERED` - Device not found

#### Game Errors (2000-2099)
- `2000` - `GAME_NOT_FOUND` - Game session doesn't exist
- `2001` - `GAME_FULL` - Game has reached max players
- `2002` - `INVALID_BET` - Bet amount or type invalid
- `2003` - `INSUFFICIENT_FUNDS` - Not enough balance
- `2004` - `GAME_ENDED` - Game session has ended

#### Network Errors (3000-3099)
- `3000` - `PEER_NOT_FOUND` - Peer ID not found
- `3001` - `CONNECTION_FAILED` - Unable to establish connection
- `3002` - `NETWORK_PARTITION` - Network split detected
- `3003` - `CONSENSUS_FAILED` - Consensus round failed

#### System Errors (4000-4099)
- `4000` - `INTERNAL_ERROR` - Internal server error
- `4001` - `SERVICE_UNAVAILABLE` - Service temporarily unavailable
- `4002` - `MAINTENANCE_MODE` - System under maintenance

### 7.3 Mobile SDK Error Handling

```swift
// Swift error handling
do {
    let gameHandle = try await node.joinGame(gameId: "game-12345")
    // Success
} catch let error as BitCrapsError {
    switch error {
    case .gameError(let reason):
        print("Game error: \(reason)")
    case .networkError(let reason):
        print("Network error: \(reason)")
    case .timeout:
        print("Operation timed out")
    default:
        print("Unknown error: \(error)")
    }
}
```

```kotlin
// Kotlin error handling
try {
    val gameHandle = node.joinGame("game-12345")
    // Success
} catch (error: BitCrapsException.GameError) {
    Log.e("BitCraps", "Game error: ${error.reason}")
} catch (error: BitCrapsException.NetworkError) {
    Log.e("BitCraps", "Network error: ${error.reason}")
} catch (error: BitCrapsException.Timeout) {
    Log.e("BitCraps", "Operation timed out")
}
```

---

## 8. SDK Integration Examples

### 8.1 Complete iOS Integration

```swift
import SwiftUI
import BitCraps

@main
struct BitCrapsApp: App {
    var body: some Scene {
        WindowGroup {
            ContentView()
        }
    }
}

class GameViewModel: ObservableObject {
    @Published var peers: [PeerInfo] = []
    @Published var gameState: GameState = .waiting
    @Published var networkStats: NetworkStats?
    @Published var isConnected: Bool = false
    
    private var node: BitCrapsNode?
    private var gameHandle: GameHandle?
    private var eventPollingTask: Task<Void, Never>?
    
    func initialize() {
        do {
            let config = BitCrapsConfig(
                bluetoothName: UIDevice.current.name,
                enableBatteryOptimization: true,
                maxPeers: 8,
                discoveryTimeoutSeconds: 30
            )
            
            node = try createNode(config: config)
            
            // Configure for iOS
            let platformConfig = PlatformConfig(
                platform: .iOS,
                backgroundScanning: true,
                scanWindowMs: 30,
                scanIntervalMs: 1875,
                lowPowerMode: true,
                serviceUuids: ["12345678-1234-1234-1234-123456789abc"]
            )
            
            try node?.configureForPlatform(config: platformConfig)
            try await node?.startDiscovery()
            
            startEventPolling()
            
        } catch {
            print("Failed to initialize: \(error)")
        }
    }
    
    private func startEventPolling() {
        eventPollingTask = Task {
            while !Task.isCancelled {
                await pollEvents()
                try? await Task.sleep(nanoseconds: 100_000_000) // 100ms
            }
        }
    }
    
    private func pollEvents() async {
        guard let node = node else { return }
        
        let events = await node.drainEvents()
        
        await MainActor.run {
            for event in events {
                handleGameEvent(event)
            }
        }
    }
    
    private func handleGameEvent(_ event: GameEvent) {
        switch event {
        case .peerDiscovered(let peer):
            peers.append(peer)
        case .peerConnected(let peerId):
            isConnected = true
        case .gameJoined(let gameId, let peerId):
            print("Joined game: \(gameId)")
        case .diceRolled(let roll):
            print("Dice rolled: \(roll.die1), \(roll.die2)")
        default:
            print("Unhandled event: \(event)")
        }
    }
    
    func createGame() async {
        guard let node = node else { return }
        
        do {
            let config = GameConfig(
                gameName: "My Game",
                minBet: 10,
                maxBet: 1000,
                maxPlayers: 8,
                timeoutSeconds: 300
            )
            
            gameHandle = try await node.createGame(config: config)
        } catch {
            print("Failed to create game: \(error)")
        }
    }
    
    func placeBet(_ betType: BetType, amount: UInt64) async {
        guard let gameHandle = gameHandle else { return }
        
        do {
            try await gameHandle.placeBet(betType: betType, amount: amount)
        } catch {
            print("Failed to place bet: \(error)")
        }
    }
}

struct ContentView: View {
    @StateObject private var gameViewModel = GameViewModel()
    
    var body: some View {
        NavigationView {
            VStack {
                // Network Status
                HStack {
                    Circle()
                        .fill(gameViewModel.isConnected ? .green : .red)
                        .frame(width: 10, height: 10)
                    Text(gameViewModel.isConnected ? "Connected" : "Offline")
                }
                
                // Peer List
                List(gameViewModel.peers, id: \.peerId) { peer in
                    HStack {
                        Text(peer.displayName ?? peer.peerId)
                        Spacer()
                        Text("\(peer.signalStrength) dBm")
                            .foregroundColor(.gray)
                    }
                }
                
                // Game Controls
                HStack {
                    Button("Create Game") {
                        Task {
                            await gameViewModel.createGame()
                        }
                    }
                    .buttonStyle(.borderedProminent)
                    
                    Button("Place Bet") {
                        Task {
                            await gameViewModel.placeBet(.pass, amount: 50)
                        }
                    }
                    .buttonStyle(.bordered)
                }
            }
            .navigationTitle("BitCraps")
            .onAppear {
                gameViewModel.initialize()
            }
        }
    }
}
```

### 8.2 Complete Android Integration

```kotlin
// MainActivity.kt
package com.bitcraps.example

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.runtime.*
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.launch
import com.bitcraps.uniffi.bitcraps.*

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        
        setContent {
            BitCrapsTheme {
                BitCrapsScreen()
            }
        }
    }
}

class GameViewModel : ViewModel() {
    var peers by mutableStateOf<List<PeerInfo>>(emptyList())
    var gameState by mutableStateOf<GameState>(GameState.Waiting())
    var isConnected by mutableStateOf(false)
    
    private var node: BitCrapsNode? = null
    private var gameHandle: GameHandle? = null
    
    fun initialize() {
        viewModelScope.launch {
            try {
                val config = BitCrapsConfig(
                    bluetoothName = "Android Player",
                    enableBatteryOptimization = true,
                    maxPeers = 8u,
                    discoveryTimeoutSeconds = 30u
                )
                
                node = createNode(config)
                
                // Configure for Android
                val platformConfig = PlatformConfig(
                    platform = PlatformType.ANDROID,
                    backgroundScanning = true,
                    scanWindowMs = 500u,
                    scanIntervalMs = 5000u,
                    lowPowerMode = true,
                    serviceUuids = listOf("12345678-1234-1234-1234-123456789abc")
                )
                
                node?.configureForPlatform(platformConfig)
                node?.startDiscovery()
                
                startEventPolling()
                
            } catch (error: BitCrapsException) {
                println("Failed to initialize: $error")
            }
        }
    }
    
    private fun startEventPolling() {
        viewModelScope.launch {
            while (true) {
                val events = node?.drainEvents() ?: continue
                
                events.forEach { event ->
                    handleGameEvent(event)
                }
                
                kotlinx.coroutines.delay(100) // 100ms
            }
        }
    }
    
    private fun handleGameEvent(event: GameEvent) {
        when (event) {
            is GameEvent.PeerDiscovered -> {
                peers = peers + event.peer
            }
            is GameEvent.PeerConnected -> {
                isConnected = true
            }
            is GameEvent.GameJoined -> {
                println("Joined game: ${event.gameId}")
            }
            is GameEvent.DiceRolled -> {
                println("Dice rolled: ${event.roll.die1}, ${event.roll.die2}")
            }
            else -> {
                println("Unhandled event: $event")
            }
        }
    }
    
    fun createGame() {
        viewModelScope.launch {
            try {
                val config = GameConfig(
                    gameName = "My Android Game",
                    minBet = 10u,
                    maxBet = 1000u,
                    maxPlayers = 8u,
                    timeoutSeconds = 300u
                )
                
                gameHandle = node?.createGame(config)
            } catch (error: BitCrapsException) {
                println("Failed to create game: $error")
            }
        }
    }
    
    fun placeBet(betType: BetType, amount: ULong) {
        viewModelScope.launch {
            try {
                gameHandle?.placeBet(betType, amount)
            } catch (error: BitCrapsException) {
                println("Failed to place bet: $error")
            }
        }
    }
}

@Composable
fun BitCrapsScreen() {
    val viewModel = remember { GameViewModel() }
    
    LaunchedEffect(Unit) {
        viewModel.initialize()
    }
    
    Column {
        // Network status indicator
        Row {
            Canvas(modifier = Modifier.size(12.dp)) {
                drawCircle(
                    color = if (viewModel.isConnected) Color.Green else Color.Red
                )
            }
            Text(
                text = if (viewModel.isConnected) "Connected" else "Offline",
                modifier = Modifier.padding(start = 8.dp)
            )
        }
        
        // Peer list
        LazyColumn {
            items(viewModel.peers) { peer ->
                Row(
                    modifier = Modifier.fillMaxWidth().padding(8.dp),
                    horizontalArrangement = Arrangement.SpaceBetween
                ) {
                    Text(peer.displayName ?: peer.peerId)
                    Text(
                        text = "${peer.signalStrength} dBm",
                        color = Color.Gray
                    )
                }
            }
        }
        
        // Game controls
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceEvenly
        ) {
            Button(
                onClick = { viewModel.createGame() }
            ) {
                Text("Create Game")
            }
            
            Button(
                onClick = { viewModel.placeBet(BetType.Pass(), 50u) }
            ) {
                Text("Place Bet")
            }
        }
    }
}
```

---

This comprehensive API documentation provides all the necessary information for integrating with the BitCraps platform across all supported interfaces and platforms. The documentation covers mobile SDKs, P2P protocols, real-time WebSocket APIs, and REST endpoints with complete examples and error handling procedures.

**Document Control:**
- Review Cycle: Monthly for API changes, Quarterly for comprehensive review
- Owner: API Development Team
- Approval: Engineering Leadership and Product Team
- Distribution: Development Team, Partners, Community Developers

---

*Classification: Technical Documentation - Public Distribution*