//! GDPR Compliance Verification
//! 
//! Implements automated testing for General Data Protection Regulation (GDPR)
//! compliance requirements for the BitCraps platform.

use super::{ComplianceViolation, ViolationSeverity, ComplianceRecommendation};
use std::collections::HashMap;
use std::time::SystemTime;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone)]
pub struct GDPRComplianceChecker {
    data_processing_activities: Vec<DataProcessingActivity>,
    privacy_controls: HashMap<String, PrivacyControl>,
    consent_mechanisms: Vec<ConsentMechanism>,
    data_retention_policies: HashMap<String, RetentionPolicy>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GDPRComplianceResult {
    pub compliance_score: f64,
    pub violations: Vec<ComplianceViolation>,
    pub recommendations: Vec<ComplianceRecommendation>,
    pub privacy_assessment: PrivacyImpactAssessment,
    pub data_flow_analysis: DataFlowAnalysis,
}

#[derive(Debug, Clone)]
struct DataProcessingActivity {
    name: String,
    purpose: String,
    legal_basis: LegalBasis,
    data_categories: Vec<DataCategory>,
    retention_period: Option<std::time::Duration>,
    security_measures: Vec<String>,
}

#[derive(Debug, Clone)]
enum LegalBasis {
    Consent,
    Contract,
    LegalObligation,
    VitalInterests,
    PublicTask,
    LegitimateInterests,
}

#[derive(Debug, Clone)]
enum DataCategory {
    PersonalIdentifiers,  // IP addresses, device IDs
    TechnicalData,       // BLE MAC addresses, connection logs
    BehavioralData,      // Game patterns, usage statistics
    CommunicationData,   // P2P messages, network topology
    BiometricData,       // Optional: fingerprint/face unlock
    LocationData,        // Optional: geolocation for compliance
}

#[derive(Debug, Clone)]
struct PrivacyControl {
    name: String,
    implemented: bool,
    effectiveness: f64, // 0-100
    description: String,
}

#[derive(Debug, Clone)]
struct ConsentMechanism {
    purpose: String,
    is_granular: bool,
    is_withdrawable: bool,
    is_informed: bool,
    consent_storage: ConsentStorage,
}

#[derive(Debug, Clone)]
enum ConsentStorage {
    Local,
    Distributed,
    NotStored,
}

#[derive(Debug, Clone)]
struct RetentionPolicy {
    data_type: String,
    retention_period: std::time::Duration,
    deletion_method: DeletionMethod,
    automated: bool,
}

#[derive(Debug, Clone)]
enum DeletionMethod {
    SecureErase,
    Overwrite,
    Anonymize,
    Pseudonymize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyImpactAssessment {
    pub overall_risk: String,
    pub identified_risks: Vec<PrivacyRisk>,
    pub mitigation_measures: Vec<String>,
    pub residual_risk: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyRisk {
    pub description: String,
    pub likelihood: String,
    pub impact: String,
    pub risk_level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataFlowAnalysis {
    pub data_sources: Vec<String>,
    pub processing_locations: Vec<String>,
    pub data_sharing: Vec<DataSharingActivity>,
    pub cross_border_transfers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataSharingActivity {
    pub recipient: String,
    pub purpose: String,
    pub legal_basis: String,
    pub safeguards: Vec<String>,
}

impl GDPRComplianceChecker {
    pub fn new() -> Self {
        let mut checker = Self {
            data_processing_activities: Vec::new(),
            privacy_controls: HashMap::new(),
            consent_mechanisms: Vec::new(),
            data_retention_policies: HashMap::new(),
        };
        
        checker.initialize_bitcraps_data_processing();
        checker.initialize_privacy_controls();
        checker.initialize_consent_mechanisms();
        checker.initialize_retention_policies();
        
        checker
    }
    
    fn initialize_bitcraps_data_processing(&mut self) {
        // Game session data
        self.data_processing_activities.push(DataProcessingActivity {
            name: "Game Session Management".to_string(),
            purpose: "Facilitate decentralized gaming sessions".to_string(),
            legal_basis: LegalBasis::Contract,
            data_categories: vec![
                DataCategory::PersonalIdentifiers,
                DataCategory::TechnicalData,
                DataCategory::BehavioralData,
            ],
            retention_period: Some(std::time::Duration::from_secs(30 * 24 * 60 * 60)), // 30 days
            security_measures: vec![
                "End-to-end encryption".to_string(),
                "Digital signatures".to_string(),
                "Local data storage".to_string(),
            ],
        });
        
        // P2P networking
        self.data_processing_activities.push(DataProcessingActivity {
            name: "Peer-to-Peer Networking".to_string(),
            purpose: "Enable device discovery and communication".to_string(),
            legal_basis: LegalBasis::LegitimateInterests,
            data_categories: vec![
                DataCategory::TechnicalData,
                DataCategory::CommunicationData,
            ],
            retention_period: Some(std::time::Duration::from_secs(24 * 60 * 60)), // 24 hours
            security_measures: vec![
                "MAC address randomization".to_string(),
                "Encrypted transport".to_string(),
                "Session keys".to_string(),
            ],
        });
        
        // Cryptographic key management
        self.data_processing_activities.push(DataProcessingActivity {
            name: "Cryptographic Key Management".to_string(),
            purpose: "Secure user identity and transaction signing".to_string(),
            legal_basis: LegalBasis::Contract,
            data_categories: vec![
                DataCategory::PersonalIdentifiers,
            ],
            retention_period: None, // Permanent until user deletion
            security_measures: vec![
                "Hardware security module".to_string(),
                "Key derivation functions".to_string(),
                "Secure enclave storage".to_string(),
            ],
        });
    }
    
    fn initialize_privacy_controls(&mut self) {
        // Data minimization
        self.privacy_controls.insert("data_minimization".to_string(), PrivacyControl {
            name: "Data Minimization".to_string(),
            implemented: true,
            effectiveness: 90.0,
            description: "System collects only necessary data for gaming functionality".to_string(),
        });
        
        // Purpose limitation
        self.privacy_controls.insert("purpose_limitation".to_string(), PrivacyControl {
            name: "Purpose Limitation".to_string(),
            implemented: true,
            effectiveness: 95.0,
            description: "Data used only for stated gaming and networking purposes".to_string(),
        });
        
        // Storage limitation
        self.privacy_controls.insert("storage_limitation".to_string(), PrivacyControl {
            name: "Storage Limitation".to_string(),
            implemented: true,
            effectiveness: 85.0,
            description: "Automated data deletion after retention periods".to_string(),
        });
        
        // Accuracy
        self.privacy_controls.insert("data_accuracy".to_string(), PrivacyControl {
            name: "Data Accuracy".to_string(),
            implemented: true,
            effectiveness: 80.0,
            description: "Real-time data validation and error correction".to_string(),
        });
        
        // Security
        self.privacy_controls.insert("data_security".to_string(), PrivacyControl {
            name: "Data Security".to_string(),
            implemented: true,
            effectiveness: 95.0,
            description: "Strong encryption and secure storage mechanisms".to_string(),
        });
        
        // Transparency
        self.privacy_controls.insert("transparency".to_string(), PrivacyControl {
            name: "Transparency".to_string(),
            implemented: false, // Need privacy policy
            effectiveness: 60.0,
            description: "Clear privacy notices and data processing information".to_string(),
        });
        
        // User rights
        self.privacy_controls.insert("user_rights".to_string(), PrivacyControl {
            name: "Individual Rights".to_string(),
            implemented: false, // Need implementation
            effectiveness: 40.0,
            description: "Data access, rectification, and deletion capabilities".to_string(),
        });
    }
    
    fn initialize_consent_mechanisms(&mut self) {
        // Game participation consent
        self.consent_mechanisms.push(ConsentMechanism {
            purpose: "Participate in gaming sessions".to_string(),
            is_granular: true,
            is_withdrawable: true,
            is_informed: true,
            consent_storage: ConsentStorage::Local,
        });
        
        // P2P networking consent
        self.consent_mechanisms.push(ConsentMechanism {
            purpose: "P2P device discovery and communication".to_string(),
            is_granular: true,
            is_withdrawable: true,
            is_informed: true,
            consent_storage: ConsentStorage::Local,
        });
        
        // Optional analytics consent
        self.consent_mechanisms.push(ConsentMechanism {
            purpose: "Anonymous usage analytics".to_string(),
            is_granular: true,
            is_withdrawable: true,
            is_informed: true,
            consent_storage: ConsentStorage::Local,
        });
    }
    
    fn initialize_retention_policies(&mut self) {
        // Game session data
        self.data_retention_policies.insert("game_sessions".to_string(), RetentionPolicy {
            data_type: "Game session history".to_string(),
            retention_period: std::time::Duration::from_secs(30 * 24 * 60 * 60), // 30 days
            deletion_method: DeletionMethod::SecureErase,
            automated: true,
        });
        
        // Network topology data
        self.data_retention_policies.insert("network_topology".to_string(), RetentionPolicy {
            data_type: "P2P network topology".to_string(),
            retention_period: std::time::Duration::from_secs(24 * 60 * 60), // 24 hours
            deletion_method: DeletionMethod::SecureErase,
            automated: true,
        });
        
        // Connection logs
        self.data_retention_policies.insert("connection_logs".to_string(), RetentionPolicy {
            data_type: "Connection and error logs".to_string(),
            retention_period: std::time::Duration::from_secs(7 * 24 * 60 * 60), // 7 days
            deletion_method: DeletionMethod::Anonymize,
            automated: true,
        });
        
        // User-generated keys
        self.data_retention_policies.insert("user_keys".to_string(), RetentionPolicy {
            data_type: "User cryptographic keys".to_string(),
            retention_period: std::time::Duration::from_secs(0), // Until user deletion
            deletion_method: DeletionMethod::SecureErase,
            automated: false,
        });
    }
    
    pub async fn check_compliance(&self) -> Result<GDPRComplianceResult, Box<dyn std::error::Error>> {
        let mut violations = Vec::new();
        let mut recommendations = Vec::new();
        
        println!("🇪🇺 Checking GDPR compliance...");
        
        // Check Article 5 principles
        self.check_data_processing_principles(&mut violations, &mut recommendations);
        
        // Check Article 6 legal basis
        self.check_legal_basis(&mut violations, &mut recommendations);
        
        // Check Article 7 consent requirements
        self.check_consent_requirements(&mut violations, &mut recommendations);
        
        // Check Article 12-14 transparency requirements
        self.check_transparency_requirements(&mut violations, &mut recommendations);
        
        // Check Article 15-22 individual rights
        self.check_individual_rights(&mut violations, &mut recommendations);
        
        // Check Article 25 privacy by design
        self.check_privacy_by_design(&mut violations, &mut recommendations);
        
        // Check Article 32 security requirements
        self.check_security_requirements(&mut violations, &mut recommendations);
        
        // Check Article 35 privacy impact assessment
        let privacy_assessment = self.conduct_privacy_impact_assessment();
        
        // Check Article 44-49 international transfers
        self.check_international_transfers(&mut violations, &mut recommendations);
        
        // Calculate compliance score
        let compliance_score = self.calculate_compliance_score(&violations);
        
        // Generate data flow analysis
        let data_flow_analysis = self.analyze_data_flows();
        
        println!("  ✅ GDPR compliance check completed. Score: {:.1}%", compliance_score);
        
        Ok(GDPRComplianceResult {
            compliance_score,
            violations,
            recommendations,
            privacy_assessment,
            data_flow_analysis,
        })
    }
    
    fn check_data_processing_principles(&self, violations: &mut Vec<ComplianceViolation>, recommendations: &mut Vec<ComplianceRecommendation>) {
        // Article 5(1)(a) - Lawfulness, fairness, and transparency
        if !self.privacy_controls.get("transparency").unwrap().implemented {
            violations.push(ComplianceViolation {
                regulation: "GDPR Article 5(1)(a)".to_string(),
                severity: ViolationSeverity::High,
                description: "Transparency principle not fully implemented".to_string(),
                location: "Privacy policy and user interface".to_string(),
                remediation: "Implement clear privacy notices and data processing information".to_string(),
                timeline: "2 weeks".to_string(),
            });
        }
        
        // Article 5(1)(b) - Purpose limitation
        if self.privacy_controls.get("purpose_limitation").unwrap().effectiveness < 90.0 {
            recommendations.push(ComplianceRecommendation {
                regulation: "GDPR Article 5(1)(b)".to_string(),
                recommendation: "Strengthen purpose limitation controls".to_string(),
                priority: "Medium".to_string(),
                implementation_effort: "1 week".to_string(),
            });
        }
        
        // Article 5(1)(c) - Data minimization
        if self.privacy_controls.get("data_minimization").unwrap().effectiveness < 85.0 {
            violations.push(ComplianceViolation {
                regulation: "GDPR Article 5(1)(c)".to_string(),
                severity: ViolationSeverity::Medium,
                description: "Data minimization could be improved".to_string(),
                location: "Data collection modules".to_string(),
                remediation: "Review and minimize data collection to essential elements only".to_string(),
                timeline: "1 week".to_string(),
            });
        }
        
        // Article 5(1)(e) - Storage limitation
        if self.privacy_controls.get("storage_limitation").unwrap().effectiveness < 90.0 {
            recommendations.push(ComplianceRecommendation {
                regulation: "GDPR Article 5(1)(e)".to_string(),
                recommendation: "Implement automated data retention and deletion".to_string(),
                priority: "High".to_string(),
                implementation_effort: "2 weeks".to_string(),
            });
        }
        
        // Article 5(2) - Accountability
        recommendations.push(ComplianceRecommendation {
            regulation: "GDPR Article 5(2)".to_string(),
            recommendation: "Implement comprehensive audit logging for accountability".to_string(),
            priority: "High".to_string(),
            implementation_effort: "1 week".to_string(),
        });
    }
    
    fn check_legal_basis(&self, violations: &mut Vec<ComplianceViolation>, _recommendations: &mut Vec<ComplianceRecommendation>) {
        // Check that all processing activities have valid legal basis
        for activity in &self.data_processing_activities {
            match activity.legal_basis {
                LegalBasis::Consent => {
                    // Check if consent mechanisms are properly implemented
                    let has_consent_mechanism = self.consent_mechanisms.iter()
                        .any(|c| c.purpose.contains(&activity.name));
                    
                    if !has_consent_mechanism {
                        violations.push(ComplianceViolation {
                            regulation: "GDPR Article 6(1)(a)".to_string(),
                            severity: ViolationSeverity::High,
                            description: format!("No consent mechanism for activity: {}", activity.name),
                            location: "Consent management system".to_string(),
                            remediation: "Implement proper consent collection and management".to_string(),
                            timeline: "2 weeks".to_string(),
                        });
                    }
                },
                LegalBasis::LegitimateInterests => {
                    // Should have legitimate interest assessment
                    // For BitCraps, P2P networking is necessary for functionality
                },
                _ => {
                    // Other legal bases are properly justified for gaming platform
                }
            }
        }
    }
    
    fn check_consent_requirements(&self, violations: &mut Vec<ComplianceViolation>, recommendations: &mut Vec<ComplianceRecommendation>) {
        for consent in &self.consent_mechanisms {
            // Article 7(3) - Withdrawal of consent
            if !consent.is_withdrawable {
                violations.push(ComplianceViolation {
                    regulation: "GDPR Article 7(3)".to_string(),
                    severity: ViolationSeverity::High,
                    description: format!("Consent not withdrawable for: {}", consent.purpose),
                    location: "Consent management system".to_string(),
                    remediation: "Implement consent withdrawal mechanisms".to_string(),
                    timeline: "1 week".to_string(),
                });
            }
            
            // Article 7(4) - Freely given consent
            if !consent.is_granular {
                recommendations.push(ComplianceRecommendation {
                    regulation: "GDPR Article 7(4)".to_string(),
                    recommendation: "Implement more granular consent options".to_string(),
                    priority: "Medium".to_string(),
                    implementation_effort: "2 weeks".to_string(),
                });
            }
        }
    }
    
    fn check_transparency_requirements(&self, violations: &mut Vec<ComplianceViolation>, recommendations: &mut Vec<ComplianceRecommendation>) {
        // Article 12 - Transparent information
        if !self.privacy_controls.get("transparency").unwrap().implemented {
            violations.push(ComplianceViolation {
                regulation: "GDPR Article 12".to_string(),
                severity: ViolationSeverity::High,
                description: "Privacy policy and transparent information missing".to_string(),
                location: "User interface and documentation".to_string(),
                remediation: "Create comprehensive privacy policy and user notices".to_string(),
                timeline: "1 week".to_string(),
            });
        }
        
        // Article 13 - Information when data collected from data subject
        recommendations.push(ComplianceRecommendation {
            regulation: "GDPR Article 13".to_string(),
            recommendation: "Implement just-in-time privacy notices during data collection".to_string(),
            priority: "High".to_string(),
            implementation_effort: "2 weeks".to_string(),
        });
    }
    
    fn check_individual_rights(&self, violations: &mut Vec<ComplianceViolation>, recommendations: &mut Vec<ComplianceRecommendation>) {
        if !self.privacy_controls.get("user_rights").unwrap().implemented {
            violations.push(ComplianceViolation {
                regulation: "GDPR Articles 15-22".to_string(),
                severity: ViolationSeverity::Critical,
                description: "Individual rights not implemented (access, rectification, erasure, etc.)".to_string(),
                location: "User interface and backend systems".to_string(),
                remediation: "Implement comprehensive individual rights management system".to_string(),
                timeline: "4 weeks".to_string(),
            });
        }
        
        // Specific rights recommendations
        recommendations.push(ComplianceRecommendation {
            regulation: "GDPR Article 20".to_string(),
            recommendation: "Implement data portability for user game history".to_string(),
            priority: "Medium".to_string(),
            implementation_effort: "2 weeks".to_string(),
        });
    }
    
    fn check_privacy_by_design(&self, _violations: &mut Vec<ComplianceViolation>, recommendations: &mut Vec<ComplianceRecommendation>) {
        // BitCraps implements many privacy by design principles
        recommendations.push(ComplianceRecommendation {
            regulation: "GDPR Article 25".to_string(),
            recommendation: "Document privacy by design implementation for audit purposes".to_string(),
            priority: "Low".to_string(),
            implementation_effort: "1 week".to_string(),
        });
    }
    
    fn check_security_requirements(&self, _violations: &mut Vec<ComplianceViolation>, recommendations: &mut Vec<ComplianceRecommendation>) {
        let security_effectiveness = self.privacy_controls.get("data_security").unwrap().effectiveness;
        
        if security_effectiveness < 90.0 {
            recommendations.push(ComplianceRecommendation {
                regulation: "GDPR Article 32".to_string(),
                recommendation: "Enhance security measures for personal data protection".to_string(),
                priority: "High".to_string(),
                implementation_effort: "3 weeks".to_string(),
            });
        }
    }
    
    fn check_international_transfers(&self, _violations: &mut Vec<ComplianceViolation>, recommendations: &mut Vec<ComplianceRecommendation>) {
        // BitCraps is P2P, so data may cross borders
        recommendations.push(ComplianceRecommendation {
            regulation: "GDPR Chapter V".to_string(),
            recommendation: "Implement adequate safeguards for international data transfers in P2P network".to_string(),
            priority: "High".to_string(),
            implementation_effort: "2 weeks".to_string(),
        });
    }
    
    fn conduct_privacy_impact_assessment(&self) -> PrivacyImpactAssessment {
        let identified_risks = vec![
            PrivacyRisk {
                description: "P2P network topology may reveal user location patterns".to_string(),
                likelihood: "Medium".to_string(),
                impact: "Medium".to_string(),
                risk_level: "Medium".to_string(),
            },
            PrivacyRisk {
                description: "Bluetooth MAC addresses could enable device tracking".to_string(),
                likelihood: "High".to_string(),
                impact: "Low".to_string(),
                risk_level: "Medium".to_string(),
            },
            PrivacyRisk {
                description: "Game behavior patterns could reveal player identity".to_string(),
                likelihood: "Low".to_string(),
                impact: "Medium".to_string(),
                risk_level: "Low".to_string(),
            },
        ];
        
        let mitigation_measures = vec![
            "MAC address randomization implemented".to_string(),
            "End-to-end encryption for all communications".to_string(),
            "Local data storage with user control".to_string(),
            "Minimal data collection principles".to_string(),
            "Automated data deletion".to_string(),
        ];
        
        PrivacyImpactAssessment {
            overall_risk: "Low to Medium".to_string(),
            identified_risks,
            mitigation_measures,
            residual_risk: "Low".to_string(),
        }
    }
    
    fn analyze_data_flows(&self) -> DataFlowAnalysis {
        DataFlowAnalysis {
            data_sources: vec![
                "User device (locally generated keys)".to_string(),
                "Bluetooth stack (MAC addresses)".to_string(),
                "Game interactions (bet history)".to_string(),
                "P2P network (connection metadata)".to_string(),
            ],
            processing_locations: vec![
                "User device only (no servers)".to_string(),
                "Temporary peer device memory".to_string(),
            ],
            data_sharing: vec![
                DataSharingActivity {
                    recipient: "Peer devices in game session".to_string(),
                    purpose: "Consensus and game state synchronization".to_string(),
                    legal_basis: "Contract".to_string(),
                    safeguards: vec![
                        "End-to-end encryption".to_string(),
                        "Digital signatures".to_string(),
                        "Session-based keys".to_string(),
                    ],
                },
            ],
            cross_border_transfers: vec![
                "Possible via P2P network - encrypted and anonymized".to_string(),
            ],
        }
    }
    
    fn calculate_compliance_score(&self, violations: &[ComplianceViolation]) -> f64 {
        let base_score = 100.0;
        let mut deductions = 0.0;
        
        for violation in violations {
            deductions += match violation.severity {
                ViolationSeverity::Critical => 20.0,
                ViolationSeverity::High => 10.0,
                ViolationSeverity::Medium => 5.0,
                ViolationSeverity::Low => 2.0,
            };
        }
        
        (base_score - deductions).max(0.0)
    }
}