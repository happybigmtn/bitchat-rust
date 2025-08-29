//! Enhanced Secure Keystore for Transport Layer
//!
//! This module provides a comprehensive secure keystore implementation for:
//! - Persistent encrypted key storage
//! - Hardware Security Module (HSM) interface preparation
//! - Key derivation and rotation
//! - Secure memory management with zeroization
//! - Cross-platform keystore integration

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::fs;
use zeroize::ZeroizeOnDrop;
use serde::{Serialize, Deserialize};
use rand::{RngCore, rngs::OsRng};
use chacha20poly1305::{ChaCha20Poly1305, KeyInit, aead::Aead, Nonce};
use argon2::{Argon2, PasswordHasher};
use argon2::password_hash::SaltString;

use crate::error::{Error, Result};
use crate::crypto::{BitchatIdentity, BitchatKeypair, GameCrypto, KeyDerivation};
use crate::protocol::PeerId;

/// Keystore entry containing encrypted key data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeystoreEntry {
    /// Unique identifier for the key
    pub key_id: String,
    /// Encrypted key material
    pub encrypted_data: Vec<u8>,
    /// Salt used for key derivation
    pub salt: [u8; 32],
    /// Key derivation parameters
    pub derivation_params: KeyDerivationParams,
    /// Key metadata
    pub metadata: KeyMetadata,
    /// Creation timestamp
    pub created_at: u64,
    /// Last access timestamp
    pub last_accessed: u64,
}

/// Key derivation parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyDerivationParams {
    /// Argon2 memory cost (in KB)
    pub memory_cost: u32,
    /// Argon2 time cost (iterations)
    pub time_cost: u32,
    /// Argon2 parallelism
    pub parallelism: u32,
    /// Output key length
    pub output_length: usize,
}

impl Default for KeyDerivationParams {
    fn default() -> Self {
        Self {
            memory_cost: 65536,  // 64 MB
            time_cost: 3,        // 3 iterations
            parallelism: 4,      // 4 threads
            output_length: 32,   // 256 bits
        }
    }
}

/// Key metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyMetadata {
    /// Key type identifier
    pub key_type: KeyType,
    /// Purpose description
    pub purpose: String,
    /// Associated peer ID (if applicable)
    pub peer_id: Option<PeerId>,
    /// Key version
    pub version: u32,
    /// Expiration timestamp (None for non-expiring keys)
    pub expires_at: Option<u64>,
    /// Usage counter
    pub usage_count: u64,
}

/// Supported key types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum KeyType {
    /// Ed25519 signing keypair
    SigningKeypair,
    /// X25519 ECDH keypair
    EcdhKeypair,
    /// Symmetric encryption key
    SymmetricKey,
    /// HMAC authentication key
    HmacKey,
    /// Session key
    SessionKey,
    /// Master key for key derivation
    MasterKey,
    /// Identity private key
    IdentityKey,
}

/// Secure keystore implementation with encrypted storage
#[derive(Clone)]
pub struct SecureTransportKeystore {
    /// Storage directory
    storage_path: PathBuf,
    /// In-memory key cache (encrypted)
    key_cache: Arc<RwLock<HashMap<String, KeystoreEntry>>>,
    /// Master encryption key (derived from password)
    master_key: Arc<RwLock<Option<[u8; 32]>>>,
    /// Keystore configuration
    config: KeystoreConfig,
    /// Runtime statistics
    stats: Arc<RwLock<KeystoreStats>>,
}

/// Keystore configuration
#[derive(Debug, Clone)]
pub struct KeystoreConfig {
    /// Enable in-memory caching
    pub enable_cache: bool,
    /// Maximum cache size
    pub max_cache_size: usize,
    /// Auto-save interval in seconds
    pub auto_save_interval: u64,
    /// Key rotation interval in seconds
    pub key_rotation_interval: u64,
    /// Enable HSM integration (future)
    pub enable_hsm: bool,
    /// Backup encryption enabled
    pub enable_backup_encryption: bool,
}

impl Default for KeystoreConfig {
    fn default() -> Self {
        Self {
            enable_cache: true,
            max_cache_size: 1000,
            auto_save_interval: 300, // 5 minutes
            key_rotation_interval: 24 * 60 * 60, // 24 hours
            enable_hsm: false,
            enable_backup_encryption: true,
        }
    }
}

/// Keystore runtime statistics
#[derive(Debug, Clone, Default)]
pub struct KeystoreStats {
    pub keys_stored: u64,
    pub keys_retrieved: u64,
    pub keys_rotated: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub encryption_operations: u64,
    pub decryption_operations: u64,
    pub last_backup_time: Option<u64>,
}

/// Secure memory container that zeroizes on drop
#[derive(ZeroizeOnDrop)]
pub(crate) struct SecureBytes {
    data: Vec<u8>,
}

impl SecureBytes {
    fn new(data: Vec<u8>) -> Self {
        Self { data }
    }

    fn as_slice(&self) -> &[u8] {
        &self.data
    }
}

impl SecureTransportKeystore {
    /// Create new secure keystore
    pub async fn new<P: AsRef<Path>>(storage_path: P) -> Result<Self> {
        Self::new_with_config(storage_path, KeystoreConfig::default()).await
    }

    /// Create new secure keystore with configuration
    pub async fn new_with_config<P: AsRef<Path>>(
        storage_path: P, 
        config: KeystoreConfig
    ) -> Result<Self> {
        let storage_path = storage_path.as_ref().to_path_buf();
        
        // Create storage directory if it doesn't exist
        if !storage_path.exists() {
            fs::create_dir_all(&storage_path).await
                .map_err(|e| Error::IoError(format!("Failed to create keystore directory: {}", e)))?;
        }

        let keystore = Self {
            storage_path,
            key_cache: Arc::new(RwLock::new(HashMap::new())),
            master_key: Arc::new(RwLock::new(None)),
            config,
            stats: Arc::new(RwLock::new(KeystoreStats::default())),
        };

        // Start background tasks
        keystore.start_background_tasks();

        Ok(keystore)
    }

    /// Initialize keystore with master password
    pub async fn initialize(&self, password: &str) -> Result<()> {
        // Derive master key from password
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        
        // Use Argon2 for password-based key derivation
        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| Error::Crypto(format!("Password hashing failed: {}", e)))?;

        // Derive 256-bit master key
        let master_key = KeyDerivation::derive_key_pbkdf2(
            password.as_bytes(),
            salt.as_salt().as_str().as_bytes(),
            100_000, // iterations
            32,      // key length
        )?;

        let mut key_array = [0u8; 32];
        key_array.copy_from_slice(&master_key[..32]);

        *self.master_key.write().await = Some(key_array);

        // Load existing keys from storage
        self.load_keys_from_storage().await?;

        log::info!("Keystore initialized with {} keys", self.key_cache.read().await.len());
        Ok(())
    }

    /// Check if keystore is unlocked
    pub async fn is_unlocked(&self) -> bool {
        self.master_key.read().await.is_some()
    }

    /// Store a new key in the keystore
    pub async fn store_key<K: AsRef<[u8]>>(
        &self,
        key_id: &str,
        key_data: K,
        key_type: KeyType,
        purpose: &str,
        peer_id: Option<PeerId>,
    ) -> Result<()> {
        if !self.is_unlocked().await {
            return Err(Error::Crypto("Keystore is locked".to_string()));
        }

        let master_key = self.master_key.read().await.unwrap();
        
        // Generate salt for this key
        let mut salt = [0u8; 32];
        OsRng.fill_bytes(&mut salt);

        // Encrypt key data
        let encrypted_data = self.encrypt_key_data(&master_key, key_data.as_ref(), &salt)?;

        // Create keystore entry
        let entry = KeystoreEntry {
            key_id: key_id.to_string(),
            encrypted_data,
            salt,
            derivation_params: KeyDerivationParams::default(),
            metadata: KeyMetadata {
                key_type,
                purpose: purpose.to_string(),
                peer_id,
                version: 1,
                expires_at: None,
                usage_count: 0,
            },
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            last_accessed: 0,
        };

        // Store in cache
        self.key_cache.write().await.insert(key_id.to_string(), entry.clone());

        // Persist to storage
        self.save_key_to_storage(&entry).await?;

        // Update stats
        self.stats.write().await.keys_stored += 1;

        log::debug!("Stored key: {} ({})", key_id, purpose);
        Ok(())
    }

    /// Retrieve a key from the keystore
    pub(crate) async fn retrieve_key(&self, key_id: &str) -> Result<SecureBytes> {
        if !self.is_unlocked().await {
            return Err(Error::Crypto("Keystore is locked".to_string()));
        }

        let master_key = self.master_key.read().await.unwrap();

        // Try cache first
        if self.config.enable_cache {
            let mut cache = self.key_cache.write().await;
            if let Some(entry) = cache.get_mut(key_id) {
                // Update access time and usage count
                entry.last_accessed = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                entry.metadata.usage_count += 1;

                let decrypted_data = self.decrypt_key_data(&master_key, &entry.encrypted_data, &entry.salt)?;
                
                self.stats.write().await.cache_hits += 1;
                self.stats.write().await.keys_retrieved += 1;
                
                return Ok(SecureBytes::new(decrypted_data));
            }
        }

        // Load from storage
        let entry = self.load_key_from_storage(key_id).await?;
        let decrypted_data = self.decrypt_key_data(&master_key, &entry.encrypted_data, &entry.salt)?;

        // Update cache if enabled
        if self.config.enable_cache {
            self.key_cache.write().await.insert(key_id.to_string(), entry);
        }

        self.stats.write().await.cache_misses += 1;
        self.stats.write().await.keys_retrieved += 1;

        Ok(SecureBytes::new(decrypted_data))
    }

    /// Store an identity keypair securely
    pub async fn store_identity(
        &self,
        identity: &BitchatIdentity,
        key_id: &str,
    ) -> Result<()> {
        // Store private key
        let private_key_bytes = identity.keypair.secret_key_bytes();
        self.store_key(
            &format!("{}_private", key_id),
            &private_key_bytes,
            KeyType::IdentityKey,
            "Identity private key",
            Some(identity.peer_id),
        ).await?;

        // Store public key and metadata
        let mut identity_data = Vec::new();
        identity_data.extend_from_slice(&identity.peer_id);
        identity_data.extend_from_slice(&identity.pow_nonce.to_be_bytes());
        identity_data.extend_from_slice(&identity.pow_difficulty.to_be_bytes());

        self.store_key(
            &format!("{}_metadata", key_id),
            &identity_data,
            KeyType::IdentityKey,
            "Identity metadata",
            Some(identity.peer_id),
        ).await?;

        Ok(())
    }

    /// Retrieve an identity keypair
    pub async fn retrieve_identity(&self, key_id: &str) -> Result<BitchatIdentity> {
        // Load private key
        let private_key_data = self.retrieve_key(&format!("{}_private", key_id)).await?;
        if private_key_data.as_slice().len() != 32 {
            return Err(Error::Crypto("Invalid private key size".to_string()));
        }

        let mut private_key = [0u8; 32];
        private_key.copy_from_slice(private_key_data.as_slice());

        // Load metadata
        let metadata = self.retrieve_key(&format!("{}_metadata", key_id)).await?;
        if metadata.as_slice().len() < 32 + 8 + 4 {
            return Err(Error::Crypto("Invalid identity metadata".to_string()));
        }

        let mut peer_id = [0u8; 32];
        peer_id.copy_from_slice(&metadata.as_slice()[..32]);

        let pow_nonce = u64::from_be_bytes([
            metadata.as_slice()[32], metadata.as_slice()[33], metadata.as_slice()[34], metadata.as_slice()[35],
            metadata.as_slice()[36], metadata.as_slice()[37], metadata.as_slice()[38], metadata.as_slice()[39],
        ]);

        let pow_difficulty = u32::from_be_bytes([
            metadata.as_slice()[40], metadata.as_slice()[41], metadata.as_slice()[42], metadata.as_slice()[43],
        ]);

        // Reconstruct keypair
        let keypair = BitchatKeypair::from_secret_key(&private_key)?;

        // Create identity
        let identity = BitchatIdentity {
            peer_id,
            keypair,
            pow_nonce,
            pow_difficulty,
        };

        // Verify integrity
        if !identity.verify_pow() {
            return Err(Error::Crypto("Identity PoW verification failed".to_string()));
        }

        Ok(identity)
    }

    /// Generate and store a new session key for a peer
    pub async fn generate_session_key(&self, peer_id: PeerId) -> Result<[u8; 32]> {
        let mut session_key = [0u8; 32];
        OsRng.fill_bytes(&mut session_key);

        let key_id = format!("session_{}", hex::encode(peer_id));
        
        self.store_key(
            &key_id,
            &session_key,
            KeyType::SessionKey,
            "BLE session key",
            Some(peer_id),
        ).await?;

        Ok(session_key)
    }

    /// Rotate all keys for a specific peer
    pub async fn rotate_peer_keys(&self, peer_id: PeerId) -> Result<()> {
        let cache = self.key_cache.read().await;
        let keys_to_rotate: Vec<String> = cache
            .iter()
            .filter(|(_, entry)| entry.metadata.peer_id == Some(peer_id))
            .map(|(key_id, _)| key_id.clone())
            .collect();
        drop(cache);
        
        let keys_count = keys_to_rotate.len();

        for key_id in keys_to_rotate {
            // Load existing key
            let mut cache = self.key_cache.write().await;
            if let Some(entry) = cache.get_mut(&key_id) {
                // Generate new key data based on type
                let new_key_data = match entry.metadata.key_type {
                    KeyType::SessionKey | KeyType::SymmetricKey | KeyType::HmacKey => {
                        let mut new_key = [0u8; 32];
                        OsRng.fill_bytes(&mut new_key);
                        new_key.to_vec()
                    }
                    KeyType::EcdhKeypair => {
                        let ephemeral_secret = x25519_dalek::EphemeralSecret::random_from_rng(OsRng);
                        let public_key = x25519_dalek::PublicKey::from(&ephemeral_secret);
                        public_key.as_bytes().to_vec()
                    }
                    KeyType::SigningKeypair => {
                        let keypair = BitchatKeypair::generate();
                        keypair.secret_key_bytes().to_vec()
                    }
                    _ => continue, // Skip identity keys and master keys
                };

                // Re-encrypt with new salt
                let master_key = self.master_key.read().await.unwrap();
                let mut new_salt = [0u8; 32];
                OsRng.fill_bytes(&mut new_salt);
                
                entry.encrypted_data = self.encrypt_key_data(&master_key, &new_key_data, &new_salt)?;
                entry.salt = new_salt;
                entry.metadata.version += 1;

                // Persist
                self.save_key_to_storage(entry).await?;
            }
        }

        self.stats.write().await.keys_rotated += keys_count as u64;

        log::info!("Rotated {} keys for peer {:?}", keys_count, peer_id);
        Ok(())
    }

    /// List all stored keys with metadata
    pub async fn list_keys(&self) -> Vec<(String, KeyMetadata)> {
        let cache = self.key_cache.read().await;
        cache
            .iter()
            .map(|(key_id, entry)| (key_id.clone(), entry.metadata.clone()))
            .collect()
    }

    /// Remove a key from the keystore
    pub async fn remove_key(&self, key_id: &str) -> Result<()> {
        // Remove from cache
        self.key_cache.write().await.remove(key_id);

        // Remove from storage
        let file_path = self.storage_path.join(format!("{}.key", key_id));
        if file_path.exists() {
            fs::remove_file(file_path).await
                .map_err(|e| Error::IoError(format!("Failed to remove key file: {}", e)))?;
        }

        log::debug!("Removed key: {}", key_id);
        Ok(())
    }

    /// Create encrypted backup of keystore
    pub async fn create_backup(&self, backup_path: &Path, backup_password: &str) -> Result<()> {
        if !self.config.enable_backup_encryption {
            return Err(Error::Crypto("Backup encryption is disabled".to_string()));
        }

        let cache = self.key_cache.read().await;
        let backup_data = serde_json::to_vec(&*cache)
            .map_err(|e| Error::Serialization(format!("Backup serialization failed: {}", e)))?;
        drop(cache);

        // Encrypt backup
        let backup_key = KeyDerivation::derive_key_pbkdf2(
            backup_password.as_bytes(),
            b"bitcraps_backup_salt_v1",
            100_000,
            32,
        )?;

        let cipher = ChaCha20Poly1305::new(chacha20poly1305::Key::from_slice(&backup_key[..32]));
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let encrypted_backup = cipher.encrypt(nonce, backup_data.as_slice())
            .map_err(|_| Error::Crypto("Backup encryption failed".to_string()))?;

        // Write backup file
        let mut final_backup = nonce_bytes.to_vec();
        final_backup.extend_from_slice(&encrypted_backup);

        fs::write(backup_path, final_backup).await
            .map_err(|e| Error::IoError(format!("Failed to write backup file: {}", e)))?;

        // Update stats
        self.stats.write().await.last_backup_time = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        );

        log::info!("Created encrypted backup at: {}", backup_path.display());
        Ok(())
    }

    /// Restore keystore from encrypted backup
    pub async fn restore_backup(&self, backup_path: &Path, backup_password: &str) -> Result<()> {
        let backup_data = fs::read(backup_path).await
            .map_err(|e| Error::IoError(format!("Failed to read backup file: {}", e)))?;

        if backup_data.len() < 12 {
            return Err(Error::Crypto("Invalid backup file format".to_string()));
        }

        // Extract nonce and ciphertext
        let nonce = Nonce::from_slice(&backup_data[..12]);
        let ciphertext = &backup_data[12..];

        // Decrypt backup
        let backup_key = KeyDerivation::derive_key_pbkdf2(
            backup_password.as_bytes(),
            b"bitcraps_backup_salt_v1",
            100_000,
            32,
        )?;

        let cipher = ChaCha20Poly1305::new(chacha20poly1305::Key::from_slice(&backup_key[..32]));
        let decrypted_data = cipher.decrypt(nonce, ciphertext)
            .map_err(|_| Error::Crypto("Backup decryption failed".to_string()))?;

        // Deserialize keystore data
        let restored_cache: HashMap<String, KeystoreEntry> = serde_json::from_slice(&decrypted_data)
            .map_err(|e| Error::Serialization(format!("Backup deserialization failed: {}", e)))?;

        // Replace current cache
        *self.key_cache.write().await = restored_cache;

        // Save all keys to storage
        let cache = self.key_cache.read().await;
        for entry in cache.values() {
            self.save_key_to_storage(entry).await?;
        }

        log::info!("Restored keystore from backup: {} keys", cache.len());
        Ok(())
    }

    /// Get keystore statistics
    pub async fn get_stats(&self) -> KeystoreStats {
        self.stats.read().await.clone()
    }

    /// Lock the keystore (clear master key from memory)
    pub async fn lock(&self) {
        *self.master_key.write().await = None;
        self.key_cache.write().await.clear();
        log::info!("Keystore locked");
    }

    // Private helper methods

    /// Encrypt key data using master key
    fn encrypt_key_data(&self, master_key: &[u8; 32], data: &[u8], salt: &[u8; 32]) -> Result<Vec<u8>> {
        // Derive encryption key from master key + salt
        let encryption_key = KeyDerivation::derive_key_simple(master_key, salt, 32);

        let cipher = ChaCha20Poly1305::new(chacha20poly1305::Key::from_slice(&encryption_key[..32]));
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher.encrypt(nonce, data)
            .map_err(|_| Error::Crypto("Key encryption failed".to_string()))?;

        let mut encrypted_data = nonce_bytes.to_vec();
        encrypted_data.extend_from_slice(&ciphertext);

        // Note: stats update removed from sync function

        Ok(encrypted_data)
    }

    /// Decrypt key data using master key
    fn decrypt_key_data(&self, master_key: &[u8; 32], encrypted_data: &[u8], salt: &[u8; 32]) -> Result<Vec<u8>> {
        if encrypted_data.len() < 12 {
            return Err(Error::Crypto("Invalid encrypted data format".to_string()));
        }

        // Derive decryption key
        let decryption_key = KeyDerivation::derive_key_simple(master_key, salt, 32);

        let cipher = ChaCha20Poly1305::new(chacha20poly1305::Key::from_slice(&decryption_key[..32]));
        let nonce = Nonce::from_slice(&encrypted_data[..12]);
        let ciphertext = &encrypted_data[12..];

        let plaintext = cipher.decrypt(nonce, ciphertext)
            .map_err(|_| Error::Crypto("Key decryption failed".to_string()))?;

        // Note: Can't update stats here due to borrowing issues in real implementation
        
        Ok(plaintext)
    }

    /// Load all keys from storage into cache
    async fn load_keys_from_storage(&self) -> Result<()> {
        let mut entries = fs::read_dir(&self.storage_path).await
            .map_err(|e| Error::IoError(format!("Failed to read keystore directory: {}", e)))?;

        let mut loaded_count = 0;

        while let Some(entry) = entries.next_entry().await
            .map_err(|e| Error::IoError(format!("Failed to iterate keystore directory: {}", e)))?
        {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("key") {
                if let Some(key_id) = path.file_stem().and_then(|s| s.to_str()) {
                    match self.load_key_from_storage(key_id).await {
                        Ok(entry) => {
                            self.key_cache.write().await.insert(key_id.to_string(), entry);
                            loaded_count += 1;
                        }
                        Err(e) => {
                            log::warn!("Failed to load key {}: {}", key_id, e);
                        }
                    }
                }
            }
        }

        log::debug!("Loaded {} keys from storage", loaded_count);
        Ok(())
    }

    /// Load a single key from storage
    async fn load_key_from_storage(&self, key_id: &str) -> Result<KeystoreEntry> {
        let file_path = self.storage_path.join(format!("{}.key", key_id));
        let data = fs::read(file_path).await
            .map_err(|e| Error::IoError(format!("Failed to read key file: {}", e)))?;

        let entry: KeystoreEntry = serde_json::from_slice(&data)
            .map_err(|e| Error::Serialization(format!("Key deserialization failed: {}", e)))?;

        Ok(entry)
    }

    /// Save a key to storage
    async fn save_key_to_storage(&self, entry: &KeystoreEntry) -> Result<()> {
        let file_path = self.storage_path.join(format!("{}.key", entry.key_id));
        let data = serde_json::to_vec(entry)
            .map_err(|e| Error::Serialization(format!("Key serialization failed: {}", e)))?;

        fs::write(file_path, data).await
            .map_err(|e| Error::IoError(format!("Failed to write key file: {}", e)))?;

        Ok(())
    }

    /// Start background tasks for maintenance
    fn start_background_tasks(&self) {
        if self.config.auto_save_interval > 0 {
            self.start_auto_save_task();
        }
    }

    /// Start periodic auto-save task
    fn start_auto_save_task(&self) {
        let key_cache = self.key_cache.clone();
        let storage_path = self.storage_path.clone();
        let interval = self.config.auto_save_interval;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(interval));
            loop {
                interval.tick().await;
                
                let cache = key_cache.read().await;
                for entry in cache.values() {
                    let file_path = storage_path.join(format!("{}.key", entry.key_id));
                    if let Ok(data) = serde_json::to_vec(entry) {
                        let _ = fs::write(file_path, data).await;
                    }
                }
                
                log::trace!("Auto-saved {} keys to storage", cache.len());
            }
        });
    }
}

/// HSM interface for future hardware security module integration
pub trait HsmInterface: Send + Sync {
    /// Generate key pair in HSM
    fn generate_keypair(&self, key_type: KeyType) -> Result<String>;
    
    /// Sign data using HSM key
    fn sign(&self, key_id: &str, data: &[u8]) -> Result<Vec<u8>>;
    
    /// Encrypt data using HSM key
    fn encrypt(&self, key_id: &str, plaintext: &[u8]) -> Result<Vec<u8>>;
    
    /// Decrypt data using HSM key
    fn decrypt(&self, key_id: &str, ciphertext: &[u8]) -> Result<Vec<u8>>;
    
    /// List available keys in HSM
    fn list_keys(&self) -> Result<Vec<String>>;
}

/// Mock HSM implementation for development
pub struct MockHsm;

impl HsmInterface for MockHsm {
    fn generate_keypair(&self, _key_type: KeyType) -> Result<String> {
        Ok(format!("hsm_key_{}", uuid::Uuid::new_v4()))
    }
    
    fn sign(&self, _key_id: &str, data: &[u8]) -> Result<Vec<u8>> {
        // Mock signature - just hash the data
        Ok(GameCrypto::hash(data).to_vec())
    }
    
    fn encrypt(&self, _key_id: &str, plaintext: &[u8]) -> Result<Vec<u8>> {
        // Mock encryption - just return plaintext for now
        Ok(plaintext.to_vec())
    }
    
    fn decrypt(&self, _key_id: &str, ciphertext: &[u8]) -> Result<Vec<u8>> {
        // Mock decryption - just return ciphertext for now
        Ok(ciphertext.to_vec())
    }
    
    fn list_keys(&self) -> Result<Vec<String>> {
        Ok(vec!["mock_key_1".to_string(), "mock_key_2".to_string()])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    async fn create_test_keystore() -> (SecureTransportKeystore, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let keystore = SecureTransportKeystore::new(temp_dir.path()).await.unwrap();
        keystore.initialize("test_password_123").await.unwrap();
        (keystore, temp_dir)
    }
    
    #[tokio::test]
    async fn test_keystore_basic_operations() {
        let (keystore, _temp) = create_test_keystore().await;
        
        // Store a key
        let test_key = b"test_symmetric_key_32_bytes_long";
        keystore.store_key(
            "test_key",
            test_key,
            KeyType::SymmetricKey,
            "Test symmetric key",
            None,
        ).await.unwrap();
        
        // Retrieve the key
        let retrieved = keystore.retrieve_key("test_key").await.unwrap();
        assert_eq!(retrieved.as_slice(), test_key);
    }
    
    #[tokio::test]
    async fn test_identity_storage() {
        let (keystore, _temp) = create_test_keystore().await;
        
        // Create test identity
        let identity = BitchatIdentity::generate_with_pow(8);
        
        // Store identity
        keystore.store_identity(&identity, "test_identity").await.unwrap();
        
        // Retrieve identity
        let retrieved = keystore.retrieve_identity("test_identity").await.unwrap();
        
        // Verify they match
        assert_eq!(identity.peer_id, retrieved.peer_id);
        assert_eq!(identity.pow_nonce, retrieved.pow_nonce);
        assert_eq!(identity.pow_difficulty, retrieved.pow_difficulty);
    }
    
    #[tokio::test]
    async fn test_keystore_backup_restore() {
        let (keystore, temp) = create_test_keystore().await;
        
        // Store some keys
        let test_key = b"test_backup_key_32_bytes_long!!";
        keystore.store_key(
            "backup_test",
            test_key,
            KeyType::SymmetricKey,
            "Backup test key",
            None,
        ).await.unwrap();
        
        // Create backup
        let backup_path = temp.path().join("keystore_backup.enc");
        keystore.create_backup(&backup_path, "backup_password").await.unwrap();
        
        // Clear keystore
        keystore.lock().await;
        keystore.initialize("test_password_123").await.unwrap();
        keystore.remove_key("backup_test").await.unwrap();
        
        // Verify key is gone
        assert!(keystore.retrieve_key("backup_test").await.is_err());
        
        // Restore from backup
        keystore.restore_backup(&backup_path, "backup_password").await.unwrap();
        
        // Verify key is back
        let retrieved = keystore.retrieve_key("backup_test").await.unwrap();
        assert_eq!(retrieved.as_slice(), test_key);
    }
    
    #[tokio::test]
    async fn test_key_rotation() {
        let (keystore, _temp) = create_test_keystore().await;
        let peer_id = [42u8; 32];
        
        // Store a session key
        let session_key = keystore.generate_session_key(peer_id).await.unwrap();
        
        // Retrieve original key
        let key_id = format!("session_{}", hex::encode(peer_id));
        let original = keystore.retrieve_key(&key_id).await.unwrap();
        
        // Rotate keys
        keystore.rotate_peer_keys(peer_id).await.unwrap();
        
        // Retrieve rotated key
        let rotated = keystore.retrieve_key(&key_id).await.unwrap();
        
        // Keys should be different
        assert_ne!(original.as_slice(), rotated.as_slice());
    }
}