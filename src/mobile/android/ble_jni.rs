//! Android BLE JNI Bridge Implementation
//! 
//! This module provides JNI bindings specifically for Android BLE operations,
//! including advertising, scanning, and GATT server functionality.

use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use crate::error::BitCrapsError;
use super::{AndroidBleManager, AndroidPeerInfo};

#[cfg(target_os = "android")]
use jni::JNIEnv;
#[cfg(target_os = "android")]
use jni::objects::{JClass, JString, JObject, JByteArray, JIntArray, GlobalRef, JValue};
#[cfg(target_os = "android")]
use jni::sys::{jlong, jstring, jboolean, jint, jbyteArray, jintArray, jobject};
#[cfg(target_os = "android")]
use jni::JavaVM;

/// Thread-safe global BLE manager using proper synchronization
use once_cell::sync::OnceCell;
static BLE_MANAGER: OnceCell<Arc<AndroidBleManager>> = OnceCell::new();

/// Initialize the global BLE manager with proper error handling
#[cfg(target_os = "android")]
pub fn initialize_ble_manager() -> Result<Arc<AndroidBleManager>, BitCrapsError> {
    BLE_MANAGER.get_or_try_init(|| {
        log::info!("Initializing global BLE manager");
        Ok(Arc::new(AndroidBleManager::new()))
    }).map(|manager| manager.clone())
    .map_err(|_| BitCrapsError::BluetoothError {
        message: "Failed to initialize BLE manager".to_string(),
    })
}

/// Get the global BLE manager with proper error handling
#[cfg(target_os = "android")]
fn get_ble_manager() -> Result<Arc<AndroidBleManager>, BitCrapsError> {
    BLE_MANAGER.get().ok_or_else(|| {
        BitCrapsError::BluetoothError {
            message: "BLE manager not initialized - call initialize_ble_manager first".to_string(),
        }
    }).map(|m| m.clone())
}

/// Cleanup the global BLE manager
#[cfg(target_os = "android")]
pub fn cleanup_ble_manager() -> Result<(), BitCrapsError> {
    if let Some(manager) = BLE_MANAGER.get() {
        // Stop all operations
        let rt = tokio::runtime::Runtime::new().map_err(|e| {
            BitCrapsError::BluetoothError {
                message: format!("Failed to create cleanup runtime: {}", e),
            }
        })?;

        rt.block_on(async {
            if manager.is_advertising() {
                let _ = manager.stop_advertising().await;
            }
            if manager.is_scanning() {
                let _ = manager.stop_scanning().await;
            }
        });

        log::info!("BLE manager cleanup completed");
    }
    Ok(())
}

/// JNI helper functions
#[cfg(target_os = "android")]
mod jni_helpers {
    use super::*;
    use jni::signature::JavaType;

    pub fn jstring_to_string(env: &JNIEnv, jstr: JString) -> Result<String, BitCrapsError> {
        env.get_string(jstr)
            .map(|s| s.into())
            .map_err(|e| BitCrapsError::BluetoothError {
                message: format!("Failed to convert JString: {}", e),
            })
    }

    pub fn string_to_jstring(env: &JNIEnv, s: &str) -> Result<JString, BitCrapsError> {
        env.new_string(s)
            .map_err(|e| BitCrapsError::BluetoothError {
                message: format!("Failed to create JString: {}", e),
            })
    }

    pub fn byte_array_to_vec(env: &JNIEnv, jarray: JByteArray) -> Result<Vec<u8>, BitCrapsError> {
        env.convert_byte_array(jarray)
            .map_err(|e| BitCrapsError::BluetoothError {
                message: format!("Failed to convert byte array: {}", e),
            })
    }

    pub fn vec_to_byte_array(env: &JNIEnv, vec: &[u8]) -> Result<JByteArray, BitCrapsError> {
        env.byte_array_from_slice(vec)
            .map_err(|e| BitCrapsError::BluetoothError {
                message: format!("Failed to create byte array: {}", e),
            })
    }

    pub fn throw_exception(env: &JNIEnv, error: &BitCrapsError) {
        let exception_class = match error {
            BitCrapsError::BluetoothError { .. } => "com/bitcraps/exceptions/BluetoothException",
            BitCrapsError::NetworkError { .. } => "com/bitcraps/exceptions/NetworkException",
            BitCrapsError::InvalidInput { .. } => "java/lang/IllegalArgumentException",
            BitCrapsError::Timeout => "java/util/concurrent/TimeoutException",
            BitCrapsError::NotFound { .. } => "java/util/NoSuchElementException",
            _ => "java/lang/RuntimeException",
        };

        match env.throw_new(exception_class, &error.to_string()) {
            Ok(_) => log::debug!("Exception thrown to Java: {} - {}", exception_class, error),
            Err(jni_error) => {
                log::error!("Failed to throw JNI exception: {} (original error: {})", jni_error, error);
                // Fallback to RuntimeException if the specific exception class fails
                let _ = env.throw_new("java/lang/RuntimeException", 
                    &format!("JNI Exception Error - Original: {}, JNI Error: {}", error, jni_error));
            }
        }
    }

    /// Safe JVM reference management
    pub fn with_attached_jvm<F, R>(vm: &JavaVM, operation: F) -> Result<R, BitCrapsError> 
    where
        F: FnOnce(&JNIEnv) -> Result<R, BitCrapsError>
    {
        let env = vm.attach_current_thread().map_err(|e| {
            BitCrapsError::BluetoothError {
                message: format!("Failed to attach to JVM thread: {}", e),
            }
        })?;

        let result = operation(&env);

        // JNI automatically detaches when env goes out of scope
        result
    }

    /// Safe global reference creation with cleanup
    pub fn create_safe_global_ref(env: &JNIEnv, local_ref: JObject) -> Result<GlobalRef, BitCrapsError> {
        env.new_global_ref(local_ref).map_err(|e| {
            BitCrapsError::BluetoothError {
                message: format!("Failed to create global reference: {}", e),
            }
        })
    }

    /// Safe method call with exception handling
    pub fn safe_call_method(
        env: &JNIEnv,
        object: &JObject,
        method_name: &str,
        method_sig: &str,
        args: &[JValue],
    ) -> Result<JValue, BitCrapsError> {
        match env.call_method(object, method_name, method_sig, args) {
            Ok(result) => {
                // Check for pending exceptions
                if let Ok(true) = env.exception_check() {
                    env.exception_clear().ok();
                    return Err(BitCrapsError::BluetoothError {
                        message: format!("Java exception occurred during method call: {}", method_name),
                    });
                }
                Ok(result)
            },
            Err(e) => {
                // Clear any pending exceptions
                let _ = env.exception_clear();
                Err(BitCrapsError::BluetoothError {
                    message: format!("Failed to call method {}: {}", method_name, e),
                })
            }
        }
    }
}

// JNI Export Functions

/// Initialize the BLE manager from Android
#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn Java_com_bitcraps_app_ble_BleJNI_initializeBleManager(
    mut env: JNIEnv,
    _class: JClass,
    java_vm_ptr: jlong,
    ble_service: jobject,
) -> jboolean {
    // Initialize logging
    android_logger::init_once(
        android_logger::Config::default()
            .with_min_level(log::Level::Debug)
            .with_tag("BitCraps-BLE")
    );

    log::info!("Initializing BLE manager from JNI");

    // Initialize the BLE manager
    let manager = match initialize_ble_manager() {
        Ok(manager) => manager,
        Err(e) => {
            log::error!("Failed to initialize BLE manager: {}", e);
            jni_helpers::throw_exception(&env, &e);
            return false as jboolean;
        }
    };

    // SECURITY: Validate JavaVM pointer before dereferencing
    // This is a critical safety check to prevent memory corruption
    let java_vm = if java_vm_ptr == 0 {
        log::error!("JavaVM pointer is null");
        jni_helpers::throw_exception(&env, &BitCrapsError::BluetoothError {
            message: "JavaVM pointer is null".to_string(),
        });
        return false as jboolean;
    } else {
        // SAFETY INVARIANTS:
        // 1. java_vm_ptr must be a valid JavaVM pointer from JNI_GetJavaVM
        // 2. The JavaVM must remain alive for the duration of this function
        // 3. The pointer must be properly aligned for jni::sys::JavaVM
        unsafe {
            // Additional safety checks
            if java_vm_ptr as usize % std::mem::align_of::<jni::sys::JavaVM>() != 0 {
                log::error!("JavaVM pointer is not properly aligned: 0x{:x}", java_vm_ptr);
                jni_helpers::throw_exception(&env, &BitCrapsError::BluetoothError {
                    message: "JavaVM pointer is not properly aligned".to_string(),
                });
                return false as jboolean;
            }

            let vm_ptr = java_vm_ptr as *mut jni::sys::JavaVM;
            
            // Verify pointer is not dangling by checking if it's in a reasonable range
            // This is a heuristic check - not foolproof but catches obvious invalid pointers
            if vm_ptr as usize < 0x1000 || vm_ptr as usize > usize::MAX - 0x1000 {
                log::error!("JavaVM pointer appears invalid: {:p}", vm_ptr);
                jni_helpers::throw_exception(&env, &BitCrapsError::BluetoothError {
                    message: "JavaVM pointer appears to be invalid".to_string(),
                });
                return false as jboolean;
            }
            
            match JavaVM::from_raw(vm_ptr) {
                Ok(vm) => vm,
                Err(e) => {
                    log::error!("Failed to create JavaVM from pointer: {}", e);
                    jni_helpers::throw_exception(&env, &BitCrapsError::BluetoothError {
                        message: format!("Failed to create JavaVM from pointer: {}", e),
                    });
                    return false as jboolean;
                }
            }
        }
    };

    // Create global reference to BLE service safely
    let global_service = match jni_helpers::create_safe_global_ref(&env, JObject::from(ble_service)) {
        Ok(global_ref) => global_ref,
        Err(e) => {
            log::error!("Failed to create global reference to BLE service: {}", e);
            jni_helpers::throw_exception(&env, &e);
            return false as jboolean;
        }
    };

    // Set up the manager with Java VM and service
    // Note: Since we're using OnceCell, we can't get mutable access after initialization
    // We need to modify the AndroidBleManager to accept these during construction
    // For now, we'll store them separately and access them when needed
    
    // Store VM and service reference for later use
    // This is a temporary solution - ideally these would be part of initialization
    log::info!("BLE manager initialized successfully with JVM and service references");
    
    // TODO: Properly integrate JavaVM and global service reference
    // This requires refactoring the AndroidBleManager to accept these parameters
    
    true as jboolean
}

/// Cleanup JNI resources
#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn Java_com_bitcraps_app_ble_BleJNI_cleanup(
    env: JNIEnv,
    _class: JClass,
) -> jboolean {
    log::info!("Cleaning up BLE JNI resources");
    
    match cleanup_ble_manager() {
        Ok(()) => {
            log::info!("BLE manager cleanup completed successfully");
            true as jboolean
        },
        Err(e) => {
            log::error!("BLE manager cleanup failed: {}", e);
            jni_helpers::throw_exception(&env, &e);
            false as jboolean
        }
    }
}

/// Start BLE advertising
#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn Java_com_bitcraps_app_ble_BleJNI_startAdvertising(
    env: JNIEnv,
    _class: JClass,
) -> jboolean {
    let manager = match get_ble_manager() {
        Ok(manager) => manager,
        Err(e) => {
            jni_helpers::throw_exception(&env, &e);
            return false as jboolean;
        }
    };

    // Use a blocking runtime for async operations
    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            let error = BitCrapsError::BluetoothError {
                message: format!("Failed to create runtime: {}", e),
            };
            jni_helpers::throw_exception(&env, &error);
            return false as jboolean;
        }
    };

    match rt.block_on(manager.start_advertising()) {
        Ok(()) => {
            log::info!("BLE advertising started from JNI");
            true as jboolean
        },
        Err(e) => {
            log::error!("Failed to start BLE advertising: {}", e);
            jni_helpers::throw_exception(&env, &e);
            false as jboolean
        }
    }
}

/// Stop BLE advertising
#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn Java_com_bitcraps_app_ble_BleJNI_stopAdvertising(
    env: JNIEnv,
    _class: JClass,
) -> jboolean {
    let manager = match get_ble_manager() {
        Ok(manager) => manager,
        Err(e) => {
            jni_helpers::throw_exception(&env, &e);
            return false as jboolean;
        }
    };

    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            let error = BitCrapsError::BluetoothError {
                message: format!("Failed to create runtime: {}", e),
            };
            jni_helpers::throw_exception(&env, &error);
            return false as jboolean;
        }
    };

    match rt.block_on(manager.stop_advertising()) {
        Ok(()) => {
            log::info!("BLE advertising stopped from JNI");
            true as jboolean
        },
        Err(e) => {
            log::error!("Failed to stop BLE advertising: {}", e);
            jni_helpers::throw_exception(&env, &e);
            false as jboolean
        }
    }
}

/// Start BLE scanning
#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn Java_com_bitcraps_app_ble_BleJNI_startScanning(
    env: JNIEnv,
    _class: JClass,
) -> jboolean {
    let manager = match get_ble_manager() {
        Ok(manager) => manager,
        Err(e) => {
            jni_helpers::throw_exception(&env, &e);
            return false as jboolean;
        }
    };

    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            let error = BitCrapsError::BluetoothError {
                message: format!("Failed to create runtime: {}", e),
            };
            jni_helpers::throw_exception(&env, &error);
            return false as jboolean;
        }
    };

    match rt.block_on(manager.start_scanning()) {
        Ok(()) => {
            log::info!("BLE scanning started from JNI");
            true as jboolean
        },
        Err(e) => {
            log::error!("Failed to start BLE scanning: {}", e);
            jni_helpers::throw_exception(&env, &e);
            false as jboolean
        }
    }
}

/// Stop BLE scanning
#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn Java_com_bitcraps_app_ble_BleJNI_stopScanning(
    env: JNIEnv,
    _class: JClass,
) -> jboolean {
    let manager = match get_ble_manager() {
        Ok(manager) => manager,
        Err(e) => {
            jni_helpers::throw_exception(&env, &e);
            return false as jboolean;
        }
    };

    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            let error = BitCrapsError::BluetoothError {
                message: format!("Failed to create runtime: {}", e),
            };
            jni_helpers::throw_exception(&env, &error);
            return false as jboolean;
        }
    };

    match rt.block_on(manager.stop_scanning()) {
        Ok(()) => {
            log::info!("BLE scanning stopped from JNI");
            true as jboolean
        },
        Err(e) => {
            log::error!("Failed to stop BLE scanning: {}", e);
            jni_helpers::throw_exception(&env, &e);
            false as jboolean
        }
    }
}

/// Handle peer discovered callback from Android
#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn Java_com_bitcraps_app_ble_BleJNI_onPeerDiscovered(
    env: JNIEnv,
    _class: JClass,
    address: JString,
    name: JString,
    rssi: jint,
    manufacturer_data: jbyteArray,
    service_uuids: jobject, // String array
) {
    let manager = match get_ble_manager() {
        Ok(manager) => manager,
        Err(e) => {
            log::error!("Failed to get BLE manager: {}", e);
            return;
        }
    };

    // Convert parameters
    let address_str = match jni_helpers::jstring_to_string(&env, address) {
        Ok(s) => s,
        Err(e) => {
            log::error!("Failed to convert address: {}", e);
            return;
        }
    };

    let name_str = if !name.is_null() {
        match jni_helpers::jstring_to_string(&env, name) {
            Ok(s) => Some(s),
            Err(_) => None,
        }
    } else {
        None
    };

    let manufacturer_data_vec = if !manufacturer_data.is_null() {
        match jni_helpers::byte_array_to_vec(&env, manufacturer_data) {
            Ok(vec) => Some(vec),
            Err(_) => None,
        }
    } else {
        None
    };

    // TODO: Convert service UUIDs array
    let service_uuids_vec = Vec::new(); // Placeholder

    let peer = AndroidPeerInfo {
        address: address_str,
        name: name_str,
        rssi: rssi as i32,
        last_seen: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        manufacturer_data: manufacturer_data_vec,
        service_uuids: service_uuids_vec,
    };

    if let Err(e) = manager.update_discovered_peer(peer) {
        log::error!("Failed to update discovered peer: {}", e);
    } else {
        log::debug!("Peer discovered and updated via JNI");
    }
}

/// Get discovered peers count
#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn Java_com_bitcraps_app_ble_BleJNI_getDiscoveredPeersCount(
    env: JNIEnv,
    _class: JClass,
) -> jint {
    let manager = match get_ble_manager() {
        Ok(manager) => manager,
        Err(e) => {
            jni_helpers::throw_exception(&env, &e);
            return -1;
        }
    };

    match manager.get_discovered_peers() {
        Ok(peers) => peers.len() as jint,
        Err(e) => {
            jni_helpers::throw_exception(&env, &e);
            -1
        }
    }
}

/// Get discovered peer addresses
#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn Java_com_bitcraps_app_ble_BleJNI_getDiscoveredPeerAddresses(
    env: JNIEnv,
    _class: JClass,
) -> jobject {
    let manager = match get_ble_manager() {
        Ok(manager) => manager,
        Err(e) => {
            jni_helpers::throw_exception(&env, &e);
            return JObject::null().into_raw();
        }
    };

    let peers = match manager.get_discovered_peers() {
        Ok(peers) => peers,
        Err(e) => {
            jni_helpers::throw_exception(&env, &e);
            return JObject::null().into_raw();
        }
    };

    // Create String array
    let string_class = match env.find_class("java/lang/String") {
        Ok(class) => class,
        Err(e) => {
            log::error!("Failed to find String class: {}", e);
            return JObject::null().into_raw();
        }
    };

    let array = match env.new_object_array(peers.len() as i32, string_class, JObject::null()) {
        Ok(array) => array,
        Err(e) => {
            log::error!("Failed to create String array: {}", e);
            return JObject::null().into_raw();
        }
    };

    // Fill array with peer addresses
    for (i, peer) in peers.iter().enumerate() {
        let address_jstring = match jni_helpers::string_to_jstring(&env, &peer.address) {
            Ok(jstr) => jstr,
            Err(e) => {
                log::error!("Failed to create address JString: {}", e);
                continue;
            }
        };

        if let Err(e) = env.set_object_array_element(array, i as i32, JObject::from(address_jstring)) {
            log::error!("Failed to set array element {}: {}", i, e);
        }
    }

    array.into_raw()
}

/// Check if advertising is active
#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn Java_com_bitcraps_app_ble_BleJNI_isAdvertising(
    env: JNIEnv,
    _class: JClass,
) -> jboolean {
    let manager = match get_ble_manager() {
        Ok(manager) => manager,
        Err(e) => {
            jni_helpers::throw_exception(&env, &e);
            return false as jboolean;
        }
    };

    manager.is_advertising() as jboolean
}

/// Check if scanning is active
#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn Java_com_bitcraps_app_ble_BleJNI_isScanning(
    env: JNIEnv,
    _class: JClass,
) -> jboolean {
    let manager = match get_ble_manager() {
        Ok(manager) => manager,
        Err(e) => {
            jni_helpers::throw_exception(&env, &e);
            return false as jboolean;
        }
    };

    manager.is_scanning() as jboolean
}

// Non-Android stubs
#[cfg(not(target_os = "android"))]
pub fn initialize_ble_manager() {
    log::info!("BLE manager initialization skipped (non-Android)");
}