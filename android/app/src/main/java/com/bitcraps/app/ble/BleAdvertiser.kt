package com.bitcraps.app.ble

import android.Manifest
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
import java.util.concurrent.atomic.AtomicBoolean

/**
 * BLE Advertiser for BitCraps peer visibility with Android 14+ compliance
 */
class BleAdvertiser(private val context: Context) {
    
    companion object {
        // BitCraps service UUID (matches Rust implementation)
        val BITCRAPS_SERVICE_UUID: UUID = UUID.fromString("12345678-1234-5678-1234-567812345678")
        val BITCRAPS_SERVICE_PARCEL_UUID = ParcelUuid(BITCRAPS_SERVICE_UUID)
        
        // Manufacturer ID for BitCraps (using reserved test range)
        private const val BITCRAPS_MANUFACTURER_ID = 0xFFFF
        
        // Advertisement intervals
        private const val ADVERTISE_DURATION_MS = 30000L // 30 seconds
        private const val ADVERTISE_PAUSE_MS = 10000L    // 10 seconds pause
    }
    
    private val bluetoothManager: BluetoothManager by lazy {
        context.getSystemService(Context.BLUETOOTH_SERVICE) as BluetoothManager
    }
    
    private val bluetoothAdapter: BluetoothAdapter? by lazy {
        bluetoothManager.adapter
    }
    
    private val bleAdvertiser: BluetoothLeAdvertiser? by lazy {
        bluetoothAdapter?.bluetoothLeAdvertiser
    }
    
    private var advertisingJob: Job? = null
    private val isAdvertising = AtomicBoolean(false)
    
    /**
     * Start BLE advertising for BitCraps peer visibility
     */
    fun startAdvertising() {
        if (!hasRequiredPermissions()) {
            Timber.w("Cannot start advertising - missing BLE permissions")
            return
        }
        
        if (!isBluetoothEnabled()) {
            Timber.w("Cannot start advertising - Bluetooth is disabled")
            return
        }
        
        if (!isAdvertisingSupported()) {
            Timber.w("BLE advertising not supported on this device")
            return
        }
        
        if (isAdvertising.get()) {
            Timber.d("BLE advertising already running")
            return
        }
        
        Timber.i("Starting BLE advertising for BitCraps visibility")
        
        val advertiseSettings = createAdvertiseSettings()
        val advertiseData = createAdvertiseData()
        val scanResponse = createScanResponse()
        
        advertisingJob = CoroutineScope(Dispatchers.Default).launch {
            startPeriodicAdvertising(advertiseSettings, advertiseData, scanResponse)
        }
    }
    
    /**
     * Stop BLE advertising
     */
    fun stopAdvertising() {
        if (!isAdvertising.get()) {
            Timber.d("BLE advertising not running")
            return
        }
        
        Timber.i("Stopping BLE advertising")
        
        advertisingJob?.cancel()
        advertisingJob = null
        
        try {
            if (hasRequiredPermissions()) {
                bleAdvertiser?.stopAdvertising(advertiseCallback)
            }
        } catch (e: SecurityException) {
            Timber.e(e, "Security exception stopping advertising")
        } catch (e: Exception) {
            Timber.e(e, "Error stopping advertising")
        }
        
        isAdvertising.set(false)
    }
    
    /**
     * Periodic advertising to manage power consumption
     */
    private suspend fun startPeriodicAdvertising(
        advertiseSettings: AdvertiseSettings,
        advertiseData: AdvertiseData,
        scanResponse: AdvertiseData
    ) {
        while (!advertisingJob?.isCancelled!!) {
            try {
                // Start advertising
                if (hasRequiredPermissions() && isBluetoothEnabled()) {
                    bleAdvertiser?.startAdvertising(
                        advertiseSettings,
                        advertiseData,
                        scanResponse,
                        advertiseCallback
                    )
                    Timber.d("BLE advertising cycle started")
                    
                    // Advertise for specified duration
                    delay(ADVERTISE_DURATION_MS)
                    
                    // Stop advertising
                    bleAdvertiser?.stopAdvertising(advertiseCallback)
                    Timber.d("BLE advertising cycle stopped")
                    
                    // Pause before next cycle
                    delay(ADVERTISE_PAUSE_MS)
                } else {
                    delay(1000) // Short delay if BLE not available
                }
            } catch (e: SecurityException) {
                Timber.e(e, "Security exception during advertising")
                delay(5000) // Wait longer if permission issue
            } catch (e: Exception) {
                Timber.e(e, "Error during BLE advertising cycle")
                delay(1000)
            }
        }
        
        isAdvertising.set(false)
    }
    
    /**
     * Create advertising settings
     */
    private fun createAdvertiseSettings(): AdvertiseSettings {
        return AdvertiseSettings.Builder()
            .setAdvertiseMode(AdvertiseSettings.ADVERTISE_MODE_BALANCED)
            .setTxPowerLevel(AdvertiseSettings.ADVERTISE_TX_POWER_MEDIUM)
            .setConnectable(true)
            .setTimeout(0) // Advertise until stopped
            .build()
    }
    
    /**
     * Create advertisement data
     */
    private fun createAdvertiseData(): AdvertiseData {
        // Generate BitCraps node identification
        val nodeId = generateNodeId()
        
        return AdvertiseData.Builder()
            .setIncludeDeviceName(true)
            .setIncludeTxPowerLevel(true)
            .addServiceUuid(BITCRAPS_SERVICE_PARCEL_UUID)
            .addManufacturerData(BITCRAPS_MANUFACTURER_ID, nodeId)
            .build()
    }
    
    /**
     * Create scan response data
     */
    private fun createScanResponse(): AdvertiseData {
        // Additional data for BitCraps capabilities
        val capabilities = createCapabilitiesData()
        
        return AdvertiseData.Builder()
            .setIncludeDeviceName(false)
            .addServiceData(BITCRAPS_SERVICE_PARCEL_UUID, capabilities)
            .build()
    }
    
    /**
     * Generate node identification data
     */
    private fun generateNodeId(): ByteArray {
        // Create a simplified node ID for advertisement
        // In production, this would be derived from the BitCraps peer ID
        val nodeData = ByteArray(8) // Shortened for BLE advertisement limits
        
        // Use device-specific data for consistent identification
        val deviceId = bluetoothAdapter?.address?.hashCode() ?: 0
        val timestamp = (System.currentTimeMillis() / 1000).toInt() // Current time in seconds
        
        // Pack device ID and timestamp
        nodeData[0] = ((deviceId shr 24) and 0xFF).toByte()
        nodeData[1] = ((deviceId shr 16) and 0xFF).toByte()
        nodeData[2] = ((deviceId shr 8) and 0xFF).toByte()
        nodeData[3] = (deviceId and 0xFF).toByte()
        
        nodeData[4] = ((timestamp shr 24) and 0xFF).toByte()
        nodeData[5] = ((timestamp shr 16) and 0xFF).toByte()
        nodeData[6] = ((timestamp shr 8) and 0xFF).toByte()
        nodeData[7] = (timestamp and 0xFF).toByte()
        
        return nodeData
    }
    
    /**
     * Create capabilities data for scan response
     */
    private fun createCapabilitiesData(): ByteArray {
        // Pack BitCraps capabilities into bytes
        val capabilities = ByteArray(4)
        
        // Capability flags
        var flags = 0
        flags = flags or (1 shl 0) // Supports gaming
        flags = flags or (1 shl 1) // Supports mesh routing
        flags = flags or (1 shl 2) // Supports token transactions
        
        capabilities[0] = (flags and 0xFF).toByte()
        capabilities[1] = 1 // Protocol version
        capabilities[2] = 8 // Max game players
        capabilities[3] = 0 // Reserved
        
        return capabilities
    }
    
    /**
     * BLE advertise callback
     */
    private val advertiseCallback = object : AdvertiseCallback() {
        override fun onStartSuccess(settingsInEffect: AdvertiseSettings) {
            super.onStartSuccess(settingsInEffect)
            isAdvertising.set(true)
            Timber.d("BLE advertising started successfully")
        }
        
        override fun onStartFailure(errorCode: Int) {
            super.onStartFailure(errorCode)
            isAdvertising.set(false)
            
            val errorMessage = when (errorCode) {
                ADVERTISE_FAILED_ALREADY_STARTED -> "Already started"
                ADVERTISE_FAILED_DATA_TOO_LARGE -> "Data too large"
                ADVERTISE_FAILED_FEATURE_UNSUPPORTED -> "Feature unsupported"
                ADVERTISE_FAILED_INTERNAL_ERROR -> "Internal error"
                ADVERTISE_FAILED_TOO_MANY_ADVERTISERS -> "Too many advertisers"
                else -> "Unknown error: $errorCode"
            }
            
            Timber.e("BLE advertising failed: %s", errorMessage)
            
            // Retry after delay if not a permanent failure
            if (errorCode != ADVERTISE_FAILED_FEATURE_UNSUPPORTED) {
                advertisingJob = CoroutineScope(Dispatchers.Default).launch {
                    delay(5000) // Wait 5 seconds before retry
                    if (!advertisingJob?.isCancelled!!) {
                        startAdvertising()
                    }
                }
            }
        }
    }
    
    /**
     * Check if required BLE permissions are granted
     */
    private fun hasRequiredPermissions(): Boolean {
        val requiredPermissions = mutableListOf<String>().apply {
            add(Manifest.permission.BLUETOOTH)
            add(Manifest.permission.BLUETOOTH_ADMIN)
            
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                add(Manifest.permission.BLUETOOTH_ADVERTISE)
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
     * Check if BLE advertising is supported
     */
    private fun isAdvertisingSupported(): Boolean {
        return bluetoothAdapter?.isMultipleAdvertisementSupported == true
    }
    
    /**
     * Check if currently advertising
     */
    fun isCurrentlyAdvertising(): Boolean = isAdvertising.get()
    
    /**
     * Get advertising capabilities
     */
    fun getAdvertisingCapabilities(): String {
        return when {
            !isBluetoothEnabled() -> "Bluetooth disabled"
            !isAdvertisingSupported() -> "Advertising not supported"
            !hasRequiredPermissions() -> "Missing permissions"
            isAdvertising.get() -> "Currently advertising"
            else -> "Ready to advertise"
        }
    }
}