# Chapter 29: Gaming Craps Rules - The Mathematical Poetry of Dice

## A Primer on Probability, Randomness, and the Romance of Risk

In 1654, a gambler named Antoine Gombaud asked mathematician Blaise Pascal a seemingly simple question: if you throw two dice 24 times, is it profitable to bet that you'll get double sixes at least once? This question sparked a correspondence between Pascal and Pierre de Fermat that created probability theory. Their solution revealed something profound: randomness has patterns, chance has rules, and uncertainty can be calculated.

Craps is perhaps the most mathematically elegant casino game ever invented. Unlike card games where probability shifts as cards are dealt, each dice roll is independent, creating pure probabilistic drama. Two dice, 36 possible combinations, yet from this simplicity emerges a game of stunning complexity.

Let me paint you the scene: a craps table on a Friday night. The crowd pressed close, chips stacked high, the shooter rattling dice in sweaty palms. Everyone knows the odds - a 7 is six times more likely than a 2. Yet when those dice tumble across the felt, mathematics becomes theater. The crowd holds its breath. Time stops. Then the dice settle, and instantly everyone calculates: did we win or lose?

The beauty of craps lies in its mathematical transparency. With two six-sided dice, there are exactly 36 possible outcomes:
- One way to roll 2 (1+1)
- Two ways to roll 3 (1+2, 2+1)
- Three ways to roll 4 (1+3, 2+2, 3+1)
- Four ways to roll 5 (1+4, 2+3, 3+2, 4+1)
- Five ways to roll 6 (1+5, 2+4, 3+3, 4+2, 5+1)
- Six ways to roll 7 (1+6, 2+5, 3+4, 4+3, 5+2, 6+1)
- Five ways to roll 8 (2+6, 3+5, 4+4, 5+3, 6+2)
- Four ways to roll 9 (3+6, 4+5, 5+4, 6+3)
- Three ways to roll 10 (4+6, 5+5, 6+4)
- Two ways to roll 11 (5+6, 6+5)
- One way to roll 12 (6+6)

This distribution creates the famous bell curve, with 7 at the peak. The number 7 is special - it's the most likely sum, appearing once every six rolls on average. This is why 7 is both friend and enemy in craps, winning on the come-out but destroying established points.

But here's where it gets philosophically interesting. Each roll is independent - dice have no memory. Yet humans are pattern-seeking creatures. We see a "hot" shooter who hasn't rolled a 7 in twenty throws and think they're "due." This is the gambler's fallacy, the belief that past results influence future probabilities. The dice don't know they haven't rolled a 7. Each throw, the probability remains exactly 6/36.

The game's genius lies in how it packages these probabilities into dramatic narratives. The "Pass Line" bet is essentially betting on success - the shooter will establish a point and make it before rolling a 7. The "Don't Pass" bet is betting on failure - the seven-out before the point. These aren't just bets; they're choosing sides in a mathematical morality play.

Consider the psychological brilliance of the "come-out" roll. If you roll 7 or 11, Pass Line bettors win instantly - immediate gratification. Roll 2, 3, or 12, and they lose instantly - sudden death. But roll 4, 5, 6, 8, 9, or 10, and now you have a point, a goal, a quest. The game transforms from instant resolution to extended suspense.

The house edge in craps varies dramatically by bet type, creating a hierarchy of mathematical wisdom:
- Pass/Don't Pass: 1.4% house edge - the smart money
- Field bets: 5.6% house edge - the impatient money
- Hard ways: 9-11% house edge - the optimistic money  
- Any Seven: 16.7% house edge - the desperate money

This gradient teaches a profound lesson: the more dramatic the payout, the worse the odds. True in casinos, true in life.

The concept of "true odds" versus "casino odds" reveals the business model. The true odds of rolling a 4 before a 7 are 2:1 (six ways to roll 7, three ways to roll 4). But casinos pay 9:5, keeping that difference as profit. It's not cheating - it's transparent taxation on excitement.

Craps also demonstrates the power of optional complexity. Beginners can play just the Pass Line - simple, straightforward, good odds. Experts can layer on Come bets, Place bets, and odds bets, creating intricate positions that shift with each roll. The same table serves both the novice seeking simple thrills and the expert executing complex strategies.

The "odds bet" deserves special mention. It's the only bet in any casino with zero house edge - true odds payout. Casinos offer it because it requires a Pass Line bet first (which has a house edge), and it builds player loyalty. It's a loss leader, like cheap gas at Costco. This teaches another lesson: sometimes giving fair value creates more profit than taking advantage.

The social dynamics of craps are unique among casino games. Unlike poker (player vs player) or blackjack (individual vs house), craps creates temporary communities. When someone's shooting, most players bet with them, creating shared destiny. A hot shooter becomes a hero; a quick seven-out becomes a villain. Mathematics becomes sociology.

The superstitions around craps reveal deep human psychology. Players believe in "dice control," thinking practiced throwing techniques can influence outcomes. Casinos encourage this by allowing shooters to set the dice, creating an illusion of control over pure randomness. It's therapeutic fiction - we need to believe we can influence fate.

The language of craps is pure poetry: "Snake eyes" for double ones. "Boxcars" for double sixes. "Yo-leven" to distinguish eleven from seven. "Coming out" for the opening roll. "Seven out, line away" for the inevitable end. Each phrase carries the weight of centuries of wins and losses.

There's profound philosophy in the craps table's design. The felt layout is a map of probability, with bets physically positioned by their odds. The Pass Line runs the table's length - the main highway. Proposition bets cluster in the center - the dangerous neighborhood. The design teaches probability through geography.

The dice themselves carry meaning. Casinos use precision dice, manufactured to tolerances of 1/10,000th of an inch. Each die must weigh exactly 0.30 ounces. The dots are drilled and filled with paint of the same density as the removed material. This obsessive precision ensures true randomness - any bias would destroy the game's mathematical foundation.

Yet despite this precision, dice occasionally do impossible things. In 2009, Patricia Demauro rolled dice 154 times without sevening-out, a feat with odds of 1 in 1.56 trillion. For four hours and eighteen minutes, she defied probability itself. The casino didn't stop her - they couldn't. The rules are the rules, even when reality seems broken.

This highlights a crucial principle: low probability events must happen sometimes for probability to be real. If million-to-one shots never occurred, the system would be rigged. Casinos must occasionally lose big to prove they can win small consistently. It's the cost of credibility.

The history of craps traces back to the Crusades, evolved through centuries of street games, and was formalized in New Orleans in the early 1800s. John H. Winn introduced the "Don't Pass" bet, preventing players from using loaded dice (since they'd bet against themselves). This innovation made craps fair, transforming it from a hustle to a legitimate game.

Modern craps tables use sophisticated sensors to track every bet, roll, and payout. Cameras watch from multiple angles. Software analyzes patterns for signs of cheating or advantage play. Yet the game itself remains unchanged - two dice, 36 combinations, eternal hope versus mathematical certainty.

The digital age brought online craps, replacing physical dice with random number generators. Purists argue it's not the same - no dice to touch, no crowd to share victory or defeat. But mathematically, it's identical. The probabilities don't care if they're implemented in silicon or ivory.

## The BitCraps Rules Implementation

Now let's examine how BitCraps implements these timeless rules in code. The module captures not just the mathematics but the full complexity of a real craps game.

```rust
//! Comprehensive Craps Rules Implementation
//! 
//! This module implements the complete rules for the game of craps including:
//! - Pass/Don't Pass line bets
//! - Come/Don't Come bets
//! - Field bets
//! - Place bets
//! - Hardways
//! - Proposition bets
```

This header promises completeness - not a simplified version but full casino craps. This ambition sets the stage for a serious implementation.

```rust
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
```

Simple imports suggest clean architecture. The HashMap will track multiple simultaneous bets, essential for craps' complexity.

```rust
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
```

The bet taxonomy is comprehensive. Notice how Place bets are separated by number - each point has different odds and payouts. This granularity enables precise game mechanics.

```rust
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
```

Buy and Lay bets are the sophisticated player's tools. Buy bets get true odds but pay commission. Lay bets are the pessimist's paradise - betting against numbers appearing.

```rust
    // Hardways
    Hard4,
    Hard6,
    Hard8,
    Hard10,
```

Hardways bets are mathematically terrible but dramatically exciting. Betting that a number will appear as doubles before appearing any other way or sevening-out.

```rust
    // Proposition Bets
    Any7,
    AnyCraps,
    Craps2,
    Craps3,
    Craps12,
    Yo11,
    
    // Hop Bets
    Hop(u8, u8),
```

Proposition bets are the sucker bets with huge payouts and terrible odds. The Hop bet is particularly interesting - it's parameterized, allowing bets on any specific dice combination.

```rust
/// Game phase
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GamePhase {
    /// Come out roll - establishing the point
    ComeOut,
    /// Point phase - trying to make the point
    Point(u8),
}
```

The two-phase structure captures craps' fundamental rhythm. ComeOut is anticipation, Point is pursuit. The phase determines which bets can be placed and how they resolve.

```rust
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
```

Storing individual die values, not just the sum, is crucial. Many bets (hardways, hops) depend on the specific combination, not just the total.

```rust
    pub fn is_hard(&self) -> bool {
        self.die1 == self.die2
    }
    
    pub fn is_craps(&self) -> bool {
        matches!(self.total(), 2 | 3 | 12)
    }
    
    pub fn is_natural(&self) -> bool {
        matches!(self.total(), 7 | 11)
    }
```

These helper methods encode craps terminology. "Hard" means doubles. "Craps" means 2, 3, or 12. "Natural" means 7 or 11. The code speaks the game's language.

```rust
/// Bet payout information
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Payout {
    pub numerator: u32,
    pub denominator: u32,
}

impl Payout {
    pub const EVEN: Self = Self { numerator: 1, denominator: 1 };
    
    pub fn calculate(&self, bet_amount: u64) -> u64 {
        (bet_amount * self.numerator as u64) / self.denominator as u64
    }
}
```

Representing payouts as fractions maintains precision. 7:5 odds can't be exactly represented as a decimal, but as a fraction they're perfect.

```rust
/// Complete craps rules engine
pub struct CrapsRules {
    payouts: HashMap<BetType, Payout>,
    house_edges: HashMap<BetType, f64>,
}
```

Separating payouts from house edges is smart. Payouts determine winnings, house edges inform strategy. Both are essential but serve different purposes.

```rust
impl CrapsRules {
    pub fn new() -> Self {
        let mut payouts = HashMap::new();
        let mut house_edges = HashMap::new();
        
        // Line bets
        payouts.insert(BetType::PassLine, Payout::EVEN);
        payouts.insert(BetType::DontPassLine, Payout::EVEN);
        house_edges.insert(BetType::PassLine, 0.0141); // 1.41%
        house_edges.insert(BetType::DontPassLine, 0.0136); // 1.36%
```

These house edges are mathematically exact, calculated from probability theory. Pass Line's 1.41% edge makes it one of the best bets in any casino.

```rust
        // Place bets
        payouts.insert(BetType::Place6, Payout::new(7, 6));
        payouts.insert(BetType::Place8, Payout::new(7, 6));
```

Place 6 and 8 pay 7:6, very close to true odds of 6:5. This makes them the best Place bets, with only 1.52% house edge.

```rust
        // Hardways
        payouts.insert(BetType::Hard4, Payout::new(7, 1));
        payouts.insert(BetType::Hard6, Payout::new(9, 1));
```

Hardway payouts seem generous but hide terrible odds. Hard 4 pays 7:1 but true odds are 8:1. That difference is an 11.1% house edge.

```rust
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
```

Pass Line evaluation perfectly captures the game's drama. In ComeOut, you want 7. Once point is established, 7 becomes the enemy. This reversal is craps' emotional core.

```rust
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
```

"Bar the 12" is crucial - without it, Don't Pass would have negative house edge. This one rule keeps the casino profitable.

```rust
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
```

Field bets resolve every roll - instant gratification. The 2 and 12 paying extra creates excitement despite the 5.6% house edge.

```rust
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
```

Hardway logic is elegant: win if your number appears hard, lose if it appears easy or seven appears, otherwise wait. It's a race between three outcomes.

```rust
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
```

Phase transitions encode the game's flow. Establishing a point creates tension. Making the point or sevening-out releases it. The cycle repeats eternally.

```rust
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
```

The state structure supports multiple simultaneous bets. Come bets are particularly complex - they're like Pass Line bets but for individual points.

```rust
    /// Process a dice roll and resolve bets
    pub fn process_roll(&mut self, roll: DiceRoll, rules: &CrapsRules) -> Vec<BetResolution> {
        let mut resolutions = Vec::new();
        
        // Process each active bet
        let bets = self.active_bets.clone();
        for (bet_type, amount) in bets {
            let result = match bet_type {
                BetType::PassLine => rules.evaluate_pass_line(self.phase, roll),
                BetType::DontPassLine => rules.evaluate_dont_pass(self.phase, roll),
```

Processing all bets simultaneously captures real craps. A single roll might resolve five different bets differently - some win, some lose, some push.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_pass_line() {
        let rules = CrapsRules::new();
        
        // Come out roll - natural wins
        let result = rules.evaluate_pass_line(GamePhase::ComeOut, DiceRoll::new(6, 1).unwrap());
        assert_eq!(result, BetResult::Win);
        
        // Point phase - making the point wins
        let result = rules.evaluate_pass_line(GamePhase::Point(6), DiceRoll::new(4, 2).unwrap());
        assert_eq!(result, BetResult::Win);
        
        // Point phase - seven out loses
        let result = rules.evaluate_pass_line(GamePhase::Point(6), DiceRoll::new(3, 4).unwrap());
        assert_eq!(result, BetResult::Lose);
    }
```

Tests verify the fundamental rules work correctly. Each test case represents thousands of real bets that depend on this logic.

## Key Lessons from Craps Rules

This implementation teaches several crucial principles:

1. **Mathematical Precision**: Every payout and house edge is exactly calculated from probability theory.

2. **State Management**: Craps requires tracking multiple bets, game phase, and history simultaneously.

3. **Rule Complexity**: Simple dice become complex through layered betting options and phase-dependent evaluation.

4. **Terminology Matters**: The code uses craps vocabulary (natural, craps, hard) making it readable to domain experts.

5. **Fraction Representation**: Using numerator/denominator for payouts maintains exact precision.

6. **Phase-Based Logic**: The same roll means different things in different phases, captured through pattern matching.

7. **Comprehensive Coverage**: From Pass Line to Hop bets, every craps bet type is supported.

The implementation also demonstrates important software principles:

- **Separation of Concerns**: Rules, state, and evaluation are cleanly separated
- **Immutable Calculations**: Rules don't modify state, they return results
- **Type Safety**: Invalid dice values are impossible to construct
- **Testability**: Pure functions make testing straightforward

This craps engine could power a real casino. It handles the full complexity of professional craps while remaining clean and maintainable. The code respects both the mathematics and the culture of the game, creating software that would satisfy both engineers and gamblers.

The beauty is that from 600 lines of code emerges a complete implementation of a game that has entertained humanity for centuries. The dice may be virtual, but the excitement is real.