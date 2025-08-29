import Foundation
import CoreBluetooth

// MARK: - Core Models

/**
 * Node status information
 */
public struct NodeStatus: Codable, Equatable {
    public let nodeId: String
    public var state: NodeState
    public var bluetoothState: CBManagerState
    public var isDiscoveryActive: Boolean
    public var currentGameId: String?
    public var activeConnectionCount: Int
    public var currentPowerMode: PowerMode
    public var lastUpdateTime: Date
    
    public init(
        nodeId: String,
        state: NodeState = .initializing,
        bluetoothState: CBManagerState = .unknown,
        isDiscoveryActive: Boolean = false,
        currentGameId: String? = nil,
        activeConnectionCount: Int = 0,
        currentPowerMode: PowerMode = .balanced,
        lastUpdateTime: Date = Date()
    ) {
        self.nodeId = nodeId
        self.state = state
        self.bluetoothState = bluetoothState
        self.isDiscoveryActive = isDiscoveryActive
        self.currentGameId = currentGameId
        self.activeConnectionCount = activeConnectionCount
        self.currentPowerMode = currentPowerMode
        self.lastUpdateTime = lastUpdateTime
    }
}

/**
 * Node operational states
 */
public enum NodeState: String, Codable, CaseIterable {
    case initializing
    case ready
    case discovering
    case connected
    case inGame
    case error
    
    public var displayName: String {
        switch self {
        case .initializing: return "Initializing"
        case .ready: return "Ready"
        case .discovering: return "Discovering"
        case .connected: return "Connected"
        case .inGame: return "In Game"
        case .error: return "Error"
        }
    }
    
    public var isOperational: Boolean {
        switch self {
        case .ready, .discovering, .connected, .inGame:
            return true
        case .initializing, .error:
            return false
        }
    }
}

/**
 * Connection status information
 */
public struct ConnectionStatus: Codable, Equatable {
    public let isConnected: Boolean
    public let connectedPeerCount: Int
    public let totalDiscoveredPeers: Int
    public let connectionQuality: ConnectionQuality
    public let lastConnectionTime: Date?
    public let reconnectAttempts: Int
    
    public init(
        isConnected: Boolean = false,
        connectedPeerCount: Int = 0,
        totalDiscoveredPeers: Int = 0,
        connectionQuality: ConnectionQuality = .disconnected,
        lastConnectionTime: Date? = nil,
        reconnectAttempts: Int = 0
    ) {
        self.isConnected = isConnected
        self.connectedPeerCount = connectedPeerCount
        self.totalDiscoveredPeers = totalDiscoveredPeers
        self.connectionQuality = connectionQuality
        self.lastConnectionTime = lastConnectionTime
        self.reconnectAttempts = reconnectAttempts
    }
    
    public static let disconnected = ConnectionStatus()
}

/**
 * Connection quality levels
 */
public enum ConnectionQuality: Int, Codable, CaseIterable {
    case excellent = 4
    case good = 3
    case fair = 2
    case poor = 1
    case disconnected = 0
    
    public var displayName: String {
        switch self {
        case .excellent: return "Excellent"
        case .good: return "Good"
        case .fair: return "Fair"
        case .poor: return "Poor"
        case .disconnected: return "Disconnected"
        }
    }
    
    public var color: String {
        switch self {
        case .excellent: return "green"
        case .good: return "blue"
        case .fair: return "yellow"
        case .poor: return "orange"
        case .disconnected: return "red"
        }
    }
}

/**
 * Peer information
 */
public struct PeerInfo: Codable, Equatable, Identifiable {
    public let id: String
    public var displayName: String?
    public var deviceModel: String?
    public var signalStrength: Int // RSSI value
    public var isConnected: Boolean
    public var lastSeen: Date
    public var capabilities: [String]
    public var trustLevel: TrustLevel
    public var connectionQuality: ConnectionQuality?
    public var batteryLevel: Int?
    public var osVersion: String?
    
    public init(
        id: String,
        displayName: String? = nil,
        deviceModel: String? = nil,
        signalStrength: Int = -100,
        isConnected: Boolean = false,
        lastSeen: Date = Date(),
        capabilities: [String] = [],
        trustLevel: TrustLevel = .unknown,
        connectionQuality: ConnectionQuality? = nil,
        batteryLevel: Int? = nil,
        osVersion: String? = nil
    ) {
        self.id = id
        self.displayName = displayName
        self.deviceModel = deviceModel
        self.signalStrength = signalStrength
        self.isConnected = isConnected
        self.lastSeen = lastSeen
        self.capabilities = capabilities
        self.trustLevel = trustLevel
        self.connectionQuality = connectionQuality
        self.batteryLevel = batteryLevel
        self.osVersion = osVersion
    }
    
    public var signalQuality: ConnectionQuality {
        switch signalStrength {
        case -50...0: return .excellent
        case -65...<(-50): return .good
        case -80...<(-65): return .fair
        case -95...<(-80): return .poor
        default: return .disconnected
        }
    }
}

/**
 * Trust levels for peers
 */
public enum TrustLevel: String, Codable, CaseIterable {
    case trusted
    case verified
    case unknown
    case suspicious
    case blocked
    
    public var displayName: String {
        switch self {
        case .trusted: return "Trusted"
        case .verified: return "Verified"
        case .unknown: return "Unknown"
        case .suspicious: return "Suspicious"
        case .blocked: return "Blocked"
        }
    }
    
    public var color: String {
        switch self {
        case .trusted: return "green"
        case .verified: return "blue"
        case .unknown: return "gray"
        case .suspicious: return "orange"
        case .blocked: return "red"
        }
    }
}

// MARK: - Game Models

/**
 * Available game information
 */
public struct AvailableGame: Codable, Equatable, Identifiable {
    public let id: String
    public let gameType: GameType
    public let hostPeerId: String
    public let hostDisplayName: String?
    public let currentPlayers: Int
    public let maxPlayers: Int
    public let minBet: Int64
    public let maxBet: Int64
    public let gameState: PublicGameState
    public let requiresPassword: Boolean
    public let createdAt: Date
    
    public init(
        id: String,
        gameType: GameType,
        hostPeerId: String,
        hostDisplayName: String? = nil,
        currentPlayers: Int,
        maxPlayers: Int,
        minBet: Int64,
        maxBet: Int64,
        gameState: PublicGameState,
        requiresPassword: Boolean = false,
        createdAt: Date = Date()
    ) {
        self.id = id
        self.gameType = gameType
        self.hostPeerId = hostPeerId
        self.hostDisplayName = hostDisplayName
        self.currentPlayers = currentPlayers
        self.maxPlayers = maxPlayers
        self.minBet = minBet
        self.maxBet = maxBet
        self.gameState = gameState
        self.requiresPassword = requiresPassword
        self.createdAt = createdAt
    }
    
    public var isFull: Boolean {
        return currentPlayers >= maxPlayers
    }
    
    public var canJoin: Boolean {
        return !isFull && gameState.status == .waitingForPlayers
    }
}

/**
 * Game types supported by the platform
 */
public enum GameType: String, Codable, CaseIterable {
    case craps
    case poker
    case blackjack
    case diceRoll
    case custom
    
    public var displayName: String {
        switch self {
        case .craps: return "Craps"
        case .poker: return "Poker"
        case .blackjack: return "Blackjack"
        case .diceRoll: return "Dice Roll"
        case .custom: return "Custom"
        }
    }
    
    public var iconName: String {
        switch self {
        case .craps: return "dice"
        case .poker: return "suit.spade"
        case .blackjack: return "suit.heart"
        case .diceRoll: return "die.face.1"
        case .custom: return "gamecontroller"
        }
    }
}

/**
 * Public game state (visible to all peers)
 */
public struct PublicGameState: Codable, Equatable {
    public let status: GameStatus
    public let currentRound: Int
    public let phase: GamePhase
    public let timeRemaining: TimeInterval?
    public let lastAction: String?
    public let lastActionTime: Date?
    
    public init(
        status: GameStatus,
        currentRound: Int = 0,
        phase: GamePhase = .lobby,
        timeRemaining: TimeInterval? = nil,
        lastAction: String? = nil,
        lastActionTime: Date? = nil
    ) {
        self.status = status
        self.currentRound = currentRound
        self.phase = phase
        self.timeRemaining = timeRemaining
        self.lastAction = lastAction
        self.lastActionTime = lastActionTime
    }
    
    public var isActive: Boolean {
        return status == .inProgress
    }
}

/**
 * Game status
 */
public enum GameStatus: String, Codable, CaseIterable {
    case waitingForPlayers
    case inProgress
    case paused
    case completed
    case cancelled
    
    public var displayName: String {
        switch self {
        case .waitingForPlayers: return "Waiting for Players"
        case .inProgress: return "In Progress"
        case .paused: return "Paused"
        case .completed: return "Completed"
        case .cancelled: return "Cancelled"
        }
    }
    
    public var color: String {
        switch self {
        case .waitingForPlayers: return "yellow"
        case .inProgress: return "green"
        case .paused: return "orange"
        case .completed: return "blue"
        case .cancelled: return "red"
        }
    }
}

/**
 * Game phase (specific to game type)
 */
public enum GamePhase: String, Codable, CaseIterable {
    case lobby
    case betting
    case rolling
    case resolving
    case payout
    case finished
    
    public var displayName: String {
        switch self {
        case .lobby: return "Lobby"
        case .betting: return "Betting"
        case .rolling: return "Rolling"
        case .resolving: return "Resolving"
        case .payout: return "Payout"
        case .finished: return "Finished"
        }
    }
}

/**
 * Complete game state
 */
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
    
    public init(
        id: String = UUID().uuidString,
        gameId: String,
        gameType: GameType,
        publicState: PublicGameState,
        players: [Player] = [],
        currentPlayer: Player? = nil,
        pot: Int64 = 0,
        round: GameRound? = nil,
        history: [GameAction] = [],
        myPlayerId: String? = nil
    ) {
        self.id = id
        self.gameId = gameId
        self.gameType = gameType
        self.publicState = publicState
        self.players = players
        self.currentPlayer = currentPlayer
        self.pot = pot
        self.round = round
        self.history = history
        self.myPlayerId = myPlayerId
    }
    
    public var isMyTurn: Boolean {
        return currentPlayer?.playerId == myPlayerId
    }
    
    public var myPlayer: Player? {
        return players.first { $0.playerId == myPlayerId }
    }
}

/**
 * Player information
 */
public struct Player: Codable, Equatable, Identifiable {
    public let id: String
    public let playerId: String
    public let peerId: String
    public let displayName: String?
    public let balance: Int64
    public let currentBet: Int64
    public let isActive: Boolean
    public let position: Int
    public let statistics: PlayerStatistics
    public let joinedAt: Date
    
    public init(
        id: String = UUID().uuidString,
        playerId: String,
        peerId: String,
        displayName: String? = nil,
        balance: Int64 = 0,
        currentBet: Int64 = 0,
        isActive: Boolean = true,
        position: Int = 0,
        statistics: PlayerStatistics = PlayerStatistics(),
        joinedAt: Date = Date()
    ) {
        self.id = id
        self.playerId = playerId
        self.peerId = peerId
        self.displayName = displayName
        self.balance = balance
        self.currentBet = currentBet
        self.isActive = isActive
        self.position = position
        self.statistics = statistics
        self.joinedAt = joinedAt
    }
    
    public var effectiveDisplayName: String {
        return displayName ?? "Player \(position + 1)"
    }
}

/**
 * Player statistics
 */
public struct PlayerStatistics: Codable, Equatable {
    public let gamesPlayed: Int
    public let gamesWon: Int
    public let totalWinnings: Int64
    public let averageBet: Int64
    public let winRate: Double
    public let lastGameTime: Date?
    
    public init(
        gamesPlayed: Int = 0,
        gamesWon: Int = 0,
        totalWinnings: Int64 = 0,
        averageBet: Int64 = 0,
        winRate: Double = 0.0,
        lastGameTime: Date? = nil
    ) {
        self.gamesPlayed = gamesPlayed
        self.gamesWon = gamesWon
        self.totalWinnings = totalWinnings
        self.averageBet = averageBet
        self.winRate = winRate
        self.lastGameTime = lastGameTime
    }
}

/**
 * Game round information
 */
public struct GameRound: Codable, Equatable, Identifiable {
    public let id: String
    public let roundId: String
    public let roundNumber: Int
    public let startTime: Date
    public let currentPlayer: String?
    public let phase: GamePhase
    public let dice: [Int]?
    public let bets: [String: Int64]
    public let results: RoundResult?
    
    public init(
        id: String = UUID().uuidString,
        roundId: String,
        roundNumber: Int,
        startTime: Date = Date(),
        currentPlayer: String? = nil,
        phase: GamePhase = .betting,
        dice: [Int]? = nil,
        bets: [String: Int64] = [:],
        results: RoundResult? = nil
    ) {
        self.id = id
        self.roundId = roundId
        self.roundNumber = roundNumber
        self.startTime = startTime
        self.currentPlayer = currentPlayer
        self.phase = phase
        self.dice = dice
        self.bets = bets
        self.results = results
    }
    
    public var totalBets: Int64 {
        return bets.values.reduce(0, +)
    }
    
    public var diceTotal: Int {
        return dice?.reduce(0, +) ?? 0
    }
}

/**
 * Round results
 */
public struct RoundResult: Codable, Equatable {
    public let winners: [String]
    public let payouts: [String: Int64]
    public let houseEdge: Int64
    public let completedAt: Date
    public let winningCondition: String?
    
    public init(
        winners: [String] = [],
        payouts: [String: Int64] = [:],
        houseEdge: Int64 = 0,
        completedAt: Date = Date(),
        winningCondition: String? = nil
    ) {
        self.winners = winners
        self.payouts = payouts
        self.houseEdge = houseEdge
        self.completedAt = completedAt
        self.winningCondition = winningCondition
    }
    
    public var totalPayouts: Int64 {
        return payouts.values.reduce(0, +)
    }
}

/**
 * Game actions/events
 */
public enum GameAction: Codable, Equatable, Identifiable {
    case joinGame(actionId: String, playerId: String, timestamp: Date)
    case placeBet(actionId: String, playerId: String, amount: Int64, betType: String, timestamp: Date)
    case rollDice(actionId: String, playerId: String, dice: [Int], timestamp: Date)
    case leaveGame(actionId: String, playerId: String, timestamp: Date)
    case gameStateChanged(actionId: String, newState: PublicGameState, timestamp: Date)
    
    public var id: String {
        switch self {
        case .joinGame(let actionId, _, _): return actionId
        case .placeBet(let actionId, _, _, _, _): return actionId
        case .rollDice(let actionId, _, _, _): return actionId
        case .leaveGame(let actionId, _, _): return actionId
        case .gameStateChanged(let actionId, _, _): return actionId
        }
    }
    
    public var timestamp: Date {
        switch self {
        case .joinGame(_, _, let timestamp): return timestamp
        case .placeBet(_, _, _, _, let timestamp): return timestamp
        case .rollDice(_, _, _, let timestamp): return timestamp
        case .leaveGame(_, _, let timestamp): return timestamp
        case .gameStateChanged(_, _, let timestamp): return timestamp
        }
    }
    
    public var playerId: String {
        switch self {
        case .joinGame(_, let playerId, _): return playerId
        case .placeBet(_, let playerId, _, _, _): return playerId
        case .rollDice(_, let playerId, _, _): return playerId
        case .leaveGame(_, let playerId, _): return playerId
        case .gameStateChanged: return ""
        }
    }
    
    public var displayDescription: String {
        switch self {
        case .joinGame(_, let playerId, _):
            return "Player \(playerId) joined the game"
        case .placeBet(_, let playerId, let amount, let betType, _):
            return "Player \(playerId) bet \(amount) on \(betType)"
        case .rollDice(_, let playerId, let dice, _):
            let diceStr = dice.map { String($0) }.joined(separator: ", ")
            return "Player \(playerId) rolled: \(diceStr)"
        case .leaveGame(_, let playerId, _):
            return "Player \(playerId) left the game"
        case .gameStateChanged(_, let newState, _):
            return "Game state changed to \(newState.phase.displayName)"
        }
    }
}

// MARK: - Event Models

/**
 * Game events that can be observed
 */
public enum GameEvent: Equatable, Identifiable {
    case sdkInitialized
    case discoveryStarted
    case discoveryStopped
    case peerDiscovered(peer: PeerInfo)
    case peerConnected(peerId: String)
    case peerDisconnected(peerId: String, reason: String?)
    case gameCreated(gameId: String, gameType: GameType)
    case gameJoined(gameId: String)
    case gameLeft(gameId: String)
    case gameStateChanged(gameId: String, newState: GameState)
    case betPlaced(gameId: String, playerId: String, amount: Int64, betType: String)
    case diceRolled(gameId: String, playerId: String, dice: [Int])
    case roundCompleted(gameId: String, results: RoundResult)
    case messageSent(to: String, message: String)
    case messageReceived(from: String, message: String)
    case powerModeChanged(powerMode: PowerMode)
    case bluetoothStateChanged(state: CBManagerState)
    case bluetoothConfigurationChanged(config: BluetoothConfig)
    case biometricAuthenticationChanged(enabled: Boolean)
    case biometricAuthenticationSucceeded
    case biometricAuthenticationFailed(reason: String)
    case batteryOptimizationDetected(reason: String)
    case securityViolationDetected(violation: String)
    case gameHistoryExported
    case gameHistoryImported
    case nodeReset
    case errorOccurred(error: BitCrapsError)
    
    public var id: String {
        switch self {
        case .sdkInitialized: return "sdk_initialized"
        case .discoveryStarted: return "discovery_started"
        case .discoveryStopped: return "discovery_stopped"
        case .peerDiscovered(let peer): return "peer_discovered_\(peer.id)"
        case .peerConnected(let peerId): return "peer_connected_\(peerId)"
        case .peerDisconnected(let peerId, _): return "peer_disconnected_\(peerId)"
        case .gameCreated(let gameId, _): return "game_created_\(gameId)"
        case .gameJoined(let gameId): return "game_joined_\(gameId)"
        case .gameLeft(let gameId): return "game_left_\(gameId)"
        case .gameStateChanged(let gameId, _): return "game_state_changed_\(gameId)"
        case .betPlaced(let gameId, let playerId, _, _): return "bet_placed_\(gameId)_\(playerId)"
        case .diceRolled(let gameId, let playerId, _): return "dice_rolled_\(gameId)_\(playerId)"
        case .roundCompleted(let gameId, _): return "round_completed_\(gameId)"
        case .messageSent(let to, _): return "message_sent_\(to)"
        case .messageReceived(let from, _): return "message_received_\(from)"
        case .powerModeChanged: return "power_mode_changed"
        case .bluetoothStateChanged: return "bluetooth_state_changed"
        case .bluetoothConfigurationChanged: return "bluetooth_config_changed"
        case .biometricAuthenticationChanged: return "biometric_auth_changed"
        case .biometricAuthenticationSucceeded: return "biometric_auth_succeeded"
        case .biometricAuthenticationFailed: return "biometric_auth_failed"
        case .batteryOptimizationDetected: return "battery_optimization_detected"
        case .securityViolationDetected: return "security_violation_detected"
        case .gameHistoryExported: return "game_history_exported"
        case .gameHistoryImported: return "game_history_imported"
        case .nodeReset: return "node_reset"
        case .errorOccurred: return "error_occurred"
        }
    }
    
    public var displayDescription: String {
        switch self {
        case .sdkInitialized: return "SDK initialized"
        case .discoveryStarted: return "Started discovering peers"
        case .discoveryStopped: return "Stopped discovering peers"
        case .peerDiscovered(let peer): return "Discovered peer: \(peer.displayName ?? peer.id)"
        case .peerConnected(let peerId): return "Connected to peer: \(peerId)"
        case .peerDisconnected(let peerId, let reason): return "Disconnected from peer: \(peerId). Reason: \(reason ?? "Unknown")"
        case .gameCreated(let gameId, let gameType): return "Created \(gameType.displayName) game: \(gameId)"
        case .gameJoined(let gameId): return "Joined game: \(gameId)"
        case .gameLeft(let gameId): return "Left game: \(gameId)"
        case .gameStateChanged(let gameId, let newState): return "Game \(gameId) state changed to \(newState.publicState.phase.displayName)"
        case .betPlaced(_, let playerId, let amount, let betType): return "Player \(playerId) bet \(amount) on \(betType)"
        case .diceRolled(_, let playerId, let dice): return "Player \(playerId) rolled: \(dice.map(String.init).joined(separator: ", "))"
        case .roundCompleted(_, let results): return "Round completed with \(results.winners.count) winners"
        case .messageSent(let to, let message): return "Sent message to \(to): \(message)"
        case .messageReceived(let from, let message): return "Received message from \(from): \(message)"
        case .powerModeChanged(let powerMode): return "Power mode changed to \(powerMode.displayName)"
        case .bluetoothStateChanged(let state): return "Bluetooth state changed to \(state.description)"
        case .bluetoothConfigurationChanged: return "Bluetooth configuration updated"
        case .biometricAuthenticationChanged(let enabled): return "Biometric authentication \(enabled ? "enabled" : "disabled")"
        case .biometricAuthenticationSucceeded: return "Biometric authentication succeeded"
        case .biometricAuthenticationFailed(let reason): return "Biometric authentication failed: \(reason)"
        case .batteryOptimizationDetected(let reason): return "Battery optimization detected: \(reason)"
        case .securityViolationDetected(let violation): return "Security violation detected: \(violation)"
        case .gameHistoryExported: return "Game history exported"
        case .gameHistoryImported: return "Game history imported"
        case .nodeReset: return "Node reset completed"
        case .errorOccurred(let error): return "Error occurred: \(error.displayMessage)"
        }
    }
}

/**
 * Peer message structure
 */
public struct PeerMessage: Codable, Equatable, Identifiable {
    public let id: String
    public let fromPeerId: String
    public let content: String
    public let timestamp: Date
    public let messageType: MessageType
    public let isEncrypted: Boolean
    
    public init(
        id: String = UUID().uuidString,
        fromPeerId: String,
        content: String,
        timestamp: Date = Date(),
        messageType: MessageType = .text,
        isEncrypted: Boolean = true
    ) {
        self.id = id
        self.fromPeerId = fromPeerId
        self.content = content
        self.timestamp = timestamp
        self.messageType = messageType
        self.isEncrypted = isEncrypted
    }
    
    public enum MessageType: String, Codable, CaseIterable {
        case text
        case gameData
        case systemNotification
        case error
        
        public var displayName: String {
            switch self {
            case .text: return "Text"
            case .gameData: return "Game Data"
            case .systemNotification: return "System"
            case .error: return "Error"
            }
        }
    }
}

// MARK: - Configuration Models

/**
 * Power management modes
 */
public enum PowerMode: String, Codable, CaseIterable {
    case highPerformance
    case balanced
    case batterySaver
    case ultraLowPower
    
    public var displayName: String {
        switch self {
        case .highPerformance: return "High Performance"
        case .balanced: return "Balanced"
        case .batterySaver: return "Battery Saver"
        case .ultraLowPower: return "Ultra Low Power"
        }
    }
    
    public var scanInterval: TimeInterval {
        switch self {
        case .highPerformance: return 1.0
        case .balanced: return 2.0
        case .batterySaver: return 5.0
        case .ultraLowPower: return 10.0
        }
    }
    
    public var scanWindow: TimeInterval {
        switch self {
        case .highPerformance: return 0.5
        case .balanced: return 0.3
        case .batterySaver: return 0.1
        case .ultraLowPower: return 0.05
        }
    }
}

/**
 * Discovery configuration
 */
public struct DiscoveryConfig: Codable, Equatable {
    public let scanWindowMs: Int
    public let scanIntervalMs: Int
    public let discoveryTimeoutMs: Int64
    public let maxPeers: Int
    public let serviceUuids: [String]
    public let powerMode: PowerMode
    public let allowDuplicates: Boolean
    
    public init(
        scanWindowMs: Int = 300,
        scanIntervalMs: Int = 2000,
        discoveryTimeoutMs: Int64 = 30000,
        maxPeers: Int = 50,
        serviceUuids: [String] = Self.defaultServiceUuids,
        powerMode: PowerMode = .balanced,
        allowDuplicates: Boolean = false
    ) {
        self.scanWindowMs = scanWindowMs
        self.scanIntervalMs = scanIntervalMs
        self.discoveryTimeoutMs = discoveryTimeoutMs
        self.maxPeers = maxPeers
        self.serviceUuids = serviceUuids
        self.powerMode = powerMode
        self.allowDuplicates = allowDuplicates
    }
    
    public static let defaultServiceUuids = ["12345678-1234-5678-1234-567812345678"]
    
    public static let `default` = DiscoveryConfig()
    
    public static let lowPower = DiscoveryConfig(
        scanWindowMs: 100,
        scanIntervalMs: 5000,
        powerMode: .batterySaver
    )
    
    public static let highPerformance = DiscoveryConfig(
        scanWindowMs: 500,
        scanIntervalMs: 1000,
        powerMode: .highPerformance,
        allowDuplicates: true
    )
}

/**
 * Game configuration
 */
public struct GameConfig: Codable, Equatable {
    public let gameName: String?
    public let gameType: GameType
    public let minBet: Int64
    public let maxBet: Int64
    public let maxPlayers: Int
    public let minPlayers: Int
    public let timeoutSeconds: Int
    public let houseEdgePercent: Double
    public let requireBiometric: Boolean
    public let enableChat: Boolean
    public let privateGame: Boolean
    public let password: String?
    
    public init(
        gameName: String? = nil,
        gameType: GameType = .craps,
        minBet: Int64 = 1,
        maxBet: Int64 = 1000,
        maxPlayers: Int = 8,
        minPlayers: Int = 2,
        timeoutSeconds: Int = 300,
        houseEdgePercent: Double = 2.5,
        requireBiometric: Boolean = false,
        enableChat: Boolean = true,
        privateGame: Boolean = false,
        password: String? = nil
    ) {
        self.gameName = gameName
        self.gameType = gameType
        self.minBet = minBet
        self.maxBet = maxBet
        self.maxPlayers = maxPlayers
        self.minPlayers = minPlayers
        self.timeoutSeconds = timeoutSeconds
        self.houseEdgePercent = houseEdgePercent
        self.requireBiometric = requireBiometric
        self.enableChat = enableChat
        self.privateGame = privateGame
        self.password = password
    }
    
    public static let `default` = GameConfig()
    
    public static let quickGame = GameConfig(
        timeoutSeconds: 60,
        maxPlayers: 4
    )
    
    public static let tournamentGame = GameConfig(
        minBet: 10,
        maxBet: 10000,
        maxPlayers: 16,
        timeoutSeconds: 600,
        requireBiometric: true
    )
}

/**
 * Bluetooth configuration
 */
public struct BluetoothConfig: Codable, Equatable {
    public let advertisingIntervalMs: Int
    public let connectionIntervalMs: Int
    public let mtuSize: Int
    public let txPowerLevel: TxPowerLevel
    public let enableEncryption: Boolean
    public let maxConnections: Int
    public let backgroundScanningEnabled: Boolean
    
    public init(
        advertisingIntervalMs: Int = 1000,
        connectionIntervalMs: Int = 100,
        mtuSize: Int = 247,
        txPowerLevel: TxPowerLevel = .medium,
        enableEncryption: Boolean = true,
        maxConnections: Int = 7,
        backgroundScanningEnabled: Boolean = false
    ) {
        self.advertisingIntervalMs = advertisingIntervalMs
        self.connectionIntervalMs = connectionIntervalMs
        self.mtuSize = mtuSize
        self.txPowerLevel = txPowerLevel
        self.enableEncryption = enableEncryption
        self.maxConnections = maxConnections
        self.backgroundScanningEnabled = backgroundScanningEnabled
    }
    
    public static let `default` = BluetoothConfig()
    
    public static let lowPower = BluetoothConfig(
        advertisingIntervalMs: 2000,
        connectionIntervalMs: 200,
        txPowerLevel: .low,
        maxConnections: 3
    )
    
    public static let highPerformance = BluetoothConfig(
        advertisingIntervalMs: 500,
        connectionIntervalMs: 50,
        txPowerLevel: .high,
        maxConnections: 7
    )
}

/**
 * Bluetooth transmission power levels
 */
public enum TxPowerLevel: String, Codable, CaseIterable {
    case ultraLow
    case low
    case medium
    case high
    
    public var displayName: String {
        switch self {
        case .ultraLow: return "Ultra Low"
        case .low: return "Low"
        case .medium: return "Medium"
        case .high: return "High"
        }
    }
    
    public var powerValue: Int {
        switch self {
        case .ultraLow: return -20
        case .low: return -10
        case .medium: return 0
        case .high: return 4
        }
    }
}

/**
 * SDK configuration
 */
public struct SDKConfig: Codable, Equatable {
    public let dataDirectory: String?
    public let enableLogging: Boolean
    public let logLevel: LogLevel
    public let defaultPowerMode: PowerMode
    public let enableBiometrics: Boolean
    public let enableBackgroundScanning: Boolean
    public let maxEventHistorySize: Int
    public let networkTimeoutMs: Int64
    public let discoveryConfig: DiscoveryConfig
    public let bluetoothConfig: BluetoothConfig
    public let foregroundEventPollingInterval: TimeInterval
    public let backgroundEventPollingInterval: TimeInterval
    
    public init(
        dataDirectory: String? = nil,
        enableLogging: Boolean = true,
        logLevel: LogLevel = .info,
        defaultPowerMode: PowerMode = .balanced,
        enableBiometrics: Boolean = true,
        enableBackgroundScanning: Boolean = false,
        maxEventHistorySize: Int = 100,
        networkTimeoutMs: Int64 = 30000,
        discoveryConfig: DiscoveryConfig = .default,
        bluetoothConfig: BluetoothConfig = .default,
        foregroundEventPollingInterval: TimeInterval = 0.1,
        backgroundEventPollingInterval: TimeInterval = 1.0
    ) {
        self.dataDirectory = dataDirectory
        self.enableLogging = enableLogging
        self.logLevel = logLevel
        self.defaultPowerMode = defaultPowerMode
        self.enableBiometrics = enableBiometrics
        self.enableBackgroundScanning = enableBackgroundScanning
        self.maxEventHistorySize = maxEventHistorySize
        self.networkTimeoutMs = networkTimeoutMs
        self.discoveryConfig = discoveryConfig
        self.bluetoothConfig = bluetoothConfig
        self.foregroundEventPollingInterval = foregroundEventPollingInterval
        self.backgroundEventPollingInterval = backgroundEventPollingInterval
    }
    
    public static let `default` = SDKConfig()
    
    public static let production = SDKConfig(
        logLevel: .warning,
        maxEventHistorySize: 50,
        enableBackgroundScanning: true
    )
    
    public static let development = SDKConfig(
        logLevel: .debug,
        maxEventHistorySize: 1000,
        foregroundEventPollingInterval: 0.05
    )
}

// MARK: - Statistics and Diagnostics Models

/**
 * Network statistics
 */
public struct NetworkStats: Codable, Equatable {
    public let bytesTransferred: Int64
    public let messagesReceived: Int64
    public let messagesSent: Int64
    public let averageLatencyMs: Double
    public let connectionUptime: TimeInterval
    public let droppedConnections: Int
    public let peersDiscovered: Int
    public let peersConnected: Int
    public let gameStateLatency: Double
    public let consensusLatency: Double
    public let throughputKbps: Double
    public let lastUpdated: Date
    
    public init(
        bytesTransferred: Int64 = 0,
        messagesReceived: Int64 = 0,
        messagesSent: Int64 = 0,
        averageLatencyMs: Double = 0,
        connectionUptime: TimeInterval = 0,
        droppedConnections: Int = 0,
        peersDiscovered: Int = 0,
        peersConnected: Int = 0,
        gameStateLatency: Double = 0,
        consensusLatency: Double = 0,
        throughputKbps: Double = 0,
        lastUpdated: Date = Date()
    ) {
        self.bytesTransferred = bytesTransferred
        self.messagesReceived = messagesReceived
        self.messagesSent = messagesSent
        self.averageLatencyMs = averageLatencyMs
        self.connectionUptime = connectionUptime
        self.droppedConnections = droppedConnections
        self.peersDiscovered = peersDiscovered
        self.peersConnected = peersConnected
        self.gameStateLatency = gameStateLatency
        self.consensusLatency = consensusLatency
        self.throughputKbps = throughputKbps
        self.lastUpdated = lastUpdated
    }
}

/**
 * Performance metrics
 */
public struct PerformanceMetrics: Codable, Equatable {
    public let cpuUsagePercent: Double
    public let memoryUsageMB: Double
    public let batteryLevel: Int
    public let bluetoothLatency: Double
    public let gameStateUpdateLatency: Double
    public let consensusLatency: Double
    public let networkThroughput: Double
    public let frameRate: Double
    public let lastMeasured: Date
    
    public init(
        cpuUsagePercent: Double = 0,
        memoryUsageMB: Double = 0,
        batteryLevel: Int = 100,
        bluetoothLatency: Double = 0,
        gameStateUpdateLatency: Double = 0,
        consensusLatency: Double = 0,
        networkThroughput: Double = 0,
        frameRate: Double = 60,
        lastMeasured: Date = Date()
    ) {
        self.cpuUsagePercent = cpuUsagePercent
        self.memoryUsageMB = memoryUsageMB
        self.batteryLevel = batteryLevel
        self.bluetoothLatency = bluetoothLatency
        self.gameStateUpdateLatency = gameStateUpdateLatency
        self.consensusLatency = consensusLatency
        self.networkThroughput = networkThroughput
        self.frameRate = frameRate
        self.lastMeasured = lastMeasured
    }
}

/**
 * Battery optimization status
 */
public struct BatteryOptimizationStatus: Codable, Equatable {
    public let isOptimizationActive: Boolean
    public let batteryLevel: Int
    public let isCharging: Boolean
    public let estimatedScanTime: TimeInterval
    public let recommendations: [String]
    public let reason: String?
    
    public init(
        isOptimizationActive: Boolean = false,
        batteryLevel: Int = 100,
        isCharging: Boolean = false,
        estimatedScanTime: TimeInterval = 0,
        recommendations: [String] = [],
        reason: String? = nil
    ) {
        self.isOptimizationActive = isOptimizationActive
        self.batteryLevel = batteryLevel
        self.isCharging = isCharging
        self.estimatedScanTime = estimatedScanTime
        self.recommendations = recommendations
        self.reason = reason
    }
}

/**
 * Diagnostics report
 */
public struct DiagnosticsReport: Codable, Equatable, Identifiable {
    public let id: String
    public let systemInfo: SystemInfo
    public let networkDiagnostics: NetworkDiagnostics
    public let bluetoothDiagnostics: BluetoothDiagnostics
    public let performanceMetrics: PerformanceMetrics
    public let recommendations: [String]
    public let overallScore: Double
    public let generatedAt: Date
    
    public init(
        id: String = UUID().uuidString,
        systemInfo: SystemInfo,
        networkDiagnostics: NetworkDiagnostics,
        bluetoothDiagnostics: BluetoothDiagnostics,
        performanceMetrics: PerformanceMetrics,
        recommendations: [String] = [],
        overallScore: Double = 0.0,
        generatedAt: Date = Date()
    ) {
        self.id = id
        self.systemInfo = systemInfo
        self.networkDiagnostics = networkDiagnostics
        self.bluetoothDiagnostics = bluetoothDiagnostics
        self.performanceMetrics = performanceMetrics
        self.recommendations = recommendations
        self.overallScore = overallScore
        self.generatedAt = generatedAt
    }
}

/**
 * System information
 */
public struct SystemInfo: Codable, Equatable {
    public let deviceModel: String
    public let osVersion: String
    public let sdkVersion: String
    public let availableMemoryMB: Int64
    public let totalMemoryMB: Int64
    public let batteryLevel: Int
    public let bluetoothVersion: String
    public let isCharging: Boolean
    public let processorCount: Int
    
    public init(
        deviceModel: String,
        osVersion: String,
        sdkVersion: String,
        availableMemoryMB: Int64,
        totalMemoryMB: Int64,
        batteryLevel: Int,
        bluetoothVersion: String,
        isCharging: Boolean,
        processorCount: Int
    ) {
        self.deviceModel = deviceModel
        self.osVersion = osVersion
        self.sdkVersion = sdkVersion
        self.availableMemoryMB = availableMemoryMB
        self.totalMemoryMB = totalMemoryMB
        self.batteryLevel = batteryLevel
        self.bluetoothVersion = bluetoothVersion
        self.isCharging = isCharging
        self.processorCount = processorCount
    }
}

/**
 * Network diagnostics
 */
public struct NetworkDiagnostics: Codable, Equatable {
    public let connectivity: ConnectivityStatus
    public let latencyTests: [LatencyTest]
    public let throughputTests: [ThroughputTest]
    public let packetLoss: Double
    public let averageResponseTime: Double
    public let peakThroughput: Double
    
    public init(
        connectivity: ConnectivityStatus,
        latencyTests: [LatencyTest] = [],
        throughputTests: [ThroughputTest] = [],
        packetLoss: Double = 0.0,
        averageResponseTime: Double = 0.0,
        peakThroughput: Double = 0.0
    ) {
        self.connectivity = connectivity
        self.latencyTests = latencyTests
        self.throughputTests = throughputTests
        self.packetLoss = packetLoss
        self.averageResponseTime = averageResponseTime
        self.peakThroughput = peakThroughput
    }
}

/**
 * Bluetooth diagnostics
 */
public struct BluetoothDiagnostics: Codable, Equatable {
    public let bluetoothEnabled: Boolean
    public let advertisingSupported: Boolean
    public let centralRoleSupported: Boolean
    public let peripheralRoleSupported: Boolean
    public let maxConnections: Int
    public let currentConnections: Int
    public let scanResults: [ScanResult]
    public let averageRSSI: Int
    public let connectionSuccess: Double
    
    public init(
        bluetoothEnabled: Boolean,
        advertisingSupported: Boolean,
        centralRoleSupported: Boolean,
        peripheralRoleSupported: Boolean,
        maxConnections: Int,
        currentConnections: Int,
        scanResults: [ScanResult] = [],
        averageRSSI: Int = -100,
        connectionSuccess: Double = 0.0
    ) {
        self.bluetoothEnabled = bluetoothEnabled
        self.advertisingSupported = advertisingSupported
        self.centralRoleSupported = centralRoleSupported
        self.peripheralRoleSupported = peripheralRoleSupported
        self.maxConnections = maxConnections
        self.currentConnections = currentConnections
        self.scanResults = scanResults
        self.averageRSSI = averageRSSI
        self.connectionSuccess = connectionSuccess
    }
}

/**
 * Connectivity status
 */
public enum ConnectivityStatus: String, Codable, CaseIterable {
    case excellent
    case good
    case fair
    case poor
    case offline
    
    public var displayName: String {
        switch self {
        case .excellent: return "Excellent"
        case .good: return "Good"
        case .fair: return "Fair"
        case .poor: return "Poor"
        case .offline: return "Offline"
        }
    }
    
    public var color: String {
        switch self {
        case .excellent: return "green"
        case .good: return "blue"
        case .fair: return "yellow"
        case .poor: return "orange"
        case .offline: return "red"
        }
    }
}

/**
 * Latency test result
 */
public struct LatencyTest: Codable, Equatable, Identifiable {
    public let id: String
    public let peerId: String
    public let averageLatencyMs: Double
    public let minLatencyMs: Double
    public let maxLatencyMs: Double
    public let packetsSent: Int
    public let packetsReceived: Int
    public let testDuration: TimeInterval
    
    public init(
        id: String = UUID().uuidString,
        peerId: String,
        averageLatencyMs: Double,
        minLatencyMs: Double,
        maxLatencyMs: Double,
        packetsSent: Int,
        packetsReceived: Int,
        testDuration: TimeInterval
    ) {
        self.id = id
        self.peerId = peerId
        self.averageLatencyMs = averageLatencyMs
        self.minLatencyMs = minLatencyMs
        self.maxLatencyMs = maxLatencyMs
        self.packetsSent = packetsSent
        self.packetsReceived = packetsReceived
        self.testDuration = testDuration
    }
    
    public var packetLossRate: Double {
        guard packetsSent > 0 else { return 0.0 }
        return Double(packetsSent - packetsReceived) / Double(packetsSent)
    }
}

/**
 * Throughput test result
 */
public struct ThroughputTest: Codable, Equatable, Identifiable {
    public let id: String
    public let peerId: String
    public let uploadKbps: Double
    public let downloadKbps: Double
    public let testDurationMs: Int64
    public let dataSentBytes: Int64
    public let dataReceivedBytes: Int64
    
    public init(
        id: String = UUID().uuidString,
        peerId: String,
        uploadKbps: Double,
        downloadKbps: Double,
        testDurationMs: Int64,
        dataSentBytes: Int64,
        dataReceivedBytes: Int64
    ) {
        self.id = id
        self.peerId = peerId
        self.uploadKbps = uploadKbps
        self.downloadKbps = downloadKbps
        self.testDurationMs = testDurationMs
        self.dataSentBytes = dataSentBytes
        self.dataReceivedBytes = dataReceivedBytes
    }
    
    public var averageThroughput: Double {
        return (uploadKbps + downloadKbps) / 2.0
    }
}

/**
 * Bluetooth scan result
 */
public struct ScanResult: Codable, Equatable, Identifiable {
    public let id: String
    public let deviceAddress: String
    public let deviceName: String?
    public let rssi: Int
    public let serviceUuids: [String]
    public let advertisementData: [String: String]
    public let timestamp: Date
    
    public init(
        id: String = UUID().uuidString,
        deviceAddress: String,
        deviceName: String? = nil,
        rssi: Int,
        serviceUuids: [String] = [],
        advertisementData: [String: String] = [:],
        timestamp: Date = Date()
    ) {
        self.id = id
        self.deviceAddress = deviceAddress
        self.deviceName = deviceName
        self.rssi = rssi
        self.serviceUuids = serviceUuids
        self.advertisementData = advertisementData
        self.timestamp = timestamp
    }
    
    public var signalQuality: ConnectionQuality {
        switch rssi {
        case -50...0: return .excellent
        case -65...<(-50): return .good
        case -80...<(-65): return .fair
        case -95...<(-80): return .poor
        default: return .disconnected
        }
    }
}

// MARK: - Error Models

/**
 * BitCraps specific errors
 */
public enum BitCrapsError: Error, Equatable, Identifiable {
    case initializationFailed(reason: String, underlyingError: Error? = nil)
    case bluetoothError(reason: String, errorCode: Int? = nil, underlyingError: Error? = nil)
    case networkError(reason: String, networkErrorType: NetworkErrorType = .unknown, underlyingError: Error? = nil)
    case gameError(reason: String, gameId: String? = nil, underlyingError: Error? = nil)
    case securityError(reason: String, securityContext: String? = nil, underlyingError: Error? = nil)
    case permissionError(reason: String, permissionType: String, underlyingError: Error? = nil)
    case configurationError(reason: String, configField: String? = nil, underlyingError: Error? = nil)
    case platformError(reason: String, platformErrorType: PlatformErrorType = .unknown, underlyingError: Error? = nil)
    case resourceError(reason: String, resourceType: ResourceType, underlyingError: Error? = nil)
    case stateError(reason: String, expectedState: String? = nil, actualState: String? = nil, underlyingError: Error? = nil)
    case timeoutError(reason: String, timeoutDuration: TimeInterval, operation: String? = nil, underlyingError: Error? = nil)
    case validationError(reason: String, fieldName: String? = nil, fieldValue: String? = nil, underlyingError: Error? = nil)
    
    public var id: String {
        switch self {
        case .initializationFailed: return "initialization_failed"
        case .bluetoothError: return "bluetooth_error"
        case .networkError: return "network_error"
        case .gameError: return "game_error"
        case .securityError: return "security_error"
        case .permissionError: return "permission_error"
        case .configurationError: return "configuration_error"
        case .platformError: return "platform_error"
        case .resourceError: return "resource_error"
        case .stateError: return "state_error"
        case .timeoutError: return "timeout_error"
        case .validationError: return "validation_error"
        }
    }
    
    public var displayMessage: String {
        switch self {
        case .initializationFailed(let reason, _): return reason
        case .bluetoothError(let reason, _, _): return reason
        case .networkError(let reason, _, _): return reason
        case .gameError(let reason, _, _): return reason
        case .securityError(let reason, _, _): return reason
        case .permissionError(let reason, _, _): return reason
        case .configurationError(let reason, _, _): return reason
        case .platformError(let reason, _, _): return reason
        case .resourceError(let reason, _, _): return reason
        case .stateError(let reason, _, _, _): return reason
        case .timeoutError(let reason, _, _, _): return reason
        case .validationError(let reason, _, _, _): return reason
        }
    }
    
    public var isRecoverable: Boolean {
        switch self {
        case .initializationFailed, .securityError, .platformError, .configurationError:
            return false
        case .bluetoothError, .networkError, .gameError, .permissionError, .resourceError, .stateError, .timeoutError, .validationError:
            return true
        }
    }
    
    public var isRetryable: Boolean {
        switch self {
        case .bluetoothError, .networkError, .timeoutError, .resourceError:
            return true
        case .initializationFailed, .gameError, .securityError, .permissionError, .configurationError, .platformError, .stateError, .validationError:
            return false
        }
    }
    
    public static func == (lhs: BitCrapsError, rhs: BitCrapsError) -> Bool {
        return lhs.id == rhs.id && lhs.displayMessage == rhs.displayMessage
    }
    
    public enum NetworkErrorType: String, Codable, CaseIterable {
        case connectionTimeout
        case connectionRefused
        case peerUnreachable
        case messageSendFailed
        case protocolError
        case consensusFailed
        case unknown
    }
    
    public enum PlatformErrorType: String, Codable, CaseIterable {
        case unsupportedOSVersion
        case hardwareNotSupported
        case driverIssue
        case resourceUnavailable
        case unknown
    }
    
    public enum ResourceType: String, Codable, CaseIterable {
        case memory
        case storage
        case battery
        case cpu
        case networkBandwidth
        case fileHandle
        case threadPool
    }
}

/**
 * Log levels
 */
public enum LogLevel: String, Codable, CaseIterable {
    case verbose
    case debug
    case info
    case warning
    case error
    
    public var displayName: String {
        switch self {
        case .verbose: return "Verbose"
        case .debug: return "Debug"
        case .info: return "Info"
        case .warning: return "Warning"
        case .error: return "Error"
        }
    }
    
    public var priority: Int {
        switch self {
        case .verbose: return 0
        case .debug: return 1
        case .info: return 2
        case .warning: return 3
        case .error: return 4
        }
    }
}

// MARK: - Internal Event Models

/**
 * Core manager events (internal)
 */
internal enum CoreEvent {
    case peerDiscovered(PeerInfo)
    case peerConnected(String)
    case peerDisconnected(String, reason: String?)
    case gameStateUpdated(GameState)
    case messageReceived(String, message: String)
    case errorOccurred(BitCrapsError)
}

/**
 * Bluetooth manager events (internal)
 */
internal enum BluetoothEvent {
    case stateChanged(CBManagerState)
    case scanningStarted
    case scanningStopped
    case connectionQualityChanged(String, ConnectionQuality)
    case batteryOptimizationDetected(String)
}

/**
 * Security manager events (internal)
 */
internal enum SecurityEvent {
    case biometricAuthenticationSucceeded
    case biometricAuthenticationFailed(String)
    case securityViolationDetected(String)
    case dataExported
    case dataImported
}

/**
 * Diagnostic events (internal)
 */
internal enum DiagnosticEvent {
    case info(String)
    case warning(String)
    case error(String)
    case diagnosticsCompleted(DiagnosticsReport)
}

// MARK: - Game Session Protocol

/**
 * Game session protocol for active games
 */
public protocol GameSession: AnyObject {
    var gameId: String { get }
    var gameType: GameType { get }
    var currentState: GameState? { get }
    
    func placeBet(amount: Int64, betType: String) async throws
    func rollDice() async throws -> [Int]
    func sendMessage(_ message: String) async throws
    func leaveGame() async throws
    
    var stateUpdates: AnyPublisher<GameState, Never> { get }
    var eventStream: AnyPublisher<GameEvent, Never> { get }
}

// MARK: - Type Aliases

public typealias Boolean = Bool
public typealias Integer = Int
public typealias Double = Swift.Double