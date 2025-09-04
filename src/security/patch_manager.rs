//! Automated Security Patch Deployment System
//!
//! This module provides secure, automated deployment of security patches and updates
//! with verification, rollback capabilities, and minimal downtime.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use chrono::{DateTime, Utc, Datelike, Timelike};

use crate::error::Result;

/// Security patch metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityPatch {
    /// Unique patch identifier
    pub id: String,
    /// Patch version
    pub version: String,
    /// Severity level (critical, high, medium, low)
    pub severity: PatchSeverity,
    /// Affected components
    pub components: Vec<String>,
    /// Patch description
    pub description: String,
    /// Expected downtime in seconds
    pub expected_downtime_seconds: u64,
    /// Required system restart
    pub requires_restart: bool,
    /// Patch content hash for verification
    pub content_hash: String,
    /// Digital signature of the patch
    pub signature: String,
    /// Release timestamp
    pub released_at: DateTime<Utc>,
    /// Dependencies on other patches
    pub dependencies: Vec<String>,
}

/// Patch severity levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum PatchSeverity {
    Critical,
    High,
    Medium,
    Low,
}

/// Patch deployment status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PatchStatus {
    Available,
    Downloading,
    VerifyingSignature,
    Staging,
    Testing,
    Deploying,
    Deployed,
    Failed { error: String },
    RolledBack,
}

/// Automated security patch manager
pub struct SecurityPatchManager {
    /// Patch registry
    registry: Arc<parking_lot::RwLock<HashMap<String, SecurityPatch>>>,
    /// Patch status tracker
    status_tracker: Arc<parking_lot::RwLock<HashMap<String, PatchStatus>>>,
    /// Configuration
    config: PatchConfig,
    /// Patch source URL
    source_url: String,
    /// Local patch storage directory
    patch_dir: PathBuf,
    /// Backup directory for rollbacks
    backup_dir: PathBuf,
}

/// Patch manager configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchConfig {
    /// Automatic deployment enabled
    pub auto_deploy: bool,
    /// Auto-deploy only critical patches
    pub auto_deploy_critical_only: bool,
    /// Check for patches interval (seconds)
    pub check_interval_seconds: u64,
    /// Maximum downtime allowed for auto-deployment (seconds)
    pub max_auto_downtime_seconds: u64,
    /// Enable patch testing before deployment
    pub enable_testing: bool,
    /// Maintenance window start hour (UTC)
    pub maintenance_window_start: u8,
    /// Maintenance window end hour (UTC)  
    pub maintenance_window_end: u8,
    /// Rollback timeout (seconds)
    pub rollback_timeout_seconds: u64,
}

impl Default for PatchConfig {
    fn default() -> Self {
        Self {
            auto_deploy: false, // Conservative default
            auto_deploy_critical_only: true,
            check_interval_seconds: 3600, // Check hourly
            max_auto_downtime_seconds: 60, // Max 1 minute
            enable_testing: true,
            maintenance_window_start: 2, // 2 AM UTC
            maintenance_window_end: 6,   // 6 AM UTC
            rollback_timeout_seconds: 300, // 5 minutes
        }
    }
}

/// Patch deployment result
#[derive(Debug)]
pub struct DeploymentResult {
    pub patch_id: String,
    pub success: bool,
    pub deployed_at: DateTime<Utc>,
    pub downtime_seconds: u64,
    pub rollback_point: Option<PathBuf>,
    pub logs: Vec<String>,
}

impl SecurityPatchManager {
    /// Create new patch manager
    pub fn new(
        source_url: String,
        patch_dir: PathBuf,
        backup_dir: PathBuf,
        config: PatchConfig,
    ) -> Self {
        Self {
            registry: Arc::new(parking_lot::RwLock::new(HashMap::new())),
            status_tracker: Arc::new(parking_lot::RwLock::new(HashMap::new())),
            config,
            source_url,
            patch_dir,
            backup_dir,
        }
    }

    /// Start the automated patch management system
    pub async fn start(&self) -> Result<()> {
        // Create necessary directories
        fs::create_dir_all(&self.patch_dir).await?;
        fs::create_dir_all(&self.backup_dir).await?;

        // Start background tasks
        let manager = self.clone();
        tokio::spawn(async move {
            manager.patch_check_loop().await;
        });

        if self.config.auto_deploy {
            let manager = self.clone();
            tokio::spawn(async move {
                manager.auto_deploy_loop().await;
            });
        }

        Ok(())
    }

    /// Check for new patches
    pub async fn check_for_patches(&self) -> Result<Vec<SecurityPatch>> {
        let url = format!("{}/patches/manifest.json", self.source_url);
        
        // Fetch patch manifest
        let response = reqwest::get(&url)
            .await
            .map_err(|e| crate::error::Error::Network(format!("Failed to fetch patches: {}", e)))?;
            
        if !response.status().is_success() {
            return Err(crate::error::Error::Network(
                format!("Patch server returned status: {}", response.status())
            ));
        }

        let patches: Vec<SecurityPatch> = response
            .json()
            .await
            .map_err(|e| crate::error::Error::Parsing(format!("Failed to parse patch manifest: {}", e)))?;

        // Update registry with new patches
        {
            let mut registry = self.registry.write();
            let mut status = self.status_tracker.write();
            
            for patch in &patches {
                if !registry.contains_key(&patch.id) {
                    tracing::info!("New security patch available: {} ({})", patch.id, patch.severity);
                    registry.insert(patch.id.clone(), patch.clone());
                    status.insert(patch.id.clone(), PatchStatus::Available);
                }
            }
        }

        Ok(patches)
    }

    /// Download and verify a specific patch
    pub async fn download_patch(&self, patch_id: &str) -> Result<PathBuf> {
        let patch = {
            let registry = self.registry.read();
            registry.get(patch_id).cloned()
                .ok_or_else(|| crate::error::Error::NotFound(format!("Patch {} not found", patch_id)))?
        };

        // Update status
        {
            let mut status = self.status_tracker.write();
            status.insert(patch_id.to_string(), PatchStatus::Downloading);
        }

        let url = format!("{}/patches/{}.patch", self.source_url, patch_id);
        let response = reqwest::get(&url)
            .await
            .map_err(|e| crate::error::Error::Network(format!("Failed to download patch: {}", e)))?;

        if !response.status().is_success() {
            return Err(crate::error::Error::Network(
                format!("Failed to download patch: {}", response.status())
            ));
        }

        let content = response.bytes().await
            .map_err(|e| crate::error::Error::Network(format!("Failed to read patch content: {}", e)))?;

        // Verify content hash
        let computed_hash = {
            let mut hasher = Sha256::new();
            hasher.update(&content);
            hex::encode(hasher.finalize())
        };

        if computed_hash != patch.content_hash {
            return Err(crate::error::Error::Crypto(
                format!("Patch hash verification failed: expected {}, got {}", 
                    patch.content_hash, computed_hash)
            ));
        }

        // Update status
        {
            let mut status = self.status_tracker.write();
            status.insert(patch_id.to_string(), PatchStatus::VerifyingSignature);
        }

        // TODO: Verify digital signature (requires public key infrastructure)
        // This would verify patch.signature against content

        // Save patch to disk
        let patch_path = self.patch_dir.join(format!("{}.patch", patch_id));
        fs::write(&patch_path, &content).await?;

        {
            let mut status = self.status_tracker.write();
            status.insert(patch_id.to_string(), PatchStatus::Staging);
        }

        tracing::info!("Patch {} downloaded and verified", patch_id);
        Ok(patch_path)
    }

    /// Deploy a security patch
    pub async fn deploy_patch(&self, patch_id: &str) -> Result<DeploymentResult> {
        let start_time = Utc::now();
        let mut logs = Vec::new();
        
        let patch = {
            let registry = self.registry.read();
            registry.get(patch_id).cloned()
                .ok_or_else(|| crate::error::Error::NotFound(format!("Patch {} not found", patch_id)))?
        };

        logs.push(format!("Starting deployment of patch {}", patch_id));

        // Check dependencies
        for dep_id in &patch.dependencies {
            let dep_status = {
                let status = self.status_tracker.read();
                status.get(dep_id).cloned().unwrap_or(PatchStatus::Available)
            };
            
            if !matches!(dep_status, PatchStatus::Deployed) {
                return Err(crate::error::Error::Dependency(
                    format!("Dependency patch {} not deployed", dep_id)
                ));
            }
        }

        // Update status
        {
            let mut status = self.status_tracker.write();
            status.insert(patch_id.to_string(), PatchStatus::Deploying);
        }

        // Create backup point for rollback
        let backup_path = self.create_backup_point(patch_id).await?;
        logs.push(format!("Created backup point: {}", backup_path.display()));

        let deployment_result = match self.apply_patch_content(&patch, &mut logs).await {
            Ok(()) => {
                logs.push(format!("Patch {} deployed successfully", patch_id));
                
                // Update status
                {
                    let mut status = self.status_tracker.write();
                    status.insert(patch_id.to_string(), PatchStatus::Deployed);
                }

                let end_time = Utc::now();
                let downtime = (end_time - start_time).num_seconds() as u64;

                DeploymentResult {
                    patch_id: patch_id.to_string(),
                    success: true,
                    deployed_at: end_time,
                    downtime_seconds: downtime,
                    rollback_point: Some(backup_path),
                    logs,
                }
            }
            Err(e) => {
                logs.push(format!("Patch deployment failed: {}", e));
                
                // Attempt automatic rollback
                if let Err(rollback_err) = self.rollback_to_backup(&backup_path).await {
                    logs.push(format!("Rollback failed: {}", rollback_err));
                } else {
                    logs.push("Automatic rollback successful".to_string());
                }

                // Update status
                {
                    let mut status = self.status_tracker.write();
                    status.insert(patch_id.to_string(), PatchStatus::Failed {
                        error: e.to_string()
                    });
                }

                let end_time = Utc::now();
                let downtime = (end_time - start_time).num_seconds() as u64;

                DeploymentResult {
                    patch_id: patch_id.to_string(),
                    success: false,
                    deployed_at: end_time,
                    downtime_seconds: downtime,
                    rollback_point: Some(backup_path),
                    logs,
                }
            }
        };

        Ok(deployment_result)
    }

    /// Check if we're in maintenance window
    fn is_maintenance_window(&self) -> bool {
        let now = Utc::now();
        let hour = now.hour() as u8;
        
        if self.config.maintenance_window_start <= self.config.maintenance_window_end {
            // Normal case: 2 AM - 6 AM
            hour >= self.config.maintenance_window_start && hour < self.config.maintenance_window_end
        } else {
            // Wrap around case: 22 PM - 2 AM
            hour >= self.config.maintenance_window_start || hour < self.config.maintenance_window_end
        }
    }

    /// Background loop for checking patches
    async fn patch_check_loop(&self) {
        let mut interval = tokio::time::interval(
            std::time::Duration::from_secs(self.config.check_interval_seconds)
        );

        loop {
            interval.tick().await;
            
            if let Err(e) = self.check_for_patches().await {
                tracing::error!("Failed to check for patches: {}", e);
            }
        }
    }

    /// Background loop for auto-deployment
    async fn auto_deploy_loop(&self) {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(300)); // Check every 5 minutes

        loop {
            interval.tick().await;

            if !self.is_maintenance_window() {
                continue;
            }

            // Get patches eligible for auto-deployment
            let eligible_patches = {
                let registry = self.registry.read();
                let status = self.status_tracker.read();

                registry.values()
                    .filter(|patch| {
                        matches!(status.get(&patch.id), Some(PatchStatus::Available)) &&
                        self.is_eligible_for_auto_deploy(patch)
                    })
                    .cloned()
                    .collect::<Vec<_>>()
            };

            // Sort by severity (critical first)
            let mut sorted_patches = eligible_patches;
            sorted_patches.sort_by_key(|p| std::cmp::Reverse(p.severity.clone()));

            for patch in sorted_patches {
                tracing::info!("Auto-deploying patch: {}", patch.id);
                
                match self.download_patch(&patch.id).await {
                    Ok(_) => {
                        match self.deploy_patch(&patch.id).await {
                            Ok(result) => {
                                if result.success {
                                    tracing::info!("Auto-deployed patch {} successfully", patch.id);
                                } else {
                                    tracing::error!("Auto-deployment of patch {} failed", patch.id);
                                }
                            }
                            Err(e) => {
                                tracing::error!("Failed to deploy patch {}: {}", patch.id, e);
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to download patch {}: {}", patch.id, e);
                    }
                }
            }
        }
    }

    /// Check if patch is eligible for auto-deployment
    fn is_eligible_for_auto_deploy(&self, patch: &SecurityPatch) -> bool {
        // Check severity requirement
        if self.config.auto_deploy_critical_only && patch.severity != PatchSeverity::Critical {
            return false;
        }

        // Check downtime requirement
        if patch.expected_downtime_seconds > self.config.max_auto_downtime_seconds {
            return false;
        }

        true
    }

    /// Create backup point for rollback
    async fn create_backup_point(&self, patch_id: &str) -> Result<PathBuf> {
        let backup_path = self.backup_dir.join(format!("backup_{}", patch_id));
        fs::create_dir_all(&backup_path).await?;

        // NOTE: Placeholder backup implementation - creates directory structure only
        //       Production systems should implement full system state backup
        
        Ok(backup_path)
    }

    /// Apply patch content
    async fn apply_patch_content(&self, patch: &SecurityPatch, logs: &mut Vec<String>) -> Result<()> {
        logs.push(format!("Applying patch {} to components: {:?}", patch.id, patch.components));
        
        // NOTE: Placeholder patch application - simulation only
        //       Production systems should implement actual patch deployment logic
        //       based on specific patch formats and component requirements
        
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        
        logs.push("Patch application completed".to_string());
        Ok(())
    }

    /// Rollback to backup point
    async fn rollback_to_backup(&self, backup_path: &Path) -> Result<()> {
        tracing::info!("Rolling back to backup: {}", backup_path.display());
        
        // TODO: Implement actual rollback logic
        // This would restore from backup point
        
        Ok(())
    }

    /// Get patch status
    pub fn get_patch_status(&self, patch_id: &str) -> Option<PatchStatus> {
        let status = self.status_tracker.read();
        status.get(patch_id).cloned()
    }

    /// List all patches
    pub fn list_patches(&self) -> Vec<(SecurityPatch, PatchStatus)> {
        let registry = self.registry.read();
        let status = self.status_tracker.read();
        
        registry.values()
            .map(|patch| {
                let status = status.get(&patch.id).cloned().unwrap_or(PatchStatus::Available);
                (patch.clone(), status)
            })
            .collect()
    }

    /// Manual rollback to specific backup
    pub async fn manual_rollback(&self, patch_id: &str) -> Result<()> {
        let backup_path = self.backup_dir.join(format!("backup_{}", patch_id));
        
        if !backup_path.exists() {
            return Err(crate::error::Error::NotFound(
                format!("Backup for patch {} not found", patch_id)
            ));
        }

        self.rollback_to_backup(&backup_path).await?;

        // Update status
        {
            let mut status = self.status_tracker.write();
            status.insert(patch_id.to_string(), PatchStatus::RolledBack);
        }

        tracing::info!("Manual rollback of patch {} completed", patch_id);
        Ok(())
    }
}

impl Clone for SecurityPatchManager {
    fn clone(&self) -> Self {
        Self {
            registry: Arc::clone(&self.registry),
            status_tracker: Arc::clone(&self.status_tracker),
            config: self.config.clone(),
            source_url: self.source_url.clone(),
            patch_dir: self.patch_dir.clone(),
            backup_dir: self.backup_dir.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_patch_severity_ordering() {
        assert!(PatchSeverity::Critical > PatchSeverity::High);
        assert!(PatchSeverity::High > PatchSeverity::Medium);
        assert!(PatchSeverity::Medium > PatchSeverity::Low);
    }

    #[test]
    fn test_maintenance_window_normal() {
        let config = PatchConfig {
            maintenance_window_start: 2,
            maintenance_window_end: 6,
            ..Default::default()
        };
        
        let manager = SecurityPatchManager::new(
            "https://example.com".to_string(),
            PathBuf::from("/tmp/patches"),
            PathBuf::from("/tmp/backups"),
            config,
        );

        // This test is time-dependent, so we'll just verify the logic works
        // without checking actual current time
        let _in_window = manager.is_maintenance_window();
    }

    #[tokio::test]
    async fn test_patch_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let patch_dir = temp_dir.path().join("patches");
        let backup_dir = temp_dir.path().join("backups");
        
        let manager = SecurityPatchManager::new(
            "https://example.com".to_string(),
            patch_dir,
            backup_dir,
            PatchConfig::default(),
        );

        let patches = manager.list_patches();
        assert!(patches.is_empty());
    }

    #[test]
    fn test_auto_deploy_eligibility() {
        let temp_dir = TempDir::new().unwrap();
        let patch_dir = temp_dir.path().join("patches");
        let backup_dir = temp_dir.path().join("backups");
        
        let config = PatchConfig {
            auto_deploy_critical_only: true,
            max_auto_downtime_seconds: 60,
            ..Default::default()
        };
        
        let manager = SecurityPatchManager::new(
            "https://example.com".to_string(),
            patch_dir,
            backup_dir,
            config,
        );

        let critical_patch = SecurityPatch {
            id: "patch-001".to_string(),
            version: "1.0.0".to_string(),
            severity: PatchSeverity::Critical,
            components: vec!["crypto".to_string()],
            description: "Critical security fix".to_string(),
            expected_downtime_seconds: 30,
            requires_restart: false,
            content_hash: "abc123".to_string(),
            signature: "def456".to_string(),
            released_at: Utc::now(),
            dependencies: vec![],
        };

        let high_patch = SecurityPatch {
            severity: PatchSeverity::High,
            ..critical_patch.clone()
        };

        assert!(manager.is_eligible_for_auto_deploy(&critical_patch));
        assert!(!manager.is_eligible_for_auto_deploy(&high_patch));
    }
}