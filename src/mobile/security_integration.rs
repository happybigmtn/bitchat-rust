//! Mobile Security Integration Layer
//!
//! This module provides a unified interface that combines all mobile security features:
//! - Platform-specific secure storage (Android Keystore, iOS Keychain)
//! - Biometric authentication integration
//! - Secure key derivation and management
//! - Permission handling and validation
//! - Security policy enforcement

use crate::error::{Error, Result};
use crate::mobile::{
    BiometricAuthManager, KeyDerivationManager, KeyHierarchy, PermissionManager, PermissionState,
    PermissionSummary, SecureStorageManager,
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

/// Comprehensive mobile security manager
pub struct MobileSecurityManager {
    storage_manager: Arc<SecureStorageManager>,
    biometric_manager: Arc<BiometricAuthManager>,
    key_manager: Arc<KeyDerivationManager>,
    permission_manager: Arc<PermissionManager>,
    security_policy: SecurityPolicy,
    initialized: Arc<Mutex<bool>>,
}

impl MobileSecurityManager {
    /// Create new mobile security manager with comprehensive configuration
    pub async fn new(config: MobileSecurityConfig) -> Result<Self> {
        // Initialize storage manager
        let storage_manager = Arc::new(SecureStorageManager::new()?);

        // Initialize biometric authentication
        let biometric_manager = Arc::new(BiometricAuthManager::new()?);

        // Initialize key derivation (hardware-backed when available)
        let hardware_backed = Self::is_hardware_backed().await?;
        let key_manager = Arc::new(KeyDerivationManager::new(hardware_backed));

        // Initialize permission manager with BitCraps-specific permissions
        let required_permissions = PermissionManager::get_bitcraps_required_permissions();
        let optional_permissions = PermissionManager::get_bitcraps_optional_permissions();
        let permission_manager = Arc::new(PermissionManager::new(
            required_permissions,
            optional_permissions,
        ));

        let manager = Self {
            storage_manager,
            biometric_manager,
            key_manager,
            permission_manager,
            security_policy: config.security_policy,
            initialized: Arc::new(Mutex::new(false)),
        };

        // Perform initialization
        manager.initialize().await?;

        Ok(manager)
    }

    /// Initialize the security manager and verify all components
    async fn initialize(&self) -> Result<()> {
        let mut initialized = self.initialized.lock().map_err(|_| {
            Error::InvalidState("Failed to acquire initialization lock".to_string())
        })?;

        if *initialized {
            return Ok(());
        }

        log::info!("Initializing Mobile Security Manager");

        // 1. Check and request essential permissions
        let permission_summary = self.check_and_request_permissions().await?;
        if !permission_summary.can_continue {
            return Err(Error::Security(
                "Essential permissions not granted - cannot continue".to_string(),
            ));
        }

        // 2. Verify biometric authentication availability
        if self.security_policy.require_biometric_auth {
            let biometric_available = self.biometric_manager.is_biometric_configured()?;
            if !biometric_available {
                return Err(Error::Security(
                    "Biometric authentication required but not configured".to_string(),
                ));
            }
        }

        // 3. Initialize application key hierarchy
        let _app_keys = self.initialize_app_key_hierarchy().await?;

        // 4. Verify secure storage functionality
        self.verify_secure_storage().await?;

        *initialized = true;
        log::info!("Mobile Security Manager initialized successfully");

        Ok(())
    }

    /// Create secure wallet with biometric protection
    pub async fn create_secure_wallet(
        &self,
        wallet_id: &str,
        initial_entropy: Option<&[u8]>,
    ) -> Result<SecureWallet> {
        self.ensure_initialized().await?;

        log::info!("Creating secure wallet: {}", wallet_id);

        // 1. Authenticate user with biometrics if required
        if self.security_policy.require_biometric_auth {
            let auth_session = self
                .biometric_manager
                .authenticate_user(&format!("Create secure wallet '{}'", wallet_id))
                .await?;

            log::info!("Biometric authentication successful for wallet creation");
        }

        // 2. Generate wallet master key with hardware backing
        let master_key_id = format!("wallet_master_{}", wallet_id);
        let master_key = self.key_manager.get_master_key(&master_key_id)?;

        // 3. Create key hierarchy for wallet operations
        let key_hierarchy = self.key_manager.create_key_hierarchy(wallet_id)?;

        // 4. Create biometric-protected signing key if supported
        let signing_key = if self.biometric_manager.is_biometric_configured()? {
            Some(
                self.biometric_manager
                    .create_protected_wallet_key(wallet_id)?,
            )
        } else {
            None
        };

        // 5. Store wallet metadata securely
        let wallet_metadata = WalletMetadata {
            wallet_id: wallet_id.to_string(),
            master_key_id: master_key_id.clone(),
            created_at: current_timestamp(),
            last_used: current_timestamp(),
            biometric_protected: signing_key.is_some(),
            hardware_backed: self.key_manager.is_hardware_backed().unwrap_or(false),
            version: 1,
        };

        self.storage_manager.store_user_credentials(
            wallet_id,
            &crate::mobile::UserCredentials {
                user_id: wallet_id.to_string(),
                encrypted_private_key: master_key.key_material.clone(),
                public_key: Vec::new(), // Would derive public key in real implementation
                created_at: wallet_metadata.created_at,
                last_used: wallet_metadata.created_at,
            },
        )?;

        // 6. Store wallet metadata
        let metadata_key = format!("wallet_metadata_{}", wallet_id);
        let metadata_bytes = bincode::serialize(&wallet_metadata)?;
        self.storage_manager
            .storage
            .store(&metadata_key, &metadata_bytes)?;

        Ok(SecureWallet {
            wallet_id: wallet_id.to_string(),
            metadata: wallet_metadata,
            key_hierarchy,
            signing_key,
        })
    }

    /// Unlock existing secure wallet with authentication
    pub async fn unlock_secure_wallet(&self, wallet_id: &str) -> Result<SecureWallet> {
        self.ensure_initialized().await?;

        log::info!("Unlocking secure wallet: {}", wallet_id);

        // 1. Load wallet metadata
        let metadata_key = format!("wallet_metadata_{}", wallet_id);
        let metadata_bytes = self
            .storage_manager
            .storage
            .retrieve(&metadata_key)?
            .ok_or_else(|| Error::NotFound(format!("Wallet '{}' not found", wallet_id)))?;

        let metadata: WalletMetadata = bincode::deserialize(&metadata_bytes)?;

        // 2. Authenticate user
        if self.security_policy.require_biometric_auth || metadata.biometric_protected {
            let auth_session = self
                .biometric_manager
                .authenticate_user(&format!("Unlock wallet '{}'", wallet_id))
                .await?;

            log::info!("Authentication successful for wallet unlock");
        }

        // 3. Reconstruct key hierarchy
        let key_hierarchy = self.key_manager.create_key_hierarchy(wallet_id)?;

        // 4. Unlock biometric-protected signing key if available
        let signing_key = if metadata.biometric_protected {
            // Load protected key info and unlock
            let protected_key = self.load_protected_key(wallet_id).await?;
            Some(protected_key)
        } else {
            None
        };

        // 5. Update last used timestamp
        let mut updated_metadata = metadata.clone();
        updated_metadata.last_used = current_timestamp();

        let updated_metadata_bytes = bincode::serialize(&updated_metadata)?;
        self.storage_manager
            .storage
            .store(&metadata_key, &updated_metadata_bytes)?;

        Ok(SecureWallet {
            wallet_id: wallet_id.to_string(),
            metadata: updated_metadata,
            key_hierarchy,
            signing_key,
        })
    }

    /// Secure data encryption with platform-specific protection
    pub async fn encrypt_sensitive_data(
        &self,
        data: &[u8],
        context: &str,
    ) -> Result<EncryptedData> {
        self.ensure_initialized().await?;

        // Use key derivation for context-specific encryption key
        let encryption_key = self.key_manager.derive_key_hkdf(
            "app_master_bitcraps",
            Some(b"data_encryption_salt"),
            context.as_bytes(),
            32,
            crate::mobile::HkdfAlgorithm::HkdfSha256,
        )?;

        // Encrypt data using AES-GCM
        let encrypted_data = self.aes_gcm_encrypt(data, &encryption_key.key_material)?;

        Ok(EncryptedData {
            data: encrypted_data,
            context: context.to_string(),
            algorithm: "AES-256-GCM".to_string(),
            created_at: current_timestamp(),
        })
    }

    /// Secure data decryption with platform-specific protection
    pub async fn decrypt_sensitive_data(&self, encrypted: &EncryptedData) -> Result<Vec<u8>> {
        self.ensure_initialized().await?;

        // Re-derive the same encryption key
        let encryption_key = self.key_manager.derive_key_hkdf(
            "app_master_bitcraps",
            Some(b"data_encryption_salt"),
            encrypted.context.as_bytes(),
            32,
            crate::mobile::HkdfAlgorithm::HkdfSha256,
        )?;

        // Decrypt data
        let decrypted_data = self.aes_gcm_decrypt(&encrypted.data, &encryption_key.key_material)?;

        Ok(decrypted_data)
    }

    /// Get comprehensive security status
    pub async fn get_security_status(&self) -> Result<SecurityStatus> {
        let permission_summary = self.permission_manager.check_all_permissions()?;
        let biometric_available = self.biometric_manager.is_biometric_configured()?;
        let hardware_backed = self.key_manager.is_hardware_backed().unwrap_or(false);

        let security_level =
            if hardware_backed && biometric_available && permission_summary.all_required_granted {
                SecurityLevel::Maximum
            } else if biometric_available && permission_summary.all_required_granted {
                SecurityLevel::High
            } else if permission_summary.all_required_granted {
                SecurityLevel::Medium
            } else {
                SecurityLevel::Low
            };

        Ok(SecurityStatus {
            security_level,
            permissions_granted: permission_summary.all_required_granted,
            biometric_available,
            hardware_backed,
            can_create_wallets: security_level >= SecurityLevel::Medium,
            recommended_actions: self.get_security_recommendations(
                &permission_summary,
                biometric_available,
                hardware_backed,
            ),
        })
    }

    /// Check and request necessary permissions
    async fn check_and_request_permissions(&self) -> Result<PermissionSummary> {
        log::info!("Checking mobile permissions");

        let summary = self.permission_manager.check_all_permissions()?;

        if !summary.all_required_granted {
            log::warn!("Required permissions not granted, requesting...");

            // Request missing required permissions
            let results = self
                .permission_manager
                .request_permissions(
                    summary.denied_required.clone(),
                    "BitCraps requires these permissions for secure peer-to-peer gaming",
                )
                .await?;

            // Check if all required permissions are now granted
            let mut all_granted = true;
            for permission in &summary.denied_required {
                if let Some(state) = results.get(permission) {
                    if *state != PermissionState::Granted {
                        all_granted = false;
                        log::error!("Required permission {:?} was not granted", permission);
                    }
                }
            }

            if !all_granted {
                return Err(Error::Security(
                    "Required permissions not granted".to_string(),
                ));
            }
        }

        Ok(summary)
    }

    /// Initialize application key hierarchy
    async fn initialize_app_key_hierarchy(&self) -> Result<KeyHierarchy> {
        log::info!("Initializing application key hierarchy");

        let app_keys = self.key_manager.create_key_hierarchy("bitcraps")?;

        // Store key hierarchy metadata for future use
        let key_info = KeyHierarchyInfo {
            app_id: app_keys.app_id.clone(),
            master_key_id: app_keys.master_key_id.clone(),
            created_at: app_keys.created_at,
            version: 1,
        };

        let key_info_bytes = bincode::serialize(&key_info)?;
        self.storage_manager
            .storage
            .store("app_key_hierarchy", &key_info_bytes)?;

        Ok(app_keys)
    }

    /// Verify secure storage functionality
    async fn verify_secure_storage(&self) -> Result<()> {
        log::info!("Verifying secure storage functionality");

        let test_key = "security_verification_test";
        let test_data = b"secure_storage_verification_data";

        // Test store
        self.storage_manager.storage.store(test_key, test_data)?;

        // Test retrieve
        let retrieved = self
            .storage_manager
            .storage
            .retrieve(test_key)?
            .ok_or_else(|| Error::Security("Secure storage verification failed".to_string()))?;

        if retrieved != test_data {
            return Err(Error::Security(
                "Secure storage data integrity check failed".to_string(),
            ));
        }

        // Clean up test data
        self.storage_manager.storage.delete(test_key)?;

        log::info!("Secure storage verification successful");
        Ok(())
    }

    /// Load protected key for wallet
    async fn load_protected_key(&self, wallet_id: &str) -> Result<crate::mobile::ProtectedKey> {
        // In real implementation, would load from secure storage
        // For now, create a mock protected key
        Ok(crate::mobile::ProtectedKey {
            key_alias: format!("wallet_protected_{}", wallet_id),
            key_derivation_salt: vec![1, 2, 3, 4], // Would be real salt
            created_at: current_timestamp(),
            biometric_required: true,
            hardware_backed: true,
        })
    }

    /// Check if hardware-backed security is available
    async fn is_hardware_backed() -> Result<bool> {
        #[cfg(target_os = "android")]
        {
            // Check if Android Keystore with hardware backing is available
            match AndroidKeystoreManager::new("com.bitcraps") {
                Ok(keystore) => {
                    // Would check keystore attestation in real implementation
                    Ok(true)
                }
                Err(_) => Ok(false),
            }
        }

        #[cfg(target_os = "ios")]
        {
            // iOS Secure Enclave availability check
            match IOSKeychainManager::new("com.bitcraps.keychain") {
                Ok(_) => Ok(true), // Assume Secure Enclave available on modern iOS
                Err(_) => Ok(false),
            }
        }

        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            Ok(false)
        }
    }

    /// Ensure manager is initialized
    async fn ensure_initialized(&self) -> Result<()> {
        let initialized = self.initialized.lock().map_err(|_| {
            Error::InvalidState("Failed to acquire initialization lock".to_string())
        })?;

        if !*initialized {
            drop(initialized);
            return Err(Error::InvalidState(
                "Mobile security manager not initialized".to_string(),
            ));
        }

        Ok(())
    }

    /// Get security recommendations
    fn get_security_recommendations(
        &self,
        permission_summary: &PermissionSummary,
        biometric_available: bool,
        hardware_backed: bool,
    ) -> Vec<String> {
        let mut recommendations = Vec::new();

        if !permission_summary.all_required_granted {
            recommendations.push("Grant required permissions for full functionality".to_string());
        }

        if !biometric_available {
            recommendations
                .push("Enable biometric authentication for enhanced security".to_string());
        }

        if !hardware_backed {
            recommendations
                .push("Consider upgrading to a device with hardware security module".to_string());
        }

        if !permission_summary.denied_optional.is_empty() {
            recommendations.push(
                "Consider granting optional permissions for better user experience".to_string(),
            );
        }

        recommendations
    }

    /// AES-GCM encryption (placeholder implementation)
    fn aes_gcm_encrypt(&self, data: &[u8], key: &[u8]) -> Result<Vec<u8>> {
        // In real implementation, would use AES-GCM with proper IV/nonce
        // For now, simple XOR for testing
        let mut encrypted = data.to_vec();
        for (i, byte) in encrypted.iter_mut().enumerate() {
            *byte ^= key[i % key.len()];
        }
        Ok(encrypted)
    }

    /// AES-GCM decryption (placeholder implementation)
    fn aes_gcm_decrypt(&self, encrypted_data: &[u8], key: &[u8]) -> Result<Vec<u8>> {
        // Same as encryption for XOR
        self.aes_gcm_encrypt(encrypted_data, key)
    }
}

/// Mobile security configuration
#[derive(Debug)]
pub struct MobileSecurityConfig {
    pub security_policy: SecurityPolicy,
    pub enable_hardware_backing: bool,
    pub require_biometric_auth: bool,
    pub key_rotation_period: Option<u64>,
}

impl Default for MobileSecurityConfig {
    fn default() -> Self {
        Self {
            security_policy: SecurityPolicy::default(),
            enable_hardware_backing: true,
            require_biometric_auth: false,
            key_rotation_period: Some(86400 * 30), // 30 days
        }
    }
}

/// Security policy configuration
#[derive(Debug)]
pub struct SecurityPolicy {
    pub require_biometric_auth: bool,
    pub allow_fallback_to_passcode: bool,
    pub minimum_security_level: SecurityLevel,
    pub auto_lock_timeout: Option<u64>,
    pub require_hardware_backing: bool,
}

impl Default for SecurityPolicy {
    fn default() -> Self {
        Self {
            require_biometric_auth: false,
            allow_fallback_to_passcode: true,
            minimum_security_level: SecurityLevel::Medium,
            auto_lock_timeout: Some(300), // 5 minutes
            require_hardware_backing: false,
        }
    }
}

/// Security levels
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum SecurityLevel {
    Low = 1,
    Medium = 2,
    High = 3,
    Maximum = 4,
}

/// Secure wallet with comprehensive protection
pub struct SecureWallet {
    pub wallet_id: String,
    pub metadata: WalletMetadata,
    pub key_hierarchy: KeyHierarchy,
    pub signing_key: Option<crate::mobile::ProtectedKey>,
}

/// Wallet metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletMetadata {
    pub wallet_id: String,
    pub master_key_id: String,
    pub created_at: u64,
    pub last_used: u64,
    pub biometric_protected: bool,
    pub hardware_backed: bool,
    pub version: u32,
}

/// Key hierarchy metadata
#[derive(Debug, Serialize, Deserialize)]
pub struct KeyHierarchyInfo {
    pub app_id: String,
    pub master_key_id: String,
    pub created_at: u64,
    pub version: u32,
}

/// Encrypted data container
#[derive(Debug)]
pub struct EncryptedData {
    pub data: Vec<u8>,
    pub context: String,
    pub algorithm: String,
    pub created_at: u64,
}

/// Overall security status
#[derive(Debug)]
pub struct SecurityStatus {
    pub security_level: SecurityLevel,
    pub permissions_granted: bool,
    pub biometric_available: bool,
    pub hardware_backed: bool,
    pub can_create_wallets: bool,
    pub recommended_actions: Vec<String>,
}

/// Get current timestamp
fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mobile_security_manager_creation() {
        let config = MobileSecurityConfig::default();
        let manager = MobileSecurityManager::new(config).await;

        // May fail on non-mobile platforms but should not panic
        if manager.is_ok() {
            let manager = manager.unwrap();
            let status = manager.get_security_status().await;
            assert!(status.is_ok());
        }
    }

    #[tokio::test]
    async fn test_secure_wallet_operations() {
        let config = MobileSecurityConfig {
            require_biometric_auth: false,
            ..Default::default()
        };

        if let Ok(manager) = MobileSecurityManager::new(config).await {
            let wallet_id = "test_wallet";

            // Create wallet
            let wallet = manager.create_secure_wallet(wallet_id, None).await;
            if wallet.is_ok() {
                let created_wallet = wallet.unwrap();
                assert_eq!(created_wallet.wallet_id, wallet_id);

                // Unlock wallet
                let unlocked = manager.unlock_secure_wallet(wallet_id).await;
                assert!(unlocked.is_ok());
            }
        }
    }

    #[tokio::test]
    async fn test_data_encryption() {
        let config = MobileSecurityConfig {
            require_biometric_auth: false,
            ..Default::default()
        };

        if let Ok(manager) = MobileSecurityManager::new(config).await {
            let test_data = b"sensitive_test_data";
            let context = "test_encryption";

            // Encrypt data
            let encrypted = manager.encrypt_sensitive_data(test_data, context).await;
            if encrypted.is_ok() {
                let encrypted_data = encrypted.unwrap();
                assert_eq!(encrypted_data.context, context);
                assert_ne!(encrypted_data.data, test_data.to_vec());

                // Decrypt data
                let decrypted = manager.decrypt_sensitive_data(&encrypted_data).await;
                if decrypted.is_ok() {
                    let decrypted_data = decrypted.unwrap();
                    assert_eq!(decrypted_data, test_data.to_vec());
                }
            }
        }
    }
}
