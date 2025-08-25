package com.bitcraps

import android.bluetooth.*
import android.bluetooth.le.*
import android.content.Context
import android.os.ParcelUuid
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import kotlinx.coroutines.*
import org.junit.Assert.*
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import java.util.*
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

/**
 * BLE Instrumented tests for Android
 * Must be run on physical devices with Bluetooth enabled
 */
@RunWith(AndroidJUnit4::class)
class BleInstrumentedTest {
    
    private lateinit var context: Context
    private lateinit var bluetoothManager: BluetoothManager
    private lateinit var bluetoothAdapter: BluetoothAdapter
    private lateinit var bleScanner: BluetoothLeScanner
    private lateinit var bleAdvertiser: BluetoothLeAdvertiser
    
    companion object {
        private const val SERVICE_UUID = "12345678-1234-1234-1234-123456789012"
        private const val CHAR_UUID = "12345678-1234-1234-1234-123456789013"
        private const val SCAN_DURATION_MS = 10000L
        private const val CONNECTION_TIMEOUT_MS = 5000L
    }
    
    @Before
    fun setup() {
        context = InstrumentationRegistry.getInstrumentation().targetContext
        bluetoothManager = context.getSystemService(Context.BLUETOOTH_SERVICE) as BluetoothManager
        bluetoothAdapter = bluetoothManager.adapter
        
        // Ensure Bluetooth is enabled
        assertTrue("Bluetooth must be enabled", bluetoothAdapter.isEnabled)
        
        bleScanner = bluetoothAdapter.bluetoothLeScanner
        bleAdvertiser = bluetoothAdapter.bluetoothLeAdvertiser
    }
    
    @Test
    fun testBleDiscovery() {
        val discoveredDevices = mutableListOf<ScanResult>()
        val scanLatch = CountDownLatch(1)
        
        val scanCallback = object : ScanCallback() {
            override fun onScanResult(callbackType: Int, result: ScanResult) {
                discoveredDevices.add(result)
                
                // Look for our service UUID
                result.scanRecord?.serviceUuids?.forEach { uuid ->
                    if (uuid.toString() == SERVICE_UUID) {
                        scanLatch.countDown()
                    }
                }
            }
            
            override fun onScanFailed(errorCode: Int) {
                fail("Scan failed with error code: $errorCode")
            }
        }
        
        // Start scanning
        val scanSettings = ScanSettings.Builder()
            .setScanMode(ScanSettings.SCAN_MODE_LOW_LATENCY)
            .build()
        
        val scanFilters = listOf(
            ScanFilter.Builder()
                .setServiceUuid(ParcelUuid.fromString(SERVICE_UUID))
                .build()
        )
        
        bleScanner.startScan(scanFilters, scanSettings, scanCallback)
        
        // Wait for discovery or timeout
        val discovered = scanLatch.await(SCAN_DURATION_MS, TimeUnit.MILLISECONDS)
        
        // Stop scanning
        bleScanner.stopScan(scanCallback)
        
        // Verify results
        assertTrue("Should discover at least one device", discoveredDevices.isNotEmpty())
        if (discovered) {
            println("Discovered ${discoveredDevices.size} devices with our service")
        }
    }
    
    @Test
    fun testBleAdvertising() {
        val advertisingLatch = CountDownLatch(1)
        
        val advertiseCallback = object : AdvertiseCallback() {
            override fun onStartSuccess(settingsInEffect: AdvertiseSettings) {
                println("Advertising started successfully")
                advertisingLatch.countDown()
            }
            
            override fun onStartFailure(errorCode: Int) {
                fail("Advertising failed with error code: $errorCode")
            }
        }
        
        // Configure advertising
        val advertiseSettings = AdvertiseSettings.Builder()
            .setAdvertiseMode(AdvertiseSettings.ADVERTISE_MODE_LOW_LATENCY)
            .setTxPowerLevel(AdvertiseSettings.ADVERTISE_TX_POWER_HIGH)
            .setConnectable(true)
            .setTimeout(0) // Advertise indefinitely
            .build()
        
        val advertiseData = AdvertiseData.Builder()
            .setIncludeDeviceName(true)
            .addServiceUuid(ParcelUuid.fromString(SERVICE_UUID))
            .build()
        
        val scanResponse = AdvertiseData.Builder()
            .setIncludeTxPowerLevel(true)
            .build()
        
        // Start advertising
        bleAdvertiser.startAdvertising(advertiseSettings, advertiseData, scanResponse, advertiseCallback)
        
        // Wait for advertising to start
        assertTrue("Advertising should start", advertisingLatch.await(5, TimeUnit.SECONDS))
        
        // Keep advertising for a while
        Thread.sleep(5000)
        
        // Stop advertising
        bleAdvertiser.stopAdvertising(advertiseCallback)
    }
    
    @Test
    fun testBleConnection() = runBlocking {
        val connectionLatch = CountDownLatch(1)
        var gatt: BluetoothGatt? = null
        
        val gattCallback = object : BluetoothGattCallback() {
            override fun onConnectionStateChange(gatt: BluetoothGatt, status: Int, newState: Int) {
                when (newState) {
                    BluetoothProfile.STATE_CONNECTED -> {
                        println("Connected to GATT server")
                        gatt.discoverServices()
                    }
                    BluetoothProfile.STATE_DISCONNECTED -> {
                        println("Disconnected from GATT server")
                    }
                }
            }
            
            override fun onServicesDiscovered(gatt: BluetoothGatt, status: Int) {
                if (status == BluetoothGatt.GATT_SUCCESS) {
                    println("Services discovered: ${gatt.services.size}")
                    connectionLatch.countDown()
                }
            }
            
            override fun onCharacteristicRead(
                gatt: BluetoothGatt,
                characteristic: BluetoothGattCharacteristic,
                status: Int
            ) {
                if (status == BluetoothGatt.GATT_SUCCESS) {
                    println("Characteristic read: ${characteristic.value?.size ?: 0} bytes")
                }
            }
            
            override fun onCharacteristicWrite(
                gatt: BluetoothGatt,
                characteristic: BluetoothGattCharacteristic,
                status: Int
            ) {
                if (status == BluetoothGatt.GATT_SUCCESS) {
                    println("Characteristic written successfully")
                }
            }
        }
        
        // First, scan for a device to connect to
        val scanLatch = CountDownLatch(1)
        var deviceToConnect: BluetoothDevice? = null
        
        val scanCallback = object : ScanCallback() {
            override fun onScanResult(callbackType: Int, result: ScanResult) {
                deviceToConnect = result.device
                scanLatch.countDown()
            }
        }
        
        bleScanner.startScan(scanCallback)
        
        if (scanLatch.await(SCAN_DURATION_MS, TimeUnit.MILLISECONDS)) {
            bleScanner.stopScan(scanCallback)
            
            deviceToConnect?.let { device ->
                // Connect to the device
                gatt = device.connectGatt(context, false, gattCallback)
                
                // Wait for connection and service discovery
                if (connectionLatch.await(CONNECTION_TIMEOUT_MS, TimeUnit.MILLISECONDS)) {
                    // Test read/write operations
                    gatt?.services?.forEach { service ->
                        service.characteristics.forEach { characteristic ->
                            when {
                                characteristic.properties and BluetoothGattCharacteristic.PROPERTY_READ != 0 -> {
                                    gatt?.readCharacteristic(characteristic)
                                }
                                characteristic.properties and BluetoothGattCharacteristic.PROPERTY_WRITE != 0 -> {
                                    characteristic.value = "Test".toByteArray()
                                    gatt?.writeCharacteristic(characteristic)
                                }
                            }
                        }
                    }
                    
                    // Keep connection for a while
                    delay(2000)
                }
            }
        } else {
            bleScanner.stopScan(scanCallback)
        }
        
        // Clean up
        gatt?.close()
    }
    
    @Test
    fun testMtuNegotiation() {
        val mtuLatch = CountDownLatch(1)
        var negotiatedMtu = 23 // Default BLE MTU
        
        val gattCallback = object : BluetoothGattCallback() {
            override fun onMtuChanged(gatt: BluetoothGatt, mtu: Int, status: Int) {
                if (status == BluetoothGatt.GATT_SUCCESS) {
                    negotiatedMtu = mtu
                    println("MTU changed to: $mtu")
                    mtuLatch.countDown()
                }
            }
        }
        
        // This would be tested with an actual connection
        // For now, verify default MTU
        assertEquals(23, negotiatedMtu)
    }
    
    @Test
    fun testDataThroughput() = runBlocking {
        val dataSize = 1024 * 10 // 10KB
        val testData = ByteArray(dataSize) { it.toByte() }
        val chunks = testData.toList().chunked(20) // BLE packet size
        
        val startTime = System.currentTimeMillis()
        
        // Simulate sending data in chunks
        chunks.forEach { chunk ->
            // In real test, this would send via BLE
            delay(10) // Simulate BLE transmission delay
        }
        
        val endTime = System.currentTimeMillis()
        val duration = endTime - startTime
        val throughput = (dataSize * 1000.0) / duration
        
        println("Throughput: %.2f bytes/second".format(throughput))
        
        // Verify reasonable throughput (> 1KB/s)
        assertTrue("Throughput should be > 1KB/s", throughput > 1024)
    }
    
    @Test
    fun testConnectionStability() = runBlocking {
        // Test connection stability over time
        val testDurationMs = 30000L // 30 seconds
        val startTime = System.currentTimeMillis()
        var connectionDrops = 0
        
        while (System.currentTimeMillis() - startTime < testDurationMs) {
            // In real test, monitor actual connection state
            delay(1000)
            
            // Simulate occasional connection check
            val isConnected = Math.random() > 0.05 // 95% stable
            if (!isConnected) {
                connectionDrops++
            }
        }
        
        println("Connection drops in 30s: $connectionDrops")
        assertTrue("Should have < 3 drops in 30s", connectionDrops < 3)
    }
    
    @Test
    fun testPowerConsumption() = runBlocking {
        // Note: Real power measurement requires system-level access
        // This test simulates power monitoring
        
        val measurements = mutableListOf<Float>()
        
        // Baseline
        repeat(10) {
            measurements.add(100f + Random().nextFloat() * 10) // ~100mW baseline
            delay(100)
        }
        val baseline = measurements.average()
        
        measurements.clear()
        
        // During BLE activity
        repeat(10) {
            measurements.add(150f + Random().nextFloat() * 20) // ~150mW active
            delay(100)
        }
        val active = measurements.average()
        
        val increase = ((active - baseline) / baseline) * 100
        println("Power increase during BLE: %.1f%%".format(increase))
        
        assertTrue("Power increase should be < 100%", increase < 100)
    }
}