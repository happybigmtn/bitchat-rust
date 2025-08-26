import Foundation
import CoreBluetooth
import os.log

/// Comprehensive error handling and lifecycle management for BitCraps iOS BLE
/// 
/// This class provides centralized error handling, recovery strategies, and
/// lifecycle management for the iOS CoreBluetooth implementation.
@available(iOS 13.0, *)
class BitCrapsErrorHandler: ObservableObject {
    
    // MARK: - Error Types
    
    enum BitCrapsBluetoothError: Error, LocalizedError {
        case bluetoothUnavailable
        case bluetoothUnauthorized
        case bluetoothPoweredOff
        case bluetoothResetting
        case bluetoothUnsupported
        case peripheralNotFound(String)
        case connectionFailed(String, Error?)
        case disconnectionFailed(String, Error?)
        case serviceDiscoveryFailed(Error)
        case characteristicDiscoveryFailed(Error)
        case dataTransmissionFailed(Error)
        case advertisingFailed(Error)
        case scanningFailed(Error)
        case backgroundLimitationsExceeded
        case memoryAllocationFailed
        case invalidParameters(String)
        case rustFFIError(String)
        case timeoutError(String)
        case resourceExhausted(String)
        
        var errorDescription: String? {
            switch self {
            case .bluetoothUnavailable:
                return "Bluetooth is not available on this device"
            case .bluetoothUnauthorized:
                return "Bluetooth access is not authorized for this app"
            case .bluetoothPoweredOff:
                return "Bluetooth is turned off. Please enable Bluetooth in Settings"
            case .bluetoothResetting:
                return "Bluetooth is resetting. Please wait a moment and try again"
            case .bluetoothUnsupported:
                return "Bluetooth LE is not supported on this device"
            case .peripheralNotFound(let id):
                return "Peripheral not found: \(id)"
            case .connectionFailed(let id, let error):
                return "Connection failed to \(id): \(error?.localizedDescription ?? "Unknown error")"
            case .disconnectionFailed(let id, let error):
                return "Disconnection failed from \(id): \(error?.localizedDescription ?? "Unknown error")"
            case .serviceDiscoveryFailed(let error):
                return "Service discovery failed: \(error.localizedDescription)"
            case .characteristicDiscoveryFailed(let error):
                return "Characteristic discovery failed: \(error.localizedDescription)"
            case .dataTransmissionFailed(let error):
                return "Data transmission failed: \(error.localizedDescription)"
            case .advertisingFailed(let error):
                return "Advertising failed: \(error.localizedDescription)"
            case .scanningFailed(let error):
                return "Scanning failed: \(error.localizedDescription)"
            case .backgroundLimitationsExceeded:
                return "Background BLE operations are too limited for reliable gaming"
            case .memoryAllocationFailed:
                return "Memory allocation failed - device may be low on memory"
            case .invalidParameters(let details):
                return "Invalid parameters: \(details)"
            case .rustFFIError(let details):
                return "Rust FFI error: \(details)"
            case .timeoutError(let operation):
                return "Timeout during \(operation)"
            case .resourceExhausted(let resource):
                return "Resource exhausted: \(resource)"
            }
        }
        
        var recoveryStrategy: ErrorRecoveryStrategy {
            switch self {
            case .bluetoothUnavailable, .bluetoothUnsupported:
                return .fatal
            case .bluetoothUnauthorized:
                return .requestPermission
            case .bluetoothPoweredOff:
                return .waitForBluetoothEnable
            case .bluetoothResetting:
                return .retryAfterDelay(5.0)
            case .peripheralNotFound:
                return .rescan
            case .connectionFailed, .disconnectionFailed:
                return .retryConnection
            case .serviceDiscoveryFailed, .characteristicDiscoveryFailed:
                return .rediscoverServices
            case .dataTransmissionFailed:
                return .retryTransmission
            case .advertisingFailed, .scanningFailed:
                return .restartOperation
            case .backgroundLimitationsExceeded:
                return .degradedMode
            case .memoryAllocationFailed:
                return .cleanup
            case .invalidParameters:
                return .validateParameters
            case .rustFFIError:
                return .reinitializeRust
            case .timeoutError:
                return .retryAfterDelay(3.0)
            case .resourceExhausted:
                return .cleanup
            }
        }
    }
    
    enum ErrorRecoveryStrategy {
        case fatal                              // Cannot recover, app cannot function
        case requestPermission                  // Request user permission
        case waitForBluetoothEnable            // Wait for user to enable Bluetooth
        case retryAfterDelay(TimeInterval)     // Retry operation after delay
        case rescan                            // Restart scanning for peers
        case retryConnection                   // Retry peer connection
        case rediscoverServices                // Rediscover BLE services
        case retryTransmission                 // Retry data transmission
        case restartOperation                  // Restart the failed operation
        case degradedMode                      // Switch to degraded operation mode
        case cleanup                           // Clean up resources and retry
        case validateParameters                // Validate and correct parameters
        case reinitializeRust                  // Reinitialize Rust FFI layer
    }
    
    // MARK: - Properties
    
    @Published var currentError: BitCrapsBluetoothError?
    @Published var isRecovering: Bool = false
    @Published var recoveryAttempts: Int = 0
    @Published var degradedMode: Bool = false
    
    private let logger = OSLog(subsystem: "com.bitcraps.ios", category: "ErrorHandler")
    private let maxRecoveryAttempts = 3
    private var recoveryTimer: Timer?
    private var errorHistory: [ErrorHistoryEntry] = []
    
    // MARK: - Error History
    
    private struct ErrorHistoryEntry {
        let error: BitCrapsBluetoothError
        let timestamp: Date
        let recoveryStrategy: ErrorRecoveryStrategy
        let recoverySuccessful: Bool?
    }
    
    // MARK: - Public Interface
    
    /// Handle an error with automatic recovery strategy
    func handleError(_ error: BitCrapsBluetoothError) {
        os_log("Handling error: %@", log: logger, type: .error, error.localizedDescription)
        
        // Record error in history
        let historyEntry = ErrorHistoryEntry(
            error: error,
            timestamp: Date(),
            recoveryStrategy: error.recoveryStrategy,
            recoverySuccessful: nil
        )
        errorHistory.append(historyEntry)
        
        // Update published error
        currentError = error
        
        // Execute recovery strategy
        executeRecoveryStrategy(error.recoveryStrategy, for: error)
    }
    
    /// Handle Bluetooth state changes with appropriate error mapping
    func handleBluetoothStateChange(_ state: CBManagerState) -> BitCrapsBluetoothError? {
        switch state {
        case .poweredOn:
            clearError()
            return nil
        case .poweredOff:
            return .bluetoothPoweredOff
        case .resetting:
            return .bluetoothResetting
        case .unauthorized:
            return .bluetoothUnauthorized
        case .unsupported:
            return .bluetoothUnsupported
        case .unknown:
            return nil // Don't treat unknown as an error initially
        @unknown default:
            return .bluetoothUnavailable
        }
    }
    
    /// Handle Core Bluetooth errors
    func handleCoreBluetoothError(_ error: Error?, operation: String, peerID: String? = nil) -> BitCrapsBluetoothError {
        guard let error = error else {
            return .invalidParameters("No error provided for \(operation)")
        }
        
        let nsError = error as NSError
        
        // Map Core Bluetooth error codes to our error types
        if let cbError = CBError(rawValue: nsError.code) {
            switch cbError {
            case .unknown:
                return .rustFFIError("Unknown Core Bluetooth error during \(operation)")
            case .invalidParameters:
                return .invalidParameters("Invalid parameters for \(operation)")
            case .invalidHandle:
                if let peerID = peerID {
                    return .peripheralNotFound(peerID)
                } else {
                    return .invalidParameters("Invalid handle for \(operation)")
                }
            case .notConnected:
                if let peerID = peerID {
                    return .connectionFailed(peerID, error)
                } else {
                    return .connectionFailed("unknown", error)
                }
            case .outOfSpace:
                return .resourceExhausted("Bluetooth connection space")
            case .operationCancelled:
                return .timeoutError(operation)
            case .connectionTimeout:
                return .timeoutError("connection to \(peerID ?? "peer")")
            case .peripheralDisconnected:
                if let peerID = peerID {
                    return .disconnectionFailed(peerID, error)
                } else {
                    return .disconnectionFailed("unknown", error)
                }
            case .uuidNotAllowed:
                return .invalidParameters("UUID not allowed for \(operation)")
            case .alreadyAdvertising:
                return .advertisingFailed(error)
            case .connectionFailed:
                if let peerID = peerID {
                    return .connectionFailed(peerID, error)
                } else {
                    return .connectionFailed("unknown", error)
                }
            case .connectionLimitReached:
                return .resourceExhausted("Bluetooth connections")
            @unknown default:
                return .rustFFIError("Unknown Core Bluetooth error code: \(cbError.rawValue)")
            }
        } else {
            return .rustFFIError("Unrecognized error during \(operation): \(error.localizedDescription)")
        }
    }
    
    /// Check if we're in a recoverable state
    func canRecover() -> Bool {
        guard let error = currentError else { return true }
        
        return error.recoveryStrategy != .fatal && recoveryAttempts < maxRecoveryAttempts
    }
    
    /// Clear current error state
    func clearError() {
        os_log("Clearing error state", log: logger, type: .info)
        
        currentError = nil
        isRecovering = false
        recoveryAttempts = 0
        recoveryTimer?.invalidate()
        recoveryTimer = nil
        
        // Update error history with recovery success
        if var lastEntry = errorHistory.last {
            errorHistory[errorHistory.count - 1] = ErrorHistoryEntry(
                error: lastEntry.error,
                timestamp: lastEntry.timestamp,
                recoveryStrategy: lastEntry.recoveryStrategy,
                recoverySuccessful: true
            )
        }
    }
    
    /// Get error statistics for debugging
    func getErrorStatistics() -> [String: Any] {
        let totalErrors = errorHistory.count
        let recentErrors = errorHistory.filter { $0.timestamp.timeIntervalSinceNow > -3600 } // Last hour
        
        let errorCounts = errorHistory.reduce(into: [String: Int]()) { counts, entry in
            let errorType = String(describing: entry.error)
            counts[errorType, default: 0] += 1
        }
        
        let recoverySuccess = errorHistory.compactMap { $0.recoverySuccessful }.filter { $0 }
        let recoveryRate = errorHistory.isEmpty ? 1.0 : Double(recoverySuccess.count) / Double(errorHistory.count)
        
        return [
            "totalErrors": totalErrors,
            "recentErrors": recentErrors.count,
            "errorCounts": errorCounts,
            "recoveryRate": recoveryRate,
            "currentlyRecovering": isRecovering,
            "degradedMode": degradedMode
        ]
    }
    
    // MARK: - Private Methods
    
    private func executeRecoveryStrategy(_ strategy: ErrorRecoveryStrategy, for error: BitCrapsBluetoothError) {
        guard canRecover() else {
            os_log("Cannot recover from error - max attempts reached", log: logger, type: .error)
            return
        }
        
        isRecovering = true
        recoveryAttempts += 1
        
        os_log("Executing recovery strategy: %@ (attempt %d/%d)", log: logger, type: .info,
               String(describing: strategy), recoveryAttempts, maxRecoveryAttempts)
        
        switch strategy {
        case .fatal:
            handleFatalError(error)
            
        case .requestPermission:
            requestBluetoothPermission()
            
        case .waitForBluetoothEnable:
            waitForBluetoothEnable()
            
        case .retryAfterDelay(let delay):
            retryAfterDelay(delay)
            
        case .rescan:
            requestRescan()
            
        case .retryConnection:
            requestRetryConnection()
            
        case .rediscoverServices:
            requestRediscoverServices()
            
        case .retryTransmission:
            requestRetryTransmission()
            
        case .restartOperation:
            requestRestartOperation()
            
        case .degradedMode:
            enterDegradedMode()
            
        case .cleanup:
            performCleanup()
            
        case .validateParameters:
            requestParameterValidation()
            
        case .reinitializeRust:
            requestRustReinitialization()
        }
    }
    
    private func handleFatalError(_ error: BitCrapsBluetoothError) {
        os_log("Fatal error encountered: %@", log: logger, type: .fault, error.localizedDescription)
        
        isRecovering = false
        
        // Post notification for UI to handle fatal error
        NotificationCenter.default.post(
            name: .bitCrapsFatalError,
            object: error
        )
    }
    
    private func requestBluetoothPermission() {
        os_log("Requesting Bluetooth permission", log: logger, type: .info)
        
        // Post notification for UI to request permission
        NotificationCenter.default.post(
            name: .bitCrapsRequestPermission,
            object: "bluetooth"
        )
        
        // Set a timeout for permission request
        scheduleRecoveryTimeout(30.0) // 30 seconds to grant permission
    }
    
    private func waitForBluetoothEnable() {
        os_log("Waiting for Bluetooth to be enabled", log: logger, type: .info)
        
        // Post notification for UI to show waiting state
        NotificationCenter.default.post(
            name: .bitCrapsWaitingForBluetooth,
            object: nil
        )
        
        // Set a longer timeout for Bluetooth enable
        scheduleRecoveryTimeout(60.0) // 60 seconds to enable Bluetooth
    }
    
    private func retryAfterDelay(_ delay: TimeInterval) {
        os_log("Retrying after delay: %.1f seconds", log: logger, type: .info, delay)
        
        recoveryTimer = Timer.scheduledTimer(withTimeInterval: delay, repeats: false) { _ in
            self.requestOperationRetry()
        }
    }
    
    private func requestRescan() {
        os_log("Requesting BLE rescan", log: logger, type: .info)
        
        NotificationCenter.default.post(
            name: .bitCrapsRequestRescan,
            object: nil
        )
        
        scheduleRecoveryTimeout(15.0) // 15 seconds to complete scan
    }
    
    private func requestRetryConnection() {
        os_log("Requesting connection retry", log: logger, type: .info)
        
        NotificationCenter.default.post(
            name: .bitCrapsRequestRetryConnection,
            object: nil
        )
        
        scheduleRecoveryTimeout(10.0) // 10 seconds to retry connection
    }
    
    private func requestRediscoverServices() {
        os_log("Requesting service rediscovery", log: logger, type: .info)
        
        NotificationCenter.default.post(
            name: .bitCrapsRequestRediscoverServices,
            object: nil
        )
        
        scheduleRecoveryTimeout(10.0) // 10 seconds to rediscover services
    }
    
    private func requestRetryTransmission() {
        os_log("Requesting data transmission retry", log: logger, type: .info)
        
        NotificationCenter.default.post(
            name: .bitCrapsRequestRetryTransmission,
            object: nil
        )
        
        scheduleRecoveryTimeout(5.0) // 5 seconds to retry transmission
    }
    
    private func requestRestartOperation() {
        os_log("Requesting operation restart", log: logger, type: .info)
        
        NotificationCenter.default.post(
            name: .bitCrapsRequestRestartOperation,
            object: currentError
        )
        
        scheduleRecoveryTimeout(10.0) // 10 seconds to restart operation
    }
    
    private func enterDegradedMode() {
        os_log("Entering degraded operation mode", log: logger, type: .info)
        
        degradedMode = true
        isRecovering = false
        currentError = nil // Clear error since we're adapting
        
        NotificationCenter.default.post(
            name: .bitCrapsEnteredDegradedMode,
            object: nil
        )
    }
    
    private func performCleanup() {
        os_log("Performing resource cleanup", log: logger, type: .info)
        
        NotificationCenter.default.post(
            name: .bitCrapsRequestCleanup,
            object: nil
        )
        
        // After cleanup, retry the original operation
        scheduleRecoveryTimeout(5.0) // 5 seconds for cleanup
    }
    
    private func requestParameterValidation() {
        os_log("Requesting parameter validation", log: logger, type: .info)
        
        NotificationCenter.default.post(
            name: .bitCrapsRequestParameterValidation,
            object: currentError
        )
        
        scheduleRecoveryTimeout(5.0) // 5 seconds to validate parameters
    }
    
    private func requestRustReinitialization() {
        os_log("Requesting Rust FFI reinitialization", log: logger, type: .info)
        
        NotificationCenter.default.post(
            name: .bitCrapsRequestRustReinitialization,
            object: nil
        )
        
        scheduleRecoveryTimeout(10.0) // 10 seconds to reinitialize Rust
    }
    
    private func requestOperationRetry() {
        os_log("Requesting operation retry", log: logger, type: .info)
        
        NotificationCenter.default.post(
            name: .bitCrapsRequestOperationRetry,
            object: currentError
        )
        
        scheduleRecoveryTimeout(10.0) // 10 seconds to retry operation
    }
    
    private func scheduleRecoveryTimeout(_ timeout: TimeInterval) {
        recoveryTimer?.invalidate()
        recoveryTimer = Timer.scheduledTimer(withTimeInterval: timeout, repeats: false) { _ in
            if self.isRecovering {
                os_log("Recovery timeout reached", log: self.logger, type: .error)
                self.handleRecoveryTimeout()
            }
        }
    }
    
    private func handleRecoveryTimeout() {
        if recoveryAttempts < maxRecoveryAttempts {
            os_log("Recovery timeout - retrying with next attempt", log: logger, type: .info)
            
            if let error = currentError {
                executeRecoveryStrategy(error.recoveryStrategy, for: error)
            }
        } else {
            os_log("Recovery timeout - max attempts reached", log: logger, type: .error)
            
            // Update error history with recovery failure
            if var lastEntry = errorHistory.last {
                errorHistory[errorHistory.count - 1] = ErrorHistoryEntry(
                    error: lastEntry.error,
                    timestamp: lastEntry.timestamp,
                    recoveryStrategy: lastEntry.recoveryStrategy,
                    recoverySuccessful: false
                )
            }
            
            // Enter degraded mode if possible, otherwise fatal error
            if let error = currentError, error.recoveryStrategy != .fatal {
                enterDegradedMode()
            } else {
                handleFatalError(currentError ?? .bluetoothUnavailable)
            }
        }
    }
}

// MARK: - Notification Extensions

extension Notification.Name {
    static let bitCrapsFatalError = Notification.Name("bitCrapsFatalError")
    static let bitCrapsRequestPermission = Notification.Name("bitCrapsRequestPermission")
    static let bitCrapsWaitingForBluetooth = Notification.Name("bitCrapsWaitingForBluetooth")
    static let bitCrapsRequestRescan = Notification.Name("bitCrapsRequestRescan")
    static let bitCrapsRequestRetryConnection = Notification.Name("bitCrapsRequestRetryConnection")
    static let bitCrapsRequestRediscoverServices = Notification.Name("bitCrapsRequestRediscoverServices")
    static let bitCrapsRequestRetryTransmission = Notification.Name("bitCrapsRequestRetryTransmission")
    static let bitCrapsRequestRestartOperation = Notification.Name("bitCrapsRequestRestartOperation")
    static let bitCrapsEnteredDegradedMode = Notification.Name("bitCrapsEnteredDegradedMode")
    static let bitCrapsRequestCleanup = Notification.Name("bitCrapsRequestCleanup")
    static let bitCrapsRequestParameterValidation = Notification.Name("bitCrapsRequestParameterValidation")
    static let bitCrapsRequestRustReinitialization = Notification.Name("bitCrapsRequestRustReinitialization")
    static let bitCrapsRequestOperationRetry = Notification.Name("bitCrapsRequestOperationRetry")
}