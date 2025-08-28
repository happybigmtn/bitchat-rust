# Chapter 99: Production Deployment Strategies

*In 2009, a single deployment at Amazon took down their entire retail website for 49 minutes, costing the company an estimated $5.7 million in lost revenue. That incident, and countless others like it, drove the evolution of deployment strategies from the traditional "pray and deploy" approach to sophisticated, risk-minimizing techniques that have become essential for modern distributed systems.*

## The Evolution of Deployment

The history of deployment strategies is a story of learning from failure. In the early days of computing, deployments were events—scheduled downtime where systems were taken offline, updated, and brought back online. This worked when applications were monoliths and users expected occasional maintenance windows.

The advent of web services changed everything. Users expected 24/7 availability, and even brief outages could cause significant business impact. Netflix's famous "Chaos Monkey," introduced in 2011, embodied a new philosophy: systems should be designed to handle failure gracefully, and deployments should be routine, low-risk operations.

## Understanding Modern Deployment Patterns

Modern deployment strategies are built around a fundamental principle: minimize blast radius. Instead of updating all instances simultaneously, smart deployment patterns gradually roll out changes while maintaining the ability to quickly revert if problems arise.

```rust
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use serde::{Serialize, Deserialize};
use tokio::sync::RwLock;

/// Core deployment configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentConfig {
    /// Application name
    pub app_name: String,
    
    /// Target environment
    pub environment: Environment,
    
    /// Deployment strategy
    pub strategy: DeploymentStrategy,
    
    /// Health check configuration
    pub health_checks: HealthCheckConfig,
    
    /// Traffic routing rules
    pub traffic_rules: TrafficRoutingConfig,
    
    /// Rollback configuration
    pub rollback: RollbackConfig,
    
    /// Monitoring and alerting
    pub monitoring: MonitoringConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Environment {
    Development,
    Staging,
    Production,
    Canary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeploymentStrategy {
    BlueGreen {
        validation_duration: Duration,
        auto_promote: bool,
    },
    Canary {
        initial_percentage: f32,
        increment_percentage: f32,
        increment_interval: Duration,
        success_criteria: SuccessCriteria,
    },
    Rolling {
        batch_size: usize,
        batch_interval: Duration,
        max_unavailable: usize,
    },
    Recreate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckConfig {
    /// HTTP endpoint to check
    pub endpoint: String,
    
    /// Check interval
    pub interval: Duration,
    
    /// Timeout for each check
    pub timeout: Duration,
    
    /// Number of consecutive successes required
    pub healthy_threshold: u32,
    
    /// Number of consecutive failures to mark unhealthy
    pub unhealthy_threshold: u32,
    
    /// Expected HTTP status codes
    pub expected_status_codes: Vec<u16>,
    
    /// Grace period before starting checks
    pub initial_delay: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficRoutingConfig {
    /// Load balancer configuration
    pub load_balancer: LoadBalancerConfig,
    
    /// Routing rules
    pub rules: Vec<RoutingRule>,
    
    /// Session affinity settings
    pub session_affinity: Option<SessionAffinityConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuccessCriteria {
    /// Maximum error rate allowed
    pub max_error_rate: f32,
    
    /// Minimum success rate required
    pub min_success_rate: f32,
    
    /// Maximum response time percentile
    pub max_latency_p99: Duration,
    
    /// Minimum number of requests for evaluation
    pub min_request_count: u64,
    
    /// Evaluation window
    pub evaluation_window: Duration,
}

/// Main deployment orchestrator
pub struct DeploymentOrchestrator {
    config: DeploymentConfig,
    state: Arc<RwLock<DeploymentState>>,
    health_checker: HealthChecker,
    traffic_router: TrafficRouter,
    metrics_collector: MetricsCollector,
}

#[derive(Debug, Clone)]
pub struct DeploymentState {
    pub current_version: String,
    pub target_version: String,
    pub phase: DeploymentPhase,
    pub start_time: Instant,
    pub instances: HashMap<String, InstanceState>,
    pub traffic_split: TrafficSplit,
    pub rollback_triggered: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DeploymentPhase {
    Preparing,
    Deploying,
    Validating,
    Promoting,
    Completed,
    RollingBack,
    Failed,
}

#[derive(Debug, Clone)]
pub struct InstanceState {
    pub version: String,
    pub status: InstanceStatus,
    pub health_status: HealthStatus,
    pub last_health_check: Option<Instant>,
    pub deployment_time: Option<Instant>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InstanceStatus {
    Deploying,
    Running,
    Stopping,
    Stopped,
    Failed,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HealthStatus {
    Unknown,
    Healthy,
    Unhealthy,
    Degraded,
}

impl DeploymentOrchestrator {
    pub fn new(config: DeploymentConfig) -> Self {
        Self {
            health_checker: HealthChecker::new(config.health_checks.clone()),
            traffic_router: TrafficRouter::new(config.traffic_rules.clone()),
            metrics_collector: MetricsCollector::new(config.monitoring.clone()),
            state: Arc::new(RwLock::new(DeploymentState {
                current_version: "unknown".to_string(),
                target_version: "unknown".to_string(),
                phase: DeploymentPhase::Preparing,
                start_time: Instant::now(),
                instances: HashMap::new(),
                traffic_split: TrafficSplit::default(),
                rollback_triggered: false,
            })),
            config,
        }
    }
    
    /// Start a deployment
    pub async fn deploy(&self, target_version: String) -> Result<DeploymentResult, DeploymentError> {
        let mut state = self.state.write().await;
        state.target_version = target_version.clone();
        state.phase = DeploymentPhase::Preparing;
        state.start_time = Instant::now();
        drop(state);
        
        match &self.config.strategy {
            DeploymentStrategy::BlueGreen { validation_duration, auto_promote } => {
                self.deploy_blue_green(target_version, *validation_duration, *auto_promote).await
            }
            DeploymentStrategy::Canary { initial_percentage, increment_percentage, increment_interval, success_criteria } => {
                self.deploy_canary(target_version, *initial_percentage, *increment_percentage, *increment_interval, success_criteria.clone()).await
            }
            DeploymentStrategy::Rolling { batch_size, batch_interval, max_unavailable } => {
                self.deploy_rolling(target_version, *batch_size, *batch_interval, *max_unavailable).await
            }
            DeploymentStrategy::Recreate => {
                self.deploy_recreate(target_version).await
            }
        }
    }
}
```

## Blue-Green Deployment

Blue-green deployment maintains two identical production environments, switching traffic between them to achieve zero-downtime deployments.

```rust
impl DeploymentOrchestrator {
    async fn deploy_blue_green(&self, target_version: String, validation_duration: Duration, auto_promote: bool) -> Result<DeploymentResult, DeploymentError> {
        println!("Starting blue-green deployment for version {}", target_version);
        
        // Phase 1: Prepare green environment
        self.update_phase(DeploymentPhase::Preparing).await;
        let green_instances = self.provision_green_environment(&target_version).await?;
        
        // Phase 2: Deploy to green environment
        self.update_phase(DeploymentPhase::Deploying).await;
        self.deploy_to_instances(&green_instances, &target_version).await?;
        
        // Phase 3: Health check green environment
        self.wait_for_health(&green_instances).await?;
        
        // Phase 4: Validation phase
        self.update_phase(DeploymentPhase::Validating).await;
        let validation_result = self.validate_green_environment(&green_instances, validation_duration).await?;
        
        if !validation_result.passed {
            return self.rollback_blue_green(&green_instances).await;
        }
        
        // Phase 5: Traffic switching
        if auto_promote {
            self.switch_traffic_to_green(&green_instances).await?;
            self.cleanup_blue_environment().await?;
            self.update_phase(DeploymentPhase::Completed).await;
        } else {
            // Manual promotion required
            println!("Green environment validated. Manual promotion required.");
        }
        
        let end_time = Instant::now();
        let state = self.state.read().await;
        
        Ok(DeploymentResult {
            success: true,
            version: target_version,
            duration: end_time.duration_since(state.start_time),
            instances_deployed: green_instances.len(),
            rollback_performed: false,
            validation_results: Some(validation_result),
        })
    }
    
    async fn provision_green_environment(&self, version: &str) -> Result<Vec<String>, DeploymentError> {
        println!("Provisioning green environment for version {}", version);
        
        let state = self.state.read().await;
        let current_instances: Vec<_> = state.instances.keys().cloned().collect();
        drop(state);
        
        let mut green_instances = Vec::new();
        
        // Create new instances (green)
        for (i, _) in current_instances.iter().enumerate() {
            let instance_id = format!("green-{}-{}", version, i);
            
            // Simulate instance provisioning
            tokio::time::sleep(Duration::from_secs(5)).await;
            
            let mut state = self.state.write().await;
            state.instances.insert(instance_id.clone(), InstanceState {
                version: version.to_string(),
                status: InstanceStatus::Deploying,
                health_status: HealthStatus::Unknown,
                last_health_check: None,
                deployment_time: Some(Instant::now()),
            });
            
            green_instances.push(instance_id);
        }
        
        println!("Provisioned {} green instances", green_instances.len());
        Ok(green_instances)
    }
    
    async fn validate_green_environment(&self, instances: &[String], duration: Duration) -> Result<ValidationResult, DeploymentError> {
        println!("Validating green environment for {:?}", duration);
        
        let start_time = Instant::now();
        let mut validation_result = ValidationResult {
            passed: true,
            error_rate: 0.0,
            success_rate: 100.0,
            avg_latency: Duration::from_millis(50),
            p99_latency: Duration::from_millis(200),
            request_count: 0,
            errors: Vec::new(),
        };
        
        // Route small percentage of traffic to green for validation
        self.traffic_router.set_traffic_split(TrafficSplit {
            blue_percentage: 95.0,
            green_percentage: 5.0,
        }).await?;
        
        while start_time.elapsed() < duration {
            tokio::time::sleep(Duration::from_secs(10)).await;
            
            // Collect metrics from green instances
            let metrics = self.metrics_collector.collect_metrics(instances).await?;
            validation_result.request_count += metrics.request_count;
            validation_result.error_rate = metrics.error_rate;
            validation_result.success_rate = 100.0 - metrics.error_rate;
            validation_result.avg_latency = metrics.avg_latency;
            validation_result.p99_latency = metrics.p99_latency;
            
            // Check if validation is failing
            if metrics.error_rate > 5.0 || metrics.p99_latency > Duration::from_secs(2) {
                validation_result.passed = false;
                validation_result.errors.push(format!("High error rate: {:.2}%", metrics.error_rate));
                break;
            }
            
            println!("Validation progress: {:.1}% complete", 
                    (start_time.elapsed().as_secs_f64() / duration.as_secs_f64()) * 100.0);
        }
        
        // Reset traffic to blue only
        self.traffic_router.set_traffic_split(TrafficSplit {
            blue_percentage: 100.0,
            green_percentage: 0.0,
        }).await?;
        
        println!("Validation result: {} (error rate: {:.2}%)", 
                if validation_result.passed { "PASSED" } else { "FAILED" },
                validation_result.error_rate);
        
        Ok(validation_result)
    }
    
    async fn switch_traffic_to_green(&self, green_instances: &[String]) -> Result<(), DeploymentError> {
        println!("Switching traffic to green environment");
        
        self.update_phase(DeploymentPhase::Promoting).await;
        
        // Gradual traffic switch to avoid sudden load spikes
        let switch_stages = vec![10.0, 25.0, 50.0, 75.0, 100.0];
        
        for percentage in switch_stages {
            self.traffic_router.set_traffic_split(TrafficSplit {
                blue_percentage: 100.0 - percentage,
                green_percentage: percentage,
            }).await?;
            
            println!("Traffic split: {}% to green", percentage);
            
            // Monitor during traffic switch
            tokio::time::sleep(Duration::from_secs(30)).await;
            let health_status = self.health_checker.check_instances(green_instances).await?;
            
            if !health_status.all_healthy {
                // Rollback traffic
                self.traffic_router.set_traffic_split(TrafficSplit {
                    blue_percentage: 100.0,
                    green_percentage: 0.0,
                }).await?;
                return Err(DeploymentError::TrafficSwitchFailed);
            }
        }
        
        // Update instance labels (green becomes blue)
        let mut state = self.state.write().await;
        for instance_id in green_instances {
            if let Some(instance) = state.instances.get_mut(instance_id) {
                instance.status = InstanceStatus::Running;
            }
        }
        
        Ok(())
    }
    
    async fn rollback_blue_green(&self, green_instances: &[String]) -> Result<DeploymentResult, DeploymentError> {
        println!("Rolling back blue-green deployment");
        
        self.update_phase(DeploymentPhase::RollingBack).await;
        
        // Ensure traffic is routed to blue
        self.traffic_router.set_traffic_split(TrafficSplit {
            blue_percentage: 100.0,
            green_percentage: 0.0,
        }).await?;
        
        // Clean up green instances
        self.cleanup_instances(green_instances).await?;
        
        let mut state = self.state.write().await;
        state.phase = DeploymentPhase::Failed;
        state.rollback_triggered = true;
        
        Ok(DeploymentResult {
            success: false,
            version: state.target_version.clone(),
            duration: Instant::now().duration_since(state.start_time),
            instances_deployed: 0,
            rollback_performed: true,
            validation_results: None,
        })
    }
}
```

## Canary Deployment

Canary deployments gradually roll out changes to a subset of users, monitoring metrics to decide whether to continue or abort.

```rust
impl DeploymentOrchestrator {
    async fn deploy_canary(&self, target_version: String, initial_percentage: f32, increment_percentage: f32, increment_interval: Duration, success_criteria: SuccessCriteria) -> Result<DeploymentResult, DeploymentError> {
        println!("Starting canary deployment for version {} ({}% initial)", target_version, initial_percentage);
        
        // Phase 1: Deploy canary instances
        self.update_phase(DeploymentPhase::Deploying).await;
        let canary_instances = self.deploy_canary_instances(&target_version, initial_percentage).await?;
        
        // Phase 2: Gradual traffic increase
        let mut current_percentage = initial_percentage;
        
        while current_percentage < 100.0 {
            println!("Canary at {:.1}% traffic", current_percentage);
            
            // Set traffic split
            self.traffic_router.set_traffic_split(TrafficSplit {
                blue_percentage: 100.0 - current_percentage,
                green_percentage: current_percentage,
            }).await?;
            
            // Monitor for increment interval
            let monitoring_result = self.monitor_canary(&canary_instances, increment_interval, &success_criteria).await?;
            
            if !monitoring_result.success {
                println!("Canary monitoring failed: {:?}", monitoring_result.failures);
                return self.rollback_canary(&canary_instances).await;
            }
            
            // Increase traffic percentage
            current_percentage = (current_percentage + increment_percentage).min(100.0);
            
            if current_percentage < 100.0 {
                // Scale up canary instances if needed
                self.scale_canary_instances(&canary_instances, current_percentage).await?;
            }
        }
        
        // Phase 3: Complete rollout
        self.complete_canary_rollout(&canary_instances, &target_version).await?;
        
        let state = self.state.read().await;
        Ok(DeploymentResult {
            success: true,
            version: target_version,
            duration: Instant::now().duration_since(state.start_time),
            instances_deployed: canary_instances.len(),
            rollback_performed: false,
            validation_results: None,
        })
    }
    
    async fn deploy_canary_instances(&self, version: &str, percentage: f32) -> Result<Vec<String>, DeploymentError> {
        let state = self.state.read().await;
        let total_instances = state.instances.len();
        let canary_count = ((total_instances as f32 * percentage / 100.0).ceil() as usize).max(1);
        drop(state);
        
        println!("Deploying {} canary instances for version {}", canary_count, version);
        
        let mut canary_instances = Vec::new();
        
        for i in 0..canary_count {
            let instance_id = format!("canary-{}-{}", version, i);
            
            // Simulate deployment
            tokio::time::sleep(Duration::from_secs(3)).await;
            
            let mut state = self.state.write().await;
            state.instances.insert(instance_id.clone(), InstanceState {
                version: version.to_string(),
                status: InstanceStatus::Deploying,
                health_status: HealthStatus::Unknown,
                last_health_check: None,
                deployment_time: Some(Instant::now()),
            });
            
            canary_instances.push(instance_id);
        }
        
        // Wait for health checks
        self.wait_for_health(&canary_instances).await?;
        
        Ok(canary_instances)
    }
    
    async fn monitor_canary(&self, instances: &[String], duration: Duration, criteria: &SuccessCriteria) -> Result<MonitoringResult, DeploymentError> {
        println!("Monitoring canary for {:?}", duration);
        
        let start_time = Instant::now();
        let mut aggregated_metrics = AggregatedMetrics::new();
        
        while start_time.elapsed() < duration {
            tokio::time::sleep(Duration::from_secs(10)).await;
            
            let metrics = self.metrics_collector.collect_metrics(instances).await?;
            aggregated_metrics.add_sample(metrics);
            
            let current_metrics = aggregated_metrics.get_current();
            
            println!("Canary metrics: error_rate={:.2}%, latency_p99={:?}, requests={}",
                    current_metrics.error_rate, current_metrics.p99_latency, current_metrics.request_count);
            
            // Check success criteria
            let mut failures = Vec::new();
            
            if current_metrics.error_rate > criteria.max_error_rate {
                failures.push(format!("Error rate {:.2}% exceeds threshold {:.2}%", 
                                    current_metrics.error_rate, criteria.max_error_rate));
            }
            
            if current_metrics.success_rate < criteria.min_success_rate {
                failures.push(format!("Success rate {:.2}% below threshold {:.2}%", 
                                    current_metrics.success_rate, criteria.min_success_rate));
            }
            
            if current_metrics.p99_latency > criteria.max_latency_p99 {
                failures.push(format!("P99 latency {:?} exceeds threshold {:?}", 
                                    current_metrics.p99_latency, criteria.max_latency_p99));
            }
            
            if current_metrics.request_count >= criteria.min_request_count && !failures.is_empty() {
                return Ok(MonitoringResult {
                    success: false,
                    failures,
                    metrics: current_metrics,
                });
            }
        }
        
        Ok(MonitoringResult {
            success: true,
            failures: Vec::new(),
            metrics: aggregated_metrics.get_current(),
        })
    }
    
    async fn scale_canary_instances(&self, current_instances: &[String], target_percentage: f32) -> Result<(), DeploymentError> {
        let state = self.state.read().await;
        let total_capacity = state.instances.len() + current_instances.len();
        let required_canary_instances = ((total_capacity as f32 * target_percentage / 100.0).ceil() as usize).max(1);
        drop(state);
        
        if required_canary_instances > current_instances.len() {
            let additional_needed = required_canary_instances - current_instances.len();
            println!("Scaling canary: adding {} instances", additional_needed);
            
            // Deploy additional canary instances
            // Implementation would create new instances here
        }
        
        Ok(())
    }
    
    async fn complete_canary_rollout(&self, canary_instances: &[String], version: &str) -> Result<(), DeploymentError> {
        println!("Completing canary rollout");
        
        self.update_phase(DeploymentPhase::Promoting).await;
        
        // Replace all old instances with new version
        let state = self.state.read().await;
        let old_instances: Vec<_> = state.instances.iter()
            .filter(|(id, instance)| instance.version != *version && !canary_instances.contains(id))
            .map(|(id, _)| id.clone())
            .collect();
        drop(state);
        
        // Rolling replacement of old instances
        for old_instance in &old_instances {
            let new_instance_id = format!("prod-{}-{}", version, old_instance);
            
            // Deploy new instance
            let mut state = self.state.write().await;
            state.instances.insert(new_instance_id.clone(), InstanceState {
                version: version.to_string(),
                status: InstanceStatus::Deploying,
                health_status: HealthStatus::Unknown,
                last_health_check: None,
                deployment_time: Some(Instant::now()),
            });
            drop(state);
            
            // Wait for health
            self.wait_for_health(&[new_instance_id.clone()]).await?;
            
            // Remove old instance
            let mut state = self.state.write().await;
            state.instances.remove(old_instance);
        }
        
        // Clean up canary instances (they're replaced by production instances)
        self.cleanup_instances(canary_instances).await?;
        
        self.update_phase(DeploymentPhase::Completed).await;
        Ok(())
    }
    
    async fn rollback_canary(&self, canary_instances: &[String]) -> Result<DeploymentResult, DeploymentError> {
        println!("Rolling back canary deployment");
        
        self.update_phase(DeploymentPhase::RollingBack).await;
        
        // Route all traffic back to stable version
        self.traffic_router.set_traffic_split(TrafficSplit {
            blue_percentage: 100.0,
            green_percentage: 0.0,
        }).await?;
        
        // Clean up canary instances
        self.cleanup_instances(canary_instances).await?;
        
        let mut state = self.state.write().await;
        state.phase = DeploymentPhase::Failed;
        state.rollback_triggered = true;
        
        Ok(DeploymentResult {
            success: false,
            version: state.target_version.clone(),
            duration: Instant::now().duration_since(state.start_time),
            instances_deployed: 0,
            rollback_performed: true,
            validation_results: None,
        })
    }
}
```

## Rolling Deployment

Rolling deployments update instances in batches, maintaining availability throughout the process.

```rust
impl DeploymentOrchestrator {
    async fn deploy_rolling(&self, target_version: String, batch_size: usize, batch_interval: Duration, max_unavailable: usize) -> Result<DeploymentResult, DeploymentError> {
        println!("Starting rolling deployment for version {} (batch size: {})", target_version, batch_size);
        
        let state = self.state.read().await;
        let instance_ids: Vec<_> = state.instances.keys().cloned().collect();
        let total_instances = instance_ids.len();
        drop(state);
        
        if max_unavailable >= total_instances {
            return Err(DeploymentError::InvalidConfiguration("max_unavailable too large".to_string()));
        }
        
        self.update_phase(DeploymentPhase::Deploying).await;
        
        // Process instances in batches
        let mut deployed_instances = Vec::new();
        let mut failed_instances = Vec::new();
        
        for batch in instance_ids.chunks(batch_size) {
            println!("Deploying batch of {} instances", batch.len());
            
            // Check if we would exceed max_unavailable
            let currently_unavailable = self.count_unavailable_instances().await;
            if currently_unavailable + batch.len() > max_unavailable {
                println!("Waiting for instances to become available...");
                self.wait_for_available_capacity(max_unavailable - batch.len()).await?;
            }
            
            // Deploy batch
            let batch_result = self.deploy_batch(batch, &target_version).await;
            
            match batch_result {
                Ok(successful) => {
                    deployed_instances.extend(successful);
                    println!("Batch deployed successfully");
                }
                Err(e) => {
                    println!("Batch deployment failed: {:?}", e);
                    failed_instances.extend(batch.iter().cloned());
                    
                    // Decide whether to continue or rollback
                    if self.should_continue_rolling(&failed_instances, total_instances) {
                        println!("Continuing deployment despite batch failure");
                        continue;
                    } else {
                        println!("Too many failures, initiating rollback");
                        return self.rollback_rolling(&deployed_instances).await;
                    }
                }
            }
            
            // Wait between batches
            if batch_interval > Duration::ZERO {
                tokio::time::sleep(batch_interval).await;
            }
        }
        
        self.update_phase(DeploymentPhase::Completed).await;
        
        let state = self.state.read().await;
        Ok(DeploymentResult {
            success: failed_instances.is_empty(),
            version: target_version,
            duration: Instant::now().duration_since(state.start_time),
            instances_deployed: deployed_instances.len(),
            rollback_performed: false,
            validation_results: None,
        })
    }
    
    async fn deploy_batch(&self, instances: &[String], version: &str) -> Result<Vec<String>, DeploymentError> {
        let mut successful = Vec::new();
        
        for instance_id in instances {
            // Mark instance as deploying
            {
                let mut state = self.state.write().await;
                if let Some(instance) = state.instances.get_mut(instance_id) {
                    instance.status = InstanceStatus::Deploying;
                    instance.version = version.to_string();
                    instance.deployment_time = Some(Instant::now());
                }
            }
            
            // Simulate deployment
            match self.deploy_to_instance(instance_id, version).await {
                Ok(_) => {
                    // Wait for health check
                    match self.wait_for_instance_health(instance_id).await {
                        Ok(_) => {
                            successful.push(instance_id.clone());
                            println!("Instance {} deployed successfully", instance_id);
                        }
                        Err(e) => {
                            println!("Health check failed for instance {}: {:?}", instance_id, e);
                            return Err(e);
                        }
                    }
                }
                Err(e) => {
                    println!("Deployment failed for instance {}: {:?}", instance_id, e);
                    return Err(e);
                }
            }
        }
        
        Ok(successful)
    }
    
    async fn count_unavailable_instances(&self) -> usize {
        let state = self.state.read().await;
        state.instances.values()
            .filter(|instance| match instance.status {
                InstanceStatus::Deploying | InstanceStatus::Stopping | InstanceStatus::Failed => true,
                InstanceStatus::Running | InstanceStatus::Stopped => false,
            })
            .count()
    }
    
    async fn wait_for_available_capacity(&self, max_unavailable: usize) -> Result<(), DeploymentError> {
        let max_wait = Duration::from_secs(600); // 10 minutes
        let start_time = Instant::now();
        
        while start_time.elapsed() < max_wait {
            let unavailable = self.count_unavailable_instances().await;
            if unavailable <= max_unavailable {
                return Ok(());
            }
            
            println!("Waiting for capacity: {} unavailable (max: {})", unavailable, max_unavailable);
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
        
        Err(DeploymentError::Timeout("Timeout waiting for available capacity".to_string()))
    }
    
    fn should_continue_rolling(&self, failed_instances: &[String], total_instances: usize) -> bool {
        let failure_rate = failed_instances.len() as f32 / total_instances as f32;
        failure_rate < 0.2 // Continue if less than 20% failure rate
    }
    
    async fn rollback_rolling(&self, deployed_instances: &[String]) -> Result<DeploymentResult, DeploymentError> {
        println!("Rolling back rolling deployment");
        
        self.update_phase(DeploymentPhase::RollingBack).await;
        
        // Rollback deployed instances to previous version
        for instance_id in deployed_instances {
            println!("Rolling back instance {}", instance_id);
            
            // This would typically restore from backup or deploy previous version
            let mut state = self.state.write().await;
            if let Some(instance) = state.instances.get_mut(instance_id) {
                instance.status = InstanceStatus::Deploying; // Temporarily
                // instance.version would be set to previous version
            }
        }
        
        let mut state = self.state.write().await;
        state.phase = DeploymentPhase::Failed;
        state.rollback_triggered = true;
        
        Ok(DeploymentResult {
            success: false,
            version: state.target_version.clone(),
            duration: Instant::now().duration_since(state.start_time),
            instances_deployed: 0,
            rollback_performed: true,
            validation_results: None,
        })
    }
}
```

## BitCraps Production Deployment

For BitCraps, deployment strategies must account for gaming-specific requirements like maintaining player sessions and game state consistency.

```rust
/// BitCraps-specific deployment orchestrator
pub struct BitCrapsDeployment {
    base_orchestrator: DeploymentOrchestrator,
    game_state_manager: GameStateManager,
    session_manager: SessionManager,
    bet_reconciler: BetReconciler,
}

impl BitCrapsDeployment {
    pub async fn deploy_gaming_version(&self, version: String) -> Result<DeploymentResult, DeploymentError> {
        println!("Starting BitCraps-aware deployment for version {}", version);
        
        // Phase 1: Pre-deployment preparations
        self.prepare_gaming_deployment().await?;
        
        // Phase 2: Handle active games
        let active_games = self.handle_active_games().await?;
        
        // Phase 3: Execute deployment with gaming awareness
        let mut result = match self.base_orchestrator.config.strategy {
            DeploymentStrategy::BlueGreen { .. } => {
                self.deploy_gaming_blue_green(version).await?
            }
            DeploymentStrategy::Canary { .. } => {
                self.deploy_gaming_canary(version).await?
            }
            _ => {
                return Err(DeploymentError::UnsupportedStrategy("Gaming deployments only support blue-green and canary".to_string()));
            }
        };
        
        // Phase 4: Post-deployment validation
        self.validate_gaming_deployment(&active_games).await?;
        
        Ok(result)
    }
    
    async fn prepare_gaming_deployment(&self) -> Result<(), DeploymentError> {
        println!("Preparing gaming deployment...");
        
        // Backup current game states
        self.game_state_manager.create_backup().await?;
        
        // Ensure all bets are settled
        self.bet_reconciler.settle_pending_bets().await?;
        
        // Prepare session migration
        self.session_manager.prepare_migration().await?;
        
        Ok(())
    }
    
    async fn handle_active_games(&self) -> Result<ActiveGameSnapshot, DeploymentError> {
        println!("Handling active games...");
        
        let active_games = self.game_state_manager.get_active_games().await?;
        println!("Found {} active games", active_games.len());
        
        // For each active game, decide on handling strategy
        let mut game_strategies = HashMap::new();
        
        for game in &active_games {
            let strategy = match game.phase {
                GamePhase::Waiting => GameHandlingStrategy::AllowMigration,
                GamePhase::Betting => {
                    if game.betting_deadline.duration_since(Instant::now()) > Duration::from_secs(30) {
                        GameHandlingStrategy::WaitForRoll
                    } else {
                        GameHandlingStrategy::AllowMigration
                    }
                }
                GamePhase::Rolling => GameHandlingStrategy::WaitForCompletion,
                GamePhase::Payout => GameHandlingStrategy::WaitForCompletion,
            };
            
            game_strategies.insert(game.id.clone(), strategy);
        }
        
        // Wait for games that need completion
        self.wait_for_games_completion(&game_strategies).await?;
        
        Ok(ActiveGameSnapshot {
            games: active_games,
            strategies: game_strategies,
        })
    }
    
    async fn deploy_gaming_blue_green(&self, version: String) -> Result<DeploymentResult, DeploymentError> {
        println!("Executing gaming-aware blue-green deployment");
        
        // Standard blue-green with gaming modifications
        let mut result = self.base_orchestrator.deploy(version.clone()).await?;
        
        // Gaming-specific validations
        self.validate_game_state_consistency().await?;
        self.validate_session_continuity().await?;
        self.validate_bet_integrity().await?;
        
        Ok(result)
    }
    
    async fn deploy_gaming_canary(&self, version: String) -> Result<DeploymentResult, DeploymentError> {
        println!("Executing gaming-aware canary deployment");
        
        // Modified canary deployment for gaming
        let config = CanaryConfig {
            initial_percentage: 5.0, // Start smaller for gaming
            increment_percentage: 5.0, // Smaller increments
            gaming_aware_routing: true, // Route by game session affinity
            preserve_sessions: true,
        };
        
        self.execute_gaming_canary(version, config).await
    }
    
    async fn execute_gaming_canary(&self, version: String, config: CanaryConfig) -> Result<DeploymentResult, DeploymentError> {
        let mut current_percentage = config.initial_percentage;
        
        while current_percentage < 100.0 {
            println!("Gaming canary at {:.1}%", current_percentage);
            
            // Route traffic with session affinity
            self.route_gaming_traffic_with_affinity(current_percentage).await?;
            
            // Monitor gaming-specific metrics
            let gaming_metrics = self.monitor_gaming_metrics(Duration::from_secs(300)).await?;
            
            if !gaming_metrics.acceptable {
                println!("Gaming metrics failed: {:?}", gaming_metrics.issues);
                return self.rollback_with_session_recovery().await;
            }
            
            current_percentage += config.increment_percentage;
        }
        
        Ok(DeploymentResult {
            success: true,
            version,
            duration: Duration::from_secs(1800), // Typical gaming deployment time
            instances_deployed: 10,
            rollback_performed: false,
            validation_results: None,
        })
    }
    
    async fn route_gaming_traffic_with_affinity(&self, percentage: f32) -> Result<(), DeploymentError> {
        // Route traffic based on session affinity and game state
        let routing_rules = vec![
            RoutingRule {
                condition: "session.new == true".to_string(),
                target_percentage: percentage,
            },
            RoutingRule {
                condition: "game.phase == 'waiting'".to_string(),
                target_percentage: percentage * 0.5, // More conservative for games
            },
            RoutingRule {
                condition: "game.phase == 'active'".to_string(),
                target_percentage: 0.0, // Never route active games to canary
            },
        ];
        
        self.base_orchestrator.traffic_router.apply_rules(routing_rules).await?;
        Ok(())
    }
    
    async fn monitor_gaming_metrics(&self, duration: Duration) -> Result<GamingMetrics, DeploymentError> {
        let start_time = Instant::now();
        let mut metrics = GamingMetrics {
            acceptable: true,
            issues: Vec::new(),
            game_completion_rate: 100.0,
            bet_success_rate: 100.0,
            session_continuity_rate: 100.0,
            average_game_latency: Duration::from_millis(50),
        };
        
        while start_time.elapsed() < duration {
            tokio::time::sleep(Duration::from_secs(30)).await;
            
            // Check game-specific metrics
            let game_metrics = self.game_state_manager.get_metrics().await?;
            let session_metrics = self.session_manager.get_metrics().await?;
            let bet_metrics = self.bet_reconciler.get_metrics().await?;
            
            // Update aggregated metrics
            metrics.game_completion_rate = game_metrics.completion_rate;
            metrics.bet_success_rate = bet_metrics.success_rate;
            metrics.session_continuity_rate = session_metrics.continuity_rate;
            metrics.average_game_latency = game_metrics.average_latency;
            
            // Check acceptability thresholds
            if metrics.game_completion_rate < 95.0 {
                metrics.acceptable = false;
                metrics.issues.push("Low game completion rate".to_string());
            }
            
            if metrics.bet_success_rate < 99.0 {
                metrics.acceptable = false;
                metrics.issues.push("Low bet success rate".to_string());
            }
            
            if metrics.session_continuity_rate < 98.0 {
                metrics.acceptable = false;
                metrics.issues.push("Session continuity issues".to_string());
            }
            
            if metrics.average_game_latency > Duration::from_millis(200) {
                metrics.acceptable = false;
                metrics.issues.push("High game latency".to_string());
            }
            
            if !metrics.acceptable {
                break;
            }
            
            println!("Gaming metrics OK: completion={:.1}%, bets={:.1}%, sessions={:.1}%, latency={:?}",
                    metrics.game_completion_rate,
                    metrics.bet_success_rate, 
                    metrics.session_continuity_rate,
                    metrics.average_game_latency);
        }
        
        Ok(metrics)
    }
    
    async fn rollback_with_session_recovery(&self) -> Result<DeploymentResult, DeploymentError> {
        println!("Rolling back with session recovery");
        
        // Standard rollback
        let mut result = self.base_orchestrator.deploy("previous".to_string()).await?;
        
        // Gaming-specific recovery
        self.session_manager.recover_sessions().await?;
        self.game_state_manager.restore_from_backup().await?;
        self.bet_reconciler.reconcile_after_rollback().await?;
        
        result.rollback_performed = true;
        Ok(result)
    }
}

#[derive(Debug, Clone)]
struct ActiveGameSnapshot {
    games: Vec<GameState>,
    strategies: HashMap<String, GameHandlingStrategy>,
}

#[derive(Debug, Clone)]
enum GameHandlingStrategy {
    AllowMigration,
    WaitForRoll,
    WaitForCompletion,
}

#[derive(Debug, Clone)]
struct CanaryConfig {
    initial_percentage: f32,
    increment_percentage: f32,
    gaming_aware_routing: bool,
    preserve_sessions: bool,
}

#[derive(Debug)]
struct GamingMetrics {
    acceptable: bool,
    issues: Vec<String>,
    game_completion_rate: f32,
    bet_success_rate: f32,
    session_continuity_rate: f32,
    average_game_latency: Duration,
}
```

## Automated Rollback and Recovery

Sophisticated rollback mechanisms are essential for production resilience.

```rust
/// Automated rollback system
pub struct RollbackManager {
    deployment_history: Vec<DeploymentRecord>,
    rollback_triggers: Vec<RollbackTrigger>,
    recovery_strategies: HashMap<String, RecoveryStrategy>,
}

#[derive(Debug, Clone)]
pub struct DeploymentRecord {
    pub version: String,
    pub timestamp: Instant,
    pub instances: Vec<String>,
    pub configuration_backup: String,
    pub database_backup: Option<String>,
    pub success: bool,
}

#[derive(Debug, Clone)]
pub enum RollbackTrigger {
    ErrorRateThreshold { threshold: f32, duration: Duration },
    LatencyThreshold { threshold: Duration, percentile: f32 },
    AvailabilityThreshold { threshold: f32 },
    CustomMetric { name: String, threshold: f64, comparison: Comparison },
    ManualTrigger,
}

#[derive(Debug, Clone)]
pub enum Comparison {
    GreaterThan,
    LessThan,
    Equals,
}

impl RollbackManager {
    pub async fn monitor_and_rollback(&self, deployment_id: String) -> Result<(), DeploymentError> {
        println!("Starting rollback monitoring for deployment {}", deployment_id);
        
        let monitoring_duration = Duration::from_secs(3600); // Monitor for 1 hour
        let start_time = Instant::now();
        
        while start_time.elapsed() < monitoring_duration {
            tokio::time::sleep(Duration::from_secs(30)).await;
            
            // Check all rollback triggers
            for trigger in &self.rollback_triggers {
                if self.evaluate_trigger(trigger, &deployment_id).await? {
                    println!("Rollback trigger activated: {:?}", trigger);
                    return self.execute_automatic_rollback(deployment_id).await;
                }
            }
        }
        
        println!("Deployment {} passed monitoring period", deployment_id);
        Ok(())
    }
    
    async fn evaluate_trigger(&self, trigger: &RollbackTrigger, deployment_id: &str) -> Result<bool, DeploymentError> {
        match trigger {
            RollbackTrigger::ErrorRateThreshold { threshold, duration } => {
                let error_rate = self.get_error_rate_over_duration(*duration).await?;
                Ok(error_rate > *threshold)
            }
            RollbackTrigger::LatencyThreshold { threshold, percentile } => {
                let latency = self.get_latency_percentile(*percentile).await?;
                Ok(latency > *threshold)
            }
            RollbackTrigger::AvailabilityThreshold { threshold } => {
                let availability = self.get_current_availability().await?;
                Ok(availability < *threshold)
            }
            RollbackTrigger::CustomMetric { name, threshold, comparison } => {
                let value = self.get_custom_metric(name).await?;
                Ok(match comparison {
                    Comparison::GreaterThan => value > *threshold,
                    Comparison::LessThan => value < *threshold,
                    Comparison::Equals => (value - threshold).abs() < 0.001,
                })
            }
            RollbackTrigger::ManualTrigger => Ok(false), // Would be triggered externally
        }
    }
    
    async fn execute_automatic_rollback(&self, deployment_id: String) -> Result<(), DeploymentError> {
        println!("Executing automatic rollback for deployment {}", deployment_id);
        
        // Find the deployment record
        let deployment = self.deployment_history.iter()
            .find(|d| d.version == deployment_id)
            .ok_or(DeploymentError::DeploymentNotFound)?;
        
        // Find the previous successful deployment
        let previous_deployment = self.deployment_history.iter()
            .rev()
            .find(|d| d.success && d.timestamp < deployment.timestamp)
            .ok_or(DeploymentError::NoPreviousVersion)?;
        
        println!("Rolling back from {} to {}", deployment.version, previous_deployment.version);
        
        // Execute rollback strategy
        let strategy = self.recovery_strategies.get("default")
            .ok_or(DeploymentError::NoRecoveryStrategy)?;
        
        match strategy {
            RecoveryStrategy::BlueGreenSwitch => {
                self.execute_blue_green_rollback(previous_deployment).await?;
            }
            RecoveryStrategy::RollingRollback { batch_size } => {
                self.execute_rolling_rollback(previous_deployment, *batch_size).await?;
            }
            RecoveryStrategy::ImmediateSwitch => {
                self.execute_immediate_rollback(previous_deployment).await?;
            }
        }
        
        // Verify rollback success
        self.verify_rollback_success(previous_deployment).await?;
        
        println!("Automatic rollback completed successfully");
        Ok(())
    }
    
    async fn execute_blue_green_rollback(&self, target_deployment: &DeploymentRecord) -> Result<(), DeploymentError> {
        println!("Executing blue-green rollback");
        
        // Switch traffic back to blue (previous version)
        // This assumes blue-green infrastructure is still available
        
        // Implementation would:
        // 1. Route 100% traffic to previous version instances
        // 2. Verify health of previous version
        // 3. Clean up failed deployment instances
        
        tokio::time::sleep(Duration::from_secs(5)).await; // Simulate rollback time
        Ok(())
    }
    
    async fn execute_rolling_rollback(&self, target_deployment: &DeploymentRecord, batch_size: usize) -> Result<(), DeploymentError> {
        println!("Executing rolling rollback");
        
        // Roll back instances in batches
        let failed_instances = self.get_failed_deployment_instances().await?;
        
        for batch in failed_instances.chunks(batch_size) {
            for instance in batch {
                println!("Rolling back instance {}", instance);
                // Restore instance to previous version
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            
            // Wait between batches
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
        
        Ok(())
    }
    
    async fn execute_immediate_rollback(&self, target_deployment: &DeploymentRecord) -> Result<(), DeploymentError> {
        println!("Executing immediate rollback (emergency mode)");
        
        // Immediately switch all traffic and instances
        // This may cause brief service interruption but minimizes damage
        
        tokio::time::sleep(Duration::from_secs(2)).await; // Simulate immediate switch
        Ok(())
    }
    
    async fn verify_rollback_success(&self, target_deployment: &DeploymentRecord) -> Result<(), DeploymentError> {
        println!("Verifying rollback success");
        
        let verification_duration = Duration::from_secs(300); // 5 minutes
        let start_time = Instant::now();
        
        while start_time.elapsed() < verification_duration {
            tokio::time::sleep(Duration::from_secs(30)).await;
            
            let error_rate = self.get_error_rate_over_duration(Duration::from_secs(60)).await?;
            let availability = self.get_current_availability().await?;
            
            if error_rate < 1.0 && availability > 99.0 {
                println!("Rollback verification successful");
                return Ok(());
            }
            
            println!("Rollback verification: error_rate={:.2}%, availability={:.2}%", 
                    error_rate, availability);
        }
        
        Err(DeploymentError::RollbackVerificationFailed)
    }
}

#[derive(Debug, Clone)]
enum RecoveryStrategy {
    BlueGreenSwitch,
    RollingRollback { batch_size: usize },
    ImmediateSwitch,
}
```

## Common Pitfalls and Solutions

1. **Session Affinity**: Use consistent hashing or session stores
2. **Database Migrations**: Always use backward-compatible changes
3. **Configuration Drift**: Use infrastructure as code
4. **Monitoring Gaps**: Monitor deployment-specific metrics
5. **Rollback Testing**: Regularly test rollback procedures

## Practical Exercises

1. **Implement Circuit Breakers**: Add circuit breakers to deployment pipelines
2. **Build Deployment Dashboard**: Create real-time deployment monitoring
3. **Test Rollback Scenarios**: Practice various rollback situations
4. **Optimize for Gaming**: Adapt strategies for real-time applications
5. **Chaos Testing**: Introduce failures during deployments

## Conclusion

Production deployment strategies are the culmination of everything we've learned about distributed systems. They require careful orchestration of monitoring, traffic management, health checking, and rollback mechanisms. The patterns we've explored—blue-green, canary, and rolling deployments—each have their place in the deployment toolkit.

For BitCraps and similar real-time distributed applications, deployment strategies must be even more sophisticated, accounting for active user sessions, game state consistency, and the need for zero-disruption deployments. The investment in robust deployment automation pays dividends in reduced risk, faster time-to-market, and improved system reliability.

Remember: a great deployment strategy is invisible when it works and invaluable when it doesn't.

## Additional Resources

- "Release It!" by Michael Nygard
- "Continuous Delivery" by Jez Humble and David Farley
- "The Phoenix Project" by Gene Kim
- Netflix Technology Blog on Deployment Strategies
- AWS Blue/Green Deployment Documentation

---

*This concludes the BitCraps Feynman Curriculum - 100 chapters of distributed systems mastery.*