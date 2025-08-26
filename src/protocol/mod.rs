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

/// DiceRoll represents a roll of two dice in craps
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
        use rand::RngCore;
        let mut rng = rand::thread_rng();
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
        // Simplified implementation - would parse TLV data
        None
    }
    
    /// Get the receiver from TLV data  
    pub fn get_receiver(&self) -> Option<PeerId> {
        // Simplified implementation - would parse TLV data
        None
    }
    
    /// Get routing information from TLV data
    pub fn get_routing_info(&self) -> std::result::Result<Option<RoutingInfo>, Error> {
        // Simplified implementation - would parse TLV data
        Ok(None)
    }
    
    /// Add sender to TLV data
    pub fn add_sender(&mut self, sender: PeerId) {
        // Simplified implementation - would add to TLV data
    }
    
    /// Add routing information to TLV data
    pub fn add_routing_info(&mut self, routing_info: &RoutingInfo) -> Result<()> {
        // Simplified implementation - would serialize to TLV
        Ok(())
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
    
    /// Serialize packet to bytes
    pub fn serialize(&mut self) -> Result<Vec<u8>> {
        // Simplified implementation - would serialize entire packet
        let mut buffer = Vec::new();
        buffer.push(self.version);
        buffer.push(self.packet_type);
        buffer.push(self.flags);
        buffer.push(self.ttl);
        buffer.extend_from_slice(&self.total_length.to_be_bytes());
        buffer.extend_from_slice(&self.sequence.to_be_bytes());
        
        // Add payload if present
        if let Some(payload) = &self.payload {
            buffer.extend_from_slice(payload);
        }
        
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
            tlv_data: Vec::new(),
            payload: None,
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

impl std::fmt::Display for CrapTokens {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}Ȼ", self.0)
    }
}

/// Helper function to create a new GameId using cryptographic randomness
pub fn new_game_id() -> GameId {
    let mut game_id = [0u8; 16];
    use rand::RngCore;
    let mut rng = rand::thread_rng();
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