//! Core craps game logic and state management
//! 
//! This module contains the main game state management,
//! player operations, dice rolling, and phase transitions.

use std::collections::{HashMap, HashSet};
use super::{PeerId, GameId, CrapTokens, DiceRoll, BetType, Bet};
use crate::protocol::bet_types::{GamePhase, BetResolution, BetValidator};

/// Complete craps game state with all tracking
/// 
/// Feynman: Think of this as the "casino floor manager" - it tracks
/// everything happening at the craps table: who's shooting, what phase
/// we're in, what bets are active, and the complete history.
#[derive(Clone)]
pub struct CrapsGame {
    pub game_id: GameId,
    pub phase: GamePhase,
    pub current_phase: GamePhase,  // Alias for phase for compatibility
    pub shooter: PeerId,
    pub participants: Vec<PeerId>,  // Added for compatibility
    pub point: Option<u8>,
    pub series_id: u64,
    pub roll_count: u64,
    pub roll_history: Vec<DiceRoll>,
    
    // Active bets by player and type
    pub player_bets: HashMap<PeerId, HashMap<BetType, Bet>>,
    
    // Special bet tracking
    pub fire_points: HashSet<u8>,           // Unique points made for Fire bet
    pub repeater_counts: HashMap<u8, u8>,   // Count of each number for Repeater
    pub bonus_numbers: HashSet<u8>,         // Numbers rolled for Bonus Small/Tall/All
    pub hot_roller_streak: u64,             // Consecutive pass line wins
    pub hardway_streak: HashMap<u8, u8>,    // Consecutive hardway rolls
    
    // Come/Don't Come point tracking
    pub come_points: HashMap<PeerId, HashMap<u8, CrapTokens>>,
    pub dont_come_points: HashMap<PeerId, HashMap<u8, CrapTokens>>,
}

impl CrapsGame {
    /// Create a new craps game
    pub fn new(game_id: GameId, shooter: PeerId) -> Self {
        Self {
            game_id,
            phase: GamePhase::ComeOut,
            current_phase: GamePhase::ComeOut,
            shooter,
            participants: vec![shooter],
            point: None,
            series_id: 0,
            roll_count: 0,
            roll_history: Vec::new(),
            player_bets: HashMap::new(),
            fire_points: HashSet::new(),
            repeater_counts: HashMap::new(),
            bonus_numbers: HashSet::new(),
            hot_roller_streak: 0,
            hardway_streak: HashMap::new(),
            come_points: HashMap::new(),
            dont_come_points: HashMap::new(),
        }
    }
    
    /// Add a player to the game
    pub fn add_player(&mut self, player: PeerId) -> bool {
        if !self.participants.contains(&player) {
            self.participants.push(player);
            true
        } else {
            false
        }
    }
    
    /// Generate a cryptographically secure dice roll
    pub fn roll_dice_secure() -> Result<DiceRoll, crate::error::Error> {
        use crate::crypto::GameCrypto;
        let (die1, die2) = GameCrypto::generate_secure_dice_roll();
        DiceRoll::new(die1, die2)
    }
    
    /// Generate dice roll from multiple entropy sources (for multiplayer consensus)
    pub fn roll_dice_from_sources(entropy_sources: &[[u8; 32]]) -> Result<DiceRoll, crate::error::Error> {
        use crate::crypto::GameCrypto;
        let (die1, die2) = GameCrypto::combine_randomness(entropy_sources);
        DiceRoll::new(die1, die2)
    }
    
    /// Place a bet with validation
    pub fn place_bet(&mut self, player: PeerId, bet: Bet) -> Result<(), crate::error::Error> {
        // Validate bet is appropriate for current game phase
        if !bet.bet_type.is_valid_for_phase(&self.phase) {
            return Err(crate::error::Error::InvalidBet(
                format!("Bet type {:?} not allowed in phase {:?}", bet.bet_type, self.phase)
            ));
        }
        
        // Check if player already has this bet type (prevent duplicate bets)
        if let Some(player_bets) = self.player_bets.get(&player) {
            if player_bets.contains_key(&bet.bet_type) {
                return Err(crate::error::Error::InvalidBet(
                    format!("Player already has a {:?} bet", bet.bet_type)
                ));
            }
        }
        
        // Add bet to player's bets
        self.player_bets
            .entry(player)
            .or_insert_with(HashMap::new)
            .insert(bet.bet_type.clone(), bet);
        Ok(())
    }
    
    /// Process a dice roll and return all bet resolutions
    /// 
    /// Feynman: This is the "moment of truth" - when dice land, we need to:
    /// 1. Check every active bet to see if it wins/loses/pushes
    /// 2. Update game phase (establish point, seven-out, etc.)
    /// 3. Track special bet progress (Fire points, Repeater counts)
    /// 4. Calculate exact payouts based on bet type and amount
    pub fn process_roll(&mut self, roll: DiceRoll) -> Vec<BetResolution> {
        // Import the resolution logic from the resolution module
        use crate::protocol::resolution::BetResolver;
        
        let mut resolutions = Vec::new();
        let total = roll.total();
        
        // Track roll history
        self.roll_history.push(roll);
        self.roll_count += 1;
        
        // Update special bet tracking
        self.update_special_tracking(roll);
        
        // Resolve bets based on current phase
        match self.phase {
            GamePhase::ComeOut => {
                resolutions.extend(self.resolve_comeout_roll(roll));
            },
            GamePhase::Point => {
                resolutions.extend(self.resolve_point_roll(roll));
            },
            _ => {},
        }
        
        // Always resolve one-roll bets
        resolutions.extend(self.resolve_one_roll_bets(roll));
        
        // Update game phase based on roll
        self.update_phase(total);
        
        resolutions
    }
    
    /// Update special bet tracking
    pub fn update_special_tracking(&mut self, roll: DiceRoll) {
        let total = roll.total();
        
        // Track for Bonus Small/Tall/All
        if total != 7 {
            self.bonus_numbers.insert(total);
        }
        
        // Track for Repeater bets
        *self.repeater_counts.entry(total).or_insert(0) += 1;
        
        // Track for Fire bet (unique points made)
        if self.phase == GamePhase::Point && total == self.point.unwrap() {
            self.fire_points.insert(total);
        }
        
        // Track hardway streaks
        if roll.is_hard_way() {
            *self.hardway_streak.entry(total).or_insert(0) += 1;
        } else if total == 4 || total == 6 || total == 8 || total == 10 {
            self.hardway_streak.remove(&total);
        }
    }
    
    /// Update game phase based on roll
    pub fn update_phase(&mut self, total: u8) {
        match self.phase {
            GamePhase::ComeOut => {
                match total {
                    4 | 5 | 6 | 8 | 9 | 10 => {
                        self.point = Some(total);
                        self.phase = GamePhase::Point;
                        self.current_phase = GamePhase::Point;
                    },
                    _ => {}, // Stay in come-out
                }
            },
            GamePhase::Point => {
                if total == 7 || total == self.point.unwrap() {
                    // Seven-out or point made - new series
                    self.point = None;
                    self.phase = GamePhase::ComeOut;
                    self.current_phase = GamePhase::ComeOut;
                    self.series_id += 1;
                    
                    // Reset special tracking for new series
                    if total == 7 {
                        self.fire_points.clear();
                        self.bonus_numbers.clear();
                        self.hot_roller_streak = 0;
                        
                        // Clear all Come/Don't Come points on seven-out
                        self.come_points.clear();
                        self.dont_come_points.clear();
                    } else {
                        self.hot_roller_streak += 1;
                        
                        // Remove resolved Come/Don't Come points
                        for come_points in self.come_points.values_mut() {
                            come_points.remove(&total);
                        }
                        for dont_come_points in self.dont_come_points.values_mut() {
                            dont_come_points.remove(&total);
                        }
                    }
                }
            },
            _ => {},
        }
    }
    
    /// Get the current game state summary
    pub fn get_game_state(&self) -> GameState {
        GameState {
            phase: self.phase,
            point: self.point,
            series_id: self.series_id,
            roll_count: self.roll_count,
            player_count: self.participants.len(),
            last_roll: self.roll_history.last().copied(),
        }
    }
    
    /// Check if the game is active (accepting bets and rolls)
    pub fn is_active(&self) -> bool {
        matches!(self.phase, GamePhase::ComeOut | GamePhase::Point)
    }
    
    /// Get the current shooter
    pub fn get_shooter(&self) -> PeerId {
        self.shooter
    }
    
    /// Set a new shooter (when current shooter sevens out)
    pub fn set_shooter(&mut self, new_shooter: PeerId) {
        if self.participants.contains(&new_shooter) {
            self.shooter = new_shooter;
        }
    }
    
    /// Get all active bets for a player
    pub fn get_player_bets(&self, player: &PeerId) -> Option<&HashMap<BetType, Bet>> {
        self.player_bets.get(player)
    }
    
    /// Remove a specific bet (when resolved)
    pub fn remove_bet(&mut self, player: &PeerId, bet_type: &BetType) -> Option<Bet> {
        self.player_bets
            .get_mut(player)?
            .remove(bet_type)
    }
    
    /// Clear all bets for a player
    pub fn clear_player_bets(&mut self, player: &PeerId) {
        self.player_bets.remove(player);
    }
    
    /// Get game statistics
    pub fn get_stats(&self) -> GameStats {
        GameStats {
            game_id: self.game_id,
            phase: self.phase,
            participants: self.participants.len(),
            total_rolls: self.roll_count,
            series_count: self.series_id + 1,
            active_bets: self.player_bets.values()
                .map(|bets| bets.len())
                .sum(),
            fire_points: self.fire_points.len(),
            bonus_numbers: self.bonus_numbers.len(),
        }
    }
}

/// Simplified game state for external consumers
#[derive(Debug, Clone, Copy)]
pub struct GameState {
    pub phase: GamePhase,
    pub point: Option<u8>,
    pub series_id: u64,
    pub roll_count: u64,
    pub player_count: usize,
    pub last_roll: Option<DiceRoll>,
}

/// Game statistics
#[derive(Debug, Clone)]
pub struct GameStats {
    pub game_id: GameId,
    pub phase: GamePhase,
    pub participants: usize,
    pub total_rolls: u64,
    pub series_count: u64,
    pub active_bets: usize,
    pub fire_points: usize,
    pub bonus_numbers: usize,
}

// Forward declarations for methods that will be implemented in the resolution module
impl CrapsGame {
    pub fn resolve_comeout_roll(&self, roll: DiceRoll) -> Vec<BetResolution> {
        // This will be implemented in the resolution module
        Vec::new()
    }
    
    pub fn resolve_point_roll(&mut self, roll: DiceRoll) -> Vec<BetResolution> {
        // This will be implemented in the resolution module  
        Vec::new()
    }
    
    pub fn resolve_one_roll_bets(&self, roll: DiceRoll) -> Vec<BetResolution> {
        // This will be implemented in the resolution module
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_game_creation() {
        let game_id = [1; 16];
        let shooter = [2; 32];
        let game = CrapsGame::new(game_id, shooter);
        
        assert_eq!(game.game_id, game_id);
        assert_eq!(game.shooter, shooter);
        assert_eq!(game.phase, GamePhase::ComeOut);
        assert_eq!(game.participants.len(), 1);
        assert_eq!(game.roll_count, 0);
    }
    
    #[test]
    fn test_add_player() {
        let mut game = CrapsGame::new([1; 16], [2; 32]);
        let new_player = [3; 32];
        
        assert!(game.add_player(new_player));
        assert_eq!(game.participants.len(), 2);
        
        // Adding same player again should fail
        assert!(!game.add_player(new_player));
        assert_eq!(game.participants.len(), 2);
    }
    
    #[test]
    fn test_phase_transitions() {
        let mut game = CrapsGame::new([1; 16], [2; 32]);
        
        // Start in come-out phase
        assert_eq!(game.phase, GamePhase::ComeOut);
        assert_eq!(game.point, None);
        
        // Rolling a point number should establish point
        game.update_phase(6);
        assert_eq!(game.phase, GamePhase::Point);
        assert_eq!(game.point, Some(6));
        
        // Seven-out should reset to come-out
        game.update_phase(7);
        assert_eq!(game.phase, GamePhase::ComeOut);
        assert_eq!(game.point, None);
        assert_eq!(game.series_id, 1);
    }
    
    #[test]
    fn test_game_state() {
        let game = CrapsGame::new([1; 16], [2; 32]);
        let state = game.get_game_state();
        
        assert_eq!(state.phase, GamePhase::ComeOut);
        assert_eq!(state.point, None);
        assert_eq!(state.series_id, 0);
        assert_eq!(state.roll_count, 0);
        assert_eq!(state.player_count, 1);
        assert_eq!(state.last_roll, None);
    }
}