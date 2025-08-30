//! Security event logging and monitoring for BitCraps
//!
//! This module provides comprehensive security event logging:
//! - Structured security event definitions
//! - Configurable logging levels
//! - Event correlation and analysis
//! - Integration with external monitoring systems

use std::net::IpAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Serialize, Deserialize};

/// Security event severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SecurityLevel {
    /// Informational events (normal operation)
    Info = 0,
    /// Warning events (potentially suspicious)
    Warning = 1,
    /// High priority events (likely attack)
    High = 2,
    /// Critical events (active attack or compromise)
    Critical = 3,
}

impl SecurityLevel {
    /// Convert to string for logging
    pub fn as_str(&self) -> &'static str {
        match self {
            SecurityLevel::Info => "INFO",
            SecurityLevel::Warning => "WARNING",
            SecurityLevel::High => "HIGH",
            SecurityLevel::Critical => "CRITICAL",
        }
    }

    /// Get numeric value for comparisons
    pub fn as_u8(&self) -> u8 {
        *self as u8
    }
}

/// Comprehensive security events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecurityEvent {
    /// Input validation events
    ValidationFailed {
        client_ip: IpAddr,
        operation: String,
        field: String,
        error: String,
    },
    
    /// Authentication events
    AuthenticationFailed {
        client_ip: IpAddr,
        reason: String,
        attempts: u32,
    },
    
    /// Rate limiting events
    RateLimitExceeded {
        client_ip: IpAddr,
        operation: String,
        attempts: u32,
    },
    
    /// DoS protection events
    DosAttempt {
        client_ip: IpAddr,
        operation: String,
        reason: String,
    },
    
    /// Network security events
    SuspiciousNetworkActivity {
        client_ip: IpAddr,
        activity_type: String,
        details: String,
    },
    
    /// Cryptographic events
    CryptographicFailure {
        operation: String,
        error: String,
        context: Option<String>,
    },
    
    /// Game integrity events
    GameIntegrityViolation {
        game_id: [u8; 16],
        player_id: [u8; 32],
        violation_type: String,
        details: String,
    },
    
    /// Consensus security events
    ConsensusViolation {
        peer_id: [u8; 32],
        violation_type: String,
        severity: String,
    },
    
    /// Message size violations
    OversizedMessage {
        client_ip: IpAddr,
        size: usize,
        max_size: usize,
    },
    
    /// Timing attack detection
    TimingAttackDetected {
        client_ip: IpAddr,
        operation: String,
        suspicious_timing: String,
    },
    
    /// Successful validations (for monitoring)
    ValidatedGameJoin {
        game_id: [u8; 16],
        player_id: [u8; 32],
        buy_in: u64,
    },
    
    ValidatedDiceRoll {
        game_id: [u8; 16],
        player_id: [u8; 32],
    },
    
    /// IP blocking events
    IpBlocked {
        ip: IpAddr,
        duration_seconds: u64,
        reason: String,
    },
    
    IpUnblocked {
        ip: IpAddr,
        reason: String,
    },
    
    /// System security events
    SecuritySystemError {
        component: String,
        error: String,
    },
    
    /// Memory exhaustion attacks
    MemoryExhaustionAttempt {
        client_ip: IpAddr,
        operation: String,
        memory_requested: usize,
    },
    
    /// Replay attack detection
    ReplayAttackDetected {
        client_ip: IpAddr,
        message_hash: String,
        original_timestamp: u64,
    },
}

impl SecurityEvent {
    /// Get the client IP associated with this event, if any
    pub fn client_ip(&self) -> Option<IpAddr> {
        match self {
            SecurityEvent::ValidationFailed { client_ip, .. } => Some(*client_ip),
            SecurityEvent::AuthenticationFailed { client_ip, .. } => Some(*client_ip),
            SecurityEvent::RateLimitExceeded { client_ip, .. } => Some(*client_ip),
            SecurityEvent::DosAttempt { client_ip, .. } => Some(*client_ip),
            SecurityEvent::SuspiciousNetworkActivity { client_ip, .. } => Some(*client_ip),
            SecurityEvent::OversizedMessage { client_ip, .. } => Some(*client_ip),
            SecurityEvent::TimingAttackDetected { client_ip, .. } => Some(*client_ip),
            SecurityEvent::IpBlocked { ip, .. } => Some(*ip),
            SecurityEvent::IpUnblocked { ip, .. } => Some(*ip),
            SecurityEvent::MemoryExhaustionAttempt { client_ip, .. } => Some(*client_ip),
            SecurityEvent::ReplayAttackDetected { client_ip, .. } => Some(*client_ip),
            _ => None,
        }
    }

    /// Get a short description of the event
    pub fn event_type(&self) -> &'static str {
        match self {
            SecurityEvent::ValidationFailed { .. } => "validation_failed",
            SecurityEvent::AuthenticationFailed { .. } => "authentication_failed",
            SecurityEvent::RateLimitExceeded { .. } => "rate_limit_exceeded",
            SecurityEvent::DosAttempt { .. } => "dos_attempt",
            SecurityEvent::SuspiciousNetworkActivity { .. } => "suspicious_network_activity",
            SecurityEvent::CryptographicFailure { .. } => "cryptographic_failure",
            SecurityEvent::GameIntegrityViolation { .. } => "game_integrity_violation",
            SecurityEvent::ConsensusViolation { .. } => "consensus_violation",
            SecurityEvent::OversizedMessage { .. } => "oversized_message",
            SecurityEvent::TimingAttackDetected { .. } => "timing_attack_detected",
            SecurityEvent::ValidatedGameJoin { .. } => "validated_game_join",
            SecurityEvent::ValidatedDiceRoll { .. } => "validated_dice_roll",
            SecurityEvent::IpBlocked { .. } => "ip_blocked",
            SecurityEvent::IpUnblocked { .. } => "ip_unblocked",
            SecurityEvent::SecuritySystemError { .. } => "security_system_error",
            SecurityEvent::MemoryExhaustionAttempt { .. } => "memory_exhaustion_attempt",
            SecurityEvent::ReplayAttackDetected { .. } => "replay_attack_detected",
        }
    }

    /// Check if this event indicates potential malicious activity
    pub fn is_malicious(&self) -> bool {
        matches!(
            self,
            SecurityEvent::DosAttempt { .. } |
            SecurityEvent::TimingAttackDetected { .. } |
            SecurityEvent::GameIntegrityViolation { .. } |
            SecurityEvent::MemoryExhaustionAttempt { .. } |
            SecurityEvent::ReplayAttackDetected { .. }
        )
    }
}

/// Security event with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggedSecurityEvent {
    pub timestamp: u64,
    pub event_id: u64,
    pub level: SecurityLevel,
    pub event: SecurityEvent,
    pub correlation_id: Option<String>,
}

/// Security event logger
pub struct SecurityEventLogger {
    /// Whether logging is enabled
    enabled: bool,
    /// Whether to log sensitive data
    log_sensitive: bool,
    /// Event counter for unique IDs
    event_counter: AtomicU64,
    /// Various event type counters
    info_events: AtomicU64,
    warning_events: AtomicU64,
    high_events: AtomicU64,
    critical_events: AtomicU64,
}

impl SecurityEventLogger {
    pub fn new(enabled: bool, log_sensitive: bool) -> Self {
        Self {
            enabled,
            log_sensitive,
            event_counter: AtomicU64::new(0),
            info_events: AtomicU64::new(0),
            warning_events: AtomicU64::new(0),
            high_events: AtomicU64::new(0),
            critical_events: AtomicU64::new(0),
        }
    }

    /// Log a security event
    pub fn log_security_event(&self, event: SecurityEvent, level: SecurityLevel) {
        if !self.enabled {
            return;
        }

        let event_id = self.event_counter.fetch_add(1, Ordering::Relaxed);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Update counters
        match level {
            SecurityLevel::Info => self.info_events.fetch_add(1, Ordering::Relaxed),
            SecurityLevel::Warning => self.warning_events.fetch_add(1, Ordering::Relaxed),
            SecurityLevel::High => self.high_events.fetch_add(1, Ordering::Relaxed),
            SecurityLevel::Critical => self.critical_events.fetch_add(1, Ordering::Relaxed),
        };

        let logged_event = LoggedSecurityEvent {
            timestamp,
            event_id,
            level,
            event: self.sanitize_event_if_needed(event),
            correlation_id: None, // Could be implemented for event correlation
        };

        // Log the event
        self.write_event_log(&logged_event);

        // For high-severity events, also use error logging
        if level >= SecurityLevel::High {
            self.write_high_severity_log(&logged_event);
        }
    }

    /// Log multiple related events with correlation ID
    pub fn log_correlated_events(
        &self, 
        events: Vec<(SecurityEvent, SecurityLevel)>, 
        correlation_id: String
    ) {
        for (event, level) in events {
            let event_id = self.event_counter.fetch_add(1, Ordering::Relaxed);
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            let logged_event = LoggedSecurityEvent {
                timestamp,
                event_id,
                level,
                event: self.sanitize_event_if_needed(event),
                correlation_id: Some(correlation_id.clone()),
            };

            self.write_event_log(&logged_event);
        }
    }

    /// Get total count of events by level
    pub fn get_event_count(&self) -> u64 {
        self.info_events.load(Ordering::Relaxed) +
        self.warning_events.load(Ordering::Relaxed) +
        self.high_events.load(Ordering::Relaxed) +
        self.critical_events.load(Ordering::Relaxed)
    }

    /// Get event statistics
    pub fn get_event_stats(&self) -> SecurityEventStats {
        SecurityEventStats {
            total_events: self.get_event_count(),
            info_events: self.info_events.load(Ordering::Relaxed),
            warning_events: self.warning_events.load(Ordering::Relaxed),
            high_events: self.high_events.load(Ordering::Relaxed),
            critical_events: self.critical_events.load(Ordering::Relaxed),
        }
    }

    /// Enable or disable logging
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Enable or disable sensitive data logging
    pub fn set_log_sensitive(&mut self, log_sensitive: bool) {
        self.log_sensitive = log_sensitive;
    }

    /// Sanitize event to remove sensitive data if needed
    fn sanitize_event_if_needed(&self, mut event: SecurityEvent) -> SecurityEvent {
        if self.log_sensitive {
            return event;
        }

        // Remove or redact sensitive information
        match &mut event {
            SecurityEvent::ValidatedGameJoin { player_id, .. } => {
                *player_id = [0u8; 32]; // Zero out player ID
            },
            SecurityEvent::ValidatedDiceRoll { player_id, .. } => {
                *player_id = [0u8; 32]; // Zero out player ID
            },
            SecurityEvent::GameIntegrityViolation { player_id, .. } => {
                *player_id = [0u8; 32]; // Zero out player ID
            },
            SecurityEvent::ConsensusViolation { peer_id, .. } => {
                *peer_id = [0u8; 32]; // Zero out peer ID
            },
            _ => {}, // Other events don't contain sensitive data
        }

        event
    }

    /// Write event to log
    fn write_event_log(&self, event: &LoggedSecurityEvent) {
        // Use structured logging
        log::info!(
            target: "bitcraps_security",
            "Security event: {} [{}] ID={} IP={:?} at {}",
            event.event.event_type(),
            event.level.as_str(),
            event.event_id,
            event.event.client_ip(),
            event.timestamp
        );

        // For debugging, also log the full event details
        log::debug!(
            target: "bitcraps_security_details",
            "Security event details: {:?}",
            event
        );
    }

    /// Write high-severity events to separate log
    fn write_high_severity_log(&self, event: &LoggedSecurityEvent) {
        log::error!(
            target: "bitcraps_security_alerts",
            "HIGH SEVERITY SECURITY EVENT: {} [{}] ID={} - {:?}",
            event.event.event_type(),
            event.level.as_str(),
            event.event_id,
            event.event
        );
    }

    /// Create alert for critical events (could integrate with external systems)
    pub fn create_alert(&self, event: &SecurityEvent, level: SecurityLevel) {
        if level == SecurityLevel::Critical {
            log::error!(
                target: "bitcraps_security_critical",
                "CRITICAL SECURITY ALERT: {} - This requires immediate attention!",
                event.event_type()
            );
            
            // Here you could integrate with:
            // - Email alerts
            // - Slack/Discord notifications
            // - External monitoring systems (Datadog, New Relic, etc.)
            // - SMS alerts for critical incidents
        }
    }
}

/// Security event statistics
#[derive(Debug, Clone)]
pub struct SecurityEventStats {
    pub total_events: u64,
    pub info_events: u64,
    pub warning_events: u64,
    pub high_events: u64,
    pub critical_events: u64,
}

impl SecurityEventStats {
    /// Calculate percentage of high-severity events
    pub fn high_severity_percentage(&self) -> f64 {
        if self.total_events == 0 {
            return 0.0;
        }
        ((self.high_events + self.critical_events) as f64 / self.total_events as f64) * 100.0
    }

    /// Check if event patterns indicate an ongoing attack
    pub fn indicates_attack(&self) -> bool {
        // Simple heuristic: if more than 10% of events are high severity, possible attack
        self.high_severity_percentage() > 10.0 && self.total_events > 50
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn test_security_levels() {
        assert!(SecurityLevel::Critical > SecurityLevel::High);
        assert!(SecurityLevel::High > SecurityLevel::Warning);
        assert!(SecurityLevel::Warning > SecurityLevel::Info);
        
        assert_eq!(SecurityLevel::Info.as_str(), "INFO");
        assert_eq!(SecurityLevel::Critical.as_str(), "CRITICAL");
        assert_eq!(SecurityLevel::High.as_u8(), 2);
    }

    #[test]
    fn test_security_event_client_ip() {
        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));
        
        let event = SecurityEvent::ValidationFailed {
            client_ip: ip,
            operation: "test".to_string(),
            field: "amount".to_string(),
            error: "too large".to_string(),
        };
        
        assert_eq!(event.client_ip(), Some(ip));
        assert_eq!(event.event_type(), "validation_failed");
        assert!(!event.is_malicious());
        
        let malicious_event = SecurityEvent::DosAttempt {
            client_ip: ip,
            operation: "flood".to_string(),
            reason: "too many requests".to_string(),
        };
        
        assert!(malicious_event.is_malicious());
    }

    #[test]
    fn test_event_logger() {
        let mut logger = SecurityEventLogger::new(true, false);
        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));
        
        let event = SecurityEvent::ValidationFailed {
            client_ip: ip,
            operation: "test".to_string(),
            field: "amount".to_string(),
            error: "too large".to_string(),
        };
        
        logger.log_security_event(event, SecurityLevel::Warning);
        
        let stats = logger.get_event_stats();
        assert_eq!(stats.total_events, 1);
        assert_eq!(stats.warning_events, 1);
        assert_eq!(stats.info_events, 0);
    }

    #[test]
    fn test_sensitive_data_sanitization() {
        let mut logger = SecurityEventLogger::new(true, false); // Don't log sensitive data
        
        let event = SecurityEvent::ValidatedGameJoin {
            game_id: [1u8; 16],
            player_id: [2u8; 32],
            buy_in: 1000,
        };
        
        let sanitized = logger.sanitize_event_if_needed(event);
        
        if let SecurityEvent::ValidatedGameJoin { player_id, .. } = sanitized {
            assert_eq!(player_id, [0u8; 32]); // Should be zeroed out
        } else {
            panic!("Event type should not change");
        }
        
        // Test with sensitive logging enabled
        logger.set_log_sensitive(true);
        
        let event = SecurityEvent::ValidatedGameJoin {
            game_id: [1u8; 16],
            player_id: [2u8; 32],
            buy_in: 1000,
        };
        
        let not_sanitized = logger.sanitize_event_if_needed(event);
        
        if let SecurityEvent::ValidatedGameJoin { player_id, .. } = not_sanitized {
            assert_eq!(player_id, [2u8; 32]); // Should be preserved
        } else {
            panic!("Event type should not change");
        }
    }

    #[test]
    fn test_correlated_events() {
        let logger = SecurityEventLogger::new(true, true);
        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));
        
        let events = vec![
            (SecurityEvent::ValidationFailed {
                client_ip: ip,
                operation: "test1".to_string(),
                field: "amount".to_string(),
                error: "too large".to_string(),
            }, SecurityLevel::Warning),
            (SecurityEvent::RateLimitExceeded {
                client_ip: ip,
                operation: "test1".to_string(),
                attempts: 10,
            }, SecurityLevel::High),
        ];
        
        logger.log_correlated_events(events, "correlation-123".to_string());
        
        let stats = logger.get_event_stats();
        assert_eq!(stats.total_events, 2);
        assert_eq!(stats.warning_events, 1);
        assert_eq!(stats.high_events, 1);
    }

    #[test]
    fn test_event_stats_analysis() {
        let mut stats = SecurityEventStats {
            total_events: 100,
            info_events: 70,
            warning_events: 20,
            high_events: 8,
            critical_events: 2,
        };
        
        assert_eq!(stats.high_severity_percentage(), 10.0);
        assert!(stats.indicates_attack());
        
        stats.high_events = 2;
        stats.critical_events = 0;
        assert!(!stats.indicates_attack());
    }

    #[test]
    fn test_logger_enable_disable() {
        let mut logger = SecurityEventLogger::new(false, false);
        
        let event = SecurityEvent::ValidationFailed {
            client_ip: IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)),
            operation: "test".to_string(),
            field: "amount".to_string(),
            error: "too large".to_string(),
        };
        
        // Should not log when disabled
        logger.log_security_event(event.clone(), SecurityLevel::Warning);
        assert_eq!(logger.get_event_count(), 0);
        
        // Enable and log
        logger.set_enabled(true);
        logger.log_security_event(event, SecurityLevel::Warning);
        assert_eq!(logger.get_event_count(), 1);
    }
}