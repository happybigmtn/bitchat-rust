//! Thread-safe callback handling for Android JNI bridge
//!
//! This module provides thread-safe callback mechanisms between the Rust
//! BLE implementation and Android Java/Kotlin code, ensuring proper
//! synchronization and memory management.

use crate::error::BitCrapsError;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::Duration;
use tokio::sync::mpsc;

#[cfg(target_os = "android")]
use jni::objects::{GlobalRef, JByteArray, JClass, JObject, JString, JValue};
#[cfg(target_os = "android")]
use jni::sys::{jboolean, jbyteArray, jint, jlong, jstring};
#[cfg(target_os = "android")]
use jni::{JNIEnv, JavaVM};

/// Callback event types
#[derive(Debug, Clone)]
pub enum CallbackEvent {
    PeerDiscovered {
        address: String,
        name: Option<String>,
        rssi: i32,
        manufacturer_data: Option<Vec<u8>>,
        service_uuids: Vec<String>,
    },
    DeviceConnected {
        address: String,
    },
    DeviceDisconnected {
        address: String,
    },
    CommandReceived {
        device_address: String,
        data: Vec<u8>,
    },
    AdvertisingStateChanged {
        is_advertising: bool,
    },
    ScanningStateChanged {
        is_scanning: bool,
    },
    GattServerStateChanged {
        is_running: bool,
    },
}

/// Callback handler trait
pub trait CallbackHandler: Send + Sync {
    fn handle_event(&self, event: CallbackEvent) -> Result<(), BitCrapsError>;
}

/// Thread-safe callback manager
pub struct CallbackManager {
    #[cfg(target_os = "android")]
    java_vm: Option<JavaVM>,
    #[cfg(target_os = "android")]
    callback_object: Option<GlobalRef>,
    handlers: RwLock<Vec<Arc<dyn CallbackHandler>>>,
    event_sender: Option<mpsc::Sender<CallbackEvent>>,
    event_receiver: Arc<Mutex<Option<mpsc::Receiver<CallbackEvent>>>>,
    is_running: Arc<Mutex<bool>>,
}

impl CallbackManager {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel(1000); // Moderate traffic for Android callbacks

        Self {
            #[cfg(target_os = "android")]
            java_vm: None,
            #[cfg(target_os = "android")]
            callback_object: None,
            handlers: RwLock::new(Vec::new()),
            event_sender: Some(sender),
            event_receiver: Arc::new(Mutex::new(Some(receiver))),
            is_running: Arc::new(Mutex::new(false)),
        }
    }

    #[cfg(target_os = "android")]
    pub fn set_java_vm(&mut self, vm: JavaVM) {
        self.java_vm = Some(vm);
    }

    #[cfg(target_os = "android")]
    pub fn set_callback_object(&mut self, callback: GlobalRef) {
        self.callback_object = Some(callback);
    }

    /// Register a callback handler
    pub fn register_handler(&self, handler: Arc<dyn CallbackHandler>) -> Result<(), BitCrapsError> {
        let mut handlers = self
            .handlers
            .write()
            .map_err(|_| BitCrapsError::BluetoothError {
                message: "Failed to lock handlers for registration".to_string(),
            })?;

        handlers.push(handler);
        Ok(())
    }

    /// Unregister all handlers
    pub fn clear_handlers(&self) -> Result<(), BitCrapsError> {
        let mut handlers = self
            .handlers
            .write()
            .map_err(|_| BitCrapsError::BluetoothError {
                message: "Failed to lock handlers for clearing".to_string(),
            })?;

        handlers.clear();
        Ok(())
    }

    /// Send an event to be processed
    pub fn send_event(&self, event: CallbackEvent) -> Result<(), BitCrapsError> {
        if let Some(sender) = &self.event_sender {
            // Use try_send for bounded channels to handle backpressure
            match sender.try_send(event) {
                Ok(_) => {},
                Err(mpsc::error::TrySendError::Full(_)) => {
                    log::warn!("Android callback channel full, dropping event (backpressure)");
                    // Could add metrics here: CALLBACK_DROPS.inc();
                    return Ok(()); // Drop the event instead of blocking
                },
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    return Err(BitCrapsError::BluetoothError {
                        message: "Callback channel closed".to_string(),
                    });
                }
            }
        }
        Ok(())
    }

    /// Start the callback processing loop
    pub fn start(&self) -> Result<(), BitCrapsError> {
        let mut running = self
            .is_running
            .lock()
            .map_err(|_| BitCrapsError::BluetoothError {
                message: "Failed to lock running state".to_string(),
            })?;

        if *running {
            return Ok(()); // Already running
        }

        *running = true;

        // Take the receiver
        let receiver = {
            let mut recv_lock =
                self.event_receiver
                    .lock()
                    .map_err(|_| BitCrapsError::BluetoothError {
                        message: "Failed to lock event receiver".to_string(),
                    })?;
            recv_lock
                .take()
                .ok_or_else(|| BitCrapsError::BluetoothError {
                    message: "Event receiver already taken".to_string(),
                })?
        };

        // Clone necessary data for the processing thread
        let handlers = Arc::clone(&self.handlers);
        let is_running = Arc::clone(&self.is_running);

        #[cfg(target_os = "android")]
        let java_vm = self.java_vm.clone();
        #[cfg(target_os = "android")]
        let callback_object = self.callback_object.clone();

        // Spawn the processing thread
        thread::spawn(move || {
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    log::error!("Failed to create callback runtime: {}", e);
                    return;
                }
            };

            rt.block_on(async move {
                let mut receiver = receiver;

                while let Some(event) = receiver.recv().await {
                    // Check if we should continue running
                    {
                        let running = is_running.lock().unwrap_or_default();
                        if !*running {
                            break;
                        }
                    }

                    // Process event with registered handlers
                    if let Ok(handlers_guard) = handlers.read() {
                        for handler in handlers_guard.iter() {
                            if let Err(e) = handler.handle_event(event.clone()) {
                                log::error!("Callback handler error: {}", e);
                            }
                        }
                    }

                    // Forward to Android if available
                    #[cfg(target_os = "android")]
                    if let (Some(vm), Some(callback)) = (&java_vm, &callback_object) {
                        if let Err(e) = Self::forward_to_android(vm, callback, &event) {
                            log::error!("Failed to forward event to Android: {}", e);
                        }
                    }
                }

                log::info!("Callback processing loop ended");
            });
        });

        log::info!("Callback manager started");
        Ok(())
    }

    /// Stop the callback processing
    pub fn stop(&self) -> Result<(), BitCrapsError> {
        let mut running = self
            .is_running
            .lock()
            .map_err(|_| BitCrapsError::BluetoothError {
                message: "Failed to lock running state".to_string(),
            })?;

        *running = false;
        log::info!("Callback manager stopped");
        Ok(())
    }

    /// Forward event to Android callback object
    #[cfg(target_os = "android")]
    fn forward_to_android(
        vm: &JavaVM,
        callback_object: &GlobalRef,
        event: &CallbackEvent,
    ) -> Result<(), BitCrapsError> {
        let env = vm
            .attach_current_thread()
            .map_err(|e| BitCrapsError::BluetoothError {
                message: format!("Failed to attach to JVM: {}", e),
            })?;

        match event {
            CallbackEvent::PeerDiscovered {
                address,
                name,
                rssi,
                manufacturer_data,
                service_uuids,
            } => {
                let address_jstring =
                    env.new_string(address)
                        .map_err(|e| BitCrapsError::BluetoothError {
                            message: format!("Failed to create address JString: {}", e),
                        })?;

                let name_jstring = if let Some(name) = name {
                    env.new_string(name).ok()
                } else {
                    None
                };

                let manufacturer_data_array = if let Some(data) = manufacturer_data {
                    env.byte_array_from_slice(data).ok()
                } else {
                    None
                };

                // Convert service UUIDs to string array
                let string_class = env.find_class("java/lang/String").map_err(|e| {
                    BitCrapsError::BluetoothError {
                        message: format!("Failed to find String class: {}", e),
                    }
                })?;

                let uuids_array = env
                    .new_object_array(service_uuids.len() as i32, string_class, JObject::null())
                    .map_err(|e| BitCrapsError::BluetoothError {
                        message: format!("Failed to create UUID array: {}", e),
                    })?;

                for (i, uuid) in service_uuids.iter().enumerate() {
                    let uuid_jstring =
                        env.new_string(uuid)
                            .map_err(|e| BitCrapsError::BluetoothError {
                                message: format!("Failed to create UUID JString: {}", e),
                            })?;
                    env.set_object_array_element(
                        uuids_array,
                        i as i32,
                        JObject::from(uuid_jstring),
                    )
                    .map_err(|e| BitCrapsError::BluetoothError {
                        message: format!("Failed to set UUID array element: {}", e),
                    })?;
                }

                // Call Android callback method
                let args = [
                    JValue::Object(&address_jstring),
                    JValue::Object(&name_jstring.unwrap_or_else(|| JObject::null().into())),
                    JValue::Int(*rssi),
                    JValue::Object(
                        &manufacturer_data_array.unwrap_or_else(|| JObject::null().into()),
                    ),
                    JValue::Object(&uuids_array),
                ];

                env.call_method(
                    callback_object,
                    "onPeerDiscovered",
                    "(Ljava/lang/String;Ljava/lang/String;I[B[Ljava/lang/String;)V",
                    &args,
                )
                .map_err(|e| BitCrapsError::BluetoothError {
                    message: format!("Failed to call onPeerDiscovered: {}", e),
                })?;
            }

            CallbackEvent::DeviceConnected { address } => {
                let address_jstring =
                    env.new_string(address)
                        .map_err(|e| BitCrapsError::BluetoothError {
                            message: format!("Failed to create address JString: {}", e),
                        })?;

                env.call_method(
                    callback_object,
                    "onDeviceConnected",
                    "(Ljava/lang/String;)V",
                    &[JValue::Object(&address_jstring)],
                )
                .map_err(|e| BitCrapsError::BluetoothError {
                    message: format!("Failed to call onDeviceConnected: {}", e),
                })?;
            }

            CallbackEvent::DeviceDisconnected { address } => {
                let address_jstring =
                    env.new_string(address)
                        .map_err(|e| BitCrapsError::BluetoothError {
                            message: format!("Failed to create address JString: {}", e),
                        })?;

                env.call_method(
                    callback_object,
                    "onDeviceDisconnected",
                    "(Ljava/lang/String;)V",
                    &[JValue::Object(&address_jstring)],
                )
                .map_err(|e| BitCrapsError::BluetoothError {
                    message: format!("Failed to call onDeviceDisconnected: {}", e),
                })?;
            }

            CallbackEvent::CommandReceived {
                device_address,
                data,
            } => {
                let address_jstring =
                    env.new_string(device_address)
                        .map_err(|e| BitCrapsError::BluetoothError {
                            message: format!("Failed to create address JString: {}", e),
                        })?;

                let data_array =
                    env.byte_array_from_slice(data)
                        .map_err(|e| BitCrapsError::BluetoothError {
                            message: format!("Failed to create data array: {}", e),
                        })?;

                env.call_method(
                    callback_object,
                    "onCommandReceived",
                    "(Ljava/lang/String;[B)V",
                    &[
                        JValue::Object(&address_jstring),
                        JValue::Object(&data_array),
                    ],
                )
                .map_err(|e| BitCrapsError::BluetoothError {
                    message: format!("Failed to call onCommandReceived: {}", e),
                })?;
            }

            CallbackEvent::AdvertisingStateChanged { is_advertising } => {
                env.call_method(
                    callback_object,
                    "onAdvertisingStateChanged",
                    "(Z)V",
                    &[JValue::Bool(*is_advertising as u8)],
                )
                .map_err(|e| BitCrapsError::BluetoothError {
                    message: format!("Failed to call onAdvertisingStateChanged: {}", e),
                })?;
            }

            CallbackEvent::ScanningStateChanged { is_scanning } => {
                env.call_method(
                    callback_object,
                    "onScanningStateChanged",
                    "(Z)V",
                    &[JValue::Bool(*is_scanning as u8)],
                )
                .map_err(|e| BitCrapsError::BluetoothError {
                    message: format!("Failed to call onScanningStateChanged: {}", e),
                })?;
            }

            CallbackEvent::GattServerStateChanged { is_running } => {
                env.call_method(
                    callback_object,
                    "onGattServerStateChanged",
                    "(Z)V",
                    &[JValue::Bool(*is_running as u8)],
                )
                .map_err(|e| BitCrapsError::BluetoothError {
                    message: format!("Failed to call onGattServerStateChanged: {}", e),
                })?;
            }
        }

        Ok(())
    }

    /// Check if callback manager is running
    pub fn is_running(&self) -> bool {
        self.is_running
            .lock()
            .map(|running| *running)
            .unwrap_or(false)
    }
}

/// Default callback handler that logs events
pub struct LoggingCallbackHandler;

impl CallbackHandler for LoggingCallbackHandler {
    fn handle_event(&self, event: CallbackEvent) -> Result<(), BitCrapsError> {
        match event {
            CallbackEvent::PeerDiscovered {
                address,
                name,
                rssi,
                ..
            } => {
                log::info!("Peer discovered: {} ({:?}) RSSI: {}", address, name, rssi);
            }
            CallbackEvent::DeviceConnected { address } => {
                log::info!("Device connected: {}", address);
            }
            CallbackEvent::DeviceDisconnected { address } => {
                log::info!("Device disconnected: {}", address);
            }
            CallbackEvent::CommandReceived {
                device_address,
                data,
            } => {
                log::debug!(
                    "Command received from {}: {} bytes",
                    device_address,
                    data.len()
                );
            }
            CallbackEvent::AdvertisingStateChanged { is_advertising } => {
                log::info!("Advertising state changed: {}", is_advertising);
            }
            CallbackEvent::ScanningStateChanged { is_scanning } => {
                log::info!("Scanning state changed: {}", is_scanning);
            }
            CallbackEvent::GattServerStateChanged { is_running } => {
                log::info!("GATT server state changed: {}", is_running);
            }
        }
        Ok(())
    }
}

/// Global callback manager instance
static mut CALLBACK_MANAGER: Option<Arc<CallbackManager>> = None;
static CALLBACK_MANAGER_INIT: std::sync::Once = std::sync::Once::new();

/// Get the global callback manager
pub fn get_callback_manager() -> Arc<CallbackManager> {
    unsafe {
        CALLBACK_MANAGER_INIT.call_once(|| {
            let manager = Arc::new(CallbackManager::new());

            // Register default logging handler
            let logging_handler = Arc::new(LoggingCallbackHandler);
            if let Err(e) = manager.register_handler(logging_handler) {
                log::error!("Failed to register logging handler: {}", e);
            }

            CALLBACK_MANAGER = Some(manager);
        });

        CALLBACK_MANAGER.as_ref().unwrap().clone()
    }
}

/// Initialize the callback manager
pub fn initialize_callback_manager() -> Result<(), BitCrapsError> {
    let manager = get_callback_manager();
    manager.start()?;
    log::info!("Callback manager initialized");
    Ok(())
}

/// Shutdown the callback manager
pub fn shutdown_callback_manager() -> Result<(), BitCrapsError> {
    let manager = get_callback_manager();
    manager.stop()?;
    manager.clear_handlers()?;
    log::info!("Callback manager shutdown");
    Ok(())
}
