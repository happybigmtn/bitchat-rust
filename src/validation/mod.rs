//! Input validation framework for production safety
//!
//! Provides comprehensive validation for all external inputs to prevent:
//! - Buffer overflows
//! - SQL injection
//! - Integer overflows
//! - Malformed data attacks
//! - DoS through resource exhaustion

use crate::error::{Error, Result};
use crate::protocol::PeerId;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Validation rules for different input types
#[derive(Debug, Clone)]
pub struct ValidationRules {
    pub max_packet_size: usize,
    pub max_string_length: usize,
    pub max_array_length: usize,
    pub max_bet_amount: u64,
    pub min_bet_amount: u64,
    pub max_players_per_game: usize,
    pub max_games_per_player: usize,
    pub max_message_rate: u32,
    pub rate_limit_window: Duration,
    pub require_signatures: bool,
    pub allow_anonymous: bool,
}

impl Default for ValidationRules {
    fn default() -> Self {
        Self {
            max_packet_size: 65536,
            max_string_length: 1024,
            max_array_length: 1000,
            max_bet_amount: 1_000_000,
            min_bet_amount: 1,
            max_players_per_game: 8,
            max_games_per_player: 5,
            max_message_rate: 100,
            rate_limit_window: Duration::from_secs(60),
            require_signatures: true,
            allow_anonymous: false,
        }
    }
}

impl ValidationRules {
    /// Create validation rules from application configuration
    pub fn from_app_config(config: &crate::app::ApplicationConfig) -> Self {
        Self {
            max_packet_size: 65536,
            max_string_length: config.max_string_length,
            max_array_length: config.max_array_length,
            max_bet_amount: 1_000_000,
            min_bet_amount: 1,
            max_players_per_game: 8,
            max_games_per_player: 5,
            max_message_rate: config.max_message_rate,
            rate_limit_window: Duration::from_secs(60),
            require_signatures: true,
            allow_anonymous: false,
        }
    }
}

/// Input validator with rate limiting
pub struct InputValidator {
    rules: ValidationRules,
    rate_limiter: Arc<RateLimiter>,
    sanitizer: Arc<InputSanitizer>,
}

/// Rate limiter using token bucket algorithm
pub struct RateLimiter {
    buckets: Arc<RwLock<HashMap<PeerId, TokenBucket>>>,
    max_requests: u32,
    window: Duration,
}

/// Token bucket for rate limiting
#[derive(Debug, Clone)]
struct TokenBucket {
    tokens: f64,
    last_refill: Instant,
    max_tokens: f64,
    refill_rate: f64,
}

/// Input sanitizer for cleaning dangerous inputs
pub struct InputSanitizer {
    dangerous_patterns: Vec<regex::Regex>,
    _max_depth: usize,
}

impl InputValidator {
    /// Create a new input validator
    pub fn new(rules: ValidationRules) -> Result<Self> {
        Ok(Self {
            rules: rules.clone(),
            rate_limiter: Arc::new(RateLimiter::new(
                rules.max_message_rate,
                rules.rate_limit_window,
            )),
            sanitizer: Arc::new(InputSanitizer::new().map_err(|e| {
                Error::ValidationError(format!("Failed to initialize input sanitizer: {}", e))
            })?),
        })
    }

    /// Validate a packet
    pub async fn validate_packet(&self, data: &[u8], sender: PeerId) -> Result<()> {
        // Check rate limit
        if !self.rate_limiter.check_and_consume(sender, 1.0).await? {
            return Err(Error::ValidationError("Rate limit exceeded".to_string()));
        }

        // Check packet size
        if data.len() > self.rules.max_packet_size {
            return Err(Error::ValidationError(format!(
                "Packet size {} exceeds maximum {}",
                data.len(),
                self.rules.max_packet_size
            )));
        }

        // Check for malformed data
        if data.is_empty() {
            return Err(Error::ValidationError("Empty packet".to_string()));
        }

        // Check packet structure (basic validation)
        if data.len() < 4 {
            return Err(Error::ValidationError("Packet too small".to_string()));
        }

        Ok(())
    }

    /// Validate a bet
    pub fn validate_bet(&self, amount: u64, player_balance: u64) -> Result<()> {
        // Check bet limits
        if amount < self.rules.min_bet_amount {
            return Err(Error::ValidationError(format!(
                "Bet {} below minimum {}",
                amount, self.rules.min_bet_amount
            )));
        }

        if amount > self.rules.max_bet_amount {
            return Err(Error::ValidationError(format!(
                "Bet {} exceeds maximum {}",
                amount, self.rules.max_bet_amount
            )));
        }

        // Check player balance
        if amount > player_balance {
            return Err(Error::ValidationError(format!(
                "Bet {} exceeds balance {}",
                amount, player_balance
            )));
        }

        // Check for integer overflow
        if amount == u64::MAX {
            return Err(Error::ValidationError("Invalid bet amount".to_string()));
        }

        Ok(())
    }

    /// Validate game parameters
    pub fn validate_game_params(
        &self,
        num_players: usize,
        min_bet: u64,
        max_bet: u64,
    ) -> Result<()> {
        // Check player count
        if num_players == 0 {
            return Err(Error::ValidationError(
                "Game requires at least 1 player".to_string(),
            ));
        }

        if num_players > self.rules.max_players_per_game {
            return Err(Error::ValidationError(format!(
                "Player count {} exceeds maximum {}",
                num_players, self.rules.max_players_per_game
            )));
        }

        // Check bet limits
        if min_bet > max_bet {
            return Err(Error::ValidationError(
                "Min bet exceeds max bet".to_string(),
            ));
        }

        if max_bet > self.rules.max_bet_amount {
            return Err(Error::ValidationError(format!(
                "Max bet {} exceeds system maximum {}",
                max_bet, self.rules.max_bet_amount
            )));
        }

        Ok(())
    }

    /// Validate a string input
    pub fn validate_string(&self, input: &str, field_name: &str) -> Result<String> {
        // Check length
        if input.len() > self.rules.max_string_length {
            return Err(Error::ValidationError(format!(
                "{} length {} exceeds maximum {}",
                field_name,
                input.len(),
                self.rules.max_string_length
            )));
        }

        // Sanitize dangerous characters
        let sanitized = self.sanitizer.sanitize_string(input)?;

        // Check for null bytes
        if sanitized.contains('\0') {
            return Err(Error::ValidationError(format!(
                "{} contains null bytes",
                field_name
            )));
        }

        Ok(sanitized)
    }

    /// Validate an array/vector input
    pub fn validate_array<T>(&self, array: &[T], field_name: &str) -> Result<()> {
        if array.len() > self.rules.max_array_length {
            return Err(Error::ValidationError(format!(
                "{} length {} exceeds maximum {}",
                field_name,
                array.len(),
                self.rules.max_array_length
            )));
        }

        Ok(())
    }

    /// Validate a peer ID
    pub fn validate_peer_id(&self, peer_id: &PeerId) -> Result<()> {
        // Check for reserved addresses
        if peer_id.iter().all(|&b| b == 0x00) {
            return Err(Error::ValidationError(
                "Invalid peer ID: all zeros".to_string(),
            ));
        }

        if peer_id.iter().all(|&b| b == 0xFF) {
            return Err(Error::ValidationError(
                "Invalid peer ID: reserved address".to_string(),
            ));
        }

        Ok(())
    }

    /// Validate a signature
    pub fn validate_signature(&self, signature: &[u8]) -> Result<()> {
        if self.rules.require_signatures && signature.is_empty() {
            return Err(Error::ValidationError("Signature required".to_string()));
        }

        if signature.len() != 64 && !signature.is_empty() {
            return Err(Error::ValidationError(format!(
                "Invalid signature length: {}",
                signature.len()
            )));
        }

        Ok(())
    }

    /// Validate a network address
    pub fn validate_address(&self, address: &str) -> Result<()> {
        if address.is_empty() {
            return Err(Error::ValidationError("Address cannot be empty".to_string()));
        }

        if address.len() > self.rules.max_string_length {
            return Err(Error::ValidationError(format!(
                "Address length {} exceeds maximum {}",
                address.len(),
                self.rules.max_string_length
            )));
        }

        // Basic format validation - check for common address patterns
        if !address.contains(':') && !address.contains('.') && !address.starts_with('/') {
            return Err(Error::ValidationError(
                "Address format not recognized".to_string(),
            ));
        }

        // Check for dangerous characters
        if address.contains('\0') || address.contains('\r') || address.contains('\n') {
            return Err(Error::ValidationError(
                "Address contains invalid characters".to_string(),
            ));
        }

        Ok(())
    }
}

impl RateLimiter {
    fn new(max_requests: u32, window: Duration) -> Self {
        Self {
            buckets: Arc::new(RwLock::new(HashMap::new())),
            max_requests,
            window,
        }
    }

    /// Check if request is allowed and consume tokens
    pub async fn check_and_consume(&self, peer: PeerId, tokens: f64) -> Result<bool> {
        let mut buckets = self.buckets.write().await;

        let bucket = buckets.entry(peer).or_insert_with(|| TokenBucket {
            tokens: self.max_requests as f64,
            last_refill: Instant::now(),
            max_tokens: self.max_requests as f64,
            refill_rate: self.max_requests as f64 / self.window.as_secs_f64(),
        });

        // Refill tokens based on elapsed time
        let now = Instant::now();
        let elapsed = now.duration_since(bucket.last_refill).as_secs_f64();
        bucket.tokens = (bucket.tokens + elapsed * bucket.refill_rate).min(bucket.max_tokens);
        bucket.last_refill = now;

        // Check if we have enough tokens
        if bucket.tokens >= tokens {
            bucket.tokens -= tokens;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Reset rate limit for a peer
    pub async fn reset(&self, peer: PeerId) {
        let mut buckets = self.buckets.write().await;
        buckets.remove(&peer);
    }

    /// Clean up old buckets
    pub async fn cleanup(&self) {
        let mut buckets = self.buckets.write().await;
        let now = Instant::now();

        buckets
            .retain(|_, bucket| now.duration_since(bucket.last_refill) < Duration::from_secs(3600));
    }
}

impl InputSanitizer {
    fn new() -> Result<Self> {
        // Compile dangerous patterns
        let patterns = vec![
            regex::Regex::new(r"<script.*?>.*?</script>")
                .map_err(|e| Error::ValidationError(format!("Invalid regex pattern: {}", e)))?, // XSS
            regex::Regex::new(r"javascript:")
                .map_err(|e| Error::ValidationError(format!("Invalid regex pattern: {}", e)))?, // JS injection
            regex::Regex::new(r"on\w+\s*=")
                .map_err(|e| Error::ValidationError(format!("Invalid regex pattern: {}", e)))?, // Event handlers
            regex::Regex::new(r"[';]--")
                .map_err(|e| Error::ValidationError(format!("Invalid regex pattern: {}", e)))?, // SQL comments
            regex::Regex::new(r"union\s+select")
                .map_err(|e| Error::ValidationError(format!("Invalid regex pattern: {}", e)))?, // SQL injection
            regex::Regex::new(r"exec\s*\(")
                .map_err(|e| Error::ValidationError(format!("Invalid regex pattern: {}", e)))?, // Code execution
            regex::Regex::new(r"eval\s*\(")
                .map_err(|e| Error::ValidationError(format!("Invalid regex pattern: {}", e)))?, // Eval
            regex::Regex::new(r"\.\./")
                .map_err(|e| Error::ValidationError(format!("Invalid regex pattern: {}", e)))?, // Path traversal
            regex::Regex::new(r"\\x[0-9a-f]{2}")
                .map_err(|e| Error::ValidationError(format!("Invalid regex pattern: {}", e)))?, // Hex encoding
        ];

        Ok(Self {
            dangerous_patterns: patterns,
            _max_depth: 10,
        })
    }

    /// Sanitize a string input
    pub fn sanitize_string(&self, input: &str) -> Result<String> {
        let mut sanitized = input.to_string();

        // Remove dangerous patterns
        for pattern in &self.dangerous_patterns {
            sanitized = pattern.replace_all(&sanitized, "").to_string();
        }

        // Remove control characters except newline and tab
        sanitized = sanitized
            .chars()
            .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
            .collect();

        // Trim whitespace
        sanitized = sanitized.trim().to_string();

        Ok(sanitized)
    }

    /// Sanitize binary data
    pub fn sanitize_bytes(&self, data: &[u8]) -> Result<Vec<u8>> {
        // Check for common attack patterns in binary
        if data.len() > 4 {
            // Check for zip bombs (high compression ratio)
            if data[0..4] == [0x50, 0x4B, 0x03, 0x04] {
                // ZIP file header
                return Err(Error::ValidationError(
                    "Compressed data not allowed".to_string(),
                ));
            }

            // Check for executable headers
            if data[0..2] == [0x4D, 0x5A] {
                // MZ header
                return Err(Error::ValidationError(
                    "Executable files not allowed".to_string(),
                ));
            }

            if data[0..4] == [0x7F, 0x45, 0x4C, 0x46] {
                // ELF header
                return Err(Error::ValidationError(
                    "Executable files not allowed".to_string(),
                ));
            }
        }

        Ok(data.to_vec())
    }
}

/// Validation middleware for automatic input checking
pub struct ValidationMiddleware {
    validator: Arc<InputValidator>,
    stats: Arc<RwLock<ValidationStats>>,
}

#[derive(Debug, Default)]
pub struct ValidationStats {
    pub total_requests: u64,
    pub rejected_requests: u64,
    pub rate_limited_requests: u64,
    pub malformed_requests: u64,
}

impl ValidationMiddleware {
    pub fn new(rules: ValidationRules) -> Result<Self> {
        Ok(Self {
            validator: Arc::new(InputValidator::new(rules)?),
            stats: Arc::new(RwLock::new(ValidationStats::default())),
        })
    }

    /// Process and validate incoming data
    pub async fn process(&self, data: &[u8], sender: PeerId) -> Result<Vec<u8>> {
        let mut stats = self.stats.write().await;
        stats.total_requests += 1;

        // Validate packet
        if let Err(e) = self.validator.validate_packet(data, sender).await {
            stats.rejected_requests += 1;
            if e.to_string().contains("Rate limit") {
                stats.rate_limited_requests += 1;
            } else {
                stats.malformed_requests += 1;
            }
            return Err(e);
        }

        // Sanitize data
        let sanitized = self.validator.sanitizer.sanitize_bytes(data)?;

        Ok(sanitized)
    }

    /// Get validation statistics
    pub async fn get_stats(&self) -> ValidationStats {
        let stats = self.stats.read().await;
        ValidationStats {
            total_requests: stats.total_requests,
            rejected_requests: stats.rejected_requests,
            rate_limited_requests: stats.rate_limited_requests,
            malformed_requests: stats.malformed_requests,
        }
    }

    /// Reset statistics
    pub async fn reset_stats(&self) {
        *self.stats.write().await = ValidationStats::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limiting() {
        let validator = InputValidator::new(ValidationRules {
            max_message_rate: 5,
            rate_limit_window: Duration::from_secs(1),
            ..Default::default()
        })
        .expect("Failed to create validator");

        let peer = [1u8; 32];
        let data = vec![0u8; 100];

        // First 5 requests should succeed
        for _ in 0..5 {
            assert!(validator.validate_packet(&data, peer).await.is_ok());
        }

        // 6th request should fail
        assert!(validator.validate_packet(&data, peer).await.is_err());
    }

    #[test]
    fn test_bet_validation() {
        let validator =
            InputValidator::new(ValidationRules::default()).expect("Failed to create validator");

        // Valid bet
        assert!(validator.validate_bet(100, 1000).is_ok());

        // Bet exceeds balance
        assert!(validator.validate_bet(1001, 1000).is_err());

        // Bet below minimum
        assert!(validator.validate_bet(0, 1000).is_err());
    }

    #[test]
    fn test_string_sanitization() {
        let sanitizer = InputSanitizer::new().unwrap();

        // XSS attempt
        let input = "<script>alert('xss')</script>Hello";
        let result = sanitizer.sanitize_string(input).unwrap();
        assert_eq!(result, "Hello");

        // SQL injection attempt
        let input = "'; DROP TABLE users; --";
        let result = sanitizer.sanitize_string(input).unwrap();
        assert!(!result.contains("--"));
    }
}
