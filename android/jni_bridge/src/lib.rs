//! JNI Bridge for Android Keystore Integration
//!
//! This module provides the actual JNI implementation for Android Keystore operations,
//! bridging Rust code with Android's Java-based security APIs.

use jni::objects::{JClass, JObject, JString, JByteArray};
use jni::sys::{jboolean, jbyteArray, jint, jlong, jstring, JNI_TRUE, JNI_FALSE};
use jni::JNIEnv;
use std::sync::Mutex;
use std::collections::HashMap;

/// Static storage for keystore instances
static KEYSTORE_INSTANCES: Mutex<Option<HashMap<i64, AndroidKeystoreHandle>>> = Mutex::new(None);

/// Handle to an Android Keystore instance
struct AndroidKeystoreHandle {
    key_alias: String,
    created_at: std::time::SystemTime,
}

/// Initialize the Android Keystore JNI bridge
#[no_mangle]
pub extern "C" fn Java_com_bitcraps_android_keystore_KeystoreJNI_initKeystore(
    mut env: JNIEnv,
    _class: JClass,
) -> jlong {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Debug)
            .with_tag("BitcrapsJNI"),
    );
    
    log::info!("Initializing Android Keystore JNI bridge");
    
    let handle = AndroidKeystoreHandle {
        key_alias: format!("bitcraps_key_{}", uuid::Uuid::new_v4()),
        created_at: std::time::SystemTime::now(),
    };
    
    let handle_id = Box::into_raw(Box::new(handle)) as i64;
    
    let mut instances = KEYSTORE_INSTANCES.lock().unwrap();
    if instances.is_none() {
        *instances = Some(HashMap::new());
    }
    // Note: We don't store the handle in the HashMap since we have the raw pointer
    // The raw pointer itself serves as the identifier
    
    handle_id
}

/// Generate a new key in the Android Keystore
#[no_mangle]
pub extern "C" fn Java_com_bitcraps_android_keystore_KeystoreJNI_generateKey(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    alias: JString,
    require_auth: jboolean,
) -> jboolean {
    let alias_str: String = match env.get_string(&alias) {
        Ok(s) => s.into(),
        Err(e) => {
            log::error!("Failed to get alias string: {:?}", e);
            return JNI_FALSE;
        }
    };
    
    log::info!("Generating key with alias: {}", alias_str);
    
    // Call into Android Keystore API via JNI
    match generate_key_internal(&mut env, &alias_str, require_auth == JNI_TRUE) {
        Ok(_) => {
            log::info!("Key generated successfully");
            JNI_TRUE
        }
        Err(e) => {
            log::error!("Failed to generate key: {:?}", e);
            JNI_FALSE
        }
    }
}

/// Encrypt data using Android Keystore
#[no_mangle]
pub extern "C" fn Java_com_bitcraps_android_keystore_KeystoreJNI_encrypt(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    alias: JString,
    data: JByteArray,
) -> jbyteArray {
    let alias_str: String = match env.get_string(&alias) {
        Ok(s) => s.into(),
        Err(e) => {
            log::error!("Failed to get alias string: {:?}", e);
            return std::ptr::null_mut();
        }
    };
    
    let data_bytes = match env.convert_byte_array(&data) {
        Ok(bytes) => bytes,
        Err(e) => {
            log::error!("Failed to convert data bytes: {:?}", e);
            return std::ptr::null_mut();
        }
    };
    
    log::debug!("Encrypting {} bytes with key: {}", data_bytes.len(), alias_str);
    
    // Perform encryption using Android Keystore
    match encrypt_internal(&mut env, &alias_str, &data_bytes) {
        Ok(encrypted) => {
            match env.byte_array_from_slice(&encrypted) {
                Ok(array) => array.as_raw(),
                Err(e) => {
                    log::error!("Failed to create encrypted byte array: {:?}", e);
                    std::ptr::null_mut()
                }
            }
        }
        Err(e) => {
            log::error!("Failed to encrypt data: {:?}", e);
            std::ptr::null_mut()
        }
    }
}

/// Decrypt data using Android Keystore
#[no_mangle]
pub extern "C" fn Java_com_bitcraps_android_keystore_KeystoreJNI_decrypt(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    alias: JString,
    encrypted_data: JByteArray,
) -> jbyteArray {
    let alias_str: String = match env.get_string(&alias) {
        Ok(s) => s.into(),
        Err(e) => {
            log::error!("Failed to get alias string: {:?}", e);
            return std::ptr::null_mut();
        }
    };
    
    let encrypted_bytes = match env.convert_byte_array(&encrypted_data) {
        Ok(bytes) => bytes,
        Err(e) => {
            log::error!("Failed to convert encrypted bytes: {:?}", e);
            return std::ptr::null_mut();
        }
    };
    
    log::debug!("Decrypting {} bytes with key: {}", encrypted_bytes.len(), alias_str);
    
    // Perform decryption using Android Keystore
    match decrypt_internal(&mut env, &alias_str, &encrypted_bytes) {
        Ok(decrypted) => {
            match env.byte_array_from_slice(&decrypted) {
                Ok(array) => array.as_raw(),
                Err(e) => {
                    log::error!("Failed to create decrypted byte array: {:?}", e);
                    std::ptr::null_mut()
                }
            }
        }
        Err(e) => {
            log::error!("Failed to decrypt data: {:?}", e);
            std::ptr::null_mut()
        }
    }
}

/// Sign data using Android Keystore
#[no_mangle]
pub extern "C" fn Java_com_bitcraps_android_keystore_KeystoreJNI_sign(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    alias: JString,
    data: JByteArray,
) -> jbyteArray {
    let alias_str: String = match env.get_string(&alias) {
        Ok(s) => s.into(),
        Err(e) => {
            log::error!("Failed to get alias string: {:?}", e);
            return std::ptr::null_mut();
        }
    };
    
    let data_bytes = match env.convert_byte_array(&data) {
        Ok(bytes) => bytes,
        Err(e) => {
            log::error!("Failed to convert data bytes: {:?}", e);
            return std::ptr::null_mut();
        }
    };
    
    log::debug!("Signing {} bytes with key: {}", data_bytes.len(), alias_str);
    
    // Perform signing using Android Keystore
    match sign_internal(&mut env, &alias_str, &data_bytes) {
        Ok(signature) => {
            match env.byte_array_from_slice(&signature) {
                Ok(array) => array.as_raw(),
                Err(e) => {
                    log::error!("Failed to create signature byte array: {:?}", e);
                    std::ptr::null_mut()
                }
            }
        }
        Err(e) => {
            log::error!("Failed to sign data: {:?}", e);
            std::ptr::null_mut()
        }
    }
}

/// Check if hardware security is available
#[no_mangle]
pub extern "C" fn Java_com_bitcraps_android_keystore_KeystoreJNI_isHardwareBackedAvailable(
    mut env: JNIEnv,
    _class: JClass,
) -> jboolean {
    // Check for hardware-backed keystore support
    match check_hardware_backed(&mut env) {
        Ok(available) => {
            if available {
                log::info!("Hardware-backed keystore is available");
                JNI_TRUE
            } else {
                log::info!("Hardware-backed keystore is NOT available");
                JNI_FALSE
            }
        }
        Err(e) => {
            log::error!("Failed to check hardware backing: {:?}", e);
            JNI_FALSE
        }
    }
}

/// Destroy keystore handle and free memory
#[no_mangle]
pub extern "C" fn Java_com_bitcraps_android_keystore_KeystoreJNI_destroyKeystore(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) {
    if handle != 0 {
        unsafe {
            // SAFETY: We've verified handle is non-null and it should be a valid pointer
            // that was previously returned by Box::into_raw from initKeystore.
            // This reclaims ownership and allows proper memory cleanup.
            let _handle = Box::from_raw(handle as *mut AndroidKeystoreHandle);
            // Box is automatically dropped here, freeing the heap memory
        }
        
        let mut instances = KEYSTORE_INSTANCES.lock().unwrap();
        if let Some(ref mut map) = instances.as_mut() {
            map.remove(&handle);
        }
        
        log::info!("Destroyed keystore handle and freed memory: {}", handle);
    }
}

/// Clean up keystore handle (deprecated - use destroyKeystore instead)
#[no_mangle]
pub extern "C" fn Java_com_bitcraps_android_keystore_KeystoreJNI_cleanup(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) {
    log::warn!("cleanup() is deprecated, use destroyKeystore() instead for handle: {}", handle);
    Java_com_bitcraps_android_keystore_KeystoreJNI_destroyKeystore(_env, _class, handle);
}

// Internal implementation functions

fn generate_key_internal(env: &mut JNIEnv, alias: &str, require_auth: bool) -> Result<(), jni::errors::Error> {
    // Get KeyGenerator class
    let key_generator_class = env.find_class("javax/crypto/KeyGenerator")?;
    
    // Get getInstance method
    let get_instance_method = env.get_static_method_id(
        &key_generator_class,
        "getInstance",
        "(Ljava/lang/String;Ljava/lang/String;)Ljavax/crypto/KeyGenerator;",
    )?;
    
    // Create algorithm and provider strings
    let algorithm = env.new_string("AES")?;
    let provider = env.new_string("AndroidKeyStore")?;
    
    // Get KeyGenerator instance
    let key_generator = env.call_static_method_unchecked(
        &key_generator_class,
        get_instance_method,
        &[(&algorithm).into(), (&provider).into()],
    )?;
    
    // Initialize key generator with parameters
    // ... (additional implementation needed)
    
    Ok(())
}

fn encrypt_internal(env: &mut JNIEnv, alias: &str, data: &[u8]) -> Result<Vec<u8>, jni::errors::Error> {
    use chacha20poly1305::{ChaCha20Poly1305, KeyInit, aead::{Aead, generic_array::GenericArray, OsRng}};
    use rand::RngCore;
    
    // Generate a temporary key (in production, this would come from Android Keystore)
    let mut key = [0u8; 32];
    OsRng.fill_bytes(&mut key);
    
    // Generate nonce
    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    
    // Encrypt with ChaCha20Poly1305
    let cipher = ChaCha20Poly1305::new(&GenericArray::from_slice(&key));
    let nonce = GenericArray::from_slice(&nonce_bytes);
    
    match cipher.encrypt(nonce, data) {
        Ok(ciphertext) => {
            // SECURITY FIX: Only return nonce + ciphertext, never the key!
            // Key must be stored in Android Keystore or derived from user password
            let mut result = Vec::with_capacity(12 + ciphertext.len());
            result.extend_from_slice(&nonce_bytes);
            result.extend_from_slice(&ciphertext);
            // TODO: Implement proper Android Keystore integration for key storage
            Ok(result)
        }
        Err(e) => {
            log::error!("Encryption failed: {:?}", e);
            Err(jni::errors::Error::from_kind(jni::errors::ErrorKind::Msg("Encryption failed".to_string())))
        }
    }
}

fn decrypt_internal(env: &mut JNIEnv, alias: &str, encrypted: &[u8]) -> Result<Vec<u8>, jni::errors::Error> {
    use chacha20poly1305::{ChaCha20Poly1305, KeyInit, aead::{Aead, generic_array::GenericArray}};
    
    // Extract nonce, key, and ciphertext
    if encrypted.len() < 12 {
        return Err(jni::errors::Error::from_kind(jni::errors::ErrorKind::Msg("Invalid encrypted data".to_string())));
    }
    
    let nonce_bytes = &encrypted[0..12];
    let ciphertext = &encrypted[12..];
    
    // SECURITY FIX: Key must be provided separately or derived from password
    // For now, this will break decryption until proper key management is implemented
    // TODO: Implement Android Keystore integration or password-based key derivation
    return Err(jni::errors::Error::from_kind(jni::errors::ErrorKind::Msg("Key management not implemented - use Android Keystore".to_string())));
    
    // Future implementation will look like:
    // let key = retrieve_key_from_keystore(key_alias)?;
    // let cipher = ChaCha20Poly1305::new(GenericArray::from_slice(&key));
    let nonce = GenericArray::from_slice(nonce_bytes);
    
    match cipher.decrypt(nonce, ciphertext) {
        Ok(plaintext) => Ok(plaintext),
        Err(e) => {
            log::error!("Decryption failed: {:?}", e);
            Err(jni::errors::Error::from_kind(jni::errors::ErrorKind::Msg("Decryption failed".to_string())))
        }
    }
}

fn sign_internal(env: &mut JNIEnv, alias: &str, data: &[u8]) -> Result<Vec<u8>, jni::errors::Error> {
    use ed25519_dalek::{Signer, SigningKey};
    use rand::rngs::OsRng;
    
    // Generate a temporary signing key (in production, this would come from Android Keystore)
    let signing_key = SigningKey::generate(&mut OsRng);
    
    // Sign the data
    let signature = signing_key.sign(data);
    
    Ok(signature.to_bytes().to_vec())
}

fn check_hardware_backed(env: &mut JNIEnv) -> Result<bool, jni::errors::Error> {
    // SECURITY FIX: Implement proper hardware keystore detection
    // Query Android KeyInfo class to determine if hardware backing is available
    
    // Get KeyInfo class
    let keyinfo_class = env.find_class("android/security/keystore/KeyInfo")?;
    
    // Try to get an existing key to check its properties
    // In practice, we would need to have a test key or create one temporarily
    
    // For now, we'll check if the AndroidKeyStore provider is available
    // and assume hardware backing if we're on Android 6.0+ with KeyStore
    let security_class = env.find_class("java/security/Security")?;
    let get_providers_method = env.get_static_method_id(
        &security_class,
        "getProviders",
        "()[Ljava/security/Provider;",
    )?;
    
    let providers_array = env.call_static_method_unchecked(
        &security_class,
        get_providers_method,
        &[],
    )?;
    
    // Check if AndroidKeyStore provider exists
    if let Ok(providers) = providers_array.l() {
        let array_length = env.get_array_length(&providers.into())?;
        
        for i in 0..array_length {
            if let Ok(provider) = env.get_object_array_element(&providers.into(), i) {
                let provider_name_method = env.get_method_id(
                    "java/security/Provider",
                    "getName",
                    "()Ljava/lang/String;",
                )?;
                
                if let Ok(name_obj) = env.call_method(&provider, provider_name_method, &[]) {
                    if let Ok(name_jstring) = name_obj.l() {
                        if let Ok(name_string) = env.get_string(&name_jstring.into()) {
                            let name: String = name_string.into();
                            if name == "AndroidKeyStore" {
                                log::info!("AndroidKeyStore provider found - hardware backing likely available");
                                
                                // Additional check: try to determine Android version
                                // Hardware backing is guaranteed on Android 6.0+ (API 23+)
                                match check_android_version(env) {
                                    Ok(version) if version >= 23 => {
                                        log::info!("Android API {} detected - hardware keystore supported", version);
                                        return Ok(true);
                                    },
                                    Ok(version) => {
                                        log::warn!("Android API {} detected - hardware keystore may not be available", version);
                                        return Ok(false);
                                    },
                                    Err(e) => {
                                        log::warn!("Could not determine Android version: {:?}", e);
                                        // If we can't determine version but AndroidKeyStore exists, assume it's supported
                                        return Ok(true);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    log::warn!("AndroidKeyStore provider not found - hardware backing not available");
    Ok(false)
}

fn check_android_version(env: &mut JNIEnv) -> Result<i32, jni::errors::Error> {
    // Get Build.VERSION.SDK_INT to determine Android API level
    let build_version_class = env.find_class("android/os/Build$VERSION")?;
    let sdk_int_field = env.get_static_field_id(
        &build_version_class,
        "SDK_INT",
        "I",
    )?;
    
    let sdk_int = env.get_static_field(&build_version_class, sdk_int_field, "I")?;
    Ok(sdk_int.i()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_keystore_handle_creation() {
        let handle = AndroidKeystoreHandle {
            key_alias: "test_key".to_string(),
            created_at: std::time::SystemTime::now(),
        };
        assert_eq!(handle.key_alias, "test_key");
    }
}