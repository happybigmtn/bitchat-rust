import Foundation
import Combine
import os.log

/// Real-time game state synchronization for cross-platform BitCraps gaming
@available(iOS 15.0, *)
class GameStateSynchronizer: ObservableObject {
    private let logger = Logger(subsystem: "com.bitcraps.app", category: "GameStateSynchronizer")
    
    // Configuration
    private let nodeId: String
    private let syncInterval: TimeInterval = 0.1
    private let conflictResolutionTimeout: TimeInterval = 5.0
    private let maxNetworkLatency: TimeInterval = 2.0
    
    // Published state
    @Published var gameState: GameStateSnapshot
    @Published var connectionState: ConnectionState = .disconnected
    @Published var participants: [String: ParticipantInfo] = [:]
    
    // Internal state
    private var vectorClock: VectorClock
    private var pendingOperations: [String: PendingOperation] = [:]
    private var operationHistory: [GameOperation] = []
    private var networkLatencyEstimate: TimeInterval = 0.1
    
    // Network interface
    private var networkInterface: NetworkInterface?
    private var cancellables = Set<AnyCancellable>()
    private var syncTimer: Timer?
    private var conflictResolutionTimer: Timer?
    
    init(nodeId: String? = nil) {
        self.nodeId = nodeId ?? "node_\(Int.random(in: 1000...9999))"
        self.gameState = GameStateSnapshot.initial()
        self.vectorClock = VectorClock(nodeId: self.nodeId)
        
        setupTimers()
        logger.info("Initialized GameStateSynchronizer with nodeId: \(self.nodeId)")
    }
    
    deinit {
        cleanup()
    }
    
    // MARK: - Public API
    
    func connect(networkInterface: NetworkInterface) {
        self.networkInterface = networkInterface
        networkInterface.messageHandler = { [weak self] message in
            self?.handleIncomingMessage(message)
        }
        connectionState = .connected
        logger.info("Connected to network")
    }
    
    func disconnect() {
        networkInterface = nil
        connectionState = .disconnected
        participants.removeAll()
        logger.info("Disconnected from network")
    }
    
    func proposeGameOperation(_ operation: GameOperation) {
        logger.info("Proposing operation: \(operation.type)")
        
        // Add vector clock timestamp
        let timestampedOperation = GameOperation(
            id: generateOperationId(),
            type: operation.type,
            nodeId: nodeId,
            timestamp: vectorClock.increment(),
            data: operation.data
        )
        
        // Apply optimistically for local responsiveness
        applyOperationLocally(timestampedOperation)
        
        // Add to pending operations for conflict resolution
        pendingOperations[timestampedOperation.id] = PendingOperation(
            operation: timestampedOperation,
            timestamp: Date(),
            acknowledged: Set([nodeId])
        )
        
        // Broadcast to network
        broadcastOperation(timestampedOperation)
    }
    
    // MARK: - Private Implementation
    
    private func setupTimers() {
        // Synchronization timer
        syncTimer = Timer.scheduledTimer(withTimeInterval: syncInterval, repeats: true) { [weak self] _ in
            self?.synchronizeWithPeers()
        }
        
        // Conflict resolution timer
        conflictResolutionTimer = Timer.scheduledTimer(withTimeInterval: 1.0, repeats: true) { [weak self] _ in
            self?.resolveConflicts()
            self?.cleanupOldOperations()
        }
    }
    
    private func handleIncomingMessage(_ message: SyncMessage) {
        switch message.type {
        case .gameOperation:
            handleGameOperation(message)
        case .syncRequest:
            handleSyncRequest(message)
        case .syncResponse:
            handleSyncResponse(message)
        case .heartbeat:
            handleHeartbeat(message)
        case .conflictResolution:
            handleConflictResolution(message)
        }
    }
    
    private func handleGameOperation(_ message: SyncMessage) {
        guard let data = message.payload.data(using: .utf8),
              let operation = try? JSONDecoder().decode(GameOperation.self, from: data) else {
            logger.error("Failed to decode game operation")
            return
        }
        
        logger.info("Received operation: \(operation.type) from \(operation.nodeId)")
        
        // Update vector clock
        vectorClock.update(with: operation.timestamp)
        
        // Check for conflicts
        let hasConflict = checkForConflicts(operation)
        
        if hasConflict {
            logger.warning("Conflict detected for operation: \(operation.id)")
            initiateConflictResolution(for: operation)
        } else {
            // Apply operation
            applyOperationLocally(operation)
            acknowledgeOperation(operation)
        }
    }
    
    private func applyOperationLocally(_ operation: GameOperation) {
        let newState: GameStateSnapshot
        
        switch operation.type {
        case .diceRoll:
            newState = GameStateSnapshot(
                version: vectorClock.getTime(),
                gamePhase: gameState.gamePhase,
                dice1: Int(operation.data["dice1"] ?? "0") ?? gameState.dice1,
                dice2: Int(operation.data["dice2"] ?? "0") ?? gameState.dice2,
                isRolling: Bool(operation.data["isRolling"] ?? "false") ?? false,
                point: gameState.point,
                players: gameState.players,
                playerBets: gameState.playerBets,
                totalPot: gameState.totalPot,
                lastRollTimestamp: Date().timeIntervalSince1970,
                lastUpdateTimestamp: Date().timeIntervalSince1970
            )
            
        case .placeBet:
            let playerId = operation.nodeId
            let amount = Int(operation.data["amount"] ?? "0") ?? 0
            var newBets = gameState.playerBets
            newBets[playerId] = (newBets[playerId] ?? 0) + amount
            
            newState = GameStateSnapshot(
                version: vectorClock.getTime(),
                gamePhase: gameState.gamePhase,
                dice1: gameState.dice1,
                dice2: gameState.dice2,
                isRolling: gameState.isRolling,
                point: gameState.point,
                players: gameState.players,
                playerBets: newBets,
                totalPot: gameState.totalPot + amount,
                lastRollTimestamp: gameState.lastRollTimestamp,
                lastUpdateTimestamp: Date().timeIntervalSince1970
            )
            
        case .playerJoin:
            let playerId = operation.nodeId
            let playerName = operation.data["playerName"] ?? "Unknown"
            var newPlayers = gameState.players
            newPlayers[playerId] = PlayerInfo(
                id: playerId,
                name: playerName,
                balance: 1000,
                isConnected: true
            )
            
            newState = GameStateSnapshot(
                version: vectorClock.getTime(),
                gamePhase: gameState.gamePhase,
                dice1: gameState.dice1,
                dice2: gameState.dice2,
                isRolling: gameState.isRolling,
                point: gameState.point,
                players: newPlayers,
                playerBets: gameState.playerBets,
                totalPot: gameState.totalPot,
                lastRollTimestamp: gameState.lastRollTimestamp,
                lastUpdateTimestamp: Date().timeIntervalSince1970
            )
            
        case .playerLeave:
            let playerId = operation.nodeId
            var newPlayers = gameState.players
            var newBets = gameState.playerBets
            newPlayers.removeValue(forKey: playerId)
            newBets.removeValue(forKey: playerId)
            
            newState = GameStateSnapshot(
                version: vectorClock.getTime(),
                gamePhase: gameState.gamePhase,
                dice1: gameState.dice1,
                dice2: gameState.dice2,
                isRolling: gameState.isRolling,
                point: gameState.point,
                players: newPlayers,
                playerBets: newBets,
                totalPot: gameState.totalPot,
                lastRollTimestamp: gameState.lastRollTimestamp,
                lastUpdateTimestamp: Date().timeIntervalSince1970
            )
            
        case .gamePhaseChange:
            let phaseString = operation.data["phase"] ?? "comeOut"
            let newPhase = GamePhase(rawValue: phaseString) ?? .comeOut
            let point = Int(operation.data["point"] ?? "")
            
            newState = GameStateSnapshot(
                version: vectorClock.getTime(),
                gamePhase: newPhase,
                dice1: gameState.dice1,
                dice2: gameState.dice2,
                isRolling: gameState.isRolling,
                point: point,
                players: gameState.players,
                playerBets: gameState.playerBets,
                totalPot: gameState.totalPot,
                lastRollTimestamp: gameState.lastRollTimestamp,
                lastUpdateTimestamp: Date().timeIntervalSince1970
            )
        }
        
        DispatchQueue.main.async {
            self.gameState = newState
        }
        
        // Update operation history
        operationHistory.append(operation)
        if operationHistory.count > 1000 {
            operationHistory.removeFirst()
        }
        
        logger.info("Applied operation: \(operation.type), new state version: \(newState.version)")
    }
    
    private func checkForConflicts(_ operation: GameOperation) -> Bool {
        return pendingOperations.values.contains { pending in
            pending.operation.timestamp.happensBefore(operation.timestamp) &&
            pending.operation.type == operation.type &&
            areOperationsConflicting(pending.operation, operation)
        }
    }
    
    private func areOperationsConflicting(_ op1: GameOperation, _ op2: GameOperation) -> Bool {
        switch (op1.type, op2.type) {
        case (.diceRoll, .diceRoll):
            // Multiple simultaneous dice rolls conflict
            let timestamp1 = Double(op1.data["timestamp"] ?? "0") ?? 0
            let timestamp2 = Double(op2.data["timestamp"] ?? "0") ?? 0
            return abs(timestamp1 - timestamp2) < 1.0
            
        case (.placeBet, .placeBet):
            // Same player betting simultaneously
            return op1.nodeId == op2.nodeId
            
        default:
            return false
        }
    }
    
    private func initiateConflictResolution(for operation: GameOperation) {
        logger.info("Initiating conflict resolution for operation: \(operation.id)")
        
        // Collect all conflicting operations
        let conflictingOps = pendingOperations.values
            .filter { areOperationsConflicting($0.operation, operation) }
            .map { $0.operation } + [operation]
        
        // Resolve using deterministic ordering (timestamp + node ID)
        let resolvedOrder = conflictingOps.sorted { op1, op2 in
            if op1.timestamp.description != op2.timestamp.description {
                return op1.timestamp.description < op2.timestamp.description
            }
            return op1.nodeId < op2.nodeId
        }
        
        guard let winningOperation = resolvedOrder.first else { return }
        
        if winningOperation.id == operation.id {
            // This operation wins, apply it
            applyOperationLocally(operation)
            acknowledgeOperation(operation)
        } else {
            // Another operation wins
            logger.info("Operation \(operation.id) lost conflict resolution")
        }
        
        // Clean up conflicting pending operations
        for op in conflictingOps {
            if op.id != winningOperation.id {
                pendingOperations.removeValue(forKey: op.id)
            }
        }
    }
    
    private func synchronizeWithPeers() {
        guard let network = networkInterface else { return }
        
        let syncRequest = SyncRequest(
            currentVersion: vectorClock.getTime(),
            lastOperationId: operationHistory.last?.id
        )
        
        guard let data = try? JSONEncoder().encode(syncRequest),
              let payload = String(data: data, encoding: .utf8) else {
            return
        }
        
        let syncMessage = SyncMessage(
            type: .syncRequest,
            senderId: nodeId,
            payload: payload,
            timestamp: Date().timeIntervalSince1970
        )
        
        network.broadcast(syncMessage)
    }
    
    private func broadcastOperation(_ operation: GameOperation) {
        guard let network = networkInterface else { return }
        
        guard let data = try? JSONEncoder().encode(operation),
              let payload = String(data: data, encoding: .utf8) else {
            logger.error("Failed to encode operation for broadcast")
            return
        }
        
        let message = SyncMessage(
            type: .gameOperation,
            senderId: nodeId,
            payload: payload,
            timestamp: Date().timeIntervalSince1970
        )
        
        network.broadcast(message)
    }
    
    private func acknowledgeOperation(_ operation: GameOperation) {
        guard var pending = pendingOperations[operation.id] else { return }
        
        pending.acknowledged.insert(nodeId)
        pendingOperations[operation.id] = pending
        
        // If majority acknowledged, consider confirmed
        let totalParticipants = participants.count + 1 // +1 for self
        if pending.acknowledged.count >= (totalParticipants + 1) / 2 {
            pendingOperations.removeValue(forKey: operation.id)
            logger.info("Operation \(operation.id) confirmed by majority")
        }
    }
    
    private func resolveConflicts() {
        let now = Date()
        let timedOutOperations = pendingOperations.values.filter {
            now.timeIntervalSince($0.timestamp) > conflictResolutionTimeout
        }
        
        for pending in timedOutOperations {
            logger.warning("Operation \(pending.operation.id) timed out, applying with current state")
            pendingOperations.removeValue(forKey: pending.operation.id)
        }
    }
    
    private func cleanupOldOperations() {
        let cutoff = Date().timeIntervalSince1970 - 60.0 // 1 minute
        operationHistory.removeAll { operation in
            Double(operation.data["timestamp"] ?? "0") ?? 0 < cutoff
        }
    }
    
    private func handleSyncRequest(_ message: SyncMessage) {
        // TODO: Implement sync request handling
        logger.debug("Received sync request from \(message.senderId)")
    }
    
    private func handleSyncResponse(_ message: SyncMessage) {
        // TODO: Implement sync response handling
        logger.debug("Received sync response from \(message.senderId)")
    }
    
    private func handleHeartbeat(_ message: SyncMessage) {
        let participantInfo = ParticipantInfo(
            nodeId: message.senderId,
            lastSeen: Date().timeIntervalSince1970,
            latency: estimateLatency(messageTimestamp: message.timestamp)
        )
        
        DispatchQueue.main.async {
            self.participants[message.senderId] = participantInfo
        }
    }
    
    private func handleConflictResolution(_ message: SyncMessage) {
        // TODO: Implement conflict resolution message handling
        logger.debug("Received conflict resolution from \(message.senderId)")
    }
    
    private func estimateLatency(messageTimestamp: TimeInterval) -> TimeInterval {
        let now = Date().timeIntervalSince1970
        let latency = now - messageTimestamp
        
        // Update running average
        networkLatencyEstimate = networkLatencyEstimate * 0.9 + latency * 0.1
        
        return latency
    }
    
    private func generateOperationId() -> String {
        return "\(nodeId)_\(Int(Date().timeIntervalSince1970 * 1000))_\(Int.random(in: 100...999))"
    }
    
    private func cleanup() {
        syncTimer?.invalidate()
        conflictResolutionTimer?.invalidate()
        cancellables.removeAll()
        pendingOperations.removeAll()
        operationHistory.removeAll()
    }
}

// MARK: - Data Models

struct GameStateSnapshot: Codable {
    let version: [String: Int64]
    let gamePhase: GamePhase
    let dice1: Int
    let dice2: Int
    let isRolling: Bool
    let point: Int?
    let players: [String: PlayerInfo]
    let playerBets: [String: Int]
    let totalPot: Int
    let lastRollTimestamp: TimeInterval
    let lastUpdateTimestamp: TimeInterval
    
    static func initial() -> GameStateSnapshot {
        return GameStateSnapshot(
            version: [:],
            gamePhase: .comeOut,
            dice1: 0,
            dice2: 0,
            isRolling: false,
            point: nil,
            players: [:],
            playerBets: [:],
            totalPot: 0,
            lastRollTimestamp: 0,
            lastUpdateTimestamp: Date().timeIntervalSince1970
        )
    }
}

struct GameOperation: Codable {
    let id: String
    let type: OperationType
    let nodeId: String
    let timestamp: [String: Int64]
    let data: [String: String]
}

enum OperationType: String, Codable {
    case diceRoll = "diceRoll"
    case placeBet = "placeBet"
    case playerJoin = "playerJoin"
    case playerLeave = "playerLeave"
    case gamePhaseChange = "gamePhaseChange"
}

enum GamePhase: String, Codable {
    case comeOut = "comeOut"
    case point = "point"
    case gameOver = "gameOver"
}

struct PlayerInfo: Codable {
    let id: String
    let name: String
    let balance: Int
    let isConnected: Bool
}

struct PendingOperation {
    let operation: GameOperation
    let timestamp: Date
    var acknowledged: Set<String>
}

struct ParticipantInfo {
    let nodeId: String
    let lastSeen: TimeInterval
    let latency: TimeInterval
}

enum ConnectionState {
    case disconnected
    case connecting
    case connected
    case error
}

// MARK: - Network Interface

protocol NetworkInterface {
    var messageHandler: ((SyncMessage) -> Void)? { get set }
    func broadcast(_ message: SyncMessage)
    func send(to nodeId: String, message: SyncMessage)
}

struct SyncMessage: Codable {
    let type: MessageType
    let senderId: String
    let payload: String
    let timestamp: TimeInterval
}

enum MessageType: String, Codable {
    case gameOperation = "gameOperation"
    case syncRequest = "syncRequest"
    case syncResponse = "syncResponse"
    case heartbeat = "heartbeat"
    case conflictResolution = "conflictResolution"
}

struct SyncRequest: Codable {
    let currentVersion: [String: Int64]
    let lastOperationId: String?
}

// MARK: - Vector Clock

class VectorClock {
    private let nodeId: String
    private var clock: [String: Int64] = [:]
    
    init(nodeId: String) {
        self.nodeId = nodeId
        clock[nodeId] = 0
    }
    
    func increment() -> [String: Int64] {
        clock[nodeId] = (clock[nodeId] ?? 0) + 1
        return clock
    }
    
    func update(with otherClock: [String: Int64]) {
        for (node, time) in otherClock {
            clock[node] = max(clock[node] ?? 0, time)
        }
        clock[nodeId] = (clock[nodeId] ?? 0) + 1
    }
    
    func getTime() -> [String: Int64] {
        return clock
    }
}

// MARK: - Extensions

extension Dictionary where Key == String, Value == Int64 {
    func happensBefore(_ other: [String: Int64]) -> Bool {
        var hasSmaller = false
        let allNodes = Set(self.keys).union(Set(other.keys))
        
        for node in allNodes {
            let thisTime = self[node] ?? 0
            let otherTime = other[node] ?? 0
            
            if thisTime > otherTime { return false }
            if thisTime < otherTime { hasSmaller = true }
        }
        
        return hasSmaller
    }
}