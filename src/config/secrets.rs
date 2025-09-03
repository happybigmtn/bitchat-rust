//! Production Secrets Management
//!
//! Secure handling of sensitive configuration data including API keys,
//! database credentials, and cryptographic keys.

use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce, Key
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};

use crate::error::{Error, Result};

/// Secrets manager for secure configuration
pub struct SecretsManager {
    provider: Box<dyn SecretsProvider>,
    cache: HashMap<String, SecretValue>,
    encryption_key: Option<[u8; 32]>,
}

/// Secret value with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretValue {
    pub value: String,
    pub expires_at: Option<u64>,
    pub rotation_version: u32,
    pub encrypted: bool,
}

/// Secrets provider trait for different backends
pub trait SecretsProvider: Send + Sync {
    /// Get a secret value
    fn get_secret(&self, key: &str) -> Result<Option<SecretValue>>;

    /// Store a secret value
    fn set_secret(&mut self, key: &str, value: SecretValue) -> Result<()>;

    /// Delete a secret
    fn delete_secret(&mut self, key: &str) -> Result<()>;

    /// List all secret keys
    fn list_keys(&self) -> Result<Vec<String>>;

    /// Rotate a secret
    fn rotate_secret(&mut self, key: &str) -> Result<SecretValue>;
}

/// Environment variable secrets provider
pub struct EnvSecretsProvider {
    prefix: String,
}

impl EnvSecretsProvider {
    pub fn new(prefix: &str) -> Self {
        Self {
            prefix: prefix.to_string(),
        }
    }
}

impl SecretsProvider for EnvSecretsProvider {
    fn get_secret(&self, key: &str) -> Result<Option<SecretValue>> {
        let env_key = format!("{}_{}", self.prefix, key.to_uppercase());

        match env::var(&env_key) {
            Ok(value) => Ok(Some(SecretValue {
                value,
                expires_at: None,
                rotation_version: 0,
                encrypted: false,
            })),
            Err(env::VarError::NotPresent) => Ok(None),
            Err(e) => Err(Error::Config(format!("Failed to read environment variable: {}", e))),
        }
    }

    fn set_secret(&mut self, _key: &str, _value: SecretValue) -> Result<()> {
        Err(Error::Config("Cannot set environment variables at runtime".into()))
    }

    fn delete_secret(&mut self, _key: &str) -> Result<()> {
        Err(Error::Config("Cannot delete environment variables at runtime".into()))
    }

    fn list_keys(&self) -> Result<Vec<String>> {
        let keys: Vec<String> = env::vars()
            .filter(|(k, _)| k.starts_with(&self.prefix))
            .map(|(k, _)| k.strip_prefix(&format!("{}_", self.prefix))
                .unwrap_or(&k)
                .to_string())
            .collect();
        Ok(keys)
    }

    fn rotate_secret(&mut self, _key: &str) -> Result<SecretValue> {
        Err(Error::Config("Cannot rotate environment variables".into()))
    }
}

/// File-based secrets provider with encryption
pub struct FileSecretsProvider {
    secrets_dir: PathBuf,
    encryption_key: [u8; 32],
}

impl FileSecretsProvider {
    pub fn new<P: AsRef<Path>>(secrets_dir: P, master_password: &str) -> Result<Self> {
        let secrets_dir = secrets_dir.as_ref().to_path_buf();

        // Create directory if it doesn't exist
        fs::create_dir_all(&secrets_dir)
            .map_err(|e| Error::Config(format!("Failed to create secrets directory: {}", e)))?;

        // Derive encryption key from master password
        let mut hasher = Sha256::new();
        hasher.update(master_password.as_bytes());
        hasher.update(b"bitcraps_secrets_salt");
        let key_bytes = hasher.finalize();

        let mut encryption_key = [0u8; 32];
        encryption_key.copy_from_slice(&key_bytes);

        Ok(Self {
            secrets_dir,
            encryption_key,
        })
    }

    fn secret_path(&self, key: &str) -> PathBuf {
        // Sanitize key to prevent directory traversal
        let safe_key = key.replace(['/', '\\', '.'], "_");
        self.secrets_dir.join(format!("{}.secret", safe_key))
    }

    fn encrypt_value(&self, plaintext: &str) -> Result<String> {
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.encryption_key));

        // Generate cryptographically secure random nonce
        let mut nonce_bytes = [0u8; 12];
        use rand::{RngCore, rngs::OsRng};
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Encrypt
        let ciphertext = cipher.encrypt(nonce, plaintext.as_bytes())
            .map_err(|e| Error::Config(format!("Encryption failed: {}", e)))?;

        // Combine nonce and ciphertext
        let mut combined = Vec::new();
        combined.extend_from_slice(&nonce_bytes);
        combined.extend_from_slice(&ciphertext);

        Ok(BASE64.encode(combined))
    }

    fn decrypt_value(&self, encrypted: &str) -> Result<String> {
        let combined = BASE64.decode(encrypted)
            .map_err(|e| Error::Config(format!("Base64 decode failed: {}", e)))?;

        if combined.len() < 12 {
            return Err(Error::Config("Invalid encrypted value".into()));
        }

        let (nonce_bytes, ciphertext) = combined.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);

        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.encryption_key));

        let plaintext = cipher.decrypt(nonce, ciphertext)
            .map_err(|e| Error::Config(format!("Decryption failed: {}", e)))?;

        String::from_utf8(plaintext)
            .map_err(|e| Error::Config(format!("Invalid UTF-8: {}", e)))
    }
}

impl SecretsProvider for FileSecretsProvider {
    fn get_secret(&self, key: &str) -> Result<Option<SecretValue>> {
        let path = self.secret_path(key);

        if !path.exists() {
            return Ok(None);
        }

        let encrypted_data = fs::read_to_string(&path)
            .map_err(|e| Error::Config(format!("Failed to read secret file: {}", e)))?;

        let secret: SecretValue = serde_json::from_str(&encrypted_data)
            .map_err(|e| Error::Config(format!("Failed to parse secret: {}", e)))?;

        // Decrypt if needed
        let decrypted_value = if secret.encrypted {
            self.decrypt_value(&secret.value)?
        } else {
            secret.value.clone()
        };

        Ok(Some(SecretValue {
            value: decrypted_value,
            expires_at: secret.expires_at,
            rotation_version: secret.rotation_version,
            encrypted: false,
        }))
    }

    fn set_secret(&mut self, key: &str, mut value: SecretValue) -> Result<()> {
        let path = self.secret_path(key);

        // Encrypt the value
        let encrypted_value = self.encrypt_value(&value.value)?;
        value.value = encrypted_value;
        value.encrypted = true;

        let json = serde_json::to_string_pretty(&value)
            .map_err(|e| Error::Config(format!("Failed to serialize secret: {}", e)))?;

        fs::write(&path, json)
            .map_err(|e| Error::Config(format!("Failed to write secret file: {}", e)))?;

        // Set restrictive permissions on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&path)?.permissions();
            perms.set_mode(0o600); // Read/write for owner only
            fs::set_permissions(&path, perms)?;
        }

        Ok(())
    }

    fn delete_secret(&mut self, key: &str) -> Result<()> {
        let path = self.secret_path(key);

        if path.exists() {
            fs::remove_file(&path)
                .map_err(|e| Error::Config(format!("Failed to delete secret: {}", e)))?;
        }

        Ok(())
    }

    fn list_keys(&self) -> Result<Vec<String>> {
        let mut keys = Vec::new();

        let entries = fs::read_dir(&self.secrets_dir)
            .map_err(|e| Error::Config(format!("Failed to read secrets directory: {}", e)))?;

        for entry in entries {
            let entry = entry.map_err(|e| Error::Config(format!("Failed to read directory entry: {}", e)))?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("secret") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    keys.push(stem.to_string());
                }
            }
        }

        Ok(keys)
    }

    fn rotate_secret(&mut self, key: &str) -> Result<SecretValue> {
        let mut secret = self.get_secret(key)?
            .ok_or_else(|| Error::Config(format!("Secret not found: {}", key)))?;

        // Generate new value (this would be application-specific)
        // For now, we just increment the version
        secret.rotation_version += 1;

        self.set_secret(key, secret.clone())?;

        Ok(secret)
    }
}

/// Kubernetes secrets provider
pub struct K8sSecretsProvider {
    namespace: String,
    secret_name: String,
}

impl K8sSecretsProvider {
    pub fn new(namespace: &str, secret_name: &str) -> Self {
        Self {
            namespace: namespace.to_string(),
            secret_name: secret_name.to_string(),
        }
    }

    fn secret_path(&self) -> PathBuf {
        PathBuf::from(format!("/var/run/secrets/{}/{}", self.namespace, self.secret_name))
    }
}

impl SecretsProvider for K8sSecretsProvider {
    fn get_secret(&self, key: &str) -> Result<Option<SecretValue>> {
        let secret_file = self.secret_path().join(key);

        if !secret_file.exists() {
            return Ok(None);
        }

        let value = fs::read_to_string(&secret_file)
            .map_err(|e| Error::Config(format!("Failed to read K8s secret: {}", e)))?;

        Ok(Some(SecretValue {
            value,
            expires_at: None,
            rotation_version: 0,
            encrypted: false,
        }))
    }

    fn set_secret(&mut self, _key: &str, _value: SecretValue) -> Result<()> {
        Err(Error::Config("Cannot modify K8s secrets from application".into()))
    }

    fn delete_secret(&mut self, _key: &str) -> Result<()> {
        Err(Error::Config("Cannot delete K8s secrets from application".into()))
    }

    fn list_keys(&self) -> Result<Vec<String>> {
        let secret_dir = self.secret_path();

        if !secret_dir.exists() {
            return Ok(Vec::new());
        }

        let mut keys = Vec::new();
        let entries = fs::read_dir(&secret_dir)
            .map_err(|e| Error::Config(format!("Failed to read K8s secrets: {}", e)))?;

        for entry in entries {
            let entry = entry.map_err(|e| Error::Config(format!("Failed to read entry: {}", e)))?;
            if let Some(name) = entry.file_name().to_str() {
                keys.push(name.to_string());
            }
        }

        Ok(keys)
    }

    fn rotate_secret(&mut self, _key: &str) -> Result<SecretValue> {
        Err(Error::Config("Cannot rotate K8s secrets from application".into()))
    }
}

impl SecretsManager {
    /// Create a new secrets manager with the specified provider
    pub fn new(provider: Box<dyn SecretsProvider>) -> Self {
        Self {
            provider,
            cache: HashMap::new(),
            encryption_key: None,
        }
    }

    /// Create with environment variable provider
    pub fn from_env(prefix: &str) -> Self {
        Self::new(Box::new(EnvSecretsProvider::new(prefix)))
    }

    /// Create with file-based provider
    pub fn from_file<P: AsRef<Path>>(secrets_dir: P, master_password: &str) -> Result<Self> {
        Ok(Self::new(Box::new(FileSecretsProvider::new(secrets_dir, master_password)?)))
    }

    /// Create with Kubernetes provider
    pub fn from_k8s(namespace: &str, secret_name: &str) -> Self {
        Self::new(Box::new(K8sSecretsProvider::new(namespace, secret_name)))
    }

    /// Get a secret value
    pub fn get(&mut self, key: &str) -> Result<String> {
        // Check cache first
        if let Some(cached) = self.cache.get(key) {
            if let Some(expires_at) = cached.expires_at {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();

                if now < expires_at {
                    return Ok(cached.value.clone());
                }
            } else {
                return Ok(cached.value.clone());
            }
        }

        // Fetch from provider
        let secret = self.provider.get_secret(key)?
            .ok_or_else(|| Error::Config(format!("Secret not found: {}", key)))?;

        // Cache the value
        self.cache.insert(key.to_string(), secret.clone());

        Ok(secret.value)
    }

    /// Get database connection string
    pub fn get_database_url(&mut self) -> Result<String> {
        self.get("DATABASE_URL")
    }

    /// Get API key for external service
    pub fn get_api_key(&mut self, service: &str) -> Result<String> {
        self.get(&format!("{}_API_KEY", service.to_uppercase()))
    }

    /// Get TLS certificate
    pub fn get_tls_cert(&mut self) -> Result<String> {
        self.get("TLS_CERT")
    }

    /// Get TLS private key
    pub fn get_tls_key(&mut self) -> Result<String> {
        self.get("TLS_KEY")
    }

    /// Set a secret (if provider supports it)
    pub fn set(&mut self, key: &str, value: &str) -> Result<()> {
        let secret = SecretValue {
            value: value.to_string(),
            expires_at: None,
            rotation_version: 0,
            encrypted: false,
        };

        self.provider.set_secret(key, secret)?;
        self.cache.remove(key); // Invalidate cache

        Ok(())
    }

    /// Rotate a secret
    pub fn rotate(&mut self, key: &str) -> Result<()> {
        self.provider.rotate_secret(key)?;
        self.cache.remove(key); // Invalidate cache
        Ok(())
    }

    /// Clear cache
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }
}

/// Production secrets configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionSecrets {
    pub database_url: String,
    pub redis_url: Option<String>,
    pub jwt_secret: String,
    pub api_keys: HashMap<String, String>,
    pub tls_cert_path: Option<String>,
    pub tls_key_path: Option<String>,
    pub master_seed: String,
    pub encryption_key: String,
}

impl ProductionSecrets {
    /// Load from secrets manager
    pub fn load(secrets: &mut SecretsManager) -> Result<Self> {
        Ok(Self {
            database_url: secrets.get_database_url()?,
            redis_url: secrets.get("REDIS_URL").ok(),
            jwt_secret: secrets.get("JWT_SECRET")?,
            api_keys: HashMap::new(), // Would load specific keys
            tls_cert_path: secrets.get("TLS_CERT_PATH").ok(),
            tls_key_path: secrets.get("TLS_KEY_PATH").ok(),
            master_seed: secrets.get("MASTER_SEED")?,
            encryption_key: secrets.get("ENCRYPTION_KEY")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_env_secrets_provider() {
        env::set_var("TEST_SECRET_KEY", "secret_value");

        let provider = EnvSecretsProvider::new("TEST");
        let secret = provider.get_secret("SECRET_KEY").unwrap().unwrap();

        assert_eq!(secret.value, "secret_value");

        env::remove_var("TEST_SECRET_KEY");
    }

    #[test]
    fn test_file_secrets_provider() {
        let temp_dir = TempDir::new().unwrap();
        let mut provider = FileSecretsProvider::new(temp_dir.path(), "test_password").unwrap();

        let secret = SecretValue {
            value: "test_secret".to_string(),
            expires_at: None,
            rotation_version: 1,
            encrypted: false,
        };

        provider.set_secret("test_key", secret).unwrap();
        let retrieved = provider.get_secret("test_key").unwrap().unwrap();

        assert_eq!(retrieved.value, "test_secret");
        assert_eq!(retrieved.rotation_version, 1);
    }
}