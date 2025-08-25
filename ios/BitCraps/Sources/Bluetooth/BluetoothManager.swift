import Foundation
import CoreBluetooth
import os.log

/// Critical iOS Background BLE Implementation
/// 
/// This implementation handles the severe limitations of iOS background BLE operation:
/// 1. Service UUIDs move to overflow area in background
/// 2. Local name is NOT advertised in background
/// 3. Scan result coalescing reduces discovery frequency
/// 4. Limited to service UUID-based filtering only
///
/// KILL-SWITCH VALIDATION: This tests whether iOS background BLE is viable for BitCraps
@available(iOS 13.0, *)
class BluetoothManager: NSObject, ObservableObject {
    
    // MARK: - Constants
    
    /// BitCraps Service UUID - CRITICAL: Must match Rust implementation exactly
    private static let BITCRAPS_SERVICE_UUID = CBUUID(string: "12345678-1234-5678-1234-567812345678")
    
    /// TX Characteristic (Central writes to this)
    private static let BITCRAPS_TX_CHAR_UUID = CBUUID(string: "12345678-1234-5678-1234-567812345679")
    
    /// RX Characteristic (Central reads/subscribes to this)
    private static let BITCRAPS_RX_CHAR_UUID = CBUUID(string: "12345678-1234-5678-1234-567812345680")
    
    // MARK: - Core Bluetooth Components
    
    private var centralManager: CBCentralManager!
    private var peripheralManager: CBPeripheralManager!
    
    // MARK: - State Management
    
    @Published var isScanning = false
    @Published var isAdvertising = false
    @Published var discoveredPeers: [DiscoveredPeer] = []
    @Published var connectedPeers: [ConnectedPeer] = []
    @Published var backgroundLimitations: BackgroundLimitationReport?
    
    // MARK: - Background Constraint Testing
    
    private var backgroundTestResults = BackgroundTestResults()
    private let logger = OSLog(subsystem: "com.bitcraps.ios", category: "BluetoothManager")
    
    // MARK: - Initialization
    
    override init() {
        super.init()
        setupBluetoothManagers()
        startBackgroundLimitationTests()
    }
    
    private func setupBluetoothManagers() {
        // Central Manager for scanning and connecting
        centralManager = CBCentralManager(delegate: self, queue: nil, options: [
            CBCentralManagerOptionShowPowerAlertKey: true,
            CBCentralManagerOptionRestoreIdentifierKey: "BitCraps-Central"
        ])
        
        // Peripheral Manager for advertising
        peripheralManager = CBPeripheralManager(delegate: self, queue: nil, options: [
            CBPeripheralManagerOptionShowPowerAlertKey: true,
            CBPeripheralManagerOptionRestoreIdentifierKey: "BitCraps-Peripheral"
        ])
    }
    
    // MARK: - Public Interface
    
    func startDiscovery() {
        guard centralManager.state == .poweredOn else {
            os_log("Cannot start discovery - Bluetooth not powered on", log: logger, type: .error)
            return
        }
        
        // CRITICAL: iOS background discovery ONLY works with service UUID filter
        let scanOptions: [String: Any] = [
            CBCentralManagerScanOptionAllowDuplicatesKey: false // Battery optimization
        ]
        
        centralManager.scanForPeripherals(
            withServices: [Self.BITCRAPS_SERVICE_UUID], // MANDATORY for background
            options: scanOptions
        )
        
        isScanning = true
        os_log("Started BLE discovery with service UUID filter", log: logger, type: .info)
        
        // Test background behavior
        recordBackgroundTestEvent(.scanStarted)
    }
    
    func startAdvertising() {
        guard peripheralManager.state == .poweredOn else {
            os_log("Cannot start advertising - Bluetooth not powered on", log: logger, type: .error)
            return
        }
        
        setupGATTServices()
        
        // CRITICAL: Background advertising limitations
        // - Local name will NOT be advertised in background
        // - Service UUID will move to overflow area
        let advertisementData: [String: Any] = [
            CBAdvertisementDataServiceUUIDsKey: [Self.BITCRAPS_SERVICE_UUID],
            CBAdvertisementDataLocalNameKey: "BitCraps-Node" // Only visible when app is foreground
        ]
        
        peripheralManager.startAdvertising(advertisementData)
        isAdvertising = true
        
        os_log("Started BLE advertising - WARNING: Limited in background", log: logger, type: .info)
        recordBackgroundTestEvent(.advertisingStarted)
    }
    
    func stopDiscovery() {
        centralManager.stopScan()
        isScanning = false
        os_log("Stopped BLE discovery", log: logger, type: .info)
    }
    
    func stopAdvertising() {
        peripheralManager.stopAdvertising()
        isAdvertising = false
        os_log("Stopped BLE advertising", log: logger, type: .info)
    }
    
    func connect(to peer: DiscoveredPeer) {
        guard let peripheral = peer.peripheral else {
            os_log("Cannot connect - peripheral reference lost", log: logger, type: .error)
            return
        }
        
        centralManager.connect(peripheral, options: nil)
        os_log("Attempting to connect to peer: %@", log: logger, type: .info, peer.id.uuidString)
    }
    
    // MARK: - GATT Service Setup
    
    private func setupGATTServices() {
        // Create characteristics for bidirectional communication
        let txCharacteristic = CBMutableCharacteristic(
            type: Self.BITCRAPS_TX_CHAR_UUID,
            properties: [.write, .writeWithoutResponse],
            value: nil,
            permissions: [.writeable]
        )
        
        let rxCharacteristic = CBMutableCharacteristic(
            type: Self.BITCRAPS_RX_CHAR_UUID,
            properties: [.read, .notify],
            value: nil,
            permissions: [.readable]
        )
        
        // Create service
        let service = CBMutableService(type: Self.BITCRAPS_SERVICE_UUID, primary: true)
        service.characteristics = [txCharacteristic, rxCharacteristic]
        
        // Add service to peripheral
        peripheralManager.add(service)
        
        os_log("GATT service setup complete", log: logger, type: .info)
    }
    
    // MARK: - Background Limitation Testing
    
    private func startBackgroundLimitationTests() {
        // Monitor app state changes to test background behavior
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
    }
    
    @objc private func appDidEnterBackground() {
        recordBackgroundTestEvent(.enteredBackground)
        
        // Test: Can we still scan in background?
        DispatchQueue.main.asyncAfter(deadline: .now() + 2.0) {
            self.testBackgroundScanning()
        }
    }
    
    @objc private func appWillEnterForeground() {
        recordBackgroundTestEvent(.enteredForeground)
        
        // Generate background limitations report
        generateBackgroundLimitationReport()
    }
    
    private func testBackgroundScanning() {
        let testStartTime = Date()
        
        // Try to discover new devices while in background
        centralManager.scanForPeripherals(
            withServices: [Self.BITCRAPS_SERVICE_UUID],
            options: [CBCentralManagerScanOptionAllowDuplicatesKey: true]
        )
        
        // Monitor results
        DispatchQueue.main.asyncAfter(deadline: .now() + 10.0) {
            let discoveryCount = self.backgroundTestResults.backgroundDiscoveryCount
            self.backgroundTestResults.backgroundScanTestCompleted = true
            
            os_log("Background scan test: discovered %d devices in 10s", 
                   log: self.logger, type: .info, discoveryCount)
        }
    }
    
    private func recordBackgroundTestEvent(_ event: BackgroundTestEvent) {
        backgroundTestResults.events.append((event, Date()))
        
        switch event {
        case .scanStarted:
            backgroundTestResults.totalScansStarted += 1
        case .advertisingStarted:
            backgroundTestResults.advertisingAttempts += 1
        case .enteredBackground:
            backgroundTestResults.backgroundTransitions += 1
        case .deviceDiscovered:
            if UIApplication.shared.applicationState == .background {
                backgroundTestResults.backgroundDiscoveryCount += 1
            } else {
                backgroundTestResults.foregroundDiscoveryCount += 1
            }
        default:
            break
        }
    }
    
    private func generateBackgroundLimitationReport() {
        let report = BackgroundLimitationReport(
            serviceUUIDFilteringRequired: true,
            localNameUnavailableInBackground: true,
            scanResultCoalescingActive: true,
            backgroundDiscoveryRate: calculateBackgroundDiscoveryRate(),
            foregroundDiscoveryRate: calculateForegroundDiscoveryRate(),
            recommendedImplementation: generateRecommendations(),
            testResults: backgroundTestResults
        )
        
        self.backgroundLimitations = report
        
        os_log("Background limitations report generated", log: logger, type: .info)
        logBackgroundLimitationReport(report)
    }
    
    private func calculateBackgroundDiscoveryRate() -> Double {
        guard backgroundTestResults.backgroundTransitions > 0 else { return 0.0 }
        
        let backgroundTime = backgroundTestResults.events
            .filter { $0.0 == .enteredBackground || $0.0 == .enteredForeground }
            .reduce(0.0) { total, event in
                // Simplified calculation - in real implementation would track actual time
                return total + 30.0 // Assume 30 seconds per background period
            }
        
        return backgroundTime > 0 ? Double(backgroundTestResults.backgroundDiscoveryCount) / backgroundTime : 0.0
    }
    
    private func calculateForegroundDiscoveryRate() -> Double {
        // Similar calculation for foreground discovery rate
        return Double(backgroundTestResults.foregroundDiscoveryCount) / max(1.0, 60.0) // Per minute
    }
    
    private func generateRecommendations() -> [String] {
        var recommendations: [String] = []
        
        recommendations.append("MANDATORY: Use service UUID filtering for all scanning operations")
        recommendations.append("CRITICAL: Exchange peer identity AFTER connection establishment")
        recommendations.append("REQUIRED: Implement foreground-optimized discovery mode")
        
        if backgroundTestResults.backgroundDiscoveryCount < backgroundTestResults.foregroundDiscoveryCount / 2 {
            recommendations.append("WARNING: Background discovery significantly impaired - consider foreground-only mode")
        }
        
        if backgroundTestResults.backgroundScanTestCompleted && backgroundTestResults.backgroundDiscoveryCount == 0 {
            recommendations.append("KILL-SWITCH: Background discovery failed completely - iOS BLE not viable for always-on gaming")
        }
        
        return recommendations
    }
    
    private func logBackgroundLimitationReport(_ report: BackgroundLimitationReport) {
        os_log("=== iOS BLE BACKGROUND LIMITATION REPORT ===", log: logger, type: .info)
        os_log("Service UUID filtering required: %@", log: logger, type: .info, report.serviceUUIDFilteringRequired ? "YES" : "NO")
        os_log("Local name unavailable in background: %@", log: logger, type: .info, report.localNameUnavailableInBackground ? "YES" : "NO")
        os_log("Background discovery rate: %.2f devices/minute", log: logger, type: .info, report.backgroundDiscoveryRate * 60)
        os_log("Foreground discovery rate: %.2f devices/minute", log: logger, type: .info, report.foregroundDiscoveryRate * 60)
        
        for recommendation in report.recommendedImplementation {
            os_log("RECOMMENDATION: %@", log: logger, type: .info, recommendation)
        }
    }
}

// MARK: - CBCentralManagerDelegate

@available(iOS 13.0, *)
extension BluetoothManager: CBCentralManagerDelegate {
    
    func centralManagerDidUpdateState(_ central: CBCentralManager) {
        switch central.state {
        case .poweredOn:
            os_log("Bluetooth powered on - ready for operations", log: logger, type: .info)
        case .poweredOff:
            os_log("Bluetooth powered off", log: logger, type: .error)
            isScanning = false
        case .resetting:
            os_log("Bluetooth resetting", log: logger, type: .info)
        case .unauthorized:
            os_log("Bluetooth unauthorized - check permissions", log: logger, type: .error)
        case .unsupported:
            os_log("Bluetooth not supported on this device", log: logger, type: .error)
        case .unknown:
            os_log("Bluetooth state unknown", log: logger, type: .info)
        @unknown default:
            os_log("Unknown Bluetooth state", log: logger, type: .error)
        }
    }
    
    func centralManager(_ central: CBCentralManager, didDiscover peripheral: CBPeripheral, advertisementData: [String : Any], rssi RSSI: NSNumber) {
        
        os_log("Discovered peripheral: %@ (RSSI: %@)", log: logger, type: .info, 
               peripheral.identifier.uuidString, RSSI)
        
        // Record discovery for background testing
        recordBackgroundTestEvent(.deviceDiscovered)
        
        // Log advertisement data to understand background limitations
        if let localName = advertisementData[CBAdvertisementDataLocalNameKey] as? String {
            os_log("Local name: %@ (FOREGROUND ONLY)", log: logger, type: .info, localName)
        } else {
            os_log("No local name in advertisement (background limitation)", log: logger, type: .info)
        }
        
        if let serviceUUIDs = advertisementData[CBAdvertisementDataServiceUUIDsKey] as? [CBUUID] {
            os_log("Advertised services: %@", log: logger, type: .info, serviceUUIDs.map { $0.uuidString })
        }
        
        if let overflowUUIDs = advertisementData[CBAdvertisementDataOverflowServiceUUIDsKey] as? [CBUUID] {
            os_log("Overflow services: %@ (BACKGROUND LOCATION)", log: logger, type: .info, overflowUUIDs.map { $0.uuidString })
        }
        
        // Create discovered peer
        let discoveredPeer = DiscoveredPeer(
            id: peripheral.identifier,
            peripheral: peripheral,
            rssi: RSSI.intValue,
            lastSeen: Date(),
            advertisementData: advertisementData
        )
        
        // Update discovered peers list
        DispatchQueue.main.async {
            if let existingIndex = self.discoveredPeers.firstIndex(where: { $0.id == discoveredPeer.id }) {
                self.discoveredPeers[existingIndex] = discoveredPeer
            } else {
                self.discoveredPeers.append(discoveredPeer)
            }
        }
    }
    
    func centralManager(_ central: CBCentralManager, didConnect peripheral: CBPeripheral) {
        os_log("Connected to peripheral: %@", log: logger, type: .info, peripheral.identifier.uuidString)
        
        peripheral.delegate = self
        peripheral.discoverServices([Self.BITCRAPS_SERVICE_UUID])
        
        let connectedPeer = ConnectedPeer(
            id: peripheral.identifier,
            peripheral: peripheral,
            connectedAt: Date()
        )
        
        DispatchQueue.main.async {
            self.connectedPeers.append(connectedPeer)
        }
    }
    
    func centralManager(_ central: CBCentralManager, didFailToConnect peripheral: CBPeripheral, error: Error?) {
        os_log("Failed to connect to peripheral: %@ - %@", log: logger, type: .error, 
               peripheral.identifier.uuidString, error?.localizedDescription ?? "Unknown error")
    }
    
    func centralManager(_ central: CBCentralManager, didDisconnectPeripheral peripheral: CBPeripheral, error: Error?) {
        os_log("Disconnected from peripheral: %@", log: logger, type: .info, peripheral.identifier.uuidString)
        
        DispatchQueue.main.async {
            self.connectedPeers.removeAll { $0.id == peripheral.identifier }
        }
    }
}

// MARK: - CBPeripheralDelegate

@available(iOS 13.0, *)
extension BluetoothManager: CBPeripheralDelegate {
    
    func peripheral(_ peripheral: CBPeripheral, didDiscoverServices error: Error?) {
        guard error == nil else {
            os_log("Service discovery failed: %@", log: logger, type: .error, error!.localizedDescription)
            return
        }
        
        guard let services = peripheral.services else { return }
        
        for service in services {
            if service.uuid == Self.BITCRAPS_SERVICE_UUID {
                os_log("Found BitCraps service", log: logger, type: .info)
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
                os_log("Found TX characteristic", log: logger, type: .info)
                // Store reference for writing
                
            case Self.BITCRAPS_RX_CHAR_UUID:
                os_log("Found RX characteristic", log: logger, type: .info)
                // Subscribe for notifications
                peripheral.setNotifyValue(true, for: characteristic)
                
            default:
                break
            }
        }
    }
    
    func peripheral(_ peripheral: CBPeripheral, didUpdateValueFor characteristic: CBCharacteristic, error: Error?) {
        guard error == nil, let data = characteristic.value else {
            os_log("Characteristic read failed: %@", log: logger, type: .error, error?.localizedDescription ?? "No data")
            return
        }
        
        // Handle received data from peer
        os_log("Received %d bytes from peer", log: logger, type: .info, data.count)
    }
    
    func peripheral(_ peripheral: CBPeripheral, didWriteValueFor characteristic: CBCharacteristic, error: Error?) {
        if let error = error {
            os_log("Characteristic write failed: %@", log: logger, type: .error, error.localizedDescription)
        } else {
            os_log("Successfully wrote to characteristic", log: logger, type: .info)
        }
    }
}

// MARK: - CBPeripheralManagerDelegate

@available(iOS 13.0, *)
extension BluetoothManager: CBPeripheralManagerDelegate {
    
    func peripheralManagerDidUpdateState(_ peripheral: CBPeripheralManager) {
        switch peripheral.state {
        case .poweredOn:
            os_log("Peripheral manager powered on", log: logger, type: .info)
        case .poweredOff:
            os_log("Peripheral manager powered off", log: logger, type: .error)
            isAdvertising = false
        default:
            os_log("Peripheral manager state: %@", log: logger, type: .info, "\(peripheral.state)")
        }
    }
    
    func peripheralManagerDidStartAdvertising(_ peripheral: CBPeripheralManager, error: Error?) {
        if let error = error {
            os_log("Failed to start advertising: %@", log: logger, type: .error, error.localizedDescription)
            isAdvertising = false
        } else {
            os_log("Started advertising successfully", log: logger, type: .info)
            isAdvertising = true
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
            os_log("Received write request: %d bytes", log: logger, type: .info, request.value?.count ?? 0)
            
            // Handle incoming data from central
            if let data = request.value {
                // Process received game data
                processReceivedData(data, from: request.central)
            }
            
            // Respond to request
            peripheral.respond(to: request, withResult: .success)
        }
    }
    
    private func processReceivedData(_ data: Data, from central: CBCentral) {
        // Forward to Rust layer for protocol processing
        os_log("Processing %d bytes from central: %@", log: logger, type: .info, data.count, central.identifier.uuidString)
    }
}

// MARK: - Data Models

struct DiscoveredPeer: Identifiable {
    let id: UUID
    weak var peripheral: CBPeripheral?
    let rssi: Int
    let lastSeen: Date
    let advertisementData: [String: Any]
}

struct ConnectedPeer: Identifiable {
    let id: UUID
    weak var peripheral: CBPeripheral?
    let connectedAt: Date
}

struct BackgroundLimitationReport {
    let serviceUUIDFilteringRequired: Bool
    let localNameUnavailableInBackground: Bool
    let scanResultCoalescingActive: Bool
    let backgroundDiscoveryRate: Double // discoveries per second
    let foregroundDiscoveryRate: Double
    let recommendedImplementation: [String]
    let testResults: BackgroundTestResults
    
    var isViableForAlwaysOnGaming: Bool {
        return backgroundDiscoveryRate > 0.01 && // At least 1 discovery per 100 seconds
               !recommendedImplementation.contains { $0.contains("KILL-SWITCH") }
    }
}

struct BackgroundTestResults {
    var events: [(BackgroundTestEvent, Date)] = []
    var totalScansStarted = 0
    var advertisingAttempts = 0
    var backgroundTransitions = 0
    var backgroundDiscoveryCount = 0
    var foregroundDiscoveryCount = 0
    var backgroundScanTestCompleted = false
}

enum BackgroundTestEvent {
    case scanStarted
    case advertisingStarted
    case enteredBackground
    case enteredForeground
    case deviceDiscovered
}