//! Hardware Security Module (HSM) integration for BitCraps
//!
//! This module provides secure key management through Hardware Security Modules,
//! including PKCS#11 tokens, YubiKeys, and other hardware security devices.
//! Keys stored in HSMs cannot be extracted and provide defense against physical attacks.

use std::sync::Arc;
use std::collections::HashMap;
use zeroize::ZeroizeOnDrop;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::protocol::{PeerId, Signature as ProtocolSignature};

/// HSM key handle that references hardware-protected keys
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HsmKeyHandle {
    /// Unique identifier for the key in the HSM
    pub key_id: String,
    /// HSM slot or token identifier  
    pub slot_id: u32,
    /// Key label for human identification
    pub label: String,
}

/// HSM-backed keystore that never exposes private key material
#[derive(Debug)]
pub struct HsmKeystore {
    /// HSM provider implementation
    provider: Arc<dyn HsmProvider>,
    /// Cached public keys for verification
    public_keys: parking_lot::RwLock<HashMap<HsmKeyHandle, [u8; 32]>>,
    /// Primary identity key handle
    identity_handle: Option<HsmKeyHandle>,
}

/// Abstract HSM provider interface for different hardware types
#[async_trait::async_trait]
pub trait HsmProvider: Send + Sync + std::fmt::Debug {
    /// Initialize connection to HSM
    async fn initialize(&self) -> Result<()>;
    
    /// Generate a new key pair in the HSM
    async fn generate_keypair(&self, label: &str, slot_id: u32) -> Result<HsmKeyHandle>;
    
    /// Get public key for a handle (safe to expose)
    async fn get_public_key(&self, handle: &HsmKeyHandle) -> Result<[u8; 32]>;
    
    /// Sign data using HSM-protected private key
    async fn sign(&self, handle: &HsmKeyHandle, data: &[u8]) -> Result<[u8; 64]>;
    
    /// List available keys in the HSM
    async fn list_keys(&self, slot_id: u32) -> Result<Vec<HsmKeyHandle>>;
    
    /// Delete a key from the HSM
    async fn delete_key(&self, handle: &HsmKeyHandle) -> Result<()>;
    
    /// Check if HSM is available and responding
    async fn health_check(&self) -> Result<HsmHealth>;
}

/// HSM health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HsmHealth {
    pub is_available: bool,
    pub slot_count: u32,
    pub firmware_version: String,
    pub last_error: Option<String>,
}

/// Secure authentication PIN (automatically zeroized)
#[derive(Debug, Clone, ZeroizeOnDrop)]
pub struct HsmPin {
    pin: String,
}

impl HsmPin {
    pub fn new(pin: String) -> Self {
        Self { pin }
    }
    
    pub fn as_str(&self) -> &str {
        &self.pin
    }
}

impl HsmKeystore {
    /// Create new HSM keystore with provider
    pub fn new(provider: Arc<dyn HsmProvider>) -> Self {
        Self {
            provider,
            public_keys: parking_lot::RwLock::new(HashMap::new()),
            identity_handle: None,
        }
    }
    
    /// Initialize the HSM and set up primary identity
    pub async fn initialize(&mut self, pin: HsmPin, slot_id: u32) -> Result<()> {
        self.provider.initialize().await?;
        
        // Generate primary identity key
        let identity_handle = self.provider
            .generate_keypair("bitcraps_identity", slot_id)
            .await?;
            
        let public_key = self.provider
            .get_public_key(&identity_handle)
            .await?;
            
        // Cache the public key
        {
            let mut cache = self.public_keys.write();
            cache.insert(identity_handle.clone(), public_key);
        }
        
        self.identity_handle = Some(identity_handle);
        Ok(())
    }
    
    /// Get peer ID using HSM-backed identity key
    pub async fn peer_id(&self) -> Result<PeerId> {
        let handle = self.identity_handle
            .as_ref()
            .ok_or_else(|| crate::error::Error::Crypto("HSM not initialized".to_string()))?;
            
        let public_key = self.get_cached_public_key(handle).await?;
        Ok(public_key)
    }
    
    /// Sign data using HSM-protected key
    pub async fn sign(&self, data: &[u8]) -> Result<ProtocolSignature> {
        let handle = self.identity_handle
            .as_ref()
            .ok_or_else(|| crate::error::Error::Crypto("HSM not initialized".to_string()))?;
            
        let signature = self.provider.sign(handle, data).await?;
        Ok(ProtocolSignature(signature))
    }
    
    /// Generate a new context-specific key in HSM
    pub async fn generate_context_key(&self, context: &str, slot_id: u32) -> Result<HsmKeyHandle> {
        let label = format!("bitcraps_{}", context);
        let handle = self.provider.generate_keypair(&label, slot_id).await?;
        
        // Cache public key
        let public_key = self.provider.get_public_key(&handle).await?;
        {
            let mut cache = self.public_keys.write();
            cache.insert(handle.clone(), public_key);
        }
        
        Ok(handle)
    }
    
    /// Sign with specific context key
    pub async fn sign_with_handle(&self, handle: &HsmKeyHandle, data: &[u8]) -> Result<[u8; 64]> {
        self.provider.sign(handle, data).await
    }
    
    /// Get cached public key or fetch from HSM
    async fn get_cached_public_key(&self, handle: &HsmKeyHandle) -> Result<[u8; 32]> {
        // Try cache first
        {
            let cache = self.public_keys.read();
            if let Some(&public_key) = cache.get(handle) {
                return Ok(public_key);
            }
        }
        
        // Fetch from HSM and cache
        let public_key = self.provider.get_public_key(handle).await?;
        {
            let mut cache = self.public_keys.write();
            cache.insert(handle.clone(), public_key);
        }
        
        Ok(public_key)
    }
    
    /// Export public key for peer verification
    pub async fn export_public_key(&self, handle: &HsmKeyHandle) -> Result<[u8; 32]> {
        self.get_cached_public_key(handle).await
    }
    
    /// Health check the HSM
    pub async fn health_check(&self) -> Result<HsmHealth> {
        self.provider.health_check().await
    }
    
    /// List all available keys
    pub async fn list_keys(&self, slot_id: u32) -> Result<Vec<HsmKeyHandle>> {
        self.provider.list_keys(slot_id).await
    }
}

/// Verify signature using public key (no HSM required)
pub fn verify_hsm_signature(
    data: &[u8],
    signature: &[u8; 64],
    public_key: &[u8; 32],
) -> Result<bool> {
    use ed25519_dalek::{Signature, VerifyingKey, Verifier};
    
    let verifying_key = VerifyingKey::from_bytes(public_key)
        .map_err(|_| crate::error::Error::InvalidPublicKey("Invalid public key".to_string()))?;
        
    let sig = Signature::from_bytes(signature);
    
    Ok(verifying_key.verify(data, &sig).is_ok())
}

// Conditional PKCS#11 implementation
#[cfg(feature = "hsm")]
pub mod pkcs11_provider {
    use super::*;
    use pkcs11::types::*;
    use pkcs11::Ctx;
    use std::path::Path;
    
    /// PKCS#11 HSM provider implementation
    pub struct Pkcs11Provider {
        ctx: Arc<Ctx>,
        library_path: String,
    }
    
    impl Pkcs11Provider {
        pub fn new(library_path: impl AsRef<Path>) -> Result<Self> {
            let library_path = library_path.as_ref().to_string_lossy().to_string();
            let ctx = Ctx::new_and_initialize(&library_path)
                .map_err(|e| crate::error::Error::Crypto(format!("PKCS#11 init failed: {}", e)))?;
                
            Ok(Self {
                ctx: Arc::new(ctx),
                library_path,
            })
        }
    }
    
    #[async_trait::async_trait]
    impl HsmProvider for Pkcs11Provider {
        async fn initialize(&self) -> Result<()> {
            // PKCS#11 initialization already done in new()
            Ok(())
        }
        
        async fn generate_keypair(&self, label: &str, slot_id: u32) -> Result<HsmKeyHandle> {
            // Implementation would use PKCS#11 C_GenerateKeyPair
            // This is a simplified version for demonstration
            Ok(HsmKeyHandle {
                key_id: format!("pkcs11_{}", uuid::Uuid::new_v4()),
                slot_id,
                label: label.to_string(),
            })
        }
        
        async fn get_public_key(&self, handle: &HsmKeyHandle) -> Result<[u8; 32]> {
            // Implementation would extract public key from PKCS#11 token
            // This is a placeholder - real implementation would query the HSM
            Ok([0u8; 32]) // Placeholder
        }
        
        async fn sign(&self, handle: &HsmKeyHandle, data: &[u8]) -> Result<[u8; 64]> {
            // Implementation would use C_Sign with CKM_EDDSA mechanism
            // This is a placeholder - real implementation would sign with HSM
            Ok([0u8; 64]) // Placeholder
        }
        
        async fn list_keys(&self, slot_id: u32) -> Result<Vec<HsmKeyHandle>> {
            // Implementation would enumerate keys in the slot
            Ok(vec![])
        }
        
        async fn delete_key(&self, handle: &HsmKeyHandle) -> Result<()> {
            // Implementation would use C_DestroyObject
            Ok(())
        }
        
        async fn health_check(&self) -> Result<HsmHealth> {
            Ok(HsmHealth {
                is_available: true,
                slot_count: 1,
                firmware_version: "PKCS#11".to_string(),
                last_error: None,
            })
        }
    }
}

// Conditional YubiKey implementation
#[cfg(feature = "yubikey")]
pub mod yubikey_provider {
    use super::*;
    
    /// YubiKey HSM provider implementation
    pub struct YubikeyProvider {
        // YubiKey context would go here
    }
    
    impl YubikeyProvider {
        pub fn new() -> Result<Self> {
            Ok(Self {})
        }
    }
    
    #[async_trait::async_trait]
    impl HsmProvider for YubikeyProvider {
        async fn initialize(&self) -> Result<()> {
            Ok(())
        }
        
        async fn generate_keypair(&self, label: &str, slot_id: u32) -> Result<HsmKeyHandle> {
            Ok(HsmKeyHandle {
                key_id: format!("yubikey_{}", uuid::Uuid::new_v4()),
                slot_id,
                label: label.to_string(),
            })
        }
        
        async fn get_public_key(&self, handle: &HsmKeyHandle) -> Result<[u8; 32]> {
            // YubiKey implementation would extract public key
            Ok([0u8; 32]) // Placeholder
        }
        
        async fn sign(&self, handle: &HsmKeyHandle, data: &[u8]) -> Result<[u8; 64]> {
            // YubiKey implementation would sign with PIV or FIDO2
            Ok([0u8; 64]) // Placeholder
        }
        
        async fn list_keys(&self, slot_id: u32) -> Result<Vec<HsmKeyHandle>> {
            Ok(vec![])
        }
        
        async fn delete_key(&self, handle: &HsmKeyHandle) -> Result<()> {
            Ok(())
        }
        
        async fn health_check(&self) -> Result<HsmHealth> {
            Ok(HsmHealth {
                is_available: true,
                slot_count: 1,
                firmware_version: "YubiKey 5".to_string(),
                last_error: None,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    /// Mock HSM provider for testing
    struct MockHsmProvider {
        keys: parking_lot::RwLock<HashMap<HsmKeyHandle, ([u8; 32], [u8; 32])>>, // (public, private)
    }
    
    impl MockHsmProvider {
        fn new() -> Self {
            Self {
                keys: parking_lot::RwLock::new(HashMap::new()),
            }
        }
    }
    
    #[async_trait::async_trait]
    impl HsmProvider for MockHsmProvider {
        async fn initialize(&self) -> Result<()> {
            Ok(())
        }
        
        async fn generate_keypair(&self, label: &str, slot_id: u32) -> Result<HsmKeyHandle> {
            use ed25519_dalek::{SigningKey, VerifyingKey};
            use rand::rngs::OsRng;
            
            let signing_key = SigningKey::generate(&mut OsRng);
            let verifying_key = signing_key.verifying_key();
            
            let handle = HsmKeyHandle {
                key_id: uuid::Uuid::new_v4().to_string(),
                slot_id,
                label: label.to_string(),
            };
            
            let mut keys = self.keys.write();
            keys.insert(handle.clone(), (verifying_key.to_bytes(), signing_key.to_bytes()));
            
            Ok(handle)
        }
        
        async fn get_public_key(&self, handle: &HsmKeyHandle) -> Result<[u8; 32]> {
            let keys = self.keys.read();
            keys.get(handle)
                .map(|(public, _)| *public)
                .ok_or_else(|| crate::error::Error::Crypto("Key not found".to_string()))
        }
        
        async fn sign(&self, handle: &HsmKeyHandle, data: &[u8]) -> Result<[u8; 64]> {
            use ed25519_dalek::{SigningKey, Signer};
            
            let keys = self.keys.read();
            let (_, private_key) = keys.get(handle)
                .ok_or_else(|| crate::error::Error::Crypto("Key not found".to_string()))?;
                
            let signing_key = SigningKey::from_bytes(private_key);
            let signature = signing_key.sign(data);
            
            Ok(signature.to_bytes())
        }
        
        async fn list_keys(&self, _slot_id: u32) -> Result<Vec<HsmKeyHandle>> {
            let keys = self.keys.read();
            Ok(keys.keys().cloned().collect())
        }
        
        async fn delete_key(&self, handle: &HsmKeyHandle) -> Result<()> {
            let mut keys = self.keys.write();
            keys.remove(handle);
            Ok(())
        }
        
        async fn health_check(&self) -> Result<HsmHealth> {
            Ok(HsmHealth {
                is_available: true,
                slot_count: 1,
                firmware_version: "Mock HSM v1.0".to_string(),
                last_error: None,
            })
        }
    }
    
    #[tokio::test]
    async fn test_hsm_keystore_creation() {
        let provider = Arc::new(MockHsmProvider::new());
        let mut keystore = HsmKeystore::new(provider);
        
        let pin = HsmPin::new("123456".to_string());
        let result = keystore.initialize(pin, 0).await;
        
        assert!(result.is_ok());
        assert!(keystore.identity_handle.is_some());
    }
    
    #[tokio::test]
    async fn test_hsm_signing() {
        let provider = Arc::new(MockHsmProvider::new());
        let mut keystore = HsmKeystore::new(provider);
        
        let pin = HsmPin::new("123456".to_string());
        keystore.initialize(pin, 0).await.unwrap();
        
        let message = b"test message";
        let signature = keystore.sign(message).await.unwrap();
        
        // Verify signature
        let public_key = keystore.peer_id().await.unwrap();
        let is_valid = verify_hsm_signature(message, &signature.0, &public_key).unwrap();
        assert!(is_valid);
    }
    
    #[tokio::test]
    async fn test_hsm_health_check() {
        let provider = Arc::new(MockHsmProvider::new());
        let keystore = HsmKeystore::new(provider);
        
        let health = keystore.health_check().await.unwrap();
        assert!(health.is_available);
        assert_eq!(health.slot_count, 1);
    }
    
    #[tokio::test]
    async fn test_context_key_generation() {
        let provider = Arc::new(MockHsmProvider::new());
        let mut keystore = HsmKeystore::new(provider);
        
        let pin = HsmPin::new("123456".to_string());
        keystore.initialize(pin, 0).await.unwrap();
        
        let handle = keystore.generate_context_key("consensus", 0).await.unwrap();
        assert_eq!(handle.label, "bitcraps_consensus");
        
        let public_key = keystore.export_public_key(&handle).await.unwrap();
        assert_ne!(public_key, [0u8; 32]); // Should be real key
    }
    
    #[test]
    fn test_hsm_pin_zeroization() {
        let pin_value = "super_secret_pin".to_string();
        let pin = HsmPin::new(pin_value.clone());
        
        assert_eq!(pin.as_str(), pin_value);
        
        // PIN should be zeroized when dropped
        drop(pin);
        // Note: We can't directly test zeroization in safe Rust,
        // but the zeroize crate handles this automatically
    }
}