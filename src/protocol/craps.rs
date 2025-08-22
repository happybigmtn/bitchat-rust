//! Complete craps game implementation with all 64 bet types
//! 
//! This module contains the full game logic from week1.md documentation,
//! including all bet types, resolution logic, and special bet tracking.

use std::collections::{HashMap, HashSet};
use super::{PeerId, GameId, CrapTokens, DiceRoll, BetType, Bet};
use serde::{Serialize, Deserialize};

/// Game phase in craps
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GamePhase {
    ComeOut,
    Point,
    Ended,
    GameEnded,  // Alias for compatibility
}

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

/// Result of bet resolution
#[derive(Debug, Clone)]
pub enum BetResolution {
    Won {
        player: PeerId,
        bet_type: BetType,
        amount: CrapTokens,
        payout: CrapTokens,
    },
    Lost {
        player: PeerId,
        bet_type: BetType,
        amount: CrapTokens,
    },
    Push {
        player: PeerId,
        bet_type: BetType,
        amount: CrapTokens,
    },
}

impl CrapsGame {
    /// Add a player to the game
    pub fn add_player(&mut self, player: PeerId) -> bool {
        if !self.participants.contains(&player) {
            self.participants.push(player);
            true
        } else {
            false
        }
    }
    
    /// Place a bet
    pub fn place_bet(&mut self, player: PeerId, bet: Bet) -> Result<(), String> {
        // Add bet to player's bets
        self.player_bets
            .entry(player)
            .or_insert_with(HashMap::new)
            .insert(bet.bet_type.clone(), bet);
        Ok(())
    }
    
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
    
    /// Process a dice roll and return all bet resolutions
    /// 
    /// Feynman: This is the "moment of truth" - when dice land, we need to:
    /// 1. Check every active bet to see if it wins/loses/pushes
    /// 2. Update game phase (establish point, seven-out, etc.)
    /// 3. Track special bet progress (Fire points, Repeater counts)
    /// 4. Calculate exact payouts based on bet type and amount
    pub fn process_roll(&mut self, roll: DiceRoll) -> Vec<BetResolution> {
        let mut resolutions = Vec::new();
        let total = roll.total();
        
        // Track roll history
        self.roll_history.push(roll);
        self.roll_count += 1;
        
        // Update special bet tracking
        self.update_special_tracking(roll);
        
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
    
    /// Resolve come-out roll bets
    fn resolve_comeout_roll(&self, roll: DiceRoll) -> Vec<BetResolution> {
        let mut resolutions = Vec::new();
        let total = roll.total();
        
        for (player, bets) in &self.player_bets {
            // Pass Line
            if let Some(bet) = bets.get(&BetType::Pass) {
                match total {
                    7 | 11 => {
                        let payout = CrapTokens::new(bet.amount.amount * 2); // 1:1 payout
                        resolutions.push(BetResolution::Won {
                            player: *player,
                            bet_type: BetType::Pass,
                            amount: bet.amount,
                            payout,
                        });
                    },
                    2 | 3 | 12 => {
                        resolutions.push(BetResolution::Lost {
                            player: *player,
                            bet_type: BetType::Pass,
                            amount: bet.amount,
                        });
                    },
                    _ => {}, // Point established, bet remains
                }
            }
            
            // Don't Pass
            if let Some(bet) = bets.get(&BetType::DontPass) {
                match total {
                    2 | 3 => {
                        let payout = CrapTokens::new(bet.amount.amount * 2); // 1:1 payout
                        resolutions.push(BetResolution::Won {
                            player: *player,
                            bet_type: BetType::DontPass,
                            amount: bet.amount,
                            payout,
                        });
                    },
                    7 | 11 => {
                        resolutions.push(BetResolution::Lost {
                            player: *player,
                            bet_type: BetType::DontPass,
                            amount: bet.amount,
                        });
                    },
                    12 => {
                        resolutions.push(BetResolution::Push {
                            player: *player,
                            bet_type: BetType::DontPass,
                            amount: bet.amount,
                        });
                    },
                    _ => {}, // Point established, bet remains
                }
            }
        }
        
        resolutions
    }
    
    /// Resolve point phase roll bets
    fn resolve_point_roll(&mut self, roll: DiceRoll) -> Vec<BetResolution> {
        let mut resolutions = Vec::new();
        let total = roll.total();
        let point = self.point.unwrap();
        
        for (player, bets) in &self.player_bets {
            // Check if point made or seven-out
            if total == point {
                // Point made - Pass wins
                if let Some(bet) = bets.get(&BetType::Pass) {
                    let payout = CrapTokens::new(bet.amount.amount * 2);
                    resolutions.push(BetResolution::Won {
                        player: *player,
                        bet_type: BetType::Pass,
                        amount: bet.amount,
                        payout,
                    });
                }
                
                // Don't Pass loses
                if let Some(bet) = bets.get(&BetType::DontPass) {
                    resolutions.push(BetResolution::Lost {
                        player: *player,
                        bet_type: BetType::DontPass,
                        amount: bet.amount,
                    });
                }
                
                // Resolve Pass Odds bets
                if let Some(bet) = bets.get(&BetType::OddsPass) {
                    let multiplier = Self::get_odds_multiplier(point, true);
                    let payout = CrapTokens::new(bet.amount.amount + (bet.amount.amount * multiplier as u64 / 100));
                    resolutions.push(BetResolution::Won {
                        player: *player,
                        bet_type: BetType::OddsPass,
                        amount: bet.amount,
                        payout,
                    });
                }
            } else if total == 7 {
                // Seven-out - Pass loses
                if let Some(bet) = bets.get(&BetType::Pass) {
                    resolutions.push(BetResolution::Lost {
                        player: *player,
                        bet_type: BetType::Pass,
                        amount: bet.amount,
                    });
                }
                
                // Don't Pass wins
                if let Some(bet) = bets.get(&BetType::DontPass) {
                    let payout = CrapTokens::new(bet.amount.amount * 2);
                    resolutions.push(BetResolution::Won {
                        player: *player,
                        bet_type: BetType::DontPass,
                        amount: bet.amount,
                        payout,
                    });
                }
                
                // All YES bets lose on 7
                for bet_type in [BetType::Yes2, BetType::Yes3, BetType::Yes4, 
                                BetType::Yes5, BetType::Yes6, BetType::Yes8,
                                BetType::Yes9, BetType::Yes10, BetType::Yes11, 
                                BetType::Yes12].iter() {
                    if let Some(bet) = bets.get(bet_type) {
                        resolutions.push(BetResolution::Lost {
                            player: *player,
                            bet_type: *bet_type,
                            amount: bet.amount,
                        });
                    }
                }
                
                // All hardways lose on 7
                for bet_type in [BetType::Hard4, BetType::Hard6, 
                                BetType::Hard8, BetType::Hard10].iter() {
                    if let Some(bet) = bets.get(bet_type) {
                        resolutions.push(BetResolution::Lost {
                            player: *player,
                            bet_type: *bet_type,
                            amount: bet.amount,
                        });
                    }
                }
            } else {
                // Neither point nor seven - check other bets
                resolutions.extend(self.resolve_yes_bets(roll, player, bets));
                resolutions.extend(self.resolve_no_bets(roll, player, bets));
                resolutions.extend(self.resolve_hardway_bets(roll, player, bets));
            }
        }
        
        resolutions
    }
    
    /// Resolve one-roll proposition bets
    fn resolve_one_roll_bets(&self, roll: DiceRoll) -> Vec<BetResolution> {
        let mut resolutions = Vec::new();
        let total = roll.total();
        
        for (player, bets) in &self.player_bets {
            // NEXT bets - win if exact number rolled
            resolutions.extend(self.resolve_next_bets(roll, player, bets));
            
            // Field bet
            if let Some(bet) = bets.get(&BetType::Field) {
                match total {
                    2 | 12 => {
                        // Field pays 2:1 on 2 and 12
                        let payout = CrapTokens::new(bet.amount.amount * 3);
                        resolutions.push(BetResolution::Won {
                            player: *player,
                            bet_type: BetType::Field,
                            amount: bet.amount,
                            payout,
                        });
                    },
                    3 | 4 | 9 | 10 | 11 => {
                        // Field pays 1:1 on these
                        let payout = CrapTokens::new(bet.amount.amount * 2);
                        resolutions.push(BetResolution::Won {
                            player: *player,
                            bet_type: BetType::Field,
                            amount: bet.amount,
                            payout,
                        });
                    },
                    _ => {
                        resolutions.push(BetResolution::Lost {
                            player: *player,
                            bet_type: BetType::Field,
                            amount: bet.amount,
                        });
                    }
                }
            }
        }
        
        resolutions
    }
    
    /// Update special bet tracking
    fn update_special_tracking(&mut self, roll: DiceRoll) {
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
    
    /// Get true odds multiplier for odds bets
    fn get_odds_multiplier(point: u8, is_pass: bool) -> u32 {
        match (point, is_pass) {
            (4 | 10, true) => 200,  // 2:1 for pass
            (5 | 9, true) => 150,   // 3:2 for pass
            (6 | 8, true) => 120,   // 6:5 for pass
            (4 | 10, false) => 50,  // 1:2 for don't pass
            (5 | 9, false) => 67,   // 2:3 for don't pass
            (6 | 8, false) => 83,   // 5:6 for don't pass
            _ => 100,
        }
    }
    
    /// Update game phase based on roll
    fn update_phase(&mut self, total: u8) {
        match self.phase {
            GamePhase::ComeOut => {
                match total {
                    4 | 5 | 6 | 8 | 9 | 10 => {
                        self.point = Some(total);
                        self.phase = GamePhase::Point;
                    },
                    _ => {}, // Stay in come-out
                }
            },
            GamePhase::Point => {
                if total == 7 || total == self.point.unwrap() {
                    // Seven-out or point made - new series
                    self.point = None;
                    self.phase = GamePhase::ComeOut;
                    self.series_id += 1;
                    
                    // Reset special tracking for new series
                    if total == 7 {
                        self.fire_points.clear();
                        self.bonus_numbers.clear();
                        self.hot_roller_streak = 0;
                    } else {
                        self.hot_roller_streak += 1;
                    }
                }
            },
            _ => {},
        }
    }
    
    /// Resolve YES bets (player bets number will come before 7)
    pub fn resolve_yes_bets(&self, roll: DiceRoll, player: &PeerId, bets: &HashMap<BetType, Bet>) -> Vec<BetResolution> {
        let mut resolutions = Vec::new();
        let total = roll.total();
        
        // Check each YES bet type
        for bet_type in [BetType::Yes2, BetType::Yes3, BetType::Yes4, BetType::Yes5, 
                         BetType::Yes6, BetType::Yes8, BetType::Yes9, BetType::Yes10,
                         BetType::Yes11, BetType::Yes12] {
            if let Some(bet) = bets.get(&bet_type) {
                // Extract the target number from the bet type
                let target = match bet_type {
                    BetType::Yes2 => 2,
                    BetType::Yes3 => 3,
                    BetType::Yes4 => 4,
                    BetType::Yes5 => 5,
                    BetType::Yes6 => 6,
                    BetType::Yes8 => 8,
                    BetType::Yes9 => 9,
                    BetType::Yes10 => 10,
                    BetType::Yes11 => 11,
                    BetType::Yes12 => 12,
                    _ => continue,
                };
                
                if total == target {
                    // Win! Number came up
                    let multiplier = self.get_yes_bet_multiplier(target);
                    let payout = CrapTokens::new(bet.amount.amount + (bet.amount.amount * multiplier as u64 / 100));
                    resolutions.push(BetResolution::Won {
                        player: *player,
                        bet_type,
                        amount: bet.amount,
                        payout,
                    });
                } else if total == 7 {
                    // Loss - seven came first
                    resolutions.push(BetResolution::Lost {
                        player: *player,
                        bet_type,
                        amount: bet.amount,
                    });
                }
            }
        }
        
        resolutions
    }
    
    /// Resolve NO bets (player bets 7 will come before the number)
    pub fn resolve_no_bets(&self, roll: DiceRoll, player: &PeerId, bets: &HashMap<BetType, Bet>) -> Vec<BetResolution> {
        let mut resolutions = Vec::new();
        let total = roll.total();
        
        for bet_type in [BetType::No2, BetType::No3, BetType::No4, BetType::No5,
                         BetType::No6, BetType::No8, BetType::No9, BetType::No10,
                         BetType::No11, BetType::No12] {
            if let Some(bet) = bets.get(&bet_type) {
                let target = match bet_type {
                    BetType::No2 => 2,
                    BetType::No3 => 3,
                    BetType::No4 => 4,
                    BetType::No5 => 5,
                    BetType::No6 => 6,
                    BetType::No8 => 8,
                    BetType::No9 => 9,
                    BetType::No10 => 10,
                    BetType::No11 => 11,
                    BetType::No12 => 12,
                    _ => continue,
                };
                
                if total == 7 {
                    // Win! Seven came first
                    let multiplier = self.get_no_bet_multiplier(target);
                    let payout = CrapTokens::new(bet.amount.amount + (bet.amount.amount * multiplier as u64 / 100));
                    resolutions.push(BetResolution::Won {
                        player: *player,
                        bet_type,
                        amount: bet.amount,
                        payout,
                    });
                } else if total == target {
                    // Loss - target number came first
                    resolutions.push(BetResolution::Lost {
                        player: *player,
                        bet_type,
                        amount: bet.amount,
                    });
                }
            }
        }
        
        resolutions
    }
    
    /// Resolve Hardway bets
    pub fn resolve_hardway_bets(&self, roll: DiceRoll, player: &PeerId, bets: &HashMap<BetType, Bet>) -> Vec<BetResolution> {
        let mut resolutions = Vec::new();
        let total = roll.total();
        let is_hard = roll.is_hard_way();
        
        // Hard 4 (2+2)
        if let Some(bet) = bets.get(&BetType::Hard4) {
            if total == 4 {
                if is_hard {
                    // Win - came the hard way!
                    let payout = CrapTokens::new(bet.amount.amount * 8); // 7:1 + original
                    resolutions.push(BetResolution::Won {
                        player: *player,
                        bet_type: BetType::Hard4,
                        amount: bet.amount,
                        payout,
                    });
                } else {
                    // Loss - came easy way
                    resolutions.push(BetResolution::Lost {
                        player: *player,
                        bet_type: BetType::Hard4,
                        amount: bet.amount,
                    });
                }
            } else if total == 7 {
                // Loss - seven out
                resolutions.push(BetResolution::Lost {
                    player: *player,
                    bet_type: BetType::Hard4,
                    amount: bet.amount,
                });
            }
        }
        
        // Similar logic for Hard 6, 8, 10
        for (bet_type, target, payout_mult) in [
            (BetType::Hard6, 6, 10),
            (BetType::Hard8, 8, 10),
            (BetType::Hard10, 10, 8),
        ] {
            if let Some(bet) = bets.get(&bet_type) {
                if total == target {
                    if is_hard {
                        let payout = CrapTokens::new(bet.amount.amount * payout_mult);
                        resolutions.push(BetResolution::Won {
                            player: *player,
                            bet_type,
                            amount: bet.amount,
                            payout,
                        });
                    } else {
                        resolutions.push(BetResolution::Lost {
                            player: *player,
                            bet_type,
                            amount: bet.amount,
                        });
                    }
                } else if total == 7 {
                    resolutions.push(BetResolution::Lost {
                        player: *player,
                        bet_type,
                        amount: bet.amount,
                    });
                }
            }
        }
        
        resolutions
    }
    
    /// Resolve NEXT bets (one-roll proposition bets)
    pub fn resolve_next_bets(&self, roll: DiceRoll, player: &PeerId, bets: &HashMap<BetType, Bet>) -> Vec<BetResolution> {
        let mut resolutions = Vec::new();
        let total = roll.total();
        
        for bet_type in [BetType::Next2, BetType::Next3, BetType::Next4, BetType::Next5,
                         BetType::Next6, BetType::Next7, BetType::Next8, BetType::Next9,
                         BetType::Next10, BetType::Next11, BetType::Next12] {
            if let Some(bet) = bets.get(&bet_type) {
                let target = match bet_type {
                    BetType::Next2 => 2,
                    BetType::Next3 => 3,
                    BetType::Next4 => 4,
                    BetType::Next5 => 5,
                    BetType::Next6 => 6,
                    BetType::Next7 => 7,
                    BetType::Next8 => 8,
                    BetType::Next9 => 9,
                    BetType::Next10 => 10,
                    BetType::Next11 => 11,
                    BetType::Next12 => 12,
                    _ => continue,
                };
                
                if total == target {
                    // Win!
                    let multiplier = self.get_next_bet_multiplier(target);
                    let payout = CrapTokens::new(bet.amount.amount + (bet.amount.amount * multiplier as u64 / 100));
                    resolutions.push(BetResolution::Won {
                        player: *player,
                        bet_type,
                        amount: bet.amount,
                        payout,
                    });
                } else {
                    // Loss - didn't hit the number
                    resolutions.push(BetResolution::Lost {
                        player: *player,
                        bet_type,
                        amount: bet.amount,
                    });
                }
            }
        }
        
        resolutions
    }
    
    /// Get payout multipliers matching the Hackathon contracts exactly
    fn get_yes_bet_multiplier(&self, target: u8) -> u32 {
        match target {
            2 | 12 => 588,  // 5.88:1
            3 | 11 => 294,  // 2.94:1
            4 | 10 => 196,  // 1.96:1
            5 | 9 => 147,   // 1.47:1
            6 | 8 => 118,   // 1.18:1
            _ => 100,
        }
    }
    
    fn get_no_bet_multiplier(&self, target: u8) -> u32 {
        match target {
            2 | 12 => 16,   // 0.16:1
            3 | 11 => 33,   // 0.33:1
            4 | 10 => 49,   // 0.49:1
            5 | 9 => 65,    // 0.65:1
            6 | 8 => 82,    // 0.82:1
            _ => 100,
        }
    }
    
    fn get_next_bet_multiplier(&self, target: u8) -> u32 {
        match target {
            2 | 12 => 3430, // 34.3:1
            3 | 11 => 1666, // 16.66:1
            4 | 10 => 1078, // 10.78:1
            5 | 9 => 784,   // 7.84:1
            6 | 8 => 608,   // 6.08:1
            7 => 490,       // 4.9:1
            _ => 100,
        }
    }
    
    fn get_repeater_multiplier(&self, number: u8) -> u32 {
        match number {
            2 | 12 => 4000,  // 40:1
            3 | 11 => 5000,  // 50:1
            4 | 10 => 6500,  // 65:1
            5 | 9 => 8000,   // 80:1
            6 | 8 => 9000,   // 90:1
            _ => 100,
        }
    }
    
    /// Check if Fire bet wins (4-6 unique points made)
    /// 
    /// Feynman: Fire bet is like collecting stamps - you need to
    /// make different point numbers. More unique points = bigger payout.
    pub fn resolve_fire_bet(&self, player: &PeerId, bet: &Bet) -> Option<BetResolution> {
        let unique_points = self.fire_points.len();
        
        match unique_points {
            4 => Some(BetResolution::Won {
                player: *player,
                bet_type: BetType::Fire,
                amount: bet.amount,
                payout: CrapTokens::new(bet.amount.amount * 25), // 24:1
            }),
            5 => Some(BetResolution::Won {
                player: *player,
                bet_type: BetType::Fire,
                amount: bet.amount,
                payout: CrapTokens::new(bet.amount.amount * 250), // 249:1
            }),
            6 => Some(BetResolution::Won {
                player: *player,
                bet_type: BetType::Fire,
                amount: bet.amount,
                payout: CrapTokens::new(bet.amount.amount * 1000), // 999:1
            }),
            _ => None, // Still active
        }
    }
    
    /// Check Repeater bets
    /// 
    /// Feynman: Repeater bets need a number to appear N times.
    /// Harder numbers (2, 12) need fewer repeats than easier ones (6, 8).
    pub fn resolve_repeater_bets(&self, player: &PeerId, bets: &HashMap<BetType, Bet>) -> Vec<BetResolution> {
        let mut resolutions = Vec::new();
        
        let repeater_requirements = [
            (BetType::Repeater2, 2, 2),   // 2 must appear 2 times
            (BetType::Repeater3, 3, 3),   // 3 must appear 3 times
            (BetType::Repeater4, 4, 4),   // 4 must appear 4 times
            (BetType::Repeater5, 5, 5),   // 5 must appear 5 times
            (BetType::Repeater6, 6, 6),   // 6 must appear 6 times
            (BetType::Repeater8, 8, 6),   // 8 must appear 6 times
            (BetType::Repeater9, 9, 5),   // 9 must appear 5 times
            (BetType::Repeater10, 10, 4), // 10 must appear 4 times
            (BetType::Repeater11, 11, 3), // 11 must appear 3 times
            (BetType::Repeater12, 12, 2), // 12 must appear 2 times
        ];
        
        for (bet_type, number, required) in repeater_requirements.iter() {
            if let Some(bet) = bets.get(bet_type) {
                let count = self.repeater_counts.get(number).copied().unwrap_or(0);
                
                if count >= *required {
                    let multiplier = self.get_repeater_multiplier(*number);
                    let payout = CrapTokens::new(bet.amount.amount + (bet.amount.amount * multiplier as u64 / 100));
                    resolutions.push(BetResolution::Won {
                        player: *player,
                        bet_type: *bet_type,
                        amount: bet.amount,
                        payout,
                    });
                }
            }
        }
        
        resolutions
    }
    
    /// Check Bonus Small/Tall/All bets
    /// 
    /// Feynman: These are "collection" bets - roll all numbers in a range
    /// before rolling a 7. Like completing a set before time runs out.
    pub fn resolve_bonus_bets(&self, player: &PeerId, bets: &HashMap<BetType, Bet>) -> Vec<BetResolution> {
        let mut resolutions = Vec::new();
        
        // Bonus Small: All 2,3,4,5,6 before 7
        if let Some(bet) = bets.get(&BetType::BonusSmall) {
            let small_numbers: HashSet<u8> = [2, 3, 4, 5, 6].iter().copied().collect();
            if small_numbers.is_subset(&self.bonus_numbers) {
                let payout = CrapTokens::new(bet.amount.amount * 31); // 30:1
                resolutions.push(BetResolution::Won {
                    player: *player,
                    bet_type: BetType::BonusSmall,
                    amount: bet.amount,
                    payout,
                });
            }
        }
        
        // Bonus Tall: All 8,9,10,11,12 before 7
        if let Some(bet) = bets.get(&BetType::BonusTall) {
            let tall_numbers: HashSet<u8> = [8, 9, 10, 11, 12].iter().copied().collect();
            if tall_numbers.is_subset(&self.bonus_numbers) {
                let payout = CrapTokens::new(bet.amount.amount * 31); // 30:1
                resolutions.push(BetResolution::Won {
                    player: *player,
                    bet_type: BetType::BonusTall,
                    amount: bet.amount,
                    payout,
                });
            }
        }
        
        // Bonus All: All numbers 2-12 except 7
        if let Some(bet) = bets.get(&BetType::BonusAll) {
            let all_numbers: HashSet<u8> = [2, 3, 4, 5, 6, 8, 9, 10, 11, 12].iter().copied().collect();
            if all_numbers.is_subset(&self.bonus_numbers) {
                let payout = CrapTokens::new(bet.amount.amount * 151); // 150:1
                resolutions.push(BetResolution::Won {
                    player: *player,
                    bet_type: BetType::BonusAll,
                    amount: bet.amount,
                    payout,
                });
            }
        }
        
        resolutions
    }
    
    /// Resolve Hot Roller bet (progressive streak)
    /// 
    /// Feynman: Hot Roller rewards consistency - the more rolls without
    /// sevening out, the bigger your multiplier grows. It's like a 
    /// combo meter in a video game.
    pub fn resolve_hot_roller_bet(&self, player: &PeerId, bet: &Bet) -> Option<BetResolution> {
        if self.roll_count > 20 && self.phase == GamePhase::Point {
            // Progressive payout based on roll streak
            let multiplier = match self.roll_count {
                20..=30 => 200,   // 2:1
                31..=40 => 500,   // 5:1
                41..=50 => 1000,  // 10:1
                _ => 2000,        // 20:1 for 50+ rolls
            };
            let payout = CrapTokens::new((bet.amount.amount * multiplier as u64) / 100);
            return Some(BetResolution::Won {
                player: *player,
                bet_type: BetType::HotRoller,
                amount: bet.amount,
                payout: CrapTokens::new(bet.amount.amount + payout.amount),
            });
        }
        None
    }
    
    /// Resolve Twice Hard bet (same hardway twice in a row)
    /// 
    /// Feynman: Lightning striking twice - you need the exact same
    /// hardway number to come up twice consecutively. It's rare,
    /// so it pays well.
    pub fn resolve_twice_hard_bet(&self, player: &PeerId, bet: &Bet) -> Option<BetResolution> {
        // Check hardway streak tracker
        for (_number, &count) in &self.hardway_streak {
            if count >= 2 {
                let payout = CrapTokens::new(bet.amount.amount * 7); // 6:1 + original
                return Some(BetResolution::Won {
                    player: *player,
                    bet_type: BetType::TwiceHard,
                    amount: bet.amount,
                    payout,
                });
            }
        }
        None
    }
    
    /// Resolve Ride the Line bet (pass line win streak)
    /// 
    /// Feynman: This bet rewards loyalty to the pass line - the more
    /// consecutive pass line wins, the higher your bonus multiplier.
    pub fn resolve_ride_line_bet(&self, player: &PeerId, bet: &Bet, pass_wins: u32) -> Option<BetResolution> {
        if pass_wins >= 3 {
            let multiplier = match pass_wins {
                3 => 300,   // 3:1
                4 => 500,   // 5:1
                5 => 1000,  // 10:1
                _ => 2500,  // 25:1 for 6+ wins
            };
            let payout = CrapTokens::new((bet.amount.amount * multiplier as u64) / 100);
            return Some(BetResolution::Won {
                player: *player,
                bet_type: BetType::RideLine,
                amount: bet.amount,
                payout: CrapTokens::new(bet.amount.amount + payout.amount),
            });
        }
        None
    }
    
    /// Resolve Muggsy bet (7 on comeout or point-7 combination)
    /// 
    /// Feynman: Muggsy is a contrarian bet - you win if the shooter
    /// gets a natural 7 on comeout, then establishes a point and
    /// immediately sevens out. It's a specific sequence.
    pub fn resolve_muggsy_bet(&self, player: &PeerId, bet: &Bet, last_two_rolls: &[DiceRoll]) -> Option<BetResolution> {
        if last_two_rolls.len() >= 2 {
            let prev = last_two_rolls[last_two_rolls.len() - 2].total();
            let curr = last_two_rolls[last_two_rolls.len() - 1].total();
            
            // Check for the Muggsy pattern
            if prev == 7 && self.phase == GamePhase::ComeOut {
                // Natural 7 on comeout followed by establishing point
                if curr >= 4 && curr <= 10 && curr != 7 {
                    let payout = CrapTokens::new(bet.amount.amount * 3); // 2:1 + original
                    return Some(BetResolution::Won {
                        player: *player,
                        bet_type: BetType::Muggsy,
                        amount: bet.amount,
                        payout,
                    });
                }
            }
        }
        None
    }
    
    /// Resolve Replay bet (same point 3+ times)
    /// 
    /// Feynman: Replay is about repetition - if the shooter makes
    /// the same point number 3 or more times in their series, you win.
    /// It's like the shooter has a "favorite" number.
    pub fn resolve_replay_bet(&self, player: &PeerId, bet: &Bet, point_history: &[u8]) -> Option<BetResolution> {
        let mut point_counts: HashMap<u8, u32> = HashMap::new();
        for &point in point_history {
            *point_counts.entry(point).or_insert(0) += 1;
        }
        
        for (&_point, &count) in &point_counts {
            if count >= 3 {
                let multiplier = match count {
                    3 => 1000,  // 10:1
                    4 => 2500,  // 25:1
                    _ => 5000,  // 50:1 for 5+
                };
                let payout = CrapTokens::new((bet.amount.amount * multiplier as u64) / 100);
                return Some(BetResolution::Won {
                    player: *player,
                    bet_type: BetType::Replay,
                    amount: bet.amount,
                    payout: CrapTokens::new(bet.amount.amount + payout.amount),
                });
            }
        }
        None
    }
    
    /// Resolve Different Doubles bet (unique doubles before 7)
    /// 
    /// Feynman: This bet is about variety in doubles - you need to roll
    /// different hardway numbers (2+2, 3+3, 4+4, 5+5, 6+6) before a 7.
    /// The more unique doubles, the bigger the payout.
    pub fn resolve_different_doubles_bet(&self, player: &PeerId, bet: &Bet, doubles_rolled: &HashSet<u8>) -> Option<BetResolution> {
        let count = doubles_rolled.len();
        if count >= 2 {
            let multiplier = match count {
                2 => 600,   // 6:1
                3 => 2500,  // 25:1
                4 => 10000, // 100:1
                _ => 25000, // 250:1 for all 5
            };
            let payout = CrapTokens::new((bet.amount.amount * multiplier as u64) / 100);
            return Some(BetResolution::Won {
                player: *player,
                bet_type: BetType::DifferentDoubles,
                amount: bet.amount,
                payout: CrapTokens::new(bet.amount.amount + payout.amount),
            });
        }
        None
    }
}
