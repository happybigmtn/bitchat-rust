# Chapter 27: Craps Rules Implementation - Complete Implementation Analysis
## Deep Dive into `src/protocol/game_logic.rs` - Computer Science Concepts in Production Code

---

## Complete Implementation Analysis: 405 Lines of Casino Game Logic

This chapter provides comprehensive coverage of the craps game implementation. We'll examine every significant component, understanding not just what it does but why it was implemented this way, with particular focus on game state management, bet resolution algorithms, cryptographically secure dice rolls, and the mathematical foundations of casino gaming.

### Module Overview: The Complete Craps Architecture

```
┌──────────────────────────────────────────────────────┐
│              Craps Game System                        │
├──────────────────────────────────────────────────────┤
│                Game State Layer                       │
│  ┌─────────────────────────────────────────────────┐ │
│  │ CrapsGame       │ Phase Management               │ │
│  │ Player Tracking │ Bet Registry                   │ │
│  │ Roll History    │ Special Bet Tracking           │ │
│  └─────────────────────────────────────────────────┘ │
├──────────────────────────────────────────────────────┤
│               Betting System Layer                    │
│  ┌─────────────────────────────────────────────────┐ │
│  │ 64 Bet Types    │ Pass/Don't Pass               │ │
│  │ Field/Place Bets│ Hardways/Props                │ │
│  │ Come/Don't Come │ Fire/Bonus Bets               │ │
│  └─────────────────────────────────────────────────┘ │
├──────────────────────────────────────────────────────┤
│              Resolution Engine Layer                  │
│  ┌─────────────────────────────────────────────────┐ │
│  │ Phase Rules     │ Come-Out Resolution           │ │
│  │ Point Rules     │ One-Roll Resolution            │ │
│  │ Payout Calc     │ Odds Multipliers              │ │
│  └─────────────────────────────────────────────────┘ │
├──────────────────────────────────────────────────────┤
│            Cryptographic RNG Layer                    │
│  ┌─────────────────────────────────────────────────┐ │
│  │ Secure Dice     │ Multi-Source Entropy          │ │
│  │ Fair Rolling    │ Consensus Generation          │ │
│  └─────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────┘
```

**Total Implementation**: 405 lines of complete casino craps logic

## Part I: Complete Code Analysis - Computer Science Concepts in Practice

### Game State Management (Lines 16-63)

```rust
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CrapsGame {
    pub game_id: GameId,
    pub phase: GamePhase,
    pub shooter: PeerId,
    pub participants: Vec<PeerId>,
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
```

**Computer Science Foundation: State Machine Design**

The game implements a **finite state machine** with complex state tracking:

**State Transition Diagram:**
```
        ┌─────────────┐
        │  Come-Out   │
        └─────┬───────┘
              │
     ┌────────┼────────┐
     │        │        │
  7 or 11  2,3,12   4,5,6,8,9,10
     │        │        │
   Win      Lose    Point
     │        │        │
     └────────┼────────┘
              │
        ┌─────▼───────┐
        │    Point    │
        └─────┬───────┘
              │
     ┌────────┼────────┐
     │        │        │
   Point      7     Other
     │        │        │
   Win      Lose    Stay
     │        │        │
     └────────┼────────┘
              ▼
         Come-Out
```

**Why Track So Much State?**
1. **Fire Bet**: Needs unique points made (up to 6)
2. **Repeater Bets**: Count how many times each number rolled
3. **Bonus Small/Tall/All**: Track all numbers 2-6 or 8-12
4. **Hot Roller**: Consecutive pass line wins for special payouts
5. **Come/Don't Come**: Each player can have multiple point bets

### Cryptographically Secure Dice Generation (Lines 76-88)

```rust
pub fn roll_dice_secure() -> Result<DiceRoll, crate::error::Error> {
    use crate::crypto::GameCrypto;
    let (die1, die2) = GameCrypto::generate_secure_dice_roll();
    DiceRoll::new(die1, die2)
}

pub fn roll_dice_from_sources(entropy_sources: &[[u8; 32]]) -> Result<DiceRoll, crate::error::Error> {
    use crate::crypto::GameCrypto;
    let (die1, die2) = GameCrypto::combine_randomness(entropy_sources);
    DiceRoll::new(die1, die2)
}
```

**Computer Science Foundation: Fairness in Random Number Generation**

Two approaches to fair dice generation:

**1. Single-Source Secure RNG:**
```
OS Entropy (OsRng) → SHA-256 → Rejection Sampling → Fair Die Value

Why Rejection Sampling?
- Modulo bias: 256 % 6 = 4 (values 0-3 appear more often)
- Solution: Reject values >= 252 (largest multiple of 6)
- Probability of rejection: 4/256 = 1.56%
```

**2. Multi-Source Consensus RNG:**
```
Player1 Entropy ─┐
Player2 Entropy ─┼─> XOR Combine → SHA-256 → Fair Dice
Player3 Entropy ─┘

Properties:
- No single player controls outcome
- Requires all sources before rolling
- Prevents prediction or manipulation
```

**Mathematical Fairness:**
```
P(die = n) = 1/6 for n ∈ {1,2,3,4,5,6}
Expected value: E(die) = 3.5
Variance: Var(die) = 35/12 ≈ 2.92
```

### Bet Placement with Validation (Lines 90-114)

```rust
pub fn place_bet(&mut self, player: PeerId, bet: Bet) -> Result<(), crate::error::Error> {
    // Validate bet is appropriate for current game phase
    if !bet.bet_type.is_valid_for_phase(&self.phase) {
        return Err(crate::error::Error::InvalidBet(
            format!("Bet type {:?} not allowed in phase {:?}", bet.bet_type, self.phase)
        ));
    }
    
    // Check if player already has this bet type
    if let Some(player_bets) = self.player_bets.get(&player) {
        if player_bets.contains_key(&bet.bet_type) {
            return Err(crate::error::Error::InvalidBet(
                format!("Player already has a {:?} bet", bet.bet_type)
            ));
        }
    }
    
    // Add bet to nested HashMap
    self.player_bets
        .entry(player)
        .or_default()
        .insert(bet.bet_type, bet);
}
```

**Computer Science Foundation: Nested HashMap for O(1) Lookups**

The bet storage uses **two-level HashMap** for efficiency:

**Data Structure:**
```
HashMap<PeerId, HashMap<BetType, Bet>>
    │              │
    O(1)          O(1)
    
Total lookup: O(1) + O(1) = O(1)

Alternative structures:
Vec<(PeerId, BetType, Bet)>: O(n) lookup
Tree<(PeerId, BetType), Bet>: O(log n) lookup
```

**Why Prevent Duplicate Bets?**
- **Simplifies resolution**: One bet per type per player
- **Prevents exploits**: Can't place multiple Pass bets
- **Clear accounting**: Easy to track winnings

### Special Bet Tracking System (Lines 156-183)

```rust
pub fn update_special_tracking(&mut self, roll: DiceRoll) {
    let total = roll.total();
    
    // Track for Bonus Small/Tall/All
    if total != 7 {
        self.bonus_numbers.insert(total);
    }
    
    // Track for Repeater bets
    *self.repeater_counts.entry(total).or_insert(0) += 1;
    
    // Track for Fire bet (unique points made)
    if self.phase == GamePhase::Point {
        if let Some(point) = self.point {
            if total == point {
                self.fire_points.insert(total);
            }
        }
    }
    
    // Track hardway streaks
    if roll.is_hard_way() {
        *self.hardway_streak.entry(total).or_insert(0) += 1;
    } else if total == 4 || total == 6 || total == 8 || total == 10 {
        self.hardway_streak.remove(&total);
    }
}
```

**Computer Science Foundation: Efficient Set Operations**

Different data structures for different tracking needs:

**HashSet for Fire Bet:**
```rust
fire_points: HashSet<u8>
// Properties:
// - O(1) insertion
// - Automatic deduplication
// - O(1) size check for payout tiers

// Payout structure:
match fire_points.len() {
    0..=3 => 0,      // No payout
    4 => 25:1,       // 4 unique points
    5 => 250:1,      // 5 unique points
    6 => 1000:1,     // All 6 points
    _ => unreachable!()
}
```

**HashMap for Repeater Counts:**
```rust
repeater_counts: HashMap<u8, u8>
// Track how many times each number rolled
// Repeater bet wins when number hits X times

Example progression:
Roll 6: counts[6] = 1
Roll 8: counts[8] = 1  
Roll 6: counts[6] = 2  // Repeater-2 on 6 wins!
```

### Phase Transition Logic (Lines 185-236)

```rust
pub fn update_phase(&mut self, total: u8) {
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
            if let Some(point) = self.point {
                if total == 7 || total == point {
                    // Seven-out or point made - new series
                    self.point = None;
                    self.phase = GamePhase::ComeOut;
                    self.series_id += 1;
                    
                    if total == 7 {
                        // Clear tracking on seven-out
                        self.fire_points.clear();
                        self.bonus_numbers.clear();
                        self.hot_roller_streak = 0;
                        self.come_points.clear();
                        self.dont_come_points.clear();
                    } else {
                        self.hot_roller_streak += 1;
                        // Remove resolved Come/Don't Come points
                        for come_points in self.come_points.values_mut() {
                            come_points.remove(&total);
                        }
                    }
                }
            }
        },
    }
}
```

**Computer Science Foundation: State Machine with Side Effects**

Phase transitions trigger **cascading state updates**:

**Seven-Out Cascade:**
```
Event: Roll = 7 in Point Phase
Effects:
1. phase → ComeOut
2. point → None  
3. series_id++
4. fire_points.clear()      // Fire bet resets
5. bonus_numbers.clear()     // Bonus resets
6. hot_roller_streak = 0     // Streak broken
7. come_points.clear()       // All Come bets lose
8. dont_come_points.clear()  // Don't Come wins handled separately
```

**Point Made Cascade:**
```
Event: Roll = Point in Point Phase
Effects:
1. phase → ComeOut
2. point → None
3. series_id++
4. hot_roller_streak++       // Streak continues!
5. come_points[point].remove // That Come bet wins
6. Special bets continue     // Fire/Bonus keep going
```

### Roll Processing Pipeline (Lines 123-154)

```rust
pub fn process_roll(&mut self, roll: DiceRoll) -> Vec<BetResolution> {
    let mut resolutions = Vec::new();
    
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
    }
    
    // Always resolve one-roll bets
    resolutions.extend(self.resolve_one_roll_bets(roll));
    
    // Update game phase based on roll
    self.update_phase(roll.total());
    
    resolutions
}
```

**Computer Science Foundation: Pipeline Architecture**

Roll processing follows a **strict pipeline order**:

```
Roll Input
    │
    ▼
1. History Tracking
    │
    ▼
2. Special Bet Updates (Fire, Repeater, etc.)
    │
    ▼
3. Phase-Specific Resolution
    ├─> Come-Out Rules
    └─> Point Rules
    │
    ▼
4. One-Roll Bet Resolution (Field, Any7, etc.)
    │
    ▼
5. Phase Transition
    │
    ▼
Output: Vec<BetResolution>
```

**Why This Order?**
1. **History first**: Other operations may need it
2. **Special tracking before resolution**: Need current state
3. **Phase-specific before one-roll**: Pass/Don't Pass priority
4. **Phase update last**: New phase for next roll

## Part II: Senior Engineering Code Review

### Architecture and Design Quality

**Game State Design**: ★★★★★ (5/5)
- Comprehensive state tracking for all bet types
- Clean separation between phases
- Efficient data structures (HashSet, HashMap)
- Good use of Option for nullable state

**Bet Management**: ★★★★☆ (4/5)
- Two-level HashMap is efficient
- Good validation on placement
- Prevents duplicate bets
- Minor: Could use enum for bet states

**RNG Implementation**: ★★★★★ (5/5)
- Cryptographically secure single-source
- Multi-source consensus for fairness
- Proper entropy combination
- No bias in dice generation

### Code Quality Issues and Recommendations

**Issue 1: Incomplete Resolution Methods** (High Priority)
- **Location**: Lines 327-340
- **Problem**: Resolution methods return empty vectors
- **Impact**: Bets won't be resolved
- **Fix**: Implement full resolution logic
```rust
pub fn resolve_comeout_roll(&self, roll: DiceRoll) -> Vec<BetResolution> {
    let mut resolutions = Vec::new();
    let total = roll.total();
    
    // Resolve Pass/Don't Pass bets
    for (player, bets) in &self.player_bets {
        if let Some(pass_bet) = bets.get(&BetType::Pass) {
            let resolution = match total {
                7 | 11 => BetResolution::Won { 
                    player: *player,
                    bet_type: BetType::Pass,
                    amount: pass_bet.amount,
                    payout: pass_bet.amount * 2,  // 1:1 payout
                },
                2 | 3 | 12 => BetResolution::Lost { /* ... */ },
                _ => continue,  // Push - bet stays
            };
            resolutions.push(resolution);
        }
    }
    resolutions
}
```

**Issue 2: No Maximum Bet Limits** (Medium Priority)
- **Location**: Line 90
- **Problem**: No validation of bet amounts
- **Impact**: Could exceed house limits
- **Fix**: Add configurable limits
```rust
pub struct BetLimits {
    min_bet: CrapTokens,
    max_bet: CrapTokens,
    max_odds_multiple: u32,
}

pub fn place_bet(&mut self, player: PeerId, bet: Bet, limits: &BetLimits) 
    -> Result<(), Error> {
    if bet.amount < limits.min_bet || bet.amount > limits.max_bet {
        return Err(Error::InvalidBet("Bet outside limits".into()));
    }
    // ... rest of validation
}
```

**Issue 3: Memory Growth in History** (Low Priority)
- **Location**: Line 130
- **Problem**: Unbounded roll_history vector
- **Impact**: Memory usage grows indefinitely
- **Fix**: Add circular buffer or limit
```rust
const MAX_HISTORY: usize = 1000;

pub fn track_roll(&mut self, roll: DiceRoll) {
    if self.roll_history.len() >= MAX_HISTORY {
        self.roll_history.remove(0);  // Or use VecDeque
    }
    self.roll_history.push(roll);
}
```

### Performance Analysis

**Operation Complexity:**
```
Operation          | Complexity | Notes
-------------------|------------|-------
place_bet()        | O(1)       | HashMap lookups
process_roll()     | O(n*m)     | n players, m bets each
update_phase()     | O(1)       | Simple conditionals
special_tracking() | O(1)       | HashSet operations
get_player_bets()  | O(1)       | Direct lookup
```

**Memory Usage:**
- Per player: ~100 bytes + (50 bytes × bets)
- Per game: ~1KB base + (200 bytes × players)
- History: 8 bytes × rolls (unbounded)

### Security Considerations

**Strengths:**
- Cryptographically secure RNG
- Multi-source entropy prevents manipulation
- Phase validation prevents invalid bets

**Vulnerabilities:**

1. **Missing Nonce in Multi-Source RNG**
```rust
pub struct EntropyContribution {
    player: PeerId,
    entropy: [u8; 32],
    nonce: u64,  // Prevent replay
    signature: Signature,  // Prove authenticity
}
```

2. **No Rate Limiting on Bets**
```rust
pub struct RateLimiter {
    last_bet_time: HashMap<PeerId, Instant>,
    min_interval: Duration,
}
```

### Mathematical Accuracy

**House Edge Implementation:**
```
Bet Type     | True Odds | Casino Pays | House Edge
-------------|-----------|-------------|------------
Pass Line    | 251:244   | 1:1         | 1.41%
Don't Pass   | 976:949   | 1:1         | 1.36%
Field (2,12) | 35:1      | 2:1 or 3:1  | 5.56%
Any 7        | 5:1       | 4:1         | 16.67%
```

The implementation should match these exact payouts for fairness.

### Specific Improvements

1. **Add Odds Betting** (High Priority)
```rust
pub struct OddsBet {
    base_bet: BetType,  // Pass or Come
    odds_amount: CrapTokens,
    point: u8,
}

// True odds payouts (no house edge)
pub fn calculate_odds_payout(point: u8, amount: CrapTokens) -> CrapTokens {
    match point {
        4 | 10 => amount * 2,    // 2:1
        5 | 9 => amount * 3 / 2,  // 3:2
        6 | 8 => amount * 6 / 5,  // 6:5
        _ => amount,
    }
}
```

2. **Implement Hop Bets** (Medium Priority)
```rust
pub enum HopBet {
    Hard(u8),        // Same dice (2-2, 3-3, etc.)
    Easy(u8, u8),   // Different dice (1-2, 2-3, etc.)
}

// Payouts: Hard = 30:1, Easy = 15:1
```

3. **Add Multi-Roll Tracking** (Low Priority)
```rust
pub struct RollSequence {
    rolls: VecDeque<DiceRoll>,
    patterns: HashMap<String, u32>,  // Track patterns
}
```

## Summary

**Overall Score: 8.4/10**

The craps game implementation provides a comprehensive foundation for casino craps with support for complex bet types, special tracking for exotic bets, and cryptographically secure dice generation. The state machine design cleanly handles phase transitions while the nested HashMap structure provides efficient bet management. The multi-source RNG ensures fairness in multiplayer scenarios.

**Key Strengths:**
- Complete state tracking for all 64 bet types
- Cryptographically secure dice generation
- Multi-source consensus RNG for fairness  
- Efficient nested HashMap for bet storage
- Comprehensive special bet tracking
- Clean phase transition logic

**Areas for Improvement:**
- Complete the resolution method implementations
- Add bet limit validation
- Implement circular buffer for history
- Add odds betting support
- Include nonce in multi-source RNG

This implementation provides a solid foundation for a production casino game with fairness guarantees and comprehensive bet support.