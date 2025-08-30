//! Advanced routing algorithms for mesh networking
//!
//! This module implements sophisticated routing algorithms optimized
//! for mobile mesh networks with dynamic topology changes.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

use crate::error::Result;
use crate::protocol::PeerId;

/// Advanced routing table with multiple algorithms
pub struct AdvancedRoutingTable {
    /// Dijkstra-based shortest path routing
    shortest_paths: Arc<RwLock<HashMap<PeerId, PathInfo>>>,
    /// Load-balanced routing with traffic awareness
    load_balanced_paths: Arc<RwLock<HashMap<PeerId, Vec<PathInfo>>>>,
    /// Geographic routing for mobile devices
    geographic_routes: Arc<RwLock<HashMap<PeerId, GeographicRoute>>>,
    /// Ant colony optimization routes
    aco_routes: Arc<RwLock<HashMap<PeerId, AcoRoute>>>,
    /// Network topology graph
    topology: Arc<RwLock<NetworkTopology>>,
    /// Routing metrics
    metrics: Arc<RwLock<RoutingMetrics>>,
    /// Configuration
    config: RoutingConfig,
}

/// Path information for routing decisions
#[derive(Debug, Clone, PartialEq)]
pub struct PathInfo {
    pub destination: PeerId,
    pub next_hop: PeerId,
    pub path: Vec<PeerId>,
    pub cost: f64,
    pub latency: Duration,
    pub bandwidth: f64,
    pub reliability: f64,
    pub congestion: f64,
    pub last_updated: Instant,
    pub hop_count: u8,
}

/// Geographic routing information
#[allow(dead_code)]
#[derive(Debug, Clone)]
struct GeographicRoute {
    destination: PeerId,
    destination_location: Option<Location>,
    next_hop: PeerId,
    next_hop_location: Location,
    distance_remaining: f64,
    direction: f64, // Radians
    last_updated: Instant,
}

/// Ant Colony Optimization route
#[allow(dead_code)]
#[derive(Debug, Clone)]
struct AcoRoute {
    destination: PeerId,
    paths: Vec<AcoPath>,
    pheromone_levels: HashMap<(PeerId, PeerId), f64>,
    last_optimization: Instant,
}

/// ACO path with pheromone information
#[derive(Debug, Clone)]
struct AcoPath {
    path: Vec<PeerId>,
    pheromone: f64,
    quality: f64,
    usage_count: u32,
}

/// Geographic location
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Location {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: Option<f64>,
    pub accuracy: f64, // meters
}

/// Network topology representation
#[derive(Debug, Clone)]
struct NetworkTopology {
    nodes: HashMap<PeerId, NodeInfo>,
    edges: HashMap<(PeerId, PeerId), EdgeInfo>,
    adjacency_list: HashMap<PeerId, Vec<PeerId>>,
    last_updated: Instant,
}

/// Node information in topology
#[derive(Debug, Clone)]
struct NodeInfo {
    peer_id: PeerId,
    location: Option<Location>,
    capabilities: NodeCapabilities,
    mobility: MobilityInfo,
    energy_level: Option<f64>, // 0.0-1.0
    last_seen: Instant,
}

/// Node capabilities
#[derive(Debug, Clone, Default)]
pub struct NodeCapabilities {
    max_bandwidth: f64,
    supports_relay: bool,
    battery_powered: bool,
    computing_power: f64, // Relative metric
}

/// Mobility information
#[derive(Debug, Clone, Default)]
struct MobilityInfo {
    velocity: Option<f64>,  // m/s
    direction: Option<f64>, // radians
    mobility_pattern: MobilityPattern,
    predicted_location: Option<(Location, Instant)>,
}

/// Mobility patterns for prediction
#[derive(Debug, Clone, Copy, Default)]
enum MobilityPattern {
    #[default]
    Static,
    Random,
    Linear,
    Circular,
    Commuter, // Regular patterns
}

/// Edge information in topology
#[derive(Debug, Clone)]
struct EdgeInfo {
    from: PeerId,
    to: PeerId,
    weight: f64,
    latency: Duration,
    bandwidth: f64,
    packet_loss: f64,
    jitter: Duration,
    last_measured: Instant,
    measurements: VecDeque<LinkMeasurement>,
}

/// Link measurement for history
#[derive(Debug, Clone)]
struct LinkMeasurement {
    timestamp: Instant,
    latency: Duration,
    bandwidth: f64,
    packet_loss: f64,
}

/// Routing metrics and statistics
#[derive(Debug, Clone, Default)]
pub struct RoutingMetrics {
    total_routes: usize,
    successful_deliveries: u64,
    failed_deliveries: u64,
    average_hop_count: f64,
    average_latency: Duration,
    route_convergence_time: Duration,
    topology_changes: u64,
    last_updated: Option<Instant>,
}

/// Routing configuration
#[derive(Debug, Clone)]
pub struct RoutingConfig {
    /// Enable geographic routing
    pub enable_geographic: bool,
    /// Enable ACO routing
    pub enable_aco: bool,
    /// Maximum path length
    pub max_hops: u8,
    /// Route update interval
    pub update_interval: Duration,
    /// Path diversity (number of alternative paths)
    pub path_diversity: usize,
    /// Pheromone evaporation rate for ACO
    pub pheromone_evaporation: f64,
    /// Geographic routing greedy forwarding threshold
    pub greedy_threshold: f64,
    /// Mobility prediction window
    pub mobility_prediction_window: Duration,
}

impl Default for RoutingConfig {
    fn default() -> Self {
        Self {
            enable_geographic: true,
            enable_aco: true,
            max_hops: 8,
            update_interval: Duration::from_secs(30),
            path_diversity: 3,
            pheromone_evaporation: 0.1,
            greedy_threshold: 0.8,
            mobility_prediction_window: Duration::from_secs(300), // 5 minutes
        }
    }
}

/// Node for Dijkstra's algorithm
#[derive(Debug, Clone)]
struct DijkstraNode {
    peer_id: PeerId,
    cost: f64,
    path: Vec<PeerId>,
}

impl Eq for DijkstraNode {}

impl PartialEq for DijkstraNode {
    fn eq(&self, other: &Self) -> bool {
        self.cost.total_cmp(&other.cost) == Ordering::Equal
    }
}

impl Ord for DijkstraNode {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse ordering for min-heap
        other.cost.total_cmp(&self.cost)
    }
}

impl PartialOrd for DijkstraNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl AdvancedRoutingTable {
    /// Create new advanced routing table
    pub fn new(config: RoutingConfig) -> Self {
        Self {
            shortest_paths: Arc::new(RwLock::new(HashMap::new())),
            load_balanced_paths: Arc::new(RwLock::new(HashMap::new())),
            geographic_routes: Arc::new(RwLock::new(HashMap::new())),
            aco_routes: Arc::new(RwLock::new(HashMap::new())),
            topology: Arc::new(RwLock::new(NetworkTopology::new())),
            metrics: Arc::new(RwLock::new(RoutingMetrics::default())),
            config,
        }
    }

    /// Update topology with new node information
    pub async fn update_node(
        &self,
        peer_id: PeerId,
        location: Option<Location>,
        capabilities: NodeCapabilities,
    ) -> Result<()> {
        let mut topology = self.topology.write().await;

        let node_info = NodeInfo {
            peer_id,
            location,
            capabilities,
            mobility: MobilityInfo::default(),
            energy_level: None,
            last_seen: Instant::now(),
        };

        topology.nodes.insert(peer_id, node_info);
        topology.last_updated = Instant::now();

        // Trigger route recalculation
        self.recalculate_routes().await?;

        Ok(())
    }

    /// Update link information
    pub async fn update_link(
        &self,
        from: PeerId,
        to: PeerId,
        latency: Duration,
        bandwidth: f64,
        packet_loss: f64,
    ) -> Result<()> {
        let mut topology = self.topology.write().await;

        // Calculate dynamic weight based on multiple factors
        let weight = self.calculate_link_weight(latency, bandwidth, packet_loss);

        let edge_info = EdgeInfo {
            from,
            to,
            weight,
            latency,
            bandwidth,
            packet_loss,
            jitter: Duration::ZERO, // Would be measured separately
            last_measured: Instant::now(),
            measurements: VecDeque::new(),
        };

        topology.edges.insert((from, to), edge_info);

        // Update adjacency list
        topology.adjacency_list.entry(from).or_default().push(to);
        topology.adjacency_list.entry(to).or_default().push(from); // Bidirectional

        topology.last_updated = Instant::now();

        // Trigger route recalculation
        self.recalculate_routes().await?;

        Ok(())
    }

    /// Find best route using multiple algorithms
    pub async fn find_best_route(
        &self,
        destination: PeerId,
        criteria: RoutingCriteria,
    ) -> Option<PathInfo> {
        match criteria.algorithm {
            RoutingAlgorithm::Dijkstra => self.find_shortest_path(destination).await,
            RoutingAlgorithm::LoadBalanced => self.find_load_balanced_path(destination).await,
            RoutingAlgorithm::Geographic => self.find_geographic_path(destination).await,
            RoutingAlgorithm::AntColony => self.find_aco_path(destination).await,
            RoutingAlgorithm::Hybrid => self.find_hybrid_path(destination, criteria).await,
        }
    }

    /// Find shortest path using Dijkstra's algorithm
    async fn find_shortest_path(&self, destination: PeerId) -> Option<PathInfo> {
        let shortest_paths = self.shortest_paths.read().await;
        shortest_paths.get(&destination).cloned()
    }

    /// Find load-balanced path
    async fn find_load_balanced_path(&self, destination: PeerId) -> Option<PathInfo> {
        let load_balanced_paths = self.load_balanced_paths.read().await;

        if let Some(paths) = load_balanced_paths.get(&destination) {
            // Select path with lowest congestion
            paths
                .iter()
                .min_by(|a, b| {
                    a.congestion
                        .partial_cmp(&b.congestion)
                        .unwrap_or(Ordering::Equal)
                })
                .cloned()
        } else {
            None
        }
    }

    /// Find geographic path using location information
    async fn find_geographic_path(&self, destination: PeerId) -> Option<PathInfo> {
        if !self.config.enable_geographic {
            return None;
        }

        let geographic_routes = self.geographic_routes.read().await;
        let topology = self.topology.read().await;

        if let Some(route) = geographic_routes.get(&destination) {
            // Convert geographic route to PathInfo
            if topology.nodes.contains_key(&destination)
                && topology.nodes.contains_key(&route.next_hop)
            {
                return Some(PathInfo {
                    destination,
                    next_hop: route.next_hop,
                    path: vec![route.next_hop, destination], // Simplified
                    cost: route.distance_remaining,
                    latency: Duration::from_millis(100), // Estimated
                    bandwidth: 1.0,                      // Default
                    reliability: 0.8,                    // Default
                    congestion: 0.1,                     // Default
                    last_updated: route.last_updated,
                    hop_count: 2, // Simplified
                });
            }
        }

        None
    }

    /// Find ACO-optimized path
    async fn find_aco_path(&self, destination: PeerId) -> Option<PathInfo> {
        if !self.config.enable_aco {
            return None;
        }

        let aco_routes = self.aco_routes.read().await;

        if let Some(aco_route) = aco_routes.get(&destination) {
            // Select best path based on pheromone levels and quality
            if let Some(best_path) = aco_route.paths.iter().max_by(|a, b| {
                (a.pheromone * a.quality)
                    .partial_cmp(&(b.pheromone * b.quality))
                    .unwrap_or(Ordering::Equal)
            }) {
                return Some(PathInfo {
                    destination,
                    next_hop: *best_path.path.first()?,
                    path: best_path.path.clone(),
                    cost: 1.0 / best_path.quality, // Inverse of quality
                    latency: Duration::from_millis(50 * best_path.path.len() as u64),
                    bandwidth: 1.0,
                    reliability: best_path.quality,
                    congestion: 0.1,
                    last_updated: aco_route.last_optimization,
                    hop_count: best_path.path.len() as u8,
                });
            }
        }

        None
    }

    /// Find hybrid path combining multiple algorithms
    async fn find_hybrid_path(
        &self,
        destination: PeerId,
        criteria: RoutingCriteria,
    ) -> Option<PathInfo> {
        // Get paths from different algorithms
        let dijkstra_path = self.find_shortest_path(destination).await;
        let load_balanced_path = self.find_load_balanced_path(destination).await;
        let geographic_path = self.find_geographic_path(destination).await;
        let aco_path = self.find_aco_path(destination).await;

        // Collect all valid paths
        let mut paths = Vec::new();
        if let Some(path) = dijkstra_path {
            paths.push(path);
        }
        if let Some(path) = load_balanced_path {
            paths.push(path);
        }
        if let Some(path) = geographic_path {
            paths.push(path);
        }
        if let Some(path) = aco_path {
            paths.push(path);
        }

        if paths.is_empty() {
            return None;
        }

        // Score paths based on criteria
        paths.into_iter().max_by(|a, b| {
            let score_a = self.score_path(a, &criteria);
            let score_b = self.score_path(b, &criteria);
            score_a.partial_cmp(&score_b).unwrap_or(Ordering::Equal)
        })
    }

    /// Score a path based on routing criteria
    fn score_path(&self, path: &PathInfo, criteria: &RoutingCriteria) -> f64 {
        let mut score = 0.0;

        // Latency component (lower is better)
        score += criteria.latency_weight * (1.0 / (path.latency.as_millis() as f64 + 1.0));

        // Bandwidth component (higher is better)
        score += criteria.bandwidth_weight * path.bandwidth;

        // Reliability component (higher is better)
        score += criteria.reliability_weight * path.reliability;

        // Congestion component (lower is better)
        score += criteria.congestion_weight * (1.0 - path.congestion);

        // Hop count component (lower is better)
        score += criteria.hop_count_weight * (1.0 / (path.hop_count as f64 + 1.0));

        score
    }

    /// Recalculate all routes
    async fn recalculate_routes(&self) -> Result<()> {
        // Run Dijkstra for all destinations
        self.run_dijkstra().await?;

        // Update load-balanced paths
        self.update_load_balanced_paths().await?;

        // Update geographic routes
        if self.config.enable_geographic {
            self.update_geographic_routes().await?;
        }

        // Update ACO routes
        if self.config.enable_aco {
            self.update_aco_routes().await?;
        }

        Ok(())
    }

    /// Run Dijkstra's algorithm for shortest paths
    async fn run_dijkstra(&self) -> Result<()> {
        let topology = self.topology.read().await;
        let mut shortest_paths = self.shortest_paths.write().await;

        // For each node, calculate shortest paths to all other nodes
        for &source in topology.nodes.keys() {
            let paths = self.dijkstra_single_source(&topology, source).await;

            for (destination, path_info) in paths {
                if source != destination {
                    shortest_paths.insert(destination, path_info);
                }
            }
        }

        Ok(())
    }

    /// Single-source Dijkstra implementation
    async fn dijkstra_single_source(
        &self,
        topology: &NetworkTopology,
        source: PeerId,
    ) -> HashMap<PeerId, PathInfo> {
        let mut distances: HashMap<PeerId, f64> = HashMap::new();
        let mut previous: HashMap<PeerId, Option<PeerId>> = HashMap::new();
        let mut heap = BinaryHeap::new();

        // Initialize
        for &node in topology.nodes.keys() {
            distances.insert(node, f64::INFINITY);
            previous.insert(node, None);
        }

        distances.insert(source, 0.0);
        heap.push(DijkstraNode {
            peer_id: source,
            cost: 0.0,
            path: vec![source],
        });

        while let Some(current) = heap.pop() {
            if current.cost > distances[&current.peer_id] {
                continue;
            }

            // Check neighbors
            if let Some(neighbors) = topology.adjacency_list.get(&current.peer_id) {
                for &neighbor in neighbors {
                    if let Some(edge) = topology.edges.get(&(current.peer_id, neighbor)) {
                        let alt = distances[&current.peer_id] + edge.weight;

                        if alt < distances[&neighbor] {
                            distances.insert(neighbor, alt);
                            previous.insert(neighbor, Some(current.peer_id));

                            let mut new_path = current.path.clone();
                            new_path.push(neighbor);

                            heap.push(DijkstraNode {
                                peer_id: neighbor,
                                cost: alt,
                                path: new_path,
                            });
                        }
                    }
                }
            }
        }

        // Build result
        let mut result = HashMap::new();
        for (&destination, &distance) in &distances {
            if destination != source && distance < f64::INFINITY {
                let path = self.reconstruct_path(&previous, source, destination);
                if let Some(next_hop) = path.get(1) {
                    let path_info = PathInfo {
                        destination,
                        next_hop: *next_hop,
                        path: path.clone(),
                        cost: distance,
                        latency: Duration::from_millis((distance * 50.0) as u64), // Estimate
                        bandwidth: 1.0,                                           // Default
                        reliability: 0.9,                                         // Default
                        congestion: 0.1,                                          // Default
                        last_updated: Instant::now(),
                        hop_count: path.len() as u8 - 1,
                    };
                    result.insert(destination, path_info);
                }
            }
        }

        result
    }

    /// Reconstruct path from Dijkstra results
    fn reconstruct_path(
        &self,
        previous: &HashMap<PeerId, Option<PeerId>>,
        source: PeerId,
        destination: PeerId,
    ) -> Vec<PeerId> {
        let mut path = Vec::new();
        let mut current = destination;

        while let Some(&Some(prev)) = previous.get(&current) {
            path.push(current);
            current = prev;
            if current == source {
                break;
            }
        }

        path.push(source);
        path.reverse();
        path
    }

    /// Update load-balanced paths
    async fn update_load_balanced_paths(&self) -> Result<()> {
        // Implementation would analyze current traffic and create alternative paths
        // For now, use shortest paths as baseline
        let shortest_paths = self.shortest_paths.read().await;
        let mut load_balanced_paths = self.load_balanced_paths.write().await;

        for (destination, path) in shortest_paths.iter() {
            load_balanced_paths.insert(*destination, vec![path.clone()]);
        }

        Ok(())
    }

    /// Update geographic routes
    async fn update_geographic_routes(&self) -> Result<()> {
        // Geographic routing implementation would go here
        Ok(())
    }

    /// Update ACO routes
    async fn update_aco_routes(&self) -> Result<()> {
        // Ant Colony Optimization implementation would go here
        Ok(())
    }

    /// Calculate dynamic link weight
    fn calculate_link_weight(&self, latency: Duration, bandwidth: f64, packet_loss: f64) -> f64 {
        let latency_ms = latency.as_millis() as f64;
        let bandwidth_factor = 1.0 / (bandwidth + 0.1); // Avoid division by zero
        let loss_factor = 1.0 + packet_loss * 10.0; // Penalize packet loss

        latency_ms * bandwidth_factor * loss_factor
    }

    /// Get routing statistics
    pub async fn get_metrics(&self) -> RoutingMetrics {
        let metrics = self.metrics.read().await;
        metrics.clone()
    }
}

impl NetworkTopology {
    fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: HashMap::new(),
            adjacency_list: HashMap::new(),
            last_updated: Instant::now(),
        }
    }
}

impl Location {
    /// Calculate distance to another location in meters
    pub fn distance_to(&self, other: &Location) -> f64 {
        let r = 6371000.0; // Earth's radius in meters

        let lat1 = self.latitude.to_radians();
        let lat2 = other.latitude.to_radians();
        let delta_lat = (other.latitude - self.latitude).to_radians();
        let delta_lon = (other.longitude - self.longitude).to_radians();

        let a = (delta_lat / 2.0).sin().powi(2)
            + lat1.cos() * lat2.cos() * (delta_lon / 2.0).sin().powi(2);
        let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());

        r * c
    }

    /// Calculate bearing to another location in radians
    pub fn bearing_to(&self, other: &Location) -> f64 {
        let lat1 = self.latitude.to_radians();
        let lat2 = other.latitude.to_radians();
        let delta_lon = (other.longitude - self.longitude).to_radians();

        let y = delta_lon.sin() * lat2.cos();
        let x = lat1.cos() * lat2.sin() - lat1.sin() * lat2.cos() * delta_lon.cos();

        y.atan2(x)
    }
}

/// Routing criteria for path selection
#[derive(Debug, Clone)]
pub struct RoutingCriteria {
    pub algorithm: RoutingAlgorithm,
    pub latency_weight: f64,
    pub bandwidth_weight: f64,
    pub reliability_weight: f64,
    pub congestion_weight: f64,
    pub hop_count_weight: f64,
    pub energy_weight: f64,
}

/// Available routing algorithms
#[derive(Debug, Clone, Copy)]
pub enum RoutingAlgorithm {
    Dijkstra,
    LoadBalanced,
    Geographic,
    AntColony,
    Hybrid,
}

impl Default for RoutingCriteria {
    fn default() -> Self {
        Self {
            algorithm: RoutingAlgorithm::Hybrid,
            latency_weight: 0.3,
            bandwidth_weight: 0.2,
            reliability_weight: 0.2,
            congestion_weight: 0.15,
            hop_count_weight: 0.1,
            energy_weight: 0.05,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_routing_table_creation() {
        let config = RoutingConfig::default();
        let routing_table = AdvancedRoutingTable::new(config);

        let metrics = routing_table.get_metrics().await;
        assert_eq!(metrics.total_routes, 0);
    }

    #[tokio::test]
    async fn test_topology_update() {
        let config = RoutingConfig::default();
        let routing_table = AdvancedRoutingTable::new(config);

        let peer_id = [1u8; 32];
        let location = Location {
            latitude: 37.7749,
            longitude: -122.4194,
            altitude: None,
            accuracy: 10.0,
        };
        let capabilities = NodeCapabilities::default();

        routing_table
            .update_node(peer_id, Some(location), capabilities)
            .await
            .expect("Node update should succeed");

        let topology = routing_table.topology.read().await;
        assert!(topology.nodes.contains_key(&peer_id));
    }

    #[test]
    fn test_location_distance() {
        let sf = Location {
            latitude: 37.7749,
            longitude: -122.4194,
            altitude: None,
            accuracy: 10.0,
        };

        let ny = Location {
            latitude: 40.7128,
            longitude: -74.0060,
            altitude: None,
            accuracy: 10.0,
        };

        let distance = sf.distance_to(&ny);

        // Distance between SF and NY is approximately 4,135 km
        assert!((distance - 4_135_000.0).abs() < 50_000.0); // Allow 50km error
    }

    #[test]
    fn test_routing_criteria_scoring() {
        let criteria = RoutingCriteria::default();
        let path = PathInfo {
            destination: [0u8; 32],
            next_hop: [1u8; 32],
            path: vec![[1u8; 32], [0u8; 32]],
            cost: 1.0,
            latency: Duration::from_millis(100),
            bandwidth: 10.0,
            reliability: 0.9,
            congestion: 0.1,
            last_updated: Instant::now(),
            hop_count: 2,
        };

        let config = RoutingConfig::default();
        let routing_table = AdvancedRoutingTable::new(config);
        let score = routing_table.score_path(&path, &criteria);

        assert!(score > 0.0);
    }
}
