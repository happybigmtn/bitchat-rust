//! Packet creation utilities for the BitCraps gaming protocol
//!
//! This module provides convenience functions for creating common packet types
//! used in the BitCraps peer-to-peer gaming protocol.

use crate::protocol::{
    BitchatPacket, GameId, PeerId, TlvField, PACKET_TYPE_DISCOVERY, PACKET_TYPE_GAME_DATA,
    PACKET_TYPE_PING,
};
use serde::{Deserialize, Serialize};

/// Game creation data for network packets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameCreationData {
    pub game_id: GameId,
    pub max_players: u8,
    pub buy_in: u64, // Buy-in amount in CRAP tokens (microtokens)
    pub creator: PeerId,
    pub timestamp: u64,
}

/// Game discovery data for announcement packets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameDiscoveryData {
    pub game_id: GameId,
    pub current_players: u8,
    pub max_players: u8,
    pub buy_in: u64,
    pub phase: String,
}

/// TLV field type constants for game protocol
pub const TLV_GAME_CREATION: u8 = 0x10;
pub const TLV_GAME_DISCOVERY: u8 = 0x11;
pub const TLV_TIMESTAMP: u8 = 0x12;

impl GameCreationData {
    /// Create new game creation data
    pub fn new(game_id: GameId, creator: PeerId, max_players: u8, buy_in: u64) -> Self {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            game_id,
            max_players,
            buy_in,
            creator,
            timestamp,
        }
    }

    /// Serialize to bytes for packet payload
    pub fn serialize(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        bincode::serialize(self).map_err(|e| e.into())
    }

    /// Deserialize from bytes
    pub fn deserialize(data: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        bincode::deserialize(data).map_err(|e| e.into())
    }
}

impl GameDiscoveryData {
    /// Create new game discovery data
    pub fn new(
        game_id: GameId,
        current_players: u8,
        max_players: u8,
        buy_in: u64,
        phase: String,
    ) -> Self {
        Self {
            game_id,
            current_players,
            max_players,
            buy_in,
            phase,
        }
    }

    /// Serialize to bytes for packet payload
    pub fn serialize(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        bincode::serialize(self).map_err(|e| e.into())
    }

    /// Deserialize from bytes
    pub fn deserialize(data: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        bincode::deserialize(data).map_err(|e| e.into())
    }
}

/// Create a game creation packet for broadcasting new games
pub fn create_game_packet(
    peer_id: PeerId,
    game_id: GameId,
    max_players: u8,
    buy_in: u64,
) -> BitchatPacket {
    let mut packet = BitchatPacket::new(PACKET_TYPE_GAME_DATA);

    // Set source to the game creator
    packet.source = peer_id;

    // Add sender TLV
    packet.add_sender(peer_id);

    // Create game creation data
    let game_data = GameCreationData::new(game_id, peer_id, max_players, buy_in);

    // Serialize game data and add as TLV field
    if let Ok(serialized_data) = game_data.serialize() {
        packet.tlv_data.push(TlvField {
            field_type: TLV_GAME_CREATION,
            length: serialized_data.len() as u16,
            value: serialized_data,
        });
    }

    // Add timestamp TLV
    let timestamp_bytes = game_data.timestamp.to_le_bytes();
    packet.tlv_data.push(TlvField {
        field_type: TLV_TIMESTAMP,
        length: 8,
        value: timestamp_bytes.to_vec(),
    });

    packet
}

/// Create a ping packet for peer discovery
pub fn create_ping_packet(peer_id: PeerId) -> BitchatPacket {
    let mut packet = BitchatPacket::new(PACKET_TYPE_PING);

    // Set source to the sender
    packet.source = peer_id;

    // Add sender TLV
    packet.add_sender(peer_id);

    // Add timestamp TLV
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let timestamp_bytes = timestamp.to_le_bytes();

    packet.tlv_data.push(TlvField {
        field_type: TLV_TIMESTAMP,
        length: 8,
        value: timestamp_bytes.to_vec(),
    });

    packet
}

/// Create a discovery packet for network topology discovery
pub fn create_discovery_packet(peer_id: PeerId) -> BitchatPacket {
    let mut packet = BitchatPacket::new(PACKET_TYPE_DISCOVERY);

    // Set source to the sender
    packet.source = peer_id;

    // Add sender TLV
    packet.add_sender(peer_id);

    // Add timestamp TLV
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let timestamp_bytes = timestamp.to_le_bytes();

    packet.tlv_data.push(TlvField {
        field_type: TLV_TIMESTAMP,
        length: 8,
        value: timestamp_bytes.to_vec(),
    });

    packet
}

/// Create a game discovery packet for announcing available games
pub fn create_game_discovery_packet(
    peer_id: PeerId,
    game_discovery: GameDiscoveryData,
) -> BitchatPacket {
    let mut packet = BitchatPacket::new(PACKET_TYPE_GAME_DATA);

    // Set source to the announcer
    packet.source = peer_id;

    // Add sender TLV
    packet.add_sender(peer_id);

    // Serialize game discovery data and add as TLV field
    if let Ok(serialized_data) = game_discovery.serialize() {
        packet.tlv_data.push(TlvField {
            field_type: TLV_GAME_DISCOVERY,
            length: serialized_data.len() as u16,
            value: serialized_data,
        });
    }

    // Add timestamp TLV
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let timestamp_bytes = timestamp.to_le_bytes();

    packet.tlv_data.push(TlvField {
        field_type: TLV_TIMESTAMP,
        length: 8,
        value: timestamp_bytes.to_vec(),
    });

    packet
}

/// Utility function to parse game creation data from a packet
pub fn parse_game_creation_data(packet: &BitchatPacket) -> Option<GameCreationData> {
    for tlv in &packet.tlv_data {
        if tlv.field_type == TLV_GAME_CREATION {
            return GameCreationData::deserialize(&tlv.value).ok();
        }
    }
    None
}

/// Utility function to parse game discovery data from a packet
pub fn parse_game_discovery_data(packet: &BitchatPacket) -> Option<GameDiscoveryData> {
    for tlv in &packet.tlv_data {
        if tlv.field_type == TLV_GAME_DISCOVERY {
            return GameDiscoveryData::deserialize(&tlv.value).ok();
        }
    }
    None
}

/// Utility function to get timestamp from a packet
pub fn get_packet_timestamp(packet: &BitchatPacket) -> Option<u64> {
    for tlv in &packet.tlv_data {
        if tlv.field_type == TLV_TIMESTAMP && tlv.value.len() >= 8 {
            let timestamp_bytes: [u8; 8] = tlv.value[..8].try_into().ok()?;
            return Some(u64::from_le_bytes(timestamp_bytes));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::random_peer_id;

    #[test]
    fn test_create_game_packet() {
        let peer_id = random_peer_id();
        let game_id = [1u8; 16];
        let packet = create_game_packet(peer_id, game_id, 8, 1000);

        assert_eq!(packet.packet_type, PACKET_TYPE_GAME_DATA);
        assert_eq!(packet.source, peer_id);
        assert_eq!(packet.get_sender(), Some(peer_id));

        // Check if game creation data can be parsed
        let game_data = parse_game_creation_data(&packet);
        assert!(game_data.is_some());
        let game_data = game_data.unwrap();
        assert_eq!(game_data.game_id, game_id);
        assert_eq!(game_data.creator, peer_id);
        assert_eq!(game_data.max_players, 8);
        assert_eq!(game_data.buy_in, 1000);
    }

    #[test]
    fn test_create_ping_packet() {
        let peer_id = random_peer_id();
        let packet = create_ping_packet(peer_id);

        assert_eq!(packet.packet_type, PACKET_TYPE_PING);
        assert_eq!(packet.source, peer_id);
        assert_eq!(packet.get_sender(), Some(peer_id));

        // Check if timestamp is present
        let timestamp = get_packet_timestamp(&packet);
        assert!(timestamp.is_some());
    }

    #[test]
    fn test_create_discovery_packet() {
        let peer_id = random_peer_id();
        let packet = create_discovery_packet(peer_id);

        assert_eq!(packet.packet_type, PACKET_TYPE_DISCOVERY);
        assert_eq!(packet.source, peer_id);
        assert_eq!(packet.get_sender(), Some(peer_id));

        // Check if timestamp is present
        let timestamp = get_packet_timestamp(&packet);
        assert!(timestamp.is_some());
    }

    #[test]
    fn test_game_creation_data_serialization() {
        let peer_id = random_peer_id();
        let game_id = [2u8; 16];
        let game_data = GameCreationData::new(game_id, peer_id, 6, 500);

        let serialized = game_data.serialize().unwrap();
        let deserialized = GameCreationData::deserialize(&serialized).unwrap();

        assert_eq!(deserialized.game_id, game_id);
        assert_eq!(deserialized.creator, peer_id);
        assert_eq!(deserialized.max_players, 6);
        assert_eq!(deserialized.buy_in, 500);
    }

    #[test]
    fn test_game_discovery_data_serialization() {
        let game_id = [3u8; 16];
        let game_data = GameDiscoveryData::new(game_id, 2, 8, 1000, "Betting".to_string());

        let serialized = game_data.serialize().unwrap();
        let deserialized = GameDiscoveryData::deserialize(&serialized).unwrap();

        assert_eq!(deserialized.game_id, game_id);
        assert_eq!(deserialized.current_players, 2);
        assert_eq!(deserialized.max_players, 8);
        assert_eq!(deserialized.buy_in, 1000);
        assert_eq!(deserialized.phase, "Betting");
    }
}
