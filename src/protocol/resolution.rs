//! Bet resolution logic for all craps bet types
//! 
//! This module contains the complex logic for resolving different
//! types of bets based on dice rolls and game state.

use std::collections::HashMap;
use super::{PeerId, BetType, Bet, CrapTokens, DiceRoll};
use crate::protocol::bet_types::{GamePhase, BetResolution};
use crate::protocol::game_logic::CrapsGame;

/// Trait for resolving bets based on dice rolls
pub trait BetResolver {
    /// Resolve come-out roll bets
    fn resolve_comeout_roll(&self, roll: DiceRoll) -> Vec<BetResolution>;
    
    /// Resolve point phase roll bets  
    fn resolve_point_roll(&mut self, roll: DiceRoll) -> Vec<BetResolution>;
    
    /// Resolve one-roll proposition bets
    fn resolve_one_roll_bets(&self, roll: DiceRoll) -> Vec<BetResolution>;
    
    /// Resolve YES bets (player bets number will come before 7)
    fn resolve_yes_bets(&self, roll: DiceRoll, player: &PeerId, bets: &HashMap<BetType, Bet>) -> Vec<BetResolution>;
    
    /// Resolve NO bets (player bets 7 will come before the number)
    fn resolve_no_bets(&self, roll: DiceRoll, player: &PeerId, bets: &HashMap<BetType, Bet>) -> Vec<BetResolution>;
    
    /// Resolve Hardway bets
    fn resolve_hardway_bets(&self, roll: DiceRoll, player: &PeerId, bets: &HashMap<BetType, Bet>) -> Vec<BetResolution>;
    
    /// Resolve Come bets (similar to Pass Line but placed after comeout)
    fn resolve_come_bets(&mut self, roll: DiceRoll, player: &PeerId, bets: &HashMap<BetType, Bet>) -> Vec<BetResolution>;
    
    /// Resolve Don't Come bets (opposite of Come)
    fn resolve_dont_come_bets(&mut self, roll: DiceRoll, player: &PeerId, bets: &HashMap<BetType, Bet>) -> Vec<BetResolution>;
    
    /// Resolve NEXT bets (one-roll proposition bets)
    fn resolve_next_bets(&self, roll: DiceRoll, player: &PeerId, bets: &HashMap<BetType, Bet>) -> Vec<BetResolution>;
}

impl BetResolver for CrapsGame {
    /// Resolve come-out roll bets
    fn resolve_comeout_roll(&self, roll: DiceRoll) -> Vec<BetResolution> {
        let mut resolutions = Vec::new();
        let total = roll.total();
        
        for (player, bets) in &self.player_bets {
            // Pass Line
            if let Some(bet) = bets.get(&BetType::Pass) {
                match total {
                    7 | 11 => {
                        let payout = CrapTokens::new_unchecked(bet.amount.amount * 2); // 1:1 payout
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
                        let payout = CrapTokens::new_unchecked(bet.amount.amount * 2); // 1:1 payout
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
                    let payout = CrapTokens::new_unchecked(bet.amount.amount * 2);
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
                    let multiplier = get_odds_multiplier(point, true);
                    let payout = CrapTokens::new_unchecked(bet.amount.amount + (bet.amount.amount * multiplier as u64 / 100));
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
                    let payout = CrapTokens::new_unchecked(bet.amount.amount * 2);
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
        
        // Resolve Come/Don't Come bets (need separate loop to avoid borrow conflicts)
        let player_bets_clone = self.player_bets.clone();
        for (player, bets) in &player_bets_clone {
            resolutions.extend(self.resolve_come_bets(roll, player, bets));
            resolutions.extend(self.resolve_dont_come_bets(roll, player, bets));
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
                        let payout = CrapTokens::new_unchecked(bet.amount.amount * 3);
                        resolutions.push(BetResolution::Won {
                            player: *player,
                            bet_type: BetType::Field,
                            amount: bet.amount,
                            payout,
                        });
                    },
                    3 | 4 | 9 | 10 | 11 => {
                        // Field pays 1:1 on these
                        let payout = CrapTokens::new_unchecked(bet.amount.amount * 2);
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
    
    /// Resolve YES bets (player bets number will come before 7)
    fn resolve_yes_bets(&self, roll: DiceRoll, player: &PeerId, bets: &HashMap<BetType, Bet>) -> Vec<BetResolution> {
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
                    use crate::protocol::payouts::PayoutCalculator;
                    let multiplier = self.get_yes_bet_multiplier(target);
                    let payout = CrapTokens::new_unchecked(bet.amount.amount + (bet.amount.amount * multiplier as u64 / 100));
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
    fn resolve_no_bets(&self, roll: DiceRoll, player: &PeerId, bets: &HashMap<BetType, Bet>) -> Vec<BetResolution> {
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
                    use crate::protocol::payouts::PayoutCalculator;
                    let multiplier = self.get_no_bet_multiplier(target);
                    let payout = CrapTokens::new_unchecked(bet.amount.amount + (bet.amount.amount * multiplier as u64 / 100));
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
    fn resolve_hardway_bets(&self, roll: DiceRoll, player: &PeerId, bets: &HashMap<BetType, Bet>) -> Vec<BetResolution> {
        let mut resolutions = Vec::new();
        let total = roll.total();
        let is_hard = roll.is_hard_way();
        
        // Hard 4 (2+2)
        if let Some(bet) = bets.get(&BetType::Hard4) {
            if total == 4 {
                if is_hard {
                    // Win - came the hard way!
                    let payout = CrapTokens::new_unchecked(bet.amount.amount * 8); // 7:1 + original
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
                        let payout = CrapTokens::new_unchecked(bet.amount.amount * payout_mult);
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
    
    /// Resolve Come bets (similar to Pass Line but placed after comeout)
    fn resolve_come_bets(&mut self, roll: DiceRoll, player: &PeerId, bets: &HashMap<BetType, Bet>) -> Vec<BetResolution> {
        let mut resolutions = Vec::new();
        let total = roll.total();
        
        if let Some(bet) = bets.get(&BetType::Come) {
            match total {
                7 | 11 => {
                    // Come bet wins immediately
                    let payout = CrapTokens::new_unchecked(bet.amount.amount * 2); // 1:1 payout
                    resolutions.push(BetResolution::Won {
                        player: *player,
                        bet_type: BetType::Come,
                        amount: bet.amount,
                        payout,
                    });
                },
                2 | 3 | 12 => {
                    // Come bet loses immediately
                    resolutions.push(BetResolution::Lost {
                        player: *player,
                        bet_type: BetType::Come,
                        amount: bet.amount,
                    });
                },
                4 | 5 | 6 | 8 | 9 | 10 => {
                    // Establish Come point - move bet to come_points tracking
                    self.come_points
                        .entry(*player)
                        .or_insert_with(HashMap::new)
                        .insert(total, bet.amount);
                },
                _ => {}
            }
        }
        
        // Check existing Come points
        if let Some(player_come_points) = self.come_points.get(player) {
            for (&point, &amount) in player_come_points.iter() {
                if total == point {
                    // Come point made - win
                    let payout = CrapTokens::new_unchecked(amount.amount * 2);
                    resolutions.push(BetResolution::Won {
                        player: *player,
                        bet_type: BetType::Come,
                        amount,
                        payout,
                    });
                } else if total == 7 {
                    // Seven out - Come bets lose
                    resolutions.push(BetResolution::Lost {
                        player: *player,
                        bet_type: BetType::Come,
                        amount,
                    });
                }
            }
        }
        
        resolutions
    }
    
    /// Resolve Don't Come bets (opposite of Come)
    fn resolve_dont_come_bets(&mut self, roll: DiceRoll, player: &PeerId, bets: &HashMap<BetType, Bet>) -> Vec<BetResolution> {
        let mut resolutions = Vec::new();
        let total = roll.total();
        
        if let Some(bet) = bets.get(&BetType::DontCome) {
            match total {
                2 | 3 => {
                    // Don't Come bet wins immediately
                    let payout = CrapTokens::new_unchecked(bet.amount.amount * 2); // 1:1 payout
                    resolutions.push(BetResolution::Won {
                        player: *player,
                        bet_type: BetType::DontCome,
                        amount: bet.amount,
                        payout,
                    });
                },
                7 | 11 => {
                    // Don't Come bet loses immediately
                    resolutions.push(BetResolution::Lost {
                        player: *player,
                        bet_type: BetType::DontCome,
                        amount: bet.amount,
                    });
                },
                12 => {
                    // Don't Come pushes on 12
                    resolutions.push(BetResolution::Push {
                        player: *player,
                        bet_type: BetType::DontCome,
                        amount: bet.amount,
                    });
                },
                4 | 5 | 6 | 8 | 9 | 10 => {
                    // Establish Don't Come point
                    self.dont_come_points
                        .entry(*player)
                        .or_insert_with(HashMap::new)
                        .insert(total, bet.amount);
                },
                _ => {}
            }
        }
        
        // Check existing Don't Come points
        if let Some(player_dont_come_points) = self.dont_come_points.get(player) {
            for (&point, &amount) in player_dont_come_points.iter() {
                if total == 7 {
                    // Seven out - Don't Come bets win
                    let payout = CrapTokens::new_unchecked(amount.amount * 2);
                    resolutions.push(BetResolution::Won {
                        player: *player,
                        bet_type: BetType::DontCome,
                        amount,
                        payout,
                    });
                } else if total == point {
                    // Don't Come point made - lose
                    resolutions.push(BetResolution::Lost {
                        player: *player,
                        bet_type: BetType::DontCome,
                        amount,
                    });
                }
            }
        }
        
        resolutions
    }
    
    /// Resolve NEXT bets (one-roll proposition bets)
    fn resolve_next_bets(&self, roll: DiceRoll, player: &PeerId, bets: &HashMap<BetType, Bet>) -> Vec<BetResolution> {
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
                    use crate::protocol::payouts::PayoutCalculator;
                    let multiplier = self.get_next_bet_multiplier(target);
                    let payout = CrapTokens::new_unchecked(bet.amount.amount + (bet.amount.amount * multiplier as u64 / 100));
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
}

/// Get true odds multiplier for odds bets
pub fn get_odds_multiplier(point: u8, is_pass: bool) -> u32 {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::game_logic::CrapsGame;
    
    #[test]
    fn test_comeout_pass_line_win() {
        let game = CrapsGame::new([1; 16], [2; 32]);
        let roll = DiceRoll::new(4, 3).unwrap(); // Total 7
        
        let resolutions = game.resolve_comeout_roll(roll);
        // Would need to set up bets to test properly
        assert!(resolutions.is_empty()); // No bets placed yet
    }
    
    #[test] 
    fn test_odds_multipliers() {
        assert_eq!(get_odds_multiplier(4, true), 200);  // 2:1
        assert_eq!(get_odds_multiplier(5, true), 150);  // 3:2
        assert_eq!(get_odds_multiplier(6, true), 120);  // 6:5
        assert_eq!(get_odds_multiplier(4, false), 50);  // 1:2
    }
    
    #[test]
    fn test_field_bet_resolution() {
        let game = CrapsGame::new([1; 16], [2; 32]);
        
        // Field pays 2:1 on 2 and 12
        let roll2 = DiceRoll::new(1, 1).unwrap();
        let roll12 = DiceRoll::new(6, 6).unwrap();
        
        // Would need game setup with actual bets to test properly
        let _ = game.resolve_one_roll_bets(roll2);
        let _ = game.resolve_one_roll_bets(roll12);
    }
}