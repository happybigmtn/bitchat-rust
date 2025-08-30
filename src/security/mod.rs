//! Security hardening module for BitCraps
//!
//! This module provides comprehensive security controls including:
//! - Input validation and sanitization
//! - Bounds checking for all numeric types
//! - Rate limiting and DoS protection
//! - Constant-time operations for sensitive comparisons
//! - Security event logging and monitoring

pub mod input_validation;
pub mod rate_limiting;
pub mod constant_time;
pub mod dos_protection;
pub mod security_events;

pub use input_validation::{InputValidator, ValidationContext, ValidationResult};
pub use rate_limiting::{RateLimiter, RateLimitConfig, RateLimitResult};
pub use constant_time::ConstantTimeOps;
pub use dos_protection::{DosProtection, DosProtectionConfig, ProtectionResult};
pub use security_events::{SecurityEventLogger, SecurityEvent, SecurityLevel};

use crate::error::{Error, Result};
use std::time::{SystemTime, UNIX_EPOCH};

/// Maximum allowed values for various game parameters
#[derive(Clone)]
pub struct SecurityLimits {
    pub max_bet_amount: u64,
    pub max_players_per_game: usize,
    pub max_message_size: usize,
    pub max_games_per_player: usize,
    pub max_bets_per_player: usize,
    pub max_dice_value: u8,
    pub max_timestamp_drift: u64, // seconds
    pub max_string_length: usize,
    pub max_array_length: usize,
}

impl Default for SecurityLimits {
    fn default() -> Self {
        Self {
            max_bet_amount: 1_000_000,        // 1M tokens max bet
            max_players_per_game: 20,         // 20 players max per game
            max_message_size: 64 * 1024,      // 64KB max message
            max_games_per_player: 10,         // 10 concurrent games max
            max_bets_per_player: 50,          // 50 active bets max
            max_dice_value: 6,                // Standard dice: 1-6
            max_timestamp_drift: 300,         // 5 minutes drift allowed
            max_string_length: 1024,          // 1KB max string
            max_array_length: 1000,           // 1000 elements max
        }
    }
}

/// Security configuration for the entire system
pub struct SecurityConfig {
    pub limits: SecurityLimits,
    pub rate_limit_config: RateLimitConfig,
    pub dos_protection_config: DosProtectionConfig,
    pub enable_security_logging: bool,
    pub log_sensitive_data: bool,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            limits: SecurityLimits::default(),
            rate_limit_config: RateLimitConfig::default(),
            dos_protection_config: DosProtectionConfig::default(),
            enable_security_logging: true,
            log_sensitive_data: false, // Never log sensitive data by default
        }
    }
}

/// Central security manager that coordinates all security controls
pub struct SecurityManager {
    config: SecurityConfig,
    validator: InputValidator,
    rate_limiter: RateLimiter,
    dos_protection: DosProtection,
    event_logger: SecurityEventLogger,
}

impl SecurityManager {
    pub fn new(config: SecurityConfig) -> Self {
        let validator = InputValidator::new(&config.limits);
        let rate_limiter = RateLimiter::new(config.rate_limit_config.clone());
        let dos_protection = DosProtection::new(config.dos_protection_config.clone());
        let event_logger = SecurityEventLogger::new(
            config.enable_security_logging,
            config.log_sensitive_data,
        );

        Self {
            config,
            validator,
            rate_limiter,
            dos_protection,
            event_logger,
        }
    }

    /// Validate all aspects of a game join request
    pub fn validate_game_join_request(
        &self,
        game_id: &[u8; 16],
        player_id: &[u8; 32],
        buy_in: u64,
        timestamp: u64,
        client_ip: std::net::IpAddr,
    ) -> Result<()> {
        let context = ValidationContext {
            operation: "game_join".to_string(),
            client_ip: Some(client_ip),
            timestamp: Some(timestamp),
        };

        // Rate limiting check
        let rate_result = self.rate_limiter.check_rate_limit(client_ip, "game_join");
        if rate_result.is_blocked() {
            self.event_logger.log_security_event(
                SecurityEvent::RateLimitExceeded {
                    client_ip,
                    operation: "game_join".to_string(),
                    attempts: rate_result.current_count(),
                },
                SecurityLevel::Warning,
            );
            return Err(Error::Security("Rate limit exceeded for game join".to_string()));
        }

        // DoS protection check
        let dos_result = self.dos_protection.check_request(client_ip, 256); // Estimate request size
        if !dos_result.is_allowed() {
            self.event_logger.log_security_event(
                SecurityEvent::DosAttempt {
                    client_ip,
                    operation: "game_join".to_string(),
                    reason: dos_result.get_reason(),
                },
                SecurityLevel::High,
            );
            return Err(Error::Security("DoS protection triggered".to_string()));
        }

        // Input validation
        self.validator.validate_game_id(game_id, &context)?;
        self.validator.validate_player_id(player_id, &context)?;
        self.validator.validate_bet_amount(buy_in, &context)?;
        self.validator.validate_timestamp(timestamp, &context)?;

        self.event_logger.log_security_event(
            SecurityEvent::ValidatedGameJoin {
                game_id: *game_id,
                player_id: *player_id,
                buy_in,
            },
            SecurityLevel::Info,
        );

        Ok(())
    }

    /// Validate a dice roll commit with entropy validation
    pub fn validate_dice_roll_commit(
        &self,
        game_id: &[u8; 16],
        player_id: &[u8; 32],
        entropy: &[u8; 32],
        commitment: &[u8; 32],
        timestamp: u64,
        client_ip: std::net::IpAddr,
    ) -> Result<()> {
        let context = ValidationContext {
            operation: "dice_roll_commit".to_string(),
            client_ip: Some(client_ip),
            timestamp: Some(timestamp),
        };

        // Rate limiting for dice rolls (more restrictive)
        let rate_result = self.rate_limiter.check_rate_limit(client_ip, "dice_roll");
        if rate_result.is_blocked() {
            self.event_logger.log_security_event(
                SecurityEvent::RateLimitExceeded {
                    client_ip,
                    operation: "dice_roll".to_string(),
                    attempts: rate_result.current_count(),
                },
                SecurityLevel::Warning,
            );
            return Err(Error::Security("Rate limit exceeded for dice roll".to_string()));
        }

        // Validate inputs
        self.validator.validate_game_id(game_id, &context)?;
        self.validator.validate_player_id(player_id, &context)?;
        self.validator.validate_entropy_source(entropy, &context)?;
        self.validator.validate_commitment(commitment, &context)?;
        self.validator.validate_timestamp(timestamp, &context)?;

        self.event_logger.log_security_event(
            SecurityEvent::ValidatedDiceRoll {
                game_id: *game_id,
                player_id: *player_id,
            },
            SecurityLevel::Info,
        );

        Ok(())
    }

    /// Validate network message before processing
    pub fn validate_network_message(
        &self,
        message_data: &[u8],
        sender_ip: std::net::IpAddr,
    ) -> Result<()> {
        // Size validation
        if message_data.len() > self.config.limits.max_message_size {
            self.event_logger.log_security_event(
                SecurityEvent::OversizedMessage {
                    client_ip: sender_ip,
                    size: message_data.len(),
                    max_size: self.config.limits.max_message_size,
                },
                SecurityLevel::Warning,
            );
            return Err(Error::Security(format!(
                "Message too large: {} bytes (max: {})",
                message_data.len(),
                self.config.limits.max_message_size
            )));
        }

        // DoS protection
        let dos_result = self.dos_protection.check_request(sender_ip, message_data.len());
        if !dos_result.is_allowed() {
            self.event_logger.log_security_event(
                SecurityEvent::DosAttempt {
                    client_ip: sender_ip,
                    operation: "network_message".to_string(),
                    reason: dos_result.get_reason(),
                },
                SecurityLevel::High,
            );
            return Err(Error::Security("DoS protection triggered for message".to_string()));
        }

        // Rate limiting
        let rate_result = self.rate_limiter.check_rate_limit(sender_ip, "network_message");
        if rate_result.is_blocked() {
            return Err(Error::Security("Rate limit exceeded for network messages".to_string()));
        }

        Ok(())
    }

    /// Get current security statistics
    pub fn get_security_stats(&self) -> SecurityStats {
        SecurityStats {
            rate_limit_violations: self.rate_limiter.get_violation_count(),
            dos_attempts_blocked: self.dos_protection.get_blocked_count(),
            total_validations: self.validator.get_validation_count(),
            security_events_logged: self.event_logger.get_event_count(),
        }
    }
}

/// Security statistics for monitoring
#[derive(Debug, Clone)]
pub struct SecurityStats {
    pub rate_limit_violations: u64,
    pub dos_attempts_blocked: u64,
    pub total_validations: u64,
    pub security_events_logged: u64,
}

/// Utility function to get current timestamp with bounds checking
pub fn get_current_timestamp() -> Result<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .map_err(|e| Error::InvalidTimestamp(format!("System time error: {}", e)))
}

/// Validate timestamp is within acceptable drift
pub fn validate_timestamp_drift(timestamp: u64, max_drift: u64) -> Result<()> {
    let current = get_current_timestamp()?;
    let drift = if timestamp > current {
        timestamp - current
    } else {
        current - timestamp
    };

    if drift > max_drift {
        return Err(Error::InvalidTimestamp(format!(
            "Timestamp drift too large: {} seconds (max: {})",
            drift, max_drift
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn test_security_limits_defaults() {
        let limits = SecurityLimits::default();
        assert_eq!(limits.max_bet_amount, 1_000_000);
        assert_eq!(limits.max_players_per_game, 20);
        assert_eq!(limits.max_dice_value, 6);
    }

    #[test]
    fn test_timestamp_validation() {
        let current = get_current_timestamp().unwrap();
        
        // Valid timestamp (within drift)
        assert!(validate_timestamp_drift(current, 300).is_ok());
        assert!(validate_timestamp_drift(current + 100, 300).is_ok());
        assert!(validate_timestamp_drift(current.saturating_sub(100), 300).is_ok());
        
        // Invalid timestamp (outside drift)
        assert!(validate_timestamp_drift(current + 400, 300).is_err());
        assert!(validate_timestamp_drift(current.saturating_sub(400), 300).is_err());
    }

    #[test]
    fn test_security_manager_creation() {
        let config = SecurityConfig::default();
        let manager = SecurityManager::new(config);
        
        let stats = manager.get_security_stats();
        assert_eq!(stats.total_validations, 0);
        assert_eq!(stats.rate_limit_violations, 0);
    }

    #[test]
    fn test_game_join_validation() {
        let config = SecurityConfig::default();
        let manager = SecurityManager::new(config);
        
        let game_id = [1u8; 16];
        let player_id = [2u8; 32];
        let buy_in = 1000;
        let timestamp = get_current_timestamp().unwrap();
        let client_ip = std::net::IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        
        // Should pass validation
        let result = manager.validate_game_join_request(
            &game_id,
            &player_id,
            buy_in,
            timestamp,
            client_ip,
        );
        
        assert!(result.is_ok());
    }
}