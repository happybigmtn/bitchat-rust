# BitCraps SDK API Documentation

Complete API reference for BitCraps Android and iOS SDKs with code examples, error handling, and testing strategies.

## Table of Contents

- [Overview](#overview)
- [Android SDK API](#android-sdk-api)
- [iOS SDK API](#ios-sdk-api)
- [Common Models](#common-models)
- [Error Handling](#error-handling)
- [Testing Guide](#testing-guide)
- [Advanced Usage](#advanced-usage)
- [Performance Optimization](#performance-optimization)

## Overview

The BitCraps SDKs provide identical functionality across Android and iOS platforms with platform-appropriate APIs. Both SDKs are built around reactive programming patterns and provide comprehensive error handling and diagnostics.

### Key Features

- **Reactive APIs**: StateFlow (Android) / @Published (iOS) for real-time updates
- **Type Safety**: Full type safety with sealed classes and enums
- **Error Handling**: Comprehensive error types with recovery suggestions
- **Performance**: Built-in performance monitoring and optimization
- **Security**: Biometric authentication and encrypted communication
- **Testing**: Comprehensive testing utilities and mocking support

### Architecture

```
┌─────────────────────────────────────────┐
│                Your App                 │
├─────────────────────────────────────────┤
│             BitCraps SDK               │
│  ┌─────────────┬─────────────────────┐  │
│  │  Android    │      iOS            │  │
│  │  (Kotlin)   │    (Swift)          │  │
│  ├─────────────┼─────────────────────┤  │
│  │          Shared Core                │  │
│  │        (Rust + FFI)               │  │
│  └─────────────────────────────────────┘  │
└─────────────────────────────────────────┘
```

## Android SDK API

### Primary Classes

#### BitCrapsSDK

The main entry point for the Android SDK.

```kotlin
class BitCrapsSDK private constructor(private val manager: BitCrapsManager) {
    
    companion object {
        suspend fun initialize(
            context: Context,
            config: SDKConfig = SDKConfig.default()
        ): BitCrapsSDK
        
        fun getInstance(): BitCrapsSDK
        val isInitialized: Boolean
        
        const val VERSION = "1.0.0"
        const val MIN_ANDROID_SDK = 21
        const val TARGET_ANDROID_SDK = 34
    }
}
```

**Methods:**

##### initialize()
```kotlin
suspend fun initialize(
    context: Context,
    config: SDKConfig = SDKConfig.default()
): BitCrapsSDK
```

Initializes the BitCraps SDK with the provided configuration.

**Parameters:**
- `context`: Android application context
- `config`: SDK configuration options

**Returns:** Initialized SDK instance

**Throws:** `BitCrapsException` if initialization fails

**Example:**
```kotlin
try {
    val sdk = BitCrapsSDK.initialize(
        context = applicationContext,
        config = SDKConfig.production()
    )
    // SDK ready to use
} catch (e: InitializationException) {
    // Handle initialization error
    Log.e("BitCraps", "Failed to initialize SDK", e)
}
```

#### State Properties

##### nodeStatus
```kotlin
val nodeStatus: StateFlow<NodeStatus?>
```

Current node status including state, connections, and configuration.

**Example:**
```kotlin
sdk.nodeStatus.collect { status ->
    status?.let { nodeStatus ->
        println("Node state: ${nodeStatus.state}")
        println("Active connections: ${nodeStatus.activeConnections}")
        println("Discovery active: ${nodeStatus.discoveryActive}")
    }
}
```

##### events
```kotlin
val events: Flow<GameEvent>
```

Stream of all game events including peer discovery, game state changes, and errors.

**Example:**
```kotlin
sdk.events.collect { event ->
    when (event) {
        is GameEvent.PeerDiscovered -> {
            println("Discovered peer: ${event.peer.displayName}")
            handleNewPeer(event.peer)
        }
        is GameEvent.GameCreated -> {
            println("Game created: ${event.gameId}")
            navigateToGame(event.gameId)
        }
        is GameEvent.DiceRolled -> {
            println("Dice rolled: ${event.dice}")
            updateGameUI(event)
        }
        is GameEvent.ErrorOccurred -> {
            handleError(event.error)
        }
    }
}
```

##### discoveredPeers
```kotlin
val discoveredPeers: StateFlow<List<PeerInfo>>
```

List of currently discovered peers with connection status and metadata.

**Example:**
```kotlin
sdk.discoveredPeers.collect { peers ->
    val connectedPeers = peers.filter { it.isConnected }
    updatePeerList(connectedPeers)
}
```

##### gameState
```kotlin
val gameState: StateFlow<GameState?>
```

Current game state if participating in a game.

**Example:**
```kotlin
sdk.gameState.collect { state ->
    state?.let { gameState ->
        if (gameState.isMyTurn) {
            enableGameControls()
        } else {
            disableGameControls()
        }
        updateGameBoard(gameState)
    }
}
```

### Discovery Operations

#### startDiscovery()
```kotlin
suspend fun startDiscovery(config: DiscoveryConfig = DiscoveryConfig.default())
```

Starts Bluetooth Low Energy peer discovery.

**Parameters:**
- `config`: Discovery configuration options

**Throws:** `BluetoothException` if discovery cannot be started

**Example:**
```kotlin
try {
    val config = DiscoveryConfig(
        scanWindowMs = 300,
        scanIntervalMs = 2000,
        powerMode = PowerMode.BALANCED,
        maxPeers = 20
    )
    
    sdk.startDiscovery(config)
    
    // Listen for discovered peers
    sdk.discoveredPeers.collect { peers ->
        updatePeerList(peers)
    }
} catch (e: BluetoothException) {
    when (e.bluetoothErrorCode) {
        BluetoothException.ERROR_ADAPTER_DISABLED -> {
            promptEnableBluetooth()
        }
        BluetoothException.ERROR_PERMISSION_DENIED -> {
            requestBluetoothPermissions()
        }
        else -> {
            showError("Discovery failed: ${e.message}")
        }
    }
}
```

#### stopDiscovery()
```kotlin
suspend fun stopDiscovery()
```

Stops peer discovery and cleans up resources.

**Example:**
```kotlin
try {
    sdk.stopDiscovery()
    println("Discovery stopped")
} catch (e: BluetoothException) {
    Log.w("BitCraps", "Error stopping discovery", e)
}
```

### Game Operations

#### createGame()
```kotlin
suspend fun createGame(
    gameType: GameType = GameType.CRAPS,
    config: GameConfig = GameConfig.default()
): GameSession
```

Creates a new game session that other peers can join.

**Parameters:**
- `gameType`: Type of game to create
- `config`: Game configuration options

**Returns:** `GameSession` handle for managing the game

**Throws:** `GameException` if game creation fails

**Example:**
```kotlin
try {
    val config = GameConfig(
        gameName = "Friday Night Craps",
        minBet = 10,
        maxBet = 500,
        maxPlayers = 6,
        timeoutSeconds = 300,
        requireBiometric = true
    )
    
    val gameSession = sdk.createGame(GameType.CRAPS, config)
    
    // Game created, wait for players
    println("Game created: ${gameSession.gameId}")
    
    // Listen for game events
    gameSession.observeEvents().collect { event ->
        when (event) {
            is GameEvent.GameJoined -> {
                println("Player joined: ${event.playerId}")
            }
            is GameEvent.GameStateChanged -> {
                updateGameState(event.newState)
            }
        }
    }
    
} catch (e: GameException) {
    showError("Failed to create game: ${e.message}")
}
```

#### joinGame()
```kotlin
suspend fun joinGame(gameId: String): GameSession
```

Joins an existing game by ID.

**Parameters:**
- `gameId`: Unique identifier of the game to join

**Returns:** `GameSession` handle for the joined game

**Throws:** `GameException` if joining fails

**Example:**
```kotlin
try {
    val gameSession = sdk.joinGame("550e8400-e29b-41d4-a716-446655440000")
    
    // Successfully joined game
    println("Joined game: ${gameSession.gameId}")
    
    // Start observing game state
    gameSession.observeState().collect { state ->
        updateGameUI(state)
    }
    
} catch (e: GameException) {
    when (e.gameErrorType) {
        GameException.GameErrorType.GAME_NOT_FOUND -> {
            showError("Game not found. Please check the game ID.")
        }
        GameException.GameErrorType.GAME_FULL -> {
            showError("Game is full. Please try another game.")
        }
        GameException.GameErrorType.GAME_ENDED -> {
            showError("This game has already ended.")
        }
        else -> {
            showError("Failed to join game: ${e.message}")
        }
    }
}
```

#### leaveGame()
```kotlin
suspend fun leaveGame()
```

Leaves the current game session.

**Throws:** `GameException` if leave operation fails

**Example:**
```kotlin
try {
    sdk.leaveGame()
    println("Left game successfully")
    navigateToLobby()
} catch (e: GameException) {
    Log.w("BitCraps", "Error leaving game", e)
    // Force navigation anyway
    navigateToLobby()
}
```

#### getAvailableGames()
```kotlin
suspend fun getAvailableGames(): List<AvailableGame>
```

Gets list of available games from discovered peers.

**Returns:** List of joinable games

**Throws:** `NetworkException` if retrieval fails

**Example:**
```kotlin
try {
    val availableGames = sdk.getAvailableGames()
    
    val joinableGames = availableGames.filter { game ->
        game.canJoin && !game.requiresPassword
    }
    
    displayAvailableGames(joinableGames)
    
} catch (e: NetworkException) {
    showError("Failed to get available games: ${e.message}")
    // Show cached games if available
    displayCachedGames()
}
```

### Peer Operations

#### connectToPeer()
```kotlin
suspend fun connectToPeer(peerId: String)
```

Establishes a direct connection to a specific peer.

**Parameters:**
- `peerId`: Identifier of the peer to connect to

**Throws:** `NetworkException` if connection fails

**Example:**
```kotlin
try {
    sdk.connectToPeer("peer-12345")
    
    // Connection established
    println("Connected to peer")
    
    // Listen for connection status changes
    sdk.connectionStatus.collect { status ->
        if (status.isConnected) {
            enablePeerFeatures()
        } else {
            disablePeerFeatures()
        }
    }
    
} catch (e: NetworkException) {
    showError("Failed to connect to peer: ${e.message}")
}
```

#### sendMessage()
```kotlin
suspend fun sendMessage(peerId: String, message: String)
```

Sends a direct message to a connected peer.

**Parameters:**
- `peerId`: Target peer identifier
- `message`: Message content

**Throws:** `NetworkException` if sending fails

**Example:**
```kotlin
try {
    sdk.sendMessage("peer-12345", "Good game!")
    println("Message sent")
} catch (e: NetworkException) {
    showError("Failed to send message: ${e.message}")
    // Queue message for retry
    queueMessage(peerId, message)
}
```

### Configuration Methods

#### setPowerMode()
```kotlin
suspend fun setPowerMode(powerMode: PowerMode)
```

Updates power management settings for discovery and connections.

**Parameters:**
- `powerMode`: New power mode to apply

**Example:**
```kotlin
// Detect battery level and adjust power mode
val batteryManager = getSystemService(Context.BATTERY_SERVICE) as BatteryManager
val batteryLevel = batteryManager.getIntProperty(BatteryManager.BATTERY_PROPERTY_CAPACITY)

val powerMode = when {
    batteryLevel < 20 -> PowerMode.ULTRA_LOW_POWER
    batteryLevel < 50 -> PowerMode.BATTERY_SAVER
    else -> PowerMode.BALANCED
}

sdk.setPowerMode(powerMode)
```

#### configureBluetoothSettings()
```kotlin
suspend fun configureBluetoothSettings(bluetoothConfig: BluetoothConfig)
```

Updates Bluetooth configuration parameters.

**Parameters:**
- `bluetoothConfig`: New Bluetooth configuration

**Example:**
```kotlin
val config = BluetoothConfig(
    advertisingIntervalMs = 1000,
    connectionIntervalMs = 100,
    mtuSize = 247,
    txPowerLevel = TxPowerLevel.MEDIUM,
    enableEncryption = true,
    maxConnections = 5
)

sdk.configureBluetoothSettings(config)
```

### Diagnostics and Monitoring

#### runNetworkDiagnostics()
```kotlin
suspend fun runNetworkDiagnostics(): DiagnosticsReport
```

Runs comprehensive network diagnostics and returns a detailed report.

**Returns:** `DiagnosticsReport` with system and network information

**Throws:** `NetworkException` if diagnostics fail

**Example:**
```kotlin
try {
    val report = sdk.runNetworkDiagnostics()
    
    println("System Info:")
    println("  Device: ${report.systemInfo.deviceModel}")
    println("  Android: ${report.systemInfo.androidVersion}")
    println("  Battery: ${report.systemInfo.batteryLevel}%")
    
    println("Network Diagnostics:")
    println("  Connectivity: ${report.networkDiagnostics.connectivity}")
    println("  Packet Loss: ${report.networkDiagnostics.packetLoss}%")
    
    println("Bluetooth Diagnostics:")
    println("  Enabled: ${report.bluetoothDiagnostics.bluetoothEnabled}")
    println("  Connections: ${report.bluetoothDiagnostics.currentConnections}/${report.bluetoothDiagnostics.maxConnections}")
    
    // Show recommendations
    report.recommendations.forEach { recommendation ->
        println("Recommendation: $recommendation")
    }
    
} catch (e: NetworkException) {
    showError("Diagnostics failed: ${e.message}")
}
```

#### getPerformanceMetrics()
```kotlin
suspend fun getPerformanceMetrics(): PerformanceMetrics
```

Gets current performance metrics for monitoring and optimization.

**Returns:** `PerformanceMetrics` with current performance data

**Example:**
```kotlin
val metrics = sdk.getPerformanceMetrics()

// Monitor performance
println("Performance Metrics:")
println("  CPU Usage: ${metrics.cpuUsagePercent}%")
println("  Memory Usage: ${metrics.memoryUsageMB}MB")
println("  Battery Level: ${metrics.batteryLevel}%")
println("  Bluetooth Latency: ${metrics.bluetoothLatencyMs}ms")
println("  Game State Latency: ${metrics.gameStateUpdateLatencyMs}ms")

// Alert if performance is poor
if (metrics.cpuUsagePercent > 80.0) {
    showPerformanceWarning("High CPU usage detected")
}

if (metrics.bluetoothLatencyMs > 100.0) {
    showPerformanceWarning("High Bluetooth latency detected")
}
```

## iOS SDK API

### Primary Classes

#### BitCrapsSDK

The main entry point for the iOS SDK.

```swift
@MainActor
public final class BitCrapsSDK: ObservableObject {
    
    // Published properties for SwiftUI
    @Published public private(set) var nodeStatus: NodeStatus?
    @Published public private(set) var connectionStatus: ConnectionStatus = .disconnected
    @Published public private(set) var discoveredPeers: [PeerInfo] = []
    @Published public private(set) var gameState: GameState?
    @Published public private(set) var networkStats: NetworkStats = NetworkStats()
    @Published public private(set) var isInitialized: Boolean = false
    @Published public private(set) var isDiscovering: Boolean = false
    
    // Combine publishers
    public let events = PassthroughSubject<GameEvent, Never>()
    public let messages = PassthroughSubject<PeerMessage, Never>()
    public let diagnostics = PassthroughSubject<DiagnosticEvent, Never>()
    
    // Static methods
    public static func initialize(config: SDKConfig = .default) async throws -> BitCrapsSDK
    public static func getInstance() throws -> BitCrapsSDK
    public static var isInitialized: Bool
    
    // Version info
    public static let version = "1.0.0"
    public static let minimumIOSVersion = "14.0"
}
```

**Methods:**

##### initialize()
```swift
public static func initialize(
    config: SDKConfig = .default
) async throws -> BitCrapsSDK
```

Initializes the BitCraps SDK with the provided configuration.

**Parameters:**
- `config`: SDK configuration options

**Returns:** Initialized SDK instance

**Throws:** `BitCrapsError` if initialization fails

**Example:**
```swift
do {
    let sdk = try await BitCrapsSDK.initialize(
        config: SDKConfig.production
    )
    
    // SDK ready to use
    print("SDK initialized successfully")
    
} catch let error as BitCrapsError {
    switch error {
    case .initializationFailed(let reason, _):
        print("SDK initialization failed: \(reason)")
        // Handle initialization error
    default:
        print("Unexpected error: \(error.displayMessage)")
    }
}
```

### Discovery Operations

#### startDiscovery()
```swift
public func startDiscovery(
    config: DiscoveryConfig = .default
) async throws
```

Starts Bluetooth Low Energy peer discovery.

**Parameters:**
- `config`: Discovery configuration options

**Throws:** `BitCrapsError` if discovery cannot be started

**Example:**
```swift
do {
    let config = DiscoveryConfig(
        scanWindowMs: 300,
        scanIntervalMs: 2000,
        powerMode: .balanced,
        maxPeers: 20
    )
    
    try await sdk.startDiscovery(config: config)
    
    // Listen for discovered peers
    sdk.$discoveredPeers
        .sink { peers in
            self.updatePeerList(peers)
        }
        .store(in: &cancellables)
        
} catch BitCrapsError.bluetoothError(let reason, _, _) {
    switch reason {
    case "Bluetooth adapter disabled":
        promptEnableBluetooth()
    case "Bluetooth permission denied":
        requestBluetoothPermissions()
    default:
        showError("Discovery failed: \(reason)")
    }
} catch {
    showError("Unexpected error: \(error)")
}
```

#### stopDiscovery()
```swift
public func stopDiscovery() async throws
```

Stops peer discovery and cleans up resources.

**Example:**
```swift
do {
    try await sdk.stopDiscovery()
    print("Discovery stopped")
} catch {
    print("Warning: Error stopping discovery: \(error)")
}
```

### Game Operations

#### createGame()
```swift
public func createGame(
    gameType: GameType = .craps,
    config: GameConfig = .default
) async throws -> GameSession
```

Creates a new game session that other peers can join.

**Parameters:**
- `gameType`: Type of game to create
- `config`: Game configuration options

**Returns:** `GameSession` handle for managing the game

**Throws:** `BitCrapsError` if game creation fails

**Example:**
```swift
do {
    let config = GameConfig(
        gameName: "Friday Night Craps",
        minBet: 10,
        maxBet: 500,
        maxPlayers: 6,
        timeoutSeconds: 300,
        requireBiometric: true
    )
    
    let gameSession = try await sdk.createGame(
        gameType: .craps,
        config: config
    )
    
    print("Game created: \(gameSession.gameId)")
    
    // Observe game events
    gameSession.eventStream
        .sink { event in
            switch event {
            case .gameJoined(let gameId):
                print("Player joined game: \(gameId)")
            case .gameStateChanged(let gameId, let newState):
                self.updateGameState(newState)
            default:
                break
            }
        }
        .store(in: &cancellables)
        
} catch BitCrapsError.gameError(let reason, _, _) {
    showError("Failed to create game: \(reason)")
} catch {
    showError("Unexpected error: \(error)")
}
```

#### joinGame()
```swift
public func joinGame(gameId: String) async throws -> GameSession
```

Joins an existing game by ID.

**Parameters:**
- `gameId`: Unique identifier of the game to join

**Returns:** `GameSession` handle for the joined game

**Throws:** `BitCrapsError` if joining fails

**Example:**
```swift
do {
    let gameSession = try await sdk.joinGame(
        gameId: "550e8400-e29b-41d4-a716-446655440000"
    )
    
    print("Joined game: \(gameSession.gameId)")
    
    // Observe game state
    gameSession.stateUpdates
        .sink { state in
            self.updateGameUI(state)
        }
        .store(in: &cancellables)
        
} catch BitCrapsError.gameError(let reason, _, _) {
    if reason.contains("not found") {
        showError("Game not found. Please check the game ID.")
    } else if reason.contains("full") {
        showError("Game is full. Please try another game.")
    } else if reason.contains("ended") {
        showError("This game has already ended.")
    } else {
        showError("Failed to join game: \(reason)")
    }
} catch {
    showError("Unexpected error: \(error)")
}
```

### SwiftUI Integration

The iOS SDK is designed for seamless SwiftUI integration with `@Published` properties and Combine publishers.

#### ObservableObject Usage

```swift
@MainActor
class GameManager: ObservableObject {
    @Published var discoveredPeers: [PeerInfo] = []
    @Published var gameState: GameState?
    @Published var isDiscovering = false
    
    private var sdk: BitCrapsSDK?
    private var cancellables = Set<AnyCancellable>()
    
    func initializeSDK() async {
        do {
            sdk = try await BitCrapsSDK.initialize()
            
            // Bind SDK state to published properties
            sdk?.$discoveredPeers
                .assign(to: &$discoveredPeers)
            
            sdk?.$gameState
                .assign(to: &$gameState)
                
            sdk?.$isDiscovering
                .assign(to: &$isDiscovering)
                
        } catch {
            print("SDK initialization failed: \(error)")
        }
    }
}
```

#### SwiftUI View Integration

```swift
struct GameView: View {
    @StateObject private var gameManager = GameManager()
    
    var body: some View {
        VStack {
            // Peer discovery section
            Section("Discovered Peers") {
                ForEach(gameManager.discoveredPeers) { peer in
                    PeerRow(peer: peer) {
                        Task {
                            await gameManager.connectToPeer(peer.id)
                        }
                    }
                }
            }
            
            // Game state section
            if let gameState = gameManager.gameState {
                GameStateView(gameState: gameState)
            }
            
            // Controls
            HStack {
                Button(gameManager.isDiscovering ? "Stop Discovery" : "Start Discovery") {
                    Task {
                        if gameManager.isDiscovering {
                            await gameManager.stopDiscovery()
                        } else {
                            await gameManager.startDiscovery()
                        }
                    }
                }
                
                Button("Create Game") {
                    Task {
                        await gameManager.createGame()
                    }
                }
            }
        }
        .task {
            await gameManager.initializeSDK()
        }
    }
}
```

### Lifecycle Management

#### App State Handling

```swift
@main
struct BitCrapsApp: App {
    @StateObject private var gameManager = GameManager()
    
    var body: some Scene {
        WindowGroup {
            ContentView()
                .environmentObject(gameManager)
                .onReceive(NotificationCenter.default.publisher(
                    for: UIApplication.willEnterForegroundNotification
                )) { _ in
                    gameManager.handleAppWillEnterForeground()
                }
                .onReceive(NotificationCenter.default.publisher(
                    for: UIApplication.didEnterBackgroundNotification
                )) { _ in
                    gameManager.handleAppDidEnterBackground()
                }
        }
    }
}

extension GameManager {
    func handleAppWillEnterForeground() {
        sdk?.applicationWillEnterForeground()
        
        // Refresh state when returning from background
        Task {
            await refreshState()
        }
    }
    
    func handleAppDidEnterBackground() {
        sdk?.applicationDidEnterBackground()
        
        // Optimize for background operation
        sdk?.setPowerMode(.batterySaver)
    }
}
```

## Common Models

### Core Data Models

Both Android and iOS SDKs use identical data models with platform-appropriate implementations.

#### NodeStatus

Represents the current status of the BitCraps node.

```kotlin
// Android
data class NodeStatus(
    val state: NodeState,
    val nodeId: String,
    val bluetoothEnabled: Boolean,
    val discoveryActive: Boolean,
    val currentGameId: String?,
    val activeConnections: Int,
    val currentPowerMode: PowerMode,
    val lastUpdateTime: Long
)
```

```swift
// iOS
public struct NodeStatus: Codable, Equatable {
    public let nodeId: String
    public var state: NodeState
    public var bluetoothState: CBManagerState
    public var isDiscoveryActive: Boolean
    public var currentGameId: String?
    public var activeConnectionCount: Int
    public var currentPowerMode: PowerMode
    public var lastUpdateTime: Date
}
```

#### PeerInfo

Information about discovered or connected peers.

```kotlin
// Android
data class PeerInfo(
    val peerId: String,
    val displayName: String?,
    val deviceModel: String?,
    val signalStrength: Int, // RSSI value
    val isConnected: Boolean,
    val lastSeen: Long,
    val capabilities: List<String>,
    val trustLevel: TrustLevel
)
```

```swift
// iOS
public struct PeerInfo: Codable, Equatable, Identifiable {
    public let id: String
    public var displayName: String?
    public var deviceModel: String?
    public var signalStrength: Int // RSSI value
    public var isConnected: Boolean
    public var lastSeen: Date
    public var capabilities: [String]
    public var trustLevel: TrustLevel
}
```

#### GameState

Complete state of an active game.

```kotlin
// Android
data class GameState(
    val gameId: String,
    val gameType: GameType,
    val publicState: PublicGameState,
    val players: List<Player>,
    val currentPlayer: Player?,
    val pot: Long,
    val round: GameRound?,
    val history: List<GameAction>,
    val myPlayerId: String?
) {
    val isMyTurn: Boolean get() = currentPlayer?.playerId == myPlayerId
    val myPlayer: Player? get() = players.find { it.playerId == myPlayerId }
}
```

```swift
// iOS
public struct GameState: Codable, Equatable, Identifiable {
    public let id: String
    public let gameId: String
    public let gameType: GameType
    public let publicState: PublicGameState
    public let players: [Player]
    public let currentPlayer: Player?
    public let pot: Int64
    public let round: GameRound?
    public let history: [GameAction]
    public let myPlayerId: String?
    
    public var isMyTurn: Boolean { 
        return currentPlayer?.playerId == myPlayerId 
    }
    
    public var myPlayer: Player? { 
        return players.first { $0.playerId == myPlayerId }
    }
}
```

### Enums and Types

#### GameType

```kotlin
// Android
enum class GameType {
    CRAPS,
    POKER,
    BLACKJACK,
    DICE_ROLL,
    CUSTOM
}
```

```swift
// iOS
public enum GameType: String, Codable, CaseIterable {
    case craps
    case poker
    case blackjack
    case diceRoll
    case custom
}
```

#### PowerMode

```kotlin
// Android
enum class PowerMode {
    HIGH_PERFORMANCE,
    BALANCED,
    BATTERY_SAVER,
    ULTRA_LOW_POWER
}
```

```swift
// iOS
public enum PowerMode: String, Codable, CaseIterable {
    case highPerformance
    case balanced
    case batterySaver
    case ultraLowPower
}
```

#### GameEvent

Events that can be observed from the SDK.

```kotlin
// Android
sealed class GameEvent {
    data class PeerDiscovered(val peer: PeerInfo) : GameEvent()
    data class PeerConnected(val peerId: String) : GameEvent()
    data class GameCreated(val gameId: String, val gameType: GameType) : GameEvent()
    data class DiceRolled(val gameId: String, val playerId: String, val dice: List<Int>) : GameEvent()
    data class ErrorOccurred(val error: BitCrapsError) : GameEvent()
    // ... more events
}
```

```swift
// iOS
public enum GameEvent: Equatable, Identifiable {
    case peerDiscovered(peer: PeerInfo)
    case peerConnected(peerId: String)
    case gameCreated(gameId: String, gameType: GameType)
    case diceRolled(gameId: String, playerId: String, dice: [Int])
    case errorOccurred(error: BitCrapsError)
    // ... more events
}
```

## Error Handling

Both SDKs provide comprehensive error handling with specific error types and recovery suggestions.

### Android Error Handling

#### Exception Hierarchy

```kotlin
abstract class BitCrapsException(message: String, cause: Throwable? = null) : Exception(message, cause) {
    abstract val errorCode: String
    abstract val errorType: String
    open val recoverable: Boolean = true
    open val retryable: Boolean = false
}

// Specific exception types
class InitializationException(message: String, cause: Throwable? = null) : BitCrapsException(message, cause)
class BluetoothException(message: String, val bluetoothErrorCode: Int? = null, cause: Throwable? = null) : BitCrapsException(message, cause)
class NetworkException(message: String, val networkErrorType: NetworkErrorType, cause: Throwable? = null) : BitCrapsException(message, cause)
class GameException(message: String, val gameId: String? = null, cause: Throwable? = null) : BitCrapsException(message, cause)
class SecurityException(message: String, val securityContext: String? = null, cause: Throwable? = null) : BitCrapsException(message, cause)
```

#### Error Handling Strategies

```kotlin
suspend fun handleBitCrapsError(error: BitCrapsException) {
    when (error) {
        is BluetoothException -> {
            when (error.bluetoothErrorCode) {
                BluetoothException.ERROR_ADAPTER_DISABLED -> {
                    showBluetoothEnableDialog()
                }
                BluetoothException.ERROR_PERMISSION_DENIED -> {
                    requestBluetoothPermissions()
                }
                BluetoothException.ERROR_SCANNING_NOT_SUPPORTED -> {
                    showError("This device doesn't support BLE scanning")
                }
                else -> {
                    if (error.retryable) {
                        retryWithExponentialBackoff { 
                            // Retry the operation
                        }
                    } else {
                        showError("Bluetooth error: ${error.message}")
                    }
                }
            }
        }
        
        is NetworkException -> {
            when (error.networkErrorType) {
                NetworkException.NetworkErrorType.CONNECTION_TIMEOUT -> {
                    // Retry with longer timeout
                    retryWithTimeout { /* retry operation */ }
                }
                NetworkException.NetworkErrorType.PEER_UNREACHABLE -> {
                    showError("Peer is no longer available")
                    removePeerFromList()
                }
                else -> {
                    showError("Network error: ${error.message}")
                }
            }
        }
        
        is GameException -> {
            when (error.gameErrorType) {
                GameException.GameErrorType.INSUFFICIENT_BALANCE -> {
                    showError("Insufficient balance for this bet")
                    suggestLowerBet()
                }
                GameException.GameErrorType.INVALID_MOVE -> {
                    showError("Invalid move. Please try again.")
                    highlightValidMoves()
                }
                else -> {
                    showError("Game error: ${error.message}")
                }
            }
        }
        
        is SecurityException -> {
            if (error.securityErrorType == SecurityException.SecurityErrorType.BIOMETRIC_AUTHENTICATION_FAILED) {
                offerAlternativeAuthentication()
            } else {
                showError("Security error: ${error.message}")
            }
        }
        
        else -> {
            showError("Unexpected error: ${error.message}")
            if (error.recoverable) {
                offerRetry()
            }
        }
    }
}

// Retry with exponential backoff
suspend fun retryWithExponentialBackoff(
    maxAttempts: Int = 3,
    initialDelayMs: Long = 1000,
    maxDelayMs: Long = 10000,
    backoffMultiplier: Double = 2.0,
    operation: suspend () -> Unit
) {
    var attempt = 0
    var delayMs = initialDelayMs
    
    while (attempt < maxAttempts) {
        try {
            operation()
            return // Success
        } catch (e: BitCrapsException) {
            attempt++
            
            if (attempt >= maxAttempts || !e.retryable) {
                throw e // Give up
            }
            
            delay(delayMs)
            delayMs = minOf(delayMs * backoffMultiplier.toLong(), maxDelayMs)
        }
    }
}
```

### iOS Error Handling

#### Error Types

```swift
public enum BitCrapsError: Error, Equatable, Identifiable {
    case initializationFailed(reason: String, underlyingError: Error? = nil)
    case bluetoothError(reason: String, errorCode: Int? = nil, underlyingError: Error? = nil)
    case networkError(reason: String, networkErrorType: NetworkErrorType = .unknown, underlyingError: Error? = nil)
    case gameError(reason: String, gameId: String? = nil, underlyingError: Error? = nil)
    case securityError(reason: String, securityContext: String? = nil, underlyingError: Error? = nil)
    
    public var id: String { /* implementation */ }
    public var displayMessage: String { /* implementation */ }
    public var isRecoverable: Boolean { /* implementation */ }
    public var isRetryable: Boolean { /* implementation */ }
}
```

#### Error Handling Patterns

```swift
func handleBitCrapsError(_ error: BitCrapsError) async {
    switch error {
    case .bluetoothError(let reason, let errorCode, _):
        switch errorCode {
        case CBError.poweredOff.rawValue:
            await promptEnableBluetooth()
            
        case CBError.unauthorized.rawValue:
            await requestBluetoothPermissions()
            
        case CBError.unsupported.rawValue:
            showError("This device doesn't support Bluetooth LE")
            
        default:
            if error.isRetryable {
                await retryWithExponentialBackoff {
                    // Retry the operation
                }
            } else {
                showError("Bluetooth error: \(reason)")
            }
        }
        
    case .networkError(let reason, let networkErrorType, _):
        switch networkErrorType {
        case .connectionTimeout:
            // Retry with longer timeout
            await retryWithTimeout { /* retry operation */ }
            
        case .peerUnreachable:
            showError("Peer is no longer available")
            await removePeerFromList()
            
        default:
            showError("Network error: \(reason)")
        }
        
    case .gameError(let reason, let gameId, _):
        if reason.contains("insufficient balance") {
            showError("Insufficient balance for this bet")
            suggestLowerBet()
        } else if reason.contains("invalid move") {
            showError("Invalid move. Please try again.")
            highlightValidMoves()
        } else {
            showError("Game error: \(reason)")
        }
        
    case .securityError(let reason, _, _):
        if reason.contains("biometric") {
            await offerAlternativeAuthentication()
        } else {
            showError("Security error: \(reason)")
        }
        
    default:
        showError("Unexpected error: \(error.displayMessage)")
        if error.isRecoverable {
            offerRetry()
        }
    }
}

// Retry with exponential backoff
func retryWithExponentialBackoff<T>(
    maxAttempts: Int = 3,
    initialDelay: TimeInterval = 1.0,
    maxDelay: TimeInterval = 10.0,
    backoffMultiplier: Double = 2.0,
    operation: @escaping () async throws -> T
) async throws -> T {
    var attempt = 0
    var delay = initialDelay
    
    while attempt < maxAttempts {
        do {
            return try await operation()
        } catch let error as BitCrapsError {
            attempt += 1
            
            if attempt >= maxAttempts || !error.isRetryable {
                throw error // Give up
            }
            
            try await Task.sleep(nanoseconds: UInt64(delay * 1_000_000_000))
            delay = min(delay * backoffMultiplier, maxDelay)
        }
    }
    
    fatalError("Should not reach here")
}
```

## Testing Guide

Both SDKs provide comprehensive testing utilities and support for unit testing, integration testing, and UI testing.

### Android Testing

#### Unit Testing

```kotlin
class BitCrapsSDKTest {
    
    @Mock
    private lateinit var mockContext: Context
    
    @Mock
    private lateinit var mockBluetoothManager: BluetoothManager
    
    private lateinit var sdk: BitCrapsSDK
    
    @Before
    fun setup() {
        MockitoAnnotations.openMocks(this)
        
        // Setup test configuration
        val testConfig = SDKConfig(
            enableLogging = true,
            logLevel = LogLevel.DEBUG,
            maxEventHistorySize = 10
        )
        
        // Initialize SDK with mocked dependencies
        sdk = BitCrapsSDK.createForTesting(
            context = mockContext,
            config = testConfig,
            bluetoothManager = mockBluetoothManager
        )
    }
    
    @Test
    fun testDiscoveryStartsSuccessfully() = runTest {
        // Arrange
        whenever(mockBluetoothManager.isEnabled()).thenReturn(true)
        whenever(mockBluetoothManager.startScanning(any())).thenReturn(Unit)
        
        val discoveryConfig = DiscoveryConfig.default()
        
        // Act
        sdk.startDiscovery(discoveryConfig)
        
        // Assert
        assertTrue(sdk.isDiscovering)
        verify(mockBluetoothManager).startScanning(discoveryConfig)
    }
    
    @Test
    fun testPeerDiscoveryEvent() = runTest {
        // Arrange
        val testPeer = PeerInfo(
            peerId = "test-peer",
            displayName = "Test Device",
            signalStrength = -60,
            isConnected = false,
            lastSeen = System.currentTimeMillis()
        )
        
        val events = mutableListOf<GameEvent>()
        val job = launch {
            sdk.events.collect { event ->
                events.add(event)
            }
        }
        
        // Act
        sdk.simulatePeerDiscovered(testPeer)
        
        // Wait for event to be processed
        delay(100)
        
        // Assert
        assertEquals(1, events.size)
        assertTrue(events[0] is GameEvent.PeerDiscovered)
        assertEquals(testPeer, (events[0] as GameEvent.PeerDiscovered).peer)
        
        job.cancel()
    }
    
    @Test
    fun testGameCreationWithValidConfig() = runTest {
        // Arrange
        val gameConfig = GameConfig(
            gameType = GameType.CRAPS,
            minBet = 10,
            maxBet = 100,
            maxPlayers = 4
        )
        
        // Act
        val gameSession = sdk.createGame(GameType.CRAPS, gameConfig)
        
        // Assert
        assertNotNull(gameSession)
        assertNotNull(gameSession.gameId)
        assertEquals(GameType.CRAPS, gameSession.gameType)
    }
    
    @Test
    fun testErrorHandling() = runTest {
        // Arrange
        whenever(mockBluetoothManager.startScanning(any()))
            .thenThrow(BluetoothException("Bluetooth not available"))
        
        // Act & Assert
        assertThrows<BluetoothException> {
            sdk.startDiscovery()
        }
    }
}

// Test utilities
class BitCrapsTestUtils {
    
    companion object {
        fun createTestPeer(
            id: String = "test-peer",
            name: String = "Test Device",
            rssi: Int = -60,
            connected: Boolean = false
        ): PeerInfo {
            return PeerInfo(
                peerId = id,
                displayName = name,
                signalStrength = rssi,
                isConnected = connected,
                lastSeen = System.currentTimeMillis(),
                capabilities = listOf("gaming", "chat"),
                trustLevel = TrustLevel.UNKNOWN
            )
        }
        
        fun createTestGameState(
            gameId: String = "test-game",
            playerId: String = "test-player"
        ): GameState {
            return GameState(
                gameId = gameId,
                gameType = GameType.CRAPS,
                publicState = PublicGameState(
                    status = GameStatus.IN_PROGRESS,
                    phase = GamePhase.BETTING
                ),
                players = listOf(
                    Player(
                        playerId = playerId,
                        peerId = "peer-1",
                        displayName = "Test Player",
                        balance = 1000,
                        position = 0
                    )
                ),
                myPlayerId = playerId,
                pot = 0,
                round = null,
                history = emptyList()
            )
        }
    }
}
```

#### Integration Testing

```kotlin
@RunWith(AndroidJUnit4::class)
@LargeTest
class BitCrapsIntegrationTest {
    
    @get:Rule
    val activityRule = ActivityScenarioRule(MainActivity::class.java)
    
    private lateinit var sdk: BitCrapsSDK
    
    @Before
    fun setup() {
        // Grant necessary permissions
        grantPermissions()
        
        // Initialize SDK
        runBlocking {
            sdk = BitCrapsSDK.initialize(
                context = InstrumentationRegistry.getInstrumentation().targetContext,
                config = SDKConfig.development()
            )
        }
    }
    
    @Test
    fun testFullDiscoveryFlow() {
        runBlocking {
            // Start discovery
            sdk.startDiscovery()
            
            // Wait for discovery to be active
            sdk.nodeStatus.first { it?.discoveryActive == true }
            
            // Simulate peer discovery
            val testPeer = BitCrapsTestUtils.createTestPeer()
            sdk.simulatePeerDiscovered(testPeer)
            
            // Verify peer appears in list
            val peers = sdk.discoveredPeers.first { it.isNotEmpty() }
            assertTrue(peers.contains(testPeer))
            
            // Stop discovery
            sdk.stopDiscovery()
            
            // Verify discovery stopped
            sdk.nodeStatus.first { it?.discoveryActive == false }
        }
    }
    
    @Test
    fun testGameCreationAndJoining() {
        runBlocking {
            // Create game
            val gameSession = sdk.createGame(
                gameType = GameType.CRAPS,
                config = GameConfig.default()
            )
            
            // Verify game state
            assertNotNull(sdk.gameState.value)
            assertEquals(gameSession.gameId, sdk.gameState.value?.gameId)
            
            // Simulate another player joining
            sdk.simulatePlayerJoined(gameSession.gameId, "player-2")
            
            // Verify player count updated
            val updatedState = sdk.gameState.first { 
                it?.players?.size == 2 
            }
            assertEquals(2, updatedState.players.size)
            
            // Leave game
            sdk.leaveGame()
            
            // Verify game state cleared
            assertNull(sdk.gameState.value)
        }
    }
    
    private fun grantPermissions() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
            val permissions = arrayOf(
                Manifest.permission.ACCESS_FINE_LOCATION,
                Manifest.permission.BLUETOOTH_SCAN,
                Manifest.permission.BLUETOOTH_ADVERTISE,
                Manifest.permission.BLUETOOTH_CONNECT
            )
            
            permissions.forEach { permission ->
                val instrumentation = InstrumentationRegistry.getInstrumentation()
                instrumentation.uiAutomation.grantRuntimePermission(
                    instrumentation.targetContext.packageName,
                    permission
                )
            }
        }
    }
}
```

### iOS Testing

#### Unit Testing

```swift
import XCTest
import Combine
@testable import BitCrapsSDK

class BitCrapsSDKTests: XCTestCase {
    
    private var sdk: BitCrapsSDK!
    private var cancellables: Set<AnyCancellable>!
    private var mockBluetoothManager: MockBluetoothManager!
    
    override func setUp() async throws {
        cancellables = Set<AnyCancellable>()
        mockBluetoothManager = MockBluetoothManager()
        
        // Create SDK with test configuration
        let testConfig = SDKConfig(
            enableLogging: true,
            logLevel: .debug,
            maxEventHistorySize: 10
        )
        
        sdk = try await BitCrapsSDK.createForTesting(
            config: testConfig,
            bluetoothManager: mockBluetoothManager
        )
    }
    
    override func tearDown() {
        cancellables = nil
        Task {
            await sdk?.shutdown()
        }
        sdk = nil
        mockBluetoothManager = nil
    }
    
    func testSDKInitialization() async throws {
        // Given
        XCTAssertTrue(sdk.isInitialized)
        
        // When
        let nodeStatus = sdk.nodeStatus
        
        // Then
        XCTAssertNotNil(nodeStatus)
        XCTAssertEqual(nodeStatus?.state, .ready)
    }
    
    func testDiscoveryStartsSuccessfully() async throws {
        // Given
        mockBluetoothManager.isBluetoothEnabled = true
        let discoveryConfig = DiscoveryConfig.default
        
        // When
        try await sdk.startDiscovery(config: discoveryConfig)
        
        // Then
        XCTAssertTrue(sdk.isDiscovering)
        XCTAssertTrue(mockBluetoothManager.startScanningCalled)
    }
    
    func testPeerDiscoveryEvent() async throws {
        // Given
        let testPeer = BitCrapsTestUtils.createTestPeer(
            id: "test-peer",
            name: "Test Device",
            rssi: -60
        )
        
        let expectation = XCTestExpectation(description: "Peer discovered event")
        var receivedEvent: GameEvent?
        
        sdk.events
            .sink { event in
                receivedEvent = event
                expectation.fulfill()
            }
            .store(in: &cancellables)
        
        // When
        await sdk.simulatePeerDiscovered(testPeer)
        
        // Then
        await fulfillment(of: [expectation], timeout: 1.0)
        
        guard case .peerDiscovered(let peer) = receivedEvent else {
            XCTFail("Expected PeerDiscovered event")
            return
        }
        
        XCTAssertEqual(peer.id, testPeer.id)
        XCTAssertEqual(peer.displayName, testPeer.displayName)
    }
    
    func testGameCreationWithValidConfig() async throws {
        // Given
        let gameConfig = GameConfig(
            gameType: .craps,
            minBet: 10,
            maxBet: 100,
            maxPlayers: 4
        )
        
        // When
        let gameSession = try await sdk.createGame(
            gameType: .craps,
            config: gameConfig
        )
        
        // Then
        XCTAssertNotNil(gameSession)
        XCTAssertFalse(gameSession.gameId.isEmpty)
        XCTAssertEqual(gameSession.gameType, .craps)
        XCTAssertNotNil(sdk.gameState)
    }
    
    func testBluetoothErrorHandling() async {
        // Given
        mockBluetoothManager.shouldThrowError = true
        mockBluetoothManager.errorToThrow = BitCrapsError.bluetoothError(
            reason: "Bluetooth not available",
            errorCode: CBError.poweredOff.rawValue
        )
        
        // When & Then
        do {
            try await sdk.startDiscovery()
            XCTFail("Expected error to be thrown")
        } catch BitCrapsError.bluetoothError(let reason, let errorCode, _) {
            XCTAssertEqual(reason, "Bluetooth not available")
            XCTAssertEqual(errorCode, CBError.poweredOff.rawValue)
        } catch {
            XCTFail("Unexpected error: \(error)")
        }
    }
}

// Test utilities
class BitCrapsTestUtils {
    
    static func createTestPeer(
        id: String = "test-peer",
        name: String = "Test Device",
        rssi: Int = -60,
        connected: Boolean = false
    ) -> PeerInfo {
        return PeerInfo(
            id: id,
            displayName: name,
            signalStrength: rssi,
            isConnected: connected,
            lastSeen: Date(),
            capabilities: ["gaming", "chat"],
            trustLevel: .unknown
        )
    }
    
    static func createTestGameState(
        gameId: String = "test-game",
        playerId: String = "test-player"
    ) -> GameState {
        return GameState(
            gameId: gameId,
            gameType: .craps,
            publicState: PublicGameState(
                status: .inProgress,
                phase: .betting
            ),
            players: [
                Player(
                    playerId: playerId,
                    peerId: "peer-1",
                    displayName: "Test Player",
                    balance: 1000,
                    position: 0
                )
            ],
            myPlayerId: playerId,
            pot: 0,
            round: nil,
            history: []
        )
    }
}

// Mock Bluetooth Manager
class MockBluetoothManager {
    var isBluetoothEnabled = true
    var startScanningCalled = false
    var shouldThrowError = false
    var errorToThrow: BitCrapsError?
    
    func startScanning(config: DiscoveryConfig) async throws {
        startScanningCalled = true
        
        if shouldThrowError, let error = errorToThrow {
            throw error
        }
    }
    
    func stopScanning() async {
        // Mock implementation
    }
}
```

#### SwiftUI Testing

```swift
import XCTest
import SwiftUI
@testable import BitCrapsSDK

class BitCrapsSwiftUITests: XCTestCase {
    
    func testGameViewUpdatesWithPeerDiscovery() throws {
        // Given
        let gameManager = GameManager()
        let testPeers = [
            BitCrapsTestUtils.createTestPeer(id: "peer-1", name: "Device 1"),
            BitCrapsTestUtils.createTestPeer(id: "peer-2", name: "Device 2")
        ]
        
        // When
        gameManager.discoveredPeers = testPeers
        
        let gameView = GameView()
            .environmentObject(gameManager)
        
        // Then
        // Verify UI updates with peer list
        // (This would use ViewInspector or similar UI testing framework)
        XCTAssertEqual(gameManager.discoveredPeers.count, 2)
    }
    
    func testErrorHandlingInUI() throws {
        // Given
        let gameManager = GameManager()
        let testError = BitCrapsError.bluetoothError(
            reason: "Bluetooth disabled",
            errorCode: CBError.poweredOff.rawValue
        )
        
        // When
        gameManager.lastError = testError
        
        // Then
        XCTAssertNotNil(gameManager.lastError)
        XCTAssertEqual(gameManager.lastError?.displayMessage, "Bluetooth disabled")
    }
}
```

### Test Configuration

#### Android Test Configuration

```kotlin
// app/src/test/java/resources/test_config.json
{
  "sdk_config": {
    "enable_logging": true,
    "log_level": "DEBUG",
    "mock_bluetooth": true,
    "mock_network": true,
    "test_mode": true
  },
  "discovery_config": {
    "scan_window_ms": 100,
    "scan_interval_ms": 500,
    "discovery_timeout_ms": 5000,
    "max_peers": 5
  }
}
```

#### iOS Test Configuration

```swift
// Tests/BitCrapsSDKTests/Resources/test_config.json
{
  "sdk_config": {
    "enableLogging": true,
    "logLevel": "debug",
    "mockBluetooth": true,
    "mockNetwork": true,
    "testMode": true
  },
  "discovery_config": {
    "scanWindowMs": 100,
    "scanIntervalMs": 500,
    "discoveryTimeoutMs": 5000,
    "maxPeers": 5
  }
}
```

## Advanced Usage

### Custom Game Implementation

Both SDKs support custom game types beyond the built-in games.

#### Android Custom Game

```kotlin
class CustomPokerGame : GameSession {
    override val gameId: String = UUID.randomUUID().toString()
    override val gameType: GameType = GameType.CUSTOM
    override val currentState: GameState? get() = _gameState.value
    
    private val _gameState = MutableStateFlow<GameState?>(null)
    private val _events = MutableSharedFlow<GameEvent>()
    
    // Custom game logic
    suspend fun dealCards() {
        // Implement card dealing logic
        val newState = currentState?.copy(
            phase = GamePhase.DEALING,
            // Update with dealt cards
        )
        _gameState.value = newState
        _events.emit(GameEvent.CardsDealt(gameId, cards))
    }
    
    suspend fun placeBet(amount: Long, betType: String) {
        // Custom betting logic for poker
        validateBet(amount, betType)
        
        val newState = currentState?.copy(
            pot = currentState.pot + amount,
            // Update player bets
        )
        _gameState.value = newState
        _events.emit(GameEvent.BetPlaced(gameId, "current_player", amount, betType))
    }
    
    override fun observeState(): Flow<GameState> = _gameState.filterNotNull()
    override fun observeEvents(): Flow<GameEvent> = _events.asSharedFlow()
    
    // Implement other required methods
}

// Register custom game
sdk.registerCustomGame("poker") { config ->
    CustomPokerGame().apply {
        initialize(config)
    }
}
```

#### iOS Custom Game

```swift
class CustomPokerGame: GameSession, ObservableObject {
    let gameId: String = UUID().uuidString
    let gameType: GameType = .custom
    
    @Published private(set) var currentState: GameState?
    
    private let stateSubject = PassthroughSubject<GameState, Never>()
    private let eventSubject = PassthroughSubject<GameEvent, Never>()
    
    var stateUpdates: AnyPublisher<GameState, Never> {
        stateSubject.eraseToAnyPublisher()
    }
    
    var eventStream: AnyPublisher<GameEvent, Never> {
        eventSubject.eraseToAnyPublisher()
    }
    
    // Custom game logic
    func dealCards() async throws {
        // Implement card dealing logic
        let newState = currentState?.copy(
            phase: .dealing
            // Update with dealt cards
        )
        
        currentState = newState
        if let state = newState {
            stateSubject.send(state)
        }
        eventSubject.send(.cardsDealt(gameId: gameId, cards: cards))
    }
    
    func placeBet(amount: Int64, betType: String) async throws {
        // Custom betting logic for poker
        try validateBet(amount: amount, betType: betType)
        
        let newState = currentState?.copy(
            pot: (currentState?.pot ?? 0) + amount
            // Update player bets
        )
        
        currentState = newState
        if let state = newState {
            stateSubject.send(state)
        }
        eventSubject.send(.betPlaced(
            gameId: gameId,
            playerId: "current_player",
            amount: amount,
            betType: betType
        ))
    }
    
    // Implement other required methods
}

// Register custom game
sdk.registerCustomGame("poker") { config in
    let game = CustomPokerGame()
    try await game.initialize(config: config)
    return game
}
```

### Plugin System

Create plugins to extend SDK functionality.

#### Android Plugin Interface

```kotlin
interface BitCrapsPlugin {
    val name: String
    val version: String
    
    suspend fun initialize(sdk: BitCrapsSDK)
    suspend fun onPeerDiscovered(peer: PeerInfo)
    suspend fun onGameStateChanged(gameState: GameState)
    suspend fun shutdown()
}

class AnalyticsPlugin : BitCrapsPlugin {
    override val name = "Analytics"
    override val version = "1.0.0"
    
    private lateinit var analyticsService: AnalyticsService
    
    override suspend fun initialize(sdk: BitCrapsSDK) {
        analyticsService = AnalyticsService()
        
        // Track events
        sdk.events.collect { event ->
            when (event) {
                is GameEvent.PeerDiscovered -> {
                    analyticsService.trackEvent("peer_discovered", mapOf(
                        "peer_id" to event.peer.peerId,
                        "signal_strength" to event.peer.signalStrength
                    ))
                }
                is GameEvent.GameCreated -> {
                    analyticsService.trackEvent("game_created", mapOf(
                        "game_type" to event.gameType.name,
                        "game_id" to event.gameId
                    ))
                }
            }
        }
    }
    
    override suspend fun onPeerDiscovered(peer: PeerInfo) {
        analyticsService.trackPeerDiscovery(peer)
    }
    
    override suspend fun onGameStateChanged(gameState: GameState) {
        analyticsService.trackGameState(gameState)
    }
    
    override suspend fun shutdown() {
        analyticsService.flush()
    }
}

// Register plugin
sdk.registerPlugin(AnalyticsPlugin())
```

## Performance Optimization

### Memory Management

#### Android Memory Optimization

```kotlin
class MemoryOptimizedGameManager {
    private val peerCache = LRUCache<String, PeerInfo>(50) // Limit peer cache
    private val eventHistory = CircularBuffer<GameEvent>(100) // Limit event history
    private var gameStateHistory = CircularBuffer<GameState>(20) // Limit state history
    
    fun optimizeMemoryUsage() {
        // Clean up old peers
        val cutoffTime = System.currentTimeMillis() - TimeUnit.MINUTES.toMillis(5)
        peerCache.evictAll { peer ->
            peer.lastSeen < cutoffTime && !peer.isConnected
        }
        
        // Compress event history
        eventHistory.compress { events ->
            events.groupBy { it.type }
                .mapValues { (_, events) -> events.takeLast(10) }
                .values.flatten()
        }
    }
    
    // Monitor memory usage
    private fun monitorMemoryUsage() {
        val runtime = Runtime.getRuntime()
        val usedMemory = runtime.totalMemory() - runtime.freeMemory()
        val memoryUsageMB = usedMemory / (1024 * 1024)
        
        if (memoryUsageMB > 200) { // Alert if over 200MB
            optimizeMemoryUsage()
            System.gc() // Suggest garbage collection
        }
    }
}
```

#### iOS Memory Optimization

```swift
class MemoryOptimizedGameManager: ObservableObject {
    private let peerCache = NSCache<NSString, PeerInfo>()
    private var eventHistory = CircularBuffer<GameEvent>(capacity: 100)
    private var gameStateHistory = CircularBuffer<GameState>(capacity: 20)
    
    init() {
        peerCache.countLimit = 50
        peerCache.totalCostLimit = 10 * 1024 * 1024 // 10MB limit
        
        // Monitor memory warnings
        NotificationCenter.default.addObserver(
            forName: UIApplication.didReceiveMemoryWarningNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            self?.handleMemoryWarning()
        }
    }
    
    private func handleMemoryWarning() {
        // Clean up caches
        peerCache.removeAllObjects()
        
        // Limit event history
        eventHistory = CircularBuffer(eventHistory.suffix(50))
        
        // Limit state history
        gameStateHistory = CircularBuffer(gameStateHistory.suffix(10))
        
        // Force cleanup of discovered peers
        discoveredPeers = Array(discoveredPeers.suffix(20))
    }
    
    private func monitorMemoryUsage() {
        let memoryUsage = getMemoryUsage()
        
        if memoryUsage > 200 * 1024 * 1024 { // 200MB
            handleMemoryWarning()
        }
    }
    
    private func getMemoryUsage() -> Int64 {
        var info = mach_task_basic_info()
        var count = mach_msg_type_number_t(MemoryLayout<mach_task_basic_info>.size) / 4
        
        let kerr: kern_return_t = withUnsafeMutablePointer(to: &info) {
            $0.withMemoryRebound(to: integer_t.self, capacity: 1) {
                task_info(mach_task_self_, task_flavor_t(MACH_TASK_BASIC_INFO), $0, &count)
            }
        }
        
        return kerr == KERN_SUCCESS ? Int64(info.resident_size) : 0
    }
}

// Circular buffer implementation
struct CircularBuffer<T> {
    private var buffer: [T]
    private var head = 0
    private let capacity: Int
    
    init(capacity: Int) {
        self.capacity = capacity
        self.buffer = []
        self.buffer.reserveCapacity(capacity)
    }
    
    init<S: Sequence>(_ sequence: S) where S.Element == T {
        let array = Array(sequence)
        self.capacity = array.count
        self.buffer = array
        self.head = 0
    }
    
    mutating func append(_ element: T) {
        if buffer.count < capacity {
            buffer.append(element)
        } else {
            buffer[head] = element
            head = (head + 1) % capacity
        }
    }
    
    func suffix(_ maxLength: Int) -> [T] {
        let count = min(maxLength, buffer.count)
        if head == 0 || buffer.count < capacity {
            return Array(buffer.suffix(count))
        } else {
            let tailCount = min(count, capacity - head)
            let headCount = count - tailCount
            return Array(buffer[head..<(head + tailCount)] + buffer[0..<headCount])
        }
    }
}
```

### Battery Optimization

#### Adaptive Scanning

```kotlin
// Android adaptive scanning
class AdaptiveScanningManager(private val context: Context) {
    private val batteryManager = context.getSystemService(Context.BATTERY_SERVICE) as BatteryManager
    
    fun getOptimalScanConfig(): DiscoveryConfig {
        val batteryLevel = batteryManager.getIntProperty(BatteryManager.BATTERY_PROPERTY_CAPACITY)
        val isCharging = batteryManager.getIntProperty(BatteryManager.BATTERY_PROPERTY_STATUS) == 
                        BatteryManager.BATTERY_STATUS_CHARGING
        
        return when {
            batteryLevel < 15 -> DiscoveryConfig(
                scanWindowMs = 50,
                scanIntervalMs = 10000,
                powerMode = PowerMode.ULTRA_LOW_POWER
            )
            batteryLevel < 30 -> DiscoveryConfig(
                scanWindowMs = 100,
                scanIntervalMs = 5000,
                powerMode = PowerMode.BATTERY_SAVER
            )
            isCharging -> DiscoveryConfig(
                scanWindowMs = 500,
                scanIntervalMs = 1000,
                powerMode = PowerMode.HIGH_PERFORMANCE
            )
            else -> DiscoveryConfig.default()
        }
    }
    
    fun startAdaptiveScanning() {
        Timer().schedule(object : TimerTask() {
            override fun run() {
                val config = getOptimalScanConfig()
                // Update scanning configuration
                sdk.updateDiscoveryConfig(config)
            }
        }, 0, 30000) // Update every 30 seconds
    }
}
```

```swift
// iOS adaptive scanning
class AdaptiveScanningManager: ObservableObject {
    private var timer: Timer?
    
    func getOptimalScanConfig() -> DiscoveryConfig {
        let batteryLevel = UIDevice.current.batteryLevel * 100
        let isCharging = UIDevice.current.batteryState == .charging
        
        switch (batteryLevel, isCharging) {
        case (..<15, false):
            return DiscoveryConfig(
                scanWindowMs: 50,
                scanIntervalMs: 10000,
                powerMode: .ultraLowPower
            )
        case (..<30, false):
            return DiscoveryConfig(
                scanWindowMs: 100,
                scanIntervalMs: 5000,
                powerMode: .batterySaver
            )
        case (_, true):
            return DiscoveryConfig(
                scanWindowMs: 500,
                scanIntervalMs: 1000,
                powerMode: .highPerformance
            )
        default:
            return .default
        }
    }
    
    func startAdaptiveScanning() {
        timer = Timer.scheduledTimer(withTimeInterval: 30.0, repeats: true) { [weak self] _ in
            guard let self = self else { return }
            
            let config = self.getOptimalScanConfig()
            Task {
                await self.sdk.updateDiscoveryConfig(config)
            }
        }
    }
    
    func stopAdaptiveScanning() {
        timer?.invalidate()
        timer = nil
    }
}
```

This comprehensive API documentation provides everything needed to successfully integrate and use the BitCraps SDKs in both Android and iOS applications. The examples demonstrate real-world usage patterns, proper error handling, and performance optimization techniques.

For the latest updates and additional examples, refer to the official SDK documentation at https://docs.bitcraps.com and the sample applications in the repository.