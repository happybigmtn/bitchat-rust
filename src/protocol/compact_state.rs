//! Bit-packed game state representation for maximal data efficiency
//!
//! This module implements ultra-compact game state serialization using:
//! - Bit-field packing for booleans and small integers
//! - Variable-length integer encoding (varint) for amounts
//! - Delta compression for state updates
//! - Memory pools for efficient allocation
//! - Zero-copy operations where possible

use bytes::{Buf, BufMut, BytesMut};
use std::collections::HashMap;

use super::craps::{CrapsGame, GamePhase};
use super::{BetType, GameId, PeerId};
use crate::error::{Error, Result};

/// Bit flags for game state
const STATE_FLAG_HAS_POINT: u8 = 0x01;
const STATE_FLAG_COME_OUT_ROLL: u8 = 0x02;
const STATE_FLAG_GAME_ENDED: u8 = 0x04;
#[allow(dead_code)]
const STATE_FLAG_HOT_STREAK: u8 = 0x08;
#[allow(dead_code)]
const STATE_FLAG_RESERVED_4: u8 = 0x10;
#[allow(dead_code)]
const STATE_FLAG_RESERVED_5: u8 = 0x20;
#[allow(dead_code)]
const STATE_FLAG_RESERVED_6: u8 = 0x40;
#[allow(dead_code)]
const STATE_FLAG_RESERVED_7: u8 = 0x80;

/// Compact representation of a bet (12 bytes total)
#[derive(Debug, Clone, PartialEq)]
pub struct CompactBet {
    /// Player ID (32 bytes -> 4 byte hash for compactness)
    pub player_hash: u32,
    /// Bet type (6 bits) + flags (2 bits)
    pub bet_type_flags: u8,
    /// Amount as varint (1-5 bytes typically)
    pub amount: u64,
    /// Timestamp delta from game start (2 bytes for ~18 hours)
    pub timestamp_delta: u16,
}

/// Bit-packed game state (target: <64 bytes for cache efficiency)
#[derive(Debug, Clone, PartialEq)]
pub struct CompactGameState {
    /// Game ID (16 bytes)
    pub game_id: GameId,
    /// State flags (1 byte)
    pub flags: u8,
    /// Current point value (0-12, 4 bits) + phase (4 bits)
    pub point_phase: u8,
    /// Number of rolls since game start (2 bytes, max 65535 rolls)
    pub roll_count: u16,
    /// Hot streak counter (1 byte, max 255)
    pub hot_streak: u8,
    /// Total number of players (1 byte)
    pub player_count: u8,
    /// Checksum for integrity (4 bytes)
    pub checksum: u32,
    /// Variable-length data follows (bets, balances, etc.)
    pub var_data_size: u16, // Size of variable data that follows
}

/// Delta for state updates (extremely compact)
#[derive(Debug, Clone)]
pub struct StateDelta {
    /// Delta type (4 bits) + flags (4 bits)
    pub delta_type_flags: u8,
    /// Sequence number for ordering
    pub sequence: u16,
    /// Variable payload based on delta type
    pub payload: DeltaPayload,
}

#[derive(Debug, Clone)]
pub enum DeltaPayload {
    /// New bet placed (player_hash + bet data)
    NewBet { player_hash: u32, bet: CompactBet },
    /// Dice roll (2 bytes: die1 + die2)
    DiceRoll { die1: u8, die2: u8 },
    /// Phase change (1 byte)
    PhaseChange(GamePhase),
    /// Balance update (player_hash + amount_delta)
    BalanceUpdate {
        player_hash: u32,
        delta: i64, // Can be negative for losses
    },
    /// Point value change (1 byte)
    PointChange(u8),
}

/// Memory pool for efficient state management
pub struct StateMemoryPool {
    /// Pre-allocated state buffers
    state_buffers: Vec<Vec<u8>>,
    /// Pre-allocated delta buffers
    delta_buffers: Vec<Vec<u8>>,
    /// Buffer size (power of 2 for efficient allocation)
    buffer_size: usize,
}

/// Varint encoding/decoding utilities
pub struct VarInt;

impl VarInt {
    /// Encode u64 as varint (1-10 bytes, typically 1-5)
    pub fn encode(value: u64, buf: &mut BytesMut) {
        let mut val = value;
        while val >= 0x80 {
            buf.put_u8((val as u8) | 0x80);
            val >>= 7;
        }
        buf.put_u8(val as u8);
    }

    /// Decode varint from buffer
    pub fn decode(buf: &mut &[u8]) -> Result<u64> {
        let mut result = 0u64;
        let mut shift = 0;

        loop {
            if buf.is_empty() {
                return Err(Error::InvalidData("Incomplete varint".to_string()));
            }

            let byte = buf.get_u8();
            result |= ((byte & 0x7F) as u64) << shift;

            if byte & 0x80 == 0 {
                break;
            }

            shift += 7;
            if shift >= 64 {
                return Err(Error::InvalidData("Varint overflow".to_string()));
            }
        }

        Ok(result)
    }

    /// Get encoded size without actually encoding
    pub fn encoded_size(value: u64) -> usize {
        if value == 0 {
            return 1;
        }

        let mut size = 0;
        let mut val = value;
        while val > 0 {
            size += 1;
            val >>= 7;
        }
        size
    }
}

impl CompactGameState {
    /// Create compact state from full game state
    pub fn from_game_state(game: &CrapsGame, _game_start_time: u64) -> Self {
        let mut flags = 0u8;

        // Set flags based on game state
        if game.point.is_some() {
            flags |= STATE_FLAG_HAS_POINT;
        }
        if game.phase == GamePhase::ComeOut {
            flags |= STATE_FLAG_COME_OUT_ROLL;
        }
        if matches!(game.phase, GamePhase::Ended | GamePhase::GameEnded) {
            flags |= STATE_FLAG_GAME_ENDED;
        }

        // Pack point and phase into single byte
        let point_val = game.point.unwrap_or(0).min(15);
        let phase_val = match game.phase {
            GamePhase::ComeOut => 0,
            GamePhase::Point => 1,
            GamePhase::Ended => 2,
            GamePhase::GameEnded => 3,
        };
        let point_phase = (point_val << 4) | phase_val;

        Self {
            game_id: game.game_id,
            flags,
            point_phase,
            roll_count: game.roll_history.len().min(65535) as u16,
            hot_streak: 0,    // Could be computed from roll history
            player_count: 0,  // Would need to be passed in
            checksum: 0,      // Computed after serialization
            var_data_size: 0, // Computed after variable data is packed
        }
    }

    /// Calculate memory footprint in bytes
    pub fn memory_footprint(&self) -> usize {
        // Fixed part: 32 bytes
        32 + self.var_data_size as usize
    }

    /// Serialize to bytes with maximum compression
    pub fn serialize(
        &mut self,
        bets: &[CompactBet],
        balances: &HashMap<u32, u64>,
    ) -> Result<Vec<u8>> {
        let mut buf = BytesMut::new();

        // Fixed header
        buf.put_slice(&self.game_id);
        buf.put_u8(self.flags);
        buf.put_u8(self.point_phase);
        buf.put_u16(self.roll_count);
        buf.put_u8(self.hot_streak);
        buf.put_u8(self.player_count);

        // Variable data section
        let var_data_start = buf.len();

        // Encode bets with varint lengths
        VarInt::encode(bets.len() as u64, &mut buf);
        for bet in bets {
            bet.serialize(&mut buf)?;
        }

        // Encode balances
        VarInt::encode(balances.len() as u64, &mut buf);
        for (&player_hash, &amount) in balances {
            buf.put_u32(player_hash);
            VarInt::encode(amount, &mut buf);
        }

        // Update variable data size
        self.var_data_size = (buf.len() - var_data_start - 4) as u16; // Subtract header without var_data_size

        // Calculate and insert checksum
        let checksum = self.calculate_checksum(&buf);
        self.checksum = checksum;

        // Insert var_data_size and checksum into fixed positions
        let mut final_buf = BytesMut::with_capacity(buf.len() + 6);
        final_buf.put_slice(&buf[..22]); // Up to player_count
        final_buf.put_u32(checksum);
        final_buf.put_u16(self.var_data_size);
        final_buf.put_slice(&buf[22..]); // Variable data

        Ok(final_buf.to_vec())
    }

    /// Deserialize from bytes
    pub fn deserialize(data: &[u8]) -> Result<(Self, Vec<CompactBet>, HashMap<u32, u64>)> {
        if data.len() < 28 {
            return Err(Error::InvalidData(
                "Insufficient data for compact state".to_string(),
            ));
        }

        let mut buf = data;

        // Fixed header
        let mut game_id = [0u8; 16];
        buf.copy_to_slice(&mut game_id);

        let flags = buf.get_u8();
        let point_phase = buf.get_u8();
        let roll_count = buf.get_u16();
        let hot_streak = buf.get_u8();
        let player_count = buf.get_u8();
        let checksum = buf.get_u32();
        let var_data_size = buf.get_u16();

        // Verify we have enough data
        if buf.len() < var_data_size as usize {
            return Err(Error::InvalidData("Insufficient variable data".to_string()));
        }

        let state = CompactGameState {
            game_id,
            flags,
            point_phase,
            roll_count,
            hot_streak,
            player_count,
            checksum,
            var_data_size,
        };

        // Verify checksum
        let computed_checksum = state.calculate_checksum(&data[..28 + var_data_size as usize]);
        if computed_checksum != checksum {
            return Err(Error::InvalidData("Checksum mismatch".to_string()));
        }

        // Decode variable data
        let bet_count = VarInt::decode(&mut buf)? as usize;
        let mut bets = Vec::with_capacity(bet_count);

        for _ in 0..bet_count {
            bets.push(CompactBet::deserialize(&mut buf)?);
        }

        let balance_count = VarInt::decode(&mut buf)? as usize;
        let mut balances = HashMap::with_capacity(balance_count);

        for _ in 0..balance_count {
            let player_hash = buf.get_u32();
            let amount = VarInt::decode(&mut buf)?;
            balances.insert(player_hash, amount);
        }

        Ok((state, bets, balances))
    }

    /// Calculate checksum for integrity verification
    fn calculate_checksum(&self, data: &[u8]) -> u32 {
        // Simple but effective checksum
        let mut checksum = 0u32;
        for (i, &byte) in data.iter().enumerate() {
            checksum = checksum.wrapping_add((byte as u32).wrapping_mul((i as u32) + 1));
        }
        checksum
    }

    /// Extract game phase from packed byte
    pub fn get_phase(&self) -> GamePhase {
        match self.point_phase & 0x0F {
            0 => GamePhase::ComeOut,
            1 => GamePhase::Point,
            2 => GamePhase::Ended,
            3 => GamePhase::GameEnded,
            _ => GamePhase::ComeOut, // Fallback
        }
    }

    /// Extract point value from packed byte
    pub fn get_point(&self) -> Option<u8> {
        if self.flags & STATE_FLAG_HAS_POINT != 0 {
            let point = (self.point_phase >> 4) & 0x0F;
            if point > 0 {
                Some(point)
            } else {
                None
            }
        } else {
            None
        }
    }
}

impl CompactBet {
    /// Create compact bet from full bet
    pub fn from_bet(bet: &super::Bet, game_start_time: u64) -> Self {
        let player_hash = Self::hash_peer_id(&bet.player);
        let bet_type_flags = bet.bet_type.to_u8() & 0x3F; // 6 bits for bet type
        let timestamp_delta =
            ((bet.timestamp.saturating_sub(game_start_time)) / 1000).min(65535) as u16;

        Self {
            player_hash,
            bet_type_flags,
            amount: bet.amount.amount(),
            timestamp_delta,
        }
    }

    /// Hash peer ID to 32-bit value for compactness
    fn hash_peer_id(peer_id: &PeerId) -> u32 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        peer_id.hash(&mut hasher);
        hasher.finish() as u32
    }

    /// Serialize compact bet
    pub fn serialize(&self, buf: &mut BytesMut) -> Result<()> {
        buf.put_u32(self.player_hash);
        buf.put_u8(self.bet_type_flags);
        VarInt::encode(self.amount, buf);
        buf.put_u16(self.timestamp_delta);
        Ok(())
    }

    /// Deserialize compact bet
    pub fn deserialize(buf: &mut &[u8]) -> Result<Self> {
        if buf.len() < 7 {
            return Err(Error::InvalidData(
                "Insufficient data for compact bet".to_string(),
            ));
        }

        let player_hash = buf.get_u32();
        let bet_type_flags = buf.get_u8();
        let amount = VarInt::decode(buf)?;
        let timestamp_delta = buf.get_u16();

        Ok(Self {
            player_hash,
            bet_type_flags,
            amount,
            timestamp_delta,
        })
    }

    /// Get bet type from packed flags
    pub fn get_bet_type(&self) -> BetType {
        // Convert back to enum, with safety bounds checking
        let bet_val = self.bet_type_flags & 0x3F;
        match bet_val {
            0 => BetType::Pass,
            1 => BetType::DontPass,
            // ... (would need full mapping)
            _ => BetType::Pass, // Safe fallback
        }
    }

    /// Calculate memory size
    pub fn memory_size(&self) -> usize {
        4 + 1 + VarInt::encoded_size(self.amount) + 2
    }
}

impl StateDelta {
    /// Create delta for new bet
    pub fn new_bet(sequence: u16, player_hash: u32, bet: CompactBet) -> Self {
        Self {
            delta_type_flags: 0x01, // Type 1: New bet
            sequence,
            payload: DeltaPayload::NewBet { player_hash, bet },
        }
    }

    /// Create delta for dice roll
    pub fn dice_roll(sequence: u16, die1: u8, die2: u8) -> Self {
        Self {
            delta_type_flags: 0x02, // Type 2: Dice roll
            sequence,
            payload: DeltaPayload::DiceRoll { die1, die2 },
        }
    }

    /// Serialize delta (ultra-compact)
    pub fn serialize(&self) -> Result<Vec<u8>> {
        let mut buf = BytesMut::new();

        buf.put_u8(self.delta_type_flags);
        buf.put_u16(self.sequence);

        match &self.payload {
            DeltaPayload::NewBet { player_hash, bet } => {
                buf.put_u32(*player_hash);
                bet.serialize(&mut buf)?;
            }
            DeltaPayload::DiceRoll { die1, die2 } => {
                buf.put_u8(*die1);
                buf.put_u8(*die2);
            }
            DeltaPayload::PhaseChange(phase) => {
                let phase_byte = match phase {
                    GamePhase::ComeOut => 0,
                    GamePhase::Point => 1,
                    GamePhase::Ended => 2,
                    GamePhase::GameEnded => 3,
                };
                buf.put_u8(phase_byte);
            }
            DeltaPayload::BalanceUpdate { player_hash, delta } => {
                buf.put_u32(*player_hash);
                // Encode signed delta as varint
                let encoded = if *delta >= 0 {
                    (*delta as u64) << 1
                } else {
                    (((-*delta) as u64) << 1) | 1
                };
                VarInt::encode(encoded, &mut buf);
            }
            DeltaPayload::PointChange(point) => {
                buf.put_u8(*point);
            }
        }

        Ok(buf.to_vec())
    }

    /// Calculate delta size
    pub fn size_bytes(&self) -> usize {
        3 + match &self.payload {
            DeltaPayload::NewBet { bet, .. } => 4 + bet.memory_size(),
            DeltaPayload::DiceRoll { .. } => 2,
            DeltaPayload::PhaseChange(_) => 1,
            DeltaPayload::BalanceUpdate { delta, .. } => {
                let encoded = if *delta >= 0 {
                    (*delta as u64) << 1
                } else {
                    (((-*delta) as u64) << 1) | 1
                };
                4 + VarInt::encoded_size(encoded)
            }
            DeltaPayload::PointChange(_) => 1,
        }
    }
}

impl StateMemoryPool {
    /// Create new memory pool
    pub fn new(initial_capacity: usize, buffer_size: usize) -> Self {
        let mut state_buffers = Vec::with_capacity(initial_capacity);
        let mut delta_buffers = Vec::with_capacity(initial_capacity);

        // Pre-allocate buffers
        for _ in 0..initial_capacity {
            state_buffers.push(vec![0u8; buffer_size]);
            delta_buffers.push(vec![0u8; buffer_size / 4]); // Deltas are typically smaller
        }

        Self {
            state_buffers,
            delta_buffers,
            buffer_size,
        }
    }

    /// Get a state buffer from the pool
    pub fn get_state_buffer(&mut self) -> Vec<u8> {
        self.state_buffers
            .pop()
            .unwrap_or_else(|| vec![0u8; self.buffer_size])
    }

    /// Return state buffer to pool
    pub fn return_state_buffer(&mut self, mut buffer: Vec<u8>) {
        if buffer.capacity() >= self.buffer_size {
            buffer.clear();
            buffer.resize(self.buffer_size, 0);
            self.state_buffers.push(buffer);
        }
    }

    /// Get delta buffer from pool
    pub fn get_delta_buffer(&mut self) -> Vec<u8> {
        self.delta_buffers
            .pop()
            .unwrap_or_else(|| vec![0u8; self.buffer_size / 4])
    }

    /// Return delta buffer to pool
    pub fn return_delta_buffer(&mut self, mut buffer: Vec<u8>) {
        if buffer.capacity() >= self.buffer_size / 4 {
            buffer.clear();
            buffer.resize(self.buffer_size / 4, 0);
            self.delta_buffers.push(buffer);
        }
    }

    /// Get pool statistics
    pub fn stats(&self) -> (usize, usize) {
        (self.state_buffers.len(), self.delta_buffers.len())
    }
}

/// State compression utilities
pub struct StateCompressor;

impl StateCompressor {
    /// Apply delta compression to a sequence of states
    pub fn compress_state_sequence(
        states: &[CompactGameState],
    ) -> Result<(CompactGameState, Vec<StateDelta>)> {
        if states.is_empty() {
            return Err(Error::InvalidData("Empty state sequence".to_string()));
        }

        let base_state = states[0].clone();
        let mut deltas = Vec::new();

        for (i, state) in states.iter().enumerate().skip(1) {
            let delta = Self::compute_delta(&states[i - 1], state, i as u16)?;
            deltas.push(delta);
        }

        Ok((base_state, deltas))
    }

    /// Compute delta between two states
    fn compute_delta(
        old_state: &CompactGameState,
        new_state: &CompactGameState,
        sequence: u16,
    ) -> Result<StateDelta> {
        // Simple example: detect phase changes
        if old_state.get_phase() != new_state.get_phase() {
            return Ok(StateDelta {
                delta_type_flags: 0x03, // Type 3: Phase change
                sequence,
                payload: DeltaPayload::PhaseChange(new_state.get_phase()),
            });
        }

        // More sophisticated delta computation would go here
        // For now, return a simple change
        Ok(StateDelta {
            delta_type_flags: 0x00, // No change
            sequence,
            payload: DeltaPayload::PointChange(0),
        })
    }

    /// Reconstruct state from base + deltas
    pub fn reconstruct_state(
        base: &CompactGameState,
        deltas: &[StateDelta],
    ) -> Result<CompactGameState> {
        let mut state = base.clone();

        for delta in deltas {
            state = Self::apply_delta(state, delta)?;
        }

        Ok(state)
    }

    /// Apply single delta to state
    fn apply_delta(mut state: CompactGameState, delta: &StateDelta) -> Result<CompactGameState> {
        match &delta.payload {
            DeltaPayload::PhaseChange(new_phase) => {
                let point_val = (state.point_phase >> 4) & 0x0F;
                let phase_val = match new_phase {
                    GamePhase::ComeOut => 0,
                    GamePhase::Point => 1,
                    GamePhase::Ended => 2,
                    GamePhase::GameEnded => 3,
                };
                state.point_phase = (point_val << 4) | phase_val;
            }
            DeltaPayload::PointChange(new_point) => {
                let phase_val = state.point_phase & 0x0F;
                state.point_phase = (new_point << 4) | phase_val;

                if *new_point > 0 {
                    state.flags |= STATE_FLAG_HAS_POINT;
                } else {
                    state.flags &= !STATE_FLAG_HAS_POINT;
                }
            }
            _ => {
                // Other delta types would be handled here
            }
        }

        Ok(state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_varint_encoding() {
        let mut buf = BytesMut::new();

        // Test small values
        VarInt::encode(0, &mut buf);
        VarInt::encode(127, &mut buf);
        VarInt::encode(128, &mut buf);
        VarInt::encode(16383, &mut buf);
        VarInt::encode(16384, &mut buf);

        let mut slice = &buf[..];
        assert_eq!(VarInt::decode(&mut slice).unwrap(), 0);
        assert_eq!(VarInt::decode(&mut slice).unwrap(), 127);
        assert_eq!(VarInt::decode(&mut slice).unwrap(), 128);
        assert_eq!(VarInt::decode(&mut slice).unwrap(), 16383);
        assert_eq!(VarInt::decode(&mut slice).unwrap(), 16384);
    }

    #[test]
    fn test_compact_state_memory_efficiency() {
        let game_id = [1u8; 16];
        let state = CompactGameState {
            game_id,
            flags: 0,
            point_phase: 0,
            roll_count: 0,
            hot_streak: 0,
            player_count: 0,
            checksum: 0,
            var_data_size: 0,
        };

        // Base state should be very compact
        assert!(state.memory_footprint() <= 64);
    }

    #[test]
    fn test_delta_compression() {
        let game_id = [1u8; 16];
        let state1 = CompactGameState {
            game_id,
            flags: 0,
            point_phase: 0, // ComeOut phase
            roll_count: 0,
            hot_streak: 0,
            player_count: 2,
            checksum: 0,
            var_data_size: 0,
        };

        let mut state2 = state1.clone();
        state2.point_phase = 1 | (4 << 4); // Point phase, point = 4
        state2.flags |= STATE_FLAG_HAS_POINT;

        let delta = StateCompressor::compute_delta(&state1, &state2, 1).unwrap();

        // Delta should be very small
        assert!(delta.size_bytes() <= 8);
    }
}
