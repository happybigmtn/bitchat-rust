//! Comprehensive Token Economics and Treasury Management Tests
//!
//! This test suite validates the complete token economics ecosystem including:
//! - Advanced staking mechanisms with variable rewards
//! - Treasury management with automated market making
//! - Cross-chain contract integration
//! - Economic models and risk assessment
//! - Integration between all components

use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

use bitcraps::{
    BlockchainNetwork, CompoundingFrequency, ContractManager, CrapTokens, EconomicsConfig, Error,
    Result, TokenEconomics, TokenLedger, TreasuryConfig, TreasuryManager,
};

#[tokio::test]
async fn test_complete_token_economics_flow() -> Result<()> {
    // Initialize the complete token economics system
    let ledger = Arc::new(TokenLedger::new());
    let economics = Arc::new(TokenEconomics::new(ledger.clone()));
    let treasury = Arc::new(TreasuryManager::new(ledger.clone()));
    let contracts = Arc::new(ContractManager::new(treasury.clone(), economics.clone()));

    // Initialize treasury with 1M CRAP
    let initial_balance = CrapTokens::from(1_000_000_000_000); // 1M CRAP
    treasury.initialize_treasury(initial_balance).await?;

    // Test 1: Advanced staking with different lock periods and compounding
    let staker1 = [1u8; 32];
    let staker2 = [2u8; 32];
    let staker3 = [3u8; 32];

    // Staker 1: Short-term stake with daily compounding
    economics
        .stake_tokens(
            staker1,
            CrapTokens::from(10_000_000_000),   // 10k CRAP
            Duration::from_secs(7 * 24 * 3600), // 7 days
            CompoundingFrequency::Daily,
        )
        .await?;

    // Staker 2: Medium-term stake with weekly compounding
    economics
        .stake_tokens(
            staker2,
            CrapTokens::from(50_000_000_000),    // 50k CRAP
            Duration::from_secs(90 * 24 * 3600), // 90 days
            CompoundingFrequency::Weekly,
        )
        .await?;

    // Staker 3: Long-term stake with monthly compounding
    economics
        .stake_tokens(
            staker3,
            CrapTokens::from(100_000_000_000),    // 100k CRAP
            Duration::from_secs(365 * 24 * 3600), // 1 year
            CompoundingFrequency::Monthly,
        )
        .await?;

    // Test 2: Calculate and verify staking rewards
    economics.calculate_staking_rewards().await?;

    let stats = economics.get_economics_stats().await;
    assert_eq!(stats.total_stakers, 3);
    assert_eq!(stats.total_staking_positions, 3);
    assert!(stats.average_staking_apy > 5.0); // Should be above base APY
    assert_eq!(stats.staked_supply, CrapTokens::from(160_000_000_000));

    println!("✓ Advanced staking system working correctly");

    // Test 3: Treasury operations and fee collection
    let fees_collected = CrapTokens::from(1_000_000_000); // 1k CRAP fees
    treasury.collect_and_distribute_fees(fees_collected).await?;

    let treasury_stats = treasury.get_treasury_stats().await;
    assert_eq!(treasury_stats.active_wallets, 4); // hot, cold, escrow, reserve
    assert!(treasury_stats.reserve_ratio >= 0.2); // At least 20% reserves

    println!("✓ Treasury fee collection and distribution working");

    // Test 4: Automated Market Maker operations
    let token_a_reserve = CrapTokens::from(100_000_000_000); // 100k CRAP
    let token_b_reserve = CrapTokens::from(1_000_000_000_000); // 1M token_b
    let swap_fee = 0.003; // 0.3%

    let amm_id = treasury
        .create_amm_pool(token_a_reserve, token_b_reserve, swap_fee)
        .await?;

    // Test token swapping
    let trader = [4u8; 32];
    let swap_input = CrapTokens::from(1_000_000_000); // 1k CRAP
    let min_output = CrapTokens::from(9_000_000_000); // Expect ~10k token_b

    let output = treasury
        .swap_tokens(amm_id, true, swap_input, min_output, trader)
        .await?;
    assert!(output.0 > min_output.0);

    println!("✓ Automated Market Maker functioning correctly");

    // Test 5: Dynamic fee structure updates
    economics.update_fee_structure(150).await?; // Low volume
    let low_fee = economics
        .calculate_dynamic_fee(bitcraps::economics::FeeCategory::Transfer, false)
        .await;

    economics.update_fee_structure(3000).await?; // High volume
    let high_fee = economics
        .calculate_dynamic_fee(bitcraps::economics::FeeCategory::Transfer, false)
        .await;

    assert!(high_fee.0 > low_fee.0);

    println!("✓ Dynamic fee structure responding to network conditions");

    // Test 6: Token burning mechanism
    let burn_amount = CrapTokens::from(500_000_000); // 500 CRAP
    economics
        .burn_tokens(burn_amount, "Quarterly burn".to_string())
        .await?;

    let updated_stats = economics.get_economics_stats().await;
    assert_eq!(updated_stats.burned_supply, burn_amount);

    println!("✓ Token burning mechanism working");

    // Test 7: Smart contract integration

    // Deploy token contract on Ethereum
    let eth_contract = contracts
        .deploy_token_contract(
            BlockchainNetwork::Ethereum,
            "BitCraps Token".to_string(),
            "CRAP".to_string(),
            21_000_000_000_000, // 21M CRAP
            12,
        )
        .await?;

    assert!(eth_contract.starts_with("0x"));

    // Deploy staking contract with reward rates
    let mut reward_rates = std::collections::HashMap::new();
    reward_rates.insert(7 * 24 * 3600, 5.0); // 5% APY for 7 days
    reward_rates.insert(30 * 24 * 3600, 8.0); // 8% APY for 30 days
    reward_rates.insert(365 * 24 * 3600, 12.0); // 12% APY for 1 year

    let staking_contract = contracts
        .deploy_staking_contract(
            BlockchainNetwork::Ethereum,
            eth_contract.clone(),
            eth_contract.clone(),
            reward_rates,
        )
        .await?;

    assert!(staking_contract.starts_with("0x"));

    println!("✓ Smart contract deployment simulation working");

    // Test 8: Cross-chain bridge setup
    let validator_addresses = vec![
        "0x1234567890123456789012345678901234567890".to_string(),
        "0xabcdefabcdefabcdefabcdefabcdefabcdefabcd".to_string(),
        "0x9876543210987654321098765432109876543210".to_string(),
    ];

    let bridge_contract = contracts
        .deploy_bridge_contract(
            BlockchainNetwork::Ethereum,
            vec![
                BlockchainNetwork::BinanceSmartChain,
                BlockchainNetwork::Polygon,
            ],
            vec![eth_contract.clone()],
            validator_addresses,
        )
        .await?;

    assert!(bridge_contract.starts_with("0x"));

    // Test bridge operation
    let bridge_operation = contracts
        .bridge_tokens(
            bridge_contract,
            BlockchainNetwork::Polygon,
            eth_contract,
            10_000_000_000, // 10k CRAP
            "0xrecipient123456789012345678901234567890".to_string(),
        )
        .await?;

    assert_ne!(bridge_operation, [0u8; 32]);

    println!("✓ Cross-chain bridge integration working");

    // Test 9: Oracle price feed setup
    contracts
        .setup_oracle_feed(
            BlockchainNetwork::Ethereum,
            "0xoracle1234567890123456789012345678901234567890".to_string(),
            "CRAP/USD".to_string(),
            Duration::from_secs(300), // 5 minute updates
        )
        .await?;

    let (price, timestamp) = contracts
        .get_oracle_price("0xoracle1234567890123456789012345678901234567890")
        .await?;

    assert!(price > 0.0);
    assert!(timestamp > 0);

    println!("✓ Oracle price feed integration working");

    // Test 10: Economic models and risk assessment
    use bitcraps::economics::models::{EconomicModelsEngine, MarketDataPoint};

    let models = EconomicModelsEngine::new();

    // Add some market data
    for i in 0..30 {
        let data = MarketDataPoint {
            timestamp: 1640995200 + i * 86400, // Daily data for 30 days
            price: 0.15 + (i as f64 * 0.001),  // Slight upward trend
            volume: 1000.0 + (i as f64 * 10.0),
            market_cap: 3_150_000.0 + (i as f64 * 21_000.0),
            active_users: 1000 + (i * 10),
            transaction_count: 500 + (i * 5),
            staked_amount: 500_000.0 + (i as f64 * 1000.0),
        };
        models.add_market_data(data);
    }

    // Calculate token valuation
    let latest_data = MarketDataPoint {
        timestamp: 1640995200 + 30 * 86400,
        price: 0.18,
        volume: 1300.0,
        market_cap: 3_780_000.0,
        active_users: 1300,
        transaction_count: 650,
        staked_amount: 530_000.0,
    };

    let valuation = models.calculate_token_valuation(&latest_data)?;
    assert!(valuation > 0.0);

    // Run Monte Carlo risk simulation
    let simulation = models.monte_carlo_simulation(1000, 30)?;
    assert_eq!(simulation.simulation_runs, 1000);
    assert!(!simulation.outcome_percentiles.is_empty());

    println!("✓ Economic models and risk assessment working");

    // Test 11: Network effects analysis
    let network_analysis = models.analyze_network_effects(1300, 7.5);
    assert_eq!(network_analysis.active_users, 1300);
    assert!(network_analysis.metcalfe_value > 0.0);

    println!("✓ Network effects analysis working");

    // Test 12: Game theory auction optimization
    let auction_result = models.optimize_auction_mechanism(25, "normal")?;
    assert!(auction_result.efficiency_score > 0.8);
    assert!(auction_result.expected_revenue > 0.0);

    println!("✓ Game theory models working");

    // Test 13: Integration validation - verify all systems work together

    // Collect final statistics from all systems
    let final_economics_stats = economics.get_economics_stats().await;
    let final_treasury_stats = treasury.get_treasury_stats().await;
    let final_contract_stats = contracts.get_contract_stats().await;

    // Verify system coherence
    assert!(final_economics_stats.total_supply.0 > 0);
    assert!(final_treasury_stats.total_balance.0 > 0);
    assert!(final_contract_stats.networks_supported > 0);

    // Verify staking system is functional
    assert_eq!(final_economics_stats.total_stakers, 3);
    assert!(final_economics_stats.average_staking_apy >= final_economics_stats.base_staking_apy);

    // Verify treasury has proper reserve ratios
    assert!(final_treasury_stats.reserve_ratio >= 0.2);
    assert!(final_treasury_stats.active_amm_pools > 0);

    // Verify smart contract integration
    assert!(final_contract_stats.total_token_contracts > 0);
    assert!(final_contract_stats.total_staking_contracts > 0);
    assert!(final_contract_stats.total_bridge_contracts > 0);

    println!("✓ Complete integration validation successful");

    // Performance validation
    let start_time = std::time::Instant::now();

    // Perform 100 rapid operations
    for i in 0..100 {
        let test_user = [(i % 256) as u8; 32];

        // Rapid fee calculations
        let _fee = economics
            .calculate_dynamic_fee(bitcraps::economics::FeeCategory::Transfer, i % 2 == 0)
            .await;

        // Risk assessments should be fast
        let operation = bitcraps::treasury::TreasuryOperation {
            operation_id: [i as u8; 32],
            operation_type: bitcraps::treasury::OperationType::Withdrawal,
            amount: CrapTokens::from(1_000_000),
            source_wallet: None,
            destination_wallet: None,
            initiator: test_user,
            approvers: vec![],
            timestamp: 1640995200,
            status: bitcraps::treasury::OperationStatus::Pending,
            reason: "Test operation".to_string(),
            risk_assessment: bitcraps::treasury::RiskAssessment {
                risk_score: 0.0,
                var_impact: 0.0,
                stress_test_results: Vec::new(),
                recommendation: bitcraps::treasury::RiskRecommendation::Approve,
            },
        };

        let _risk = treasury.assess_operation_risk(&operation).await;
    }

    let elapsed = start_time.elapsed();
    println!(
        "✓ Performance test: 100 operations completed in {:?}",
        elapsed
    );

    // Should complete in under 1 second for good performance
    assert!(elapsed < Duration::from_secs(1));

    // Final integration check - simulate a complete user journey
    println!("\n🎯 Running complete user journey simulation...");

    // User joins the platform
    let new_user = [99u8; 32];

    // User receives tokens (simulated)
    let user_balance = CrapTokens::from(25_000_000_000); // 25k CRAP

    // User stakes tokens for rewards
    economics
        .stake_tokens(
            new_user,
            CrapTokens::from(15_000_000_000),    // 15k CRAP
            Duration::from_secs(30 * 24 * 3600), // 30 days
            CompoundingFrequency::Weekly,
        )
        .await?;

    // User provides liquidity to AMM
    // (In a real implementation, this would involve token deposits)

    // User interacts with cross-chain bridge
    let bridge_amount = 5_000_000_000; // 5k CRAP
    let bridge_op = contracts
        .bridge_tokens(
            bridge_contract,
            BlockchainNetwork::BinanceSmartChain,
            eth_contract,
            bridge_amount,
            "0xuseraddress1234567890123456789012345678".to_string(),
        )
        .await?;

    assert_ne!(bridge_op, [0u8; 32]);

    // Verify system state after user interactions
    let final_stats = economics.get_economics_stats().await;
    assert_eq!(final_stats.total_stakers, 4); // Original 3 + new user

    println!("✓ Complete user journey simulation successful");

    println!("\n🏆 ALL COMPREHENSIVE TESTS PASSED!");
    println!("   - Advanced staking mechanisms ✓");
    println!("   - Treasury management with AMM ✓");
    println!("   - Smart contract integration ✓");
    println!("   - Economic models and risk assessment ✓");
    println!("   - Cross-chain bridge operations ✓");
    println!("   - Dynamic fee structures ✓");
    println!("   - Token burning mechanisms ✓");
    println!("   - Oracle price feeds ✓");
    println!("   - Performance validation ✓");
    println!("   - Complete user journey ✓");

    Ok(())
}

#[tokio::test]
async fn test_treasury_risk_management() -> Result<()> {
    let ledger = Arc::new(TokenLedger::new());
    let treasury = Arc::new(TreasuryManager::new(ledger));

    // Initialize with large treasury for risk testing
    let initial_balance = CrapTokens::from(10_000_000_000_000); // 10M CRAP
    treasury.initialize_treasury(initial_balance).await?;

    // Test risk assessment for different operation types
    let large_withdrawal = bitcraps::treasury::TreasuryOperation {
        operation_id: [1u8; 32],
        operation_type: bitcraps::treasury::OperationType::Withdrawal,
        amount: CrapTokens::from(1_000_000_000_000), // 1M CRAP (10% of treasury)
        source_wallet: None,
        destination_wallet: None,
        initiator: [1u8; 32],
        approvers: vec![],
        timestamp: 1640995200,
        status: bitcraps::treasury::OperationStatus::Pending,
        reason: "Large withdrawal test".to_string(),
        risk_assessment: bitcraps::treasury::RiskAssessment {
            risk_score: 0.0,
            var_impact: 0.0,
            stress_test_results: Vec::new(),
            recommendation: bitcraps::treasury::RiskRecommendation::Approve,
        },
    };

    let risk_assessment = treasury.assess_operation_risk(&large_withdrawal).await;

    // Large withdrawal should have high risk score
    assert!(risk_assessment.risk_score > 5.0);

    // Should recommend additional approvals or rejection
    match risk_assessment.recommendation {
        bitcraps::treasury::RiskRecommendation::RequireAdditionalApprovals => {
            println!("✓ Large withdrawal correctly flagged for additional approvals");
        }
        bitcraps::treasury::RiskRecommendation::Reject(_) => {
            println!("✓ Large withdrawal correctly rejected");
        }
        _ => panic!("Large withdrawal should require additional scrutiny"),
    }

    // Test emergency operation
    let emergency_freeze = bitcraps::treasury::TreasuryOperation {
        operation_id: [2u8; 32],
        operation_type: bitcraps::treasury::OperationType::EmergencyFreeze,
        amount: CrapTokens::from(0),
        source_wallet: None,
        destination_wallet: None,
        initiator: [1u8; 32],
        approvers: vec![],
        timestamp: 1640995200,
        status: bitcraps::treasury::OperationStatus::Pending,
        reason: "Emergency freeze test".to_string(),
        risk_assessment: bitcraps::treasury::RiskAssessment {
            risk_score: 0.0,
            var_impact: 0.0,
            stress_test_results: Vec::new(),
            recommendation: bitcraps::treasury::RiskRecommendation::Approve,
        },
    };

    let emergency_risk = treasury.assess_operation_risk(&emergency_freeze).await;
    assert!(emergency_risk.risk_score > 3.0); // Emergency operations have inherent risk

    println!("✓ Treasury risk management working correctly");

    Ok(())
}

#[tokio::test]
async fn test_economic_model_accuracy() -> Result<()> {
    use bitcraps::economics::models::{EconomicModelsEngine, MarketDataPoint};

    let models = EconomicModelsEngine::new();

    // Create realistic market data with known patterns
    let mut prices = Vec::new();
    let base_price = 0.10;

    // Generate 100 days of data with trend + noise
    for i in 0..100 {
        let trend = 0.001 * i as f64; // 0.1% daily growth
        let noise = (rand::random::<f64>() - 0.5) * 0.02; // ±1% noise
        let price = base_price + trend + noise;
        prices.push(price.max(0.01)); // Minimum price floor

        let data = MarketDataPoint {
            timestamp: 1640995200 + i * 86400,
            price: prices[i as usize],
            volume: 1000.0 + (rand::random::<f64>() * 500.0),
            market_cap: prices[i as usize] * 21_000_000.0,
            active_users: 1000 + (i * 2), // Steady user growth
            transaction_count: 500 + (rand::random::<u64>() % 100),
            staked_amount: 500_000.0 + (i as f64 * 1000.0),
        };

        models.add_market_data(data);
    }

    // Test Monte Carlo simulation with sufficient data
    let simulation = models.monte_carlo_simulation(10000, 30)?;

    // Validate simulation results
    assert_eq!(simulation.simulation_runs, 10000);
    assert!(simulation.outcome_percentiles.contains_key(&50)); // Median should exist
    assert!(simulation.outcome_percentiles.contains_key(&95)); // 95th percentile

    // 95th percentile should be higher than 50th percentile (basic sanity check)
    assert!(simulation.outcome_percentiles[&95] > simulation.outcome_percentiles[&50]);

    // Test network effects with realistic numbers
    let network_analysis = models.analyze_network_effects(50000, 12.0); // 50k users, 12 connections each
    assert_eq!(network_analysis.active_users, 50000);
    assert_eq!(network_analysis.connections, 600000);
    assert!(network_analysis.metcalfe_value > 0.0);
    assert!(network_analysis.network_density < 1.0); // Should be less than fully connected

    println!("✓ Economic model accuracy validation passed");

    Ok(())
}

#[tokio::test]
async fn test_integration_stress_scenarios() -> Result<()> {
    let ledger = Arc::new(TokenLedger::new());
    let economics = Arc::new(TokenEconomics::new(ledger.clone()));
    let treasury = Arc::new(TreasuryManager::new(ledger.clone()));

    // Initialize with substantial funds
    treasury
        .initialize_treasury(CrapTokens::from(5_000_000_000_000))
        .await?; // 5M CRAP

    // Stress test 1: Rapid staking/unstaking
    let mut stake_handles = Vec::new();

    for i in 0..50 {
        let economics_clone = economics.clone();
        let staker = [i as u8; 32];

        let handle = tokio::spawn(async move {
            economics_clone
                .stake_tokens(
                    staker,
                    CrapTokens::from(1_000_000_000), // 1k CRAP each
                    Duration::from_secs(7 * 24 * 3600), // 7 days
                    CompoundingFrequency::Daily,
                )
                .await
        });

        stake_handles.push(handle);
    }

    // Wait for all staking operations to complete
    for handle in stake_handles {
        handle.await.unwrap()?;
    }

    let stats = economics.get_economics_stats().await;
    assert_eq!(stats.total_stakers, 50);
    assert_eq!(stats.total_staking_positions, 50);
    assert_eq!(stats.staked_supply, CrapTokens::from(50_000_000_000));

    println!("✓ Stress test 1 (rapid staking) passed");

    // Stress test 2: Rapid AMM operations
    let token_a = CrapTokens::from(1_000_000_000_000); // 1M CRAP
    let token_b = CrapTokens::from(5_000_000_000_000); // 5M token_b

    let amm_id = treasury.create_amm_pool(token_a, token_b, 0.003).await?;

    let mut swap_handles = Vec::new();

    for i in 0..20 {
        let treasury_clone = treasury.clone();
        let trader = [i as u8; 32];

        let handle = tokio::spawn(async move {
            let swap_amount = CrapTokens::from(1_000_000_000); // 1k CRAP
            let min_output = CrapTokens::from(4_500_000_000); // Expect ~5k token_b

            treasury_clone
                .swap_tokens(amm_id, true, swap_amount, min_output, trader)
                .await
        });

        swap_handles.push(handle);
    }

    let mut successful_swaps = 0;
    for handle in swap_handles {
        match handle.await.unwrap() {
            Ok(_) => successful_swaps += 1,
            Err(_) => {} // Some swaps may fail due to slippage/reserves
        }
    }

    assert!(successful_swaps > 10); // At least half should succeed

    println!(
        "✓ Stress test 2 (rapid AMM swaps) passed with {}/20 successful swaps",
        successful_swaps
    );

    // Stress test 3: Fee collection under high volume
    for _i in 0..100 {
        let fees = CrapTokens::from(10_000_000); // 10 CRAP per collection
        treasury.collect_and_distribute_fees(fees).await?;
    }

    let final_stats = treasury.get_treasury_stats().await;
    assert!(final_stats.reserve_balance.0 > 0);

    println!("✓ Stress test 3 (high volume fee collection) passed");

    // Stress test 4: Concurrent economic calculations
    let mut calculation_handles = Vec::new();

    for _i in 0..10 {
        let economics_clone = economics.clone();

        let handle = tokio::spawn(async move { economics_clone.calculate_staking_rewards().await });

        calculation_handles.push(handle);
    }

    for handle in calculation_handles {
        handle.await.unwrap()?;
    }

    println!("✓ Stress test 4 (concurrent calculations) passed");

    println!("🎯 All stress tests passed - system is robust under load");

    Ok(())
}
#![cfg(feature = "legacy-tests")]
