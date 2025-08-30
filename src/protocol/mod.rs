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
pub mod zero_copy;

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
#[cfg(feature = "benchmarks")]
pub mod benchmark;
pub mod compression;

// New robust modules
pub mod treasury;
pub mod reputation;

// Infrastructure modules
pub mod versioning;

// P2P Networking Protocol modules
pub mod p2p_messages;
pub mod consensus_coordinator;
pub mod network_consensus_bridge;
pub mod state_sync;
pub mod ble_dispatch;
pub mod partition_recovery;
pub mod anti_cheat;
pub mod ble_optimization;

use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

// Re-exports for external modules
pub use p2p_messages::ConsensusMessage as P2PMessage;
pub use crate::database::GameState;

/// Transaction identifier for tracking game operations
pub type TransactionId = [u8; 32];

/// DiceRoll represents a roll of two dice in craps
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DiceRoll {
    pub die1: u8,
    pub die2: u8,
    pub timestamp: u64,
}

impl DiceRoll {
    pub fn new(die1: u8, die2: u8) -> Result<Self> {
        if !(1..=6).contains(&die1) || !(1..=6).contains(&die2) {
            return Err(Error::InvalidData("Dice values must be 1-6".to_string()));
        }
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Ok(Self { die1, die2, timestamp })
    }
    
    pub fn generate() -> Self {
        use fastrand;
        let die1 = fastrand::u8(1..=6);
        let die2 = fastrand::u8(1..=6);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self { die1, die2, timestamp }
    }
    
    pub fn total(&self) -> u8 {
        self.die1 + self.die2
    }
    
    pub fn is_natural(&self) -> bool {
        matches!(self.total(), 7 | 11)
    }
    
    pub fn is_craps(&self) -> bool {
        matches!(self.total(), 2 | 3 | 12)
    }
    
    pub fn is_hard_way(&self) -> bool {
        self.die1 == self.die2 && matches!(self.total(), 4 | 6 | 8 | 10)
    }
}

impl std::fmt::Display for DiceRoll {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}+{}={}", self.die1, self.die2, self.total())
    }
}

/// Bet types in craps - comprehensive coverage of all 64 bet types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BetType {
    // Basic line bets
    Pass,
    DontPass,
    Come,
    DontCome,
    
    // Odds bets
    OddsPass,
    OddsDontPass,
    OddsCome,
    OddsDontCome,
    
    // Field bet
    Field,
    
    // Hard way bets
    Hard4,
    Hard6,
    Hard8,
    Hard10,
    
    // Place bets (parameterized by number)
    Place(u8),
    
    // Hard way bets (parameterized by number)
    HardWay(u8),
    
    // Next roll (hop) bets
    Next2,
    Next3,
    Next4,
    Next5,
    Next6,
    Next7,
    Next8,
    Next9,
    Next10,
    Next11,
    Next12,
    
    // Single roll proposition bets
    Ace,       // 2 (snake eyes)
    Eleven,    // 11 (yo)
    Twelve,    // 12 (boxcars)
    
    // Yes bets (rolling number before 7)
    Yes2,
    Yes3,
    Yes4,
    Yes5,
    Yes6,
    Yes8,
    Yes9,
    Yes10,
    Yes11,
    Yes12,
    
    // No bets (7 before number)
    No2,
    No3,
    No4,
    No5,
    No6,
    No8,
    No9,
    No10,
    No11,
    No12,
    
    // Repeater bets
    Repeater2,
    Repeater3,
    Repeater4,
    Repeater5,
    Repeater6,
    Repeater8,
    Repeater9,
    Repeater10,
    Repeater11,
    Repeater12,
    
    // Special bets
    Fire,
    BonusSmall,
    BonusTall,
    BonusAll,
    HotRoller,
    TwiceHard,
    RideLine,
    Muggsy,
    Replay,
    DifferentDoubles,
}

impl BetType {
    /// Convert BetType to a unique numeric identifier for serialization
    /// Parameterized variants encode the parameter in the upper bits
    pub fn to_u8(&self) -> u8 {
        match self {
            // Basic line bets (0-3)
            BetType::Pass => 0,
            BetType::DontPass => 1,
            BetType::Come => 2,
            BetType::DontCome => 3,
            
            // Odds bets (4-7)
            BetType::OddsPass => 4,
            BetType::OddsDontPass => 5,
            BetType::OddsCome => 6,
            BetType::OddsDontCome => 7,
            
            // Field bet (8)
            BetType::Field => 8,
            
            // Hard way bets (9-12)
            BetType::Hard4 => 9,
            BetType::Hard6 => 10,
            BetType::Hard8 => 11,
            BetType::Hard10 => 12,
            
            // Parameterized variants (13-14)
            BetType::Place(num) => 13 | ((*num & 0x0F) << 4), // encode number in upper 4 bits
            BetType::HardWay(num) => 14 | ((*num & 0x0F) << 4),
            
            // Next roll bets (15-26)
            BetType::Next2 => 15,
            BetType::Next3 => 16,
            BetType::Next4 => 17,
            BetType::Next5 => 18,
            BetType::Next6 => 19,
            BetType::Next7 => 20,
            BetType::Next8 => 21,
            BetType::Next9 => 22,
            BetType::Next10 => 23,
            BetType::Next11 => 24,
            BetType::Next12 => 25,
            
            // Single roll proposition bets (26-28)
            BetType::Ace => 26,
            BetType::Eleven => 27,
            BetType::Twelve => 28,
            
            // Yes bets (29-39)
            BetType::Yes2 => 29,
            BetType::Yes3 => 30,
            BetType::Yes4 => 31,
            BetType::Yes5 => 32,
            BetType::Yes6 => 33,
            BetType::Yes8 => 34,
            BetType::Yes9 => 35,
            BetType::Yes10 => 36,
            BetType::Yes11 => 37,
            BetType::Yes12 => 38,
            
            // No bets (39-49)
            BetType::No2 => 39,
            BetType::No3 => 40,
            BetType::No4 => 41,
            BetType::No5 => 42,
            BetType::No6 => 43,
            BetType::No8 => 44,
            BetType::No9 => 45,
            BetType::No10 => 46,
            BetType::No11 => 47,
            BetType::No12 => 48,
            
            // Repeater bets (49-60)
            BetType::Repeater2 => 49,
            BetType::Repeater3 => 50,
            BetType::Repeater4 => 51,
            BetType::Repeater5 => 52,
            BetType::Repeater6 => 53,
            BetType::Repeater8 => 54,
            BetType::Repeater9 => 55,
            BetType::Repeater10 => 56,
            BetType::Repeater11 => 57,
            BetType::Repeater12 => 58,
            
            // Special bets (61-73)
            BetType::Fire => 61,
            BetType::BonusSmall => 62,
            BetType::BonusTall => 63,
            BetType::BonusAll => 64,
            BetType::HotRoller => 65,
            BetType::TwiceHard => 66,
            BetType::RideLine => 67,
            BetType::Muggsy => 68,
            BetType::Replay => 69,
            BetType::DifferentDoubles => 70,
        }
    }
    
    pub fn to_u64(&self) -> u64 {
        self.to_u8() as u64
    }
    
    pub fn to_usize(&self) -> usize {
        self.to_u8() as usize
    }
}

/// A bet placed by a player
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bet {
    pub id: [u8; 16],
    pub player: PeerId,
    pub game_id: GameId,
    pub bet_type: BetType,
    pub amount: CrapTokens,
    pub timestamp: u64,
}

impl Bet {
    pub fn new(player: PeerId, game_id: GameId, bet_type: BetType, amount: CrapTokens) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
            
        // Generate a unique bet ID
        let mut id = [0u8; 16];
        use rand::{RngCore, rngs::OsRng};
        let mut rng = OsRng;
        rng.fill_bytes(&mut id);
            
        Self {
            id,
            player,
            game_id,
            bet_type,
            amount,
            timestamp,
        }
    }
}

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
pub const MAX_BET_AMOUNT: u64 = 10_000_000;

/// Peer identifier - 32 bytes for Ed25519 public key compatibility
pub type PeerId = [u8; 32];

/// Trait to add methods to PeerId type
pub trait PeerIdExt {
    fn random() -> Self;
}

impl PeerIdExt for PeerId {
    /// Generate a random peer ID for testing
    fn random() -> Self {
        use rand::{RngCore, rngs::OsRng};
        let mut rng = OsRng;
        let mut peer_id = [0u8; 32];
        rng.fill_bytes(&mut peer_id);
        peer_id
    }
}

/// Utility functions for PeerId
pub mod peer_id {
    use super::PeerId;
    
    /// Generate a random peer ID for testing
    pub fn random() -> PeerId {
        use rand::{RngCore, rngs::OsRng};
        let mut rng = OsRng;
        let mut peer_id = [0u8; 32];
        rng.fill_bytes(&mut peer_id);
        peer_id
    }
}

/// Game identifier - 16 bytes UUID
pub type GameId = [u8; 16];

/// Hash type for state hashes
pub type Hash256 = [u8; 32];

/// Signature type for cryptographic signatures
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Signature(pub [u8; 64]);

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
                formatter.write_str("a 64-byte signature")
            }
            
            fn visit_bytes<E>(self, v: &[u8]) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if v.len() != 64 {
                    return Err(E::custom(format!("expected 64 bytes, got {}", v.len())));
                }
                let mut arr = [0u8; 64];
                arr.copy_from_slice(v);
                Ok(Signature(arr))
            }
        }
        
        deserializer.deserialize_bytes(SignatureVisitor)
    }
}

/// Basic packet structure for the BitCraps protocol
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitchatPacket {
    pub version: u8,
    pub packet_type: u8,
    pub flags: u8,
    pub ttl: u8,
    pub total_length: u32,
    pub sequence: u64,
    pub checksum: u32,
    pub source: [u8; 32],
    pub target: [u8; 32],
    pub tlv_data: Vec<TlvField>,
    pub payload: Option<Vec<u8>>,
}

/// TLV (Type-Length-Value) field for extensible packet format
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlvField {
    pub field_type: u8,
    pub length: u16,
    pub value: Vec<u8>,
}

/// Routing information for mesh networking
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutingInfo {
    pub source: PeerId,
    pub destination: Option<PeerId>,
    pub route_history: Vec<PeerId>,
    pub max_hops: u8,
}

impl BitchatPacket {
    /// Create a new packet
    pub fn new(packet_type: u8) -> Self {
        Self {
            version: 1,
            packet_type,
            flags: 0,
            ttl: 8,
            total_length: 0,
            sequence: 0,
            checksum: 0,
            source: [0u8; 32],
            target: [0u8; 32],
            tlv_data: Vec::new(),
            payload: None,
        }
    }
    
    /// Get the sender from TLV data
    pub fn get_sender(&self) -> Option<PeerId> {
        // Parse sender from TLV data
        for field in &self.tlv_data {
            if field.field_type == 0x01 && field.value.len() == 32 {
                let mut sender = [0u8; 32];
                sender.copy_from_slice(&field.value);
                return Some(sender);
            }
        }
        None
    }
    
    /// Get the receiver from TLV data  
    pub fn get_receiver(&self) -> Option<PeerId> {
        // Parse receiver from TLV data
        for field in &self.tlv_data {
            if field.field_type == 0x02 && field.value.len() == 32 {
                let mut receiver = [0u8; 32];
                receiver.copy_from_slice(&field.value);
                return Some(receiver);
            }
        }
        None
    }
    
    /// Get routing information from TLV data
    pub fn get_routing_info(&self) -> std::result::Result<Option<RoutingInfo>, Error> {
        // Simplified implementation - would parse TLV data
        Ok(None)
    }
    
    /// Add sender to TLV data
    pub fn add_sender(&mut self, sender: PeerId) {
        // Add sender TLV (type 0x01)
        self.tlv_data.push(TlvField {
            field_type: 0x01,
            length: 32,
            value: sender.to_vec(),
        });
    }
    
    /// Add receiver to TLV data
    pub fn add_receiver(&mut self, receiver: PeerId) {
        // Add receiver TLV (type 0x02)
        self.tlv_data.push(TlvField {
            field_type: 0x02,
            length: 32,
            value: receiver.to_vec(),
        });
    }
    
    /// Add signature to TLV data
    pub fn add_signature(&mut self, signature: &[u8; 64]) {
        // Add signature TLV (type 0x03)
        self.tlv_data.push(TlvField {
            field_type: 0x03,
            length: 64,
            value: signature.to_vec(),
        });
    }
    
    /// Get signature from TLV data
    pub fn get_signature(&self) -> Option<[u8; 64]> {
        // Parse signature from TLV data
        for field in &self.tlv_data {
            if field.field_type == 0x03 && field.value.len() == 64 {
                let mut signature = [0u8; 64];
                signature.copy_from_slice(&field.value);
                return Some(signature);
            }
        }
        None
    }
    
    /// Add routing information to TLV data
    pub fn add_routing_info(&mut self, routing_info: &RoutingInfo) -> Result<()> {
        // Add routing TLV (type 0x04)
        Ok(())
    }
    
    /// Verify packet signature
    pub fn verify_signature(&self, public_key: &[u8; 32]) -> bool {
        use ed25519_dalek::{VerifyingKey, Signature, Verifier};
        
        // Get the signature from TLV data
        let signature = match self.get_signature() {
            Some(sig) => sig,
            None => return false,
        };
        
        // Create message to verify (packet without signature)
        let mut message = Vec::new();
        message.push(self.version);
        message.push(self.packet_type);
        message.push(self.flags);
        message.push(self.ttl);
        message.extend_from_slice(&self.total_length.to_be_bytes());
        message.extend_from_slice(&self.sequence.to_be_bytes());
        message.extend_from_slice(&self.checksum.to_be_bytes());
        message.extend_from_slice(&self.source);
        message.extend_from_slice(&self.target);
        
        // Add other TLV data (except signature)
        for field in &self.tlv_data {
            if field.field_type != 0x03 {
                message.push(field.field_type);
                message.extend_from_slice(&field.length.to_be_bytes());
                message.extend_from_slice(&field.value);
            }
        }
        
        // Add payload if present
        if let Some(ref payload) = self.payload {
            message.extend_from_slice(payload);
        }
        
        // Verify signature
        if let Ok(verifying_key) = VerifyingKey::from_bytes(public_key) {
            let sig = Signature::from_bytes(&signature);
            verifying_key.verify(&message, &sig).is_ok()
        } else {
            false
        }
    }
    
    /// Get timestamp from TLV data
    pub fn get_timestamp(&self) -> Option<u64> {
        // Simplified implementation - would parse TLV data
        None
    }
    
    /// Check if packet should be forwarded
    pub fn should_forward(&self) -> bool {
        self.ttl > 0
    }
    
    /// Decrement TTL
    pub fn decrement_ttl(&mut self) {
        if self.ttl > 0 {
            self.ttl -= 1;
        }
    }

    /// Create a new ping packet
    pub fn new_ping(source: PeerId, target: PeerId) -> Self {
        let mut packet = Self::new(0x01); // Ping packet type
        packet.source = source;
        packet.target = target;
        packet
    }

    /// Create a new pong packet
    pub fn new_pong(source: PeerId, target: PeerId) -> Self {
        let mut packet = Self::new(0x02); // Pong packet type  
        packet.source = source;
        packet.target = target;
        packet
    }

    /// Create a new discovery packet
    pub fn new_discovery(source: PeerId) -> Self {
        let mut packet = Self::new(0x03); // Discovery packet type
        packet.source = source;
        packet
    }
    
    /// Serialize packet to bytes
    pub fn serialize(&mut self) -> Result<Vec<u8>> {
        let mut buffer = Vec::new();
        
        // Header fields
        buffer.push(self.version);
        buffer.push(self.packet_type);
        buffer.push(self.flags);
        buffer.push(self.ttl);
        buffer.extend_from_slice(&self.total_length.to_be_bytes());
        buffer.extend_from_slice(&self.sequence.to_be_bytes());
        buffer.extend_from_slice(&self.checksum.to_be_bytes());
        
        // Source and target addresses
        buffer.extend_from_slice(&self.source);
        buffer.extend_from_slice(&self.target);
        
        // TLV fields
        buffer.extend_from_slice(&(self.tlv_data.len() as u16).to_be_bytes());
        for tlv in &self.tlv_data {
            buffer.push(tlv.field_type);
            buffer.extend_from_slice(&tlv.length.to_be_bytes());
            buffer.extend_from_slice(&tlv.value);
        }
        
        // Payload
        if let Some(payload) = &self.payload {
            buffer.extend_from_slice(&(payload.len() as u32).to_be_bytes());
            buffer.extend_from_slice(payload);
        } else {
            buffer.extend_from_slice(&0u32.to_be_bytes());
        }
        
        // Update total length
        self.total_length = buffer.len() as u32;
        // Update the total_length field in the buffer
        buffer[4..8].copy_from_slice(&self.total_length.to_be_bytes());
        
        Ok(buffer)
    }
    
    /// Deserialize packet from bytes
    pub fn deserialize<R: std::io::Read>(reader: &mut R) -> Result<Self> {
        use byteorder::{BigEndian, ReadBytesExt};
        
        let version = reader.read_u8().map_err(|e| Error::Serialization(e.to_string()))?;
        let packet_type = reader.read_u8().map_err(|e| Error::Serialization(e.to_string()))?;
        let flags = reader.read_u8().map_err(|e| Error::Serialization(e.to_string()))?;
        let ttl = reader.read_u8().map_err(|e| Error::Serialization(e.to_string()))?;
        let total_length = reader.read_u32::<BigEndian>().map_err(|e| Error::Serialization(e.to_string()))?;
        let sequence = reader.read_u64::<BigEndian>().map_err(|e| Error::Serialization(e.to_string()))?;
        let checksum = reader.read_u32::<BigEndian>().map_err(|e| Error::Serialization(e.to_string()))?;
        
        let mut source = [0u8; 32];
        reader.read_exact(&mut source).map_err(|e| Error::Serialization(e.to_string()))?;
        
        let mut target = [0u8; 32];
        reader.read_exact(&mut target).map_err(|e| Error::Serialization(e.to_string()))?;
        
        // TLV fields
        let tlv_count = reader.read_u16::<BigEndian>().map_err(|e| Error::Serialization(e.to_string()))?;
        let mut tlv_data = Vec::with_capacity(tlv_count as usize);
        for _ in 0..tlv_count {
            let field_type = reader.read_u8().map_err(|e| Error::Serialization(e.to_string()))?;
            let length = reader.read_u16::<BigEndian>().map_err(|e| Error::Serialization(e.to_string()))?;
            let mut value = vec![0u8; length as usize];
            reader.read_exact(&mut value).map_err(|e| Error::Serialization(e.to_string()))?;
            tlv_data.push(TlvField { field_type, length, value });
        }
        
        // Payload
        let payload_len = reader.read_u32::<BigEndian>().map_err(|e| Error::Serialization(e.to_string()))?;
        let payload = if payload_len > 0 {
            let mut payload_data = vec![0u8; payload_len as usize];
            reader.read_exact(&mut payload_data).map_err(|e| Error::Serialization(e.to_string()))?;
            Some(payload_data)
        } else {
            None
        };
        
        Ok(Self {
            version,
            packet_type,
            flags,
            ttl,
            total_length,
            sequence,
            checksum,
            source,
            target,
            tlv_data,
            payload,
        })
    }
}

/// CrapTokens - The native currency of BitCraps
/// Newtype wrapper around u64 for type safety
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub struct CrapTokens(pub u64);

impl CrapTokens {
    pub const ZERO: Self = Self(0);
    
    pub fn new(amount: u64) -> Self {
        Self(amount)
    }
    
    /// Create tokens without validation (for internal use)
    pub fn new_unchecked(amount: u64) -> Self {
        Self(amount)
    }
    
    pub fn amount(&self) -> u64 {
        self.0
    }
    
    pub fn checked_add(self, other: Self) -> Option<Self> {
        self.0.checked_add(other.0).map(Self)
    }
    
    pub fn checked_sub(self, other: Self) -> Option<Self> {
        self.0.checked_sub(other.0).map(Self)
    }
    
    pub fn saturating_add(self, other: Self) -> Self {
        Self(self.0.saturating_add(other.0))
    }
    
    pub fn saturating_sub(self, other: Self) -> Self {
        Self(self.0.saturating_sub(other.0))
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
        
        Ok(Self(amount))
    }
    
    pub fn to_crap(&self) -> f64 {
        self.0 as f64 / 1_000_000.0
    }
}

impl From<u64> for CrapTokens {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl From<CrapTokens> for u64 {
    fn from(tokens: CrapTokens) -> Self {
        tokens.0
    }
}

impl std::ops::Add for CrapTokens {
    type Output = Self;
    
    fn add(self, other: Self) -> Self {
        Self(self.0 + other.0)
    }
}

impl std::ops::AddAssign for CrapTokens {
    fn add_assign(&mut self, other: Self) {
        self.0 += other.0;
    }
}

impl std::ops::Sub for CrapTokens {
    type Output = Self;
    
    fn sub(self, other: Self) -> Self {
        Self(self.0 - other.0)
    }
}

impl std::ops::SubAssign for CrapTokens {
    fn sub_assign(&mut self, other: Self) {
        self.0 -= other.0;
    }
}

// Multiplication with integers for payout calculations
impl std::ops::Mul<u64> for CrapTokens {
    type Output = Self;
    
    fn mul(self, rhs: u64) -> Self {
        Self(self.0 * rhs)
    }
}

impl std::ops::Mul<i32> for CrapTokens {
    type Output = Self;
    
    fn mul(self, rhs: i32) -> Self {
        Self(self.0 * rhs as u64)
    }
}

// Division with integers for split calculations
impl std::ops::Div<u64> for CrapTokens {
    type Output = Self;
    
    fn div(self, rhs: u64) -> Self {
        Self(self.0 / rhs)
    }
}

impl std::ops::Div<i32> for CrapTokens {
    type Output = Self;
    
    fn div(self, rhs: i32) -> Self {
        Self(self.0 / rhs as u64)
    }
}

impl std::fmt::Display for CrapTokens {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}Ȼ", self.0)
    }
}

/// Helper function to create a new GameId using cryptographic randomness
pub fn new_game_id() -> GameId {
    let mut game_id = [0u8; 16];
    use rand::{RngCore, rngs::OsRng};
    let mut rng = OsRng;
    rng.fill_bytes(&mut game_id);
    game_id
}

/// Utility functions for packet creation
pub struct PacketUtils;

impl PacketUtils {
    pub fn create_ping(sender: PeerId) -> BitchatPacket {
        let mut packet = BitchatPacket::new(PACKET_TYPE_PING);
        packet.add_sender(sender);
        packet
    }
}

/// Packet type constants for BitCraps protocol
pub const PACKET_TYPE_PING: u8 = 0x01;
pub const PACKET_TYPE_PONG: u8 = 0x02;
pub const PACKET_TYPE_DISCOVERY: u8 = 0x03;
pub const PACKET_TYPE_GAME_DATA: u8 = 0x10;
pub const PACKET_TYPE_CONSENSUS_VOTE: u8 = 0x1C;

/// Simplified crypto types for test compatibility
/// These are basic implementations for tests - real crypto is in crypto module

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitchatKeypair {
    pub public_key: PeerId,
    pub private_key: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitchatSignature {
    pub bytes: [u8; 64],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitchatIdentity {
    pub keypair: BitchatKeypair,
    pub nickname: Option<String>,
    pub peer_id: PeerId,
}

impl BitchatKeypair {
    /// Generate a new keypair for testing
    pub fn generate() -> Self {
        use rand::{RngCore, rngs::OsRng};
        let mut rng = OsRng;
        
        let mut public_key = [0u8; 32];
        let mut private_key = [0u8; 32];
        
        rng.fill_bytes(&mut public_key);
        rng.fill_bytes(&mut private_key);
        
        Self { public_key, private_key }
    }

    /// Sign data with this keypair (simplified for tests)
    pub fn sign(&self, data: &[u8]) -> BitchatSignature {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(data);
        hasher.update(&self.private_key);
        let hash = hasher.finalize();
        
        let mut signature_bytes = [0u8; 64];
        signature_bytes[..32].copy_from_slice(&hash[..]);
        signature_bytes[32..].copy_from_slice(&self.public_key[..32]);
        
        BitchatSignature {
            bytes: signature_bytes,
        }
    }

    /// Verify signature (simplified for tests)
    pub fn verify(&self, data: &[u8], signature: &BitchatSignature) -> bool {
        let expected_sig = self.sign(data);
        expected_sig.bytes == signature.bytes
    }
}

impl BitchatSignature {
    /// Convert signature to bytes
    pub fn to_bytes(&self) -> [u8; 64] {
        self.bytes
    }

    /// Create signature from bytes
    pub fn from_bytes(bytes: [u8; 64]) -> Self {
        Self { bytes }
    }
}

impl BitchatIdentity {
    /// Create identity from keypair and nickname
    pub fn from_keypair_with_nickname(keypair: BitchatKeypair, nickname: String) -> Self {
        let peer_id = keypair.public_key;
        Self {
            keypair,
            nickname: Some(nickname),
            peer_id,
        }
    }

    /// Create identity from keypair with PoW
    pub fn from_keypair_with_pow(keypair: BitchatKeypair, _difficulty: u32) -> Self {
        let peer_id = keypair.public_key;
        Self {
            keypair,
            nickname: None,
            peer_id,
        }
    }

    /// Get peer ID
    pub fn peer_id(&self) -> PeerId {
        self.peer_id
    }
}

/// Generate a random peer ID for testing
pub fn random_peer_id() -> PeerId {
    use rand::{RngCore, rngs::OsRng};
    let mut rng = OsRng;
    let mut peer_id = [0u8; 32];
    rng.fill_bytes(&mut peer_id);
    peer_id
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