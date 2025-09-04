//! Service Discovery Implementation
//!
//! Implements service discovery patterns for microservices communication.

use super::*;
use crate::error::{Error, Result};
use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::RwLock;

/// Static service discovery implementation
pub struct StaticServiceDiscovery {
    services: Arc<DashMap<String, Vec<ServiceInstance>>>,
    health_checker: Arc<HealthChecker>,
}

impl StaticServiceDiscovery {
    pub fn new() -> Self {
        Self {
            services: Arc::new(DashMap::new()),
            health_checker: Arc::new(HealthChecker::new()),
        }
    }
    
    pub fn add_service_instance(&self, service_name: String, instance: ServiceInstance) {
        self.services.entry(service_name).or_insert_with(Vec::new).push(instance);
    }
    
    pub async fn start_health_checking(&self, interval: Duration) {
        let services = self.services.clone();
        let health_checker = self.health_checker.clone();
        
        tokio::spawn(async move {
            let mut health_interval = tokio::time::interval(interval);
            
            loop {
                health_interval.tick().await;
                
                for mut entry in services.iter_mut() {
                    let instances = entry.value_mut();
                    for instance in instances.iter_mut() {
                        let health = health_checker.check_instance_health(instance).await;
                        instance.health = health;
                    }
                }
            }
        });
    }
}

#[async_trait::async_trait]
impl ServiceDiscovery for StaticServiceDiscovery {
    async fn register(&self, registration: ServiceRegistration) -> Result<()> {
        let instance = ServiceInstance {
            service_id: registration.service_id,
            service_name: registration.service_name.clone(),
            address: registration.address,
            tags: registration.tags,
            metadata: registration.metadata,
            health: ServiceHealth::Unknown,
        };
        
        self.add_service_instance(registration.service_name, instance);
        Ok(())
    }
    
    async fn deregister(&self, service_id: &str) -> Result<()> {
        for mut entry in self.services.iter_mut() {
            let instances = entry.value_mut();
            instances.retain(|instance| instance.service_id != service_id);
        }
        Ok(())
    }
    
    async fn discover(&self, service_name: &str) -> Result<Vec<ServiceInstance>> {
        match self.services.get(service_name) {
            Some(instances) => Ok(instances.clone()),
            None => Ok(Vec::new()),
        }
    }
    
    async fn list_services(&self) -> Result<Vec<String>> {
        Ok(self.services.iter().map(|entry| entry.key().clone()).collect())
    }
    
    async fn health_check(&self, service_id: &str) -> Result<ServiceHealth> {
        for entry in self.services.iter() {
            for instance in entry.value().iter() {
                if instance.service_id == service_id {
                    return Ok(instance.health);
                }
            }
        }
        Err(Error::ServiceError("Service not found".to_string()))
    }
}

/// Consul service discovery implementation
pub struct ConsulServiceDiscovery {
    client: ConsulClient,
    registered_services: Arc<RwLock<Vec<String>>>,
}

impl ConsulServiceDiscovery {
    pub fn new(consul_address: &str, datacenter: Option<&str>, token: Option<&str>) -> Result<Self> {
        let client = ConsulClient::new(consul_address, datacenter, token)?;
        
        Ok(Self {
            client,
            registered_services: Arc::new(RwLock::new(Vec::new())),
        })
    }
}

#[async_trait::async_trait]
impl ServiceDiscovery for ConsulServiceDiscovery {
    async fn register(&self, registration: ServiceRegistration) -> Result<()> {
        self.client.register_service(&registration).await?;
        
        let mut services = self.registered_services.write().await;
        services.push(registration.service_id);
        
        Ok(())
    }
    
    async fn deregister(&self, service_id: &str) -> Result<()> {
        self.client.deregister_service(service_id).await?;
        
        let mut services = self.registered_services.write().await;
        services.retain(|id| id != service_id);
        
        Ok(())
    }
    
    async fn discover(&self, service_name: &str) -> Result<Vec<ServiceInstance>> {
        self.client.discover_service(service_name).await
    }
    
    async fn list_services(&self) -> Result<Vec<String>> {
        self.client.list_services().await
    }
    
    async fn health_check(&self, service_id: &str) -> Result<ServiceHealth> {
        self.client.get_service_health(service_id).await
    }
}

/// Health checker for service instances
pub struct HealthChecker {
    client: reqwest::Client,
}

impl HealthChecker {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap(),
        }
    }
    
    pub async fn check_instance_health(&self, instance: &ServiceInstance) -> ServiceHealth {
        // Try to find a health check configuration
        if let Some(health_check_path) = instance.metadata.get("health_check_path") {
            let url = format!("http://{}{}", instance.address, health_check_path);
            
            match self.client.get(&url).send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        ServiceHealth::Passing
                    } else if response.status().is_server_error() {
                        ServiceHealth::Critical
                    } else {
                        ServiceHealth::Warning
                    }
                },
                Err(_) => ServiceHealth::Critical,
            }
        } else {
            // Fallback to TCP health check
            self.tcp_health_check(instance.address).await
        }
    }
    
    async fn tcp_health_check(&self, address: std::net::SocketAddr) -> ServiceHealth {
        match tokio::time::timeout(
            Duration::from_secs(3),
            tokio::net::TcpStream::connect(address),
        ).await {
            Ok(Ok(_)) => ServiceHealth::Passing,
            _ => ServiceHealth::Critical,
        }
    }
}

/// Consul client implementation
pub struct ConsulClient {
    base_url: String,
    client: reqwest::Client,
}

impl ConsulClient {
    pub fn new(address: &str, _datacenter: Option<&str>, _token: Option<&str>) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| Error::NetworkError(e.to_string()))?;
        
        Ok(Self {
            base_url: format!("http://{}/v1", address),
            client,
        })
    }
    
    pub async fn register_service(&self, registration: &ServiceRegistration) -> Result<()> {
        let consul_registration = ConsulServiceRegistration {
            id: registration.service_id.clone(),
            name: registration.service_name.clone(),
            tags: registration.tags.clone(),
            address: registration.address.ip().to_string(),
            port: registration.address.port(),
            check: registration.health_check.as_ref().map(|hc| ConsulHealthCheck {
                http: hc.http.as_ref().map(|http| http.url.clone()),
                tcp: hc.tcp.as_ref().map(|tcp| tcp.address.to_string()),
                interval: format!("{}s", hc.interval.as_secs()),
                timeout: format!("{}s", hc.timeout.as_secs()),
                status: "passing".to_string(),
            }),
        };
        
        let url = format!("{}/agent/service/register", self.base_url);
        let response = self.client
            .put(&url)
            .json(&consul_registration)
            .send()
            .await
            .map_err(|e| Error::NetworkError(e.to_string()))?;
        
        if !response.status().is_success() {
            return Err(Error::ServiceError(format!(
                "Failed to register service: {}",
                response.status()
            )));
        }
        
        Ok(())
    }
    
    pub async fn deregister_service(&self, service_id: &str) -> Result<()> {
        let url = format!("{}/agent/service/deregister/{}", self.base_url, service_id);
        let response = self.client
            .put(&url)
            .send()
            .await
            .map_err(|e| Error::NetworkError(e.to_string()))?;
        
        if !response.status().is_success() {
            return Err(Error::ServiceError(format!(
                "Failed to deregister service: {}",
                response.status()
            )));
        }
        
        Ok(())
    }
    
    pub async fn discover_service(&self, service_name: &str) -> Result<Vec<ServiceInstance>> {
        let url = format!("{}/health/service/{}", self.base_url, service_name);
        let response = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| Error::NetworkError(e.to_string()))?;
        
        if !response.status().is_success() {
            return Err(Error::ServiceError(format!(
                "Failed to discover service: {}",
                response.status()
            )));
        }
        
        let consul_services: Vec<ConsulServiceEntry> = response
            .json()
            .await
            .map_err(|e| Error::SerializationError(e.to_string()))?;
        
        let instances = consul_services
            .into_iter()
            .map(|entry| ServiceInstance {
                service_id: entry.service.id,
                service_name: entry.service.service,
                address: format!("{}:{}", entry.service.address, entry.service.port)
                    .parse()
                    .unwrap(),
                tags: entry.service.tags,
                metadata: std::collections::HashMap::new(),
                health: self.convert_consul_health(&entry.checks),
            })
            .collect();
        
        Ok(instances)
    }
    
    pub async fn list_services(&self) -> Result<Vec<String>> {
        let url = format!("{}/catalog/services", self.base_url);
        let response = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| Error::NetworkError(e.to_string()))?;
        
        if !response.status().is_success() {
            return Err(Error::ServiceError(format!(
                "Failed to list services: {}",
                response.status()
            )));
        }
        
        let services: std::collections::HashMap<String, Vec<String>> = response
            .json()
            .await
            .map_err(|e| Error::SerializationError(e.to_string()))?;
        
        Ok(services.into_keys().collect())
    }
    
    pub async fn get_service_health(&self, service_id: &str) -> Result<ServiceHealth> {
        let url = format!("{}/health/service/{}", self.base_url, service_id);
        let response = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| Error::NetworkError(e.to_string()))?;
        
        if !response.status().is_success() {
            return Ok(ServiceHealth::Unknown);
        }
        
        let consul_services: Vec<ConsulServiceEntry> = response
            .json()
            .await
            .map_err(|e| Error::SerializationError(e.to_string()))?;
        
        if let Some(entry) = consul_services.first() {
            Ok(self.convert_consul_health(&entry.checks))
        } else {
            Ok(ServiceHealth::Unknown)
        }
    }
    
    fn convert_consul_health(&self, checks: &[ConsulHealthCheck]) -> ServiceHealth {
        if checks.is_empty() {
            return ServiceHealth::Unknown;
        }
        
        let mut has_critical = false;
        let mut has_warning = false;
        
        for check in checks {
            match check.status.as_str() {
                "passing" => {},
                "warning" => has_warning = true,
                "critical" => has_critical = true,
                _ => has_warning = true,
            }
        }
        
        if has_critical {
            ServiceHealth::Critical
        } else if has_warning {
            ServiceHealth::Warning
        } else {
            ServiceHealth::Passing
        }
    }
}

// Consul API types
#[derive(serde::Serialize)]
struct ConsulServiceRegistration {
    #[serde(rename = "ID")]
    id: String,
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Tags")]
    tags: Vec<String>,
    #[serde(rename = "Address")]
    address: String,
    #[serde(rename = "Port")]
    port: u16,
    #[serde(rename = "Check", skip_serializing_if = "Option::is_none")]
    check: Option<ConsulHealthCheck>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct ConsulHealthCheck {
    #[serde(rename = "HTTP", skip_serializing_if = "Option::is_none")]
    http: Option<String>,
    #[serde(rename = "TCP", skip_serializing_if = "Option::is_none")]
    tcp: Option<String>,
    #[serde(rename = "Interval")]
    interval: String,
    #[serde(rename = "Timeout")]
    timeout: String,
    #[serde(rename = "Status", default)]
    status: String,
}

#[derive(serde::Deserialize)]
struct ConsulServiceEntry {
    #[serde(rename = "Service")]
    service: ConsulService,
    #[serde(rename = "Checks")]
    checks: Vec<ConsulHealthCheck>,
}

#[derive(serde::Deserialize)]
struct ConsulService {
    #[serde(rename = "ID")]
    id: String,
    #[serde(rename = "Service")]
    service: String,
    #[serde(rename = "Tags")]
    tags: Vec<String>,
    #[serde(rename = "Address")]
    address: String,
    #[serde(rename = "Port")]
    port: u16,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_static_service_discovery() {
        let discovery = StaticServiceDiscovery::new();
        
        let registration = ServiceRegistration {
            service_name: "test-service".to_string(),
            service_id: "test-1".to_string(),
            address: "127.0.0.1:8080".parse().unwrap(),
            tags: vec!["test".to_string()],
            metadata: std::collections::HashMap::new(),
            health_check: None,
            ttl: None,
        };
        
        discovery.register(registration).await.unwrap();
        
        let instances = discovery.discover("test-service").await.unwrap();
        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].service_id, "test-1");
    }
    
    #[tokio::test]
    async fn test_health_checker() {
        let checker = HealthChecker::new();
        
        // Test with a non-existent address
        let health = checker.tcp_health_check("127.0.0.1:99999".parse().unwrap()).await;
        assert_eq!(health, ServiceHealth::Critical);
    }
}