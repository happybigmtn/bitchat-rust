//! Zero-Copy Message Serialization and Deserialization
//! 
//! This module provides high-performance serialization with minimal memory allocation
//! and copying, particularly important for real-time game communication.

use bytes::{Bytes, BytesMut, BufMut};
use crate::error::{Error, Result};
use crate::protocol::{BitchatPacket, PeerId, GameId};

/// Zero-copy serialization buffer that reuses allocated memory
pub struct ZeroCopyBuffer {
    buffer: BytesMut,
    write_position: usize,
}

/// Zero-copy message writer for efficient serialization
pub struct ZeroCopyWriter<'a> {
    buffer: &'a mut ZeroCopyBuffer,
}

/// Zero-copy message reader for efficient deserialization
pub struct ZeroCopyReader {
    data: Bytes,
    position: usize,
}

/// Serializable trait for zero-copy operations
pub trait ZeroCopySerialize {
    fn serialize_zero_copy(&self, writer: &mut ZeroCopyWriter) -> Result<()>;
    fn serialized_size(&self) -> usize;
}

/// Deserializable trait for zero-copy operations
pub trait ZeroCopyDeserialize: Sized {
    fn deserialize_zero_copy(reader: &mut ZeroCopyReader) -> Result<Self>;
}

impl ZeroCopyBuffer {
    /// Create a new zero-copy buffer with initial capacity
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: BytesMut::with_capacity(capacity),
            write_position: 0,
        }
    }

    /// Reset buffer for reuse without deallocating memory
    pub fn reset(&mut self) {
        self.write_position = 0;
        self.buffer.clear();
    }

    /// Get the current capacity
    pub fn capacity(&self) -> usize {
        self.buffer.capacity()
    }

    /// Get the current length of serialized data
    pub fn len(&self) -> usize {
        self.write_position
    }

    /// Check if buffer is empty
    pub fn is_empty(&self) -> bool {
        self.write_position == 0
    }

    /// Get a zero-copy writer for this buffer
    pub fn writer(&mut self) -> ZeroCopyWriter {
        ZeroCopyWriter { buffer: self }
    }

    /// Get the serialized data as bytes without copying
    pub fn as_bytes(&self) -> &[u8] {
        &self.buffer[..self.write_position]
    }

    /// Convert to frozen bytes for network transmission
    pub fn freeze(self) -> Bytes {
        let mut buffer = self.buffer;
        buffer.truncate(self.write_position);
        buffer.freeze()
    }

    /// Reserve additional capacity if needed
    pub fn reserve(&mut self, additional: usize) {
        self.buffer.reserve(additional);
    }
}

impl<'a> ZeroCopyWriter<'a> {
    /// Write raw bytes directly
    pub fn write_bytes(&mut self, data: &[u8]) -> Result<()> {
        if self.buffer.buffer.remaining_mut() < data.len() {
            self.buffer.buffer.reserve(data.len());
        }
        
        self.buffer.buffer.put_slice(data);
        self.buffer.write_position += data.len();
        Ok(())
    }

    /// Write a u8 value
    pub fn write_u8(&mut self, value: u8) -> Result<()> {
        if self.buffer.buffer.remaining_mut() < 1 {
            self.buffer.buffer.reserve(64); // Reserve some extra space
        }
        
        self.buffer.buffer.put_u8(value);
        self.buffer.write_position += 1;
        Ok(())
    }

    /// Write a u16 value in big-endian format
    pub fn write_u16_be(&mut self, value: u16) -> Result<()> {
        if self.buffer.buffer.remaining_mut() < 2 {
            self.buffer.buffer.reserve(64);
        }
        
        self.buffer.buffer.put_u16(value);
        self.buffer.write_position += 2;
        Ok(())
    }

    /// Write a u32 value in big-endian format
    pub fn write_u32_be(&mut self, value: u32) -> Result<()> {
        if self.buffer.buffer.remaining_mut() < 4 {
            self.buffer.buffer.reserve(64);
        }
        
        self.buffer.buffer.put_u32(value);
        self.buffer.write_position += 4;
        Ok(())
    }

    /// Write a u64 value in big-endian format
    pub fn write_u64_be(&mut self, value: u64) -> Result<()> {
        if self.buffer.buffer.remaining_mut() < 8 {
            self.buffer.buffer.reserve(64);
        }
        
        self.buffer.buffer.put_u64(value);
        self.buffer.write_position += 8;
        Ok(())
    }

    /// Write a variable-length byte array with length prefix
    pub fn write_bytes_with_length(&mut self, data: &[u8]) -> Result<()> {
        if data.len() > u32::MAX as usize {
            return Err(Error::Protocol("Data too large for length prefix".to_string()));
        }
        
        self.write_u32_be(data.len() as u32)?;
        self.write_bytes(data)?;
        Ok(())
    }

    /// Write a peer ID (32 bytes)
    pub fn write_peer_id(&mut self, peer_id: &PeerId) -> Result<()> {
        self.write_bytes(peer_id)
    }

    /// Write a game ID (16 bytes)  
    pub fn write_game_id(&mut self, game_id: &GameId) -> Result<()> {
        self.write_bytes(game_id)
    }
}

impl ZeroCopyReader {
    /// Create a new zero-copy reader from bytes
    pub fn new(data: Bytes) -> Self {
        Self { data, position: 0 }
    }

    /// Get remaining bytes available to read
    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.position)
    }

    /// Check if there are no more bytes to read
    pub fn is_empty(&self) -> bool {
        self.position >= self.data.len()
    }

    /// Read raw bytes without copying
    pub fn read_bytes(&mut self, len: usize) -> Result<&[u8]> {
        if self.remaining() < len {
            return Err(Error::Protocol("Not enough data to read".to_string()));
        }
        
        let start = self.position;
        self.position += len;
        Ok(&self.data[start..self.position])
    }

    /// Read a u8 value
    pub fn read_u8(&mut self) -> Result<u8> {
        if self.remaining() < 1 {
            return Err(Error::Protocol("Not enough data for u8".to_string()));
        }
        
        let value = self.data[self.position];
        self.position += 1;
        Ok(value)
    }

    /// Read a u16 value in big-endian format
    pub fn read_u16_be(&mut self) -> Result<u16> {
        if self.remaining() < 2 {
            return Err(Error::Protocol("Not enough data for u16".to_string()));
        }
        
        let bytes = &self.data[self.position..self.position + 2];
        self.position += 2;
        Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
    }

    /// Read a u32 value in big-endian format
    pub fn read_u32_be(&mut self) -> Result<u32> {
        if self.remaining() < 4 {
            return Err(Error::Protocol("Not enough data for u32".to_string()));
        }
        
        let bytes = &self.data[self.position..self.position + 4];
        self.position += 4;
        Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    /// Read a u64 value in big-endian format
    pub fn read_u64_be(&mut self) -> Result<u64> {
        if self.remaining() < 8 {
            return Err(Error::Protocol("Not enough data for u64".to_string()));
        }
        
        let bytes = &self.data[self.position..self.position + 8];
        self.position += 8;
        Ok(u64::from_be_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3],
            bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    /// Read a variable-length byte array with length prefix
    pub fn read_bytes_with_length(&mut self) -> Result<&[u8]> {
        let len = self.read_u32_be()? as usize;
        self.read_bytes(len)
    }

    /// Read a peer ID (32 bytes)
    pub fn read_peer_id(&mut self) -> Result<PeerId> {
        let bytes = self.read_bytes(32)?;
        let mut peer_id = [0u8; 32];
        peer_id.copy_from_slice(bytes);
        Ok(peer_id)
    }

    /// Read a game ID (16 bytes)
    pub fn read_game_id(&mut self) -> Result<GameId> {
        let bytes = self.read_bytes(16)?;
        let mut game_id = [0u8; 16];
        game_id.copy_from_slice(bytes);
        Ok(game_id)
    }

    /// Peek at the next bytes without advancing position
    pub fn peek_bytes(&self, len: usize) -> Result<&[u8]> {
        if self.remaining() < len {
            return Err(Error::Protocol("Not enough data to peek".to_string()));
        }
        
        Ok(&self.data[self.position..self.position + len])
    }
}

/// Efficient message serializer with buffer reuse
pub struct MessageSerializer {
    buffer_pool: crate::memory_pool::MemoryPool<ZeroCopyBuffer>,
}

impl MessageSerializer {
    /// Create a new message serializer with buffer pool
    pub fn new(pool_size: usize, buffer_capacity: usize) -> Self {
        Self {
            buffer_pool: crate::memory_pool::MemoryPool::with_factory(
                pool_size,
                move || ZeroCopyBuffer::new(buffer_capacity),
            ),
        }
    }

    /// Serialize a message using a pooled buffer
    pub async fn serialize<T: ZeroCopySerialize>(&self, message: &T) -> Result<Bytes> {
        let mut buffer = self.buffer_pool.get().await;
        buffer.reset(); // Prepare buffer for reuse
        
        // Reserve space if we know the size
        let size_hint = message.serialized_size();
        if size_hint > 0 {
            let current_capacity = buffer.capacity();
            if current_capacity < size_hint {
                buffer.reserve(size_hint - current_capacity);
            }
        }
        
        // Serialize the message
        {
            let mut writer = buffer.writer();
            message.serialize_zero_copy(&mut writer)?;
        }
        
        // Get the final bytes
        let bytes = buffer.as_bytes().to_vec();
        Ok(Bytes::from(bytes))
    }

    /// Get pool statistics
    pub async fn pool_stats(&self) -> crate::memory_pool::PoolStats {
        self.buffer_pool.stats().await
    }
}

/// High-performance packet serialization for BitchatPacket
impl ZeroCopySerialize for BitchatPacket {
    fn serialize_zero_copy(&self, writer: &mut ZeroCopyWriter) -> Result<()> {
        // Write packet header
        writer.write_u8(self.version)?;
        writer.write_u8(self.packet_type)?;
        writer.write_u8(self.flags)?;
        writer.write_u8(self.ttl)?;
        writer.write_u32_be(self.total_length)?;
        writer.write_u64_be(self.sequence)?;
        
        // Write TLV data
        writer.write_u32_be(self.tlv_data.len() as u32)?;
        for tlv in &self.tlv_data {
            writer.write_u8(tlv.field_type)?;
            writer.write_u16_be(tlv.length)?;
            writer.write_bytes(&tlv.value)?;
        }
        
        Ok(())
    }

    fn serialized_size(&self) -> usize {
        // Header: 1 + 1 + 1 + 1 + 4 + 4 = 12 bytes
        // TLV count: 4 bytes
        // TLV data: sum of (1 + 2 + value.len()) for each TLV
        let tlv_size: usize = self.tlv_data.iter()
            .map(|tlv| 1 + 2 + tlv.value.len())
            .sum();
        
        12 + 4 + tlv_size
    }
}

impl ZeroCopyDeserialize for BitchatPacket {
    fn deserialize_zero_copy(reader: &mut ZeroCopyReader) -> Result<Self> {
        // Read packet header
        let version = reader.read_u8()?;
        let packet_type = reader.read_u8()?;
        let flags = reader.read_u8()?;
        let ttl = reader.read_u8()?;
        let total_length = reader.read_u32_be()?;
        let sequence = reader.read_u64_be()?;
        
        // Read TLV data
        let tlv_count = reader.read_u32_be()?;
        let mut tlv_data = Vec::with_capacity(tlv_count as usize);
        
        for _ in 0..tlv_count {
            let field_type = reader.read_u8()?;
            let length = reader.read_u16_be()?;
            let value = reader.read_bytes(length as usize)?.to_vec();
            
            tlv_data.push(crate::protocol::TlvField {
                field_type,
                length,
                value,
            });
        }
        
        Ok(BitchatPacket {
            version,
            packet_type,
            flags,
            ttl,
            total_length,
            sequence,
            checksum: 0, // Will be computed after full packet assembly
            source: [0u8; 32], // Will be set by transport layer
            target: [0u8; 32], // Will be set by transport layer
            tlv_data,
            payload: None, // No additional payload in zero-copy format
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zero_copy_buffer_operations() {
        let mut buffer = ZeroCopyBuffer::new(256);
        
        {
            let mut writer = buffer.writer();
            writer.write_u32_be(0x12345678).unwrap();
            writer.write_bytes(b"hello world").unwrap();
            writer.write_u16_be(0xABCD).unwrap();
        }
        
        assert_eq!(buffer.len(), 4 + 11 + 2);
        
        let bytes = buffer.freeze();
        let mut reader = ZeroCopyReader::new(bytes);
        
        assert_eq!(reader.read_u32_be().unwrap(), 0x12345678);
        assert_eq!(reader.read_bytes(11).unwrap(), b"hello world");
        assert_eq!(reader.read_u16_be().unwrap(), 0xABCD);
        assert!(reader.is_empty());
    }

    #[test]
    fn test_buffer_reuse() {
        let mut buffer = ZeroCopyBuffer::new(256);
        
        // First use
        {
            let mut writer = buffer.writer();
            writer.write_bytes(b"first message").unwrap();
        }
        assert_eq!(buffer.len(), 13);
        
        // Reset and reuse
        buffer.reset();
        assert_eq!(buffer.len(), 0);
        assert_eq!(buffer.capacity(), 256); // Capacity preserved
        
        {
            let mut writer = buffer.writer();
            writer.write_bytes(b"second message").unwrap();
        }
        assert_eq!(buffer.len(), 14);
    }

    #[tokio::test]
    async fn test_message_serializer() {
        let serializer = MessageSerializer::new(10, 1024);
        
        // Create a test packet
        let packet = BitchatPacket {
            version: 1,
            packet_type: 42,
            flags: 0,
            ttl: 8,
            total_length: 100,
            sequence: 12345,
            checksum: 0,
            source: [0u8; 32],
            target: [0u8; 32],
            tlv_data: vec![
                crate::protocol::TlvField {
                    field_type: 1,
                    length: 4,
                    value: vec![1, 2, 3, 4],
                },
            ],
            payload: None,
        };
        
        // Serialize
        let bytes = serializer.serialize(&packet).await.unwrap();
        
        // Deserialize
        let mut reader = ZeroCopyReader::new(bytes);
        let deserialized = BitchatPacket::deserialize_zero_copy(&mut reader).unwrap();
        
        // Verify
        assert_eq!(packet.version, deserialized.version);
        assert_eq!(packet.packet_type, deserialized.packet_type);
        assert_eq!(packet.sequence, deserialized.sequence);
        assert_eq!(packet.tlv_data.len(), deserialized.tlv_data.len());
    }

    #[test]
    fn test_peer_id_game_id_serialization() {
        let mut buffer = ZeroCopyBuffer::new(256);
        
        let peer_id: PeerId = [42u8; 32];
        let game_id: GameId = [7u8; 16];
        
        {
            let mut writer = buffer.writer();
            writer.write_peer_id(&peer_id).unwrap();
            writer.write_game_id(&game_id).unwrap();
        }
        
        let bytes = buffer.freeze();
        let mut reader = ZeroCopyReader::new(bytes);
        
        let read_peer_id = reader.read_peer_id().unwrap();
        let read_game_id = reader.read_game_id().unwrap();
        
        assert_eq!(peer_id, read_peer_id);
        assert_eq!(game_id, read_game_id);
    }
}