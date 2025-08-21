// src/protocol/binary.rs
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Cursor, Read, Write};
use lz4_flex::{compress_prepend_size, decompress_size_prepended};

use super::{BitchatPacket, ProtocolError, ProtocolResult, PeerId};
use super::constants::*;

pub struct BinaryProtocol;

impl BinaryProtocol {
    /// Encode a packet to binary format
    pub fn encode(packet: &BitchatPacket) -> ProtocolResult<Vec<u8>> {
        let mut buffer = Vec::with_capacity(MAX_PACKET_SIZE);
        
        // Prepare payload (compress if beneficial)
        let (final_payload, is_compressed) = Self::prepare_payload(&packet.payload)?;
        
        // Calculate flags
        let mut flags = packet.flags;
        if is_compressed {
            flags |= FLAG_PAYLOAD_COMPRESSED;
        }
        
        // Write fixed header (13 bytes)
        buffer.write_u8(packet.version)?;
        buffer.write_u8(packet.packet_type)?;
        buffer.write_u8(packet.ttl)?;
        buffer.write_u64::<BigEndian>(packet.timestamp)?;
        buffer.write_u8(flags)?;
        
        // Calculate total payload length including optional fields
        let mut total_payload_len = 32 + final_payload.len(); // sender_id + payload
        if flags & FLAG_RECIPIENT_PRESENT != 0 {
            total_payload_len += 32; // recipient_id
        }
        if flags & FLAG_SIGNATURE_PRESENT != 0 {
            total_payload_len += packet.signature.as_ref().map_or(0, |s| s.len());
        }
        
        buffer.write_u16::<BigEndian>(total_payload_len as u16)?;
        
        // Write sender ID (32 bytes)
        buffer.extend_from_slice(packet.sender_id.as_bytes());
        
        // Write optional recipient ID
        if flags & FLAG_RECIPIENT_PRESENT != 0 {
            if let Some(recipient) = &packet.recipient_id {
                buffer.extend_from_slice(recipient.as_bytes());
            } else {
                return Err(ProtocolError::InvalidHeader(
                    "Recipient flag set but no recipient provided".to_string()
                ));
            }
        }
        
        // Write payload
        buffer.extend_from_slice(&final_payload);
        
        // Write optional signature
        if flags & FLAG_SIGNATURE_PRESENT != 0 {
            if let Some(signature) = &packet.signature {
                buffer.extend_from_slice(signature);
            } else {
                return Err(ProtocolError::InvalidHeader(
                    "Signature flag set but no signature provided".to_string()
                ));
            }
        }
        
        Ok(buffer)
    }
    
    /// Decode binary data to a packet
    pub fn decode(data: &[u8]) -> ProtocolResult<BitchatPacket> {
        if data.len() < HEADER_SIZE {
            return Err(ProtocolError::PacketTooSmall {
                expected: HEADER_SIZE,
                actual: data.len(),
            });
        }
        
        let mut cursor = Cursor::new(data);
        
        // Read fixed header
        let version = cursor.read_u8()?;
        if version != PROTOCOL_VERSION {
            return Err(ProtocolError::InvalidVersion {
                expected: PROTOCOL_VERSION,
                actual: version,
            });
        }
        
        let packet_type = cursor.read_u8()?;
        let ttl = cursor.read_u8()?;
        let timestamp = cursor.read_u64::<BigEndian>()?;
        let flags = cursor.read_u8()?;
        let payload_length = cursor.read_u16::<BigEndian>()?;
        
        // Validate remaining data length
        let remaining = data.len() - HEADER_SIZE;
        if remaining != payload_length as usize {
            return Err(ProtocolError::InvalidHeader(
                format!("Payload length mismatch: header says {}, got {}", 
                    payload_length, remaining)
            ));
        }
        
        // Read sender ID
        let mut sender_bytes = [0u8; 32];
        cursor.read_exact(&mut sender_bytes)?;
        let sender_id = PeerId::new(sender_bytes);
        
        // Read optional recipient ID
        let recipient_id = if flags & FLAG_RECIPIENT_PRESENT != 0 {
            let mut recipient_bytes = [0u8; 32];
            cursor.read_exact(&mut recipient_bytes)?;
            Some(PeerId::new(recipient_bytes))
        } else {
            None
        };
        
        // Calculate payload size
        let mut payload_size = remaining - 32; // Subtract sender ID
        if flags & FLAG_RECIPIENT_PRESENT != 0 {
            payload_size -= 32; // Subtract recipient ID
        }
        
        // Read signature if present (signature comes after payload)
        let signature = if flags & FLAG_SIGNATURE_PRESENT != 0 {
            // For now, assume 64-byte Ed25519 signature
            payload_size -= 64;
            let mut sig_bytes = vec![0u8; 64];
            
            // We need to read the payload first, then the signature
            let mut payload_bytes = vec![0u8; payload_size];
            cursor.read_exact(&mut payload_bytes)?;
            cursor.read_exact(&mut sig_bytes)?;
            
            // Handle payload decompression
            let final_payload = if flags & FLAG_PAYLOAD_COMPRESSED != 0 {
                Self::decompress_payload(&payload_bytes)?
            } else {
                payload_bytes
            };
            
            return Ok(BitchatPacket {
                version,
                packet_type,
                ttl,
                timestamp,
                flags,
                payload_length: final_payload.len() as u16,
                sender_id,
                recipient_id,
                payload: final_payload,
                signature: Some(sig_bytes),
            });
        } else {
            None
        };
        
        // Read payload
        let mut payload_bytes = vec![0u8; payload_size];
        cursor.read_exact(&mut payload_bytes)?;
        
        // Handle payload decompression
        let final_payload = if flags & FLAG_PAYLOAD_COMPRESSED != 0 {
            Self::decompress_payload(&payload_bytes)?
        } else {
            payload_bytes
        };
        
        Ok(BitchatPacket {
            version,
            packet_type,
            ttl,
            timestamp,
            flags,
            payload_length: final_payload.len() as u16,
            sender_id,
            recipient_id,
            payload: final_payload,
            signature,
        })
    }
    
    /// Prepare payload for transmission (compress if beneficial)
    fn prepare_payload(payload: &[u8]) -> ProtocolResult<(Vec<u8>, bool)> {
        // Only compress if payload is larger than threshold
        if payload.len() > 64 {
            let compressed = compress_prepend_size(payload);
            // Only use compression if it actually reduces size
            if compressed.len() < payload.len() {
                return Ok((compressed, true));
            }
        }
        
        Ok((payload.to_vec(), false))
    }
    
    /// Decompress payload
    fn decompress_payload(compressed: &[u8]) -> ProtocolResult<Vec<u8>> {
        decompress_size_prepended(compressed)
            .map_err(|e| ProtocolError::DecompressionError(format!("Decompression error: {:?}", e)))
    }
}