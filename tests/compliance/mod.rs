//! Compliance Verification System
//! 
//! This module implements automated compliance testing for various
//! regulations including GDPR, CCPA, and security standards.

pub mod gdpr_compliance;
pub mod ccpa_compliance;
pub mod security_compliance;
pub mod audit_trail;
pub mod privacy_assessment;

use std::collections::HashMap;
use std::time::{SystemTime, Duration};
use serde::{Serialize, Deserialize};

/// Overall compliance framework coordinator
pub struct ComplianceFramework {
    pub gdpr: gdpr_compliance::GDPRComplianceChecker,
    pub ccpa: ccpa_compliance::CCPAComplianceChecker,
    pub security: security_compliance::SecurityComplianceChecker,
    pub audit_trail: audit_trail::AuditTrailManager,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceReport {
    pub timestamp: SystemTime,
    pub gdpr_score: f64,
    pub ccpa_score: f64,
    pub security_score: f64,
    pub overall_score: f64,
    pub violations: Vec<ComplianceViolation>,
    pub recommendations: Vec<ComplianceRecommendation>,
    pub audit_trail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceViolation {
    pub regulation: String,
    pub severity: ViolationSeverity,
    pub description: String,
    pub location: String,
    pub remediation: String,
    pub timeline: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ViolationSeverity {
    Critical,
    High, 
    Medium,
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceRecommendation {
    pub regulation: String,
    pub recommendation: String,
    pub priority: String,
    pub implementation_effort: String,
}

impl ComplianceFramework {
    pub fn new() -> Self {
        Self {
            gdpr: gdpr_compliance::GDPRComplianceChecker::new(),
            ccpa: ccpa_compliance::CCPAComplianceChecker::new(),
            security: security_compliance::SecurityComplianceChecker::new(),
            audit_trail: audit_trail::AuditTrailManager::new(),
        }
    }
    
    pub async fn run_comprehensive_compliance_check(&mut self) -> Result<ComplianceReport, Box<dyn std::error::Error>> {
        let start_time = SystemTime::now();
        println!("🔍 Starting comprehensive compliance verification...");
        
        // Run individual compliance checks
        let gdpr_result = self.gdpr.check_compliance().await?;
        let ccpa_result = self.ccpa.check_compliance().await?;
        let security_result = self.security.check_compliance().await?;
        
        // Collect all violations
        let mut violations = Vec::new();
        violations.extend(gdpr_result.violations);
        violations.extend(ccpa_result.violations);
        violations.extend(security_result.violations);
        
        // Collect all recommendations
        let mut recommendations = Vec::new();
        recommendations.extend(gdpr_result.recommendations);
        recommendations.extend(ccpa_result.recommendations);
        recommendations.extend(security_result.recommendations);
        
        // Calculate overall compliance score
        let overall_score = (gdpr_result.compliance_score + 
                           ccpa_result.compliance_score + 
                           security_result.compliance_score) / 3.0;
        
        // Generate audit trail
        let audit_trail = self.audit_trail.generate_report().await?;
        
        let report = ComplianceReport {
            timestamp: start_time,
            gdpr_score: gdpr_result.compliance_score,
            ccpa_score: ccpa_result.compliance_score, 
            security_score: security_result.compliance_score,
            overall_score,
            violations,
            recommendations,
            audit_trail: audit_trail.summary,
        };
        
        self.print_compliance_summary(&report);
        Ok(report)
    }
    
    fn print_compliance_summary(&self, report: &ComplianceReport) {
        println!("\n" + "=".repeat(70).as_str());
        println!("🛡️  COMPLIANCE VERIFICATION SUMMARY");
        println!("=".repeat(70));
        
        println!("\n📊 Compliance Scores:");
        println!("  GDPR Compliance: {:.1}%", report.gdpr_score);
        println!("  CCPA Compliance: {:.1}%", report.ccpa_score);
        println!("  Security Standards: {:.1}%", report.security_score);
        println!("  Overall Score: {:.1}%", report.overall_score);
        
        println!("\n⚠️  Violations Found:");
        let critical = report.violations.iter().filter(|v| v.severity == ViolationSeverity::Critical).count();
        let high = report.violations.iter().filter(|v| v.severity == ViolationSeverity::High).count();
        let medium = report.violations.iter().filter(|v| v.severity == ViolationSeverity::Medium).count();
        let low = report.violations.iter().filter(|v| v.severity == ViolationSeverity::Low).count();
        
        println!("  Critical: {}", critical);
        println!("  High: {}", high);
        println!("  Medium: {}", medium);
        println!("  Low: {}", low);
        
        if critical > 0 {
            println!("\n🚨 CRITICAL VIOLATIONS REQUIRE IMMEDIATE ATTENTION:");
            for violation in &report.violations {
                if violation.severity == ViolationSeverity::Critical {
                    println!("  • {} - {}", violation.regulation, violation.description);
                    println!("    Remediation: {}", violation.remediation);
                }
            }
        }
        
        println!("\n📋 Priority Recommendations:");
        for rec in report.recommendations.iter().take(5) {
            println!("  • [{}] {}", rec.regulation, rec.recommendation);
        }
        
        println!("\n" + "=".repeat(70).as_str());
    }
}