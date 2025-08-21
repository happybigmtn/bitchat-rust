// src/crypto/keys.rs
use ed25519_dalek::{SigningKey, VerifyingKey, Signature, Signer, Verifier, SecretKey};
use rand::{rngs::OsRng, RngCore, CryptoRng};
use crate::protocol::{PeerId, GameId, RandomnessReveal, DiceRoll};
use crate::protocol::constants::NONCE_SIZE;

#[derive(Debug, Clone)]
pub struct NoiseKeyPair {
    pub private: [u8; 32],
    pub public: [u8; 32],
}

impl NoiseKeyPair {
    pub fn generate() -> Self {
        let mut private = [0u8; 32];
        getrandom::getrandom(&mut private).expect("Failed to generate random bytes");
        
        // Use x25519-dalek for key derivation if needed, but for now use direct bytes
        let public = Self::derive_public(&private);
        Self { private, public }
    }
    
    pub fn from_bytes(private_bytes: [u8; 32]) -> Self {
        let public = Self::derive_public(&private_bytes);
        Self {
            private: private_bytes,
            public,
        }
    }
    
    pub fn public_bytes(&self) -> [u8; 32] {
        self.public
    }
    
    pub fn private_bytes(&self) -> [u8; 32] {
        self.private
    }
    
    // For now, use a simple derivation - in production you'd use actual X25519
    fn derive_public(private: &[u8; 32]) -> [u8; 32] {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(b"X25519_PUBLIC");
        hasher.update(private);
        hasher.finalize().into()
    }
}

#[derive(Debug, Clone)]
pub struct SigningKeyPair {
    pub signing_key: SigningKey,
    pub verifying_key: VerifyingKey,
}

impl SigningKeyPair {
    pub fn generate() -> Self {
        let mut secret_bytes = [0u8; 32];
        getrandom::getrandom(&mut secret_bytes).expect("Failed to generate random bytes");
        let signing_key = SigningKey::from_bytes(&secret_bytes);
        let verifying_key = signing_key.verifying_key();
        Self {
            signing_key,
            verifying_key,
        }
    }
    
    pub fn from_bytes(private_bytes: [u8; 32]) -> Result<Self, ed25519_dalek::SignatureError> {
        let signing_key = SigningKey::from_bytes(&private_bytes);
        let verifying_key = signing_key.verifying_key();
        Ok(Self {
            signing_key,
            verifying_key,
        })
    }
    
    pub fn sign(&self, message: &[u8]) -> Signature {
        self.signing_key.sign(message)
    }
    
    pub fn verify(
        verifying_key: &VerifyingKey,
        message: &[u8],
        signature: &Signature,
    ) -> Result<(), ed25519_dalek::SignatureError> {
        verifying_key.verify(message, signature)
    }
    
    pub fn public_bytes(&self) -> [u8; 32] {
        self.verifying_key.to_bytes()
    }
    
    pub fn private_bytes(&self) -> [u8; 32] {
        self.signing_key.to_bytes()
    }
}

/// Combined identity containing both key pairs
#[derive(Debug, Clone)]
pub struct BitchatIdentity {
    pub noise_keypair: NoiseKeyPair,
    pub signing_keypair: SigningKeyPair,
}

impl BitchatIdentity {
    pub fn generate() -> Self {
        Self {
            noise_keypair: NoiseKeyPair::generate(),
            signing_keypair: SigningKeyPair::generate(),
        }
    }
    
    pub fn peer_id(&self) -> PeerId {
        PeerId::from_public_key(&self.noise_keypair.public_bytes())
    }
}

/// Gaming-specific cryptographic operations
#[derive(Debug, Clone)]
pub struct GameCrypto {
    identity: BitchatIdentity,
}

impl GameCrypto {
    pub fn new(identity: BitchatIdentity) -> Self {
        Self { identity }
    }
    
    /// Generate a cryptographically secure nonce for randomness commitment
    pub fn generate_nonce() -> [u8; NONCE_SIZE] {
        let mut nonce = [0u8; NONCE_SIZE];
        getrandom::getrandom(&mut nonce).expect("Failed to generate random bytes");
        nonce
    }
    
    /// Derive a game-specific key for encryption
    pub fn derive_game_key(&self, game_id: &GameId) -> [u8; 32] {
        use sha2::{Sha256, Digest};
        
        let mut hasher = Sha256::new();
        hasher.update(b"BITCRAPS_GAME_KEY");
        hasher.update(self.identity.noise_keypair.private_bytes());
        hasher.update(game_id.as_bytes());
        
        hasher.finalize().into()
    }
    
    /// Create a verifiable random seed from multiple player commitments
    pub fn combine_randomness(reveals: &[RandomnessReveal]) -> Result<[u8; 32], String> {
        if reveals.is_empty() {
            return Err("No randomness reveals provided".to_string());
        }
        
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        
        // Sort by player ID to ensure deterministic ordering
        let mut sorted_reveals = reveals.to_vec();
        sorted_reveals.sort_by_key(|r| r.player_id);
        
        for reveal in sorted_reveals {
            hasher.update(&reveal.nonce);
            hasher.update(reveal.player_id.as_bytes());
        }
        
        Ok(hasher.finalize().into())
    }
    
    /// Generate dice roll from combined randomness seed
    pub fn generate_dice_roll(seed: &[u8; 32], round: u64) -> DiceRoll {
        use sha2::{Sha256, Digest};
        
        let mut hasher = Sha256::new();
        hasher.update(seed);
        hasher.update(&round.to_be_bytes());
        hasher.update(b"DICE_ROLL");
        
        let hash = hasher.finalize();
        
        // Use first two bytes to determine dice values (1-6 each)
        let die1 = ((hash[0] as u16 * 6) / 256) as u8 + 1;
        let die2 = ((hash[1] as u16 * 6) / 256) as u8 + 1;
        
        // Ensure dice are in valid range (1-6)
        let die1 = die1.clamp(1, 6);
        let die2 = die2.clamp(1, 6);
        
        DiceRoll::new(die1, die2)
    }
    
    /// Sign game data with identity
    pub fn sign_game_data(&self, data: &[u8]) -> Signature {
        self.identity.signing_keypair.sign(data)
    }
    
    /// Verify game data signature
    pub fn verify_game_signature(
        &self,
        data: &[u8],
        signature: &Signature,
        public_key: &VerifyingKey,
    ) -> Result<(), ed25519_dalek::SignatureError> {
        SigningKeyPair::verify(public_key, data, signature)
    }
    
    /// Create a hash-based commitment for bet amounts (to prevent front-running)
    pub fn create_bet_commitment(&self, bet_amount: u64, nonce: &[u8; 16]) -> [u8; 32] {
        use sha2::{Sha256, Digest};
        
        let mut hasher = Sha256::new();
        hasher.update(b"BET_COMMITMENT");
        hasher.update(&bet_amount.to_be_bytes());
        hasher.update(nonce);
        hasher.update(self.identity.peer_id().as_bytes());
        
        hasher.finalize().into()
    }
    
    /// Verify a bet commitment
    pub fn verify_bet_commitment(
        &self,
        commitment: &[u8; 32],
        bet_amount: u64,
        nonce: &[u8; 16],
        player_id: &PeerId,
    ) -> bool {
        use sha2::{Sha256, Digest};
        
        let mut hasher = Sha256::new();
        hasher.update(b"BET_COMMITMENT");
        hasher.update(&bet_amount.to_be_bytes());
        hasher.update(nonce);
        hasher.update(player_id.as_bytes());
        
        let computed: [u8; 32] = hasher.finalize().into();
        computed == *commitment
    }
}