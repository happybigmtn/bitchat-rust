//! Comprehensive input validation for all user inputs
//!
//! This module provides validation for:
//! - Game parameters (IDs, amounts, counts)
//! - Network data (messages, packets, addresses)
//! - Cryptographic inputs (keys, signatures, entropy)
//! - Temporal data (timestamps, durations)

use crate::error::{Error, Result};
use crate::security::SecurityLimits;
use std::net::IpAddr;
use std::sync::atomic::{AtomicU64, Ordering};

/// Validation context for security logging and tracing
#[derive(Debug, Clone)]
pub struct ValidationContext {
    pub operation: String,
    pub client_ip: Option<IpAddr>,
    pub timestamp: Option<u64>,
}

/// Result of input validation with detailed information
#[derive(Debug, Clone)]
pub enum ValidationResult {
    Valid,
    Invalid { reason: String },
}

impl ValidationResult {
    pub fn is_valid(&self) -> bool {
        matches!(self, ValidationResult::Valid)
    }

    pub fn is_invalid(&self) -> bool {
        !self.is_valid()
    }

    pub fn error_message(&self) -> Option<&str> {
        match self {
            ValidationResult::Valid => None,
            ValidationResult::Invalid { reason } => Some(reason),
        }
    }
}

/// Comprehensive input validator with security bounds checking
pub struct InputValidator {
    limits: SecurityLimits,
    validation_count: AtomicU64,
    violation_count: AtomicU64,
}

impl InputValidator {
    pub fn new(limits: &SecurityLimits) -> Self {
        Self {
            limits: limits.clone(),
            validation_count: AtomicU64::new(0),
            violation_count: AtomicU64::new(0),
        }
    }

    /// Get total number of validations performed
    pub fn get_validation_count(&self) -> u64 {
        self.validation_count.load(Ordering::Relaxed)
    }

    /// Get total number of validation violations
    pub fn get_violation_count(&self) -> u64 {
        self.violation_count.load(Ordering::Relaxed)
    }

    /// Internal method to record validation attempt
    fn record_validation(&self, is_valid: bool) {
        self.validation_count.fetch_add(1, Ordering::Relaxed);
        if !is_valid {
            self.violation_count.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Validate game ID format and bounds
    pub fn validate_game_id(&self, game_id: &[u8; 16], context: &ValidationContext) -> Result<()> {
        self.record_validation(true);

        // Check for all-zero ID (invalid)
        if *game_id == [0u8; 16] {
            self.record_validation(false);
            return Err(Error::InvalidInput(format!(
                "Invalid game ID: all-zero ID not allowed (operation: {})",
                context.operation
            )));
        }

        // Check for all-ones ID (reserved)
        if *game_id == [0xFFu8; 16] {
            self.record_validation(false);
            return Err(Error::InvalidInput(format!(
                "Invalid game ID: all-ones ID is reserved (operation: {})",
                context.operation
            )));
        }

        Ok(())
    }

    /// Validate player ID format and bounds
    pub fn validate_player_id(&self, player_id: &[u8; 32], context: &ValidationContext) -> Result<()> {
        self.record_validation(true);

        // Check for all-zero ID (invalid)
        if *player_id == [0u8; 32] {
            self.record_validation(false);
            return Err(Error::InvalidInput(format!(
                "Invalid player ID: all-zero ID not allowed (operation: {})",
                context.operation
            )));
        }

        // Check for all-ones ID (reserved)
        if *player_id == [0xFFu8; 32] {
            self.record_validation(false);
            return Err(Error::InvalidInput(format!(
                "Invalid player ID: all-ones ID is reserved (operation: {})",
                context.operation
            )));
        }

        Ok(())
    }

    /// Validate bet amount with bounds checking
    pub fn validate_bet_amount(&self, amount: u64, context: &ValidationContext) -> Result<()> {
        self.record_validation(true);

        // Check minimum bet (must be positive)
        if amount == 0 {
            self.record_validation(false);
            return Err(Error::InvalidBet(format!(
                "Invalid bet amount: zero amount not allowed (operation: {})",
                context.operation
            )));
        }

        // Check maximum bet
        if amount > self.limits.max_bet_amount {
            self.record_validation(false);
            return Err(Error::InvalidBet(format!(
                "Bet amount {} exceeds maximum {} (operation: {})",
                amount, self.limits.max_bet_amount, context.operation
            )));
        }

        // Check for overflow-prone values
        if amount > u64::MAX / 100 {
            self.record_validation(false);
            return Err(Error::ArithmeticOverflow(format!(
                "Bet amount {} too large for safe arithmetic (operation: {})",
                amount, context.operation
            )));
        }

        Ok(())
    }

    /// Validate dice values
    pub fn validate_dice_value(&self, value: u8, context: &ValidationContext) -> Result<()> {
        self.record_validation(true);

        if value < 1 || value > self.limits.max_dice_value {
            self.record_validation(false);
            return Err(Error::InvalidInput(format!(
                "Invalid dice value {}: must be 1-{} (operation: {})",
                value, self.limits.max_dice_value, context.operation
            )));
        }

        Ok(())
    }

    /// Validate dice roll with both dice values
    pub fn validate_dice_roll(&self, die1: u8, die2: u8, context: &ValidationContext) -> Result<()> {
        self.validate_dice_value(die1, context)?;
        self.validate_dice_value(die2, context)?;

        // Additional validation: ensure dice total is valid
        let total = die1 + die2;
        if total < 2 || total > (self.limits.max_dice_value * 2) {
            self.record_validation(false);
            return Err(Error::InvalidInput(format!(
                "Invalid dice total {}: dice values {}, {} (operation: {})",
                total, die1, die2, context.operation
            )));
        }

        Ok(())
    }

    /// Validate timestamp with drift checking
    pub fn validate_timestamp(&self, timestamp: u64, context: &ValidationContext) -> Result<()> {
        self.record_validation(true);

        // Get current time
        let current = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| Error::InvalidTimestamp(format!("System time error: {}", e)))?
            .as_secs();

        // Check for timestamps too far in the past (more than 1 hour)
        let min_timestamp = current.saturating_sub(3600);
        if timestamp < min_timestamp {
            self.record_validation(false);
            return Err(Error::InvalidTimestamp(format!(
                "Timestamp {} too old (current: {}, operation: {})",
                timestamp, current, context.operation
            )));
        }

        // Check for timestamps too far in the future
        let max_timestamp = current + self.limits.max_timestamp_drift;
        if timestamp > max_timestamp {
            self.record_validation(false);
            return Err(Error::InvalidTimestamp(format!(
                "Timestamp {} too far in future (current: {}, max_drift: {}, operation: {})",
                timestamp, current, self.limits.max_timestamp_drift, context.operation
            )));
        }

        Ok(())
    }

    /// Validate entropy source for randomness quality
    pub fn validate_entropy_source(&self, entropy: &[u8; 32], context: &ValidationContext) -> Result<()> {
        self.record_validation(true);

        // Check for all-zero entropy (completely invalid)
        if *entropy == [0u8; 32] {
            self.record_validation(false);
            return Err(Error::Crypto(format!(
                "Invalid entropy: all-zero entropy not allowed (operation: {})",
                context.operation
            )));
        }

        // Check for all-ones entropy (suspicious)
        if *entropy == [0xFFu8; 32] {
            self.record_validation(false);
            return Err(Error::Crypto(format!(
                "Invalid entropy: all-ones entropy not allowed (operation: {})",
                context.operation
            )));
        }

        // Basic entropy quality check: ensure some variation
        let mut zero_bytes = 0;
        let mut ones_bytes = 0;
        for &byte in entropy.iter() {
            if byte == 0 {
                zero_bytes += 1;
            } else if byte == 0xFF {
                ones_bytes += 1;
            }
        }

        // If more than 75% of bytes are zero or ones, reject
        if zero_bytes > 24 || ones_bytes > 24 {
            self.record_validation(false);
            return Err(Error::Crypto(format!(
                "Low entropy quality: too many identical bytes (operation: {})",
                context.operation
            )));
        }

        Ok(())
    }

    /// Validate cryptographic commitment
    pub fn validate_commitment(&self, commitment: &[u8; 32], context: &ValidationContext) -> Result<()> {
        self.record_validation(true);

        // Check for all-zero commitment (invalid)
        if *commitment == [0u8; 32] {
            self.record_validation(false);
            return Err(Error::Crypto(format!(
                "Invalid commitment: all-zero commitment not allowed (operation: {})",
                context.operation
            )));
        }

        // Commitments should look random - no obvious patterns
        let mut pattern_score = 0;
        for window in commitment.windows(4) {
            if window[0] == window[1] && window[1] == window[2] && window[2] == window[3] {
                pattern_score += 1;
            }
        }

        if pattern_score > 2 {
            self.record_validation(false);
            return Err(Error::Crypto(format!(
                "Suspicious commitment: too many repeated patterns (operation: {})",
                context.operation
            )));
        }

        Ok(())
    }

    /// Validate string input with length and content checks
    pub fn validate_string(&self, input: &str, field_name: &str, context: &ValidationContext) -> Result<()> {
        self.record_validation(true);

        // Length check
        if input.len() > self.limits.max_string_length {
            self.record_validation(false);
            return Err(Error::InvalidInput(format!(
                "String '{}' too long: {} chars (max: {}, operation: {})",
                field_name, input.len(), self.limits.max_string_length, context.operation
            )));
        }

        // Content validation: no null bytes
        if input.contains('\0') {
            self.record_validation(false);
            return Err(Error::InvalidInput(format!(
                "String '{}' contains null bytes (operation: {})",
                field_name, context.operation
            )));
        }

        // Content validation: no control characters except tab, newline, carriage return
        for ch in input.chars() {
            if ch.is_control() && ch != '\t' && ch != '\n' && ch != '\r' {
                self.record_validation(false);
                return Err(Error::InvalidInput(format!(
                    "String '{}' contains invalid control character (operation: {})",
                    field_name, context.operation
                )));
            }
        }

        Ok(())
    }

    /// Validate array length
    pub fn validate_array_length<T>(&self, array: &[T], field_name: &str, context: &ValidationContext) -> Result<()> {
        self.record_validation(true);

        if array.len() > self.limits.max_array_length {
            self.record_validation(false);
            return Err(Error::InvalidInput(format!(
                "Array '{}' too long: {} elements (max: {}, operation: {})",
                field_name, array.len(), self.limits.max_array_length, context.operation
            )));
        }

        Ok(())
    }

    /// Validate player count for game
    pub fn validate_player_count(&self, count: usize, context: &ValidationContext) -> Result<()> {
        self.record_validation(true);

        if count == 0 {
            self.record_validation(false);
            return Err(Error::InvalidInput(format!(
                "Invalid player count: cannot be zero (operation: {})",
                context.operation
            )));
        }

        if count > self.limits.max_players_per_game {
            self.record_validation(false);
            return Err(Error::InvalidInput(format!(
                "Too many players: {} (max: {}, operation: {})",
                count, self.limits.max_players_per_game, context.operation
            )));
        }

        Ok(())
    }

    /// Validate network message size
    pub fn validate_message_size(&self, size: usize, context: &ValidationContext) -> Result<()> {
        self.record_validation(true);

        if size == 0 {
            self.record_validation(false);
            return Err(Error::InvalidInput(format!(
                "Invalid message size: cannot be zero (operation: {})",
                context.operation
            )));
        }

        if size > self.limits.max_message_size {
            self.record_validation(false);
            return Err(Error::InvalidInput(format!(
                "Message too large: {} bytes (max: {}, operation: {})",
                size, self.limits.max_message_size, context.operation
            )));
        }

        Ok(())
    }

    /// Validate IP address (basic checks for suspicious patterns)
    pub fn validate_ip_address(&self, ip: IpAddr, context: &ValidationContext) -> Result<()> {
        self.record_validation(true);

        match ip {
            IpAddr::V4(ipv4) => {
                // Block obviously invalid addresses
                if ipv4.is_unspecified() || ipv4.is_broadcast() {
                    self.record_validation(false);
                    return Err(Error::InvalidInput(format!(
                        "Invalid IPv4 address: {} (operation: {})",
                        ipv4, context.operation
                    )));
                }
            }
            IpAddr::V6(ipv6) => {
                // Block unspecified IPv6
                if ipv6.is_unspecified() {
                    self.record_validation(false);
                    return Err(Error::InvalidInput(format!(
                        "Invalid IPv6 address: {} (operation: {})",
                        ipv6, context.operation
                    )));
                }
            }
        }

        Ok(())
    }

    /// Comprehensive validation for game join requests
    pub fn validate_complete_game_join(
        &self,
        game_id: &[u8; 16],
        player_id: &[u8; 32],
        buy_in: u64,
        timestamp: u64,
        player_name: Option<&str>,
        context: &ValidationContext,
    ) -> Result<()> {
        // Validate all components
        self.validate_game_id(game_id, context)?;
        self.validate_player_id(player_id, context)?;
        self.validate_bet_amount(buy_in, context)?;
        self.validate_timestamp(timestamp, context)?;

        // Validate optional player name
        if let Some(name) = player_name {
            self.validate_string(name, "player_name", context)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::SecurityLimits;
    use std::net::{Ipv4Addr, Ipv6Addr};

    fn create_test_validator() -> InputValidator {
        let limits = SecurityLimits::default();
        InputValidator::new(&limits)
    }

    fn create_test_context() -> ValidationContext {
        ValidationContext {
            operation: "test".to_string(),
            client_ip: Some(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))),
            timestamp: None,
        }
    }

    #[test]
    fn test_game_id_validation() {
        let validator = create_test_validator();
        let context = create_test_context();

        // Valid game ID
        let valid_id = [1u8; 16];
        assert!(validator.validate_game_id(&valid_id, &context).is_ok());

        // Invalid: all-zero ID
        let zero_id = [0u8; 16];
        assert!(validator.validate_game_id(&zero_id, &context).is_err());

        // Invalid: all-ones ID
        let ones_id = [0xFFu8; 16];
        assert!(validator.validate_game_id(&ones_id, &context).is_err());
    }

    #[test]
    fn test_bet_amount_validation() {
        let validator = create_test_validator();
        let context = create_test_context();

        // Valid bet amounts
        assert!(validator.validate_bet_amount(100, &context).is_ok());
        assert!(validator.validate_bet_amount(1000, &context).is_ok());
        assert!(validator.validate_bet_amount(500_000, &context).is_ok());

        // Invalid: zero amount
        assert!(validator.validate_bet_amount(0, &context).is_err());

        // Invalid: exceeds maximum
        assert!(validator.validate_bet_amount(2_000_000, &context).is_err());

        // Invalid: overflow-prone value
        assert!(validator.validate_bet_amount(u64::MAX, &context).is_err());
    }

    #[test]
    fn test_dice_validation() {
        let validator = create_test_validator();
        let context = create_test_context();

        // Valid dice values
        assert!(validator.validate_dice_value(1, &context).is_ok());
        assert!(validator.validate_dice_value(6, &context).is_ok());

        // Invalid dice values
        assert!(validator.validate_dice_value(0, &context).is_err());
        assert!(validator.validate_dice_value(7, &context).is_err());

        // Valid dice rolls
        assert!(validator.validate_dice_roll(1, 1, &context).is_ok());
        assert!(validator.validate_dice_roll(6, 6, &context).is_ok());
        assert!(validator.validate_dice_roll(3, 4, &context).is_ok());

        // Invalid dice rolls
        assert!(validator.validate_dice_roll(0, 1, &context).is_err());
        assert!(validator.validate_dice_roll(1, 7, &context).is_err());
    }

    #[test]
    fn test_entropy_validation() {
        let validator = create_test_validator();
        let context = create_test_context();

        // Valid entropy
        let good_entropy = [42u8; 32];
        assert!(validator.validate_entropy_source(&good_entropy, &context).is_ok());

        // Invalid: all-zero entropy
        let zero_entropy = [0u8; 32];
        assert!(validator.validate_entropy_source(&zero_entropy, &context).is_err());

        // Invalid: all-ones entropy
        let ones_entropy = [0xFFu8; 32];
        assert!(validator.validate_entropy_source(&ones_entropy, &context).is_err());

        // Invalid: low entropy (mostly zeros)
        let mut low_entropy = [42u8; 32];
        for i in 0..28 {
            low_entropy[i] = 0;
        }
        assert!(validator.validate_entropy_source(&low_entropy, &context).is_err());
    }

    #[test]
    fn test_string_validation() {
        let validator = create_test_validator();
        let context = create_test_context();

        // Valid strings
        assert!(validator.validate_string("valid_name", "test", &context).is_ok());
        assert!(validator.validate_string("Player123", "test", &context).is_ok());

        // Invalid: too long
        let long_string = "x".repeat(2000);
        assert!(validator.validate_string(&long_string, "test", &context).is_err());

        // Invalid: contains null byte
        let null_string = "invalid\0string";
        assert!(validator.validate_string(null_string, "test", &context).is_err());

        // Invalid: contains control characters
        let control_string = "invalid\x01string";
        assert!(validator.validate_string(control_string, "test", &context).is_err());
    }

    #[test]
    fn test_ip_validation() {
        let validator = create_test_validator();
        let context = create_test_context();

        // Valid IPs
        let valid_ipv4 = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));
        assert!(validator.validate_ip_address(valid_ipv4, &context).is_ok());

        let valid_ipv6 = IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1));
        assert!(validator.validate_ip_address(valid_ipv6, &context).is_ok());

        // Invalid IPs
        let invalid_ipv4 = IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0));
        assert!(validator.validate_ip_address(invalid_ipv4, &context).is_err());

        let invalid_ipv6 = IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0));
        assert!(validator.validate_ip_address(invalid_ipv6, &context).is_err());
    }

    #[test]
    fn test_validation_counters() {
        let validator = create_test_validator();
        let context = create_test_context();

        let initial_count = validator.get_validation_count();
        let initial_violations = validator.get_violation_count();

        // Perform some validations
        let _ = validator.validate_bet_amount(100, &context); // Valid
        let _ = validator.validate_bet_amount(0, &context);   // Invalid

        assert_eq!(validator.get_validation_count(), initial_count + 2);
        assert_eq!(validator.get_violation_count(), initial_violations + 1);
    }
}