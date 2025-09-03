//! Comprehensive Craps Rules Implementation
//!
//! This module implements the complete rules for the game of craps including:
//! - Pass/Don't Pass line bets
//! - Come/Don't Come bets
//! - Field bets
//! - Place bets
//! - Hardways
//! - Proposition bets

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Complete bet types in craps
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BetType {
    // Line Bets
    PassLine,
    DontPassLine,

    // Come Bets
    Come,
    DontCome,

    // Field Bet
    Field,

    // Place Bets
    Place4,
    Place5,
    Place6,
    Place8,
    Place9,
    Place10,

    // Buy/Lay Bets
    Buy4,
    Buy5,
    Buy6,
    Buy8,
    Buy9,
    Buy10,
    Lay4,
    Lay5,
    Lay6,
    Lay8,
    Lay9,
    Lay10,

    // Hardways
    Hard4,
    Hard6,
    Hard8,
    Hard10,

    // Proposition Bets
    Any7,
    AnyCraps,
    Craps2,
    Craps3,
    Craps12,
    Yo11,

    // Hop Bets
    Hop(u8, u8),

    // Big 6/8
    Big6,
    Big8,
}

/// Game phase
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GamePhase {
    /// Come out roll - establishing the point
    ComeOut,
    /// Point phase - trying to make the point
    Point(u8),
}

/// Dice roll result
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiceRoll {
    pub die1: u8,
    pub die2: u8,
}

impl DiceRoll {
    pub fn new(die1: u8, die2: u8) -> Result<Self> {
        if die1 < 1 || die1 > 6 || die2 < 1 || die2 > 6 {
            return Err(Error::InvalidInput("Invalid dice values".to_string()));
        }
        Ok(Self { die1, die2 })
    }

    pub fn total(&self) -> u8 {
        self.die1 + self.die2
    }

    pub fn is_hard(&self) -> bool {
        self.die1 == self.die2
    }

    pub fn is_craps(&self) -> bool {
        matches!(self.total(), 2 | 3 | 12)
    }

    pub fn is_natural(&self) -> bool {
        matches!(self.total(), 7 | 11)
    }
}

/// Bet payout information
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Payout {
    pub numerator: u32,
    pub denominator: u32,
}

impl Payout {
    pub const EVEN: Self = Self { numerator: 1, denominator: 1 };

    pub fn new(numerator: u32, denominator: u32) -> Self {
        Self { numerator, denominator }
    }

    pub fn calculate(&self, bet_amount: u64) -> u64 {
        (bet_amount * self.numerator as u64) / self.denominator as u64
    }
}

/// Complete craps rules engine
pub struct CrapsRules {
    payouts: HashMap<BetType, Payout>,
    house_edges: HashMap<BetType, f64>,
}

impl Default for CrapsRules {
    fn default() -> Self {
        Self::new()
    }
}

impl CrapsRules {
    pub fn new() -> Self {
        let mut payouts = HashMap::new();
        let mut house_edges = HashMap::new();

        // Line bets
        payouts.insert(BetType::PassLine, Payout::EVEN);
        payouts.insert(BetType::DontPassLine, Payout::EVEN);
        house_edges.insert(BetType::PassLine, 0.0141); // 1.41%
        house_edges.insert(BetType::DontPassLine, 0.0136); // 1.36%

        // Come bets
        payouts.insert(BetType::Come, Payout::EVEN);
        payouts.insert(BetType::DontCome, Payout::EVEN);
        house_edges.insert(BetType::Come, 0.0141);
        house_edges.insert(BetType::DontCome, 0.0136);

        // Field bet
        payouts.insert(BetType::Field, Payout::EVEN); // 2 and 12 pay double/triple
        house_edges.insert(BetType::Field, 0.0556); // 5.56%

        // Place bets
        payouts.insert(BetType::Place4, Payout::new(9, 5));
        payouts.insert(BetType::Place5, Payout::new(7, 5));
        payouts.insert(BetType::Place6, Payout::new(7, 6));
        payouts.insert(BetType::Place8, Payout::new(7, 6));
        payouts.insert(BetType::Place9, Payout::new(7, 5));
        payouts.insert(BetType::Place10, Payout::new(9, 5));

        house_edges.insert(BetType::Place4, 0.0667);
        house_edges.insert(BetType::Place5, 0.0400);
        house_edges.insert(BetType::Place6, 0.0152);
        house_edges.insert(BetType::Place8, 0.0152);
        house_edges.insert(BetType::Place9, 0.0400);
        house_edges.insert(BetType::Place10, 0.0667);

        // Buy bets (true odds minus 5% commission)
        payouts.insert(BetType::Buy4, Payout::new(2, 1));
        payouts.insert(BetType::Buy5, Payout::new(3, 2));
        payouts.insert(BetType::Buy6, Payout::new(6, 5));
        payouts.insert(BetType::Buy8, Payout::new(6, 5));
        payouts.insert(BetType::Buy9, Payout::new(3, 2));
        payouts.insert(BetType::Buy10, Payout::new(2, 1));

        // Lay bets (opposite of buy)
        payouts.insert(BetType::Lay4, Payout::new(1, 2));
        payouts.insert(BetType::Lay5, Payout::new(2, 3));
        payouts.insert(BetType::Lay6, Payout::new(5, 6));
        payouts.insert(BetType::Lay8, Payout::new(5, 6));
        payouts.insert(BetType::Lay9, Payout::new(2, 3));
        payouts.insert(BetType::Lay10, Payout::new(1, 2));

        // Hardways
        payouts.insert(BetType::Hard4, Payout::new(7, 1));
        payouts.insert(BetType::Hard6, Payout::new(9, 1));
        payouts.insert(BetType::Hard8, Payout::new(9, 1));
        payouts.insert(BetType::Hard10, Payout::new(7, 1));

        house_edges.insert(BetType::Hard4, 0.1111);
        house_edges.insert(BetType::Hard6, 0.0909);
        house_edges.insert(BetType::Hard8, 0.0909);
        house_edges.insert(BetType::Hard10, 0.1111);

        // Proposition bets
        payouts.insert(BetType::Any7, Payout::new(4, 1));
        payouts.insert(BetType::AnyCraps, Payout::new(7, 1));
        payouts.insert(BetType::Craps2, Payout::new(30, 1));
        payouts.insert(BetType::Craps3, Payout::new(15, 1));
        payouts.insert(BetType::Craps12, Payout::new(30, 1));
        payouts.insert(BetType::Yo11, Payout::new(15, 1));

        house_edges.insert(BetType::Any7, 0.1667);
        house_edges.insert(BetType::AnyCraps, 0.1111);

        // Big 6/8
        payouts.insert(BetType::Big6, Payout::EVEN);
        payouts.insert(BetType::Big8, Payout::EVEN);
        house_edges.insert(BetType::Big6, 0.0909);
        house_edges.insert(BetType::Big8, 0.0909);

        Self { payouts, house_edges }
    }

    /// Evaluate pass line bet
    pub fn evaluate_pass_line(&self, phase: GamePhase, roll: DiceRoll) -> BetResult {
        match phase {
            GamePhase::ComeOut => {
                match roll.total() {
                    7 | 11 => BetResult::Win,
                    2 | 3 | 12 => BetResult::Lose,
                    _ => BetResult::Push,
                }
            }
            GamePhase::Point(point) => {
                match roll.total() {
                    total if total == point => BetResult::Win,
                    7 => BetResult::Lose,
                    _ => BetResult::Push,
                }
            }
        }
    }

    /// Evaluate don't pass bet
    pub fn evaluate_dont_pass(&self, phase: GamePhase, roll: DiceRoll) -> BetResult {
        match phase {
            GamePhase::ComeOut => {
                match roll.total() {
                    2 | 3 => BetResult::Win,
                    7 | 11 => BetResult::Lose,
                    12 => BetResult::Push, // Bar the 12
                    _ => BetResult::Push,
                }
            }
            GamePhase::Point(point) => {
                match roll.total() {
                    7 => BetResult::Win,
                    total if total == point => BetResult::Lose,
                    _ => BetResult::Push,
                }
            }
        }
    }

    /// Evaluate field bet
    pub fn evaluate_field(&self, roll: DiceRoll) -> (BetResult, Payout) {
        match roll.total() {
            2 => (BetResult::Win, Payout::new(2, 1)), // Double
            3 | 4 | 9 | 10 | 11 => (BetResult::Win, Payout::EVEN),
            12 => (BetResult::Win, Payout::new(3, 1)), // Triple
            5 | 6 | 7 | 8 => (BetResult::Lose, Payout::EVEN),
            _ => (BetResult::Push, Payout::EVEN),
        }
    }

    /// Evaluate place bet
    pub fn evaluate_place(&self, number: u8, roll: DiceRoll) -> BetResult {
        match roll.total() {
            total if total == number => BetResult::Win,
            7 => BetResult::Lose,
            _ => BetResult::Push,
        }
    }

    /// Evaluate hardway bet
    pub fn evaluate_hardway(&self, number: u8, roll: DiceRoll) -> BetResult {
        let total = roll.total();

        if total == number && roll.is_hard() {
            BetResult::Win
        } else if total == 7 || (total == number && !roll.is_hard()) {
            BetResult::Lose
        } else {
            BetResult::Push
        }
    }

    /// Evaluate proposition bet
    pub fn evaluate_proposition(&self, bet_type: BetType, roll: DiceRoll) -> BetResult {
        let total = roll.total();

        match bet_type {
            BetType::Any7 => {
                if total == 7 { BetResult::Win } else { BetResult::Lose }
            }
            BetType::AnyCraps => {
                if roll.is_craps() { BetResult::Win } else { BetResult::Lose }
            }
            BetType::Craps2 => {
                if total == 2 { BetResult::Win } else { BetResult::Lose }
            }
            BetType::Craps3 => {
                if total == 3 { BetResult::Win } else { BetResult::Lose }
            }
            BetType::Craps12 => {
                if total == 12 { BetResult::Win } else { BetResult::Lose }
            }
            BetType::Yo11 => {
                if total == 11 { BetResult::Win } else { BetResult::Lose }
            }
            _ => BetResult::Push,
        }
    }

    /// Get payout for a bet type
    pub fn get_payout(&self, bet_type: BetType) -> Option<Payout> {
        self.payouts.get(&bet_type).copied()
    }

    /// Get house edge for a bet type
    pub fn get_house_edge(&self, bet_type: BetType) -> Option<f64> {
        self.house_edges.get(&bet_type).copied()
    }

    /// Calculate commission for buy/lay bets
    pub fn calculate_commission(&self, bet_amount: u64, bet_type: BetType) -> u64 {
        match bet_type {
            BetType::Buy4 | BetType::Buy5 | BetType::Buy6 |
            BetType::Buy8 | BetType::Buy9 | BetType::Buy10 |
            BetType::Lay4 | BetType::Lay5 | BetType::Lay6 |
            BetType::Lay8 | BetType::Lay9 | BetType::Lay10 => {
                // 5% commission
                (bet_amount * 5) / 100
            }
            _ => 0,
        }
    }

    /// Update game phase based on roll
    pub fn update_phase(&self, current_phase: GamePhase, roll: DiceRoll) -> GamePhase {
        match current_phase {
            GamePhase::ComeOut => {
                match roll.total() {
                    4 | 5 | 6 | 8 | 9 | 10 => GamePhase::Point(roll.total()),
                    _ => GamePhase::ComeOut,
                }
            }
            GamePhase::Point(point) => {
                match roll.total() {
                    7 => GamePhase::ComeOut,
                    total if total == point => GamePhase::ComeOut,
                    _ => GamePhase::Point(point),
                }
            }
        }
    }
}

/// Bet resolution result
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BetResult {
    Win,
    Lose,
    Push,
}

/// Complete game state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrapsGameState {
    pub phase: GamePhase,
    pub active_bets: HashMap<BetType, u64>,
    pub come_bets: HashMap<u8, u64>, // Point -> Amount
    pub dont_come_bets: HashMap<u8, u64>,
    pub total_wagered: u64,
    pub total_won: u64,
}

impl Default for CrapsGameState {
    fn default() -> Self {
        Self::new()
    }
}

impl CrapsGameState {
    pub fn new() -> Self {
        Self {
            phase: GamePhase::ComeOut,
            active_bets: HashMap::new(),
            come_bets: HashMap::new(),
            dont_come_bets: HashMap::new(),
            total_wagered: 0,
            total_won: 0,
        }
    }

    /// Place a new bet
    pub fn place_bet(&mut self, bet_type: BetType, amount: u64) -> Result<()> {
        // Validate bet based on phase
        match (self.phase, bet_type) {
            (GamePhase::ComeOut, BetType::PassLine | BetType::DontPassLine) => {},
            (GamePhase::Point(_), BetType::Come | BetType::DontCome) => {},
            (_, BetType::Field | BetType::Any7 | BetType::AnyCraps |
                BetType::Craps2 | BetType::Craps3 | BetType::Craps12 | BetType::Yo11) => {},
            (GamePhase::Point(_), BetType::Place4 | BetType::Place5 | BetType::Place6 |
                                  BetType::Place8 | BetType::Place9 | BetType::Place10) => {},
            _ => return Err(Error::InvalidInput("Invalid bet for current phase".to_string())),
        }

        *self.active_bets.entry(bet_type).or_insert(0) += amount;
        self.total_wagered += amount;
        Ok(())
    }

    /// Process a dice roll and resolve bets
    pub fn process_roll(&mut self, roll: DiceRoll, rules: &CrapsRules) -> Vec<BetResolution> {
        let mut resolutions = Vec::new();

        // Process each active bet
        let bets = self.active_bets.clone();
        for (bet_type, amount) in bets {
            let result = match bet_type {
                BetType::PassLine => rules.evaluate_pass_line(self.phase, roll),
                BetType::DontPassLine => rules.evaluate_dont_pass(self.phase, roll),
                BetType::Field => {
                    let (result, payout) = rules.evaluate_field(roll);
                    if result == BetResult::Win {
                        let win_amount = payout.calculate(amount);
                        resolutions.push(BetResolution {
                            bet_type,
                            result,
                            amount,
                            payout: Some(win_amount),
                        });
                        self.total_won += win_amount;
                    }
                    result
                }
                BetType::Place4 => rules.evaluate_place(4, roll),
                BetType::Place5 => rules.evaluate_place(5, roll),
                BetType::Place6 => rules.evaluate_place(6, roll),
                BetType::Place8 => rules.evaluate_place(8, roll),
                BetType::Place9 => rules.evaluate_place(9, roll),
                BetType::Place10 => rules.evaluate_place(10, roll),
                BetType::Hard4 => rules.evaluate_hardway(4, roll),
                BetType::Hard6 => rules.evaluate_hardway(6, roll),
                BetType::Hard8 => rules.evaluate_hardway(8, roll),
                BetType::Hard10 => rules.evaluate_hardway(10, roll),
                bet => rules.evaluate_proposition(bet, roll),
            };

            // Handle standard win/lose/push
            match result {
                BetResult::Win => {
                    if let Some(payout) = rules.get_payout(bet_type) {
                        let win_amount = payout.calculate(amount);
                        resolutions.push(BetResolution {
                            bet_type,
                            result,
                            amount,
                            payout: Some(win_amount),
                        });
                        self.total_won += win_amount + amount;
                    }
                    self.active_bets.remove(&bet_type);
                }
                BetResult::Lose => {
                    resolutions.push(BetResolution {
                        bet_type,
                        result,
                        amount,
                        payout: None,
                    });
                    self.active_bets.remove(&bet_type);
                }
                BetResult::Push => {
                    // Bet stays active
                }
            }
        }

        // Update phase
        self.phase = rules.update_phase(self.phase, roll);

        resolutions
    }
}

/// Individual bet resolution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BetResolution {
    pub bet_type: BetType,
    pub result: BetResult,
    pub amount: u64,
    pub payout: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dice_roll() {
        let roll = DiceRoll::new(3, 4).unwrap();
        assert_eq!(roll.total(), 7);
        assert!(!roll.is_hard());
        assert!(roll.is_natural());
    }

    #[test]
    fn test_pass_line() {
        let rules = CrapsRules::new();

        // Come out roll - natural wins
        let result = rules.evaluate_pass_line(GamePhase::ComeOut, DiceRoll::new(6, 1).unwrap());
        assert_eq!(result, BetResult::Win);

        // Come out roll - craps loses
        let result = rules.evaluate_pass_line(GamePhase::ComeOut, DiceRoll::new(1, 1).unwrap());
        assert_eq!(result, BetResult::Lose);

        // Point phase - making the point wins
        let result = rules.evaluate_pass_line(GamePhase::Point(6), DiceRoll::new(4, 2).unwrap());
        assert_eq!(result, BetResult::Win);

        // Point phase - seven out loses
        let result = rules.evaluate_pass_line(GamePhase::Point(6), DiceRoll::new(3, 4).unwrap());
        assert_eq!(result, BetResult::Lose);
    }

    #[test]
    fn test_field_bet() {
        let rules = CrapsRules::new();

        // Field win on 3
        let (result, _payout) = rules.evaluate_field(DiceRoll::new(1, 2).unwrap());
        assert_eq!(result, BetResult::Win);

        // Field lose on 7
        let (result, _payout) = rules.evaluate_field(DiceRoll::new(3, 4).unwrap());
        assert_eq!(result, BetResult::Lose);

        // Field double on 2
        let (result, payout) = rules.evaluate_field(DiceRoll::new(1, 1).unwrap());
        assert_eq!(result, BetResult::Win);
        assert_eq!(payout.numerator, 2);
    }
}