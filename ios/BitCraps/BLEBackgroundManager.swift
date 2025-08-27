//
//  BLEBackgroundManager.swift
//  BitCraps
//
//  Manages Bluetooth Low Energy operations with proper background support
//  Handles both central and peripheral modes for iOS background BLE limitations
//

import Foundation
import CoreBluetooth
import UIKit

/// BLE Background Manager for BitCraps
/// Handles advertising and scanning with iOS background limitations
class BLEBackgroundManager: NSObject {
    
    // MARK: - Properties
    
    private var centralManager: CBCentralManager?
    private var peripheralManager: CBPeripheralManager?
    
    // Service UUIDs for BitCraps
    private let serviceUUID = CBUUID(string: "00000001-B17C-4A93-0000-000000000000")
    private let characteristicUUID = CBUUID(string: "00000002-B17C-4A93-0000-000000000000")
    
    // Background task management
    private var backgroundTask: UIBackgroundTaskIdentifier = .invalid
    
    // Connection management
    private var connectedPeripherals: Set<CBPeripheral> = []
    private var discoveredPeers: [String: DiscoveredPeer] = [:]
    
    // Advertising state
    private var isAdvertising = false
    private var advertisingData: [String: Any] = [:]
    
    // State restoration
    private let restoreIdentifierKey = "BitCrapsBluetoothRestoration"
    
    // MARK: - Initialization
    
    override init() {
        super.init()
        setupBluetooth()
        registerForBackgroundModes()
    }
    
    private func setupBluetooth() {
        // Initialize with state restoration
        let options: [String: Any] = [
            CBCentralManagerOptionRestoreIdentifierKey: "\(restoreIdentifierKey).central",
            CBCentralManagerOptionShowPowerAlertKey: true
        ]
        
        centralManager = CBCentralManager(
            delegate: self,
            queue: DispatchQueue.global(qos: .background),
            options: options
        )
        
        let peripheralOptions: [String: Any] = [
            CBPeripheralManagerOptionRestoreIdentifierKey: "\(restoreIdentifierKey).peripheral",
            CBPeripheralManagerOptionShowPowerAlertKey: true
        ]
        
        peripheralManager = CBPeripheralManager(
            delegate: self,
            queue: DispatchQueue.global(qos: .background),
            options: peripheralOptions
        )
    }
    
    private func registerForBackgroundModes() {
        // Register for background notifications
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
    
    // MARK: - Background Handling
    
    @objc private func appDidEnterBackground() {
        // Begin background task to ensure BLE operations continue
        backgroundTask = UIApplication.shared.beginBackgroundTask { [weak self] in
            self?.endBackgroundTask()
        }
        
        // Update advertising for background mode
        updateBackgroundAdvertising()
    }
    
    @objc private func appWillEnterForeground() {
        // End background task
        endBackgroundTask()
        
        // Restore full advertising
        updateForegroundAdvertising()
    }
    
    private func endBackgroundTask() {
        if backgroundTask != .invalid {
            UIApplication.shared.endBackgroundTask(backgroundTask)
            backgroundTask = .invalid
        }
    }
    
    // MARK: - Advertising Management
    
    func startAdvertising(with peerID: String) {
        guard let peripheralManager = peripheralManager,
              peripheralManager.state == .poweredOn else {
            return
        }
        
        // Create BitCraps service
        let service = CBMutableService(type: serviceUUID, primary: true)
        
        // Create characteristic for peer exchange
        let characteristic = CBMutableCharacteristic(
            type: characteristicUUID,
            properties: [.read, .write, .notify],
            value: peerID.data(using: .utf8),
            permissions: [.readable, .writeable]
        )
        
        service.characteristics = [characteristic]
        peripheralManager.add(service)
        
        // Prepare advertising data
        advertisingData = [
            CBAdvertisementDataServiceUUIDsKey: [serviceUUID],
            CBAdvertisementDataLocalNameKey: "BitCraps-\(String(peerID.prefix(8)))"
        ]
        
        // Start advertising
        peripheralManager.startAdvertising(advertisingData)
        isAdvertising = true
    }
    
    func stopAdvertising() {
        peripheralManager?.stopAdvertising()
        isAdvertising = false
    }
    
    private func updateBackgroundAdvertising() {
        guard isAdvertising else { return }
        
        // In background, iOS restricts advertising
        // Only service UUIDs are advertised, no local name
        let backgroundData: [String: Any] = [
            CBAdvertisementDataServiceUUIDsKey: [serviceUUID]
        ]
        
        peripheralManager?.stopAdvertising()
        peripheralManager?.startAdvertising(backgroundData)
    }
    
    private func updateForegroundAdvertising() {
        guard isAdvertising else { return }
        
        // Restore full advertising in foreground
        peripheralManager?.stopAdvertising()
        peripheralManager?.startAdvertising(advertisingData)
    }
    
    // MARK: - Scanning Management
    
    func startScanning() {
        guard let centralManager = centralManager,
              centralManager.state == .poweredOn else {
            return
        }
        
        // Scan for BitCraps service
        // In background, must specify service UUID
        let options: [String: Any] = [
            CBCentralManagerScanOptionAllowDuplicatesKey: false,
            CBCentralManagerScanOptionSolicitedServiceUUIDsKey: [serviceUUID]
        ]
        
        centralManager.scanForPeripherals(
            withServices: [serviceUUID],
            options: options
        )
    }
    
    func stopScanning() {
        centralManager?.stopScan()
    }
    
    // MARK: - State Restoration
    
    func handleStateRestoration(
        central: CBCentralManager?,
        peripherals: [CBPeripheral]
    ) {
        // Restore central manager state
        if let central = central {
            self.centralManager = central
            central.delegate = self
        }
        
        // Restore connected peripherals
        for peripheral in peripherals {
            connectedPeripherals.insert(peripheral)
            peripheral.delegate = self
        }
        
        // Resume operations
        if UIApplication.shared.applicationState == .background {
            updateBackgroundAdvertising()
        } else {
            updateForegroundAdvertising()
        }
    }
}

// MARK: - CBCentralManagerDelegate

extension BLEBackgroundManager: CBCentralManagerDelegate {
    
    func centralManagerDidUpdateState(_ central: CBCentralManager) {
        switch central.state {
        case .poweredOn:
            // Resume scanning if needed
            startScanning()
            
        case .poweredOff:
            // Clear connections
            connectedPeripherals.removeAll()
            
        default:
            break
        }
    }
    
    func centralManager(
        _ central: CBCentralManager,
        didDiscover peripheral: CBPeripheral,
        advertisementData: [String : Any],
        rssi RSSI: NSNumber
    ) {
        // Process discovered peer
        let peerID = peripheral.identifier.uuidString
        
        let discoveredPeer = DiscoveredPeer(
            id: peerID,
            name: advertisementData[CBAdvertisementDataLocalNameKey] as? String ?? "Unknown",
            rssi: RSSI.intValue,
            lastSeen: Date()
        )
        
        discoveredPeers[peerID] = discoveredPeer
        
        // Auto-connect in background if needed
        if UIApplication.shared.applicationState == .background {
            central.connect(peripheral, options: nil)
        }
    }
    
    func centralManager(
        _ central: CBCentralManager,
        didConnect peripheral: CBPeripheral
    ) {
        connectedPeripherals.insert(peripheral)
        peripheral.delegate = self
        peripheral.discoverServices([serviceUUID])
    }
    
    func centralManager(
        _ central: CBCentralManager,
        willRestoreState dict: [String : Any]
    ) {
        // Handle state restoration
        if let peripherals = dict[CBCentralManagerRestoredStatePeripheralsKey] as? [CBPeripheral] {
            for peripheral in peripherals {
                connectedPeripherals.insert(peripheral)
                peripheral.delegate = self
            }
        }
    }
}

// MARK: - CBPeripheralManagerDelegate

extension BLEBackgroundManager: CBPeripheralManagerDelegate {
    
    func peripheralManagerDidUpdateState(_ peripheral: CBPeripheralManager) {
        switch peripheral.state {
        case .poweredOn:
            // Ready to advertise
            if isAdvertising {
                updateForegroundAdvertising()
            }
            
        case .poweredOff:
            isAdvertising = false
            
        default:
            break
        }
    }
    
    func peripheralManager(
        _ peripheral: CBPeripheralManager,
        willRestoreState dict: [String : Any]
    ) {
        // Restore advertising state
        if let services = dict[CBPeripheralManagerRestoredStateServicesKey] as? [CBMutableService] {
            for service in services {
                peripheral.add(service)
            }
        }
        
        if let _ = dict[CBPeripheralManagerRestoredStateAdvertisementDataKey] {
            isAdvertising = true
            updateBackgroundAdvertising()
        }
    }
    
    func peripheralManager(
        _ peripheral: CBPeripheralManager,
        didReceiveRead request: CBATTRequest
    ) {
        // Handle read requests
        if request.characteristic.uuid == characteristicUUID {
            // Provide peer ID data
            if let value = request.characteristic.value {
                request.value = value
                peripheral.respond(to: request, withResult: .success)
            } else {
                peripheral.respond(to: request, withResult: .attributeNotFound)
            }
        }
    }
}

// MARK: - CBPeripheralDelegate

extension BLEBackgroundManager: CBPeripheralDelegate {
    
    func peripheral(
        _ peripheral: CBPeripheral,
        didDiscoverServices error: Error?
    ) {
        guard error == nil,
              let services = peripheral.services else { return }
        
        for service in services {
            if service.uuid == serviceUUID {
                peripheral.discoverCharacteristics(
                    [characteristicUUID],
                    for: service
                )
            }
        }
    }
    
    func peripheral(
        _ peripheral: CBPeripheral,
        didDiscoverCharacteristicsFor service: CBService,
        error: Error?
    ) {
        guard error == nil,
              let characteristics = service.characteristics else { return }
        
        for characteristic in characteristics {
            if characteristic.uuid == characteristicUUID {
                // Read peer information
                peripheral.readValue(for: characteristic)
                
                // Subscribe to notifications
                if characteristic.properties.contains(.notify) {
                    peripheral.setNotifyValue(true, for: characteristic)
                }
            }
        }
    }
}

// MARK: - Supporting Types

struct DiscoveredPeer {
    let id: String
    let name: String
    let rssi: Int
    let lastSeen: Date
}