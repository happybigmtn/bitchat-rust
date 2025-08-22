//! Protocol implementation for BitCraps
//! 
//! This module implements the core binary protocol for BitCraps decentralized casino.
//! Features:
//! - TLV (Type-Length-Value) encoding for extensible messages
//! - LZ4 compression for bandwidth efficiency
//! - Gaming-specific packet types (bets, dice rolls, payouts)
//! - Mesh routing with TTL management
//! - Session management integration

use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::time::{SystemTime, UNIX_EPOCH};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use lz4_flex::{compress_prepend_size, decompress_size_prepended};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{Error, Result};

/// Peer identifier - 32 bytes for Ed25519 public key compatibility
pub type PeerId = [u8; 32];

/// Game identifier - 16 bytes UUID
pub type GameId = [u8; 16];

/// Packet type constants for BitCraps protocol
pub const PACKET_TYPE_PING: u8 = 0x01;
pub const PACKET_TYPE_PONG: u8 = 0x02;
pub const PACKET_TYPE_CHAT: u8 = 0x10;
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
}

/// TLV field structure
#[derive(Debug, Clone, PartialEq)]
pub struct TlvField {
    pub field_type: u8,
    pub length: u16,
    pub value: Vec<u8>,
}

/// Gaming-specific data structures
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum BetType {
    Pass = 0,
    DontPass = 1,
    Field = 2,
    Any7 = 3,
    AnyCraps = 4,
    Hardways(u8) = 5,
    Place(u8) = 6,
    Come = 7,
    DontCome = 8,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CrapTokens {
    amount: u64, // Amount in smallest unit (like satoshis)
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

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DiceRoll {
    pub die1: u8,
    pub die2: u8,
    pub total: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RandomnessCommitment {
    pub game_id: GameId,
    pub player: PeerId,
    pub commitment: [u8; 32], // SHA256 hash of nonce
    pub round: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RandomnessReveal {
    pub game_id: GameId,
    pub player: PeerId,
    pub nonce: [u8; 32],
    pub round: u32,
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
            .unwrap()
            .as_secs();
        let mut buf = Vec::new();
        buf.write_u64::<BigEndian>(timestamp).unwrap();
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
            Some(DiceRoll {
                die1: tlv.value[0],
                die2: tlv.value[1],
                total: tlv.value[0] + tlv.value[1],
            })
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
        
        let packet = Self {
            version,
            packet_type,
            flags,
            ttl,
            total_length,
            checksum,
            tlv_data,
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

// Implementations for data types
impl CrapTokens {
    pub fn new(amount: u64) -> Self {
        Self { amount }
    }
    
    pub fn amount(&self) -> u64 {
        self.amount
    }
    
    pub fn from_crap(crap: f64) -> Self {
        Self {
            amount: (crap * 1_000_000.0) as u64, // 1 CRAP = 1,000,000 units
        }
    }
    
    pub fn to_crap(&self) -> f64 {
        self.amount as f64 / 1_000_000.0
    }
}

impl DiceRoll {
    pub fn new(die1: u8, die2: u8) -> Self {
        Self {
            die1,
            die2,
            total: die1 + die2,
        }
    }
    
    pub fn is_craps(&self) -> bool {
        matches!(self.total, 2 | 3 | 12)
    }
    
    pub fn is_natural(&self) -> bool {
        matches!(self.total, 7 | 11)
    }
    
    pub fn is_point(&self) -> bool {
        matches!(self.total, 4 | 5 | 6 | 8 | 9 | 10)
    }
    
    pub fn is_hard_way(&self) -> bool {
        self.die1 == self.die2 && matches!(self.total, 4 | 6 | 8 | 10)
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
        let roll = DiceRoll::new(3, 4);
        assert_eq!(roll.total, 7);
        assert!(roll.is_natural());
        assert!(!roll.is_craps());
        assert!(!roll.is_hard_way());
        
        let craps_roll = DiceRoll::new(1, 1);
        assert_eq!(craps_roll.total, 2);
        assert!(craps_roll.is_craps());
        assert!(!craps_roll.is_natural());
    }
    
    #[test]
    fn test_crap_tokens() {
        let tokens = CrapTokens::from_crap(5.5);
        assert_eq!(tokens.amount(), 5_500_000);
        assert_eq!(tokens.to_crap(), 5.5);
    }
}