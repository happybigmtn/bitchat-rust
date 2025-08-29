package com.bitcraps.sdk.exceptions

import com.bitcraps.sdk.models.BitCrapsError

/**
 * Base exception class for all BitCraps SDK errors
 */
abstract class BitCrapsException(
    message: String,
    cause: Throwable? = null
) : Exception(message, cause) {
    abstract val errorCode: String
    abstract val errorType: String
    open val recoverable: Boolean = true
    open val retryable: Boolean = false
    
    /**
     * Convert to BitCrapsError model for serialization
     */
    abstract fun toBitCrapsError(): BitCrapsError
}

/**
 * SDK initialization failed
 */
class InitializationException(
    message: String,
    cause: Throwable? = null,
    override val retryable: Boolean = true
) : BitCrapsException(message, cause) {
    override val errorCode = "INIT_FAILED"
    override val errorType = "Initialization"
    override val recoverable = false
    
    override fun toBitCrapsError(): BitCrapsError {
        return BitCrapsError.InitializationError(message ?: "Initialization failed")
    }
}

/**
 * Bluetooth related errors
 */
class BluetoothException(
    message: String,
    val bluetoothErrorCode: Int? = null,
    cause: Throwable? = null
) : BitCrapsException(message, cause) {
    override val errorCode = "BLE_ERROR"
    override val errorType = "Bluetooth"
    override val retryable = true
    
    override fun toBitCrapsError(): BitCrapsError {
        return BitCrapsError.BluetoothError(
            message = message ?: "Bluetooth error",
            errorCode = bluetoothErrorCode
        )
    }
    
    companion object {
        // Common Bluetooth error codes
        const val ERROR_ADAPTER_NOT_AVAILABLE = 1001
        const val ERROR_ADAPTER_DISABLED = 1002
        const val ERROR_ADVERTISING_NOT_SUPPORTED = 1003
        const val ERROR_SCANNING_NOT_SUPPORTED = 1004
        const val ERROR_CONNECTION_FAILED = 1005
        const val ERROR_GATT_FAILURE = 1006
        const val ERROR_PERMISSION_DENIED = 1007
        const val ERROR_SERVICE_DISCOVERY_FAILED = 1008
        const val ERROR_CHARACTERISTIC_READ_FAILED = 1009
        const val ERROR_CHARACTERISTIC_WRITE_FAILED = 1010
    }
}

/**
 * Network communication errors
 */
class NetworkException(
    message: String,
    val networkErrorType: NetworkErrorType = NetworkErrorType.UNKNOWN,
    cause: Throwable? = null,
    override val retryable: Boolean = true
) : BitCrapsException(message, cause) {
    override val errorCode = "NETWORK_ERROR"
    override val errorType = "Network"
    
    override fun toBitCrapsError(): BitCrapsError {
        return BitCrapsError.NetworkError(
            message = message ?: "Network error",
            networkErrorType = when (networkErrorType) {
                NetworkErrorType.CONNECTION_TIMEOUT -> com.bitcraps.sdk.models.NetworkErrorType.CONNECTION_TIMEOUT
                NetworkErrorType.CONNECTION_REFUSED -> com.bitcraps.sdk.models.NetworkErrorType.CONNECTION_REFUSED
                NetworkErrorType.PEER_UNREACHABLE -> com.bitcraps.sdk.models.NetworkErrorType.PEER_UNREACHABLE
                NetworkErrorType.MESSAGE_SEND_FAILED -> com.bitcraps.sdk.models.NetworkErrorType.MESSAGE_SEND_FAILED
                NetworkErrorType.PROTOCOL_ERROR -> com.bitcraps.sdk.models.NetworkErrorType.PROTOCOL_ERROR
                NetworkErrorType.CONSENSUS_FAILED -> com.bitcraps.sdk.models.NetworkErrorType.CONSENSUS_FAILED
                NetworkErrorType.UNKNOWN -> com.bitcraps.sdk.models.NetworkErrorType.PROTOCOL_ERROR
            }
        )
    }
    
    enum class NetworkErrorType {
        CONNECTION_TIMEOUT,
        CONNECTION_REFUSED,
        PEER_UNREACHABLE,
        MESSAGE_SEND_FAILED,
        PROTOCOL_ERROR,
        CONSENSUS_FAILED,
        UNKNOWN
    }
}

/**
 * Game logic and state errors
 */
class GameException(
    message: String,
    val gameId: String? = null,
    val gameErrorType: GameErrorType = GameErrorType.UNKNOWN,
    cause: Throwable? = null,
    override val retryable: Boolean = false
) : BitCrapsException(message, cause) {
    override val errorCode = "GAME_ERROR"
    override val errorType = "Game"
    
    override fun toBitCrapsError(): BitCrapsError {
        return BitCrapsError.GameError(
            message = message ?: "Game error",
            gameId = gameId
        )
    }
    
    enum class GameErrorType {
        INVALID_MOVE,
        INSUFFICIENT_BALANCE,
        GAME_NOT_FOUND,
        GAME_FULL,
        GAME_ENDED,
        INVALID_BET,
        PLAYER_NOT_FOUND,
        CONSENSUS_FAILURE,
        CHEATING_DETECTED,
        UNKNOWN
    }
}

/**
 * Security and authentication errors
 */
class SecurityException(
    message: String,
    val securityContext: String? = null,
    val securityErrorType: SecurityErrorType = SecurityErrorType.UNKNOWN,
    cause: Throwable? = null,
    override val retryable: Boolean = false
) : BitCrapsException(message, cause) {
    override val errorCode = "SECURITY_ERROR"
    override val errorType = "Security"
    override val recoverable = false
    
    override fun toBitCrapsError(): BitCrapsError {
        return BitCrapsError.SecurityError(
            message = message ?: "Security error",
            securityContext = securityContext
        )
    }
    
    enum class SecurityErrorType {
        AUTHENTICATION_FAILED,
        BIOMETRIC_AUTHENTICATION_FAILED,
        ENCRYPTION_FAILED,
        DECRYPTION_FAILED,
        KEY_GENERATION_FAILED,
        SIGNATURE_VERIFICATION_FAILED,
        TAMPER_DETECTED,
        UNTRUSTED_PEER,
        UNKNOWN
    }
}

/**
 * Permission related errors
 */
class PermissionException(
    message: String,
    val permissionType: PermissionType,
    cause: Throwable? = null,
    override val retryable: Boolean = false
) : BitCrapsException(message, cause) {
    override val errorCode = "PERMISSION_ERROR"
    override val errorType = "Permission"
    
    override fun toBitCrapsError(): BitCrapsError {
        return BitCrapsError.PermissionError(
            message = message ?: "Permission error",
            permissionType = permissionType.name
        )
    }
    
    enum class PermissionType {
        BLUETOOTH,
        BLUETOOTH_ADVERTISE,
        BLUETOOTH_CONNECT,
        BLUETOOTH_SCAN,
        LOCATION,
        CAMERA,
        MICROPHONE,
        STORAGE,
        BIOMETRIC,
        NETWORK_STATE
    }
}

/**
 * Configuration and validation errors
 */
class ConfigurationException(
    message: String,
    val configField: String? = null,
    cause: Throwable? = null,
    override val retryable: Boolean = false
) : BitCrapsException(message, cause) {
    override val errorCode = "CONFIG_ERROR"
    override val errorType = "Configuration"
    
    override fun toBitCrapsError(): BitCrapsError {
        return BitCrapsError.InitializationError(message ?: "Configuration error")
    }
}

/**
 * Platform specific errors
 */
class PlatformException(
    message: String,
    val platformErrorType: PlatformErrorType = PlatformErrorType.UNKNOWN,
    cause: Throwable? = null
) : BitCrapsException(message, cause) {
    override val errorCode = "PLATFORM_ERROR"
    override val errorType = "Platform"
    
    override fun toBitCrapsError(): BitCrapsError {
        return BitCrapsError.InitializationError(message ?: "Platform error")
    }
    
    enum class PlatformErrorType {
        UNSUPPORTED_OS_VERSION,
        HARDWARE_NOT_SUPPORTED,
        DRIVER_ISSUE,
        RESOURCE_UNAVAILABLE,
        UNKNOWN
    }
}

/**
 * Resource and system errors
 */
class ResourceException(
    message: String,
    val resourceType: ResourceType,
    cause: Throwable? = null,
    override val retryable: Boolean = true
) : BitCrapsException(message, cause) {
    override val errorCode = "RESOURCE_ERROR"
    override val errorType = "Resource"
    
    override fun toBitCrapsError(): BitCrapsError {
        return BitCrapsError.InitializationError(message ?: "Resource error")
    }
    
    enum class ResourceType {
        MEMORY,
        STORAGE,
        BATTERY,
        CPU,
        NETWORK_BANDWIDTH,
        FILE_HANDLE,
        THREAD_POOL
    }
}

/**
 * State consistency errors
 */
class StateException(
    message: String,
    val expectedState: String? = null,
    val actualState: String? = null,
    cause: Throwable? = null,
    override val retryable: Boolean = false
) : BitCrapsException(message, cause) {
    override val errorCode = "STATE_ERROR"
    override val errorType = "State"
    
    override fun toBitCrapsError(): BitCrapsError {
        return BitCrapsError.GameError(
            message = message ?: "State error",
            gameId = null
        )
    }
}

/**
 * Timeout related errors
 */
class TimeoutException(
    message: String,
    val timeoutDurationMs: Long,
    val operation: String? = null,
    cause: Throwable? = null,
    override val retryable: Boolean = true
) : BitCrapsException(message, cause) {
    override val errorCode = "TIMEOUT_ERROR"
    override val errorType = "Timeout"
    
    override fun toBitCrapsError(): BitCrapsError {
        return BitCrapsError.NetworkError(
            message = message ?: "Operation timed out",
            networkErrorType = com.bitcraps.sdk.models.NetworkErrorType.CONNECTION_TIMEOUT
        )
    }
}

/**
 * Data validation errors
 */
class ValidationException(
    message: String,
    val fieldName: String? = null,
    val fieldValue: String? = null,
    cause: Throwable? = null,
    override val retryable: Boolean = false
) : BitCrapsException(message, cause) {
    override val errorCode = "VALIDATION_ERROR"
    override val errorType = "Validation"
    
    override fun toBitCrapsError(): BitCrapsError {
        return BitCrapsError.GameError(
            message = message ?: "Validation error",
            gameId = null
        )
    }
}

/**
 * Utility functions for exception handling
 */
object ExceptionUtils {
    
    /**
     * Check if an exception is recoverable
     */
    fun isRecoverable(exception: Throwable): Boolean {
        return when (exception) {
            is BitCrapsException -> exception.recoverable
            else -> true // Assume unknown exceptions are recoverable
        }
    }
    
    /**
     * Check if an exception warrants a retry
     */
    fun isRetryable(exception: Throwable): Boolean {
        return when (exception) {
            is BitCrapsException -> exception.retryable
            else -> false
        }
    }
    
    /**
     * Get user-friendly error message
     */
    fun getUserFriendlyMessage(exception: Throwable): String {
        return when (exception) {
            is BluetoothException -> "Bluetooth connection issue. Please check your Bluetooth settings."
            is NetworkException -> "Network connection problem. Please check your connection."
            is GameException -> "Game error occurred. Please try again."
            is SecurityException -> "Security verification failed. Please authenticate again."
            is PermissionException -> "Permission required. Please grant the necessary permissions."
            is TimeoutException -> "Operation timed out. Please try again."
            is BitCrapsException -> exception.message ?: "An error occurred"
            else -> exception.message ?: "An unexpected error occurred"
        }
    }
    
    /**
     * Get suggested recovery actions
     */
    fun getRecoveryActions(exception: Throwable): List<String> {
        return when (exception) {
            is BluetoothException -> listOf(
                "Enable Bluetooth",
                "Check Bluetooth permissions",
                "Restart Bluetooth adapter",
                "Move closer to other devices"
            )
            is NetworkException -> listOf(
                "Check network connection",
                "Retry the operation",
                "Restart the app"
            )
            is PermissionException -> listOf(
                "Grant required permissions in Settings",
                "Restart the app"
            )
            is GameException -> listOf(
                "Check game rules",
                "Verify sufficient balance",
                "Try joining a different game"
            )
            is SecurityException -> listOf(
                "Re-authenticate",
                "Check biometric settings",
                "Clear app data if necessary"
            )
            else -> listOf("Restart the app", "Contact support if problem persists")
        }
    }
}