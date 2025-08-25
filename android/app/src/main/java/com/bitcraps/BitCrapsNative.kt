package com.bitcraps

/**
 * Native interface for BitCraps Rust library
 * This class provides JNI bindings to the Rust core
 */
object BitCrapsNative {
    
    init {
        System.loadLibrary("bitcraps")
    }
    
    // Library initialization
    external fun initialize(): Boolean
    
    // Node management
    external fun createNode(
        dataDir: String,
        powDifficulty: Int,
        protocolVersion: Int
    ): Long
    
    external fun destroyNode(nodeHandle: Long)
    
    // Discovery operations
    external fun startDiscovery(nodeHandle: Long): Boolean
    external fun stopDiscovery(nodeHandle: Long): Boolean
    
    // Event polling
    external fun pollEvent(nodeHandle: Long): String?
    
    // Status and info
    external fun getNodeStatus(nodeHandle: Long): String?
    
    // Power management
    external fun setPowerMode(nodeHandle: Long, powerMode: Int): Boolean
    
    companion object {
        // Power mode constants
        const val POWER_MODE_HIGH_PERFORMANCE = 0
        const val POWER_MODE_BALANCED = 1
        const val POWER_MODE_BATTERY_SAVER = 2
        const val POWER_MODE_ULTRA_LOW_POWER = 3
    }
}

/**
 * Exception classes for BitCraps errors
 */
sealed class BitCrapsException(message: String, cause: Throwable? = null) : Exception(message, cause)

class InitializationException(message: String, cause: Throwable? = null) : BitCrapsException(message, cause)
class BluetoothException(message: String, cause: Throwable? = null) : BitCrapsException(message, cause)
class NetworkException(message: String, cause: Throwable? = null) : BitCrapsException(message, cause)
class GameException(message: String, cause: Throwable? = null) : BitCrapsException(message, cause)
class CryptoException(message: String, cause: Throwable? = null) : BitCrapsException(message, cause)