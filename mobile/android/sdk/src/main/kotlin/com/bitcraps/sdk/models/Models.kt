package com.bitcraps.sdk.models

import kotlinx.serialization.Serializable
import java.util.UUID

// MARK: - Core Models

/**
 * Node status information
 */
@Serializable
data class NodeStatus(
    val state: NodeState,
    val nodeId: String,
    val bluetoothEnabled: Boolean,
    val discoveryActive: Boolean,
    val currentGameId: String? = null,
    val activeConnections: Int,
    val currentPowerMode: PowerMode,
    val lastUpdateTime: Long = System.currentTimeMillis()
)

/**
 * Node operational states
 */
@Serializable
enum class NodeState {
    INITIALIZING,
    READY,
    DISCOVERING,
    CONNECTED,
    IN_GAME,
    ERROR
}

/**
 * Connection status information
 */
@Serializable
data class ConnectionStatus(
    val isConnected: Boolean,
    val connectedPeers: Int,
    val connectionQuality: ConnectionQuality,
    val lastConnectionTime: Long? = null,
    val reconnectAttempts: Int = 0
)

/**
 * Connection quality levels
 */
@Serializable
enum class ConnectionQuality {
    EXCELLENT,
    GOOD,
    FAIR,
    POOR,
    DISCONNECTED
}

/**
 * Peer information
 */
@Serializable
data class PeerInfo(
    val peerId: String,
    val displayName: String?,
    val deviceModel: String?,
    val signalStrength: Int, // RSSI value
    val isConnected: Boolean,
    val lastSeen: Long,
    val capabilities: List<String> = emptyList(),
    val trustLevel: TrustLevel = TrustLevel.UNKNOWN
)

/**
 * Trust levels for peers
 */
@Serializable
enum class TrustLevel {
    TRUSTED,
    VERIFIED,
    UNKNOWN,
    SUSPICIOUS,
    BLOCKED
}

// MARK: - Game Models

/**
 * Available game information
 */
@Serializable
data class AvailableGame(
    val gameId: String,
    val gameType: GameType,
    val hostPeerId: String,
    val hostDisplayName: String?,
    val currentPlayers: Int,
    val maxPlayers: Int,
    val minBet: Long,
    val maxBet: Long,
    val gameState: PublicGameState,
    val requiresPassword: Boolean = false,
    val createdAt: Long = System.currentTimeMillis()
)

/**
 * Game types supported by the platform
 */
@Serializable
enum class GameType {
    CRAPS,
    POKER,
    BLACKJACK,
    DICE_ROLL,
    CUSTOM
}

/**
 * Public game state (visible to all peers)
 */
@Serializable
data class PublicGameState(
    val status: GameStatus,
    val currentRound: Int,
    val phase: GamePhase,
    val timeRemaining: Long? = null,
    val lastAction: String? = null
)

/**
 * Game status
 */
@Serializable
enum class GameStatus {
    WAITING_FOR_PLAYERS,
    IN_PROGRESS,
    PAUSED,
    COMPLETED,
    CANCELLED
}

/**
 * Game phase (specific to game type)
 */
@Serializable
enum class GamePhase {
    LOBBY,
    BETTING,
    ROLLING,
    RESOLVING,
    PAYOUT,
    FINISHED
}

/**
 * Complete game state
 */
@Serializable
data class GameState(
    val gameId: String,
    val gameType: GameType,
    val publicState: PublicGameState,
    val players: List<Player>,
    val currentPlayer: Player?,
    val pot: Long,
    val round: GameRound?,
    val history: List<GameAction> = emptyList(),
    val myPlayerId: String? = null
)

/**
 * Player information
 */
@Serializable
data class Player(
    val playerId: String,
    val peerId: String,
    val displayName: String?,
    val balance: Long,
    val currentBet: Long = 0,
    val isActive: Boolean = true,
    val position: Int,
    val statistics: PlayerStatistics = PlayerStatistics()
)

/**
 * Player statistics
 */
@Serializable
data class PlayerStatistics(
    val gamesPlayed: Int = 0,
    val gamesWon: Int = 0,
    val totalWinnings: Long = 0,
    val averageBet: Long = 0,
    val winRate: Double = 0.0
)

/**
 * Game round information
 */
@Serializable
data class GameRound(
    val roundId: String,
    val roundNumber: Int,
    val startTime: Long,
    val currentPlayer: String?,
    val phase: GamePhase,
    val dice: List<Int>? = null,
    val bets: Map<String, Long> = emptyMap(),
    val results: RoundResult? = null
)

/**
 * Round results
 */
@Serializable
data class RoundResult(
    val winners: List<String>,
    val payouts: Map<String, Long>,
    val houseEdge: Long,
    val completedAt: Long = System.currentTimeMillis()
)

/**
 * Game actions/events
 */
@Serializable
sealed class GameAction {
    abstract val actionId: String
    abstract val playerId: String
    abstract val timestamp: Long
    
    @Serializable
    data class JoinGame(
        override val actionId: String = UUID.randomUUID().toString(),
        override val playerId: String,
        override val timestamp: Long = System.currentTimeMillis()
    ) : GameAction()
    
    @Serializable
    data class PlaceBet(
        override val actionId: String = UUID.randomUUID().toString(),
        override val playerId: String,
        val amount: Long,
        val betType: String,
        override val timestamp: Long = System.currentTimeMillis()
    ) : GameAction()
    
    @Serializable
    data class RollDice(
        override val actionId: String = UUID.randomUUID().toString(),
        override val playerId: String,
        val dice: List<Int>,
        override val timestamp: Long = System.currentTimeMillis()
    ) : GameAction()
    
    @Serializable
    data class LeaveGame(
        override val actionId: String = UUID.randomUUID().toString(),
        override val playerId: String,
        override val timestamp: Long = System.currentTimeMillis()
    ) : GameAction()
}

// MARK: - Event Models

/**
 * Game events that can be observed
 */
@Serializable
sealed class GameEvent {
    abstract val eventId: String
    abstract val timestamp: Long
    
    @Serializable
    data class PeerDiscovered(
        val peer: PeerInfo,
        override val eventId: String = UUID.randomUUID().toString(),
        override val timestamp: Long = System.currentTimeMillis()
    ) : GameEvent()
    
    @Serializable
    data class PeerConnected(
        val peerId: String,
        override val eventId: String = UUID.randomUUID().toString(),
        override val timestamp: Long = System.currentTimeMillis()
    ) : GameEvent()
    
    @Serializable
    data class PeerDisconnected(
        val peerId: String,
        val reason: String?,
        override val eventId: String = UUID.randomUUID().toString(),
        override val timestamp: Long = System.currentTimeMillis()
    ) : GameEvent()
    
    @Serializable
    data class GameCreated(
        val gameId: String,
        val gameType: GameType,
        override val eventId: String = UUID.randomUUID().toString(),
        override val timestamp: Long = System.currentTimeMillis()
    ) : GameEvent()
    
    @Serializable
    data class GameJoined(
        val gameId: String,
        val playerId: String,
        override val eventId: String = UUID.randomUUID().toString(),
        override val timestamp: Long = System.currentTimeMillis()
    ) : GameEvent()
    
    @Serializable
    data class GameLeft(
        val gameId: String,
        val playerId: String,
        override val eventId: String = UUID.randomUUID().toString(),
        override val timestamp: Long = System.currentTimeMillis()
    ) : GameEvent()
    
    @Serializable
    data class GameStateChanged(
        val gameId: String,
        val newState: PublicGameState,
        override val eventId: String = UUID.randomUUID().toString(),
        override val timestamp: Long = System.currentTimeMillis()
    ) : GameEvent()
    
    @Serializable
    data class BetPlaced(
        val gameId: String,
        val playerId: String,
        val amount: Long,
        val betType: String,
        override val eventId: String = UUID.randomUUID().toString(),
        override val timestamp: Long = System.currentTimeMillis()
    ) : GameEvent()
    
    @Serializable
    data class DiceRolled(
        val gameId: String,
        val playerId: String,
        val dice: List<Int>,
        val total: Int,
        override val eventId: String = UUID.randomUUID().toString(),
        override val timestamp: Long = System.currentTimeMillis()
    ) : GameEvent()
    
    @Serializable
    data class RoundCompleted(
        val gameId: String,
        val roundResult: RoundResult,
        override val eventId: String = UUID.randomUUID().toString(),
        override val timestamp: Long = System.currentTimeMillis()
    ) : GameEvent()
    
    @Serializable
    data class MessageReceived(
        val fromPeerId: String,
        val message: String,
        override val eventId: String = UUID.randomUUID().toString(),
        override val timestamp: Long = System.currentTimeMillis()
    ) : GameEvent()
    
    @Serializable
    data class ErrorOccurred(
        val error: BitCrapsError,
        override val eventId: String = UUID.randomUUID().toString(),
        override val timestamp: Long = System.currentTimeMillis()
    ) : GameEvent()
    
    @Serializable
    data class BatteryOptimizationDetected(
        val reason: String,
        val recommendations: List<String>,
        override val eventId: String = UUID.randomUUID().toString(),
        override val timestamp: Long = System.currentTimeMillis()
    ) : GameEvent()
}

// MARK: - Configuration Models

/**
 * Power management modes
 */
@Serializable
enum class PowerMode {
    HIGH_PERFORMANCE,
    BALANCED,
    BATTERY_SAVER,
    ULTRA_LOW_POWER
}

/**
 * Discovery configuration
 */
@Serializable
data class DiscoveryConfig(
    val scanWindowMs: Int = 300,
    val scanIntervalMs: Int = 2000,
    val discoveryTimeoutMs: Long = 30000,
    val maxPeers: Int = 50,
    val serviceUuids: List<String> = defaultServiceUuids,
    val powerMode: PowerMode = PowerMode.BALANCED
) {
    companion object {
        val defaultServiceUuids = listOf("12345678-1234-5678-1234-567812345678")
        
        fun default() = DiscoveryConfig()
        
        fun lowPower() = DiscoveryConfig(
            scanWindowMs = 100,
            scanIntervalMs = 5000,
            powerMode = PowerMode.BATTERY_SAVER
        )
        
        fun highPerformance() = DiscoveryConfig(
            scanWindowMs = 500,
            scanIntervalMs = 1000,
            powerMode = PowerMode.HIGH_PERFORMANCE
        )
    }
}

/**
 * Game configuration
 */
@Serializable
data class GameConfig(
    val gameName: String? = null,
    val gameType: GameType = GameType.CRAPS,
    val minBet: Long = 1,
    val maxBet: Long = 1000,
    val maxPlayers: Int = 8,
    val minPlayers: Int = 2,
    val timeoutSeconds: Int = 300,
    val houseEdgePercent: Double = 2.5,
    val requireBiometric: Boolean = false,
    val enableChat: Boolean = true,
    val privateGame: Boolean = false,
    val password: String? = null
) {
    companion object {
        fun default() = GameConfig()
        
        fun quickGame() = GameConfig(
            timeoutSeconds = 60,
            maxPlayers = 4
        )
        
        fun tournamentGame() = GameConfig(
            minBet = 10,
            maxBet = 10000,
            maxPlayers = 16,
            timeoutSeconds = 600,
            requireBiometric = true
        )
    }
}

/**
 * Bluetooth configuration
 */
@Serializable
data class BluetoothConfig(
    val advertisingIntervalMs: Int = 1000,
    val connectionIntervalMs: Int = 100,
    val mtuSize: Int = 247,
    val txPowerLevel: TxPowerLevel = TxPowerLevel.MEDIUM,
    val enableEncryption: Boolean = true,
    val maxConnections: Int = 7
)

/**
 * Bluetooth transmission power levels
 */
@Serializable
enum class TxPowerLevel {
    ULTRA_LOW,
    LOW,
    MEDIUM,
    HIGH
}

/**
 * SDK configuration
 */
@Serializable
data class SDKConfig(
    val dataDirectory: String? = null,
    val enableLogging: Boolean = true,
    val logLevel: LogLevel = LogLevel.INFO,
    val defaultPowerMode: PowerMode = PowerMode.BALANCED,
    val enableBiometrics: Boolean = true,
    val enableBackgroundScanning: Boolean = false,
    val maxEventHistorySize: Int = 100,
    val networkTimeoutMs: Long = 30000,
    val discoveryConfig: DiscoveryConfig = DiscoveryConfig.default(),
    val bluetoothConfig: BluetoothConfig = BluetoothConfig()
) {
    companion object {
        fun default() = SDKConfig()
        
        fun production() = SDKConfig(
            logLevel = LogLevel.WARN,
            enableLogging = true,
            maxEventHistorySize = 50
        )
        
        fun development() = SDKConfig(
            logLevel = LogLevel.DEBUG,
            enableLogging = true,
            maxEventHistorySize = 1000
        )
    }
}

// MARK: - Statistics and Diagnostics

/**
 * Network statistics
 */
@Serializable
data class NetworkStats(
    val bytesTransferred: Long,
    val messagesReceived: Long,
    val messagesSent: Long,
    val averageLatencyMs: Double,
    val connectionUptime: Long,
    val droppedConnections: Int,
    val peersDiscovered: Int,
    val peersConnected: Int,
    val lastUpdated: Long = System.currentTimeMillis()
)

/**
 * Performance metrics
 */
@Serializable
data class PerformanceMetrics(
    val cpuUsagePercent: Double,
    val memoryUsageMB: Double,
    val batteryLevel: Int,
    val bluetoothLatencyMs: Double,
    val gameStateUpdateLatencyMs: Double,
    val consensusLatencyMs: Double,
    val networkThroughputKbps: Double,
    val frameRate: Double = 0.0,
    val lastMeasured: Long = System.currentTimeMillis()
)

/**
 * Diagnostics report
 */
@Serializable
data class DiagnosticsReport(
    val systemInfo: SystemInfo,
    val networkDiagnostics: NetworkDiagnostics,
    val bluetoothDiagnostics: BluetoothDiagnostics,
    val performanceMetrics: PerformanceMetrics,
    val recommendations: List<String>,
    val generatedAt: Long = System.currentTimeMillis()
)

/**
 * System information
 */
@Serializable
data class SystemInfo(
    val deviceModel: String,
    val androidVersion: String,
    val apiLevel: Int,
    val availableMemoryMB: Long,
    val batteryLevel: Int,
    val bluetoothVersion: String,
    val isCharging: Boolean
)

/**
 * Network diagnostics
 */
@Serializable
data class NetworkDiagnostics(
    val connectivity: ConnectivityStatus,
    val latencyTests: List<LatencyTest>,
    val throughputTests: List<ThroughputTest>,
    val packetLoss: Double
)

/**
 * Bluetooth diagnostics
 */
@Serializable
data class BluetoothDiagnostics(
    val bluetoothEnabled: Boolean,
    val advertisingSupported: Boolean,
    val centralRoleSupported: Boolean,
    val peripheralRoleSupported: Boolean,
    val maxConnections: Int,
    val currentConnections: Int,
    val scanResults: List<ScanResult>
)

/**
 * Connectivity status
 */
@Serializable
enum class ConnectivityStatus {
    EXCELLENT,
    GOOD,
    FAIR,
    POOR,
    OFFLINE
}

/**
 * Latency test result
 */
@Serializable
data class LatencyTest(
    val peerId: String,
    val averageLatencyMs: Double,
    val minLatencyMs: Double,
    val maxLatencyMs: Double,
    val packetsSent: Int,
    val packetsReceived: Int
)

/**
 * Throughput test result
 */
@Serializable
data class ThroughputTest(
    val peerId: String,
    val uploadKbps: Double,
    val downloadKbps: Double,
    val testDurationMs: Long
)

/**
 * Bluetooth scan result
 */
@Serializable
data class ScanResult(
    val deviceAddress: String,
    val deviceName: String?,
    val rssi: Int,
    val serviceUuids: List<String>,
    val advertisementData: Map<String, String>
)

/**
 * Battery optimization status
 */
@Serializable
data class BatteryOptimizationStatus(
    val isOptimizationEnabled: Boolean,
    val batteryLevel: Int,
    val isCharging: Boolean,
    val estimatedScanTime: Long,
    val recommendations: List<String>
)

// MARK: - Error Models

/**
 * BitCraps specific errors
 */
@Serializable
sealed class BitCrapsError {
    abstract val message: String
    abstract val timestamp: Long
    
    @Serializable
    data class InitializationError(
        override val message: String,
        override val timestamp: Long = System.currentTimeMillis()
    ) : BitCrapsError()
    
    @Serializable
    data class BluetoothError(
        override val message: String,
        val errorCode: Int? = null,
        override val timestamp: Long = System.currentTimeMillis()
    ) : BitCrapsError()
    
    @Serializable
    data class NetworkError(
        override val message: String,
        val networkErrorType: NetworkErrorType,
        override val timestamp: Long = System.currentTimeMillis()
    ) : BitCrapsError()
    
    @Serializable
    data class GameError(
        override val message: String,
        val gameId: String?,
        override val timestamp: Long = System.currentTimeMillis()
    ) : BitCrapsError()
    
    @Serializable
    data class SecurityError(
        override val message: String,
        val securityContext: String?,
        override val timestamp: Long = System.currentTimeMillis()
    ) : BitCrapsError()
    
    @Serializable
    data class PermissionError(
        override val message: String,
        val permissionType: String,
        override val timestamp: Long = System.currentTimeMillis()
    ) : BitCrapsError()
}

/**
 * Network error types
 */
@Serializable
enum class NetworkErrorType {
    CONNECTION_TIMEOUT,
    CONNECTION_REFUSED,
    PEER_UNREACHABLE,
    MESSAGE_SEND_FAILED,
    PROTOCOL_ERROR,
    CONSENSUS_FAILED
}

/**
 * Log levels
 */
@Serializable
enum class LogLevel {
    VERBOSE,
    DEBUG,
    INFO,
    WARN,
    ERROR
}

// MARK: - Game Session Interface

/**
 * Game session handle for active games
 */
interface GameSession {
    val gameId: String
    val gameType: GameType
    val currentState: GameState?
    
    suspend fun placeBet(amount: Long, betType: String)
    suspend fun rollDice(): List<Int>
    suspend fun sendMessage(message: String)
    suspend fun leaveGame()
    
    fun observeState(): kotlinx.coroutines.flow.Flow<GameState>
    fun observeEvents(): kotlinx.coroutines.flow.Flow<GameEvent>
}