//! DoS protection mechanisms for BitCraps
//!
//! This module implements comprehensive DoS (Denial of Service) protection:
//! - Request size limiting
//! - Connection throttling
//! - Memory usage monitoring
//! - Bandwidth limiting
//! - Automatic IP blocking

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// DoS protection configuration
#[derive(Debug, Clone)]
pub struct DosProtectionConfig {
    /// Maximum request size in bytes
    pub max_request_size: usize,
    /// Maximum requests per IP per minute
    pub max_requests_per_minute: u32,
    /// Maximum bandwidth per IP per minute (bytes)
    pub max_bandwidth_per_minute: usize,
    /// Maximum concurrent connections per IP
    pub max_connections_per_ip: usize,
    /// Block duration for violating IPs
    pub block_duration: Duration,
    /// Maximum memory usage for tracking (bytes)
    pub max_memory_usage: usize,
    /// Cleanup interval for expired entries
    pub cleanup_interval: Duration,
    /// Suspicious request threshold (triggers monitoring)
    pub suspicious_threshold: u32,
}

impl Default for DosProtectionConfig {
    fn default() -> Self {
        // NOTE: [Security] Current thresholds are conservative defaults
        //       Production deployments should tune these values based on:
        //       - Load testing results and expected traffic patterns
        //       - Network conditions and infrastructure capacity
        //       - IP reputation data and geographic distribution
        //       Current values are production-safe but may need optimization
        Self {
            max_request_size: 64 * 1024,                // 64KB max request
            max_requests_per_minute: 1000,              // 1000 requests per minute
            max_bandwidth_per_minute: 10 * 1024 * 1024, // 10MB per minute
            max_connections_per_ip: 20,                 // 20 concurrent connections
            block_duration: Duration::from_secs(3600),  // 1 hour block
            max_memory_usage: 100 * 1024 * 1024,        // 100MB for tracking
            cleanup_interval: Duration::from_secs(60),  // 1 minute cleanup
            suspicious_threshold: 100,                  // 100 requests triggers monitoring
        }
    }
}

/// Result of DoS protection check
#[derive(Debug, Clone)]
pub enum ProtectionResult {
    Allowed,
    Blocked {
        reason: String,
        retry_after: Duration,
    },
    Suspicious {
        reason: String,
        monitoring_level: u8,
    },
}

impl ProtectionResult {
    pub fn is_allowed(&self) -> bool {
        matches!(self, ProtectionResult::Allowed)
    }

    pub fn is_blocked(&self) -> bool {
        matches!(self, ProtectionResult::Blocked { .. })
    }

    pub fn is_suspicious(&self) -> bool {
        matches!(self, ProtectionResult::Suspicious { .. })
    }

    pub fn get_reason(&self) -> String {
        match self {
            ProtectionResult::Allowed => "Allowed".to_string(),
            ProtectionResult::Blocked { reason, .. } => reason.clone(),
            ProtectionResult::Suspicious { reason, .. } => reason.clone(),
        }
    }

    pub fn retry_after(&self) -> Option<Duration> {
        match self {
            ProtectionResult::Blocked { retry_after, .. } => Some(*retry_after),
            _ => None,
        }
    }
}

/// Per-IP tracking information
#[derive(Debug)]
struct IpTracker {
    /// Request count in current minute
    request_count: u32,
    /// Bandwidth used in current minute
    bandwidth_used: usize,
    /// Current connections
    active_connections: usize,
    /// Last request time
    last_request: Instant,
    /// Window start time for rate limiting
    window_start: Instant,
    /// Total suspicious activities
    suspicious_count: u32,
    /// Block end time (if blocked)
    blocked_until: Option<Instant>,
    /// Block reason
    block_reason: Option<String>,
}

impl IpTracker {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            request_count: 0,
            bandwidth_used: 0,
            active_connections: 0,
            last_request: now,
            window_start: now,
            suspicious_count: 0,
            blocked_until: None,
            block_reason: None,
        }
    }

    /// Reset counters if window has expired
    fn reset_if_window_expired(&mut self) {
        let now = Instant::now();
        if now.duration_since(self.window_start) >= Duration::from_secs(60) {
            self.request_count = 0;
            self.bandwidth_used = 0;
            self.window_start = now;
        }
    }

    /// Check if IP is currently blocked
    fn is_blocked(&self) -> bool {
        if let Some(blocked_until) = self.blocked_until {
            Instant::now() < blocked_until
        } else {
            false
        }
    }

    /// Block the IP for specified duration
    fn block(&mut self, duration: Duration, reason: String) {
        self.blocked_until = Some(Instant::now() + duration);
        self.block_reason = Some(reason);
    }

    /// Check if tracking data is expired and can be cleaned up
    fn is_expired(&self, ttl: Duration) -> bool {
        self.last_request.elapsed() > ttl && !self.is_blocked()
    }
}

/// DoS protection system
pub struct DosProtection {
    config: DosProtectionConfig,
    /// IP tracking data
    ip_trackers: Arc<RwLock<HashMap<IpAddr, IpTracker>>>,
    /// Global statistics
    total_requests: AtomicU64,
    blocked_requests: AtomicU64,
    suspicious_requests: AtomicU64,
    memory_usage: AtomicUsize,
    /// Last cleanup time
    last_cleanup: Arc<RwLock<Instant>>,
}

impl DosProtection {
    pub fn new(config: DosProtectionConfig) -> Self {
        Self {
            config,
            ip_trackers: Arc::new(RwLock::new(HashMap::new())),
            total_requests: AtomicU64::new(0),
            blocked_requests: AtomicU64::new(0),
            suspicious_requests: AtomicU64::new(0),
            memory_usage: AtomicUsize::new(0),
            last_cleanup: Arc::new(RwLock::new(Instant::now())),
        }
    }

    /// Check if a request should be allowed
    pub fn check_request(&self, ip: IpAddr, request_size: usize) -> ProtectionResult {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.cleanup_if_needed();

        // Check request size limit
        if request_size > self.config.max_request_size {
            self.blocked_requests.fetch_add(1, Ordering::Relaxed);
            return ProtectionResult::Blocked {
                reason: format!(
                    "Request size {} exceeds limit {}",
                    request_size, self.config.max_request_size
                ),
                retry_after: Duration::from_secs(60),
            };
        }

        let result = {
            let mut trackers = match self.ip_trackers.write() {
                Ok(lock) => lock,
                Err(poisoned) => {
                    log::error!("DoS protection lock poisoned, recovering: {}", poisoned);
                    poisoned.into_inner()
                }
            };
            let tracker = trackers.entry(ip).or_insert_with(IpTracker::new);

            tracker.last_request = Instant::now();
            tracker.reset_if_window_expired();

            // Check if IP is blocked
            if tracker.is_blocked() {
                let reason = tracker
                    .block_reason
                    .clone()
                    .unwrap_or_else(|| "IP blocked".to_string());
                let retry_after = tracker
                    .blocked_until
                    .map(|until| until.duration_since(Instant::now()))
                    .unwrap_or(Duration::from_secs(3600));

                return ProtectionResult::Blocked {
                    reason,
                    retry_after,
                };
            }

            // Check request rate limit
            if tracker.request_count >= self.config.max_requests_per_minute {
                tracker.block(self.config.block_duration, "Too many requests".to_string());
                self.blocked_requests.fetch_add(1, Ordering::Relaxed);
                return ProtectionResult::Blocked {
                    reason: "Request rate limit exceeded".to_string(),
                    retry_after: self.config.block_duration,
                };
            }

            // Check bandwidth limit
            if tracker.bandwidth_used + request_size > self.config.max_bandwidth_per_minute {
                tracker.block(
                    self.config.block_duration,
                    "Bandwidth limit exceeded".to_string(),
                );
                self.blocked_requests.fetch_add(1, Ordering::Relaxed);
                return ProtectionResult::Blocked {
                    reason: "Bandwidth limit exceeded".to_string(),
                    retry_after: self.config.block_duration,
                };
            }

            // Update counters
            tracker.request_count += 1;
            tracker.bandwidth_used += request_size;

            // Check for suspicious activity
            if tracker.request_count > self.config.suspicious_threshold {
                tracker.suspicious_count += 1;
                self.suspicious_requests.fetch_add(1, Ordering::Relaxed);
                ProtectionResult::Suspicious {
                    reason: "High request rate detected".to_string(),
                    monitoring_level: (tracker.suspicious_count / 10).min(5) as u8,
                }
            } else {
                ProtectionResult::Allowed
            }
        };

        // Update memory usage estimate
        self.update_memory_usage();

        result
    }

    /// Register a new connection from IP
    pub fn register_connection(&self, ip: IpAddr) -> ProtectionResult {
        let mut trackers = match self.ip_trackers.write() {
            Ok(lock) => lock,
            Err(poisoned) => {
                log::error!(
                    "DoS protection lock poisoned during connection registration, recovering"
                );
                poisoned.into_inner()
            }
        };
        let tracker = trackers.entry(ip).or_insert_with(IpTracker::new);

        // Check if IP is blocked
        if tracker.is_blocked() {
            let reason = tracker
                .block_reason
                .clone()
                .unwrap_or_else(|| "IP blocked".to_string());
            let retry_after = tracker
                .blocked_until
                .map(|until| until.duration_since(Instant::now()))
                .unwrap_or(Duration::from_secs(3600));

            return ProtectionResult::Blocked {
                reason,
                retry_after,
            };
        }

        // Check connection limit
        if tracker.active_connections >= self.config.max_connections_per_ip {
            tracker.block(
                self.config.block_duration,
                "Too many connections".to_string(),
            );
            self.blocked_requests.fetch_add(1, Ordering::Relaxed);
            return ProtectionResult::Blocked {
                reason: "Connection limit exceeded".to_string(),
                retry_after: self.config.block_duration,
            };
        }

        tracker.active_connections += 1;
        ProtectionResult::Allowed
    }

    /// Unregister a connection from IP
    pub fn unregister_connection(&self, ip: IpAddr) {
        let mut trackers = match self.ip_trackers.write() {
            Ok(lock) => lock,
            Err(poisoned) => {
                log::error!(
                    "DoS protection lock poisoned during connection unregistration, recovering"
                );
                poisoned.into_inner()
            }
        };
        if let Some(tracker) = trackers.get_mut(&ip) {
            tracker.active_connections = tracker.active_connections.saturating_sub(1);
        }
    }

    /// Manually block an IP
    pub fn block_ip(&self, ip: IpAddr, duration: Duration, reason: String) {
        let mut trackers = match self.ip_trackers.write() {
            Ok(lock) => lock,
            Err(poisoned) => {
                log::error!("DoS protection lock poisoned during IP blocking, recovering");
                poisoned.into_inner()
            }
        };
        let tracker = trackers.entry(ip).or_insert_with(IpTracker::new);
        tracker.block(duration, reason);
        log::warn!("Manually blocked IP: {} for {:?}", ip, duration);
    }

    /// Unblock an IP
    pub fn unblock_ip(&self, ip: IpAddr) {
        let mut trackers = match self.ip_trackers.write() {
            Ok(lock) => lock,
            Err(poisoned) => {
                log::error!("DoS protection lock poisoned during IP unblocking, recovering");
                poisoned.into_inner()
            }
        };
        if let Some(tracker) = trackers.get_mut(&ip) {
            tracker.blocked_until = None;
            tracker.block_reason = None;
            log::info!("Unblocked IP: {}", ip);
        }
    }

    /// Get list of currently blocked IPs
    pub fn get_blocked_ips(&self) -> Vec<(IpAddr, Duration, String)> {
        let trackers = match self.ip_trackers.read() {
            Ok(lock) => lock,
            Err(poisoned) => {
                log::error!("DoS protection lock poisoned when getting blocked IPs, recovering");
                poisoned.into_inner()
            }
        };
        let now = Instant::now();

        trackers
            .iter()
            .filter_map(|(&ip, tracker)| {
                if let Some(blocked_until) = tracker.blocked_until {
                    if now < blocked_until {
                        let remaining = blocked_until.duration_since(now);
                        let reason = tracker
                            .block_reason
                            .clone()
                            .unwrap_or_else(|| "Unknown".to_string());
                        Some((ip, remaining, reason))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get DoS protection statistics
    pub fn get_stats(&self) -> DosProtectionStats {
        let trackers = match self.ip_trackers.read() {
            Ok(lock) => lock,
            Err(poisoned) => {
                log::error!("DoS protection lock poisoned when getting stats, recovering");
                poisoned.into_inner()
            }
        };
        let blocked_ips = trackers
            .values()
            .filter(|tracker| tracker.is_blocked())
            .count();
        let suspicious_ips = trackers
            .values()
            .filter(|tracker| tracker.suspicious_count > 0)
            .count();

        DosProtectionStats {
            total_requests: self.total_requests.load(Ordering::Relaxed),
            blocked_requests: self.blocked_requests.load(Ordering::Relaxed),
            suspicious_requests: self.suspicious_requests.load(Ordering::Relaxed),
            tracked_ips: trackers.len(),
            blocked_ips,
            suspicious_ips,
            memory_usage: self.memory_usage.load(Ordering::Relaxed),
        }
    }

    /// Get total number of blocked attempts
    pub fn get_blocked_count(&self) -> u64 {
        self.blocked_requests.load(Ordering::Relaxed)
    }

    /// Update memory usage estimate
    fn update_memory_usage(&self) {
        let trackers = match self.ip_trackers.read() {
            Ok(lock) => lock,
            Err(poisoned) => {
                log::error!("DoS protection lock poisoned when updating memory usage, recovering");
                poisoned.into_inner()
            }
        };
        let estimated_usage = trackers.len() * std::mem::size_of::<IpTracker>()
            + trackers.len() * std::mem::size_of::<IpAddr>();
        self.memory_usage.store(estimated_usage, Ordering::Relaxed);

        // Check memory limit
        if estimated_usage > self.config.max_memory_usage {
            log::warn!(
                "DoS protection memory usage {} exceeds limit {}",
                estimated_usage,
                self.config.max_memory_usage
            );
        }
    }

    /// Cleanup expired tracking entries
    pub fn cleanup_expired_entries(&self) {
        let ttl = Duration::from_secs(3600); // 1 hour TTL

        let mut trackers = match self.ip_trackers.write() {
            Ok(lock) => lock,
            Err(poisoned) => {
                log::error!("DoS protection lock poisoned during cleanup, recovering");
                poisoned.into_inner()
            }
        };
        let initial_count = trackers.len();

        trackers.retain(|_, tracker| !tracker.is_expired(ttl));

        let removed_count = initial_count - trackers.len();
        if removed_count > 0 {
            log::debug!(
                "Cleaned up {} expired DoS protection entries",
                removed_count
            );
        }

        match self.last_cleanup.write() {
            Ok(mut lock) => *lock = Instant::now(),
            Err(poisoned) => {
                log::error!("Last cleanup lock poisoned, recovering");
                *poisoned.into_inner() = Instant::now();
            }
        }
        self.update_memory_usage();
    }

    /// Cleanup if needed
    fn cleanup_if_needed(&self) {
        let should_cleanup = {
            let last_cleanup = match self.last_cleanup.read() {
                Ok(lock) => lock,
                Err(poisoned) => {
                    log::error!("Last cleanup lock poisoned when checking interval, recovering");
                    poisoned.into_inner()
                }
            };
            last_cleanup.elapsed() > self.config.cleanup_interval
        };

        if should_cleanup {
            self.cleanup_expired_entries();
        }
    }

    /// Emergency cleanup when memory usage is too high
    pub fn emergency_cleanup(&self) {
        log::warn!("Performing emergency DoS protection cleanup");

        let target_size = {
            let trackers = match self.ip_trackers.read() {
                Ok(lock) => lock,
                Err(poisoned) => {
                    log::error!(
                        "DoS protection lock poisoned when calculating target size, recovering"
                    );
                    poisoned.into_inner()
                }
            };
            trackers.len() / 2 // Remove half the entries
        };

        // Keep only the most recently active and blocked entries
        let entries_to_keep: Vec<(IpAddr, IpTracker)> = {
            let trackers = match self.ip_trackers.read() {
                Ok(lock) => lock,
                Err(poisoned) => {
                    log::error!("DoS protection lock poisoned when evicting entries, recovering");
                    poisoned.into_inner()
                }
            };
            let mut entries: Vec<_> = trackers.iter().collect();
            entries.sort_by_key(|(_, tracker)| {
                if tracker.is_blocked() {
                    // Blocked IPs have highest priority
                    (0, std::cmp::Reverse(tracker.last_request))
                } else {
                    // Then by recency
                    (1, std::cmp::Reverse(tracker.last_request))
                }
            });

            entries
                .into_iter()
                .take(target_size)
                .map(|(ip, tracker)| {
                    // Clone the tracker data
                    let mut new_tracker = IpTracker::new();
                    new_tracker.request_count = tracker.request_count;
                    new_tracker.bandwidth_used = tracker.bandwidth_used;
                    new_tracker.active_connections = tracker.active_connections;
                    new_tracker.last_request = tracker.last_request;
                    new_tracker.window_start = tracker.window_start;
                    new_tracker.suspicious_count = tracker.suspicious_count;
                    new_tracker.blocked_until = tracker.blocked_until;
                    new_tracker.block_reason = tracker.block_reason.clone();

                    (*ip, new_tracker)
                })
                .collect()
        };

        // Replace the trackers map
        {
            let mut trackers = match self.ip_trackers.write() {
                Ok(lock) => lock,
                Err(poisoned) => {
                    log::error!("DoS protection lock poisoned when inserting entries, recovering");
                    poisoned.into_inner()
                }
            };
            trackers.clear();
            for (ip, tracker) in entries_to_keep {
                trackers.insert(ip, tracker);
            }
        }

        self.update_memory_usage();
    }
}

/// DoS protection statistics
#[derive(Debug, Clone)]
pub struct DosProtectionStats {
    pub total_requests: u64,
    pub blocked_requests: u64,
    pub suspicious_requests: u64,
    pub tracked_ips: usize,
    pub blocked_ips: usize,
    pub suspicious_ips: usize,
    pub memory_usage: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    fn create_test_config() -> DosProtectionConfig {
        DosProtectionConfig {
            max_request_size: 1024,
            max_requests_per_minute: 10,
            max_bandwidth_per_minute: 10240,
            max_connections_per_ip: 5,
            block_duration: Duration::from_secs(60),
            max_memory_usage: 1024 * 1024,
            cleanup_interval: Duration::from_millis(100),
            suspicious_threshold: 5,
        }
    }

    #[test]
    fn test_request_size_limit() {
        let config = create_test_config();
        let dos_protection = DosProtection::new(config);
        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));

        // Request within limit should be allowed
        let result = dos_protection.check_request(ip, 512);
        assert!(result.is_allowed());

        // Request exceeding limit should be blocked
        let result = dos_protection.check_request(ip, 2048);
        assert!(result.is_blocked());
    }

    #[test]
    fn test_rate_limiting() {
        let config = create_test_config();
        let dos_protection = DosProtection::new(config);
        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));

        // First few requests should be allowed
        for i in 0..5 {
            let result = dos_protection.check_request(ip, 100);
            if i < 5 {
                assert!(result.is_allowed() || result.is_suspicious());
            }
        }

        // Requests exceeding rate limit should be blocked
        for _ in 0..10 {
            dos_protection.check_request(ip, 100);
        }

        let result = dos_protection.check_request(ip, 100);
        assert!(result.is_blocked());
    }

    #[test]
    fn test_bandwidth_limiting() {
        let config = create_test_config();
        let dos_protection = DosProtection::new(config);
        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));

        // Consume most of the bandwidth
        let result = dos_protection.check_request(ip, 9000);
        assert!(result.is_allowed());

        // Next request should exceed bandwidth limit
        let result = dos_protection.check_request(ip, 2000);
        assert!(result.is_blocked());
    }

    #[test]
    fn test_connection_limiting() {
        let config = create_test_config();
        let dos_protection = DosProtection::new(config);
        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));

        // Register up to the limit
        for _ in 0..5 {
            let result = dos_protection.register_connection(ip);
            assert!(result.is_allowed());
        }

        // Next connection should be blocked
        let result = dos_protection.register_connection(ip);
        assert!(result.is_blocked());

        // Unregister a connection
        dos_protection.unregister_connection(ip);

        // Should be able to connect again
        let result = dos_protection.register_connection(ip);
        assert!(result.is_allowed());
    }

    #[test]
    fn test_manual_blocking() {
        let config = create_test_config();
        let dos_protection = DosProtection::new(config);
        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));

        // Should be allowed initially
        let result = dos_protection.check_request(ip, 100);
        assert!(result.is_allowed());

        // Block manually
        dos_protection.block_ip(ip, Duration::from_secs(60), "Manual block".to_string());

        // Should now be blocked
        let result = dos_protection.check_request(ip, 100);
        assert!(result.is_blocked());

        // Unblock
        dos_protection.unblock_ip(ip);

        // Should be allowed again
        let result = dos_protection.check_request(ip, 100);
        assert!(result.is_allowed());
    }

    #[test]
    fn test_suspicious_detection() {
        let config = create_test_config();
        let dos_protection = DosProtection::new(config);
        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));

        // Make requests up to suspicious threshold
        for _ in 0..6 {
            let result = dos_protection.check_request(ip, 100);
            if result.is_suspicious() {
                break;
            }
        }

        let stats = dos_protection.get_stats();
        assert!(stats.suspicious_requests > 0);
    }

    #[test]
    fn test_blocked_ips_list() {
        let config = create_test_config();
        let dos_protection = DosProtection::new(config);
        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));

        // Block IP
        dos_protection.block_ip(ip, Duration::from_secs(60), "Test block".to_string());

        let blocked_ips = dos_protection.get_blocked_ips();
        assert_eq!(blocked_ips.len(), 1);
        assert_eq!(blocked_ips[0].0, ip);
        assert_eq!(blocked_ips[0].2, "Test block");
    }

    #[test]
    fn test_statistics() {
        let config = create_test_config();
        let dos_protection = DosProtection::new(config);
        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));

        // Make some requests
        for i in 0..15 {
            dos_protection.check_request(ip, 100);
            if i > 10 {
                // These should be blocked
                break;
            }
        }

        let stats = dos_protection.get_stats();
        assert!(stats.total_requests > 0);
        assert!(stats.blocked_requests > 0);
        assert_eq!(stats.tracked_ips, 1);
    }

    #[test]
    fn test_cleanup() {
        let config = create_test_config();
        let dos_protection = DosProtection::new(config);
        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));

        // Create some tracked data
        dos_protection.check_request(ip, 100);

        let stats_before = dos_protection.get_stats();
        assert_eq!(stats_before.tracked_ips, 1);

        // Force cleanup (though entries won't be expired yet)
        dos_protection.cleanup_expired_entries();

        let stats_after = dos_protection.get_stats();
        // Entry should still exist since it's not expired
        assert_eq!(stats_after.tracked_ips, 1);
    }
}
