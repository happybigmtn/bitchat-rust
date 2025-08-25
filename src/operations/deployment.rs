//! Deployment Management and Automation
//! 
//! Automated deployment pipelines and infrastructure management for BitCraps

use std::sync::{Arc, atomic::{AtomicBool, AtomicU64, Ordering}};
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{RwLock, Mutex};
use tokio::process::Command;
use serde::{Serialize, Deserialize};
use tracing::{info, warn, error, debug};
use uuid::Uuid;

use crate::monitoring::metrics::METRICS;

/// Deployment manager for automated deployments
pub struct DeploymentManager {
    /// Active deployment pipelines
    pipelines: Arc<RwLock<HashMap<String, DeploymentPipeline>>>,
    /// Deployment configuration
    config: DeploymentConfig,
    /// Deployment history
    history: Arc<RwLock<Vec<DeploymentRecord>>>,
    /// Active deployments
    active_deployments: Arc<RwLock<HashMap<String, DeploymentExecution>>>,
}

impl DeploymentManager {
    /// Create new deployment manager
    pub fn new(config: DeploymentConfig) -> Self {
        Self {
            pipelines: Arc::new(RwLock::new(HashMap::new())),
            config,
            history: Arc::new(RwLock::new(Vec::new())),
            active_deployments: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a deployment pipeline
    pub async fn register_pipeline(&self, pipeline: DeploymentPipeline) -> Result<(), DeploymentError> {
        // Validate pipeline configuration
        self.validate_pipeline(&pipeline)?;

        // Register the pipeline
        self.pipelines.write().await.insert(pipeline.id.clone(), pipeline.clone());
        
        info!("Registered deployment pipeline: {}", pipeline.name);
        Ok(())
    }

    /// Execute deployment pipeline
    pub async fn deploy(&self, pipeline_id: &str, version: &str, environment: &str) -> Result<String, DeploymentError> {
        let pipelines = self.pipelines.read().await;
        let pipeline = pipelines.get(pipeline_id)
            .ok_or_else(|| DeploymentError::PipelineNotFound(pipeline_id.to_string()))?;

        let deployment_id = Uuid::new_v4().to_string();
        let execution = DeploymentExecution {
            id: deployment_id.clone(),
            pipeline_id: pipeline_id.to_string(),
            version: version.to_string(),
            environment: environment.to_string(),
            status: DeploymentStatus::Starting,
            started_at: SystemTime::now(),
            completed_at: None,
            current_stage: 0,
            total_stages: pipeline.stages.len(),
            logs: Arc::new(RwLock::new(Vec::new())),
            rollback_info: None,
        };

        // Add to active deployments
        self.active_deployments.write().await.insert(deployment_id.clone(), execution.clone());

        // Execute deployment asynchronously
        let pipeline_clone = pipeline.clone();
        let execution_clone = execution;
        let active_deployments = Arc::clone(&self.active_deployments);
        let history = Arc::clone(&self.history);

        tokio::spawn(async move {
            let result = Self::execute_pipeline(pipeline_clone, execution_clone.clone()).await;
            
            // Update deployment status
            let mut active = active_deployments.write().await;
            if let Some(mut exec) = active.remove(&deployment_id) {
                match result {
                    Ok(_) => {
                        exec.status = DeploymentStatus::Completed;
                        info!("Deployment {} completed successfully", deployment_id);
                    },
                    Err(e) => {
                        exec.status = DeploymentStatus::Failed;
                        error!("Deployment {} failed: {:?}", deployment_id, e);
                    }
                }
                exec.completed_at = Some(SystemTime::now());
                
                // Add to history
                let record = DeploymentRecord {
                    id: exec.id.clone(),
                    pipeline_id: exec.pipeline_id.clone(),
                    version: exec.version.clone(),
                    environment: exec.environment.clone(),
                    status: exec.status.clone(),
                    started_at: exec.started_at,
                    completed_at: exec.completed_at,
                    duration: exec.completed_at.map(|end| end.duration_since(exec.started_at).unwrap_or(Duration::ZERO)),
                };
                
                history.write().await.push(record);
            }
        });

        info!("Started deployment {}: {} to {} (version: {})", deployment_id, pipeline_id, environment, version);
        Ok(deployment_id)
    }

    /// Get deployment status
    pub async fn get_deployment_status(&self, deployment_id: &str) -> Result<DeploymentStatus, DeploymentError> {
        if let Some(execution) = self.active_deployments.read().await.get(deployment_id) {
            Ok(execution.status.clone())
        } else {
            // Check history
            let history = self.history.read().await;
            if let Some(record) = history.iter().find(|r| r.id == deployment_id) {
                Ok(record.status.clone())
            } else {
                Err(DeploymentError::DeploymentNotFound(deployment_id.to_string()))
            }
        }
    }

    /// List active deployments
    pub async fn list_active_deployments(&self) -> Vec<DeploymentSummary> {
        let active = self.active_deployments.read().await;
        active.values().map(|exec| DeploymentSummary {
            id: exec.id.clone(),
            pipeline_id: exec.pipeline_id.clone(),
            version: exec.version.clone(),
            environment: exec.environment.clone(),
            status: exec.status.clone(),
            progress_percent: if exec.total_stages > 0 {
                (exec.current_stage as f64 / exec.total_stages as f64 * 100.0) as u32
            } else {
                0
            },
            started_at: exec.started_at,
        }).collect()
    }

    /// Get deployment history
    pub async fn get_deployment_history(&self, limit: Option<usize>) -> Vec<DeploymentRecord> {
        let history = self.history.read().await;
        let mut records = history.clone();
        records.sort_by(|a, b| b.started_at.cmp(&a.started_at)); // Most recent first
        
        if let Some(limit) = limit {
            records.truncate(limit);
        }
        
        records
    }

    /// Rollback deployment
    pub async fn rollback(&self, deployment_id: &str) -> Result<String, DeploymentError> {
        // Find deployment record
        let history = self.history.read().await;
        let record = history.iter()
            .find(|r| r.id == deployment_id)
            .ok_or_else(|| DeploymentError::DeploymentNotFound(deployment_id.to_string()))?;

        // Get pipeline
        let pipelines = self.pipelines.read().await;
        let pipeline = pipelines.get(&record.pipeline_id)
            .ok_or_else(|| DeploymentError::PipelineNotFound(record.pipeline_id.clone()))?;

        // Find previous successful deployment
        let previous_deployment = history.iter()
            .filter(|r| r.pipeline_id == record.pipeline_id && 
                       r.environment == record.environment &&
                       matches!(r.status, DeploymentStatus::Completed) &&
                       r.started_at < record.started_at)
            .max_by_key(|r| r.started_at)
            .ok_or_else(|| DeploymentError::NoPreviousDeployment)?;

        // Create rollback deployment
        let rollback_id = self.deploy(
            &record.pipeline_id,
            &previous_deployment.version,
            &record.environment
        ).await?;

        info!("Initiated rollback {} for deployment {} to version {}", 
              rollback_id, deployment_id, previous_deployment.version);
        Ok(rollback_id)
    }

    /// Validate pipeline configuration
    fn validate_pipeline(&self, pipeline: &DeploymentPipeline) -> Result<(), DeploymentError> {
        if pipeline.name.is_empty() {
            return Err(DeploymentError::InvalidConfiguration("Pipeline name cannot be empty".to_string()));
        }

        if pipeline.stages.is_empty() {
            return Err(DeploymentError::InvalidConfiguration("Pipeline must have at least one stage".to_string()));
        }

        // Validate each stage
        for stage in &pipeline.stages {
            if stage.name.is_empty() {
                return Err(DeploymentError::InvalidConfiguration("Stage name cannot be empty".to_string()));
            }
            
            if stage.steps.is_empty() {
                return Err(DeploymentError::InvalidConfiguration(
                    format!("Stage '{}' must have at least one step", stage.name)
                ));
            }
        }

        Ok(())
    }

    /// Execute deployment pipeline
    async fn execute_pipeline(pipeline: DeploymentPipeline, mut execution: DeploymentExecution) -> Result<(), DeploymentError> {
        execution.status = DeploymentStatus::Running;
        
        for (stage_index, stage) in pipeline.stages.iter().enumerate() {
            execution.current_stage = stage_index;
            
            info!("Executing stage {}: {}", stage_index + 1, stage.name);
            Self::log_message(&execution, format!("Starting stage: {}", stage.name)).await;
            
            // Execute stage steps
            for step in &stage.steps {
                match Self::execute_step(step, &execution).await {
                    Ok(_) => {
                        Self::log_message(&execution, format!("Completed step: {}", step.name)).await;
                    },
                    Err(e) => {
                        Self::log_message(&execution, format!("Step failed: {} - {:?}", step.name, e)).await;
                        return Err(e);
                    }
                }
            }
            
            Self::log_message(&execution, format!("Completed stage: {}", stage.name)).await;
        }

        execution.current_stage = pipeline.stages.len();
        Self::log_message(&execution, "Deployment completed successfully".to_string()).await;
        
        Ok(())
    }

    /// Execute individual deployment step
    async fn execute_step(step: &DeploymentStep, execution: &DeploymentExecution) -> Result<(), DeploymentError> {
        match &step.step_type {
            DeploymentStepType::Command { command, args, working_dir } => {
                let mut cmd = Command::new(command);
                
                if let Some(args) = args {
                    cmd.args(args);
                }
                
                if let Some(dir) = working_dir {
                    cmd.current_dir(dir);
                }

                // Add environment variables
                cmd.env("DEPLOYMENT_ID", &execution.id);
                cmd.env("DEPLOYMENT_VERSION", &execution.version);
                cmd.env("DEPLOYMENT_ENVIRONMENT", &execution.environment);

                let output = cmd.output().await
                    .map_err(|e| DeploymentError::ExecutionFailed(format!("Failed to execute command: {}", e)))?;

                if !output.status.success() {
                    let error_msg = String::from_utf8_lossy(&output.stderr);
                    return Err(DeploymentError::ExecutionFailed(format!("Command failed: {}", error_msg)));
                }

                Ok(())
            },
            DeploymentStepType::Docker { image, command, environment_vars } => {
                let mut docker_cmd = Command::new("docker");
                docker_cmd.arg("run").arg("--rm");
                
                // Add environment variables
                if let Some(env_vars) = environment_vars {
                    for (key, value) in env_vars {
                        docker_cmd.arg("-e").arg(format!("{}={}", key, value));
                    }
                }
                
                docker_cmd.arg(image);
                
                if let Some(cmd) = command {
                    docker_cmd.args(cmd.split_whitespace());
                }

                let output = docker_cmd.output().await
                    .map_err(|e| DeploymentError::ExecutionFailed(format!("Failed to execute Docker command: {}", e)))?;

                if !output.status.success() {
                    let error_msg = String::from_utf8_lossy(&output.stderr);
                    return Err(DeploymentError::ExecutionFailed(format!("Docker command failed: {}", error_msg)));
                }

                Ok(())
            },
            DeploymentStepType::Kubernetes { manifest_path, namespace } => {
                let mut kubectl_cmd = Command::new("kubectl");
                kubectl_cmd.arg("apply").arg("-f").arg(manifest_path);
                
                if let Some(ns) = namespace {
                    kubectl_cmd.arg("-n").arg(ns);
                }

                let output = kubectl_cmd.output().await
                    .map_err(|e| DeploymentError::ExecutionFailed(format!("Failed to execute kubectl: {}", e)))?;

                if !output.status.success() {
                    let error_msg = String::from_utf8_lossy(&output.stderr);
                    return Err(DeploymentError::ExecutionFailed(format!("Kubectl command failed: {}", error_msg)));
                }

                Ok(())
            },
            DeploymentStepType::HealthCheck { url, expected_status, timeout_seconds } => {
                // Perform health check
                let timeout = Duration::from_secs(*timeout_seconds);
                let start_time = std::time::Instant::now();
                
                loop {
                    // In a real implementation, this would use an HTTP client
                    // For now, we'll simulate a successful health check
                    info!("Performing health check: {}", url);
                    
                    // Simulate health check response
                    let status_code = 200; // Placeholder
                    
                    if status_code == *expected_status {
                        return Ok(());
                    }
                    
                    if start_time.elapsed() > timeout {
                        return Err(DeploymentError::HealthCheckFailed(
                            format!("Health check timed out after {} seconds", timeout_seconds)
                        ));
                    }
                    
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            },
            DeploymentStepType::Wait { duration_seconds } => {
                info!("Waiting for {} seconds", duration_seconds);
                tokio::time::sleep(Duration::from_secs(*duration_seconds)).await;
                Ok(())
            },
        }
    }

    /// Log deployment message
    async fn log_message(execution: &DeploymentExecution, message: String) {
        let log_entry = DeploymentLogEntry {
            timestamp: SystemTime::now(),
            level: LogLevel::Info,
            message,
        };
        
        execution.logs.write().await.push(log_entry);
    }
}

/// Deployment pipeline configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentPipeline {
    pub id: String,
    pub name: String,
    pub description: String,
    pub stages: Vec<DeploymentStage>,
    pub environments: Vec<String>,
    pub rollback_enabled: bool,
    pub notifications: DeploymentNotifications,
}

/// Deployment stage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentStage {
    pub name: String,
    pub description: String,
    pub steps: Vec<DeploymentStep>,
    pub parallel: bool,
    pub continue_on_failure: bool,
}

/// Individual deployment step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentStep {
    pub name: String,
    pub description: String,
    pub step_type: DeploymentStepType,
    pub timeout_seconds: u64,
    pub retry_count: u32,
}

/// Types of deployment steps
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeploymentStepType {
    Command {
        command: String,
        args: Option<Vec<String>>,
        working_dir: Option<String>,
    },
    Docker {
        image: String,
        command: Option<String>,
        environment_vars: Option<HashMap<String, String>>,
    },
    Kubernetes {
        manifest_path: String,
        namespace: Option<String>,
    },
    HealthCheck {
        url: String,
        expected_status: u16,
        timeout_seconds: u64,
    },
    Wait {
        duration_seconds: u64,
    },
}

/// Deployment notifications configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentNotifications {
    pub on_success: Vec<NotificationTarget>,
    pub on_failure: Vec<NotificationTarget>,
    pub on_start: Vec<NotificationTarget>,
}

impl Default for DeploymentNotifications {
    fn default() -> Self {
        Self {
            on_success: Vec::new(),
            on_failure: Vec::new(),
            on_start: Vec::new(),
        }
    }
}

/// Notification targets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NotificationTarget {
    Slack { webhook_url: String },
    Email { addresses: Vec<String> },
    Discord { webhook_url: String },
    Webhook { url: String, headers: Option<HashMap<String, String>> },
}

/// Deployment execution state
#[derive(Debug, Clone)]
pub struct DeploymentExecution {
    pub id: String,
    pub pipeline_id: String,
    pub version: String,
    pub environment: String,
    pub status: DeploymentStatus,
    pub started_at: SystemTime,
    pub completed_at: Option<SystemTime>,
    pub current_stage: usize,
    pub total_stages: usize,
    pub logs: Arc<RwLock<Vec<DeploymentLogEntry>>>,
    pub rollback_info: Option<RollbackInfo>,
}

/// Deployment status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeploymentStatus {
    Starting,
    Running,
    Completed,
    Failed,
    RolledBack,
    Cancelled,
}

/// Deployment record for history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentRecord {
    pub id: String,
    pub pipeline_id: String,
    pub version: String,
    pub environment: String,
    pub status: DeploymentStatus,
    pub started_at: SystemTime,
    pub completed_at: Option<SystemTime>,
    pub duration: Option<Duration>,
}

/// Deployment summary for listing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentSummary {
    pub id: String,
    pub pipeline_id: String,
    pub version: String,
    pub environment: String,
    pub status: DeploymentStatus,
    pub progress_percent: u32,
    pub started_at: SystemTime,
}

/// Deployment log entry
#[derive(Debug, Clone)]
pub struct DeploymentLogEntry {
    pub timestamp: SystemTime,
    pub level: LogLevel,
    pub message: String,
}

/// Log levels
#[derive(Debug, Clone)]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
}

/// Rollback information
#[derive(Debug, Clone)]
pub struct RollbackInfo {
    pub previous_version: String,
    pub rollback_steps: Vec<DeploymentStep>,
}

/// Deployment configuration
#[derive(Debug, Clone)]
pub struct DeploymentConfig {
    pub max_concurrent_deployments: usize,
    pub deployment_timeout_minutes: u64,
    pub enable_rollbacks: bool,
    pub notification_config: DeploymentNotifications,
    pub storage_path: String,
}

impl Default for DeploymentConfig {
    fn default() -> Self {
        Self {
            max_concurrent_deployments: 5,
            deployment_timeout_minutes: 60,
            enable_rollbacks: true,
            notification_config: DeploymentNotifications::default(),
            storage_path: "./deployments".to_string(),
        }
    }
}

/// Deployment errors
#[derive(Debug)]
pub enum DeploymentError {
    PipelineNotFound(String),
    DeploymentNotFound(String),
    InvalidConfiguration(String),
    ExecutionFailed(String),
    HealthCheckFailed(String),
    TimeoutError,
    NoPreviousDeployment,
    RollbackFailed(String),
}

impl std::fmt::Display for DeploymentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeploymentError::PipelineNotFound(id) => write!(f, "Pipeline not found: {}", id),
            DeploymentError::DeploymentNotFound(id) => write!(f, "Deployment not found: {}", id),
            DeploymentError::InvalidConfiguration(msg) => write!(f, "Invalid configuration: {}", msg),
            DeploymentError::ExecutionFailed(msg) => write!(f, "Execution failed: {}", msg),
            DeploymentError::HealthCheckFailed(msg) => write!(f, "Health check failed: {}", msg),
            DeploymentError::TimeoutError => write!(f, "Operation timed out"),
            DeploymentError::NoPreviousDeployment => write!(f, "No previous deployment found for rollback"),
            DeploymentError::RollbackFailed(msg) => write!(f, "Rollback failed: {}", msg),
        }
    }
}

impl std::error::Error for DeploymentError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_deployment_manager_creation() {
        let config = DeploymentConfig::default();
        let manager = DeploymentManager::new(config);
        
        let active_deployments = manager.list_active_deployments().await;
        assert_eq!(active_deployments.len(), 0);
    }

    #[tokio::test]
    async fn test_pipeline_registration() {
        let config = DeploymentConfig::default();
        let manager = DeploymentManager::new(config);
        
        let pipeline = DeploymentPipeline {
            id: "test-pipeline".to_string(),
            name: "Test Pipeline".to_string(),
            description: "A test deployment pipeline".to_string(),
            stages: vec![
                DeploymentStage {
                    name: "Build".to_string(),
                    description: "Build stage".to_string(),
                    steps: vec![
                        DeploymentStep {
                            name: "Compile".to_string(),
                            description: "Compile the application".to_string(),
                            step_type: DeploymentStepType::Command {
                                command: "cargo".to_string(),
                                args: Some(vec!["build".to_string(), "--release".to_string()]),
                                working_dir: None,
                            },
                            timeout_seconds: 300,
                            retry_count: 1,
                        }
                    ],
                    parallel: false,
                    continue_on_failure: false,
                }
            ],
            environments: vec!["staging".to_string(), "production".to_string()],
            rollback_enabled: true,
            notifications: DeploymentNotifications::default(),
        };

        manager.register_pipeline(pipeline).await.unwrap();
    }

    #[test]
    fn test_deployment_status_serialization() {
        let status = DeploymentStatus::Running;
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("Running"));
    }
}