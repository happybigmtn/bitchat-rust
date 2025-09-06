//! Microservices Architecture
//!
//! This module contains the microservices implementation for BitCraps,
//! extracting key components into standalone services.

pub mod api_gateway;
pub mod common;
pub mod consensus;
pub mod game_engine;

#[cfg(feature = "api-gateway")]
pub use api_gateway::ApiGateway;
pub use consensus::ConsensusService;
pub use game_engine::GameEngineService;

use crate::error::{Error, Result};
use common::{ServiceDiscovery, ServiceRegistration};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Service orchestrator that manages all microservices
pub struct ServiceOrchestrator {
    services: Arc<RwLock<Vec<Box<dyn MicroService>>>>,
    service_discovery: Arc<dyn ServiceDiscovery>,
    #[cfg(feature = "api-gateway")]
    gateway: Option<ApiGateway>,
}

/// Microservice trait that all services implement
#[async_trait::async_trait]
pub trait MicroService: Send + Sync {
    /// Get service name
    fn name(&self) -> &str;
    
    /// Get service listening address
    fn address(&self) -> SocketAddr;
    
    /// Start the service
    async fn start(&mut self) -> Result<()>;
    
    /// Stop the service
    async fn stop(&mut self) -> Result<()>;
    
    /// Health check
    async fn health_check(&self) -> Result<ServiceHealth>;
    
    /// Get service registration info
    fn registration_info(&self) -> ServiceRegistration;
}

/// Service health status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceHealth {
    Healthy,
    Degraded,
    Unhealthy,
}

impl ServiceOrchestrator {
    /// Create a new service orchestrator
    pub fn new(service_discovery: Arc<dyn ServiceDiscovery>) -> Self {
        Self {
            services: Arc::new(RwLock::new(Vec::new())),
            service_discovery,
            #[cfg(feature = "api-gateway")]
            gateway: None,
        }
    }
    
    /// Add a microservice to the orchestrator
    pub async fn add_service(&self, service: Box<dyn MicroService>) -> Result<()> {
        // Register service with service discovery
        let registration = service.registration_info();
        self.service_discovery.register(registration).await?;
        
        // Add to our list
        let mut services = self.services.write().await;
        services.push(service);
        
        Ok(())
    }
    
    /// Set up the API gateway
    #[cfg(feature = "api-gateway")]
    pub fn with_gateway(&mut self, gateway: ApiGateway) -> &mut Self {
        #[cfg(feature = "api-gateway")]
        {
            self.gateway = Some(gateway);
        }
        self
    }
    
    /// Start all services
    pub async fn start_all(&mut self) -> Result<()> {
        // Start all microservices
        let mut services = self.services.write().await;
        for service in services.iter_mut() {
            log::info!("Starting service: {}", service.name());
            service.start().await?;
        }
        drop(services);
        
        // Start gateway last
        #[cfg(feature = "api-gateway")]
        if let Some(gateway) = &mut self.gateway {
            log::info!("Starting API Gateway");
            gateway.start().await?;
        }
        
        log::info!("All services started successfully");
        Ok(())
    }
    
    /// Stop all services
    pub async fn stop_all(&mut self) -> Result<()> {
        // Stop gateway first
        #[cfg(feature = "api-gateway")]
        if let Some(gateway) = &mut self.gateway {
            log::info!("Stopping API Gateway");
            gateway.stop().await?;
        }
        
        // Stop all microservices
        let mut services = self.services.write().await;
        for service in services.iter_mut() {
            log::info!("Stopping service: {}", service.name());
            service.stop().await?;
            
            // Deregister from service discovery
            let registration = service.registration_info();
            if let Err(e) = self.service_discovery.deregister(&registration.service_id).await {
                log::warn!("Failed to deregister service {}: {}", service.name(), e);
            }
        }
        
        log::info!("All services stopped");
        Ok(())
    }
    
    /// Check health of all services
    pub async fn health_check_all(&self) -> std::collections::HashMap<String, ServiceHealth> {
        let mut health_status = std::collections::HashMap::new();
        
        let services = self.services.read().await;
        for service in services.iter() {
            let health = service.health_check().await.unwrap_or(ServiceHealth::Unhealthy);
            health_status.insert(service.name().to_string(), health);
        }
        
        // Check gateway health
        #[cfg(feature = "api-gateway")]
        if let Some(_gateway) = &self.gateway {
            // Gateway health would be checked here
            health_status.insert("api-gateway".to_string(), ServiceHealth::Healthy);
        }
        
        health_status
    }
    
    /// Get service discovery instance
    pub fn service_discovery(&self) -> Arc<dyn ServiceDiscovery> {
        self.service_discovery.clone()
    }
}

/// Builder pattern for easy service setup
pub struct ServiceBuilder {
    game_engine_config: Option<game_engine::GameEngineConfig>,
    consensus_config: Option<consensus::ConsensusConfig>,
    #[cfg(feature = "api-gateway")]
    gateway_config: Option<api_gateway::GatewayConfig>,
    service_discovery: Option<Arc<dyn ServiceDiscovery>>,
}

impl ServiceBuilder {
    pub fn new() -> Self {
        Self {
            game_engine_config: None,
            consensus_config: None,
            #[cfg(feature = "api-gateway")]
            gateway_config: None,
            service_discovery: None,
        }
    }
    
    pub fn with_game_engine(mut self, config: game_engine::GameEngineConfig) -> Self {
        self.game_engine_config = Some(config);
        self
    }
    
    pub fn with_consensus(mut self, config: consensus::ConsensusConfig) -> Self {
        self.consensus_config = Some(config);
        self
    }
    
    #[cfg(feature = "api-gateway")]
    pub fn with_gateway(mut self, config: api_gateway::GatewayConfig) -> Self {
        self.gateway_config = Some(config);
        self
    }
    
    pub fn with_service_discovery(mut self, discovery: Arc<dyn ServiceDiscovery>) -> Self {
        self.service_discovery = Some(discovery);
        self
    }
    
    pub async fn build(self) -> Result<ServiceOrchestrator> {
        let service_discovery = self.service_discovery
            .ok_or_else(|| Error::ConfigError("Service discovery not configured".to_string()))?;
        
        let mut orchestrator = ServiceOrchestrator::new(service_discovery);
        
        // Add game engine service if configured
        if let Some(config) = self.game_engine_config {
            let game_service = GameEngineServiceWrapper::new(config);
            orchestrator.add_service(Box::new(game_service)).await?;
        }
        
        // Add consensus service if configured
        if let Some(config) = self.consensus_config {
            let consensus_service = ConsensusServiceWrapper::new(config);
            orchestrator.add_service(Box::new(consensus_service)).await?;
        }
        
        // Set up gateway if configured
        #[cfg(feature = "api-gateway")]
        if let Some(mut config) = self.gateway_config {
            // Thread region_self from global config if available via discovery, else leave None
            // (Assumes discovery might carry app-level config; skip if not available)
            // Keep default lb_strategy from config
            let gateway = ApiGateway::new(config);
            orchestrator.with_gateway(gateway);
        }
        
        Ok(orchestrator)
    }
}

impl Default for ServiceBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Wrapper for GameEngineService to implement MicroService trait
pub struct GameEngineServiceWrapper {
    service: std::sync::Arc<tokio::sync::RwLock<game_engine::GameEngineService>>,
    address: SocketAddr,
}

impl GameEngineServiceWrapper {
    pub fn new(config: game_engine::GameEngineConfig) -> Self {
        let address = "127.0.0.1:8081".parse().unwrap(); // Default address
        let service = game_engine::GameEngineService::new(config);
        let service = std::sync::Arc::new(tokio::sync::RwLock::new(service));
        Self { service, address }
    }
}

#[async_trait::async_trait]
impl MicroService for GameEngineServiceWrapper {
    fn name(&self) -> &str {
        "game-engine"
    }
    
    fn address(&self) -> SocketAddr {
        self.address
    }
    
    async fn start(&mut self) -> Result<()> {
        use crate::services::game_engine::http::start_http;
        {
            let mut svc = self.service.write().await;
            svc.start().await?;
        }
        start_http(self.service.clone(), self.address).await?;
        Ok(())
    }
    
    async fn stop(&mut self) -> Result<()> {
        let mut svc = self.service.write().await;
        svc.stop().await
    }
    
    async fn health_check(&self) -> Result<ServiceHealth> {
        match self.service.read().await.health_check().await {
            Ok(_) => Ok(ServiceHealth::Healthy),
            Err(_) => Ok(ServiceHealth::Unhealthy),
        }
    }
    
    fn registration_info(&self) -> ServiceRegistration {
        let mut metadata = std::collections::HashMap::new();
        metadata.insert("health_check_path".to_string(), "/health".to_string());
        
        ServiceRegistration {
            service_name: "game-engine".to_string(),
            service_id: format!("game-engine-{}", uuid::Uuid::new_v4()),
            address: self.address,
            tags: vec!["game".to_string(), "engine".to_string()],
            metadata,
            health_check: Some(common::HealthCheck {
                http: Some(common::HttpHealthCheck {
                    url: format!("http://{}/health", self.address),
                    method: "GET".to_string(),
                    headers: std::collections::HashMap::new(),
                    expected_status: 200,
                }),
                tcp: None,
                interval: std::time::Duration::from_secs(30),
                timeout: std::time::Duration::from_secs(5),
                deregister_critical_service_after: Some(std::time::Duration::from_secs(300)),
            }),
            ttl: None,
        }
    }
}

/// Wrapper for ConsensusService to implement MicroService trait
pub struct ConsensusServiceWrapper {
    service: std::sync::Arc<tokio::sync::RwLock<consensus::ConsensusService>>,
    address: SocketAddr,
}

impl ConsensusServiceWrapper {
    pub fn new(config: consensus::ConsensusConfig) -> Self {
        let address = "127.0.0.1:8082".parse().unwrap(); // Default address
        let peer_id: crate::protocol::PeerId = [0u8; 32];
        let service = consensus::ConsensusService::new(config, peer_id);
        let service = std::sync::Arc::new(tokio::sync::RwLock::new(service));
        Self { service, address }
    }
}

#[async_trait::async_trait]
impl MicroService for ConsensusServiceWrapper {
    fn name(&self) -> &str {
        "consensus"
    }
    
    fn address(&self) -> SocketAddr {
        self.address
    }
    
    async fn start(&mut self) -> Result<()> {
        use crate::services::consensus::http::start_http;
        {
            let mut svc = self.service.write().await;
            svc.start().await?;
        }
        start_http(self.service.clone(), self.address).await?;
        Ok(())
    }
    
    async fn stop(&mut self) -> Result<()> {
        let mut svc = self.service.write().await;
        svc.stop().await
    }
    
    async fn health_check(&self) -> Result<ServiceHealth> {
        // Check if service is responding to status requests
        match self.service.read().await.get_status(consensus::types::StatusRequest { proposal_id: None }).await {
            Ok(_) => Ok(ServiceHealth::Healthy),
            Err(_) => Ok(ServiceHealth::Unhealthy),
        }
    }
    
    fn registration_info(&self) -> ServiceRegistration {
        let mut metadata = std::collections::HashMap::new();
        metadata.insert("health_check_path".to_string(), "/health".to_string());
        
        ServiceRegistration {
            service_name: "consensus".to_string(),
            service_id: format!("consensus-{}", uuid::Uuid::new_v4()),
            address: self.address,
            tags: vec!["consensus".to_string(), "blockchain".to_string()],
            metadata,
            health_check: Some(common::HealthCheck {
                http: Some(common::HttpHealthCheck {
                    url: format!("http://{}/health", self.address),
                    method: "GET".to_string(),
                    headers: std::collections::HashMap::new(),
                    expected_status: 200,
                }),
                tcp: None,
                interval: std::time::Duration::from_secs(30),
                timeout: std::time::Duration::from_secs(5),
                deregister_critical_service_after: Some(std::time::Duration::from_secs(300)),
            }),
            ttl: None,
        }
    }
}

/// Convenience function to create a default microservices setup
pub async fn create_default_setup() -> Result<ServiceOrchestrator> {
    use common::discovery::StaticServiceDiscovery;
    
    let discovery = Arc::new(StaticServiceDiscovery::new());
    
    let mut builder = ServiceBuilder::new()
        .with_service_discovery(discovery)
        .with_game_engine(game_engine::GameEngineConfig::default())
        .with_consensus(consensus::ConsensusConfig::default());
        
    #[cfg(feature = "api-gateway")]
    {
        builder = builder.with_gateway(api_gateway::GatewayConfig::default());
    }
    
    builder.build().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::discovery::StaticServiceDiscovery;
    
    #[tokio::test]
    async fn test_service_orchestrator() {
        let discovery = Arc::new(StaticServiceDiscovery::new());
        let orchestrator = ServiceOrchestrator::new(discovery);
        
        assert!(orchestrator.services.read().await.is_empty());
    }
    
    #[tokio::test]
    async fn test_service_builder() {
        let discovery = Arc::new(StaticServiceDiscovery::new());
        
        let orchestrator = ServiceBuilder::new()
            .with_service_discovery(discovery)
            .with_game_engine(game_engine::GameEngineConfig::default())
            .with_consensus(consensus::ConsensusConfig::default())
            .build()
            .await
            .unwrap();
        
        let services = orchestrator.services.read().await;
        assert_eq!(services.len(), 2); // Game engine + Consensus
    }
}
