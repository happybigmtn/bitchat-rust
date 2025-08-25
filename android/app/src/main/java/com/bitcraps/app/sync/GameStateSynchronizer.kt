package com.bitcraps.app.sync

import android.util.Log
import com.google.gson.Gson
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.*
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.atomic.AtomicLong
import kotlin.random.Random

/**
 * Real-time game state synchronization for cross-platform BitCraps gaming
 */
class GameStateSynchronizer(
    private val nodeId: String = generateNodeId()
) {
    companion object {
        private const val TAG = "GameStateSynchronizer"
        private const val SYNC_INTERVAL_MS = 100L
        private const val CONFLICT_RESOLUTION_TIMEOUT_MS = 5000L
        private const val MAX_NETWORK_LATENCY_MS = 2000L
        
        private fun generateNodeId(): String = "node_${Random.nextInt(10000)}"
    }
    
    private val gson = Gson()
    private val scope = CoroutineScope(Dispatchers.IO + SupervisorJob())
    
    // State management
    private val _gameState = MutableStateFlow(GameStateSnapshot.initial())
    val gameState: StateFlow<GameStateSnapshot> = _gameState.asStateFlow()
    
    private val _connectionState = MutableStateFlow(ConnectionState.DISCONNECTED)
    val connectionState: StateFlow<ConnectionState> = _connectionState.asStateFlow()
    
    private val _participants = MutableStateFlow<Map<String, ParticipantInfo>>(emptyMap())
    val participants: StateFlow<Map<String, ParticipantInfo>> = _participants.asStateFlow()
    
    // Synchronization internals
    private val vectorClock = VectorClock(nodeId)
    private val pendingOperations = ConcurrentHashMap<String, PendingOperation>()
    private val operationHistory = mutableListOf<GameOperation>()
    private val networkLatencyEstimate = AtomicLong(100L)
    
    // Network interface (to be implemented by actual transport layer)
    private var networkInterface: NetworkInterface? = null
    
    init {
        startSynchronizationLoop()
        startConflictResolutionLoop()
    }
    
    // Public API
    fun connect(networkInterface: NetworkInterface) {
        this.networkInterface = networkInterface
        networkInterface.setMessageHandler { message ->
            handleIncomingMessage(message)
        }
        _connectionState.value = ConnectionState.CONNECTED
        Log.d(TAG, "Connected to network with node ID: $nodeId")
    }
    
    fun disconnect() {
        networkInterface = null
        _connectionState.value = ConnectionState.DISCONNECTED
        _participants.value = emptyMap()
        Log.d(TAG, "Disconnected from network")
    }
    
    fun proposeGameOperation(operation: GameOperation) {
        Log.d(TAG, "Proposing operation: ${operation.type}")
        
        // Add vector clock timestamp
        val timestampedOperation = operation.copy(
            timestamp = vectorClock.increment(),
            nodeId = nodeId,
            id = generateOperationId()
        )
        
        // Apply optimistically for local responsiveness
        applyOperationLocally(timestampedOperation)
        
        // Add to pending operations for conflict resolution
        pendingOperations[timestampedOperation.id] = PendingOperation(
            operation = timestampedOperation,
            timestamp = System.currentTimeMillis(),
            acknowledged = setOf(nodeId)
        )
        
        // Broadcast to network
        broadcastOperation(timestampedOperation)
    }
    
    // Private implementation
    private fun startSynchronizationLoop() {
        scope.launch {
            while (isActive) {
                delay(SYNC_INTERVAL_MS)
                synchronizeWithPeers()
            }
        }
    }
    
    private fun startConflictResolutionLoop() {
        scope.launch {
            while (isActive) {
                delay(1000L)
                resolveConflicts()
                cleanupOldOperations()
            }
        }
    }
    
    private fun handleIncomingMessage(message: SyncMessage) {
        when (message.type) {
            MessageType.GAME_OPERATION -> handleGameOperation(message)
            MessageType.SYNC_REQUEST -> handleSyncRequest(message)
            MessageType.SYNC_RESPONSE -> handleSyncResponse(message)
            MessageType.HEARTBEAT -> handleHeartbeat(message)
            MessageType.CONFLICT_RESOLUTION -> handleConflictResolution(message)
        }
    }
    
    private fun handleGameOperation(message: SyncMessage) {
        val operation = gson.fromJson(message.payload, GameOperation::class.java)
        
        Log.d(TAG, "Received operation: ${operation.type} from ${operation.nodeId}")
        
        // Update vector clock
        vectorClock.update(operation.timestamp)
        
        // Check for conflicts
        val hasConflict = checkForConflicts(operation)
        
        if (hasConflict) {
            Log.w(TAG, "Conflict detected for operation: ${operation.id}")
            initiateConflictResolution(operation)
        } else {
            // Apply operation
            applyOperationLocally(operation)
            acknowledgeOperation(operation)
        }
    }
    
    private fun applyOperationLocally(operation: GameOperation) {
        val currentState = _gameState.value
        val newState = when (operation.type) {
            OperationType.DICE_ROLL -> {
                currentState.copy(
                    dice1 = operation.data["dice1"]?.toInt() ?: currentState.dice1,
                    dice2 = operation.data["dice2"]?.toInt() ?: currentState.dice2,
                    isRolling = operation.data["isRolling"]?.toBoolean() ?: false,
                    lastRollTimestamp = System.currentTimeMillis()
                )
            }
            OperationType.PLACE_BET -> {
                val playerId = operation.nodeId
                val amount = operation.data["amount"]?.toInt() ?: 0
                val newBets = currentState.playerBets.toMutableMap()
                newBets[playerId] = (newBets[playerId] ?: 0) + amount
                
                currentState.copy(
                    playerBets = newBets,
                    totalPot = currentState.totalPot + amount
                )
            }
            OperationType.PLAYER_JOIN -> {
                val playerId = operation.nodeId
                val playerName = operation.data["playerName"] ?: "Unknown"
                
                currentState.copy(
                    players = currentState.players + (playerId to PlayerInfo(
                        id = playerId,
                        name = playerName,
                        balance = 1000,
                        isConnected = true
                    ))
                )
            }
            OperationType.PLAYER_LEAVE -> {
                val playerId = operation.nodeId
                currentState.copy(
                    players = currentState.players - playerId,
                    playerBets = currentState.playerBets - playerId
                )
            }
            OperationType.GAME_PHASE_CHANGE -> {
                val newPhase = GamePhase.valueOf(operation.data["phase"] ?: "COME_OUT")
                val point = operation.data["point"]?.toIntOrNull()
                
                currentState.copy(
                    gamePhase = newPhase,
                    point = point
                )
            }
        }
        
        _gameState.value = newState.copy(
            version = vectorClock.getTime(),
            lastUpdateTimestamp = System.currentTimeMillis()
        )
        
        // Update operation history
        operationHistory.add(operation)
        if (operationHistory.size > 1000) {
            operationHistory.removeAt(0)
        }
        
        Log.d(TAG, "Applied operation: ${operation.type}, new state version: ${newState.version}")
    }
    
    private fun checkForConflicts(operation: GameOperation): Boolean {
        // Check if operation conflicts with pending operations
        return pendingOperations.values.any { pending ->
            pending.operation.timestamp.happensBefore(operation.timestamp) &&
            pending.operation.type == operation.type &&
            areOperationsConflicting(pending.operation, operation)
        }
    }
    
    private fun areOperationsConflicting(op1: GameOperation, op2: GameOperation): Boolean {
        return when {
            op1.type == OperationType.DICE_ROLL && op2.type == OperationType.DICE_ROLL -> {
                // Multiple simultaneous dice rolls conflict
                kotlin.math.abs((op1.data["timestamp"]?.toLong() ?: 0) - (op2.data["timestamp"]?.toLong() ?: 0)) < 1000
            }
            op1.type == OperationType.PLACE_BET && op2.type == OperationType.PLACE_BET -> {
                // Same player betting simultaneously
                op1.nodeId == op2.nodeId
            }
            else -> false
        }
    }
    
    private fun initiateConflictResolution(operation: GameOperation) {
        scope.launch {
            Log.d(TAG, "Initiating conflict resolution for operation: ${operation.id}")
            
            // Collect all conflicting operations
            val conflictingOps = pendingOperations.values
                .filter { areOperationsConflicting(it.operation, operation) }
                .map { it.operation }
                .plus(operation)
            
            // Resolve using deterministic ordering (timestamp + node ID)
            val resolvedOrder = conflictingOps.sortedWith(compareBy({ it.timestamp.toString() }, { it.nodeId }))
            val winningOperation = resolvedOrder.first()
            
            if (winningOperation.id == operation.id) {
                // This operation wins, apply it
                applyOperationLocally(operation)
                acknowledgeOperation(operation)
            } else {
                // Another operation wins, revert if necessary
                Log.d(TAG, "Operation ${operation.id} lost conflict resolution")
            }
            
            // Clean up conflicting pending operations
            conflictingOps.forEach { op ->
                if (op.id != winningOperation.id) {
                    pendingOperations.remove(op.id)
                }
            }
        }
    }
    
    private fun synchronizeWithPeers() {
        networkInterface?.let { network ->
            val syncMessage = SyncMessage(
                type = MessageType.SYNC_REQUEST,
                senderId = nodeId,
                payload = gson.toJson(SyncRequest(
                    currentVersion = vectorClock.getTime(),
                    lastOperationId = operationHistory.lastOrNull()?.id
                )),
                timestamp = System.currentTimeMillis()
            )
            
            network.broadcast(syncMessage)
        }
    }
    
    private fun broadcastOperation(operation: GameOperation) {
        networkInterface?.let { network ->
            val message = SyncMessage(
                type = MessageType.GAME_OPERATION,
                senderId = nodeId,
                payload = gson.toJson(operation),
                timestamp = System.currentTimeMillis()
            )
            
            network.broadcast(message)
        }
    }
    
    private fun acknowledgeOperation(operation: GameOperation) {
        pendingOperations[operation.id]?.let { pending ->
            val updatedPending = pending.copy(
                acknowledged = pending.acknowledged + nodeId
            )
            pendingOperations[operation.id] = updatedPending
            
            // If majority acknowledged, consider confirmed
            val totalParticipants = _participants.value.size + 1 // +1 for self
            if (updatedPending.acknowledged.size >= (totalParticipants + 1) / 2) {
                pendingOperations.remove(operation.id)
                Log.d(TAG, "Operation ${operation.id} confirmed by majority")
            }
        }
    }
    
    private fun resolveConflicts() {
        val now = System.currentTimeMillis()
        val timedOutOperations = pendingOperations.values.filter { 
            now - it.timestamp > CONFLICT_RESOLUTION_TIMEOUT_MS 
        }
        
        timedOutOperations.forEach { pending ->
            Log.w(TAG, "Operation ${pending.operation.id} timed out, applying with current state")
            pendingOperations.remove(pending.operation.id)
        }
    }
    
    private fun cleanupOldOperations() {
        // Keep only recent operations in history
        val cutoff = System.currentTimeMillis() - 60000L // 1 minute
        operationHistory.removeAll { it.data["timestamp"]?.toLong() ?: 0 < cutoff }
    }
    
    private fun handleSyncRequest(message: SyncMessage) {
        // TODO: Implement sync request handling
    }
    
    private fun handleSyncResponse(message: SyncMessage) {
        // TODO: Implement sync response handling
    }
    
    private fun handleHeartbeat(message: SyncMessage) {
        // Update participant info
        val participantInfo = ParticipantInfo(
            nodeId = message.senderId,
            lastSeen = System.currentTimeMillis(),
            latency = estimateLatency(message.timestamp)
        )
        
        _participants.value = _participants.value + (message.senderId to participantInfo)
    }
    
    private fun handleConflictResolution(message: SyncMessage) {
        // TODO: Implement conflict resolution message handling
    }
    
    private fun estimateLatency(messageTimestamp: Long): Long {
        val now = System.currentTimeMillis()
        val latency = now - messageTimestamp
        
        // Update running average
        val current = networkLatencyEstimate.get()
        val updated = (current * 0.9 + latency * 0.1).toLong()
        networkLatencyEstimate.set(updated)
        
        return latency
    }
    
    private fun generateOperationId(): String = "${nodeId}_${System.currentTimeMillis()}_${Random.nextInt(1000)}"
    
    fun cleanup() {
        scope.cancel()
        pendingOperations.clear()
        operationHistory.clear()
    }
}

// Data classes
data class GameStateSnapshot(
    val version: Map<String, Long>,
    val gamePhase: GamePhase,
    val dice1: Int,
    val dice2: Int,
    val isRolling: Boolean,
    val point: Int?,
    val players: Map<String, PlayerInfo>,
    val playerBets: Map<String, Int>,
    val totalPot: Int,
    val lastRollTimestamp: Long,
    val lastUpdateTimestamp: Long
) {
    companion object {
        fun initial() = GameStateSnapshot(
            version = emptyMap(),
            gamePhase = GamePhase.COME_OUT,
            dice1 = 0,
            dice2 = 0,
            isRolling = false,
            point = null,
            players = emptyMap(),
            playerBets = emptyMap(),
            totalPot = 0,
            lastRollTimestamp = 0L,
            lastUpdateTimestamp = System.currentTimeMillis()
        )
    }
}

data class GameOperation(
    val id: String,
    val type: OperationType,
    val nodeId: String,
    val timestamp: Map<String, Long>,
    val data: Map<String, String>
)

enum class OperationType {
    DICE_ROLL,
    PLACE_BET,
    PLAYER_JOIN,
    PLAYER_LEAVE,
    GAME_PHASE_CHANGE
}

enum class GamePhase {
    COME_OUT,
    POINT,
    GAME_OVER
}

data class PlayerInfo(
    val id: String,
    val name: String,
    val balance: Int,
    val isConnected: Boolean
)

data class PendingOperation(
    val operation: GameOperation,
    val timestamp: Long,
    val acknowledged: Set<String>
)

data class ParticipantInfo(
    val nodeId: String,
    val lastSeen: Long,
    val latency: Long
)

enum class ConnectionState {
    DISCONNECTED,
    CONNECTING,
    CONNECTED,
    ERROR
}

// Network interface (to be implemented by transport layer)
interface NetworkInterface {
    fun broadcast(message: SyncMessage)
    fun send(nodeId: String, message: SyncMessage)
    fun setMessageHandler(handler: (SyncMessage) -> Unit)
}

data class SyncMessage(
    val type: MessageType,
    val senderId: String,
    val payload: String,
    val timestamp: Long
)

enum class MessageType {
    GAME_OPERATION,
    SYNC_REQUEST,
    SYNC_RESPONSE,
    HEARTBEAT,
    CONFLICT_RESOLUTION
}

data class SyncRequest(
    val currentVersion: Map<String, Long>,
    val lastOperationId: String?
)

// Vector clock implementation for ordering events
class VectorClock(private val nodeId: String) {
    private val clock = mutableMapOf<String, Long>()
    
    init {
        clock[nodeId] = 0L
    }
    
    fun increment(): Map<String, Long> {
        clock[nodeId] = (clock[nodeId] ?: 0L) + 1L
        return clock.toMap()
    }
    
    fun update(otherClock: Map<String, Long>) {
        otherClock.forEach { (node, time) ->
            clock[node] = maxOf(clock[node] ?: 0L, time)
        }
        clock[nodeId] = (clock[nodeId] ?: 0L) + 1L
    }
    
    fun getTime(): Map<String, Long> = clock.toMap()
}

// Extension function for vector clock comparison
fun Map<String, Long>.happensBefore(other: Map<String, Long>): Boolean {
    var hasSmaller = false
    val allNodes = this.keys + other.keys
    
    for (node in allNodes) {
        val thisTime = this[node] ?: 0L
        val otherTime = other[node] ?: 0L
        
        if (thisTime > otherTime) return false
        if (thisTime < otherTime) hasSmaller = true
    }
    
    return hasSmaller
}