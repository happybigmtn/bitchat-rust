import Foundation
import CoreBluetooth
import UIKit
import os.log

/// Comprehensive lifecycle management for BitCraps iOS BLE operations
/// 
/// This class manages the complete lifecycle of BLE operations including
/// app state transitions, background handling, state restoration, and
/// resource management.
@available(iOS 13.0, *)
class BitCrapsLifecycleManager: ObservableObject {
    
    // MARK: - Lifecycle States
    
    enum LifecycleState: String, CaseIterable {
        case uninitialized = "uninitialized"
        case initializing = "initializing"
        case ready = "ready"
        case active = "active"
        case backgrounded = "backgrounded"
        case suspended = "suspended"
        case terminating = "terminating"
        case error = "error"
        
        var allowsNewConnections: Bool {
            switch self {
            case .active, .ready:
                return true
            default:
                return false
            }
        }
        
        var allowsScanning: Bool {
            switch self {
            case .active, .ready, .backgrounded:
                return true
            default:
                return false
            }
        }
        
        var allowsAdvertising: Bool {
            switch self {
            case .active, .ready, .backgrounded:
                return true
            default:
                return false
            }
        }
    }
    
    // MARK: - Properties
    
    @Published var currentState: LifecycleState = .uninitialized
    @Published var isInitialized: Bool = false
    @Published var backgroundTimeRemaining: TimeInterval = 0
    @Published var stateRestorationActive: Bool = false
    
    private let logger = OSLog(subsystem: "com.bitcraps.ios", category: "LifecycleManager")
    
    // Dependencies
    private weak var bluetoothBridge: BitCrapsBluetoothBridge?
    private weak var errorHandler: BitCrapsErrorHandler?
    
    // State management
    private var stateTransitionHistory: [StateTransition] = []
    private var backgroundTask: UIBackgroundTaskIdentifier = .invalid
    private var stateRestorationTimer: Timer?
    private var resourceCleanupTimer: Timer?
    
    // Lifecycle tracking
    private var initializationStartTime: Date?
    private var lastStateChangeTime: Date = Date()
    private var backgroundEntryTime: Date?
    
    // MARK: - State Transition History
    
    private struct StateTransition {
        let fromState: LifecycleState
        let toState: LifecycleState
        let timestamp: Date
        let reason: String?
        let success: Bool
    }
    
    // MARK: - Initialization
    
    init() {
        setupNotificationObservers()
        os_log("LifecycleManager initialized", log: logger, type: .info)
    }
    
    deinit {
        cleanup()
        NotificationCenter.default.removeObserver(self)
    }
    
    // MARK: - Public Interface
    
    /// Initialize the lifecycle manager with dependencies
    func initialize(bluetoothBridge: BitCrapsBluetoothBridge, errorHandler: BitCrapsErrorHandler) {
        os_log("Initializing lifecycle manager", log: logger, type: .info)
        
        initializationStartTime = Date()
        
        self.bluetoothBridge = bluetoothBridge
        self.errorHandler = errorHandler
        
        transitionToState(.initializing, reason: "Manager initialization")
        
        // Perform initialization tasks
        DispatchQueue.main.async {
            self.performInitialization()
        }
    }
    
    /// Handle app state transitions
    func handleAppStateTransition(_ newState: UIApplication.State) {
        os_log("App state transition: %@", log: logger, type: .info, String(describing: newState))
        
        switch newState {
        case .active:
            handleAppBecameActive()
        case .inactive:
            handleAppBecameInactive()
        case .background:
            handleAppEnteredBackground()
        @unknown default:
            os_log("Unknown app state: %@", log: logger, type: .error, String(describing: newState))
        }
    }
    
    /// Handle Bluetooth state changes
    func handleBluetoothStateChange(_ state: CBManagerState) {
        os_log("Bluetooth state changed: %@", log: logger, type: .info, String(describing: state))
        
        switch state {
        case .poweredOn:
            if currentState == .ready || currentState == .backgrounded {
                transitionToState(.active, reason: "Bluetooth powered on")
            }
        case .poweredOff, .resetting, .unauthorized, .unsupported:
            if currentState != .error {
                transitionToState(.error, reason: "Bluetooth unavailable: \(state)")
            }
        case .unknown:
            // Don't transition on unknown state
            break
        @unknown default:
            os_log("Unknown Bluetooth state: %d", log: logger, type: .error, state.rawValue)
        }
    }
    
    /// Handle system memory warnings
    func handleMemoryWarning() {
        os_log("Memory warning received", log: logger, type: .error)
        
        // Perform aggressive cleanup
        performResourceCleanup(aggressive: true)
        
        // Notify error handler
        errorHandler?.handleError(.memoryAllocationFailed)
    }
    
    /// Handle app termination
    func handleAppTermination() {
        os_log("App termination initiated", log: logger, type: .info)
        
        transitionToState(.terminating, reason: "App termination")
        
        // Perform final cleanup
        cleanup()
    }
    
    /// Request state restoration
    func requestStateRestoration(restorationIdentifier: String) {
        os_log("State restoration requested: %@", log: logger, type: .info, restorationIdentifier)
        
        stateRestorationActive = true
        
        // Set up restoration timeout
        stateRestorationTimer = Timer.scheduledTimer(withTimeInterval: 10.0, repeats: false) { _ in
            self.completeStateRestoration()
        }
        
        // Notify Bluetooth bridge to handle restoration
        NotificationCenter.default.post(
            name: .bitCrapsStateRestorationRequested,
            object: restorationIdentifier
        )
    }
    
    /// Complete state restoration
    func completeStateRestoration() {
        os_log("State restoration completed", log: logger, type: .info)
        
        stateRestorationActive = false
        stateRestorationTimer?.invalidate()
        stateRestorationTimer = nil
        
        // Transition to appropriate state
        if UIApplication.shared.applicationState == .background {
            transitionToState(.backgrounded, reason: "State restoration completed in background")
        } else {
            transitionToState(.active, reason: "State restoration completed in foreground")
        }
    }
    
    /// Get current lifecycle statistics
    func getLifecycleStatistics() -> [String: Any] {
        let uptime = initializationStartTime?.timeIntervalSinceNow.magnitude ?? 0
        let stateChanges = stateTransitionHistory.count
        let backgroundTime = backgroundEntryTime?.timeIntervalSinceNow.magnitude ?? 0
        
        let stateDistribution = stateTransitionHistory.reduce(into: [String: Int]()) { counts, transition in
            counts[transition.toState.rawValue, default: 0] += 1
        }
        
        return [
            "currentState": currentState.rawValue,
            "uptime": uptime,
            "totalStateChanges": stateChanges,
            "currentBackgroundTime": backgroundTime,
            "stateDistribution": stateDistribution,
            "isInitialized": isInitialized,
            "stateRestorationActive": stateRestorationActive,
            "backgroundTimeRemaining": backgroundTimeRemaining
        ]
    }
    
    /// Check if operation is allowed in current state
    func isOperationAllowed(_ operation: String) -> Bool {
        switch operation.lowercased() {
        case "connect", "connection":
            return currentState.allowsNewConnections
        case "scan", "scanning":
            return currentState.allowsScanning
        case "advertise", "advertising":
            return currentState.allowsAdvertising
        default:
            return currentState != .uninitialized && currentState != .terminating
        }
    }
    
    // MARK: - Private Methods
    
    private func setupNotificationObservers() {
        // App lifecycle notifications
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(appDidBecomeActive),
            name: UIApplication.didBecomeActiveNotification,
            object: nil
        )
        
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(appWillResignActive),
            name: UIApplication.willResignActiveNotification,
            object: nil
        )
        
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(appDidEnterBackground),
            name: UIApplication.didEnterBackgroundNotification,
            object: nil
        )
        
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(appWillEnterForeground),
            name: UIApplication.willEnterForegroundNotification,
            object: nil
        )
        
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(appWillTerminate),
            name: UIApplication.willTerminateNotification,
            object: nil
        )
        
        // Memory warnings
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(memoryWarningReceived),
            name: UIApplication.didReceiveMemoryWarningNotification,
            object: nil
        )
        
        // Error handler recovery requests
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(handleCleanupRequest),
            name: .bitCrapsRequestCleanup,
            object: nil
        )
        
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(handleRustReinitialization),
            name: .bitCrapsRequestRustReinitialization,
            object: nil
        )
    }
    
    private func performInitialization() {
        guard currentState == .initializing else { return }
        
        os_log("Performing lifecycle initialization", log: logger, type: .info)
        
        // Initialize Rust FFI layer
        let rustInitResult = ios_ble_initialize()
        guard rustInitResult == 1 else {
            transitionToState(.error, reason: "Rust FFI initialization failed")
            return
        }
        
        // Set up resource monitoring
        setupResourceMonitoring()
        
        // Mark as initialized
        isInitialized = true
        
        // Transition to ready state
        transitionToState(.ready, reason: "Initialization completed successfully")
        
        os_log("Lifecycle initialization completed in %.2f seconds", 
               log: logger, type: .info,
               Date().timeIntervalSince(initializationStartTime ?? Date()))
    }
    
    private func transitionToState(_ newState: LifecycleState, reason: String?) {
        let oldState = currentState
        
        guard oldState != newState else {
            os_log("Ignoring redundant state transition to %@", log: logger, type: .debug, newState.rawValue)
            return
        }
        
        os_log("State transition: %@ -> %@ (%@)", log: logger, type: .info,
               oldState.rawValue, newState.rawValue, reason ?? "no reason")
        
        // Record transition
        let transition = StateTransition(
            fromState: oldState,
            toState: newState,
            timestamp: Date(),
            reason: reason,
            success: true // Will be updated if transition fails
        )
        
        // Update state
        currentState = newState
        lastStateChangeTime = Date()
        
        // Perform state-specific actions
        performStateTransitionActions(from: oldState, to: newState)
        
        // Record successful transition
        stateTransitionHistory.append(transition)
        
        // Notify observers
        NotificationCenter.default.post(
            name: .bitCrapsLifecycleStateChanged,
            object: ["oldState": oldState, "newState": newState, "reason": reason as Any]
        )
    }
    
    private func performStateTransitionActions(from oldState: LifecycleState, to newState: LifecycleState) {
        // Handle state exit actions
        switch oldState {
        case .active:
            // Stop active operations if moving to inactive states
            if !newState.allowsNewConnections {
                pauseActiveOperations()
            }
        case .backgrounded:
            // End background task if leaving background
            endBackgroundTask()
        default:
            break
        }
        
        // Handle state entry actions
        switch newState {
        case .ready:
            // Prepare for operations but don't start them
            prepareForOperations()
            
        case .active:
            // Start or resume operations
            resumeOperations()
            
        case .backgrounded:
            // Prepare for background operation
            prepareForBackground()
            
        case .suspended:
            // Save state and suspend operations
            suspendOperations()
            
        case .error:
            // Handle error state
            handleErrorState()
            
        case .terminating:
            // Final cleanup before termination
            performFinalCleanup()
            
        default:
            break
        }
    }
    
    private func prepareForOperations() {
        os_log("Preparing for operations", log: logger, type: .info)
        
        // Set up timers and monitoring
        setupResourceMonitoring()
    }
    
    private func resumeOperations() {
        os_log("Resuming operations", log: logger, type: .info)
        
        // Notify Bluetooth bridge to resume operations
        NotificationCenter.default.post(
            name: .bitCrapsResumeOperations,
            object: nil
        )
    }
    
    private func pauseActiveOperations() {
        os_log("Pausing active operations", log: logger, type: .info)
        
        // Notify Bluetooth bridge to pause non-essential operations
        NotificationCenter.default.post(
            name: .bitCrapsPauseOperations,
            object: nil
        )
    }
    
    private func prepareForBackground() {
        os_log("Preparing for background operation", log: logger, type: .info)
        
        backgroundEntryTime = Date()
        
        // Start background task
        startBackgroundTask()
        
        // Notify Bluetooth bridge of background transition
        NotificationCenter.default.post(
            name: .bitCrapsEnteredBackground,
            object: nil
        )
    }
    
    private func suspendOperations() {
        os_log("Suspending operations", log: logger, type: .info)
        
        // Save current state
        saveCurrentState()
        
        // Notify Bluetooth bridge to suspend operations
        NotificationCenter.default.post(
            name: .bitCrapsSuspendOperations,
            object: nil
        )
    }
    
    private func handleErrorState() {
        os_log("Handling error state", log: logger, type: .error)
        
        // Pause operations
        pauseActiveOperations()
        
        // Notify error handler
        // Error handler will drive recovery
    }
    
    private func performFinalCleanup() {
        os_log("Performing final cleanup", log: logger, type: .info)
        
        cleanup()
    }
    
    private func startBackgroundTask() {
        guard backgroundTask == .invalid else { return }
        
        backgroundTask = UIApplication.shared.beginBackgroundTask(withName: "BitCraps BLE") {
            os_log("Background task expired", log: self.logger, type: .error)
            self.endBackgroundTask()
        }
        
        // Monitor background time remaining
        monitorBackgroundTime()
        
        os_log("Background task started: %@", log: logger, type: .info, String(describing: backgroundTask))
    }
    
    private func endBackgroundTask() {
        guard backgroundTask != .invalid else { return }
        
        os_log("Ending background task: %@", log: logger, type: .info, String(describing: backgroundTask))
        
        UIApplication.shared.endBackgroundTask(backgroundTask)
        backgroundTask = .invalid
        backgroundTimeRemaining = 0
    }
    
    private func monitorBackgroundTime() {
        guard backgroundTask != .invalid else { return }
        
        Timer.scheduledTimer(withTimeInterval: 1.0, repeats: true) { timer in
            guard self.backgroundTask != .invalid else {
                timer.invalidate()
                return
            }
            
            self.backgroundTimeRemaining = UIApplication.shared.backgroundTimeRemaining
            
            if self.backgroundTimeRemaining <= 5.0 { // 5 seconds warning
                os_log("Background time low: %.1f seconds remaining", 
                       log: self.logger, type: .error, self.backgroundTimeRemaining)
                
                // Prepare for suspension
                self.transitionToState(.suspended, reason: "Background time expiring")
                timer.invalidate()
            }
        }
    }
    
    private func setupResourceMonitoring() {
        resourceCleanupTimer?.invalidate()
        resourceCleanupTimer = Timer.scheduledTimer(withTimeInterval: 60.0, repeats: true) { _ in
            self.performResourceCleanup(aggressive: false)
        }
    }
    
    private func performResourceCleanup(aggressive: Bool) {
        os_log("Performing resource cleanup (aggressive: %@)", log: logger, type: .info, aggressive ? "YES" : "NO")
        
        // Clean up old state transitions
        let cutoffTime = Date().addingTimeInterval(-3600) // Keep 1 hour of history
        stateTransitionHistory = stateTransitionHistory.filter { $0.timestamp > cutoffTime }
        
        // Notify Bluetooth bridge to clean up resources
        NotificationCenter.default.post(
            name: .bitCrapsCleanupResources,
            object: ["aggressive": aggressive]
        )
    }
    
    private func saveCurrentState() {
        os_log("Saving current state", log: logger, type: .info)
        
        let stateData: [String: Any] = [
            "currentState": currentState.rawValue,
            "lastStateChangeTime": lastStateChangeTime.timeIntervalSince1970,
            "isInitialized": isInitialized
        ]
        
        UserDefaults.standard.set(stateData, forKey: "BitCrapsLifecycleState")
        UserDefaults.standard.synchronize()
    }
    
    private func restoreState() {
        guard let stateData = UserDefaults.standard.object(forKey: "BitCrapsLifecycleState") as? [String: Any] else {
            os_log("No saved state found", log: logger, type: .info)
            return
        }
        
        if let savedStateRaw = stateData["currentState"] as? String,
           let savedState = LifecycleState(rawValue: savedStateRaw) {
            os_log("Restoring state: %@", log: logger, type: .info, savedState.rawValue)
            
            // Don't restore error or terminating states
            if savedState != .error && savedState != .terminating {
                currentState = savedState
                lastStateChangeTime = Date(timeIntervalSince1970: stateData["lastStateChangeTime"] as? TimeInterval ?? Date().timeIntervalSince1970)
                isInitialized = stateData["isInitialized"] as? Bool ?? false
            }
        }
    }
    
    private func cleanup() {
        os_log("Cleaning up lifecycle manager", log: logger, type: .info)
        
        // End background task
        endBackgroundTask()
        
        // Invalidate timers
        stateRestorationTimer?.invalidate()
        resourceCleanupTimer?.invalidate()
        
        // Save final state
        saveCurrentState()
        
        // Shutdown Rust FFI if initialized
        if isInitialized {
            let _ = ios_ble_shutdown()
        }
        
        isInitialized = false
    }
    
    // MARK: - App State Handlers
    
    private func handleAppBecameActive() {
        if currentState == .backgrounded || currentState == .ready {
            transitionToState(.active, reason: "App became active")
        }
    }
    
    private func handleAppBecameInactive() {
        if currentState == .active {
            transitionToState(.ready, reason: "App became inactive")
        }
    }
    
    private func handleAppEnteredBackground() {
        if currentState == .active || currentState == .ready {
            transitionToState(.backgrounded, reason: "App entered background")
        }
    }
    
    // MARK: - Notification Handlers
    
    @objc private func appDidBecomeActive() {
        handleAppBecameActive()
    }
    
    @objc private func appWillResignActive() {
        handleAppBecameInactive()
    }
    
    @objc private func appDidEnterBackground() {
        handleAppEnteredBackground()
    }
    
    @objc private func appWillEnterForeground() {
        // State transition will be handled by appDidBecomeActive
    }
    
    @objc private func appWillTerminate() {
        handleAppTermination()
    }
    
    @objc private func memoryWarningReceived() {
        handleMemoryWarning()
    }
    
    @objc private func handleCleanupRequest() {
        performResourceCleanup(aggressive: true)
    }
    
    @objc private func handleRustReinitialization() {
        os_log("Reinitializing Rust FFI layer", log: logger, type: .info)
        
        // Shutdown and reinitialize Rust
        if isInitialized {
            let _ = ios_ble_shutdown()
        }
        
        let initResult = ios_ble_initialize()
        if initResult == 1 {
            os_log("Rust FFI reinitialization successful", log: logger, type: .info)
            
            // Transition back to ready state if we were in error
            if currentState == .error {
                transitionToState(.ready, reason: "Rust FFI reinitialized")
            }
        } else {
            os_log("Rust FFI reinitialization failed", log: logger, type: .error)
            transitionToState(.error, reason: "Rust FFI reinitialization failed")
        }
    }
}

// MARK: - Additional Notification Extensions

extension Notification.Name {
    static let bitCrapsLifecycleStateChanged = Notification.Name("bitCrapsLifecycleStateChanged")
    static let bitCrapsStateRestorationRequested = Notification.Name("bitCrapsStateRestorationRequested")
    static let bitCrapsResumeOperations = Notification.Name("bitCrapsResumeOperations")
    static let bitCrapsPauseOperations = Notification.Name("bitCrapsPauseOperations")
    static let bitCrapsEnteredBackground = Notification.Name("bitCrapsEnteredBackground")
    static let bitCrapsSuspendOperations = Notification.Name("bitCrapsSuspendOperations")
    static let bitCrapsCleanupResources = Notification.Name("bitCrapsCleanupResources")
}