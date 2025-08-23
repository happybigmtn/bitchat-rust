//! Cryptographic primitives for BitCraps
//! 
//! This module provides all cryptographic functionality for the BitCraps casino:
//! - Ed25519/Curve25519 key management
//! - Noise protocol for secure sessions
//! - Gaming-specific cryptography (commitment schemes, randomness)
//! - Proof-of-work for identity generation
//! - Signature verification and validation
//! - SIMD-accelerated batch operations

pub mod simd_acceleration;

use std::time::{SystemTime, UNIX_EPOCH};
use ed25519_dalek::{SigningKey, VerifyingKey, Signature, Signer, Verifier};
use rand::{RngCore, thread_rng};
use sha2::{Sha256, Digest};
use hmac::{Hmac, Mac};
use pbkdf2::pbkdf2_hmac;
use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;

use crate::protocol::{PeerId, GameId};
use crate::error::Result;

/// Ed25519 keypair for signing and identity
#[derive(Debug, Clone)]
pub struct BitchatKeypair {
    pub signing_key: SigningKey,
    pub verifying_key: VerifyingKey,
}

/// BitCraps identity with proof-of-work
#[derive(Debug, Clone)]
pub struct BitchatIdentity {
    pub peer_id: PeerId,
    pub keypair: BitchatKeypair,
    pub pow_nonce: u64,
    pub pow_difficulty: u32,
}

/// Gaming cryptography utilities
pub struct GameCrypto;

/// Key derivation utilities
pub struct KeyDerivation;

/// Proof-of-work implementation
pub struct ProofOfWork;

/// Randomness commitment for fair gaming
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RandomnessCommitment {
    pub hash: [u8; 32],
    pub timestamp: u64,
}

/// Signature wrapper for BitCraps
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BitchatSignature {
    pub signature: Vec<u8>,
    pub public_key: Vec<u8>,
}

impl BitchatKeypair {
    /// Generate a new keypair
    pub fn generate() -> Self {
        let mut rng = thread_rng();
        let signing_key = SigningKey::generate(&mut rng);
        let verifying_key = signing_key.verifying_key();
        Self { signing_key, verifying_key }
    }
    
    /// Create from existing secret key
    pub fn from_secret_key(secret_key: &[u8; 32]) -> Result<Self> {
        let signing_key = SigningKey::from_bytes(secret_key);
        let verifying_key = signing_key.verifying_key();
        Ok(Self { signing_key, verifying_key })
    }
    
    /// Get public key bytes
    pub fn public_key_bytes(&self) -> [u8; 32] {
        self.verifying_key.to_bytes()
    }
    
    /// Get secret key bytes
    pub fn secret_key_bytes(&self) -> [u8; 32] {
        self.signing_key.to_bytes()
    }
    
    /// Sign data
    pub fn sign(&self, data: &[u8]) -> BitchatSignature {
        let signature = self.signing_key.sign(data);
        BitchatSignature {
            signature: signature.to_bytes().to_vec(),
            public_key: self.public_key_bytes().to_vec(),
        }
    }
    
    /// Verify signature
    pub fn verify(&self, data: &[u8], signature: &BitchatSignature) -> bool {
        // Check if this is our public key
        if signature.public_key != self.public_key_bytes().to_vec() {
            return false;
        }
        
        let sig_bytes: [u8; 64] = match signature.signature.as_slice().try_into() {
            Ok(bytes) => bytes,
            Err(_) => return false,
        };
        let sig = Signature::from_bytes(&sig_bytes);
        
        self.verifying_key.verify(data, &sig).is_ok()
    }
}

impl BitchatIdentity {
    /// Generate identity with proof-of-work
    pub fn generate_with_pow(difficulty: u32) -> Self {
        let keypair = BitchatKeypair::generate();
        let public_key_bytes = keypair.public_key_bytes();
        
        // Mine proof-of-work
        let (nonce, _hash) = ProofOfWork::mine_identity(&public_key_bytes, difficulty);
        
        Self {
            peer_id: public_key_bytes,
            keypair,
            pow_nonce: nonce,
            pow_difficulty: difficulty,
        }
    }
    
    /// Create from existing keypair with PoW
    pub fn from_keypair_with_pow(keypair: BitchatKeypair, difficulty: u32) -> Self {
        let public_key_bytes = keypair.public_key_bytes();
        let (nonce, _hash) = ProofOfWork::mine_identity(&public_key_bytes, difficulty);
        
        Self {
            peer_id: public_key_bytes,
            keypair,
            pow_nonce: nonce,
            pow_difficulty: difficulty,
        }
    }
    
    /// Verify proof-of-work
    pub fn verify_pow(&self) -> bool {
        ProofOfWork::verify_identity(&self.peer_id, self.pow_nonce, self.pow_difficulty)
    }
    
    /// Sign data with identity
    pub fn sign(&self, data: &[u8]) -> BitchatSignature {
        self.keypair.sign(data)
    }
    
    /// Verify signature from another identity
    pub fn verify_signature(data: &[u8], signature: &BitchatSignature) -> bool {
        let pk_bytes: [u8; 32] = match signature.public_key.as_slice().try_into() {
            Ok(bytes) => bytes,
            Err(_) => return false,
        };
        let public_key = match VerifyingKey::from_bytes(&pk_bytes) {
            Ok(pk) => pk,
            Err(_) => return false,
        };
        
        let sig_bytes: [u8; 64] = match signature.signature.as_slice().try_into() {
            Ok(bytes) => bytes,
            Err(_) => return false,
        };
        let sig = Signature::from_bytes(&sig_bytes);
        
        public_key.verify(data, &sig).is_ok()
    }
}

impl ProofOfWork {
    /// Mine proof-of-work for identity generation
    pub fn mine_identity(public_key: &[u8; 32], difficulty: u32) -> (u64, [u8; 32]) {
        let mut nonce = 0u64;
        
        loop {
            let hash = Self::compute_identity_hash(public_key, nonce);
            if Self::check_difficulty(&hash, difficulty) {
                return (nonce, hash);
            }
            nonce = nonce.wrapping_add(1);
        }
    }
    
    /// Verify proof-of-work for identity
    pub fn verify_identity(public_key: &[u8; 32], nonce: u64, difficulty: u32) -> bool {
        let hash = Self::compute_identity_hash(public_key, nonce);
        Self::check_difficulty(&hash, difficulty)
    }
    
    /// Compute hash for identity PoW
    fn compute_identity_hash(public_key: &[u8; 32], nonce: u64) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(b"BITCRAPS_IDENTITY_POW");
        hasher.update(public_key);
        hasher.update(&nonce.to_be_bytes());
        
        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        hash
    }
    
    /// Check if hash meets difficulty requirement
    fn check_difficulty(hash: &[u8; 32], difficulty: u32) -> bool {
        let required_zero_bits = difficulty as usize;
        let required_zero_bytes = required_zero_bits / 8;
        let remaining_bits = required_zero_bits % 8;
        
        // Check full zero bytes
        for i in 0..required_zero_bytes {
            if hash[i] != 0 {
                return false;
            }
        }
        
        // Check remaining bits in next byte
        if remaining_bits > 0 && required_zero_bytes < 32 {
            let mask = 0xFF << (8 - remaining_bits);
            if (hash[required_zero_bytes] & mask) != 0 {
                return false;
            }
        }
        
        true
    }
}

impl GameCrypto {
    /// Generate a secure game ID
    pub fn generate_game_id() -> GameId {
        let mut rng = thread_rng();
        let mut game_id = [0u8; 16];
        rng.fill_bytes(&mut game_id);
        game_id
    }
    
    /// Create commitment for randomness (commit-reveal scheme)
    pub fn commit_randomness(secret: &[u8; 32]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(b"BITCRAPS_RANDOMNESS_COMMIT");
        hasher.update(secret);
        
        let result = hasher.finalize();
        let mut commitment = [0u8; 32];
        commitment.copy_from_slice(&result);
        commitment
    }
    
    /// Verify randomness commitment
    pub fn verify_commitment(commitment: &[u8; 32], secret: &[u8; 32]) -> bool {
        let computed_commitment = Self::commit_randomness(secret);
        commitment.ct_eq(&computed_commitment).into()
    }
    
    /// Combine multiple sources of randomness for fair dice rolls using cryptographic RNG
    pub fn combine_randomness(sources: &[[u8; 32]]) -> (u8, u8) {
        let mut combined = [0u8; 32];
        
        // XOR all randomness sources
        for source in sources {
            for (i, byte) in source.iter().enumerate() {
                combined[i] ^= byte;
            }
        }
        
        // Add fresh cryptographic randomness
        let mut csprng_bytes = [0u8; 32];
        use rand::RngCore;
        let mut rng = rand::thread_rng();
        rng.fill_bytes(&mut csprng_bytes);
        
        // Combine with existing sources
        for (i, byte) in csprng_bytes.iter().enumerate() {
            combined[i] ^= byte;
        }
        
        // Hash the combined result for final randomness
        let mut hasher = Sha256::new();
        hasher.update(b"BITCRAPS_DICE_ROLL_V2");
        hasher.update(&combined);
        
        // Use fallback timestamp if system time is unavailable
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| std::time::Duration::from_secs(0))
            .as_nanos();
        hasher.update(&timestamp.to_be_bytes());
        
        let hash = hasher.finalize();
        
        // Convert to dice values (1-6) using unbiased method
        let die1 = Self::hash_to_die_value(&hash[0..8]);
        let die2 = Self::hash_to_die_value(&hash[8..16]);
        
        (die1, die2)
    }
    
    /// Convert hash bytes to unbiased die value (1-6)
    fn hash_to_die_value(bytes: &[u8]) -> u8 {
        // Use rejection sampling to avoid modulo bias
        let mut value = u64::from_le_bytes(bytes.try_into().unwrap_or([0u8; 8]));
        
        // Reject values that would cause bias (multiples of 6 near u64::MAX)
        const MAX_VALID: u64 = u64::MAX - (u64::MAX % 6);
        
        while value >= MAX_VALID {
            // If we hit a biased value, hash again to get new randomness
            let mut hasher = Sha256::new();
            hasher.update(b"BITCRAPS_REROLL");
            hasher.update(&value.to_le_bytes());
            let new_hash = hasher.finalize();
            value = u64::from_le_bytes(new_hash[0..8].try_into().unwrap_or([0u8; 8]));
        }
        
        ((value % 6) + 1) as u8
    }
    
    /// Generate session key for encrypted gaming
    pub fn derive_session_key(game_id: &GameId, participants: &[PeerId]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(b"BITCRAPS_SESSION_KEY");
        hasher.update(game_id);
        
        // Sort participants for deterministic ordering
        let mut sorted_participants = participants.to_vec();
        sorted_participants.sort();
        
        for participant in sorted_participants {
            hasher.update(&participant);
        }
        
        let result = hasher.finalize();
        let mut key = [0u8; 32];
        key.copy_from_slice(&result);
        key
    }
    
    /// Create secure bet hash for integrity
    pub fn hash_bet(game_id: &GameId, player: &PeerId, amount: u64, bet_type: u8, timestamp: u64) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(b"BITCRAPS_BET_HASH");
        hasher.update(game_id);
        hasher.update(player);
        hasher.update(&amount.to_be_bytes());
        hasher.update(&[bet_type]);
        hasher.update(&timestamp.to_be_bytes());
        
        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        hash
    }
    
    /// Generate cryptographically secure random bytes
    pub fn generate_random_bytes(length: usize) -> Vec<u8> {
        use rand::RngCore;
        let mut rng = rand::thread_rng();
        let mut bytes = vec![0u8; length];
        rng.fill_bytes(&mut bytes);
        bytes
    }
    
    /// Generate a secure dice roll without external sources
    pub fn generate_secure_dice_roll() -> (u8, u8) {
        use rand::RngCore;
        let mut rng = rand::thread_rng();
        let mut bytes = [0u8; 16];
        rng.fill_bytes(&mut bytes);
        
        let die1 = Self::hash_to_die_value(&bytes[0..8]);
        let die2 = Self::hash_to_die_value(&bytes[8..16]);
        
        (die1, die2)
    }
    
    /// Create HMAC for message authentication
    pub fn create_hmac(key: &[u8], message: &[u8]) -> [u8; 32] {
        type HmacSha256 = Hmac<Sha256>;
        
        let mut mac = HmacSha256::new_from_slice(key)
            .expect("HMAC can take key of any size");
        mac.update(message);
        
        let result = mac.finalize().into_bytes();
        let mut hmac = [0u8; 32];
        hmac.copy_from_slice(&result);
        hmac
    }
    
    /// Verify HMAC
    pub fn verify_hmac(key: &[u8], message: &[u8], expected_hmac: &[u8; 32]) -> bool {
        let computed_hmac = Self::create_hmac(key, message);
        computed_hmac.ct_eq(expected_hmac).into()
    }
}

impl KeyDerivation {
    /// Derive key using secure PBKDF2 with established library
    /// Uses minimum 100,000 iterations for modern security standards
    pub fn derive_key_pbkdf2(
        password: &[u8],
        salt: &[u8],
        iterations: u32,
        output_length: usize,
    ) -> Result<Vec<u8>> {
        // Ensure minimum security: at least 100,000 iterations
        let secure_iterations = std::cmp::max(iterations, 100_000);
        
        let mut output = vec![0u8; output_length];
        pbkdf2_hmac::<Sha256>(password, salt, secure_iterations, &mut output);
        Ok(output)
    }
    
    /// Simple key derivation using HKDF-like construction
    pub fn derive_key_simple(master_key: &[u8], info: &[u8], output_length: usize) -> Vec<u8> {
        let mut output = Vec::new();
        let mut counter = 1u8;
        
        while output.len() < output_length {
            let mut hasher = Sha256::new();
            hasher.update(master_key);
            hasher.update(info);
            hasher.update(&[counter]);
            
            let hash = hasher.finalize();
            output.extend_from_slice(&hash);
            counter += 1;
        }
        
        output.truncate(output_length);
        output
    }
    
    /// Derive multiple keys from master key
    pub fn derive_multiple_keys(master_key: &[u8], contexts: &[&[u8]], key_length: usize) -> Vec<Vec<u8>> {
        contexts.iter()
            .map(|context| Self::derive_key_simple(master_key, context, key_length))
            .collect()
    }
}

/// Secure random number generator for gaming
pub struct SecureRng {
    state: [u8; 32],
    counter: u64,
}

impl SecureRng {
    /// Create new secure RNG with seed from multiple sources plus cryptographic randomness
    pub fn new_from_sources(sources: &[[u8; 32]]) -> Self {
        let mut state = [0u8; 32];
        
        // Combine all sources
        for source in sources {
            for (i, byte) in source.iter().enumerate() {
                state[i] ^= byte;
            }
        }
        
        // Add fresh cryptographic randomness
        use rand::RngCore;
        let mut rng = rand::thread_rng();
        let mut csprng_bytes = [0u8; 32];
        rng.fill_bytes(&mut csprng_bytes);
        
        for (i, byte) in csprng_bytes.iter().enumerate() {
            state[i] ^= byte;
        }
        
        // Add timestamp entropy with fallback
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| std::time::Duration::from_secs(0))
            .as_nanos();
        
        let mut hasher = Sha256::new();
        hasher.update(&state);
        hasher.update(&timestamp.to_be_bytes());
        hasher.update(b"BITCRAPS_SECURE_RNG_V2");
        let final_state = hasher.finalize();
        state.copy_from_slice(&final_state);
        
        Self {
            state,
            counter: 0,
        }
    }
    
    /// Generate next random bytes
    pub fn next_bytes(&mut self, length: usize) -> Vec<u8> {
        let mut output = Vec::new();
        
        while output.len() < length {
            // Update state with counter
            let mut hasher = Sha256::new();
            hasher.update(&self.state);
            hasher.update(&self.counter.to_be_bytes());
            
            let hash = hasher.finalize();
            output.extend_from_slice(&hash);
            
            // Update state for forward secrecy
            self.state.copy_from_slice(&hash);
            self.counter += 1;
        }
        
        output.truncate(length);
        output
    }
    
    /// Generate dice roll using unbiased method
    pub fn roll_dice(&mut self) -> (u8, u8) {
        let bytes = self.next_bytes(16);
        let die1 = GameCrypto::hash_to_die_value(&bytes[0..8]);
        let die2 = GameCrypto::hash_to_die_value(&bytes[8..16]);
        (die1, die2)
    }
}

/// Merkle tree for consensus verification
pub struct MerkleTree {
    leaves: Vec<[u8; 32]>,
    root: [u8; 32],
}

impl MerkleTree {
    /// Build merkle tree from data
    pub fn new(data: &[Vec<u8>]) -> Self {
        let leaves: Vec<[u8; 32]> = data.iter()
            .map(|item| {
                let mut hasher = Sha256::new();
                hasher.update(item);
                let result = hasher.finalize();
                let mut hash = [0u8; 32];
                hash.copy_from_slice(&result);
                hash
            })
            .collect();
        
        let root = Self::compute_root(&leaves);
        
        Self { leaves, root }
    }
    
    /// Get merkle root
    pub fn root(&self) -> [u8; 32] {
        self.root
    }
    
    /// Compute merkle root from leaves
    fn compute_root(leaves: &[[u8; 32]]) -> [u8; 32] {
        if leaves.is_empty() {
            return [0u8; 32];
        }
        
        if leaves.len() == 1 {
            return leaves[0];
        }
        
        let mut current_level = leaves.to_vec();
        
        while current_level.len() > 1 {
            let mut next_level = Vec::new();
            
            for chunk in current_level.chunks(2) {
                let mut hasher = Sha256::new();
                hasher.update(&chunk[0]);
                
                if chunk.len() > 1 {
                    hasher.update(&chunk[1]);
                } else {
                    // Odd number of nodes, duplicate the last one
                    hasher.update(&chunk[0]);
                }
                
                let result = hasher.finalize();
                let mut hash = [0u8; 32];
                hash.copy_from_slice(&result);
                next_level.push(hash);
            }
            
            current_level = next_level;
        }
        
        current_level[0]
    }
    
    /// Generate merkle proof for a leaf
    pub fn generate_proof(&self, index: usize) -> Option<Vec<[u8; 32]>> {
        if index >= self.leaves.len() {
            return None;
        }
        
        let mut proof = Vec::new();
        let mut current_level = self.leaves.clone();
        let mut current_index = index;
        
        while current_level.len() > 1 {
            // Find sibling
            let sibling_index = if current_index % 2 == 0 {
                current_index + 1
            } else {
                current_index - 1
            };
            
            if sibling_index < current_level.len() {
                proof.push(current_level[sibling_index]);
            } else {
                // Duplicate for odd number of nodes
                proof.push(current_level[current_index]);
            }
            
            // Move to next level
            let mut next_level = Vec::new();
            for chunk in current_level.chunks(2) {
                let mut hasher = Sha256::new();
                hasher.update(&chunk[0]);
                hasher.update(&chunk.get(1).unwrap_or(&chunk[0]));
                
                let result = hasher.finalize();
                let mut hash = [0u8; 32];
                hash.copy_from_slice(&result);
                next_level.push(hash);
            }
            
            current_level = next_level;
            current_index /= 2;
        }
        
        Some(proof)
    }
    
    /// Verify merkle proof with position information
    pub fn verify_proof_with_index(leaf: &[u8; 32], proof: &[[u8; 32]], root: &[u8; 32], mut index: usize) -> bool {
        if proof.is_empty() {
            return leaf == root;
        }
        
        let mut current_hash = *leaf;
        
        for sibling in proof {
            let mut hasher = Sha256::new();
            
            // Use consistent left-to-right order like compute_root
            if index % 2 == 0 {
                // Current node is on the left, sibling on the right
                hasher.update(&current_hash);
                hasher.update(sibling);
            } else {
                // Current node is on the right, sibling on the left
                hasher.update(sibling);
                hasher.update(&current_hash);
            }
            
            let result = hasher.finalize();
            let mut hash = [0u8; 32];
            hash.copy_from_slice(&result);
            current_hash = hash;
            
            index /= 2;
        }
        
        current_hash.ct_eq(root).into()
    }
    
    /// Verify merkle proof (backward compatibility - assumes index 0)
    pub fn verify_proof(leaf: &[u8; 32], proof: &[[u8; 32]], root: &[u8; 32]) -> bool {
        Self::verify_proof_with_index(leaf, proof, root, 0)
    }
}

/// Utilities for consensus and state verification
pub fn hash_dice(die1: u8, die2: u8) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"BITCRAPS_DICE_HASH");
    hasher.update(&[die1, die2]);
    
    let result = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}

pub fn compute_merkle_root<T: AsRef<[u8]>>(items: &[T]) -> [u8; 32] {
    let data: Vec<Vec<u8>> = items.iter()
        .map(|item| item.as_ref().to_vec())
        .collect();
    
    let tree = MerkleTree::new(&data);
    tree.root()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_keypair_generation() {
        let keypair = BitchatKeypair::generate();
        let message = b"test message";
        let signature = keypair.sign(message);
        assert!(keypair.verify(message, &signature));
    }
    
    #[test]
    fn test_proof_of_work() {
        let keypair = BitchatKeypair::generate();
        let identity = BitchatIdentity::from_keypair_with_pow(keypair, 8);
        assert!(identity.verify_pow());
    }
    
    #[test]
    fn test_commitment_scheme() {
        let secret = [42u8; 32];
        let commitment = GameCrypto::commit_randomness(&secret);
        assert!(GameCrypto::verify_commitment(&commitment, &secret));
        
        let wrong_secret = [43u8; 32];
        assert!(!GameCrypto::verify_commitment(&commitment, &wrong_secret));
    }
    
    #[test]
    fn test_randomness_combination() {
        let sources = vec![
            [1u8; 32],
            [2u8; 32],
            [3u8; 32],
        ];
        
        let (die1, die2) = GameCrypto::combine_randomness(&sources);
        assert!(die1 >= 1 && die1 <= 6);
        assert!(die2 >= 1 && die2 <= 6);
    }
    
    #[test]
    fn test_merkle_tree() {
        let data = vec![
            b"transaction1".to_vec(),
            b"transaction2".to_vec(),
            b"transaction3".to_vec(),
        ];
        
        let tree = MerkleTree::new(&data);
        let root = tree.root();
        
        // Test proof generation and verification
        let proof = tree.generate_proof(0).unwrap();
        let leaf_hash = {
            let mut hasher = Sha256::new();
            hasher.update(&data[0]);
            let result = hasher.finalize();
            let mut hash = [0u8; 32];
            hash.copy_from_slice(&result);
            hash
        };
        
        assert!(MerkleTree::verify_proof(&leaf_hash, &proof, &root));
    }
    
    #[test]
    fn test_secure_rng() {
        let sources = vec![
            [1u8; 32],
            [2u8; 32],
        ];
        
        let mut rng = SecureRng::new_from_sources(&sources);
        let (die1, die2) = rng.roll_dice();
        
        assert!(die1 >= 1 && die1 <= 6);
        assert!(die2 >= 1 && die2 <= 6);
    }
    
    #[test]
    fn test_pbkdf2_key_derivation() {
        let password = b"test_password";
        let salt = b"test_salt_123";
        let iterations = 50_000; // Will be increased to 100k minimum
        let output_length = 32;
        
        let result = KeyDerivation::derive_key_pbkdf2(password, salt, iterations, output_length);
        assert!(result.is_ok());
        
        let key = result.unwrap();
        assert_eq!(key.len(), output_length);
        
        // Ensure different passwords produce different keys
        let result2 = KeyDerivation::derive_key_pbkdf2(b"different_password", salt, iterations, output_length);
        assert!(result2.is_ok());
        
        let key2 = result2.unwrap();
        assert_ne!(key, key2);
        
        // Ensure same password produces same key
        let result3 = KeyDerivation::derive_key_pbkdf2(password, salt, iterations, output_length);
        assert!(result3.is_ok());
        
        let key3 = result3.unwrap();
        assert_eq!(key, key3);
    }
}