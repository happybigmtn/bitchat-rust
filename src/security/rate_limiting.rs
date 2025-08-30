//! Rate limiting implementation for BitCraps security
//!
//! This module provides rate limiting to prevent abuse and DoS attacks:
//! - Token bucket algorithm for smooth rate limiting
//! - Per-IP and per-operation rate limits
//! - Configurable windows and burst limits
//! - Automatic cleanup of expired entries

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use std::sync::atomic::{AtomicU64, Ordering};

/// Rate limiting configuration for different operations
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Default requests per minute
    pub default_rpm: u32,
    /// Game join requests per minute per IP
    pub game_join_rpm: u32,
    /// Dice roll requests per minute per IP
    pub dice_roll_rpm: u32,
    /// General network messages per minute per IP
    pub network_message_rpm: u32,
    /// Bet placement requests per minute per IP
    pub bet_placement_rpm: u32,
    /// Maximum burst size (multiple of RPM)
    pub burst_multiplier: f32,
    /// Cleanup interval for expired buckets
    pub cleanup_interval: Duration,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            default_rpm: 60,        // 1 request per second default
            game_join_rpm: 10,      // 10 game joins per minute
            dice_roll_rpm: 30,      // 30 dice rolls per minute
            network_message_rpm: 300, // 300 messages per minute
            bet_placement_rpm: 60,   // 60 bets per minute
            burst_multiplier: 1.5,   // Allow 50% burst
            cleanup_interval: Duration::from_secs(300), // Cleanup every 5 minutes
        }
    }
}

/// Result of rate limit check
#[derive(Debug, Clone)]
pub enum RateLimitResult {
    Allowed { remaining: u32 },
    Blocked { retry_after: Duration, current_count: u32 },
}

impl RateLimitResult {
    pub fn is_allowed(&self) -> bool {
        matches!(self, RateLimitResult::Allowed { .. })
    }

    pub fn is_blocked(&self) -> bool {
        !self.is_allowed()
    }

    pub fn remaining_requests(&self) -> Option<u32> {
        match self {
            RateLimitResult::Allowed { remaining } => Some(*remaining),
            RateLimitResult::Blocked { .. } => None,
        }
    }

    pub fn retry_after(&self) -> Option<Duration> {
        match self {
            RateLimitResult::Allowed { .. } => None,
            RateLimitResult::Blocked { retry_after, .. } => Some(*retry_after),
        }
    }

    pub fn current_count(&self) -> u32 {
        match self {
            RateLimitResult::Allowed { remaining } => {
                // This is an approximation since we don't store the limit in the result
                100 - remaining // Assume limit was around 100
            },
            RateLimitResult::Blocked { current_count, .. } => *current_count,
        }
    }
}

/// Token bucket for rate limiting using the token bucket algorithm
#[derive(Debug)]
struct TokenBucket {
    /// Maximum number of tokens (burst capacity)
    capacity: u32,
    /// Current number of tokens
    tokens: f64,
    /// Rate at which tokens are replenished (tokens per second)
    refill_rate: f64,
    /// Last time the bucket was updated
    last_refill: Instant,
}

impl TokenBucket {
    fn new(requests_per_minute: u32, burst_multiplier: f32) -> Self {
        let capacity = ((requests_per_minute as f32) * burst_multiplier) as u32;
        let refill_rate = requests_per_minute as f64 / 60.0; // tokens per second

        Self {
            capacity,
            tokens: capacity as f64, // Start with full bucket
            refill_rate,
            last_refill: Instant::now(),
        }
    }

    /// Check if a request is allowed and consume a token if so
    fn check_and_consume(&mut self, tokens_requested: u32) -> RateLimitResult {
        self.refill();

        if self.tokens >= tokens_requested as f64 {
            self.tokens -= tokens_requested as f64;
            RateLimitResult::Allowed {
                remaining: self.tokens as u32,
            }
        } else {
            // Calculate retry after based on refill rate
            let tokens_needed = tokens_requested as f64 - self.tokens;
            let retry_after_secs = tokens_needed / self.refill_rate;
            let retry_after = Duration::from_secs_f64(retry_after_secs.max(1.0));

            RateLimitResult::Blocked {
                retry_after,
                current_count: (self.capacity as f64 - self.tokens) as u32,
            }
        }
    }

    /// Refill tokens based on elapsed time
    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        
        if elapsed > 0.0 {
            let tokens_to_add = elapsed * self.refill_rate;
            self.tokens = (self.tokens + tokens_to_add).min(self.capacity as f64);
            self.last_refill = now;
        }
    }

    /// Check if bucket is expired (no activity for a long time)
    fn is_expired(&self, ttl: Duration) -> bool {
        self.last_refill.elapsed() > ttl
    }
}

/// Rate limiter managing multiple token buckets per IP and operation
pub struct RateLimiter {
    config: RateLimitConfig,
    /// Buckets organized by (IP, operation) pairs
    buckets: Arc<RwLock<HashMap<(IpAddr, String), TokenBucket>>>,
    /// Violation counter for monitoring
    violation_count: AtomicU64,
    /// Last cleanup time
    last_cleanup: Arc<RwLock<Instant>>,
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            buckets: Arc::new(RwLock::new(HashMap::new())),
            violation_count: AtomicU64::new(0),
            last_cleanup: Arc::new(RwLock::new(Instant::now())),
        }
    }

    /// Check rate limit for a specific IP and operation
    pub fn check_rate_limit(&self, ip: IpAddr, operation: &str) -> RateLimitResult {
        self.cleanup_if_needed();

        let key = (ip, operation.to_string());
        let rpm = self.get_rpm_for_operation(operation);

        let result = {
            let mut buckets = self.buckets.write().unwrap();
            let bucket = buckets.entry(key).or_insert_with(|| {
                TokenBucket::new(rpm, self.config.burst_multiplier)
            });
            
            bucket.check_and_consume(1)
        };

        // Record violations for monitoring
        if result.is_blocked() {
            self.violation_count.fetch_add(1, Ordering::Relaxed);
        }

        result
    }

    /// Check rate limit for multiple tokens at once (for bulk operations)
    pub fn check_rate_limit_bulk(&self, ip: IpAddr, operation: &str, tokens: u32) -> RateLimitResult {
        self.cleanup_if_needed();

        let key = (ip, operation.to_string());
        let rpm = self.get_rpm_for_operation(operation);

        let result = {
            let mut buckets = self.buckets.write().unwrap();
            let bucket = buckets.entry(key).or_insert_with(|| {
                TokenBucket::new(rpm, self.config.burst_multiplier)
            });
            
            bucket.check_and_consume(tokens)
        };

        // Record violations for monitoring
        if result.is_blocked() {
            self.violation_count.fetch_add(1, Ordering::Relaxed);
        }

        result
    }

    /// Get RPM limit for specific operation
    fn get_rpm_for_operation(&self, operation: &str) -> u32 {
        match operation {
            "game_join" => self.config.game_join_rpm,
            "dice_roll" => self.config.dice_roll_rpm,
            "network_message" => self.config.network_message_rpm,
            "bet_placement" => self.config.bet_placement_rpm,
            _ => self.config.default_rpm,
        }
    }

    /// Get total number of rate limit violations
    pub fn get_violation_count(&self) -> u64 {
        self.violation_count.load(Ordering::Relaxed)
    }

    /// Get current number of tracked buckets
    pub fn get_bucket_count(&self) -> usize {
        self.buckets.read().unwrap().len()
    }

    /// Force cleanup of expired buckets
    pub fn cleanup_expired_buckets(&self) {
        let ttl = Duration::from_secs(3600); // 1 hour TTL for buckets
        
        let mut buckets = self.buckets.write().unwrap();
        let initial_count = buckets.len();
        
        buckets.retain(|_, bucket| !bucket.is_expired(ttl));
        
        let removed_count = initial_count - buckets.len();
        if removed_count > 0 {
            log::debug!("Cleaned up {} expired rate limit buckets", removed_count);
        }
        
        *self.last_cleanup.write().unwrap() = Instant::now();
    }

    /// Cleanup buckets if enough time has elapsed
    fn cleanup_if_needed(&self) {
        let should_cleanup = {
            let last_cleanup = self.last_cleanup.read().unwrap();
            last_cleanup.elapsed() > self.config.cleanup_interval
        };

        if should_cleanup {
            self.cleanup_expired_buckets();
        }
    }

    /// Get rate limiting statistics
    pub fn get_stats(&self) -> RateLimitStats {
        let bucket_count = self.get_bucket_count();
        let violation_count = self.get_violation_count();

        RateLimitStats {
            active_buckets: bucket_count,
            total_violations: violation_count,
            config: self.config.clone(),
        }
    }

    /// Reset rate limits for a specific IP (admin function)
    pub fn reset_ip_limits(&self, ip: IpAddr) {
        let mut buckets = self.buckets.write().unwrap();
        buckets.retain(|(bucket_ip, _), _| *bucket_ip != ip);
        log::info!("Reset rate limits for IP: {}", ip);
    }

    /// Block an IP temporarily by consuming all their tokens
    pub fn temporary_block(&self, ip: IpAddr, duration: Duration) {
        let operations = ["game_join", "dice_roll", "network_message", "bet_placement"];
        
        let mut buckets = self.buckets.write().unwrap();
        
        for operation in &operations {
            let key = (ip, operation.to_string());
            if let Some(bucket) = buckets.get_mut(&key) {
                bucket.tokens = 0.0; // Consume all tokens
                // Set last refill to future time to extend the block
                bucket.last_refill = Instant::now() + duration;
            }
        }
        
        log::warn!("Temporarily blocked IP {} for {:?}", ip, duration);
    }
}

/// Rate limiting statistics
#[derive(Debug, Clone)]
pub struct RateLimitStats {
    pub active_buckets: usize,
    pub total_violations: u64,
    pub config: RateLimitConfig,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;
    use std::thread;

    fn create_test_limiter() -> RateLimiter {
        let mut config = RateLimitConfig::default();
        config.game_join_rpm = 60; // 1 per second for testing
        config.cleanup_interval = Duration::from_millis(100);
        RateLimiter::new(config)
    }

    #[test]
    fn test_token_bucket_basic() {
        let mut bucket = TokenBucket::new(60, 1.0); // 1 request per second, no burst
        
        // Should allow initial request
        let result = bucket.check_and_consume(1);
        assert!(result.is_allowed());
        
        // Should block rapid subsequent request
        let result = bucket.check_and_consume(1);
        assert!(result.is_blocked());
    }

    #[test]
    fn test_token_bucket_refill() {
        let mut bucket = TokenBucket::new(60, 1.0); // 1 request per second
        
        // Consume all tokens
        let result = bucket.check_and_consume(1);
        assert!(result.is_allowed());
        
        // Should be blocked immediately
        let result = bucket.check_and_consume(1);
        assert!(result.is_blocked());
        
        // Wait for refill and try again
        thread::sleep(Duration::from_secs(2));
        let result = bucket.check_and_consume(1);
        assert!(result.is_allowed());
    }

    #[test]
    fn test_rate_limiter_per_ip() {
        let limiter = create_test_limiter();
        let ip1 = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));
        let ip2 = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 2));
        
        // Each IP should have independent limits
        let result1 = limiter.check_rate_limit(ip1, "game_join");
        let result2 = limiter.check_rate_limit(ip2, "game_join");
        
        assert!(result1.is_allowed());
        assert!(result2.is_allowed());
        
        assert_eq!(limiter.get_bucket_count(), 2);
    }

    #[test]
    fn test_rate_limiter_per_operation() {
        let limiter = create_test_limiter();
        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));
        
        // Same IP, different operations should have independent limits
        let result1 = limiter.check_rate_limit(ip, "game_join");
        let result2 = limiter.check_rate_limit(ip, "dice_roll");
        
        assert!(result1.is_allowed());
        assert!(result2.is_allowed());
        
        assert_eq!(limiter.get_bucket_count(), 2);
    }

    #[test]
    fn test_bulk_token_consumption() {
        let limiter = create_test_limiter();
        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));
        
        // Request multiple tokens at once
        let result = limiter.check_rate_limit_bulk(ip, "network_message", 5);
        assert!(result.is_allowed());
        
        // Should have fewer tokens remaining
        if let RateLimitResult::Allowed { remaining } = result {
            assert!(remaining < 100); // Assuming burst capacity is around 100
        }
    }

    #[test]
    fn test_violation_counting() {
        let limiter = create_test_limiter();
        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));
        
        let initial_violations = limiter.get_violation_count();
        
        // Consume all tokens to trigger violations
        for _ in 0..100 {
            let result = limiter.check_rate_limit(ip, "game_join");
            if result.is_blocked() {
                break;
            }
        }
        
        let final_violations = limiter.get_violation_count();
        assert!(final_violations > initial_violations);
    }

    #[test]
    fn test_bucket_cleanup() {
        let limiter = create_test_limiter();
        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));
        
        // Create some buckets
        let _ = limiter.check_rate_limit(ip, "game_join");
        let _ = limiter.check_rate_limit(ip, "dice_roll");
        
        assert_eq!(limiter.get_bucket_count(), 2);
        
        // Force cleanup
        limiter.cleanup_expired_buckets();
        
        // Buckets should still exist (not old enough)
        assert_eq!(limiter.get_bucket_count(), 2);
    }

    #[test]
    fn test_ip_reset() {
        let limiter = create_test_limiter();
        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));
        
        // Create buckets for IP
        let _ = limiter.check_rate_limit(ip, "game_join");
        let _ = limiter.check_rate_limit(ip, "dice_roll");
        
        assert_eq!(limiter.get_bucket_count(), 2);
        
        // Reset IP limits
        limiter.reset_ip_limits(ip);
        
        assert_eq!(limiter.get_bucket_count(), 0);
    }

    #[test]
    fn test_temporary_block() {
        let limiter = create_test_limiter();
        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));
        
        // Should work initially
        let result = limiter.check_rate_limit(ip, "game_join");
        assert!(result.is_allowed());
        
        // Block the IP
        limiter.temporary_block(ip, Duration::from_secs(60));
        
        // Should now be blocked
        let result = limiter.check_rate_limit(ip, "game_join");
        assert!(result.is_blocked());
    }

    #[test]
    fn test_rate_limit_stats() {
        let limiter = create_test_limiter();
        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));
        
        // Create some activity
        let _ = limiter.check_rate_limit(ip, "game_join");
        
        let stats = limiter.get_stats();
        assert_eq!(stats.active_buckets, 1);
        assert!(stats.config.game_join_rpm > 0);
    }
}