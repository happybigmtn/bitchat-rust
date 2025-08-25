//! Audit Trail Management
//! 
//! Implements comprehensive audit logging and trail management
//! for compliance and security monitoring purposes.

use std::collections::HashMap;
use std::time::{SystemTime, Duration};
use serde::{Serialize, Deserialize};
use sha2::{Sha256, Digest};

#[derive(Debug, Clone)]
pub struct AuditTrailManager {
    audit_entries: Vec<AuditEntry>,
    integrity_chain: Vec<IntegrityLink>,
    compliance_events: HashMap<String, Vec<ComplianceEvent>>,
    security_events: Vec<SecurityEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditReport {
    pub summary: String,
    pub total_events: usize,
    pub compliance_events: usize,
    pub security_events: usize,
    pub integrity_verified: bool,
    pub time_range: TimeRange,
    pub key_findings: Vec<String>,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TimeRange {
    start: SystemTime,
    end: SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AuditEntry {
    id: String,
    timestamp: SystemTime,
    event_type: AuditEventType,
    actor: String,
    action: String,
    resource: String,
    outcome: AuditOutcome,
    details: HashMap<String, String>,
    hash: String,
    previous_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum AuditEventType {
    Authentication,
    Authorization,
    DataAccess,
    DataModification,
    SystemConfiguration,
    SecurityIncident,
    ComplianceCheck,
    PolicyViolation,
    UserAction,
    SystemEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum AuditOutcome {
    Success,
    Failure,
    Warning,
    Information,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct IntegrityLink {
    entry_id: String,
    hash: String,
    timestamp: SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ComplianceEvent {
    regulation: String,
    event_type: ComplianceEventType,
    description: String,
    timestamp: SystemTime,
    remediation_required: bool,
    deadline: Option<SystemTime>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum ComplianceEventType {
    DataCollection,
    DataProcessing,
    DataDeletion,
    ConsentGiven,
    ConsentWithdrawn,
    RightsExercised,
    PolicyUpdated,
    TrainingCompleted,
    AuditPerformed,
    ViolationDetected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SecurityEvent {
    event_id: String,
    severity: SecuritySeverity,
    category: SecurityCategory,
    description: String,
    source_ip: Option<String>,
    user_id: Option<String>,
    timestamp: SystemTime,
    mitigation_applied: bool,
    false_positive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum SecuritySeverity {
    Critical,
    High,
    Medium,
    Low,
    Informational,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum SecurityCategory {
    Authentication,
    Authorization,
    DataBreach,
    Malware,
    NetworkIntrusion,
    PolicyViolation,
    VulnerabilityExploit,
    DenialOfService,
    CryptoAttack,
    PhysicalSecurity,
}

impl AuditTrailManager {
    pub fn new() -> Self {
        Self {
            audit_entries: Vec::new(),
            integrity_chain: Vec::new(),
            compliance_events: HashMap::new(),
            security_events: Vec::new(),
        }
    }
    
    pub fn log_event(&mut self, 
                    event_type: AuditEventType,
                    actor: String,
                    action: String,
                    resource: String,
                    outcome: AuditOutcome,
                    details: HashMap<String, String>) -> Result<String, Box<dyn std::error::Error>> {
        
        let timestamp = SystemTime::now();
        let id = self.generate_event_id(&timestamp);
        
        // Get previous hash for chain integrity
        let previous_hash = self.audit_entries.last().map(|e| e.hash.clone());
        
        // Create entry content for hashing
        let entry_content = format!("{}:{}:{}:{}:{:?}:{:?}",
            id, actor, action, resource, event_type, outcome);
        
        // Calculate hash
        let mut hasher = Sha256::new();
        hasher.update(&entry_content);
        if let Some(prev_hash) = &previous_hash {
            hasher.update(prev_hash);
        }
        let hash = format!("{:x}", hasher.finalize());
        
        let entry = AuditEntry {
            id: id.clone(),
            timestamp,
            event_type,
            actor,
            action,
            resource,
            outcome,
            details,
            hash: hash.clone(),
            previous_hash,
        };
        
        self.audit_entries.push(entry);
        self.integrity_chain.push(IntegrityLink {
            entry_id: id.clone(),
            hash,
            timestamp,
        });
        
        Ok(id)
    }
    
    pub fn log_compliance_event(&mut self,
                               regulation: String,
                               event_type: ComplianceEventType,
                               description: String,
                               remediation_required: bool,
                               deadline: Option<SystemTime>) {
        
        let event = ComplianceEvent {
            regulation: regulation.clone(),
            event_type,
            description,
            timestamp: SystemTime::now(),
            remediation_required,
            deadline,
        };
        
        self.compliance_events.entry(regulation).or_insert_with(Vec::new).push(event);
    }
    
    pub fn log_security_event(&mut self,
                             severity: SecuritySeverity,
                             category: SecurityCategory,
                             description: String,
                             source_ip: Option<String>,
                             user_id: Option<String>,
                             mitigation_applied: bool) {
        
        let event_id = self.generate_event_id(&SystemTime::now());
        
        let event = SecurityEvent {
            event_id,
            severity,
            category,
            description,
            source_ip,
            user_id,
            timestamp: SystemTime::now(),
            mitigation_applied,
            false_positive: false,
        };
        
        self.security_events.push(event);
        
        // Also log as audit event
        let mut details = HashMap::new();
        details.insert("severity".to_string(), format!("{:?}", event.severity));
        details.insert("category".to_string(), format!("{:?}", event.category));
        if let Some(ip) = &event.source_ip {
            details.insert("source_ip".to_string(), ip.clone());
        }
        
        let _ = self.log_event(
            AuditEventType::SecurityIncident,
            user_id.unwrap_or_else(|| "system".to_string()),
            "security_event".to_string(),
            "security_system".to_string(),
            if mitigation_applied { AuditOutcome::Success } else { AuditOutcome::Warning },
            details,
        );
    }
    
    pub async fn generate_report(&self) -> Result<AuditReport, Box<dyn std::error::Error>> {
        let total_events = self.audit_entries.len();
        let compliance_events = self.compliance_events.values()
            .map(|events| events.len())
            .sum::<usize>();
        let security_events = self.security_events.len();
        
        let integrity_verified = self.verify_integrity_chain();
        
        let time_range = if let (Some(first), Some(last)) = (self.audit_entries.first(), self.audit_entries.last()) {
            TimeRange {
                start: first.timestamp,
                end: last.timestamp,
            }
        } else {
            let now = SystemTime::now();
            TimeRange { start: now, end: now }
        };
        
        let key_findings = self.generate_key_findings();
        let recommendations = self.generate_recommendations();
        
        let summary = format!(
            "Audit trail contains {} total events ({} compliance, {} security) over {} days. Integrity chain: {}.",
            total_events,
            compliance_events,
            security_events,
            self.calculate_days_in_range(&time_range),
            if integrity_verified { "VERIFIED" } else { "COMPROMISED" }
        );
        
        Ok(AuditReport {
            summary,
            total_events,
            compliance_events,
            security_events,
            integrity_verified,
            time_range,
            key_findings,
            recommendations,
        })
    }
    
    fn verify_integrity_chain(&self) -> bool {
        if self.audit_entries.is_empty() {
            return true;
        }
        
        let mut previous_hash: Option<String> = None;
        
        for entry in &self.audit_entries {
            // Recreate hash
            let entry_content = format!("{}:{}:{}:{}:{:?}:{:?}",
                entry.id, entry.actor, entry.action, entry.resource, entry.event_type, entry.outcome);
            
            let mut hasher = Sha256::new();
            hasher.update(&entry_content);
            if let Some(prev_hash) = &previous_hash {
                hasher.update(prev_hash);
            }
            let expected_hash = format!("{:x}", hasher.finalize());
            
            // Verify hash matches
            if entry.hash != expected_hash {
                return false;
            }
            
            // Verify chain linkage
            if entry.previous_hash != previous_hash {
                return false;
            }
            
            previous_hash = Some(entry.hash.clone());
        }
        
        true
    }
    
    fn generate_key_findings(&self) -> Vec<String> {
        let mut findings = Vec::new();
        
        // Authentication failures
        let auth_failures = self.audit_entries.iter()
            .filter(|e| matches!(e.event_type, AuditEventType::Authentication) && 
                       matches!(e.outcome, AuditOutcome::Failure))
            .count();
        
        if auth_failures > 0 {
            findings.push(format!("{} authentication failures detected", auth_failures));
        }
        
        // Critical security events
        let critical_security = self.security_events.iter()
            .filter(|e| matches!(e.severity, SecuritySeverity::Critical))
            .count();
        
        if critical_security > 0 {
            findings.push(format!("{} critical security events require attention", critical_security));
        }
        
        // Compliance violations
        let violations = self.compliance_events.values()
            .flat_map(|events| events.iter())
            .filter(|e| matches!(e.event_type, ComplianceEventType::ViolationDetected))
            .count();
        
        if violations > 0 {
            findings.push(format!("{} compliance violations detected", violations));
        }
        
        // Data access patterns
        let data_access = self.audit_entries.iter()
            .filter(|e| matches!(e.event_type, AuditEventType::DataAccess))
            .count();
        
        findings.push(format!("{} data access events logged", data_access));
        
        // System configuration changes
        let config_changes = self.audit_entries.iter()
            .filter(|e| matches!(e.event_type, AuditEventType::SystemConfiguration))
            .count();
        
        if config_changes > 0 {
            findings.push(format!("{} system configuration changes", config_changes));
        }
        
        findings
    }
    
    fn generate_recommendations(&self) -> Vec<String> {
        let mut recommendations = Vec::new();
        
        // Check for high-frequency authentication failures
        let recent_failures = self.audit_entries.iter()
            .filter(|e| {
                matches!(e.event_type, AuditEventType::Authentication) &&
                matches!(e.outcome, AuditOutcome::Failure) &&
                e.timestamp > SystemTime::now() - Duration::from_secs(24 * 60 * 60) // Last 24 hours
            })
            .count();
        
        if recent_failures > 10 {
            recommendations.push("Investigate authentication failure spike - potential brute force attack".to_string());
        }
        
        // Check for unmitigated security events
        let unmitigated = self.security_events.iter()
            .filter(|e| !e.mitigation_applied && 
                       matches!(e.severity, SecuritySeverity::High | SecuritySeverity::Critical))
            .count();
        
        if unmitigated > 0 {
            recommendations.push(format!("Address {} unmitigated high/critical security events", unmitigated));
        }
        
        // Check for overdue compliance actions
        let now = SystemTime::now();
        let overdue_compliance = self.compliance_events.values()
            .flat_map(|events| events.iter())
            .filter(|e| e.remediation_required && 
                       e.deadline.map_or(false, |d| d < now))
            .count();
        
        if overdue_compliance > 0 {
            recommendations.push(format!("Complete {} overdue compliance remediations", overdue_compliance));
        }
        
        // General recommendations
        recommendations.push("Implement automated log analysis and alerting".to_string());
        recommendations.push("Establish regular audit trail review schedule".to_string());
        recommendations.push("Configure real-time security event monitoring".to_string());
        
        recommendations
    }
    
    fn generate_event_id(&self, timestamp: &SystemTime) -> String {
        let duration = timestamp.duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0));
        
        format!("AE-{:016X}-{:04X}", 
                duration.as_secs(),
                self.audit_entries.len())
    }
    
    fn calculate_days_in_range(&self, range: &TimeRange) -> u64 {
        range.end.duration_since(range.start)
            .unwrap_or(Duration::from_secs(0))
            .as_secs() / (24 * 60 * 60)
    }
    
    // Simulation methods for testing
    pub fn simulate_bitcraps_events(&mut self) {
        // Simulate typical BitCraps application events
        
        // User authentication
        let mut details = HashMap::new();
        details.insert("method".to_string(), "cryptographic_key".to_string());
        let _ = self.log_event(
            AuditEventType::Authentication,
            "user_001".to_string(),
            "authenticate".to_string(),
            "app_login".to_string(),
            AuditOutcome::Success,
            details,
        );
        
        // Game session creation
        let mut details = HashMap::new();
        details.insert("game_type".to_string(), "craps".to_string());
        details.insert("max_players".to_string(), "6".to_string());
        let _ = self.log_event(
            AuditEventType::UserAction,
            "user_001".to_string(),
            "create_game_session".to_string(),
            "game_session_001".to_string(),
            AuditOutcome::Success,
            details,
        );
        
        // P2P network join
        let mut details = HashMap::new();
        details.insert("transport".to_string(), "bluetooth_le".to_string());
        details.insert("peers_discovered".to_string(), "3".to_string());
        let _ = self.log_event(
            AuditEventType::SystemEvent,
            "system".to_string(),
            "join_p2p_network".to_string(),
            "mesh_network".to_string(),
            AuditOutcome::Success,
            details,
        );
        
        // Data processing (GDPR event)
        self.log_compliance_event(
            "GDPR".to_string(),
            ComplianceEventType::DataProcessing,
            "Processing game session data for consensus".to_string(),
            false,
            None,
        );
        
        // Cryptographic operation
        let mut details = HashMap::new();
        details.insert("operation".to_string(), "signature_verification".to_string());
        details.insert("algorithm".to_string(), "Ed25519".to_string());
        let _ = self.log_event(
            AuditEventType::DataAccess,
            "consensus_engine".to_string(),
            "verify_transaction".to_string(),
            "transaction_001".to_string(),
            AuditOutcome::Success,
            details,
        );
        
        // Security event - rate limiting
        self.log_security_event(
            SecuritySeverity::Medium,
            SecurityCategory::PolicyViolation,
            "Connection rate limit exceeded".to_string(),
            Some("192.168.1.100".to_string()),
            None,
            true,
        );
        
        // CCPA compliance event
        self.log_compliance_event(
            "CCPA".to_string(),
            ComplianceEventType::RightsExercised,
            "User requested data deletion".to_string(),
            true,
            Some(SystemTime::now() + Duration::from_secs(45 * 24 * 60 * 60)), // 45 days
        );
    }
    
    pub fn get_events_by_type(&self, event_type: &AuditEventType) -> Vec<&AuditEntry> {
        self.audit_entries.iter()
            .filter(|e| std::mem::discriminant(&e.event_type) == std::mem::discriminant(event_type))
            .collect()
    }
    
    pub fn get_security_events_by_severity(&self, severity: &SecuritySeverity) -> Vec<&SecurityEvent> {
        self.security_events.iter()
            .filter(|e| std::mem::discriminant(&e.severity) == std::mem::discriminant(severity))
            .collect()
    }
    
    pub fn get_compliance_events_by_regulation(&self, regulation: &str) -> Option<&Vec<ComplianceEvent>> {
        self.compliance_events.get(regulation)
    }
}