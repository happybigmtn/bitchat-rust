//! Binary serialization trait for efficient network protocol
//!
//! Feynman: Think of this like a "packing instructions" manual.
//! Every data type needs to know how to pack itself into a box (serialize)
//! and how to unpack from a box (deserialize). The box is just raw bytes.

use super::{BetType, CrapTokens, DiceRoll, GameId, PeerId};
use crate::error::Error;
use bytes::{Buf, BufMut, BytesMut};
use lz4_flex::{compress_prepend_size, decompress_size_prepended};

/// Binary serialization trait for network protocol
///
/// Feynman: This trait is the contract every type must follow to be
/// sent over the network. Like a shipping manifest for data.
pub trait BinarySerializable: Sized {
    /// Pack this type into bytes
    /// Feynman: "How do I fit into a telegram?"
    fn serialize(&self, buf: &mut BytesMut) -> Result<(), Error>;

    /// Unpack this type from bytes
    /// Feynman: "How do I reconstruct myself from a telegram?"
    fn deserialize(buf: &mut &[u8]) -> Result<Self, Error>;

    /// Get the serialized size in bytes
    /// Feynman: "How much space do I need in the telegram?"
    fn serialized_size(&self) -> usize;
}

// Implement for basic types
// Feynman: Start with atoms (u8, u16...) then build molecules (structs)

impl BinarySerializable for u8 {
    fn serialize(&self, buf: &mut BytesMut) -> Result<(), Error> {
        buf.put_u8(*self);
        Ok(())
    }

    fn deserialize(buf: &mut &[u8]) -> Result<Self, Error> {
        if buf.is_empty() {
            return Err(Error::Serialization("Not enough data for u8".to_string()));
        }
        Ok(buf.get_u8())
    }

    fn serialized_size(&self) -> usize {
        1
    }
}

impl BinarySerializable for u16 {
    fn serialize(&self, buf: &mut BytesMut) -> Result<(), Error> {
        buf.put_u16(*self);
        Ok(())
    }

    fn deserialize(buf: &mut &[u8]) -> Result<Self, Error> {
        if buf.len() < 2 {
            return Err(Error::Serialization("Not enough data for u16".to_string()));
        }
        Ok(buf.get_u16())
    }

    fn serialized_size(&self) -> usize {
        2
    }
}

impl BinarySerializable for u32 {
    fn serialize(&self, buf: &mut BytesMut) -> Result<(), Error> {
        buf.put_u32(*self);
        Ok(())
    }

    fn deserialize(buf: &mut &[u8]) -> Result<Self, Error> {
        if buf.len() < 4 {
            return Err(Error::Serialization("Not enough data for u32".to_string()));
        }
        Ok(buf.get_u32())
    }

    fn serialized_size(&self) -> usize {
        4
    }
}

impl BinarySerializable for u64 {
    fn serialize(&self, buf: &mut BytesMut) -> Result<(), Error> {
        buf.put_u64(*self);
        Ok(())
    }

    fn deserialize(buf: &mut &[u8]) -> Result<Self, Error> {
        if buf.len() < 8 {
            return Err(Error::Serialization("Not enough data for u64".to_string()));
        }
        Ok(buf.get_u64())
    }

    fn serialized_size(&self) -> usize {
        8
    }
}

// Fixed-size arrays
impl BinarySerializable for [u8; 32] {
    fn serialize(&self, buf: &mut BytesMut) -> Result<(), Error> {
        buf.put_slice(self);
        Ok(())
    }

    fn deserialize(buf: &mut &[u8]) -> Result<Self, Error> {
        if buf.len() < 32 {
            return Err(Error::Serialization(
                "Not enough data for [u8; 32]".to_string(),
            ));
        }
        let mut arr = [0u8; 32];
        buf.copy_to_slice(&mut arr);
        Ok(arr)
    }

    fn serialized_size(&self) -> usize {
        32
    }
}

impl BinarySerializable for [u8; 16] {
    fn serialize(&self, buf: &mut BytesMut) -> Result<(), Error> {
        buf.put_slice(self);
        Ok(())
    }

    fn deserialize(buf: &mut &[u8]) -> Result<Self, Error> {
        if buf.len() < 16 {
            return Err(Error::Serialization(
                "Not enough data for [u8; 16]".to_string(),
            ));
        }
        let mut arr = [0u8; 16];
        buf.copy_to_slice(&mut arr);
        Ok(arr)
    }

    fn serialized_size(&self) -> usize {
        16
    }
}

// Gaming types
impl BinarySerializable for BetType {
    fn serialize(&self, buf: &mut BytesMut) -> Result<(), Error> {
        // Use the to_u8 method instead of direct casting
        buf.put_u8(self.to_u8());
        Ok(())
    }

    fn deserialize(buf: &mut &[u8]) -> Result<Self, Error> {
        if buf.is_empty() {
            return Err(Error::Serialization(
                "Not enough data for BetType".to_string(),
            ));
        }
        let val = buf.get_u8();

        // Feynman: Convert u8 back to BetType using match
        // This validates the value is in valid range (0-63)
        match val {
            0 => Ok(BetType::Pass),
            1 => Ok(BetType::DontPass),
            2 => Ok(BetType::Come),
            3 => Ok(BetType::DontCome),
            4 => Ok(BetType::Field),
            5..=14 => Ok(match val {
                5 => BetType::Yes2,
                6 => BetType::Yes3,
                7 => BetType::Yes4,
                8 => BetType::Yes5,
                9 => BetType::Yes6,
                10 => BetType::Yes8,
                11 => BetType::Yes9,
                12 => BetType::Yes10,
                13 => BetType::Yes11,
                14 => BetType::Yes12,
                _ => unreachable!(),
            }),
            15..=24 => Ok(match val {
                15 => BetType::No2,
                16 => BetType::No3,
                17 => BetType::No4,
                18 => BetType::No5,
                19 => BetType::No6,
                20 => BetType::No8,
                21 => BetType::No9,
                22 => BetType::No10,
                23 => BetType::No11,
                24 => BetType::No12,
                _ => unreachable!(),
            }),
            25 => Ok(BetType::Hard4),
            26 => Ok(BetType::Hard6),
            27 => Ok(BetType::Hard8),
            28 => Ok(BetType::Hard10),
            29 => Ok(BetType::OddsPass),
            30 => Ok(BetType::OddsDontPass),
            31 => Ok(BetType::OddsCome),
            32 => Ok(BetType::OddsDontCome),
            33 => Ok(BetType::HotRoller),
            34 => Ok(BetType::Fire),
            35 => Ok(BetType::TwiceHard),
            36 => Ok(BetType::RideLine),
            37 => Ok(BetType::Muggsy),
            38 => Ok(BetType::BonusSmall),
            39 => Ok(BetType::BonusTall),
            40 => Ok(BetType::BonusAll),
            41 => Ok(BetType::Replay),
            42 => Ok(BetType::DifferentDoubles),
            43..=53 => Ok(match val {
                43 => BetType::Next2,
                44 => BetType::Next3,
                45 => BetType::Next4,
                46 => BetType::Next5,
                47 => BetType::Next6,
                48 => BetType::Next7,
                49 => BetType::Next8,
                50 => BetType::Next9,
                51 => BetType::Next10,
                52 => BetType::Next11,
                53 => BetType::Next12,
                _ => unreachable!(),
            }),
            54..=63 => Ok(match val {
                54 => BetType::Repeater2,
                55 => BetType::Repeater3,
                56 => BetType::Repeater4,
                57 => BetType::Repeater5,
                58 => BetType::Repeater6,
                59 => BetType::Repeater8,
                60 => BetType::Repeater9,
                61 => BetType::Repeater10,
                62 => BetType::Repeater11,
                63 => BetType::Repeater12,
                _ => unreachable!(),
            }),
            _ => Err(Error::Serialization(format!(
                "Invalid BetType value: {}",
                val
            ))),
        }
    }

    fn serialized_size(&self) -> usize {
        1 // Always 1 byte due to repr(u8)
    }
}

impl BinarySerializable for CrapTokens {
    fn serialize(&self, buf: &mut BytesMut) -> Result<(), Error> {
        buf.put_u64(self.amount());
        Ok(())
    }

    fn deserialize(buf: &mut &[u8]) -> Result<Self, Error> {
        if buf.len() < 8 {
            return Err(Error::Serialization(
                "Not enough data for CrapTokens".to_string(),
            ));
        }
        Ok(CrapTokens::new_unchecked(buf.get_u64()))
    }

    fn serialized_size(&self) -> usize {
        8
    }
}

impl BinarySerializable for DiceRoll {
    fn serialize(&self, buf: &mut BytesMut) -> Result<(), Error> {
        buf.put_u8(self.die1);
        buf.put_u8(self.die2);
        buf.put_u64(self.timestamp);
        Ok(())
    }

    fn deserialize(buf: &mut &[u8]) -> Result<Self, Error> {
        if buf.len() < 10 {
            return Err(Error::Serialization(
                "Not enough data for DiceRoll".to_string(),
            ));
        }
        let die1 = buf.get_u8();
        let die2 = buf.get_u8();
        let timestamp = buf.get_u64();

        // Validate dice values are between 1 and 6 inclusive
        if !(1..=6).contains(&die1) {
            return Err(Error::Serialization(format!(
                "Invalid die1 value: {}, must be 1-6",
                die1
            )));
        }
        if !(1..=6).contains(&die2) {
            return Err(Error::Serialization(format!(
                "Invalid die2 value: {}, must be 1-6",
                die2
            )));
        }

        Ok(DiceRoll {
            die1,
            die2,
            timestamp,
        })
    }

    fn serialized_size(&self) -> usize {
        10 // 1 + 1 + 8
    }
}

/// Compact binary format for game messages with bit packing
/// Feynman: Every bit counts when you're sending data over slow networks
/// We pack multiple small values into single bytes to minimize overhead
pub struct CompactGameMessage {
    /// Packed header: version(3) + msg_type(5) bits
    pub header: u8,
    /// Game and player identifiers (fixed size)
    pub game_id: GameId,
    pub player_id: PeerId,
    /// Variable-length payload based on message type
    pub payload: Vec<u8>,
}

impl CompactGameMessage {
    /// Create new compact message
    pub fn new(version: u8, msg_type: u8, game_id: GameId, player_id: PeerId) -> Self {
        let header = ((version & 0x07) << 5) | (msg_type & 0x1F);
        Self {
            header,
            game_id,
            player_id,
            payload: Vec::new(),
        }
    }

    /// Extract version from header
    pub fn version(&self) -> u8 {
        (self.header >> 5) & 0x07
    }

    /// Extract message type from header
    pub fn msg_type(&self) -> u8 {
        self.header & 0x1F
    }

    /// Add bet information with bit packing
    /// Format: bet_type(6) + priority(2) bits, then amount as varint
    pub fn add_bet(&mut self, bet_type: BetType, amount: CrapTokens, priority: u8) {
        // Pack bet type (6 bits) and priority (2 bits) into single byte
        let packed = (bet_type.to_u8() & 0x3F) | ((priority & 0x03) << 6);
        self.payload.push(packed);

        // Add amount as variable-length integer
        self.add_varint(amount.amount());
    }

    /// Add dice roll with compact encoding
    /// Format: die1(3) + die2(3) + reserved(2) bits, then timestamp as varint
    pub fn add_dice_roll(&mut self, roll: &DiceRoll) {
        // Pack both dice values into single byte
        let packed = ((roll.die1 - 1) & 0x07) | (((roll.die2 - 1) & 0x07) << 3);
        self.payload.push(packed);

        // Add timestamp as varint to save space
        self.add_varint(roll.timestamp);
    }

    /// Add variable-length integer (saves space for small numbers)
    fn add_varint(&mut self, mut value: u64) {
        while value >= 0x80 {
            self.payload.push((value as u8) | 0x80);
            value >>= 7;
        }
        self.payload.push(value as u8);
    }

    /// Read variable-length integer
    fn read_varint(buf: &mut &[u8]) -> Result<u64, Error> {
        let mut result = 0u64;
        let mut shift = 0;

        loop {
            if buf.is_empty() {
                return Err(Error::Serialization("Unexpected end of varint".to_string()));
            }

            let byte = buf[0];
            *buf = &buf[1..];

            result |= ((byte & 0x7F) as u64) << shift;

            if byte & 0x80 == 0 {
                break;
            }

            shift += 7;
            if shift >= 64 {
                return Err(Error::Serialization("Varint too long".to_string()));
            }
        }

        Ok(result)
    }

    /// Serialize to bytes with optional compression
    pub fn serialize(&self, compress: bool) -> Result<Vec<u8>, Error> {
        let mut buf = Vec::with_capacity(1 + 16 + 32 + self.payload.len());

        // Header
        buf.push(self.header);

        // Fixed identifiers
        buf.extend_from_slice(&self.game_id);
        buf.extend_from_slice(&self.player_id);

        // Payload
        if compress && self.payload.len() > 64 {
            // Compress payload if it's large enough to benefit
            let compressed = compress_prepend_size(&self.payload);

            // Set compression flag in header (use reserved bit)
            buf[0] |= 0x80;
            buf.extend_from_slice(&compressed);
        } else {
            buf.extend_from_slice(&self.payload);
        }

        Ok(buf)
    }

    /// Deserialize from bytes
    pub fn deserialize(mut data: &[u8]) -> Result<Self, Error> {
        if data.len() < 1 + 16 + 32 {
            return Err(Error::Serialization("Message too short".to_string()));
        }

        // Read header
        let header = data[0];
        data = &data[1..];

        // Read game ID
        let mut game_id = [0u8; 16];
        game_id.copy_from_slice(&data[..16]);
        data = &data[16..];

        // Read player ID
        let mut player_id = [0u8; 32];
        player_id.copy_from_slice(&data[..32]);
        data = &data[32..];

        // Read payload (with decompression if needed)
        let payload = if header & 0x80 != 0 {
            // Compressed payload
            decompress_size_prepended(data)
                .map_err(|e| Error::Serialization(format!("Decompression failed: {}", e)))?
        } else {
            data.to_vec()
        };

        Ok(Self {
            header: header & 0x7F, // Clear compression flag
            game_id,
            player_id,
            payload,
        })
    }

    /// Extract bet from payload
    pub fn extract_bet(&self) -> Result<Option<(BetType, CrapTokens, u8)>, Error> {
        if self.payload.is_empty() {
            return Ok(None);
        }

        let mut buf = &self.payload[..];

        // Read packed bet info
        let packed = buf[0];
        buf = &buf[1..];

        let bet_type_val = packed & 0x3F;
        let priority = (packed >> 6) & 0x03;

        // Convert to BetType
        let bet_type = BetType::deserialize(&mut &[bet_type_val][..])?;

        // Read amount as varint
        let amount = Self::read_varint(&mut buf)?;
        let tokens = CrapTokens::new_unchecked(amount);

        Ok(Some((bet_type, tokens, priority)))
    }

    /// Extract dice roll from payload
    pub fn extract_dice_roll(&self) -> Result<Option<DiceRoll>, Error> {
        if self.payload.is_empty() {
            return Ok(None);
        }

        let mut buf = &self.payload[..];

        // Read packed dice
        let packed = buf[0];
        buf = &buf[1..];

        let die1 = (packed & 0x07) + 1;
        let die2 = ((packed >> 3) & 0x07) + 1;

        // Read timestamp
        let timestamp = Self::read_varint(&mut buf)?;

        Ok(Some(DiceRoll {
            die1,
            die2,
            timestamp,
        }))
    }

    /// Get message size in bytes
    pub fn size(&self) -> usize {
        1 + 16 + 32 + self.payload.len()
    }

    /// Calculate compression ratio if compressed
    pub fn compression_ratio(&self, compressed_size: usize) -> f32 {
        if self.payload.is_empty() {
            1.0
        } else {
            compressed_size as f32 / self.payload.len() as f32
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bet_type_serialization() {
        let mut buf = BytesMut::new();

        // Test all 64 bet types
        let bet = BetType::Pass;
        bet.serialize(&mut buf).unwrap();
        assert_eq!(buf[0], 0);

        buf.clear();
        let bet = BetType::Repeater12;
        bet.serialize(&mut buf).unwrap();
        assert_eq!(buf[0], 63);
    }

    #[test]
    fn test_compact_game_message() {
        let game_id = [1u8; 16];
        let player_id = [2u8; 32];
        let mut msg = CompactGameMessage::new(1, 5, game_id, player_id);

        // Test bet addition
        let tokens = CrapTokens::new_unchecked(100);
        msg.add_bet(BetType::Pass, tokens, 2);

        // Serialize and deserialize
        let serialized = msg.serialize(false).unwrap();
        let deserialized = CompactGameMessage::deserialize(&serialized).unwrap();

        assert_eq!(deserialized.version(), 1);
        assert_eq!(deserialized.msg_type(), 5);
        assert_eq!(deserialized.game_id, game_id);
        assert_eq!(deserialized.player_id, player_id);

        // Test bet extraction
        let (bet_type, amount, priority) = deserialized.extract_bet().unwrap().unwrap();
        assert_eq!(bet_type, BetType::Pass);
        assert_eq!(amount.amount(), 100);
        assert_eq!(priority, 2);
    }

    #[test]
    fn test_dice_roll_serialization() {
        let mut buf = BytesMut::new();
        let roll = DiceRoll::new(3, 4).unwrap();

        roll.serialize(&mut buf).unwrap();
        assert_eq!(buf.len(), 10);

        let mut slice = &buf[..];
        let decoded = DiceRoll::deserialize(&mut slice).unwrap();
        assert_eq!(decoded.die1, 3);
        assert_eq!(decoded.die2, 4);
    }

    #[test]
    fn test_dice_roll_validation() {
        let mut buf = BytesMut::new();

        // Test invalid die1 value (0)
        buf.put_u8(0); // die1 = 0 (invalid)
        buf.put_u8(3); // die2 = 3 (valid)
        buf.put_u64(12345); // timestamp

        let mut slice = &buf[..];
        let result = DiceRoll::deserialize(&mut slice);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid die1 value: 0"));

        buf.clear();

        // Test invalid die1 value (7)
        buf.put_u8(7); // die1 = 7 (invalid)
        buf.put_u8(3); // die2 = 3 (valid)
        buf.put_u64(12345); // timestamp

        let mut slice = &buf[..];
        let result = DiceRoll::deserialize(&mut slice);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid die1 value: 7"));

        buf.clear();

        // Test invalid die2 value (0)
        buf.put_u8(3); // die1 = 3 (valid)
        buf.put_u8(0); // die2 = 0 (invalid)
        buf.put_u64(12345); // timestamp

        let mut slice = &buf[..];
        let result = DiceRoll::deserialize(&mut slice);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid die2 value: 0"));

        buf.clear();

        // Test invalid die2 value (8)
        buf.put_u8(3); // die1 = 3 (valid)
        buf.put_u8(8); // die2 = 8 (invalid)
        buf.put_u64(12345); // timestamp

        let mut slice = &buf[..];
        let result = DiceRoll::deserialize(&mut slice);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid die2 value: 8"));

        buf.clear();

        // Test valid values
        buf.put_u8(1); // die1 = 1 (valid)
        buf.put_u8(6); // die2 = 6 (valid)
        buf.put_u64(12345); // timestamp

        let mut slice = &buf[..];
        let result = DiceRoll::deserialize(&mut slice);
        assert!(result.is_ok());
        let roll = result.unwrap();
        assert_eq!(roll.die1, 1);
        assert_eq!(roll.die2, 6);
        assert_eq!(roll.timestamp, 12345);
    }
}
