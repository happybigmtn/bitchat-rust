//! Economic Models for Token Economics Analysis
//!
//! This module implements sophisticated mathematical models for analyzing
//! and predicting token economics behavior, including:
//! - Token valuation models using various approaches
//! - Liquidity analysis and market depth models
//! - Risk assessment models for treasury management
//! - Game theory models for mechanism design
//! - Network effects and adoption models
//! - Monte Carlo simulations for scenario analysis

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};

use crate::protocol::CrapTokens;
use crate::error::{Error, Result};

/// Token valuation model using multiple approaches
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenValuationModel {
    /// Network Value-to-Transaction (NVT) ratio
    pub nvt_ratio: f64,

    /// Market cap to total supply ratio
    pub market_cap_ratio: f64,

    /// Token velocity metrics
    pub velocity_metrics: VelocityMetrics,

    /// Metcalfe's law valuation (value ∝ users²)
    pub metcalfe_valuation: f64,

    /// Discounted cash flow model parameters
    pub dcf_model: DiscountedCashFlowModel,

    /// Comparable analysis with other tokens
    pub comparable_analysis: ComparableAnalysis,

    /// Fundamental value based on utility
    pub fundamental_value: FundamentalValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VelocityMetrics {
    /// Token velocity (transactions per token per time period)
    pub velocity: f64,

    /// Average holding time
    pub average_hold_time: Duration,

    /// Turnover ratio (trading volume / market cap)
    pub turnover_ratio: f64,

    /// Dormancy flow (old coins vs new coins moved)
    pub dormancy_flow: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscountedCashFlowModel {
    /// Expected future cash flows
    pub future_cash_flows: Vec<f64>,

    /// Discount rate (risk-free rate + risk premium)
    pub discount_rate: f64,

    /// Terminal value growth rate
    pub terminal_growth_rate: f64,

    /// Present value of cash flows
    pub present_value: f64,

    /// Risk premium for token volatility
    pub risk_premium: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparableAnalysis {
    /// Price-to-earnings equivalents for similar tokens
    pub pe_multiples: Vec<f64>,

    /// Price-to-book equivalents
    pub pb_multiples: Vec<f64>,

    /// Enterprise value to revenue ratios
    pub ev_revenue_ratios: Vec<f64>,

    /// Market cap to active users ratios
    pub mc_user_ratios: Vec<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundamentalValue {
    /// Value derived from gaming utility
    pub gaming_utility_value: f64,

    /// Value from staking rewards
    pub staking_value: f64,

    /// Value from governance rights
    pub governance_value: f64,

    /// Network effect value
    pub network_effect_value: f64,

    /// Scarcity premium
    pub scarcity_premium: f64,
}

/// Liquidity analysis model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidityModel {
    /// Market depth at various price levels
    pub market_depth: MarketDepth,

    /// Order book analysis
    pub order_book_metrics: OrderBookMetrics,

    /// Liquidity risk assessment
    pub liquidity_risk: LiquidityRisk,

    /// Market impact functions
    pub market_impact: MarketImpactModel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketDepth {
    /// Bid-ask spread at different percentiles
    pub bid_ask_spreads: HashMap<u32, f64>, // percentile -> spread

    /// Order book depth (quantity available at price levels)
    pub depth_levels: Vec<DepthLevel>,

    /// Price impact for different trade sizes
    pub price_impacts: HashMap<u64, f64>, // trade_size -> price_impact
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepthLevel {
    pub price: f64,
    pub bid_quantity: f64,
    pub ask_quantity: f64,
    pub cumulative_bid: f64,
    pub cumulative_ask: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookMetrics {
    /// Order book imbalance ratio
    pub imbalance_ratio: f64,

    /// Weighted mid price
    pub weighted_mid_price: f64,

    /// Order flow toxicity measure
    pub order_flow_toxicity: f64,

    /// Microstructure noise level
    pub microstructure_noise: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidityRisk {
    /// Liquidity-at-Risk (LaR) measure
    pub liquidity_at_risk: f64,

    /// Funding liquidity risk
    pub funding_liquidity_risk: f64,

    /// Market liquidity risk
    pub market_liquidity_risk: f64,

    /// Concentration risk (single large holders)
    pub concentration_risk: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketImpactModel {
    /// Linear market impact coefficient
    pub linear_impact: f64,

    /// Square-root market impact coefficient
    pub sqrt_impact: f64,

    /// Temporary vs permanent impact ratio
    pub temp_permanent_ratio: f64,

    /// Recovery half-life for temporary impact
    pub recovery_half_life: Duration,
}

/// Risk assessment models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskModel {
    /// Value-at-Risk (VaR) calculations
    pub var_analysis: VarAnalysis,

    /// Stress testing results
    pub stress_tests: StressTestResults,

    /// Correlation analysis with other assets
    pub correlation_analysis: CorrelationAnalysis,

    /// Volatility models
    pub volatility_models: VolatilityModels,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VarAnalysis {
    /// 1-day VaR at different confidence levels
    pub daily_var: HashMap<u32, f64>, // confidence_level -> var

    /// 10-day VaR projections
    pub var_10day: HashMap<u32, f64>,

    /// Expected shortfall (conditional VaR)
    pub expected_shortfall: HashMap<u32, f64>,

    /// Back-testing results for VaR model accuracy
    pub backtesting_results: BacktestingResults,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestingResults {
    /// Number of VaR violations
    pub violations: u32,

    /// Total observations
    pub total_observations: u32,

    /// Violation rate (should match confidence level)
    pub violation_rate: f64,

    /// Kupiec test p-value
    pub kupiec_test_pvalue: f64,

    /// Model accuracy score
    pub accuracy_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StressTestResults {
    /// Scenario-based stress test results
    pub scenario_results: Vec<StressScenarioResult>,

    /// Monte Carlo stress test summary
    pub monte_carlo_summary: MonteCarloSummary,

    /// Reverse stress test (break-even point)
    pub reverse_stress_test: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StressScenarioResult {
    pub scenario_name: String,
    pub probability: f64,
    pub token_price_impact: f64,
    pub liquidity_impact: f64,
    pub treasury_impact: f64,
    pub recovery_time: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonteCarloSummary {
    /// Number of simulation runs
    pub simulation_runs: u32,

    /// Percentile outcomes
    pub outcome_percentiles: HashMap<u32, f64>,

    /// Probability of loss exceeding thresholds
    pub loss_probabilities: HashMap<f64, f64>, // threshold -> probability

    /// Expected loss under adverse conditions
    pub expected_adverse_loss: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelationAnalysis {
    /// Correlation with major cryptocurrencies
    pub crypto_correlations: HashMap<String, f64>, // symbol -> correlation

    /// Correlation with traditional assets
    pub traditional_correlations: HashMap<String, f64>,

    /// Rolling correlation windows
    pub rolling_correlations: Vec<RollingCorrelation>,

    /// Tail dependence measures
    pub tail_dependence: HashMap<String, f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollingCorrelation {
    pub timestamp: u64,
    pub window_days: u32,
    pub correlations: HashMap<String, f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolatilityModels {
    /// Historical volatility
    pub historical_volatility: f64,

    /// GARCH model parameters
    pub garch_model: GarchModel,

    /// Implied volatility (if options exist)
    pub implied_volatility: Option<f64>,

    /// Volatility clustering measures
    pub volatility_clustering: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GarchModel {
    pub omega: f64,      // Long-term variance component
    pub alpha: f64,      // ARCH parameter
    pub beta: f64,       // GARCH parameter
    pub conditional_variance: f64,
    pub log_likelihood: f64,
}

/// Game theory models for mechanism design
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameTheoryModel {
    /// Nash equilibrium analysis for different player strategies
    pub nash_equilibria: Vec<NashEquilibrium>,

    /// Mechanism design properties
    pub mechanism_properties: MechanismProperties,

    /// Auction theory models for token distribution
    pub auction_models: AuctionModels,

    /// Coalition formation analysis
    pub coalition_analysis: CoalitionAnalysis,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NashEquilibrium {
    pub equilibrium_id: String,
    pub player_strategies: HashMap<String, Strategy>,
    pub equilibrium_payoffs: HashMap<String, f64>,
    pub stability_score: f64,
    pub social_welfare: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Strategy {
    pub strategy_name: String,
    pub strategy_parameters: HashMap<String, f64>,
    pub expected_payoff: f64,
    pub risk_level: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MechanismProperties {
    /// Incentive compatibility (truthfulness)
    pub incentive_compatible: bool,

    /// Individual rationality (participation constraint)
    pub individual_rational: bool,

    /// Budget balance
    pub budget_balanced: bool,

    /// Efficiency (social welfare maximization)
    pub efficient: bool,

    /// Revenue optimization score
    pub revenue_optimization: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuctionModels {
    /// First-price sealed-bid auction results
    pub first_price_auction: AuctionResult,

    /// Second-price (Vickrey) auction results
    pub second_price_auction: AuctionResult,

    /// Dutch auction dynamics
    pub dutch_auction: AuctionResult,

    /// English auction with ascending prices
    pub english_auction: AuctionResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuctionResult {
    pub mechanism_name: String,
    pub optimal_reserve_price: f64,
    pub expected_revenue: f64,
    pub winner_surplus: f64,
    pub seller_surplus: f64,
    pub efficiency_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoalitionAnalysis {
    /// Shapley values for fair allocation
    pub shapley_values: HashMap<String, f64>,

    /// Core stability analysis
    pub core_stable_allocations: Vec<HashMap<String, f64>>,

    /// Bargaining power indices
    pub bargaining_power: HashMap<String, f64>,

    /// Coalition formation probabilities
    pub coalition_probabilities: HashMap<String, f64>,
}

/// Network effects and adoption models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkEffectsModel {
    /// Metcalfe's law analysis (value ∝ n²)
    pub metcalfe_analysis: MetcalfeAnalysis,

    /// Reed's law analysis (value ∝ 2ⁿ for groups)
    pub reed_analysis: ReedAnalysis,

    /// Adoption curve modeling
    pub adoption_curve: AdoptionCurve,

    /// Network density and connectivity metrics
    pub network_metrics: NetworkMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetcalfeAnalysis {
    pub active_users: u64,
    pub connections: u64,
    pub metcalfe_value: f64,
    pub value_per_user: f64,
    pub network_density: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReedAnalysis {
    pub possible_groups: u64,
    pub active_groups: u64,
    pub reed_value: f64,
    pub group_formation_rate: f64,
    pub group_utility_factor: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdoptionCurve {
    /// S-curve adoption parameters
    pub adoption_model: AdoptionModel,

    /// Current adoption phase
    pub current_phase: AdoptionPhase,

    /// Projected user growth
    pub growth_projections: Vec<GrowthProjection>,

    /// Viral coefficient (users brought per user)
    pub viral_coefficient: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AdoptionModel {
    Logistic { carrying_capacity: u64, growth_rate: f64 },
    Gompertz { asymptote: u64, displacement: f64, growth_rate: f64 },
    Bass { market_potential: u64, innovation_coeff: f64, imitation_coeff: f64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AdoptionPhase {
    Introduction,
    EarlyAdoption,
    EarlyMajority,
    LateMajority,
    Laggards,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrowthProjection {
    pub timestamp: u64,
    pub projected_users: u64,
    pub confidence_interval: (u64, u64),
    pub growth_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkMetrics {
    pub clustering_coefficient: f64,
    pub average_path_length: f64,
    pub degree_distribution: Vec<(u32, f64)>, // (degree, probability)
    pub centrality_measures: CentralityMeasures,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CentralityMeasures {
    pub betweenness_centrality: HashMap<String, f64>,
    pub closeness_centrality: HashMap<String, f64>,
    pub eigenvector_centrality: HashMap<String, f64>,
    pub page_rank: HashMap<String, f64>,
}

/// Main economic models engine
pub struct EconomicModelsEngine {
    valuation_model: Arc<std::sync::RwLock<TokenValuationModel>>,
    liquidity_model: Arc<std::sync::RwLock<LiquidityModel>>,
    risk_model: Arc<std::sync::RwLock<RiskModel>>,
    game_theory_model: Arc<std::sync::RwLock<GameTheoryModel>>,
    network_effects_model: Arc<std::sync::RwLock<NetworkEffectsModel>>,
    historical_data: Arc<std::sync::RwLock<Vec<MarketDataPoint>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketDataPoint {
    pub timestamp: u64,
    pub price: f64,
    pub volume: f64,
    pub market_cap: f64,
    pub active_users: u64,
    pub transaction_count: u64,
    pub staked_amount: f64,
}

impl EconomicModelsEngine {
    /// Create new economic models engine
    pub fn new() -> Self {
        Self {
            valuation_model: Arc::new(std::sync::RwLock::new(Self::create_default_valuation_model())),
            liquidity_model: Arc::new(std::sync::RwLock::new(Self::create_default_liquidity_model())),
            risk_model: Arc::new(std::sync::RwLock::new(Self::create_default_risk_model())),
            game_theory_model: Arc::new(std::sync::RwLock::new(Self::create_default_game_theory_model())),
            network_effects_model: Arc::new(std::sync::RwLock::new(Self::create_default_network_effects_model())),
            historical_data: Arc::new(std::sync::RwLock::new(Vec::new())),
        }
    }

    /// Update token valuation using multiple models
    pub fn calculate_token_valuation(&self, market_data: &MarketDataPoint) -> Result<f64> {
        let mut valuation = self.valuation_model.write().unwrap();

        // Update NVT ratio
        if market_data.transaction_count > 0 {
            valuation.nvt_ratio = market_data.market_cap / (market_data.transaction_count as f64);
        }

        // Update velocity metrics
        let total_supply = 21_000_000.0; // 21M CRAP total supply
        if market_data.volume > 0.0 && market_data.market_cap > 0.0 {
            valuation.velocity_metrics.velocity = market_data.volume / market_data.market_cap;
            valuation.velocity_metrics.turnover_ratio = market_data.volume / market_data.market_cap;
            valuation.velocity_metrics.average_hold_time = Duration::from_secs(
                (86400.0 / valuation.velocity_metrics.velocity) as u64
            );
        }

        // Update Metcalfe's law valuation
        valuation.metcalfe_valuation = (market_data.active_users as f64).powi(2) * 0.0001;

        // Update fundamental value components
        valuation.fundamental_value.gaming_utility_value =
            market_data.active_users as f64 * 0.50; // $0.50 per active user
        valuation.fundamental_value.staking_value =
            market_data.staked_amount * 0.05; // 5% staking premium
        valuation.fundamental_value.network_effect_value =
            valuation.metcalfe_valuation * 0.1;

        // Calculate composite valuation
        let composite_value =
            valuation.fundamental_value.gaming_utility_value +
            valuation.fundamental_value.staking_value +
            valuation.fundamental_value.network_effect_value +
            valuation.fundamental_value.governance_value +
            valuation.fundamental_value.scarcity_premium;

        Ok(composite_value / total_supply) // Per-token value
    }

    /// Perform Monte Carlo risk simulation
    pub fn monte_carlo_simulation(&self, scenarios: u32, time_horizon_days: u32) -> Result<MonteCarloSummary> {
        let mut outcomes = Vec::with_capacity(scenarios as usize);
        let historical_data = self.historical_data.read().unwrap();

        if historical_data.is_empty() {
            return Err(Error::InvalidData("No historical data available".to_string()));
        }

        // Calculate daily returns
        let daily_returns: Vec<f64> = historical_data.windows(2)
            .map(|w| (w[1].price / w[0].price - 1.0))
            .collect();

        if daily_returns.is_empty() {
            return Err(Error::InvalidData("Insufficient data for simulation".to_string()));
        }

        let mean_return = daily_returns.iter().sum::<f64>() / daily_returns.len() as f64;
        let std_dev = {
            let variance = daily_returns.iter()
                .map(|r| (r - mean_return).powi(2))
                .sum::<f64>() / (daily_returns.len() - 1) as f64;
            variance.sqrt()
        };

        // Run Monte Carlo simulations
        for _ in 0..scenarios {
            let mut current_price = historical_data.last().unwrap().price;
            let mut cumulative_return = 0.0;

            for _ in 0..time_horizon_days {
                // Generate random return using normal distribution (simplified)
                let random_return = mean_return + std_dev * self.generate_normal_random();
                current_price *= 1.0 + random_return;
                cumulative_return += random_return;
            }

            outcomes.push(cumulative_return);
        }

        outcomes.sort_by(|a, b| a.partial_cmp(b).unwrap());

        // Calculate percentiles
        let mut outcome_percentiles = HashMap::new();
        for percentile in [5, 10, 25, 50, 75, 90, 95] {
            let index = (scenarios as f64 * percentile as f64 / 100.0) as usize;
            outcome_percentiles.insert(percentile, outcomes[index.min(outcomes.len() - 1)]);
        }

        // Calculate loss probabilities
        let mut loss_probabilities = HashMap::new();
        for threshold in [0.1, 0.2, 0.3, 0.5] {
            let losses = outcomes.iter().filter(|&&r| r < -threshold).count();
            loss_probabilities.insert(threshold, losses as f64 / scenarios as f64);
        }

        let expected_adverse_loss = outcomes.iter()
            .filter(|&&r| r < outcome_percentiles[&5])
            .sum::<f64>() / outcomes.iter().filter(|&&r| r < outcome_percentiles[&5]).count() as f64;

        Ok(MonteCarloSummary {
            simulation_runs: scenarios,
            outcome_percentiles,
            loss_probabilities,
            expected_adverse_loss,
        })
    }

    /// Calculate optimal auction parameters
    pub fn optimize_auction_mechanism(&self, bidders: u32, value_distribution: &str) -> Result<AuctionResult> {
        // Simplified auction optimization
        let reserve_price = match value_distribution {
            "uniform" => 0.5, // Reserve at median value
            "exponential" => 0.3, // Lower reserve for skewed distribution
            "normal" => 0.6, // Higher reserve for normal distribution
            _ => 0.5,
        };

        let efficiency_score = match bidders {
            1..=5 => 0.7,
            6..=20 => 0.85,
            21..=100 => 0.95,
            _ => 0.98,
        };

        let expected_revenue = reserve_price * bidders as f64 * 0.8; // Simplified calculation

        Ok(AuctionResult {
            mechanism_name: "Second-Price Sealed-Bid".to_string(),
            optimal_reserve_price: reserve_price,
            expected_revenue,
            winner_surplus: expected_revenue * 0.15,
            seller_surplus: expected_revenue * 0.85,
            efficiency_score,
        })
    }

    /// Analyze network effects using Metcalfe's law
    pub fn analyze_network_effects(&self, current_users: u64, connections_per_user: f64) -> MetcalfeAnalysis {
        let connections = (current_users as f64 * connections_per_user) as u64;
        let max_connections = current_users * (current_users - 1) / 2;
        let network_density = if max_connections > 0 {
            connections as f64 / max_connections as f64
        } else {
            0.0
        };

        let metcalfe_value = (current_users as f64).powi(2) * network_density * 0.0001;
        let value_per_user = if current_users > 0 {
            metcalfe_value / current_users as f64
        } else {
            0.0
        };

        MetcalfeAnalysis {
            active_users: current_users,
            connections,
            metcalfe_value,
            value_per_user,
            network_density,
        }
    }

    /// Add new market data point
    pub fn add_market_data(&self, data_point: MarketDataPoint) {
        let mut historical_data = self.historical_data.write().unwrap();
        historical_data.push(data_point);

        // Keep only last 1000 data points to manage memory
        if historical_data.len() > 1000 {
            historical_data.drain(0..historical_data.len() - 1000);
        }
    }

    // Helper functions

    fn create_default_valuation_model() -> TokenValuationModel {
        TokenValuationModel {
            nvt_ratio: 0.0,
            market_cap_ratio: 0.0,
            velocity_metrics: VelocityMetrics {
                velocity: 1.0,
                average_hold_time: Duration::from_secs(86400),
                turnover_ratio: 0.1,
                dormancy_flow: 0.5,
            },
            metcalfe_valuation: 0.0,
            dcf_model: DiscountedCashFlowModel {
                future_cash_flows: vec![100.0, 110.0, 120.0, 130.0, 140.0],
                discount_rate: 0.1,
                terminal_growth_rate: 0.03,
                present_value: 0.0,
                risk_premium: 0.05,
            },
            comparable_analysis: ComparableAnalysis {
                pe_multiples: vec![15.0, 20.0, 25.0],
                pb_multiples: vec![2.0, 3.0, 4.0],
                ev_revenue_ratios: vec![5.0, 7.0, 10.0],
                mc_user_ratios: vec![100.0, 200.0, 300.0],
            },
            fundamental_value: FundamentalValue {
                gaming_utility_value: 0.0,
                staking_value: 0.0,
                governance_value: 10.0,
                network_effect_value: 0.0,
                scarcity_premium: 5.0,
            },
        }
    }

    fn create_default_liquidity_model() -> LiquidityModel {
        LiquidityModel {
            market_depth: MarketDepth {
                bid_ask_spreads: [(50, 0.01), (75, 0.015), (90, 0.025)].iter().cloned().collect(),
                depth_levels: Vec::new(),
                price_impacts: HashMap::new(),
            },
            order_book_metrics: OrderBookMetrics {
                imbalance_ratio: 0.0,
                weighted_mid_price: 0.0,
                order_flow_toxicity: 0.0,
                microstructure_noise: 0.01,
            },
            liquidity_risk: LiquidityRisk {
                liquidity_at_risk: 0.05,
                funding_liquidity_risk: 0.03,
                market_liquidity_risk: 0.02,
                concentration_risk: 0.1,
            },
            market_impact: MarketImpactModel {
                linear_impact: 0.001,
                sqrt_impact: 0.01,
                temp_permanent_ratio: 0.7,
                recovery_half_life: Duration::from_secs(3600),
            },
        }
    }

    fn create_default_risk_model() -> RiskModel {
        RiskModel {
            var_analysis: VarAnalysis {
                daily_var: [(95, 0.05), (99, 0.08)].iter().cloned().collect(),
                var_10day: [(95, 0.15), (99, 0.25)].iter().cloned().collect(),
                expected_shortfall: [(95, 0.07), (99, 0.12)].iter().cloned().collect(),
                backtesting_results: BacktestingResults {
                    violations: 5,
                    total_observations: 100,
                    violation_rate: 0.05,
                    kupiec_test_pvalue: 0.8,
                    accuracy_score: 0.95,
                },
            },
            stress_tests: StressTestResults {
                scenario_results: Vec::new(),
                monte_carlo_summary: MonteCarloSummary {
                    simulation_runs: 0,
                    outcome_percentiles: HashMap::new(),
                    loss_probabilities: HashMap::new(),
                    expected_adverse_loss: 0.0,
                },
                reverse_stress_test: 0.3,
            },
            correlation_analysis: CorrelationAnalysis {
                crypto_correlations: [
                    ("BTC".to_string(), 0.7),
                    ("ETH".to_string(), 0.6),
                ].iter().cloned().collect(),
                traditional_correlations: [
                    ("S&P500".to_string(), 0.3),
                ].iter().cloned().collect(),
                rolling_correlations: Vec::new(),
                tail_dependence: HashMap::new(),
            },
            volatility_models: VolatilityModels {
                historical_volatility: 0.5,
                garch_model: GarchModel {
                    omega: 0.001,
                    alpha: 0.1,
                    beta: 0.85,
                    conditional_variance: 0.01,
                    log_likelihood: -100.0,
                },
                implied_volatility: None,
                volatility_clustering: 0.3,
            },
        }
    }

    fn create_default_game_theory_model() -> GameTheoryModel {
        GameTheoryModel {
            nash_equilibria: Vec::new(),
            mechanism_properties: MechanismProperties {
                incentive_compatible: true,
                individual_rational: true,
                budget_balanced: true,
                efficient: false,
                revenue_optimization: 0.8,
            },
            auction_models: AuctionModels {
                first_price_auction: AuctionResult {
                    mechanism_name: "First-Price".to_string(),
                    optimal_reserve_price: 0.5,
                    expected_revenue: 100.0,
                    winner_surplus: 20.0,
                    seller_surplus: 80.0,
                    efficiency_score: 0.85,
                },
                second_price_auction: AuctionResult {
                    mechanism_name: "Second-Price".to_string(),
                    optimal_reserve_price: 0.4,
                    expected_revenue: 95.0,
                    winner_surplus: 25.0,
                    seller_surplus: 70.0,
                    efficiency_score: 0.95,
                },
                dutch_auction: AuctionResult {
                    mechanism_name: "Dutch".to_string(),
                    optimal_reserve_price: 0.6,
                    expected_revenue: 105.0,
                    winner_surplus: 15.0,
                    seller_surplus: 90.0,
                    efficiency_score: 0.75,
                },
                english_auction: AuctionResult {
                    mechanism_name: "English".to_string(),
                    optimal_reserve_price: 0.3,
                    expected_revenue: 110.0,
                    winner_surplus: 30.0,
                    seller_surplus: 80.0,
                    efficiency_score: 0.98,
                },
            },
            coalition_analysis: CoalitionAnalysis {
                shapley_values: HashMap::new(),
                core_stable_allocations: Vec::new(),
                bargaining_power: HashMap::new(),
                coalition_probabilities: HashMap::new(),
            },
        }
    }

    fn create_default_network_effects_model() -> NetworkEffectsModel {
        NetworkEffectsModel {
            metcalfe_analysis: MetcalfeAnalysis {
                active_users: 0,
                connections: 0,
                metcalfe_value: 0.0,
                value_per_user: 0.0,
                network_density: 0.0,
            },
            reed_analysis: ReedAnalysis {
                possible_groups: 0,
                active_groups: 0,
                reed_value: 0.0,
                group_formation_rate: 0.1,
                group_utility_factor: 1.5,
            },
            adoption_curve: AdoptionCurve {
                adoption_model: AdoptionModel::Logistic {
                    carrying_capacity: 1_000_000,
                    growth_rate: 0.1,
                },
                current_phase: AdoptionPhase::Introduction,
                growth_projections: Vec::new(),
                viral_coefficient: 0.5,
            },
            network_metrics: NetworkMetrics {
                clustering_coefficient: 0.3,
                average_path_length: 4.5,
                degree_distribution: Vec::new(),
                centrality_measures: CentralityMeasures {
                    betweenness_centrality: HashMap::new(),
                    closeness_centrality: HashMap::new(),
                    eigenvector_centrality: HashMap::new(),
                    page_rank: HashMap::new(),
                },
            },
        }
    }

    fn generate_normal_random(&self) -> f64 {
        // Box-Muller transform for normal random numbers
        use std::f64::consts::PI;

        let u1: f64 = rand::random();
        let u2: f64 = rand::random();

        (-2.0 * u1.ln()).sqrt() * (2.0 * PI * u2).cos()
    }
}

impl Default for EconomicModelsEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_economic_models_creation() {
        let engine = EconomicModelsEngine::new();

        // Test basic functionality
        let market_data = MarketDataPoint {
            timestamp: 1640995200, // 2022-01-01
            price: 0.15,
            volume: 1000.0,
            market_cap: 3_150_000.0,
            active_users: 1000,
            transaction_count: 500,
            staked_amount: 500_000.0,
        };

        let valuation = engine.calculate_token_valuation(&market_data).unwrap();
        assert!(valuation > 0.0);
    }

    #[test]
    fn test_monte_carlo_simulation() {
        let engine = EconomicModelsEngine::new();

        // Add some historical data
        for i in 0..10 {
            let data_point = MarketDataPoint {
                timestamp: 1640995200 + i * 86400,
                price: 0.15 + (i as f64 * 0.01),
                volume: 1000.0,
                market_cap: 3_150_000.0,
                active_users: 1000,
                transaction_count: 500,
                staked_amount: 500_000.0,
            };
            engine.add_market_data(data_point);
        }

        let simulation = engine.monte_carlo_simulation(100, 30).unwrap();
        assert_eq!(simulation.simulation_runs, 100);
        assert!(!simulation.outcome_percentiles.is_empty());
    }

    #[test]
    fn test_network_effects_analysis() {
        let engine = EconomicModelsEngine::new();

        let analysis = engine.analyze_network_effects(1000, 5.0);
        assert_eq!(analysis.active_users, 1000);
        assert_eq!(analysis.connections, 5000);
        assert!(analysis.network_density > 0.0);
        assert!(analysis.metcalfe_value > 0.0);
    }
}