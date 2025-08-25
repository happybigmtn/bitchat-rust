import Foundation
import Combine

/**
 * Swift wrapper for BitCraps Rust library using UniFFI bindings
 * Provides iOS-friendly APIs with Combine publishers for reactive UI updates
 */
@MainActor
public class BitCrapsManager: ObservableObject {
    
    private var bitcrapsNode: BitCrapsNode?
    private var eventPollingTask: Task<Void, Never>?
    
    // Published properties for SwiftUI
    @Published public private(set) var nodeStatus: NodeStatus?
    @Published public private(set) var events: [GameEvent] = []
    @Published public private(set) var isInitialized = false
    @Published public private(set) var isDiscovering = false
    @Published public private(set) var connectedPeers: [PeerInfo] = []
    @Published public private(set) var networkStats: NetworkStats?
    @Published public private(set) var lastError: BitCrapsError?
    
    // Configuration
    private let config: BitCrapsConfig
    
    public init(dataDirectory: String? = nil) {
        // Create default configuration
        let documentsPath = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first?.path ?? ""
        let dataDir = dataDirectory ?? "\(documentsPath)/BitCraps"
        
        self.config = BitCrapsConfig(
            dataDir: dataDir,
            powDifficulty: 4,
            protocolVersion: 1,
            powerMode: .balanced,
            platformConfig: createIOSPlatformConfig(),
            enableLogging: true,
            logLevel: .info
        )
        
        createDirectory(at: dataDir)
    }
    
    deinit {
        Task { @MainActor in
            await shutdown()
        }
    }
    
    // MARK: - Public API
    
    /**
     * Initialize the BitCraps node
     */
    public func initialize() async throws {
        guard !isInitialized else { return }
        
        do {
            bitcrapsNode = try createNode(config: config)
            isInitialized = true
            await updateNodeStatus()
            print("BitCraps node initialized successfully")
        } catch {
            lastError = error as? BitCrapsError
            throw error
        }
    }
    
    /**
     * Start peer discovery
     */
    public func startDiscovery() async throws {
        guard let node = bitcrapsNode else {
            throw BitCrapsError.InitializationError(reason: "Node not initialized")
        }
        
        do {
            try await node.startDiscovery()
            isDiscovering = true
            startEventPolling()
            print("Started peer discovery")
        } catch {
            lastError = error as? BitCrapsError
            throw error
        }
    }
    
    /**
     * Stop peer discovery
     */
    public func stopDiscovery() async throws {
        guard let node = bitcrapsNode else {
            throw BitCrapsError.InitializationError(reason: "Node not initialized")
        }
        
        do {
            try await node.stopDiscovery()
            isDiscovering = false
            stopEventPolling()
            print("Stopped peer discovery")
        } catch {
            lastError = error as? BitCrapsError
            throw error
        }
    }
    
    /**
     * Create a new game
     */
    public func createGame(config: GameConfig = GameConfig.default) async throws -> GameHandle {
        guard let node = bitcrapsNode else {
            throw BitCrapsError.InitializationError(reason: "Node not initialized")
        }
        
        do {
            let gameHandle = try await node.createGame(config: config)
            print("Created game: \(gameHandle.getGameId())")
            return gameHandle
        } catch {
            lastError = error as? BitCrapsError
            throw error
        }
    }
    
    /**
     * Join an existing game
     */
    public func joinGame(gameId: String) async throws -> GameHandle {
        guard let node = bitcrapsNode else {
            throw BitCrapsError.InitializationError(reason: "Node not initialized")
        }
        
        do {
            let gameHandle = try await node.joinGame(gameId: gameId)
            print("Joined game: \(gameId)")
            return gameHandle
        } catch {
            lastError = error as? BitCrapsError
            throw error
        }
    }
    
    /**
     * Leave current game
     */
    public func leaveGame() async throws {
        guard let node = bitcrapsNode else {
            throw BitCrapsError.InitializationError(reason: "Node not initialized")
        }
        
        do {
            try await node.leaveGame()
            print("Left current game")
        } catch {
            lastError = error as? BitCrapsError
            throw error
        }
    }
    
    /**
     * Set power management mode
     */
    public func setPowerMode(_ mode: PowerMode) throws {
        guard let node = bitcrapsNode else {
            throw BitCrapsError.InitializationError(reason: "Node not initialized")
        }
        
        do {
            try node.setPowerMode(mode: mode)
            print("Set power mode to: \(mode)")
            Task { await updateNodeStatus() }
        } catch {
            lastError = error as? BitCrapsError
            throw error
        }
    }
    
    /**
     * Configure platform-specific settings
     */
    public func configurePlatform(_ config: PlatformConfig) throws {
        guard let node = bitcrapsNode else {
            throw BitCrapsError.InitializationError(reason: "Node not initialized")
        }
        
        do {
            try node.configureForPlatform(config: config)
            print("Configured platform settings")
        } catch {
            lastError = error as? BitCrapsError
            throw error
        }
    }
    
    /**
     * Refresh connected peers list
     */
    public func refreshPeers() {
        guard let node = bitcrapsNode else { return }
        
        Task {
            let peers = node.getConnectedPeers()
            await MainActor.run {
                self.connectedPeers = peers
            }
        }
    }
    
    /**
     * Refresh network statistics
     */
    public func refreshNetworkStats() {
        guard let node = bitcrapsNode else { return }
        
        Task {
            let stats = node.getNetworkStats()
            await MainActor.run {
                self.networkStats = stats
            }
        }
    }
    
    /**
     * Clear error state
     */
    public func clearError() {
        lastError = nil
    }
    
    // MARK: - Private Implementation
    
    private func createDirectory(at path: String) {
        do {
            try FileManager.default.createDirectory(
                atPath: path,
                withIntermediateDirectories: true,
                attributes: nil
            )
        } catch {
            print("Failed to create directory at \(path): \(error)")
        }
    }
    
    private func updateNodeStatus() async {
        guard let node = bitcrapsNode else { return }
        
        let status = node.getStatus()
        nodeStatus = status
    }
    
    private func startEventPolling() {
        eventPollingTask = Task { @MainActor in
            guard let node = bitcrapsNode else { return }
            
            while !Task.isCancelled && isDiscovering {
                do {
                    // Poll for new events
                    if let event = await node.pollEvent() {
                        events.append(event)
                        
                        // Limit event history
                        if events.count > 100 {
                            events.removeFirst()
                        }
                        
                        // Handle specific events
                        await handleEvent(event)
                    }
                    
                    // Update status and stats periodically
                    await updateNodeStatus()
                    refreshPeers()
                    refreshNetworkStats()
                    
                    try await Task.sleep(nanoseconds: 100_000_000) // 100ms
                } catch {
                    if !Task.isCancelled {
                        print("Event polling error: \(error)")
                        try? await Task.sleep(nanoseconds: 1_000_000_000) // 1 second on error
                    }
                }
            }
        }
    }
    
    private func stopEventPolling() {
        eventPollingTask?.cancel()
        eventPollingTask = nil
    }
    
    private func handleEvent(_ event: GameEvent) async {
        switch event {
        case .peerDiscovered(let peer):
            print("Discovered peer: \(peer.peerId)")
            refreshPeers()
            
        case .peerConnected(let peerId):
            print("Peer connected: \(peerId)")
            refreshPeers()
            
        case .peerDisconnected(let peerId):
            print("Peer disconnected: \(peerId)")
            refreshPeers()
            
        case .diceRolled(let roll):
            print("Dice rolled: \(roll.die1) + \(roll.die2) = \(roll.die1 + roll.die2)")
            
        case .errorOccurred(let error):
            print("Error occurred: \(error)")
            lastError = error
            
        case .batteryOptimizationDetected(let reason):
            print("Battery optimization detected: \(reason)")
            
        default:
            print("Received event: \(event)")
        }
    }
    
    private func shutdown() async {
        stopEventPolling()
        
        if isDiscovering {
            try? await stopDiscovery()
        }
        
        bitcrapsNode = nil
        isInitialized = false
    }
}

// MARK: - Configuration Extensions

extension BitCrapsManager {
    
    private func createIOSPlatformConfig() -> PlatformConfig {
        return PlatformConfig(
            platform: .iOS,
            backgroundScanning: false, // Limited on iOS
            scanWindowMs: 300,
            scanIntervalMs: 2000,
            lowPowerMode: true,
            serviceUuids: ["12345678-1234-5678-1234-567812345678"]
        )
    }
}

extension GameConfig {
    static let `default` = GameConfig(
        gameName: nil,
        minBet: 1,
        maxBet: 1000,
        maxPlayers: 8,
        timeoutSeconds: 300
    )
}

// MARK: - Convenience Extensions

extension PowerMode: CaseIterable {
    public static var allCases: [PowerMode] {
        return [.highPerformance, .balanced, .batterySaver, .ultraLowPower]
    }
    
    public var displayName: String {
        switch self {
        case .highPerformance: return "High Performance"
        case .balanced: return "Balanced"
        case .batterySaver: return "Battery Saver"
        case .ultraLowPower: return "Ultra Low Power"
        }
    }
}

extension NodeState {
    public var displayName: String {
        switch self {
        case .initializing: return "Initializing"
        case .ready: return "Ready"
        case .discovering: return "Discovering"
        case .connected: return "Connected"
        case .inGame: return "In Game"
        case .error(let reason): return "Error: \(reason)"
        }
    }
    
    public var isOperational: Bool {
        switch self {
        case .ready, .discovering, .connected, .inGame:
            return true
        case .initializing, .error:
            return false
        }
    }
}