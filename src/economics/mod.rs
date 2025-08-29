//! Advanced Token Economics Module
//! 
//! This module implements sophisticated token economics mechanisms for the
//! decentralized BitCraps casino platform, including:
//! - Token supply management with inflation/deflation controls
//! - Advanced staking mechanisms with variable rewards
//! - Dynamic fee structures based on network conditions
//! - Token burn mechanisms for supply management
//! - Liquidity incentives and yield farming
//! - Economic governance and parameter adjustment

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};

use crate::protocol::{PeerId, CrapTokens};
use crate::error::{Error, Result};
use crate::token::TokenLedger;

pub mod staking;
pub mod supply;
pub mod fees;
pub mod governance;
pub mod liquidity;

/// Token economics configuration parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EconomicsConfig {
    /// Base annual percentage yield for staking
    pub base_staking_apy: f64,
    
    /// Maximum staking APY with bonuses
    pub max_staking_apy: f64,
    
    /// Minimum staking duration in seconds
    pub min_stake_duration: u64,
    
    /// Token burn rate as percentage of fees
    pub burn_rate: f64,
    
    /// Treasury fee percentage
    pub treasury_fee: f64,
    
    /// Validator fee percentage
    pub validator_fee: f64,
    
    /// Minimum tokens required for governance voting
    pub governance_threshold: u64,
    
    /// Inflation target percentage per year
    pub inflation_target: f64,
    
    /// Deflation trigger threshold (tokens burned vs minted ratio)
    pub deflation_trigger: f64,
}

impl Default for EconomicsConfig {
    fn default() -> Self {
        Self {
            base_staking_apy: 5.0,           // 5% base APY
            max_staking_apy: 15.0,           // 15% max APY with bonuses
            min_stake_duration: 86400 * 7,   // 7 days minimum
            burn_rate: 0.5,                  // 50% of fees burned
            treasury_fee: 0.003,             // 0.3% treasury fee
            validator_fee: 0.002,            // 0.2% validator fee
            governance_threshold: 10_000_000_000, // 10k CRAP minimum
            inflation_target: 2.0,           // 2% annual inflation
            deflation_trigger: 1.5,          // Deflation if burn > 1.5x mint
        }
    }
}

/// Advanced staking position with enhanced features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedStakingPosition {
    pub staker: PeerId,
    pub amount: CrapTokens,
    pub staked_at: u64,
    pub lock_duration: Duration,
    pub unlock_time: u64,
    pub reward_rate: f64,
    pub accumulated_rewards: CrapTokens,
    pub compound_frequency: CompoundingFrequency,
    pub delegation_target: Option<PeerId>,
    pub early_unlock_penalty: f64,
    pub loyalty_bonus: f64,
    pub governance_power: u64,
}

/// Compounding frequency for staking rewards
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompoundingFrequency {
    Manual,
    Daily,
    Weekly,
    Monthly,
}

/// Token supply metrics and controls
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupplyMetrics {
    pub total_supply: CrapTokens,
    pub circulating_supply: CrapTokens,
    pub staked_supply: CrapTokens,
    pub treasury_supply: CrapTokens,
    pub burned_supply: CrapTokens,
    pub inflation_rate: f64,
    pub deflation_rate: f64,
    pub velocity: f64,
    pub last_adjustment: u64,
}

/// Dynamic fee structure based on network conditions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicFeeStructure {
    pub base_fee: CrapTokens,
    pub congestion_multiplier: f64,
    pub priority_fee: CrapTokens,
    pub gas_price: f64,
    pub fee_categories: HashMap<FeeCategory, CrapTokens>,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum FeeCategory {
    Transfer,
    GameBet,
    Staking,
    Governance,
    LiquidityProvision,
    CrossChain,
}

/// Liquidity pool for automated market making
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidityPool {
    pub pool_id: [u8; 32],
    pub token_a_reserve: CrapTokens,
    pub token_b_reserve: CrapTokens,
    pub total_liquidity_tokens: CrapTokens,
    pub fee_rate: f64,
    pub providers: HashMap<PeerId, LiquidityPosition>,
    pub swap_volume_24h: CrapTokens,
    pub fees_collected: CrapTokens,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidityPosition {
    pub provider: PeerId,
    pub liquidity_tokens: CrapTokens,
    pub token_a_contributed: CrapTokens,
    pub token_b_contributed: CrapTokens,
    pub rewards_earned: CrapTokens,
    pub position_opened: u64,
}

/// Governance proposal for economic parameter changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EconomicsProposal {
    pub proposal_id: [u8; 32],
    pub proposer: PeerId,
    pub title: String,
    pub description: String,
    pub parameter_changes: HashMap<String, serde_json::Value>,
    pub voting_start: u64,
    pub voting_end: u64,
    pub votes_for: CrapTokens,
    pub votes_against: CrapTokens,
    pub status: ProposalStatus,
    pub execution_time: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProposalStatus {
    Pending,
    Active,
    Succeeded,
    Failed,
    Executed,
    Cancelled,
}

/// Main economics engine
pub struct TokenEconomics {
    config: Arc<RwLock<EconomicsConfig>>,
    ledger: Arc<TokenLedger>,
    staking_positions: Arc<RwLock<HashMap<PeerId, Vec<AdvancedStakingPosition>>>>,
    supply_metrics: Arc<RwLock<SupplyMetrics>>,
    fee_structure: Arc<RwLock<DynamicFeeStructure>>,
    liquidity_pools: Arc<RwLock<HashMap<[u8; 32], LiquidityPool>>>,
    governance_proposals: Arc<RwLock<HashMap<[u8; 32], EconomicsProposal>>>,
    burn_queue: Arc<RwLock<Vec<(CrapTokens, String)>>>,
}

impl TokenEconomics {
    /// Create new token economics engine
    pub fn new(ledger: Arc<TokenLedger>) -> Self {
        let config = EconomicsConfig::default();
        let supply_metrics = SupplyMetrics {
            total_supply: CrapTokens::from(0),
            circulating_supply: CrapTokens::from(0),
            staked_supply: CrapTokens::from(0),
            treasury_supply: CrapTokens::from(0),
            burned_supply: CrapTokens::from(0),
            inflation_rate: 0.0,
            deflation_rate: 0.0,
            velocity: 0.0,
            last_adjustment: Self::current_timestamp(),
        };
        
        let fee_structure = DynamicFeeStructure {
            base_fee: CrapTokens::from(1000), // 0.001 CRAP base fee
            congestion_multiplier: 1.0,
            priority_fee: CrapTokens::from(0),
            gas_price: 1.0,
            fee_categories: HashMap::new(),
        };
        
        Self {
            config: Arc::new(RwLock::new(config)),
            ledger,
            staking_positions: Arc::new(RwLock::new(HashMap::new())),
            supply_metrics: Arc::new(RwLock::new(supply_metrics)),
            fee_structure: Arc::new(RwLock::new(fee_structure)),
            liquidity_pools: Arc::new(RwLock::new(HashMap::new())),
            governance_proposals: Arc::new(RwLock::new(HashMap::new())),
            burn_queue: Arc::new(RwLock::new(Vec::new())),
        }
    }
    
    /// Create advanced staking position with enhanced features
    pub async fn stake_tokens(
        &self,
        staker: PeerId,
        amount: CrapTokens,
        lock_duration: Duration,
        compound_frequency: CompoundingFrequency,
    ) -> Result<()> {
        let config = self.config.read().await;
        
        // Validate minimum duration
        if lock_duration.as_secs() < config.min_stake_duration {
            return Err(Error::InvalidData(
                format!("Staking duration must be at least {} seconds", config.min_stake_duration)
            ));
        }
        
        // Calculate reward rate based on duration and amount
        let base_rate = config.base_staking_apy;
        let duration_bonus = self.calculate_duration_bonus(lock_duration).await;
        let amount_bonus = self.calculate_amount_bonus(amount).await;
        let loyalty_bonus = self.calculate_loyalty_bonus(staker).await;
        
        let total_rate = (base_rate + duration_bonus + amount_bonus + loyalty_bonus)
            .min(config.max_staking_apy);
        
        let now = Self::current_timestamp();
        let unlock_time = now + lock_duration.as_secs();
        
        let position = AdvancedStakingPosition {
            staker,
            amount,
            staked_at: now,
            lock_duration,
            unlock_time,
            reward_rate: total_rate,
            accumulated_rewards: CrapTokens::from(0),
            compound_frequency,
            delegation_target: None,
            early_unlock_penalty: 0.1, // 10% penalty for early unlock
            loyalty_bonus,
            governance_power: self.calculate_governance_power(amount, lock_duration).await,
        };
        
        // Lock tokens in staking
        {
            let mut positions = self.staking_positions.write().await;
            positions.entry(staker).or_insert_with(Vec::new).push(position);
        }
        
        // Update supply metrics
        {
            let mut metrics = self.supply_metrics.write().await;
            metrics.staked_supply = metrics.staked_supply.checked_add(amount)
                .ok_or_else(|| Error::InvalidData("Staking overflow".to_string()))?;
        }
        
        log::info!("Staked {} CRAP for {:?} with {}% APY", 
                  amount.to_crap(), staker, total_rate);
        
        Ok(())
    }
    
    /// Calculate staking rewards for all positions
    pub async fn calculate_staking_rewards(&self) -> Result<()> {
        let now = Self::current_timestamp();
        let mut total_rewards = CrapTokens::from(0);
        
        {
            let mut positions = self.staking_positions.write().await;
            
            for (staker, staker_positions) in positions.iter_mut() {
                for position in staker_positions.iter_mut() {
                    let time_elapsed = now - position.staked_at;
                    let annual_seconds = 365 * 24 * 3600;
                    
                    // Calculate rewards based on time elapsed
                    let reward_rate = position.reward_rate / 100.0; // Convert percentage
                    let time_factor = time_elapsed as f64 / annual_seconds as f64;
                    let rewards = (position.amount.0 as f64 * reward_rate * time_factor) as u64;
                    
                    let new_rewards = CrapTokens::from(rewards);
                    position.accumulated_rewards = position.accumulated_rewards
                        .checked_add(new_rewards)
                        .unwrap_or(position.accumulated_rewards);
                    
                    total_rewards = total_rewards.checked_add(new_rewards)
                        .unwrap_or(total_rewards);
                    
                    // Handle compounding based on frequency
                    match position.compound_frequency {
                        CompoundingFrequency::Daily if time_elapsed % (24 * 3600) == 0 => {
                            self.compound_rewards(staker, position).await?;
                        },
                        CompoundingFrequency::Weekly if time_elapsed % (7 * 24 * 3600) == 0 => {
                            self.compound_rewards(staker, position).await?;
                        },
                        CompoundingFrequency::Monthly if time_elapsed % (30 * 24 * 3600) == 0 => {
                            self.compound_rewards(staker, position).await?;
                        },
                        _ => {}
                    }
                }
            }
        }
        
        // Update supply metrics with minted rewards
        {
            let mut metrics = self.supply_metrics.write().await;
            metrics.total_supply = metrics.total_supply.checked_add(total_rewards)
                .unwrap_or(metrics.total_supply);
        }
        
        log::info!("Calculated {} CRAP total staking rewards", total_rewards.to_crap());
        Ok(())
    }
    
    /// Implement token burning mechanism
    pub async fn burn_tokens(&self, amount: CrapTokens, reason: String) -> Result<()> {
        {
            let mut metrics = self.supply_metrics.write().await;
            metrics.total_supply = metrics.total_supply.checked_sub(amount)
                .ok_or_else(|| Error::InvalidData("Cannot burn more than total supply".to_string()))?;
            metrics.burned_supply = metrics.burned_supply.checked_add(amount)
                .unwrap_or(metrics.burned_supply);
        }
        
        // Add to burn queue for transparency
        self.burn_queue.write().await.push((amount, reason.clone()));
        
        log::info!("Burned {} CRAP - Reason: {}", amount.to_crap(), reason);
        Ok(())
    }
    
    /// Calculate dynamic fees based on network conditions
    pub async fn calculate_dynamic_fee(&self, category: FeeCategory, priority: bool) -> CrapTokens {
        let fee_structure = self.fee_structure.read().await;
        let base_fee = fee_structure.fee_categories.get(&category)
            .copied()
            .unwrap_or(fee_structure.base_fee);
        
        let congestion_fee = CrapTokens::from(
            (base_fee.0 as f64 * fee_structure.congestion_multiplier) as u64
        );
        
        let priority_fee = if priority {
            fee_structure.priority_fee
        } else {
            CrapTokens::from(0)
        };
        
        congestion_fee.checked_add(priority_fee).unwrap_or(congestion_fee)
    }
    
    /// Update fee structure based on network congestion
    pub async fn update_fee_structure(&self, transaction_volume: u64) -> Result<()> {
        let mut fee_structure = self.fee_structure.write().await;
        
        // Adjust congestion multiplier based on volume
        fee_structure.congestion_multiplier = match transaction_volume {
            0..=100 => 1.0,
            101..=500 => 1.5,
            501..=1000 => 2.0,
            1001..=5000 => 3.0,
            _ => 5.0,
        };
        
        // Update category-specific fees
        fee_structure.fee_categories.insert(FeeCategory::Transfer, CrapTokens::from(500));
        fee_structure.fee_categories.insert(FeeCategory::GameBet, CrapTokens::from(100));
        fee_structure.fee_categories.insert(FeeCategory::Staking, CrapTokens::from(1000));
        fee_structure.fee_categories.insert(FeeCategory::Governance, CrapTokens::from(10000));
        fee_structure.fee_categories.insert(FeeCategory::LiquidityProvision, CrapTokens::from(2000));
        fee_structure.fee_categories.insert(FeeCategory::CrossChain, CrapTokens::from(5000));
        
        log::info!("Updated fee structure - congestion multiplier: {}x", 
                  fee_structure.congestion_multiplier);
        Ok(())
    }
    
    /// Get comprehensive economics statistics
    pub async fn get_economics_stats(&self) -> EconomicsStats {
        let config = self.config.read().await;
        let metrics = self.supply_metrics.read().await;
        let positions = self.staking_positions.read().await;
        let pools = self.liquidity_pools.read().await;
        let proposals = self.governance_proposals.read().await;
        
        let total_stakers = positions.len();
        let total_staking_positions = positions.values().map(|p| p.len()).sum::<usize>();
        let average_apy = positions.values()
            .flatten()
            .map(|p| p.reward_rate)
            .sum::<f64>() / total_staking_positions.max(1) as f64;
        
        let total_liquidity = pools.values()
            .map(|p| p.token_a_reserve.0 + p.token_b_reserve.0)
            .sum::<u64>();
        
        let active_proposals = proposals.values()
            .filter(|p| matches!(p.status, ProposalStatus::Active))
            .count();
        
        EconomicsStats {
            total_supply: metrics.total_supply,
            circulating_supply: metrics.circulating_supply,
            staked_supply: metrics.staked_supply,
            burned_supply: metrics.burned_supply,
            inflation_rate: metrics.inflation_rate,
            deflation_rate: metrics.deflation_rate,
            token_velocity: metrics.velocity,
            total_stakers,
            total_staking_positions,
            average_staking_apy: average_apy,
            base_staking_apy: config.base_staking_apy,
            max_staking_apy: config.max_staking_apy,
            total_liquidity_value: CrapTokens::from(total_liquidity),
            active_liquidity_pools: pools.len(),
            active_governance_proposals: active_proposals,
            treasury_fee_rate: config.treasury_fee,
            burn_rate: config.burn_rate,
        }
    }
    
    // Helper functions
    
    async fn calculate_duration_bonus(&self, duration: Duration) -> f64 {
        let days = duration.as_secs() / (24 * 3600);
        match days {
            0..=6 => 0.0,
            7..=30 => 1.0,   // +1% for 1 week to 1 month
            31..=90 => 2.5,  // +2.5% for 1-3 months
            91..=365 => 5.0, // +5% for 3 months to 1 year
            _ => 7.5,        // +7.5% for over 1 year
        }
    }
    
    async fn calculate_amount_bonus(&self, amount: CrapTokens) -> f64 {
        let crap_amount = amount.to_crap();
        match crap_amount as u64 {
            0..=999 => 0.0,
            1000..=9999 => 0.5,    // +0.5% for 1k-10k CRAP
            10000..=99999 => 1.0,  // +1% for 10k-100k CRAP
            100000..=999999 => 2.0, // +2% for 100k-1M CRAP
            _ => 3.0,              // +3% for over 1M CRAP
        }
    }
    
    async fn calculate_loyalty_bonus(&self, staker: PeerId) -> f64 {
        // This would check historical staking behavior
        // For now, return a default value
        0.0
    }
    
    async fn calculate_governance_power(&self, amount: CrapTokens, duration: Duration) -> u64 {
        let base_power = amount.0;
        let duration_multiplier = (duration.as_secs() / (30 * 24 * 3600)).max(1); // Monthly multiplier
        base_power * duration_multiplier
    }
    
    async fn compound_rewards(&self, _staker: &PeerId, position: &mut AdvancedStakingPosition) -> Result<()> {
        let rewards = position.accumulated_rewards;
        position.amount = position.amount.checked_add(rewards)
            .ok_or_else(|| Error::InvalidData("Compound overflow".to_string()))?;
        position.accumulated_rewards = CrapTokens::from(0);
        
        log::debug!("Compounded {} CRAP rewards for staking position", rewards.to_crap());
        Ok(())
    }
    
    fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
}

/// Comprehensive economics statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EconomicsStats {
    pub total_supply: CrapTokens,
    pub circulating_supply: CrapTokens,
    pub staked_supply: CrapTokens,
    pub burned_supply: CrapTokens,
    pub inflation_rate: f64,
    pub deflation_rate: f64,
    pub token_velocity: f64,
    pub total_stakers: usize,
    pub total_staking_positions: usize,
    pub average_staking_apy: f64,
    pub base_staking_apy: f64,
    pub max_staking_apy: f64,
    pub total_liquidity_value: CrapTokens,
    pub active_liquidity_pools: usize,
    pub active_governance_proposals: usize,
    pub treasury_fee_rate: f64,
    pub burn_rate: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token::TokenLedger;
    
    #[tokio::test]
    async fn test_token_economics_creation() {
        let ledger = Arc::new(TokenLedger::new());
        let economics = TokenEconomics::new(ledger);
        
        let stats = economics.get_economics_stats().await;
        assert_eq!(stats.total_stakers, 0);
        assert_eq!(stats.total_staking_positions, 0);
    }
    
    #[tokio::test]
    async fn test_dynamic_fees() {
        let ledger = Arc::new(TokenLedger::new());
        let economics = TokenEconomics::new(ledger);
        
        // Update for low volume
        economics.update_fee_structure(50).await.unwrap();
        let low_fee = economics.calculate_dynamic_fee(FeeCategory::Transfer, false).await;
        
        // Update for high volume
        economics.update_fee_structure(2000).await.unwrap();
        let high_fee = economics.calculate_dynamic_fee(FeeCategory::Transfer, false).await;
        
        assert!(high_fee.0 > low_fee.0);
    }
}