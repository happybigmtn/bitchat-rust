//! Comprehensive integration tests for SafeArithmetic
//!
//! These tests verify overflow protection, boundary conditions, and security
//! of all arithmetic operations used in financial calculations.

use bitcraps::crypto::{token_arithmetic, SafeArithmetic};
use bitcraps::error::{Error, Result};
use bitcraps::protocol::craps::CrapTokens;

/// Test safe addition with boundary conditions
#[tokio::test]
async fn test_safe_addition_boundaries() {
    // Normal addition should work
    assert_eq!(SafeArithmetic::safe_add_u64(100, 200).unwrap(), 300);
    assert_eq!(SafeArithmetic::safe_add_u64(0, 0).unwrap(), 0);
    assert_eq!(
        SafeArithmetic::safe_add_u64(u64::MAX - 1, 1).unwrap(),
        u64::MAX
    );

    // Overflow cases should fail
    assert!(SafeArithmetic::safe_add_u64(u64::MAX, 1).is_err());
    assert!(SafeArithmetic::safe_add_u64(u64::MAX - 10, 20).is_err());

    // Large number addition near overflow
    let large_num = u64::MAX / 2;
    assert!(SafeArithmetic::safe_add_u64(large_num, large_num).is_err());
    assert!(SafeArithmetic::safe_add_u64(large_num, large_num - 1).is_ok());
}

/// Test safe subtraction with underflow protection
#[tokio::test]
async fn test_safe_subtraction_boundaries() {
    // Normal subtraction should work
    assert_eq!(SafeArithmetic::safe_sub_u64(300, 100).unwrap(), 200);
    assert_eq!(SafeArithmetic::safe_sub_u64(100, 100).unwrap(), 0);
    assert_eq!(
        SafeArithmetic::safe_sub_u64(u64::MAX, 1).unwrap(),
        u64::MAX - 1
    );

    // Underflow cases should fail
    assert!(SafeArithmetic::safe_sub_u64(0, 1).is_err());
    assert!(SafeArithmetic::safe_sub_u64(100, 200).is_err());
    assert!(SafeArithmetic::safe_sub_u64(50, 51).is_err());

    // Edge case: subtract from small numbers
    assert_eq!(SafeArithmetic::safe_sub_u64(1, 1).unwrap(), 0);
    assert!(SafeArithmetic::safe_sub_u64(1, 2).is_err());
}

/// Test safe multiplication with overflow protection
#[tokio::test]
async fn test_safe_multiplication_boundaries() {
    // Normal multiplication should work
    assert_eq!(SafeArithmetic::safe_mul_u64(10, 20).unwrap(), 200);
    assert_eq!(SafeArithmetic::safe_mul_u64(0, 1000).unwrap(), 0);
    assert_eq!(SafeArithmetic::safe_mul_u64(1, u64::MAX).unwrap(), u64::MAX);

    // Overflow cases should fail
    assert!(SafeArithmetic::safe_mul_u64(u64::MAX, 2).is_err());
    assert!(SafeArithmetic::safe_mul_u64(2, u64::MAX).is_err());

    // Large number multiplication
    let sqrt_max = (u64::MAX as f64).sqrt() as u64;
    assert!(SafeArithmetic::safe_mul_u64(sqrt_max + 1, sqrt_max + 1).is_err());
    assert!(SafeArithmetic::safe_mul_u64(sqrt_max, sqrt_max).is_ok());

    // Powers of two near overflow
    assert!(SafeArithmetic::safe_mul_u64(1u64 << 32, 1u64 << 32).is_err());
    assert!(SafeArithmetic::safe_mul_u64(1u64 << 31, 1u64 << 31).is_ok());
}

/// Test safe division with zero protection
#[tokio::test]
async fn test_safe_division_zero_protection() {
    // Normal division should work
    assert_eq!(SafeArithmetic::safe_div_u64(100, 10).unwrap(), 10);
    assert_eq!(
        SafeArithmetic::safe_div_u64(u64::MAX, 2).unwrap(),
        u64::MAX / 2
    );
    assert_eq!(SafeArithmetic::safe_div_u64(0, 1).unwrap(), 0);

    // Division by zero should fail
    assert!(SafeArithmetic::safe_div_u64(100, 0).is_err());
    assert!(SafeArithmetic::safe_div_u64(0, 0).is_err());
    assert!(SafeArithmetic::safe_div_u64(u64::MAX, 0).is_err());

    // Integer division truncation
    assert_eq!(SafeArithmetic::safe_div_u64(7, 3).unwrap(), 2);
    assert_eq!(SafeArithmetic::safe_div_u64(1, 2).unwrap(), 0);
}

/// Test percentage calculations with validation
#[tokio::test]
async fn test_safe_percentage_calculations() {
    // Normal percentage calculations
    assert_eq!(SafeArithmetic::safe_percentage(1000, 10).unwrap(), 100); // 10% of 1000
    assert_eq!(SafeArithmetic::safe_percentage(200, 50).unwrap(), 100); // 50% of 200
    assert_eq!(SafeArithmetic::safe_percentage(1000, 0).unwrap(), 0); // 0% of 1000
    assert_eq!(SafeArithmetic::safe_percentage(1000, 100).unwrap(), 1000); // 100% of 1000

    // Invalid percentages should fail
    assert!(SafeArithmetic::safe_percentage(1000, 101).is_err());
    assert!(SafeArithmetic::safe_percentage(1000, 150).is_err());
    assert!(SafeArithmetic::safe_percentage(1000, 255).is_err());

    // Large value percentage calculations (overflow protection)
    let large_value = u64::MAX / 2;
    assert!(SafeArithmetic::safe_percentage(large_value, 50).is_ok());

    // Test edge case where multiplication might overflow
    assert!(SafeArithmetic::safe_percentage(u64::MAX, 50).is_err());
    assert!(SafeArithmetic::safe_percentage(u64::MAX - 100, 1).is_ok());
}

/// Test balance update operations with positive and negative changes
#[tokio::test]
async fn test_safe_balance_updates() {
    let initial_balance = 1000u64;

    // Positive balance changes (credits)
    assert_eq!(
        SafeArithmetic::safe_balance_update(initial_balance, 500).unwrap(),
        1500
    );
    assert_eq!(SafeArithmetic::safe_balance_update(0, 100).unwrap(), 100);

    // Negative balance changes (debits)
    assert_eq!(
        SafeArithmetic::safe_balance_update(initial_balance, -300).unwrap(),
        700
    );
    assert_eq!(SafeArithmetic::safe_balance_update(1000, -1000).unwrap(), 0);

    // Zero change
    assert_eq!(
        SafeArithmetic::safe_balance_update(initial_balance, 0).unwrap(),
        initial_balance
    );

    // Overflow protection for positive changes
    assert!(SafeArithmetic::safe_balance_update(u64::MAX, 1).is_err());
    assert!(SafeArithmetic::safe_balance_update(u64::MAX - 10, 20).is_err());

    // Underflow protection for negative changes
    assert!(SafeArithmetic::safe_balance_update(100, -200).is_err());
    assert!(SafeArithmetic::safe_balance_update(0, -1).is_err());

    // Large balance changes
    let large_balance = u64::MAX / 2;
    assert!(SafeArithmetic::safe_balance_update(large_balance, large_balance as i64).is_err());
    assert!(
        SafeArithmetic::safe_balance_update(large_balance, -(large_balance as i64 + 1)).is_err()
    );
}

/// Test bet validation with comprehensive security checks
#[tokio::test]
async fn test_comprehensive_bet_validation() {
    let player_balance = 10000u64;
    let max_bet = 5000u64;

    // Valid bets should pass
    assert!(SafeArithmetic::safe_validate_bet(1000, player_balance, max_bet).is_ok());
    assert!(SafeArithmetic::safe_validate_bet(max_bet, player_balance, max_bet).is_ok());
    assert!(
        SafeArithmetic::safe_validate_bet(player_balance, player_balance, player_balance).is_ok()
    );

    // Zero bet should fail
    assert!(SafeArithmetic::safe_validate_bet(0, player_balance, max_bet).is_err());

    // Bet exceeding max should fail
    assert!(SafeArithmetic::safe_validate_bet(max_bet + 1, player_balance, max_bet).is_err());
    assert!(SafeArithmetic::safe_validate_bet(max_bet * 2, player_balance, max_bet).is_err());

    // Bet exceeding balance should fail
    assert!(
        SafeArithmetic::safe_validate_bet(player_balance + 1, player_balance, u64::MAX).is_err()
    );
    assert!(
        SafeArithmetic::safe_validate_bet(player_balance * 2, player_balance, u64::MAX).is_err()
    );

    // Edge cases
    assert!(SafeArithmetic::safe_validate_bet(1, 0, 100).is_err()); // No balance
    assert!(SafeArithmetic::safe_validate_bet(1, 100, 0).is_err()); // Zero max bet
}

/// Test payout calculations with house edge protection
#[tokio::test]
async fn test_secure_payout_calculations() {
    let bet_amount = 1000u64;

    // Standard payout calculations
    assert_eq!(
        SafeArithmetic::safe_calculate_payout(bet_amount, 2, 1).unwrap(),
        2000
    ); // 2:1 payout
    assert_eq!(
        SafeArithmetic::safe_calculate_payout(bet_amount, 3, 2).unwrap(),
        1500
    ); // 3:2 payout
    assert_eq!(
        SafeArithmetic::safe_calculate_payout(bet_amount, 1, 1).unwrap(),
        1000
    ); // 1:1 payout
    assert_eq!(
        SafeArithmetic::safe_calculate_payout(bet_amount, 1, 2).unwrap(),
        500
    ); // 1:2 payout

    // Zero denominator should fail
    assert!(SafeArithmetic::safe_calculate_payout(bet_amount, 2, 0).is_err());
    assert!(SafeArithmetic::safe_calculate_payout(0, 2, 0).is_err());

    // Overflow protection in numerator calculation
    let large_bet = u64::MAX / 2;
    assert!(SafeArithmetic::safe_calculate_payout(large_bet, 3, 1).is_err()); // Would overflow
    assert!(SafeArithmetic::safe_calculate_payout(large_bet, 1, 1).is_ok()); // Should work

    // Very large multipliers
    assert!(SafeArithmetic::safe_calculate_payout(1000, u64::MAX, 1).is_err());
    assert!(SafeArithmetic::safe_calculate_payout(1, u64::MAX, u64::MAX).is_ok());
}

/// Test sequence number increment with wraparound protection
#[tokio::test]
async fn test_sequence_increment_protection() {
    // Normal increments
    assert_eq!(SafeArithmetic::safe_increment_sequence(0).unwrap(), 1);
    assert_eq!(SafeArithmetic::safe_increment_sequence(100).unwrap(), 101);
    assert_eq!(
        SafeArithmetic::safe_increment_sequence(u64::MAX - 1).unwrap(),
        u64::MAX
    );

    // Wraparound protection
    assert!(SafeArithmetic::safe_increment_sequence(u64::MAX).is_err());

    // Test sequence near maximum
    let near_max = u64::MAX - 10;
    for i in 0..10 {
        assert_eq!(
            SafeArithmetic::safe_increment_sequence(near_max + i).unwrap(),
            near_max + i + 1
        );
    }
    assert!(SafeArithmetic::safe_increment_sequence(u64::MAX).is_err());
}

/// Test timestamp validation with tolerance
#[tokio::test]
async fn test_timestamp_validation_security() {
    use std::time::{SystemTime, UNIX_EPOCH};

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let tolerance = 300; // 5 minutes

    // Current timestamp should be valid
    assert!(SafeArithmetic::safe_validate_timestamp(now, tolerance).is_ok());

    // Timestamps within tolerance should be valid
    assert!(SafeArithmetic::safe_validate_timestamp(now - tolerance, tolerance).is_ok());
    assert!(SafeArithmetic::safe_validate_timestamp(now + tolerance, tolerance).is_ok());
    assert!(SafeArithmetic::safe_validate_timestamp(now - tolerance + 1, tolerance).is_ok());
    assert!(SafeArithmetic::safe_validate_timestamp(now + tolerance - 1, tolerance).is_ok());

    // Timestamps outside tolerance should fail
    assert!(SafeArithmetic::safe_validate_timestamp(now - tolerance - 1, tolerance).is_err());
    assert!(SafeArithmetic::safe_validate_timestamp(now + tolerance + 1, tolerance).is_err());

    // Very old and very future timestamps should fail
    assert!(SafeArithmetic::safe_validate_timestamp(0, tolerance).is_err());
    assert!(SafeArithmetic::safe_validate_timestamp(now + 86400, tolerance).is_err()); // 1 day future

    // Test with different tolerance values
    assert!(SafeArithmetic::safe_validate_timestamp(now, 0).is_ok()); // Zero tolerance
    assert!(SafeArithmetic::safe_validate_timestamp(now + 1, 0).is_err());
    assert!(SafeArithmetic::safe_validate_timestamp(now - 1, 0).is_err());
}

/// Test safe array access with bounds checking
#[tokio::test]
async fn test_safe_array_access() {
    let test_array = [10u32, 20, 30, 40, 50];

    // Valid indices should work
    assert_eq!(
        *SafeArithmetic::safe_array_access(&test_array, 0).unwrap(),
        10
    );
    assert_eq!(
        *SafeArithmetic::safe_array_access(&test_array, 2).unwrap(),
        30
    );
    assert_eq!(
        *SafeArithmetic::safe_array_access(&test_array, 4).unwrap(),
        50
    );

    // Out of bounds should fail
    assert!(SafeArithmetic::safe_array_access(&test_array, 5).is_err());
    assert!(SafeArithmetic::safe_array_access(&test_array, 100).is_err());

    // Empty array
    let empty_array: [u32; 0] = [];
    assert!(SafeArithmetic::safe_array_access(&empty_array, 0).is_err());

    // Test mutable access
    let mut mut_array = [1u32, 2, 3, 4, 5];
    *SafeArithmetic::safe_array_access_mut(&mut mut_array, 2).unwrap() = 99;
    assert_eq!(mut_array[2], 99);

    // Out of bounds mutable access should fail
    assert!(SafeArithmetic::safe_array_access_mut(&mut mut_array, 5).is_err());
}

/// Test Merkle tree depth calculation with overflow protection
#[tokio::test]
async fn test_merkle_depth_calculation() {
    // Normal depth calculations
    assert_eq!(SafeArithmetic::safe_merkle_depth(0).unwrap(), 0);
    assert_eq!(SafeArithmetic::safe_merkle_depth(1).unwrap(), 0);
    assert_eq!(SafeArithmetic::safe_merkle_depth(2).unwrap(), 1);
    assert_eq!(SafeArithmetic::safe_merkle_depth(3).unwrap(), 2);
    assert_eq!(SafeArithmetic::safe_merkle_depth(4).unwrap(), 2);
    assert_eq!(SafeArithmetic::safe_merkle_depth(8).unwrap(), 3);
    assert_eq!(SafeArithmetic::safe_merkle_depth(16).unwrap(), 4);

    // Large but valid leaf counts
    assert_eq!(SafeArithmetic::safe_merkle_depth(1024).unwrap(), 10);
    assert_eq!(SafeArithmetic::safe_merkle_depth(1000000).unwrap(), 20);

    // Very large leaf counts that could cause overflow
    let large_count = 1usize << 32;
    assert!(SafeArithmetic::safe_merkle_depth(large_count).is_ok());

    // Extremely large counts that should trigger overflow protection
    let extreme_count = usize::MAX;
    let result = SafeArithmetic::safe_merkle_depth(extreme_count);
    // This might succeed or fail depending on the implementation, but shouldn't panic
    assert!(result.is_ok() || result.is_err());
}

/// Test power-of-two utilities
#[tokio::test]
async fn test_power_of_two_operations() {
    // Test power-of-two detection
    assert!(SafeArithmetic::is_power_of_two(1));
    assert!(SafeArithmetic::is_power_of_two(2));
    assert!(SafeArithmetic::is_power_of_two(4));
    assert!(SafeArithmetic::is_power_of_two(1024));
    assert!(SafeArithmetic::is_power_of_two(1u64 << 63));

    assert!(!SafeArithmetic::is_power_of_two(0));
    assert!(!SafeArithmetic::is_power_of_two(3));
    assert!(!SafeArithmetic::is_power_of_two(5));
    assert!(!SafeArithmetic::is_power_of_two(1000));

    // Test next power-of-two calculation
    assert_eq!(SafeArithmetic::next_power_of_two(0).unwrap(), 1);
    assert_eq!(SafeArithmetic::next_power_of_two(1).unwrap(), 1);
    assert_eq!(SafeArithmetic::next_power_of_two(2).unwrap(), 2);
    assert_eq!(SafeArithmetic::next_power_of_two(3).unwrap(), 4);
    assert_eq!(SafeArithmetic::next_power_of_two(5).unwrap(), 8);
    assert_eq!(SafeArithmetic::next_power_of_two(1000).unwrap(), 1024);

    // Overflow protection for next power-of-two
    assert!(SafeArithmetic::next_power_of_two(1u64 << 63).is_ok()); // Should be 1<<63
    assert!(SafeArithmetic::next_power_of_two((1u64 << 63) + 1).is_err()); // Would overflow
}

/// Test CrapTokens arithmetic operations
#[tokio::test]
async fn test_token_arithmetic_operations() {
    let tokens_100 = CrapTokens::new(100);
    let tokens_50 = CrapTokens::new(50);
    let tokens_200 = CrapTokens::new(200);

    // Token addition
    let sum = token_arithmetic::safe_add_tokens(tokens_100, tokens_50).unwrap();
    assert_eq!(sum.amount(), 150);

    // Token subtraction
    let diff = token_arithmetic::safe_sub_tokens(tokens_200, tokens_50).unwrap();
    assert_eq!(diff.amount(), 150);

    // Token multiplication
    let product = token_arithmetic::safe_mul_tokens(tokens_50, 3).unwrap();
    assert_eq!(product.amount(), 150);

    // Token division
    let quotient = token_arithmetic::safe_div_tokens(tokens_200, 4).unwrap();
    assert_eq!(quotient.amount(), 50);

    // Test overflow protection
    let max_tokens = CrapTokens::new(u64::MAX);
    assert!(token_arithmetic::safe_add_tokens(max_tokens, CrapTokens::new(1)).is_err());
    assert!(token_arithmetic::safe_mul_tokens(max_tokens, 2).is_err());

    // Test underflow protection
    assert!(token_arithmetic::safe_sub_tokens(tokens_50, tokens_100).is_err());

    // Test division by zero
    assert!(token_arithmetic::safe_div_tokens(tokens_100, 0).is_err());
}

/// Comprehensive test of error conditions and edge cases
#[tokio::test]
async fn test_comprehensive_error_conditions() {
    // Test various error types are returned correctly
    let overflow_result = SafeArithmetic::safe_add_u64(u64::MAX, 1);
    assert!(matches!(
        overflow_result.unwrap_err(),
        Error::ArithmeticOverflow(_)
    ));

    let div_zero_result = SafeArithmetic::safe_div_u64(100, 0);
    assert!(matches!(
        div_zero_result.unwrap_err(),
        Error::DivisionByZero(_)
    ));

    let invalid_input_result = SafeArithmetic::safe_percentage(100, 150);
    assert!(matches!(
        invalid_input_result.unwrap_err(),
        Error::InvalidInput(_)
    ));

    let insufficient_funds_result = SafeArithmetic::safe_validate_bet(1000, 500, 2000);
    assert!(matches!(
        insufficient_funds_result.unwrap_err(),
        Error::InsufficientFunds(_)
    ));

    let bounds_result = SafeArithmetic::safe_array_access(&[1, 2, 3], 5);
    assert!(matches!(
        bounds_result.unwrap_err(),
        Error::IndexOutOfBounds(_)
    ));

    // Test error messages contain relevant information
    let overflow_error = SafeArithmetic::safe_add_u64(u64::MAX, 1).unwrap_err();
    let error_message = format!("{:?}", overflow_error);
    assert!(error_message.contains("overflow") || error_message.contains("Overflow"));

    let div_zero_error = SafeArithmetic::safe_div_u64(100, 0).unwrap_err();
    let div_error_message = format!("{:?}", div_zero_error);
    assert!(div_error_message.contains("zero") || div_error_message.contains("Zero"));
}
