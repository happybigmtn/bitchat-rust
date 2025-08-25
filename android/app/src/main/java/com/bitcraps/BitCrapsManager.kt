package com.bitcraps

import android.content.Context
import android.os.Handler
import android.os.Looper
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import org.json.JSONObject
import timber.log.Timber
import java.io.File

/**
 * High-level manager for BitCraps functionality
 * Wraps the native JNI interface with Kotlin-friendly APIs
 */
class BitCrapsManager(private val context: Context) {
    
    private var nodeHandle: Long = 0
    private var eventPollingJob: Job? = null
    private val handler = Handler(Looper.getMainLooper())
    private val scope = CoroutineScope(Dispatchers.IO + SupervisorJob())
    
    // State flows for UI updates
    private val _nodeStatus = MutableStateFlow<NodeStatus?>(null)
    val nodeStatus: StateFlow<NodeStatus?> = _nodeStatus.asStateFlow()
    
    private val _events = MutableStateFlow<List<GameEvent>>(emptyList())
    val events: StateFlow<List<GameEvent>> = _events.asStateFlow()
    
    private val _isInitialized = MutableStateFlow(false)
    val isInitialized: StateFlow<Boolean> = _isInitialized.asStateFlow()
    
    private val _isDiscovering = MutableStateFlow(false)
    val isDiscovering: StateFlow<Boolean> = _isDiscovering.asStateFlow()
    
    init {
        initialize()
    }
    
    /**
     * Initialize the native BitCraps library
     */
    private fun initialize() {
        try {
            val success = BitCrapsNative.initialize()
            if (success) {
                _isInitialized.value = true
                Timber.i("BitCraps native library initialized successfully")
            } else {
                throw InitializationException("Failed to initialize native library")
            }
        } catch (e: Exception) {
            Timber.e(e, "Failed to initialize BitCraps library")
            throw InitializationException("Library initialization failed", e)
        }
    }
    
    /**
     * Create a new BitCraps node
     */
    fun createNode(
        powDifficulty: Int = 4,
        protocolVersion: Int = 1
    ) {
        if (!_isInitialized.value) {
            throw InitializationException("Library not initialized")
        }
        
        try {
            val dataDir = File(context.filesDir, "bitcraps").absolutePath
            
            nodeHandle = BitCrapsNative.createNode(
                dataDir,
                powDifficulty,
                protocolVersion
            )
            
            if (nodeHandle == 0L) {
                throw InitializationException("Failed to create node")
            }
            
            Timber.i("Created BitCraps node with handle: $nodeHandle")
            updateNodeStatus()
            
        } catch (e: Exception) {
            Timber.e(e, "Failed to create BitCraps node")
            throw InitializationException("Node creation failed", e)
        }
    }
    
    /**
     * Start peer discovery
     */
    fun startDiscovery() {
        ensureNodeCreated()
        
        try {
            val success = BitCrapsNative.startDiscovery(nodeHandle)
            if (success) {
                _isDiscovering.value = true
                startEventPolling()
                Timber.i("Started peer discovery")
            } else {
                throw BluetoothException("Failed to start discovery")
            }
        } catch (e: Exception) {
            Timber.e(e, "Failed to start discovery")
            throw BluetoothException("Discovery start failed", e)
        }
    }
    
    /**
     * Stop peer discovery
     */
    fun stopDiscovery() {
        ensureNodeCreated()
        
        try {
            val success = BitCrapsNative.stopDiscovery(nodeHandle)
            if (success) {
                _isDiscovering.value = false
                stopEventPolling()
                Timber.i("Stopped peer discovery")
            } else {
                throw BluetoothException("Failed to stop discovery")
            }
        } catch (e: Exception) {
            Timber.e(e, "Failed to stop discovery")
            throw BluetoothException("Discovery stop failed", e)
        }
    }
    
    /**
     * Set power management mode
     */
    fun setPowerMode(powerMode: PowerMode) {
        ensureNodeCreated()
        
        try {
            val modeInt = when (powerMode) {
                PowerMode.HIGH_PERFORMANCE -> BitCrapsNative.POWER_MODE_HIGH_PERFORMANCE
                PowerMode.BALANCED -> BitCrapsNative.POWER_MODE_BALANCED
                PowerMode.BATTERY_SAVER -> BitCrapsNative.POWER_MODE_BATTERY_SAVER
                PowerMode.ULTRA_LOW_POWER -> BitCrapsNative.POWER_MODE_ULTRA_LOW_POWER
            }
            
            val success = BitCrapsNative.setPowerMode(nodeHandle, modeInt)
            if (success) {
                Timber.i("Set power mode to: $powerMode")
                updateNodeStatus()
            } else {
                throw NetworkException("Failed to set power mode")
            }
        } catch (e: Exception) {
            Timber.e(e, "Failed to set power mode")
            throw NetworkException("Power mode change failed", e)
        }
    }
    
    /**
     * Update node status from native layer
     */
    private fun updateNodeStatus() {
        if (nodeHandle == 0L) return
        
        scope.launch {
            try {
                val statusJson = BitCrapsNative.getNodeStatus(nodeHandle)
                statusJson?.let { json ->
                    val status = parseNodeStatus(json)
                    _nodeStatus.value = status
                }
            } catch (e: Exception) {
                Timber.e(e, "Failed to update node status")
            }
        }
    }
    
    /**
     * Start polling for events from the native layer
     */
    private fun startEventPolling() {
        eventPollingJob?.cancel()
        eventPollingJob = scope.launch {
            while (isActive && _isDiscovering.value) {
                try {
                    val eventJson = BitCrapsNative.pollEvent(nodeHandle)
                    eventJson?.let { json ->
                        val event = parseGameEvent(json)
                        addEvent(event)
                    }
                    
                    delay(100) // Poll every 100ms
                } catch (e: Exception) {
                    Timber.e(e, "Error polling events")
                    delay(1000) // Wait longer on error
                }
            }
        }
    }
    
    /**
     * Stop event polling
     */
    private fun stopEventPolling() {
        eventPollingJob?.cancel()
        eventPollingJob = null
    }
    
    /**
     * Add a new event to the event list
     */
    private fun addEvent(event: GameEvent) {
        val currentEvents = _events.value.toMutableList()
        currentEvents.add(event)
        
        // Limit event history to last 100 events
        if (currentEvents.size > 100) {
            currentEvents.removeAt(0)
        }
        
        _events.value = currentEvents
        Timber.d("New event: ${event.type}")
    }
    
    /**
     * Parse JSON node status from native layer
     */
    private fun parseNodeStatus(json: String): NodeStatus {
        try {
            val jsonObject = JSONObject(json)
            return NodeStatus(
                state = jsonObject.getString("state"),
                bluetoothEnabled = jsonObject.getBoolean("bluetooth_enabled"),
                discoveryActive = jsonObject.getBoolean("discovery_active"),
                currentGameId = jsonObject.optString("current_game_id").takeIf { it.isNotEmpty() },
                activeConnections = jsonObject.getInt("active_connections"),
                currentPowerMode = jsonObject.getString("current_power_mode")
            )
        } catch (e: Exception) {
            Timber.e(e, "Failed to parse node status: $json")
            throw NetworkException("Status parsing failed", e)
        }
    }
    
    /**
     * Parse JSON game event from native layer
     */
    private fun parseGameEvent(json: String): GameEvent {
        try {
            val jsonObject = JSONObject(json)
            val type = jsonObject.getString("type")
            
            return when (type) {
                "PeerDiscovered" -> {
                    val peer = jsonObject.getJSONObject("peer")
                    GameEvent.PeerDiscovered(
                        peerId = peer.getString("peer_id"),
                        displayName = peer.optString("display_name").takeIf { it.isNotEmpty() },
                        signalStrength = peer.getInt("signal_strength"),
                        lastSeen = peer.getLong("last_seen"),
                        isConnected = peer.getBoolean("is_connected")
                    )
                }
                
                "PeerConnected" -> GameEvent.PeerConnected(
                    peerId = jsonObject.getString("peer_id")
                )
                
                "DiceRolled" -> {
                    val roll = jsonObject.getJSONObject("roll")
                    GameEvent.DiceRolled(
                        die1 = roll.getInt("die1"),
                        die2 = roll.getInt("die2"),
                        rollTime = roll.getLong("roll_time"),
                        rollerPeerId = roll.getString("roller_peer_id")
                    )
                }
                
                else -> GameEvent.Unknown(type, json)
            }
        } catch (e: Exception) {
            Timber.e(e, "Failed to parse game event: $json")
            return GameEvent.Unknown("ParseError", json)
        }
    }
    
    /**
     * Ensure a node has been created
     */
    private fun ensureNodeCreated() {
        if (!_isInitialized.value) {
            throw InitializationException("Library not initialized")
        }
        
        if (nodeHandle == 0L) {
            throw InitializationException("Node not created")
        }
    }
    
    /**
     * Clean up resources
     */
    fun destroy() {
        stopDiscovery()
        scope.cancel()
        
        if (nodeHandle != 0L) {
            BitCrapsNative.destroyNode(nodeHandle)
            nodeHandle = 0L
            Timber.i("Destroyed BitCraps node")
        }
    }
}

/**
 * Data classes for BitCraps state
 */
data class NodeStatus(
    val state: String,
    val bluetoothEnabled: Boolean,
    val discoveryActive: Boolean,
    val currentGameId: String?,
    val activeConnections: Int,
    val currentPowerMode: String
)

sealed class GameEvent {
    abstract val type: String
    abstract val timestamp: Long
    
    data class PeerDiscovered(
        val peerId: String,
        val displayName: String?,
        val signalStrength: Int,
        val lastSeen: Long,
        val isConnected: Boolean,
        override val timestamp: Long = System.currentTimeMillis()
    ) : GameEvent() {
        override val type = "PeerDiscovered"
    }
    
    data class PeerConnected(
        val peerId: String,
        override val timestamp: Long = System.currentTimeMillis()
    ) : GameEvent() {
        override val type = "PeerConnected"
    }
    
    data class DiceRolled(
        val die1: Int,
        val die2: Int,
        val rollTime: Long,
        val rollerPeerId: String,
        override val timestamp: Long = System.currentTimeMillis()
    ) : GameEvent() {
        override val type = "DiceRolled"
    }
    
    data class Unknown(
        override val type: String,
        val rawData: String,
        override val timestamp: Long = System.currentTimeMillis()
    ) : GameEvent()
}

enum class PowerMode {
    HIGH_PERFORMANCE,
    BALANCED,
    BATTERY_SAVER,
    ULTRA_LOW_POWER
}