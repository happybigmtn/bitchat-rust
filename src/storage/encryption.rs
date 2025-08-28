//! Encryption at rest implementation for BitCraps storage
//!
//! This module provides AES-256-GCM encryption for data at rest with secure key management.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs;
use std::io::{Read, Write};
use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Nonce, Key
};
use serde::{Serialize, Deserialize};
use rand::{RngCore, rngs::OsRng as RandOsRng};
use blake3::Hasher;
use zeroize::{Zeroize, ZeroizeOnDrop};
use crate::error::{Error, Result};

/// Key derivation parameters
const KEY_DERIVATION_ITERATIONS: u32 = 100_000;
const SALT_SIZE: usize = 32;
const NONCE_SIZE: usize = 12;

/// Master key for encryption (zeroized on drop for security)
#[derive(Clone, ZeroizeOnDrop)]
struct MasterKey {
    key: [u8; 32],
}

impl MasterKey {
    fn new(key: [u8; 32]) -> Self {
        Self { key }
    }

    fn as_bytes(&self) -> &[u8; 32] {
        &self.key
    }
}

/// Key derivation parameters stored with encrypted data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyDerivationParams {
    pub salt: [u8; SALT_SIZE],
    pub iterations: u32,
}

/// Encrypted data container
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedData {
    pub data: Vec<u8>,
    pub nonce: [u8; NONCE_SIZE],
    pub key_id: String,
}

/// Key management interface
pub trait KeyManager: Send + Sync {
    /// Get encryption key by ID
    fn get_key(&self, key_id: &str) -> Result<MasterKey>;
    
    /// Generate new encryption key
    fn generate_key(&mut self) -> Result<String>;
    
    /// Derive key from password
    fn derive_key_from_password(&mut self, password: &str, params: KeyDerivationParams) -> Result<String>;
    
    /// List available key IDs
    fn list_keys(&self) -> Vec<String>;
    
    /// Rotate to new key
    fn rotate_key(&mut self) -> Result<String>;
}

/// File-based key manager (for development/testing)
pub struct FileKeyManager {
    key_dir: PathBuf,
    keys: HashMap<String, MasterKey>,
    current_key_id: Option<String>,
}

impl FileKeyManager {
    pub fn new<P: AsRef<Path>>(key_dir: P) -> Result<Self> {
        let key_dir = key_dir.as_ref().to_path_buf();
        
        // Create key directory if it doesn't exist
        fs::create_dir_all(&key_dir)
            .map_err(|e| Error::Storage(format!("Failed to create key directory: {}", e)))?;

        let mut manager = Self {
            key_dir,
            keys: HashMap::new(),
            current_key_id: None,
        };

        // Load existing keys
        manager.load_keys()?;

        // Generate initial key if none exist
        if manager.keys.is_empty() {
            let key_id = manager.generate_key()?;
            manager.current_key_id = Some(key_id);
        }

        Ok(manager)
    }

    fn load_keys(&mut self) -> Result<()> {
        let entries = fs::read_dir(&self.key_dir)
            .map_err(|e| Error::Storage(format!("Failed to read key directory: {}", e)))?;

        for entry in entries {
            let entry = entry
                .map_err(|e| Error::Storage(format!("Failed to read directory entry: {}", e)))?;
            
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("key") {
                if let Some(key_id) = path.file_stem().and_then(|s| s.to_str()) {
                    let key = self.load_key_from_file(&path)?;
                    self.keys.insert(key_id.to_string(), key);
                    
                    // Set as current if this is the first key or if it's marked as current
                    if self.current_key_id.is_none() || key_id.ends_with("_current") {
                        self.current_key_id = Some(key_id.to_string());
                    }
                }
            }
        }

        Ok(())
    }

    fn load_key_from_file<P: AsRef<Path>>(&self, path: P) -> Result<MasterKey> {
        let mut file = fs::File::open(path)
            .map_err(|e| Error::Storage(format!("Failed to open key file: {}", e)))?;

        let mut key_bytes = [0u8; 32];
        file.read_exact(&mut key_bytes)
            .map_err(|e| Error::Storage(format!("Failed to read key file: {}", e)))?;

        Ok(MasterKey::new(key_bytes))
    }

    fn save_key_to_file(&self, key_id: &str, key: &MasterKey) -> Result<()> {
        let key_path = self.key_dir.join(format!("{}.key", key_id));
        let mut file = fs::File::create(&key_path)
            .map_err(|e| Error::Storage(format!("Failed to create key file: {}", e)))?;

        file.write_all(key.as_bytes())
            .map_err(|e| Error::Storage(format!("Failed to write key file: {}", e)))?;

        file.sync_all()
            .map_err(|e| Error::Storage(format!("Failed to sync key file: {}", e)))?;

        Ok(())
    }
}

impl KeyManager for FileKeyManager {
    fn get_key(&self, key_id: &str) -> Result<MasterKey> {
        self.keys.get(key_id)
            .cloned()
            .ok_or_else(|| Error::Storage(format!("Key not found: {}", key_id)))
    }

    fn generate_key(&mut self) -> Result<String> {
        let mut key_bytes = [0u8; 32];
        RandOsRng.fill_bytes(&mut key_bytes);
        
        let key = MasterKey::new(key_bytes);
        let key_id = format!("key_{}", hex::encode(&key_bytes[..8])); // Use first 8 bytes as ID
        
        self.save_key_to_file(&key_id, &key)?;
        self.keys.insert(key_id.clone(), key);
        
        Ok(key_id)
    }

    fn derive_key_from_password(&mut self, password: &str, params: KeyDerivationParams) -> Result<String> {
        let mut key = [0u8; 32];
        
        // Use PBKDF2 for key derivation
        pbkdf2::pbkdf2::<hmac::Hmac<sha2::Sha256>>(
            password.as_bytes(),
            &params.salt,
            params.iterations,
            &mut key
        ).map_err(|_| Error::Storage("Key derivation failed".to_string()))?;

        let master_key = MasterKey::new(key);
        let key_id = format!("derived_{}", hex::encode(&params.salt[..8]));
        
        self.save_key_to_file(&key_id, &master_key)?;
        self.keys.insert(key_id.clone(), master_key);
        
        Ok(key_id)
    }

    fn list_keys(&self) -> Vec<String> {
        self.keys.keys().cloned().collect()
    }

    fn rotate_key(&mut self) -> Result<String> {
        let new_key_id = self.generate_key()?;
        self.current_key_id = Some(new_key_id.clone());
        Ok(new_key_id)
    }
}

/// Production key manager using hardware security modules or key vaults
#[cfg(feature = "hsm")]
pub struct HsmKeyManager {
    // HSM connection details would go here
    current_key_id: Option<String>,
}

#[cfg(feature = "hsm")]
impl KeyManager for HsmKeyManager {
    fn get_key(&self, _key_id: &str) -> Result<MasterKey> {
        // HSM key retrieval implementation
        unimplemented!("HSM key manager not implemented")
    }

    fn generate_key(&mut self) -> Result<String> {
        // HSM key generation implementation
        unimplemented!("HSM key manager not implemented")
    }

    fn derive_key_from_password(&mut self, _password: &str, _params: KeyDerivationParams) -> Result<String> {
        // HSM key derivation implementation
        unimplemented!("HSM key manager not implemented")
    }

    fn list_keys(&self) -> Vec<String> {
        unimplemented!("HSM key manager not implemented")
    }

    fn rotate_key(&mut self) -> Result<String> {
        unimplemented!("HSM key manager not implemented")
    }
}

/// Encryption engine using AES-256-GCM
pub struct EncryptionEngine {
    key_manager: Box<dyn KeyManager>,
    current_key_id: Option<String>,
}

impl EncryptionEngine {
    pub fn new(key_manager: Box<dyn KeyManager>) -> Self {
        Self {
            key_manager,
            current_key_id: None,
        }
    }

    /// Encrypt data using current key
    pub fn encrypt(&mut self, data: &[u8]) -> Result<EncryptedData> {
        // Get or generate current key
        let key_id = if let Some(ref id) = self.current_key_id {
            id.clone()
        } else {
            let id = self.key_manager.generate_key()?;
            self.current_key_id = Some(id.clone());
            id
        };

        self.encrypt_with_key(data, &key_id)
    }

    /// Encrypt data with specific key
    pub fn encrypt_with_key(&self, data: &[u8], key_id: &str) -> Result<EncryptedData> {
        let master_key = self.key_manager.get_key(key_id)?;
        let aes_key = Key::<Aes256Gcm>::from_slice(master_key.as_bytes());
        let cipher = Aes256Gcm::new(aes_key);

        // Generate random nonce
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let nonce_bytes: [u8; NONCE_SIZE] = nonce.as_slice().try_into()
            .map_err(|_| Error::Storage("Invalid nonce size".to_string()))?;

        // Encrypt data
        let ciphertext = cipher.encrypt(&nonce, data)
            .map_err(|_| Error::Storage("Encryption failed".to_string()))?;

        Ok(EncryptedData {
            data: ciphertext,
            nonce: nonce_bytes,
            key_id: key_id.to_string(),
        })
    }

    /// Decrypt data
    pub fn decrypt(&self, encrypted: &EncryptedData) -> Result<Vec<u8>> {
        let master_key = self.key_manager.get_key(&encrypted.key_id)?;
        let aes_key = Key::<Aes256Gcm>::from_slice(master_key.as_bytes());
        let cipher = Aes256Gcm::new(aes_key);

        let nonce = Nonce::from_slice(&encrypted.nonce);

        let plaintext = cipher.decrypt(nonce, encrypted.data.as_ref())
            .map_err(|_| Error::Storage("Decryption failed".to_string()))?;

        Ok(plaintext)
    }

    /// Rotate encryption key
    pub fn rotate_key(&mut self) -> Result<String> {
        let new_key_id = self.key_manager.rotate_key()?;
        self.current_key_id = Some(new_key_id.clone());
        Ok(new_key_id)
    }

    /// List available keys
    pub fn list_keys(&self) -> Vec<String> {
        self.key_manager.list_keys()
    }

    /// Derive key from password (for user-provided encryption)
    pub fn derive_key_from_password(&mut self, password: &str) -> Result<String> {
        let mut salt = [0u8; SALT_SIZE];
        RandOsRng.fill_bytes(&mut salt);
        
        let params = KeyDerivationParams {
            salt,
            iterations: KEY_DERIVATION_ITERATIONS,
        };

        self.key_manager.derive_key_from_password(password, params)
    }

    /// Generate key derivation parameters
    pub fn generate_key_derivation_params() -> KeyDerivationParams {
        let mut salt = [0u8; SALT_SIZE];
        RandOsRng.fill_bytes(&mut salt);
        
        KeyDerivationParams {
            salt,
            iterations: KEY_DERIVATION_ITERATIONS,
        }
    }
}

/// Utility functions for data integrity
pub fn calculate_integrity_hash(data: &[u8]) -> String {
    let mut hasher = Hasher::new();
    hasher.update(data);
    hex::encode(hasher.finalize().as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_file_key_manager() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = FileKeyManager::new(temp_dir.path()).unwrap();

        let key_id = manager.generate_key().unwrap();
        assert!(!key_id.is_empty());

        let key = manager.get_key(&key_id).unwrap();
        assert_eq!(key.as_bytes().len(), 32);

        let keys = manager.list_keys();
        assert!(keys.contains(&key_id));
    }

    #[test]
    fn test_encryption_engine() {
        let temp_dir = TempDir::new().unwrap();
        let key_manager = Box::new(FileKeyManager::new(temp_dir.path()).unwrap());
        let mut engine = EncryptionEngine::new(key_manager);

        let plaintext = b"Hello, World! This is a test message.";
        let encrypted = engine.encrypt(plaintext).unwrap();
        
        assert_ne!(encrypted.data, plaintext);
        assert_eq!(encrypted.nonce.len(), NONCE_SIZE);
        assert!(!encrypted.key_id.is_empty());

        let decrypted = engine.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_key_derivation() {
        let temp_dir = TempDir::new().unwrap();
        let key_manager = Box::new(FileKeyManager::new(temp_dir.path()).unwrap());
        let mut engine = EncryptionEngine::new(key_manager);

        let password = "test_password_123";
        let key_id = engine.derive_key_from_password(password).unwrap();
        assert!(!key_id.is_empty());

        let plaintext = b"Secret message";
        let encrypted = engine.encrypt_with_key(plaintext, &key_id).unwrap();
        let decrypted = engine.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_integrity_hash() {
        let data = b"test data";
        let hash1 = calculate_integrity_hash(data);
        let hash2 = calculate_integrity_hash(data);
        assert_eq!(hash1, hash2);

        let different_data = b"different data";
        let hash3 = calculate_integrity_hash(different_data);
        assert_ne!(hash1, hash3);
    }
}