//! Encryption utilities for BitCraps
//! 
//! Provides high-level encryption/decryption interfaces for testing and security validation.

use rand::{RngCore, thread_rng};
use chacha20poly1305::{ChaCha20Poly1305, KeyInit};
use chacha20poly1305::aead::{Aead, generic_array::GenericArray};

/// Keypair for encryption/decryption
#[derive(Debug, Clone)]
pub struct EncryptionKeypair {
    pub public_key: [u8; 32],
    pub private_key: [u8; 32],
}

/// High-level encryption interface
pub struct Encryption;

impl Encryption {
    /// Generate a new keypair
    pub fn generate_keypair() -> EncryptionKeypair {
        let mut private_key = [0u8; 32];
        let mut public_key = [0u8; 32];
        
        thread_rng().fill_bytes(&mut private_key);
        // For simplicity, use private key as public key (in real crypto, would derive properly)
        public_key.copy_from_slice(&private_key);
        
        EncryptionKeypair {
            public_key,
            private_key,
        }
    }

    /// Encrypt a message using ChaCha20Poly1305
    pub fn encrypt(message: &[u8], public_key: &[u8; 32]) -> Result<Vec<u8>, String> {
        let cipher = ChaCha20Poly1305::new(GenericArray::from_slice(public_key));
        
        // Generate random nonce
        let mut nonce_bytes = [0u8; 12];
        thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = GenericArray::from_slice(&nonce_bytes);
        
        match cipher.encrypt(nonce, message) {
            Ok(mut ciphertext) => {
                // Prepend nonce to ciphertext
                let mut result = nonce_bytes.to_vec();
                result.append(&mut ciphertext);
                Ok(result)
            },
            Err(_) => Err("Encryption failed".to_string()),
        }
    }

    /// Decrypt a message using ChaCha20Poly1305
    pub fn decrypt(encrypted: &[u8], private_key: &[u8; 32]) -> Result<Vec<u8>, String> {
        if encrypted.len() < 12 {
            return Err("Invalid ciphertext length".to_string());
        }
        
        let cipher = ChaCha20Poly1305::new(GenericArray::from_slice(private_key));
        
        // Extract nonce and ciphertext
        let nonce = GenericArray::from_slice(&encrypted[..12]);
        let ciphertext = &encrypted[12..];
        
        match cipher.decrypt(nonce, ciphertext) {
            Ok(plaintext) => Ok(plaintext),
            Err(_) => Err("Decryption failed".to_string()),
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
        
        let encrypted = Encryption::encrypt(message, &keypair.public_key).unwrap();
        assert_ne!(encrypted.as_slice(), message);
        
        let decrypted = Encryption::decrypt(&encrypted, &keypair.private_key).unwrap();
        assert_eq!(decrypted.as_slice(), message);
    }

    #[test]
    fn test_different_nonces() {
        let keypair = Encryption::generate_keypair();
        let message = b"Test message";
        
        let encrypted1 = Encryption::encrypt(message, &keypair.public_key).unwrap();
        let encrypted2 = Encryption::encrypt(message, &keypair.public_key).unwrap();
        
        // Should produce different ciphertexts due to random nonces
        assert_ne!(encrypted1, encrypted2);
        
        // But both should decrypt correctly
        let decrypted1 = Encryption::decrypt(&encrypted1, &keypair.private_key).unwrap();
        let decrypted2 = Encryption::decrypt(&encrypted2, &keypair.private_key).unwrap();
        
        assert_eq!(decrypted1.as_slice(), message);
        assert_eq!(decrypted2.as_slice(), message);
    }
}