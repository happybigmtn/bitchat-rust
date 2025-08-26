import Foundation
import CoreBluetooth
import SwiftUI
import os.log

/// Main coordinator that integrates all BitCraps iOS Bluetooth components
/// 
/// This class serves as the central coordinator for all Bluetooth operations,
/// integrating the bridge, error handler, lifecycle manager, and permission manager
/// into a cohesive system.
@available(iOS 13.0, *)
@MainActor
class BitCrapsBluetoothCoordinator: NSObject, ObservableObject {
    
    // MARK: - Component Dependencies
    
    private let bluetoothBridge: BitCrapsBluetoothBridge
    private let errorHandler: BitCrapsErrorHandler
    private let lifecycleManager: BitCrapsLifecycleManager
    private let permissionManager: BitCrapsPermissionManager
    
    // MARK: - Published State
    
    @Published var isInitialized: Bool = false
    @Published var currentState: String = "uninitialized"
    @Published var bluetoothEnabled: Bool = false
    @Published var permissionsGranted: Bool = false
    @Published var isAdvertising: Bool = false
    @Published var isScanning: Bool = false
    @Published var connectedPeers: [String] = []
    @Published var discoveredPeers: [String] = []
    @Published var currentError: String?
    @Published var degradedMode: Bool = false
    
    // MARK: - Private Properties
    
    private let logger = OSLog(subsystem: "com.bitcraps.ios", category: "BluetoothCoordinator")
    private var stateUpdateTimer: Timer?
    
    // MARK: - Initialization
    
    override init() {
        // Initialize all components
        self.bluetoothBridge = BitCrapsBluetoothBridge()
        self.errorHandler = BitCrapsErrorHandler()
        self.lifecycleManager = BitCrapsLifecycleManager()
        self.permissionManager = BitCrapsPermissionManager()
        
        super.init()
        
        setupComponentIntegration()
        setupStateMonitoring()
        setupNotificationObservers()
        
        os_log("BluetoothCoordinator initialized", log: logger, type: .info)
    }
    
    deinit {
        cleanup()
        NotificationCenter.default.removeObserver(self)
    }
    
    // MARK: - Public Interface
    
    /// Initialize the entire Bluetooth system
    func initialize() async -> Bool {
        os_log("Initializing Bluetooth system", log: logger, type: .info)
        
        do {
            // Step 1: Initialize lifecycle manager
            lifecycleManager.initialize(bluetoothBridge: bluetoothBridge, errorHandler: errorHandler)
            
            // Step 2: Request permissions
            let permissionsGranted = await requestPermissions()
            guard permissionsGranted else {
                os_log("Permission request failed", log: logger, type: .error)
                return false
            }
            
            // Step 3: Wait for Bluetooth to be ready
            let bluetoothReady = await waitForBluetoothReady()
            guard bluetoothReady else {
                os_log("Bluetooth not ready", log: logger, type: .error)
                return false
            }
            
            // Step 4: Initialize Rust FFI bridge
            guard bluetoothBridge.isInitialized else {
                os_log("Bluetooth bridge not initialized", log: logger, type: .error)
                return false
            }
            
            isInitialized = true
            currentState = "ready"
            
            os_log("Bluetooth system initialization completed successfully", log: logger, type: .info)
            return true
            
        } catch {
            os_log("Bluetooth system initialization failed: %@", log: logger, type: .error, error.localizedDescription)
            currentError = error.localizedDescription
            return false
        }
    }
    
    /// Start advertising for other BitCraps players
    func startAdvertising() async -> Bool {
        guard isInitialized else {
            os_log("Cannot start advertising - system not initialized", log: logger, type: .error)
            return false
        }
        
        guard lifecycleManager.isOperationAllowed("advertising") else {
            os_log("Cannot start advertising - operation not allowed in current state", log: logger, type: .error)
            return false
        }
        
        let success = bluetoothBridge.startAdvertising()
        if success {
            isAdvertising = true
            os_log("Advertising started successfully", log: logger, type: .info)
        } else {
            os_log("Failed to start advertising", log: logger, type: .error)
        }
        
        return success
    }
    
    /// Stop advertising
    func stopAdvertising() async -> Bool {
        guard isInitialized else { return false }
        
        let success = bluetoothBridge.stopAdvertising()
        if success {
            isAdvertising = false
            os_log("Advertising stopped successfully", log: logger, type: .info)
        }
        
        return success
    }
    
    /// Start scanning for other BitCraps players
    func startScanning() async -> Bool {
        guard isInitialized else {
            os_log("Cannot start scanning - system not initialized", log: logger, type: .error)
            return false
        }
        
        guard lifecycleManager.isOperationAllowed("scanning") else {
            os_log("Cannot start scanning - operation not allowed in current state", log: logger, type: .error)
            return false
        }
        
        let success = bluetoothBridge.startScanning()
        if success {
            isScanning = true
            os_log("Scanning started successfully", log: logger, type: .info)
        } else {
            os_log("Failed to start scanning", log: logger, type: .error)
        }
        
        return success
    }
    
    /// Stop scanning
    func stopScanning() async -> Bool {
        guard isInitialized else { return false }
        
        let success = bluetoothBridge.stopScanning()
        if success {
            isScanning = false
            os_log("Scanning stopped successfully", log: logger, type: .info)
        }
        
        return success
    }
    
    /// Connect to a discovered peer
    func connectToPeer(_ peerID: String) async -> Bool {
        guard isInitialized else {
            os_log("Cannot connect - system not initialized", log: logger, type: .error)
            return false
        }
        
        guard lifecycleManager.isOperationAllowed("connection") else {
            os_log("Cannot connect - operation not allowed in current state", log: logger, type: .error)
            return false
        }
        
        let success = bluetoothBridge.connectToPeripheral(withIdentifier: peerID)
        if success {
            os_log("Connection initiated to peer: %@", log: logger, type: .info, peerID)
        } else {
            os_log("Failed to initiate connection to peer: %@", log: logger, type: .error, peerID)
        }
        
        return success
    }
    
    /// Disconnect from a peer
    func disconnectFromPeer(_ peerID: String) async -> Bool {
        guard isInitialized else { return false }
        
        let success = bluetoothBridge.disconnectFromPeripheral(withIdentifier: peerID)
        if success {
            os_log("Disconnection initiated from peer: %@", log: logger, type: .info, peerID)
        }
        
        return success
    }
    
    /// Send data to a connected peer
    func sendData(_ data: Data, toPeer peerID: String) async -> Bool {
        guard isInitialized else {
            os_log("Cannot send data - system not initialized", log: logger, type: .error)
            return false
        }
        
        let success = bluetoothBridge.sendData(data, toPeripheral: peerID)
        if success {
            os_log("Data sent to peer: %@ (%d bytes)", log: logger, type: .debug, peerID, data.count)
        } else {
            os_log("Failed to send data to peer: %@", log: logger, type: .error, peerID)
        }
        
        return success
    }
    
    /// Get system status report
    func getSystemStatus() -> [String: Any] {
        return [
            "initialized": isInitialized,
            "currentState": currentState,
            "bluetoothEnabled": bluetoothEnabled,
            "permissionsGranted": permissionsGranted,
            "isAdvertising": isAdvertising,
            "isScanning": isScanning,
            "connectedPeers": connectedPeers.count,
            "discoveredPeers": discoveredPeers.count,
            "currentError": currentError as Any,
            "degradedMode": degradedMode,
            "lifecycle": lifecycleManager.getLifecycleStatistics(),
            "permissions": permissionManager.getBackgroundCapabilitiesReport(),
            "errors": errorHandler.getErrorStatistics()
        ]
    }
    
    /// Handle app state transitions
    func handleAppStateTransition(_ state: UIApplication.State) {
        os_log("Handling app state transition: %@", log: logger, type: .info, String(describing: state))
        
        lifecycleManager.handleAppStateTransition(state)
        updatePublishedState()
    }
    
    /// Open system settings for permission configuration
    func openSystemSettings() {
        permissionManager.openSystemSettings()
    }
    
    // MARK: - Private Methods
    
    private func setupComponentIntegration() {
        os_log("Setting up component integration", log: logger, type: .info)
        
        // The components are already integrated through notification system
        // Each component posts notifications that others can observe
    }
    
    private func setupStateMonitoring() {
        // Update published state every second
        stateUpdateTimer = Timer.scheduledTimer(withTimeInterval: 1.0, repeats: true) { _ in
            Task { @MainActor in
                self.updatePublishedState()
            }
        }
    }
    
    private func setupNotificationObservers() {
        // Bluetooth bridge notifications
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(handleBluetoothStateChange),
            name: Notification.Name("bluetoothStateChanged"),
            object: nil
        )
        
        // Error handler notifications
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(handleErrorStateChange),
            name: .bitCrapsFatalError,
            object: nil
        )
        
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(handleDegradedMode),
            name: .bitCrapsEnteredDegradedMode,
            object: nil
        )
        
        // Lifecycle manager notifications
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(handleLifecycleStateChange),
            name: .bitCrapsLifecycleStateChanged,
            object: nil
        )
        
        // Permission manager notifications
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(handlePermissionStateChange),
            name: Notification.Name("permissionStateChanged"),
            object: nil
        )
    }
    
    private func updatePublishedState() {
        // Update from lifecycle manager
        currentState = lifecycleManager.currentState.rawValue
        
        // Update from permission manager
        permissionsGranted = permissionManager.hasAllRequiredPermissions()
        bluetoothEnabled = permissionManager.bluetoothPermissionStatus.isAuthorized
        
        // Update from error handler
        if let error = errorHandler.currentError {
            currentError = error.localizedDescription
        } else {
            currentError = nil
        }
        degradedMode = errorHandler.degradedMode
        
        // Update from Bluetooth bridge
        isAdvertising = bluetoothBridge.isAdvertising
        isScanning = bluetoothBridge.isScanning
        connectedPeers = Array(bluetoothBridge.connectedPeripherals.keys)
        discoveredPeers = Array(bluetoothBridge.discoveredPeripherals.keys)
    }
    
    private func requestPermissions() async -> Bool {
        os_log("Requesting permissions", log: logger, type: .info)
        
        return await withCheckedContinuation { continuation in
            permissionManager.requestAllPermissions { granted in
                continuation.resume(returning: granted)
            }
        }
    }
    
    private func waitForBluetoothReady() async -> Bool {
        os_log("Waiting for Bluetooth to be ready", log: logger, type: .info)
        
        // Wait up to 10 seconds for Bluetooth to be ready
        for attempt in 1...20 {
            if bluetoothBridge.bluetoothState == .poweredOn {
                os_log("Bluetooth ready after %d attempts", log: logger, type: .info, attempt)
                return true
            }
            
            try? await Task.sleep(nanoseconds: 500_000_000) // 0.5 seconds
        }
        
        os_log("Bluetooth not ready after 10 seconds", log: logger, type: .error)
        return false
    }
    
    private func cleanup() {
        os_log("Cleaning up Bluetooth coordinator", log: logger, type: .info)
        
        stateUpdateTimer?.invalidate()
        stateUpdateTimer = nil
        
        // Components will clean up themselves in their deinit
    }
    
    // MARK: - Notification Handlers
    
    @objc private func handleBluetoothStateChange(_ notification: Notification) {
        Task { @MainActor in
            self.updatePublishedState()
        }
    }
    
    @objc private func handleErrorStateChange(_ notification: Notification) {
        Task { @MainActor in
            if let error = notification.object as? BitCrapsErrorHandler.BitCrapsBluetoothError {
                self.currentError = error.localizedDescription
                os_log("Error state changed: %@", log: self.logger, type: .error, error.localizedDescription)
            }
        }
    }
    
    @objc private func handleDegradedMode(_ notification: Notification) {
        Task { @MainActor in
            self.degradedMode = true
            os_log("Entered degraded mode", log: self.logger, type: .info)
        }
    }
    
    @objc private func handleLifecycleStateChange(_ notification: Notification) {
        Task { @MainActor in
            if let stateInfo = notification.object as? [String: Any],
               let newState = stateInfo["newState"] as? BitCrapsLifecycleManager.LifecycleState {
                self.currentState = newState.rawValue
                os_log("Lifecycle state changed: %@", log: self.logger, type: .info, newState.rawValue)
            }
        }
    }
    
    @objc private func handlePermissionStateChange(_ notification: Notification) {
        Task { @MainActor in
            self.updatePublishedState()
        }
    }
}

// MARK: - SwiftUI Integration

@available(iOS 13.0, *)
extension BitCrapsBluetoothCoordinator {
    
    /// Create a SwiftUI view model for the coordinator
    func createViewModel() -> BitCrapsBluetoothViewModel {
        return BitCrapsBluetoothViewModel(coordinator: self)
    }
}

/// SwiftUI view model for BitCraps Bluetooth operations
@available(iOS 13.0, *)
@MainActor
class BitCrapsBluetoothViewModel: ObservableObject {
    
    private let coordinator: BitCrapsBluetoothCoordinator
    
    // Published properties mirror the coordinator
    @Published var isInitialized: Bool = false
    @Published var currentState: String = "uninitialized"
    @Published var bluetoothEnabled: Bool = false
    @Published var permissionsGranted: Bool = false
    @Published var isAdvertising: Bool = false
    @Published var isScanning: Bool = false
    @Published var connectedPeersCount: Int = 0
    @Published var discoveredPeersCount: Int = 0
    @Published var currentError: String?
    @Published var degradedMode: Bool = false
    
    init(coordinator: BitCrapsBluetoothCoordinator) {
        self.coordinator = coordinator
        
        // Observe coordinator changes
        coordinator.objectWillChange.sink { [weak self] _ in
            DispatchQueue.main.async {
                self?.updateFromCoordinator()
            }
        }.store(in: &cancellables)
        
        updateFromCoordinator()
    }
    
    private var cancellables = Set<AnyCancellable>()
    
    private func updateFromCoordinator() {
        isInitialized = coordinator.isInitialized
        currentState = coordinator.currentState
        bluetoothEnabled = coordinator.bluetoothEnabled
        permissionsGranted = coordinator.permissionsGranted
        isAdvertising = coordinator.isAdvertising
        isScanning = coordinator.isScanning
        connectedPeersCount = coordinator.connectedPeers.count
        discoveredPeersCount = coordinator.discoveredPeers.count
        currentError = coordinator.currentError
        degradedMode = coordinator.degradedMode
    }
    
    // MARK: - Public Interface
    
    func initialize() async -> Bool {
        return await coordinator.initialize()
    }
    
    func startAdvertising() async -> Bool {
        return await coordinator.startAdvertising()
    }
    
    func stopAdvertising() async -> Bool {
        return await coordinator.stopAdvertising()
    }
    
    func startScanning() async -> Bool {
        return await coordinator.startScanning()
    }
    
    func stopScanning() async -> Bool {
        return await coordinator.stopScanning()
    }
    
    func openSystemSettings() {
        coordinator.openSystemSettings()
    }
    
    func getSystemStatus() -> [String: Any] {
        return coordinator.getSystemStatus()
    }
}

import Combine