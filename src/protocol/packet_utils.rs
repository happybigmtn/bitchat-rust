//! Packet creation utilities for the BitCraps gaming protocol
//!
//! This module provides convenience functions for creating common packet types
//! used in the BitCraps peer-to-peer gaming protocol.

use crate::protocol::{
    BitchatPacket, GameId, PeerId, TlvField, PACKET_TYPE_DISCOVERY, PACKET_TYPE_GAME_DATA,
    PACKET_TYPE_PING,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

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

/// Maximum allowed buy-in amount (prevents overflow attacks)
const MAX_BUY_IN: u64 = 1_000_000; // 1M CRAP tokens
/// Maximum allowed players in a game
const MAX_PLAYERS: u8 = 16;
/// Minimum allowed players in a game  
const MIN_PLAYERS: u8 = 2;

/// Validates game creation parameters
fn validate_game_creation_params(data: &GameCreationData) -> bool {
    // Validate player count
    if data.max_players < MIN_PLAYERS || data.max_players > MAX_PLAYERS {
        log::warn!("Invalid player count: {}", data.max_players);
        return false;
    }
    
    // Validate buy-in amount (prevent overflow/underflow)
    if data.buy_in == 0 || data.buy_in > MAX_BUY_IN {
        log::warn!("Invalid buy-in amount: {}", data.buy_in);
        return false;
    }
    
    // Validate timestamp (should be within reasonable range)
    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    
    // Allow timestamps within 5 minutes of current time (clock drift tolerance)
    let time_diff = if data.timestamp > current_time {
        data.timestamp - current_time
    } else {
        current_time - data.timestamp
    };
    
    if time_diff > 300 {
        log::warn!("Timestamp too far from current time: {} seconds", time_diff);
        return false;
    }
    
    true
}

/// Utility function to parse game creation data from a packet with validation
pub fn parse_game_creation_data(packet: &BitchatPacket) -> Option<GameCreationData> {
    // Verify packet has a valid sender
    let sender = packet.get_sender()?;
    
    // Check packet source matches sender TLV (prevents spoofing)
    if packet.source != sender {
        log::warn!("Packet source mismatch: source={:?}, sender={:?}", packet.source, sender);
        return None;
    }
    
    for tlv in &packet.tlv_data {
        if tlv.field_type == TLV_GAME_CREATION {
            if let Ok(game_data) = GameCreationData::deserialize(&tlv.value) {
                // Validate game parameters
                if !validate_game_creation_params(&game_data) {
                    log::warn!("Invalid game creation parameters from peer: {:?}", sender);
                    return None;
                }
                
                // Verify creator matches packet sender
                if game_data.creator != sender {
                    log::warn!("Game creator doesn't match packet sender");
                    return None;
                }
                
                return Some(game_data);
            }
        }
    }
    None
}

/// Rate limiter for discovery requests
struct DiscoveryRateLimiter {
    /// Track last request time per peer
    peer_requests: HashMap<PeerId, Instant>,
    /// Minimum time between requests per peer
    min_interval: Duration,
    /// Cache of recent discoveries to avoid re-processing
    discovery_cache: HashMap<GameId, (GameDiscoveryData, Instant)>,
    /// Cache TTL
    cache_ttl: Duration,
}

impl DiscoveryRateLimiter {
    fn new() -> Self {
        Self {
            peer_requests: HashMap::new(),
            min_interval: Duration::from_millis(100), // Max 10 requests per second per peer
            discovery_cache: HashMap::new(),
            cache_ttl: Duration::from_secs(5), // Cache discoveries for 5 seconds
        }
    }
    
    fn check_rate_limit(&mut self, peer_id: PeerId) -> bool {
        let now = Instant::now();
        
        if let Some(last_request) = self.peer_requests.get(&peer_id) {
            if now.duration_since(*last_request) < self.min_interval {
                return false; // Too many requests
            }
        }
        
        self.peer_requests.insert(peer_id, now);
        
        // Clean up old entries periodically
        if self.peer_requests.len() > 1000 {
            let cutoff = now - Duration::from_secs(60);
            self.peer_requests.retain(|_, time| *time > cutoff);
        }
        
        true
    }
    
    fn get_cached(&mut self, game_id: &GameId) -> Option<GameDiscoveryData> {
        let now = Instant::now();
        
        if let Some((data, cached_at)) = self.discovery_cache.get(game_id) {
            if now.duration_since(*cached_at) < self.cache_ttl {
                return Some(data.clone());
            }
        }
        
        // Clean up expired cache entries
        if self.discovery_cache.len() > 100 {
            let cutoff = now - self.cache_ttl;
            self.discovery_cache.retain(|_, (_, time)| *time > cutoff);
        }
        
        None
    }
    
    fn cache_discovery(&mut self, game_id: GameId, data: GameDiscoveryData) {
        self.discovery_cache.insert(game_id, (data, Instant::now()));
    }
}

// Global rate limiter instance
lazy_static::lazy_static! {
    static ref DISCOVERY_RATE_LIMITER: Mutex<DiscoveryRateLimiter> = 
        Mutex::new(DiscoveryRateLimiter::new());
}

/// Utility function to parse game discovery data from a packet with rate limiting
pub fn parse_game_discovery_data(packet: &BitchatPacket) -> Option<GameDiscoveryData> {
    // Get sender for rate limiting
    let sender = packet.get_sender()?;
    
    // Apply rate limiting
    {
        let mut limiter = DISCOVERY_RATE_LIMITER.lock().unwrap();
        if !limiter.check_rate_limit(sender) {
            log::debug!("Rate limit exceeded for peer: {:?}", sender);
            return None;
        }
    }
    
    for tlv in &packet.tlv_data {
        if tlv.field_type == TLV_GAME_DISCOVERY {
            if let Ok(discovery_data) = GameDiscoveryData::deserialize(&tlv.value) {
                // Check cache first
                {
                    let mut limiter = DISCOVERY_RATE_LIMITER.lock().unwrap();
                    if let Some(cached) = limiter.get_cached(&discovery_data.game_id) {
                        log::debug!("Returning cached discovery for game: {:?}", discovery_data.game_id);
                        return Some(cached);
                    }
                    
                    // Cache the new discovery
                    limiter.cache_discovery(discovery_data.game_id, discovery_data.clone());
                }
                
                return Some(discovery_data);
            }
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
