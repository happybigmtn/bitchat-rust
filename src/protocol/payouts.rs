//! Payout calculations and special bet resolution for craps
//! 
//! This module contains all the payout multipliers and special
//! bet resolution logic (Fire, Repeater, Bonus bets, etc.)

use std::collections::{HashMap, HashSet};
use super::{PeerId, BetType, Bet, CrapTokens, DiceRoll};
use crate::protocol::bet_types::BetResolution;
use crate::protocol::game_logic::CrapsGame;

/// Trait for calculating payouts for different bet types
pub trait PayoutCalculator {
    /// Get payout multipliers matching the Hackathon contracts exactly
    fn get_yes_bet_multiplier(&self, target: u8) -> u32;
    
    fn get_no_bet_multiplier(&self, target: u8) -> u32;
    
    fn get_next_bet_multiplier(&self, target: u8) -> u32;
    
    fn get_repeater_multiplier(&self, number: u8) -> u32;
    
    /// Check if Fire bet wins (4-6 unique points made)
    fn resolve_fire_bet(&self, player: &PeerId, bet: &Bet) -> Option<BetResolution>;
    
    /// Check Repeater bets
    fn resolve_repeater_bets(&self, player: &PeerId, bets: &HashMap<BetType, Bet>) -> Vec<BetResolution>;
    
    /// Check Bonus Small/Tall/All bets
    fn resolve_bonus_bets(&self, player: &PeerId, bets: &HashMap<BetType, Bet>) -> Vec<BetResolution>;
    
    /// Resolve Hot Roller bet (progressive streak)
    fn resolve_hot_roller_bet(&self, player: &PeerId, bet: &Bet) -> Option<BetResolution>;
    
    /// Resolve Twice Hard bet (same hardway twice in a row)
    fn resolve_twice_hard_bet(&self, player: &PeerId, bet: &Bet) -> Option<BetResolution>;
    
    /// Resolve Ride the Line bet (pass line win streak)
    fn resolve_ride_line_bet(&self, player: &PeerId, bet: &Bet, pass_wins: u32) -> Option<BetResolution>;
    
    /// Resolve Muggsy bet (7 on comeout or point-7 combination)
    fn resolve_muggsy_bet(&self, player: &PeerId, bet: &Bet, last_two_rolls: &[DiceRoll]) -> Option<BetResolution>;
    
    /// Resolve Replay bet (same point 3+ times)
    fn resolve_replay_bet(&self, player: &PeerId, bet: &Bet, point_history: &[u8]) -> Option<BetResolution>;
    
    /// Resolve Different Doubles bet (unique doubles before 7)
    fn resolve_different_doubles_bet(&self, player: &PeerId, bet: &Bet, doubles_rolled: &HashSet<u8>) -> Option<BetResolution>;
}

impl PayoutCalculator for CrapsGame {
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
    fn resolve_fire_bet(&self, player: &PeerId, bet: &Bet) -> Option<BetResolution> {
        let unique_points = self.fire_points.len();
        
        match unique_points {
            4 => Some(BetResolution::Won {
                player: *player,
                bet_type: BetType::Fire,
                amount: bet.amount,
                payout: CrapTokens::new_unchecked(bet.amount.amount() * 25), // 24:1
            }),
            5 => Some(BetResolution::Won {
                player: *player,
                bet_type: BetType::Fire,
                amount: bet.amount,
                payout: CrapTokens::new_unchecked(bet.amount.amount() * 250), // 249:1
            }),
            6 => Some(BetResolution::Won {
                player: *player,
                bet_type: BetType::Fire,
                amount: bet.amount,
                payout: CrapTokens::new_unchecked(bet.amount.amount() * 1000), // 999:1
            }),
            _ => None, // Still active
        }
    }
    
    /// Check Repeater bets
    /// 
    /// Feynman: Repeater bets need a number to appear N times.
    /// Harder numbers (2, 12) need fewer repeats than easier ones (6, 8).
    fn resolve_repeater_bets(&self, player: &PeerId, bets: &HashMap<BetType, Bet>) -> Vec<BetResolution> {
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
                    let payout = CrapTokens::new_unchecked(bet.amount.amount() + (bet.amount.amount() * multiplier as u64 / 100));
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
    fn resolve_bonus_bets(&self, player: &PeerId, bets: &HashMap<BetType, Bet>) -> Vec<BetResolution> {
        let mut resolutions = Vec::new();
        
        // Bonus Small: All 2,3,4,5,6 before 7
        if let Some(bet) = bets.get(&BetType::BonusSmall) {
            let small_numbers: HashSet<u8> = [2, 3, 4, 5, 6].iter().copied().collect();
            if small_numbers.is_subset(&self.bonus_numbers) {
                let payout = CrapTokens::new_unchecked(bet.amount.amount() * 31); // 30:1
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
                let payout = CrapTokens::new_unchecked(bet.amount.amount() * 31); // 30:1
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
                let payout = CrapTokens::new_unchecked(bet.amount.amount() * 151); // 150:1
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
    fn resolve_hot_roller_bet(&self, player: &PeerId, bet: &Bet) -> Option<BetResolution> {
        use crate::protocol::bet_types::GamePhase;
        
        if self.roll_count > 20 && self.phase == GamePhase::Point {
            // Progressive payout based on roll streak
            let multiplier = match self.roll_count {
                20..=30 => 200,   // 2:1
                31..=40 => 500,   // 5:1
                41..=50 => 1000,  // 10:1
                _ => 2000,        // 20:1 for 50+ rolls
            };
            let payout = CrapTokens::new_unchecked((bet.amount.amount() * multiplier as u64) / 100);
            return Some(BetResolution::Won {
                player: *player,
                bet_type: BetType::HotRoller,
                amount: bet.amount,
                payout: CrapTokens::new_unchecked(bet.amount.amount() + payout.0),
            });
        }
        None
    }
    
    /// Resolve Twice Hard bet (same hardway twice in a row)
    /// 
    /// Feynman: Lightning striking twice - you need the exact same
    /// hardway number to come up twice consecutively. It's rare,
    /// so it pays well.
    fn resolve_twice_hard_bet(&self, player: &PeerId, bet: &Bet) -> Option<BetResolution> {
        // Check hardway streak tracker
        for (_number, &count) in &self.hardway_streak {
            if count >= 2 {
                let payout = CrapTokens::new_unchecked(bet.amount.amount() * 7); // 6:1 + original
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
    fn resolve_ride_line_bet(&self, player: &PeerId, bet: &Bet, pass_wins: u32) -> Option<BetResolution> {
        if pass_wins >= 3 {
            let multiplier = match pass_wins {
                3 => 300,   // 3:1
                4 => 500,   // 5:1
                5 => 1000,  // 10:1
                _ => 2500,  // 25:1 for 6+ wins
            };
            let payout = CrapTokens::new_unchecked((bet.amount.amount() * multiplier as u64) / 100);
            return Some(BetResolution::Won {
                player: *player,
                bet_type: BetType::RideLine,
                amount: bet.amount,
                payout: CrapTokens::new_unchecked(bet.amount.amount() + payout.0),
            });
        }
        None
    }
    
    /// Resolve Muggsy bet (7 on comeout or point-7 combination)
    /// 
    /// Feynman: Muggsy is a contrarian bet - you win if the shooter
    /// gets a natural 7 on comeout, then establishes a point and
    /// immediately sevens out. It's a specific sequence.
    fn resolve_muggsy_bet(&self, player: &PeerId, bet: &Bet, last_two_rolls: &[DiceRoll]) -> Option<BetResolution> {
        use crate::protocol::bet_types::GamePhase;
        
        if last_two_rolls.len() >= 2 {
            let prev = last_two_rolls[last_two_rolls.len() - 2].total();
            let curr = last_two_rolls[last_two_rolls.len() - 1].total();
            
            // Check for the Muggsy pattern
            if prev == 7 && self.phase == GamePhase::ComeOut {
                // Natural 7 on comeout followed by establishing point
                if curr >= 4 && curr <= 10 && curr != 7 {
                    let payout = CrapTokens::new_unchecked(bet.amount.amount() * 3); // 2:1 + original
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
    fn resolve_replay_bet(&self, player: &PeerId, bet: &Bet, point_history: &[u8]) -> Option<BetResolution> {
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
                let payout = CrapTokens::new_unchecked((bet.amount.amount() * multiplier as u64) / 100);
                return Some(BetResolution::Won {
                    player: *player,
                    bet_type: BetType::Replay,
                    amount: bet.amount,
                    payout: CrapTokens::new_unchecked(bet.amount.amount() + payout.0),
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
    fn resolve_different_doubles_bet(&self, player: &PeerId, bet: &Bet, doubles_rolled: &HashSet<u8>) -> Option<BetResolution> {
        let count = doubles_rolled.len();
        if count >= 2 {
            let multiplier = match count {
                2 => 600,   // 6:1
                3 => 2500,  // 25:1
                4 => 10000, // 100:1
                _ => 25000, // 250:1 for all 5
            };
            let payout = CrapTokens::new_unchecked((bet.amount.amount() * multiplier as u64) / 100);
            return Some(BetResolution::Won {
                player: *player,
                bet_type: BetType::DifferentDoubles,
                amount: bet.amount,
                payout: CrapTokens::new_unchecked(bet.amount.amount() + payout.0),
            });
        }
        None
    }
}

/// Utility functions for payout calculations
pub mod utils {
    use super::*;
    
    /// Calculate standard payout with multiplier
    pub fn calculate_payout(bet_amount: u64, multiplier: u32) -> CrapTokens {
        let payout_amount = bet_amount + (bet_amount * multiplier as u64 / 100);
        CrapTokens::new_unchecked(payout_amount)
    }
    
    /// Calculate odds payout for true odds bets
    pub fn calculate_odds_payout(bet_amount: u64, numerator: u32, denominator: u32) -> CrapTokens {
        let payout_amount = bet_amount + (bet_amount * numerator as u64 / denominator as u64);
        CrapTokens::new_unchecked(payout_amount)
    }
    
    /// Get the house edge for different bet types
    pub fn get_house_edge(bet_type: &BetType) -> f64 {
        match bet_type {
            BetType::Pass | BetType::DontPass => 1.36, // Very low house edge
            BetType::OddsPass | BetType::OddsDontPass => 0.0, // No house edge
            BetType::Come | BetType::DontCome => 1.36,
            BetType::Field => 2.78, // Moderate house edge
            BetType::Hard4 | BetType::Hard10 => 11.11, // High house edge
            BetType::Hard6 | BetType::Hard8 => 9.09,
            BetType::Yes4 | BetType::Yes10 => 6.67,
            BetType::Yes5 | BetType::Yes9 => 4.00,
            BetType::Yes6 | BetType::Yes8 => 1.52,
            BetType::Fire => 20.0, // Very high house edge, but huge payouts
            _ => 5.0, // Default moderate house edge
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::game_logic::CrapsGame;
    
    #[test]
    fn test_payout_multipliers() {
        let game = CrapsGame::new([1; 16], [2; 32]);
        
        // Test YES bet multipliers
        assert_eq!(game.get_yes_bet_multiplier(2), 588);  // 5.88:1
        assert_eq!(game.get_yes_bet_multiplier(4), 196);  // 1.96:1
        assert_eq!(game.get_yes_bet_multiplier(6), 118);  // 1.18:1
        
        // Test NO bet multipliers
        assert_eq!(game.get_no_bet_multiplier(2), 16);    // 0.16:1
        assert_eq!(game.get_no_bet_multiplier(4), 49);    // 0.49:1
        
        // Test NEXT bet multipliers
        assert_eq!(game.get_next_bet_multiplier(2), 3430); // 34.3:1
        assert_eq!(game.get_next_bet_multiplier(7), 490);  // 4.9:1
        
        // Test Repeater multipliers
        assert_eq!(game.get_repeater_multiplier(2), 4000); // 40:1
        assert_eq!(game.get_repeater_multiplier(6), 9000); // 90:1
    }
    
    #[test]
    fn test_fire_bet_resolution() {
        let mut game = CrapsGame::new([1; 16], [2; 32]);
        let player = [3; 32];
        let bet = Bet {
            id: [1; 16],
            game_id: [1; 16],
            player,
            bet_type: BetType::Fire,
            amount: CrapTokens::new_unchecked(100),
            timestamp: 0,
        };
        
        // No points made yet
        assert!(game.resolve_fire_bet(&player, &bet).is_none());
        
        // Add 4 unique fire points
        game.fire_points.insert(4);
        game.fire_points.insert(5);
        game.fire_points.insert(6);
        game.fire_points.insert(8);
        
        let resolution = game.resolve_fire_bet(&player, &bet);
        assert!(resolution.is_some());
        
        if let Some(BetResolution::Won { payout, .. }) = resolution {
            assert_eq!(payout, CrapTokens::new_unchecked(2500)); // 24:1 + original
        }
    }
    
    #[test]
    fn test_bonus_bet_resolution() {
        let mut game = CrapsGame::new([1; 16], [2; 32]);
        let player = [3; 32];
        let mut bets = HashMap::new();
        
        let bet = Bet {
            id: [1; 16],
            game_id: [1; 16],
            player,
            bet_type: BetType::BonusSmall,
            amount: CrapTokens::new_unchecked(100),
            timestamp: 0,
        };
        bets.insert(BetType::BonusSmall, bet);
        
        // Add all small numbers (2,3,4,5,6)
        game.bonus_numbers.insert(2);
        game.bonus_numbers.insert(3);
        game.bonus_numbers.insert(4);
        game.bonus_numbers.insert(5);
        game.bonus_numbers.insert(6);
        
        let resolutions = game.resolve_bonus_bets(&player, &bets);
        assert_eq!(resolutions.len(), 1);
        
        if let BetResolution::Won { payout, .. } = &resolutions[0] {
            assert_eq!(*payout, CrapTokens::new_unchecked(3100)); // 30:1 + original
        }
    }
    
    #[test]
    fn test_payout_utility_functions() {
        let payout = utils::calculate_payout(100, 200); // 2:1
        assert_eq!(payout, CrapTokens::new_unchecked(300));
        
        let odds_payout = utils::calculate_odds_payout(100, 3, 2); // 3:2
        assert_eq!(odds_payout, CrapTokens::new_unchecked(250));
        
        let house_edge = utils::get_house_edge(&BetType::Pass);
        assert_eq!(house_edge, 1.36);
    }
}