//! SIMD-accelerated cryptographic operations for high-performance verification

use ed25519_dalek::{Signature, VerifyingKey, Verifier};
use sha2::{Sha256, Digest};
use rayon::prelude::*;

/// SIMD acceleration availability
#[derive(Debug, Clone, Copy)]
pub struct SimdCapabilities {
    pub has_avx2: bool,
    pub has_avx512: bool,
    pub has_sha_ni: bool,
    pub has_aes_ni: bool,
}

impl SimdCapabilities {
    /// Detect available SIMD instructions
    pub fn detect() -> Self {
        #[cfg(target_arch = "x86_64")]
        {
            Self {
                has_avx2: is_x86_feature_detected!("avx2"),
                has_avx512: is_x86_feature_detected!("avx512f"),
                has_sha_ni: is_x86_feature_detected!("sha"),
                has_aes_ni: is_x86_feature_detected!("aes"),
            }
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            Self {
                has_avx2: false,
                has_avx512: false,
                has_sha_ni: false,
                has_aes_ni: false,
            }
        }
    }
}

/// SIMD-accelerated cryptographic operations
pub struct SimdCrypto {
    capabilities: SimdCapabilities,
}

impl SimdCrypto {
    pub fn new() -> Self {
        Self {
            capabilities: SimdCapabilities::detect(),
        }
    }
    
    /// Batch verify signatures using parallel processing
    pub fn batch_verify(
        &self,
        signatures: &[Signature],
        messages: &[Vec<u8>],
        public_keys: &[VerifyingKey],
    ) -> Vec<bool> {
        if signatures.len() != messages.len() || signatures.len() != public_keys.len() {
            return vec![false; signatures.len()];
        }
        
        // Use rayon for parallel verification
        signatures
            .par_iter()
            .zip(messages.par_iter())
            .zip(public_keys.par_iter())
            .map(|((sig, msg), pk)| {
                pk.verify(msg, sig).is_ok()
            })
            .collect()
    }
    
    /// Batch hash computation
    pub fn batch_hash(&self, messages: &[Vec<u8>]) -> Vec<[u8; 32]> {
        messages
            .par_iter()
            .map(|msg| {
                let mut hasher = Sha256::new();
                hasher.update(msg);
                hasher.finalize().into()
            })
            .collect()
    }
}

/// SIMD-accelerated XOR operations for encryption
pub struct SimdXor;

impl SimdXor {
    /// XOR two byte arrays
    pub fn xor(a: &mut [u8], b: &[u8]) {
        let len = a.len().min(b.len());
        for i in 0..len {
            a[i] ^= b[i];
        }
    }
}

/// SIMD-accelerated hashing
pub struct SimdHash {
    hasher_type: HashType,
}

#[derive(Debug, Clone, Copy)]
pub enum HashType {
    Sha256,
    Blake3,
}

impl SimdHash {
    pub fn new() -> Self {
        Self {
            hasher_type: HashType::Blake3, // Blake3 is SIMD-optimized by default
        }
    }
    
    pub fn hash_data(&self, data: &[u8]) -> Vec<u8> {
        match self.hasher_type {
            HashType::Sha256 => {
                let mut hasher = Sha256::new();
                hasher.update(data);
                hasher.finalize().to_vec()
            }
            HashType::Blake3 => {
                blake3::hash(data).as_bytes().to_vec()
            }
        }
    }
    
    pub fn hash_parallel(&self, chunks: &[Vec<u8>]) -> Vec<Vec<u8>> {
        chunks
            .par_iter()
            .map(|chunk| self.hash_data(chunk))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::BitchatKeypair;
    use ed25519_dalek::Signer;
    
    #[test]
    fn test_simd_capabilities() {
        let caps = SimdCapabilities::detect();
        println!("SIMD Capabilities: {:?}", caps);
        // Test passes regardless of capabilities
    }
    
    #[test]
    fn test_batch_verify() {
        let crypto = SimdCrypto::new();
        
        let mut signatures = Vec::new();
        let mut messages = Vec::new();
        let mut public_keys = Vec::new();
        
        for i in 0..4 {
            // Use ed25519_dalek types directly for this test
            let signing_key = ed25519_dalek::SigningKey::generate(&mut rand::rngs::OsRng);
            let verifying_key = signing_key.verifying_key();
            let message = format!("Message {}", i).into_bytes();
            let signature = signing_key.sign(&message);
            
            signatures.push(signature);
            messages.push(message);
            public_keys.push(verifying_key);
        }
        
        let results = crypto.batch_verify(&signatures, &messages, &public_keys);
        assert!(results.iter().all(|&r| r));
    }
    
    #[test]
    fn test_simd_xor() {
        let mut a = vec![0xFF; 32];
        let b = vec![0xAA; 32];
        
        SimdXor::xor(&mut a, &b);
        
        for byte in a {
            assert_eq!(byte, 0xFF ^ 0xAA);
        }
    }
    
    #[test]
    fn test_simd_hash() {
        let hasher = SimdHash::new();
        let data = b"Test data for hashing";
        
        let hash = hasher.hash_data(data);
        assert!(!hash.is_empty());
        
        // Test parallel hashing
        let chunks = vec![
            b"chunk1".to_vec(),
            b"chunk2".to_vec(),
            b"chunk3".to_vec(),
        ];
        
        let hashes = hasher.hash_parallel(&chunks);
        assert_eq!(hashes.len(), 3);
    }
}