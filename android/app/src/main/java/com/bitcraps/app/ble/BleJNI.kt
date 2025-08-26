package com.bitcraps.app.ble

import android.bluetooth.BluetoothDevice
import android.bluetooth.le.ScanResult
import android.os.ParcelUuid
import timber.log.Timber

/**
 * JNI bridge for BitCraps BLE operations
 * This class provides the interface between Android and the Rust BLE implementation
 */
object BleJNI {
    
    init {
        try {
            // Load the native library
            System.loadLibrary("bitcraps")
            Timber.i("BitCraps native library loaded successfully")
        } catch (e: UnsatisfiedLinkError) {
            Timber.e(e, "Failed to load BitCraps native library")
            throw RuntimeException("Failed to load native library", e)
        }
    }
    
    /**
     * Initialize the BLE manager in Rust
     */
    external fun initializeBleManager(javaVmPtr: Long, bleService: Any): Boolean
    
    /**
     * Start BLE advertising
     */
    external fun startAdvertising(): Boolean
    
    /**
     * Stop BLE advertising
     */
    external fun stopAdvertising(): Boolean
    
    /**
     * Start BLE scanning
     */
    external fun startScanning(): Boolean
    
    /**
     * Stop BLE scanning
     */
    external fun stopScanning(): Boolean
    
    /**
     * Called from Android when a peer is discovered
     */
    external fun onPeerDiscovered(
        address: String,
        name: String?,
        rssi: Int,
        manufacturerData: ByteArray?,
        serviceUuids: Array<String>
    )
    
    /**
     * Get count of discovered peers
     */
    external fun getDiscoveredPeersCount(): Int
    
    /**
     * Get array of discovered peer addresses
     */
    external fun getDiscoveredPeerAddresses(): Array<String>
    
    /**
     * Check if advertising is active
     */
    external fun isAdvertising(): Boolean
    
    /**
     * Check if scanning is active
     */
    external fun isScanning(): Boolean
    
    // Internal utility functions
    
    /**
     * Convert ScanResult to parameters for JNI call
     */
    internal fun processScanResult(result: ScanResult) {
        val device = result.device
        val scanRecord = result.scanRecord
        val rssi = result.rssi
        
        val address = device.address
        val name = scanRecord?.deviceName ?: device.name
        
        // Extract manufacturer data
        val manufacturerData = scanRecord?.getManufacturerSpecificData(0xFFFF)
        
        // Extract service UUIDs
        val serviceUuids = scanRecord?.serviceUuids?.map { it.toString() }?.toTypedArray() 
            ?: emptyArray()
        
        // Call native function
        try {
            onPeerDiscovered(address, name, rssi, manufacturerData, serviceUuids)
            Timber.d("Peer processed via JNI: $address")
        } catch (e: Exception) {
            Timber.e(e, "Failed to process peer via JNI: $address")
        }
    }
    
    /**
     * Get the Java VM pointer for JNI initialization
     */
    @JvmStatic
    fun getJavaVmPointer(): Long {
        return try {
            // This is a platform-specific way to get the JavaVM pointer
            // In production, this would need proper implementation
            0L // Placeholder - needs proper implementation
        } catch (e: Exception) {
            Timber.e(e, "Failed to get JavaVM pointer")
            0L
        }
    }
    
    /**
     * Initialize JNI bridge with BLE service
     */
    fun initialize(bleService: Any): Boolean {
        return try {
            val vmPtr = getJavaVmPointer()
            initializeBleManager(vmPtr, bleService)
        } catch (e: Exception) {
            Timber.e(e, "Failed to initialize JNI bridge")
            false
        }
    }
}

/**
 * Exception classes for JNI operations
 */
package com.bitcraps.exceptions

class BluetoothException(message: String, cause: Throwable? = null) : Exception(message, cause)
class NetworkException(message: String, cause: Throwable? = null) : Exception(message, cause)
class InitializationException(message: String, cause: Throwable? = null) : Exception(message, cause)
class GameException(message: String, cause: Throwable? = null) : Exception(message, cause)
class CryptoException(message: String, cause: Throwable? = null) : Exception(message, cause)