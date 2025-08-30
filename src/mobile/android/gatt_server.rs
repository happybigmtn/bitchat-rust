//! GATT Server Implementation for Android BLE
//!
//! This module provides a GATT server implementation that works with Android's
//! BluetoothGattServer API through JNI for data exchange between BitCraps peers.

use crate::error::BitCrapsError;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[cfg(target_os = "android")]
use jni::objects::{GlobalRef, JByteArray, JClass, JObject, JString};
#[cfg(target_os = "android")]
use jni::sys::{jboolean, jbyteArray, jint, jlong, jobject, jstring};
#[cfg(target_os = "android")]
use jni::JNIEnv;

/// BitCraps GATT Service and Characteristic UUIDs
pub const BITCRAPS_SERVICE_UUID: &str = "12345678-1234-5678-1234-567812345678";
pub const BITCRAPS_CHAR_COMMAND_UUID: &str = "12345678-1234-5678-1234-567812345679";
pub const BITCRAPS_CHAR_RESPONSE_UUID: &str = "12345678-1234-5678-1234-56781234567a";
pub const BITCRAPS_CHAR_NOTIFY_UUID: &str = "12345678-1234-5678-1234-56781234567b";

/// GATT Server state
#[derive(Debug, Clone)]
pub struct GattServerState {
    pub is_running: bool,
    pub connected_devices: Vec<String>,
    pub pending_responses: HashMap<String, Vec<u8>>,
}

/// Android GATT Server manager
pub struct AndroidGattServer {
    #[cfg(target_os = "android")]
    pub(crate) java_vm: Option<jni::JavaVM>,
    #[cfg(target_os = "android")]
    pub(crate) gatt_server: Option<GlobalRef>,
    pub(crate) state: Arc<Mutex<GattServerState>>,
    pub(crate) message_handler: Option<Arc<dyn MessageHandler + Send + Sync>>,
}

/// Trait for handling GATT messages
pub trait MessageHandler {
    fn handle_command(&self, device: &str, data: &[u8]) -> Result<Vec<u8>, BitCrapsError>;
    fn handle_device_connected(&self, device: &str) -> Result<(), BitCrapsError>;
    fn handle_device_disconnected(&self, device: &str) -> Result<(), BitCrapsError>;
}

impl AndroidGattServer {
    pub fn new() -> Self {
        Self {
            #[cfg(target_os = "android")]
            java_vm: None,
            #[cfg(target_os = "android")]
            gatt_server: None,
            state: Arc::new(Mutex::new(GattServerState {
                is_running: false,
                connected_devices: Vec::new(),
                pending_responses: HashMap::new(),
            })),
            message_handler: None,
        }
    }

    #[cfg(target_os = "android")]
    pub fn set_java_vm(&mut self, vm: jni::JavaVM) {
        self.java_vm = Some(vm);
    }

    #[cfg(target_os = "android")]
    pub fn set_gatt_server(&mut self, server: GlobalRef) {
        self.gatt_server = Some(server);
    }

    pub fn set_message_handler(&mut self, handler: Arc<dyn MessageHandler + Send + Sync>) {
        self.message_handler = Some(handler);
    }

    /// Start the GATT server
    pub async fn start(&self) -> Result<(), BitCrapsError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| BitCrapsError::BluetoothError {
                message: "Failed to lock GATT server state".to_string(),
            })?;

        if state.is_running {
            return Ok(()); // Already running
        }

        #[cfg(target_os = "android")]
        if let (Some(vm), Some(server)) = (&self.java_vm, &self.gatt_server) {
            let env = vm
                .attach_current_thread()
                .map_err(|e| BitCrapsError::BluetoothError {
                    message: format!("Failed to attach to JVM: {}", e),
                })?;

            // Call Java method to start GATT server
            let result = env
                .call_method(server, "startServer", "()Z", &[])
                .map_err(|e| BitCrapsError::BluetoothError {
                    message: format!("Failed to call startServer: {}", e),
                })?;

            let success = result.z().map_err(|e| BitCrapsError::BluetoothError {
                message: format!("Failed to get boolean result: {}", e),
            })?;

            if success {
                state.is_running = true;
                log::info!("GATT server started successfully");
                Ok(())
            } else {
                Err(BitCrapsError::BluetoothError {
                    message: "Failed to start GATT server".to_string(),
                })
            }
        } else {
            Err(BitCrapsError::BluetoothError {
                message: "JVM or GATT server not initialized".to_string(),
            })
        }

        #[cfg(not(target_os = "android"))]
        {
            state.is_running = true;
            log::info!("Mock GATT server started (non-Android)");
            Ok(())
        }
    }

    /// Stop the GATT server
    pub async fn stop(&self) -> Result<(), BitCrapsError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| BitCrapsError::BluetoothError {
                message: "Failed to lock GATT server state".to_string(),
            })?;

        if !state.is_running {
            return Ok(()); // Not running
        }

        #[cfg(target_os = "android")]
        if let (Some(vm), Some(server)) = (&self.java_vm, &self.gatt_server) {
            let env = vm
                .attach_current_thread()
                .map_err(|e| BitCrapsError::BluetoothError {
                    message: format!("Failed to attach to JVM: {}", e),
                })?;

            // Call Java method to stop GATT server
            let result = env
                .call_method(server, "stopServer", "()V", &[])
                .map_err(|e| BitCrapsError::BluetoothError {
                    message: format!("Failed to call stopServer: {}", e),
                })?;

            state.is_running = false;
            state.connected_devices.clear();
            state.pending_responses.clear();
            log::info!("GATT server stopped successfully");
        }

        #[cfg(not(target_os = "android"))]
        {
            state.is_running = false;
            state.connected_devices.clear();
            state.pending_responses.clear();
            log::info!("Mock GATT server stopped (non-Android)");
        }

        Ok(())
    }

    /// Handle incoming command from a connected device
    pub fn handle_command(&self, device: &str, data: &[u8]) -> Result<(), BitCrapsError> {
        if let Some(handler) = &self.message_handler {
            match handler.handle_command(device, data) {
                Ok(response) => {
                    // Store response for sending back
                    let mut state =
                        self.state
                            .lock()
                            .map_err(|_| BitCrapsError::BluetoothError {
                                message: "Failed to lock GATT server state".to_string(),
                            })?;

                    state
                        .pending_responses
                        .insert(device.to_string(), response.clone());

                    // Send response immediately if possible
                    self.send_response(device, &response)
                }
                Err(e) => {
                    log::error!("Message handler error for device {}: {}", device, e);
                    Err(e)
                }
            }
        } else {
            log::warn!("No message handler set for GATT server");
            Ok(())
        }
    }

    /// Send response to a connected device
    pub fn send_response(&self, device: &str, data: &[u8]) -> Result<(), BitCrapsError> {
        #[cfg(target_os = "android")]
        if let (Some(vm), Some(server)) = (&self.java_vm, &self.gatt_server) {
            let env = vm
                .attach_current_thread()
                .map_err(|e| BitCrapsError::BluetoothError {
                    message: format!("Failed to attach to JVM: {}", e),
                })?;

            // Convert device address to JString
            let device_jstring =
                env.new_string(device)
                    .map_err(|e| BitCrapsError::BluetoothError {
                        message: format!("Failed to create device JString: {}", e),
                    })?;

            // Convert data to byte array
            let data_array =
                env.byte_array_from_slice(data)
                    .map_err(|e| BitCrapsError::BluetoothError {
                        message: format!("Failed to create byte array: {}", e),
                    })?;

            // Call Java method to send response
            let result = env
                .call_method(
                    server,
                    "sendResponse",
                    "(Ljava/lang/String;[B)Z",
                    &[
                        jni::objects::JValue::Object(&device_jstring),
                        jni::objects::JValue::Object(&data_array),
                    ],
                )
                .map_err(|e| BitCrapsError::BluetoothError {
                    message: format!("Failed to call sendResponse: {}", e),
                })?;

            let success = result.z().map_err(|e| BitCrapsError::BluetoothError {
                message: format!("Failed to get boolean result: {}", e),
            })?;

            if success {
                log::debug!("Response sent to device: {}", device);
                Ok(())
            } else {
                Err(BitCrapsError::BluetoothError {
                    message: format!("Failed to send response to device: {}", device),
                })
            }
        } else {
            Err(BitCrapsError::BluetoothError {
                message: "JVM or GATT server not initialized".to_string(),
            })
        }

        #[cfg(not(target_os = "android"))]
        {
            log::debug!(
                "Mock response sent to device: {} ({} bytes)",
                device,
                data.len()
            );
            Ok(())
        }
    }

    /// Send notification to a connected device
    pub fn send_notification(&self, device: &str, data: &[u8]) -> Result<(), BitCrapsError> {
        #[cfg(target_os = "android")]
        if let (Some(vm), Some(server)) = (&self.java_vm, &self.gatt_server) {
            let env = vm
                .attach_current_thread()
                .map_err(|e| BitCrapsError::BluetoothError {
                    message: format!("Failed to attach to JVM: {}", e),
                })?;

            // Convert device address to JString
            let device_jstring =
                env.new_string(device)
                    .map_err(|e| BitCrapsError::BluetoothError {
                        message: format!("Failed to create device JString: {}", e),
                    })?;

            // Convert data to byte array
            let data_array =
                env.byte_array_from_slice(data)
                    .map_err(|e| BitCrapsError::BluetoothError {
                        message: format!("Failed to create byte array: {}", e),
                    })?;

            // Call Java method to send notification
            let result = env
                .call_method(
                    server,
                    "sendNotification",
                    "(Ljava/lang/String;[B)Z",
                    &[
                        jni::objects::JValue::Object(&device_jstring),
                        jni::objects::JValue::Object(&data_array),
                    ],
                )
                .map_err(|e| BitCrapsError::BluetoothError {
                    message: format!("Failed to call sendNotification: {}", e),
                })?;

            let success = result.z().map_err(|e| BitCrapsError::BluetoothError {
                message: format!("Failed to get boolean result: {}", e),
            })?;

            if success {
                log::debug!("Notification sent to device: {}", device);
                Ok(())
            } else {
                Err(BitCrapsError::BluetoothError {
                    message: format!("Failed to send notification to device: {}", device),
                })
            }
        } else {
            Err(BitCrapsError::BluetoothError {
                message: "JVM or GATT server not initialized".to_string(),
            })
        }

        #[cfg(not(target_os = "android"))]
        {
            log::debug!(
                "Mock notification sent to device: {} ({} bytes)",
                device,
                data.len()
            );
            Ok(())
        }
    }

    /// Handle device connection
    pub fn handle_device_connected(&self, device: &str) -> Result<(), BitCrapsError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| BitCrapsError::BluetoothError {
                message: "Failed to lock GATT server state".to_string(),
            })?;

        if !state.connected_devices.contains(&device.to_string()) {
            state.connected_devices.push(device.to_string());
            log::info!("Device connected to GATT server: {}", device);
        }

        if let Some(handler) = &self.message_handler {
            handler.handle_device_connected(device)?;
        }

        Ok(())
    }

    /// Handle device disconnection
    pub fn handle_device_disconnected(&self, device: &str) -> Result<(), BitCrapsError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| BitCrapsError::BluetoothError {
                message: "Failed to lock GATT server state".to_string(),
            })?;

        state.connected_devices.retain(|d| d != device);
        state.pending_responses.remove(device);
        log::info!("Device disconnected from GATT server: {}", device);

        if let Some(handler) = &self.message_handler {
            handler.handle_device_disconnected(device)?;
        }

        Ok(())
    }

    /// Get server state
    pub fn get_state(&self) -> Result<GattServerState, BitCrapsError> {
        let state = self
            .state
            .lock()
            .map_err(|_| BitCrapsError::BluetoothError {
                message: "Failed to lock GATT server state".to_string(),
            })?;

        Ok(state.clone())
    }

    /// Check if server is running
    pub fn is_running(&self) -> bool {
        self.state
            .lock()
            .map(|state| state.is_running)
            .unwrap_or(false)
    }

    /// Get connected devices count
    pub fn get_connected_devices_count(&self) -> usize {
        self.state
            .lock()
            .map(|state| state.connected_devices.len())
            .unwrap_or(0)
    }
}

/// Default message handler implementation
pub struct DefaultMessageHandler;

impl MessageHandler for DefaultMessageHandler {
    fn handle_command(&self, device: &str, data: &[u8]) -> Result<Vec<u8>, BitCrapsError> {
        log::debug!("Received command from {}: {} bytes", device, data.len());

        // Echo back the same data for testing
        Ok(data.to_vec())
    }

    fn handle_device_connected(&self, device: &str) -> Result<(), BitCrapsError> {
        log::info!("Device connected: {}", device);
        Ok(())
    }

    fn handle_device_disconnected(&self, device: &str) -> Result<(), BitCrapsError> {
        log::info!("Device disconnected: {}", device);
        Ok(())
    }
}
