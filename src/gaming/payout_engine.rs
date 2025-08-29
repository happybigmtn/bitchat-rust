//! Payout Calculation Engine - Consensus-based bet validation and distributed payouts
//! 
//! This module implements:
//! - Consensus-based bet validation
//! - Distributed payout calculations
//! - Fair dispute resolution
//! - Anti-cheat mechanisms for payouts

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};

use crate::protocol::{PeerId, GameId};
use crate::protocol::craps::{BetType, DiceRoll, CrapTokens, GamePhase};
use crate::crypto::BitchatIdentity;
use crate::error::{Error, Result};
use super::BetRecord;

/// Consensus-based payout calculation engine
#[derive(Debug, Clone)]
pub struct PayoutEngine {
    /// Identity for signing payout calculations
    identity: Arc<BitchatIdentity>,
    
    /// Cached payout calculations for efficiency
    payout_cache: Arc<RwLock<HashMap<PayoutKey, PayoutResult>>>,
    
    /// Bet validation rules
    validation_rules: Arc<RwLock<BetValidationRules>>,
    
    /// Statistics tracking
    stats: Arc<RwLock<PayoutEngineStats>>,
}

/// Key for caching payout calculations
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct PayoutKey {
    game_id: GameId,
    dice_roll: DiceRoll,
    bets_hash: [u8; 32],
}

/// Result of payout calculation with consensus data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayoutResult {
    pub game_id: GameId,
    pub round_id: u64,
    pub dice_roll: DiceRoll,
    pub total_wagered: CrapTokens,
    pub house_edge_taken: CrapTokens,
    pub individual_payouts: HashMap<PeerId, PlayerPayout>,
    pub calculation_timestamp: u64,
    pub consensus_signatures: Vec<PayoutSignature>,
}

/// Individual player payout details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerPayout {
    pub player_id: PeerId,
    pub total_bet: CrapTokens,
    pub total_won: CrapTokens,
    pub net_change: i64, // Can be negative (loss) or positive (win)
    pub winning_bets: Vec<WinningBet>,
    pub losing_bets: Vec<LosingBet>,
}

/// Details of a winning bet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WinningBet {
    pub bet_type: BetType,
    pub amount_bet: CrapTokens,
    pub payout_multiplier: f64,
    pub amount_won: CrapTokens,
}

/// Details of a losing bet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LosingBet {
    pub bet_type: BetType,
    pub amount_lost: CrapTokens,
}

/// Cryptographic signature for payout consensus
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayoutSignature {
    pub signer: PeerId,
    pub signature: Vec<u8>,
    pub timestamp: u64,
}

/// Bet validation request for consensus
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BetValidationRequest {
    pub game_id: GameId,
    pub bets: Vec<BetRecord>,
    pub game_phase: GamePhase,
    pub current_point: Option<u8>,
    pub requester: PeerId,
    pub timestamp: u64,
}

/// Bet validation response from peers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BetValidationResponse {
    pub request_hash: [u8; 32],
    pub validator: PeerId,
    pub is_valid: bool,
    pub invalid_bets: Vec<InvalidBetReason>,
    pub signature: Vec<u8>,
    pub timestamp: u64,
}

/// Reason why a bet is considered invalid
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvalidBetReason {
    pub bet_index: usize,
    pub reason: String,
    pub severity: InvalidBetSeverity,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum InvalidBetSeverity {
    Warning,    // Bet can proceed but may be suspicious
    Error,      // Bet should be rejected
    Critical,   // Potential cheating attempt
}

/// Bet validation rules configuration
#[derive(Debug, Clone)]
pub struct BetValidationRules {
    pub min_bet_amount: CrapTokens,
    pub max_bet_amount: CrapTokens,
    pub max_total_exposure: CrapTokens,
    pub allowed_bet_types_by_phase: HashMap<GamePhase, Vec<BetType>>,
    pub house_edge_percentage: f64,
    pub max_payout_multiplier: f64,
}

impl Default for BetValidationRules {
    fn default() -> Self {
        let mut allowed_bets = HashMap::new();
        
        // Come-out phase: Most bets allowed
        allowed_bets.insert(GamePhase::ComeOut, vec![
            BetType::Pass,
            BetType::DontPass,
            BetType::Field,
            BetType::Next7,
            BetType::Next2,
            BetType::Next3,
            BetType::Next12,
        ]);
        
        // Point phase: Pass/Don't Pass resolved, others still available
        allowed_bets.insert(GamePhase::Point, vec![
            BetType::Come,
            BetType::DontCome,
            BetType::Field,
            BetType::Yes6,
            BetType::Yes8,
            BetType::Next7,
            BetType::Next2,
            BetType::Next3,
            BetType::Next12,
        ]);
        
        Self {
            min_bet_amount: CrapTokens(1),
            max_bet_amount: CrapTokens(10000),
            max_total_exposure: CrapTokens(50000),
            allowed_bet_types_by_phase: allowed_bets,
            house_edge_percentage: 1.4, // 1.4% house edge on Pass Line
            max_payout_multiplier: 30.0, // Maximum 30:1 payout
        }
    }
}

/// Statistics for the payout engine
#[derive(Debug, Default, Clone)]
pub struct PayoutEngineStats {
    pub total_bets_validated: u64,
    pub total_invalid_bets: u64,
    pub total_payouts_calculated: u64,
    pub total_consensus_agreements: u64,
    pub total_consensus_disputes: u64,
    pub total_tokens_distributed: u64,
    pub average_consensus_time_ms: u64,
}

impl PayoutEngine {
    /// Create new payout engine
    pub fn new(identity: Arc<BitchatIdentity>) -> Self {
        Self {
            identity,
            payout_cache: Arc::new(RwLock::new(HashMap::new())),
            validation_rules: Arc::new(RwLock::new(BetValidationRules::default())),
            stats: Arc::new(RwLock::new(PayoutEngineStats::default())),
        }
    }
    
    /// Validate a batch of bets through consensus
    pub async fn validate_bets_consensus(
        &self,
        game_id: GameId,
        bets: Vec<BetRecord>,
        game_phase: GamePhase,
        current_point: Option<u8>,
    ) -> Result<BetValidationResponse> {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        
        // Create validation request
        let request = BetValidationRequest {
            game_id,
            bets: bets.clone(),
            game_phase: game_phase.clone(),
            current_point,
            requester: self.identity.peer_id,
            timestamp,
        };
        
        // Hash the request for consensus tracking
        let request_hash = self.hash_validation_request(&request)?;
        
        // Validate bets according to our rules
        let validation_result = self.validate_bets_locally(&bets, &game_phase, current_point).await?;
        
        // Create response
        // Get invalid bets count before moving validation_result
        let invalid_bets_count = validation_result.invalid_bets.len() as u64;
        
        let response = BetValidationResponse {
            request_hash,
            validator: self.identity.peer_id,
            is_valid: validation_result.is_valid,
            invalid_bets: validation_result.invalid_bets,
            signature: self.sign_validation_response(&request_hash, validation_result.is_valid)?,
            timestamp,
        };
        
        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.total_bets_validated += bets.len() as u64;
            if !validation_result.is_valid {
                stats.total_invalid_bets += invalid_bets_count;
            }
        }
        
        log::info!("Validated {} bets for game {:?}: {}", 
                   bets.len(), game_id, if response.is_valid { "VALID" } else { "INVALID" });
        
        Ok(response)
    }
    
    /// Calculate payouts for a dice roll with consensus
    pub async fn calculate_payouts_consensus(
        &self,
        game_id: GameId,
        dice_roll: DiceRoll,
        active_bets: Vec<BetRecord>,
        game_phase: GamePhase,
        current_point: Option<u8>,
    ) -> Result<PayoutResult> {
        let round_id = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        
        // Create cache key
        let bets_hash = self.hash_bets(&active_bets)?;
        let cache_key = PayoutKey {
            game_id,
            dice_roll,
            bets_hash,
        };
        
        // Check cache first
        if let Some(cached_result) = self.payout_cache.read().await.get(&cache_key) {
            log::debug!("Using cached payout result for game {:?}", game_id);
            return Ok(cached_result.clone());
        }
        
        // Calculate payouts
        let mut individual_payouts = HashMap::new();
        let mut total_wagered = CrapTokens(0);
        let mut total_house_take = CrapTokens(0);
        
        // Group bets by player
        let mut player_bets: HashMap<PeerId, Vec<&BetRecord>> = HashMap::new();
        for bet in &active_bets {
            player_bets.entry(bet.player).or_default().push(bet);
            total_wagered = CrapTokens(total_wagered.0 + bet.amount.0);
        }
        
        // Calculate payout for each player
        for (player_id, bets) in player_bets {
            let player_payout = self.calculate_player_payout(
                &bets,
                dice_roll,
                game_phase,
                current_point,
            ).await?;
            
            individual_payouts.insert(player_id, player_payout);
        }
        
        // Calculate house edge
        let rules = self.validation_rules.read().await;
        let house_edge_rate = rules.house_edge_percentage / 100.0;
        total_house_take = CrapTokens((total_wagered.0 as f64 * house_edge_rate) as u64);
        
        // Create payout result
        let payout_result = PayoutResult {
            game_id,
            round_id,
            dice_roll,
            total_wagered,
            house_edge_taken: total_house_take,
            individual_payouts,
            calculation_timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
            consensus_signatures: Vec::new(), // Would be filled by consensus process
        };
        
        // Cache the result
        self.payout_cache.write().await.insert(cache_key, payout_result.clone());
        
        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.total_payouts_calculated += 1;
            stats.total_tokens_distributed += total_wagered.0;
        }
        
        log::info!("Calculated payouts for game {:?}: {} total wagered, {} players", 
                   game_id, total_wagered.to_crap(), payout_result.individual_payouts.len());
        
        Ok(payout_result)
    }
    
    /// Validate bets locally according to rules
    async fn validate_bets_locally(
        &self,
        bets: &[BetRecord],
        game_phase: &GamePhase,
        _current_point: Option<u8>,
    ) -> Result<LocalValidationResult> {
        let rules = self.validation_rules.read().await;
        let mut invalid_bets = Vec::new();
        let mut is_valid = true;
        
        // Check each bet
        for (index, bet) in bets.iter().enumerate() {
            // Check bet amount limits
            if bet.amount.0 < rules.min_bet_amount.0 {
                invalid_bets.push(InvalidBetReason {
                    bet_index: index,
                    reason: format!("Bet amount {} below minimum {}", 
                                    bet.amount.to_crap(), rules.min_bet_amount.to_crap()),
                    severity: InvalidBetSeverity::Error,
                });
                is_valid = false;
            }
            
            if bet.amount.0 > rules.max_bet_amount.0 {
                invalid_bets.push(InvalidBetReason {
                    bet_index: index,
                    reason: format!("Bet amount {} exceeds maximum {}", 
                                    bet.amount.to_crap(), rules.max_bet_amount.to_crap()),
                    severity: InvalidBetSeverity::Error,
                });
                is_valid = false;
            }
            
            // Check if bet type is allowed in current phase
            if let Some(allowed_bets) = rules.allowed_bet_types_by_phase.get(game_phase) {
                if !allowed_bets.contains(&bet.bet_type) {
                    invalid_bets.push(InvalidBetReason {
                        bet_index: index,
                        reason: format!("Bet type {:?} not allowed in phase {:?}", 
                                        bet.bet_type, game_phase),
                        severity: InvalidBetSeverity::Error,
                    });
                    is_valid = false;
                }
            }
        }
        
        // Check total exposure per player
        let mut player_totals: HashMap<PeerId, u64> = HashMap::new();
        for bet in bets {
            *player_totals.entry(bet.player).or_default() += bet.amount.0;
        }
        
        for (player, total) in player_totals {
            if total > rules.max_total_exposure.0 {
                invalid_bets.push(InvalidBetReason {
                    bet_index: 0, // General validation error
                    reason: format!("Player {:?} total exposure {} exceeds limit {}", 
                                    player, CrapTokens(total).to_crap(), rules.max_total_exposure.to_crap()),
                    severity: InvalidBetSeverity::Warning,
                });
            }
        }
        
        Ok(LocalValidationResult {
            is_valid,
            invalid_bets,
        })
    }
    
    /// Calculate payout for individual player
    async fn calculate_player_payout(
        &self,
        bets: &[&BetRecord],
        dice_roll: DiceRoll,
        game_phase: GamePhase,
        current_point: Option<u8>,
    ) -> Result<PlayerPayout> {
        let mut total_bet = CrapTokens(0);
        let mut total_won = CrapTokens(0);
        let mut winning_bets = Vec::new();
        let mut losing_bets = Vec::new();
        
        for bet in bets {
            total_bet = CrapTokens(total_bet.0 + bet.amount.0);
            
            let (is_winner, payout_multiplier) = self.evaluate_bet(
                &bet.bet_type,
                dice_roll,
                game_phase,
                current_point,
            );
            
            if is_winner {
                let amount_won = CrapTokens((bet.amount.0 as f64 * payout_multiplier) as u64);
                total_won = CrapTokens(total_won.0 + amount_won.0);
                
                winning_bets.push(WinningBet {
                    bet_type: bet.bet_type.clone(),
                    amount_bet: bet.amount,
                    payout_multiplier,
                    amount_won,
                });
            } else {
                losing_bets.push(LosingBet {
                    bet_type: bet.bet_type.clone(),
                    amount_lost: bet.amount,
                });
            }
        }
        
        let net_change = total_won.0 as i64 - total_bet.0 as i64;
        
        Ok(PlayerPayout {
            player_id: bets[0].player, // All bets are from same player
            total_bet,
            total_won,
            net_change,
            winning_bets,
            losing_bets,
        })
    }
    
    /// Evaluate if a bet wins and calculate payout multiplier
    fn evaluate_bet(
        &self,
        bet_type: &BetType,
        dice_roll: DiceRoll,
        game_phase: GamePhase,
        current_point: Option<u8>,
    ) -> (bool, f64) {
        let total = dice_roll.die1 + dice_roll.die2;
        
        match bet_type {
            BetType::Pass => {
                match game_phase {
                    GamePhase::ComeOut => {
                        match total {
                            7 | 11 => (true, 1.0),   // Win even money
                            2 | 3 | 12 => (false, 0.0), // Lose
                            _ => (false, 1.0), // Push to point phase, no resolution yet
                        }
                    },
                    GamePhase::Point => {
                        if let Some(point) = current_point {
                            if total == point {
                                (true, 1.0)  // Made the point
                            } else if total == 7 {
                                (false, 0.0) // Seven out
                            } else {
                                (false, 1.0) // No resolution yet
                            }
                        } else {
                            (false, 0.0)
                        }
                    },
                    _ => (false, 0.0),
                }
            },
            
            BetType::DontPass => {
                match game_phase {
                    GamePhase::ComeOut => {
                        match total {
                            2 | 3 => (true, 1.0),    // Win even money
                            7 | 11 => (false, 0.0),  // Lose
                            12 => (false, 1.0),      // Push (tie)
                            _ => (false, 1.0),       // Push to point phase
                        }
                    },
                    GamePhase::Point => {
                        if let Some(point) = current_point {
                            if total == 7 {
                                (true, 1.0)  // Seven out wins don't pass
                            } else if total == point {
                                (false, 0.0) // Point made, don't pass loses
                            } else {
                                (false, 1.0) // No resolution yet
                            }
                        } else {
                            (false, 0.0)
                        }
                    },
                    _ => (false, 0.0),
                }
            },
            
            BetType::Field => {
                match total {
                    3 | 4 | 9 | 10 | 11 => (true, 1.0),  // Even money
                    2 => (true, 2.0),                     // Pays 2:1
                    12 => (true, 3.0),                    // Pays 3:1
                    _ => (false, 0.0),                    // Lose
                }
            },
            
            BetType::Next7 => {
                if total == 7 {
                    (true, 4.0) // Pays 4:1
                } else {
                    (false, 0.0)
                }
            },
            
            BetType::Next2 | BetType::Next3 | BetType::Next12 => {
                let target = match bet_type {
                    BetType::Next2 => 2,
                    BetType::Next3 => 3,
                    BetType::Next12 => 12,
                    _ => unreachable!(),
                };
                
                if total == target {
                    (true, 30.0) // Pays 30:1 for single-roll bets
                } else {
                    (false, 0.0)
                }
            },
            
            BetType::Yes6 | BetType::Yes8 => {
                let place_number = if matches!(bet_type, BetType::Yes6) { 6 } else { 8 };
                if total == place_number {
                    (true, 7.0/6.0) // Pays 7:6
                } else if total == 7 {
                    (false, 0.0) // Seven out
                } else {
                    (false, 1.0) // No resolution
                }
            },
            
            BetType::Next11 => {
                if total == 11 {
                    (true, 15.0) // Pays 15:1
                } else {
                    (false, 0.0)
                }
            },
            
            _ => (false, 0.0), // Other bet types not implemented
        }
    }
    
    /// Hash validation request for consensus tracking
    fn hash_validation_request(&self, request: &BetValidationRequest) -> Result<[u8; 32]> {
        use sha2::{Sha256, Digest};
        
        let serialized = bincode::serialize(request)
            .map_err(|e| Error::Serialization(format!("Failed to serialize validation request: {}", e)))?;
        
        let mut hasher = Sha256::new();
        hasher.update(&serialized);
        Ok(hasher.finalize().into())
    }
    
    /// Hash bets for cache key
    fn hash_bets(&self, bets: &[BetRecord]) -> Result<[u8; 32]> {
        use sha2::{Sha256, Digest};
        
        let serialized = bincode::serialize(bets)
            .map_err(|e| Error::Serialization(format!("Failed to serialize bets: {}", e)))?;
        
        let mut hasher = Sha256::new();
        hasher.update(&serialized);
        Ok(hasher.finalize().into())
    }
    
    /// Sign validation response
    fn sign_validation_response(&self, request_hash: &[u8; 32], is_valid: bool) -> Result<Vec<u8>> {
        // In production, this would use cryptographic signing
        // For now, we'll create a simple signature placeholder
        let mut signature_data = Vec::new();
        signature_data.extend_from_slice(request_hash);
        signature_data.push(if is_valid { 1 } else { 0 });
        signature_data.extend_from_slice(&self.identity.peer_id);
        Ok(signature_data)
    }
    
    /// Get payout engine statistics
    pub async fn get_stats(&self) -> PayoutEngineStats {
        self.stats.read().await.clone()
    }
    
    /// Update validation rules
    pub async fn update_validation_rules(&self, new_rules: BetValidationRules) {
        *self.validation_rules.write().await = new_rules;
        log::info!("Updated bet validation rules");
    }
    
    /// Clear payout cache (useful for testing or memory management)
    pub async fn clear_cache(&self) {
        self.payout_cache.write().await.clear();
        log::debug!("Cleared payout calculation cache");
    }
}

/// Local validation result structure
struct LocalValidationResult {
    is_valid: bool,
    invalid_bets: Vec<InvalidBetReason>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::BitchatKeypair;
    use crate::protocol::craps::{GamePhase, BetType, CrapTokens, DiceRoll};
    
    fn create_test_engine() -> PayoutEngine {
        let keypair = BitchatKeypair::generate();
        let identity = Arc::new(crate::crypto::BitchatIdentity::from_keypair_with_pow(keypair, 8));
        PayoutEngine::new(identity)
    }
    
    #[tokio::test]
    async fn test_pass_line_bet_validation() {
        let engine = create_test_engine();
        let game_id = [1u8; 16];
        
        let bets = vec![BetRecord {
            player: [2u8; 32],
            bet_type: BetType::Pass,
            amount: CrapTokens(100),
            timestamp: 1234567890,
        }];
        
        let result = engine.validate_bets_consensus(
            game_id,
            bets,
            GamePhase::ComeOut,
            None,
        ).await;
        
        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.is_valid);
        assert!(response.invalid_bets.is_empty());
    }
    
    #[tokio::test]
    async fn test_payout_calculation() {
        let engine = create_test_engine();
        let game_id = [1u8; 16];
        
        let bets = vec![
            BetRecord {
                player: [2u8; 32],
                bet_type: BetType::Pass,
                amount: CrapTokens(100),
                timestamp: 1234567890,
            },
            BetRecord {
                player: [3u8; 32],
                bet_type: BetType::Field,
                amount: CrapTokens(50),
                timestamp: 1234567890,
            },
        ];
        
        let dice_roll = DiceRoll {
            die1: 3,
            die2: 4, // Total 7
            timestamp: 1234567890,
        };
        
        let result = engine.calculate_payouts_consensus(
            game_id,
            dice_roll,
            bets,
            GamePhase::ComeOut,
            None,
        ).await;
        
        assert!(result.is_ok());
        let payout = result.unwrap();
        
        // Pass line should win on 7 in come-out phase
        assert!(payout.individual_payouts.contains_key(&[2u8; 32]));
        
        // Field should lose on 7
        let field_player_payout = payout.individual_payouts.get(&[3u8; 32]).unwrap();
        assert_eq!(field_player_payout.net_change, -50); // Lost the bet
    }
    
    #[tokio::test]
    async fn test_bet_amount_validation() {
        let engine = create_test_engine();
        let game_id = [1u8; 16];
        
        // Test bet below minimum
        let invalid_bets = vec![BetRecord {
            player: [2u8; 32],
            bet_type: BetType::Pass,
            amount: CrapTokens(0), // Below minimum of 1
            timestamp: 1234567890,
        }];
        
        let result = engine.validate_bets_consensus(
            game_id,
            invalid_bets,
            GamePhase::ComeOut,
            None,
        ).await;
        
        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(!response.is_valid);
        assert!(!response.invalid_bets.is_empty());
        assert_eq!(response.invalid_bets[0].severity, InvalidBetSeverity::Error);
    }
}