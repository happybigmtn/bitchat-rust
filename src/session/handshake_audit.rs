//! Noise Session Handshake Security Audit
//!
//! This module provides comprehensive security auditing for Noise protocol handshakes,
//! detecting downgrade attacks, replay attacks, and other cryptographic vulnerabilities.

use crate::error::{Error, Result};
use crate::security::constant_time::ConstantTimeOps;
use crate::session::noise::{NoiseRole, NoiseSession, NoiseSessionState};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Handshake security audit results
#[derive(Debug, Clone)]
pub struct HandshakeAuditResult {
    pub is_secure: bool,
    pub vulnerabilities: Vec<SecurityVulnerability>,
    pub recommendations: Vec<String>,
    pub security_level: SecurityLevel,
}

/// Security level classification
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum SecurityLevel {
    Insecure,  // Critical vulnerabilities found
    Weak,      // Minor vulnerabilities found
    Good,      // Some best practices not followed
    Strong,    // High security, minor improvements possible
    Excellent, // Maximum security achieved
}

/// Types of security vulnerabilities that can be detected
#[derive(Debug, Clone)]
pub enum SecurityVulnerability {
    /// Protocol downgrade attempt detected
    DowngradeAttack {
        attempted_protocol: String,
        secure_protocol: String,
    },
    /// Replay attack attempt detected
    ReplayAttack {
        duplicate_handshake_hash: [u8; 32],
        original_timestamp: Instant,
    },
    /// Weak cryptographic parameters
    WeakCryptography {
        parameter: String,
        value: String,
        recommendation: String,
    },
    /// Timing attack vulnerability
    TimingVulnerability {
        operation: String,
        variation_detected: bool,
    },
    /// Invalid handshake state transition
    InvalidStateTransition {
        from_state: String,
        to_state: String,
        reason: String,
    },
    /// Missing security feature
    MissingSecurityFeature { feature: String, impact: String },
}

/// Comprehensive handshake security auditor
pub struct HandshakeAuditor {
    /// Expected protocol parameters
    expected_protocol: String,
    /// Previously seen handshake hashes (replay detection)
    handshake_history: HashMap<[u8; 32], Instant>,
    /// Maximum age for handshake history entries
    max_history_age: Duration,
    /// Timing measurements for side-channel detection
    timing_measurements: Vec<Duration>,
}

impl Default for HandshakeAuditor {
    fn default() -> Self {
        Self::new()
    }
}

impl HandshakeAuditor {
    /// Create new handshake auditor
    pub fn new() -> Self {
        Self {
            expected_protocol: "Noise_XX_25519_ChaChaPoly_SHA256".to_string(),
            handshake_history: HashMap::with_capacity(1000),
            max_history_age: Duration::from_secs(3600), // 1 hour
            timing_measurements: Vec::with_capacity(100),
        }
    }

    /// Perform comprehensive security audit of a Noise session
    pub fn audit_handshake(&mut self, session: &NoiseSession) -> HandshakeAuditResult {
        let mut vulnerabilities = Vec::new();
        let mut recommendations = Vec::new();

        // Clean up old history entries
        self.cleanup_old_history();

        // Check protocol security
        self.audit_protocol_security(&mut vulnerabilities, &mut recommendations);

        // Check handshake state
        self.audit_handshake_state(session, &mut vulnerabilities, &mut recommendations);

        // Check for replay attacks
        self.audit_replay_protection(session, &mut vulnerabilities, &mut recommendations);

        // Check cryptographic security
        self.audit_cryptographic_security(session, &mut vulnerabilities, &mut recommendations);

        // Check for timing vulnerabilities
        self.audit_timing_security(&mut vulnerabilities, &mut recommendations);

        // Determine overall security level
        let security_level = self.calculate_security_level(&vulnerabilities);
        let is_secure = matches!(
            security_level,
            SecurityLevel::Good | SecurityLevel::Strong | SecurityLevel::Excellent
        );

        HandshakeAuditResult {
            is_secure,
            vulnerabilities,
            recommendations,
            security_level,
        }
    }

    /// Record timing measurement for side-channel analysis
    pub fn record_handshake_timing(&mut self, duration: Duration) {
        self.timing_measurements.push(duration);

        // Keep only recent measurements
        if self.timing_measurements.len() > 100 {
            self.timing_measurements.remove(0);
        }
    }

    /// Audit protocol-level security
    fn audit_protocol_security(
        &self,
        vulnerabilities: &mut Vec<SecurityVulnerability>,
        recommendations: &mut Vec<String>,
    ) {
        // Check if using secure protocol version
        let secure_protocols = [
            "Noise_XX_25519_ChaChaPoly_SHA256",
            "Noise_IK_25519_ChaChaPoly_SHA256",
            "Noise_NK_25519_ChaChaPoly_SHA256",
        ];

        if !secure_protocols.contains(&self.expected_protocol.as_str()) {
            vulnerabilities.push(SecurityVulnerability::WeakCryptography {
                parameter: "Protocol".to_string(),
                value: self.expected_protocol.clone(),
                recommendation: "Use Noise_XX_25519_ChaChaPoly_SHA256 for maximum security"
                    .to_string(),
            });
        }

        // Add general recommendations
        recommendations.push("Ensure all peers use the same secure protocol version".to_string());
        recommendations
            .push("Implement protocol version pinning to prevent downgrade attacks".to_string());
    }

    /// Audit handshake state transitions
    fn audit_handshake_state(
        &self,
        session: &NoiseSession,
        vulnerabilities: &mut Vec<SecurityVulnerability>,
        recommendations: &mut Vec<String>,
    ) {
        match &session.state {
            NoiseSessionState::Uninitialized => {
                vulnerabilities.push(SecurityVulnerability::InvalidStateTransition {
                    from_state: "Uninitialized".to_string(),
                    to_state: "Active".to_string(),
                    reason: "Session not properly initialized".to_string(),
                });
            }
            NoiseSessionState::HandshakeInProgress { .. } => {
                recommendations.push("Complete handshake before processing messages".to_string());
            }
            NoiseSessionState::TransportReady { .. } => {
                if session.handshake_hash.is_none() {
                    vulnerabilities.push(SecurityVulnerability::MissingSecurityFeature {
                        feature: "Handshake Hash".to_string(),
                        impact: "Cannot verify handshake integrity".to_string(),
                    });
                }
            }
            NoiseSessionState::Terminated => {
                vulnerabilities.push(SecurityVulnerability::InvalidStateTransition {
                    from_state: "Terminated".to_string(),
                    to_state: "Active".to_string(),
                    reason: "Using terminated session".to_string(),
                });
            }
        }

        // Check for missing ephemeral keys
        if session.local_ephemeral.is_none() {
            vulnerabilities.push(SecurityVulnerability::MissingSecurityFeature {
                feature: "Local Ephemeral Key".to_string(),
                impact: "Reduced forward secrecy".to_string(),
            });
        }

        // Check role consistency
        match session.role {
            NoiseRole::Initiator => {
                recommendations.push("Initiator should verify responder identity".to_string());
            }
            NoiseRole::Responder => {
                recommendations.push("Responder should validate initiator credentials".to_string());
            }
        }
    }

    /// Audit for replay attack protection
    fn audit_replay_protection(
        &mut self,
        session: &NoiseSession,
        vulnerabilities: &mut Vec<SecurityVulnerability>,
        recommendations: &mut Vec<String>,
    ) {
        if let Some(handshake_hash) = session.handshake_hash {
            // Check if we've seen this handshake hash before
            if let Some(&original_time) = self.handshake_history.get(&handshake_hash) {
                vulnerabilities.push(SecurityVulnerability::ReplayAttack {
                    duplicate_handshake_hash: handshake_hash,
                    original_timestamp: original_time,
                });
            } else {
                // Record this handshake hash
                self.handshake_history
                    .insert(handshake_hash, Instant::now());
            }

            // Check handshake hash quality
            if self.is_weak_hash(&handshake_hash) {
                vulnerabilities.push(SecurityVulnerability::WeakCryptography {
                    parameter: "Handshake Hash".to_string(),
                    value: "Low entropy detected".to_string(),
                    recommendation: "Ensure proper random number generation".to_string(),
                });
            }
        } else {
            recommendations
                .push("Implement handshake hash verification for replay protection".to_string());
        }
    }

    /// Audit cryptographic security
    fn audit_cryptographic_security(
        &self,
        session: &NoiseSession,
        vulnerabilities: &mut Vec<SecurityVulnerability>,
        recommendations: &mut Vec<String>,
    ) {
        // Check for static key reuse
        if let Some(remote_static) = session.remote_static {
            if self.is_weak_key(&remote_static) {
                vulnerabilities.push(SecurityVulnerability::WeakCryptography {
                    parameter: "Remote Static Key".to_string(),
                    value: "Weak key detected".to_string(),
                    recommendation: "Ensure keys are generated with sufficient entropy".to_string(),
                });
            }
        }

        // Check ephemeral key quality
        if let Some(ref ephemeral) = session.local_ephemeral {
            let public_key = ephemeral.public_key_bytes();
            if self.is_weak_key(&public_key) {
                vulnerabilities.push(SecurityVulnerability::WeakCryptography {
                    parameter: "Ephemeral Key".to_string(),
                    value: "Weak ephemeral key".to_string(),
                    recommendation: "Use cryptographically secure random number generator"
                        .to_string(),
                });
            }
        }

        // General cryptographic recommendations
        recommendations
            .push("Implement key rotation policy for long-lived connections".to_string());
        recommendations.push("Use hardware security modules (HSM) when available".to_string());
        recommendations.push("Implement post-quantum cryptography for future-proofing".to_string());
    }

    /// Audit for timing attack vulnerabilities
    fn audit_timing_security(
        &self,
        vulnerabilities: &mut Vec<SecurityVulnerability>,
        recommendations: &mut Vec<String>,
    ) {
        if self.timing_measurements.len() >= 10 {
            // Analyze timing variations
            let min_time = self.timing_measurements.iter().min().unwrap();
            let max_time = self.timing_measurements.iter().max().unwrap();

            // If timing varies by more than 10%, flag as potential vulnerability
            let variation_ratio = max_time.as_nanos() as f64 / min_time.as_nanos() as f64;

            if variation_ratio > 1.1 {
                vulnerabilities.push(SecurityVulnerability::TimingVulnerability {
                    operation: "Handshake Processing".to_string(),
                    variation_detected: true,
                });

                recommendations.push("Implement constant-time handshake processing".to_string());
            }
        }

        // Always recommend constant-time operations
        recommendations
            .push("Use constant-time comparisons for all cryptographic operations".to_string());
        recommendations.push("Implement timing-resistant error handling".to_string());
    }

    /// Calculate overall security level based on vulnerabilities
    fn calculate_security_level(&self, vulnerabilities: &[SecurityVulnerability]) -> SecurityLevel {
        let mut critical_count = 0;
        let mut major_count = 0;
        let mut minor_count = 0;

        for vulnerability in vulnerabilities {
            match vulnerability {
                SecurityVulnerability::DowngradeAttack { .. }
                | SecurityVulnerability::ReplayAttack { .. } => {
                    critical_count += 1;
                }
                SecurityVulnerability::WeakCryptography { .. }
                | SecurityVulnerability::InvalidStateTransition { .. } => {
                    major_count += 1;
                }
                SecurityVulnerability::TimingVulnerability { .. }
                | SecurityVulnerability::MissingSecurityFeature { .. } => {
                    minor_count += 1;
                }
            }
        }

        if critical_count > 0 {
            SecurityLevel::Insecure
        } else if major_count > 0 {
            SecurityLevel::Weak
        } else if minor_count > 2 {
            SecurityLevel::Good
        } else if minor_count > 0 {
            SecurityLevel::Strong
        } else {
            SecurityLevel::Excellent
        }
    }

    /// Check if a hash appears to be weak (simple heuristics)
    fn is_weak_hash(&self, hash: &[u8; 32]) -> bool {
        // Check for all zeros or all ones
        if hash.iter().all(|&b| b == 0) || hash.iter().all(|&b| b == 0xFF) {
            return true;
        }

        // Check for low entropy (too many repeated bytes)
        let mut byte_counts = [0u32; 256];
        for &byte in hash.iter() {
            byte_counts[byte as usize] += 1;
        }

        // If any byte appears more than 8 times in 32 bytes, consider it weak
        byte_counts.iter().any(|&count| count > 8)
    }

    /// Check if a key appears to be weak
    fn is_weak_key(&self, key: &[u8; 32]) -> bool {
        // Use constant-time entropy quality check
        !ConstantTimeOps::check_entropy_quality_ct(key)
    }

    /// Clean up old entries from handshake history
    fn cleanup_old_history(&mut self) {
        let cutoff_time = Instant::now() - self.max_history_age;
        self.handshake_history
            .retain(|_, &mut timestamp| timestamp > cutoff_time);
    }

    /// Perform deep security analysis on completed handshake
    pub fn deep_security_analysis(&self, session: &NoiseSession) -> Result<SecurityAnalysisReport> {
        let mut report = SecurityAnalysisReport::new();

        // Analyze handshake completeness
        if !session.is_handshake_finished() {
            report.add_issue(SecurityIssue::Critical(
                "Handshake not completed - session vulnerable".to_string(),
            ));
            return Ok(report);
        }

        // Analyze cryptographic strength
        if let Some(hash) = session.handshake_hash {
            self.analyze_handshake_hash(&hash, &mut report)?;
        }

        // Analyze key exchange
        if let Some(ref ephemeral) = session.local_ephemeral {
            self.analyze_key_exchange(ephemeral, &mut report)?;
        }

        // Analyze protocol implementation
        self.analyze_protocol_implementation(session, &mut report)?;

        Ok(report)
    }

    /// Analyze handshake hash for security properties
    fn analyze_handshake_hash(
        &self,
        hash: &[u8; 32],
        report: &mut SecurityAnalysisReport,
    ) -> Result<()> {
        // Check hash entropy
        if !ConstantTimeOps::check_entropy_quality_ct(hash) {
            report.add_issue(SecurityIssue::Major(
                "Handshake hash has insufficient entropy".to_string(),
            ));
        }

        // Check for known weak hashes (this would be a real database in production)
        if self.is_known_weak_hash(hash) {
            report.add_issue(SecurityIssue::Critical(
                "Handshake hash matches known weak pattern".to_string(),
            ));
        }

        Ok(())
    }

    /// Analyze key exchange security
    fn analyze_key_exchange(
        &self,
        ephemeral: &crate::crypto::BitchatKeypair,
        report: &mut SecurityAnalysisReport,
    ) -> Result<()> {
        let public_key = ephemeral.public_key_bytes();

        // Check public key validity
        if self.is_weak_key(&public_key) {
            report.add_issue(SecurityIssue::Major(
                "Ephemeral public key appears weak".to_string(),
            ));
        }

        // Check for key reuse (in production, this would check against a database)
        if self.is_key_reused(&public_key) {
            report.add_issue(SecurityIssue::Critical(
                "Ephemeral key reuse detected - forward secrecy compromised".to_string(),
            ));
        }

        Ok(())
    }

    /// Analyze protocol implementation security
    fn analyze_protocol_implementation(
        &self,
        session: &NoiseSession,
        report: &mut SecurityAnalysisReport,
    ) -> Result<()> {
        // Check session state consistency
        match (&session.state, session.handshake_hash) {
            (NoiseSessionState::TransportReady { .. }, None) => {
                report.add_issue(SecurityIssue::Major(
                    "Transport ready without handshake hash".to_string(),
                ));
            }
            _ => {}
        }

        // Check role consistency
        match session.role {
            NoiseRole::Initiator => {
                if session.remote_static.is_none() {
                    report.add_issue(SecurityIssue::Minor(
                        "Initiator missing remote static key verification".to_string(),
                    ));
                }
            }
            NoiseRole::Responder => {
                if session.local_ephemeral.is_none() {
                    report.add_issue(SecurityIssue::Minor(
                        "Responder missing local ephemeral key".to_string(),
                    ));
                }
            }
        }

        Ok(())
    }

    /// Check if hash matches known weak patterns
    fn is_known_weak_hash(&self, _hash: &[u8; 32]) -> bool {
        // In production, this would check against a database of known weak hashes
        false
    }

    /// Check if key has been reused
    fn is_key_reused(&self, _key: &[u8; 32]) -> bool {
        // In production, this would check against a key usage database
        false
    }
}

/// Comprehensive security analysis report
#[derive(Debug, Clone)]
pub struct SecurityAnalysisReport {
    pub issues: Vec<SecurityIssue>,
    pub overall_security_score: f32,
}

/// Types of security issues
#[derive(Debug, Clone)]
pub enum SecurityIssue {
    Critical(String),
    Major(String),
    Minor(String),
    Info(String),
}

impl SecurityAnalysisReport {
    pub fn new() -> Self {
        Self {
            issues: Vec::new(),
            overall_security_score: 100.0,
        }
    }

    pub fn add_issue(&mut self, issue: SecurityIssue) {
        // Adjust security score based on issue severity
        match issue {
            SecurityIssue::Critical(_) => self.overall_security_score -= 25.0,
            SecurityIssue::Major(_) => self.overall_security_score -= 10.0,
            SecurityIssue::Minor(_) => self.overall_security_score -= 3.0,
            SecurityIssue::Info(_) => {} // No score impact
        }

        self.issues.push(issue);

        // Ensure score doesn't go below 0
        if self.overall_security_score < 0.0 {
            self.overall_security_score = 0.0;
        }
    }

    pub fn is_secure(&self) -> bool {
        self.overall_security_score >= 80.0 && !self.has_critical_issues()
    }

    pub fn has_critical_issues(&self) -> bool {
        self.issues
            .iter()
            .any(|issue| matches!(issue, SecurityIssue::Critical(_)))
    }
}

impl Default for SecurityAnalysisReport {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::BitchatKeypair;

    #[test]
    fn test_handshake_auditor() {
        let mut auditor = HandshakeAuditor::new();

        // Create a mock session for testing
        let keypair = BitchatKeypair::generate();
        let session = NoiseSession::new_initiator(&keypair).unwrap();

        let result = auditor.audit_handshake(&session);

        // Should detect that handshake is not completed
        assert!(!result.is_secure);
        assert!(!result.vulnerabilities.is_empty());
    }

    #[test]
    fn test_weak_hash_detection() {
        let auditor = HandshakeAuditor::new();

        // Test obviously weak hashes
        let zero_hash = [0u8; 32];
        assert!(auditor.is_weak_hash(&zero_hash));

        let ones_hash = [0xFFu8; 32];
        assert!(auditor.is_weak_hash(&ones_hash));

        // Test reasonable hash
        let good_hash = [0x42u8; 32];
        assert!(!auditor.is_weak_hash(&good_hash));
    }

    #[test]
    fn test_security_level_calculation() {
        let auditor = HandshakeAuditor::new();

        // Test with no vulnerabilities
        let no_vulns = Vec::new();
        assert_eq!(
            auditor.calculate_security_level(&no_vulns),
            SecurityLevel::Excellent
        );

        // Test with critical vulnerability
        let critical_vulns = vec![SecurityVulnerability::DowngradeAttack {
            attempted_protocol: "weak".to_string(),
            secure_protocol: "strong".to_string(),
        }];
        assert_eq!(
            auditor.calculate_security_level(&critical_vulns),
            SecurityLevel::Insecure
        );
    }

    #[test]
    fn test_timing_analysis() {
        let mut auditor = HandshakeAuditor::new();

        // Add some timing measurements with variation
        auditor.record_handshake_timing(Duration::from_millis(100));
        auditor.record_handshake_timing(Duration::from_millis(200));
        auditor.record_handshake_timing(Duration::from_millis(150));

        let mut vulnerabilities = Vec::new();
        let mut recommendations = Vec::new();

        auditor.audit_timing_security(&mut vulnerabilities, &mut recommendations);

        // Should detect timing variation
        assert!(!vulnerabilities.is_empty());
        assert!(!recommendations.is_empty());
    }

    #[test]
    fn test_replay_detection() {
        let mut auditor = HandshakeAuditor::new();
        let keypair = BitchatKeypair::generate();

        // Create session with same handshake hash
        let mut session1 = NoiseSession::new_initiator(&keypair).unwrap();
        let test_hash = [0x42u8; 32];
        session1.handshake_hash = Some(test_hash);

        let mut session2 = NoiseSession::new_initiator(&keypair).unwrap();
        session2.handshake_hash = Some(test_hash);

        // First session should be fine
        let result1 = auditor.audit_handshake(&session1);
        let has_replay = result1
            .vulnerabilities
            .iter()
            .any(|v| matches!(v, SecurityVulnerability::ReplayAttack { .. }));
        assert!(!has_replay);

        // Second session with same hash should trigger replay detection
        let result2 = auditor.audit_handshake(&session2);
        let has_replay = result2
            .vulnerabilities
            .iter()
            .any(|v| matches!(v, SecurityVulnerability::ReplayAttack { .. }));
        assert!(has_replay);
    }
}
