//! Secure key management for BitCraps
//!
//! This module provides cryptographically secure key generation, storage, and management
//! using Ed25519 signatures and secure random number generation.

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use zeroize::ZeroizeOnDrop;

use crate::error::Result;
use crate::protocol::{PeerId, Signature as ProtocolSignature};

/// Secure keystore for managing cryptographic keys
#[derive(Debug)]
pub struct SecureKeystore {
    /// Primary identity key (Ed25519)
    identity_key: SigningKey,
    /// Cached verifying key
    verifying_key: VerifyingKey,
    /// Session keys for different contexts
    session_keys: HashMap<String, SigningKey>,
    /// Secure random number generator
    secure_rng: OsRng,
}

/// Key context for different operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KeyContext {
    /// Identity/authentication key
    Identity,
    /// Consensus/voting key
    Consensus,
    /// Game state signing
    GameState,
    /// Dispute resolution
    Dispute,
    /// Randomness commitment
    RandomnessCommit,
}

/// Secure signature with context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecureSignature {
    #[serde(with = "serde_bytes")]
    pub signature: Vec<u8>,
    #[serde(with = "serde_bytes")]
    pub public_key: Vec<u8>,
    pub context: KeyContext,
    pub timestamp: u64,
}

/// Key derivation material (securely zeroized)
#[derive(Debug, Clone, ZeroizeOnDrop)]
struct KeyMaterial {
    #[zeroize(skip)]
    context: KeyContext,
    seed: [u8; 32],
}

impl SecureKeystore {
    /// Create new keystore with cryptographically secure key generation
    pub fn new() -> Result<Self> {
        let mut secure_rng = OsRng;
        let identity_key = SigningKey::generate(&mut secure_rng);
        let verifying_key = identity_key.verifying_key();

        Ok(Self {
            identity_key,
            verifying_key,
            session_keys: HashMap::new(),
            secure_rng,
        })
    }

    /// Create keystore from existing seed (for testing/deterministic scenarios)
    pub fn from_seed(seed: [u8; 32]) -> Result<Self> {
        let identity_key = SigningKey::from_bytes(&seed);
        let verifying_key = identity_key.verifying_key();

        Ok(Self {
            identity_key,
            verifying_key,
            session_keys: HashMap::new(),
            secure_rng: OsRng,
        })
    }

    /// Get peer ID (public key)
    pub fn peer_id(&self) -> PeerId {
        self.verifying_key.to_bytes()
    }

    /// Sign data with the appropriate key for given context
    pub fn sign_with_context(
        &mut self,
        data: &[u8],
        context: KeyContext,
    ) -> Result<SecureSignature> {
        let key = self.get_key_for_context(&context)?;
        let signature = key.sign(data);
        let public_key = key.verifying_key().to_bytes();

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Ok(SecureSignature {
            signature: signature.to_bytes().to_vec(),
            public_key: public_key.to_vec(),
            context,
            timestamp,
        })
    }

    /// Sign data with identity key (most common case)
    pub fn sign(&mut self, data: &[u8]) -> Result<ProtocolSignature> {
        let signature = self.identity_key.sign(data);
        Ok(ProtocolSignature(signature.to_bytes()))
    }

    /// Verify signature from any peer
    pub fn verify_signature(
        data: &[u8],
        signature: &ProtocolSignature,
        public_key: &[u8; 32],
    ) -> Result<bool> {
        let verifying_key = VerifyingKey::from_bytes(public_key).map_err(|_| {
            crate::error::Error::InvalidPublicKey("Invalid public key format".to_string())
        })?;

        let sig = Signature::from_bytes(&signature.0);

        Ok(verifying_key.verify(data, &sig).is_ok())
    }

    /// Verify secure signature with context validation
    pub fn verify_secure_signature(
        data: &[u8],
        signature: &SecureSignature,
        expected_context: &KeyContext,
    ) -> Result<bool> {
        // Verify context matches
        if std::mem::discriminant(&signature.context) != std::mem::discriminant(expected_context) {
            return Ok(false);
        }

        // Verify timestamp is reasonable (within 1 hour)
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if signature.timestamp > now + 3600 || signature.timestamp < now.saturating_sub(3600) {
            return Ok(false);
        }

        // Verify cryptographic signature
        let pk_bytes: [u8; 32] = signature.public_key.as_slice().try_into().map_err(|_| {
            crate::error::Error::InvalidPublicKey(
                "Invalid public key length in signature".to_string(),
            )
        })?;
        let verifying_key = VerifyingKey::from_bytes(&pk_bytes).map_err(|_| {
            crate::error::Error::InvalidPublicKey("Invalid public key in signature".to_string())
        })?;

        let sig_bytes: [u8; 64] = signature.signature.as_slice().try_into().map_err(|_| {
            crate::error::Error::InvalidSignature("Invalid signature length".to_string())
        })?;
        let sig = Signature::from_bytes(&sig_bytes);
        Ok(verifying_key.verify(data, &sig).is_ok())
    }

    /// Generate secure random bytes using OS entropy
    pub fn generate_random_bytes(&mut self, length: usize) -> Vec<u8> {
        use rand::RngCore;
        let mut bytes = vec![0u8; length];
        self.secure_rng.fill_bytes(&mut bytes);
        bytes
    }

    /// Generate secure randomness for commit-reveal schemes
    pub fn generate_commitment_nonce(&mut self) -> [u8; 32] {
        use rand::RngCore;
        let mut nonce = [0u8; 32];
        self.secure_rng.fill_bytes(&mut nonce);
        nonce
    }

    /// Derive session key for specific context
    fn derive_session_key(&mut self, context: &KeyContext) -> Result<SigningKey> {
        use rand::RngCore;
        use sha2::{Digest, Sha256};

        // Generate additional entropy
        let mut entropy = [0u8; 32];
        self.secure_rng.fill_bytes(&mut entropy);

        // Create deterministic but secure seed
        let mut hasher = Sha256::new();
        hasher.update(self.identity_key.to_bytes());
        hasher.update(&entropy);

        // Add context-specific data
        match context {
            KeyContext::Identity => hasher.update(b"IDENTITY_KEY_V1"),
            KeyContext::Consensus => hasher.update(b"CONSENSUS_KEY_V1"),
            KeyContext::GameState => hasher.update(b"GAMESTATE_KEY_V1"),
            KeyContext::Dispute => hasher.update(b"DISPUTE_KEY_V1"),
            KeyContext::RandomnessCommit => hasher.update(b"RANDOMNESS_KEY_V1"),
        }

        let seed = hasher.finalize();
        let mut seed_array = [0u8; 32];
        seed_array.copy_from_slice(&seed);

        Ok(SigningKey::from_bytes(&seed_array))
    }

    /// Get or create key for specific context
    fn get_key_for_context(&mut self, context: &KeyContext) -> Result<&SigningKey> {
        match context {
            KeyContext::Identity => Ok(&self.identity_key),
            _ => {
                let context_key = format!("{:?}", context);
                if !self.session_keys.contains_key(&context_key) {
                    let session_key = self.derive_session_key(context)?;
                    self.session_keys.insert(context_key.clone(), session_key);
                }
                self.session_keys.get(&context_key)
                    .ok_or_else(|| crate::error::Error::Crypto("Failed to retrieve session key after creation".to_string()))
            }
        }
    }

    /// Export public key for peer verification
    pub fn export_public_key(&self) -> [u8; 32] {
        self.verifying_key.to_bytes()
    }

    /// Import and verify a peer's public key
    pub fn verify_peer_public_key(public_key: &[u8; 32]) -> Result<VerifyingKey> {
        VerifyingKey::from_bytes(public_key).map_err(|_| {
            crate::error::Error::InvalidPublicKey("Invalid peer public key".to_string())
        })
    }
}

impl Default for SecureKeystore {
    fn default() -> Self {
        Self::new().unwrap_or_else(|e| {
            eprintln!("WARNING: Failed to create secure keystore, using fallback: {}", e);
            // Create a minimal fallback keystore
            SecureKeystore {
                identity_key: SigningKey::generate(&mut OsRng),
                verifying_key: VerifyingKey::from(&SigningKey::generate(&mut OsRng)),
                session_keys: HashMap::new(),
                secure_rng: OsRng,
            }
        })
    }
}

// Securely zeroize sensitive data
impl Drop for SecureKeystore {
    fn drop(&mut self) {
        // Session keys are automatically zeroized by HashMap drop
        // Identity key is zeroized by Ed25519 library
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secure_keystore_creation() {
        let keystore = SecureKeystore::new().unwrap();
        let peer_id = keystore.peer_id();
        assert_eq!(peer_id.len(), 32);
    }

    #[test]
    fn test_signature_creation_and_verification() {
        let mut keystore = SecureKeystore::new().unwrap();
        let message = b"test message for signing";

        let signature = keystore.sign(message).unwrap();
        let public_key = keystore.export_public_key();

        let is_valid = SecureKeystore::verify_signature(message, &signature, &public_key).unwrap();
        assert!(is_valid);

        // Test with wrong message
        let wrong_message = b"wrong message";
        let is_invalid =
            SecureKeystore::verify_signature(wrong_message, &signature, &public_key).unwrap();
        assert!(!is_invalid);
    }

    #[test]
    fn test_context_signing() {
        let mut keystore = SecureKeystore::new().unwrap();
        let message = b"consensus vote data";

        let signature = keystore
            .sign_with_context(message, KeyContext::Consensus)
            .unwrap();
        assert_eq!(signature.public_key, keystore.export_public_key());

        let is_valid =
            SecureKeystore::verify_secure_signature(message, &signature, &KeyContext::Consensus)
                .unwrap();
        assert!(is_valid);

        // Test with wrong context
        let is_invalid =
            SecureKeystore::verify_secure_signature(message, &signature, &KeyContext::Identity)
                .unwrap();
        assert!(!is_invalid);
    }

    #[test]
    fn test_random_generation() {
        let mut keystore = SecureKeystore::new().unwrap();

        let bytes1 = keystore.generate_random_bytes(32);
        let bytes2 = keystore.generate_random_bytes(32);

        assert_eq!(bytes1.len(), 32);
        assert_eq!(bytes2.len(), 32);
        assert_ne!(bytes1, bytes2); // Should be different

        let nonce1 = keystore.generate_commitment_nonce();
        let nonce2 = keystore.generate_commitment_nonce();

        assert_ne!(nonce1, nonce2); // Should be different
    }

    #[test]
    fn test_deterministic_from_seed() {
        let seed = [42u8; 32];

        let keystore1 = SecureKeystore::from_seed(seed).unwrap();
        let keystore2 = SecureKeystore::from_seed(seed).unwrap();

        assert_eq!(keystore1.peer_id(), keystore2.peer_id());
    }
}
