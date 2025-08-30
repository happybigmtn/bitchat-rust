//! Safe arithmetic operations for preventing integer overflow attacks
//!
//! This module provides overflow-safe arithmetic operations for critical
//! financial calculations in the BitCraps casino system.

use crate::error::{Error, Result};

/// Safe arithmetic operations that prevent overflow attacks
pub struct SafeArithmetic;

impl SafeArithmetic {
    /// Safe addition with overflow checking
    pub fn safe_add_u64(a: u64, b: u64) -> Result<u64> {
        a.checked_add(b)
            .ok_or_else(|| Error::ArithmeticOverflow(format!("Addition overflow: {} + {}", a, b)))
    }

    /// Safe subtraction with underflow checking
    pub fn safe_sub_u64(a: u64, b: u64) -> Result<u64> {
        a.checked_sub(b).ok_or_else(|| {
            Error::ArithmeticOverflow(format!("Subtraction underflow: {} - {}", a, b))
        })
    }

    /// Safe multiplication with overflow checking
    pub fn safe_mul_u64(a: u64, b: u64) -> Result<u64> {
        a.checked_mul(b).ok_or_else(|| {
            Error::ArithmeticOverflow(format!("Multiplication overflow: {} * {}", a, b))
        })
    }

    /// Safe division with zero-checking
    pub fn safe_div_u64(a: u64, b: u64) -> Result<u64> {
        if b == 0 {
            return Err(Error::DivisionByZero("Division by zero".to_string()));
        }
        Ok(a / b)
    }

    /// Safe percentage calculation with overflow protection
    pub fn safe_percentage(value: u64, percentage: u8) -> Result<u64> {
        if percentage > 100 {
            return Err(Error::InvalidInput(format!(
                "Invalid percentage: {}%",
                percentage
            )));
        }

        let percentage_u64 = percentage as u64;
        let numerator = Self::safe_mul_u64(value, percentage_u64)?;
        Ok(numerator / 100)
    }

    /// Safe balance update with overflow and underflow protection
    pub fn safe_balance_update(current_balance: u64, change: i64) -> Result<u64> {
        if change >= 0 {
            let positive_change = change as u64;
            Self::safe_add_u64(current_balance, positive_change)
        } else {
            let negative_change = (-change) as u64;
            Self::safe_sub_u64(current_balance, negative_change)
        }
    }

    /// Safe bet validation with maximum limits
    pub fn safe_validate_bet(bet_amount: u64, player_balance: u64, max_bet: u64) -> Result<()> {
        if bet_amount == 0 {
            return Err(Error::InvalidInput("Bet amount cannot be zero".to_string()));
        }

        if bet_amount > max_bet {
            return Err(Error::InvalidInput(format!(
                "Bet amount {} exceeds maximum {}",
                bet_amount, max_bet
            )));
        }

        if bet_amount > player_balance {
            return Err(Error::InsufficientFunds(format!(
                "Bet amount {} exceeds balance {}",
                bet_amount, player_balance
            )));
        }

        Ok(())
    }

    /// Safe payout calculation with house edge protection
    pub fn safe_calculate_payout(
        bet_amount: u64,
        multiplier_numerator: u64,
        multiplier_denominator: u64,
    ) -> Result<u64> {
        if multiplier_denominator == 0 {
            return Err(Error::DivisionByZero(
                "Multiplier denominator cannot be zero".to_string(),
            ));
        }

        let numerator = Self::safe_mul_u64(bet_amount, multiplier_numerator)?;
        Ok(numerator / multiplier_denominator)
    }

    /// Safe sequence number increment with wraparound protection
    pub fn safe_increment_sequence(current: u64) -> Result<u64> {
        if current == u64::MAX {
            return Err(Error::ArithmeticOverflow(
                "Sequence number wraparound detected".to_string(),
            ));
        }
        Ok(current + 1)
    }

    /// Safe timestamp validation
    pub fn safe_validate_timestamp(timestamp: u64, tolerance_seconds: u64) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let min_time = now.saturating_sub(tolerance_seconds);
        let max_time = Self::safe_add_u64(now, tolerance_seconds)?;

        if timestamp < min_time || timestamp > max_time {
            return Err(Error::InvalidTimestamp(format!(
                "Timestamp {} outside valid range [{}, {}]",
                timestamp, min_time, max_time
            )));
        }

        Ok(())
    }

    /// Safe array indexing to prevent buffer overruns
    pub fn safe_array_access<T>(array: &[T], index: usize) -> Result<&T> {
        array.get(index).ok_or_else(|| {
            Error::IndexOutOfBounds(format!(
                "Index {} out of bounds for array of length {}",
                index,
                array.len()
            ))
        })
    }

    /// Safe array mutable access
    pub fn safe_array_access_mut<T>(array: &mut [T], index: usize) -> Result<&mut T> {
        let len = array.len();
        array.get_mut(index).ok_or_else(|| {
            Error::IndexOutOfBounds(format!(
                "Index {} out of bounds for array of length {}",
                index, len
            ))
        })
    }

    /// Safe merkle tree depth calculation with overflow protection
    pub fn safe_merkle_depth(leaf_count: usize) -> Result<usize> {
        if leaf_count == 0 {
            return Ok(0);
        }

        let mut depth = 0;
        let mut count = leaf_count;

        while count > 1 {
            // Check for overflow in depth calculation
            if depth >= 64 {
                return Err(Error::ArithmeticOverflow(
                    "Merkle tree depth overflow".to_string(),
                ));
            }

            count = (count + 1) / 2; // Round up division
            depth += 1;
        }

        Ok(depth)
    }

    /// Safe power-of-two validation
    pub fn is_power_of_two(n: u64) -> bool {
        n > 0 && (n & (n - 1)) == 0
    }

    /// Safe next power of two calculation
    pub fn next_power_of_two(n: u64) -> Result<u64> {
        if n == 0 {
            return Ok(1);
        }

        if n > (1u64 << 63) {
            return Err(Error::ArithmeticOverflow(
                "Next power of two would overflow".to_string(),
            ));
        }

        let mut power = 1;
        while power < n {
            power = Self::safe_mul_u64(power, 2)?;
        }

        Ok(power)
    }
}

/// Safe operations on CrapTokens with overflow protection
pub mod token_arithmetic {
    use super::*;
    use crate::protocol::craps::CrapTokens;

    /// Safe token addition
    pub fn safe_add_tokens(a: CrapTokens, b: CrapTokens) -> Result<CrapTokens> {
        let sum = SafeArithmetic::safe_add_u64(a.0, b.0)?;
        Ok(CrapTokens::new_unchecked(sum))
    }

    /// Safe token subtraction  
    pub fn safe_sub_tokens(a: CrapTokens, b: CrapTokens) -> Result<CrapTokens> {
        let difference = SafeArithmetic::safe_sub_u64(a.0, b.0)?;
        Ok(CrapTokens::new_unchecked(difference))
    }

    /// Safe token multiplication for payouts
    pub fn safe_mul_tokens(tokens: CrapTokens, multiplier: u64) -> Result<CrapTokens> {
        let result = SafeArithmetic::safe_mul_u64(tokens.0, multiplier)?;
        Ok(CrapTokens::new_unchecked(result))
    }

    /// Safe token division for splits
    pub fn safe_div_tokens(tokens: CrapTokens, divisor: u64) -> Result<CrapTokens> {
        let result = SafeArithmetic::safe_div_u64(tokens.0, divisor)?;
        Ok(CrapTokens::new_unchecked(result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_addition() {
        assert_eq!(SafeArithmetic::safe_add_u64(5, 3).unwrap(), 8);
        assert!(SafeArithmetic::safe_add_u64(u64::MAX, 1).is_err());
    }

    #[test]
    fn test_safe_subtraction() {
        assert_eq!(SafeArithmetic::safe_sub_u64(10, 3).unwrap(), 7);
        assert!(SafeArithmetic::safe_sub_u64(3, 10).is_err());
    }

    #[test]
    fn test_safe_multiplication() {
        assert_eq!(SafeArithmetic::safe_mul_u64(4, 5).unwrap(), 20);
        assert!(SafeArithmetic::safe_mul_u64(u64::MAX, 2).is_err());
    }

    #[test]
    fn test_safe_division() {
        assert_eq!(SafeArithmetic::safe_div_u64(20, 4).unwrap(), 5);
        assert!(SafeArithmetic::safe_div_u64(20, 0).is_err());
    }

    #[test]
    fn test_safe_percentage() {
        assert_eq!(SafeArithmetic::safe_percentage(200, 15).unwrap(), 30);
        assert!(SafeArithmetic::safe_percentage(100, 150).is_err());
    }

    #[test]
    fn test_safe_balance_update() {
        assert_eq!(SafeArithmetic::safe_balance_update(100, 50).unwrap(), 150);
        assert_eq!(SafeArithmetic::safe_balance_update(100, -30).unwrap(), 70);
        assert!(SafeArithmetic::safe_balance_update(50, -100).is_err());
    }

    #[test]
    fn test_bet_validation() {
        assert!(SafeArithmetic::safe_validate_bet(50, 100, 200).is_ok());
        assert!(SafeArithmetic::safe_validate_bet(0, 100, 200).is_err());
        assert!(SafeArithmetic::safe_validate_bet(250, 100, 200).is_err());
        assert!(SafeArithmetic::safe_validate_bet(150, 100, 200).is_err());
    }

    #[test]
    fn test_safe_payout() {
        assert_eq!(
            SafeArithmetic::safe_calculate_payout(100, 3, 2).unwrap(),
            150
        );
        assert!(SafeArithmetic::safe_calculate_payout(100, 3, 0).is_err());
    }

    #[test]
    fn test_merkle_depth() {
        assert_eq!(SafeArithmetic::safe_merkle_depth(0).unwrap(), 0);
        assert_eq!(SafeArithmetic::safe_merkle_depth(1).unwrap(), 0);
        assert_eq!(SafeArithmetic::safe_merkle_depth(2).unwrap(), 1);
        assert_eq!(SafeArithmetic::safe_merkle_depth(4).unwrap(), 2);
        assert_eq!(SafeArithmetic::safe_merkle_depth(8).unwrap(), 3);
    }

    #[test]
    fn test_power_of_two() {
        assert!(SafeArithmetic::is_power_of_two(1));
        assert!(SafeArithmetic::is_power_of_two(2));
        assert!(SafeArithmetic::is_power_of_two(4));
        assert!(SafeArithmetic::is_power_of_two(8));
        assert!(!SafeArithmetic::is_power_of_two(3));
        assert!(!SafeArithmetic::is_power_of_two(6));

        assert_eq!(SafeArithmetic::next_power_of_two(0).unwrap(), 1);
        assert_eq!(SafeArithmetic::next_power_of_two(1).unwrap(), 1);
        assert_eq!(SafeArithmetic::next_power_of_two(2).unwrap(), 2);
        assert_eq!(SafeArithmetic::next_power_of_two(3).unwrap(), 4);
        assert_eq!(SafeArithmetic::next_power_of_two(5).unwrap(), 8);
    }
}
