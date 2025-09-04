#![cfg(feature = "compliance")]

//! # Compliance Framework
//! 
//! Comprehensive compliance and regulatory framework for BitCraps protocol.
//! Implements KYC/AML, sanctions screening, and multi-jurisdiction support.
//!
//! ## Security Architecture
//! 
//! - **Zero-Knowledge KYC**: Identity verification without storing sensitive data
//! - **Privacy-Preserving AML**: Transaction monitoring with cryptographic privacy
//! - **Jurisdiction-Aware**: Configurable compliance rules per region
//! - **Audit Trail**: Immutable logging for regulatory compliance

pub mod kyc;
pub mod aml;
pub mod sanctions;
pub mod audit;
pub mod jurisdiction;
pub mod reporting;

pub use kyc::{KycProvider, KycStatus, IdentityVerification, BiometricTemplate};
pub use aml::{AmlMonitor, TransactionRisk, MoneyLaunderingDetector, RiskScore};
pub use sanctions::{SanctionsScreening, SanctionsResult, WatchlistProvider};
pub use audit::{ComplianceAudit, AuditEvent, AuditLogger};
pub use jurisdiction::{JurisdictionEngine, ComplianceRule, RegulatoryRequirement};
pub use reporting::{ComplianceReporter, RegulatoryReport, ReportType};

use crate::{Error, Result, PeerId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};

/// Compliance configuration for different operational modes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceConfig {
    /// Enable KYC verification requirements
    pub enable_kyc: bool,
    /// Enable AML transaction monitoring
    pub enable_aml: bool,
    /// Enable sanctions screening
    pub enable_sanctions: bool,
    /// Minimum age requirement (varies by jurisdiction)
    pub minimum_age: u8,
    /// Maximum transaction amount without enhanced verification
    pub transaction_threshold: u64,
    /// Jurisdiction-specific configuration
    pub jurisdictions: HashMap<String, JurisdictionConfig>,
    /// Audit logging configuration
    pub audit_config: AuditConfig,
}

/// Jurisdiction-specific compliance requirements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JurisdictionConfig {
    /// ISO country code
    pub country_code: String,
    /// Regulatory authority
    pub authority: String,
    /// Required compliance level
    pub compliance_level: ComplianceLevel,
    /// Specific rules for this jurisdiction
    pub rules: Vec<ComplianceRule>,
    /// Required documentation
    pub required_documents: Vec<DocumentType>,
}

/// Compliance levels for different jurisdictions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComplianceLevel {
    /// Basic compliance - minimal requirements
    Basic,
    /// Standard compliance - typical financial services
    Standard,
    /// Enhanced compliance - high-risk jurisdictions
    Enhanced,
    /// Strict compliance - maximum regulatory oversight
    Strict,
}

/// Types of identity documents accepted
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DocumentType {
    /// Government-issued photo ID
    PhotoId,
    /// Passport
    Passport,
    /// Driver's license
    DriversLicense,
    /// Proof of address
    ProofOfAddress,
    /// Tax identification number
    TaxId,
    /// Professional license
    ProfessionalLicense,
}

/// Audit logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditConfig {
    /// Enable audit logging
    pub enabled: bool,
    /// Log retention period in days
    pub retention_days: u32,
    /// Encrypt audit logs
    pub encrypt_logs: bool,
    /// Digital signature for audit integrity
    pub sign_logs: bool,
}

/// User compliance status and verification level
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceStatus {
    /// User's peer ID
    pub peer_id: PeerId,
    /// KYC verification status
    pub kyc_status: KycStatus,
    /// AML risk score
    pub risk_score: RiskScore,
    /// Sanctions screening result
    pub sanctions_clear: bool,
    /// Verified jurisdiction
    pub jurisdiction: Option<String>,
    /// Age verification status
    pub age_verified: bool,
    /// Compliance level achieved
    pub compliance_level: ComplianceLevel,
    /// Last verification timestamp
    pub last_verified: DateTime<Utc>,
    /// Active restrictions or limitations
    pub restrictions: Vec<ComplianceRestriction>,
}

/// Compliance restrictions that may be applied to users
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComplianceRestriction {
    /// Maximum bet amount per game
    MaxBetAmount(u64),
    /// Maximum total stakes per day
    DailyStakeLimit(u64),
    /// Require additional verification for transactions
    EnhancedVerification,
    /// Geographic restrictions
    GeographicRestriction(Vec<String>),
    /// Time-based restrictions (cool-down periods)
    CooldownPeriod(chrono::Duration),
    /// Account suspended pending investigation
    AccountSuspended,
}

/// Central compliance manager coordinating all compliance systems
#[derive(Debug)]
pub struct ComplianceManager {
    config: ComplianceConfig,
    kyc_provider: Box<dyn KycProvider + Send + Sync>,
    aml_monitor: Box<dyn AmlMonitor + Send + Sync>,
    sanctions_screening: Box<dyn SanctionsScreening + Send + Sync>,
    jurisdiction_engine: JurisdictionEngine,
    audit_logger: AuditLogger,
    reporter: ComplianceReporter,
    user_status: HashMap<PeerId, ComplianceStatus>,
}

impl ComplianceManager {
    /// Create new compliance manager with configuration
    pub async fn new(
        config: ComplianceConfig,
        kyc_provider: Box<dyn KycProvider + Send + Sync>,
        aml_monitor: Box<dyn AmlMonitor + Send + Sync>,
        sanctions_screening: Box<dyn SanctionsScreening + Send + Sync>,
    ) -> Result<Self> {
        let jurisdiction_engine = JurisdictionEngine::new(&config.jurisdictions)?;
        let audit_logger = AuditLogger::new(audit::AuditConfig {
            enabled: config.audit_config.enabled,
            retention_days: config.audit_config.retention_days,
            encrypt_logs: config.audit_config.encrypt_logs,
            sign_logs: config.audit_config.sign_logs,
            max_entries_per_file: 10000,
            compression_level: 6,
            alert_thresholds: audit::AlertThresholds {
                high_severity_per_hour: 100,
                critical_per_day: 10,
                pattern_detection: true,
            },
        }).await?;
        let reporter = ComplianceReporter::new().await?;
        
        Ok(Self {
            config,
            kyc_provider,
            aml_monitor,
            sanctions_screening,
            jurisdiction_engine,
            audit_logger,
            reporter,
            user_status: HashMap::new(),
        })
    }

    /// Verify user compliance for game participation
    pub async fn verify_user_compliance(&mut self, peer_id: PeerId) -> Result<ComplianceStatus> {
        // Check if user already verified
        if let Some(status) = self.user_status.get(&peer_id) {
            if self.is_verification_current(status) {
                return Ok(status.clone());
            }
        }

        let mut status = ComplianceStatus {
            peer_id,
            kyc_status: KycStatus::NotVerified,
            risk_score: RiskScore::Unknown,
            sanctions_clear: false,
            jurisdiction: None,
            age_verified: false,
            compliance_level: ComplianceLevel::Basic,
            last_verified: Utc::now(),
            restrictions: Vec::new(),
        };

        // KYC verification if enabled
        if self.config.enable_kyc {
            status.kyc_status = self.kyc_provider.verify_identity(peer_id).await?;
            
            // Age verification as part of KYC
            if status.kyc_status.is_verified() {
                let age_result = self.kyc_provider.verify_age(peer_id, self.config.minimum_age).await?;
                status.age_verified = age_result.is_verified();
            }
        }

        // AML risk assessment if enabled
        if self.config.enable_aml {
            status.risk_score = self.aml_monitor.assess_risk(peer_id).await?;
        }

        // Sanctions screening if enabled
        if self.config.enable_sanctions {
            let sanctions_result = self.sanctions_screening.screen_user(peer_id).await?;
            status.sanctions_clear = sanctions_result.is_clear();
        }

        // Determine jurisdiction and applicable rules
        if let Some(jurisdiction) = self.detect_user_jurisdiction(peer_id).await? {
            status.jurisdiction = Some(jurisdiction.clone());
            let requirements = self.jurisdiction_engine.get_requirements(&jurisdiction)?;
            status.compliance_level = requirements.compliance_level;
            status.restrictions.extend(requirements.restrictions.clone());
        }

        // Log compliance check
        self.audit_logger.log_event(AuditEvent::ComplianceCheck {
            peer_id,
            result: status.clone(),
            timestamp: Utc::now(),
        }).await?;

        // Cache status
        self.user_status.insert(peer_id, status.clone());

        Ok(status)
    }

    /// Check if user can participate in a game with given stake
    pub async fn authorize_participation(
        &mut self, 
        peer_id: PeerId, 
        stake_amount: u64
    ) -> Result<bool> {
        let status = self.verify_user_compliance(peer_id).await?;

        // Check basic compliance requirements
        if !self.meets_basic_requirements(&status) {
            return Ok(false);
        }

        // Check stake amount against restrictions
        if !self.check_stake_limits(&status, stake_amount) {
            return Ok(false);
        }

        // Log authorization
        self.audit_logger.log_event(AuditEvent::ParticipationAuthorized {
            peer_id,
            stake_amount,
            timestamp: Utc::now(),
        }).await?;

        Ok(true)
    }

    /// Monitor transaction for AML compliance
    pub async fn monitor_transaction(
        &mut self,
        from_peer: PeerId,
        to_peer: PeerId,
        amount: u64,
        transaction_type: String,
    ) -> Result<TransactionRisk> {
        if !self.config.enable_aml {
            return Ok(TransactionRisk {
                score: crate::compliance::aml::RiskScore::Low,
                confidence: 100,
                risk_factors: vec![],
                recommendations: vec![crate::compliance::aml::RiskRecommendation::Allow],
                assessed_at: chrono::Utc::now(),
            });
        }

        let risk = self.aml_monitor.monitor_transaction(
            from_peer,
            to_peer,
            amount,
            transaction_type.clone(),
        ).await?;

        // Log high-risk transactions
        if risk.score() >= RiskScore::High {
            self.audit_logger.log_event(AuditEvent::HighRiskTransaction {
                from_peer,
                to_peer,
                amount,
                transaction_type,
                risk_score: risk.score(),
                timestamp: Utc::now(),
            }).await?;
        }

        Ok(risk)
    }

    /// Generate compliance report for regulatory authorities
    pub async fn generate_compliance_report(
        &self,
        report_type: ReportType,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
    ) -> Result<RegulatoryReport> {
        self.reporter.generate_report(report_type, start_date, end_date).await
    }

    /// Update user compliance status (e.g., after additional verification)
    pub async fn update_user_status(&mut self, peer_id: PeerId, status: ComplianceStatus) -> Result<()> {
        self.user_status.insert(peer_id, status.clone());

        self.audit_logger.log_event(AuditEvent::StatusUpdate {
            peer_id,
            new_status: status,
            timestamp: Utc::now(),
        }).await?;

        Ok(())
    }

    /// Check if verification is still current
    fn is_verification_current(&self, status: &ComplianceStatus) -> bool {
        let age = Utc::now().signed_duration_since(status.last_verified);
        // Re-verify after 30 days for enhanced compliance, 90 days for basic
        let max_age = match status.compliance_level {
            ComplianceLevel::Basic => chrono::Duration::days(90),
            ComplianceLevel::Standard => chrono::Duration::days(60),
            ComplianceLevel::Enhanced => chrono::Duration::days(30),
            ComplianceLevel::Strict => chrono::Duration::days(7),
        };
        
        age < max_age
    }

    /// Check if user meets basic compliance requirements
    fn meets_basic_requirements(&self, status: &ComplianceStatus) -> bool {
        // Age verification required
        if self.config.enable_kyc && !status.age_verified {
            return false;
        }

        // Must pass sanctions screening
        if self.config.enable_sanctions && !status.sanctions_clear {
            return false;
        }

        // Must not be suspended
        if status.restrictions.contains(&ComplianceRestriction::AccountSuspended) {
            return false;
        }

        true
    }

    /// Check stake amount against user restrictions
    fn check_stake_limits(&self, status: &ComplianceStatus, stake_amount: u64) -> bool {
        for restriction in &status.restrictions {
            match restriction {
                ComplianceRestriction::MaxBetAmount(limit) => {
                    if stake_amount > *limit {
                        return false;
                    }
                },
                ComplianceRestriction::DailyStakeLimit(_) => {
                    // Would need to track daily totals - simplified for now
                    // In production, would check against stored daily totals
                },
                _ => {}
            }
        }
        true
    }

    /// Detect user's jurisdiction from IP/location data
    async fn detect_user_jurisdiction(&self, _peer_id: PeerId) -> Result<Option<String>> {
        // In production, would use IP geolocation, user-provided info, etc.
        // For now, return None - jurisdiction detection is complex
        Ok(None)
    }
}

impl Default for ComplianceConfig {
    fn default() -> Self {
        let mut jurisdictions = HashMap::new();
        
        // Add US configuration as example
        jurisdictions.insert("US".to_string(), JurisdictionConfig {
            country_code: "US".to_string(),
            authority: "FinCEN".to_string(),
            compliance_level: ComplianceLevel::Enhanced,
            rules: vec![
                ComplianceRule::AgeVerification { minimum_age: 21 },
                ComplianceRule::TransactionLimit { daily_limit: 10000 },
            ],
            required_documents: vec![DocumentType::PhotoId, DocumentType::ProofOfAddress],
        });

        Self {
            enable_kyc: true,
            enable_aml: true,
            enable_sanctions: true,
            minimum_age: 18,
            transaction_threshold: 1000,
            jurisdictions,
            audit_config: AuditConfig {
                enabled: true,
                retention_days: 2555, // 7 years
                encrypt_logs: true,
                sign_logs: true,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compliance_config_default() {
        let config = ComplianceConfig::default();
        assert!(config.enable_kyc);
        assert!(config.enable_aml);
        assert!(config.enable_sanctions);
        assert_eq!(config.minimum_age, 18);
        assert!(config.jurisdictions.contains_key("US"));
    }

    #[test]
    fn test_compliance_level_ordering() {
        assert!(ComplianceLevel::Basic < ComplianceLevel::Standard);
        assert!(ComplianceLevel::Standard < ComplianceLevel::Enhanced);
        assert!(ComplianceLevel::Enhanced < ComplianceLevel::Strict);
    }
}