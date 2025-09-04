//! Edge Computing Framework for BitCraps
//!
//! This module provides a comprehensive edge computing infrastructure for low-latency
//! gaming at the network edge. It includes edge node discovery, registration,
//! workload orchestration, and latency optimization strategies.
//!
//! # Architecture Overview
//!
//! The edge framework operates in a hierarchical structure:
//! - **Edge Nodes**: Distributed compute resources at network edges
//! - **Edge Clusters**: Groups of edge nodes in geographic proximity  
//! - **Edge Orchestrator**: Central coordination for workload placement
//! - **CDN Integration**: Content delivery and static asset caching
//! - **Mobile Edge**: 5G MEC integration for ultra-low latency
//!
//! # Key Features
//!
//! - Sub-10ms latency for critical gaming operations
//! - Automatic failover and load balancing
//! - Geographic-aware routing and placement
//! - WebAssembly edge workers for custom logic
//! - Hardware acceleration support (GPUs, FPGAs)

use crate::error::{Error, Result};
use crate::utils::timeout::TimeoutExt;
use crate::protocol::PeerId;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::{RwLock, Mutex, watch};
use uuid::Uuid;

pub mod cdn;
pub mod orchestrator;
pub mod mec;
pub mod cache;
pub mod workers;

/// Maximum latency threshold for edge operations (10ms)
pub const MAX_EDGE_LATENCY: Duration = Duration::from_millis(10);

/// Edge node refresh interval
pub const NODE_REFRESH_INTERVAL: Duration = Duration::from_secs(30);

/// Edge heartbeat timeout
pub const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(60);

/// Edge node identifier
pub type EdgeNodeId = Uuid;

/// Geographic coordinates for edge nodes
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct GeoLocation {
    pub latitude: f64,
    pub longitude: f64,
    pub city: Option<String>,
    pub country: Option<String>,
}

impl GeoLocation {
    /// Calculate distance to another location (approximate Haversine)
    pub fn distance_km(&self, other: &GeoLocation) -> f64 {
        let r = 6371.0; // Earth radius in km
        let lat1_rad = self.latitude.to_radians();
        let lat2_rad = other.latitude.to_radians();
        let delta_lat = (other.latitude - self.latitude).to_radians();
        let delta_lon = (other.longitude - self.longitude).to_radians();

        let a = (delta_lat / 2.0).sin().powi(2) +
                lat1_rad.cos() * lat2_rad.cos() * (delta_lon / 2.0).sin().powi(2);
        let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
        r * c
    }
}

/// Edge node capabilities and specifications
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeCapabilities {
    /// CPU cores available
    pub cpu_cores: u32,
    /// Memory in MB
    pub memory_mb: u64,
    /// Storage in GB
    pub storage_gb: u64,
    /// Network bandwidth in Mbps
    pub bandwidth_mbps: u64,
    /// GPU acceleration available
    pub gpu_acceleration: bool,
    /// FPGA acceleration available
    pub fpga_acceleration: bool,
    /// WebAssembly runtime support
    pub wasm_runtime: bool,
    /// Supported protocols
    pub protocols: HashSet<String>,
}

impl Default for EdgeCapabilities {
    fn default() -> Self {
        Self {
            cpu_cores: 4,
            memory_mb: 8192,
            storage_gb: 100,
            bandwidth_mbps: 1000,
            gpu_acceleration: false,
            fpga_acceleration: false,
            wasm_runtime: true,
            protocols: ["tcp", "udp", "websocket", "webrtc"].iter().map(|s| s.to_string()).collect(),
        }
    }
}

/// Edge node health and performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeMetrics {
    /// CPU utilization (0-100%)
    pub cpu_usage: f32,
    /// Memory utilization (0-100%)
    pub memory_usage: f32,
    /// Network latency to core network
    pub network_latency_ms: f32,
    /// Active connections
    pub active_connections: u32,
    /// Requests per second
    pub requests_per_second: f32,
    /// Error rate (0-1)
    pub error_rate: f32,
    /// Last update timestamp
    pub timestamp: SystemTime,
}

impl Default for EdgeMetrics {
    fn default() -> Self {
        Self {
            cpu_usage: 0.0,
            memory_usage: 0.0,
            network_latency_ms: 0.0,
            active_connections: 0,
            requests_per_second: 0.0,
            error_rate: 0.0,
            timestamp: SystemTime::now(),
        }
    }
}

/// Edge node status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EdgeNodeStatus {
    /// Node is online and accepting workloads
    Online,
    /// Node is online but at capacity
    Saturated,
    /// Node is experiencing issues
    Degraded,
    /// Node is offline or unreachable
    Offline,
    /// Node is in maintenance mode
    Maintenance,
}

/// Edge node registration and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeNode {
    pub id: EdgeNodeId,
    pub address: SocketAddr,
    pub location: GeoLocation,
    pub capabilities: EdgeCapabilities,
    pub status: EdgeNodeStatus,
    pub metrics: EdgeMetrics,
    pub last_seen: SystemTime,
    pub workloads: HashSet<Uuid>,
}

impl EdgeNode {
    pub fn new(id: EdgeNodeId, address: SocketAddr, location: GeoLocation) -> Self {
        Self {
            id,
            address,
            location,
            capabilities: EdgeCapabilities::default(),
            status: EdgeNodeStatus::Online,
            metrics: EdgeMetrics::default(),
            last_seen: SystemTime::now(),
            workloads: HashSet::new(),
        }
    }

    /// Check if node is healthy and responsive
    pub fn is_healthy(&self) -> bool {
        matches!(self.status, EdgeNodeStatus::Online | EdgeNodeStatus::Saturated) &&
        self.last_seen.elapsed().unwrap_or_default() < HEARTBEAT_TIMEOUT
    }

    /// Calculate node priority score for workload placement
    pub fn priority_score(&self) -> f32 {
        if !self.is_healthy() {
            return 0.0;
        }

        let cpu_score = (100.0 - self.metrics.cpu_usage) / 100.0;
        let memory_score = (100.0 - self.metrics.memory_usage) / 100.0;
        let latency_score = 1.0 / (1.0 + self.metrics.network_latency_ms / 100.0);
        let error_score = 1.0 - self.metrics.error_rate;

        (cpu_score * 0.3 + memory_score * 0.2 + latency_score * 0.3 + error_score * 0.2).max(0.0)
    }
}

/// Workload type for edge placement decisions
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum WorkloadType {
    /// Real-time gaming logic (ultra-low latency required)
    GameLogic,
    /// Static content serving
    StaticContent,
    /// Dynamic content generation
    DynamicContent,
    /// Video/audio streaming
    MediaStreaming,
    /// Data processing and analytics
    DataProcessing,
    /// Machine learning inference
    MLInference,
}

impl WorkloadType {
    /// Get maximum acceptable latency for workload type
    pub fn max_latency(&self) -> Duration {
        match self {
            WorkloadType::GameLogic => Duration::from_millis(5),
            WorkloadType::MediaStreaming => Duration::from_millis(50),
            WorkloadType::DynamicContent => Duration::from_millis(100),
            WorkloadType::StaticContent => Duration::from_millis(200),
            WorkloadType::DataProcessing => Duration::from_millis(500),
            WorkloadType::MLInference => Duration::from_millis(100),
        }
    }

    /// Get CPU requirement weight (0-1)
    pub fn cpu_weight(&self) -> f32 {
        match self {
            WorkloadType::GameLogic => 0.8,
            WorkloadType::MLInference => 0.9,
            WorkloadType::DataProcessing => 0.7,
            WorkloadType::MediaStreaming => 0.6,
            WorkloadType::DynamicContent => 0.5,
            WorkloadType::StaticContent => 0.2,
        }
    }
}

/// Edge workload specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeWorkload {
    pub id: Uuid,
    pub workload_type: WorkloadType,
    pub resource_requirements: EdgeCapabilities,
    pub target_location: Option<GeoLocation>,
    pub created_at: SystemTime,
    pub priority: u8, // 0-255, higher = more important
}

/// Edge runtime for managing distributed edge infrastructure
pub struct EdgeRuntime {
    /// Registered edge nodes
    nodes: Arc<RwLock<HashMap<EdgeNodeId, EdgeNode>>>,
    
    /// Active workloads
    workloads: Arc<RwLock<HashMap<Uuid, EdgeWorkload>>>,
    
    /// Node placement assignments
    placements: Arc<RwLock<HashMap<Uuid, EdgeNodeId>>>,
    
    /// Edge discovery service
    discovery_tx: watch::Sender<Vec<EdgeNode>>,
    discovery_rx: watch::Receiver<Vec<EdgeNode>>,
    
    /// Runtime configuration
    config: EdgeRuntimeConfig,
    
    /// Background task handles
    _background_tasks: Vec<tokio::task::JoinHandle<()>>,
}

/// Configuration for edge runtime
#[derive(Debug, Clone)]
pub struct EdgeRuntimeConfig {
    pub max_nodes: usize,
    pub max_workloads_per_node: usize,
    pub health_check_interval: Duration,
    pub placement_algorithm: PlacementAlgorithm,
    pub enable_geographic_routing: bool,
    pub enable_load_balancing: bool,
}

impl Default for EdgeRuntimeConfig {
    fn default() -> Self {
        Self {
            max_nodes: 1000,
            max_workloads_per_node: 50,
            health_check_interval: Duration::from_secs(30),
            placement_algorithm: PlacementAlgorithm::LatencyAware,
            enable_geographic_routing: true,
            enable_load_balancing: true,
        }
    }
}

/// Workload placement algorithms
#[derive(Debug, Clone)]
pub enum PlacementAlgorithm {
    /// Place based on lowest latency
    LatencyAware,
    /// Place based on resource availability
    ResourceAware,
    /// Place based on geographic proximity
    GeographicAware,
    /// Balanced placement considering all factors
    Balanced,
}

impl EdgeRuntime {
    /// Create new edge runtime
    pub fn new(config: EdgeRuntimeConfig) -> Self {
        let (discovery_tx, discovery_rx) = watch::channel(Vec::new());
        
        Self {
            nodes: Arc::new(RwLock::new(HashMap::new())),
            workloads: Arc::new(RwLock::new(HashMap::new())),
            placements: Arc::new(RwLock::new(HashMap::new())),
            discovery_tx,
            discovery_rx,
            config,
            _background_tasks: Vec::new(),
        }
    }

    /// Start the edge runtime with background services
    pub async fn start(&mut self) -> Result<()> {
        // Start health monitoring
        let health_task = self.start_health_monitoring().await;
        self._background_tasks.push(health_task);

        // Start node discovery
        let discovery_task = self.start_node_discovery().await;
        self._background_tasks.push(discovery_task);

        // Start workload scheduler
        let scheduler_task = self.start_workload_scheduler().await;
        self._background_tasks.push(scheduler_task);

        Ok(())
    }

    /// Register a new edge node
    pub async fn register_node(&self, mut node: EdgeNode) -> Result<()> {
        node.last_seen = SystemTime::now();
        
        let mut nodes = self.nodes.write().await;
        
        if nodes.len() >= self.config.max_nodes {
            return Err(Error::ResourceExhausted("Maximum edge nodes reached".to_string()));
        }
        
        nodes.insert(node.id, node.clone());
        drop(nodes);
        
        // Notify discovery subscribers
        self.update_discovery().await?;
        
        tracing::info!("Registered edge node {} at {}", node.id, node.address);
        Ok(())
    }

    /// Unregister an edge node
    pub async fn unregister_node(&self, node_id: EdgeNodeId) -> Result<()> {
        let mut nodes = self.nodes.write().await;
        
        if let Some(node) = nodes.remove(&node_id) {
            drop(nodes);
            
            // Reschedule workloads from removed node
            self.reschedule_workloads_from_node(node_id).await?;
            
            // Update discovery
            self.update_discovery().await?;
            
            tracing::info!("Unregistered edge node {} at {}", node_id, node.address);
        }
        
        Ok(())
    }

    /// Submit a workload for edge placement
    pub async fn submit_workload(&self, workload: EdgeWorkload) -> Result<EdgeNodeId> {
        // Find optimal placement
        let node_id = self.find_optimal_placement(&workload).await?;
        
        // Store workload
        let mut workloads = self.workloads.write().await;
        workloads.insert(workload.id, workload.clone());
        drop(workloads);
        
        // Record placement
        let mut placements = self.placements.write().await;
        placements.insert(workload.id, node_id);
        drop(placements);
        
        // Update node workload list
        let mut nodes = self.nodes.write().await;
        if let Some(node) = nodes.get_mut(&node_id) {
            node.workloads.insert(workload.id);
        }
        
        tracing::debug!("Placed workload {} on edge node {}", workload.id, node_id);
        Ok(node_id)
    }

    /// Remove a workload from edge placement
    pub async fn remove_workload(&self, workload_id: Uuid) -> Result<()> {
        // Remove from placements and get node
        let mut placements = self.placements.write().await;
        let node_id = placements.remove(&workload_id);
        drop(placements);
        
        // Remove from workloads
        let mut workloads = self.workloads.write().await;
        workloads.remove(&workload_id);
        drop(workloads);
        
        // Update node workload list
        if let Some(node_id) = node_id {
            let mut nodes = self.nodes.write().await;
            if let Some(node) = nodes.get_mut(&node_id) {
                node.workloads.remove(&workload_id);
            }
        }
        
        Ok(())
    }

    /// Get edge nodes matching criteria
    pub async fn get_nodes_by_location(&self, location: GeoLocation, max_distance_km: f64) -> Vec<EdgeNode> {
        let nodes = self.nodes.read().await;
        
        nodes.values()
            .filter(|node| node.location.distance_km(&location) <= max_distance_km)
            .cloned()
            .collect()
    }

    /// Get edge node by ID
    pub async fn get_node(&self, node_id: EdgeNodeId) -> Option<EdgeNode> {
        let nodes = self.nodes.read().await;
        nodes.get(&node_id).cloned()
    }

    /// Get all healthy edge nodes
    pub async fn get_healthy_nodes(&self) -> Vec<EdgeNode> {
        let nodes = self.nodes.read().await;
        nodes.values()
            .filter(|node| node.is_healthy())
            .cloned()
            .collect()
    }

    /// Update node metrics
    pub async fn update_node_metrics(&self, node_id: EdgeNodeId, metrics: EdgeMetrics) -> Result<()> {
        let mut nodes = self.nodes.write().await;
        
        if let Some(node) = nodes.get_mut(&node_id) {
            node.metrics = metrics;
            node.last_seen = SystemTime::now();
            
            // Update status based on metrics
            node.status = if node.metrics.cpu_usage > 95.0 || node.metrics.memory_usage > 95.0 {
                EdgeNodeStatus::Saturated
            } else if node.metrics.error_rate > 0.1 || node.metrics.network_latency_ms > 100.0 {
                EdgeNodeStatus::Degraded
            } else {
                EdgeNodeStatus::Online
            };
        }
        
        Ok(())
    }

    /// Find optimal node placement for workload
    async fn find_optimal_placement(&self, workload: &EdgeWorkload) -> Result<EdgeNodeId> {
        let nodes = self.nodes.read().await;
        let healthy_nodes: Vec<_> = nodes.values()
            .filter(|node| node.is_healthy())
            .filter(|node| self.can_place_workload(node, workload))
            .collect();

        if healthy_nodes.is_empty() {
            return Err(Error::ResourceExhausted("No suitable edge nodes available".to_string()));
        }

        let selected_node = match self.config.placement_algorithm {
            PlacementAlgorithm::LatencyAware => {
                self.select_by_latency(&healthy_nodes, workload).await
            }
            PlacementAlgorithm::ResourceAware => {
                self.select_by_resources(&healthy_nodes, workload).await
            }
            PlacementAlgorithm::GeographicAware => {
                self.select_by_geography(&healthy_nodes, workload).await
            }
            PlacementAlgorithm::Balanced => {
                self.select_balanced(&healthy_nodes, workload).await
            }
        };

        selected_node.map(|node| node.id)
            .ok_or_else(|| Error::ResourceExhausted("No optimal placement found".to_string()))
    }

    /// Check if workload can be placed on node
    fn can_place_workload(&self, node: &EdgeNode, workload: &EdgeWorkload) -> bool {
        // Check workload limits
        if node.workloads.len() >= self.config.max_workloads_per_node {
            return false;
        }

        // Check resource requirements
        let reqs = &workload.resource_requirements;
        let caps = &node.capabilities;

        if reqs.cpu_cores > caps.cpu_cores ||
           reqs.memory_mb > caps.memory_mb ||
           reqs.storage_gb > caps.storage_gb ||
           reqs.bandwidth_mbps > caps.bandwidth_mbps {
            return false;
        }

        // Check special requirements
        if reqs.gpu_acceleration && !caps.gpu_acceleration {
            return false;
        }
        if reqs.fpga_acceleration && !caps.fpga_acceleration {
            return false;
        }
        if reqs.wasm_runtime && !caps.wasm_runtime {
            return false;
        }

        true
    }

    /// Select node by lowest latency
    async fn select_by_latency(&self, nodes: &[&EdgeNode], _workload: &EdgeWorkload) -> Option<&EdgeNode> {
        nodes.iter()
            .min_by(|a, b| a.metrics.network_latency_ms.total_cmp(&b.metrics.network_latency_ms))
            .copied()
    }

    /// Select node by best resource availability
    async fn select_by_resources(&self, nodes: &[&EdgeNode], _workload: &EdgeWorkload) -> Option<&EdgeNode> {
        nodes.iter()
            .max_by(|a, b| a.priority_score().total_cmp(&b.priority_score()))
            .copied()
    }

    /// Select node by geographic proximity
    async fn select_by_geography(&self, nodes: &[&EdgeNode], workload: &EdgeWorkload) -> Option<&EdgeNode> {
        if let Some(target_location) = workload.target_location {
            nodes.iter()
                .min_by(|a, b| {
                    let dist_a = a.location.distance_km(&target_location);
                    let dist_b = b.location.distance_km(&target_location);
                    dist_a.total_cmp(&dist_b)
                })
                .copied()
        } else {
            // Fallback to resource-based selection
            self.select_by_resources(nodes, workload).await
        }
    }

    /// Select node using balanced algorithm
    async fn select_balanced(&self, nodes: &[&EdgeNode], workload: &EdgeWorkload) -> Option<&EdgeNode> {
        let mut best_node = None;
        let mut best_score = 0.0f32;

        for node in nodes {
            let mut score = node.priority_score() * 0.4; // Resource score

            // Latency score (lower is better)
            let latency_score = 1.0 / (1.0 + node.metrics.network_latency_ms / 50.0);
            score += latency_score * 0.3;

            // Geographic score (if target location specified)
            if let Some(target_location) = workload.target_location {
                let distance = node.location.distance_km(&target_location);
                let geo_score = 1.0 / (1.0 + distance / 1000.0); // Normalize to ~1000km
                score += geo_score * 0.3;
            } else {
                score += 0.3; // No geographic penalty
            }

            if score > best_score {
                best_score = score;
                best_node = Some(*node);
            }
        }

        best_node
    }

    /// Reschedule workloads from a failed/removed node
    async fn reschedule_workloads_from_node(&self, failed_node_id: EdgeNodeId) -> Result<()> {
        let workloads_to_reschedule: Vec<_> = {
            let placements = self.placements.read().await;
            placements.iter()
                .filter_map(|(workload_id, node_id)| {
                    if *node_id == failed_node_id {
                        Some(*workload_id)
                    } else {
                        None
                    }
                })
                .collect()
        };

        for workload_id in workloads_to_reschedule {
            if let Some(workload) = {
                let workloads = self.workloads.read().await;
                workloads.get(&workload_id).cloned()
            } {
                // Try to find new placement
                match self.find_optimal_placement(&workload).await {
                    Ok(new_node_id) => {
                        // Update placement
                        let mut placements = self.placements.write().await;
                        placements.insert(workload_id, new_node_id);
                        drop(placements);

                        // Update new node workload list
                        let mut nodes = self.nodes.write().await;
                        if let Some(node) = nodes.get_mut(&new_node_id) {
                            node.workloads.insert(workload_id);
                        }

                        tracing::info!("Rescheduled workload {} from {} to {}", 
                                     workload_id, failed_node_id, new_node_id);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to reschedule workload {}: {}", workload_id, e);
                        // Remove workload if no placement possible
                        self.remove_workload(workload_id).await?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Update discovery channel with current nodes
    async fn update_discovery(&self) -> Result<()> {
        let nodes = self.nodes.read().await;
        let node_list: Vec<EdgeNode> = nodes.values().cloned().collect();
        
        self.discovery_tx.send(node_list)
            .map_err(|_| Error::ServiceError("Discovery channel closed".to_string()))?;
        
        Ok(())
    }

    /// Start health monitoring background task
    async fn start_health_monitoring(&self) -> tokio::task::JoinHandle<()> {
        let nodes = Arc::clone(&self.nodes);
        let interval = self.config.health_check_interval;
        
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            
            loop {
                ticker.tick().await;
                
                let mut unhealthy_nodes = Vec::new();
                
                {
                    let mut nodes_guard = nodes.write().await;
                    let now = SystemTime::now();
                    
                    for (node_id, node) in nodes_guard.iter_mut() {
                        if let Ok(elapsed) = node.last_seen.elapsed() {
                            if elapsed > HEARTBEAT_TIMEOUT {
                                node.status = EdgeNodeStatus::Offline;
                                unhealthy_nodes.push(*node_id);
                                tracing::warn!("Edge node {} marked offline (no heartbeat for {:?})", 
                                             node_id, elapsed);
                            }
                        }
                    }
                }
                
                // TODO: Implement workload rescheduling for unhealthy nodes
                for node_id in unhealthy_nodes {
                    tracing::debug!("Should reschedule workloads from unhealthy node {}", node_id);
                }
            }
        })
    }

    /// Start node discovery background task  
    async fn start_node_discovery(&self) -> tokio::task::JoinHandle<()> {
        let nodes = Arc::clone(&self.nodes);
        let discovery_tx = self.discovery_tx.clone();
        
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(NODE_REFRESH_INTERVAL);
            
            loop {
                ticker.tick().await;
                
                // Broadcast current node list for discovery
                let nodes_guard = nodes.read().await;
                let node_list: Vec<EdgeNode> = nodes_guard.values()
                    .filter(|node| node.is_healthy())
                    .cloned()
                    .collect();
                drop(nodes_guard);
                
                if let Err(_) = discovery_tx.send(node_list) {
                    tracing::error!("Edge discovery channel closed");
                    break;
                }
            }
        })
    }

    /// Start workload scheduler background task
    async fn start_workload_scheduler(&self) -> tokio::task::JoinHandle<()> {
        let nodes = Arc::clone(&self.nodes);
        let workloads = Arc::clone(&self.workloads);
        let placements = Arc::clone(&self.placements);
        
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_secs(60));
            
            loop {
                ticker.tick().await;
                
                // Rebalance workloads if needed
                // TODO: Implement intelligent workload rebalancing
                tracing::debug!("Workload scheduler tick - rebalancing evaluation");
            }
        })
    }

    /// Get discovery channel receiver for node updates
    pub fn subscribe_to_discovery(&self) -> watch::Receiver<Vec<EdgeNode>> {
        self.discovery_rx.clone()
    }

    /// Get runtime statistics
    pub async fn get_statistics(&self) -> EdgeRuntimeStats {
        let nodes = self.nodes.read().await;
        let workloads = self.workloads.read().await;
        let placements = self.placements.read().await;

        let total_nodes = nodes.len();
        let healthy_nodes = nodes.values().filter(|n| n.is_healthy()).count();
        let total_workloads = workloads.len();

        EdgeRuntimeStats {
            total_nodes,
            healthy_nodes,
            total_workloads,
            active_placements: placements.len(),
            average_cpu_usage: nodes.values().map(|n| n.metrics.cpu_usage).sum::<f32>() / total_nodes.max(1) as f32,
            average_memory_usage: nodes.values().map(|n| n.metrics.memory_usage).sum::<f32>() / total_nodes.max(1) as f32,
            average_latency_ms: nodes.values().map(|n| n.metrics.network_latency_ms).sum::<f32>() / total_nodes.max(1) as f32,
        }
    }
}

/// Edge runtime statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeRuntimeStats {
    pub total_nodes: usize,
    pub healthy_nodes: usize,
    pub total_workloads: usize,
    pub active_placements: usize,
    pub average_cpu_usage: f32,
    pub average_memory_usage: f32,
    pub average_latency_ms: f32,
}

// Re-export types from submodules
pub use cdn::{CdnManager, CdnConfig, CdnProvider, EdgeWorker};
pub use orchestrator::{EdgeOrchestrator, OrchestratorConfig, EdgeCluster, AutoScalingConfig};
pub use mec::{MecManager, MecConfig, MecPlatform, NetworkSlice, QosClass};
pub use cache::{EdgeCacheManager, EdgeCacheConfig, CacheTier, CacheMetrics};