import Foundation
import CoreBluetooth
import UIKit
import os.log

/// iOS permission and state restoration management for BitCraps
/// 
/// This class handles all iOS permission requests, permission status monitoring,
/// and state restoration for background BLE operations.
@available(iOS 13.0, *)
class BitCrapsPermissionManager: NSObject, ObservableObject {
    
    // MARK: - Permission States
    
    enum PermissionStatus: String, CaseIterable {
        case notDetermined = "notDetermined"
        case denied = "denied"
        case authorized = "authorized"
        case restricted = "restricted"
        case unavailable = "unavailable"
        
        var isAuthorized: Bool {
            return self == .authorized
        }
        
        var canRequestPermission: Bool {
            return self == .notDetermined
        }
        
        var needsUserAction: Bool {
            switch self {
            case .denied, .restricted:
                return true
            default:
                return false
            }
        }
    }
    
    enum BackgroundMode: String, CaseIterable {
        case bluetoothCentral = "bluetooth-central"
        case bluetoothPeripheral = "bluetooth-peripheral"
        case backgroundProcessing = "background-processing"
        case backgroundAppRefresh = "background-app-refresh"
        
        var displayName: String {
            switch self {
            case .bluetoothCentral:
                return "Bluetooth Central"
            case .bluetoothPeripheral:
                return "Bluetooth Peripheral" 
            case .backgroundProcessing:
                return "Background Processing"
            case .backgroundAppRefresh:
                return "Background App Refresh"
            }
        }
        
        var isRequired: Bool {
            switch self {
            case .bluetoothCentral, .bluetoothPeripheral:
                return true
            case .backgroundProcessing, .backgroundAppRefresh:
                return false // Nice to have but not critical
            }
        }
    }
    
    // MARK: - Published Properties
    
    @Published var bluetoothPermissionStatus: PermissionStatus = .notDetermined
    @Published var backgroundAppRefreshEnabled: Bool = false
    @Published var lowPowerModeEnabled: Bool = false
    @Published var permissionRequestInProgress: Bool = false
    @Published var stateRestorationData: [String: Any]?
    
    // MARK: - Private Properties
    
    private let logger = OSLog(subsystem: "com.bitcraps.ios", category: "PermissionManager")
    private var centralManager: CBCentralManager?
    private var peripheralManager: CBPeripheralManager?
    private var permissionCompletionHandlers: [(PermissionStatus) -> Void] = []
    private var stateRestorationKeys = Set<String>()
    
    // Permission monitoring
    private var permissionCheckTimer: Timer?
    private var systemSettingsMonitor: Timer?
    
    // State restoration
    private let stateRestorationIdentifier = "BitCraps-BLE-State"
    private var pendingStateRestoration: [String: Any]?
    
    // MARK: - Initialization
    
    override init() {
        super.init()
        
        setupInitialState()
        setupSystemMonitoring()
        setupNotificationObservers()
        
        os_log("PermissionManager initialized", log: logger, type: .info)
    }
    
    deinit {
        cleanup()
        NotificationCenter.default.removeObserver(self)
    }
    
    // MARK: - Public Interface
    
    /// Request all necessary permissions
    func requestAllPermissions(completion: @escaping (Bool) -> Void) {
        os_log("Requesting all permissions", log: logger, type: .info)
        
        guard !permissionRequestInProgress else {
            os_log("Permission request already in progress", log: logger, type: .info)
            completion(false)
            return
        }
        
        permissionRequestInProgress = true
        
        // Request Bluetooth permissions first
        requestBluetoothPermissions { [weak self] bluetoothGranted in
            guard let self = self else {
                completion(false)
                return
            }
            
            if bluetoothGranted {
                // Check background modes
                self.checkBackgroundModes { backgroundOK in
                    self.permissionRequestInProgress = false
                    
                    let allGranted = bluetoothGranted && backgroundOK
                    os_log("All permissions result: %@", log: self.logger, type: .info, allGranted ? "GRANTED" : "DENIED")
                    
                    completion(allGranted)
                }
            } else {
                self.permissionRequestInProgress = false
                os_log("Bluetooth permissions denied", log: self.logger, type: .error)
                completion(false)
            }
        }
    }
    
    /// Request Bluetooth-specific permissions
    func requestBluetoothPermissions(completion: @escaping (Bool) -> Void) {
        os_log("Requesting Bluetooth permissions", log: logger, type: .info)
        
        permissionCompletionHandlers.append(completion)
        
        // Create managers to trigger permission dialogs
        if centralManager == nil {
            os_log("Creating CBCentralManager to request central permissions", log: logger, type: .info)
            centralManager = CBCentralManager(delegate: self, queue: nil, options: [
                CBCentralManagerOptionShowPowerAlertKey: true
            ])
        }
        
        // Delay peripheral manager creation slightly to avoid overwhelming user
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.0) {
            if self.peripheralManager == nil {
                os_log("Creating CBPeripheralManager to request peripheral permissions", log: self.logger, type: .info)
                self.peripheralManager = CBPeripheralManager(delegate: self, queue: nil, options: [
                    CBPeripheralManagerOptionShowPowerAlertKey: true
                ])
            }
        }
        
        // Set timeout for permission request
        DispatchQueue.main.asyncAfter(deadline: .now() + 30.0) {
            self.handlePermissionRequestTimeout()
        }
    }
    
    /// Check current permission status
    func checkPermissionStatus() -> PermissionStatus {
        // For iOS 13+, we check the manager states
        let centralStatus = centralManager?.state ?? .unknown
        let peripheralStatus = peripheralManager?.state ?? .unknown
        
        let combinedStatus = mapBluetoothStateToPermissionStatus(centralStatus, peripheralStatus)
        
        if combinedStatus != bluetoothPermissionStatus {
            bluetoothPermissionStatus = combinedStatus
            os_log("Permission status changed to: %@", log: logger, type: .info, combinedStatus.rawValue)
        }
        
        return combinedStatus
    }
    
    /// Check if all required permissions are granted
    func hasAllRequiredPermissions() -> Bool {
        let bluetoothOK = bluetoothPermissionStatus.isAuthorized
        let backgroundOK = hasRequiredBackgroundModes()
        
        return bluetoothOK && backgroundOK
    }
    
    /// Open system settings for permission configuration
    func openSystemSettings() {
        os_log("Opening system settings", log: logger, type: .info)
        
        if let settingsURL = URL(string: UIApplication.openSettingsURLString) {
            DispatchQueue.main.async {
                UIApplication.shared.open(settingsURL) { success in
                    os_log("System settings opened: %@", log: self.logger, type: .info, success ? "SUCCESS" : "FAILED")
                }
            }
        }
    }
    
    /// Get user-friendly permission status description
    func getPermissionStatusDescription() -> String {
        switch bluetoothPermissionStatus {
        case .notDetermined:
            return "Bluetooth permission has not been requested yet. BitCraps needs Bluetooth access to connect with other players."
        case .denied:
            return "Bluetooth permission is denied. Please enable Bluetooth access for BitCraps in Settings > Privacy & Security > Bluetooth."
        case .authorized:
            return "Bluetooth permission is granted. BitCraps can connect with other players."
        case .restricted:
            return "Bluetooth access is restricted by system policies. BitCraps cannot access Bluetooth."
        case .unavailable:
            return "Bluetooth is not available on this device. BitCraps requires Bluetooth LE support."
        }
    }
    
    /// Get background capabilities report
    func getBackgroundCapabilitiesReport() -> [String: Any] {
        let backgroundModes = getEnabledBackgroundModes()
        let requiredModes = BackgroundMode.allCases.filter { $0.isRequired }
        
        let hasRequiredModes = requiredModes.allSatisfy { mode in
            backgroundModes.contains(mode.rawValue)
        }
        
        return [
            "bluetoothPermission": bluetoothPermissionStatus.rawValue,
            "backgroundAppRefreshEnabled": backgroundAppRefreshEnabled,
            "lowPowerModeEnabled": lowPowerModeEnabled,
            "enabledBackgroundModes": backgroundModes,
            "requiredBackgroundModes": requiredModes.map { $0.rawValue },
            "hasAllRequiredModes": hasRequiredModes,
            "backgroundViable": hasRequiredModes && backgroundAppRefreshEnabled && !lowPowerModeEnabled
        ]
    }
    
    // MARK: - State Restoration
    
    /// Save current state for restoration
    func saveStateForRestoration() {
        os_log("Saving state for restoration", log: logger, type: .info)
        
        let restorationData: [String: Any] = [
            "bluetoothPermissionStatus": bluetoothPermissionStatus.rawValue,
            "backgroundAppRefreshEnabled": backgroundAppRefreshEnabled,
            "lowPowerModeEnabled": lowPowerModeEnabled,
            "timestamp": Date().timeIntervalSince1970,
            "version": "1.0"
        ]
        
        UserDefaults.standard.set(restorationData, forKey: stateRestorationIdentifier)
        UserDefaults.standard.synchronize()
        
        self.stateRestorationData = restorationData
        
        os_log("State saved for restoration: %@", log: logger, type: .debug, String(describing: restorationData))
    }
    
    /// Restore state from previous session
    func restoreState() -> Bool {
        os_log("Attempting to restore state", log: logger, type: .info)
        
        guard let savedData = UserDefaults.standard.object(forKey: stateRestorationIdentifier) as? [String: Any] else {
            os_log("No saved state found for restoration", log: logger, type: .info)
            return false
        }
        
        // Validate saved data
        guard let timestamp = savedData["timestamp"] as? TimeInterval,
              let version = savedData["version"] as? String,
              version == "1.0" else {
            os_log("Saved state data invalid or incompatible", log: logger, type: .error)
            return false
        }
        
        // Check if data is not too old (24 hours)
        let dataAge = Date().timeIntervalSince1970 - timestamp
        guard dataAge < 86400 else { // 24 hours in seconds
            os_log("Saved state data too old (%.1f hours), ignoring", log: logger, type: .info, dataAge / 3600)
            return false
        }
        
        // Restore permission status (but recheck it)
        if let statusString = savedData["bluetoothPermissionStatus"] as? String,
           let status = PermissionStatus(rawValue: statusString) {
            bluetoothPermissionStatus = status
        }
        
        backgroundAppRefreshEnabled = savedData["backgroundAppRefreshEnabled"] as? Bool ?? false
        lowPowerModeEnabled = savedData["lowPowerModeEnabled"] as? Bool ?? false
        
        self.stateRestorationData = savedData
        
        os_log("State restored from %.1f hours ago", log: logger, type: .info, dataAge / 3600)
        
        // Re-verify current permission status
        DispatchQueue.main.async {
            let _ = self.checkPermissionStatus()
        }
        
        return true
    }
    
    /// Handle Core Bluetooth state restoration
    func handleBluetoothStateRestoration(centralManager: CBCentralManager?, peripheralManager: CBPeripheralManager?) {
        os_log("Handling Bluetooth state restoration", log: logger, type: .info)
        
        if let central = centralManager {
            self.centralManager = central
            central.delegate = self
            os_log("Central manager restored", log: logger, type: .info)
        }
        
        if let peripheral = peripheralManager {
            self.peripheralManager = peripheral
            peripheral.delegate = self
            os_log("Peripheral manager restored", log: logger, type: .info)
        }
        
        // Update permission status based on restored managers
        let _ = checkPermissionStatus()
    }
    
    // MARK: - Private Methods
    
    private func setupInitialState() {
        // Check current system settings
        updateSystemSettings()
        
        // Try to restore previous state
        let _ = restoreState()
    }
    
    private func setupSystemMonitoring() {
        // Monitor system settings changes periodically
        systemSettingsMonitor = Timer.scheduledTimer(withTimeInterval: 5.0, repeats: true) { _ in
            self.updateSystemSettings()
        }
    }
    
    private func setupNotificationObservers() {
        // App state change notifications
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(appWillEnterForeground),
            name: UIApplication.willEnterForegroundNotification,
            object: nil
        )
        
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(appDidEnterBackground),
            name: UIApplication.didEnterBackgroundNotification,
            object: nil
        )
        
        // Permission request notifications from error handler
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(handlePermissionRequest),
            name: .bitCrapsRequestPermission,
            object: nil
        )
    }
    
    private func updateSystemSettings() {
        let newBackgroundRefreshEnabled = UIApplication.shared.backgroundRefreshStatus == .available
        let newLowPowerModeEnabled = ProcessInfo.processInfo.isLowPowerModeEnabled
        
        if newBackgroundRefreshEnabled != backgroundAppRefreshEnabled {
            backgroundAppRefreshEnabled = newBackgroundRefreshEnabled
            os_log("Background App Refresh changed: %@", log: logger, type: .info, newBackgroundRefreshEnabled ? "ENABLED" : "DISABLED")
        }
        
        if newLowPowerModeEnabled != lowPowerModeEnabled {
            lowPowerModeEnabled = newLowPowerModeEnabled
            os_log("Low Power Mode changed: %@", log: logger, type: .info, newLowPowerModeEnabled ? "ENABLED" : "DISABLED")
        }
    }
    
    private func mapBluetoothStateToPermissionStatus(_ centralState: CBManagerState, _ peripheralState: CBManagerState) -> PermissionStatus {
        // Combine both central and peripheral states
        let states = [centralState, peripheralState]
        
        // If any is unauthorized, we're unauthorized
        if states.contains(.unauthorized) {
            return .denied
        }
        
        // If any is unsupported, we're unavailable
        if states.contains(.unsupported) {
            return .unavailable
        }
        
        // If any is powered on, we have permission
        if states.contains(.poweredOn) {
            return .authorized
        }
        
        // If all are powered off, we still have permission (just disabled)
        if states.allSatisfy({ $0 == .poweredOff }) {
            return .authorized
        }
        
        // If any is unknown, we haven't determined yet
        if states.contains(.unknown) {
            return .notDetermined
        }
        
        // Default to not determined
        return .notDetermined
    }
    
    private func checkBackgroundModes(completion: @escaping (Bool) -> Void) {
        os_log("Checking background modes", log: logger, type: .info)
        
        let enabledModes = getEnabledBackgroundModes()
        let requiredModes = BackgroundMode.allCases.filter { $0.isRequired }
        
        let hasRequired = requiredModes.allSatisfy { mode in
            enabledModes.contains(mode.rawValue)
        }
        
        os_log("Background modes - Required: %@, Enabled: %@, Has all: %@", 
               log: logger, type: .info,
               requiredModes.map { $0.rawValue }.joined(separator: ", "),
               enabledModes.joined(separator: ", "),
               hasRequired ? "YES" : "NO")
        
        completion(hasRequired)
    }
    
    private func getEnabledBackgroundModes() -> [String] {
        guard let infoPlist = Bundle.main.infoDictionary,
              let backgroundModes = infoPlist["UIBackgroundModes"] as? [String] else {
            return []
        }
        
        return backgroundModes
    }
    
    private func hasRequiredBackgroundModes() -> Bool {
        let enabledModes = getEnabledBackgroundModes()
        let requiredModes = BackgroundMode.allCases.filter { $0.isRequired }
        
        return requiredModes.allSatisfy { mode in
            enabledModes.contains(mode.rawValue)
        }
    }
    
    private func handlePermissionRequestTimeout() {
        guard permissionRequestInProgress else { return }
        
        os_log("Permission request timeout", log: logger, type: .error)
        
        permissionRequestInProgress = false
        
        // Notify all pending completion handlers
        let finalStatus = checkPermissionStatus()
        
        for handler in permissionCompletionHandlers {
            handler(finalStatus.isAuthorized)
        }
        
        permissionCompletionHandlers.removeAll()
    }
    
    private func notifyPermissionCompletion() {
        guard !permissionCompletionHandlers.isEmpty else { return }
        
        let status = checkPermissionStatus()
        let granted = status.isAuthorized
        
        os_log("Notifying permission completion: %@", log: logger, type: .info, granted ? "GRANTED" : "DENIED")
        
        for handler in permissionCompletionHandlers {
            handler(granted)
        }
        
        permissionCompletionHandlers.removeAll()
        permissionRequestInProgress = false
    }
    
    private func cleanup() {
        os_log("Cleaning up permission manager", log: logger, type: .info)
        
        permissionCheckTimer?.invalidate()
        systemSettingsMonitor?.invalidate()
        
        saveStateForRestoration()
        
        centralManager?.delegate = nil
        peripheralManager?.delegate = nil
    }
    
    // MARK: - Notification Handlers
    
    @objc private func appWillEnterForeground() {
        os_log("App entering foreground - rechecking permissions", log: logger, type: .info)
        
        // Recheck permissions in case user changed settings
        updateSystemSettings()
        let _ = checkPermissionStatus()
    }
    
    @objc private func appDidEnterBackground() {
        os_log("App entered background - saving state", log: logger, type: .info)
        
        saveStateForRestoration()
    }
    
    @objc private func handlePermissionRequest(_ notification: Notification) {
        guard let permissionType = notification.object as? String else { return }
        
        switch permissionType.lowercased() {
        case "bluetooth":
            requestBluetoothPermissions { success in
                os_log("Permission request result for %@: %@", log: self.logger, type: .info, 
                       permissionType, success ? "GRANTED" : "DENIED")
            }
        default:
            os_log("Unknown permission type requested: %@", log: logger, type: .error, permissionType)
        }
    }
}

// MARK: - CBCentralManagerDelegate

@available(iOS 13.0, *)
extension BitCrapsPermissionManager: CBCentralManagerDelegate {
    
    func centralManagerDidUpdateState(_ central: CBCentralManager) {
        os_log("Central manager state updated: %@", log: logger, type: .info, String(describing: central.state))
        
        // Update permission status
        let _ = checkPermissionStatus()
        
        // Notify completion handlers if both managers have reported
        if peripheralManager?.state != .unknown {
            notifyPermissionCompletion()
        }
    }
}

// MARK: - CBPeripheralManagerDelegate

@available(iOS 13.0, *)
extension BitCrapsPermissionManager: CBPeripheralManagerDelegate {
    
    func peripheralManagerDidUpdateState(_ peripheral: CBPeripheralManager) {
        os_log("Peripheral manager state updated: %@", log: logger, type: .info, String(describing: peripheral.state))
        
        // Update permission status
        let _ = checkPermissionStatus()
        
        // Notify completion handlers if both managers have reported
        if centralManager?.state != .unknown {
            notifyPermissionCompletion()
        }
    }
}