# Rust-Android JNI Integration Guide

## Overview

This document provides a comprehensive guide for integrating the BitCraps Rust core with the Android application using JNI (Java Native Interface). The integration allows the Android app to leverage the high-performance Rust implementation while maintaining a native Android UI.

---

## Architecture Overview

```
┌─────────────────────────────────────────────┐
│            Android Application              │
│                                             │
│  ┌─────────────────────────────────────┐   │
│  │        Kotlin/Java Layer            │   │
│  │  (BitCrapsService, BleManager)      │   │
│  └──────────────┬──────────────────────┘   │
│                 │                           │
│  ┌──────────────▼──────────────────────┐   │
│  │         JNI Bridge Layer            │   │
│  │    (Native method declarations)     │   │
│  └──────────────┬──────────────────────┘   │
│                 │                           │
└─────────────────┼───────────────────────────┘
                  │
┌─────────────────▼───────────────────────────┐
│           Rust Native Library               │
│                                             │
│  ┌─────────────────────────────────────┐   │
│  │         JNI Wrapper Layer           │   │
│  │    (extern "C" functions)           │   │
│  └──────────────┬──────────────────────┘   │
│                 │                           │
│  ┌──────────────▼──────────────────────┐   │
│  │       BitCraps Core Library         │   │
│  │  (Mesh, Protocol, Consensus, etc.)  │   │
│  └─────────────────────────────────────┘   │
│                                             │
└─────────────────────────────────────────────┘
```

---

## Step 1: Rust Library Setup

### Cargo.toml Configuration

```toml
[package]
name = "bitcraps-android"
version = "0.1.0"
edition = "2021"

[lib]
name = "bitcraps_android"
crate-type = ["cdylib"]  # Creates .so file for Android

[dependencies]
jni = "0.21"
bitcraps = { path = "../.." }  # Core library
tokio = { version = "1", features = ["rt-multi-thread"] }
once_cell = "1.20"
log = "0.4"
android_logger = "0.14"

[target.'cfg(target_os = "android")'.dependencies]
ndk-context = "0.1"
```

### Directory Structure

```
bitcraps/
├── src/
│   └── lib.rs          # Core library
├── android/
│   ├── src/
│   │   └── lib.rs      # Android-specific JNI wrapper
│   └── Cargo.toml      # Android library configuration
└── Cargo.toml          # Workspace configuration
```

---

## Step 2: JNI Wrapper Implementation

### src/android/lib.rs

```rust
use jni::JNIEnv;
use jni::objects::{JClass, JObject, JString, JValue};
use jni::sys::{jboolean, jint, jlong, jstring, JNI_VERSION_1_6};
use once_cell::sync::Lazy;
use std::sync::Arc;
use tokio::runtime::Runtime;
use bitcraps::{MeshService, BitcrapsConfig};

// Single runtime for the entire application
static RUNTIME: Lazy<Runtime> = Lazy::new(|| {
    Runtime::new().expect("Failed to create Tokio runtime")
});

/// Called when the library is loaded
#[no_mangle]
pub extern "C" fn JNI_OnLoad(vm: jni::JavaVM, _: *mut std::ffi::c_void) -> jint {
    // Initialize Android logger
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Debug)
            .with_tag("BitCraps-Rust")
    );
    
    log::info!("BitCraps Rust library loaded");
    
    // Store JavaVM for later use
    ndk_context::initialize_android_context(&vm, std::ptr::null_mut());
    
    JNI_VERSION_1_6
}

/// Create a new BitCraps node
#[no_mangle]
pub extern "C" fn Java_com_bitcraps_app_service_BitCrapsService_nativeCreateNode(
    mut env: JNIEnv,
    _class: JClass,
    config_json: JString,
) -> jlong {
    let config_str: String = env
        .get_string(&config_json)
        .expect("Invalid config string")
        .into();
    
    let config: BitcrapsConfig = serde_json::from_str(&config_str)
        .expect("Invalid config JSON");
    
    let node = RUNTIME.block_on(async {
        MeshService::new(config).await
    }).expect("Failed to create node");
    
    // Return pointer to node as jlong
    Box::into_raw(Box::new(node)) as jlong
}

/// Start discovery
#[no_mangle]
pub extern "C" fn Java_com_bitcraps_app_service_BitCrapsService_nativeStartDiscovery(
    _env: JNIEnv,
    _class: JClass,
    node_ptr: jlong,
) -> jboolean {
    let node = unsafe {
        &*(node_ptr as *const MeshService)
    };
    
    RUNTIME.spawn(async move {
        if let Err(e) = node.start_discovery().await {
            log::error!("Discovery failed: {}", e);
        }
    });
    
    jni::sys::JNI_TRUE
}

/// Connect to a peer
#[no_mangle]
pub extern "C" fn Java_com_bitcraps_app_service_BitCrapsService_nativeConnectToPeer(
    mut env: JNIEnv,
    _class: JClass,
    node_ptr: jlong,
    peer_address: JString,
) -> jboolean {
    let address: String = env
        .get_string(&peer_address)
        .expect("Invalid address")
        .into();
    
    let node = unsafe {
        &*(node_ptr as *const MeshService)
    };
    
    let result = RUNTIME.block_on(async {
        node.connect_to_peer(&address).await
    });
    
    match result {
        Ok(_) => jni::sys::JNI_TRUE,
        Err(e) => {
            log::error!("Connection failed: {}", e);
            jni::sys::JNI_FALSE
        }
    }
}

/// Send a message
#[no_mangle]
pub extern "C" fn Java_com_bitcraps_app_service_BitCrapsService_nativeSendMessage(
    mut env: JNIEnv,
    _class: JClass,
    node_ptr: jlong,
    peer_id: JString,
    message: JString,
) -> jboolean {
    let peer_id_str: String = env
        .get_string(&peer_id)
        .expect("Invalid peer ID")
        .into();
    
    let message_str: String = env
        .get_string(&message)
        .expect("Invalid message")
        .into();
    
    let node = unsafe {
        &*(node_ptr as *const MeshService)
    };
    
    let result = RUNTIME.block_on(async {
        node.send_message(&peer_id_str, message_str.as_bytes()).await
    });
    
    match result {
        Ok(_) => jni::sys::JNI_TRUE,
        Err(e) => {
            log::error!("Send failed: {}", e);
            jni::sys::JNI_FALSE
        }
    }
}

/// Poll for events
#[no_mangle]
pub extern "C" fn Java_com_bitcraps_app_service_BitCrapsService_nativePollEvent(
    mut env: JNIEnv,
    _class: JClass,
    node_ptr: jlong,
) -> jstring {
    let node = unsafe {
        &*(node_ptr as *const MeshService)
    };
    
    let event = RUNTIME.block_on(async {
        node.poll_event().await
    });
    
    match event {
        Some(e) => {
            let json = serde_json::to_string(&e).unwrap();
            env.new_string(json)
                .expect("Failed to create string")
                .into_raw()
        }
        None => std::ptr::null_mut()
    }
}

/// Clean up node
#[no_mangle]
pub extern "C" fn Java_com_bitcraps_app_service_BitCrapsService_nativeDestroyNode(
    _env: JNIEnv,
    _class: JClass,
    node_ptr: jlong,
) {
    if node_ptr != 0 {
        unsafe {
            let _ = Box::from_raw(node_ptr as *mut MeshService);
            // Node is automatically dropped and cleaned up
        }
        log::info!("Node destroyed");
    }
}

/// Handle callback from Java
#[no_mangle]
pub extern "C" fn Java_com_bitcraps_app_service_BitCrapsService_nativeOnBleData(
    mut env: JNIEnv,
    _class: JClass,
    node_ptr: jlong,
    peer_id: JString,
    data: jni::objects::JByteArray,
) {
    let peer_id_str: String = env
        .get_string(&peer_id)
        .expect("Invalid peer ID")
        .into();
    
    let data_vec = env
        .convert_byte_array(&data)
        .expect("Invalid data");
    
    let node = unsafe {
        &*(node_ptr as *const MeshService)
    };
    
    RUNTIME.spawn(async move {
        if let Err(e) = node.handle_incoming_data(&peer_id_str, &data_vec).await {
            log::error!("Failed to handle BLE data: {}", e);
        }
    });
}
```

---

## Step 3: Android Kotlin Integration

### BitCrapsService.kt

```kotlin
package com.bitcraps.app.service

import android.app.Service
import android.content.Intent
import android.os.IBinder
import android.util.Log
import kotlinx.coroutines.*
import org.json.JSONObject

class BitCrapsService : Service() {
    companion object {
        private const val TAG = "BitCrapsService"
        
        init {
            System.loadLibrary("bitcraps_android")
        }
    }
    
    private var nodePtr: Long = 0
    private val scope = CoroutineScope(Dispatchers.IO)
    private var eventPollingJob: Job? = null
    
    // Native method declarations
    private external fun nativeCreateNode(configJson: String): Long
    private external fun nativeStartDiscovery(nodePtr: Long): Boolean
    private external fun nativeConnectToPeer(nodePtr: Long, peerAddress: String): Boolean
    private external fun nativeSendMessage(nodePtr: Long, peerId: String, message: String): Boolean
    private external fun nativePollEvent(nodePtr: Long): String?
    private external fun nativeDestroyNode(nodePtr: Long)
    private external fun nativeOnBleData(nodePtr: Long, peerId: String, data: ByteArray)
    
    override fun onCreate() {
        super.onCreate()
        initializeNode()
        startEventPolling()
    }
    
    private fun initializeNode() {
        val config = JSONObject().apply {
            put("pow_difficulty", 10)
            put("max_connections", 50)
            put("protocol_version", 1)
        }
        
        nodePtr = nativeCreateNode(config.toString())
        if (nodePtr != 0L) {
            Log.i(TAG, "BitCraps node created successfully")
            nativeStartDiscovery(nodePtr)
        } else {
            Log.e(TAG, "Failed to create BitCraps node")
        }
    }
    
    private fun startEventPolling() {
        eventPollingJob = scope.launch {
            while (isActive) {
                val eventJson = nativePollEvent(nodePtr)
                if (eventJson != null) {
                    handleEvent(eventJson)
                }
                delay(100) // Poll every 100ms
            }
        }
    }
    
    private fun handleEvent(eventJson: String) {
        try {
            val event = JSONObject(eventJson)
            when (event.getString("type")) {
                "PeerDiscovered" -> {
                    val peerId = event.getString("peer_id")
                    Log.i(TAG, "Peer discovered: $peerId")
                    // Notify UI
                }
                "MessageReceived" -> {
                    val message = event.getString("message")
                    val from = event.getString("from")
                    Log.i(TAG, "Message from $from: $message")
                    // Process message
                }
                "ConnectionLost" -> {
                    val peerId = event.getString("peer_id")
                    Log.w(TAG, "Connection lost: $peerId")
                    // Handle disconnection
                }
            }
        } catch (e: Exception) {
            Log.e(TAG, "Failed to parse event", e)
        }
    }
    
    fun connectToPeer(address: String): Boolean {
        return if (nodePtr != 0L) {
            nativeConnectToPeer(nodePtr, address)
        } else {
            false
        }
    }
    
    fun sendMessage(peerId: String, message: String): Boolean {
        return if (nodePtr != 0L) {
            nativeSendMessage(nodePtr, peerId, message)
        } else {
            false
        }
    }
    
    fun onBleDataReceived(peerId: String, data: ByteArray) {
        if (nodePtr != 0L) {
            nativeOnBleData(nodePtr, peerId, data)
        }
    }
    
    override fun onDestroy() {
        super.onDestroy()
        eventPollingJob?.cancel()
        if (nodePtr != 0L) {
            nativeDestroyNode(nodePtr)
            nodePtr = 0
        }
    }
    
    override fun onBind(intent: Intent?): IBinder? = null
}
```

---

## Step 4: Build Configuration

### android/build.gradle

```gradle
android {
    defaultConfig {
        ndk {
            abiFilters 'arm64-v8a', 'armeabi-v7a', 'x86_64'
        }
    }
    
    sourceSets {
        main {
            jniLibs.srcDirs = ['src/main/jniLibs']
        }
    }
}

// Custom task to build Rust library
task buildRustLibrary(type: Exec) {
    workingDir '../'
    commandLine 'cargo', 'ndk',
        '-t', 'arm64-v8a',
        '-t', 'armeabi-v7a',
        '-t', 'x86_64',
        'build', '--release'
}

// Copy built libraries to JNI folder
task copyRustLibraries(type: Copy, dependsOn: buildRustLibrary) {
    from('../target/aarch64-linux-android/release/libbitcraps_android.so')
    into('src/main/jniLibs/arm64-v8a/')
    
    from('../target/armv7-linux-androideabi/release/libbitcraps_android.so')
    into('src/main/jniLibs/armeabi-v7a/')
    
    from('../target/x86_64-linux-android/release/libbitcraps_android.so')
    into('src/main/jniLibs/x86_64/')
}

preBuild.dependsOn copyRustLibraries
```

---

## Step 5: Memory Management

### Best Practices

1. **Use Box for heap allocation**
   ```rust
   Box::into_raw(Box::new(node)) as jlong  // Pass to Java
   Box::from_raw(ptr as *mut Node)         // Reclaim from Java
   ```

2. **Handle strings carefully**
   ```rust
   let string: String = env.get_string(&jstring)?.into();
   env.new_string(rust_string)?
   ```

3. **Clean up resources**
   ```rust
   #[no_mangle]
   pub extern "C" fn Java_..._destroy(env: JNIEnv, _: JClass, ptr: jlong) {
       if ptr != 0 {
           unsafe { Box::from_raw(ptr as *mut Resource); }
       }
   }
   ```

---

## Step 6: Error Handling

### Rust Side

```rust
use jni::errors::Error as JniError;

fn safe_operation(env: &mut JNIEnv) -> Result<(), JniError> {
    // Throw Java exception on error
    match risky_operation() {
        Ok(result) => Ok(result),
        Err(e) => {
            env.throw_new("java/lang/RuntimeException", &e.to_string())?;
            Err(JniError::from(e))
        }
    }
}
```

### Kotlin Side

```kotlin
try {
    nativeRiskyOperation()
} catch (e: RuntimeException) {
    Log.e(TAG, "Native operation failed", e)
    // Handle error
}
```

---

## Step 7: Threading Considerations

### Rust Side

1. **Use single Tokio runtime**
   ```rust
   static RUNTIME: Lazy<Runtime> = Lazy::new(|| {
       Runtime::new().unwrap()
   });
   ```

2. **Attach threads when needed**
   ```rust
   let vm = ndk_context::android_context().vm();
   let env = vm.attach_current_thread()?;
   ```

### Android Side

1. **Call native methods from background thread**
   ```kotlin
   scope.launch(Dispatchers.IO) {
       val result = nativeOperation()
   }
   ```

2. **Use callbacks for async operations**
   ```kotlin
   interface NativeCallback {
       fun onResult(result: String)
       fun onError(error: String)
   }
   ```

---

## Testing

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_node_creation() {
        let config = BitcrapsConfig::default();
        let node = MeshService::new(config);
        assert!(node.is_ok());
    }
}
```

### Integration Tests

```kotlin
@Test
fun testJniIntegration() {
    val nodePtr = nativeCreateNode("{}")
    assertNotEquals(0L, nodePtr)
    
    val started = nativeStartDiscovery(nodePtr)
    assertTrue(started)
    
    nativeDestroyNode(nodePtr)
}
```

---

## Troubleshooting

### Common Issues

1. **UnsatisfiedLinkError**
   - Ensure library name matches: `System.loadLibrary("bitcraps_android")`
   - Check ABI filters match device architecture
   - Verify .so files are in correct jniLibs folders

2. **SIGSEGV (Segmentation Fault)**
   - Check pointer validity before dereferencing
   - Ensure proper memory management
   - Use `Option<>` for nullable pointers

3. **Thread panics**
   - Catch panics at FFI boundary
   - Use `catch_unwind` for safety
   - Always attach thread to JVM when calling back

---

## Performance Optimization

1. **Minimize JNI calls**
   - Batch operations when possible
   - Use byte arrays for bulk data transfer

2. **Cache method IDs**
   ```rust
   static METHOD_ID: Lazy<JMethodID> = Lazy::new(|| {
       // Cache method ID on first use
   });
   ```

3. **Use direct ByteBuffers**
   ```kotlin
   val buffer = ByteBuffer.allocateDirect(1024)
   nativeProcessBuffer(buffer)
   ```

---

## Security Considerations

1. **Validate all inputs from Java**
2. **Use safe string handling**
3. **Implement proper bounds checking**
4. **Clear sensitive data after use**
5. **Enable ProGuard/R8 for release builds**

---

*Document Version: 1.0*
*Created: 2025-08-24*
*Status: Implementation Ready*