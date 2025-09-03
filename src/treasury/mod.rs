//! Advanced Treasury Management System
//!
//! This module implements sophisticated treasury management for the BitCraps
//! decentralized casino, including:
//! - Multi-wallet treasury management with hot/cold separation
//! - Automated market making for token liquidity
//! - Reserve management and risk assessment
//! - Fee collection and distribution mechanisms
//! - Liquidity pools with automated rebalancing
//! - Treasury governance and transparent operations

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

use crate::error::{Error, Result};
use crate::protocol::{CrapTokens, PeerId};
use crate::token::TokenLedger;

pub mod amm;
pub mod governance;
pub mod reserves;
pub mod risk_management;

/// Treasury configuration parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreasuryConfig {
    /// Minimum reserve ratio (reserved funds / total funds)
    pub min_reserve_ratio: f64,

    /// Target reserve ratio for optimal operations
    pub target_reserve_ratio: f64,

    /// Maximum single withdrawal as percentage of treasury
    pub max_withdrawal_ratio: f64,

    /// Hot wallet maximum balance percentage
    pub hot_wallet_limit: f64,

    /// Cold wallet minimum balance percentage
    pub cold_wallet_minimum: f64,

    /// Rebalancing threshold for hot/cold wallets
    pub rebalancing_threshold: f64,

    /// Fee distribution ratios
    pub fee_distribution: FeeDistribution,

    /// Risk management parameters
    pub risk_parameters: RiskParameters,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeDistribution {
    /// Percentage to treasury reserves
    pub treasury_reserve: f64,

    /// Percentage to staking rewards pool
    pub staking_rewards: f64,

    /// Percentage to development fund
    pub development_fund: f64,

    /// Percentage to marketing/community fund
    pub community_fund: f64,

    /// Percentage for token burns
    pub burn_allocation: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskParameters {
    /// Maximum exposure to single game
    pub max_game_exposure: f64,

    /// Value at Risk (VaR) threshold
    pub var_threshold: f64,

    /// Stress test scenarios
    pub stress_test_scenarios: Vec<StressScenario>,

    /// Insurance fund minimum
    pub insurance_minimum: CrapTokens,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StressScenario {
    pub name: String,
    pub description: String,
    pub loss_percentage: f64,
    pub probability: f64,
}

/// Multi-wallet treasury system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreasuryWallet {
    pub wallet_id: [u8; 32],
    pub wallet_type: WalletType,
    pub balance: CrapTokens,
    pub reserved_balance: CrapTokens,
    pub last_rebalance: u64,
    pub transaction_count: u64,
    pub security_level: SecurityLevel,
    pub multisig_threshold: Option<u8>,
    pub authorized_signers: Vec<PeerId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WalletType {
    Hot,     // For frequent transactions, lower security
    Cold,    // For long-term storage, high security
    Escrow,  // For game bets and settlements
    Reserve, // For emergency reserves
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecurityLevel {
    Low,    // Basic security for hot wallets
    Medium, // Enhanced security for operational wallets
    High,   // Maximum security for cold storage
}

/// Automated Market Maker for token liquidity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomatedMarketMaker {
    pub amm_id: [u8; 32],
    pub token_a_reserve: CrapTokens,
    pub token_b_reserve: CrapTokens, // Could be ETH, USDC, etc.
    pub liquidity_token_supply: CrapTokens,
    pub swap_fee_rate: f64,
    pub price_impact_limit: f64,
    pub slippage_tolerance: f64,
    pub last_price: f64,
    pub volume_24h: CrapTokens,
    pub fees_collected: CrapTokens,
    pub providers: HashMap<PeerId, LiquidityProvider>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidityProvider {
    pub provider_id: PeerId,
    pub liquidity_tokens: CrapTokens,
    pub original_deposit_a: CrapTokens,
    pub original_deposit_b: CrapTokens,
    pub rewards_earned: CrapTokens,
    pub deposit_time: u64,
    pub lock_end_time: Option<u64>,
}

/// Treasury operation record for transparency
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreasuryOperation {
    pub operation_id: [u8; 32],
    pub operation_type: OperationType,
    pub amount: CrapTokens,
    pub source_wallet: Option<[u8; 32]>,
    pub destination_wallet: Option<[u8; 32]>,
    pub initiator: PeerId,
    pub approvers: Vec<PeerId>,
    pub timestamp: u64,
    pub status: OperationStatus,
    pub reason: String,
    pub risk_assessment: RiskAssessment,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OperationType {
    Deposit,
    Withdrawal,
    Rebalancing,
    FeeCollection,
    ReserveAllocation,
    InsurancePayout,
    EmergencyFreeze,
    LiquidityProvision,
    MarketMaking,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OperationStatus {
    Pending,
    Approved,
    Executing,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskAssessment {
    pub risk_score: f64,
    pub var_impact: f64,
    pub stress_test_results: Vec<f64>,
    pub recommendation: RiskRecommendation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RiskRecommendation {
    Approve,
    ApproveWithConditions(Vec<String>),
    Reject(String),
    RequireAdditionalApprovals,
}

/// Main treasury management system
pub struct TreasuryManager {
    config: Arc<RwLock<TreasuryConfig>>,
    ledger: Arc<TokenLedger>,
    wallets: Arc<RwLock<HashMap<[u8; 32], TreasuryWallet>>>,
    amms: Arc<RwLock<HashMap<[u8; 32], AutomatedMarketMaker>>>,
    operations: Arc<RwLock<Vec<TreasuryOperation>>>,
    pending_operations: Arc<RwLock<HashMap<[u8; 32], TreasuryOperation>>>,
    reserve_funds: Arc<RwLock<HashMap<String, CrapTokens>>>,
    insurance_fund: Arc<RwLock<CrapTokens>>,
}

impl Default for TreasuryConfig {
    fn default() -> Self {
        Self {
            min_reserve_ratio: 0.2,      // 20% minimum reserves
            target_reserve_ratio: 0.3,   // 30% target reserves
            max_withdrawal_ratio: 0.05,  // 5% max single withdrawal
            hot_wallet_limit: 0.1,       // 10% in hot wallets max
            cold_wallet_minimum: 0.7,    // 70% in cold storage min
            rebalancing_threshold: 0.05, // 5% threshold triggers rebalancing
            fee_distribution: FeeDistribution {
                treasury_reserve: 0.4,  // 40% to reserves
                staking_rewards: 0.3,   // 30% to staking
                development_fund: 0.15, // 15% to development
                community_fund: 0.1,    // 10% to community
                burn_allocation: 0.05,  // 5% burned
            },
            risk_parameters: RiskParameters {
                max_game_exposure: 0.02, // 2% max exposure per game
                var_threshold: 0.1,      // 10% VaR threshold
                stress_test_scenarios: vec![
                    StressScenario {
                        name: "Market Crash".to_string(),
                        description: "50% token value drop".to_string(),
                        loss_percentage: 0.5,
                        probability: 0.01,
                    },
                    StressScenario {
                        name: "Bank Run".to_string(),
                        description: "Massive withdrawal demand".to_string(),
                        loss_percentage: 0.3,
                        probability: 0.05,
                    },
                    StressScenario {
                        name: "Smart Contract Bug".to_string(),
                        description: "Critical system vulnerability".to_string(),
                        loss_percentage: 0.2,
                        probability: 0.02,
                    },
                ],
                insurance_minimum: CrapTokens::from(50_000_000_000), // 50k CRAP
            },
        }
    }
}

impl TreasuryManager {
    /// Create new advanced treasury manager
    pub fn new(ledger: Arc<TokenLedger>) -> Self {
        Self {
            config: Arc::new(RwLock::new(TreasuryConfig::default())),
            ledger,
            wallets: Arc::new(RwLock::new(HashMap::new())),
            amms: Arc::new(RwLock::new(HashMap::new())),
            operations: Arc::new(RwLock::new(Vec::new())),
            pending_operations: Arc::new(RwLock::new(HashMap::new())),
            reserve_funds: Arc::new(RwLock::new(HashMap::new())),
            insurance_fund: Arc::new(RwLock::new(CrapTokens::from(0))),
        }
    }

    /// Initialize treasury with default wallets
    pub async fn initialize_treasury(&self, initial_balance: CrapTokens) -> Result<()> {
        let config = self.config.read().await;

        // Create hot wallet
        let hot_wallet = TreasuryWallet {
            wallet_id: self.generate_wallet_id("hot"),
            wallet_type: WalletType::Hot,
            balance: CrapTokens::from((initial_balance.0 as f64 * config.hot_wallet_limit) as u64),
            reserved_balance: CrapTokens::from(0),
            last_rebalance: Self::current_timestamp(),
            transaction_count: 0,
            security_level: SecurityLevel::Medium,
            multisig_threshold: Some(2),
            authorized_signers: Vec::new(),
        };

        // Create cold wallet
        let cold_wallet = TreasuryWallet {
            wallet_id: self.generate_wallet_id("cold"),
            wallet_type: WalletType::Cold,
            balance: CrapTokens::from(
                (initial_balance.0 as f64 * config.cold_wallet_minimum) as u64,
            ),
            reserved_balance: CrapTokens::from(0),
            last_rebalance: Self::current_timestamp(),
            transaction_count: 0,
            security_level: SecurityLevel::High,
            multisig_threshold: Some(3),
            authorized_signers: Vec::new(),
        };

        // Create escrow wallet for games
        let escrow_wallet = TreasuryWallet {
            wallet_id: self.generate_wallet_id("escrow"),
            wallet_type: WalletType::Escrow,
            balance: CrapTokens::from((initial_balance.0 as f64 * 0.15) as u64),
            reserved_balance: CrapTokens::from(0),
            last_rebalance: Self::current_timestamp(),
            transaction_count: 0,
            security_level: SecurityLevel::Medium,
            multisig_threshold: Some(2),
            authorized_signers: Vec::new(),
        };

        // Create reserve wallet
        let reserve_wallet = TreasuryWallet {
            wallet_id: self.generate_wallet_id("reserve"),
            wallet_type: WalletType::Reserve,
            balance: CrapTokens::from((initial_balance.0 as f64 * 0.05) as u64),
            reserved_balance: CrapTokens::from(0),
            last_rebalance: Self::current_timestamp(),
            transaction_count: 0,
            security_level: SecurityLevel::High,
            multisig_threshold: Some(4),
            authorized_signers: Vec::new(),
        };

        // Store wallets
        {
            let mut wallets = self.wallets.write().await;
            wallets.insert(hot_wallet.wallet_id, hot_wallet);
            wallets.insert(cold_wallet.wallet_id, cold_wallet);
            wallets.insert(escrow_wallet.wallet_id, escrow_wallet);
            wallets.insert(reserve_wallet.wallet_id, reserve_wallet);
        }

        // Initialize insurance fund
        let insurance_amount = CrapTokens::from((initial_balance.0 as f64 * 0.1) as u64);
        *self.insurance_fund.write().await = insurance_amount;

        log::info!(
            "Treasury initialized with {} CRAP across 4 wallets + insurance fund",
            initial_balance.to_crap()
        );

        Ok(())
    }

    /// Process fee collection and distribution
    pub async fn collect_and_distribute_fees(&self, fees_collected: CrapTokens) -> Result<()> {
        let config = self.config.read().await;
        let distribution = &config.fee_distribution;

        // Calculate allocations
        let reserve_amount =
            CrapTokens::from((fees_collected.0 as f64 * distribution.treasury_reserve) as u64);
        let staking_amount =
            CrapTokens::from((fees_collected.0 as f64 * distribution.staking_rewards) as u64);
        let dev_amount =
            CrapTokens::from((fees_collected.0 as f64 * distribution.development_fund) as u64);
        let community_amount =
            CrapTokens::from((fees_collected.0 as f64 * distribution.community_fund) as u64);
        let burn_amount =
            CrapTokens::from((fees_collected.0 as f64 * distribution.burn_allocation) as u64);

        // Distribute to reserves
        {
            let mut reserves = self.reserve_funds.write().await;
            let current = reserves
                .get("treasury_reserve")
                .copied()
                .unwrap_or(CrapTokens::from(0));
            reserves.insert(
                "treasury_reserve".to_string(),
                current.checked_add(reserve_amount).unwrap_or(current),
            );

            let current_staking = reserves
                .get("staking_rewards")
                .copied()
                .unwrap_or(CrapTokens::from(0));
            reserves.insert(
                "staking_rewards".to_string(),
                current_staking
                    .checked_add(staking_amount)
                    .unwrap_or(current_staking),
            );

            let current_dev = reserves
                .get("development_fund")
                .copied()
                .unwrap_or(CrapTokens::from(0));
            reserves.insert(
                "development_fund".to_string(),
                current_dev.checked_add(dev_amount).unwrap_or(current_dev),
            );

            let current_community = reserves
                .get("community_fund")
                .copied()
                .unwrap_or(CrapTokens::from(0));
            reserves.insert(
                "community_fund".to_string(),
                current_community
                    .checked_add(community_amount)
                    .unwrap_or(current_community),
            );
        }

        // Record operation
        let operation = TreasuryOperation {
            operation_id: self.generate_operation_id(),
            operation_type: OperationType::FeeCollection,
            amount: fees_collected,
            source_wallet: None,
            destination_wallet: None,
            initiator: [0u8; 32],       // System operation
            approvers: vec![[0u8; 32]], // Auto-approved
            timestamp: Self::current_timestamp(),
            status: OperationStatus::Completed,
            reason: "Automated fee collection and distribution".to_string(),
            risk_assessment: RiskAssessment {
                risk_score: 0.1, // Low risk for fee collection
                var_impact: 0.0,
                stress_test_results: Vec::new(),
                recommendation: RiskRecommendation::Approve,
            },
        };

        self.operations.write().await.push(operation);

        log::info!(
            "Distributed {} CRAP in fees: {}% to reserves, {}% to staking, {}% burned",
            fees_collected.to_crap(),
            distribution.treasury_reserve * 100.0,
            distribution.staking_rewards * 100.0,
            distribution.burn_allocation * 100.0
        );

        Ok(())
    }

    /// Create automated market maker pool
    pub async fn create_amm_pool(
        &self,
        token_a_reserve: CrapTokens,
        token_b_reserve: CrapTokens,
        swap_fee_rate: f64,
    ) -> Result<[u8; 32]> {
        let amm_id = self.generate_amm_id();
        let initial_liquidity = CrapTokens::from(
            ((token_a_reserve.0 as u128 * token_b_reserve.0 as u128) as f64).sqrt() as u64,
        );

        let amm = AutomatedMarketMaker {
            amm_id,
            token_a_reserve,
            token_b_reserve,
            liquidity_token_supply: initial_liquidity,
            swap_fee_rate,
            price_impact_limit: 0.05, // 5% max price impact
            slippage_tolerance: 0.01, // 1% slippage tolerance
            last_price: token_b_reserve.0 as f64 / token_a_reserve.0 as f64,
            volume_24h: CrapTokens::from(0),
            fees_collected: CrapTokens::from(0),
            providers: HashMap::new(),
        };

        self.amms.write().await.insert(amm_id, amm);

        log::info!(
            "Created AMM pool {} with {:.2} CRAP / {:.2} token_b reserves",
            hex::encode(&amm_id[..8]),
            token_a_reserve.to_crap(),
            token_b_reserve.to_crap()
        );

        Ok(amm_id)
    }

    /// Execute token swap through AMM
    pub async fn swap_tokens(
        &self,
        amm_id: [u8; 32],
        input_token_a: bool,
        input_amount: CrapTokens,
        min_output: CrapTokens,
        trader: PeerId,
    ) -> Result<CrapTokens> {
        let mut amms = self.amms.write().await;
        let amm = amms
            .get_mut(&amm_id)
            .ok_or_else(|| Error::InvalidData("AMM pool not found".to_string()))?;

        let (input_reserve, output_reserve) = if input_token_a {
            (amm.token_a_reserve.0, amm.token_b_reserve.0)
        } else {
            (amm.token_b_reserve.0, amm.token_a_reserve.0)
        };

        // Calculate output using constant product formula: x * y = k
        let input_with_fee = input_amount.0 as f64 * (1.0 - amm.swap_fee_rate);
        let output_amount =
            (output_reserve as f64 * input_with_fee) / (input_reserve as f64 + input_with_fee);

        let output_tokens = CrapTokens::from(output_amount as u64);

        // Check slippage tolerance
        if output_tokens < min_output {
            return Err(Error::InvalidData(
                "Slippage tolerance exceeded".to_string(),
            ));
        }

        // Check price impact
        let price_impact = (output_amount / output_reserve as f64).abs();
        if price_impact > amm.price_impact_limit {
            return Err(Error::InvalidData("Price impact too high".to_string()));
        }

        // Update reserves
        if input_token_a {
            amm.token_a_reserve = amm
                .token_a_reserve
                .checked_add(input_amount)
                .ok_or_else(|| Error::InvalidData("Reserve overflow".to_string()))?;
            amm.token_b_reserve = amm
                .token_b_reserve
                .checked_sub(output_tokens)
                .ok_or_else(|| Error::InvalidData("Insufficient reserves".to_string()))?;
        } else {
            amm.token_b_reserve = amm
                .token_b_reserve
                .checked_add(input_amount)
                .ok_or_else(|| Error::InvalidData("Reserve overflow".to_string()))?;
            amm.token_a_reserve = amm
                .token_a_reserve
                .checked_sub(output_tokens)
                .ok_or_else(|| Error::InvalidData("Insufficient reserves".to_string()))?;
        }

        // Update statistics
        amm.volume_24h = amm
            .volume_24h
            .checked_add(input_amount)
            .unwrap_or(amm.volume_24h);
        let fee_amount = CrapTokens::from((input_amount.0 as f64 * amm.swap_fee_rate) as u64);
        amm.fees_collected = amm
            .fees_collected
            .checked_add(fee_amount)
            .unwrap_or(amm.fees_collected);
        amm.last_price = amm.token_b_reserve.0 as f64 / amm.token_a_reserve.0 as f64;

        log::info!(
            "Swapped {} for {} tokens in AMM {} for trader {:?}",
            input_amount.to_crap(),
            output_tokens.to_crap(),
            hex::encode(&amm_id[..8]),
            trader
        );

        Ok(output_tokens)
    }

    /// Perform risk assessment for treasury operation
    pub async fn assess_operation_risk(&self, operation: &TreasuryOperation) -> RiskAssessment {
        let config = self.config.read().await;
        let total_balance = self.get_total_balance().await;

        let mut risk_score = 0.0;
        let mut stress_results = Vec::new();

        // Assess based on operation type and amount
        match operation.operation_type {
            OperationType::Withdrawal => {
                let withdrawal_ratio = operation.amount.0 as f64 / total_balance.0 as f64;
                risk_score = withdrawal_ratio * 10.0; // Higher risk for large withdrawals
            }
            OperationType::EmergencyFreeze => {
                risk_score = 5.0; // Medium-high risk
            }
            OperationType::FeeCollection => {
                risk_score = 0.1; // Low risk
            }
            _ => {
                risk_score = 1.0; // Default moderate risk
            }
        }

        // Run stress tests
        for scenario in &config.risk_parameters.stress_test_scenarios {
            let impact = operation.amount.0 as f64 * scenario.loss_percentage;
            stress_results.push(impact);
        }

        let recommendation = if risk_score < 2.0 {
            RiskRecommendation::Approve
        } else if risk_score < 5.0 {
            RiskRecommendation::ApproveWithConditions(vec![
                "Require additional approval".to_string(),
                "Monitor for 24 hours".to_string(),
            ])
        } else if risk_score < 8.0 {
            RiskRecommendation::RequireAdditionalApprovals
        } else {
            RiskRecommendation::Reject("Risk too high".to_string())
        };

        RiskAssessment {
            risk_score,
            var_impact: operation.amount.0 as f64 / total_balance.0 as f64,
            stress_test_results: stress_results,
            recommendation,
        }
    }

    /// Validate AMM invariants and maintain mathematical consistency
    pub async fn validate_amm_invariants(
        &self,
        amm_id: [u8; 32],
    ) -> Result<AmmInvariantValidation> {
        let amms = self.amms.read().await;
        let amm = amms
            .get(&amm_id)
            .ok_or_else(|| Error::InvalidData("AMM pool not found".to_string()))?;

        let mut validation = AmmInvariantValidation {
            amm_id,
            is_valid: true,
            violations: Vec::new(),
            k_constant: 0.0,
            price_deviation: 0.0,
            liquidity_balance: 0.0,
            reserve_ratio: 0.0,
        };

        // Invariant 1: Constant Product (k = x * y must be preserved across swaps)
        let k_constant = amm.token_a_reserve.0 as f64 * amm.token_b_reserve.0 as f64;
        validation.k_constant = k_constant;

        // Check if k is reasonable (not zero, not infinite)
        if k_constant <= 0.0 || !k_constant.is_finite() {
            validation.is_valid = false;
            validation
                .violations
                .push("K constant is invalid (zero, negative, or infinite)".to_string());
        }

        // Invariant 2: Reserve balance consistency
        let total_reserves = amm.token_a_reserve.0 + amm.token_b_reserve.0;
        if total_reserves == 0 {
            validation.is_valid = false;
            validation
                .violations
                .push("Total reserves cannot be zero".to_string());
        }

        let reserve_ratio = if amm.token_b_reserve.0 > 0 {
            amm.token_a_reserve.0 as f64 / amm.token_b_reserve.0 as f64
        } else {
            f64::INFINITY
        };
        validation.reserve_ratio = reserve_ratio;

        // Check for extreme reserve ratios (potential manipulation)
        if !reserve_ratio.is_finite() || reserve_ratio > 10000.0 || reserve_ratio < 0.0001 {
            validation.is_valid = false;
            validation.violations.push(format!(
                "Extreme reserve ratio detected: {:.6}",
                reserve_ratio
            ));
        }

        // Invariant 3: Liquidity token supply consistency
        let expected_liquidity = (k_constant.sqrt()) as u64;
        let actual_liquidity = amm.liquidity_token_supply.0;
        let liquidity_deviation = if expected_liquidity > 0 {
            (actual_liquidity as f64 / expected_liquidity as f64 - 1.0).abs()
        } else {
            1.0
        };
        validation.liquidity_balance = liquidity_deviation;

        // Allow up to 5% deviation in liquidity calculation due to fees and rounding
        if liquidity_deviation > 0.05 {
            validation.violations.push(format!(
                "Liquidity supply deviation too high: {:.2}% (expected: {}, actual: {})",
                liquidity_deviation * 100.0,
                expected_liquidity,
                actual_liquidity
            ));
        }

        // Invariant 4: Fee rate bounds
        if amm.swap_fee_rate < 0.0 || amm.swap_fee_rate > 0.1 {
            validation.is_valid = false;
            validation.violations.push(format!(
                "Invalid fee rate: {:.4} (must be 0-10%)",
                amm.swap_fee_rate
            ));
        }

        // Invariant 5: Price impact limits
        if amm.price_impact_limit <= 0.0 || amm.price_impact_limit > 1.0 {
            validation.is_valid = false;
            validation.violations.push(format!(
                "Invalid price impact limit: {:.2}%",
                amm.price_impact_limit * 100.0
            ));
        }

        // Invariant 6: Slippage tolerance bounds
        if amm.slippage_tolerance < 0.0 || amm.slippage_tolerance > 0.5 {
            validation.is_valid = false;
            validation.violations.push(format!(
                "Invalid slippage tolerance: {:.2}%",
                amm.slippage_tolerance * 100.0
            ));
        }

        // Invariant 7: Price consistency check
        let calculated_price = if amm.token_a_reserve.0 > 0 {
            amm.token_b_reserve.0 as f64 / amm.token_a_reserve.0 as f64
        } else {
            0.0
        };
        let price_deviation = if amm.last_price > 0.0 {
            (calculated_price / amm.last_price - 1.0).abs()
        } else {
            0.0
        };
        validation.price_deviation = price_deviation;

        // Allow up to 1% deviation in stored price vs calculated price
        if price_deviation > 0.01 {
            validation.violations.push(format!(
                "Price deviation too high: {:.2}% (stored: {:.6}, calculated: {:.6})",
                price_deviation * 100.0,
                amm.last_price,
                calculated_price
            ));
        }

        // Invariant 8: Volume and fees collected consistency
        if amm.fees_collected.0 as f64 > amm.volume_24h.0 as f64 * amm.swap_fee_rate {
            validation.is_valid = false;
            validation
                .violations
                .push("Fees collected exceed maximum possible from volume".to_string());
        }

        // Check for any violations
        if !validation.violations.is_empty() {
            validation.is_valid = false;
        }

        if validation.is_valid {
            log::debug!(
                "AMM invariants validated successfully for pool {}: k={:.2e}, ratio={:.6}",
                hex::encode(&amm_id[..8]),
                k_constant,
                reserve_ratio
            );
        } else {
            log::error!(
                "AMM invariant violations detected for pool {}: {:?}",
                hex::encode(&amm_id[..8]),
                validation.violations
            );
        }

        Ok(validation)
    }

    /// Validate all AMM pools and fix any inconsistencies
    pub async fn validate_all_amm_invariants(&self) -> Result<Vec<AmmInvariantValidation>> {
        // Collect AMM IDs first to avoid lock issues
        let amm_ids: Vec<[u8; 32]> = {
            let amms = self.amms.read().await;
            amms.keys().copied().collect()
        };

        let mut validations = Vec::new();

        for amm_id in amm_ids {
            let validation = self.validate_amm_invariants(amm_id).await?;
            validations.push(validation);
        }

        let total_pools = validations.len();
        let valid_pools = validations.iter().filter(|v| v.is_valid).count();
        let invalid_pools = total_pools - valid_pools;

        if invalid_pools > 0 {
            log::error!(
                "AMM invariant validation summary: {}/{} pools valid, {} violations found",
                valid_pools,
                total_pools,
                invalid_pools
            );
        } else {
            log::info!(
                "AMM invariant validation passed for all {} pools",
                total_pools
            );
        }

        Ok(validations)
    }

    /// Auto-fix AMM invariants where possible (emergency function)
    pub async fn auto_fix_amm_invariants(&self, amm_id: [u8; 32]) -> Result<bool> {
        let validation = self.validate_amm_invariants(amm_id).await?;

        if validation.is_valid {
            return Ok(false); // No fix needed
        }

        let mut amms = self.amms.write().await;
        let amm = amms
            .get_mut(&amm_id)
            .ok_or_else(|| Error::InvalidData("AMM pool not found".to_string()))?;

        let mut fixed = false;

        // Fix extreme fee rates
        if amm.swap_fee_rate < 0.0 {
            amm.swap_fee_rate = 0.003; // 0.3% default
            fixed = true;
        } else if amm.swap_fee_rate > 0.1 {
            amm.swap_fee_rate = 0.01; // 1% maximum
            fixed = true;
        }

        // Fix extreme price impact limits
        if amm.price_impact_limit <= 0.0 || amm.price_impact_limit > 1.0 {
            amm.price_impact_limit = 0.05; // 5% default
            fixed = true;
        }

        // Fix extreme slippage tolerance
        if amm.slippage_tolerance < 0.0 || amm.slippage_tolerance > 0.5 {
            amm.slippage_tolerance = 0.01; // 1% default
            fixed = true;
        }

        // Recalculate correct liquidity supply based on constant product
        if amm.token_a_reserve.0 > 0 && amm.token_b_reserve.0 > 0 {
            let k = amm.token_a_reserve.0 as f64 * amm.token_b_reserve.0 as f64;
            let correct_liquidity = k.sqrt() as u64;
            let deviation = if amm.liquidity_token_supply.0 > 0 {
                (correct_liquidity as f64 / amm.liquidity_token_supply.0 as f64 - 1.0).abs()
            } else {
                1.0
            };

            if deviation > 0.1 {
                // More than 10% deviation
                amm.liquidity_token_supply = CrapTokens::from(correct_liquidity);
                fixed = true;
            }
        }

        // Recalculate last price from reserves
        if amm.token_a_reserve.0 > 0 {
            let correct_price = amm.token_b_reserve.0 as f64 / amm.token_a_reserve.0 as f64;
            let price_deviation = if amm.last_price > 0.0 {
                (correct_price / amm.last_price - 1.0).abs()
            } else {
                1.0
            };

            if price_deviation > 0.05 {
                // More than 5% deviation
                amm.last_price = correct_price;
                fixed = true;
            }
        }

        if fixed {
            log::warn!(
                "Auto-fixed AMM invariants for pool {}: {} violations corrected",
                hex::encode(&amm_id[..8]),
                validation.violations.len()
            );
        }

        Ok(fixed)
    }

    /// Get comprehensive treasury statistics
    pub async fn get_treasury_stats(&self) -> TreasuryStats {
        let wallets = self.wallets.read().await;
        let amms = self.amms.read().await;
        let operations = self.operations.read().await;
        let reserves = self.reserve_funds.read().await;
        let insurance = *self.insurance_fund.read().await;

        let total_balance = wallets.values().map(|w| w.balance.0).sum::<u64>();
        let hot_balance = wallets
            .values()
            .filter(|w| matches!(w.wallet_type, WalletType::Hot))
            .map(|w| w.balance.0)
            .sum::<u64>();
        let cold_balance = wallets
            .values()
            .filter(|w| matches!(w.wallet_type, WalletType::Cold))
            .map(|w| w.balance.0)
            .sum::<u64>();

        let total_liquidity = amms
            .values()
            .map(|a| a.token_a_reserve.0 + a.token_b_reserve.0)
            .sum::<u64>();

        let total_reserves = reserves.values().map(|r| r.0).sum::<u64>();

        let completed_operations = operations
            .iter()
            .filter(|op| matches!(op.status, OperationStatus::Completed))
            .count();

        TreasuryStats {
            total_balance: CrapTokens::from(total_balance),
            hot_wallet_balance: CrapTokens::from(hot_balance),
            cold_wallet_balance: CrapTokens::from(cold_balance),
            reserve_balance: CrapTokens::from(total_reserves),
            insurance_fund_balance: insurance,
            total_liquidity: CrapTokens::from(total_liquidity),
            active_wallets: wallets.len(),
            active_amm_pools: amms.len(),
            total_operations: operations.len(),
            completed_operations,
            hot_wallet_ratio: hot_balance as f64 / total_balance as f64,
            cold_wallet_ratio: cold_balance as f64 / total_balance as f64,
            reserve_ratio: total_reserves as f64 / total_balance as f64,
        }
    }

    // Helper functions

    async fn get_total_balance(&self) -> CrapTokens {
        let wallets = self.wallets.read().await;
        let total = wallets.values().map(|w| w.balance.0).sum::<u64>();
        CrapTokens::from(total)
    }

    fn generate_wallet_id(&self, wallet_type: &str) -> [u8; 32] {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(wallet_type.as_bytes());
        hasher.update(Self::current_timestamp().to_be_bytes());
        let mut random_bytes = [0u8; 16];
        use rand::{rngs::OsRng, RngCore};
        OsRng.fill_bytes(&mut random_bytes);
        hasher.update(random_bytes);

        let result = hasher.finalize();
        let mut id = [0u8; 32];
        id.copy_from_slice(&result);
        id
    }

    fn generate_amm_id(&self) -> [u8; 32] {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(b"amm");
        hasher.update(Self::current_timestamp().to_be_bytes());
        let mut random_bytes = [0u8; 16];
        use rand::{rngs::OsRng, RngCore};
        OsRng.fill_bytes(&mut random_bytes);
        hasher.update(random_bytes);

        let result = hasher.finalize();
        let mut id = [0u8; 32];
        id.copy_from_slice(&result);
        id
    }

    fn generate_operation_id(&self) -> [u8; 32] {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(b"treasury_op");
        hasher.update(Self::current_timestamp().to_be_bytes());
        let mut random_bytes = [0u8; 16];
        use rand::{rngs::OsRng, RngCore};
        OsRng.fill_bytes(&mut random_bytes);
        hasher.update(random_bytes);

        let result = hasher.finalize();
        let mut id = [0u8; 32];
        id.copy_from_slice(&result);
        id
    }

    fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
}

/// AMM invariant validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmmInvariantValidation {
    pub amm_id: [u8; 32],
    pub is_valid: bool,
    pub violations: Vec<String>,
    pub k_constant: f64,
    pub price_deviation: f64,
    pub liquidity_balance: f64,
    pub reserve_ratio: f64,
}

/// Treasury statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreasuryStats {
    pub total_balance: CrapTokens,
    pub hot_wallet_balance: CrapTokens,
    pub cold_wallet_balance: CrapTokens,
    pub reserve_balance: CrapTokens,
    pub insurance_fund_balance: CrapTokens,
    pub total_liquidity: CrapTokens,
    pub active_wallets: usize,
    pub active_amm_pools: usize,
    pub total_operations: usize,
    pub completed_operations: usize,
    pub hot_wallet_ratio: f64,
    pub cold_wallet_ratio: f64,
    pub reserve_ratio: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token::TokenLedger;

    #[tokio::test]
    async fn test_treasury_initialization() {
        let ledger = Arc::new(TokenLedger::new());
        let treasury = TreasuryManager::new(ledger);

        let initial_balance = CrapTokens::from(1_000_000_000_000); // 1M CRAP
        treasury.initialize_treasury(initial_balance).await.unwrap();

        let stats = treasury.get_treasury_stats().await;
        assert_eq!(stats.total_balance.0, initial_balance.0);
        assert_eq!(stats.active_wallets, 4); // hot, cold, escrow, reserve
    }

    #[tokio::test]
    async fn test_amm_creation_and_swap() {
        let ledger = Arc::new(TokenLedger::new());
        let treasury = TreasuryManager::new(ledger);

        let token_a = CrapTokens::from(1_000_000_000); // 1k CRAP
        let token_b = CrapTokens::from(10_000_000_000); // 10k token_b

        let amm_id = treasury
            .create_amm_pool(token_a, token_b, 0.003)
            .await
            .unwrap();

        let swap_input = CrapTokens::from(1_000_000); // 1 CRAP
        let min_output = CrapTokens::from(9_000_000); // Expect ~10 token_b
        let trader = [1u8; 32];

        let output = treasury
            .swap_tokens(amm_id, true, swap_input, min_output, trader)
            .await
            .unwrap();
        assert!(output.0 > 0);
    }
}
