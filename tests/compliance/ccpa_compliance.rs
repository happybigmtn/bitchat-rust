//! CCPA Compliance Verification
//! 
//! Implements automated testing for California Consumer Privacy Act (CCPA)
//! compliance requirements for the BitCraps platform.

use super::{ComplianceViolation, ViolationSeverity, ComplianceRecommendation};
use std::collections::HashMap;
use std::time::SystemTime;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone)]
pub struct CCPAComplianceChecker {
    personal_information_categories: Vec<PICategory>,
    consumer_rights: HashMap<String, ConsumerRight>,
    business_purposes: Vec<BusinessPurpose>,
    third_party_disclosures: Vec<ThirdPartyDisclosure>,
    opt_out_mechanisms: Vec<OptOutMechanism>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CCPAComplianceResult {
    pub compliance_score: f64,
    pub violations: Vec<ComplianceViolation>,
    pub recommendations: Vec<ComplianceRecommendation>,
    pub consumer_rights_assessment: ConsumerRightsAssessment,
    pub disclosure_assessment: DisclosureAssessment,
}

#[derive(Debug, Clone)]
struct PICategory {
    name: String,
    ccpa_category: CCPACategory,
    collected: bool,
    source: String,
    business_purpose: String,
    shared_with_third_parties: bool,
    sold: bool,
}

#[derive(Debug, Clone)]
enum CCPACategory {
    Identifiers,                    // Device IDs, BLE addresses
    PersonalInfoRecords,           // Game history, preferences
    CharacteristicsProtected,      // None for BitCraps
    CommercialInformation,         // Gaming transactions
    BiometricInfo,                 // Optional: biometric authentication
    InternetActivity,              // P2P network activity
    Geolocation,                   // Optional: location-based features
    SensoryData,                   // None for BitCraps
    ProfessionalInfo,              // None for BitCraps
    NonPublicEducationInfo,        // None for BitCraps
    InferencesDrawn,               // Behavioral patterns
}

#[derive(Debug, Clone)]
struct ConsumerRight {
    name: String,
    implemented: bool,
    response_time: Option<std::time::Duration>,
    verification_method: Option<String>,
    free_of_charge: bool,
}

#[derive(Debug, Clone)]
struct BusinessPurpose {
    purpose: String,
    categories_processed: Vec<CCPACategory>,
    retention_period: Option<std::time::Duration>,
    legitimate_business_purpose: bool,
}

#[derive(Debug, Clone)]
struct ThirdPartyDisclosure {
    recipient: String,
    categories_disclosed: Vec<CCPACategory>,
    purpose: String,
    is_sale: bool,
}

#[derive(Debug, Clone)]
struct OptOutMechanism {
    method: String,
    easily_accessible: bool,
    clear_conspicuous: bool,
    implemented: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsumerRightsAssessment {
    pub right_to_know_implemented: bool,
    pub right_to_delete_implemented: bool,
    pub right_to_opt_out_implemented: bool,
    pub right_to_non_discrimination_implemented: bool,
    pub verification_methods: Vec<String>,
    pub response_timeframes: HashMap<String, u32>, // days
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisclosureAssessment {
    pub privacy_policy_complete: bool,
    pub at_collection_notice_implemented: bool,
    pub categories_disclosed: Vec<String>,
    pub business_purposes_disclosed: Vec<String>,
    pub third_party_sharing_disclosed: bool,
    pub sale_disclosure_complete: bool,
}

impl CCPAComplianceChecker {
    pub fn new() -> Self {
        let mut checker = Self {
            personal_information_categories: Vec::new(),
            consumer_rights: HashMap::new(),
            business_purposes: Vec::new(),
            third_party_disclosures: Vec::new(),
            opt_out_mechanisms: Vec::new(),
        };
        
        checker.initialize_pi_categories();
        checker.initialize_consumer_rights();
        checker.initialize_business_purposes();
        checker.initialize_third_party_disclosures();
        checker.initialize_opt_out_mechanisms();
        
        checker
    }
    
    fn initialize_pi_categories(&mut self) {
        // Device and network identifiers
        self.personal_information_categories.push(PICategory {
            name: "Device Identifiers".to_string(),
            ccpa_category: CCPACategory::Identifiers,
            collected: true,
            source: "User device and Bluetooth stack".to_string(),
            business_purpose: "P2P network communication and game session management".to_string(),
            shared_with_third_parties: true, // Shared with peer devices
            sold: false,
        });
        
        // Game transaction data
        self.personal_information_categories.push(PICategory {
            name: "Gaming Transaction Records".to_string(),
            ccpa_category: CCPACategory::CommercialInformation,
            collected: true,
            source: "User gaming interactions".to_string(),
            business_purpose: "Game state consensus and payout calculation".to_string(),
            shared_with_third_parties: true, // Shared with game participants
            sold: false,
        });
        
        // Network activity information
        self.personal_information_categories.push(PICategory {
            name: "P2P Network Activity".to_string(),
            ccpa_category: CCPACategory::InternetActivity,
            collected: true,
            source: "Network stack and mesh routing".to_string(),
            business_purpose: "Network topology management and routing".to_string(),
            shared_with_third_parties: true, // P2P network participants
            sold: false,
        });
        
        // Behavioral inferences
        self.personal_information_categories.push(PICategory {
            name: "Gaming Behavior Patterns".to_string(),
            ccpa_category: CCPACategory::InferencesDrawn,
            collected: false, // Not currently collected
            source: "Game play analysis".to_string(),
            business_purpose: "Anti-fraud and game balance optimization".to_string(),
            shared_with_third_parties: false,
            sold: false,
        });
        
        // Optional biometric data
        self.personal_information_categories.push(PICategory {
            name: "Biometric Authentication Data".to_string(),
            ccpa_category: CCPACategory::BiometricInfo,
            collected: false, // Optional feature
            source: "Device biometric sensors".to_string(),
            business_purpose: "Secure authentication and key access".to_string(),
            shared_with_third_parties: false,
            sold: false,
        });
        
        // Optional location data
        self.personal_information_categories.push(PICategory {
            name: "Approximate Location".to_string(),
            ccpa_category: CCPACategory::Geolocation,
            collected: false, // Optional for compliance/legal features
            source: "Device location services".to_string(),
            business_purpose: "Jurisdictional compliance and local regulations".to_string(),
            shared_with_third_parties: false,
            sold: false,
        });
    }
    
    fn initialize_consumer_rights(&mut self) {
        // Right to Know (CCPA Section 1798.100)
        self.consumer_rights.insert("right_to_know".to_string(), ConsumerRight {
            name: "Right to Know About Personal Information Collected".to_string(),
            implemented: false, // Need to implement
            response_time: Some(std::time::Duration::from_secs(45 * 24 * 60 * 60)), // 45 days
            verification_method: Some("Cryptographic key verification".to_string()),
            free_of_charge: true,
        });
        
        // Right to Delete (CCPA Section 1798.105)
        self.consumer_rights.insert("right_to_delete".to_string(), ConsumerRight {
            name: "Right to Delete Personal Information".to_string(),
            implemented: false, // Need to implement
            response_time: Some(std::time::Duration::from_secs(45 * 24 * 60 * 60)), // 45 days
            verification_method: Some("Cryptographic key verification".to_string()),
            free_of_charge: true,
        });
        
        // Right to Opt-Out of Sale (CCPA Section 1798.120)
        self.consumer_rights.insert("right_to_opt_out".to_string(), ConsumerRight {
            name: "Right to Opt-Out of Sale".to_string(),
            implemented: true, // No sale of PI occurs
            response_time: Some(std::time::Duration::from_secs(15 * 24 * 60 * 60)), // 15 days
            verification_method: None, // No verification required for opt-out
            free_of_charge: true,
        });
        
        // Right to Non-Discrimination (CCPA Section 1798.125)
        self.consumer_rights.insert("right_to_non_discrimination".to_string(), ConsumerRight {
            name: "Right to Non-Discrimination".to_string(),
            implemented: true, // No discrimination for exercising rights
            response_time: None, // Immediate
            verification_method: None,
            free_of_charge: true,
        });
    }
    
    fn initialize_business_purposes(&mut self) {
        // P2P networking
        self.business_purposes.push(BusinessPurpose {
            purpose: "Peer-to-peer network communication".to_string(),
            categories_processed: vec![
                CCPACategory::Identifiers,
                CCPACategory::InternetActivity,
            ],
            retention_period: Some(std::time::Duration::from_secs(24 * 60 * 60)), // 24 hours
            legitimate_business_purpose: true,
        });
        
        // Game session management
        self.business_purposes.push(BusinessPurpose {
            purpose: "Gaming session management and consensus".to_string(),
            categories_processed: vec![
                CCPACategory::Identifiers,
                CCPACategory::CommercialInformation,
            ],
            retention_period: Some(std::time::Duration::from_secs(30 * 24 * 60 * 60)), // 30 days
            legitimate_business_purpose: true,
        });
        
        // Security and fraud prevention
        self.business_purposes.push(BusinessPurpose {
            purpose: "Security, fraud prevention, and system integrity".to_string(),
            categories_processed: vec![
                CCPACategory::Identifiers,
                CCPACategory::InternetActivity,
                CCPACategory::InferencesDrawn,
            ],
            retention_period: Some(std::time::Duration::from_secs(90 * 24 * 60 * 60)), // 90 days
            legitimate_business_purpose: true,
        });
    }
    
    fn initialize_third_party_disclosures(&mut self) {
        // Peer devices in gaming sessions
        self.third_party_disclosures.push(ThirdPartyDisclosure {
            recipient: "Peer devices in gaming session".to_string(),
            categories_disclosed: vec![
                CCPACategory::Identifiers,
                CCPACategory::CommercialInformation,
            ],
            purpose: "Game consensus and state synchronization".to_string(),
            is_sale: false, // Not a sale under CCPA
        });
        
        // P2P network participants
        self.third_party_disclosures.push(ThirdPartyDisclosure {
            recipient: "P2P network participants".to_string(),
            categories_disclosed: vec![
                CCPACategory::Identifiers,
                CCPACategory::InternetActivity,
            ],
            purpose: "Network routing and topology management".to_string(),
            is_sale: false, // Not a sale under CCPA
        });
    }
    
    fn initialize_opt_out_mechanisms(&mut self) {
        // No sale opt-out (not applicable but good practice)
        self.opt_out_mechanisms.push(OptOutMechanism {
            method: "In-app settings toggle".to_string(),
            easily_accessible: true,
            clear_conspicuous: true,
            implemented: true,
        });
        
        // Data sharing opt-out
        self.opt_out_mechanisms.push(OptOutMechanism {
            method: "Privacy settings in application".to_string(),
            easily_accessible: false, // Need to implement
            clear_conspicuous: false, // Need to implement
            implemented: false,
        });
    }
    
    pub async fn check_compliance(&self) -> Result<CCPAComplianceResult, Box<dyn std::error::Error>> {
        let mut violations = Vec::new();
        let mut recommendations = Vec::new();
        
        println!("🏛️ Checking CCPA compliance...");
        
        // Check Section 1798.100 - Right to Know
        self.check_right_to_know(&mut violations, &mut recommendations);
        
        // Check Section 1798.105 - Right to Delete
        self.check_right_to_delete(&mut violations, &mut recommendations);
        
        // Check Section 1798.110 - Right to Know Categories
        self.check_category_disclosure(&mut violations, &mut recommendations);
        
        // Check Section 1798.115 - Right to Know Specific Information
        self.check_specific_info_disclosure(&mut violations, &mut recommendations);
        
        // Check Section 1798.120 - Right to Opt-Out
        self.check_right_to_opt_out(&mut violations, &mut recommendations);
        
        // Check Section 1798.125 - Non-Discrimination
        self.check_non_discrimination(&mut violations, &mut recommendations);
        
        // Check Section 1798.130 - Privacy Policy Requirements
        self.check_privacy_policy_requirements(&mut violations, &mut recommendations);
        
        // Check Section 1798.135 - Opt-Out Methods
        self.check_opt_out_methods(&mut violations, &mut recommendations);
        
        let compliance_score = self.calculate_compliance_score(&violations);
        let consumer_rights_assessment = self.assess_consumer_rights();
        let disclosure_assessment = self.assess_disclosures();
        
        println!("  ✅ CCPA compliance check completed. Score: {:.1}%", compliance_score);
        
        Ok(CCPAComplianceResult {
            compliance_score,
            violations,
            recommendations,
            consumer_rights_assessment,
            disclosure_assessment,
        })
    }
    
    fn check_right_to_know(&self, violations: &mut Vec<ComplianceViolation>, _recommendations: &mut Vec<ComplianceRecommendation>) {
        let right_to_know = self.consumer_rights.get("right_to_know").unwrap();
        
        if !right_to_know.implemented {
            violations.push(ComplianceViolation {
                regulation: "CCPA Section 1798.100".to_string(),
                severity: ViolationSeverity::High,
                description: "Right to know about personal information collection not implemented".to_string(),
                location: "Consumer rights management system".to_string(),
                remediation: "Implement mechanism for consumers to request information about PI collection".to_string(),
                timeline: "3 weeks".to_string(),
            });
        }
    }
    
    fn check_right_to_delete(&self, violations: &mut Vec<ComplianceViolation>, _recommendations: &mut Vec<ComplianceRecommendation>) {
        let right_to_delete = self.consumer_rights.get("right_to_delete").unwrap();
        
        if !right_to_delete.implemented {
            violations.push(ComplianceViolation {
                regulation: "CCPA Section 1798.105".to_string(),
                severity: ViolationSeverity::Critical,
                description: "Right to delete personal information not implemented".to_string(),
                location: "Data management and consumer rights system".to_string(),
                remediation: "Implement secure deletion mechanism for all personal information".to_string(),
                timeline: "4 weeks".to_string(),
            });
        }
    }
    
    fn check_category_disclosure(&self, violations: &mut Vec<ComplianceViolation>, recommendations: &mut Vec<ComplianceRecommendation>) {
        // Must disclose categories of PI collected, sources, and business purposes
        let has_complete_disclosure = self.personal_information_categories.iter()
            .all(|cat| !cat.collected || !cat.business_purpose.is_empty());
        
        if !has_complete_disclosure {
            violations.push(ComplianceViolation {
                regulation: "CCPA Section 1798.110".to_string(),
                severity: ViolationSeverity::Medium,
                description: "Incomplete disclosure of PI categories and business purposes".to_string(),
                location: "Privacy policy and notices".to_string(),
                remediation: "Complete disclosure of all PI categories, sources, and business purposes".to_string(),
                timeline: "2 weeks".to_string(),
            });
        }
        
        recommendations.push(ComplianceRecommendation {
            regulation: "CCPA Section 1798.110".to_string(),
            recommendation: "Implement detailed PI category tracking and disclosure automation".to_string(),
            priority: "Medium".to_string(),
            implementation_effort: "2 weeks".to_string(),
        });
    }
    
    fn check_specific_info_disclosure(&self, violations: &mut Vec<ComplianceViolation>, _recommendations: &mut Vec<ComplianceRecommendation>) {
        // Must be able to provide specific pieces of PI upon request
        let right_to_know = self.consumer_rights.get("right_to_know").unwrap();
        
        if !right_to_know.implemented {
            violations.push(ComplianceViolation {
                regulation: "CCPA Section 1798.115".to_string(),
                severity: ViolationSeverity::High,
                description: "Cannot provide specific PI upon consumer request".to_string(),
                location: "Data retrieval and consumer rights system".to_string(),
                remediation: "Implement system to retrieve and provide specific PI to consumers".to_string(),
                timeline: "3 weeks".to_string(),
            });
        }
    }
    
    fn check_right_to_opt_out(&self, _violations: &mut Vec<ComplianceViolation>, recommendations: &mut Vec<ComplianceRecommendation>) {
        // BitCraps doesn't sell PI, so this is mostly compliant by design
        let has_clear_opt_out = self.opt_out_mechanisms.iter()
            .any(|opt| opt.implemented && opt.easily_accessible);
        
        if !has_clear_opt_out {
            recommendations.push(ComplianceRecommendation {
                regulation: "CCPA Section 1798.120".to_string(),
                recommendation: "Implement clear opt-out mechanisms for data sharing preferences".to_string(),
                priority: "Medium".to_string(),
                implementation_effort: "2 weeks".to_string(),
            });
        }
    }
    
    fn check_non_discrimination(&self, _violations: &mut Vec<ComplianceViolation>, recommendations: &mut Vec<ComplianceRecommendation>) {
        // BitCraps design doesn't discriminate based on privacy choices
        recommendations.push(ComplianceRecommendation {
            regulation: "CCPA Section 1798.125".to_string(),
            recommendation: "Document non-discrimination policies in privacy notices".to_string(),
            priority: "Low".to_string(),
            implementation_effort: "1 week".to_string(),
        });
    }
    
    fn check_privacy_policy_requirements(&self, violations: &mut Vec<ComplianceViolation>, _recommendations: &mut Vec<ComplianceRecommendation>) {
        // Must have comprehensive privacy policy with all required elements
        violations.push(ComplianceViolation {
            regulation: "CCPA Section 1798.130".to_string(),
            severity: ViolationSeverity::High,
            description: "CCPA-compliant privacy policy not implemented".to_string(),
            location: "Privacy policy and user documentation".to_string(),
            remediation: "Create comprehensive CCPA-compliant privacy policy with all required disclosures".to_string(),
            timeline: "2 weeks".to_string(),
        });
    }
    
    fn check_opt_out_methods(&self, violations: &mut Vec<ComplianceViolation>, _recommendations: &mut Vec<ComplianceRecommendation>) {
        let has_proper_opt_out = self.opt_out_mechanisms.iter()
            .any(|opt| opt.implemented && opt.clear_conspicuous && opt.easily_accessible);
        
        if !has_proper_opt_out {
            violations.push(ComplianceViolation {
                regulation: "CCPA Section 1798.135".to_string(),
                severity: ViolationSeverity::Medium,
                description: "Opt-out methods not easily accessible or clearly conspicuous".to_string(),
                location: "User interface and privacy settings".to_string(),
                remediation: "Implement clear, easily accessible opt-out mechanisms".to_string(),
                timeline: "2 weeks".to_string(),
            });
        }
    }
    
    fn assess_consumer_rights(&self) -> ConsumerRightsAssessment {
        let mut response_timeframes = HashMap::new();
        
        for (key, right) in &self.consumer_rights {
            if let Some(response_time) = right.response_time {
                response_timeframes.insert(
                    key.clone(),
                    (response_time.as_secs() / (24 * 60 * 60)) as u32, // Convert to days
                );
            }
        }
        
        ConsumerRightsAssessment {
            right_to_know_implemented: self.consumer_rights.get("right_to_know").unwrap().implemented,
            right_to_delete_implemented: self.consumer_rights.get("right_to_delete").unwrap().implemented,
            right_to_opt_out_implemented: self.consumer_rights.get("right_to_opt_out").unwrap().implemented,
            right_to_non_discrimination_implemented: self.consumer_rights.get("right_to_non_discrimination").unwrap().implemented,
            verification_methods: vec![
                "Cryptographic key verification".to_string(),
                "Device-based authentication".to_string(),
            ],
            response_timeframes,
        }
    }
    
    fn assess_disclosures(&self) -> DisclosureAssessment {
        let categories_disclosed: Vec<String> = self.personal_information_categories.iter()
            .filter(|cat| cat.collected)
            .map(|cat| format!("{:?}", cat.ccpa_category))
            .collect();
        
        let business_purposes_disclosed: Vec<String> = self.business_purposes.iter()
            .map(|purpose| purpose.purpose.clone())
            .collect();
        
        DisclosureAssessment {
            privacy_policy_complete: false, // Need to implement
            at_collection_notice_implemented: false, // Need to implement
            categories_disclosed,
            business_purposes_disclosed,
            third_party_sharing_disclosed: !self.third_party_disclosures.is_empty(),
            sale_disclosure_complete: true, // No sale occurs
        }
    }
    
    fn calculate_compliance_score(&self, violations: &[ComplianceViolation]) -> f64 {
        let base_score = 100.0;
        let mut deductions = 0.0;
        
        for violation in violations {
            deductions += match violation.severity {
                ViolationSeverity::Critical => 15.0,
                ViolationSeverity::High => 10.0,
                ViolationSeverity::Medium => 5.0,
                ViolationSeverity::Low => 2.0,
            };
        }
        
        // Bonus for privacy-by-design features
        let privacy_bonus = 10.0; // BitCraps has strong privacy features
        
        (base_score - deductions + privacy_bonus).min(100.0).max(0.0)
    }
}