//! Production encryption utilities for BitCraps
//! 
//! Provides high-level encryption/decryption interfaces using cryptographically secure implementations.
//! 
//! SECURITY: Uses OsRng for all random number generation and proper ECDH key exchange.

use rand::{RngCore, rngs::OsRng};
use chacha20poly1305::{ChaCha20Poly1305, KeyInit};
use chacha20poly1305::aead::{Aead, generic_array::GenericArray};
use x25519_dalek::{PublicKey, EphemeralSecret, x25519};
use hkdf::Hkdf;
use sha2::Sha256;

/// X25519 keypair for ECDH key exchange and encryption
#[derive(Debug, Clone)]
pub struct EncryptionKeypair {
    pub public_key: [u8; 32],
    pub private_key: [u8; 32],
}

/// High-level encryption interface
pub struct Encryption;

impl Encryption {
    /// Generate a new X25519 keypair using cryptographically secure randomness
    pub fn generate_keypair() -> EncryptionKeypair {
        let mut secure_rng = OsRng;
        
        // Generate a random 32-byte private key
        let mut private_key = [0u8; 32];
        secure_rng.fill_bytes(&mut private_key);
        
        // Clamp the private key for X25519
        private_key[0] &= 248;
        private_key[31] &= 127;
        private_key[31] |= 64;
        
        // Derive the corresponding public key using the base point
        let public_key = x25519(private_key, [
            9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
        ]);
        
        EncryptionKeypair {
            public_key,
            private_key,
        }
    }

    /// Encrypt a message using ECDH + ChaCha20Poly1305
    /// 
    /// This generates a new ephemeral keypair, performs ECDH with the recipient's public key,
    /// derives a symmetric key, and encrypts the message.
    pub fn encrypt(message: &[u8], recipient_public_key: &[u8; 32]) -> Result<Vec<u8>, String> {
        let mut secure_rng = OsRng;
        
        // Generate ephemeral private key
        let ephemeral_secret = EphemeralSecret::random_from_rng(&mut secure_rng);
        let ephemeral_public = PublicKey::from(&ephemeral_secret);
        
        // Parse recipient's public key
        let recipient_public = PublicKey::from(*recipient_public_key);
        
        // Perform ECDH to get shared secret
        let shared_secret = ephemeral_secret.diffie_hellman(&recipient_public);
        
        // Derive encryption key using HKDF
        let hk = Hkdf::<Sha256>::new(None, shared_secret.as_bytes());
        let mut symmetric_key = [0u8; 32];
        hk.expand(b"BITCRAPS_ENCRYPTION_V1", &mut symmetric_key)
            .map_err(|_| "Key derivation failed")?;
        
        // Encrypt with ChaCha20Poly1305
        let cipher = ChaCha20Poly1305::new(GenericArray::from_slice(&symmetric_key));
        
        // Generate cryptographically secure nonce
        let mut nonce_bytes = [0u8; 12];
        secure_rng.fill_bytes(&mut nonce_bytes);
        let nonce = GenericArray::from_slice(&nonce_bytes);
        
        match cipher.encrypt(nonce, message) {
            Ok(ciphertext) => {
                // Format: ephemeral_public_key (32) || nonce (12) || ciphertext
                let mut result = Vec::with_capacity(32 + 12 + ciphertext.len());
                result.extend_from_slice(ephemeral_public.as_bytes());
                result.extend_from_slice(&nonce_bytes);
                result.extend_from_slice(&ciphertext);
                Ok(result)
            },
            Err(_) => Err("Encryption failed".to_string()),
        }
    }

    /// Decrypt a message using ECDH + ChaCha20Poly1305
    /// 
    /// This extracts the ephemeral public key, performs ECDH with our private key,
    /// derives the symmetric key, and decrypts the message.
    pub fn decrypt(encrypted: &[u8], private_key: &[u8; 32]) -> Result<Vec<u8>, String> {
        if encrypted.len() < 32 + 12 + 16 { // ephemeral_pub + nonce + min_ciphertext
            return Err("Invalid ciphertext length".to_string());
        }
        
        // Extract components
        let ephemeral_public_bytes: [u8; 32] = encrypted[..32].try_into()
            .map_err(|_| "Invalid ephemeral public key")?;
        let nonce_bytes: [u8; 12] = encrypted[32..44].try_into()
            .map_err(|_| "Invalid nonce")?;
        let ciphertext = &encrypted[44..];
        
        // Perform ECDH using x25519 scalar multiplication
        // shared_secret = private_key * ephemeral_public_point
        let shared_secret = x25519(*private_key, ephemeral_public_bytes);
        
        // Derive decryption key using HKDF
        let hk = Hkdf::<Sha256>::new(None, &shared_secret);
        let mut symmetric_key = [0u8; 32];
        hk.expand(b"BITCRAPS_ENCRYPTION_V1", &mut symmetric_key)
            .map_err(|_| "Key derivation failed")?;
        
        // Decrypt with ChaCha20Poly1305
        let cipher = ChaCha20Poly1305::new(GenericArray::from_slice(&symmetric_key));
        let nonce = GenericArray::from_slice(&nonce_bytes);
        
        match cipher.decrypt(nonce, ciphertext) {
            Ok(plaintext) => Ok(plaintext),
            Err(_) => Err("Decryption failed - invalid ciphertext or wrong key".to_string()),
        }
    }
    
    /// Generate a keypair from seed (for deterministic testing)
    pub fn generate_keypair_from_seed(seed: &[u8; 32]) -> EncryptionKeypair {
        // Use seed as private key with proper clamping
        let mut private_key = *seed;
        
        // Clamp the private key for X25519
        private_key[0] &= 248;
        private_key[31] &= 127;
        private_key[31] |= 64;
        
        // Derive the corresponding public key using the base point
        let public_key = x25519(private_key, [
            9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
        ]);
        
        EncryptionKeypair {
            public_key,
            private_key,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encryption_decryption() {
        let keypair = Encryption::generate_keypair();
        let message = b"Hello, BitCraps!";
        
        println!("Testing with public key: {}", hex::encode(&keypair.public_key));
        println!("Testing with private key: {}", hex::encode(&keypair.private_key));
        
        let encrypted = Encryption::encrypt(message, &keypair.public_key).unwrap();
        println!("Encrypted length: {}, expected at least: {}", encrypted.len(), 32 + 12 + message.len() + 16);
        
        assert_ne!(encrypted.as_slice(), message);
        assert!(encrypted.len() >= 32 + 12 + message.len() + 16); // ephemeral + nonce + msg + tag
        
        let decrypted = Encryption::decrypt(&encrypted, &keypair.private_key).unwrap();
        assert_eq!(decrypted.as_slice(), message);
    }

    #[test]
    fn test_different_ephemeral_keys() {
        let keypair = Encryption::generate_keypair();
        let message = b"Test message";
        
        let encrypted1 = Encryption::encrypt(message, &keypair.public_key).unwrap();
        let encrypted2 = Encryption::encrypt(message, &keypair.public_key).unwrap();
        
        // Should produce different ciphertexts due to random ephemeral keys and nonces
        assert_ne!(encrypted1, encrypted2);
        
        // But both should decrypt correctly
        let decrypted1 = Encryption::decrypt(&encrypted1, &keypair.private_key).unwrap();
        let decrypted2 = Encryption::decrypt(&encrypted2, &keypair.private_key).unwrap();
        
        assert_eq!(decrypted1.as_slice(), message);
        assert_eq!(decrypted2.as_slice(), message);
    }
    
    #[test]
    fn test_invalid_decryption() {
        let keypair1 = Encryption::generate_keypair();
        let keypair2 = Encryption::generate_keypair();
        let message = b"Secret message";
        
        let encrypted = Encryption::encrypt(message, &keypair1.public_key).unwrap();
        
        // Should fail with wrong private key
        let result = Encryption::decrypt(&encrypted, &keypair2.private_key);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_deterministic_keypair() {
        let seed = [42u8; 32];
        let keypair1 = Encryption::generate_keypair_from_seed(&seed);
        let keypair2 = Encryption::generate_keypair_from_seed(&seed);
        
        assert_eq!(keypair1.public_key, keypair2.public_key);
        assert_eq!(keypair1.private_key, keypair2.private_key);
        
        let message = b"Deterministic test";
        let encrypted = Encryption::encrypt(message, &keypair1.public_key).unwrap();
        let decrypted = Encryption::decrypt(&encrypted, &keypair2.private_key).unwrap();
        assert_eq!(decrypted.as_slice(), message);
    }
    
    #[test]
    fn test_malformed_ciphertext() {
        let keypair = Encryption::generate_keypair();
        
        // Test various malformed inputs
        let too_short = vec![0u8; 10];
        assert!(Encryption::decrypt(&too_short, &keypair.private_key).is_err());
        
        let wrong_size = vec![0u8; 40]; // Less than minimum
        assert!(Encryption::decrypt(&wrong_size, &keypair.private_key).is_err());
        
        let random_bytes = vec![0u8; 100];
        assert!(Encryption::decrypt(&random_bytes, &keypair.private_key).is_err());
    }
}