package com.bitcraps.app.ble

import android.app.Service
import android.content.Intent
import android.os.Binder
import android.os.IBinder
import timber.log.Timber
import kotlinx.coroutines.*
import java.util.concurrent.atomic.AtomicBoolean

/**
 * BLE Service that integrates with the Rust JNI bridge
 * This service provides a unified interface for BLE operations
 * and manages the integration between Android BLE and the Rust implementation
 */
class BleService : Service() {
    
    companion object {
        const val ACTION_START_ADVERTISING = "com.bitcraps.ble.START_ADVERTISING"
        const val ACTION_STOP_ADVERTISING = "com.bitcraps.ble.STOP_ADVERTISING"
        const val ACTION_START_SCANNING = "com.bitcraps.ble.START_SCANNING"
        const val ACTION_STOP_SCANNING = "com.bitcraps.ble.STOP_SCANNING"
    }
    
    private val binder = BleBinder()
    private val serviceScope = CoroutineScope(Dispatchers.Main + SupervisorJob())
    
    private lateinit var bleAdvertiser: BleAdvertiser
    private lateinit var bleManager: BleManager
    
    private val isInitialized = AtomicBoolean(false)
    
    inner class BleBinder : Binder() {
        fun getService(): BleService = this@BleService
    }
    
    override fun onCreate() {
        super.onCreate()
        
        Timber.i("BleService created")
        
        // Initialize BLE components
        bleAdvertiser = BleAdvertiser(this)
        bleManager = BleManager(this)
        
        // Initialize JNI bridge
        initializeJNIBridge()
    }
    
    override fun onBind(intent: Intent?): IBinder {
        return binder
    }
    
    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        intent?.let { handleIntent(it) }
        return START_STICKY
    }
    
    override fun onDestroy() {
        super.onDestroy()
        
        Timber.i("BleService destroyed")
        
        // Clean up
        serviceScope.cancel()
        
        // Stop all BLE operations
        stopAdvertising()
        stopScanning()
        
        // Clean up JNI
        cleanupJNIBridge()
    }
    
    private fun handleIntent(intent: Intent) {
        when (intent.action) {
            ACTION_START_ADVERTISING -> startAdvertising()
            ACTION_STOP_ADVERTISING -> stopAdvertising()
            ACTION_START_SCANNING -> startScanning()
            ACTION_STOP_SCANNING -> stopScanning()
        }
    }
    
    private fun initializeJNIBridge() {
        serviceScope.launch {
            try {
                val success = BleJNI.initialize(this@BleService)
                if (success) {
                    isInitialized.set(true)
                    Timber.i("JNI bridge initialized successfully")
                } else {
                    Timber.e("Failed to initialize JNI bridge")
                }
            } catch (e: Exception) {
                Timber.e(e, "Exception during JNI bridge initialization")
            }
        }
    }
    
    private fun cleanupJNIBridge() {
        if (isInitialized.get()) {
            try {
                // Stop any active operations
                if (BleJNI.isAdvertising()) {
                    BleJNI.stopAdvertising()
                }
                if (BleJNI.isScanning()) {
                    BleJNI.stopScanning()
                }
                
                isInitialized.set(false)
                Timber.i("JNI bridge cleaned up")
            } catch (e: Exception) {
                Timber.e(e, "Exception during JNI cleanup")
            }
        }
    }
    
    // Public API methods
    
    /**
     * Start BLE advertising
     */
    fun startAdvertising(): Boolean {
        if (!isInitialized.get()) {
            Timber.w("Cannot start advertising - JNI not initialized")
            return false
        }
        
        return try {
            // Start Android BLE advertising
            bleAdvertiser.startAdvertising()
            
            // Start Rust BLE advertising via JNI
            val rustSuccess = BleJNI.startAdvertising()
            
            if (rustSuccess) {
                Timber.i("BLE advertising started (Android + Rust)")
                true
            } else {
                Timber.w("Rust advertising failed, but Android advertising may be active")
                true // Still return success if Android part works
            }
        } catch (e: Exception) {
            Timber.e(e, "Failed to start BLE advertising")
            false
        }
    }
    
    /**
     * Stop BLE advertising
     */
    fun stopAdvertising(): Boolean {
        return try {
            // Stop Android BLE advertising
            bleAdvertiser.stopAdvertising()
            
            // Stop Rust BLE advertising via JNI
            if (isInitialized.get()) {
                BleJNI.stopAdvertising()
            }
            
            Timber.i("BLE advertising stopped")
            true
        } catch (e: Exception) {
            Timber.e(e, "Failed to stop BLE advertising")
            false
        }
    }
    
    /**
     * Start BLE scanning
     */
    fun startScanning(): Boolean {
        if (!isInitialized.get()) {
            Timber.w("Cannot start scanning - JNI not initialized")
            return false
        }
        
        return try {
            // Start Android BLE scanning
            bleManager.startScanning()
            
            // Start Rust BLE scanning via JNI
            val rustSuccess = BleJNI.startScanning()
            
            if (rustSuccess) {
                Timber.i("BLE scanning started (Android + Rust)")
                true
            } else {
                Timber.w("Rust scanning failed, but Android scanning may be active")
                true // Still return success if Android part works
            }
        } catch (e: Exception) {
            Timber.e(e, "Failed to start BLE scanning")
            false
        }
    }
    
    /**
     * Stop BLE scanning
     */
    fun stopScanning(): Boolean {
        return try {
            // Stop Android BLE scanning
            bleManager.stopScanning()
            
            // Stop Rust BLE scanning via JNI
            if (isInitialized.get()) {
                BleJNI.stopScanning()
            }
            
            Timber.i("BLE scanning stopped")
            true
        } catch (e: Exception) {
            Timber.e(e, "Failed to stop BLE scanning")
            false
        }
    }
    
    /**
     * Get BLE status information
     */
    fun getBleStatus(): BleStatus {
        val androidAdvertising = bleAdvertiser.isCurrentlyAdvertising()
        val androidScanning = bleManager.isCurrentlyScanning()
        val discoveredPeersCount = bleManager.getDiscoveredPeersCount()
        
        val rustAdvertising = if (isInitialized.get()) {
            try { BleJNI.isAdvertising() } catch (e: Exception) { false }
        } else false
        
        val rustScanning = if (isInitialized.get()) {
            try { BleJNI.isScanning() } catch (e: Exception) { false }
        } else false
        
        val rustPeersCount = if (isInitialized.get()) {
            try { BleJNI.getDiscoveredPeersCount() } catch (e: Exception) { -1 }
        } else -1
        
        return BleStatus(
            jniInitialized = isInitialized.get(),
            androidAdvertising = androidAdvertising,
            androidScanning = androidScanning,
            rustAdvertising = rustAdvertising,
            rustScanning = rustScanning,
            androidDiscoveredPeers = discoveredPeersCount,
            rustDiscoveredPeers = rustPeersCount,
            advertisingCapabilities = bleAdvertiser.getAdvertisingCapabilities()
        )
    }
    
    /**
     * Get discovered peers from Android layer
     */
    fun getAndroidDiscoveredPeers(): List<BleManager.DiscoveredPeer> {
        return bleManager.getDiscoveredPeers()
    }
    
    /**
     * Get discovered peer addresses from Rust layer
     */
    fun getRustDiscoveredPeerAddresses(): List<String> {
        return if (isInitialized.get()) {
            try {
                BleJNI.getDiscoveredPeerAddresses().toList()
            } catch (e: Exception) {
                Timber.e(e, "Failed to get Rust discovered peers")
                emptyList()
            }
        } else {
            emptyList()
        }
    }
    
    /**
     * Clean up expired peers
     */
    fun cleanupExpiredPeers() {
        bleManager.cleanupExpiredPeers()
    }
}

/**
 * BLE Status data class
 */
data class BleStatus(
    val jniInitialized: Boolean,
    val androidAdvertising: Boolean,
    val androidScanning: Boolean,
    val rustAdvertising: Boolean,
    val rustScanning: Boolean,
    val androidDiscoveredPeers: Int,
    val rustDiscoveredPeers: Int,
    val advertisingCapabilities: String
)