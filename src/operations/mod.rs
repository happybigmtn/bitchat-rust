//! Operations Tooling and Automation for BitCraps
//! 
//! This module provides comprehensive operational tools for production deployment:
//! 
//! ## Features
//! - Automated deployment pipelines
//! - Infrastructure monitoring and management
//! - Backup and disaster recovery
//! - Health checking and auto-healing
//! - Performance optimization automation
//! - Log aggregation and analysis
//! - Security incident response
//! - Capacity planning and scaling

pub mod deployment;
pub mod monitoring;
pub mod backup;
pub mod health;
pub mod scaling;
pub mod logging;
pub mod security;
pub mod maintenance;

pub use deployment::{DeploymentManager, DeploymentConfig, DeploymentPipeline, DeploymentStatus};
pub use monitoring::{InfrastructureMonitor, MonitoringConfig, SystemMetrics, AlertRule};
pub use backup::{BackupManager, BackupConfig, BackupStatus, RecoveryPlan};
pub use health::{HealthChecker, HealthConfig, HealthStatus, AutoHealer};
pub use scaling::{AutoScaler, ScalingConfig, ScalingPolicy, ResourceMetrics};
pub use logging::{LogAggregator, LogConfig, LogLevel, LogAnalyzer};
pub use security::{SecurityMonitor, SecurityConfig, ThreatDetector, IncidentResponder};
pub use maintenance::{MaintenanceScheduler, MaintenanceTask, MaintenanceWindow};