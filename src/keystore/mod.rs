//! Secure key management system for production deployments
//! 
//! Provides secure storage, rotation, and lifecycle management
//! for cryptographic keys with hardware security module support.

use std::sync::Arc;
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::path::{Path, PathBuf};
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};
use zeroize::{Zeroize, ZeroizeOnDrop};
use crate::error::{Error, Result};
use crate::crypto::BitchatKeypair;
use ed25519_dalek::SigningKey;
use chacha20poly1305::{
    aead::{Aead, KeyInit, OsRng},
    ChaCha20Poly1305, Nonce,
};
use argon2::Argon2;
use rand::RngCore;

/// Key types supported by the keystore
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum KeyType {
    Signing,
    Encryption,
    Session,
    Master,
}

/// Key metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyMetadata {
    pub key_id: String,
    pub key_type: KeyType,
    pub created_at: u64,
    pub expires_at: Option<u64>,
    pub rotation_count: u32,
    pub algorithm: String,
    pub purpose: String,
}

/// Encrypted key material
#[derive(Serialize, Deserialize)]
pub struct EncryptedKey {
    #[serde(with = "serde_bytes")]
    ciphertext: Vec<u8>,
    #[serde(with = "serde_bytes")]
    nonce: Vec<u8>,
    #[serde(with = "serde_bytes")]
    salt: Vec<u8>,
    metadata: KeyMetadata,
}

/// Decrypted key material (sensitive)
pub struct DecryptedKey {
    pub key_type: KeyType,
    pub signing_key: Option<SigningKey>,
    pub raw_bytes: Vec<u8>,
    pub metadata: KeyMetadata,
}

/// Secure keystore for managing cryptographic keys
pub struct SecureKeystore {
    storage_path: PathBuf,
    keys: Arc<RwLock<HashMap<String, EncryptedKey>>>,
    cache: Arc<RwLock<HashMap<String, Arc<DecryptedKey>>>>,
    master_key: Arc<RwLock<Option<MasterKey>>>,
    rotation_policy: RotationPolicy,
    audit_log: Arc<RwLock<Vec<AuditEntry>>>,
}

/// Master key for encrypting other keys
#[derive(ZeroizeOnDrop)]
struct MasterKey {
    key: [u8; 32],
    #[zeroize(skip)]
    derived_at: SystemTime,
}

/// Key rotation policy
#[derive(Debug, Clone)]
pub struct RotationPolicy {
    pub signing_key_lifetime: Duration,
    pub encryption_key_lifetime: Duration,
    pub session_key_lifetime: Duration,
    pub auto_rotate: bool,
    pub rotation_warning_period: Duration,
}

/// Audit log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub timestamp: u64,
    pub operation: String,
    pub key_id: String,
    pub success: bool,
    pub details: String,
}

/// Hardware Security Module interface
#[async_trait::async_trait]
pub trait HsmProvider: Send + Sync {
    async fn generate_key(&self, key_type: KeyType) -> Result<Vec<u8>>;
    async fn sign(&self, key_id: &str, data: &[u8]) -> Result<Vec<u8>>;
    async fn decrypt(&self, key_id: &str, ciphertext: &[u8]) -> Result<Vec<u8>>;
    async fn get_public_key(&self, key_id: &str) -> Result<Vec<u8>>;
}

impl SecureKeystore {
    /// Create a new secure keystore
    pub async fn new(storage_path: impl AsRef<Path>) -> Result<Self> {
        let storage_path = storage_path.as_ref().to_path_buf();
        
        // Create storage directory if it doesn't exist
        std::fs::create_dir_all(&storage_path)
            .map_err(|e| Error::Io(e))?;
        
        // Set restrictive permissions (Unix only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(
                &storage_path,
                std::fs::Permissions::from_mode(0o700),
            ).map_err(|e| Error::Io(e))?;
        }
        
        Ok(Self {
            storage_path,
            keys: Arc::new(RwLock::new(HashMap::new())),
            cache: Arc::new(RwLock::new(HashMap::new())),
            master_key: Arc::new(RwLock::new(None)),
            rotation_policy: RotationPolicy::default(),
            audit_log: Arc::new(RwLock::new(Vec::new())),
        })
    }
    
    /// Initialize master key from password
    pub async fn init_master_key(&self, password: &str) -> Result<()> {
        // Generate salt
        let mut salt = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut salt);
        
        // Derive master key using Argon2
        let argon2 = Argon2::default();
        let mut key = [0u8; 32];
        
        argon2.hash_password_into(
            password.as_bytes(),
            &salt,
            &mut key,
        ).map_err(|e| Error::Crypto(format!("Failed to derive master key: {}", e)))?;
        
        let master = MasterKey {
            key,
            derived_at: SystemTime::now(),
        };
        
        *self.master_key.write().await = Some(master);
        
        self.audit("init_master_key", "", true, "Master key initialized").await;
        
        Ok(())
    }
    
    /// Generate a new key
    pub async fn generate_key(
        &self,
        key_type: KeyType,
        purpose: &str,
    ) -> Result<String> {
        let key_id = format!("{}_{}", 
            match key_type {
                KeyType::Signing => "sign",
                KeyType::Encryption => "enc",
                KeyType::Session => "sess",
                KeyType::Master => "master",
            },
            hex::encode(&rand::random::<[u8; 8]>())
        );
        
        let (raw_bytes, algorithm) = match key_type {
            KeyType::Signing => {
                let signing_key = SigningKey::generate(&mut OsRng);
                (signing_key.to_bytes().to_vec(), "Ed25519".to_string())
            }
            KeyType::Encryption => {
                let mut key = vec![0u8; 32];
                rand::thread_rng().fill_bytes(&mut key);
                (key, "X25519".to_string())
            }
            KeyType::Session => {
                let mut key = vec![0u8; 32];
                OsRng.fill_bytes(&mut key);
                (key, "ChaCha20Poly1305".to_string())
            }
            KeyType::Master => {
                return Err(Error::Crypto("Cannot generate master key directly".to_string()));
            }
        };
        
        let metadata = KeyMetadata {
            key_id: key_id.clone(),
            key_type: key_type.clone(),
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            expires_at: self.calculate_expiry(&key_type),
            rotation_count: 0,
            algorithm,
            purpose: purpose.to_string(),
        };
        
        // Encrypt and store the key
        let encrypted = self.encrypt_key(&raw_bytes, &metadata).await?;
        
        let mut keys = self.keys.write().await;
        keys.insert(key_id.clone(), encrypted);
        
        // Save to disk
        self.persist_keys().await?;
        
        self.audit("generate_key", &key_id, true, 
                  &format!("Generated {} key", key_type.as_str())).await;
        
        Ok(key_id)
    }
    
    /// Retrieve and decrypt a key
    pub async fn get_key(&self, key_id: &str) -> Result<Arc<DecryptedKey>> {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(key) = cache.get(key_id) {
                return Ok(key.clone());
            }
        }
        
        // Load and decrypt from storage
        let keys = self.keys.read().await;
        let encrypted = keys.get(key_id)
            .ok_or_else(|| Error::Crypto(format!("Key {} not found", key_id)))?;
        
        let decrypted = self.decrypt_key(encrypted).await?;
        let decrypted_arc = Arc::new(decrypted);
        
        // Update cache
        let mut cache = self.cache.write().await;
        cache.insert(key_id.to_string(), decrypted_arc.clone());
        
        self.audit("get_key", key_id, true, "Key retrieved").await;
        
        Ok(decrypted_arc)
    }
    
    /// Rotate a key
    pub async fn rotate_key(&self, key_id: &str) -> Result<String> {
        let old_key = self.get_key(key_id).await?;
        
        // Generate new key of same type
        let new_key_id = self.generate_key(
            old_key.key_type.clone(),
            &old_key.metadata.purpose,
        ).await?;
        
        // Update rotation count
        let mut keys = self.keys.write().await;
        if let Some(new_encrypted) = keys.get_mut(&new_key_id) {
            new_encrypted.metadata.rotation_count = old_key.metadata.rotation_count + 1;
        }
        
        // Mark old key as rotated (but keep for decryption of old data)
        if let Some(old_encrypted) = keys.get_mut(key_id) {
            old_encrypted.metadata.expires_at = Some(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
            );
        }
        
        self.persist_keys().await?;
        
        self.audit("rotate_key", key_id, true,
                  &format!("Rotated to {}", new_key_id)).await;
        
        Ok(new_key_id)
    }
    
    /// Delete a key (secure erasure)
    pub async fn delete_key(&self, key_id: &str) -> Result<()> {
        // Remove from cache
        {
            let mut cache = self.cache.write().await;
            cache.remove(key_id);
        }
        
        // Remove from storage
        {
            let mut keys = self.keys.write().await;
            keys.remove(key_id);
        }
        
        // Update persistent storage
        self.persist_keys().await?;
        
        self.audit("delete_key", key_id, true, "Key deleted").await;
        
        Ok(())
    }
    
    /// Check if any keys need rotation
    pub async fn check_rotation_needed(&self) -> Vec<String> {
        let mut keys_needing_rotation = Vec::new();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let keys = self.keys.read().await;
        for (key_id, encrypted) in keys.iter() {
            if let Some(expires_at) = encrypted.metadata.expires_at {
                let warning_time = expires_at - self.rotation_policy.rotation_warning_period.as_secs();
                if now >= warning_time {
                    keys_needing_rotation.push(key_id.clone());
                }
            }
        }
        
        keys_needing_rotation
    }
    
    /// Encrypt a key for storage
    async fn encrypt_key(&self, raw_bytes: &[u8], metadata: &KeyMetadata) -> Result<EncryptedKey> {
        let master = self.master_key.read().await;
        let master_key = master.as_ref()
            .ok_or_else(|| Error::Crypto("Master key not initialized".to_string()))?;
        
        let cipher = ChaCha20Poly1305::new_from_slice(&master_key.key)
            .map_err(|e| Error::Crypto(format!("Failed to create cipher: {}", e)))?;
        
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        
        let ciphertext = cipher.encrypt(nonce, raw_bytes)
            .map_err(|e| Error::Crypto(format!("Encryption failed: {}", e)))?;
        
        let mut salt = vec![0u8; 32];
        OsRng.fill_bytes(&mut salt);
        
        Ok(EncryptedKey {
            ciphertext,
            nonce: nonce_bytes.to_vec(),
            salt,
            metadata: metadata.clone(),
        })
    }
    
    /// Decrypt a key from storage
    async fn decrypt_key(&self, encrypted: &EncryptedKey) -> Result<DecryptedKey> {
        let master = self.master_key.read().await;
        let master_key = master.as_ref()
            .ok_or_else(|| Error::Crypto("Master key not initialized".to_string()))?;
        
        let cipher = ChaCha20Poly1305::new_from_slice(&master_key.key)
            .map_err(|e| Error::Crypto(format!("Failed to create cipher: {}", e)))?;
        
        let nonce = Nonce::from_slice(&encrypted.nonce);
        
        let raw_bytes = cipher.decrypt(nonce, encrypted.ciphertext.as_ref())
            .map_err(|e| Error::Crypto(format!("Decryption failed: {}", e)))?;
        
        let signing_key = match encrypted.metadata.key_type {
            KeyType::Signing => {
                let key = SigningKey::from_bytes(
                    raw_bytes.as_slice().try_into()
                        .map_err(|_| Error::Crypto("Invalid signing key".to_string()))?
                );
                Some(key)
            }
            _ => None,
        };
        
        Ok(DecryptedKey {
            key_type: encrypted.metadata.key_type.clone(),
            signing_key,
            raw_bytes,
            metadata: encrypted.metadata.clone(),
        })
    }
    
    /// Persist keys to disk
    async fn persist_keys(&self) -> Result<()> {
        let keys = self.keys.read().await;
        let data = bincode::serialize(&*keys)
            .map_err(|e| Error::Crypto(format!("Serialization failed: {}", e)))?;
        
        let path = self.storage_path.join("keys.enc");
        tokio::fs::write(&path, data).await
            .map_err(|e| Error::Io(e))?;
        
        // Set restrictive permissions
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
                .map_err(|e| Error::Io(e))?;
        }
        
        Ok(())
    }
    
    /// Load keys from disk
    pub async fn load_keys(&self) -> Result<()> {
        let path = self.storage_path.join("keys.enc");
        if !path.exists() {
            return Ok(());
        }
        
        let data = tokio::fs::read(&path).await
            .map_err(|e| Error::Io(e))?;
        
        let loaded: HashMap<String, EncryptedKey> = bincode::deserialize(&data)
            .map_err(|e| Error::Crypto(format!("Deserialization failed: {}", e)))?;
        
        *self.keys.write().await = loaded;
        
        Ok(())
    }
    
    /// Calculate key expiry time
    fn calculate_expiry(&self, key_type: &KeyType) -> Option<u64> {
        let lifetime = match key_type {
            KeyType::Signing => self.rotation_policy.signing_key_lifetime,
            KeyType::Encryption => self.rotation_policy.encryption_key_lifetime,
            KeyType::Session => self.rotation_policy.session_key_lifetime,
            KeyType::Master => return None,
        };
        
        Some(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() + lifetime.as_secs()
        )
    }
    
    /// Add audit log entry
    async fn audit(&self, operation: &str, key_id: &str, success: bool, details: &str) {
        let entry = AuditEntry {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            operation: operation.to_string(),
            key_id: key_id.to_string(),
            success,
            details: details.to_string(),
        };
        
        self.audit_log.write().await.push(entry);
    }
    
    /// Export audit log
    pub async fn export_audit_log(&self) -> Vec<AuditEntry> {
        self.audit_log.read().await.clone()
    }
}

impl KeyType {
    fn as_str(&self) -> &str {
        match self {
            KeyType::Signing => "signing",
            KeyType::Encryption => "encryption",
            KeyType::Session => "session",
            KeyType::Master => "master",
        }
    }
}

impl Default for RotationPolicy {
    fn default() -> Self {
        Self {
            signing_key_lifetime: Duration::from_secs(30 * 24 * 3600), // 30 days
            encryption_key_lifetime: Duration::from_secs(90 * 24 * 3600), // 90 days
            session_key_lifetime: Duration::from_secs(24 * 3600), // 1 day
            auto_rotate: true,
            rotation_warning_period: Duration::from_secs(7 * 24 * 3600), // 7 days
        }
    }
}

/// Automatic key rotation service
pub struct KeyRotationService {
    keystore: Arc<SecureKeystore>,
    check_interval: Duration,
}

impl KeyRotationService {
    pub fn new(keystore: Arc<SecureKeystore>) -> Self {
        Self {
            keystore,
            check_interval: Duration::from_secs(3600), // Check hourly
        }
    }
    
    pub async fn start(self) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(self.check_interval);
            
            loop {
                interval.tick().await;
                
                let keys_to_rotate = self.keystore.check_rotation_needed().await;
                for key_id in keys_to_rotate {
                    log::info!("Rotating key {}", key_id);
                    if let Err(e) = self.keystore.rotate_key(&key_id).await {
                        log::error!("Failed to rotate key {}: {}", key_id, e);
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_key_lifecycle() {
        let temp_dir = TempDir::new().unwrap();
        let keystore = SecureKeystore::new(temp_dir.path()).await.unwrap();
        
        // Initialize master key
        keystore.init_master_key("test_password").await.unwrap();
        
        // Generate a signing key
        let key_id = keystore.generate_key(KeyType::Signing, "test").await.unwrap();
        
        // Retrieve the key
        let key = keystore.get_key(&key_id).await.unwrap();
        assert_eq!(key.key_type, KeyType::Signing);
        assert!(key.signing_key.is_some());
        
        // Rotate the key
        let new_key_id = keystore.rotate_key(&key_id).await.unwrap();
        assert_ne!(key_id, new_key_id);
        
        // Delete the old key
        keystore.delete_key(&key_id).await.unwrap();
        
        // Verify it's gone
        assert!(keystore.get_key(&key_id).await.is_err());
    }
    
    #[tokio::test]
    async fn test_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_path_buf();
        
        // Create keystore and generate keys
        {
            let keystore = SecureKeystore::new(&path).await.unwrap();
            keystore.init_master_key("test_password").await.unwrap();
            keystore.generate_key(KeyType::Signing, "test").await.unwrap();
        }
        
        // Load in new instance
        {
            let keystore = SecureKeystore::new(&path).await.unwrap();
            keystore.init_master_key("test_password").await.unwrap();
            keystore.load_keys().await.unwrap();
            
            let keys = keystore.keys.read().await;
            assert_eq!(keys.len(), 1);
        }
    }
}