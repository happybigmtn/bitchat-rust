import Foundation
import CoreBluetooth
import os.log

/// Enhanced iOS CoreBluetooth bridge for BitCraps
/// 
/// This implementation provides the complete bridge between Swift CoreBluetooth
/// and the Rust FFI layer, handling all aspects of BLE communication including
/// peripheral management, connection handling, and data transfer.
@available(iOS 13.0, *)
class BitCrapsBluetoothBridge: NSObject, ObservableObject {
    
    // MARK: - Constants
    
    /// BitCraps Service UUID - Must match Rust implementation
    private static let BITCRAPS_SERVICE_UUID = CBUUID(string: "12345678-1234-5678-1234-567812345678")
    private static let BITCRAPS_TX_CHAR_UUID = CBUUID(string: "12345678-1234-5678-1234-567812345679")
    private static let BITCRAPS_RX_CHAR_UUID = CBUUID(string: "12345678-1234-5678-1234-567812345680")
    
    // MARK: - Core Bluetooth Components
    
    private var centralManager: CBCentralManager!
    private var peripheralManager: CBPeripheralManager!
    
    // MARK: - State Management
    
    @Published var isInitialized = false
    @Published var bluetoothState: CBManagerState = .unknown
    @Published var isAdvertising = false
    @Published var isScanning = false
    @Published var connectedPeripherals: [String: CBPeripheral] = [:]
    @Published var discoveredPeripherals: [String: DiscoveredPeripheralInfo] = [:]
    
    // MARK: - Service Management
    
    private var gattService: CBMutableService?
    private var txCharacteristic: CBMutableCharacteristic?
    private var rxCharacteristic: CBMutableCharacteristic?
    
    // MARK: - Background State Management
    
    private var backgroundLimitations: BackgroundLimitationTracker
    private let logger = OSLog(subsystem: "com.bitcraps.ios", category: "BluetoothBridge")
    
    // MARK: - Initialization
    
    override init() {
        self.backgroundLimitations = BackgroundLimitationTracker()
        super.init()
        setupBluetoothManagers()
        initializeRustFFI()
        setupBackgroundObservers()
    }
    
    deinit {
        shutdownRustFFI()
        NotificationCenter.default.removeObserver(self)
    }
    
    // MARK: - Setup Methods
    
    private func setupBluetoothManagers() {
        // Central Manager for scanning and connecting
        centralManager = CBCentralManager(delegate: self, queue: nil, options: [
            CBCentralManagerOptionShowPowerAlertKey: true,
            CBCentralManagerOptionRestoreIdentifierKey: "BitCraps-Central-Bridge"
        ])
        
        // Peripheral Manager for advertising
        peripheralManager = CBPeripheralManager(delegate: self, queue: nil, options: [
            CBPeripheralManagerOptionShowPowerAlertKey: true,
            CBPeripheralManagerOptionRestoreIdentifierKey: "BitCraps-Peripheral-Bridge"
        ])
        
        os_log("CoreBluetooth managers initialized", log: logger, type: .info)
    }
    
    private func initializeRustFFI() {
        // Initialize the Rust FFI layer
        let result = ios_ble_initialize()
        if result == 1 {
            // Set up callbacks
            let eventCallbackResult = ios_ble_set_event_callback { (eventType, eventData, dataLen) in
                BitCrapsBluetoothBridge.handleRustEvent(eventType, eventData, dataLen)
            }
            
            let errorCallbackResult = ios_ble_set_error_callback { (errorMessage) in
                BitCrapsBluetoothBridge.handleRustError(errorMessage)
            }
            
            if eventCallbackResult == 1 && errorCallbackResult == 1 {
                isInitialized = true
                os_log("Rust FFI bridge initialized successfully", log: logger, type: .info)
            } else {
                os_log("Failed to set Rust FFI callbacks", log: logger, type: .error)
            }
        } else {
            os_log("Failed to initialize Rust FFI bridge", log: logger, type: .error)
        }
    }
    
    private func shutdownRustFFI() {
        if isInitialized {
            let _ = ios_ble_shutdown()
            isInitialized = false
            os_log("Rust FFI bridge shutdown", log: logger, type: .info)
        }
    }
    
    private func setupBackgroundObservers() {
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
    }
    
    // MARK: - Public Interface
    
    /// Start BLE advertising
    func startAdvertising() -> Bool {
        guard isInitialized && bluetoothState == .poweredOn else {
            os_log("Cannot start advertising - not ready", log: logger, type: .error)
            return false
        }
        
        guard !isAdvertising else {
            return true // Already advertising
        }
        
        setupGATTServices()
        
        let advertisementData: [String: Any] = [
            CBAdvertisementDataServiceUUIDsKey: [Self.BITCRAPS_SERVICE_UUID],
            CBAdvertisementDataLocalNameKey: "BitCraps-Node"
        ]
        
        peripheralManager.startAdvertising(advertisementData)
        
        // Notify Rust layer
        let _ = ios_ble_start_advertising()
        
        os_log("Started BLE advertising", log: logger, type: .info)
        return true
    }
    
    /// Stop BLE advertising
    func stopAdvertising() -> Bool {
        guard isInitialized else { return false }
        
        peripheralManager.stopAdvertising()
        isAdvertising = false
        
        // Notify Rust layer
        let _ = ios_ble_stop_advertising()
        
        os_log("Stopped BLE advertising", log: logger, type: .info)
        return true
    }
    
    /// Start BLE scanning
    func startScanning() -> Bool {
        guard isInitialized && bluetoothState == .poweredOn else {
            os_log("Cannot start scanning - not ready", log: logger, type: .error)
            return false
        }
        
        guard !isScanning else {
            return true // Already scanning
        }
        
        // Critical: iOS background scanning requires service UUID filtering
        let scanOptions: [String: Any] = [
            CBCentralManagerScanOptionAllowDuplicatesKey: false
        ]
        
        centralManager.scanForPeripherals(
            withServices: [Self.BITCRAPS_SERVICE_UUID],
            options: scanOptions
        )
        
        isScanning = true
        
        // Notify Rust layer
        let _ = ios_ble_start_scanning()
        
        os_log("Started BLE scanning", log: logger, type: .info)
        backgroundLimitations.recordEvent(.scanStarted)
        
        return true
    }
    
    /// Stop BLE scanning
    func stopScanning() -> Bool {
        guard isInitialized else { return false }
        
        centralManager.stopScan()
        isScanning = false
        
        // Notify Rust layer
        let _ = ios_ble_stop_scanning()
        
        os_log("Stopped BLE scanning", log: logger, type: .info)
        return true
    }
    
    /// Connect to a discovered peripheral
    func connectToPeripheral(withIdentifier identifier: String) -> Bool {
        guard isInitialized else { return false }
        
        guard let peripheralInfo = discoveredPeripherals[identifier],
              let peripheral = peripheralInfo.peripheral else {
            os_log("Peripheral not found: %@", log: logger, type: .error, identifier)
            return false
        }
        
        // Check connection limits
        if connectedPeripherals.count >= 8 { // Max connections
            os_log("Maximum connections reached", log: logger, type: .error)
            return false
        }
        
        centralManager.connect(peripheral, options: nil)
        os_log("Initiating connection to: %@", log: logger, type: .info, identifier)
        
        return true
    }
    
    /// Disconnect from a peripheral
    func disconnectFromPeripheral(withIdentifier identifier: String) -> Bool {
        guard isInitialized else { return false }
        
        guard let peripheral = connectedPeripherals[identifier] else {
            os_log("Connected peripheral not found: %@", log: logger, type: .error, identifier)
            return false
        }
        
        centralManager.cancelPeripheralConnection(peripheral)
        os_log("Initiating disconnection from: %@", log: logger, type: .info, identifier)
        
        return true
    }
    
    /// Send data to a connected peripheral
    func sendData(_ data: Data, toPeripheral identifier: String) -> Bool {
        guard isInitialized else { return false }
        
        guard let peripheral = connectedPeripherals[identifier] else {
            os_log("Peripheral not connected: %@", log: logger, type: .error, identifier)
            return false
        }
        
        // Find the TX characteristic for this peripheral
        guard let service = peripheral.services?.first(where: { $0.uuid == Self.BITCRAPS_SERVICE_UUID }),
              let txChar = service.characteristics?.first(where: { $0.uuid == Self.BITCRAPS_TX_CHAR_UUID }) else {
            os_log("TX characteristic not found for peripheral: %@", log: logger, type: .error, identifier)
            return false
        }
        
        peripheral.writeValue(data, for: txChar, type: .withResponse)
        os_log("Sent %d bytes to peripheral: %@", log: logger, type: .debug, data.count, identifier)
        
        return true
    }
    
    // MARK: - GATT Service Setup
    
    private func setupGATTServices() {
        guard gattService == nil else { return } // Already set up
        
        // Create TX characteristic (Central writes to this)
        txCharacteristic = CBMutableCharacteristic(
            type: Self.BITCRAPS_TX_CHAR_UUID,
            properties: [.write, .writeWithoutResponse],
            value: nil,
            permissions: [.writeable]
        )
        
        // Create RX characteristic (Central reads/subscribes to this)
        rxCharacteristic = CBMutableCharacteristic(
            type: Self.BITCRAPS_RX_CHAR_UUID,
            properties: [.read, .notify],
            value: nil,
            permissions: [.readable]
        )
        
        // Create service
        gattService = CBMutableService(type: Self.BITCRAPS_SERVICE_UUID, primary: true)
        gattService?.characteristics = [txCharacteristic!, rxCharacteristic!]
        
        // Add service to peripheral manager
        peripheralManager.add(gattService!)
        
        os_log("GATT service setup completed", log: logger, type: .info)
    }
    
    // MARK: - Background State Management
    
    @objc private func appDidEnterBackground() {
        os_log("App entered background - BLE operations will be limited", log: logger, type: .info)
        backgroundLimitations.recordEvent(.enteredBackground)
        
        // Test background scanning capabilities
        DispatchQueue.main.asyncAfter(deadline: .now() + 2.0) {
            self.testBackgroundCapabilities()
        }
    }
    
    @objc private func appWillEnterForeground() {
        os_log("App entering foreground - full BLE capabilities restored", log: logger, type: .info)
        backgroundLimitations.recordEvent(.enteredForeground)
        backgroundLimitations.generateLimitationReport()
    }
    
    @objc private func appWillTerminate() {
        os_log("App will terminate - cleaning up BLE resources", log: logger, type: .info)
        stopAdvertising()
        stopScanning()
        shutdownRustFFI()
    }
    
    private func testBackgroundCapabilities() {
        // Test if we can still scan in background
        let wasScanning = isScanning
        
        if wasScanning {
            // Restart scanning to test background behavior
            centralManager.stopScan()
            
            DispatchQueue.main.asyncAfter(deadline: .now() + 1.0) {
                self.centralManager.scanForPeripherals(
                    withServices: [Self.BITCRAPS_SERVICE_UUID],
                    options: [CBCentralManagerScanOptionAllowDuplicatesKey: true]
                )
                
                os_log("Background scanning test initiated", log: self.logger, type: .info)
            }
        }
    }
    
    // MARK: - Rust FFI Event Handlers
    
    private static func handleRustEvent(_ eventType: UnsafePointer<CChar>?, 
                                       _ eventData: UnsafeRawPointer?, 
                                       _ dataLen: UInt32) {
        guard let eventType = eventType else { return }
        
        let eventString = String(cString: eventType)
        
        // Handle different event types from Rust
        switch eventString {
        case "advertising_started":
            DispatchQueue.main.async {
                // Update UI state if needed
            }
        case "scanning_started":
            DispatchQueue.main.async {
                // Update UI state if needed
            }
        case "connect_peer":
            if let eventData = eventData, dataLen > 0 {
                let peerIdData = Data(bytes: eventData, count: Int(dataLen))
                if let peerId = String(data: peerIdData, encoding: .utf8) {
                    DispatchQueue.main.async {
                        // Handle peer connection request from Rust
                        NotificationCenter.default.post(name: .rustConnectPeerRequest, 
                                                       object: peerId)
                    }
                }
            }
        case "send_data":
            if let eventData = eventData, dataLen > 0 {
                // Handle data send request from Rust
                // eventData contains SendDataRequest structure
                DispatchQueue.main.async {
                    NotificationCenter.default.post(name: .rustSendDataRequest, 
                                                   object: eventData)
                }
            }
        default:
            os_log("Unknown Rust event: %@", type: .debug, eventString)
        }
    }
    
    private static func handleRustError(_ errorMessage: UnsafePointer<CChar>?) {
        guard let errorMessage = errorMessage else { return }
        
        let errorString = String(cString: errorMessage)
        os_log("Rust FFI Error: %@", type: .error, errorString)
        
        DispatchQueue.main.async {
            NotificationCenter.default.post(name: .rustErrorReceived, 
                                           object: errorString)
        }
    }
    
    // MARK: - Helper Methods
    
    private func notifyRustOfPeerDiscovered(_ peripheral: CBPeripheral, rssi: NSNumber, advertisementData: [String: Any]) {
        let peerIdString = peripheral.identifier.uuidString
        peerIdString.withCString { peerIdCStr in
            let _ = ios_ble_handle_event("peer_discovered", peerIdCStr, UInt32(peerIdString.count))
        }
    }
    
    private func notifyRustOfPeerConnected(_ peripheral: CBPeripheral) {
        let peerIdString = peripheral.identifier.uuidString
        peerIdString.withCString { peerIdCStr in
            let _ = ios_ble_handle_event("peer_connected", peerIdCStr, UInt32(peerIdString.count))
        }
    }
    
    private func notifyRustOfPeerDisconnected(_ peripheral: CBPeripheral) {
        let peerIdString = peripheral.identifier.uuidString
        peerIdString.withCString { peerIdCStr in
            let _ = ios_ble_handle_event("peer_disconnected", peerIdCStr, UInt32(peerIdString.count))
        }
    }
    
    private func notifyRustOfDataReceived(_ data: Data, from peripheral: CBPeripheral) {
        let peerIdString = peripheral.identifier.uuidString
        
        data.withUnsafeBytes { dataBytes in
            peerIdString.withCString { peerIdCStr in
                // Create a combined data structure for peer ID and data
                let combinedData = peerIdString.data(using: .utf8)! + Data([0]) + data // null separator
                
                combinedData.withUnsafeBytes { combinedBytes in
                    let _ = ios_ble_handle_event("data_received", combinedBytes.baseAddress, UInt32(combinedData.count))
                }
            }
        }
    }
    
    private func notifyRustOfBluetoothStateChange(_ state: CBManagerState) {
        let stateValue = state == .poweredOn ? 1 : 0
        let _ = ios_ble_handle_event("bluetooth_state_changed", &stateValue, UInt32(MemoryLayout<Int>.size))
    }
}

// MARK: - CBCentralManagerDelegate

@available(iOS 13.0, *)
extension BitCrapsBluetoothBridge: CBCentralManagerDelegate {
    
    func centralManagerDidUpdateState(_ central: CBCentralManager) {
        bluetoothState = central.state
        
        switch central.state {
        case .poweredOn:
            os_log("Bluetooth powered on", log: logger, type: .info)
        case .poweredOff:
            os_log("Bluetooth powered off", log: logger, type: .error)
            isScanning = false
            connectedPeripherals.removeAll()
        case .resetting:
            os_log("Bluetooth resetting", log: logger, type: .info)
        case .unauthorized:
            os_log("Bluetooth unauthorized", log: logger, type: .error)
        case .unsupported:
            os_log("Bluetooth unsupported", log: logger, type: .error)
        case .unknown:
            os_log("Bluetooth state unknown", log: logger, type: .info)
        @unknown default:
            os_log("Unknown Bluetooth state", log: logger, type: .error)
        }
        
        // Notify Rust layer of state change
        notifyRustOfBluetoothStateChange(central.state)
    }
    
    func centralManager(_ central: CBCentralManager, didDiscover peripheral: CBPeripheral, advertisementData: [String : Any], rssi RSSI: NSNumber) {
        
        let peripheralId = peripheral.identifier.uuidString
        
        os_log("Discovered peripheral: %@ (RSSI: %@)", log: logger, type: .info, peripheralId, RSSI)
        
        // Store discovered peripheral info
        let peripheralInfo = DiscoveredPeripheralInfo(
            peripheral: peripheral,
            rssi: RSSI.intValue,
            lastSeen: Date(),
            advertisementData: advertisementData
        )
        
        discoveredPeripherals[peripheralId] = peripheralInfo
        backgroundLimitations.recordEvent(.deviceDiscovered)
        
        // Log advertisement data analysis
        if let localName = advertisementData[CBAdvertisementDataLocalNameKey] as? String {
            os_log("Local name: %@ (foreground only)", log: logger, type: .debug, localName)
        } else {
            os_log("No local name (background limitation)", log: logger, type: .debug)
        }
        
        if let serviceUUIDs = advertisementData[CBAdvertisementDataServiceUUIDsKey] as? [CBUUID] {
            os_log("Services: %@", log: logger, type: .debug, serviceUUIDs.map { $0.uuidString })
        }
        
        if let overflowUUIDs = advertisementData[CBAdvertisementDataOverflowServiceUUIDsKey] as? [CBUUID] {
            os_log("Overflow services: %@ (background location)", log: logger, type: .debug, overflowUUIDs.map { $0.uuidString })
        }
        
        // Notify Rust layer
        notifyRustOfPeerDiscovered(peripheral, rssi: RSSI, advertisementData: advertisementData)
    }
    
    func centralManager(_ central: CBCentralManager, didConnect peripheral: CBPeripheral) {
        let peripheralId = peripheral.identifier.uuidString
        os_log("Connected to peripheral: %@", log: logger, type: .info, peripheralId)
        
        peripheral.delegate = self
        connectedPeripherals[peripheralId] = peripheral
        
        // Discover services
        peripheral.discoverServices([Self.BITCRAPS_SERVICE_UUID])
        
        // Notify Rust layer
        notifyRustOfPeerConnected(peripheral)
    }
    
    func centralManager(_ central: CBCentralManager, didFailToConnect peripheral: CBPeripheral, error: Error?) {
        let peripheralId = peripheral.identifier.uuidString
        os_log("Failed to connect to peripheral: %@ - %@", log: logger, type: .error, 
               peripheralId, error?.localizedDescription ?? "Unknown error")
    }
    
    func centralManager(_ central: CBCentralManager, didDisconnectPeripheral peripheral: CBPeripheral, error: Error?) {
        let peripheralId = peripheral.identifier.uuidString
        os_log("Disconnected from peripheral: %@", log: logger, type: .info, peripheralId)
        
        connectedPeripherals.removeValue(forKey: peripheralId)
        
        // Notify Rust layer
        notifyRustOfPeerDisconnected(peripheral)
    }
    
    func centralManager(_ central: CBCentralManager, willRestoreState dict: [String : Any]) {
        os_log("Central manager will restore state", log: logger, type: .info)
        
        // Handle state restoration for background operation
        if let peripherals = dict[CBCentralManagerRestoredStatePeripheralsKey] as? [CBPeripheral] {
            for peripheral in peripherals {
                let peripheralId = peripheral.identifier.uuidString
                connectedPeripherals[peripheralId] = peripheral
                peripheral.delegate = self
            }
            os_log("Restored %d peripheral connections", log: logger, type: .info, peripherals.count)
        }
        
        if let scanServices = dict[CBCentralManagerRestoredStateScanServicesKey] as? [CBUUID] {
            os_log("Restored scan services: %@", log: logger, type: .info, scanServices.map { $0.uuidString })
            isScanning = true
        }
    }
}

// MARK: - CBPeripheralDelegate

@available(iOS 13.0, *)
extension BitCrapsBluetoothBridge: CBPeripheralDelegate {
    
    func peripheral(_ peripheral: CBPeripheral, didDiscoverServices error: Error?) {
        guard error == nil else {
            os_log("Service discovery failed: %@", log: logger, type: .error, error!.localizedDescription)
            return
        }
        
        guard let services = peripheral.services else { return }
        
        for service in services {
            if service.uuid == Self.BITCRAPS_SERVICE_UUID {
                os_log("Discovered BitCraps service on: %@", log: logger, type: .info, peripheral.identifier.uuidString)
                peripheral.discoverCharacteristics([Self.BITCRAPS_TX_CHAR_UUID, Self.BITCRAPS_RX_CHAR_UUID], for: service)
            }
        }
    }
    
    func peripheral(_ peripheral: CBPeripheral, didDiscoverCharacteristicsFor service: CBService, error: Error?) {
        guard error == nil else {
            os_log("Characteristic discovery failed: %@", log: logger, type: .error, error!.localizedDescription)
            return
        }
        
        guard let characteristics = service.characteristics else { return }
        
        for characteristic in characteristics {
            switch characteristic.uuid {
            case Self.BITCRAPS_TX_CHAR_UUID:
                os_log("Discovered TX characteristic on: %@", log: logger, type: .info, peripheral.identifier.uuidString)
                
            case Self.BITCRAPS_RX_CHAR_UUID:
                os_log("Discovered RX characteristic on: %@", log: logger, type: .info, peripheral.identifier.uuidString)
                // Subscribe for notifications
                peripheral.setNotifyValue(true, for: characteristic)
                
            default:
                break
            }
        }
    }
    
    func peripheral(_ peripheral: CBPeripheral, didUpdateValueFor characteristic: CBCharacteristic, error: Error?) {
        guard error == nil, let data = characteristic.value, !data.isEmpty else {
            os_log("Characteristic read failed: %@", log: logger, type: .error, error?.localizedDescription ?? "No data")
            return
        }
        
        os_log("Received %d bytes from: %@", log: logger, type: .debug, data.count, peripheral.identifier.uuidString)
        
        // Notify Rust layer of received data
        notifyRustOfDataReceived(data, from: peripheral)
    }
    
    func peripheral(_ peripheral: CBPeripheral, didWriteValueFor characteristic: CBCharacteristic, error: Error?) {
        if let error = error {
            os_log("Characteristic write failed: %@", log: logger, type: .error, error.localizedDescription)
        } else {
            os_log("Successfully wrote to characteristic on: %@", log: logger, type: .debug, peripheral.identifier.uuidString)
        }
    }
}

// MARK: - CBPeripheralManagerDelegate

@available(iOS 13.0, *)
extension BitCrapsBluetoothBridge: CBPeripheralManagerDelegate {
    
    func peripheralManagerDidUpdateState(_ peripheral: CBPeripheralManager) {
        switch peripheral.state {
        case .poweredOn:
            os_log("Peripheral manager powered on", log: logger, type: .info)
        case .poweredOff:
            os_log("Peripheral manager powered off", log: logger, type: .error)
            isAdvertising = false
        case .unauthorized:
            os_log("Peripheral manager unauthorized", log: logger, type: .error)
        case .unsupported:
            os_log("Peripheral manager unsupported", log: logger, type: .error)
        default:
            os_log("Peripheral manager state: %@", log: logger, type: .info, "\(peripheral.state.rawValue)")
        }
    }
    
    func peripheralManagerDidStartAdvertising(_ peripheral: CBPeripheralManager, error: Error?) {
        if let error = error {
            os_log("Failed to start advertising: %@", log: logger, type: .error, error.localizedDescription)
            isAdvertising = false
        } else {
            os_log("Successfully started advertising", log: logger, type: .info)
            isAdvertising = true
            backgroundLimitations.recordEvent(.advertisingStarted)
        }
    }
    
    func peripheralManager(_ peripheral: CBPeripheralManager, didAdd service: CBService, error: Error?) {
        if let error = error {
            os_log("Failed to add service: %@", log: logger, type: .error, error.localizedDescription)
        } else {
            os_log("Service added successfully: %@", log: logger, type: .info, service.uuid.uuidString)
        }
    }
    
    func peripheralManager(_ peripheral: CBPeripheralManager, didReceiveWrite requests: [CBATTRequest]) {
        for request in requests {
            os_log("Received write request: %d bytes from: %@", log: logger, type: .debug, 
                   request.value?.count ?? 0, request.central.identifier.uuidString)
            
            if let data = request.value, !data.isEmpty {
                // Notify Rust layer of received data
                let centralId = request.central.identifier.uuidString
                let combinedData = centralId.data(using: .utf8)! + Data([0]) + data // null separator
                
                combinedData.withUnsafeBytes { bytes in
                    let _ = ios_ble_handle_event("data_received", bytes.baseAddress, UInt32(combinedData.count))
                }
            }
            
            // Respond to the request
            peripheral.respond(to: request, withResult: .success)
        }
    }
    
    func peripheralManager(_ peripheral: CBPeripheralManager, central: CBCentral, didSubscribeToCharacteristic characteristic: CBCharacteristic) {
        os_log("Central subscribed to characteristic: %@", log: logger, type: .info, central.identifier.uuidString)
    }
    
    func peripheralManager(_ peripheral: CBPeripheralManager, central: CBCentral, didUnsubscribeFromCharacteristic characteristic: CBCharacteristic) {
        os_log("Central unsubscribed from characteristic: %@", log: logger, type: .info, central.identifier.uuidString)
    }
}

// MARK: - Supporting Types

struct DiscoveredPeripheralInfo {
    weak var peripheral: CBPeripheral?
    let rssi: Int
    let lastSeen: Date
    let advertisementData: [String: Any]
}

class BackgroundLimitationTracker {
    private var events: [(BackgroundTestEvent, Date)] = []
    private var backgroundDiscoveryCount = 0
    private var foregroundDiscoveryCount = 0
    private var isInBackground = false
    
    enum BackgroundTestEvent {
        case scanStarted
        case advertisingStarted
        case enteredBackground
        case enteredForeground
        case deviceDiscovered
    }
    
    func recordEvent(_ event: BackgroundTestEvent) {
        events.append((event, Date()))
        
        switch event {
        case .enteredBackground:
            isInBackground = true
        case .enteredForeground:
            isInBackground = false
        case .deviceDiscovered:
            if isInBackground {
                backgroundDiscoveryCount += 1
            } else {
                foregroundDiscoveryCount += 1
            }
        default:
            break
        }
    }
    
    func generateLimitationReport() {
        let backgroundRate = backgroundDiscoveryCount > 0 ? Double(backgroundDiscoveryCount) / 60.0 : 0.0
        let foregroundRate = foregroundDiscoveryCount > 0 ? Double(foregroundDiscoveryCount) / 60.0 : 0.0
        
        os_log("Background BLE Limitation Report:", type: .info)
        os_log("- Background discoveries: %d (%.2f/min)", type: .info, backgroundDiscoveryCount, backgroundRate * 60)
        os_log("- Foreground discoveries: %d (%.2f/min)", type: .info, foregroundDiscoveryCount, foregroundRate * 60)
        
        if backgroundDiscoveryCount == 0 && foregroundDiscoveryCount > 0 {
            os_log("WARNING: Background discovery completely blocked", type: .error)
        } else if backgroundRate < foregroundRate * 0.1 {
            os_log("WARNING: Background discovery severely limited (< 10%% of foreground rate)", type: .error)
        }
    }
}

// MARK: - Notification Extensions

extension Notification.Name {
    static let rustConnectPeerRequest = Notification.Name("rustConnectPeerRequest")
    static let rustSendDataRequest = Notification.Name("rustSendDataRequest")
    static let rustErrorReceived = Notification.Name("rustErrorReceived")
}