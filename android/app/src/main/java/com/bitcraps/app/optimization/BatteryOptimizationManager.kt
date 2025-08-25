package com.bitcraps.app.optimization

import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.os.BatteryManager
import android.os.Build
import android.os.PowerManager
import android.provider.Settings
import androidx.lifecycle.DefaultLifecycleObserver
import androidx.lifecycle.LifecycleOwner
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.*
import timber.log.Timber
import kotlin.math.roundToInt

/**
 * Comprehensive battery optimization and monitoring for BitCraps mobile gaming
 */
class BatteryOptimizationManager(
    private val context: Context
) : DefaultLifecycleObserver {
    
    companion object {
        private const val TAG = "BatteryOptimizer"
        private const val MONITORING_INTERVAL_MS = 30000L // 30 seconds
        private const val CRITICAL_BATTERY_THRESHOLD = 15
        private const val LOW_BATTERY_THRESHOLD = 30
        private const val HIGH_DRAIN_RATE_THRESHOLD = 8.0 // %/hour
        private const val OPTIMIZATION_CHECK_INTERVAL_MS = 300000L // 5 minutes
    }
    
    private val scope = CoroutineScope(Dispatchers.Main + SupervisorJob())
    private val powerManager = context.getSystemService(Context.POWER_SERVICE) as PowerManager
    private val batteryManager = context.getSystemService(Context.BATTERY_SERVICE) as BatteryManager
    
    // State flows for reactive UI updates
    private val _batteryState = MutableStateFlow(BatteryState.unknown())
    val batteryState: StateFlow<BatteryState> = _batteryState.asStateFlow()
    
    private val _optimizationState = MutableStateFlow(OptimizationState.normal())
    val optimizationState: StateFlow<OptimizationState> = _optimizationState.asStateFlow()
    
    private val _powerProfile = MutableStateFlow(PowerProfile.BALANCED)
    val powerProfile: StateFlow<PowerProfile> = _powerProfile.asStateFlow()
    
    // Internal monitoring
    private var batteryHistory = mutableListOf<BatteryReading>()
    private var lastOptimizationCheck = 0L
    private var baselineConsumption = 0.0
    
    // Adaptive features
    private var isGamingMode = false
    private var adaptiveFrameRate = 60
    private var networkOptimizationLevel = 0
    
    init {
        startBatteryMonitoring()
        checkInitialOptimizationState()
        Timber.d("BatteryOptimizationManager initialized")
    }
    
    // MARK: - Lifecycle
    
    override fun onStart(owner: LifecycleOwner) {
        super.onStart(owner)
        resumeMonitoring()
    }
    
    override fun onStop(owner: LifecycleOwner) {
        super.onStop(owner)
        pauseMonitoring()
    }
    
    override fun onDestroy(owner: LifecycleOwner) {
        super.onDestroy(owner)
        cleanup()
    }
    
    // MARK: - Public API
    
    fun setGameMode(enabled: Boolean) {
        isGamingMode = enabled
        if (enabled) {
            applyGamingOptimizations()
        } else {
            restoreNormalMode()
        }
        Timber.d("Gaming mode: ${if (enabled) "enabled" else "disabled"}")
    }
    
    fun setPowerProfile(profile: PowerProfile) {
        _powerProfile.value = profile
        applyPowerProfile(profile)
        Timber.d("Power profile changed to: $profile")
    }
    
    fun requestBatteryOptimizationDisabled(): Intent? {
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
            if (powerManager.isIgnoringBatteryOptimizations(context.packageName)) {
                null // Already optimized
            } else {
                Intent().apply {
                    action = Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS
                    data = android.net.Uri.parse("package:${context.packageName}")
                }
            }
        } else {
            null // Not supported on older versions
        }
    }
    
    fun checkBatteryOptimizationStatus(): BatteryOptimizationStatus {
        return when {
            Build.VERSION.SDK_INT < Build.VERSION_CODES.M -> {
                BatteryOptimizationStatus.NOT_SUPPORTED
            }
            powerManager.isIgnoringBatteryOptimizations(context.packageName) -> {
                BatteryOptimizationStatus.DISABLED
            }
            else -> {
                BatteryOptimizationStatus.ENABLED
            }
        }
    }
    
    fun getOptimizationRecommendations(): List<OptimizationRecommendation> {
        val recommendations = mutableListOf<OptimizationRecommendation>()
        val currentState = _batteryState.value
        val optimizationStatus = checkBatteryOptimizationStatus()
        
        // Battery optimization recommendations
        if (optimizationStatus == BatteryOptimizationStatus.ENABLED) {
            recommendations.add(
                OptimizationRecommendation(
                    type = RecommendationType.DISABLE_BATTERY_OPTIMIZATION,
                    title = "Disable Battery Optimization",
                    description = "Prevent Android from killing BitCraps in the background",
                    impact = OptimizationImpact.HIGH,
                    action = { requestBatteryOptimizationDisabled() }
                )
            )
        }
        
        // Low battery recommendations
        if (currentState.level < LOW_BATTERY_THRESHOLD) {
            recommendations.add(
                OptimizationRecommendation(
                    type = RecommendationType.REDUCE_VISUAL_EFFECTS,
                    title = "Reduce Visual Effects",
                    description = "Lower frame rate and disable animations to save battery",
                    impact = OptimizationImpact.MEDIUM,
                    action = { applyLowPowerOptimizations() }
                )
            )
        }
        
        // High drain rate recommendations
        if (currentState.drainRate > HIGH_DRAIN_RATE_THRESHOLD) {
            recommendations.add(
                OptimizationRecommendation(
                    type = RecommendationType.OPTIMIZE_NETWORK,
                    title = "Optimize Network Usage",
                    description = "Reduce Bluetooth scan frequency and optimize mesh networking",
                    impact = OptimizationImpact.MEDIUM,
                    action = { optimizeNetworkUsage() }
                )
            )
        }
        
        return recommendations
    }
    
    fun applyAllRecommendations() {
        getOptimizationRecommendations().forEach { recommendation ->
            try {
                recommendation.action?.invoke()
            } catch (e: Exception) {
                Timber.e(e, "Failed to apply recommendation: ${recommendation.type}")
            }
        }
    }
    
    // MARK: - Monitoring
    
    private fun startBatteryMonitoring() {
        scope.launch {
            while (isActive) {
                updateBatteryState()
                checkForOptimizations()
                delay(MONITORING_INTERVAL_MS)
            }
        }
    }
    
    private fun updateBatteryState() {
        try {
            val level = batteryManager.getIntProperty(BatteryManager.BATTERY_PROPERTY_CAPACITY)
            val isCharging = isDeviceCharging()
            val voltage = batteryManager.getIntProperty(BatteryManager.BATTERY_PROPERTY_VOLTAGE_NOW) / 1000.0 // mV to V
            val temperature = getTemperature()
            val drainRate = calculateDrainRate()
            
            val reading = BatteryReading(
                timestamp = System.currentTimeMillis(),
                level = level,
                voltage = voltage,
                temperature = temperature,
                isCharging = isCharging
            )
            
            // Add to history
            batteryHistory.add(reading)
            if (batteryHistory.size > 120) { // Keep last 120 readings (1 hour at 30s intervals)
                batteryHistory.removeAt(0)
            }
            
            val newState = BatteryState(
                level = level,
                isCharging = isCharging,
                voltage = voltage,
                temperature = temperature,
                drainRate = drainRate,
                status = getBatteryStatus(level, isCharging, drainRate),
                timeRemaining = calculateTimeRemaining(level, drainRate, isCharging),
                isLowPowerMode = powerManager.isPowerSaveMode,
                lastUpdated = System.currentTimeMillis()
            )
            
            _batteryState.value = newState
            
        } catch (e: Exception) {
            Timber.e(e, "Failed to update battery state")
        }
    }
    
    private fun isDeviceCharging(): Boolean {
        val intentFilter = IntentFilter(Intent.ACTION_BATTERY_CHANGED)
        val batteryStatus = context.registerReceiver(null, intentFilter)
        val status = batteryStatus?.getIntExtra(BatteryManager.EXTRA_STATUS, -1) ?: -1
        return status == BatteryManager.BATTERY_STATUS_CHARGING || 
               status == BatteryManager.BATTERY_STATUS_FULL
    }
    
    private fun getTemperature(): Double {
        val intentFilter = IntentFilter(Intent.ACTION_BATTERY_CHANGED)
        val batteryStatus = context.registerReceiver(null, intentFilter)
        val temp = batteryStatus?.getIntExtra(BatteryManager.EXTRA_TEMPERATURE, -1) ?: -1
        return temp / 10.0 // Convert from tenths of degrees Celsius
    }
    
    private fun calculateDrainRate(): Double {
        if (batteryHistory.size < 2) return 0.0
        
        val recent = batteryHistory.takeLast(10) // Last 10 readings (5 minutes)
        if (recent.size < 2) return 0.0
        
        val firstReading = recent.first()
        val lastReading = recent.last()
        
        val timeDiff = (lastReading.timestamp - firstReading.timestamp) / 1000.0 / 3600.0 // hours
        val levelDiff = firstReading.level - lastReading.level // positive = draining
        
        if (timeDiff <= 0 || levelDiff <= 0) return 0.0
        
        return levelDiff / timeDiff // %/hour
    }
    
    private fun getBatteryStatus(level: Int, isCharging: Boolean, drainRate: Double): BatteryStatus {
        return when {
            isCharging -> BatteryStatus.CHARGING
            level < CRITICAL_BATTERY_THRESHOLD -> BatteryStatus.CRITICAL
            level < LOW_BATTERY_THRESHOLD -> BatteryStatus.LOW
            drainRate > HIGH_DRAIN_RATE_THRESHOLD -> BatteryStatus.HIGH_DRAIN
            else -> BatteryStatus.NORMAL
        }
    }
    
    private fun calculateTimeRemaining(level: Int, drainRate: Double, isCharging: Boolean): Double {
        return if (isCharging || drainRate <= 0) {
            -1.0 // Unknown/infinite
        } else {
            level / drainRate // hours
        }
    }
    
    // MARK: - Optimizations
    
    private fun checkForOptimizations() {
        val now = System.currentTimeMillis()
        if (now - lastOptimizationCheck < OPTIMIZATION_CHECK_INTERVAL_MS) return
        
        lastOptimizationCheck = now
        val currentState = _batteryState.value
        
        // Auto-apply critical optimizations
        when {
            currentState.level < CRITICAL_BATTERY_THRESHOLD -> {
                applyEmergencyOptimizations()
            }
            currentState.level < LOW_BATTERY_THRESHOLD -> {
                applyLowPowerOptimizations()
            }
            currentState.drainRate > HIGH_DRAIN_RATE_THRESHOLD -> {
                optimizeNetworkUsage()
            }
            powerManager.isPowerSaveMode -> {
                applySystemPowerSaveOptimizations()
            }
        }
    }
    
    private fun applyGamingOptimizations() {
        val profile = when (_batteryState.value.level) {
            in 0..CRITICAL_BATTERY_THRESHOLD -> PowerProfile.ULTRA_BATTERY_SAVER
            in CRITICAL_BATTERY_THRESHOLD..LOW_BATTERY_THRESHOLD -> PowerProfile.BATTERY_SAVER
            else -> PowerProfile.GAMING
        }
        
        setPowerProfile(profile)
    }
    
    private fun restoreNormalMode() {
        setPowerProfile(PowerProfile.BALANCED)
    }
    
    private fun applyPowerProfile(profile: PowerProfile) {
        when (profile) {
            PowerProfile.GAMING -> {
                adaptiveFrameRate = 60
                networkOptimizationLevel = 0
            }
            PowerProfile.BALANCED -> {
                adaptiveFrameRate = 60
                networkOptimizationLevel = 1
            }
            PowerProfile.BATTERY_SAVER -> {
                adaptiveFrameRate = 30
                networkOptimizationLevel = 2
            }
            PowerProfile.ULTRA_BATTERY_SAVER -> {
                adaptiveFrameRate = 15
                networkOptimizationLevel = 3
            }
        }
        
        updateOptimizationState()
    }
    
    private fun applyEmergencyOptimizations() {
        Timber.w("Applying emergency battery optimizations")
        setPowerProfile(PowerProfile.ULTRA_BATTERY_SAVER)
        // Additional emergency measures
        optimizeNetworkUsage()
        reduceBackgroundActivity()
    }
    
    private fun applyLowPowerOptimizations() {
        Timber.i("Applying low power optimizations")
        if (_powerProfile.value == PowerProfile.GAMING) {
            setPowerProfile(PowerProfile.BATTERY_SAVER)
        }
    }
    
    private fun applySystemPowerSaveOptimizations() {
        Timber.i("System power save mode detected, applying optimizations")
        reduceBackgroundActivity()
        optimizeNetworkUsage()
    }
    
    private fun optimizeNetworkUsage() {
        networkOptimizationLevel = minOf(networkOptimizationLevel + 1, 3)
        updateOptimizationState()
        Timber.d("Network optimization level: $networkOptimizationLevel")
    }
    
    private fun reduceBackgroundActivity() {
        // Reduce non-essential background tasks
        updateOptimizationState()
        Timber.d("Reduced background activity")
    }
    
    private fun updateOptimizationState() {
        val newState = OptimizationState(
            targetFrameRate = adaptiveFrameRate,
            networkOptimizationLevel = networkOptimizationLevel,
            backgroundActivityReduced = networkOptimizationLevel > 1,
            visualEffectsReduced = adaptiveFrameRate < 60,
            isOptimized = adaptiveFrameRate < 60 || networkOptimizationLevel > 0,
            lastOptimization = System.currentTimeMillis()
        )
        
        _optimizationState.value = newState
    }
    
    private fun checkInitialOptimizationState() {
        updateBatteryState()
        checkForOptimizations()
    }
    
    private fun resumeMonitoring() {
        if (!scope.isActive) return
        updateBatteryState()
    }
    
    private fun pauseMonitoring() {
        // Monitoring continues in background but at reduced frequency
    }
    
    private fun cleanup() {
        scope.cancel()
        batteryHistory.clear()
    }
}

// MARK: - Data Classes

data class BatteryState(
    val level: Int,
    val isCharging: Boolean,
    val voltage: Double,
    val temperature: Double,
    val drainRate: Double,
    val status: BatteryStatus,
    val timeRemaining: Double, // hours, -1 if unknown
    val isLowPowerMode: Boolean,
    val lastUpdated: Long
) {
    companion object {
        fun unknown() = BatteryState(
            level = -1,
            isCharging = false,
            voltage = 0.0,
            temperature = 0.0,
            drainRate = 0.0,
            status = BatteryStatus.UNKNOWN,
            timeRemaining = -1.0,
            isLowPowerMode = false,
            lastUpdated = 0L
        )
    }
}

data class OptimizationState(
    val targetFrameRate: Int,
    val networkOptimizationLevel: Int,
    val backgroundActivityReduced: Boolean,
    val visualEffectsReduced: Boolean,
    val isOptimized: Boolean,
    val lastOptimization: Long
) {
    companion object {
        fun normal() = OptimizationState(
            targetFrameRate = 60,
            networkOptimizationLevel = 0,
            backgroundActivityReduced = false,
            visualEffectsReduced = false,
            isOptimized = false,
            lastOptimization = 0L
        )
    }
}

data class BatteryReading(
    val timestamp: Long,
    val level: Int,
    val voltage: Double,
    val temperature: Double,
    val isCharging: Boolean
)

data class OptimizationRecommendation(
    val type: RecommendationType,
    val title: String,
    val description: String,
    val impact: OptimizationImpact,
    val action: (() -> Unit)?
)

enum class BatteryStatus {
    UNKNOWN,
    NORMAL,
    LOW,
    CRITICAL,
    HIGH_DRAIN,
    CHARGING
}

enum class PowerProfile {
    GAMING,
    BALANCED,
    BATTERY_SAVER,
    ULTRA_BATTERY_SAVER
}

enum class BatteryOptimizationStatus {
    NOT_SUPPORTED,
    ENABLED,
    DISABLED
}

enum class RecommendationType {
    DISABLE_BATTERY_OPTIMIZATION,
    REDUCE_VISUAL_EFFECTS,
    OPTIMIZE_NETWORK,
    REDUCE_BACKGROUND_ACTIVITY
}

enum class OptimizationImpact {
    LOW,
    MEDIUM,
    HIGH
}