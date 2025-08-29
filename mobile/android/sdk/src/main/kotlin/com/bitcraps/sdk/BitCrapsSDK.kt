package com.bitcraps.sdk

import android.content.Context
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.Flow
import com.bitcraps.sdk.core.BitCrapsManager
import com.bitcraps.sdk.models.*
import com.bitcraps.sdk.exceptions.BitCrapsException

/**
 * Main entry point for the BitCraps Android SDK
 * 
 * This SDK provides a high-level interface for integrating BitCraps
 * peer-to-peer gaming functionality into Android applications.
 * 
 * Features:
 * - Bluetooth Low Energy peer discovery
 * - Secure game state synchronization
 * - Built-in fraud detection
 * - Battery optimization
 * - Biometric authentication
 * 
 * Usage:
 * ```kotlin
 * val bitCraps = BitCrapsSDK.initialize(context)
 * 
 * // Start discovery
 * bitCraps.startDiscovery()
 * 
 * // Listen for events
 * bitCraps.events.collect { event ->
 *     when (event) {
 *         is GameEvent.PeerDiscovered -> handlePeerDiscovered(event)
 *         is GameEvent.GameCreated -> handleGameCreated(event)
 *         // ... handle other events
 *     }
 * }
 * ```
 */
class BitCrapsSDK private constructor(
    private val manager: BitCrapsManager
) {
    
    // MARK: - State Properties
    
    /**
     * Current node status
     */
    val nodeStatus: StateFlow<NodeStatus?> = manager.nodeStatus
    
    /**
     * Stream of game events
     */
    val events: Flow<GameEvent> = manager.events
    
    /**
     * Current connection status
     */
    val connectionStatus: StateFlow<ConnectionStatus> = manager.connectionStatus
    
    /**
     * List of discovered peers
     */
    val discoveredPeers: StateFlow<List<PeerInfo>> = manager.discoveredPeers
    
    /**
     * Current game state (if in a game)
     */
    val gameState: StateFlow<GameState?> = manager.gameState
    
    /**
     * Network statistics
     */
    val networkStats: StateFlow<NetworkStats> = manager.networkStats
    
    /**
     * Battery optimization status
     */
    val batteryOptimizationStatus: StateFlow<BatteryOptimizationStatus> = manager.batteryOptimizationStatus
    
    // MARK: - Discovery Operations
    
    /**
     * Start peer discovery using Bluetooth Low Energy
     * 
     * @param config Optional discovery configuration
     * @throws BitCrapsException if discovery cannot be started
     */
    suspend fun startDiscovery(config: DiscoveryConfig = DiscoveryConfig.default()) {
        manager.startDiscovery(config)
    }
    
    /**
     * Stop peer discovery
     * 
     * @throws BitCrapsException if discovery cannot be stopped
     */
    suspend fun stopDiscovery() {
        manager.stopDiscovery()
    }
    
    /**
     * Check if discovery is currently active
     */
    val isDiscovering: Boolean get() = manager.isDiscovering
    
    // MARK: - Game Operations
    
    /**
     * Create a new game session
     * 
     * @param gameType Type of game to create (default: Craps)
     * @param config Game configuration parameters
     * @return GameSession handle for managing the game
     * @throws BitCrapsException if game creation fails
     */
    suspend fun createGame(
        gameType: GameType = GameType.Craps,
        config: GameConfig = GameConfig.default()
    ): GameSession {
        return manager.createGame(gameType, config)
    }
    
    /**
     * Join an existing game by ID
     * 
     * @param gameId Unique identifier of the game to join
     * @return GameSession handle for the joined game
     * @throws BitCrapsException if joining fails
     */
    suspend fun joinGame(gameId: String): GameSession {
        return manager.joinGame(gameId)
    }
    
    /**
     * Leave the current game
     * 
     * @throws BitCrapsException if leave operation fails
     */
    suspend fun leaveGame() {
        manager.leaveGame()
    }
    
    /**
     * Get list of available games from discovered peers
     */
    suspend fun getAvailableGames(): List<AvailableGame> {
        return manager.getAvailableGames()
    }
    
    // MARK: - Peer Operations
    
    /**
     * Connect to a specific peer
     * 
     * @param peerId Identifier of the peer to connect to
     * @throws BitCrapsException if connection fails
     */
    suspend fun connectToPeer(peerId: String) {
        manager.connectToPeer(peerId)
    }
    
    /**
     * Disconnect from a specific peer
     * 
     * @param peerId Identifier of the peer to disconnect from
     */
    suspend fun disconnectFromPeer(peerId: String) {
        manager.disconnectFromPeer(peerId)
    }
    
    /**
     * Send a direct message to a peer
     * 
     * @param peerId Target peer identifier
     * @param message Message to send
     * @throws BitCrapsException if sending fails
     */
    suspend fun sendMessage(peerId: String, message: String) {
        manager.sendMessage(peerId, message)
    }
    
    // MARK: - Configuration
    
    /**
     * Update power management settings
     * 
     * @param powerMode New power mode to apply
     */
    suspend fun setPowerMode(powerMode: PowerMode) {
        manager.setPowerMode(powerMode)
    }
    
    /**
     * Configure Bluetooth settings
     * 
     * @param bluetoothConfig New Bluetooth configuration
     */
    suspend fun configureBluetoothSettings(bluetoothConfig: BluetoothConfig) {
        manager.configureBluetoothSettings(bluetoothConfig)
    }
    
    /**
     * Enable or disable biometric authentication
     * 
     * @param enabled Whether to enable biometric auth
     * @throws BitCrapsException if biometric setup fails
     */
    suspend fun setBiometricAuthenticationEnabled(enabled: Boolean) {
        manager.setBiometricAuthenticationEnabled(enabled)
    }
    
    // MARK: - Security & Privacy
    
    /**
     * Clear all cached data and reset the node
     * 
     * @throws BitCrapsException if reset fails
     */
    suspend fun resetNode() {
        manager.resetNode()
    }
    
    /**
     * Export game history for backup purposes
     * 
     * @return Encrypted backup data
     */
    suspend fun exportGameHistory(): ByteArray {
        return manager.exportGameHistory()
    }
    
    /**
     * Import game history from backup
     * 
     * @param backupData Previously exported backup data
     * @throws BitCrapsException if import fails
     */
    suspend fun importGameHistory(backupData: ByteArray) {
        manager.importGameHistory(backupData)
    }
    
    // MARK: - Monitoring & Diagnostics
    
    /**
     * Run network diagnostics
     * 
     * @return Diagnostic report
     */
    suspend fun runNetworkDiagnostics(): DiagnosticsReport {
        return manager.runNetworkDiagnostics()
    }
    
    /**
     * Get detailed performance metrics
     */
    suspend fun getPerformanceMetrics(): PerformanceMetrics {
        return manager.getPerformanceMetrics()
    }
    
    /**
     * Enable debug logging
     * 
     * @param enabled Whether to enable debug logging
     * @param logLevel Minimum log level to output
     */
    fun setDebugLogging(enabled: Boolean, logLevel: LogLevel = LogLevel.INFO) {
        manager.setDebugLogging(enabled, logLevel)
    }
    
    // MARK: - Lifecycle Management
    
    /**
     * Shutdown the BitCraps SDK and clean up resources
     * 
     * Call this when your application is being destroyed
     */
    suspend fun shutdown() {
        manager.shutdown()
    }
    
    // MARK: - Companion Object
    
    companion object {
        private var instance: BitCrapsSDK? = null
        
        /**
         * Initialize the BitCraps SDK
         * 
         * @param context Android application context
         * @param config Optional SDK configuration
         * @return BitCrapsSDK instance
         * @throws BitCrapsException if initialization fails
         */
        suspend fun initialize(
            context: Context,
            config: SDKConfig = SDKConfig.default()
        ): BitCrapsSDK {
            if (instance == null) {
                val manager = BitCrapsManager.initialize(context, config)
                instance = BitCrapsSDK(manager)
            }
            return instance!!
        }
        
        /**
         * Get the current SDK instance (must call initialize first)
         * 
         * @return Current SDK instance
         * @throws IllegalStateException if not initialized
         */
        fun getInstance(): BitCrapsSDK {
            return instance ?: throw IllegalStateException("BitCrapsSDK not initialized. Call initialize() first.")
        }
        
        /**
         * Check if SDK is initialized
         */
        val isInitialized: Boolean get() = instance != null
        
        /**
         * SDK version information
         */
        const val VERSION = "1.0.0"
        const val BUILD_DATE = "2024-12-19"
        const val MIN_ANDROID_SDK = 21
        const val TARGET_ANDROID_SDK = 34
    }
}