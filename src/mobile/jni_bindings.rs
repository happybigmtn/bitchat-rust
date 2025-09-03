//! JNI bindings for Android integration
//!
//! This module provides JNI wrappers around the UniFFI interface
//! for direct Android integration and performance-critical operations.

#[cfg(target_os = "android")]
use jni::objects::{JClass, JObject, JObjectArray, JString, JValue};
#[cfg(target_os = "android")]
use jni::signature::{JavaType, Primitive};
#[cfg(target_os = "android")]
use jni::sys::{jboolean, jint, jlong, jobjectArray, jstring};
#[cfg(target_os = "android")]
use jni::JNIEnv;

use super::*;

/// Android-specific JNI interface for BitCraps
#[cfg(target_os = "android")]
pub struct AndroidJNI {
    nodes: Arc<Mutex<HashMap<i64, Arc<BitCrapsNode>>>>,
    next_handle: Arc<Mutex<i64>>,
}

#[cfg(target_os = "android")]
impl AndroidJNI {
    pub fn new() -> Self {
        Self {
            nodes: Arc::new(Mutex::new(HashMap::new())),
            next_handle: Arc::new(Mutex::new(1)),
        }
    }

    fn get_next_handle(&self) -> i64 {
        if let Ok(mut handle) = self.next_handle.lock() {
            let current = *handle;
            *handle += 1;
            current
        } else {
            1
        }
    }

    fn store_node(&self, node: Arc<BitCrapsNode>) -> i64 {
        let handle = self.get_next_handle();
        if let Ok(mut nodes) = self.nodes.lock() {
            nodes.insert(handle, node);
        }
        handle
    }

    fn get_node(&self, handle: i64) -> Option<Arc<BitCrapsNode>> {
        if let Ok(nodes) = self.nodes.lock() {
            nodes.get(&handle).cloned()
        } else {
            None
        }
    }

    fn remove_node(&self, handle: i64) -> Option<Arc<BitCrapsNode>> {
        if let Ok(mut nodes) = self.nodes.lock() {
            nodes.remove(&handle)
        } else {
            None
        }
    }
}

// Global JNI interface instance
#[cfg(target_os = "android")]
static ANDROID_JNI: once_cell::sync::Lazy<AndroidJNI> =
    once_cell::sync::Lazy::new(|| AndroidJNI::new());

/// JNI helper functions
#[cfg(target_os = "android")]
mod jni_helpers {
    use super::*;

    pub fn jstring_to_string(env: &JNIEnv, jstr: JString) -> Result<String, BitCrapsError> {
        env.get_string(jstr)
            .map(|s| s.into())
            .map_err(|e| BitCrapsError::InvalidInput {
                reason: format!("Failed to convert JString: {}", e),
            })
    }

    pub fn string_to_jstring(env: &JNIEnv, s: &str) -> Result<JString, BitCrapsError> {
        env.new_string(s).map_err(|e| BitCrapsError::InvalidInput {
            reason: format!("Failed to create JString: {}", e),
        })
    }

    pub fn throw_exception(env: &JNIEnv, error: &BitCrapsError) {
        let exception_class = match error {
            BitCrapsError::InitializationError { .. } => {
                "com/bitcraps/exceptions/InitializationException"
            }
            BitCrapsError::BluetoothError { .. } => "com/bitcraps/exceptions/BluetoothException",
            BitCrapsError::NetworkError { .. } => "com/bitcraps/exceptions/NetworkException",
            BitCrapsError::GameError { .. } => "com/bitcraps/exceptions/GameException",
            BitCrapsError::CryptoError { .. } => "com/bitcraps/exceptions/CryptoException",
            BitCrapsError::InvalidInput { .. } => "java/lang/IllegalArgumentException",
            BitCrapsError::Timeout => "java/util/concurrent/TimeoutException",
            BitCrapsError::NotFound { .. } => "java/util/NoSuchElementException",
        };

        let _ = env.throw_new(exception_class, &error.to_string());
    }
}

// JNI export functions for Android

/// Initialize the BitCraps library
#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn Java_com_bitcraps_BitCrapsNative_initialize(
    env: JNIEnv,
    _class: JClass,
) -> jboolean {
    // Initialize logging for Android
    android_logger::init_once(
        android_logger::Config::default()
            .with_min_level(log::Level::Info)
            .with_tag("BitCraps"),
    );

    log::info!("BitCraps native library initialized");
    true as jboolean
}

/// Create a new BitCraps node
#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn Java_com_bitcraps_BitCrapsNative_createNode(
    env: JNIEnv,
    _class: JClass,
    data_dir: JString,
    pow_difficulty: jint,
    protocol_version: jint,
) -> jlong {
    let config = match (|| -> Result<BitCrapsConfig, BitCrapsError> {
        let data_dir = jni_helpers::jstring_to_string(&env, data_dir)?;

        Ok(BitCrapsConfig {
            data_dir,
            pow_difficulty: pow_difficulty as u32,
            protocol_version: protocol_version as u16,
            power_mode: PowerMode::Balanced,
            platform_config: Some(PlatformConfigBuilder::new(PlatformType::Android).build()),
            enable_logging: true,
            log_level: LogLevel::Info,
        })
    })() {
        Ok(config) => config,
        Err(e) => {
            jni_helpers::throw_exception(&env, &e);
            return 0;
        }
    };

    match create_node(config) {
        Ok(node) => ANDROID_JNI.store_node(node),
        Err(e) => {
            jni_helpers::throw_exception(&env, &e);
            0
        }
    }
}

/// Start discovery on a BitCraps node
#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn Java_com_bitcraps_BitCrapsNative_startDiscovery(
    env: JNIEnv,
    _class: JClass,
    node_handle: jlong,
) -> jboolean {
    let node = match ANDROID_JNI.get_node(node_handle) {
        Some(node) => node,
        None => {
            jni_helpers::throw_exception(
                &env,
                &BitCrapsError::InvalidInput {
                    reason: "Invalid node handle".to_string(),
                },
            );
            return false as jboolean;
        }
    };

    // Start discovery asynchronously to prevent ANR
    let rt = tokio::runtime::Runtime::new().unwrap();
    let node_clone = node.clone();
    rt.spawn(async move {
        match timeout(Duration::from_secs(5), node_clone.start_discovery()).await {
            Ok(Ok(())) => {
                log::info!("Discovery started successfully");
            }
            Ok(Err(e)) => {
                log::error!("Failed to start discovery: {}", e);
            }
            Err(_) => {
                log::error!("Discovery start timed out");
            }
        }
    });

    // Return immediately - Android should poll node status for confirmation
    true as jboolean
}

/// Stop discovery on a BitCraps node
#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn Java_com_bitcraps_BitCrapsNative_stopDiscovery(
    env: JNIEnv,
    _class: JClass,
    node_handle: jlong,
) -> jboolean {
    let node = match ANDROID_JNI.get_node(node_handle) {
        Some(node) => node,
        None => {
            jni_helpers::throw_exception(
                &env,
                &BitCrapsError::InvalidInput {
                    reason: "Invalid node handle".to_string(),
                },
            );
            return false as jboolean;
        }
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    let node_clone = node.clone();
    rt.spawn(async move {
        match timeout(Duration::from_secs(5), node_clone.stop_discovery()).await {
            Ok(Ok(())) => {
                log::info!("Discovery stopped successfully");
            }
            Ok(Err(e)) => {
                log::error!("Failed to stop discovery: {}", e);
            }
            Err(_) => {
                log::error!("Discovery stop timed out");
            }
        }
    });

    // Return immediately - Android should poll node status for confirmation
    true as jboolean
}

/// Poll for the next event
#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn Java_com_bitcraps_BitCrapsNative_pollEvent(
    env: JNIEnv,
    _class: JClass,
    node_handle: jlong,
) -> jstring {
    let node = match ANDROID_JNI.get_node(node_handle) {
        Some(node) => node,
        None => {
            jni_helpers::throw_exception(
                &env,
                &BitCrapsError::InvalidInput {
                    reason: "Invalid node handle".to_string(),
                },
            );
            return std::ptr::null_mut();
        }
    };

    // Poll events without blocking using a very short timeout
    let rt = tokio::runtime::Runtime::new().unwrap();
    let node_clone = node.clone();
    let (tx, rx) = oneshot::channel();

    rt.spawn(async move {
        // Use very short timeout for polling to prevent ANR
        let result = timeout(Duration::from_millis(50), node_clone.poll_event()).await;
        let _ = tx.send(result);
    });

    // Try to get result immediately, return null if not ready
    match rx.try_recv() {
        Ok(Ok(Some(event))) => {
            // Serialize event to JSON
            match serde_json::to_string(&event) {
                Ok(json) => match jni_helpers::string_to_jstring(&env, &json) {
                    Ok(jstr) => jstr.into_raw(),
                    Err(e) => {
                        jni_helpers::throw_exception(&env, &e);
                        std::ptr::null_mut()
                    }
                },
                Err(e) => {
                    jni_helpers::throw_exception(
                        &env,
                        &BitCrapsError::InvalidInput {
                            reason: format!("Failed to serialize event: {}", e),
                        },
                    );
                    std::ptr::null_mut()
                }
            }
        }
        _ => std::ptr::null_mut(), // No event available or not ready yet
    }
}

/// Get node status
#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn Java_com_bitcraps_BitCrapsNative_getNodeStatus(
    env: JNIEnv,
    _class: JClass,
    node_handle: jlong,
) -> jstring {
    let node = match ANDROID_JNI.get_node(node_handle) {
        Some(node) => node,
        None => {
            jni_helpers::throw_exception(
                &env,
                &BitCrapsError::InvalidInput {
                    reason: "Invalid node handle".to_string(),
                },
            );
            return std::ptr::null_mut();
        }
    };

    let status = node.get_status();
    match serde_json::to_string(&status) {
        Ok(json) => match jni_helpers::string_to_jstring(&env, &json) {
            Ok(jstr) => jstr.into_raw(),
            Err(e) => {
                jni_helpers::throw_exception(&env, &e);
                std::ptr::null_mut()
            }
        },
        Err(e) => {
            jni_helpers::throw_exception(
                &env,
                &BitCrapsError::InvalidInput {
                    reason: format!("Failed to serialize status: {}", e),
                },
            );
            std::ptr::null_mut()
        }
    }
}

/// Set power mode
#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn Java_com_bitcraps_BitCrapsNative_setPowerMode(
    env: JNIEnv,
    _class: JClass,
    node_handle: jlong,
    power_mode: jint,
) -> jboolean {
    let node = match ANDROID_JNI.get_node(node_handle) {
        Some(node) => node,
        None => {
            jni_helpers::throw_exception(
                &env,
                &BitCrapsError::InvalidInput {
                    reason: "Invalid node handle".to_string(),
                },
            );
            return false as jboolean;
        }
    };

    let mode = match power_mode {
        0 => PowerMode::HighPerformance,
        1 => PowerMode::Balanced,
        2 => PowerMode::BatterySaver,
        3 => PowerMode::UltraLowPower,
        _ => {
            jni_helpers::throw_exception(
                &env,
                &BitCrapsError::InvalidInput {
                    reason: "Invalid power mode".to_string(),
                },
            );
            return false as jboolean;
        }
    };

    match node.set_power_mode(mode) {
        Ok(()) => true as jboolean,
        Err(e) => {
            jni_helpers::throw_exception(&env, &e);
            false as jboolean
        }
    }
}

/// Destroy a BitCraps node
#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn Java_com_bitcraps_BitCrapsNative_destroyNode(
    _env: JNIEnv,
    _class: JClass,
    node_handle: jlong,
) {
    ANDROID_JNI.remove_node(node_handle);
    log::info!("Destroyed BitCraps node: {}", node_handle);
}

// Non-Android stubs
#[cfg(not(target_os = "android"))]
pub fn initialize_android_logging() {
    // No-op on non-Android platforms
}

// Serde implementations for mobile events
use serde::Serialize;

impl Serialize for GameEvent {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        match self {
            GameEvent::PeerDiscovered { peer } => {
                let mut state = serializer.serialize_struct("GameEvent", 2)?;
                state.serialize_field("type", "PeerDiscovered")?;
                state.serialize_field("peer", peer)?;
                state.end()
            }
            GameEvent::PeerConnected { peer_id } => {
                let mut state = serializer.serialize_struct("GameEvent", 2)?;
                state.serialize_field("type", "PeerConnected")?;
                state.serialize_field("peer_id", peer_id)?;
                state.end()
            }
            GameEvent::DiceRolled { roll } => {
                let mut state = serializer.serialize_struct("GameEvent", 2)?;
                state.serialize_field("type", "DiceRolled")?;
                state.serialize_field("roll", roll)?;
                state.end()
            }
            // Add more event serializations as needed
            _ => {
                let mut state = serializer.serialize_struct("GameEvent", 1)?;
                state.serialize_field("type", "Unknown")?;
                state.end()
            }
        }
    }
}

impl Serialize for NodeStatus {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("NodeStatus", 6)?;
        state.serialize_field("state", &format!("{:?}", self.state))?;
        state.serialize_field("bluetooth_enabled", &self.bluetooth_enabled)?;
        state.serialize_field("discovery_active", &self.discovery_active)?;
        state.serialize_field("current_game_id", &self.current_game_id)?;
        state.serialize_field("active_connections", &self.active_connections)?;
        state.serialize_field(
            "current_power_mode",
            &format!("{:?}", self.current_power_mode),
        )?;
        state.end()
    }
}

impl Serialize for PeerInfo {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("PeerInfo", 5)?;
        state.serialize_field("peer_id", &self.peer_id)?;
        state.serialize_field("display_name", &self.display_name)?;
        state.serialize_field("signal_strength", &self.signal_strength)?;
        state.serialize_field("last_seen", &self.last_seen)?;
        state.serialize_field("is_connected", &self.is_connected)?;
        state.end()
    }
}

impl Serialize for DiceRoll {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("DiceRoll", 4)?;
        state.serialize_field("die1", &self.die1)?;
        state.serialize_field("die2", &self.die2)?;
        state.serialize_field("roll_time", &self.roll_time)?;
        state.serialize_field("roller_peer_id", &self.roller_peer_id)?;
        state.end()
    }
}
