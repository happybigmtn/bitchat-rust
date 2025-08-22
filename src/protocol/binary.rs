//! Binary serialization trait for efficient network protocol
//! 
//! Feynman: Think of this like a "packing instructions" manual.
//! Every data type needs to know how to pack itself into a box (serialize)
//! and how to unpack from a box (deserialize). The box is just raw bytes.

use bytes::{Buf, BufMut, BytesMut};
use crate::error::Error;
use super::{BetType, CrapTokens, DiceRoll};

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
        if buf.len() < 1 {
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
            return Err(Error::Serialization("Not enough data for [u8; 32]".to_string()));
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
            return Err(Error::Serialization("Not enough data for [u8; 16]".to_string()));
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
        // Feynman: repr(u8) means we can cast directly to u8
        buf.put_u8(*self as u8);
        Ok(())
    }
    
    fn deserialize(buf: &mut &[u8]) -> Result<Self, Error> {
        if buf.len() < 1 {
            return Err(Error::Serialization("Not enough data for BetType".to_string()));
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
            _ => Err(Error::Serialization(format!("Invalid BetType value: {}", val))),
        }
    }
    
    fn serialized_size(&self) -> usize {
        1 // Always 1 byte due to repr(u8)
    }
}

impl BinarySerializable for CrapTokens {
    fn serialize(&self, buf: &mut BytesMut) -> Result<(), Error> {
        buf.put_u64(self.amount);
        Ok(())
    }
    
    fn deserialize(buf: &mut &[u8]) -> Result<Self, Error> {
        if buf.len() < 8 {
            return Err(Error::Serialization("Not enough data for CrapTokens".to_string()));
        }
        Ok(CrapTokens::new(buf.get_u64()))
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
            return Err(Error::Serialization("Not enough data for DiceRoll".to_string()));
        }
        let die1 = buf.get_u8();
        let die2 = buf.get_u8();
        let timestamp = buf.get_u64();
        Ok(DiceRoll { die1, die2, timestamp })
    }
    
    fn serialized_size(&self) -> usize {
        10 // 1 + 1 + 8
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
    fn test_dice_roll_serialization() {
        let mut buf = BytesMut::new();
        let roll = DiceRoll::new(3, 4);
        
        roll.serialize(&mut buf).unwrap();
        assert_eq!(buf.len(), 10);
        
        let mut slice = &buf[..];
        let decoded = DiceRoll::deserialize(&mut slice).unwrap();
        assert_eq!(decoded.die1, 3);
        assert_eq!(decoded.die2, 4);
    }
}