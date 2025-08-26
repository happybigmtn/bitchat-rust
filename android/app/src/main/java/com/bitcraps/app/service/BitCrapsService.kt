package com.bitcraps.app.service

import android.app.*
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.IBinder
import android.Manifest
import androidx.core.app.NotificationCompat
import androidx.core.content.ContextCompat
import timber.log.Timber
import com.bitcraps.app.R
import com.bitcraps.app.ble.BleManager
import com.bitcraps.app.ble.BleAdvertiser
import com.bitcraps.app.ble.BleService
import com.bitcraps.app.ble.BleJNI
import kotlinx.coroutines.*
import java.util.concurrent.atomic.AtomicBoolean

/**
 * Android Foreground Service for BitCraps mesh networking.
 * 
 * This service handles:
 * - Background BLE scanning for peer discovery
 * - BLE advertising for peer visibility
 * - BitCraps node lifecycle management
 * - Compliance with Android 14+ foreground service requirements
 */
class BitCrapsService : Service() {
    
    companion object {
        const val NOTIFICATION_ID = 1001
        const val CHANNEL_ID = "bitcraps_service"
        const val ACTION_START_SERVICE = "START_BITCRAPS_SERVICE"
        const val ACTION_STOP_SERVICE = "STOP_BITCRAPS_SERVICE"
        
        // Native library functions
        external fun startNode(dataDir: String, nickname: String, difficulty: Long): Long
        external fun stopNode(appPtr: Long)
        external fun createGame(appPtr: Long, buyIn: Long): String
        external fun joinGame(appPtr: Long, gameId: String): Boolean
        external fun getBalance(appPtr: Long): Long
        
        init {
            System.loadLibrary("bitcraps")
        }
        
        fun startService(context: Context) {
            val intent = Intent(context, BitCrapsService::class.java).apply {
                action = ACTION_START_SERVICE
            }
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                context.startForegroundService(intent)
            } else {
                context.startService(intent)
            }
        }
        
        fun stopService(context: Context) {
            val intent = Intent(context, BitCrapsService::class.java).apply {
                action = ACTION_STOP_SERVICE
            }
            context.startService(intent)
        }
    }
    
    private var serviceJob = SupervisorJob()
    private var serviceScope = CoroutineScope(Dispatchers.Default + serviceJob)
    
    private var bleManager: BleManager? = null
    private var bleAdvertiser: BleAdvertiser? = null
    private var bitcrapsNodePtr: Long = 0L
    
    private val isServiceRunning = AtomicBoolean(false)
    private val hasRequiredPermissions = AtomicBoolean(false)
    
    override fun onCreate() {
        super.onCreate()
        Timber.d("BitCrapsService created")
        createNotificationChannel()
        checkPermissions()
    }
    
    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_START_SERVICE -> startForegroundService()
            ACTION_STOP_SERVICE -> stopForegroundService()
            else -> startForegroundService()
        }
        
        // Return START_STICKY to restart service if killed by system
        return START_STICKY
    }
    
    override fun onBind(intent: Intent?): IBinder? = null
    
    override fun onDestroy() {
        super.onDestroy()
        Timber.d("BitCrapsService destroyed")
        stopForegroundService()
        serviceJob.cancel()
    }
    
    /**
     * Check if all required permissions are granted for Android 14+
     */
    private fun checkPermissions(): Boolean {
        val requiredPermissions = mutableListOf<String>().apply {
            // Basic BLE permissions
            add(Manifest.permission.BLUETOOTH)
            add(Manifest.permission.BLUETOOTH_ADMIN)
            
            // Android 12+ runtime permissions
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                add(Manifest.permission.BLUETOOTH_SCAN)
                add(Manifest.permission.BLUETOOTH_ADVERTISE)
                add(Manifest.permission.BLUETOOTH_CONNECT)
            }
            
            // Location permissions for older Android versions
            if (Build.VERSION.SDK_INT < Build.VERSION_CODES.S) {
                add(Manifest.permission.ACCESS_FINE_LOCATION)
                add(Manifest.permission.ACCESS_COARSE_LOCATION)
            }
            
            // Notification permission for Android 13+
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                add(Manifest.permission.POST_NOTIFICATIONS)
            }
        }
        
        val hasAllPermissions = requiredPermissions.all { permission ->
            ContextCompat.checkSelfPermission(this, permission) == PackageManager.PERMISSION_GRANTED
        }
        
        hasRequiredPermissions.set(hasAllPermissions)
        
        if (!hasAllPermissions) {
            val missingPermissions = requiredPermissions.filter { permission ->
                ContextCompat.checkSelfPermission(this, permission) != PackageManager.PERMISSION_GRANTED
            }
            Timber.w("Missing required permissions: %s", missingPermissions)
        }
        
        return hasAllPermissions
    }
    
    /**
     * Start foreground service with proper Android 14+ compliance
     */
    private fun startForegroundService() {
        if (isServiceRunning.get()) {
            Timber.d("Service already running")
            return
        }
        
        Timber.i("Starting BitCraps foreground service")
        
        // Create foreground notification
        val notification = createServiceNotification()
        
        // Start foreground service with connected device type for Android 14+
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            startForeground(
                NOTIFICATION_ID, 
                notification,
                ServiceInfo.FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE
            )
        } else {
            startForeground(NOTIFICATION_ID, notification)
        }
        
        isServiceRunning.set(true)
        
        // Initialize BitCraps node and BLE components
        serviceScope.launch {
            try {
                initializeBitCrapsNode()
                initializeBleComponents()
                startBleOperations()
            } catch (e: Exception) {
                Timber.e(e, "Failed to start service components")
                stopForegroundService()
            }
        }
    }
    
    /**
     * Stop foreground service and cleanup resources
     */
    private fun stopForegroundService() {
        if (!isServiceRunning.get()) {
            Timber.d("Service not running")
            return
        }
        
        Timber.i("Stopping BitCraps foreground service")
        
        isServiceRunning.set(false)
        
        // Stop BLE operations
        serviceScope.launch {
            stopBleOperations()
            cleanupBitCrapsNode()
        }
        
        // Stop foreground service
        stopForeground(STOP_FOREGROUND_REMOVE)
        stopSelf()
    }
    
    /**
     * Initialize BitCraps native node
     */
    private suspend fun initializeBitCrapsNode() = withContext(Dispatchers.Default) {
        try {
            val dataDir = "${filesDir.absolutePath}/bitcraps"
            val nickname = "AndroidNode"
            val difficulty = 4L
            
            Timber.d("Initializing BitCraps node: dataDir=%s, nickname=%s", dataDir, nickname)
            
            bitcrapsNodePtr = startNode(dataDir, nickname, difficulty)
            
            if (bitcrapsNodePtr == 0L) {
                throw RuntimeException("Failed to initialize BitCraps node")
            }
            
            Timber.i("BitCraps node initialized successfully: ptr=%d", bitcrapsNodePtr)
            
            // Update notification to show node is running
            val notification = createServiceNotification("Node Running")
            val notificationManager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
            notificationManager.notify(NOTIFICATION_ID, notification)
            
        } catch (e: Exception) {
            Timber.e(e, "Failed to initialize BitCraps node")
            throw e
        }
    }
    
    /**
     * Cleanup BitCraps native node
     */
    private suspend fun cleanupBitCrapsNode() = withContext(Dispatchers.Default) {
        if (bitcrapsNodePtr != 0L) {
            try {
                Timber.d("Cleaning up BitCraps node: ptr=%d", bitcrapsNodePtr)
                stopNode(bitcrapsNodePtr)
                bitcrapsNodePtr = 0L
                Timber.i("BitCraps node cleaned up successfully")
            } catch (e: Exception) {
                Timber.e(e, "Error cleaning up BitCraps node")
            }
        }
    }
    
    /**
     * Initialize BLE components
     */
    private suspend fun initializeBleComponents() = withContext(Dispatchers.Main) {
        if (!hasRequiredPermissions.get()) {
            throw SecurityException("Missing required BLE permissions")
        }
        
        try {
            bleManager = BleManager(this@BitCrapsService)
            bleAdvertiser = BleAdvertiser(this@BitCrapsService)
            
            Timber.i("BLE components initialized")
        } catch (e: Exception) {
            Timber.e(e, "Failed to initialize BLE components")
            throw e
        }
    }
    
    /**
     * Start BLE scanning and advertising
     */
    private suspend fun startBleOperations() {
        if (!hasRequiredPermissions.get()) {
            Timber.w("Cannot start BLE operations without required permissions")
            return
        }
        
        serviceScope.launch {
            try {
                // Start BLE scanning for peer discovery
                bleManager?.startScanning()
                Timber.i("BLE scanning started")
                
                // Start BLE advertising for peer visibility
                bleAdvertiser?.startAdvertising()
                Timber.i("BLE advertising started")
                
                // Update notification
                val notification = createServiceNotification("BLE Active")
                val notificationManager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
                notificationManager.notify(NOTIFICATION_ID, notification)
                
            } catch (e: SecurityException) {
                Timber.e(e, "Security exception in BLE operations - missing permissions")
            } catch (e: Exception) {
                Timber.e(e, "Failed to start BLE operations")
            }
        }
    }
    
    /**
     * Stop BLE operations
     */
    private suspend fun stopBleOperations() {
        try {
            bleAdvertiser?.stopAdvertising()
            bleManager?.stopScanning()
            Timber.i("BLE operations stopped")
        } catch (e: Exception) {
            Timber.e(e, "Error stopping BLE operations")
        }
    }
    
    /**
     * Create notification channel for Android O+
     */
    private fun createNotificationChannel() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channel = NotificationChannel(
                CHANNEL_ID,
                "BitCraps Service",
                NotificationManager.IMPORTANCE_LOW
            ).apply {
                description = "BitCraps mesh networking service"
                enableLights(false)
                enableVibration(false)
                setShowBadge(false)
            }
            
            val notificationManager = getSystemService(NotificationManager::class.java)
            notificationManager.createNotificationChannel(channel)
        }
    }
    
    /**
     * Create service notification for foreground service
     */
    private fun createServiceNotification(status: String = "Starting"): Notification {
        val stopIntent = Intent(this, BitCrapsService::class.java).apply {
            action = ACTION_STOP_SERVICE
        }
        val stopPendingIntent = PendingIntent.getService(
            this,
            0,
            stopIntent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )
        
        return NotificationCompat.Builder(this, CHANNEL_ID)
            .setContentTitle("BitCraps Mesh Network")
            .setContentText("Status: $status")
            .setSmallIcon(R.drawable.ic_notification)
            .setOngoing(true)
            .setLocalOnly(true)
            .setAutoCancel(false)
            .addAction(
                R.drawable.ic_stop,
                "Stop",
                stopPendingIntent
            )
            .build()
    }
    
    // Public API methods for interaction with the service
    
    /**
     * Get current balance from BitCraps node
     */
    fun getCurrentBalance(): Long {
        return if (bitcrapsNodePtr != 0L) {
            try {
                getBalance(bitcrapsNodePtr)
            } catch (e: Exception) {
                Timber.e(e, "Error getting balance")
                0L
            }
        } else {
            0L
        }
    }
    
    /**
     * Create a new game
     */
    fun createNewGame(buyIn: Long): String? {
        return if (bitcrapsNodePtr != 0L) {
            try {
                createGame(bitcrapsNodePtr, buyIn)
            } catch (e: Exception) {
                Timber.e(e, "Error creating game")
                null
            }
        } else {
            null
        }
    }
    
    /**
     * Join an existing game
     */
    fun joinExistingGame(gameId: String): Boolean {
        return if (bitcrapsNodePtr != 0L) {
            try {
                joinGame(bitcrapsNodePtr, gameId)
            } catch (e: Exception) {
                Timber.e(e, "Error joining game")
                false
            }
        } else {
            false
        }
    }
    
    /**
     * Check if service is currently running
     */
    fun isRunning(): Boolean = isServiceRunning.get()
    
    /**
     * Get discovered peers count
     */
    fun getDiscoveredPeersCount(): Int {
        return bleManager?.getDiscoveredPeersCount() ?: 0
    }
}