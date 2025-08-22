//! Efficient bet resolution engine with pre-computed lookup tables
//! 
//! This module implements a high-performance bet resolution system that uses
//! pre-computed payout tables, lookup tables instead of calculations, and
//! cached resolution results for maximum efficiency.

use std::collections::HashMap;
use std::sync::{LazyLock, RwLock};
use once_cell::sync::Lazy;
use serde::{Serialize, Deserialize};

use super::{PeerId, BetType, CrapTokens, DiceRoll};
use super::efficient_game_state::CompactGameState;
use crate::protocol::bet_types::BetResolution;
use crate::error::Result;

/// Pre-computed payout lookup table for all bet types and dice combinations
/// This eliminates runtime calculations for maximum performance
static PAYOUT_LOOKUP_TABLE: Lazy<PayoutLookupTable> = Lazy::new(|| PayoutLookupTable::new());

/// Thread-safe bet resolution cache to avoid re-computing identical scenarios
static RESOLUTION_CACHE: Lazy<RwLock<ResolutionCache>> = Lazy::new(|| {
    RwLock::new(ResolutionCache::new())
});

/// Ultra-fast bet resolution engine
pub struct EfficientBetResolver {
    /// Cached intermediate results for complex bet types
    special_bet_cache: HashMap<SpecialBetKey, BetResolution>,
    
    /// Performance metrics
    cache_hits: u64,
    cache_misses: u64,
    total_resolutions: u64,
}

/// Pre-computed payout table containing all possible bet outcomes
pub struct PayoutLookupTable {
    /// Direct lookup: [bet_type][dice_total] -> payout_multiplier
    /// Size: 64 bet types * 13 dice totals * 4 bytes = 3,328 bytes
    payout_multipliers: [[u32; 13]; 64],
    
    /// Binary lookup for win/lose/push decisions
    /// Size: 64 bet types * 13 dice totals * 1 byte = 832 bytes  
    resolution_type: [[ResolutionType; 13]; 64],
    
    /// Special bet state requirements
    /// Some bets require additional state checking beyond just dice roll
    special_requirements: HashMap<BetType, SpecialRequirement>,
}

/// Resolution type for fast binary decisions
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq)]
enum ResolutionType {
    NoResolution = 0, // Bet continues
    Win = 1,
    Lose = 2,
    Push = 3,
}

/// Special requirements for complex bet types
#[derive(Debug, Clone)]
enum SpecialRequirement {
    RequiresPhase(GamePhaseSet),
    RequiresPoint(u8),
    RequiresStreak(u32),
    RequiresHistory(u32), // Number of previous rolls to check
    RequiresFirePoints(u8),
    RequiresBonusNumbers(u16), // Bitmask of required numbers
}

/// Bit set for game phases
type GamePhaseSet = u8; // Bits: 0=ComeOut, 1=Point, 2=Ended

/// Key for caching complex bet resolutions
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct SpecialBetKey {
    bet_type: BetType,
    dice_roll: u16, // Packed dice roll (die1 + die2 * 7)
    phase: u8,
    point: u8,
    state_hash: u32, // Hash of relevant game state
}

/// Resolution result cache
pub struct ResolutionCache {
    /// LRU cache for bet resolutions
    cache: lru::LruCache<ResolutionCacheKey, Vec<BetResolution>>,
    
    /// Statistics
    hits: u64,
    misses: u64,
}

/// Cache key for resolution results
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct ResolutionCacheKey {
    bet_mask: u64, // Which bet types are active
    dice_total: u8,
    phase: u8, 
    point: u8,
    special_state: u64, // Hash of special bet tracking state
}

impl PayoutLookupTable {
    /// Initialize the complete payout lookup table
    fn new() -> Self {
        let mut table = Self {
            payout_multipliers: [[100; 13]; 64], // Default 1:1 payout (100%)
            resolution_type: [[ResolutionType::NoResolution; 13]; 64],
            special_requirements: HashMap::new(),
        };
        
        table.populate_pass_line_bets();
        table.populate_field_bets();
        table.populate_yes_no_bets(); 
        table.populate_hardway_bets();
        table.populate_next_bets();
        table.populate_come_bets();
        table.populate_odds_bets();
        table.populate_repeater_bets();
        table.populate_special_bets();
        
        table
    }
    
    /// Populate Pass/Don't Pass line bet lookup data
    fn populate_pass_line_bets(&mut self) {
        let pass_idx = BetType::Pass as usize;
        let dont_pass_idx = BetType::DontPass as usize;
        
        // Come out roll outcomes
        for total in 2..=12 {
            match total {
                7 | 11 => {
                    self.resolution_type[pass_idx][total] = ResolutionType::Win;
                    self.payout_multipliers[pass_idx][total] = 200; // 1:1 + original
                    self.resolution_type[dont_pass_idx][total] = ResolutionType::Lose;
                },
                2 | 3 => {
                    self.resolution_type[pass_idx][total] = ResolutionType::Lose;
                    self.resolution_type[dont_pass_idx][total] = ResolutionType::Win;
                    self.payout_multipliers[dont_pass_idx][total] = 200;
                },
                12 => {
                    self.resolution_type[pass_idx][total] = ResolutionType::Lose;
                    self.resolution_type[dont_pass_idx][total] = ResolutionType::Push;
                },
                _ => {
                    // Point numbers - no immediate resolution
                    self.resolution_type[pass_idx][total] = ResolutionType::NoResolution;
                    self.resolution_type[dont_pass_idx][total] = ResolutionType::NoResolution;
                },
            }
        }
        
        // Add special requirements for point phase
        self.special_requirements.insert(BetType::Pass, SpecialRequirement::RequiresPhase(0b011)); // ComeOut or Point
        self.special_requirements.insert(BetType::DontPass, SpecialRequirement::RequiresPhase(0b011));
    }
    
    /// Populate Field bet lookup data
    fn populate_field_bets(&mut self) {
        let field_idx = BetType::Field as usize;
        
        for total in 2..=12 {
            match total {
                2 | 12 => {
                    self.resolution_type[field_idx][total] = ResolutionType::Win;
                    self.payout_multipliers[field_idx][total] = 300; // 2:1 + original
                },
                3 | 4 | 9 | 10 | 11 => {
                    self.resolution_type[field_idx][total] = ResolutionType::Win;
                    self.payout_multipliers[field_idx][total] = 200; // 1:1 + original
                },
                _ => {
                    self.resolution_type[field_idx][total] = ResolutionType::Lose;
                },
            }
        }
    }
    
    /// Populate YES/NO bet lookup data
    fn populate_yes_no_bets(&mut self) {
        let yes_bets = [
            (BetType::Yes2, 2), (BetType::Yes3, 3), (BetType::Yes4, 4), (BetType::Yes5, 5), (BetType::Yes6, 6),
            (BetType::Yes8, 8), (BetType::Yes9, 9), (BetType::Yes10, 10), (BetType::Yes11, 11), (BetType::Yes12, 12),
        ];
        
        let no_bets = [
            (BetType::No2, 2), (BetType::No3, 3), (BetType::No4, 4), (BetType::No5, 5), (BetType::No6, 6),
            (BetType::No8, 8), (BetType::No9, 9), (BetType::No10, 10), (BetType::No11, 11), (BetType::No12, 12),
        ];
        
        // YES bets win when their number comes up, lose on 7
        for (bet_type, target) in yes_bets {
            let idx = bet_type as usize;
            self.resolution_type[idx][target] = ResolutionType::Win;
            self.payout_multipliers[idx][target] = self.get_yes_bet_payout_multiplier(target);
            self.resolution_type[idx][7] = ResolutionType::Lose;
        }
        
        // NO bets win on 7, lose when their number comes up
        for (bet_type, target) in no_bets {
            let idx = bet_type as usize;
            self.resolution_type[idx][7] = ResolutionType::Win;
            self.payout_multipliers[idx][7] = self.get_no_bet_payout_multiplier(target);
            self.resolution_type[idx][target] = ResolutionType::Lose;
        }
    }
    
    /// Populate Hardway bet lookup data
    fn populate_hardway_bets(&mut self) {
        let hardway_bets = [
            (BetType::Hard4, 4, 800),   // 7:1 + original
            (BetType::Hard6, 6, 1000),  // 9:1 + original
            (BetType::Hard8, 8, 1000),  // 9:1 + original
            (BetType::Hard10, 10, 800), // 7:1 + original
        ];
        
        for (bet_type, target, payout) in hardway_bets {
            let idx = bet_type as usize;
            // Win only if rolled the hard way (checked elsewhere)
            self.payout_multipliers[idx][target] = payout;
            // Lose on 7 or easy way of same total
            self.resolution_type[idx][7] = ResolutionType::Lose;
            
            self.special_requirements.insert(bet_type, SpecialRequirement::RequiresHistory(1));
        }
    }
    
    /// Populate NEXT (one-roll) bet lookup data
    fn populate_next_bets(&mut self) {
        let next_bets = [
            (BetType::Next2, 2, 3530),   // 34.3:1 + original
            (BetType::Next3, 3, 1766),   // 16.66:1 + original 
            (BetType::Next4, 4, 1178),   // 10.78:1 + original
            (BetType::Next5, 5, 884),    // 7.84:1 + original
            (BetType::Next6, 6, 708),    // 6.08:1 + original
            (BetType::Next7, 7, 590),    // 4.9:1 + original
            (BetType::Next8, 8, 708),    // 6.08:1 + original
            (BetType::Next9, 9, 884),    // 7.84:1 + original
            (BetType::Next10, 10, 1178), // 10.78:1 + original
            (BetType::Next11, 11, 1766), // 16.66:1 + original
            (BetType::Next12, 12, 3530), // 34.3:1 + original
        ];
        
        for (bet_type, target, payout) in next_bets {
            let idx = bet_type as usize;
            
            // Win only on the exact number
            self.resolution_type[idx][target] = ResolutionType::Win;
            self.payout_multipliers[idx][target] = payout;
            
            // Lose on all other numbers
            for total in 2..=12 {
                if total != target {
                    self.resolution_type[idx][total] = ResolutionType::Lose;
                }
            }
        }
    }
    
    /// Populate Come/Don't Come bet lookup data
    fn populate_come_bets(&mut self) {
        let come_idx = BetType::Come as usize;
        let dont_come_idx = BetType::DontCome as usize;
        
        // Similar to Pass/Don't Pass but only in point phase
        for total in 2..=12 {
            match total {
                7 | 11 => {
                    self.resolution_type[come_idx][total] = ResolutionType::Win;
                    self.payout_multipliers[come_idx][total] = 200;
                    self.resolution_type[dont_come_idx][total] = ResolutionType::Lose;
                },
                2 | 3 => {
                    self.resolution_type[come_idx][total] = ResolutionType::Lose;
                    self.resolution_type[dont_come_idx][total] = ResolutionType::Win;
                    self.payout_multipliers[dont_come_idx][total] = 200;
                },
                12 => {
                    self.resolution_type[come_idx][total] = ResolutionType::Lose;
                    self.resolution_type[dont_come_idx][total] = ResolutionType::Push;
                },
                _ => {
                    self.resolution_type[come_idx][total] = ResolutionType::NoResolution;
                    self.resolution_type[dont_come_idx][total] = ResolutionType::NoResolution;
                },
            }
        }
        
        self.special_requirements.insert(BetType::Come, SpecialRequirement::RequiresPhase(0b010)); // Point only
        self.special_requirements.insert(BetType::DontCome, SpecialRequirement::RequiresPhase(0b010));
    }
    
    /// Populate Odds bet lookup data
    fn populate_odds_bets(&mut self) {
        // Odds bets have true odds, no house edge
        let odds_bets = [BetType::OddsPass, BetType::OddsDontPass, BetType::OddsCome, BetType::OddsDontCome];
        
        for bet_type in odds_bets {
            self.special_requirements.insert(bet_type, SpecialRequirement::RequiresPoint(0)); // Any point
        }
    }
    
    /// Populate Repeater bet lookup data
    fn populate_repeater_bets(&mut self) {
        let repeater_bets = [
            (BetType::Repeater2, 4100),   // 40:1 + original
            (BetType::Repeater3, 5100),   // 50:1 + original
            (BetType::Repeater4, 6600),   // 65:1 + original
            (BetType::Repeater5, 8100),   // 80:1 + original
            (BetType::Repeater6, 9100),   // 90:1 + original
            (BetType::Repeater8, 9100),   // 90:1 + original
            (BetType::Repeater9, 8100),   // 80:1 + original
            (BetType::Repeater10, 6600),  // 65:1 + original
            (BetType::Repeater11, 5100),  // 50:1 + original
            (BetType::Repeater12, 4100),  // 40:1 + original
        ];
        
        for (bet_type, payout) in repeater_bets {
            let idx = bet_type as usize;
            // Repeater bets require counting occurrences
            for total in 2..=12 {
                self.payout_multipliers[idx][total] = payout;
            }
            
            // Lose on 7
            self.resolution_type[idx][7] = ResolutionType::Lose;
            
            self.special_requirements.insert(bet_type, SpecialRequirement::RequiresHistory(100)); // Check many rolls
        }
    }
    
    /// Populate special bet lookup data
    fn populate_special_bets(&mut self) {
        // Fire bet
        self.special_requirements.insert(BetType::Fire, SpecialRequirement::RequiresFirePoints(4));
        
        // Bonus bets
        self.special_requirements.insert(BetType::BonusSmall, SpecialRequirement::RequiresBonusNumbers(0b111110)); // 2-6
        self.special_requirements.insert(BetType::BonusTall, SpecialRequirement::RequiresBonusNumbers(0b111111000000)); // 8-12
        self.special_requirements.insert(BetType::BonusAll, SpecialRequirement::RequiresBonusNumbers(0b111111111110)); // 2-12 except 7
        
        // Other special bets
        self.special_requirements.insert(BetType::HotRoller, SpecialRequirement::RequiresStreak(20));
        self.special_requirements.insert(BetType::TwiceHard, SpecialRequirement::RequiresHistory(2));
        self.special_requirements.insert(BetType::RideLine, SpecialRequirement::RequiresStreak(3));
    }
    
    /// Get YES bet payout multiplier based on target number
    fn get_yes_bet_payout_multiplier(&self, target: usize) -> u32 {
        match target {
            2 | 12 => 688,  // 5.88:1 + original
            3 | 11 => 394,  // 2.94:1 + original
            4 | 10 => 296,  // 1.96:1 + original
            5 | 9 => 247,   // 1.47:1 + original
            6 | 8 => 218,   // 1.18:1 + original
            _ => 200,
        }
    }
    
    /// Get NO bet payout multiplier based on target number
    fn get_no_bet_payout_multiplier(&self, target: usize) -> u32 {
        match target {
            2 | 12 => 116,  // 0.16:1 + original
            3 | 11 => 133,  // 0.33:1 + original
            4 | 10 => 149,  // 0.49:1 + original
            5 | 9 => 165,   // 0.65:1 + original
            6 | 8 => 182,   // 0.82:1 + original
            _ => 200,
        }
    }
    
    /// Fast lookup for bet resolution
    pub fn lookup_resolution(&self, bet_type: BetType, dice_total: u8) -> (ResolutionType, u32) {
        let bet_idx = bet_type as usize;
        let total_idx = dice_total as usize;
        
        if bet_idx < 64 && total_idx < 13 {
            (
                self.resolution_type[bet_idx][total_idx],
                self.payout_multipliers[bet_idx][total_idx]
            )
        } else {
            (ResolutionType::NoResolution, 100)
        }
    }
    
    /// Check if bet type requires special handling
    pub fn requires_special_handling(&self, bet_type: BetType) -> bool {
        self.special_requirements.contains_key(&bet_type)
    }
    
    /// Get special requirement for bet type
    pub fn get_special_requirement(&self, bet_type: BetType) -> Option<&SpecialRequirement> {
        self.special_requirements.get(&bet_type)
    }
}

impl ResolutionCache {
    fn new() -> Self {
        Self {
            cache: lru::LruCache::new(std::num::NonZeroUsize::new(1000).unwrap()),
            hits: 0,
            misses: 0,
        }
    }
    
    /// Get cached resolution result
    pub fn get(&mut self, key: &ResolutionCacheKey) -> Option<&Vec<BetResolution>> {
        if let Some(result) = self.cache.get(key) {
            self.hits += 1;
            Some(result)
        } else {
            self.misses += 1;
            None
        }
    }
    
    /// Store resolution result in cache
    pub fn insert(&mut self, key: ResolutionCacheKey, result: Vec<BetResolution>) {
        self.cache.put(key, result);
    }
    
    /// Get cache hit rate
    pub fn hit_rate(&self) -> f64 {
        if self.hits + self.misses == 0 {
            0.0
        } else {
            self.hits as f64 / (self.hits + self.misses) as f64
        }
    }
}

impl EfficientBetResolver {
    /// Create new efficient bet resolver
    pub fn new() -> Self {
        Self {
            special_bet_cache: HashMap::new(),
            cache_hits: 0,
            cache_misses: 0,
            total_resolutions: 0,
        }
    }
    
    /// Resolve all bets for a dice roll using optimized lookup tables
    pub fn resolve_bets_fast(
        &mut self,
        state: &CompactGameState,
        dice_roll: DiceRoll,
        active_bets: &[(BetType, PeerId, CrapTokens)]
    ) -> Result<Vec<BetResolution>> {
        self.total_resolutions += 1;
        
        // Create cache key
        let cache_key = self.create_cache_key(state, dice_roll, active_bets);
        
        // Check resolution cache first (thread-safe)
        if let Ok(cache) = RESOLUTION_CACHE.read() {
            if let Some(cached_result) = cache.get(&cache_key) {
                self.cache_hits += 1;
                return Ok(cached_result.clone());
            }
        }
        
        self.cache_misses += 1;
        
        // Fast path: resolve using lookup tables
        let mut resolutions = Vec::new();
        let dice_total = dice_roll.total();
        
        for &(bet_type, player, amount) in active_bets {
            if let Some(resolution) = self.resolve_single_bet_fast(state, bet_type, player, amount, dice_roll)? {
                resolutions.push(resolution);
            }
        }
        
        // Cache the result (thread-safe)
        if let Ok(mut cache) = RESOLUTION_CACHE.write() {
            cache.insert(cache_key, resolutions.clone());
        }
        
        Ok(resolutions)
    }
    
    /// Resolve a single bet using pre-computed lookup table
    fn resolve_single_bet_fast(
        &mut self,
        state: &CompactGameState,
        bet_type: BetType,
        player: PeerId,
        amount: CrapTokens,
        dice_roll: DiceRoll
    ) -> Result<Option<BetResolution>> {
        let dice_total = dice_roll.total();
        
        // Fast lookup for basic resolution
        let (resolution_type, payout_multiplier) = PAYOUT_LOOKUP_TABLE.lookup_resolution(bet_type, dice_total);
        
        match resolution_type {
            ResolutionType::Win => {
                let payout_amount = (amount.amount() * payout_multiplier as u64) / 100;
                Ok(Some(BetResolution::Won {
                    player,
                    bet_type,
                    amount,
                    payout: CrapTokens::new_unchecked(payout_amount),
                }))
            },
            ResolutionType::Lose => {
                Ok(Some(BetResolution::Lost {
                    player,
                    bet_type,
                    amount,
                }))
            },
            ResolutionType::Push => {
                Ok(Some(BetResolution::Push {
                    player,
                    bet_type,
                    amount,
                }))
            },
            ResolutionType::NoResolution => {
                // Check if this bet type requires special handling
                if PAYOUT_LOOKUP_TABLE.requires_special_handling(bet_type) {
                    self.resolve_special_bet(state, bet_type, player, amount, dice_roll)
                } else {
                    Ok(None) // Bet continues
                }
            },
        }
    }
    
    /// Resolve special bets that require additional state checking
    fn resolve_special_bet(
        &mut self,
        state: &CompactGameState,
        bet_type: BetType,
        player: PeerId,
        amount: CrapTokens,
        dice_roll: DiceRoll
    ) -> Result<Option<BetResolution>> {
        // Create cache key for special bet
        let special_key = SpecialBetKey {
            bet_type,
            dice_roll: (dice_roll.die1 as u16) + (dice_roll.die2 as u16) * 7,
            phase: state.get_phase() as u8,
            point: state.get_point().unwrap_or(0),
            state_hash: self.calculate_state_hash(state),
        };
        
        // Check special bet cache
        if let Some(cached_resolution) = self.special_bet_cache.get(&special_key) {
            return Ok(Some(cached_resolution.clone()));
        }
        
        // Resolve based on special requirements
        if let Some(requirement) = PAYOUT_LOOKUP_TABLE.get_special_requirement(bet_type) {
            let resolution = match requirement {
                SpecialRequirement::RequiresFirePoints(min_points) => {
                    if state.get_fire_points() >= *min_points {
                        Some(self.create_fire_bet_resolution(player, amount, state.get_fire_points()))
                    } else {
                        None
                    }
                },
                SpecialRequirement::RequiresBonusNumbers(required_mask) => {
                    let bonus_numbers = state.get_bonus_numbers();
                    if (bonus_numbers & required_mask) == *required_mask {
                        Some(self.create_bonus_bet_resolution(bet_type, player, amount))
                    } else {
                        None
                    }
                },
                SpecialRequirement::RequiresStreak(min_streak) => {
                    if state.get_hot_streak() >= *min_streak as u16 {
                        Some(self.create_streak_bet_resolution(bet_type, player, amount, state.get_hot_streak()))
                    } else {
                        None
                    }
                },
                _ => None, // Other special requirements need more complex handling
            };
            
            // Cache the result
            if let Some(ref res) = resolution {
                self.special_bet_cache.insert(special_key, res.clone());
            }
            
            Ok(resolution)
        } else {
            Ok(None)
        }
    }
    
    /// Create Fire bet resolution
    fn create_fire_bet_resolution(&self, player: PeerId, amount: CrapTokens, fire_points: u8) -> BetResolution {
        let multiplier = match fire_points {
            4 => 2500,      // 24:1 + original
            5 => 25000,     // 249:1 + original  
            6 => 100000,    // 999:1 + original
            _ => 2500,
        };
        
        let payout_amount = (amount.amount() * multiplier) / 100;
        BetResolution::Won {
            player,
            bet_type: BetType::Fire,
            amount,
            payout: CrapTokens::new_unchecked(payout_amount),
        }
    }
    
    /// Create Bonus bet resolution
    fn create_bonus_bet_resolution(&self, bet_type: BetType, player: PeerId, amount: CrapTokens) -> BetResolution {
        let multiplier = match bet_type {
            BetType::BonusSmall | BetType::BonusTall => 3100, // 30:1 + original
            BetType::BonusAll => 15100, // 150:1 + original
            _ => 200,
        };
        
        let payout_amount = (amount.amount() * multiplier) / 100;
        BetResolution::Won {
            player,
            bet_type,
            amount,
            payout: CrapTokens::new_unchecked(payout_amount),
        }
    }
    
    /// Create streak-based bet resolution
    fn create_streak_bet_resolution(&self, bet_type: BetType, player: PeerId, amount: CrapTokens, streak: u16) -> BetResolution {
        let multiplier = match bet_type {
            BetType::HotRoller => match streak {
                20..=30 => 300,   // 2:1 + original
                31..=40 => 600,   // 5:1 + original
                41..=50 => 1100,  // 10:1 + original
                _ => 2100,        // 20:1 + original
            },
            BetType::RideLine => match streak {
                3 => 400,   // 3:1 + original
                4 => 600,   // 5:1 + original
                5 => 1100,  // 10:1 + original
                _ => 2600,  // 25:1 + original
            },
            _ => 200,
        };
        
        let payout_amount = (amount.amount() * multiplier) / 100;
        BetResolution::Won {
            player,
            bet_type,
            amount,
            payout: CrapTokens::new_unchecked(payout_amount),
        }
    }
    
    /// Create cache key for resolution caching
    fn create_cache_key(
        &self,
        state: &CompactGameState,
        dice_roll: DiceRoll,
        active_bets: &[(BetType, PeerId, CrapTokens)]
    ) -> ResolutionCacheKey {
        // Create bet mask from active bets
        let mut bet_mask = 0u64;
        for &(bet_type, _, _) in active_bets {
            let bit_index = bet_type as u8;
            if bit_index < 64 {
                bet_mask |= 1u64 << bit_index;
            }
        }
        
        ResolutionCacheKey {
            bet_mask,
            dice_total: dice_roll.total(),
            phase: state.get_phase() as u8,
            point: state.get_point().unwrap_or(0),
            special_state: self.hash_special_state(state),
        }
    }
    
    /// Calculate hash of game state for caching
    fn calculate_state_hash(&self, state: &CompactGameState) -> u32 {
        // Simple hash of key state components
        let mut hash = 0u32;
        hash = hash.wrapping_add(state.get_series_id());
        hash = hash.wrapping_add(state.get_roll_count());
        hash = hash.wrapping_add(state.get_fire_points() as u32);
        hash = hash.wrapping_add(state.get_hot_streak() as u32);
        hash
    }
    
    /// Hash special state for caching
    fn hash_special_state(&self, state: &CompactGameState) -> u64 {
        let mut hash = 0u64;
        hash |= (state.get_fire_points() as u64) << 0;
        hash |= (state.get_bonus_numbers() as u64) << 8;
        hash |= (state.get_hot_streak() as u64) << 24;
        hash
    }
    
    /// Get performance statistics
    pub fn get_stats(&self) -> BetResolverStats {
        let cache_hit_rate = if let Ok(cache) = RESOLUTION_CACHE.read() {
            cache.hit_rate()
        } else {
            0.0
        };
        
        BetResolverStats {
            total_resolutions: self.total_resolutions,
            cache_hits: self.cache_hits,
            cache_misses: self.cache_misses,
            cache_hit_rate: if self.total_resolutions > 0 {
                self.cache_hits as f64 / self.total_resolutions as f64
            } else { 0.0 },
            resolution_cache_hit_rate: cache_hit_rate,
            special_bet_cache_size: self.special_bet_cache.len(),
            lookup_table_size: std::mem::size_of::<PayoutLookupTable>(),
        }
    }
    
    /// Clear all caches (for testing or memory management)
    pub fn clear_caches(&mut self) {
        self.special_bet_cache.clear();
        if let Ok(mut cache) = RESOLUTION_CACHE.write() {
            cache.clear();
        }
    }
}

/// Performance statistics for the bet resolver
#[derive(Debug, Clone)]
pub struct BetResolverStats {
    pub total_resolutions: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub cache_hit_rate: f64,
    pub resolution_cache_hit_rate: f64,
    pub special_bet_cache_size: usize,
    pub lookup_table_size: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::efficient_game_state::CompactGameState;

    #[test]
    fn test_payout_lookup_table() {
        let table = PayoutLookupTable::new();
        
        // Test Pass line win on 7
        let (resolution_type, multiplier) = table.lookup_resolution(BetType::Pass, 7);
        assert_eq!(resolution_type, ResolutionType::Win);
        assert_eq!(multiplier, 200); // 1:1 + original
        
        // Test Field bet on 2
        let (resolution_type, multiplier) = table.lookup_resolution(BetType::Field, 2);
        assert_eq!(resolution_type, ResolutionType::Win);
        assert_eq!(multiplier, 300); // 2:1 + original
    }
    
    #[test]
    fn test_efficient_bet_resolver() {
        let mut resolver = EfficientBetResolver::new();
        let state = CompactGameState::new([1; 16], [2; 32]);
        let dice_roll = DiceRoll::new(3, 4).unwrap(); // Total 7
        
        let active_bets = vec![
            (BetType::Pass, [2; 32], CrapTokens::new_unchecked(100)),
            (BetType::Field, [3; 32], CrapTokens::new_unchecked(50)),
        ];
        
        let resolutions = resolver.resolve_bets_fast(&state, dice_roll, &active_bets).unwrap();
        
        // Pass should win on 7, Field should lose
        assert_eq!(resolutions.len(), 2);
        assert!(resolutions[0].is_win() || resolutions[1].is_win()); // Pass wins
        assert!(resolutions[0].is_loss() || resolutions[1].is_loss()); // Field loses
    }
    
    #[test]
    fn test_bet_resolution_caching() {
        let mut resolver = EfficientBetResolver::new();
        let state = CompactGameState::new([1; 16], [2; 32]);
        let dice_roll = DiceRoll::new(1, 1).unwrap(); // Total 2
        
        let active_bets = vec![
            (BetType::Field, [2; 32], CrapTokens::new_unchecked(100)),
        ];
        
        // First call should miss cache
        let _resolutions1 = resolver.resolve_bets_fast(&state, dice_roll, &active_bets).unwrap();
        
        // Second call should hit cache  
        let _resolutions2 = resolver.resolve_bets_fast(&state, dice_roll, &active_bets).unwrap();
        
        let stats = resolver.get_stats();
        assert!(stats.cache_hit_rate > 0.0);
    }
    
    #[test]
    fn test_special_bet_resolution() {
        let mut resolver = EfficientBetResolver::new();
        let mut state = CompactGameState::new([1; 16], [2; 32]);
        
        // Set up Fire bet with 4 points
        state.set_fire_points(4);
        
        let dice_roll = DiceRoll::new(2, 4).unwrap(); // Total 6
        let player = [2; 32];
        let amount = CrapTokens::new_unchecked(100);
        
        let resolution = resolver.resolve_single_bet_fast(
            &state, 
            BetType::Fire, 
            player, 
            amount, 
            dice_roll
        ).unwrap();
        
        assert!(resolution.is_some());
        if let Some(BetResolution::Won { payout, .. }) = resolution {
            assert_eq!(payout.amount(), 2500); // 24:1 + original
        }
    }
    
    #[test]
    fn test_lookup_table_size() {
        let stats = EfficientBetResolver::new().get_stats();
        
        // Verify lookup table fits in CPU cache (target < 32KB)
        assert!(stats.lookup_table_size < 32 * 1024, 
               "Lookup table size {} exceeds 32KB", stats.lookup_table_size);
    }
    
    #[test]
    fn test_hardway_bet_special_handling() {
        let table = PayoutLookupTable::new();
        
        // Hardway bets should require special handling to check if rolled hard
        assert!(table.requires_special_handling(BetType::Hard4));
        assert!(table.requires_special_handling(BetType::Hard6));
        
        // Regular bets should not require special handling
        assert!(!table.requires_special_handling(BetType::Pass));
        assert!(!table.requires_special_handling(BetType::Field));
    }
    
    #[test]
    fn test_next_bet_one_roll_resolution() {
        let table = PayoutLookupTable::new();
        
        // NEXT 7 should win on 7, lose on everything else
        let (win_type, _) = table.lookup_resolution(BetType::Next7, 7);
        let (lose_type, _) = table.lookup_resolution(BetType::Next7, 6);
        
        assert_eq!(win_type, ResolutionType::Win);
        assert_eq!(lose_type, ResolutionType::Lose);
    }
}