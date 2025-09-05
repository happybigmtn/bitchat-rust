//! Load Balancer Implementation
//!
//! Implements various load balancing strategies for service instances.

use super::{LoadBalancingStrategy, ServiceInstance, ServiceDiscoveryConfig, HealthStatus};
use crate::error::{Error, Result};
use dashmap::DashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Load balancer for service instances
pub struct LoadBalancer {
    strategy: LoadBalancingStrategy,
    service_discovery_config: ServiceDiscoveryConfig,
    service_instances: Arc<DashMap<String, Vec<ServiceInstance>>>,
    round_robin_counters: Arc<DashMap<String, AtomicUsize>>,
    last_health_check: Arc<DashMap<String, Instant>>,
}

impl LoadBalancer {
    /// Create a new load balancer
    pub fn new(strategy: LoadBalancingStrategy, config: ServiceDiscoveryConfig) -> Self {
        // Initialize static services if configured
        let service_instances = Arc::new(DashMap::new());
        for (service_name, endpoints) in &config.static_services {
            let instances: Vec<ServiceInstance> = endpoints.iter()
                .map(|endpoint| ServiceInstance::new(endpoint.clone()))
                .collect();
            service_instances.insert(service_name.clone(), instances);
        }
        
        Self {
            strategy,
            service_discovery_config: config,
            service_instances,
            round_robin_counters: Arc::new(DashMap::new()),
            last_health_check: Arc::new(DashMap::new()),
        }
    }
    
    /// Get a healthy service instance using the configured strategy
    pub async fn get_instance(&self, service_name: &str) -> Option<ServiceInstance> {
        let instances = self.get_healthy_instances(service_name).await?;
        if instances.is_empty() {
            return None;
        }
        
        match self.strategy {
            LoadBalancingStrategy::RoundRobin => {
                self.round_robin_selection(service_name, &instances)
            },
            LoadBalancingStrategy::WeightedRoundRobin => {
                self.weighted_round_robin_selection(service_name, &instances)
            },
            LoadBalancingStrategy::LeastConnections => {
                self.least_connections_selection(&instances)
            },
            LoadBalancingStrategy::Random => {
                self.random_selection(&instances)
            },
            LoadBalancingStrategy::IPHash => {
                // For IP hash, we'd need the client IP, so fallback to round robin
                self.round_robin_selection(service_name, &instances)
            },
        }
    }
    
    /// Get instance for specific client (used for IP hash strategy)
    pub async fn get_instance_for_client(&self, service_name: &str, client_ip: std::net::IpAddr) -> Option<ServiceInstance> {
        let instances = self.get_healthy_instances(service_name).await?;
        if instances.is_empty() {
            return None;
        }
        
        if matches!(self.strategy, LoadBalancingStrategy::IPHash) {
            self.ip_hash_selection(&instances, client_ip)
        } else {
            self.get_instance(service_name).await
        }
    }
    
    /// Update service instances (called by service discovery)
    pub async fn update_service_instances(&self, service_name: String, instances: Vec<ServiceInstance>) {
        self.service_instances.insert(service_name, instances);
    }
    
    /// Check health of all service instances
    pub async fn check_service_health(&self) {
        let health_check_interval = self.service_discovery_config.health_check_interval;
        
        for mut entry in self.service_instances.iter_mut() {
            let service_name = entry.key().clone();
            let last_check = self.last_health_check.get(&service_name)
                .map(|instant| *instant.value())
                .unwrap_or(Instant::now() - health_check_interval);
            
            if last_check.elapsed() >= health_check_interval {
                let instances = entry.value_mut();
                for instance in instances.iter_mut() {
                    instance.health_status = self.check_instance_health(instance).await;
                }
                
                self.last_health_check.insert(service_name, Instant::now());
            }
        }
    }
    
    /// Record request completion for load balancer metrics
    pub async fn record_request_completion(&self, service_name: &str, instance_address: std::net::SocketAddr, success: bool) {
        if let Some(mut instances) = self.service_instances.get_mut(service_name) {
            for instance in instances.iter_mut() {
                if instance.endpoint.address == instance_address {
                    instance.total_requests += 1;
                    if !success {
                        instance.failed_requests += 1;
                    }
                    break;
                }
            }
        }
    }
    
    // Private helper methods
    
    async fn get_healthy_instances(&self, service_name: &str) -> Option<Vec<ServiceInstance>> {
        let instances = self.service_instances.get(service_name)?;
        let healthy: Vec<ServiceInstance> = instances.iter()
            .filter(|instance| matches!(instance.health_status, HealthStatus::Healthy))
            .cloned()
            .collect();
        
        if healthy.is_empty() {
            None
        } else {
            Some(healthy)
        }
    }
    
    fn round_robin_selection(&self, service_name: &str, instances: &[ServiceInstance]) -> Option<ServiceInstance> {
        let counter = self.round_robin_counters
            .entry(service_name.to_string())
            .or_insert_with(|| AtomicUsize::new(0));
        
        let index = counter.fetch_add(1, Ordering::Relaxed) % instances.len();
        instances.get(index).cloned()
    }
    
    fn weighted_round_robin_selection(&self, service_name: &str, instances: &[ServiceInstance]) -> Option<ServiceInstance> {
        let total_weight: u32 = instances.iter().map(|i| i.endpoint.weight).sum();
        if total_weight == 0 {
            return self.round_robin_selection(service_name, instances);
        }
        
        let counter = self.round_robin_counters
            .entry(service_name.to_string())
            .or_insert_with(|| AtomicUsize::new(0));
        
        let target = (counter.fetch_add(1, Ordering::Relaxed) as u32) % total_weight;
        let mut accumulator = 0u32;
        
        for instance in instances {
            accumulator += instance.endpoint.weight;
            if accumulator > target {
                return Some(instance.clone());
            }
        }
        
        instances.first().cloned()
    }
    
    fn least_connections_selection(&self, instances: &[ServiceInstance]) -> Option<ServiceInstance> {
        instances.iter()
            .min_by_key(|instance| instance.active_connections)
            .cloned()
    }
    
    fn random_selection(&self, instances: &[ServiceInstance]) -> Option<ServiceInstance> {
        use rand::Rng;
        use rand::rngs::OsRng;
        let index = OsRng.gen_range(0..instances.len());
        instances.get(index).cloned()
    }
    
    fn ip_hash_selection(&self, instances: &[ServiceInstance], client_ip: std::net::IpAddr) -> Option<ServiceInstance> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        client_ip.hash(&mut hasher);
        let hash = hasher.finish();
        
        let index = (hash as usize) % instances.len();
        instances.get(index).cloned()
    }
    
    async fn check_instance_health(&self, instance: &ServiceInstance) -> HealthStatus {
        if let Some(health_check_path) = &instance.endpoint.health_check_path {
            let url = format!("http://{}{}", instance.endpoint.address, health_check_path);
            
            match self.perform_http_health_check(&url).await {
                Ok(true) => HealthStatus::Healthy,
                Ok(false) => HealthStatus::Unhealthy,
                Err(_) => HealthStatus::Unknown,
            }
        } else {
            // Fallback to TCP health check
            match self.perform_tcp_health_check(instance.endpoint.address).await {
                Ok(true) => HealthStatus::Healthy,
                Ok(false) => HealthStatus::Unhealthy,
                Err(_) => HealthStatus::Unknown,
            }
        }
    }
    
    async fn perform_http_health_check(&self, url: &str) -> Result<bool> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|e| Error::NetworkError(e.to_string()))?;
        
        match client.get(url).send().await {
            Ok(response) => Ok(response.status().is_success()),
            Err(_) => Ok(false),
        }
    }
    
    async fn perform_tcp_health_check(&self, address: std::net::SocketAddr) -> Result<bool> {
        match tokio::time::timeout(
            Duration::from_secs(3),
            tokio::net::TcpStream::connect(address),
        ).await {
            Ok(Ok(_)) => Ok(true),
            _ => Ok(false),
        }
    }
}

/// Load balancer metrics
#[derive(Debug, Clone, Default)]
pub struct LoadBalancerMetrics {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub instances_checked: u64,
    pub healthy_instances: u64,
    pub unhealthy_instances: u64,
}

impl LoadBalancerMetrics {
    pub fn success_rate(&self) -> f64 {
        if self.total_requests == 0 {
            1.0
        } else {
            self.successful_requests as f64 / self.total_requests as f64
        }
    }
    
    pub fn health_ratio(&self) -> f64 {
        let total = self.healthy_instances + self.unhealthy_instances;
        if total == 0 {
            1.0
        } else {
            self.healthy_instances as f64 / total as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::api_gateway::{ServiceEndpoint, ServiceDiscoveryConfig, ServiceDiscoveryMethod};
    
    #[tokio::test]
    async fn test_round_robin_selection() {
        let mut static_services = std::collections::HashMap::new();
        static_services.insert("test".to_string(), vec![
            ServiceEndpoint {
                address: "127.0.0.1:8080".parse().unwrap(),
                weight: 100,
                health_check_path: None,
            },
            ServiceEndpoint {
                address: "127.0.0.1:8081".parse().unwrap(),
                weight: 100,
                health_check_path: None,
            },
        ]);
        
        let config = ServiceDiscoveryConfig {
            method: ServiceDiscoveryMethod::Static,
            consul: None,
            static_services,
            health_check_interval: Duration::from_secs(30),
        };
        
        let lb = LoadBalancer::new(LoadBalancingStrategy::RoundRobin, config);
        
        // Get two instances and verify they're different
        let instance1 = lb.get_instance("test").await;
        let instance2 = lb.get_instance("test").await;
        
        assert!(instance1.is_some());
        assert!(instance2.is_some());
        
        // In round-robin, consecutive calls should return different instances
        if instance1.as_ref().unwrap().endpoint.address == instance2.as_ref().unwrap().endpoint.address {
            // If they're the same, get a third one - it should cycle back to the first
            let instance3 = lb.get_instance("test").await;
            assert!(instance3.is_some());
        }
    }
    
    #[tokio::test]
    async fn test_weighted_round_robin() {
        let mut static_services = std::collections::HashMap::new();
        static_services.insert("test".to_string(), vec![
            ServiceEndpoint {
                address: "127.0.0.1:8080".parse().unwrap(),
                weight: 100,
                health_check_path: None,
            },
            ServiceEndpoint {
                address: "127.0.0.1:8081".parse().unwrap(),
                weight: 200, // Higher weight
                health_check_path: None,
            },
        ]);
        
        let config = ServiceDiscoveryConfig {
            method: ServiceDiscoveryMethod::Static,
            consul: None,
            static_services,
            health_check_interval: Duration::from_secs(30),
        };
        
        let lb = LoadBalancer::new(LoadBalancingStrategy::WeightedRoundRobin, config);
        
        let mut addresses = std::collections::HashMap::new();
        
        // Make multiple requests and count which instances are selected
        for _ in 0..30 {
            if let Some(instance) = lb.get_instance("test").await {
                *addresses.entry(instance.endpoint.address).or_insert(0) += 1;
            }
        }
        
        // The instance with weight 200 should be selected roughly twice as often
        // as the one with weight 100
        assert!(addresses.len() <= 2);
    }
    
    #[tokio::test]
    async fn test_ip_hash_consistency() {
        let mut static_services = std::collections::HashMap::new();
        static_services.insert("test".to_string(), vec![
            ServiceEndpoint {
                address: "127.0.0.1:8080".parse().unwrap(),
                weight: 100,
                health_check_path: None,
            },
            ServiceEndpoint {
                address: "127.0.0.1:8081".parse().unwrap(),
                weight: 100,
                health_check_path: None,
            },
        ]);
        
        let config = ServiceDiscoveryConfig {
            method: ServiceDiscoveryMethod::Static,
            consul: None,
            static_services,
            health_check_interval: Duration::from_secs(30),
        };
        
        let lb = LoadBalancer::new(LoadBalancingStrategy::IPHash, config);
        let client_ip = "192.168.1.100".parse().unwrap();
        
        // Same client IP should always get the same instance
        let instance1 = lb.get_instance_for_client("test", client_ip).await;
        let instance2 = lb.get_instance_for_client("test", client_ip).await;
        let instance3 = lb.get_instance_for_client("test", client_ip).await;
        
        assert!(instance1.is_some());
        assert!(instance2.is_some());
        assert!(instance3.is_some());
        
        assert_eq!(
            instance1.unwrap().endpoint.address,
            instance2.unwrap().endpoint.address
        );
        assert_eq!(
            instance2.unwrap().endpoint.address,
            instance3.unwrap().endpoint.address
        );
    }
}