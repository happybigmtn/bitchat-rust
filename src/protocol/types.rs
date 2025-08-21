// src/protocol/types.rs
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Serialize, Deserialize};
use crate::protocol::constants::*;

/// Represents a unique 32-byte identifier for peers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PeerId([u8; 32]);

impl PeerId {
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
    
    pub fn from_public_key(public_key: &[u8; 32]) -> Self {
        Self(*public_key)
    }
}

/// Represents a message ID for deduplication
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageId([u8; 16]);

impl MessageId {
    pub fn new() -> Self {
        let mut bytes = [0u8; 16];
        use rand::RngCore;
        rand::thread_rng().fill_bytes(&mut bytes);
        Self(bytes)
    }
    
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }
    
    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

/// Core packet structure matching the Swift implementation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitchatPacket {
    pub version: u8,
    pub packet_type: u8,
    pub ttl: u8,
    pub timestamp: u64,         // Unix timestamp in seconds
    pub flags: u8,
    pub payload_length: u16,
    pub sender_id: PeerId,
    pub recipient_id: Option<PeerId>,  // Present if FLAG_RECIPIENT_PRESENT
    pub payload: Vec<u8>,
    pub signature: Option<Vec<u8>>,    // Present if FLAG_SIGNATURE_PRESENT
}

impl BitchatPacket {
    pub fn new(
        packet_type: u8,
        sender_id: PeerId,
        payload: Vec<u8>,
    ) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
            
        Self {
            version: PROTOCOL_VERSION,
            packet_type,
            ttl: MAX_TTL,
            timestamp,
            flags: 0,
            payload_length: payload.len() as u16,
            sender_id,
            recipient_id: None,
            payload,
            signature: None,
        }
    }
    
    pub fn with_recipient(mut self, recipient_id: PeerId) -> Self {
        self.recipient_id = Some(recipient_id);
        self.flags |= FLAG_RECIPIENT_PRESENT;
        self
    }
    
    pub fn with_signature(mut self, signature: Vec<u8>) -> Self {
        self.signature = Some(signature);
        self.flags |= FLAG_SIGNATURE_PRESENT;
        self
    }
    
    pub fn is_expired(&self, max_age_seconds: u64) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        now.saturating_sub(self.timestamp) > max_age_seconds
    }
}

/// Gaming-specific data structures for BitCraps

/// Represents a unique game identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GameId([u8; 16]);

impl GameId {
    pub fn new() -> Self {
        let mut bytes = [0u8; 16];
        use rand::RngCore;
        rand::thread_rng().fill_bytes(&mut bytes);
        Self(bytes)
    }
    
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }
    
    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

/// CRAP token balance and operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrapTokens(u64);

impl CrapTokens {
    pub fn new(amount: u64) -> Self {
        Self(amount)
    }
    
    pub fn amount(&self) -> u64 {
        self.0
    }
    
    pub fn can_subtract(&self, amount: u64) -> bool {
        self.0 >= amount
    }
    
    pub fn subtract(&mut self, amount: u64) -> Result<(), &'static str> {
        if self.can_subtract(amount) {
            self.0 -= amount;
            Ok(())
        } else {
            Err("Insufficient tokens")
        }
    }
    
    pub fn add(&mut self, amount: u64) {
        self.0 = self.0.saturating_add(amount);
    }
}

/// Represents different types of craps bets
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BetType {
    Pass,           // Betting on the shooter to win
    DontPass,       // Betting against the shooter
    Come,           // Like pass but after come-out roll
    DontCome,       // Like don't pass but after come-out roll
    Field,          // Single roll bet
    Any7,           // Single roll bet on 7
    Any11,          // Single roll bet on 11
    AnyCraps,       // Single roll bet on 2, 3, or 12
    Hardway(u8),    // Betting on doubles (4, 6, 8, 10)
    Place(u8),      // Betting on specific number before 7
}

/// Represents a bet in the game
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bet {
    pub id: MessageId,
    pub player_id: PeerId,
    pub game_id: GameId,
    pub bet_type: BetType,
    pub amount: CrapTokens,
    pub timestamp: u64,
}

/// Game phases for craps
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GamePhase {
    WaitingForPlayers,
    ComeOutRoll,
    Point(u8),           // Point is established
    Resolved,
}

/// Game state for a craps game
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    pub game_id: GameId,
    pub host_id: PeerId,
    pub phase: GamePhase,
    pub point: Option<u8>,
    pub players: Vec<PeerId>,
    pub bets: Vec<Bet>,
    pub total_pot: CrapTokens,
    pub created_at: u64,
    pub last_roll: Option<DiceRoll>,
}

/// Represents a dice roll result
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiceRoll {
    pub die1: u8,
    pub die2: u8,
    pub timestamp: u64,
}

impl DiceRoll {
    pub fn new(die1: u8, die2: u8) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        Self { die1, die2, timestamp }
    }
    
    pub fn total(&self) -> u8 {
        self.die1 + self.die2
    }
    
    pub fn is_hard_way(&self) -> bool {
        self.die1 == self.die2 && [4, 6, 8, 10].contains(&self.total())
    }
    
    pub fn is_craps(&self) -> bool {
        matches!(self.total(), 2 | 3 | 12)
    }
    
    pub fn is_natural(&self) -> bool {
        matches!(self.total(), 7 | 11)
    }
}

/// Cryptographic commitment for fair randomness
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RandomnessCommitment {
    pub commitment: [u8; COMMITMENT_SIZE],  // SHA-256 hash
    pub player_id: PeerId,
    pub game_id: GameId,
    pub timestamp: u64,
}

impl RandomnessCommitment {
    pub fn new(nonce: &[u8; NONCE_SIZE], player_id: PeerId, game_id: GameId) -> Self {
        use sha2::{Sha256, Digest};
        
        let mut hasher = Sha256::new();
        hasher.update(nonce);
        hasher.update(player_id.as_bytes());
        hasher.update(game_id.as_bytes());
        
        let commitment = hasher.finalize().into();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
            
        Self {
            commitment,
            player_id,
            game_id,
            timestamp,
        }
    }
    
    pub fn verify(&self, nonce: &[u8; NONCE_SIZE]) -> bool {
        use sha2::{Sha256, Digest};
        
        let mut hasher = Sha256::new();
        hasher.update(nonce);
        hasher.update(self.player_id.as_bytes());
        hasher.update(self.game_id.as_bytes());
        
        let computed: [u8; COMMITMENT_SIZE] = hasher.finalize().into();
        computed == self.commitment
    }
}

/// Randomness reveal containing the nonce
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RandomnessReveal {
    pub nonce: [u8; NONCE_SIZE],
    pub player_id: PeerId,
    pub game_id: GameId,
    pub timestamp: u64,
}

/// Game result with payouts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameResult {
    pub game_id: GameId,
    pub final_roll: DiceRoll,
    pub winning_bets: Vec<(PeerId, CrapTokens)>,
    pub losing_bets: Vec<(PeerId, CrapTokens)>,
    pub house_edge: CrapTokens,
    pub timestamp: u64,
}