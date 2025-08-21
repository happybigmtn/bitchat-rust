// src/protocol/constants.rs
pub const PROTOCOL_VERSION: u8 = 1;
pub const HEADER_SIZE: usize = 14;
pub const MAX_PACKET_SIZE: usize = 4096;
pub const MAX_TTL: u8 = 7;
pub const MAX_PAYLOAD_SIZE: usize = MAX_PACKET_SIZE - HEADER_SIZE;

// Packet type constants
pub const PACKET_TYPE_ANNOUNCEMENT: u8 = 0x01;
pub const PACKET_TYPE_PRIVATE_MESSAGE: u8 = 0x02;
pub const PACKET_TYPE_PUBLIC_MESSAGE: u8 = 0x03;
pub const PACKET_TYPE_HANDSHAKE_INIT: u8 = 0x04;
pub const PACKET_TYPE_HANDSHAKE_RESPONSE: u8 = 0x05;
pub const PACKET_TYPE_PING: u8 = 0x06;
pub const PACKET_TYPE_PONG: u8 = 0x07;

// Gaming packet types for BitCraps
pub const PACKET_TYPE_GAME_CREATE: u8 = 0x10;
pub const PACKET_TYPE_GAME_JOIN: u8 = 0x11;
pub const PACKET_TYPE_GAME_BET: u8 = 0x12;
pub const PACKET_TYPE_GAME_ROLL_COMMIT: u8 = 0x13;
pub const PACKET_TYPE_GAME_ROLL_REVEAL: u8 = 0x14;
pub const PACKET_TYPE_GAME_RESULT: u8 = 0x15;
pub const PACKET_TYPE_CRAP_TOKEN_TRANSFER: u8 = 0x16;
pub const PACKET_TYPE_GAME_STATE_SYNC: u8 = 0x17;

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
pub const COMMITMENT_SIZE: usize = 32; // SHA-256 hash size
pub const NONCE_SIZE: usize = 32;