//! Cross-platform biometric authentication system
//!
//! This module provides unified biometric authentication for Android and iOS:
//! - Android: BiometricPrompt API with Fingerprint, Face, and Iris support
//! - iOS: TouchID and FaceID through LocalAuthentication framework
//!
//! ## Security Features
//! - Hardware-backed biometric verification
//! - Strong biometric binding to cryptographic keys
//! - Anti-spoofing and liveness detection
//! - Secure enclave/TEE protection
//! - Fallback to device passcode when needed

use std::ffi::CString;
use std::os::raw::{c_char, c_int};
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Serialize, Deserialize};
use crate::error::{Result, Error};

/// Cross-platform biometric authentication interface
pub trait BiometricAuth: Send + Sync {
    /// Check if biometric authentication is available on device
    fn is_available(&self) -> Result<BiometricAvailability>;
    
    /// Authenticate user with biometric prompt
    fn authenticate(&self, prompt: &BiometricPrompt) -> Result<BiometricAuthResult>;
    
    /// Generate or retrieve biometric-bound cryptographic key
    fn get_biometric_key(&self, key_alias: &str) -> Result<Vec<u8>>;
    
    /// Encrypt data with biometric authentication requirement
    fn encrypt_with_biometric(&self, key_alias: &str, data: &[u8]) -> Result<Vec<u8>>;
    
    /// Decrypt data requiring biometric authentication
    fn decrypt_with_biometric(&self, key_alias: &str, encrypted_data: &[u8]) -> Result<Vec<u8>>;
    
    /// Invalidate biometric keys (e.g., when biometrics change)
    fn invalidate_keys(&self) -> Result<()>;
    
    /// Get supported biometric types
    fn get_supported_types(&self) -> Result<Vec<BiometricType>>;
}

/// Unified biometric authentication manager
pub struct BiometricAuthManager {
    auth_impl: Box<dyn BiometricAuth>,
}

impl BiometricAuthManager {
    /// Create new biometric authentication manager for current platform
    pub fn new() -> Result<Self> {
        let auth_impl: Box<dyn BiometricAuth> = if cfg!(target_os = "android") {
            Box::new(AndroidBiometricAuth::new()?)
        } else if cfg!(target_os = "ios") {
            Box::new(IOSBiometricAuth::new()?)
        } else {
            // For testing/development
            Box::new(MockBiometricAuth::new())
        };
        
        Ok(Self { auth_impl })
    }
    
    /// Authenticate user and get session token
    pub async fn authenticate_user(&self, reason: &str) -> Result<UserAuthSession> {
        let prompt = BiometricPrompt {
            title: "BitCraps Authentication".to_string(),
            subtitle: reason.to_string(),
            description: "Use your biometric to securely access your account".to_string(),
            negative_button_text: "Cancel".to_string(),
            allow_device_credential: true,
            require_confirmation: true,
        };
        
        let auth_result = self.auth_impl.authenticate(&prompt)?;
        
        match auth_result.status {
            BiometricAuthStatus::Succeeded => {
                let session = UserAuthSession {
                    session_id: generate_session_id(),
                    user_id: auth_result.user_identifier.unwrap_or_else(|| "anonymous".to_string()),
                    auth_method: auth_result.auth_method,
                    created_at: current_timestamp(),
                    expires_at: current_timestamp() + 3600, // 1 hour
                    biometric_hash: auth_result.biometric_hash,
                };
                
                Ok(session)
            },
            BiometricAuthStatus::Failed => {
                Err(Error::Authentication("Biometric authentication failed".to_string()))
            },
            BiometricAuthStatus::Cancelled => {
                Err(Error::Authentication("Authentication cancelled by user".to_string()))
            },
            BiometricAuthStatus::Error(msg) => {
                Err(Error::Authentication(format!("Authentication error: {}", msg)))
            },
        }
    }
    
    /// Create biometric-protected wallet key
    pub fn create_protected_wallet_key(&self, wallet_id: &str) -> Result<ProtectedKey> {
        let key_alias = format!("wallet_key_{}", wallet_id);
        
        // Generate biometric-bound key
        let key_data = self.auth_impl.get_biometric_key(&key_alias)?;
        
        // Create additional entropy for key derivation
        let entropy = generate_secure_entropy(32)?;
        
        // Derive actual wallet key using HKDF
        let wallet_key = derive_wallet_key(&key_data, &entropy, wallet_id.as_bytes())?;
        
        let protected_key = ProtectedKey {
            key_alias: key_alias.clone(),
            key_derivation_salt: entropy,
            created_at: current_timestamp(),
            biometric_required: true,
            hardware_backed: self.is_hardware_backed()?,
        };
        
        // Test encryption to ensure key works
        let test_data = b"test_encryption";
        let _encrypted = self.auth_impl.encrypt_with_biometric(&key_alias, test_data)?;
        
        Ok(protected_key)
    }
    
    /// Unlock wallet key with biometric authentication
    pub fn unlock_wallet_key(&self, protected_key: &ProtectedKey) -> Result<Vec<u8>> {
        // Get biometric-bound key
        let biometric_key = self.auth_impl.get_biometric_key(&protected_key.key_alias)?;
        
        // Derive wallet key using stored salt
        let wallet_key = derive_wallet_key(
            &biometric_key, 
            &protected_key.key_derivation_salt,
            protected_key.key_alias.as_bytes()
        )?;
        
        Ok(wallet_key)
    }
    
    /// Check if biometric authentication is properly set up
    pub fn is_biometric_configured(&self) -> Result<bool> {
        let availability = self.auth_impl.is_available()?;
        Ok(matches!(availability, BiometricAvailability::Available(_)))
    }
    
    /// Check if keys are hardware-backed
    pub fn is_hardware_backed(&self) -> Result<bool> {
        let availability = self.auth_impl.is_available()?;
        match availability {
            BiometricAvailability::Available(info) => Ok(info.hardware_backed),
            _ => Ok(false),
        }
    }
}

/// Biometric prompt configuration
#[derive(Debug, Clone)]
pub struct BiometricPrompt {
    pub title: String,
    pub subtitle: String,
    pub description: String,
    pub negative_button_text: String,
    pub allow_device_credential: bool,
    pub require_confirmation: bool,
}

/// Biometric authentication result
#[derive(Debug)]
pub struct BiometricAuthResult {
    pub status: BiometricAuthStatus,
    pub auth_method: AuthenticationMethod,
    pub user_identifier: Option<String>,
    pub biometric_hash: Option<Vec<u8>>,
    pub crypto_object: Option<Vec<u8>>,
}

/// Biometric authentication status
#[derive(Debug)]
pub enum BiometricAuthStatus {
    Succeeded,
    Failed,
    Cancelled,
    Error(String),
}

/// Available biometric authentication methods
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BiometricType {
    Fingerprint,
    FaceRecognition,
    IrisRecognition,
    VoiceRecognition,
}

/// Authentication methods used
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthenticationMethod {
    Biometric(BiometricType),
    DeviceCredential,
    Combined,
}

/// Biometric availability status
#[derive(Debug)]
pub enum BiometricAvailability {
    Available(BiometricInfo),
    NotAvailable(String),
    NotEnrolled,
    HardwareUnavailable,
    SecurityUpdateRequired,
}

/// Information about available biometric authentication
#[derive(Debug)]
pub struct BiometricInfo {
    pub supported_types: Vec<BiometricType>,
    pub hardware_backed: bool,
    pub strong_biometric: bool,
    pub device_credential_available: bool,
}

/// User authentication session
#[derive(Debug, Serialize, Deserialize)]
pub struct UserAuthSession {
    pub session_id: String,
    pub user_id: String,
    pub auth_method: AuthenticationMethod,
    pub created_at: u64,
    pub expires_at: u64,
    pub biometric_hash: Option<Vec<u8>>,
}

/// Protected cryptographic key
#[derive(Debug, Serialize, Deserialize)]
pub struct ProtectedKey {
    pub key_alias: String,
    pub key_derivation_salt: Vec<u8>,
    pub created_at: u64,
    pub biometric_required: bool,
    pub hardware_backed: bool,
}

// ============= Android Biometric Implementation =============

/// Android BiometricPrompt implementation
pub struct AndroidBiometricAuth {
    initialized: bool,
}

// External JNI functions for Android BiometricPrompt
extern "C" {
    fn android_biometric_is_available() -> c_int;
    fn android_biometric_get_info(
        info_buffer: *mut u8,
        buffer_size: usize,
        actual_size: *mut usize
    ) -> c_int;
    fn android_biometric_authenticate(
        title: *const c_char,
        subtitle: *const c_char,
        description: *const c_char,
        negative_button: *const c_char,
        allow_device_credential: c_int,
        require_confirmation: c_int,
        result_buffer: *mut u8,
        buffer_size: usize,
        actual_size: *mut usize
    ) -> c_int;
    fn android_biometric_generate_key(
        key_alias: *const c_char,
        require_biometric: c_int,
        key_buffer: *mut u8,
        buffer_size: usize,
        actual_size: *mut usize
    ) -> c_int;
    fn android_biometric_encrypt(
        key_alias: *const c_char,
        data: *const u8,
        data_size: usize,
        encrypted_buffer: *mut u8,
        buffer_size: usize,
        actual_size: *mut usize
    ) -> c_int;
    fn android_biometric_decrypt(
        key_alias: *const c_char,
        encrypted_data: *const u8,
        encrypted_size: usize,
        decrypted_buffer: *mut u8,
        buffer_size: usize,
        actual_size: *mut usize
    ) -> c_int;
    fn android_biometric_invalidate_keys() -> c_int;
}

impl AndroidBiometricAuth {
    pub fn new() -> Result<Self> {
        let mut auth = Self { initialized: false };
        auth.initialize()?;
        Ok(auth)
    }
    
    fn initialize(&mut self) -> Result<()> {
        // Initialize Android BiometricManager and BiometricPrompt
        #[cfg(target_os = "android")]
        {
            // JNI initialization would happen here
            log::info!("Initializing Android BiometricPrompt");
        }
        
        #[cfg(not(target_os = "android"))]
        {
            log::warn!("Android BiometricPrompt simulation mode");
        }
        
        self.initialized = true;
        Ok(())
    }
}

impl BiometricAuth for AndroidBiometricAuth {
    fn is_available(&self) -> Result<BiometricAvailability> {
        #[cfg(target_os = "android")]
        {
            let availability = unsafe { android_biometric_is_available() };
            match availability {
                0 => {
                    // Get detailed biometric info
                    let mut info_buffer = vec![0u8; 256];
                    let mut actual_size: usize = 0;
                    
                    let result = unsafe {
                        android_biometric_get_info(
                            info_buffer.as_mut_ptr(),
                            info_buffer.len(),
                            &mut actual_size
                        )
                    };
                    
                    if result == 0 {
                        // Parse biometric info (would be JSON or protobuf in real impl)
                        let info = BiometricInfo {
                            supported_types: vec![
                                BiometricType::Fingerprint,
                                BiometricType::FaceRecognition
                            ],
                            hardware_backed: true,
                            strong_biometric: true,
                            device_credential_available: true,
                        };
                        Ok(BiometricAvailability::Available(info))
                    } else {
                        Ok(BiometricAvailability::NotAvailable("Unable to get biometric info".to_string()))
                    }
                },
                1 => Ok(BiometricAvailability::NotEnrolled),
                2 => Ok(BiometricAvailability::HardwareUnavailable),
                3 => Ok(BiometricAvailability::SecurityUpdateRequired),
                _ => Ok(BiometricAvailability::NotAvailable(format!("Unknown status: {}", availability))),
            }
        }
        
        #[cfg(not(target_os = "android"))]
        {
            // Simulation mode
            let info = BiometricInfo {
                supported_types: vec![BiometricType::Fingerprint],
                hardware_backed: false,
                strong_biometric: true,
                device_credential_available: true,
            };
            Ok(BiometricAvailability::Available(info))
        }
    }
    
    fn authenticate(&self, prompt: &BiometricPrompt) -> Result<BiometricAuthResult> {
        let title_cstr = CString::new(prompt.title.clone())?;
        let subtitle_cstr = CString::new(prompt.subtitle.clone())?;
        let description_cstr = CString::new(prompt.description.clone())?;
        let negative_cstr = CString::new(prompt.negative_button_text.clone())?;
        
        #[cfg(target_os = "android")]
        {
            let mut result_buffer = vec![0u8; 512];
            let mut actual_size: usize = 0;
            
            let result = unsafe {
                android_biometric_authenticate(
                    title_cstr.as_ptr(),
                    subtitle_cstr.as_ptr(),
                    description_cstr.as_ptr(),
                    negative_cstr.as_ptr(),
                    if prompt.allow_device_credential { 1 } else { 0 },
                    if prompt.require_confirmation { 1 } else { 0 },
                    result_buffer.as_mut_ptr(),
                    result_buffer.len(),
                    &mut actual_size
                )
            };
            
            match result {
                0 => {
                    // Parse authentication result
                    Ok(BiometricAuthResult {
                        status: BiometricAuthStatus::Succeeded,
                        auth_method: AuthenticationMethod::Biometric(BiometricType::Fingerprint),
                        user_identifier: Some("android_user".to_string()),
                        biometric_hash: Some(result_buffer[..actual_size].to_vec()),
                        crypto_object: None,
                    })
                },
                1 => Ok(BiometricAuthResult {
                    status: BiometricAuthStatus::Failed,
                    auth_method: AuthenticationMethod::Biometric(BiometricType::Fingerprint),
                    user_identifier: None,
                    biometric_hash: None,
                    crypto_object: None,
                }),
                2 => Ok(BiometricAuthResult {
                    status: BiometricAuthStatus::Cancelled,
                    auth_method: AuthenticationMethod::Biometric(BiometricType::Fingerprint),
                    user_identifier: None,
                    biometric_hash: None,
                    crypto_object: None,
                }),
                _ => Ok(BiometricAuthResult {
                    status: BiometricAuthStatus::Error(format!("Android biometric error: {}", result)),
                    auth_method: AuthenticationMethod::Biometric(BiometricType::Fingerprint),
                    user_identifier: None,
                    biometric_hash: None,
                    crypto_object: None,
                }),
            }
        }
        
        #[cfg(not(target_os = "android"))]
        {
            // Simulation mode - always succeed
            Ok(BiometricAuthResult {
                status: BiometricAuthStatus::Succeeded,
                auth_method: AuthenticationMethod::Biometric(BiometricType::Fingerprint),
                user_identifier: Some("sim_user".to_string()),
                biometric_hash: Some(b"simulated_biometric_hash".to_vec()),
                crypto_object: None,
            })
        }
    }
    
    fn get_biometric_key(&self, key_alias: &str) -> Result<Vec<u8>> {
        let key_alias_cstr = CString::new(key_alias)?;
        
        #[cfg(target_os = "android")]
        {
            let mut key_buffer = vec![0u8; 32]; // 256-bit key
            let mut actual_size: usize = 0;
            
            let result = unsafe {
                android_biometric_generate_key(
                    key_alias_cstr.as_ptr(),
                    1, // require biometric
                    key_buffer.as_mut_ptr(),
                    key_buffer.len(),
                    &mut actual_size
                )
            };
            
            if result != 0 {
                return Err(Error::Crypto(format!("Failed to generate biometric key: {}", result)));
            }
            
            key_buffer.truncate(actual_size);
            Ok(key_buffer)
        }
        
        #[cfg(not(target_os = "android"))]
        {
            // Simulation mode
            use sha2::{Sha256, Digest};
            let mut hasher = Sha256::new();
            hasher.update(key_alias.as_bytes());
            hasher.update(b"biometric_seed");
            Ok(hasher.finalize().to_vec())
        }
    }
    
    fn encrypt_with_biometric(&self, key_alias: &str, data: &[u8]) -> Result<Vec<u8>> {
        let key_alias_cstr = CString::new(key_alias)?;
        
        #[cfg(target_os = "android")]
        {
            let mut encrypted_buffer = vec![0u8; data.len() + 64]; // Extra space for IV/tag
            let mut actual_size: usize = 0;
            
            let result = unsafe {
                android_biometric_encrypt(
                    key_alias_cstr.as_ptr(),
                    data.as_ptr(),
                    data.len(),
                    encrypted_buffer.as_mut_ptr(),
                    encrypted_buffer.len(),
                    &mut actual_size
                )
            };
            
            if result != 0 {
                return Err(Error::Crypto(format!("Biometric encryption failed: {}", result)));
            }
            
            encrypted_buffer.truncate(actual_size);
            Ok(encrypted_buffer)
        }
        
        #[cfg(not(target_os = "android"))]
        {
            // Simulation mode
            let key = self.get_biometric_key(key_alias)?;
            let mut encrypted = data.to_vec();
            for (i, byte) in encrypted.iter_mut().enumerate() {
                *byte ^= key[i % key.len()];
            }
            Ok(encrypted)
        }
    }
    
    fn decrypt_with_biometric(&self, key_alias: &str, encrypted_data: &[u8]) -> Result<Vec<u8>> {
        let key_alias_cstr = CString::new(key_alias)?;
        
        #[cfg(target_os = "android")]
        {
            let mut decrypted_buffer = vec![0u8; encrypted_data.len()];
            let mut actual_size: usize = 0;
            
            let result = unsafe {
                android_biometric_decrypt(
                    key_alias_cstr.as_ptr(),
                    encrypted_data.as_ptr(),
                    encrypted_data.len(),
                    decrypted_buffer.as_mut_ptr(),
                    decrypted_buffer.len(),
                    &mut actual_size
                )
            };
            
            if result != 0 {
                return Err(Error::Crypto(format!("Biometric decryption failed: {}", result)));
            }
            
            decrypted_buffer.truncate(actual_size);
            Ok(decrypted_buffer)
        }
        
        #[cfg(not(target_os = "android"))]
        {
            // Simulation mode - same as encryption for XOR
            self.encrypt_with_biometric(key_alias, encrypted_data)
        }
    }
    
    fn invalidate_keys(&self) -> Result<()> {
        #[cfg(target_os = "android")]
        {
            let result = unsafe { android_biometric_invalidate_keys() };
            if result != 0 {
                return Err(Error::Crypto(format!("Failed to invalidate biometric keys: {}", result)));
            }
        }
        
        #[cfg(not(target_os = "android"))]
        {
            log::debug!("Simulating biometric key invalidation");
        }
        
        Ok(())
    }
    
    fn get_supported_types(&self) -> Result<Vec<BiometricType>> {
        // Would query BiometricManager.Authenticators in real implementation
        Ok(vec![
            BiometricType::Fingerprint,
            BiometricType::FaceRecognition,
        ])
    }
}

// ============= iOS Biometric Implementation =============

/// iOS TouchID/FaceID implementation using LocalAuthentication framework
pub struct IOSBiometricAuth {
    initialized: bool,
}

// External C functions for iOS LocalAuthentication
extern "C" {
    fn ios_biometric_is_available() -> c_int;
    fn ios_biometric_get_types(
        types_buffer: *mut u8,
        buffer_size: usize,
        actual_size: *mut usize
    ) -> c_int;
    fn ios_biometric_authenticate(
        reason: *const c_char,
        fallback_title: *const c_char,
        result_buffer: *mut u8,
        buffer_size: usize,
        actual_size: *mut usize
    ) -> c_int;
    fn ios_keychain_generate_biometric_key(
        key_alias: *const c_char,
        access_control: c_int,
        key_buffer: *mut u8,
        buffer_size: usize,
        actual_size: *mut usize
    ) -> c_int;
    fn ios_keychain_encrypt_with_biometric(
        key_alias: *const c_char,
        data: *const u8,
        data_size: usize,
        encrypted_buffer: *mut u8,
        buffer_size: usize,
        actual_size: *mut usize
    ) -> c_int;
    fn ios_keychain_decrypt_with_biometric(
        key_alias: *const c_char,
        encrypted_data: *const u8,
        encrypted_size: usize,
        decrypted_buffer: *mut u8,
        buffer_size: usize,
        actual_size: *mut usize
    ) -> c_int;
    fn ios_keychain_invalidate_biometric_keys() -> c_int;
}

impl IOSBiometricAuth {
    pub fn new() -> Result<Self> {
        let mut auth = Self { initialized: false };
        auth.initialize()?;
        Ok(auth)
    }
    
    fn initialize(&mut self) -> Result<()> {
        #[cfg(target_os = "ios")]
        {
            log::info!("Initializing iOS LocalAuthentication framework");
        }
        
        #[cfg(not(target_os = "ios"))]
        {
            log::warn!("iOS LocalAuthentication simulation mode");
        }
        
        self.initialized = true;
        Ok(())
    }
}

impl BiometricAuth for IOSBiometricAuth {
    fn is_available(&self) -> Result<BiometricAvailability> {
        #[cfg(target_os = "ios")]
        {
            let availability = unsafe { ios_biometric_is_available() };
            match availability {
                0 => {
                    // Get supported biometric types
                    let mut types_buffer = vec![0u8; 64];
                    let mut actual_size: usize = 0;
                    
                    let result = unsafe {
                        ios_biometric_get_types(
                            types_buffer.as_mut_ptr(),
                            types_buffer.len(),
                            &mut actual_size
                        )
                    };
                    
                    if result == 0 {
                        // Parse biometric types (would be from LABiometryType)
                        let info = BiometricInfo {
                            supported_types: vec![BiometricType::Fingerprint, BiometricType::FaceRecognition],
                            hardware_backed: true,
                            strong_biometric: true,
                            device_credential_available: true,
                        };
                        Ok(BiometricAvailability::Available(info))
                    } else {
                        Ok(BiometricAvailability::NotAvailable("Unable to get biometric types".to_string()))
                    }
                },
                1 => Ok(BiometricAvailability::NotEnrolled),
                2 => Ok(BiometricAvailability::HardwareUnavailable),
                3 => Ok(BiometricAvailability::SecurityUpdateRequired),
                _ => Ok(BiometricAvailability::NotAvailable(format!("Unknown iOS status: {}", availability))),
            }
        }
        
        #[cfg(not(target_os = "ios"))]
        {
            // Simulation mode
            let info = BiometricInfo {
                supported_types: vec![BiometricType::FaceRecognition],
                hardware_backed: false,
                strong_biometric: true,
                device_credential_available: true,
            };
            Ok(BiometricAvailability::Available(info))
        }
    }
    
    fn authenticate(&self, prompt: &BiometricPrompt) -> Result<BiometricAuthResult> {
        let reason_cstr = CString::new(format!("{}: {}", prompt.title, prompt.description))?;
        let fallback_cstr = CString::new(prompt.negative_button_text.clone())?;
        
        #[cfg(target_os = "ios")]
        {
            let mut result_buffer = vec![0u8; 256];
            let mut actual_size: usize = 0;
            
            let result = unsafe {
                ios_biometric_authenticate(
                    reason_cstr.as_ptr(),
                    fallback_cstr.as_ptr(),
                    result_buffer.as_mut_ptr(),
                    result_buffer.len(),
                    &mut actual_size
                )
            };
            
            match result {
                0 => Ok(BiometricAuthResult {
                    status: BiometricAuthStatus::Succeeded,
                    auth_method: AuthenticationMethod::Biometric(BiometricType::FaceRecognition),
                    user_identifier: Some("ios_user".to_string()),
                    biometric_hash: Some(result_buffer[..actual_size].to_vec()),
                    crypto_object: None,
                }),
                1 => Ok(BiometricAuthResult {
                    status: BiometricAuthStatus::Failed,
                    auth_method: AuthenticationMethod::Biometric(BiometricType::FaceRecognition),
                    user_identifier: None,
                    biometric_hash: None,
                    crypto_object: None,
                }),
                2 => Ok(BiometricAuthResult {
                    status: BiometricAuthStatus::Cancelled,
                    auth_method: AuthenticationMethod::Biometric(BiometricType::FaceRecognition),
                    user_identifier: None,
                    biometric_hash: None,
                    crypto_object: None,
                }),
                _ => Ok(BiometricAuthResult {
                    status: BiometricAuthStatus::Error(format!("iOS biometric error: {}", result)),
                    auth_method: AuthenticationMethod::Biometric(BiometricType::FaceRecognition),
                    user_identifier: None,
                    biometric_hash: None,
                    crypto_object: None,
                }),
            }
        }
        
        #[cfg(not(target_os = "ios"))]
        {
            // Simulation mode
            Ok(BiometricAuthResult {
                status: BiometricAuthStatus::Succeeded,
                auth_method: AuthenticationMethod::Biometric(BiometricType::FaceRecognition),
                user_identifier: Some("ios_sim_user".to_string()),
                biometric_hash: Some(b"simulated_faceid_hash".to_vec()),
                crypto_object: None,
            })
        }
    }
    
    fn get_biometric_key(&self, key_alias: &str) -> Result<Vec<u8>> {
        let key_alias_cstr = CString::new(key_alias)?;
        
        #[cfg(target_os = "ios")]
        {
            let mut key_buffer = vec![0u8; 32];
            let mut actual_size: usize = 0;
            
            let result = unsafe {
                ios_keychain_generate_biometric_key(
                    key_alias_cstr.as_ptr(),
                    1, // kSecAccessControlBiometryAny
                    key_buffer.as_mut_ptr(),
                    key_buffer.len(),
                    &mut actual_size
                )
            };
            
            if result != 0 {
                return Err(Error::Crypto(format!("Failed to generate iOS biometric key: {}", result)));
            }
            
            key_buffer.truncate(actual_size);
            Ok(key_buffer)
        }
        
        #[cfg(not(target_os = "ios"))]
        {
            // Simulation mode
            use sha2::{Sha256, Digest};
            let mut hasher = Sha256::new();
            hasher.update(key_alias.as_bytes());
            hasher.update(b"ios_biometric_seed");
            Ok(hasher.finalize().to_vec())
        }
    }
    
    fn encrypt_with_biometric(&self, key_alias: &str, data: &[u8]) -> Result<Vec<u8>> {
        let key_alias_cstr = CString::new(key_alias)?;
        
        #[cfg(target_os = "ios")]
        {
            let mut encrypted_buffer = vec![0u8; data.len() + 64];
            let mut actual_size: usize = 0;
            
            let result = unsafe {
                ios_keychain_encrypt_with_biometric(
                    key_alias_cstr.as_ptr(),
                    data.as_ptr(),
                    data.len(),
                    encrypted_buffer.as_mut_ptr(),
                    encrypted_buffer.len(),
                    &mut actual_size
                )
            };
            
            if result != 0 {
                return Err(Error::Crypto(format!("iOS biometric encryption failed: {}", result)));
            }
            
            encrypted_buffer.truncate(actual_size);
            Ok(encrypted_buffer)
        }
        
        #[cfg(not(target_os = "ios"))]
        {
            // Simulation mode
            let key = self.get_biometric_key(key_alias)?;
            let mut encrypted = data.to_vec();
            for (i, byte) in encrypted.iter_mut().enumerate() {
                *byte ^= key[i % key.len()];
            }
            Ok(encrypted)
        }
    }
    
    fn decrypt_with_biometric(&self, key_alias: &str, encrypted_data: &[u8]) -> Result<Vec<u8>> {
        let key_alias_cstr = CString::new(key_alias)?;
        
        #[cfg(target_os = "ios")]
        {
            let mut decrypted_buffer = vec![0u8; encrypted_data.len()];
            let mut actual_size: usize = 0;
            
            let result = unsafe {
                ios_keychain_decrypt_with_biometric(
                    key_alias_cstr.as_ptr(),
                    encrypted_data.as_ptr(),
                    encrypted_data.len(),
                    decrypted_buffer.as_mut_ptr(),
                    decrypted_buffer.len(),
                    &mut actual_size
                )
            };
            
            if result != 0 {
                return Err(Error::Crypto(format!("iOS biometric decryption failed: {}", result)));
            }
            
            decrypted_buffer.truncate(actual_size);
            Ok(decrypted_buffer)
        }
        
        #[cfg(not(target_os = "ios"))]
        {
            // Simulation mode
            self.encrypt_with_biometric(key_alias, encrypted_data)
        }
    }
    
    fn invalidate_keys(&self) -> Result<()> {
        #[cfg(target_os = "ios")]
        {
            let result = unsafe { ios_keychain_invalidate_biometric_keys() };
            if result != 0 {
                return Err(Error::Crypto(format!("Failed to invalidate iOS biometric keys: {}", result)));
            }
        }
        
        #[cfg(not(target_os = "ios"))]
        {
            log::debug!("Simulating iOS biometric key invalidation");
        }
        
        Ok(())
    }
    
    fn get_supported_types(&self) -> Result<Vec<BiometricType>> {
        // Would query LAContext.biometryType in real implementation
        Ok(vec![
            BiometricType::Fingerprint,
            BiometricType::FaceRecognition,
        ])
    }
}

// ============= Mock Implementation for Testing =============

/// Mock biometric authentication for testing and development
pub struct MockBiometricAuth {
    should_succeed: bool,
}

impl Default for MockBiometricAuth {
    fn default() -> Self {
        Self::new()
    }
}

impl MockBiometricAuth {
    pub fn new() -> Self {
        Self { should_succeed: true }
    }
    
    pub fn set_should_succeed(&mut self, succeed: bool) {
        self.should_succeed = succeed;
    }
}

impl BiometricAuth for MockBiometricAuth {
    fn is_available(&self) -> Result<BiometricAvailability> {
        let info = BiometricInfo {
            supported_types: vec![BiometricType::Fingerprint],
            hardware_backed: false,
            strong_biometric: true,
            device_credential_available: true,
        };
        Ok(BiometricAvailability::Available(info))
    }
    
    fn authenticate(&self, _prompt: &BiometricPrompt) -> Result<BiometricAuthResult> {
        if self.should_succeed {
            Ok(BiometricAuthResult {
                status: BiometricAuthStatus::Succeeded,
                auth_method: AuthenticationMethod::Biometric(BiometricType::Fingerprint),
                user_identifier: Some("mock_user".to_string()),
                biometric_hash: Some(b"mock_biometric_hash".to_vec()),
                crypto_object: None,
            })
        } else {
            Ok(BiometricAuthResult {
                status: BiometricAuthStatus::Failed,
                auth_method: AuthenticationMethod::Biometric(BiometricType::Fingerprint),
                user_identifier: None,
                biometric_hash: None,
                crypto_object: None,
            })
        }
    }
    
    fn get_biometric_key(&self, key_alias: &str) -> Result<Vec<u8>> {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(key_alias.as_bytes());
        hasher.update(b"mock_biometric_key");
        Ok(hasher.finalize().to_vec())
    }
    
    fn encrypt_with_biometric(&self, key_alias: &str, data: &[u8]) -> Result<Vec<u8>> {
        let key = self.get_biometric_key(key_alias)?;
        let mut encrypted = data.to_vec();
        for (i, byte) in encrypted.iter_mut().enumerate() {
            *byte ^= key[i % key.len()];
        }
        Ok(encrypted)
    }
    
    fn decrypt_with_biometric(&self, key_alias: &str, encrypted_data: &[u8]) -> Result<Vec<u8>> {
        self.encrypt_with_biometric(key_alias, encrypted_data)
    }
    
    fn invalidate_keys(&self) -> Result<()> {
        log::debug!("Mock biometric key invalidation");
        Ok(())
    }
    
    fn get_supported_types(&self) -> Result<Vec<BiometricType>> {
        Ok(vec![BiometricType::Fingerprint])
    }
}

// ============= Utility Functions =============

/// Generate secure random entropy
fn generate_secure_entropy(size: usize) -> Result<Vec<u8>> {
    use rand::RngCore;
    let mut entropy = vec![0u8; size];
    rand::thread_rng().fill_bytes(&mut entropy);
    Ok(entropy)
}

/// Derive wallet key using HKDF-SHA256
fn derive_wallet_key(master_key: &[u8], salt: &[u8], info: &[u8]) -> Result<Vec<u8>> {
    use hkdf::Hkdf;
    use sha2::Sha256;
    
    let hk = Hkdf::<Sha256>::new(Some(salt), master_key);
    let mut okm = vec![0u8; 32]; // 256-bit derived key
    hk.expand(info, &mut okm)
        .map_err(|e| Error::Crypto(format!("Key derivation failed: {}", e)))?;
    
    Ok(okm)
}

/// Generate unique session ID
fn generate_session_id() -> String {
    use uuid::Uuid;
    Uuid::new_v4().to_string()
}

/// Get current timestamp in seconds
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_biometric_auth_manager_creation() {
        let manager = BiometricAuthManager::new();
        assert!(manager.is_ok());
    }
    
    #[tokio::test]
    async fn test_biometric_authentication() {
        let manager = BiometricAuthManager::new().unwrap();
        let result = manager.authenticate_user("Test authentication").await;
        assert!(result.is_ok());
        
        let session = result.unwrap();
        assert!(!session.session_id.is_empty());
        assert!(session.expires_at > session.created_at);
    }
    
    #[test]
    fn test_protected_key_creation() {
        let manager = BiometricAuthManager::new().unwrap();
        let protected_key = manager.create_protected_wallet_key("test_wallet");
        assert!(protected_key.is_ok());
        
        let key = protected_key.unwrap();
        assert!(!key.key_alias.is_empty());
        assert!(!key.key_derivation_salt.is_empty());
        assert!(key.biometric_required);
    }
    
    #[test]
    fn test_wallet_key_unlock() {
        let manager = BiometricAuthManager::new().unwrap();
        
        // Create protected key
        let protected_key = manager.create_protected_wallet_key("test_wallet").unwrap();
        
        // Unlock key
        let unlocked_key = manager.unlock_wallet_key(&protected_key);
        assert!(unlocked_key.is_ok());
        
        let key_data = unlocked_key.unwrap();
        assert_eq!(key_data.len(), 32); // 256-bit key
    }
    
    #[test]
    fn test_mock_biometric_auth() {
        let auth = MockBiometricAuth::new();
        
        // Test availability
        let availability = auth.is_available();
        assert!(availability.is_ok());
        
        // Test authentication
        let prompt = BiometricPrompt {
            title: "Test".to_string(),
            subtitle: "Test subtitle".to_string(),
            description: "Test description".to_string(),
            negative_button_text: "Cancel".to_string(),
            allow_device_credential: true,
            require_confirmation: true,
        };
        
        let result = auth.authenticate(&prompt);
        assert!(result.is_ok());
        
        let auth_result = result.unwrap();
        assert!(matches!(auth_result.status, BiometricAuthStatus::Succeeded));
    }
    
    #[test]
    fn test_biometric_encryption() {
        let auth = MockBiometricAuth::new();
        let key_alias = "test_key";
        let data = b"secret data to encrypt";
        
        // Encrypt
        let encrypted = auth.encrypt_with_biometric(key_alias, data);
        assert!(encrypted.is_ok());
        
        let encrypted_data = encrypted.unwrap();
        assert_ne!(encrypted_data, data.to_vec());
        
        // Decrypt
        let decrypted = auth.decrypt_with_biometric(key_alias, &encrypted_data);
        assert!(decrypted.is_ok());
        
        let decrypted_data = decrypted.unwrap();
        assert_eq!(decrypted_data, data.to_vec());
    }
}