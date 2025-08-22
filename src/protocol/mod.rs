//! Protocol implementation for BitCraps
//! 
//! This module implements the core binary protocol for BitCraps decentralized casino.
//! Features:
//! - TLV (Type-Length-Value) encoding for extensible messages
//! - LZ4 compression for bandwidth efficiency
//! - Gaming-specific packet types (bets, dice rolls, payouts)
//! - Mesh routing with TTL management
//! - Session management integration

pub mod binary;
pub mod optimized_binary;
pub mod craps;
pub mod runtime;
pub mod compact_state;

// New refactored modules
pub mod bet_types;
pub mod game_logic;
pub mod resolution;
pub mod payouts;

// Efficient optimized modules
pub mod efficient_game_state;
pub mod efficient_bet_resolution;
pub mod efficient_consensus;
pub mod efficient_history;

// Modular components
pub mod consensus;
pub mod efficient_sync;
pub mod benchmark;

use std::collections::HashSet;
use std::io::{Read, Write};
use std::time::{SystemTime, UNIX_EPOCH};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use lz4_flex::{compress_prepend_size, decompress_size_prepended};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

// Protocol constants
pub const NONCE_SIZE: usize = 32;
pub const COMMITMENT_SIZE: usize = 32;
pub const SIGNATURE_SIZE: usize = 64;

// Flag bit positions
pub const FLAG_RECIPIENT_PRESENT: u8 = 0x01;    // Bit 0
pub const FLAG_SIGNATURE_PRESENT: u8 = 0x02;    // Bit 1
pub const FLAG_PAYLOAD_COMPRESSED: u8 = 0x04;   // Bit 2
pub const FLAG_GAMING_MESSAGE: u8 = 0x08;       // Bit 3
// Bits 4-7 reserved for future use

// Gaming constants
pub const INITIAL_CRAP_TOKENS: u64 = 1000;
pub const MIN_BET_AMOUNT: u64 = 1;
pub const MAX_BET_AMOUNT: u64 = 100;

/// Peer identifier - 32 bytes for Ed25519 public key compatibility
pub type PeerId = [u8; 32];

/// Game identifier - 16 bytes UUID
pub type GameId = [u8; 16];

/// Helper function to create a new GameId using cryptographic randomness
pub fn new_game_id() -> GameId {
    let mut game_id = [0u8; 16];
    use rand::RngCore;
    let mut rng = rand::thread_rng();
    rng.fill_bytes(&mut game_id);
    game_id
}

/// Packet type constants for BitCraps protocol
pub const PACKET_TYPE_PING: u8 = 0x01;
pub const PACKET_TYPE_PONG: u8 = 0x02;
pub const PACKET_TYPE_CHAT: u8 = 0x10;
// Missing gaming packet types (0x18-0x1F)
pub const PACKET_TYPE_RANDOMNESS_COMMIT: u8 = 0x18;
pub const PACKET_TYPE_RANDOMNESS_REVEAL: u8 = 0x19;
pub const PACKET_TYPE_GAME_PHASE_CHANGE: u8 = 0x1A;
pub const PACKET_TYPE_PLAYER_READY: u8 = 0x1B;
pub const PACKET_TYPE_CONSENSUS_VOTE: u8 = 0x1C;
pub const PACKET_TYPE_DISPUTE_CLAIM: u8 = 0x1D;
pub const PACKET_TYPE_PAYOUT_CLAIM: u8 = 0x1E;
pub const PACKET_TYPE_GAME_COMPLETE: u8 = 0x1F;
// Standard gaming packet types
pub const PACKET_TYPE_GAME_CREATE: u8 = 0x20;
pub const PACKET_TYPE_GAME_JOIN: u8 = 0x21;
pub const PACKET_TYPE_GAME_BET: u8 = 0x22;
pub const PACKET_TYPE_GAME_ROLL_COMMIT: u8 = 0x23;
pub const PACKET_TYPE_GAME_ROLL_REVEAL: u8 = 0x24;
pub const PACKET_TYPE_GAME_RESULT: u8 = 0x25;
pub const PACKET_TYPE_TOKEN_TRANSFER: u8 = 0x30;
pub const PACKET_TYPE_TOKEN_MINE: u8 = 0x31;
pub const PACKET_TYPE_ROUTING: u8 = 0x40;
pub const PACKET_TYPE_SESSION_HANDSHAKE: u8 = 0x50;

/// TLV type constants
pub const TLV_TYPE_SENDER: u8 = 0x01;
pub const TLV_TYPE_RECEIVER: u8 = 0x02;
pub const TLV_TYPE_TIMESTAMP: u8 = 0x03;
pub const TLV_TYPE_GAME_ID: u8 = 0x10;
pub const TLV_TYPE_BET_TYPE: u8 = 0x11;
pub const TLV_TYPE_BET_AMOUNT: u8 = 0x12;
pub const TLV_TYPE_DICE_VALUE: u8 = 0x13;
pub const TLV_TYPE_COMMITMENT: u8 = 0x14;
pub const TLV_TYPE_REVEAL: u8 = 0x15;
pub const TLV_TYPE_SIGNATURE: u8 = 0x16;
pub const TLV_TYPE_COMPRESSED_PAYLOAD: u8 = 0x80;
pub const TLV_TYPE_ROUTING_INFO: u8 = 0x81;
pub const TLV_TYPE_SOURCE_ID: u8 = 0x82;
pub const TLV_TYPE_TARGET_ID: u8 = 0x83;
pub const TLV_TYPE_SEQUENCE: u8 = 0x84;
pub const TLV_TYPE_PAYLOAD: u8 = 0x85;

/// BitCraps packet structure
#[derive(Debug, Clone, PartialEq)]
pub struct BitchatPacket {
    pub version: u8,
    pub packet_type: u8,
    pub flags: u8,
    pub ttl: u8,
    pub total_length: u16,
    pub checksum: u16,
    pub tlv_data: Vec<TlvField>,
    // Convenience fields extracted from TLV
    pub source: PeerId,
    pub target: PeerId,
    pub sequence: u32,
    pub payload: Option<Vec<u8>>,
}

/// TLV field structure
#[derive(Debug, Clone, PartialEq)]
pub struct TlvField {
    pub field_type: u8,
    pub length: u16,
    pub value: Vec<u8>,
}

/// Gaming-specific data structures
/// Feynman: Every possible way to bet on dice - from simple to complex
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum BetType {
    // Core Line Bets (0-3)
    // Feynman: These are the "main story" of craps - will the shooter succeed or fail?
    Pass = 0,           // Pass Line: Shooter wins
    DontPass = 1,       // Don't Pass: Shooter loses  
    Come = 2,           // Come: Like Pass but starts mid-game
    DontCome = 3,       // Don't Come: Like Don't Pass mid-game
    
    // Field Bet (4)
    // Feynman: A "scatter shot" bet covering many numbers in one wager
    Field = 4,          // Field: 2,3,4,9,10,11,12 win
    
    // YES Bets - Number Before 7 (5-14)
    // Feynman: "I bet this number shows up before a 7 does"
    Yes2 = 5,           // 2 before 7
    Yes3 = 6,           // 3 before 7
    Yes4 = 7,           // 4 before 7
    Yes5 = 8,           // 5 before 7
    Yes6 = 9,           // 6 before 7
    Yes8 = 10,          // 8 before 7 (no 7!)
    Yes9 = 11,          // 9 before 7
    Yes10 = 12,         // 10 before 7
    Yes11 = 13,         // 11 before 7
    Yes12 = 14,         // 12 before 7
    
    // NO Bets - 7 Before Number (15-24)
    // Feynman: "I bet 7 shows up before this number does"
    No2 = 15,           // 7 before 2
    No3 = 16,           // 7 before 3
    No4 = 17,           // 7 before 4
    No5 = 18,           // 7 before 5
    No6 = 19,           // 7 before 6
    No8 = 20,           // 7 before 8
    No9 = 21,           // 7 before 9
    No10 = 22,          // 7 before 10
    No11 = 23,          // 7 before 11
    No12 = 24,          // 7 before 12
    
    // Hardways Bets (25-28)
    // Feynman: "I bet this sum comes up as doubles (the hard way)"
    Hard4 = 25,         // Hard 4 (2+2)
    Hard6 = 26,         // Hard 6 (3+3)  
    Hard8 = 27,         // Hard 8 (4+4)
    Hard10 = 28,        // Hard 10 (5+5)
    
    // Odds Bets (29-32)
    // Feynman: "True odds" bets with ZERO house edge - pure probability
    OddsPass = 29,      // Pass line odds
    OddsDontPass = 30,  // Don't pass odds
    OddsCome = 31,      // Come bet odds
    OddsDontCome = 32,  // Don't come odds
    
    // Special/Bonus Bets (33-42)
    // Feynman: "Achievement" bets - like video game accomplishments
    HotRoller = 33,         // Progressive win streak
    Fire = 34,              // Make 4-6 unique points
    TwiceHard = 35,         // Same hardway twice in a row
    RideLine = 36,          // Pass line win streak
    Muggsy = 37,            // 7 on comeout or after point
    BonusSmall = 38,        // All 2-6 before any 7
    BonusTall = 39,         // All 8-12 before any 7
    BonusAll = 40,          // All numbers (2-12) except 7
    Replay = 41,            // Same point 3+ times
    DifferentDoubles = 42,  // All unique doubles before 7
    
    // NEXT Bets - One-Roll Proposition (43-53)
    // Feynman: "Next roll only" bets - instant gratification gambling
    Next2 = 43,         // Next roll is 2
    Next3 = 44,         // Next roll is 3
    Next4 = 45,         // Next roll is 4
    Next5 = 46,         // Next roll is 5
    Next6 = 47,         // Next roll is 6
    Next7 = 48,         // Next roll is 7
    Next8 = 49,         // Next roll is 8
    Next9 = 50,         // Next roll is 9
    Next10 = 51,        // Next roll is 10
    Next11 = 52,        // Next roll is 11
    Next12 = 53,        // Next roll is 12
    
    // Repeater Bets (54-63)
    // Feynman: "Endurance" bets - can this number appear N times?
    Repeater2 = 54,     // 2 must appear 2 times before 7
    Repeater3 = 55,     // 3 must appear 3 times before 7
    Repeater4 = 56,     // 4 must appear 4 times before 7
    Repeater5 = 57,     // 5 must appear 5 times before 7
    Repeater6 = 58,     // 6 must appear 6 times before 7
    Repeater8 = 59,     // 8 must appear 6 times before 7
    Repeater9 = 60,     // 9 must appear 5 times before 7
    Repeater10 = 61,    // 10 must appear 4 times before 7
    Repeater11 = 62,    // 11 must appear 3 times before 7
    Repeater12 = 63,    // 12 must appear 2 times before 7
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CrapTokens {
    pub amount: u64, // Amount in smallest unit (like satoshis)
}

impl CrapTokens {
    pub fn new(amount: u64) -> crate::error::Result<Self> {
        if amount == 0 {
            return Err(crate::error::Error::InvalidData("Token amount cannot be zero".to_string()));
        }
        if amount > u64::MAX / 2 {
            return Err(crate::error::Error::InvalidData("Token amount too large".to_string()));
        }
        Ok(Self { amount })
    }
    
    /// Create tokens without validation (for internal use)
    pub fn new_unchecked(amount: u64) -> Self {
        Self { amount }
    }
    
    pub fn amount(&self) -> u64 {
        self.amount
    }
    
    pub fn from_crap(crap: f64) -> crate::error::Result<Self> {
        if crap < 0.0 {
            return Err(crate::error::Error::InvalidData("CRAP amount cannot be negative".to_string()));
        }
        if crap > (u64::MAX as f64 / 2.0) / 1_000_000.0 {
            return Err(crate::error::Error::InvalidData("CRAP amount too large".to_string()));
        }
        
        let amount = (crap * 1_000_000.0) as u64;
        if amount == 0 && crap > 0.0 {
            return Err(crate::error::Error::InvalidData("CRAP amount too small (below minimum unit)".to_string()));
        }
        
        Ok(Self { amount })
    }
    
    pub fn to_crap(&self) -> f64 {
        self.amount as f64 / 1_000_000.0
    }
    
    /// Add tokens with overflow checking
    pub fn checked_add(&self, other: &CrapTokens) -> crate::error::Result<CrapTokens> {
        self.amount.checked_add(other.amount)
            .map(|amount| CrapTokens { amount })
            .ok_or_else(|| crate::error::Error::InvalidData("Token addition overflow".to_string()))
    }
    
    /// Subtract tokens with underflow checking
    pub fn checked_sub(&self, other: &CrapTokens) -> crate::error::Result<CrapTokens> {
        self.amount.checked_sub(other.amount)
            .map(|amount| CrapTokens { amount })
            .ok_or_else(|| crate::error::Error::InsufficientBalance)
    }
}

/// Represents a dice roll result
/// Feynman: Two cubes, each showing 1-6, determine everyone's fate
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiceRoll {
    pub die1: u8,
    pub die2: u8,
    pub timestamp: u64,
}

impl DiceRoll {
    pub fn new(die1: u8, die2: u8) -> crate::error::Result<Self> {
        // Validate dice values
        if die1 < 1 || die1 > 6 {
            return Err(crate::error::Error::InvalidData(format!("Invalid die1 value: {}, must be 1-6", die1)));
        }
        if die2 < 1 || die2 > 6 {
            return Err(crate::error::Error::InvalidData(format!("Invalid die2 value: {}, must be 1-6", die2)));
        }
        
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| std::time::Duration::from_secs(0))
            .as_secs();
        Ok(Self { die1, die2, timestamp })
    }
    
    /// Create dice roll without validation (for internal use)
    pub fn new_unchecked(die1: u8, die2: u8) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| std::time::Duration::from_secs(0))
            .as_secs();
        Self { die1, die2, timestamp }
    }
    
    /// Feynman: The sum is what matters in craps - 2 through 12
    pub fn total(&self) -> u8 {
        self.die1 + self.die2
    }
    
    /// Feynman: "Hard way" means doubles - harder to roll than mixed
    pub fn is_hard_way(&self) -> bool {
        self.die1 == self.die2 && [4, 6, 8, 10].contains(&self.total())
    }
    
    /// Feynman: "Craps" are the losing numbers on comeout - 2, 3, or 12
    pub fn is_craps(&self) -> bool {
        matches!(self.total(), 2 | 3 | 12)
    }
    
    /// Feynman: "Natural" winners on comeout - lucky 7 or 11
    pub fn is_natural(&self) -> bool {
        matches!(self.total(), 7 | 11)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bet {
    pub id: [u8; 16],
    pub game_id: GameId,
    pub player: PeerId,
    pub bet_type: BetType,
    pub amount: CrapTokens,
    pub timestamp: u64,
}

impl Bet {
    /// Create a new bet with validation
    pub fn new(
        id: [u8; 16],
        game_id: GameId,
        player: PeerId,
        bet_type: BetType,
        amount: CrapTokens,
    ) -> crate::error::Result<Self> {
        // Validate bet amount
        if amount.amount < MIN_BET_AMOUNT {
            return Err(crate::error::Error::InvalidBet(
                format!("Bet amount {} below minimum {}", amount.amount, MIN_BET_AMOUNT)
            ));
        }
        if amount.amount > MAX_BET_AMOUNT {
            return Err(crate::error::Error::InvalidBet(
                format!("Bet amount {} exceeds maximum {}", amount.amount, MAX_BET_AMOUNT)
            ));
        }
        
        // Validate bet amount is not zero
        if amount.amount == 0 {
            return Err(crate::error::Error::InvalidBet(
                "Bet amount cannot be zero".to_string()
            ));
        }
        
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| std::time::Duration::from_secs(0))
            .as_secs();
        
        Ok(Self {
            id,
            game_id,
            player,
            bet_type,
            amount,
            timestamp,
        })
    }
    
    /// Validate if this bet type is allowed in the current game phase
    pub fn is_valid_for_phase(&self, phase: &crate::protocol::craps::GamePhase) -> bool {
        use crate::protocol::craps::GamePhase;
        
        match phase {
            GamePhase::ComeOut => {
                // Most bets allowed on comeout, except some odds bets
                !matches!(self.bet_type, BetType::OddsPass | BetType::OddsDontPass)
            },
            GamePhase::Point => {
                // All bets allowed during point phase
                true
            },
            GamePhase::Ended | GamePhase::GameEnded => {
                // No new bets allowed in ended games
                false
            },
        }
    }
}

/// Cryptographic commitment for fair randomness
/// Feynman: Like putting your answer in a sealed envelope before the game starts
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RandomnessCommitment {
    pub commitment: [u8; COMMITMENT_SIZE], // SHA256 hash of nonce
    pub player_id: PeerId,
    pub game_id: GameId,
    pub round: u32,
    pub timestamp: u64,
}

/// Reveal phase of commit-reveal protocol
/// Feynman: Opening the envelope to prove you didn't cheat
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RandomnessReveal {
    pub nonce: [u8; NONCE_SIZE],
    pub player_id: PeerId,
    pub game_id: GameId,
    pub round: u32,
    pub timestamp: u64,
}

/// Routing information for mesh networking
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoutingInfo {
    pub source: PeerId,
    pub destination: Option<PeerId>, // None for broadcast
    pub route_history: Vec<PeerId>,
    pub max_hops: u8,
}

impl BitchatPacket {
    /// Create a new packet with basic fields
    pub fn new(packet_type: u8, ttl: u8) -> Self {
        Self {
            version: 1,
            packet_type,
            flags: 0,
            ttl,
            total_length: 0, // Will be calculated during serialization
            checksum: 0, // Will be calculated during serialization
            tlv_data: Vec::new(),
            source: [0u8; 32],
            target: [0u8; 32],
            sequence: 0,
            payload: None,
        }
    }
    
    /// Add a TLV field to the packet
    pub fn add_tlv(&mut self, field_type: u8, value: Vec<u8>) {
        let tlv = TlvField {
            field_type,
            length: value.len() as u16,
            value,
        };
        self.tlv_data.push(tlv);
    }
    
    /// Add sender information
    pub fn add_sender(&mut self, sender: PeerId) {
        self.add_tlv(TLV_TYPE_SENDER, sender.to_vec());
    }
    
    /// Add receiver information
    pub fn add_receiver(&mut self, receiver: PeerId) {
        self.add_tlv(TLV_TYPE_RECEIVER, receiver.to_vec());
    }
    
    /// Add timestamp
    pub fn add_timestamp(&mut self) {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| std::time::Duration::from_secs(0)) // Fallback for clock issues
            .as_secs();
        let mut buf = Vec::new();
        buf.write_u64::<BigEndian>(timestamp)
            .expect("Writing to Vec should never fail - this is a programming error");
        self.add_tlv(TLV_TYPE_TIMESTAMP, buf);
    }
    
    /// Add compressed payload
    pub fn add_compressed_payload(&mut self, data: &[u8]) -> Result<()> {
        let compressed = compress_prepend_size(data);
        self.add_tlv(TLV_TYPE_COMPRESSED_PAYLOAD, compressed);
        Ok(())
    }
    
    /// Add game-specific data
    pub fn add_game_id(&mut self, game_id: GameId) {
        self.add_tlv(TLV_TYPE_GAME_ID, game_id.to_vec());
    }
    
    pub fn add_bet(&mut self, bet: &Bet) -> Result<()> {
        let bet_data = bincode::serialize(bet)
            .map_err(|e| Error::Serialization(e.to_string()))?;
        self.add_compressed_payload(&bet_data)?;
        Ok(())
    }
    
    pub fn add_dice_roll(&mut self, roll: &DiceRoll) -> Result<()> {
        let mut buf = Vec::new();
        buf.push(roll.die1);
        buf.push(roll.die2);
        self.add_tlv(TLV_TYPE_DICE_VALUE, buf);
        Ok(())
    }
    
    pub fn add_commitment(&mut self, commitment: &RandomnessCommitment) -> Result<()> {
        let commitment_data = bincode::serialize(commitment)
            .map_err(|e| Error::Serialization(e.to_string()))?;
        self.add_tlv(TLV_TYPE_COMMITMENT, commitment_data);
        Ok(())
    }
    
    pub fn add_reveal(&mut self, reveal: &RandomnessReveal) -> Result<()> {
        let reveal_data = bincode::serialize(reveal)
            .map_err(|e| Error::Serialization(e.to_string()))?;
        self.add_tlv(TLV_TYPE_REVEAL, reveal_data);
        Ok(())
    }
    
    /// Add routing information
    pub fn add_routing_info(&mut self, routing: &RoutingInfo) -> Result<()> {
        let routing_data = bincode::serialize(routing)
            .map_err(|e| Error::Serialization(e.to_string()))?;
        self.add_tlv(TLV_TYPE_ROUTING_INFO, routing_data);
        Ok(())
    }
    
    /// Get TLV field by type
    pub fn get_tlv(&self, field_type: u8) -> Option<&TlvField> {
        self.tlv_data.iter().find(|tlv| tlv.field_type == field_type)
    }
    
    /// Extract sender from TLV data
    pub fn get_sender(&self) -> Option<PeerId> {
        let tlv = self.get_tlv(TLV_TYPE_SENDER)?;
        if tlv.value.len() == 32 {
            let mut peer_id = [0u8; 32];
            peer_id.copy_from_slice(&tlv.value);
            Some(peer_id)
        } else {
            None
        }
    }
    
    /// Extract receiver from TLV data
    pub fn get_receiver(&self) -> Option<PeerId> {
        let tlv = self.get_tlv(TLV_TYPE_RECEIVER)?;
        if tlv.value.len() == 32 {
            let mut peer_id = [0u8; 32];
            peer_id.copy_from_slice(&tlv.value);
            Some(peer_id)
        } else {
            None
        }
    }
    
    /// Extract timestamp from TLV data
    pub fn get_timestamp(&self) -> Option<u64> {
        let tlv = self.get_tlv(TLV_TYPE_TIMESTAMP)?;
        let mut cursor = std::io::Cursor::new(&tlv.value);
        cursor.read_u64::<BigEndian>().ok()
    }
    
    /// Extract compressed payload
    pub fn get_compressed_payload(&self) -> Result<Option<Vec<u8>>> {
        if let Some(tlv) = self.get_tlv(TLV_TYPE_COMPRESSED_PAYLOAD) {
            let decompressed = decompress_size_prepended(&tlv.value)
                .map_err(|e| Error::InvalidData(format!("Decompression failed: {}", e)))?;
            Ok(Some(decompressed))
        } else {
            Ok(None)
        }
    }
    
    /// Extract game ID
    pub fn get_game_id(&self) -> Option<GameId> {
        let tlv = self.get_tlv(TLV_TYPE_GAME_ID)?;
        if tlv.value.len() == 16 {
            let mut game_id = [0u8; 16];
            game_id.copy_from_slice(&tlv.value);
            Some(game_id)
        } else {
            None
        }
    }
    
    /// Extract bet from packet
    pub fn get_bet(&self) -> Result<Option<Bet>> {
        if let Some(payload) = self.get_compressed_payload()? {
            let bet = bincode::deserialize(&payload)
                .map_err(|e| Error::Serialization(e.to_string()))?;
            Ok(Some(bet))
        } else {
            Ok(None)
        }
    }
    
    /// Extract dice roll
    pub fn get_dice_roll(&self) -> Option<DiceRoll> {
        let tlv = self.get_tlv(TLV_TYPE_DICE_VALUE)?;
        if tlv.value.len() >= 2 {
            DiceRoll::new(tlv.value[0], tlv.value[1]).ok()
        } else {
            None
        }
    }
    
    /// Extract commitment
    pub fn get_commitment(&self) -> Result<Option<RandomnessCommitment>> {
        if let Some(tlv) = self.get_tlv(TLV_TYPE_COMMITMENT) {
            let commitment = bincode::deserialize(&tlv.value)
                .map_err(|e| Error::Serialization(e.to_string()))?;
            Ok(Some(commitment))
        } else {
            Ok(None)
        }
    }
    
    /// Extract reveal
    pub fn get_reveal(&self) -> Result<Option<RandomnessReveal>> {
        if let Some(tlv) = self.get_tlv(TLV_TYPE_REVEAL) {
            let reveal = bincode::deserialize(&tlv.value)
                .map_err(|e| Error::Serialization(e.to_string()))?;
            Ok(Some(reveal))
        } else {
            Ok(None)
        }
    }
    
    /// Extract routing information
    pub fn get_routing_info(&self) -> Result<Option<RoutingInfo>> {
        if let Some(tlv) = self.get_tlv(TLV_TYPE_ROUTING_INFO) {
            let routing = bincode::deserialize(&tlv.value)
                .map_err(|e| Error::Serialization(e.to_string()))?;
            Ok(Some(routing))
        } else {
            Ok(None)
        }
    }
    
    /// Calculate checksum for packet integrity
    fn calculate_checksum(&self) -> u16 {
        let mut checksum: u32 = 0;
        
        // Include header fields in checksum
        checksum = checksum.wrapping_add(self.version as u32);
        checksum = checksum.wrapping_add(self.packet_type as u32);
        checksum = checksum.wrapping_add(self.flags as u32);
        checksum = checksum.wrapping_add(self.ttl as u32);
        checksum = checksum.wrapping_add(self.total_length as u32);
        
        // Include TLV data in checksum
        for tlv in &self.tlv_data {
            checksum = checksum.wrapping_add(tlv.field_type as u32);
            checksum = checksum.wrapping_add(tlv.length as u32);
            for byte in &tlv.value {
                checksum = checksum.wrapping_add(*byte as u32);
            }
        }
        
        // Return lower 16 bits
        checksum as u16
    }
    
    /// Serialize packet to binary format
    pub fn serialize(&mut self) -> Result<Vec<u8>> {
        let mut buffer = Vec::new();
        
        // Calculate total length
        let mut tlv_length = 0;
        for tlv in &self.tlv_data {
            tlv_length += 3 + tlv.value.len(); // type(1) + length(2) + value
        }
        self.total_length = (8 + tlv_length) as u16; // header(8) + tlv_data
        
        // Calculate checksum
        self.checksum = self.calculate_checksum();
        
        // Write header
        buffer.write_u8(self.version)?;
        buffer.write_u8(self.packet_type)?;
        buffer.write_u8(self.flags)?;
        buffer.write_u8(self.ttl)?;
        buffer.write_u16::<BigEndian>(self.total_length)?;
        buffer.write_u16::<BigEndian>(self.checksum)?;
        
        // Write TLV data
        for tlv in &self.tlv_data {
            buffer.write_u8(tlv.field_type)?;
            buffer.write_u16::<BigEndian>(tlv.length)?;
            buffer.write_all(&tlv.value)?;
        }
        
        Ok(buffer)
    }
    
    /// Deserialize packet from binary format
    pub fn deserialize(data: &mut std::io::Cursor<Vec<u8>>) -> Result<Self> {
        // Read header
        let version = data.read_u8()?;
        let packet_type = data.read_u8()?;
        let flags = data.read_u8()?;
        let ttl = data.read_u8()?;
        let total_length = data.read_u16::<BigEndian>()?;
        let checksum = data.read_u16::<BigEndian>()?;
        
        // Read TLV data
        let mut tlv_data = Vec::new();
        let tlv_bytes = total_length - 8; // Subtract header size
        let mut bytes_read = 0;
        
        while bytes_read < tlv_bytes {
            let field_type = data.read_u8()?;
            let length = data.read_u16::<BigEndian>()?;
            
            let mut value = vec![0u8; length as usize];
            data.read_exact(&mut value)?;
            
            tlv_data.push(TlvField {
                field_type,
                length,
                value,
            });
            
            bytes_read += 3 + length; // type(1) + length(2) + value
        }
        
        // Extract convenience fields from TLV
        let mut source = [0u8; 32];
        let mut target = [0u8; 32];
        let mut sequence = 0u32;
        let mut payload = None;
        
        for field in &tlv_data {
            match field.field_type {
                TLV_TYPE_SOURCE_ID if field.value.len() == 32 => {
                    source.copy_from_slice(&field.value);
                }
                TLV_TYPE_TARGET_ID if field.value.len() == 32 => {
                    target.copy_from_slice(&field.value);
                }
                TLV_TYPE_SEQUENCE => {
                    if field.value.len() >= 4 {
                        sequence = u32::from_be_bytes([
                            field.value[0], field.value[1], 
                            field.value[2], field.value[3]
                        ]);
                    }
                }
                TLV_TYPE_PAYLOAD => {
                    payload = Some(field.value.clone());
                }
                _ => {}
            }
        }
        
        let packet = Self {
            version,
            packet_type,
            flags,
            ttl,
            total_length,
            checksum,
            tlv_data,
            source,
            target,
            sequence,
            payload,
        };
        
        // Verify checksum
        let calculated_checksum = packet.calculate_checksum();
        if calculated_checksum != checksum {
            return Err(Error::InvalidData(
                format!("Checksum mismatch: expected {}, got {}", checksum, calculated_checksum)
            ));
        }
        
        Ok(packet)
    }
    
    /// Check if packet should be forwarded (TTL > 0)
    pub fn should_forward(&self) -> bool {
        self.ttl > 0
    }
    
    /// Decrement TTL for forwarding
    pub fn decrement_ttl(&mut self) {
        if self.ttl > 0 {
            self.ttl -= 1;
        }
    }
    
    /// Check if packet is expired (TTL = 0)
    pub fn is_expired(&self) -> bool {
        self.ttl == 0
    }
}

/// Utility functions for creating common packets
pub struct PacketUtils;

impl PacketUtils {
    /// Create a ping packet
    pub fn create_ping(sender: PeerId) -> BitchatPacket {
        let mut packet = BitchatPacket::new(PACKET_TYPE_PING, 8);
        packet.add_sender(sender);
        packet.add_timestamp();
        packet
    }
    
    /// Create a pong packet
    pub fn create_pong(sender: PeerId, receiver: PeerId) -> BitchatPacket {
        let mut packet = BitchatPacket::new(PACKET_TYPE_PONG, 8);
        packet.add_sender(sender);
        packet.add_receiver(receiver);
        packet.add_timestamp();
        packet
    }
    
    /// Create a game creation packet
    pub fn create_game_create(
        sender: PeerId,
        game_id: GameId,
        max_players: u8,
        buy_in: CrapTokens,
    ) -> BitchatPacket {
        let mut packet = BitchatPacket::new(PACKET_TYPE_GAME_CREATE, 8);
        packet.add_sender(sender);
        packet.add_game_id(game_id);
        packet.add_timestamp();
        
        // Add game parameters
        let mut params = Vec::new();
        params.push(max_players);
        params.extend_from_slice(&buy_in.amount().to_be_bytes());
        packet.add_tlv(0x20, params); // Game parameters TLV
        
        packet
    }
    
    /// Create a bet packet
    pub fn create_bet_packet(
        sender: PeerId,
        bet: &Bet,
    ) -> Result<BitchatPacket> {
        let mut packet = BitchatPacket::new(PACKET_TYPE_GAME_BET, 8);
        packet.add_sender(sender);
        packet.add_game_id(bet.game_id);
        packet.add_bet(bet)?;
        packet.add_timestamp();
        Ok(packet)
    }
    
    /// Create a dice roll result packet
    pub fn create_dice_result(
        sender: PeerId,
        game_id: GameId,
        roll: &DiceRoll,
    ) -> Result<BitchatPacket> {
        let mut packet = BitchatPacket::new(PACKET_TYPE_GAME_RESULT, 8);
        packet.add_sender(sender);
        packet.add_game_id(game_id);
        packet.add_dice_roll(roll)?;
        packet.add_timestamp();
        Ok(packet)
    }
}

/// Protocol result type
pub type ProtocolResult<T> = std::result::Result<T, ProtocolError>;

/// Protocol-specific errors
#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    #[error("Invalid packet format: {0}")]
    InvalidPacket(String),
    
    #[error("Invalid header: {0}")]
    InvalidHeader(String),
    
    #[error("Checksum mismatch")]
    ChecksumMismatch,
    
    #[error("Unsupported version: {0}")]
    UnsupportedVersion(u8),
    
    #[error("Serialization error: {0}")]
    Serialization(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

// Event sourcing types for light consensus layer

/// Type aliases for clarity
pub type Hash256 = [u8; 32];

/// Wrapper for signature to enable serialization
/// Feynman: A cryptographic signature is like a tamper-proof seal
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Signature(pub [u8; SIGNATURE_SIZE]);

// Manual impl for Serialize/Deserialize for fixed-size arrays > 32
impl Serialize for Signature {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_bytes(&self.0)
    }
}

impl<'de> Deserialize<'de> for Signature {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct SignatureVisitor;
        
        impl<'de> serde::de::Visitor<'de> for SignatureVisitor {
            type Value = Signature;
            
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("64 bytes")
            }
            
            fn visit_bytes<E>(self, v: &[u8]) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if v.len() == SIGNATURE_SIZE {
                    let mut arr = [0u8; SIGNATURE_SIZE];
                    arr.copy_from_slice(v);
                    Ok(Signature(arr))
                } else {
                    Err(E::custom(format!("expected {} bytes", SIGNATURE_SIZE)))
                }
            }
        }
        
        deserializer.deserialize_bytes(SignatureVisitor)
    }
}

/// Signed game event for event sourcing
/// Feynman: Like a notarized document - proves who did what and when
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedGameEvent {
    pub event: GameEvent,
    pub signature: Signature,
    pub event_hash: Hash256,
}

/// Game event types
/// Feynman: Every action in the game becomes a permanent record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameEvent {
    pub game_id: GameId,
    pub event_type: GameEventType,
    pub player_id: PeerId,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GameEventType {
    GameCreated { max_players: u8, buy_in: CrapTokens },
    PlayerJoined,
    BetPlaced { bet: Bet },
    RandomnessCommitted { commitment: RandomnessCommitment },
    RandomnessRevealed { reveal: RandomnessReveal },
    DiceRolled { roll: DiceRoll },
    GameResolved { winning_bets: Vec<(PeerId, CrapTokens)> },
    TokensTransferred { amount: CrapTokens, recipient: PeerId },
}

/// Round consensus checkpoint
/// Feynman: A "save point" where everyone agrees on what happened
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoundConsensus {
    pub game_id: GameId,
    pub round_number: u64,
    pub dice_hash: Hash256,         // Hash of the revealed dice
    pub bet_merkle: Hash256,        // Merkle root of all bets this round
    pub payout_merkle: Hash256,     // Merkle root of all payouts
    pub signatures: Vec<(PeerId, Signature)>, // 2/3+ participant signatures
}

/// Event log for deterministic state computation
/// Feynman: The "flight recorder" - can replay everything that happened
pub struct GameEventLog {
    pub game_id: GameId,
    pub events: Vec<SignedGameEvent>,
    pub participants: HashSet<PeerId>,
    pub event_hashes: HashSet<Hash256>,  // Prevent duplicate events
    pub round_summaries: Vec<RoundConsensus>, // Consensus checkpoints
}

impl GameEventLog {
    pub fn new(game_id: GameId) -> Self {
        Self {
            game_id,
            events: Vec::new(),
            participants: HashSet::new(),
            event_hashes: HashSet::new(),
            round_summaries: Vec::new(),
        }
    }
    
    /// Apply a new event to the log
    /// Feynman: Like adding a new entry to a ledger - once written, permanent
    pub fn apply_event(&mut self, event: SignedGameEvent) -> Result<()> {
        // Check for duplicates
        if self.event_hashes.contains(&event.event_hash) {
            return Ok(()); // Already have it, ignore
        }
        
        // Verify signature
        // First, serialize the event for signature verification
        let event_data = bincode::serialize(&event.event)
            .map_err(|e| Error::Protocol(format!("Failed to serialize event for verification: {}", e)))?;
        
        // Create BitchatSignature from the protocol signature and player's public key
        let bitchat_signature = crate::crypto::BitchatSignature {
            signature: event.signature.0.to_vec(),
            public_key: event.event.player_id.to_vec(), // Assume PeerId is the public key
        };
        
        // Verify the signature
        if !crate::crypto::BitchatIdentity::verify_signature(&event_data, &bitchat_signature) {
            return Err(Error::Protocol("Invalid event signature".to_string()));
        }
        
        // Add to log
        self.events.push(event.clone());
        self.event_hashes.insert(event.event_hash);
        self.participants.insert(event.event.player_id);
        
        Ok(())
    }
    
    /// Add a round consensus checkpoint
    /// Feynman: Like everyone signing a contract - proves agreement
    pub fn add_checkpoint(&mut self, consensus: RoundConsensus) -> Result<()> {
        // Verify we have 2/3+ signatures
        let required = (self.participants.len() * 2) / 3 + 1;
        if consensus.signatures.len() < required {
            return Err(Error::Protocol("Insufficient signatures for consensus".to_string()));
        }
        
        self.round_summaries.push(consensus);
        Ok(())
    }
    
    /// Get missing events from a peer
    /// Feynman: Like asking "what did I miss while I was gone?"
    pub fn get_missing_events(&self) -> Vec<Hash256> {
        // In a real implementation, compare with expected events
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_packet_serialization() {
        let sender = [1u8; 32];
        let mut packet = PacketUtils::create_ping(sender);
        
        let serialized = packet.serialize().unwrap();
        let mut cursor = std::io::Cursor::new(serialized);
        let deserialized = BitchatPacket::deserialize(&mut cursor).unwrap();
        
        assert_eq!(packet.version, deserialized.version);
        assert_eq!(packet.packet_type, deserialized.packet_type);
        assert_eq!(packet.get_sender(), deserialized.get_sender());
    }
    
    #[test]
    fn test_dice_roll() {
        let roll = DiceRoll::new(3, 4).unwrap();
        assert_eq!(roll.total(), 7);
        assert!(roll.is_natural());
        assert!(!roll.is_craps());
        assert!(!roll.is_hard_way());
        
        let craps_roll = DiceRoll::new(1, 1).unwrap();
        assert_eq!(craps_roll.total(), 2);
        assert!(craps_roll.is_craps());
        assert!(!craps_roll.is_natural());
    }
    
    #[test]
    fn test_crap_tokens() {
        let tokens = CrapTokens::from_crap(5.5).unwrap();
        assert_eq!(tokens.amount(), 5_500_000);
        assert_eq!(tokens.to_crap(), 5.5);
    }
}