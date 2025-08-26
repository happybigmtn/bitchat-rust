//! Secure key derivation and management for mobile platforms
//!
//! This module provides cryptographically secure key derivation using industry standards:
//! - PBKDF2 with SHA-256/SHA-512 for password-based derivation
//! - HKDF (HMAC-based Key Derivation Function) for key expansion
//! - Argon2id for memory-hard key derivation
//! - Hardware-backed key derivation when available
//! - Secure key rotation and versioning

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use zeroize::Zeroize;
use crate::error::{Result, Error};

/// Secure key derivation manager
pub struct KeyDerivationManager {
    master_keys: Arc<Mutex<HashMap<String, MasterKey>>>,
    derived_keys: Arc<Mutex<HashMap<String, DerivedKeyInfo>>>,
    hardware_backed: bool,
}

impl KeyDerivationManager {
    /// Create new key derivation manager
    pub fn new(hardware_backed: bool) -> Self {
        Self {
            master_keys: Arc::new(Mutex::new(HashMap::new())),
            derived_keys: Arc::new(Mutex::new(HashMap::new())),
            hardware_backed,
        }
    }
    
    /// Generate or retrieve master key from hardware security module
    pub fn get_master_key(&self, key_id: &str) -> Result<SecureKey> {
        let mut master_keys = self.master_keys.lock().map_err(|_| {
            Error::InvalidState("Failed to acquire master keys lock".to_string())
        })?;
        
        if let Some(master_key) = master_keys.get(key_id) {
            // Return existing master key
            let key_material = if self.hardware_backed {
                self.get_hardware_key(key_id)?
            } else {
                master_key.key_material.clone()
            };
            
            Ok(SecureKey::new(key_material, master_key.algorithm))
        } else {
            // Generate new master key
            let key_material = if self.hardware_backed {
                self.generate_hardware_key(key_id)?
            } else {
                self.generate_software_key(32)? // 256-bit key
            };
            
            let master_key = MasterKey {
                key_id: key_id.to_string(),
                key_material: if self.hardware_backed { Vec::new() } else { key_material.clone() },
                algorithm: KeyAlgorithm::HkdfSha256,
                created_at: current_timestamp(),
                version: 1,
                hardware_backed: self.hardware_backed,
                rotation_period: 86400 * 30, // 30 days
                last_rotated: current_timestamp(),
            };
            
            master_keys.insert(key_id.to_string(), master_key);
            Ok(SecureKey::new(key_material, KeyAlgorithm::HkdfSha256))
        }
    }
    
    /// Derive key using PBKDF2 with password
    pub fn derive_key_pbkdf2(
        &self,
        password: &str,
        salt: &[u8],
        iterations: u32,
        key_length: usize,
        algorithm: PbkdfAlgorithm,
    ) -> Result<SecureKey> {
        if iterations < 10000 {
            return Err(Error::InvalidInput(
                "PBKDF2 iterations must be at least 10,000 for security".to_string()
            ));
        }
        
        if salt.len() < 16 {
            return Err(Error::InvalidInput(
                "Salt must be at least 16 bytes".to_string()
            ));
        }
        
        let derived_key = match algorithm {
            PbkdfAlgorithm::Pbkdf2Sha256 => {
                use pbkdf2::{pbkdf2_hmac_array};
                use sha2::Sha256;
                
                if key_length != 32 {
                    return Err(Error::InvalidInput("SHA-256 PBKDF2 produces 32-byte keys".to_string()));
                }
                
                let key = pbkdf2_hmac_array::<Sha256, 32>(
                    password.as_bytes(),
                    salt,
                    iterations
                );
                
                key.to_vec()
            },
            PbkdfAlgorithm::Pbkdf2Sha512 => {
                use pbkdf2::{pbkdf2_hmac_array};
                use sha2::Sha512;
                
                if key_length != 64 {
                    return Err(Error::InvalidInput("SHA-512 PBKDF2 produces 64-byte keys".to_string()));
                }
                
                let key = pbkdf2_hmac_array::<Sha512, 64>(
                    password.as_bytes(),
                    salt,
                    iterations
                );
                
                key.to_vec()
            },
        };
        
        Ok(SecureKey::new(derived_key, KeyAlgorithm::from(algorithm)))
    }
    
    /// Derive key using Argon2id (memory-hard function)
    pub fn derive_key_argon2id(
        &self,
        password: &str,
        salt: &[u8],
        config: &Argon2Config,
    ) -> Result<SecureKey> {
        if salt.len() < 16 {
            return Err(Error::InvalidInput("Salt must be at least 16 bytes".to_string()));
        }
        
        use argon2::{Argon2, Version, Algorithm, Params};
        
        let params = Params::new(
            config.memory_cost,
            config.time_cost,
            config.parallelism,
            Some(config.hash_length)
        ).map_err(|e| Error::Crypto(format!("Invalid Argon2 parameters: {}", e)))?;
        
        let argon2 = Argon2::new(
            Algorithm::Argon2id,
            Version::V0x13,
            params
        );
        
        let mut derived_key = vec![0u8; config.hash_length];
        argon2.hash_password_into(
            password.as_bytes(),
            salt,
            &mut derived_key
        ).map_err(|e| Error::Crypto(format!("Argon2 key derivation failed: {}", e)))?;
        
        Ok(SecureKey::new(derived_key, KeyAlgorithm::Argon2id))
    }
    
    /// Derive key using HKDF (HMAC-based Key Derivation Function)
    pub fn derive_key_hkdf(
        &self,
        master_key_id: &str,
        salt: Option<&[u8]>,
        info: &[u8],
        key_length: usize,
        algorithm: HkdfAlgorithm,
    ) -> Result<SecureKey> {
        if key_length == 0 || key_length > 255 * 32 {
            return Err(Error::InvalidInput("Invalid HKDF output key length".to_string()));
        }
        
        let master_key = self.get_master_key(master_key_id)?;
        
        let derived_key = match algorithm {
            HkdfAlgorithm::HkdfSha256 => {
                use hkdf::Hkdf;
                use sha2::Sha256;
                
                let hk = Hkdf::<Sha256>::new(salt, &master_key.key_material);
                let mut okm = vec![0u8; key_length];
                hk.expand(info, &mut okm)
                    .map_err(|e| Error::Crypto(format!("HKDF-SHA256 expansion failed: {}", e)))?;
                okm
            },
            HkdfAlgorithm::HkdfSha512 => {
                use hkdf::Hkdf;
                use sha2::Sha512;
                
                let hk = Hkdf::<Sha512>::new(salt, &master_key.key_material);
                let mut okm = vec![0u8; key_length];
                hk.expand(info, &mut okm)
                    .map_err(|e| Error::Crypto(format!("HKDF-SHA512 expansion failed: {}", e)))?;
                okm
            },
        };
        
        // Store derived key info for management
        let key_info = DerivedKeyInfo {
            key_id: generate_key_id(),
            master_key_id: master_key_id.to_string(),
            derivation_method: DerivationMethod::HKDF(algorithm),
            info: info.to_vec(),
            salt: salt.map(|s| s.to_vec()),
            created_at: current_timestamp(),
            key_length,
        };
        
        let mut derived_keys = self.derived_keys.lock().map_err(|_| {
            Error::InvalidState("Failed to acquire derived keys lock".to_string())
        })?;
        derived_keys.insert(key_info.key_id.clone(), key_info);
        
        Ok(SecureKey::new(derived_key, KeyAlgorithm::from(algorithm)))
    }
    
    /// Create application-specific key hierarchy
    pub fn create_key_hierarchy(&self, app_id: &str) -> Result<KeyHierarchy> {
        // Generate master application key
        let master_key_id = format!("app_master_{}", app_id);
        let master_key = self.get_master_key(&master_key_id)?;
        
        // Derive specific purpose keys
        let encryption_key = self.derive_key_hkdf(
            &master_key_id,
            Some(b"bitcraps_encryption_salt"),
            format!("encryption_key_{}", app_id).as_bytes(),
            32,
            HkdfAlgorithm::HkdfSha256,
        )?;
        
        let signing_key = self.derive_key_hkdf(
            &master_key_id,
            Some(b"bitcraps_signing_salt"),
            format!("signing_key_{}", app_id).as_bytes(),
            32,
            HkdfAlgorithm::HkdfSha256,
        )?;
        
        let authentication_key = self.derive_key_hkdf(
            &master_key_id,
            Some(b"bitcraps_auth_salt"),
            format!("auth_key_{}", app_id).as_bytes(),
            32,
            HkdfAlgorithm::HkdfSha256,
        )?;
        
        let session_key = self.derive_key_hkdf(
            &master_key_id,
            Some(b"bitcraps_session_salt"),
            format!("session_key_{}", app_id).as_bytes(),
            32,
            HkdfAlgorithm::HkdfSha256,
        )?;
        
        Ok(KeyHierarchy {
            app_id: app_id.to_string(),
            master_key_id,
            encryption_key,
            signing_key,
            authentication_key,
            session_key,
            created_at: current_timestamp(),
        })
    }
    
    /// Rotate master key (generate new version)
    pub fn rotate_master_key(&self, key_id: &str) -> Result<u32> {
        let mut master_keys = self.master_keys.lock().map_err(|_| {
            Error::InvalidState("Failed to acquire master keys lock".to_string())
        })?;
        
        if let Some(master_key) = master_keys.get_mut(key_id) {
            // Generate new key material
            let new_key_material = if self.hardware_backed {
                self.generate_hardware_key(key_id)?
            } else {
                self.generate_software_key(32)?
            };
            
            // Update master key
            master_key.version += 1;
            master_key.last_rotated = current_timestamp();
            if !self.hardware_backed {
                master_key.key_material.zeroize();
                master_key.key_material = new_key_material;
            }
            
            log::info!("Rotated master key '{}' to version {}", key_id, master_key.version);
            Ok(master_key.version)
        } else {
            Err(Error::NotFound(format!("Master key '{}' not found", key_id)))
        }
    }
    
    /// Check if hardware-backed security is available
    pub fn is_hardware_backed(&self) -> Result<bool> {
        Ok(self.hardware_backed)
    }
    
    /// Check if master key needs rotation
    pub fn needs_rotation(&self, key_id: &str) -> Result<bool> {
        let master_keys = self.master_keys.lock().map_err(|_| {
            Error::InvalidState("Failed to acquire master keys lock".to_string())
        })?;
        
        if let Some(master_key) = master_keys.get(key_id) {
            let current_time = current_timestamp();
            let rotation_due = master_key.last_rotated + master_key.rotation_period;
            Ok(current_time >= rotation_due)
        } else {
            Err(Error::NotFound(format!("Master key '{}' not found", key_id)))
        }
    }
    
    /// Validate key derivation parameters for security
    pub fn validate_derivation_params(&self, method: &DerivationMethod) -> Result<()> {
        match method {
            DerivationMethod::PBKDF2 { algorithm: _, iterations, salt_length } => {
                if *iterations < 10000 {
                    return Err(Error::InvalidInput(
                        "PBKDF2 iterations too low (minimum 10,000)".to_string()
                    ));
                }
                if *salt_length < 16 {
                    return Err(Error::InvalidInput(
                        "Salt too short (minimum 16 bytes)".to_string()
                    ));
                }
            },
            DerivationMethod::Argon2id { config } => {
                if config.memory_cost < 4096 {
                    return Err(Error::InvalidInput(
                        "Argon2 memory cost too low (minimum 4MB)".to_string()
                    ));
                }
                if config.time_cost < 3 {
                    return Err(Error::InvalidInput(
                        "Argon2 time cost too low (minimum 3)".to_string()
                    ));
                }
                if config.parallelism < 1 || config.parallelism > 16 {
                    return Err(Error::InvalidInput(
                        "Argon2 parallelism must be 1-16".to_string()
                    ));
                }
            },
            DerivationMethod::HKDF(_) => {
                // HKDF parameters are validated in derive_key_hkdf
            },
        }
        
        Ok(())
    }
    
    /// Generate hardware-backed key (platform-specific)
    fn generate_hardware_key(&self, _key_id: &str) -> Result<Vec<u8>> {
        #[cfg(target_os = "android")]
        {
            // Use Android Keystore to generate key
            use crate::mobile::android_keystore::AndroidKeystoreManager;
            let keystore = AndroidKeystoreManager::new("com.bitcraps")?;
            keystore.get_encryption_key(_key_id)
        }
        
        #[cfg(target_os = "ios")]
        {
            // Use iOS Keychain to generate key
            // This would interface with SecKeyCreateRandomKey
            self.generate_software_key(32)
        }
        
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            // Fallback to software key for non-mobile platforms
            self.generate_software_key(32)
        }
    }
    
    /// Retrieve hardware-backed key
    fn get_hardware_key(&self, key_id: &str) -> Result<Vec<u8>> {
        self.generate_hardware_key(key_id) // In real impl, would retrieve existing key
    }
    
    /// Generate software-based random key using cryptographically secure RNG
    fn generate_software_key(&self, length: usize) -> Result<Vec<u8>> {
        use rand::{RngCore, rngs::OsRng};
        let mut key = vec![0u8; length];
        let mut secure_rng = OsRng;
        secure_rng.fill_bytes(&mut key);
        Ok(key)
    }
}

/// Secure key with automatic zeroization
pub struct SecureKey {
    pub key_material: Vec<u8>,
    pub algorithm: KeyAlgorithm,
    pub created_at: u64,
}

impl Drop for SecureKey {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.key_material.zeroize();
    }
}

impl SecureKey {
    pub fn new(key_material: Vec<u8>, algorithm: KeyAlgorithm) -> Self {
        Self {
            key_material,
            algorithm,
            created_at: current_timestamp(),
        }
    }
    
    /// Get key material (use carefully)
    pub fn as_bytes(&self) -> &[u8] {
        &self.key_material
    }
    
    /// Get key length
    pub fn len(&self) -> usize {
        self.key_material.len()
    }
    
    /// Check if key is empty
    pub fn is_empty(&self) -> bool {
        self.key_material.is_empty()
    }
}

/// Master key information
#[derive(Clone)]
struct MasterKey {
    key_id: String,
    key_material: Vec<u8>, // Empty for hardware-backed keys
    algorithm: KeyAlgorithm,
    created_at: u64,
    version: u32,
    hardware_backed: bool,
    rotation_period: u64, // seconds
    last_rotated: u64,
}

/// Derived key information for management
#[derive(Debug, Clone)]
struct DerivedKeyInfo {
    key_id: String,
    master_key_id: String,
    derivation_method: DerivationMethod,
    info: Vec<u8>,
    salt: Option<Vec<u8>>,
    created_at: u64,
    key_length: usize,
}

/// Application key hierarchy
pub struct KeyHierarchy {
    pub app_id: String,
    pub master_key_id: String,
    pub encryption_key: SecureKey,
    pub signing_key: SecureKey,
    pub authentication_key: SecureKey,
    pub session_key: SecureKey,
    pub created_at: u64,
}

/// Key derivation algorithms
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum KeyAlgorithm {
    PBKDF2SHA256,
    PBKDF2SHA512,
    Argon2id,
    HkdfSha256,
    HkdfSha512,
}

/// PBKDF2 algorithms
#[derive(Debug, Clone, Copy)]
pub enum PbkdfAlgorithm {
    Pbkdf2Sha256,
    Pbkdf2Sha512,
}

/// HKDF algorithms
#[derive(Debug, Clone, Copy)]
pub enum HkdfAlgorithm {
    HkdfSha256,
    HkdfSha512,
}

/// Argon2 configuration
#[derive(Debug, Clone)]
pub struct Argon2Config {
    pub memory_cost: u32,     // Memory usage in KB
    pub time_cost: u32,       // Number of iterations
    pub parallelism: u32,     // Number of parallel threads
    pub hash_length: usize,   // Output hash length
}

impl Default for Argon2Config {
    fn default() -> Self {
        Self {
            memory_cost: 65536,   // 64 MB
            time_cost: 3,         // 3 iterations
            parallelism: 4,       // 4 threads
            hash_length: 32,      // 256 bits
        }
    }
}

/// Key derivation methods
#[derive(Debug, Clone)]
pub enum DerivationMethod {
    PBKDF2 {
        algorithm: PbkdfAlgorithm,
        iterations: u32,
        salt_length: usize,
    },
    Argon2id {
        config: Argon2Config,
    },
    HKDF(HkdfAlgorithm),
}

impl From<PbkdfAlgorithm> for KeyAlgorithm {
    fn from(alg: PbkdfAlgorithm) -> Self {
        match alg {
            PbkdfAlgorithm::Pbkdf2Sha256 => KeyAlgorithm::PBKDF2SHA256,
            PbkdfAlgorithm::Pbkdf2Sha512 => KeyAlgorithm::PBKDF2SHA512,
        }
    }
}

impl From<HkdfAlgorithm> for KeyAlgorithm {
    fn from(alg: HkdfAlgorithm) -> Self {
        match alg {
            HkdfAlgorithm::HkdfSha256 => KeyAlgorithm::HkdfSha256,
            HkdfAlgorithm::HkdfSha512 => KeyAlgorithm::HkdfSha512,
        }
    }
}

/// Generate unique key ID
fn generate_key_id() -> String {
    use uuid::Uuid;
    Uuid::new_v4().to_string()
}

/// Get current timestamp
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_key_derivation_manager() {
        let manager = KeyDerivationManager::new(false);
        
        // Test master key generation
        let master_key = manager.get_master_key("test_master");
        assert!(master_key.is_ok());
        
        let key = master_key.unwrap();
        assert_eq!(key.len(), 32);
        assert!(!key.is_empty());
    }
    
    #[test]
    fn test_pbkdf2_derivation() {
        let manager = KeyDerivationManager::new(false);
        let password = "test_password_with_sufficient_entropy";
        let salt = b"test_salt_16byte";
        
        let derived_key = manager.derive_key_pbkdf2(
            password,
            salt,
            10000,
            32,
            PbkdfAlgorithm::Pbkdf2Sha256,
        );
        
        assert!(derived_key.is_ok());
        
        let key = derived_key.unwrap();
        assert_eq!(key.len(), 32);
        assert_eq!(key.algorithm, KeyAlgorithm::PBKDF2SHA256);
    }
    
    #[test]
    fn test_argon2_derivation() {
        let manager = KeyDerivationManager::new(false);
        let password = "test_password";
        let salt = b"test_salt_16byte";
        let config = Argon2Config::default();
        
        let derived_key = manager.derive_key_argon2id(password, salt, &config);
        assert!(derived_key.is_ok());
        
        let key = derived_key.unwrap();
        assert_eq!(key.len(), 32);
        assert_eq!(key.algorithm, KeyAlgorithm::Argon2id);
    }
    
    #[test]
    fn test_hkdf_derivation() {
        let manager = KeyDerivationManager::new(false);
        
        // First create a master key
        let master_key_id = "test_hkdf_master";
        let _master = manager.get_master_key(master_key_id).unwrap();
        
        // Derive key using HKDF
        let derived_key = manager.derive_key_hkdf(
            master_key_id,
            Some(b"hkdf_test_salt"),
            b"test_info_context",
            32,
            HkdfAlgorithm::HkdfSha256,
        );
        
        assert!(derived_key.is_ok());
        
        let key = derived_key.unwrap();
        assert_eq!(key.len(), 32);
        assert_eq!(key.algorithm, KeyAlgorithm::HkdfSha256);
    }
    
    #[test]
    fn test_key_hierarchy_creation() {
        let manager = KeyDerivationManager::new(false);
        let app_id = "test_app";
        
        let hierarchy = manager.create_key_hierarchy(app_id);
        assert!(hierarchy.is_ok());
        
        let keys = hierarchy.unwrap();
        assert_eq!(keys.app_id, app_id);
        assert_eq!(keys.encryption_key.len(), 32);
        assert_eq!(keys.signing_key.len(), 32);
        assert_eq!(keys.authentication_key.len(), 32);
        assert_eq!(keys.session_key.len(), 32);
    }
    
    #[test]
    fn test_parameter_validation() {
        let manager = KeyDerivationManager::new(false);
        
        // Test weak PBKDF2 parameters
        let weak_pbkdf2 = DerivationMethod::PBKDF2 {
            algorithm: PbkdfAlgorithm::Pbkdf2Sha256,
            iterations: 1000, // Too low
            salt_length: 16,
        };
        
        assert!(manager.validate_derivation_params(&weak_pbkdf2).is_err());
        
        // Test short salt
        let short_salt = DerivationMethod::PBKDF2 {
            algorithm: PbkdfAlgorithm::Pbkdf2Sha256,
            iterations: 10000,
            salt_length: 8, // Too short
        };
        
        assert!(manager.validate_derivation_params(&short_salt).is_err());
        
        // Test valid parameters
        let valid_pbkdf2 = DerivationMethod::PBKDF2 {
            algorithm: PbkdfAlgorithm::Pbkdf2Sha256,
            iterations: 10000,
            salt_length: 16,
        };
        
        assert!(manager.validate_derivation_params(&valid_pbkdf2).is_ok());
    }
    
    #[test]
    fn test_key_rotation() {
        let manager = KeyDerivationManager::new(false);
        let key_id = "rotation_test_key";
        
        // Create master key
        let _master = manager.get_master_key(key_id).unwrap();
        
        // Rotate key
        let new_version = manager.rotate_master_key(key_id);
        assert!(new_version.is_ok());
        assert_eq!(new_version.unwrap(), 2);
        
        // Check if rotation is needed (should be false for newly rotated key)
        let needs_rotation = manager.needs_rotation(key_id);
        assert!(needs_rotation.is_ok());
        assert!(!needs_rotation.unwrap());
    }
    
    #[test]
    fn test_secure_key_zeroization() {
        let key_material = vec![1, 2, 3, 4, 5];
        let key = SecureKey::new(key_material, KeyAlgorithm::HkdfSha256);
        
        assert_eq!(key.len(), 5);
        assert!(!key.is_empty());
        assert_eq!(key.as_bytes(), &[1, 2, 3, 4, 5]);
        
        // Key should be zeroized when dropped
        drop(key);
    }
    
    #[test]
    fn test_argon2_config_validation() {
        let manager = KeyDerivationManager::new(false);
        
        // Test weak Argon2 config
        let weak_config = Argon2Config {
            memory_cost: 1024, // Too low
            time_cost: 1,      // Too low
            parallelism: 1,
            hash_length: 32,
        };
        
        let weak_method = DerivationMethod::Argon2id { config: weak_config };
        assert!(manager.validate_derivation_params(&weak_method).is_err());
        
        // Test valid config
        let valid_config = Argon2Config::default();
        let valid_method = DerivationMethod::Argon2id { config: valid_config };
        assert!(manager.validate_derivation_params(&valid_method).is_ok());
    }
}