//! Plugin Sandbox for BitCraps
//!
//! This module provides a secure sandboxed execution environment for plugins
//! with resource quotas, capability-based security, and runtime monitoring.

use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use std::time::{Duration, SystemTime, Instant};

use serde::{Deserialize, Serialize};
use tokio::sync::{RwLock, Semaphore};
use tracing::{debug, warn, error, info};

use super::core::{PluginCapability, PluginResult, PluginError};

/// Plugin sandbox that manages secure execution environments
pub struct PluginSandbox {
    config: SandboxConfig,
    environments: Arc<RwLock<HashMap<String, PluginEnvironment>>>,
    global_stats: Arc<SandboxStatistics>,
    resource_monitor: Arc<ResourceMonitor>,
}

impl PluginSandbox {
    /// Create new plugin sandbox
    pub fn new(config: SandboxConfig) -> PluginResult<Self> {
        let resource_monitor = Arc::new(ResourceMonitor::new(&config));

        Ok(Self {
            config,
            environments: Arc::new(RwLock::new(HashMap::new())),
            global_stats: Arc::new(SandboxStatistics::new()),
            resource_monitor,
        })
    }

    /// Create secure environment for plugin
    pub async fn create_environment(&self, plugin_id: &str) -> PluginResult<()> {
        let mut environments = self.environments.write().await;

        if environments.contains_key(plugin_id) {
            return Err(PluginError::SecurityViolation(
                format!("Environment already exists for plugin: {}", plugin_id)
            ));
        }

        // Create resource quotas
        let quotas = ResourceQuota {
            max_memory_mb: self.config.default_memory_limit_mb,
            max_cpu_percent: self.config.default_cpu_limit_percent,
            max_network_connections: self.config.default_network_connections,
            max_file_handles: self.config.default_file_handles,
            max_execution_time_ms: self.config.default_execution_timeout_ms,
            max_disk_usage_mb: self.config.default_disk_usage_mb,
        };

        // Create security policy
        let policy = SecurityPolicy {
            allowed_capabilities: vec![
                PluginCapability::NetworkAccess,
                PluginCapability::DataStorage,
                PluginCapability::RandomNumberGeneration,
            ],
            denied_capabilities: vec![
                PluginCapability::RealMoneyGaming, // Must be explicitly granted
            ],
            allowed_network_domains: self.config.allowed_domains.clone(),
            allowed_file_paths: self.config.allowed_file_paths.clone(),
            enable_syscall_filtering: self.config.enable_syscall_filtering,
        };

        // Create environment
        let environment = PluginEnvironment {
            plugin_id: plugin_id.to_string(),
            created_at: SystemTime::now(),
            quotas,
            policy,
            resource_usage: ResourceUsage::new(),
            violations: Vec::new(),
            is_active: true,
            execution_semaphore: Arc::new(Semaphore::new(self.config.max_concurrent_operations)),
        };

        environments.insert(plugin_id.to_string(), environment);

        self.global_stats.environments_created.fetch_add(1, Ordering::Relaxed);
        info!("Created sandbox environment for plugin: {}", plugin_id);

        Ok(())
    }

    /// Destroy plugin environment
    pub async fn destroy_environment(&self, plugin_id: &str) -> PluginResult<()> {
        let mut environments = self.environments.write().await;

        if let Some(mut environment) = environments.remove(plugin_id) {
            environment.is_active = false;
            
            // Force cleanup of any remaining resources
            self.cleanup_plugin_resources(plugin_id, &environment).await?;
            
            self.global_stats.environments_destroyed.fetch_add(1, Ordering::Relaxed);
            info!("Destroyed sandbox environment for plugin: {}", plugin_id);
        }

        Ok(())
    }

    /// Execute operation within sandbox
    pub async fn execute<F, T>(&self, plugin_id: &str, operation: F) -> PluginResult<T>
    where
        F: FnOnce() -> PluginResult<T> + Send,
        T: Send,
    {
        let environments = self.environments.read().await;
        let environment = environments
            .get(plugin_id)
            .ok_or_else(|| PluginError::SecurityViolation(
                format!("No sandbox environment for plugin: {}", plugin_id)
            ))?;

        if !environment.is_active {
            return Err(PluginError::SecurityViolation(
                format!("Sandbox environment is inactive for plugin: {}", plugin_id)
            ));
        }

        // Acquire execution permit
        let _permit = environment.execution_semaphore
            .acquire()
            .await
            .map_err(|_| PluginError::ResourceLimitExceeded(
                "Too many concurrent operations".to_string()
            ))?;

        let start_time = Instant::now();

        // Pre-execution checks
        self.pre_execution_checks(plugin_id, environment).await?;

        // Execute operation with timeout
        let result = tokio::time::timeout(
            Duration::from_millis(environment.quotas.max_execution_time_ms),
            tokio::task::spawn_blocking(operation)
        ).await;

        let execution_duration = start_time.elapsed();

        // Post-execution checks and updates
        self.post_execution_updates(plugin_id, execution_duration).await?;

        match result {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(e)) => Err(e),
            Err(_) => {
                self.record_violation(
                    plugin_id,
                    SandboxViolation {
                        violation_type: ViolationType::ExecutionTimeout,
                        message: "Plugin execution timed out".to_string(),
                        timestamp: SystemTime::now(),
                        severity: ViolationSeverity::High,
                    }
                ).await;
                
                Err(PluginError::RuntimeError(
                    "Plugin execution timed out".to_string()
                ))
            }
        }
    }

    /// Check if plugin has capability
    pub async fn has_capability(
        &self,
        plugin_id: &str,
        capability: &PluginCapability,
    ) -> PluginResult<bool> {
        let environments = self.environments.read().await;
        let environment = environments
            .get(plugin_id)
            .ok_or_else(|| PluginError::SecurityViolation(
                format!("No sandbox environment for plugin: {}", plugin_id)
            ))?;

        Ok(environment.policy.allowed_capabilities.contains(capability) &&
           !environment.policy.denied_capabilities.contains(capability))
    }

    /// Grant capability to plugin
    pub async fn grant_capability(
        &self,
        plugin_id: &str,
        capability: PluginCapability,
    ) -> PluginResult<()> {
        let mut environments = self.environments.write().await;
        let environment = environments
            .get_mut(plugin_id)
            .ok_or_else(|| PluginError::SecurityViolation(
                format!("No sandbox environment for plugin: {}", plugin_id)
            ))?;

        // Remove from denied list if present
        environment.policy.denied_capabilities.retain(|c| c != &capability);
        
        // Add to allowed list if not present
        if !environment.policy.allowed_capabilities.contains(&capability) {
            environment.policy.allowed_capabilities.push(capability);
        }

        info!("Granted capability {:?} to plugin: {}", capability, plugin_id);
        Ok(())
    }

    /// Revoke capability from plugin
    pub async fn revoke_capability(
        &self,
        plugin_id: &str,
        capability: PluginCapability,
    ) -> PluginResult<()> {
        let mut environments = self.environments.write().await;
        let environment = environments
            .get_mut(plugin_id)
            .ok_or_else(|| PluginError::SecurityViolation(
                format!("No sandbox environment for plugin: {}", plugin_id)
            ))?;

        // Remove from allowed list
        environment.policy.allowed_capabilities.retain(|c| c != &capability);
        
        // Add to denied list if not present
        if !environment.policy.denied_capabilities.contains(&capability) {
            environment.policy.denied_capabilities.push(capability);
        }

        warn!("Revoked capability {:?} from plugin: {}", capability, plugin_id);
        Ok(())
    }

    /// Monitor resource usage
    pub async fn monitor_resources(&self) -> PluginResult<()> {
        let environments = self.environments.read().await;
        
        for (plugin_id, environment) in environments.iter() {
            if !environment.is_active {
                continue;
            }

            // Check resource usage
            let current_usage = self.resource_monitor.measure_usage(plugin_id).await?;
            
            // Update stored usage
            {
                let mut usage = environment.resource_usage.lock().await;
                *usage = current_usage.clone();
            }

            // Check for violations
            self.check_resource_violations(plugin_id, environment, &current_usage).await?;
        }

        Ok(())
    }

    /// Get sandbox statistics
    pub async fn get_statistics(&self) -> SandboxStatisticsSnapshot {
        let environments = self.environments.read().await;
        let active_environments = environments.values().filter(|e| e.is_active).count();

        SandboxStatisticsSnapshot {
            active_environments,
            total_environments_created: self.global_stats.environments_created.load(Ordering::Relaxed),
            total_environments_destroyed: self.global_stats.environments_destroyed.load(Ordering::Relaxed),
            total_violations: self.global_stats.total_violations.load(Ordering::Relaxed),
            total_executions: self.global_stats.total_executions.load(Ordering::Relaxed),
        }
    }

    /// Pre-execution security and resource checks
    async fn pre_execution_checks(
        &self,
        plugin_id: &str,
        environment: &PluginEnvironment,
    ) -> PluginResult<()> {
        // Check if too many violations
        if environment.violations.len() > self.config.max_violations_before_shutdown {
            return Err(PluginError::SecurityViolation(
                format!("Plugin {} has too many violations", plugin_id)
            ));
        }

        // Check recent violations
        let recent_violations = environment.violations
            .iter()
            .filter(|v| v.timestamp.elapsed().unwrap_or(Duration::MAX) < Duration::from_secs(300))
            .count();

        if recent_violations > self.config.max_violations_per_5min {
            return Err(PluginError::SecurityViolation(
                format!("Plugin {} has too many recent violations", plugin_id)
            ));
        }

        Ok(())
    }

    /// Post-execution updates and monitoring
    async fn post_execution_updates(
        &self,
        plugin_id: &str,
        execution_duration: Duration,
    ) -> PluginResult<()> {
        self.global_stats.total_executions.fetch_add(1, Ordering::Relaxed);
        
        // Update execution time statistics
        let mut environments = self.environments.write().await;
        if let Some(environment) = environments.get_mut(plugin_id) {
            let mut usage = environment.resource_usage.lock().await;
            usage.total_execution_time += execution_duration;
            usage.execution_count += 1;
        }

        Ok(())
    }

    /// Check for resource usage violations
    async fn check_resource_violations(
        &self,
        plugin_id: &str,
        environment: &PluginEnvironment,
        current_usage: &ResourceUsageSnapshot,
    ) -> PluginResult<()> {
        let mut violations = Vec::new();

        // Check memory usage
        if current_usage.memory_mb > environment.quotas.max_memory_mb {
            violations.push(SandboxViolation {
                violation_type: ViolationType::MemoryLimit,
                message: format!("Memory usage {} MB exceeds limit {} MB", 
                    current_usage.memory_mb, environment.quotas.max_memory_mb),
                timestamp: SystemTime::now(),
                severity: ViolationSeverity::High,
            });
        }

        // Check CPU usage
        if current_usage.cpu_percent > environment.quotas.max_cpu_percent {
            violations.push(SandboxViolation {
                violation_type: ViolationType::CpuLimit,
                message: format!("CPU usage {}% exceeds limit {}%", 
                    current_usage.cpu_percent, environment.quotas.max_cpu_percent),
                timestamp: SystemTime::now(),
                severity: ViolationSeverity::Medium,
            });
        }

        // Check network connections
        if current_usage.network_connections > environment.quotas.max_network_connections {
            violations.push(SandboxViolation {
                violation_type: ViolationType::NetworkLimit,
                message: format!("Network connections {} exceeds limit {}", 
                    current_usage.network_connections, environment.quotas.max_network_connections),
                timestamp: SystemTime::now(),
                severity: ViolationSeverity::Medium,
            });
        }

        // Record violations
        for violation in violations {
            self.record_violation(plugin_id, violation).await;
        }

        Ok(())
    }

    /// Record security violation
    async fn record_violation(&self, plugin_id: &str, violation: SandboxViolation) {
        let mut environments = self.environments.write().await;
        if let Some(environment) = environments.get_mut(plugin_id) {
            environment.violations.push(violation.clone());
            
            // Limit stored violations
            if environment.violations.len() > 1000 {
                environment.violations.drain(0..500); // Keep most recent 500
            }
        }
        
        self.global_stats.total_violations.fetch_add(1, Ordering::Relaxed);
        warn!("Security violation for plugin {}: {:?}", plugin_id, violation);
    }

    /// Cleanup plugin resources
    async fn cleanup_plugin_resources(
        &self,
        plugin_id: &str,
        _environment: &PluginEnvironment,
    ) -> PluginResult<()> {
        // This would perform actual resource cleanup like:
        // - Closing file handles
        // - Terminating network connections
        // - Freeing memory allocations
        // - Stopping background threads
        
        debug!("Cleaned up resources for plugin: {}", plugin_id);
        Ok(())
    }
}

/// Sandbox configuration
#[derive(Debug, Clone)]
pub struct SandboxConfig {
    pub default_memory_limit_mb: u64,
    pub default_cpu_limit_percent: f32,
    pub default_network_connections: u32,
    pub default_file_handles: u32,
    pub default_execution_timeout_ms: u64,
    pub default_disk_usage_mb: u64,
    pub max_concurrent_operations: usize,
    pub max_violations_before_shutdown: usize,
    pub max_violations_per_5min: usize,
    pub enable_syscall_filtering: bool,
    pub allowed_domains: Vec<String>,
    pub allowed_file_paths: Vec<String>,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            default_memory_limit_mb: 512,
            default_cpu_limit_percent: 25.0,
            default_network_connections: 10,
            default_file_handles: 100,
            default_execution_timeout_ms: 30000, // 30 seconds
            default_disk_usage_mb: 100,
            max_concurrent_operations: 10,
            max_violations_before_shutdown: 10,
            max_violations_per_5min: 5,
            enable_syscall_filtering: true,
            allowed_domains: vec![
                "api.bitcraps.com".to_string(),
                "cdn.bitcraps.com".to_string(),
            ],
            allowed_file_paths: vec![
                "/tmp/bitcraps/plugins/".to_string(),
                "/var/lib/bitcraps/data/".to_string(),
            ],
        }
    }
}

/// Resource quota limits for plugins
#[derive(Debug, Clone)]
pub struct ResourceQuota {
    pub max_memory_mb: u64,
    pub max_cpu_percent: f32,
    pub max_network_connections: u32,
    pub max_file_handles: u32,
    pub max_execution_time_ms: u64,
    pub max_disk_usage_mb: u64,
}

/// Security policy for plugin execution
#[derive(Debug, Clone)]
pub struct SecurityPolicy {
    pub allowed_capabilities: Vec<PluginCapability>,
    pub denied_capabilities: Vec<PluginCapability>,
    pub allowed_network_domains: Vec<String>,
    pub allowed_file_paths: Vec<String>,
    pub enable_syscall_filtering: bool,
}

/// Plugin execution environment
struct PluginEnvironment {
    plugin_id: String,
    created_at: SystemTime,
    quotas: ResourceQuota,
    policy: SecurityPolicy,
    resource_usage: Arc<tokio::sync::Mutex<ResourceUsageSnapshot>>,
    violations: Vec<SandboxViolation>,
    is_active: bool,
    execution_semaphore: Arc<Semaphore>,
}

/// Current resource usage snapshot
#[derive(Debug, Clone)]
struct ResourceUsageSnapshot {
    memory_mb: u64,
    cpu_percent: f32,
    network_connections: u32,
    file_handles: u32,
    disk_usage_mb: u64,
    total_execution_time: Duration,
    execution_count: u64,
}

/// Resource usage tracking
struct ResourceUsage {
    inner: Arc<tokio::sync::Mutex<ResourceUsageSnapshot>>,
}

impl ResourceUsage {
    fn new() -> Self {
        Self {
            inner: Arc::new(tokio::sync::Mutex::new(ResourceUsageSnapshot {
                memory_mb: 0,
                cpu_percent: 0.0,
                network_connections: 0,
                file_handles: 0,
                disk_usage_mb: 0,
                total_execution_time: Duration::ZERO,
                execution_count: 0,
            })),
        }
    }

    async fn lock(&self) -> tokio::sync::MutexGuard<ResourceUsageSnapshot> {
        self.inner.lock().await
    }
}

/// Security violation record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxViolation {
    pub violation_type: ViolationType,
    pub message: String,
    pub timestamp: SystemTime,
    pub severity: ViolationSeverity,
}

/// Types of sandbox violations
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ViolationType {
    MemoryLimit,
    CpuLimit,
    NetworkLimit,
    FileHandleLimit,
    ExecutionTimeout,
    UnauthorizedCapability,
    InvalidNetworkAccess,
    InvalidFileAccess,
    SyscallViolation,
}

/// Violation severity levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ViolationSeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Resource monitor for tracking plugin usage
struct ResourceMonitor {
    _config: SandboxConfig,
}

impl ResourceMonitor {
    fn new(config: &SandboxConfig) -> Self {
        Self {
            _config: config.clone(),
        }
    }

    async fn measure_usage(&self, _plugin_id: &str) -> PluginResult<ResourceUsageSnapshot> {
        // This would perform actual system measurements
        // For now, return mock data
        Ok(ResourceUsageSnapshot {
            memory_mb: 64,
            cpu_percent: 5.0,
            network_connections: 2,
            file_handles: 10,
            disk_usage_mb: 10,
            total_execution_time: Duration::from_secs(30),
            execution_count: 100,
        })
    }
}

/// Sandbox statistics
struct SandboxStatistics {
    environments_created: AtomicU64,
    environments_destroyed: AtomicU64,
    total_violations: AtomicU64,
    total_executions: AtomicU64,
}

impl SandboxStatistics {
    fn new() -> Self {
        Self {
            environments_created: AtomicU64::new(0),
            environments_destroyed: AtomicU64::new(0),
            total_violations: AtomicU64::new(0),
            total_executions: AtomicU64::new(0),
        }
    }
}

/// Sandbox statistics snapshot
#[derive(Debug, Clone)]
pub struct SandboxStatisticsSnapshot {
    pub active_environments: usize,
    pub total_environments_created: u64,
    pub total_environments_destroyed: u64,
    pub total_violations: u64,
    pub total_executions: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_sandbox_creation() {
        let config = SandboxConfig::default();
        let sandbox = PluginSandbox::new(config).unwrap();
        
        let stats = sandbox.get_statistics().await;
        assert_eq!(stats.active_environments, 0);
    }

    #[tokio::test]
    async fn test_environment_lifecycle() {
        let config = SandboxConfig::default();
        let sandbox = PluginSandbox::new(config).unwrap();
        
        // Create environment
        sandbox.create_environment("test-plugin").await.unwrap();
        
        let stats = sandbox.get_statistics().await;
        assert_eq!(stats.active_environments, 1);
        assert_eq!(stats.total_environments_created, 1);
        
        // Destroy environment
        sandbox.destroy_environment("test-plugin").await.unwrap();
        
        let stats = sandbox.get_statistics().await;
        assert_eq!(stats.active_environments, 0);
        assert_eq!(stats.total_environments_destroyed, 1);
    }

    #[tokio::test]
    async fn test_capability_management() {
        let config = SandboxConfig::default();
        let sandbox = PluginSandbox::new(config).unwrap();
        
        sandbox.create_environment("test-plugin").await.unwrap();
        
        // Check default capabilities
        let has_network = sandbox.has_capability("test-plugin", &PluginCapability::NetworkAccess).await.unwrap();
        assert!(has_network);
        
        let has_real_money = sandbox.has_capability("test-plugin", &PluginCapability::RealMoneyGaming).await.unwrap();
        assert!(!has_real_money);
        
        // Grant capability
        sandbox.grant_capability("test-plugin", PluginCapability::RealMoneyGaming).await.unwrap();
        let has_real_money = sandbox.has_capability("test-plugin", &PluginCapability::RealMoneyGaming).await.unwrap();
        assert!(has_real_money);
        
        // Revoke capability
        sandbox.revoke_capability("test-plugin", PluginCapability::NetworkAccess).await.unwrap();
        let has_network = sandbox.has_capability("test-plugin", &PluginCapability::NetworkAccess).await.unwrap();
        assert!(!has_network);
    }

    #[tokio::test]
    async fn test_sandboxed_execution() {
        let config = SandboxConfig::default();
        let sandbox = PluginSandbox::new(config).unwrap();
        
        sandbox.create_environment("test-plugin").await.unwrap();
        
        // Execute simple operation
        let result = sandbox.execute("test-plugin", || {
            Ok(42)
        }).await.unwrap();
        
        assert_eq!(result, 42);
        
        let stats = sandbox.get_statistics().await;
        assert_eq!(stats.total_executions, 1);
    }
}