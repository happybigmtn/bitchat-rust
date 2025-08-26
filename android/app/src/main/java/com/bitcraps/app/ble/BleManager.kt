package com.bitcraps.app.ble

import android.Manifest
import android.annotation.SuppressLint
import android.bluetooth.*
import android.bluetooth.le.*
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import android.os.ParcelUuid
import androidx.core.content.ContextCompat
import timber.log.Timber
import kotlinx.coroutines.*
import java.util.*
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.atomic.AtomicBoolean

/**
 * BLE Manager for BitCraps peer discovery with Android 14+ compliance
 */
class BleManager(private val context: Context) {
    
    companion object {
        // BitCraps service UUID (matches Rust implementation)
        val BITCRAPS_SERVICE_UUID: UUID = UUID.fromString("12345678-1234-5678-1234-567812345678")
        val BITCRAPS_SERVICE_PARCEL_UUID = ParcelUuid(BITCRAPS_SERVICE_UUID)
        
        // Scan settings for different Android versions
        private const val SCAN_MODE_BALANCED = ScanSettings.SCAN_MODE_BALANCED
        private const val SCAN_MODE_LOW_LATENCY = ScanSettings.SCAN_MODE_LOW_LATENCY
        private const val CALLBACK_TYPE_ALL_MATCHES = ScanSettings.CALLBACK_TYPE_ALL_MATCHES
        
        // Scan intervals
        private const val SCAN_DURATION_MS = 10000L // 10 seconds
        private const val SCAN_PAUSE_MS = 5000L     // 5 seconds pause
    }
    
    private val bluetoothManager: BluetoothManager by lazy {
        context.getSystemService(Context.BLUETOOTH_SERVICE) as BluetoothManager
    }
    
    private val bluetoothAdapter: BluetoothAdapter? by lazy {
        bluetoothManager.adapter
    }
    
    private val bleScanner: BluetoothLeScanner? by lazy {
        bluetoothAdapter?.bluetoothLeScanner
    }
    
    private var scanningJob: Job? = null
    private val isScanning = AtomicBoolean(false)
    private val discoveredPeers = ConcurrentHashMap<String, DiscoveredPeer>()
    
    data class DiscoveredPeer(
        val address: String,
        val name: String?,
        val rssi: Int,
        val serviceUuids: List<ParcelUuid>?,
        val manufacturerData: Map<Int, ByteArray>?,
        val firstSeen: Long,
        val lastSeen: Long
    )
    
    /**
     * Start BLE scanning for BitCraps peers
     */
    fun startScanning() {
        if (!hasRequiredPermissions()) {
            Timber.w("Cannot start scanning - missing BLE permissions")
            return
        }
        
        if (!isBluetoothEnabled()) {
            Timber.w("Cannot start scanning - Bluetooth is disabled")
            return
        }
        
        if (isScanning.get()) {
            Timber.d("BLE scanning already running")
            return
        }
        
        Timber.i("Starting BLE scanning for BitCraps peers")
        
        val scanSettings = createScanSettings()
        val scanFilters = createScanFilters()
        
        scanningJob = CoroutineScope(Dispatchers.Default).launch {
            startPeriodicScanning(scanSettings, scanFilters)
        }
    }
    
    /**
     * Stop BLE scanning
     */
    fun stopScanning() {
        if (!isScanning.get()) {
            Timber.d("BLE scanning not running")
            return
        }
        
        Timber.i("Stopping BLE scanning")
        
        scanningJob?.cancel()
        scanningJob = null
        
        try {
            if (hasRequiredPermissions()) {
                bleScanner?.stopScan(scanCallback)
            }
        } catch (e: SecurityException) {
            Timber.e(e, "Security exception stopping scan")
        } catch (e: Exception) {
            Timber.e(e, "Error stopping scan")
        }
        
        isScanning.set(false)
    }
    
    /**
     * Periodic scanning to comply with Android background scanning limits
     */
    private suspend fun startPeriodicScanning(
        scanSettings: ScanSettings,
        scanFilters: List<ScanFilter>
    ) {
        while (!scanningJob?.isCancelled!!) {
            try {
                // Start scan
                if (hasRequiredPermissions() && isBluetoothEnabled()) {
                    bleScanner?.startScan(scanFilters, scanSettings, scanCallback)
                    isScanning.set(true)
                    Timber.d("BLE scan cycle started")
                    
                    // Scan for specified duration
                    delay(SCAN_DURATION_MS)
                    
                    // Stop scan
                    bleScanner?.stopScan(scanCallback)
                    isScanning.set(false)
                    Timber.d("BLE scan cycle stopped")
                    
                    // Pause before next scan
                    delay(SCAN_PAUSE_MS)
                } else {
                    delay(1000) // Short delay if BLE not available
                }
            } catch (e: SecurityException) {
                Timber.e(e, "Security exception during scanning")
                delay(5000) // Wait longer if permission issue
            } catch (e: Exception) {
                Timber.e(e, "Error during BLE scanning cycle")
                delay(1000)
            }
        }
        
        isScanning.set(false)
    }
    
    /**
     * Create scan settings optimized for different Android versions
     */
    private fun createScanSettings(): ScanSettings {
        val builder = ScanSettings.Builder()
        
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
            // Android 6.0+ - more efficient scanning
            builder
                .setScanMode(SCAN_MODE_BALANCED)
                .setCallbackType(CALLBACK_TYPE_ALL_MATCHES)
                .setMatchMode(ScanSettings.MATCH_MODE_AGGRESSIVE)
                .setNumOfMatches(ScanSettings.MATCH_NUM_MAX_ADVERTISEMENT)
                .setReportDelay(0)
                .build()
        } else {
            // Pre-Android 6.0
            builder
                .setScanMode(SCAN_MODE_BALANCED)
                .setReportDelay(0)
                .build()
        }
    }
    
    /**
     * Create scan filters for BitCraps services
     */
    private fun createScanFilters(): List<ScanFilter> {
        return listOf(
            // Filter for BitCraps service UUID
            ScanFilter.Builder()
                .setServiceUuid(BITCRAPS_SERVICE_PARCEL_UUID)
                .build(),
                
            // Fallback filter for devices advertising with manufacturer data
            ScanFilter.Builder()
                .setManufacturerData(0xFFFF, byteArrayOf()) // BitCraps manufacturer ID
                .build()
        )
    }
    
    /**
     * BLE scan callback with proper permission handling
     */
    private val scanCallback = object : ScanCallback() {
        
        override fun onScanResult(callbackType: Int, result: ScanResult) {
            super.onScanResult(callbackType, result)
            
            try {
                processScanResult(result)
            } catch (e: Exception) {
                Timber.e(e, "Error processing scan result")
            }
        }
        
        override fun onBatchScanResults(results: MutableList<ScanResult>) {
            super.onBatchScanResults(results)
            
            results.forEach { result ->
                try {
                    processScanResult(result)
                } catch (e: Exception) {
                    Timber.e(e, "Error processing batch scan result")
                }
            }
        }
        
        override fun onScanFailed(errorCode: Int) {
            super.onScanFailed(errorCode)
            
            val errorMessage = when (errorCode) {
                SCAN_FAILED_ALREADY_STARTED -> "Scan already started"
                SCAN_FAILED_APPLICATION_REGISTRATION_FAILED -> "Application registration failed"
                SCAN_FAILED_FEATURE_UNSUPPORTED -> "Feature unsupported"
                SCAN_FAILED_INTERNAL_ERROR -> "Internal error"
                SCAN_FAILED_OUT_OF_HARDWARE_RESOURCES -> "Out of hardware resources"
                else -> "Unknown error: $errorCode"
            }
            
            Timber.e("BLE scan failed: %s", errorMessage)
            isScanning.set(false)
        }
    }
    
    /**
     * Process individual scan results
     */
    @SuppressLint("MissingPermission")
    private fun processScanResult(result: ScanResult) {
        val device = result.device
        val scanRecord = result.scanRecord
        val rssi = result.rssi
        
        // Check if this is a BitCraps device
        if (isBitCrapsDevice(scanRecord)) {
            val currentTime = System.currentTimeMillis()
            val address = device.address
            
            val peer = discoveredPeers[address]?.let { existingPeer ->
                // Update existing peer
                existingPeer.copy(
                    lastSeen = currentTime,
                    rssi = rssi
                )
            } ?: run {
                // Create new peer
                DiscoveredPeer(
                    address = address,
                    name = scanRecord?.deviceName ?: device.name,
                    rssi = rssi,
                    serviceUuids = scanRecord?.serviceUuids,
                    manufacturerData = scanRecord?.manufacturerSpecificData?.let { data ->
                        (0 until data.size()).associate { i ->
                            data.keyAt(i) to data.valueAt(i)
                        }
                    },
                    firstSeen = currentTime,
                    lastSeen = currentTime
                )
            }
            
            discoveredPeers[address] = peer
            
            Timber.d("BitCraps peer discovered: %s (RSSI: %d)", address, rssi)
            
            // Notify Rust layer of discovered peer via JNI
            BleJNI.processScanResult(result)
        }
    }
    
    /**
     * Check if scan record indicates a BitCraps device
     */
    private fun isBitCrapsDevice(scanRecord: ScanRecord?): Boolean {
        if (scanRecord == null) return false
        
        // Check for BitCraps service UUID
        scanRecord.serviceUuids?.let { serviceUuids ->
            if (serviceUuids.contains(BITCRAPS_SERVICE_PARCEL_UUID)) {
                return true
            }
        }
        
        // Check for BitCraps manufacturer data
        scanRecord.manufacturerSpecificData?.get(0xFFFF)?.let { data ->
            // TODO: Validate manufacturer data format
            return data.isNotEmpty()
        }
        
        // Check device name
        scanRecord.deviceName?.let { name ->
            if (name.startsWith("BitCraps") || name.contains("CRAP")) {
                return true
            }
        }
        
        return false
    }
    
    /**
     * Check if required BLE permissions are granted
     */
    private fun hasRequiredPermissions(): Boolean {
        val requiredPermissions = mutableListOf<String>().apply {
            add(Manifest.permission.BLUETOOTH)
            add(Manifest.permission.BLUETOOTH_ADMIN)
            
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                add(Manifest.permission.BLUETOOTH_SCAN)
            } else {
                add(Manifest.permission.ACCESS_FINE_LOCATION)
            }
        }
        
        return requiredPermissions.all { permission ->
            ContextCompat.checkSelfPermission(context, permission) == PackageManager.PERMISSION_GRANTED
        }
    }
    
    /**
     * Check if Bluetooth is enabled
     */
    private fun isBluetoothEnabled(): Boolean {
        return bluetoothAdapter?.isEnabled == true
    }
    
    /**
     * Clean up expired peers
     */
    fun cleanupExpiredPeers(maxAge: Long = 60000L) { // 1 minute
        val cutoffTime = System.currentTimeMillis() - maxAge
        
        val expiredPeers = discoveredPeers.filterValues { peer ->
            peer.lastSeen < cutoffTime
        }
        
        expiredPeers.keys.forEach { address ->
            discoveredPeers.remove(address)
        }
        
        if (expiredPeers.isNotEmpty()) {
            Timber.d("Cleaned up %d expired peers", expiredPeers.size)
        }
    }
    
    /**
     * Get count of discovered peers
     */
    fun getDiscoveredPeersCount(): Int = discoveredPeers.size
    
    /**
     * Get list of discovered peers
     */
    fun getDiscoveredPeers(): List<DiscoveredPeer> = discoveredPeers.values.toList()
    
    /**
     * Check if currently scanning
     */
    fun isCurrentlyScanning(): Boolean = isScanning.get()
}