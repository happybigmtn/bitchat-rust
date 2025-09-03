//! # Jurisdiction-Specific Compliance Engine
//!
//! Configurable compliance rules engine that adapts to different regulatory
//! jurisdictions and their specific requirements.

use crate::{Error, Result, PeerId};
use crate::compliance::{ComplianceLevel, ComplianceRestriction, DocumentType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::Duration;

/// Jurisdiction-specific compliance rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComplianceRule {
    /// Age verification requirement
    AgeVerification { minimum_age: u8 },
    /// Daily transaction limit
    TransactionLimit { daily_limit: u64 },
    /// Required identity documents
    RequiredDocuments { documents: Vec<DocumentType> },
    /// Prohibited jurisdictions
    GeoBlocking { blocked_countries: Vec<String> },
    /// Enhanced due diligence threshold
    EnhancedDueDiligence { threshold: u64 },
    /// Cooling-off periods
    CoolingOffPeriod { duration: Duration },
}

/// Regulatory requirements for a jurisdiction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegulatoryRequirement {
    /// Minimum compliance level required
    pub compliance_level: ComplianceLevel,
    /// Applicable rules
    pub rules: Vec<ComplianceRule>,
    /// Restrictions that apply
    pub restrictions: Vec<ComplianceRestriction>,
    /// Regulatory authority
    pub authority: String,
}

/// Jurisdiction compliance engine
pub struct JurisdictionEngine {
    /// Jurisdiction configurations
    jurisdictions: HashMap<String, RegulatoryRequirement>,
}

impl JurisdictionEngine {
    /// Create new jurisdiction engine
    pub fn new(jurisdictions: &HashMap<String, crate::compliance::JurisdictionConfig>) -> Result<Self> {
        let mut engine_jurisdictions = HashMap::new();
        
        for (country_code, config) in jurisdictions {
            let requirement = RegulatoryRequirement {
                compliance_level: config.compliance_level,
                rules: config.rules.clone(),
                restrictions: Vec::new(), // Would be derived from rules
                authority: config.authority.clone(),
            };
            engine_jurisdictions.insert(country_code.clone(), requirement);
        }

        Ok(Self {
            jurisdictions: engine_jurisdictions,
        })
    }

    /// Get regulatory requirements for jurisdiction
    pub fn get_requirements(&self, jurisdiction: &str) -> Result<&RegulatoryRequirement> {
        self.jurisdictions.get(jurisdiction)
            .ok_or_else(|| Error::ValidationError(format!("Unknown jurisdiction: {}", jurisdiction)))
    }

    /// Check if user meets jurisdiction requirements
    pub fn check_compliance(&self, jurisdiction: &str, user_data: &UserComplianceData) -> Result<bool> {
        let requirements = self.get_requirements(jurisdiction)?;
        
        for rule in &requirements.rules {
            if !self.check_rule(rule, user_data)? {
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Check individual compliance rule
    fn check_rule(&self, rule: &ComplianceRule, user_data: &UserComplianceData) -> Result<bool> {
        match rule {
            ComplianceRule::AgeVerification { minimum_age } => {
                Ok(user_data.age.unwrap_or(0) >= *minimum_age)
            },
            ComplianceRule::TransactionLimit { daily_limit } => {
                Ok(user_data.daily_transaction_total <= *daily_limit)
            },
            ComplianceRule::RequiredDocuments { documents } => {
                Ok(documents.iter().all(|doc| user_data.verified_documents.contains(doc)))
            },
            ComplianceRule::GeoBlocking { blocked_countries } => {
                Ok(!blocked_countries.iter().any(|country| 
                    user_data.location.as_ref().map_or(false, |loc| loc == country)
                ))
            },
            ComplianceRule::EnhancedDueDiligence { threshold } => {
                // If transaction is above threshold, enhanced due diligence is required
                Ok(user_data.enhanced_due_diligence_completed || 
                   user_data.daily_transaction_total <= *threshold)
            },
            ComplianceRule::CoolingOffPeriod { duration: _duration } => {
                // Would check if user has completed cooling-off period
                Ok(true) // Simplified for now
            },
        }
    }
}

/// User compliance data for jurisdiction checking
#[derive(Debug, Clone)]
pub struct UserComplianceData {
    pub age: Option<u8>,
    pub daily_transaction_total: u64,
    pub verified_documents: Vec<DocumentType>,
    pub location: Option<String>,
    pub enhanced_due_diligence_completed: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jurisdiction_engine() {
        let mut jurisdictions = HashMap::new();
        jurisdictions.insert("US".to_string(), crate::compliance::JurisdictionConfig {
            country_code: "US".to_string(),
            authority: "FinCEN".to_string(),
            compliance_level: ComplianceLevel::Enhanced,
            rules: vec![
                ComplianceRule::AgeVerification { minimum_age: 21 },
                ComplianceRule::TransactionLimit { daily_limit: 10000 },
            ],
            required_documents: vec![DocumentType::PhotoId],
        });

        let engine = JurisdictionEngine::new(&jurisdictions).unwrap();
        let requirements = engine.get_requirements("US").unwrap();
        assert_eq!(requirements.compliance_level, ComplianceLevel::Enhanced);
    }
}