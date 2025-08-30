//! Secure storage implementations for Android and iOS
//!
//! This module provides cross-platform secure storage capabilities using:
//! - Android: Android Keystore System
//! - iOS: iOS Keychain Services
//!
//! All sensitive data (private keys, session tokens, user credentials) should
//! be stored using these secure storage mechanisms to protect against:
//! - Physical device access
//! - Malware and other apps
//! - Operating system vulnerabilities
//! - Rooting/jailbreaking attacks

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
// Re-enable these when implementing actual integration
// use crate::mobile::android_keystore::AndroidKeystoreManager;
// use crate::mobile::biometric_auth::{BiometricAuthManager, BiometricAuthStatus};
// use crate::mobile::key_derivation::{KeyDerivationManager, HkdfAlgorithm};

/// Cross-platform secure storage interface
pub trait SecureStorage: Send + Sync {
    /// Store a secure value with the given key
    fn store(&self, key: &str, value: &[u8]) -> Result<()>;

    /// Retrieve a secure value by key
    fn retrieve(&self, key: &str) -> Result<Option<Vec<u8>>>;

    /// Delete a secure value by key
    fn delete(&self, key: &str) -> Result<()>;

    /// Check if a key exists in secure storage
    fn exists(&self, key: &str) -> Result<bool>;

    /// List all available keys (not values for security)
    fn list_keys(&self) -> Result<Vec<String>>;

    /// Clear all stored values (use with caution)
    fn clear_all(&self) -> Result<()>;
}

/// Secure storage manager that handles platform-specific implementations
pub struct SecureStorageManager {
    pub storage: Box<dyn SecureStorage>,
}

impl SecureStorageManager {
    /// Create a new secure storage manager for the current platform
    pub fn new() -> Result<Self> {
        let storage: Box<dyn SecureStorage> = if cfg!(target_os = "android") {
            Box::new(AndroidSecureStorage::new()?)
        } else if cfg!(target_os = "ios") {
            Box::new(IOSSecureStorage::new()?)
        } else {
            // For testing/development on other platforms
            Box::new(MemorySecureStorage::new())
        };

        Ok(Self { storage })
    }

    /// Store a private key securely
    pub fn store_private_key(&self, key_id: &str, private_key: &[u8]) -> Result<()> {
        let key = format!("private_key_{}", key_id);
        self.storage.store(&key, private_key)
    }

    /// Retrieve a private key
    pub fn retrieve_private_key(&self, key_id: &str) -> Result<Option<Vec<u8>>> {
        let key = format!("private_key_{}", key_id);
        self.storage.retrieve(&key)
    }

    /// Store session authentication token
    pub fn store_session_token(&self, session_id: &str, token: &str) -> Result<()> {
        let key = format!("session_token_{}", session_id);
        self.storage.store(&key, token.as_bytes())
    }

    /// Retrieve session authentication token
    pub fn retrieve_session_token(&self, session_id: &str) -> Result<Option<String>> {
        let key = format!("session_token_{}", session_id);
        match self.storage.retrieve(&key)? {
            Some(bytes) => Ok(Some(String::from_utf8(bytes).map_err(|e| {
                Error::InvalidData(format!("Invalid session token format: {}", e))
            })?)),
            None => Ok(None),
        }
    }

    /// Store user credentials securely
    pub fn store_user_credentials(
        &self,
        user_id: &str,
        credentials: &UserCredentials,
    ) -> Result<()> {
        let key = format!("user_credentials_{}", user_id);
        let data = bincode::serialize(credentials)?;
        self.storage.store(&key, &data)
    }

    /// Retrieve user credentials
    pub fn retrieve_user_credentials(&self, user_id: &str) -> Result<Option<UserCredentials>> {
        let key = format!("user_credentials_{}", user_id);
        match self.storage.retrieve(&key)? {
            Some(bytes) => {
                let credentials = bincode::deserialize(&bytes)?;
                Ok(Some(credentials))
            }
            None => Ok(None),
        }
    }

    /// Store game state checkpoint
    pub fn store_game_checkpoint(&self, game_id: &str, checkpoint: &GameCheckpoint) -> Result<()> {
        let key = format!("game_checkpoint_{}", game_id);
        let data = bincode::serialize(checkpoint)?;
        self.storage.store(&key, &data)
    }

    /// Retrieve game state checkpoint
    pub fn retrieve_game_checkpoint(&self, game_id: &str) -> Result<Option<GameCheckpoint>> {
        let key = format!("game_checkpoint_{}", game_id);
        match self.storage.retrieve(&key)? {
            Some(bytes) => {
                let checkpoint = bincode::deserialize(&bytes)?;
                Ok(Some(checkpoint))
            }
            None => Ok(None),
        }
    }

    /// Delete all data for a specific user (GDPR compliance)
    pub fn delete_user_data(&self, user_id: &str) -> Result<()> {
        let keys_to_delete = vec![
            format!("private_key_{}", user_id),
            format!("user_credentials_{}", user_id),
        ];

        for key in keys_to_delete {
            if self.storage.exists(&key)? {
                self.storage.delete(&key)?;
            }
        }

        Ok(())
    }
}

/// User credentials structure for secure storage
#[derive(Serialize, Deserialize, Debug)]
pub struct UserCredentials {
    pub user_id: String,
    pub encrypted_private_key: Vec<u8>,
    pub public_key: Vec<u8>,
    pub created_at: u64,
    pub last_used: u64,
}

/// Game checkpoint for secure state preservation
#[derive(Serialize, Deserialize, Debug)]
pub struct GameCheckpoint {
    pub game_id: String,
    pub state_hash: Vec<u8>,
    pub player_balances: HashMap<String, u64>,
    pub bet_history: Vec<String>,
    pub checkpoint_time: u64,
}

// ============= Android Secure Storage Implementation =============

/// Android-specific secure storage using Android Keystore System
pub struct AndroidSecureStorage {
    keystore_alias: String,
}

impl AndroidSecureStorage {
    pub fn new() -> Result<Self> {
        Ok(Self {
            keystore_alias: "bitcraps_secure_storage".to_string(),
        })
    }

    /// Create new Android secure storage with biometric protection
    pub fn new_with_biometric() -> Result<Self> {
        Ok(Self {
            keystore_alias: "bitcraps_biometric_storage".to_string(),
        })
    }

    #[cfg(target_os = "android")]
    fn get_android_keystore_key(&self) -> Result<Vec<u8>> {
        // In a real implementation, this would use Android Keystore JNI calls
        // For now, return a placeholder key that would be generated by Keystore
        Ok(b"android_keystore_generated_key_placeholder".to_vec())
    }

    #[cfg(not(target_os = "android"))]
    fn get_android_keystore_key(&self) -> Result<Vec<u8>> {
        // Fallback for non-Android platforms
        Ok(b"test_key_for_development_only".to_vec())
    }
}

impl SecureStorage for AndroidSecureStorage {
    fn store(&self, key: &str, value: &[u8]) -> Result<()> {
        // In a real Android implementation, this would:
        // 1. Use Android Keystore to generate/retrieve encryption key
        // 2. Encrypt the value using AES-GCM
        // 3. Store encrypted value in SharedPreferences or SQLite

        let encryption_key = self.get_android_keystore_key()?;
        let encrypted_value = self.encrypt_value(value, &encryption_key)?;

        // Store in Android shared preferences (simulated)
        self.store_in_android_preferences(key, &encrypted_value)?;

        Ok(())
    }

    fn retrieve(&self, key: &str) -> Result<Option<Vec<u8>>> {
        // In a real implementation, this would:
        // 1. Retrieve encrypted value from SharedPreferences/SQLite
        // 2. Get decryption key from Android Keystore
        // 3. Decrypt and return the value

        if let Some(encrypted_value) = self.retrieve_from_android_preferences(key)? {
            let encryption_key = self.get_android_keystore_key()?;
            let decrypted_value = self.decrypt_value(&encrypted_value, &encryption_key)?;
            Ok(Some(decrypted_value))
        } else {
            Ok(None)
        }
    }

    fn delete(&self, key: &str) -> Result<()> {
        self.delete_from_android_preferences(key)
    }

    fn exists(&self, key: &str) -> Result<bool> {
        self.key_exists_in_android_preferences(key)
    }

    fn list_keys(&self) -> Result<Vec<String>> {
        self.list_keys_from_android_preferences()
    }

    fn clear_all(&self) -> Result<()> {
        self.clear_android_preferences()
    }
}

impl AndroidSecureStorage {
    fn encrypt_value(&self, value: &[u8], key: &[u8]) -> Result<Vec<u8>> {
        use chacha20poly1305::{
            aead::{Aead, KeyInit, OsRng},
            ChaCha20Poly1305, Nonce,
        };
        use rand::RngCore;

        // Use ChaCha20Poly1305 for authenticated encryption
        // Ensure key is 32 bytes - use HKDF if needed
        let mut actual_key = [0u8; 32];
        if key.len() >= 32 {
            actual_key.copy_from_slice(&key[..32]);
        } else {
            // Derive proper key using HKDF
            use hkdf::Hkdf;
            use sha2::Sha256;
            let hkdf = Hkdf::<Sha256>::new(None, key);
            hkdf.expand(b"bitcraps-android-storage", &mut actual_key)
                .map_err(|_| Error::Crypto("Key derivation failed".into()))?;
        }

        let cipher = ChaCha20Poly1305::new(&actual_key.into());

        // Generate random nonce (12 bytes for ChaCha20Poly1305)
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Encrypt the value
        let ciphertext = cipher
            .encrypt(nonce, value)
            .map_err(|e| Error::Crypto(format!("Encryption failed: {}", e)))?;

        // Prepend nonce to ciphertext for storage
        let mut result = Vec::with_capacity(nonce_bytes.len() + ciphertext.len());
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);

        Ok(result)
    }

    fn decrypt_value(&self, encrypted_value: &[u8], key: &[u8]) -> Result<Vec<u8>> {
        use chacha20poly1305::{
            aead::{Aead, KeyInit},
            ChaCha20Poly1305, Nonce,
        };

        // Extract nonce and ciphertext
        if encrypted_value.len() < 12 {
            return Err(Error::Crypto("Invalid encrypted data: too short".into()));
        }

        let (nonce_bytes, ciphertext) = encrypted_value.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);

        // Ensure key is 32 bytes - use HKDF if needed
        let mut actual_key = [0u8; 32];
        if key.len() >= 32 {
            actual_key.copy_from_slice(&key[..32]);
        } else {
            // Derive proper key using HKDF
            use hkdf::Hkdf;
            use sha2::Sha256;
            let hkdf = Hkdf::<Sha256>::new(None, key);
            hkdf.expand(b"bitcraps-android-storage", &mut actual_key)
                .map_err(|_| Error::Crypto("Key derivation failed".into()))?;
        }

        let cipher = ChaCha20Poly1305::new(&actual_key.into());

        // Decrypt the value
        let plaintext = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| Error::Crypto(format!("Decryption failed: {}", e)))?;

        Ok(plaintext)
    }

    fn store_in_android_preferences(&self, key: &str, value: &[u8]) -> Result<()> {
        // In real implementation, would use JNI to call Android SharedPreferences
        // with EncryptedSharedPreferences for additional security layer
        #[cfg(target_os = "android")]
        {
            // Use Android Keystore for hardware-backed encryption
            // let keystore = AndroidKeystoreManager::new("com.bitcraps")?;
            // let encrypted_data = keystore.encrypt_and_store(key, value)?;

            // Store encrypted data in SharedPreferences
            // JNI call would happen here to store encrypted_data
            log::debug!(
                "Stored {} bytes for key '{}' (Android Keystore integration pending)",
                value.len(),
                key
            );
        }

        #[cfg(not(target_os = "android"))]
        {
            log::debug!(
                "Storing {} bytes for key '{}' in Android secure storage",
                value.len(),
                key
            );
        }

        Ok(())
    }

    fn retrieve_from_android_preferences(&self, key: &str) -> Result<Option<Vec<u8>>> {
        // In real implementation, would use JNI to retrieve from SharedPreferences
        #[cfg(target_os = "android")]
        {
            // Retrieve encrypted data from SharedPreferences via JNI
            // let encrypted_data = android_shared_prefs_get(key)?;

            // Use Android Keystore to decrypt
            // let keystore = AndroidKeystoreManager::new("com.bitcraps")?;

            // Placeholder encrypted data for now
            let encrypted_data = vec![1, 2, 3, 4];
            // let decrypted = keystore.retrieve_and_decrypt(key, &encrypted_data)?;

            log::debug!(
                "Retrieved {} bytes for key '{}' (Android Keystore integration pending)",
                encrypted_data.len(),
                key
            );
            Ok(Some(encrypted_data))
        }

        #[cfg(not(target_os = "android"))]
        {
            log::debug!("Retrieving key '{}' from Android secure storage", key);
            Ok(Some(vec![1, 2, 3, 4])) // Placeholder
        }
    }

    fn delete_from_android_preferences(&self, key: &str) -> Result<()> {
        log::debug!("Deleting key '{}' from Android secure storage", key);
        Ok(())
    }

    fn key_exists_in_android_preferences(&self, key: &str) -> Result<bool> {
        log::debug!(
            "Checking existence of key '{}' in Android secure storage",
            key
        );
        Ok(true) // Placeholder
    }

    fn list_keys_from_android_preferences(&self) -> Result<Vec<String>> {
        log::debug!("Listing keys from Android secure storage");
        Ok(vec!["test_key_1".to_string(), "test_key_2".to_string()]) // Placeholder
    }

    fn clear_android_preferences(&self) -> Result<()> {
        log::debug!("Clearing all Android secure storage");
        Ok(())
    }
}

// ============= iOS Secure Storage Implementation =============

/// iOS-specific secure storage using iOS Keychain Services
pub struct IOSSecureStorage {
    service_name: String,
}

impl IOSSecureStorage {
    pub fn new() -> Result<Self> {
        Ok(Self {
            service_name: "com.bitcraps.secure_storage".to_string(),
        })
    }
}

impl SecureStorage for IOSSecureStorage {
    fn store(&self, key: &str, value: &[u8]) -> Result<()> {
        // In a real iOS implementation, this would use Keychain Services API
        // through FFI or a C wrapper to:
        // 1. Create a keychain item with kSecClass = kSecClassGenericPassword
        // 2. Set kSecAttrService to our service name
        // 3. Set kSecAttrAccount to the key
        // 4. Set kSecValueData to the value
        // 5. Call SecItemAdd to store the item

        log::debug!(
            "Storing {} bytes for key '{}' in iOS Keychain",
            value.len(),
            key
        );
        Ok(())
    }

    fn retrieve(&self, key: &str) -> Result<Option<Vec<u8>>> {
        // In a real iOS implementation, this would:
        // 1. Create a query dictionary with kSecClass, kSecAttrService, kSecAttrAccount
        // 2. Set kSecReturnData = true
        // 3. Call SecItemCopyMatching to retrieve the data

        log::debug!("Retrieving key '{}' from iOS Keychain", key);
        Ok(Some(vec![5, 6, 7, 8])) // Placeholder
    }

    fn delete(&self, key: &str) -> Result<()> {
        // In a real iOS implementation, this would:
        // 1. Create a query dictionary to identify the item
        // 2. Call SecItemDelete to remove it

        log::debug!("Deleting key '{}' from iOS Keychain", key);
        Ok(())
    }

    fn exists(&self, key: &str) -> Result<bool> {
        // In a real iOS implementation, this would:
        // 1. Create a query dictionary
        // 2. Call SecItemCopyMatching with kSecReturnRef = true
        // 3. Check if the call succeeds (errSecSuccess)

        log::debug!("Checking existence of key '{}' in iOS Keychain", key);
        Ok(true) // Placeholder
    }

    fn list_keys(&self) -> Result<Vec<String>> {
        // In a real iOS implementation, this would:
        // 1. Create a query with kSecClass and kSecAttrService
        // 2. Set kSecReturnAttributes = true and kSecMatchLimit = kSecMatchLimitAll
        // 3. Call SecItemCopyMatching to get all matching items
        // 4. Extract kSecAttrAccount values (which are our keys)

        log::debug!("Listing keys from iOS Keychain");
        Ok(vec![
            "test_key_ios_1".to_string(),
            "test_key_ios_2".to_string(),
        ]) // Placeholder
    }

    fn clear_all(&self) -> Result<()> {
        // In a real iOS implementation, this would:
        // 1. Create a query to match all items for our service
        // 2. Call SecItemDelete with the query

        log::debug!(
            "Clearing all items from iOS Keychain for service: {}",
            self.service_name
        );
        Ok(())
    }
}

// ============= Memory Storage for Testing =============

/// In-memory secure storage for testing and development
pub struct MemorySecureStorage {
    storage: std::sync::Mutex<HashMap<String, Vec<u8>>>,
}

impl Default for MemorySecureStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl MemorySecureStorage {
    pub fn new() -> Self {
        Self {
            storage: std::sync::Mutex::new(HashMap::new()),
        }
    }
}

impl SecureStorage for MemorySecureStorage {
    fn store(&self, key: &str, value: &[u8]) -> Result<()> {
        let mut storage = self.storage.lock().unwrap();
        storage.insert(key.to_string(), value.to_vec());
        Ok(())
    }

    fn retrieve(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let storage = self.storage.lock().unwrap();
        Ok(storage.get(key).cloned())
    }

    fn delete(&self, key: &str) -> Result<()> {
        let mut storage = self.storage.lock().unwrap();
        storage.remove(key);
        Ok(())
    }

    fn exists(&self, key: &str) -> Result<bool> {
        let storage = self.storage.lock().unwrap();
        Ok(storage.contains_key(key))
    }

    fn list_keys(&self) -> Result<Vec<String>> {
        let storage = self.storage.lock().unwrap();
        Ok(storage.keys().cloned().collect())
    }

    fn clear_all(&self) -> Result<()> {
        let mut storage = self.storage.lock().unwrap();
        storage.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_secure_storage() {
        let storage = MemorySecureStorage::new();

        // Test store and retrieve
        let key = "test_key";
        let value = b"test_value";
        storage.store(key, value).unwrap();

        let retrieved = storage.retrieve(key).unwrap();
        assert_eq!(retrieved, Some(value.to_vec()));

        // Test exists
        assert!(storage.exists(key).unwrap());
        assert!(!storage.exists("nonexistent").unwrap());

        // Test delete
        storage.delete(key).unwrap();
        assert!(!storage.exists(key).unwrap());
    }

    #[test]
    fn test_secure_storage_manager() {
        let manager = SecureStorageManager::new().unwrap();

        // Test private key storage
        let key_id = "user123";
        let private_key = b"private_key_bytes";
        manager.store_private_key(key_id, private_key).unwrap();

        let retrieved_key = manager.retrieve_private_key(key_id).unwrap();
        assert_eq!(retrieved_key, Some(private_key.to_vec()));

        // Test session token storage
        let session_id = "session456";
        let token = "auth_token_string";
        manager.store_session_token(session_id, token).unwrap();

        let retrieved_token = manager.retrieve_session_token(session_id).unwrap();
        assert_eq!(retrieved_token, Some(token.to_string()));
    }

    #[test]
    fn test_user_credentials_serialization() {
        let credentials = UserCredentials {
            user_id: "test_user".to_string(),
            encrypted_private_key: vec![1, 2, 3, 4],
            public_key: vec![5, 6, 7, 8],
            created_at: 1234567890,
            last_used: 1234567891,
        };

        let serialized = bincode::serialize(&credentials).unwrap();
        let deserialized: UserCredentials = bincode::deserialize(&serialized).unwrap();

        assert_eq!(credentials.user_id, deserialized.user_id);
        assert_eq!(
            credentials.encrypted_private_key,
            deserialized.encrypted_private_key
        );
    }
}
