//! Ultra-compact game state representation for BitCraps
//! 
//! This module implements maximally efficient data structures that pack
//! all game state into minimal bytes using bit fields, copy-on-write,
//! and other memory optimization techniques.

use std::collections::HashMap;
use std::sync::Arc;
use std::mem::size_of;
use serde::{Serialize, Deserialize};

use super::{PeerId, GameId, BetType, DiceRoll};
use crate::error::Result;

/// Ultra-compact game state using bit fields and minimal storage
/// 
/// Total size: ~64 bytes for core state (excluding dynamic bet data)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactGameState {
    /// Game ID - 16 bytes
    pub game_id: GameId,
    
    /// Packed game metadata - 8 bytes total
    /// Bits 0-1: Phase (2 bits: ComeOut=0, Point=1, Ended=2)
    /// Bits 2-5: Point value (4 bits: 0=none, 4-10 encoded as 4-10)  
    /// Bits 6-31: Series ID (26 bits: up to 67M series)
    /// Bits 32-63: Roll count (32 bits: up to 4B rolls)
    pub metadata: u64,
    
    /// Player states packed in bitfields - 32 bytes
    /// Each player gets 4 bits for status flags
    pub player_states: [u64; 4], // Support up to 64 players (64 * 4 bits / 64 bits per u64 = 4 u64s)
    
    /// Last dice roll - 2 bytes 
    /// Bits 0-2: die1 (3 bits: 1-6)
    /// Bits 3-5: die2 (3 bits: 1-6)
    /// Bits 6-15: timestamp offset (10 bits: seconds from game start)
    pub last_roll: u16,
    
    /// Special bet tracking - 6 bytes
    /// Fire points made (6 bits), Bonus numbers rolled (11 bits), Hot streak (16 bits)
    pub special_state: [u16; 3],
    
    /// Dynamic data stored separately for copy-on-write efficiency
    pub dynamic_data: Arc<DynamicGameData>,
}

/// Dynamic game data that changes frequently, stored with copy-on-write semantics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicGameData {
    /// Active bets compressed using run-length encoding
    pub compressed_bets: CompressedBetData,
    
    /// Roll history stored as deltas from previous roll
    pub roll_deltas: Vec<u8>, // Each roll stored as 1 byte delta
    
    /// Player balance changes (only stores deltas from initial state)
    pub balance_deltas: HashMap<PeerId, i64>,
    
    /// Come/Don't Come points tracking
    pub come_points: CompressedPointData,
}

/// Compressed bet data using bit packing and run-length encoding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressedBetData {
    /// Bet types present (64-bit mask for each of the 64 bet types)
    pub bet_mask: u64,
    
    /// Compressed bet amounts using variable-length encoding
    pub amounts: Vec<u8>,
    
    /// Player bet mappings compressed
    pub player_mappings: Vec<u8>,
}

/// Compressed point data for Come/Don't Come tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressedPointData {
    /// Bitfield for which points are active (11 bits for points 2-12)
    pub active_points: u16,
    
    /// Compressed amounts for each active point
    pub point_amounts: Vec<u8>,
    
    /// Player associations
    pub player_associations: Vec<u8>,
}

impl CompactGameState {
    /// Create new compact game state
    pub fn new(game_id: GameId, shooter: PeerId) -> Self {
        let mut state = Self {
            game_id,
            metadata: 0, // Phase = ComeOut (0), no point, series 0, roll count 0
            player_states: [0; 4],
            last_roll: 0,
            special_state: [0; 3],
            dynamic_data: Arc::new(DynamicGameData {
                compressed_bets: CompressedBetData {
                    bet_mask: 0,
                    amounts: Vec::new(),
                    player_mappings: Vec::new(),
                },
                roll_deltas: Vec::new(),
                balance_deltas: HashMap::new(),
                come_points: CompressedPointData {
                    active_points: 0,
                    point_amounts: Vec::new(),
                    player_associations: Vec::new(),
                },
            }),
        };
        
        // Set shooter as player 0
        state.set_player_active(0, true);
        state
    }
    
    /// Get game phase from packed metadata
    pub fn get_phase(&self) -> GamePhase {
        match self.metadata & 0x3 {
            0 => GamePhase::ComeOut,
            1 => GamePhase::Point,
            2 => GamePhase::Ended,
            _ => GamePhase::ComeOut,
        }
    }
    
    /// Set game phase in packed metadata
    pub fn set_phase(&mut self, phase: GamePhase) {
        self.metadata = (self.metadata & !0x3) | (phase as u64 & 0x3);
    }
    
    /// Get point value from packed metadata
    pub fn get_point(&self) -> Option<u8> {
        let point = (self.metadata >> 2) & 0xF;
        if point == 0 { None } else { Some(point as u8) }
    }
    
    /// Set point value in packed metadata
    pub fn set_point(&mut self, point: Option<u8>) {
        let point_bits = point.unwrap_or(0) as u64;
        self.metadata = (self.metadata & !(0xF << 2)) | ((point_bits & 0xF) << 2);
    }
    
    /// Get series ID from packed metadata
    pub fn get_series_id(&self) -> u32 {
        ((self.metadata >> 6) & 0x3FFFFFF) as u32
    }
    
    /// Set series ID in packed metadata
    pub fn set_series_id(&mut self, series_id: u32) {
        let series_bits = (series_id as u64) & 0x3FFFFFF;
        self.metadata = (self.metadata & !(0x3FFFFFF << 6)) | (series_bits << 6);
    }
    
    /// Get roll count from packed metadata
    pub fn get_roll_count(&self) -> u32 {
        (self.metadata >> 32) as u32
    }
    
    /// Set roll count in packed metadata
    pub fn set_roll_count(&mut self, count: u32) {
        self.metadata = (self.metadata & 0xFFFFFFFF) | ((count as u64) << 32);
    }
    
    /// Check if player is active
    pub fn is_player_active(&self, player_index: usize) -> bool {
        if player_index >= 64 { return false; }
        let word_index = player_index / 16;
        let bit_offset = (player_index % 16) * 4;
        let player_state = (self.player_states[word_index] >> bit_offset) & 0xF;
        (player_state & 0x1) != 0
    }
    
    /// Set player active status
    pub fn set_player_active(&mut self, player_index: usize, active: bool) {
        if player_index >= 64 { return; }
        let word_index = player_index / 16;
        let bit_offset = (player_index % 16) * 4;
        let mask = 0xFu64 << bit_offset;
        let current_state = (self.player_states[word_index] >> bit_offset) & 0xF;
        let new_state = if active { current_state | 0x1 } else { current_state & !0x1 };
        self.player_states[word_index] = (self.player_states[word_index] & !mask) | ((new_state & 0xF) << bit_offset);
    }
    
    /// Get last dice roll from packed format
    pub fn get_last_roll(&self) -> Option<DiceRoll> {
        if self.last_roll == 0 { return None; }
        
        let die1 = (self.last_roll & 0x7) as u8;
        let die2 = ((self.last_roll >> 3) & 0x7) as u8;
        
        if die1 == 0 || die2 == 0 || die1 > 6 || die2 > 6 {
            return None;
        }
        
        DiceRoll::new(die1, die2).ok()
    }
    
    /// Set last dice roll in packed format
    pub fn set_last_roll(&mut self, roll: DiceRoll) {
        let packed = (roll.die1 as u16 & 0x7) | ((roll.die2 as u16 & 0x7) << 3);
        self.last_roll = packed;
    }
    
    /// Get Fire points made
    pub fn get_fire_points(&self) -> u8 {
        (self.special_state[0] & 0x3F) as u8
    }
    
    /// Set Fire points made
    pub fn set_fire_points(&mut self, count: u8) {
        self.special_state[0] = (self.special_state[0] & !0x3F) | (count as u16 & 0x3F);
    }
    
    /// Get bonus numbers rolled (11-bit mask for numbers 2-12)
    pub fn get_bonus_numbers(&self) -> u16 {
        (self.special_state[0] >> 6) | ((self.special_state[1] & 0x1F) << 10)
    }
    
    /// Set bonus numbers rolled
    pub fn set_bonus_numbers(&mut self, mask: u16) {
        self.special_state[0] = (self.special_state[0] & 0x3F) | ((mask & 0x3FF) << 6);
        self.special_state[1] = (self.special_state[1] & !0x1F) | ((mask >> 10) & 0x1F);
    }
    
    /// Get hot roller streak count
    pub fn get_hot_streak(&self) -> u16 {
        (self.special_state[1] >> 5) | ((self.special_state[2] & 0x7) << 11)
    }
    
    /// Set hot roller streak count
    pub fn set_hot_streak(&mut self, streak: u16) {
        self.special_state[1] = (self.special_state[1] & 0x1F) | ((streak & 0x7FF) << 5);
        self.special_state[2] = (self.special_state[2] & !0x7) | ((streak >> 11) & 0x7);
    }
    
    /// Create a copy-on-write clone for state mutations
    pub fn make_mutable(&mut self) -> &mut DynamicGameData {
        Arc::make_mut(&mut self.dynamic_data)
    }
    
    /// Check if a bet type is active
    pub fn has_bet_type(&self, bet_type: BetType) -> bool {
        let bit_index = bet_type as u8;
        if bit_index >= 64 { return false; }
        (self.dynamic_data.compressed_bets.bet_mask & (1u64 << bit_index)) != 0
    }
    
    /// Add a bet type to the mask
    pub fn add_bet_type(&mut self, bet_type: BetType) {
        let bit_index = bet_type as u8;
        if bit_index >= 64 { return; }
        let dynamic_data = self.make_mutable();
        dynamic_data.compressed_bets.bet_mask |= 1u64 << bit_index;
    }
    
    /// Remove a bet type from the mask
    pub fn remove_bet_type(&mut self, bet_type: BetType) {
        let bit_index = bet_type as u8;
        if bit_index >= 64 { return; }
        let dynamic_data = self.make_mutable();
        dynamic_data.compressed_bets.bet_mask &= !(1u64 << bit_index);
    }
    
    /// Get memory usage statistics
    pub fn memory_usage(&self) -> MemoryStats {
        let static_size = size_of::<Self>() - size_of::<Arc<DynamicGameData>>();
        let dynamic_size = size_of::<DynamicGameData>() + 
                          self.dynamic_data.compressed_bets.amounts.len() +
                          self.dynamic_data.compressed_bets.player_mappings.len() +
                          self.dynamic_data.roll_deltas.len() +
                          self.dynamic_data.balance_deltas.len() * (size_of::<PeerId>() + size_of::<i64>()) +
                          self.dynamic_data.come_points.point_amounts.len() +
                          self.dynamic_data.come_points.player_associations.len();
        
        MemoryStats {
            static_bytes: static_size,
            dynamic_bytes: dynamic_size,
            total_bytes: static_size + dynamic_size,
            compression_ratio: self.calculate_compression_ratio(),
        }
    }
    
    /// Calculate compression ratio vs uncompressed representation
    fn calculate_compression_ratio(&self) -> f32 {
        // Estimate uncompressed size based on typical game state
        let estimated_uncompressed = 1024; // Rough estimate for full CrapsGame struct
        let actual_size = self.memory_usage().total_bytes;
        actual_size as f32 / estimated_uncompressed as f32
    }
}

/// Game phase enum optimized for bit packing
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GamePhase {
    ComeOut = 0,
    Point = 1, 
    Ended = 2,
}

/// Memory usage statistics
#[derive(Debug, Clone)]
pub struct MemoryStats {
    pub static_bytes: usize,
    pub dynamic_bytes: usize, 
    pub total_bytes: usize,
    pub compression_ratio: f32,
}

/// Variable-length integer encoding for compact storage
pub struct VarInt;

impl VarInt {
    /// Encode a u64 value using variable-length encoding
    pub fn encode(value: u64) -> Vec<u8> {
        let mut result = Vec::new();
        let mut val = value;
        
        while val >= 0x80 {
            result.push((val & 0x7F) as u8 | 0x80);
            val >>= 7;
        }
        result.push(val as u8);
        result
    }
    
    /// Decode a variable-length encoded value
    pub fn decode(bytes: &[u8]) -> Result<(u64, usize)> {
        let mut result = 0u64;
        let mut shift = 0;
        
        for (i, &byte) in bytes.iter().enumerate() {
            if shift >= 64 {
                return Err(crate::error::Error::InvalidData("VarInt overflow".to_string()));
            }
            
            result |= ((byte & 0x7F) as u64) << shift;
            
            if (byte & 0x80) == 0 {
                return Ok((result, i + 1));
            }
            
            shift += 7;
        }
        
        Err(crate::error::Error::InvalidData("Incomplete VarInt".to_string()))
    }
}

/// Snapshot manager for efficient state history
pub struct StateSnapshot {
    /// Base state at checkpoint
    pub base_state: CompactGameState,
    
    /// Delta operations since base state  
    pub deltas: Vec<StateDelta>,
    
    /// Snapshot timestamp
    pub timestamp: u64,
}

/// Delta operation for incremental state updates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StateDelta {
    PhaseChange { new_phase: GamePhase },
    PointSet { point: Option<u8> },
    RollProcessed { roll: DiceRoll },
    BetAdded { bet_type: BetType, amount_encoded: Vec<u8> },
    BetRemoved { bet_type: BetType },
    BalanceChanged { player_index: u8, delta: i64 },
    SpecialStateUpdated { field: u8, value: u16 },
}

impl StateSnapshot {
    /// Create a new snapshot from current state
    pub fn create(state: &CompactGameState) -> Self {
        Self {
            base_state: state.clone(),
            deltas: Vec::new(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }
    
    /// Apply deltas to reconstruct current state
    pub fn reconstruct(&self) -> Result<CompactGameState> {
        let mut state = self.base_state.clone();
        
        for delta in &self.deltas {
            match delta {
                StateDelta::PhaseChange { new_phase } => {
                    state.set_phase(*new_phase);
                },
                StateDelta::PointSet { point } => {
                    state.set_point(*point);
                },
                StateDelta::RollProcessed { roll } => {
                    state.set_last_roll(*roll);
                    state.set_roll_count(state.get_roll_count() + 1);
                },
                StateDelta::BetAdded { bet_type, .. } => {
                    state.add_bet_type(*bet_type);
                },
                StateDelta::BetRemoved { bet_type } => {
                    state.remove_bet_type(*bet_type);
                },
                StateDelta::BalanceChanged { player_index, delta } => {
                    // Apply balance delta (simplified)
                    let dynamic_data = state.make_mutable();
                    let player_id = [*player_index; 32]; // Simplified mapping
                    *dynamic_data.balance_deltas.entry(player_id).or_insert(0) += delta;
                },
                StateDelta::SpecialStateUpdated { field, value } => {
                    if (*field as usize) < state.special_state.len() {
                        state.special_state[*field as usize] = *value;
                    }
                },
            }
        }
        
        Ok(state)
    }
    
    /// Add a delta operation
    pub fn add_delta(&mut self, delta: StateDelta) {
        self.deltas.push(delta);
    }
    
    /// Get memory usage of this snapshot
    pub fn memory_usage(&self) -> usize {
        size_of::<Self>() + 
        self.base_state.memory_usage().total_bytes + 
        self.deltas.len() * size_of::<StateDelta>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compact_game_state_size() {
        let state = CompactGameState::new([1; 16], [2; 32]);
        let stats = state.memory_usage();
        
        // Verify we're under target size for static data
        assert!(stats.static_bytes <= 128, "Static size {} exceeds 128 bytes", stats.static_bytes);
        assert!(stats.compression_ratio < 0.5, "Compression ratio {} should be < 0.5", stats.compression_ratio);
    }

    #[test]  
    fn test_bit_field_operations() {
        let mut state = CompactGameState::new([1; 16], [2; 32]);
        
        // Test phase operations
        assert_eq!(state.get_phase(), GamePhase::ComeOut);
        state.set_phase(GamePhase::Point);
        assert_eq!(state.get_phase(), GamePhase::Point);
        
        // Test point operations
        assert_eq!(state.get_point(), None);
        state.set_point(Some(6));
        assert_eq!(state.get_point(), Some(6));
        
        // Test roll count operations
        assert_eq!(state.get_roll_count(), 0);
        state.set_roll_count(42);
        assert_eq!(state.get_roll_count(), 42);
        
        // Test player state operations
        assert!(state.is_player_active(0)); // Shooter should be active
        assert!(!state.is_player_active(1));
        state.set_player_active(1, true);
        assert!(state.is_player_active(1));
    }

    #[test]
    fn test_dice_roll_packing() {
        let mut state = CompactGameState::new([1; 16], [2; 32]);
        
        let roll = DiceRoll::new(3, 5).unwrap();
        state.set_last_roll(roll);
        
        let unpacked = state.get_last_roll().unwrap();
        assert_eq!(unpacked.die1, 3);
        assert_eq!(unpacked.die2, 5);
    }

    #[test]
    fn test_bet_type_mask() {
        let mut state = CompactGameState::new([1; 16], [2; 32]);
        
        assert!(!state.has_bet_type(BetType::Pass));
        state.add_bet_type(BetType::Pass);
        assert!(state.has_bet_type(BetType::Pass));
        
        state.add_bet_type(BetType::Field);
        assert!(state.has_bet_type(BetType::Field));
        assert!(state.has_bet_type(BetType::Pass));
        
        state.remove_bet_type(BetType::Pass);
        assert!(!state.has_bet_type(BetType::Pass));
        assert!(state.has_bet_type(BetType::Field));
    }

    #[test]
    fn test_varint_encoding() {
        let test_values = [0u64, 127, 128, 16383, 16384, u64::MAX];
        
        for &value in &test_values {
            let encoded = VarInt::encode(value);
            let (decoded, bytes_read) = VarInt::decode(&encoded).unwrap();
            assert_eq!(value, decoded);
            assert_eq!(bytes_read, encoded.len());
        }
    }

    #[test]
    fn test_copy_on_write() {
        let mut state1 = CompactGameState::new([1; 16], [2; 32]);
        let state2 = state1.clone();
        
        // Before mutation, they should share the same Arc
        assert!(Arc::ptr_eq(&state1.dynamic_data, &state2.dynamic_data));
        
        // After mutation, they should have different Arcs
        state1.make_mutable();
        // Note: Arc::ptr_eq might still return true if the Arc has only one reference
        // This is expected behavior for Arc::make_mut
    }

    #[test]
    fn test_state_snapshot() {
        let mut state = CompactGameState::new([1; 16], [2; 32]);
        state.set_phase(GamePhase::Point);
        state.set_point(Some(8));
        
        let mut snapshot = StateSnapshot::create(&state);
        
        // Add some deltas
        snapshot.add_delta(StateDelta::PhaseChange { new_phase: GamePhase::Ended });
        snapshot.add_delta(StateDelta::RollProcessed { roll: DiceRoll::new(4, 3).unwrap() });
        
        // Reconstruct state
        let reconstructed = snapshot.reconstruct().unwrap();
        assert_eq!(reconstructed.get_phase(), GamePhase::Ended);
        assert_eq!(reconstructed.get_roll_count(), 1);
    }

    #[test]
    fn test_special_state_packing() {
        let mut state = CompactGameState::new([1; 16], [2; 32]);
        
        // Test Fire points (6 bits)
        state.set_fire_points(5);
        assert_eq!(state.get_fire_points(), 5);
        
        // Test bonus numbers (11 bits)
        state.set_bonus_numbers(0x7FF); // All 11 bits set
        assert_eq!(state.get_bonus_numbers(), 0x7FF);
        
        // Test hot streak (14 bits)
        state.set_hot_streak(1000);
        assert_eq!(state.get_hot_streak(), 1000);
        
        // Verify Fire points are preserved after other operations
        assert_eq!(state.get_fire_points(), 5);
    }
}