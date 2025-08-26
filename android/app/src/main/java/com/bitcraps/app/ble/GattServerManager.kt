package com.bitcraps.app.ble

import android.bluetooth.*
import android.bluetooth.le.AdvertiseCallback
import android.bluetooth.le.AdvertiseData
import android.bluetooth.le.AdvertiseSettings
import android.content.Context
import android.os.ParcelUuid
import timber.log.Timber
import java.util.*
import java.util.concurrent.ConcurrentHashMap

/**
 * GATT Server Manager for BitCraps BLE data exchange
 * Provides GATT server functionality with JNI integration
 */
class GattServerManager(private val context: Context) {
    
    companion object {
        // GATT Service and Characteristic UUIDs (must match Rust implementation)
        val BITCRAPS_SERVICE_UUID: UUID = UUID.fromString("12345678-1234-5678-1234-567812345678")
        val BITCRAPS_CHAR_COMMAND_UUID: UUID = UUID.fromString("12345678-1234-5678-1234-567812345679")
        val BITCRAPS_CHAR_RESPONSE_UUID: UUID = UUID.fromString("12345678-1234-5678-1234-56781234567a")
        val BITCRAPS_CHAR_NOTIFY_UUID: UUID = UUID.fromString("12345678-1234-5678-1234-56781234567b")
        
        // GATT characteristics properties
        const val PROPERTY_READ_WRITE = BluetoothGattCharacteristic.PROPERTY_READ or 
                BluetoothGattCharacteristic.PROPERTY_WRITE
        const val PROPERTY_NOTIFY = BluetoothGattCharacteristic.PROPERTY_NOTIFY
        
        const val PERMISSION_READ_WRITE = BluetoothGattCharacteristic.PERMISSION_READ or 
                BluetoothGattCharacteristic.PERMISSION_WRITE
    }
    
    private val bluetoothManager: BluetoothManager by lazy {
        context.getSystemService(Context.BLUETOOTH_SERVICE) as BluetoothManager
    }
    
    private val bluetoothAdapter: BluetoothAdapter? by lazy {
        bluetoothManager.adapter
    }
    
    private var gattServer: BluetoothGattServer? = null
    private val connectedDevices = ConcurrentHashMap<String, BluetoothDevice>()
    private var isRunning = false
    
    /**
     * Start the GATT server
     */
    fun startServer(): Boolean {
        if (isRunning) {
            Timber.d("GATT server already running")
            return true
        }
        
        if (bluetoothAdapter?.isEnabled != true) {
            Timber.w("Cannot start GATT server - Bluetooth is disabled")
            return false
        }
        
        try {
            // Create GATT server
            gattServer = bluetoothManager.openGattServer(context, gattServerCallback)
            
            if (gattServer == null) {
                Timber.e("Failed to create GATT server")
                return false
            }
            
            // Create and add service
            val service = createBitCrapsService()
            val success = gattServer?.addService(service) ?: false
            
            if (success) {
                isRunning = true
                Timber.i("GATT server started successfully")
                return true
            } else {
                Timber.e("Failed to add service to GATT server")
                stopServer()
                return false
            }
        } catch (e: SecurityException) {
            Timber.e(e, "Security exception starting GATT server")
            return false
        } catch (e: Exception) {
            Timber.e(e, "Exception starting GATT server")
            return false
        }
    }
    
    /**
     * Stop the GATT server
     */
    fun stopServer() {
        if (!isRunning) {
            Timber.d("GATT server not running")
            return
        }
        
        try {
            gattServer?.close()
            gattServer = null
            connectedDevices.clear()
            isRunning = false
            
            Timber.i("GATT server stopped")
        } catch (e: Exception) {
            Timber.e(e, "Exception stopping GATT server")
        }
    }
    
    /**
     * Send response to a connected device
     */
    fun sendResponse(deviceAddress: String, data: ByteArray): Boolean {
        val device = connectedDevices[deviceAddress]
        if (device == null) {
            Timber.w("Device not connected: $deviceAddress")
            return false
        }
        
        try {
            val service = gattServer?.getService(BITCRAPS_SERVICE_UUID)
            val characteristic = service?.getCharacteristic(BITCRAPS_CHAR_RESPONSE_UUID)
            
            if (characteristic == null) {
                Timber.e("Response characteristic not found")
                return false
            }
            
            characteristic.value = data
            
            return gattServer?.notifyCharacteristicChanged(
                device,
                characteristic,
                false
            ) ?: false
        } catch (e: SecurityException) {
            Timber.e(e, "Security exception sending response")
            return false
        } catch (e: Exception) {
            Timber.e(e, "Exception sending response")
            return false
        }
    }
    
    /**
     * Send notification to a connected device
     */
    fun sendNotification(deviceAddress: String, data: ByteArray): Boolean {
        val device = connectedDevices[deviceAddress]
        if (device == null) {
            Timber.w("Device not connected: $deviceAddress")
            return false
        }
        
        try {
            val service = gattServer?.getService(BITCRAPS_SERVICE_UUID)
            val characteristic = service?.getCharacteristic(BITCRAPS_CHAR_NOTIFY_UUID)
            
            if (characteristic == null) {
                Timber.e("Notify characteristic not found")
                return false
            }
            
            characteristic.value = data
            
            return gattServer?.notifyCharacteristicChanged(
                device,
                characteristic,
                true
            ) ?: false
        } catch (e: SecurityException) {
            Timber.e(e, "Security exception sending notification")
            return false
        } catch (e: Exception) {
            Timber.e(e, "Exception sending notification")
            return false
        }
    }
    
    /**
     * Create the BitCraps GATT service
     */
    private fun createBitCrapsService(): BluetoothGattService {
        val service = BluetoothGattService(
            BITCRAPS_SERVICE_UUID,
            BluetoothGattService.SERVICE_TYPE_PRIMARY
        )
        
        // Command characteristic (for receiving commands)
        val commandCharacteristic = BluetoothGattCharacteristic(
            BITCRAPS_CHAR_COMMAND_UUID,
            PROPERTY_READ_WRITE,
            PERMISSION_READ_WRITE
        )
        
        // Response characteristic (for sending responses)
        val responseCharacteristic = BluetoothGattCharacteristic(
            BITCRAPS_CHAR_RESPONSE_UUID,
            PROPERTY_READ_WRITE,
            PERMISSION_READ_WRITE
        )
        
        // Notification characteristic (for sending notifications)
        val notifyCharacteristic = BluetoothGattCharacteristic(
            BITCRAPS_CHAR_NOTIFY_UUID,
            PROPERTY_NOTIFY,
            0 // No permissions needed for notify-only characteristic
        )
        
        // Add descriptor for notifications
        val notifyDescriptor = BluetoothGattDescriptor(
            UUID.fromString("00002902-0000-1000-8000-00805f9b34fb"), // Client Characteristic Configuration
            BluetoothGattDescriptor.PERMISSION_READ or BluetoothGattDescriptor.PERMISSION_WRITE
        )
        notifyCharacteristic.addDescriptor(notifyDescriptor)
        
        // Add characteristics to service
        service.addCharacteristic(commandCharacteristic)
        service.addCharacteristic(responseCharacteristic)
        service.addCharacteristic(notifyCharacteristic)
        
        return service
    }
    
    /**
     * GATT server callback implementation
     */
    private val gattServerCallback = object : BluetoothGattServerCallback() {
        
        override fun onConnectionStateChange(device: BluetoothDevice, status: Int, newState: Int) {
            super.onConnectionStateChange(device, status, newState)
            
            when (newState) {
                BluetoothProfile.STATE_CONNECTED -> {
                    connectedDevices[device.address] = device
                    Timber.i("Device connected to GATT server: ${device.address}")
                    
                    // Notify Rust layer via JNI
                    GattJNI.onDeviceConnected(device.address)
                }
                
                BluetoothProfile.STATE_DISCONNECTED -> {
                    connectedDevices.remove(device.address)
                    Timber.i("Device disconnected from GATT server: ${device.address}")
                    
                    // Notify Rust layer via JNI
                    GattJNI.onDeviceDisconnected(device.address)
                }
            }
        }
        
        override fun onServiceAdded(status: Int, service: BluetoothGattService) {
            super.onServiceAdded(status, service)
            
            if (status == BluetoothGatt.GATT_SUCCESS) {
                Timber.d("Service added successfully: ${service.uuid}")
            } else {
                Timber.e("Failed to add service: ${service.uuid}, status: $status")
            }
        }
        
        override fun onCharacteristicReadRequest(
            device: BluetoothDevice,
            requestId: Int,
            offset: Int,
            characteristic: BluetoothGattCharacteristic
        ) {
            super.onCharacteristicReadRequest(device, requestId, offset, characteristic)
            
            try {
                val response = when (characteristic.uuid) {
                    BITCRAPS_CHAR_COMMAND_UUID -> {
                        // Return empty data for command characteristic reads
                        ByteArray(0)
                    }
                    
                    BITCRAPS_CHAR_RESPONSE_UUID -> {
                        // Return any pending response data
                        characteristic.value ?: ByteArray(0)
                    }
                    
                    else -> {
                        Timber.w("Read request for unknown characteristic: ${characteristic.uuid}")
                        ByteArray(0)
                    }
                }
                
                gattServer?.sendResponse(
                    device,
                    requestId,
                    BluetoothGatt.GATT_SUCCESS,
                    offset,
                    response
                )
                
                Timber.d("Read response sent to ${device.address} for ${characteristic.uuid}")
            } catch (e: SecurityException) {
                Timber.e(e, "Security exception handling read request")
                gattServer?.sendResponse(
                    device,
                    requestId,
                    BluetoothGatt.GATT_FAILURE,
                    offset,
                    null
                )
            }
        }
        
        override fun onCharacteristicWriteRequest(
            device: BluetoothDevice,
            requestId: Int,
            characteristic: BluetoothGattCharacteristic,
            preparedWrite: Boolean,
            responseNeeded: Boolean,
            offset: Int,
            value: ByteArray?
        ) {
            super.onCharacteristicWriteRequest(
                device, requestId, characteristic, preparedWrite, 
                responseNeeded, offset, value
            )
            
            try {
                val data = value ?: ByteArray(0)
                
                when (characteristic.uuid) {
                    BITCRAPS_CHAR_COMMAND_UUID -> {
                        // Handle command from peer
                        Timber.d("Command received from ${device.address}: ${data.size} bytes")
                        
                        // Forward to Rust layer via JNI
                        GattJNI.onCommandReceived(device.address, data)
                    }
                    
                    BITCRAPS_CHAR_RESPONSE_UUID -> {
                        // Handle response data
                        Timber.d("Response received from ${device.address}: ${data.size} bytes")
                    }
                    
                    else -> {
                        Timber.w("Write request for unknown characteristic: ${characteristic.uuid}")
                    }
                }
                
                if (responseNeeded) {
                    gattServer?.sendResponse(
                        device,
                        requestId,
                        BluetoothGatt.GATT_SUCCESS,
                        offset,
                        null
                    )
                }
                
            } catch (e: SecurityException) {
                Timber.e(e, "Security exception handling write request")
                if (responseNeeded) {
                    gattServer?.sendResponse(
                        device,
                        requestId,
                        BluetoothGatt.GATT_FAILURE,
                        offset,
                        null
                    )
                }
            }
        }
        
        override fun onDescriptorWriteRequest(
            device: BluetoothDevice,
            requestId: Int,
            descriptor: BluetoothGattDescriptor,
            preparedWrite: Boolean,
            responseNeeded: Boolean,
            offset: Int,
            value: ByteArray?
        ) {
            super.onDescriptorWriteRequest(
                device, requestId, descriptor, preparedWrite, 
                responseNeeded, offset, value
            )
            
            try {
                Timber.d("Descriptor write request from ${device.address}: ${descriptor.uuid}")
                
                if (responseNeeded) {
                    gattServer?.sendResponse(
                        device,
                        requestId,
                        BluetoothGatt.GATT_SUCCESS,
                        offset,
                        null
                    )
                }
            } catch (e: SecurityException) {
                Timber.e(e, "Security exception handling descriptor write")
                if (responseNeeded) {
                    gattServer?.sendResponse(
                        device,
                        requestId,
                        BluetoothGatt.GATT_FAILURE,
                        offset,
                        null
                    )
                }
            }
        }
        
        override fun onExecuteWrite(device: BluetoothDevice, requestId: Int, execute: Boolean) {
            super.onExecuteWrite(device, requestId, execute)
            
            try {
                gattServer?.sendResponse(
                    device,
                    requestId,
                    BluetoothGatt.GATT_SUCCESS,
                    0,
                    null
                )
            } catch (e: SecurityException) {
                Timber.e(e, "Security exception handling execute write")
            }
        }
    }
    
    /**
     * Check if server is running
     */
    fun isRunning(): Boolean = isRunning
    
    /**
     * Get connected devices count
     */
    fun getConnectedDevicesCount(): Int = connectedDevices.size
    
    /**
     * Get connected device addresses
     */
    fun getConnectedDeviceAddresses(): List<String> = connectedDevices.keys.toList()
}

/**
 * GATT JNI bridge for communication with Rust layer
 */
object GattJNI {
    
    /**
     * Notify Rust layer of device connection
     */
    fun onDeviceConnected(deviceAddress: String) {
        try {
            // TODO: Call native method when implemented
            Timber.d("Device connected (JNI): $deviceAddress")
        } catch (e: Exception) {
            Timber.e(e, "Failed to notify device connection via JNI")
        }
    }
    
    /**
     * Notify Rust layer of device disconnection
     */
    fun onDeviceDisconnected(deviceAddress: String) {
        try {
            // TODO: Call native method when implemented
            Timber.d("Device disconnected (JNI): $deviceAddress")
        } catch (e: Exception) {
            Timber.e(e, "Failed to notify device disconnection via JNI")
        }
    }
    
    /**
     * Forward command to Rust layer
     */
    fun onCommandReceived(deviceAddress: String, data: ByteArray) {
        try {
            // TODO: Call native method when implemented
            Timber.d("Command received (JNI): $deviceAddress, ${data.size} bytes")
        } catch (e: Exception) {
            Timber.e(e, "Failed to forward command via JNI")
        }
    }
}