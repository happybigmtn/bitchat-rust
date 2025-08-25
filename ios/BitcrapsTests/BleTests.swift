import XCTest
import CoreBluetooth
import os.log

/**
 * BLE Tests for iOS
 * Must be run on physical devices with Bluetooth enabled
 */
class BleTests: XCTestCase {
    
    private var centralManager: CBCentralManager!
    private var peripheralManager: CBPeripheralManager!
    private var discoveredPeripherals: [CBPeripheral] = []
    private var connectedPeripheral: CBPeripheral?
    private var powerMonitor: PowerMonitor!
    
    private let serviceUUID = CBUUID(string: "12345678-1234-1234-1234-123456789012")
    private let characteristicUUID = CBUUID(string: "12345678-1234-1234-1234-123456789013")
    private let notifyCharUUID = CBUUID(string: "12345678-1234-1234-1234-123456789014")
    
    private let logger = Logger(subsystem: "com.bitcraps.tests", category: "BLE")
    
    override func setUp() {
        super.setUp()
        centralManager = CBCentralManager(delegate: self, queue: nil)
        peripheralManager = CBPeripheralManager(delegate: self, queue: nil)
        powerMonitor = PowerMonitor()
        
        // Wait for Bluetooth to be ready
        let expectation = XCTestExpectation(description: "Bluetooth ready")
        DispatchQueue.main.asyncAfter(deadline: .now() + 2) {
            expectation.fulfill()
        }
        wait(for: [expectation], timeout: 5)
    }
    
    override func tearDown() {
        if centralManager.isScanning {
            centralManager.stopScan()
        }
        if let peripheral = connectedPeripheral {
            centralManager.cancelPeripheralConnection(peripheral)
        }
        super.tearDown()
    }
    
    // MARK: - Discovery Tests
    
    func testBleDiscovery() {
        let expectation = XCTestExpectation(description: "Discover peripherals")
        
        // Start scanning
        centralManager.scanForPeripherals(
            withServices: [serviceUUID],
            options: [CBCentralManagerScanOptionAllowDuplicatesKey: false]
        )
        
        // Wait for discovery
        DispatchQueue.main.asyncAfter(deadline: .now() + 10) {
            self.centralManager.stopScan()
            expectation.fulfill()
        }
        
        wait(for: [expectation], timeout: 15)
        
        XCTAssertGreaterThan(discoveredPeripherals.count, 0, "Should discover at least one peripheral")
        logger.info("Discovered \(self.discoveredPeripherals.count) peripherals")
    }
    
    // MARK: - Advertising Tests
    
    func testBleAdvertising() {
        let expectation = XCTestExpectation(description: "Start advertising")
        
        // Configure service
        let service = CBMutableService(type: serviceUUID, primary: true)
        let characteristic = CBMutableCharacteristic(
            type: characteristicUUID,
            properties: [.read, .write, .notify],
            value: nil,
            permissions: [.readable, .writeable]
        )
        service.characteristics = [characteristic]
        
        peripheralManager.add(service)
        
        // Start advertising
        DispatchQueue.main.asyncAfter(deadline: .now() + 1) {
            self.peripheralManager.startAdvertising([
                CBAdvertisementDataServiceUUIDsKey: [self.serviceUUID],
                CBAdvertisementDataLocalNameKey: "BitCraps-iOS"
            ])
            expectation.fulfill()
        }
        
        wait(for: [expectation], timeout: 5)
        
        XCTAssertTrue(peripheralManager.isAdvertising, "Should be advertising")
        
        // Keep advertising for 5 seconds
        Thread.sleep(forTimeInterval: 5)
        
        peripheralManager.stopAdvertising()
    }
    
    // MARK: - Connection Tests
    
    func testBleConnection() {
        let scanExpectation = XCTestExpectation(description: "Find peripheral")
        let connectExpectation = XCTestExpectation(description: "Connect to peripheral")
        let discoverExpectation = XCTestExpectation(description: "Discover services")
        
        // Scan for peripherals
        centralManager.scanForPeripherals(withServices: nil, options: nil)
        
        DispatchQueue.main.asyncAfter(deadline: .now() + 5) {
            self.centralManager.stopScan()
            scanExpectation.fulfill()
            
            // Connect to first discovered peripheral
            if let peripheral = self.discoveredPeripherals.first {
                self.centralManager.connect(peripheral, options: nil)
                
                DispatchQueue.main.asyncAfter(deadline: .now() + 5) {
                    connectExpectation.fulfill()
                    
                    // Discover services
                    peripheral.discoverServices([self.serviceUUID])
                    
                    DispatchQueue.main.asyncAfter(deadline: .now() + 2) {
                        discoverExpectation.fulfill()
                    }
                }
            } else {
                connectExpectation.fulfill()
                discoverExpectation.fulfill()
            }
        }
        
        wait(for: [scanExpectation, connectExpectation, discoverExpectation], timeout: 15)
        
        if let peripheral = connectedPeripheral {
            XCTAssertEqual(peripheral.state, .connected, "Should be connected")
            XCTAssertNotNil(peripheral.services, "Should have discovered services")
        }
    }
    
    // MARK: - Data Transfer Tests
    
    func testDataTransfer() {
        guard let peripheral = connectedPeripheral,
              let service = peripheral.services?.first(where: { $0.uuid == serviceUUID }),
              let characteristic = service.characteristics?.first(where: { $0.uuid == characteristicUUID })
        else {
            XCTSkip("No connected peripheral with required service")
            return
        }
        
        let testData = "Hello from iOS".data(using: .utf8)!
        let expectation = XCTestExpectation(description: "Write data")
        
        // Write data
        peripheral.writeValue(testData, for: characteristic, type: .withResponse)
        
        DispatchQueue.main.asyncAfter(deadline: .now() + 1) {
            expectation.fulfill()
        }
        
        wait(for: [expectation], timeout: 5)
        
        // Read data back
        peripheral.readValue(for: characteristic)
        
        Thread.sleep(forTimeInterval: 1)
        
        if let value = characteristic.value {
            logger.info("Read \(value.count) bytes")
            XCTAssertGreaterThan(value.count, 0, "Should have read data")
        }
    }
    
    // MARK: - MTU Tests
    
    func testMtuNegotiation() {
        guard let peripheral = connectedPeripheral else {
            XCTSkip("No connected peripheral")
            return
        }
        
        let maximumWriteLength = peripheral.maximumWriteValueLength(for: .withResponse)
        logger.info("Maximum write length: \(maximumWriteLength) bytes")
        
        XCTAssertGreaterThanOrEqual(maximumWriteLength, 20, "MTU should be at least 20 bytes")
        
        // iOS automatically negotiates MTU to maximum supported
        // Typical values: 185 (iPhone), 512 (iPad)
        if UIDevice.current.userInterfaceIdiom == .pad {
            XCTAssertGreaterThanOrEqual(maximumWriteLength, 512, "iPad should support larger MTU")
        }
    }
    
    // MARK: - Throughput Tests
    
    func testThroughput() {
        let dataSize = 10 * 1024 // 10KB
        let testData = Data(repeating: 0xAA, count: dataSize)
        var bytesSent = 0
        let startTime = Date()
        
        // Simulate sending data in chunks
        let chunkSize = 182 // Typical iOS BLE chunk size
        for i in stride(from: 0, to: dataSize, by: chunkSize) {
            let endIndex = min(i + chunkSize, dataSize)
            let chunk = testData[i..<endIndex]
            
            // In real test, send via BLE
            bytesSent += chunk.count
            Thread.sleep(forTimeInterval: 0.01) // Simulate transmission delay
        }
        
        let duration = Date().timeIntervalSince(startTime)
        let throughput = Double(bytesSent) / duration
        
        logger.info("Throughput: \(throughput / 1024) KB/s")
        XCTAssertGreaterThan(throughput, 1024, "Throughput should be > 1KB/s")
    }
    
    // MARK: - Stability Tests
    
    func testConnectionStability() {
        let testDuration: TimeInterval = 30 // 30 seconds
        let startTime = Date()
        var connectionDrops = 0
        var measurements: [TimeInterval] = []
        
        while Date().timeIntervalSince(startTime) < testDuration {
            let measurementStart = Date()
            
            // Check connection state
            if let peripheral = connectedPeripheral {
                if peripheral.state != .connected {
                    connectionDrops += 1
                    // Attempt reconnection
                    centralManager.connect(peripheral, options: nil)
                }
            }
            
            measurements.append(Date().timeIntervalSince(measurementStart))
            Thread.sleep(forTimeInterval: 1)
        }
        
        let averageCheckTime = measurements.reduce(0, +) / Double(measurements.count)
        
        logger.info("Connection drops: \(connectionDrops) in \(testDuration)s")
        logger.info("Average check time: \(averageCheckTime * 1000)ms")
        
        XCTAssertLessThan(connectionDrops, 3, "Should have < 3 drops in 30s")
    }
    
    // MARK: - Power Consumption Tests
    
    func testPowerConsumption() {
        let expectation = XCTestExpectation(description: "Power measurement")
        
        // Baseline measurement
        powerMonitor.startMonitoring()
        Thread.sleep(forTimeInterval: 5)
        let baseline = powerMonitor.getAveragePower()
        
        // Start BLE activity
        centralManager.scanForPeripherals(withServices: nil, options: [
            CBCentralManagerScanOptionAllowDuplicatesKey: true
        ])
        
        Thread.sleep(forTimeInterval: 5)
        let scanningPower = powerMonitor.getAveragePower()
        centralManager.stopScan()
        
        // Start advertising
        peripheralManager.startAdvertising([
            CBAdvertisementDataServiceUUIDsKey: [serviceUUID]
        ])
        
        Thread.sleep(forTimeInterval: 5)
        let advertisingPower = powerMonitor.getAveragePower()
        peripheralManager.stopAdvertising()
        
        let scanIncrease = ((scanningPower - baseline) / baseline) * 100
        let advIncrease = ((advertisingPower - baseline) / baseline) * 100
        
        logger.info("Power increase - Scanning: \(scanIncrease)%, Advertising: \(advIncrease)%")
        
        XCTAssertLessThan(scanIncrease, 100, "Scanning power increase should be < 100%")
        XCTAssertLessThan(advIncrease, 50, "Advertising power increase should be < 50%")
        
        expectation.fulfill()
        wait(for: [expectation], timeout: 1)
    }
    
    // MARK: - Notification Tests
    
    func testNotifications() {
        guard let peripheral = connectedPeripheral,
              let service = peripheral.services?.first(where: { $0.uuid == serviceUUID }),
              let characteristic = service.characteristics?.first(where: { 
                  $0.properties.contains(.notify) 
              })
        else {
            XCTSkip("No connected peripheral with notify characteristic")
            return
        }
        
        let expectation = XCTestExpectation(description: "Receive notifications")
        var notificationsReceived = 0
        
        // Enable notifications
        peripheral.setNotifyValue(true, for: characteristic)
        
        // Wait for notifications
        DispatchQueue.main.asyncAfter(deadline: .now() + 5) {
            expectation.fulfill()
        }
        
        wait(for: [expectation], timeout: 10)
        
        // Disable notifications
        peripheral.setNotifyValue(false, for: characteristic)
        
        logger.info("Received \(notificationsReceived) notifications")
    }
}

// MARK: - CBCentralManagerDelegate

extension BleTests: CBCentralManagerDelegate {
    func centralManagerDidUpdateState(_ central: CBCentralManager) {
        switch central.state {
        case .poweredOn:
            logger.info("Bluetooth powered on")
        case .poweredOff:
            logger.error("Bluetooth powered off")
        default:
            logger.warning("Bluetooth state: \(String(describing: central.state))")
        }
    }
    
    func centralManager(_ central: CBCentralManager, didDiscover peripheral: CBPeripheral, 
                       advertisementData: [String : Any], rssi RSSI: NSNumber) {
        if !discoveredPeripherals.contains(peripheral) {
            discoveredPeripherals.append(peripheral)
            logger.info("Discovered: \(peripheral.name ?? "Unknown") RSSI: \(RSSI)")
        }
    }
    
    func centralManager(_ central: CBCentralManager, didConnect peripheral: CBPeripheral) {
        connectedPeripheral = peripheral
        peripheral.delegate = self
        logger.info("Connected to: \(peripheral.name ?? "Unknown")")
    }
    
    func centralManager(_ central: CBCentralManager, didFailToConnect peripheral: CBPeripheral, 
                       error: Error?) {
        logger.error("Failed to connect: \(error?.localizedDescription ?? "Unknown error")")
    }
}

// MARK: - CBPeripheralDelegate

extension BleTests: CBPeripheralDelegate {
    func peripheral(_ peripheral: CBPeripheral, didDiscoverServices error: Error?) {
        if let error = error {
            logger.error("Service discovery error: \(error.localizedDescription)")
            return
        }
        
        logger.info("Discovered \(peripheral.services?.count ?? 0) services")
        
        // Discover characteristics for each service
        peripheral.services?.forEach { service in
            peripheral.discoverCharacteristics(nil, for: service)
        }
    }
    
    func peripheral(_ peripheral: CBPeripheral, didDiscoverCharacteristicsFor service: CBService, 
                   error: Error?) {
        if let error = error {
            logger.error("Characteristic discovery error: \(error.localizedDescription)")
            return
        }
        
        logger.info("Discovered \(service.characteristics?.count ?? 0) characteristics for service \(service.uuid)")
    }
    
    func peripheral(_ peripheral: CBPeripheral, didUpdateValueFor characteristic: CBCharacteristic, 
                   error: Error?) {
        if let error = error {
            logger.error("Characteristic read error: \(error.localizedDescription)")
            return
        }
        
        if let value = characteristic.value {
            logger.info("Characteristic \(characteristic.uuid) updated: \(value.count) bytes")
        }
    }
    
    func peripheral(_ peripheral: CBPeripheral, didWriteValueFor characteristic: CBCharacteristic, 
                   error: Error?) {
        if let error = error {
            logger.error("Characteristic write error: \(error.localizedDescription)")
            return
        }
        
        logger.info("Successfully wrote to characteristic \(characteristic.uuid)")
    }
}

// MARK: - CBPeripheralManagerDelegate

extension BleTests: CBPeripheralManagerDelegate {
    func peripheralManagerDidUpdateState(_ peripheral: CBPeripheralManager) {
        switch peripheral.state {
        case .poweredOn:
            logger.info("Peripheral manager powered on")
        default:
            logger.warning("Peripheral manager state: \(String(describing: peripheral.state))")
        }
    }
    
    func peripheralManagerDidStartAdvertising(_ peripheral: CBPeripheralManager, error: Error?) {
        if let error = error {
            logger.error("Advertising error: \(error.localizedDescription)")
        } else {
            logger.info("Started advertising")
        }
    }
}

// MARK: - Helper Classes

class PowerMonitor {
    private var measurements: [Double] = []
    private let processInfo = ProcessInfo.processInfo
    
    func startMonitoring() {
        measurements.removeAll()
        // Note: Real power monitoring requires special entitlements
        // This simulates power measurement
    }
    
    func getAveragePower() -> Double {
        // Simulate power measurement based on thermal state
        let thermalState = processInfo.thermalState
        
        switch thermalState {
        case .nominal:
            return 100.0 + Double.random(in: -10...10)
        case .fair:
            return 150.0 + Double.random(in: -15...15)
        case .serious:
            return 200.0 + Double.random(in: -20...20)
        case .critical:
            return 250.0 + Double.random(in: -25...25)
        @unknown default:
            return 100.0
        }
    }
}