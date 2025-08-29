import Foundation
import Combine
import CoreBluetooth
import LocalAuthentication

/**
 * BitCraps iOS SDK
 * 
 * Main entry point for integrating BitCraps peer-to-peer gaming functionality
 * into iOS applications.
 * 
 * Features:
 * - Bluetooth Low Energy peer discovery and communication
 * - Secure game state synchronization with Byzantine fault tolerance
 * - Built-in fraud detection and consensus mechanisms
 * - Battery optimization for background operation
 * - Biometric authentication integration
 * - SwiftUI-friendly reactive APIs
 * 
 * Usage:
 * ```swift
 * // Initialize the SDK
 * let bitCraps = try await BitCrapsSDK.initialize()
 * 
 * // Start discovering peers
 * try await bitCraps.startDiscovery()
 * 
 * // Observe events
 * bitCraps.events
 *     .sink { event in
 *         switch event {
 *         case .peerDiscovered(let peer):
 *             handlePeerDiscovered(peer)
 *         case .gameCreated(let gameId):
 *             handleGameCreated(gameId)
 *         // ... handle other events
 *         }
 *     }
 *     .store(in: &cancellables)
 * ```
 */
@MainActor
public final class BitCrapsSDK: ObservableObject {
    
    // MARK: - Published Properties
    
    /// Current node status
    @Published public private(set) var nodeStatus: NodeStatus?
    
    /// Connection status with peers
    @Published public private(set) var connectionStatus: ConnectionStatus = .disconnected
    
    /// List of discovered peers
    @Published public private(set) var discoveredPeers: [PeerInfo] = []
    
    /// Current game state (if in a game)
    @Published public private(set) var gameState: GameState?
    
    /// Network performance statistics
    @Published public private(set) var networkStats: NetworkStats = NetworkStats()
    
    /// Battery optimization status
    @Published public private(set) var batteryOptimizationStatus: BatteryOptimizationStatus = BatteryOptimizationStatus()
    
    /// Last error encountered
    @Published public private(set) var lastError: BitCrapsError?
    
    /// Whether SDK is initialized and ready
    @Published public private(set) var isInitialized: Boolean = false
    
    /// Whether peer discovery is active
    @Published public private(set) var isDiscovering: Boolean = false
    
    // MARK: - Combine Publishers
    
    /// Stream of game events
    public let events = PassthroughSubject<GameEvent, Never>()
    
    /// Stream of peer messages
    public let messages = PassthroughSubject<PeerMessage, Never>()
    
    /// Stream of diagnostic events
    public let diagnostics = PassthroughSubject<DiagnosticEvent, Never>()
    
    // MARK: - Private Properties
    
    private let coreManager: BitCrapsCoreManager
    private let bluetoothManager: BluetoothManager
    private let biometricManager: BiometricManager
    private let batteryManager: BatteryManager
    private let securityManager: SecurityManager
    
    private var eventPollingTimer: Timer?
    private var statsUpdateTimer: Timer?
    private var cancellables = Set<AnyCancellable>()
    
    // MARK: - Configuration
    
    private let config: SDKConfig
    
    // MARK: - Initialization
    
    private init(config: SDKConfig) {
        self.config = config
        self.coreManager = BitCrapsCoreManager(config: config)
        self.bluetoothManager = BluetoothManager(config: config.bluetoothConfig)
        self.biometricManager = BiometricManager()
        self.batteryManager = BatteryManager()
        self.securityManager = SecurityManager(config: config)
        
        setupEventHandling()
    }
    
    deinit {
        Task { @MainActor in
            await shutdown()
        }
    }
    
    // MARK: - Static Initialization
    
    private static var instance: BitCrapsSDK?
    
    /**
     * Initialize the BitCraps SDK
     * 
     * - Parameter config: SDK configuration options
     * - Returns: Initialized SDK instance
     * - Throws: BitCrapsError if initialization fails
     */
    public static func initialize(config: SDKConfig = .default) async throws -> BitCrapsSDK {
        guard instance == nil else {
            return instance!
        }
        
        let sdk = BitCrapsSDK(config: config)
        
        do {
            try await sdk.performInitialization()
            instance = sdk
            return sdk
        } catch {
            throw BitCrapsError.initializationFailed(
                reason: "SDK initialization failed",
                underlyingError: error
            )
        }
    }
    
    /**
     * Get current SDK instance
     * 
     * - Returns: Current SDK instance
     * - Throws: BitCrapsError if not initialized
     */
    public static func getInstance() throws -> BitCrapsSDK {
        guard let instance = instance else {
            throw BitCrapsError.initializationFailed(reason: "SDK not initialized")
        }
        return instance
    }
    
    /**
     * Check if SDK is initialized
     */
    public static var isInitialized: Bool {
        return instance != nil
    }
    
    // MARK: - Core Operations
    
    /**
     * Start peer discovery using Bluetooth Low Energy
     * 
     * - Parameter config: Optional discovery configuration
     * - Throws: BitCrapsError if discovery cannot be started
     */
    public func startDiscovery(config: DiscoveryConfig = .default) async throws {
        guard isInitialized else {
            throw BitCrapsError.initializationFailed(reason: "SDK not initialized")
        }
        
        do {
            try await bluetoothManager.startScanning(config: config)
            try await coreManager.startDiscovery(config: config)
            
            isDiscovering = true
            startEventPolling()
            
            events.send(.discoveryStarted)
            
        } catch {
            lastError = BitCrapsError.bluetoothError(
                reason: "Failed to start discovery",
                underlyingError: error
            )
            throw lastError!
        }
    }
    
    /**
     * Stop peer discovery
     */
    public func stopDiscovery() async throws {
        guard isInitialized else { return }
        
        do {
            await bluetoothManager.stopScanning()
            await coreManager.stopDiscovery()
            
            isDiscovering = false
            stopEventPolling()
            
            events.send(.discoveryStopped)
            
        } catch {
            lastError = BitCrapsError.bluetoothError(
                reason: "Failed to stop discovery",
                underlyingError: error
            )
            throw lastError!
        }
    }
    
    // MARK: - Game Operations
    
    /**
     * Create a new game
     * 
     * - Parameters:
     *   - gameType: Type of game to create
     *   - config: Game configuration
     * - Returns: Game session handle
     * - Throws: BitCrapsError if game creation fails
     */
    public func createGame(
        gameType: GameType = .craps,
        config: GameConfig = .default
    ) async throws -> GameSession {
        guard isInitialized else {
            throw BitCrapsError.initializationFailed(reason: "SDK not initialized")
        }
        
        do {
            let gameSession = try await coreManager.createGame(
                gameType: gameType,
                config: config
            )
            
            self.gameState = gameSession.currentState
            
            events.send(.gameCreated(gameId: gameSession.gameId, gameType: gameType))
            
            return gameSession
            
        } catch {
            let gameError = BitCrapsError.gameError(
                reason: "Failed to create game",
                gameId: nil,
                underlyingError: error
            )
            lastError = gameError
            throw gameError
        }
    }
    
    /**
     * Join an existing game
     * 
     * - Parameter gameId: Game identifier
     * - Returns: Game session handle
     * - Throws: BitCrapsError if joining fails
     */
    public func joinGame(gameId: String) async throws -> GameSession {
        guard isInitialized else {
            throw BitCrapsError.initializationFailed(reason: "SDK not initialized")
        }
        
        do {
            let gameSession = try await coreManager.joinGame(gameId: gameId)
            
            self.gameState = gameSession.currentState
            
            events.send(.gameJoined(gameId: gameId))
            
            return gameSession
            
        } catch {
            let gameError = BitCrapsError.gameError(
                reason: "Failed to join game",
                gameId: gameId,
                underlyingError: error
            )
            lastError = gameError
            throw gameError
        }
    }
    
    /**
     * Leave current game
     */
    public func leaveGame() async throws {
        guard let currentGame = gameState else { return }
        
        do {
            try await coreManager.leaveGame()
            
            let gameId = currentGame.gameId
            self.gameState = nil
            
            events.send(.gameLeft(gameId: gameId))
            
        } catch {
            let gameError = BitCrapsError.gameError(
                reason: "Failed to leave game",
                gameId: currentGame.gameId,
                underlyingError: error
            )
            lastError = gameError
            throw gameError
        }
    }
    
    /**
     * Get available games from discovered peers
     */
    public func getAvailableGames() async throws -> [AvailableGame] {
        guard isInitialized else {
            throw BitCrapsError.initializationFailed(reason: "SDK not initialized")
        }
        
        do {
            return try await coreManager.getAvailableGames()
        } catch {
            let networkError = BitCrapsError.networkError(
                reason: "Failed to get available games",
                underlyingError: error
            )
            lastError = networkError
            throw networkError
        }
    }
    
    // MARK: - Peer Operations
    
    /**
     * Connect to a specific peer
     * 
     * - Parameter peerId: Peer identifier
     */
    public func connectToPeer(peerId: String) async throws {
        guard isInitialized else {
            throw BitCrapsError.initializationFailed(reason: "SDK not initialized")
        }
        
        do {
            try await coreManager.connectToPeer(peerId: peerId)
            
            // Update peer connection status
            if let peerIndex = discoveredPeers.firstIndex(where: { $0.id == peerId }) {
                discoveredPeers[peerIndex].isConnected = true
                discoveredPeers[peerIndex].lastSeen = Date()
            }
            
            events.send(.peerConnected(peerId: peerId))
            
        } catch {
            let networkError = BitCrapsError.networkError(
                reason: "Failed to connect to peer",
                underlyingError: error
            )
            lastError = networkError
            throw networkError
        }
    }
    
    /**
     * Disconnect from a specific peer
     */
    public func disconnectFromPeer(peerId: String) async throws {
        guard isInitialized else {
            throw BitCrapsError.initializationFailed(reason: "SDK not initialized")
        }
        
        do {
            await coreManager.disconnectFromPeer(peerId: peerId)
            
            // Update peer connection status
            if let peerIndex = discoveredPeers.firstIndex(where: { $0.id == peerId }) {
                discoveredPeers[peerIndex].isConnected = false
            }
            
            events.send(.peerDisconnected(peerId: peerId, reason: "Manual disconnect"))
            
        } catch {
            // Disconnection failures are usually not critical
            diagnostics.send(.warning("Failed to cleanly disconnect from peer \(peerId)"))
        }
    }
    
    /**
     * Send message to a peer
     * 
     * - Parameters:
     *   - peerId: Target peer identifier
     *   - message: Message content
     */
    public func sendMessage(to peerId: String, message: String) async throws {
        guard isInitialized else {
            throw BitCrapsError.initializationFailed(reason: "SDK not initialized")
        }
        
        do {
            try await coreManager.sendMessage(to: peerId, message: message)
            
            events.send(.messageSent(to: peerId, message: message))
            
        } catch {
            let networkError = BitCrapsError.networkError(
                reason: "Failed to send message",
                underlyingError: error
            )
            lastError = networkError
            throw networkError
        }
    }
    
    // MARK: - Configuration Methods
    
    /**
     * Set power management mode
     */
    public func setPowerMode(_ powerMode: PowerMode) throws {
        guard isInitialized else {
            throw BitCrapsError.initializationFailed(reason: "SDK not initialized")
        }
        
        coreManager.setPowerMode(powerMode)
        bluetoothManager.setPowerMode(powerMode)
        
        nodeStatus?.currentPowerMode = powerMode
        
        events.send(.powerModeChanged(powerMode: powerMode))
    }
    
    /**
     * Configure Bluetooth settings
     */
    public func configureBluetoothSettings(_ bluetoothConfig: BluetoothConfig) throws {
        guard isInitialized else {
            throw BitCrapsError.initializationFailed(reason: "SDK not initialized")
        }
        
        bluetoothManager.updateConfiguration(bluetoothConfig)
        
        events.send(.bluetoothConfigurationChanged(config: bluetoothConfig))
    }
    
    /**
     * Enable or disable biometric authentication
     */
    public func setBiometricAuthentication(enabled: Boolean) async throws {
        guard isInitialized else {
            throw BitCrapsError.initializationFailed(reason: "SDK not initialized")
        }
        
        if enabled {
            let isAvailable = await biometricManager.isBiometricAuthenticationAvailable()
            guard isAvailable else {
                throw BitCrapsError.securityError(
                    reason: "Biometric authentication not available on this device"
                )
            }
            
            let isAuthenticated = try await biometricManager.authenticateUser(
                reason: "Enable biometric authentication for BitCraps"
            )
            
            guard isAuthenticated else {
                throw BitCrapsError.securityError(
                    reason: "Biometric authentication failed"
                )
            }
        }
        
        try await securityManager.setBiometricAuthenticationEnabled(enabled)
        
        events.send(.biometricAuthenticationChanged(enabled: enabled))
    }
    
    // MARK: - Diagnostics and Monitoring
    
    /**
     * Run comprehensive network diagnostics
     */
    public func runNetworkDiagnostics() async throws -> DiagnosticsReport {
        guard isInitialized else {
            throw BitCrapsError.initializationFailed(reason: "SDK not initialized")
        }
        
        do {
            let report = try await coreManager.runNetworkDiagnostics()
            
            diagnostics.send(.diagnosticsCompleted(report: report))
            
            return report
            
        } catch {
            let diagnosticError = BitCrapsError.networkError(
                reason: "Failed to run network diagnostics",
                underlyingError: error
            )
            lastError = diagnosticError
            throw diagnosticError
        }
    }
    
    /**
     * Get current performance metrics
     */
    public func getPerformanceMetrics() async -> PerformanceMetrics {
        let systemMetrics = await batteryManager.getSystemMetrics()
        let networkMetrics = await coreManager.getNetworkMetrics()
        let bluetoothMetrics = await bluetoothManager.getPerformanceMetrics()
        
        return PerformanceMetrics(
            cpuUsagePercent: systemMetrics.cpuUsage,
            memoryUsageMB: systemMetrics.memoryUsage,
            batteryLevel: systemMetrics.batteryLevel,
            bluetoothLatency: bluetoothMetrics.averageLatency,
            gameStateUpdateLatency: networkMetrics.gameStateLatency,
            consensusLatency: networkMetrics.consensusLatency,
            networkThroughput: networkMetrics.throughputKbps,
            frameRate: systemMetrics.frameRate
        )
    }
    
    /**
     * Enable debug logging
     */
    public func setDebugLogging(enabled: Boolean, logLevel: LogLevel = .info) {
        coreManager.setDebugLogging(enabled: enabled, logLevel: logLevel)
        bluetoothManager.setDebugLogging(enabled: enabled, logLevel: logLevel)
        
        if enabled {
            diagnostics.send(.info("Debug logging enabled at level: \(logLevel)"))
        } else {
            diagnostics.send(.info("Debug logging disabled"))
        }
    }
    
    // MARK: - Data Management
    
    /**
     * Export game history for backup
     */
    public func exportGameHistory() async throws -> Data {
        guard isInitialized else {
            throw BitCrapsError.initializationFailed(reason: "SDK not initialized")
        }
        
        do {
            return try await coreManager.exportGameHistory()
        } catch {
            let exportError = BitCrapsError.securityError(
                reason: "Failed to export game history",
                underlyingError: error
            )
            lastError = exportError
            throw exportError
        }
    }
    
    /**
     * Import game history from backup
     */
    public func importGameHistory(_ data: Data) async throws {
        guard isInitialized else {
            throw BitCrapsError.initializationFailed(reason: "SDK not initialized")
        }
        
        do {
            try await coreManager.importGameHistory(data)
            
            events.send(.gameHistoryImported)
            
        } catch {
            let importError = BitCrapsError.securityError(
                reason: "Failed to import game history",
                underlyingError: error
            )
            lastError = importError
            throw importError
        }
    }
    
    /**
     * Reset node and clear all data
     */
    public func resetNode() async throws {
        guard isInitialized else {
            throw BitCrapsError.initializationFailed(reason: "SDK not initialized")
        }
        
        // Stop all operations
        if isDiscovering {
            try await stopDiscovery()
        }
        
        if gameState != nil {
            try await leaveGame()
        }
        
        // Clear all data
        try await coreManager.resetNode()
        try await securityManager.clearAllData()
        
        // Reset state
        discoveredPeers.removeAll()
        gameState = nil
        networkStats = NetworkStats()
        batteryOptimizationStatus = BatteryOptimizationStatus()
        lastError = nil
        
        events.send(.nodeReset)
    }
    
    // MARK: - Lifecycle Management
    
    /**
     * Handle app entering background
     */
    public func applicationDidEnterBackground() {
        coreManager.applicationDidEnterBackground()
        bluetoothManager.applicationDidEnterBackground()
        
        // Reduce event polling frequency
        stopEventPolling()
        startEventPolling(interval: config.backgroundEventPollingInterval)
        
        diagnostics.send(.info("Application entered background"))
    }
    
    /**
     * Handle app entering foreground
     */
    public func applicationWillEnterForeground() {
        coreManager.applicationWillEnterForeground()
        bluetoothManager.applicationWillEnterForeground()
        
        // Resume normal event polling
        stopEventPolling()
        startEventPolling(interval: config.foregroundEventPollingInterval)
        
        // Refresh all state
        Task {
            await refreshAllState()
        }
        
        diagnostics.send(.info("Application entered foreground"))
    }
    
    /**
     * Shutdown SDK and clean up resources
     */
    public func shutdown() async {
        // Stop all operations
        if isDiscovering {
            try? await stopDiscovery()
        }
        
        if gameState != nil {
            try? await leaveGame()
        }
        
        // Stop timers
        stopEventPolling()
        stopStatsUpdates()
        
        // Shutdown managers
        await coreManager.shutdown()
        await bluetoothManager.shutdown()
        await batteryManager.shutdown()
        await securityManager.shutdown()
        
        // Clear publishers
        cancellables.removeAll()
        
        // Reset state
        isInitialized = false
        Self.instance = nil
        
        diagnostics.send(.info("SDK shutdown completed"))
    }
    
    // MARK: - Error Handling
    
    /**
     * Clear last error
     */
    public func clearError() {
        lastError = nil
    }
    
    // MARK: - Private Implementation
    
    private func performInitialization() async throws {
        // Initialize core components
        try await coreManager.initialize()
        try await bluetoothManager.initialize()
        try await biometricManager.initialize()
        try await batteryManager.initialize()
        try await securityManager.initialize()
        
        // Set initial node status
        nodeStatus = await coreManager.getNodeStatus()
        
        // Start periodic stats updates
        startStatsUpdates()
        
        isInitialized = true
        
        events.send(.sdkInitialized)
    }
    
    private func setupEventHandling() {
        // Handle core manager events
        coreManager.events
            .receive(on: DispatchQueue.main)
            .sink { [weak self] event in
                self?.handleCoreEvent(event)
            }
            .store(in: &cancellables)
        
        // Handle Bluetooth events
        bluetoothManager.events
            .receive(on: DispatchQueue.main)
            .sink { [weak self] event in
                self?.handleBluetoothEvent(event)
            }
            .store(in: &cancellables)
        
        // Handle security events
        securityManager.events
            .receive(on: DispatchQueue.main)
            .sink { [weak self] event in
                self?.handleSecurityEvent(event)
            }
            .store(in: &cancellables)
    }
    
    private func handleCoreEvent(_ event: CoreEvent) {
        switch event {
        case .peerDiscovered(let peer):
            if !discoveredPeers.contains(where: { $0.id == peer.id }) {
                discoveredPeers.append(peer)
            }
            events.send(.peerDiscovered(peer: peer))
            
        case .peerConnected(let peerId):
            if let peerIndex = discoveredPeers.firstIndex(where: { $0.id == peerId }) {
                discoveredPeers[peerIndex].isConnected = true
                discoveredPeers[peerIndex].lastSeen = Date()
            }
            updateConnectionStatus()
            events.send(.peerConnected(peerId: peerId))
            
        case .peerDisconnected(let peerId, let reason):
            if let peerIndex = discoveredPeers.firstIndex(where: { $0.id == peerId }) {
                discoveredPeers[peerIndex].isConnected = false
            }
            updateConnectionStatus()
            events.send(.peerDisconnected(peerId: peerId, reason: reason))
            
        case .gameStateUpdated(let newState):
            gameState = newState
            events.send(.gameStateChanged(gameId: newState.gameId, newState: newState))
            
        case .messageReceived(let peerId, let message):
            let peerMessage = PeerMessage(fromPeerId: peerId, content: message, timestamp: Date())
            messages.send(peerMessage)
            events.send(.messageReceived(from: peerId, message: message))
            
        case .errorOccurred(let error):
            lastError = error
            events.send(.errorOccurred(error: error))
        }
    }
    
    private func handleBluetoothEvent(_ event: BluetoothEvent) {
        switch event {
        case .stateChanged(let state):
            nodeStatus?.bluetoothState = state
            events.send(.bluetoothStateChanged(state: state))
            
        case .scanningStarted:
            diagnostics.send(.info("Bluetooth scanning started"))
            
        case .scanningStopped:
            diagnostics.send(.info("Bluetooth scanning stopped"))
            
        case .connectionQualityChanged(let peerId, let quality):
            if let peerIndex = discoveredPeers.firstIndex(where: { $0.id == peerId }) {
                discoveredPeers[peerIndex].connectionQuality = quality
            }
            
        case .batteryOptimizationDetected(let reason):
            batteryOptimizationStatus.isOptimizationActive = true
            batteryOptimizationStatus.reason = reason
            events.send(.batteryOptimizationDetected(reason: reason))
        }
    }
    
    private func handleSecurityEvent(_ event: SecurityEvent) {
        switch event {
        case .biometricAuthenticationSucceeded:
            events.send(.biometricAuthenticationSucceeded)
            
        case .biometricAuthenticationFailed(let reason):
            events.send(.biometricAuthenticationFailed(reason: reason))
            
        case .securityViolationDetected(let violation):
            lastError = BitCrapsError.securityError(reason: violation)
            events.send(.securityViolationDetected(violation: violation))
            
        case .dataExported:
            events.send(.gameHistoryExported)
            
        case .dataImported:
            events.send(.gameHistoryImported)
        }
    }
    
    private func startEventPolling(interval: TimeInterval = 0.1) {
        stopEventPolling()
        
        eventPollingTimer = Timer.scheduledTimer(withTimeInterval: interval, repeats: true) { [weak self] _ in
            Task { @MainActor in
                await self?.pollEvents()
            }
        }
    }
    
    private func stopEventPolling() {
        eventPollingTimer?.invalidate()
        eventPollingTimer = nil
    }
    
    private func startStatsUpdates() {
        statsUpdateTimer = Timer.scheduledTimer(withTimeInterval: 5.0, repeats: true) { [weak self] _ in
            Task { @MainActor in
                await self?.updateStats()
            }
        }
    }
    
    private func stopStatsUpdates() {
        statsUpdateTimer?.invalidate()
        statsUpdateTimer = nil
    }
    
    private func pollEvents() async {
        // Poll for new events from core manager
        await coreManager.pollEvents()
        
        // Update node status
        nodeStatus = await coreManager.getNodeStatus()
        
        // Clean up old peers
        cleanupOldPeers()
    }
    
    private func updateStats() async {
        networkStats = await coreManager.getNetworkStats()
        batteryOptimizationStatus = await batteryManager.getBatteryOptimizationStatus()
    }
    
    private func cleanupOldPeers() {
        let now = Date()
        let timeout: TimeInterval = 60.0 // 60 seconds
        
        discoveredPeers.removeAll { peer in
            !peer.isConnected && now.timeIntervalSince(peer.lastSeen) > timeout
        }
    }
    
    private func updateConnectionStatus() {
        let connectedPeers = discoveredPeers.filter { $0.isConnected }
        
        connectionStatus = ConnectionStatus(
            isConnected: !connectedPeers.isEmpty,
            connectedPeerCount: connectedPeers.count,
            totalDiscoveredPeers: discoveredPeers.count,
            connectionQuality: calculateOverallConnectionQuality(),
            lastConnectionTime: connectedPeers.first?.lastSeen
        )
    }
    
    private func calculateOverallConnectionQuality() -> ConnectionQuality {
        let connectedPeers = discoveredPeers.filter { $0.isConnected }
        
        guard !connectedPeers.isEmpty else { return .disconnected }
        
        let qualityScores = connectedPeers.compactMap { $0.connectionQuality?.rawValue }
        let averageScore = qualityScores.reduce(0, +) / qualityScores.count
        
        return ConnectionQuality(rawValue: averageScore) ?? .fair
    }
    
    private func refreshAllState() async {
        nodeStatus = await coreManager.getNodeStatus()
        networkStats = await coreManager.getNetworkStats()
        batteryOptimizationStatus = await batteryManager.getBatteryOptimizationStatus()
        
        if let currentGameId = gameState?.gameId {
            gameState = await coreManager.getGameState(gameId: currentGameId)
        }
        
        updateConnectionStatus()
    }
}

// MARK: - SDK Version Info

extension BitCrapsSDK {
    /// SDK version number
    public static let version = "1.0.0"
    
    /// SDK build date
    public static let buildDate = "2024-12-19"
    
    /// Minimum iOS version supported
    public static let minimumIOSVersion = "14.0"
    
    /// Minimum macOS version supported (for Mac Catalyst)
    public static let minimumMacOSVersion = "11.0"
    
    /// SDK build configuration
    public static let buildConfiguration: String = {
        #if DEBUG
        return "Debug"
        #else
        return "Release"
        #endif
    }()
}