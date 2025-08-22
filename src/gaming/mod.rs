//! Gaming subsystem for BitCraps
//! 
//! This module implements the core craps game logic including:
//! - Craps game rules and mechanics
//! - Bet types and payouts
//! - Game state management
//! - Multi-player game coordination
//! - Treasury participation
//! - Game runtime orchestration

pub mod runtime;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};

use crate::protocol::{GameId, PeerId, BetType, Bet, DiceRoll, CrapTokens};
use crate::crypto::GameCrypto;
use crate::error::{Error, Result};

/// The treasury address - a special peer that provides liquidity
pub const TREASURY_ADDRESS: PeerId = [0xFFu8; 32];

/// Main craps game implementation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrapsGame {
    pub game_id: GameId,
    pub creator: PeerId,
    pub participants: Vec<PeerId>,
    pub current_phase: GamePhase,
    pub point: Option<u8>,
    pub roll_count: u32,
    pub roll_history: Vec<DiceRoll>,
    pub active_bets: HashMap<PeerId, HashMap<BetType, Bet>>,
    pub created_at: u64,
    pub buy_in: CrapTokens,
    pub max_players: u8,
}

/// Phases of a craps game
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GamePhase {
    WaitingForPlayers,
    ComeOutRoll,
    PointRoll(u8), // The point that was established
    GameEnded,
}

/// Result of bet resolution
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BetResolution {
    Won { player: PeerId, bet: Bet, payout: u64 },
    Lost { player: PeerId, bet: Bet },
    Push { player: PeerId, bet: Bet }, // Tie/no action
    Active { player: PeerId, bet: Bet }, // Bet remains active
}

/// Game events for event sourcing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GameEvent {
    GameCreated { game_id: GameId, creator: PeerId, buy_in: CrapTokens },
    PlayerJoined { game_id: GameId, player: PeerId },
    BetPlaced { game_id: GameId, bet: Bet },
    DiceRolled { game_id: GameId, roll: DiceRoll },
    BetResolved { game_id: GameId, resolution: BetResolution },
    PointEstablished { game_id: GameId, point: u8 },
    GameEnded { game_id: GameId, reason: String },
}

impl CrapsGame {
    /// Create a new craps game
    pub fn new(game_id: GameId, creator: PeerId) -> Self {
        Self {
            game_id,
            creator,
            participants: vec![creator],
            current_phase: GamePhase::WaitingForPlayers,
            point: None,
            roll_count: 0,
            roll_history: Vec::new(),
            active_bets: HashMap::new(),
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            buy_in: CrapTokens::from_crap(1.0), // Default 1 CRAP buy-in
            max_players: 8,
        }
    }
    
    /// Add a player to the game
    pub fn add_player(&mut self, player: PeerId) -> Result<()> {
        if self.participants.len() >= self.max_players as usize {
            return Err(Error::Protocol("Game is full".to_string()));
        }
        
        if self.participants.contains(&player) {
            return Err(Error::Protocol("Player already in game".to_string()));
        }
        
        self.participants.push(player);
        
        // Start game if we have enough players and treasury
        if self.participants.len() >= 2 && self.participants.contains(&TREASURY_ADDRESS) {
            self.current_phase = GamePhase::ComeOutRoll;
        }
        
        Ok(())
    }
    
    /// Place a bet in the game
    pub fn place_bet(&mut self, bet: Bet) -> Result<()> {
        // Validate bet
        if bet.game_id != self.game_id {
            return Err(Error::Protocol("Bet for wrong game".to_string()));
        }
        
        if !self.participants.contains(&bet.player) {
            return Err(Error::Protocol("Player not in game".to_string()));
        }
        
        // Check if bet type is valid for current phase
        match self.current_phase {
            GamePhase::WaitingForPlayers => {
                return Err(Error::Protocol("Game not started".to_string()));
            }
            GamePhase::GameEnded => {
                return Err(Error::Protocol("Game has ended".to_string()));
            }
            _ => {}
        }
        
        // Store the bet
        self.active_bets
            .entry(bet.player)
            .or_insert_with(HashMap::new)
            .insert(bet.bet_type.clone(), bet);
        
        Ok(())
    }
    
    /// Process a dice roll and update game state
    pub fn process_roll(&mut self, roll: DiceRoll) -> Vec<BetResolution> {
        self.roll_count += 1;
        self.roll_history.push(roll);
        
        let mut resolutions = Vec::new();
        
        match self.current_phase {
            GamePhase::ComeOutRoll => {
                resolutions.extend(self.resolve_come_out_roll(roll));
            }
            GamePhase::PointRoll(point) => {
                resolutions.extend(self.resolve_point_roll(roll, point));
            }
            _ => {}
        }
        
        resolutions
    }
    
    /// Resolve bets for come-out roll
    fn resolve_come_out_roll(&mut self, roll: DiceRoll) -> Vec<BetResolution> {
        let mut resolutions = Vec::new();
        
        for (player, bets) in &self.active_bets {
            for (bet_type, bet) in bets {
                match bet_type {
                    BetType::Pass => {
                        if roll.is_natural() {
                            // Pass line wins on 7 or 11
                            resolutions.push(BetResolution::Won {
                                player: *player,
                                bet: bet.clone(),
                                payout: bet.amount.amount() * 2, // 1:1 odds
                            });
                        } else if roll.is_craps() {
                            // Pass line loses on 2, 3, 12
                            resolutions.push(BetResolution::Lost {
                                player: *player,
                                bet: bet.clone(),
                            });
                        } else {
                            // Point established - bet remains active
                            resolutions.push(BetResolution::Active {
                                player: *player,
                                bet: bet.clone(),
                            });
                        }
                    }
                    BetType::DontPass => {
                        if roll.is_natural() {
                            // Don't pass loses on 7 or 11
                            resolutions.push(BetResolution::Lost {
                                player: *player,
                                bet: bet.clone(),
                            });
                        } else if roll.total() == 2 || roll.total() == 3 {
                            // Don't pass wins on 2 or 3
                            resolutions.push(BetResolution::Won {
                                player: *player,
                                bet: bet.clone(),
                                payout: bet.amount.amount() * 2,
                            });
                        } else if roll.total() == 12 {
                            // Don't pass pushes on 12
                            resolutions.push(BetResolution::Push {
                                player: *player,
                                bet: bet.clone(),
                            });
                        } else {
                            // Point established - bet remains active
                            resolutions.push(BetResolution::Active {
                                player: *player,
                                bet: bet.clone(),
                            });
                        }
                    }
                    BetType::Field => {
                        if matches!(roll.total(), 3 | 4 | 9 | 10 | 11) {
                            // Field wins 1:1 on 3,4,9,10,11
                            resolutions.push(BetResolution::Won {
                                player: *player,
                                bet: bet.clone(),
                                payout: bet.amount.amount() * 2,
                            });
                        } else if roll.total() == 2 || roll.total() == 12 {
                            // Field wins 2:1 on 2,12
                            resolutions.push(BetResolution::Won {
                                player: *player,
                                bet: bet.clone(),
                                payout: bet.amount.amount() * 3,
                            });
                        } else {
                            // Field loses on 5,6,7,8
                            resolutions.push(BetResolution::Lost {
                                player: *player,
                                bet: bet.clone(),
                            });
                        }
                    }
                    BetType::Next7 => {
                        if roll.total() == 7 {
                            // Any 7 wins 4:1
                            resolutions.push(BetResolution::Won {
                                player: *player,
                                bet: bet.clone(),
                                payout: bet.amount.amount() * 5,
                            });
                        } else {
                            resolutions.push(BetResolution::Lost {
                                player: *player,
                                bet: bet.clone(),
                            });
                        }
                    }
                    BetType::Next2 | BetType::Next3 | BetType::Next12 => {
                        // These handle the "craps" numbers individually
                        let target = match bet_type {
                            BetType::Next2 => 2,
                            BetType::Next3 => 3,
                            BetType::Next12 => 12,
                            _ => 0,
                        };
                        if roll.total() == target {
                            // Craps number wins 30:1
                            resolutions.push(BetResolution::Won {
                                player: *player,
                                bet: bet.clone(),
                                payout: bet.amount.amount() * 31,
                            });
                        } else {
                            resolutions.push(BetResolution::Lost {
                                player: *player,
                                bet: bet.clone(),
                            });
                        }
                    }
                    _ => {
                        // Other bets remain active
                        resolutions.push(BetResolution::Active {
                            player: *player,
                            bet: bet.clone(),
                        });
                    }
                }
            }
        }
        
        // Update game phase
        if matches!(roll.total(), 4 | 5 | 6 | 8 | 9 | 10) {
            self.current_phase = GamePhase::PointRoll(roll.total());
            self.point = Some(roll.total());
        }
        
        resolutions
    }
    
    /// Resolve bets for point roll
    fn resolve_point_roll(&mut self, roll: DiceRoll, point: u8) -> Vec<BetResolution> {
        let mut resolutions = Vec::new();
        
        for (player, bets) in &self.active_bets {
            for (bet_type, bet) in bets {
                match bet_type {
                    BetType::Pass => {
                        if roll.total() == point {
                            // Pass line wins when point is made
                            resolutions.push(BetResolution::Won {
                                player: *player,
                                bet: bet.clone(),
                                payout: bet.amount.amount() * 2,
                            });
                        } else if roll.total() == 7 {
                            // Pass line loses on seven-out
                            resolutions.push(BetResolution::Lost {
                                player: *player,
                                bet: bet.clone(),
                            });
                        } else {
                            // Bet remains active
                            resolutions.push(BetResolution::Active {
                                player: *player,
                                bet: bet.clone(),
                            });
                        }
                    }
                    BetType::DontPass => {
                        if roll.total() == 7 {
                            // Don't pass wins on seven-out
                            resolutions.push(BetResolution::Won {
                                player: *player,
                                bet: bet.clone(),
                                payout: bet.amount.amount() * 2,
                            });
                        } else if roll.total() == point {
                            // Don't pass loses when point is made
                            resolutions.push(BetResolution::Lost {
                                player: *player,
                                bet: bet.clone(),
                            });
                        } else {
                            // Bet remains active
                            resolutions.push(BetResolution::Active {
                                player: *player,
                                bet: bet.clone(),
                            });
                        }
                    }
                    // Place bets handled via specific number bets
                    BetType::Yes4 | BetType::Yes5 | BetType::Yes6 | BetType::Yes8 | BetType::Yes9 | BetType::Yes10 => {
                        let target_number = match bet_type {
                            BetType::Yes4 => 4,
                            BetType::Yes5 => 5,
                            BetType::Yes6 => 6,
                            BetType::Yes8 => 8,
                            BetType::Yes9 => 9,
                            BetType::Yes10 => 10,
                            _ => 0, // Should not happen
                        };
                        
                        if roll.total() == target_number {
                            // Yes bet wins
                            let payout_multiplier = match target_number {
                                4 | 10 => 18, // 9:5 odds = 1.8x + original bet
                                5 | 9 => 14,  // 7:5 odds = 1.4x + original bet
                                6 | 8 => 12,  // 7:6 odds = 1.167x + original bet (rounded)
                                _ => 10, // Should not happen
                            };
                            
                            resolutions.push(BetResolution::Won {
                                player: *player,
                                bet: bet.clone(),
                                payout: (bet.amount.amount() * payout_multiplier) / 10,
                            });
                        } else if roll.total() == 7 {
                            // Yes bet loses on seven-out
                            resolutions.push(BetResolution::Lost {
                                player: *player,
                                bet: bet.clone(),
                            });
                        } else {
                            // Bet remains active
                            resolutions.push(BetResolution::Active {
                                player: *player,
                                bet: bet.clone(),
                            });
                        }
                    }
                    _ => {
                        // Handle other bet types (field, hardways, etc.)
                        // Most one-roll bets are resolved every roll
                        match bet_type {
                            BetType::Field => {
                                if matches!(roll.total(), 3 | 4 | 9 | 10 | 11) {
                                    resolutions.push(BetResolution::Won {
                                        player: *player,
                                        bet: bet.clone(),
                                        payout: bet.amount.amount() * 2,
                                    });
                                } else if roll.total() == 2 || roll.total() == 12 {
                                    resolutions.push(BetResolution::Won {
                                        player: *player,
                                        bet: bet.clone(),
                                        payout: bet.amount.amount() * 3,
                                    });
                                } else {
                                    resolutions.push(BetResolution::Lost {
                                        player: *player,
                                        bet: bet.clone(),
                                    });
                                }
                            }
                            BetType::Next7 => {
                                if roll.total() == 7 {
                                    resolutions.push(BetResolution::Won {
                                        player: *player,
                                        bet: bet.clone(),
                                        payout: bet.amount.amount() * 5,
                                    });
                                } else {
                                    resolutions.push(BetResolution::Lost {
                                        player: *player,
                                        bet: bet.clone(),
                                    });
                                }
                            }
                            _ => {
                                resolutions.push(BetResolution::Active {
                                    player: *player,
                                    bet: bet.clone(),
                                });
                            }
                        }
                    }
                }
            }
        }
        
        // Check if game should end
        if roll.total() == 7 || (self.point.is_some() && roll.total() == self.point.unwrap()) {
            self.current_phase = GamePhase::ComeOutRoll;
            self.point = None;
        }
        
        resolutions
    }
    
    /// Resolve all bets for a given roll
    pub fn resolve_all_bets(
        &self,
        roll: DiceRoll,
        bets: &HashMap<PeerId, HashMap<BetType, Bet>>,
    ) -> Vec<BetResolution> {
        let mut resolutions = Vec::new();
        
        // This is a simplified resolution - in practice would need to handle
        // all the complex craps betting rules
        for (player, player_bets) in bets {
            for (bet_type, bet) in player_bets {
                // Simplified resolution logic
                match bet_type {
                    BetType::Pass => {
                        if self.current_phase == GamePhase::ComeOutRoll {
                            if roll.is_natural() {
                                resolutions.push(BetResolution::Won {
                                    player: *player,
                                    bet: bet.clone(),
                                    payout: bet.amount.amount() * 2,
                                });
                            } else if roll.is_craps() {
                                resolutions.push(BetResolution::Lost {
                                    player: *player,
                                    bet: bet.clone(),
                                });
                            }
                        }
                    }
                    BetType::Field => {
                        if matches!(roll.total(), 3 | 4 | 9 | 10 | 11) {
                            resolutions.push(BetResolution::Won {
                                player: *player,
                                bet: bet.clone(),
                                payout: bet.amount.amount() * 2,
                            });
                        } else if roll.total() == 2 || roll.total() == 12 {
                            resolutions.push(BetResolution::Won {
                                player: *player,
                                bet: bet.clone(),
                                payout: bet.amount.amount() * 3,
                            });
                        } else {
                            resolutions.push(BetResolution::Lost {
                                player: *player,
                                bet: bet.clone(),
                            });
                        }
                    }
                    _ => {
                        // Other bets - simplified handling
                        resolutions.push(BetResolution::Active {
                            player: *player,
                            bet: bet.clone(),
                        });
                    }
                }
            }
        }
        
        resolutions
    }
    
    /// Get game statistics
    pub fn get_stats(&self) -> GameStats {
        let total_bets = self.active_bets.values()
            .map(|player_bets| player_bets.len())
            .sum();
        
        let total_wagered = self.active_bets.values()
            .flat_map(|player_bets| player_bets.values())
            .map(|bet| bet.amount.amount())
            .sum();
        
        GameStats {
            game_id: self.game_id,
            phase: self.current_phase.clone(),
            player_count: self.participants.len(),
            roll_count: self.roll_count,
            total_bets,
            total_wagered,
            point: self.point,
        }
    }
}

/// Game statistics
#[derive(Debug, Clone)]
pub struct GameStats {
    pub game_id: GameId,
    pub phase: GamePhase,
    pub player_count: usize,
    pub roll_count: u32,
    pub total_bets: usize,
    pub total_wagered: u64,
    pub point: Option<u8>,
}

/// Treasury participant that automatically provides liquidity
pub struct TreasuryParticipant {
    balance: Arc<RwLock<u64>>, // CRAP token balance
    game_participation: Arc<RwLock<HashMap<GameId, TreasuryPosition>>>,
    strategy: TreasuryStrategy,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct TreasuryPosition {
    game_id: GameId,
    total_exposure: u64,
    bets_placed: HashMap<BetType, u64>,
    profit_loss: i64,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct TreasuryStrategy {
    max_exposure_per_game: u64,
    preferred_bet_types: Vec<BetType>,
    risk_tolerance: f64,
}

impl Default for TreasuryStrategy {
    fn default() -> Self {
        Self {
            max_exposure_per_game: CrapTokens::from_crap(100.0).amount(), // 100 CRAP max per game
            preferred_bet_types: vec![
                BetType::DontPass, // House edge bets
                BetType::Field,
            ],
            risk_tolerance: 0.3, // Conservative
        }
    }
}

impl TreasuryParticipant {
    pub fn new(initial_balance: u64) -> Self {
        Self {
            balance: Arc::new(RwLock::new(initial_balance)),
            game_participation: Arc::new(RwLock::new(HashMap::new())),
            strategy: TreasuryStrategy::default(),
        }
    }
    
    /// Automatically join a game and provide liquidity
    pub async fn auto_join_game(&self, game_id: GameId) -> Result<()> {
        let position = TreasuryPosition {
            game_id,
            total_exposure: 0,
            bets_placed: HashMap::new(),
            profit_loss: 0,
        };
        
        self.game_participation.write().await.insert(game_id, position);
        
        log::info!("Treasury joined game: {:?}", game_id);
        Ok(())
    }
    
    /// React to player bets by placing counter-bets
    pub async fn handle_player_bet(
        &self,
        game_id: GameId,
        _player: PeerId,
        bet: Bet,
    ) -> Result<Vec<Bet>> {
        let mut positions = self.game_participation.write().await;
        let position = positions.get_mut(&game_id)
            .ok_or_else(|| Error::Protocol("Treasury not in game".to_string()))?;
        
        let mut counter_bets = Vec::new();
        
        // Treasury strategy: place opposing bets to provide liquidity
        match bet.bet_type {
            BetType::Pass => {
                // Counter with don't pass
                if position.total_exposure + bet.amount.amount() <= self.strategy.max_exposure_per_game {
                    let counter_bet = Bet {
                        id: GameCrypto::generate_random_bytes(16).try_into().unwrap(),
                        game_id,
                        player: TREASURY_ADDRESS,
                        bet_type: BetType::DontPass,
                        amount: bet.amount,
                        timestamp: SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                    };
                    
                    position.total_exposure += bet.amount.amount();
                    position.bets_placed.insert(BetType::DontPass, bet.amount.amount());
                    counter_bets.push(counter_bet);
                }
            }
            BetType::DontPass => {
                // Counter with pass
                if position.total_exposure + bet.amount.amount() <= self.strategy.max_exposure_per_game {
                    let counter_bet = Bet {
                        id: GameCrypto::generate_random_bytes(16).try_into().unwrap(),
                        game_id,
                        player: TREASURY_ADDRESS,
                        bet_type: BetType::Pass,
                        amount: bet.amount,
                        timestamp: SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                    };
                    
                    position.total_exposure += bet.amount.amount();
                    position.bets_placed.insert(BetType::Pass, bet.amount.amount());
                    counter_bets.push(counter_bet);
                }
            }
            _ => {
                // For other bets, treasury might choose not to participate
                // or place small counter-bets depending on strategy
            }
        }
        
        log::info!("Treasury placed {} counter-bets for game {:?}", counter_bets.len(), game_id);
        Ok(counter_bets)
    }
    
    /// Process game result and update treasury balance
    pub async fn process_game_result(
        &self,
        game_id: GameId,
        _roll: DiceRoll,
        winners: Vec<(PeerId, u64)>,
    ) -> Result<()> {
        let mut positions = self.game_participation.write().await;
        let position = positions.get_mut(&game_id)
            .ok_or_else(|| Error::Protocol("Treasury not in game".to_string()))?;
        
        // Calculate treasury's profit/loss
        let mut _treasury_payout = 0u64;
        
        // In a real implementation, would calculate based on bet resolutions
        // For now, simplified calculation
        
        // Update treasury balance
        let mut balance = self.balance.write().await;
        let total_payouts: u64 = winners.iter().map(|(_, amount)| amount).sum();
        
        if total_payouts > position.total_exposure {
            // Treasury lost money
            let loss = total_payouts - position.total_exposure;
            *balance = balance.saturating_sub(loss);
            position.profit_loss -= loss as i64;
        } else {
            // Treasury made money
            let profit = position.total_exposure - total_payouts;
            *balance += profit;
            position.profit_loss += profit as i64;
        }
        
        log::info!("Treasury processed game result for {:?}: P&L = {}", 
                  game_id, position.profit_loss);
        
        Ok(())
    }
    
    /// Get treasury balance
    pub async fn get_balance(&self) -> u64 {
        *self.balance.read().await
    }
    
    /// Get treasury statistics
    pub async fn get_stats(&self) -> TreasuryStats {
        let balance = *self.balance.read().await;
        let positions = self.game_participation.read().await;
        
        let active_games = positions.len();
        let total_exposure: u64 = positions.values().map(|p| p.total_exposure).sum();
        let total_pnl: i64 = positions.values().map(|p| p.profit_loss).sum();
        
        TreasuryStats {
            balance,
            active_games,
            total_exposure,
            total_profit_loss: total_pnl,
        }
    }
}

/// Treasury statistics
#[derive(Debug, Clone)]
pub struct TreasuryStats {
    pub balance: u64,
    pub active_games: usize,
    pub total_exposure: u64,
    pub total_profit_loss: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_craps_game_creation() {
        let game_id = [1u8; 16];
        let creator = [2u8; 32];
        let game = CrapsGame::new(game_id, creator);
        
        assert_eq!(game.game_id, game_id);
        assert_eq!(game.creator, creator);
        assert_eq!(game.current_phase, GamePhase::WaitingForPlayers);
        assert_eq!(game.participants.len(), 1);
    }
    
    #[test]
    fn test_bet_placement() {
        let mut game = CrapsGame::new([1u8; 16], [2u8; 32]);
        game.current_phase = GamePhase::ComeOutRoll;
        
        let bet = Bet {
            id: [3u8; 16],
            game_id: game.game_id,
            player: game.creator,
            bet_type: BetType::Pass,
            amount: CrapTokens::from_crap(5.0),
            timestamp: 12345,
        };
        
        assert!(game.place_bet(bet).is_ok());
        assert_eq!(game.active_bets.len(), 1);
    }
    
    #[test]
    fn test_dice_roll_processing() {
        let mut game = CrapsGame::new([1u8; 16], [2u8; 32]);
        game.current_phase = GamePhase::ComeOutRoll;
        
        // Place a pass line bet
        let bet = Bet {
            id: [3u8; 16],
            game_id: game.game_id,
            player: game.creator,
            bet_type: BetType::Pass,
            amount: CrapTokens::from_crap(5.0),
            timestamp: 12345,
        };
        game.place_bet(bet).unwrap();
        
        // Roll a natural (7)
        let roll = DiceRoll::new(3, 4);
        let resolutions = game.process_roll(roll);
        
        assert_eq!(resolutions.len(), 1);
        match &resolutions[0] {
            BetResolution::Won { payout, .. } => {
                assert_eq!(*payout, CrapTokens::from_crap(5.0).amount() * 2);
            }
            _ => panic!("Expected winning resolution"),
        }
    }
    
    #[tokio::test]
    async fn test_treasury_participation() {
        let treasury = TreasuryParticipant::new(CrapTokens::from_crap(1000.0).amount());
        let game_id = [1u8; 16];
        
        // Treasury joins game
        treasury.auto_join_game(game_id).await.unwrap();
        
        // Player places bet
        let player_bet = Bet {
            id: [2u8; 16],
            game_id,
            player: [3u8; 32],
            bet_type: BetType::Pass,
            amount: CrapTokens::from_crap(10.0),
            timestamp: 12345,
        };
        
        // Treasury responds with counter-bet
        let counter_bets = treasury.handle_player_bet(game_id, [3u8; 32], player_bet).await.unwrap();
        
        assert_eq!(counter_bets.len(), 1);
        assert_eq!(counter_bets[0].bet_type, BetType::DontPass);
        assert_eq!(counter_bets[0].amount.amount(), CrapTokens::from_crap(10.0).amount());
    }
}