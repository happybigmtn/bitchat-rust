//! Android-specific mobile implementations
//!
//! This module provides Android-specific functionality including:
//! - JNI bridge for BLE advertising and discovery
//! - GATT server implementation
//! - Thread-safe callback handling
//! - Android lifecycle integration

pub mod ble_jni;
pub mod callbacks;
pub mod gatt_server;
pub mod lifecycle;

use crate::error::BitCrapsError;
use crate::transport::bluetooth::BluetoothTransport;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[cfg(target_os = "android")]
use jni::objects::{GlobalRef, JClass, JObject};
#[cfg(target_os = "android")]
use jni::JNIEnv;
#[cfg(target_os = "android")]
use jni::JavaVM;

/// Android BLE service manager
pub struct AndroidBleManager {
    pub(crate) transport: Option<Arc<BluetoothTransport>>,
    #[cfg(target_os = "android")]
    pub(crate) java_vm: Option<JavaVM>,
    #[cfg(target_os = "android")]
    pub(crate) ble_service: Option<GlobalRef>,
    pub(crate) is_advertising: Arc<Mutex<bool>>,
    pub(crate) is_scanning: Arc<Mutex<bool>>,
    pub(crate) discovered_peers: Arc<Mutex<HashMap<String, AndroidPeerInfo>>>,
}

/// Android-specific peer information
#[derive(Debug, Clone)]
pub struct AndroidPeerInfo {
    pub address: String,
    pub name: Option<String>,
    pub rssi: i32,
    pub last_seen: u64,
    pub manufacturer_data: Option<Vec<u8>>,
    pub service_uuids: Vec<String>,
}

impl AndroidBleManager {
    pub fn new() -> Self {
        Self {
            transport: None,
            #[cfg(target_os = "android")]
            java_vm: None,
            #[cfg(target_os = "android")]
            ble_service: None,
            is_advertising: Arc::new(Mutex::new(false)),
            is_scanning: Arc::new(Mutex::new(false)),
            discovered_peers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Initialize with Bluetooth transport
    pub fn with_transport(mut self, transport: Arc<BluetoothTransport>) -> Self {
        self.transport = Some(transport);
        self
    }

    #[cfg(target_os = "android")]
    pub fn set_java_vm(&mut self, vm: JavaVM) {
        self.java_vm = Some(vm);
    }

    #[cfg(target_os = "android")]
    pub fn set_ble_service(&mut self, service: GlobalRef) {
        self.ble_service = Some(service);
    }

    /// Start BLE advertising
    pub async fn start_advertising(&self) -> Result<(), BitCrapsError> {
        let mut advertising =
            self.is_advertising
                .lock()
                .map_err(|_| BitCrapsError::BluetoothError {
                    message: "Failed to lock advertising state".to_string(),
                })?;

        if *advertising {
            return Ok(()); // Already advertising
        }

        #[cfg(target_os = "android")]
        if let (Some(vm), Some(service)) = (&self.java_vm, &self.ble_service) {
            let env = vm
                .attach_current_thread()
                .map_err(|e| BitCrapsError::BluetoothError {
                    message: format!("Failed to attach to JVM: {}", e),
                })?;

            // Call Java method to start advertising
            let result = env
                .call_method(service, "startAdvertising", "()Z", &[])
                .map_err(|e| BitCrapsError::BluetoothError {
                    message: format!("Failed to call startAdvertising: {}", e),
                })?;

            let success = result.z().map_err(|e| BitCrapsError::BluetoothError {
                message: format!("Failed to get boolean result: {}", e),
            })?;

            if success {
                *advertising = true;
                log::info!("BLE advertising started successfully");
                Ok(())
            } else {
                Err(BitCrapsError::BluetoothError {
                    message: "Failed to start BLE advertising".to_string(),
                })
            }
        } else {
            Err(BitCrapsError::BluetoothError {
                message: "JVM or BLE service not initialized".to_string(),
            })
        }

        #[cfg(not(target_os = "android"))]
        {
            *advertising = true;
            log::info!("Mock BLE advertising started (non-Android)");
            Ok(())
        }
    }

    /// Stop BLE advertising
    pub async fn stop_advertising(&self) -> Result<(), BitCrapsError> {
        let mut advertising =
            self.is_advertising
                .lock()
                .map_err(|_| BitCrapsError::BluetoothError {
                    message: "Failed to lock advertising state".to_string(),
                })?;

        if !*advertising {
            return Ok(()); // Not advertising
        }

        #[cfg(target_os = "android")]
        if let (Some(vm), Some(service)) = (&self.java_vm, &self.ble_service) {
            let env = vm
                .attach_current_thread()
                .map_err(|e| BitCrapsError::BluetoothError {
                    message: format!("Failed to attach to JVM: {}", e),
                })?;

            // Call Java method to stop advertising
            let result = env
                .call_method(service, "stopAdvertising", "()Z", &[])
                .map_err(|e| BitCrapsError::BluetoothError {
                    message: format!("Failed to call stopAdvertising: {}", e),
                })?;

            let success = result.z().map_err(|e| BitCrapsError::BluetoothError {
                message: format!("Failed to get boolean result: {}", e),
            })?;

            if success {
                *advertising = false;
                log::info!("BLE advertising stopped successfully");
                Ok(())
            } else {
                Err(BitCrapsError::BluetoothError {
                    message: "Failed to stop BLE advertising".to_string(),
                })
            }
        } else {
            Err(BitCrapsError::BluetoothError {
                message: "JVM or BLE service not initialized".to_string(),
            })
        }

        #[cfg(not(target_os = "android"))]
        {
            *advertising = false;
            log::info!("Mock BLE advertising stopped (non-Android)");
            Ok(())
        }
    }

    /// Start BLE scanning
    pub async fn start_scanning(&self) -> Result<(), BitCrapsError> {
        let mut scanning = self
            .is_scanning
            .lock()
            .map_err(|_| BitCrapsError::BluetoothError {
                message: "Failed to lock scanning state".to_string(),
            })?;

        if *scanning {
            return Ok(()); // Already scanning
        }

        #[cfg(target_os = "android")]
        if let (Some(vm), Some(service)) = (&self.java_vm, &self.ble_service) {
            let env = vm
                .attach_current_thread()
                .map_err(|e| BitCrapsError::BluetoothError {
                    message: format!("Failed to attach to JVM: {}", e),
                })?;

            // Call Java method to start scanning
            let result = env
                .call_method(service, "startScanning", "()Z", &[])
                .map_err(|e| BitCrapsError::BluetoothError {
                    message: format!("Failed to call startScanning: {}", e),
                })?;

            let success = result.z().map_err(|e| BitCrapsError::BluetoothError {
                message: format!("Failed to get boolean result: {}", e),
            })?;

            if success {
                *scanning = true;
                log::info!("BLE scanning started successfully");
                Ok(())
            } else {
                Err(BitCrapsError::BluetoothError {
                    message: "Failed to start BLE scanning".to_string(),
                })
            }
        } else {
            Err(BitCrapsError::BluetoothError {
                message: "JVM or BLE service not initialized".to_string(),
            })
        }

        #[cfg(not(target_os = "android"))]
        {
            *scanning = true;
            log::info!("Mock BLE scanning started (non-Android)");
            Ok(())
        }
    }

    /// Stop BLE scanning
    pub async fn stop_scanning(&self) -> Result<(), BitCrapsError> {
        let mut scanning = self
            .is_scanning
            .lock()
            .map_err(|_| BitCrapsError::BluetoothError {
                message: "Failed to lock scanning state".to_string(),
            })?;

        if !*scanning {
            return Ok(()); // Not scanning
        }

        #[cfg(target_os = "android")]
        if let (Some(vm), Some(service)) = (&self.java_vm, &self.ble_service) {
            let env = vm
                .attach_current_thread()
                .map_err(|e| BitCrapsError::BluetoothError {
                    message: format!("Failed to attach to JVM: {}", e),
                })?;

            // Call Java method to stop scanning
            let result = env
                .call_method(service, "stopScanning", "()Z", &[])
                .map_err(|e| BitCrapsError::BluetoothError {
                    message: format!("Failed to call stopScanning: {}", e),
                })?;

            let success = result.z().map_err(|e| BitCrapsError::BluetoothError {
                message: format!("Failed to get boolean result: {}", e),
            })?;

            if success {
                *scanning = false;
                log::info!("BLE scanning stopped successfully");
                Ok(())
            } else {
                Err(BitCrapsError::BluetoothError {
                    message: "Failed to stop BLE scanning".to_string(),
                })
            }
        } else {
            Err(BitCrapsError::BluetoothError {
                message: "JVM or BLE service not initialized".to_string(),
            })
        }

        #[cfg(not(target_os = "android"))]
        {
            *scanning = false;
            log::info!("Mock BLE scanning stopped (non-Android)");
            Ok(())
        }
    }

    /// Get discovered peers
    pub fn get_discovered_peers(&self) -> Result<Vec<AndroidPeerInfo>, BitCrapsError> {
        let peers = self
            .discovered_peers
            .lock()
            .map_err(|_| BitCrapsError::BluetoothError {
                message: "Failed to lock discovered peers".to_string(),
            })?;

        Ok(peers.values().cloned().collect())
    }

    /// Add or update discovered peer
    pub fn update_discovered_peer(&self, peer: AndroidPeerInfo) -> Result<(), BitCrapsError> {
        let mut peers =
            self.discovered_peers
                .lock()
                .map_err(|_| BitCrapsError::BluetoothError {
                    message: "Failed to lock discovered peers".to_string(),
                })?;

        peers.insert(peer.address.clone(), peer);
        Ok(())
    }

    /// Check if advertising is active
    pub fn is_advertising(&self) -> bool {
        self.is_advertising
            .lock()
            .map(|state| *state)
            .unwrap_or(false)
    }

    /// Check if scanning is active
    pub fn is_scanning(&self) -> bool {
        self.is_scanning.lock().map(|state| *state).unwrap_or(false)
    }
}
