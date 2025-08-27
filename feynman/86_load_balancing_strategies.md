# Chapter 86: Load Balancing Strategies - Distributing the Work Fairly

## Understanding Load Balancing Through BitCraps Connection Management
*"Load balancing is like being a restaurant host - you want every table to have the right number of customers, not too crowded, not too empty."*

---

## Part I: The Load Balancing Challenge

Imagine you run a restaurant with multiple tables and servers. Customers (network requests) keep coming in, but they arrive unpredictably:
- Sometimes everyone wants lunch at exactly 12:00 PM
- Some tables are perfect for couples, others for large groups
- Some servers are faster than others
- Some customers need special attention

Without good management, you get:
- Some servers overwhelmed while others are idle
- Long wait times even when the restaurant isn't full
- Customers leaving because service is too slow
- Uneven quality of service

In BitCraps, this same problem happens with network connections, game processing, and consensus participation. Players connect at random times, games have different computational needs, and nodes have varying capabilities. Load balancing ensures no single node gets overwhelmed while others sit idle.

## Part II: The BitCraps Load Balancing Architecture

### Connection Load Balancing

```rust
// From src/transport/connection_pool.rs (extended)
pub struct ConnectionLoadBalancer {
    connection_pools: HashMap<NodeId, ConnectionPool>,
    load_metrics: Arc<RwLock<HashMap<NodeId, LoadMetrics>>>,
    balancing_strategy: LoadBalancingStrategy,
    health_monitor: HealthMonitor,
}

#[derive(Clone, Debug)]
pub enum LoadBalancingStrategy {
    RoundRobin,
    LeastConnections,
    WeightedRandom,
    LatencyBased,
    CapabilityAware,
    Adaptive,
}

impl ConnectionLoadBalancer {
    pub async fn select_best_connection(&self, 
        request_type: RequestType
    ) -> Result<Connection, LoadBalancingError> {
        
        // Get all available connections
        let available_connections = self.get_healthy_connections().await?;
        
        if available_connections.is_empty() {
            return Err(LoadBalancingError::NoAvailableConnections);
        }
        
        // Select based on current strategy
        let selected_connection = match self.balancing_strategy {
            LoadBalancingStrategy::RoundRobin => {
                self.round_robin_selection(&available_connections).await
            }
            
            LoadBalancingStrategy::LeastConnections => {
                self.least_connections_selection(&available_connections).await?
            }
            
            LoadBalancingStrategy::WeightedRandom => {
                self.weighted_random_selection(&available_connections).await?
            }
            
            LoadBalancingStrategy::LatencyBased => {
                self.latency_based_selection(&available_connections).await?
            }
            
            LoadBalancingStrategy::CapabilityAware => {
                self.capability_aware_selection(&available_connections, request_type).await?
            }
            
            LoadBalancingStrategy::Adaptive => {
                self.adaptive_selection(&available_connections, request_type).await?
            }
        };
        
        // Update load metrics
        self.record_connection_use(&selected_connection).await;
        
        Ok(selected_connection)
    }
    
    async fn least_connections_selection(&self, 
        connections: &[Connection]
    ) -> Result<Connection, LoadBalancingError> {
        
        let load_metrics = self.load_metrics.read().await;
        
        let mut best_connection = None;
        let mut lowest_load = f64::INFINITY;
        
        for connection in connections {
            let node_id = connection.peer_node_id();
            
            if let Some(metrics) = load_metrics.get(&node_id) {
                // Calculate current load score
                let load_score = self.calculate_load_score(metrics);
                
                if load_score < lowest_load {
                    lowest_load = load_score;
                    best_connection = Some(connection.clone());
                }
            }
        }
        
        best_connection.ok_or(LoadBalancingError::NoSuitableConnection)
    }
    
    async fn capability_aware_selection(&self,
        connections: &[Connection],
        request_type: RequestType
    ) -> Result<Connection, LoadBalancingError> {
        
        // Filter connections by capability requirements
        let mut capable_connections = Vec::new();
        
        for connection in connections {
            let node_capabilities = connection.get_peer_capabilities().await?;
            
            if self.node_can_handle_request(&node_capabilities, &request_type) {
                capable_connections.push(connection.clone());
            }
        }
        
        if capable_connections.is_empty() {
            return Err(LoadBalancingError::NoCapableNodes);
        }
        
        // Among capable nodes, pick the least loaded
        self.least_connections_selection(&capable_connections).await
    }
    
    async fn adaptive_selection(&self,
        connections: &[Connection],
        request_type: RequestType
    ) -> Result<Connection, LoadBalancingError> {
        
        // Analyze current system state
        let system_state = self.analyze_system_state().await?;
        
        // Choose strategy based on current conditions
        let optimal_strategy = match system_state {
            SystemState::HighLoad => LoadBalancingStrategy::LeastConnections,
            SystemState::HighLatency => LoadBalancingStrategy::LatencyBased,
            SystemState::UnbalancedCapabilities => LoadBalancingStrategy::CapabilityAware,
            SystemState::Normal => LoadBalancingStrategy::WeightedRandom,
            SystemState::LowLoad => LoadBalancingStrategy::RoundRobin,
        };
        
        // Temporarily use the optimal strategy
        let original_strategy = self.balancing_strategy.clone();
        let temp_balancer = ConnectionLoadBalancer {
            balancing_strategy: optimal_strategy,
            ..self.clone()
        };
        
        let result = temp_balancer.select_best_connection(request_type).await;
        
        // Track strategy effectiveness
        self.record_strategy_result(&optimal_strategy, &result).await;
        
        result
    }
    
    fn calculate_load_score(&self, metrics: &LoadMetrics) -> f64 {
        // Combine multiple load factors into single score
        let cpu_score = metrics.cpu_usage * 0.3;
        let memory_score = metrics.memory_usage * 0.2;
        let connection_score = (metrics.active_connections as f64 / metrics.max_connections as f64) * 0.3;
        let latency_score = (metrics.average_latency.as_millis() as f64 / 1000.0) * 0.2;
        
        cpu_score + memory_score + connection_score + latency_score
    }
}
```

### Game Processing Load Balancing

Different types of games require different computational resources:

```rust
// From src/gaming/consensus_game_manager.rs (extended)
pub struct GameProcessingLoadBalancer {
    processing_nodes: HashMap<NodeId, ProcessingCapacity>,
    game_queue: PriorityQueue<GameRequest>,
    load_monitor: ProcessingLoadMonitor,
}

impl GameProcessingLoadBalancer {
    pub async fn assign_game_to_processor(&self, 
        game_request: GameRequest
    ) -> Result<NodeId, GameAssignmentError> {
        
        // Calculate resource requirements for this game
        let requirements = self.calculate_game_requirements(&game_request);
        
        // Find nodes that can handle this game
        let capable_nodes = self.find_capable_nodes(&requirements).await?;
        
        if capable_nodes.is_empty() {
            // Queue the game for later processing
            self.queue_game_request(game_request).await;
            return Err(GameAssignmentError::NoCapableNodes);
        }
        
        // Select best node based on current load
        let selected_node = self.select_optimal_processor(&capable_nodes, &requirements).await?;
        
        // Reserve resources on selected node
        self.reserve_resources(selected_node, &requirements).await?;
        
        Ok(selected_node)
    }
    
    async fn select_optimal_processor(&self,
        capable_nodes: &[NodeId],
        requirements: &GameRequirements
    ) -> Result<NodeId, GameAssignmentError> {
        
        let mut best_node = None;
        let mut best_score = f64::INFINITY;
        
        for node_id in capable_nodes {
            let score = self.calculate_node_suitability(*node_id, requirements).await?;
            
            if score < best_score {
                best_score = score;
                best_node = Some(*node_id);
            }
        }
        
        best_node.ok_or(GameAssignmentError::NoSuitableNode)
    }
    
    async fn calculate_node_suitability(&self,
        node_id: NodeId,
        requirements: &GameRequirements
    ) -> Result<f64, GameAssignmentError> {
        
        let capacity = self.processing_nodes.get(&node_id)
            .ok_or(GameAssignmentError::NodeNotFound)?;
        
        let current_load = self.load_monitor.get_current_load(node_id).await?;
        
        // Calculate suitability score (lower is better)
        let mut score = 0.0;
        
        // CPU utilization factor
        let cpu_utilization = current_load.cpu_usage + (requirements.cpu_cores as f64 / capacity.total_cpu_cores as f64);
        if cpu_utilization > 0.8 {
            score += (cpu_utilization - 0.8) * 100.0; // Penalty for high CPU usage
        }
        
        // Memory utilization factor
        let memory_utilization = current_load.memory_usage + (requirements.memory_mb as f64 / capacity.total_memory_mb as f64);
        if memory_utilization > 0.85 {
            score += (memory_utilization - 0.85) * 50.0; // Penalty for high memory usage
        }
        
        // Network capacity factor
        let network_utilization = current_load.network_usage + requirements.expected_network_load;
        score += network_utilization * 10.0;
        
        // Geographic preference (lower latency to most players)
        let geographic_penalty = self.calculate_geographic_penalty(node_id, requirements).await;
        score += geographic_penalty;
        
        // Specialization bonus (some nodes are optimized for certain game types)
        if capacity.specialized_for.contains(&requirements.game_type) {
            score -= 20.0; // Bonus for specialized nodes
        }
        
        Ok(score)
    }
    
    pub async fn rebalance_game_assignments(&self) -> Result<Vec<GameMigration>, RebalanceError> {
        let mut migrations = Vec::new();
        
        // Get current load across all nodes
        let load_distribution = self.get_load_distribution().await?;
        
        // Identify overloaded and underutilized nodes
        let overloaded_nodes = load_distribution.iter()
            .filter(|(_, load)| load.is_overloaded())
            .map(|(node_id, _)| *node_id)
            .collect::<Vec<_>>();
        
        let underutilized_nodes = load_distribution.iter()
            .filter(|(_, load)| load.is_underutilized())
            .map(|(node_id, _)| *node_id)
            .collect::<Vec<_>>();
        
        // For each overloaded node, try to migrate games to underutilized nodes
        for overloaded_node in overloaded_nodes {
            let games_to_migrate = self.select_games_for_migration(overloaded_node).await?;
            
            for game in games_to_migrate {
                if let Some(target_node) = self.find_migration_target(&underutilized_nodes, &game).await? {
                    migrations.push(GameMigration {
                        game_id: game.id,
                        from_node: overloaded_node,
                        to_node: target_node,
                        estimated_transfer_time: self.estimate_migration_time(&game).await,
                    });
                }
            }
        }
        
        Ok(migrations)
    }
}
```

### Consensus Load Balancing

BitCraps distributes consensus work to prevent any single node from becoming a bottleneck:

```rust
// From src/protocol/consensus/engine.rs (extended)
pub struct ConsensusLoadBalancer {
    validator_nodes: HashMap<NodeId, ValidatorCapacity>,
    consensus_queues: HashMap<ConsensusType, VecDeque<ConsensusTask>>,
    reputation_scores: HashMap<NodeId, f64>,
    load_balancer: ConsensusTaskBalancer,
}

impl ConsensusLoadBalancer {
    pub async fn assign_consensus_task(&self, 
        task: ConsensusTask
    ) -> Result<Vec<NodeId>, ConsensusAssignmentError> {
        
        // Different consensus tasks need different numbers of validators
        let required_validators = match task.consensus_type {
            ConsensusType::DiceRoll => 3,        // Simple majority
            ConsensusType::TokenTransfer => 5,   // Higher security needed
            ConsensusType::GameResolution => 7,  // Critical game outcomes
            ConsensusType::CheatDetection => 9,  // Maximum security
        };
        
        // Get eligible validators
        let eligible_validators = self.get_eligible_validators(&task).await?;
        
        if eligible_validators.len() < required_validators {
            return Err(ConsensusAssignmentError::InsufficientValidators);
        }
        
        // Select optimal subset of validators
        let selected_validators = self.select_optimal_validators(
            &eligible_validators,
            required_validators,
            &task
        ).await?;
        
        // Assign task to selected validators
        for validator_id in &selected_validators {
            self.assign_task_to_validator(*validator_id, task.clone()).await?;
        }
        
        Ok(selected_validators)
    }
    
    async fn select_optimal_validators(&self,
        candidates: &[NodeId],
        count: usize,
        task: &ConsensusTask
    ) -> Result<Vec<NodeId>, ConsensusAssignmentError> {
        
        // Score each candidate validator
        let mut scored_candidates: Vec<(NodeId, f64)> = Vec::new();
        
        for candidate_id in candidates {
            let score = self.calculate_validator_score(*candidate_id, task).await?;
            scored_candidates.push((*candidate_id, score));
        }
        
        // Sort by score (higher is better for validators)
        scored_candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        
        // Use a balanced selection approach
        let mut selected = Vec::new();
        let mut load_balance_index = 0;
        
        // First, select the highest-scoring validators up to half the requirement
        let priority_count = (count / 2).max(1);
        for i in 0..priority_count.min(scored_candidates.len()) {
            selected.push(scored_candidates[i].0);
        }
        
        // Then, balance the rest by load and diversity
        while selected.len() < count && load_balance_index < scored_candidates.len() {
            let candidate = scored_candidates[load_balance_index].0;
            
            if !selected.contains(&candidate) {
                // Check if adding this validator improves diversity
                if self.improves_validator_diversity(&selected, candidate).await {
                    selected.push(candidate);
                }
            }
            
            load_balance_index += 1;
        }
        
        // If we still don't have enough, just take the highest remaining scores
        while selected.len() < count && selected.len() < scored_candidates.len() {
            for (candidate_id, _) in &scored_candidates {
                if !selected.contains(candidate_id) && selected.len() < count {
                    selected.push(*candidate_id);
                }
            }
        }
        
        Ok(selected)
    }
    
    async fn calculate_validator_score(&self,
        validator_id: NodeId,
        task: &ConsensusTask
    ) -> Result<f64, ConsensusAssignmentError> {
        
        let capacity = self.validator_nodes.get(&validator_id)
            .ok_or(ConsensusAssignmentError::ValidatorNotFound)?;
        
        let current_load = self.get_validator_load(validator_id).await?;
        let reputation = self.reputation_scores.get(&validator_id).unwrap_or(&0.5);
        
        let mut score = 0.0;
        
        // Reputation score (0.0 to 1.0) - 40% weight
        score += reputation * 40.0;
        
        // Available capacity - 30% weight
        let capacity_ratio = (capacity.max_concurrent_tasks as f64 - current_load.active_tasks as f64) 
                           / capacity.max_concurrent_tasks as f64;
        score += capacity_ratio * 30.0;
        
        // Response time history - 20% weight
        let response_time_score = 1.0 - (current_load.average_response_time.as_millis() as f64 / 5000.0).min(1.0);
        score += response_time_score * 20.0;
        
        // Network connectivity - 10% weight
        let connectivity_score = current_load.peer_connectivity_ratio;
        score += connectivity_score * 10.0;
        
        // Penalty for specialization mismatch
        if !capacity.specialized_consensus_types.contains(&task.consensus_type) {
            score *= 0.8; // 20% penalty for non-specialized validators
        }
        
        Ok(score)
    }
    
    pub async fn monitor_and_rebalance(&self) -> Result<(), RebalanceError> {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        
        loop {
            interval.tick().await;
            
            // Check for overloaded validators
            let overloaded_validators = self.identify_overloaded_validators().await?;
            
            for validator_id in overloaded_validators {
                // Try to redistribute some of their tasks
                let redistributable_tasks = self.get_redistributable_tasks(validator_id).await?;
                
                for task in redistributable_tasks {
                    if let Ok(new_validators) = self.find_alternative_validators(&task, validator_id).await {
                        // Migrate task to less loaded validators
                        self.migrate_consensus_task(task, validator_id, new_validators).await?;
                    }
                }
            }
            
            // Check for underutilized validators and try to give them more work
            let underutilized_validators = self.identify_underutilized_validators().await?;
            
            if !underutilized_validators.is_empty() {
                // Look for tasks that could benefit from additional validators
                let enhanceable_tasks = self.find_enhanceable_tasks().await?;
                
                for task in enhanceable_tasks {
                    let additional_validators = self.select_additional_validators(
                        &underutilized_validators,
                        &task
                    ).await?;
                    
                    // Add validators to increase consensus reliability
                    self.add_validators_to_task(task.id, additional_validators).await?;
                }
            }
        }
    }
}
```

## Part III: Dynamic Load Balancing

BitCraps adjusts its load balancing in real-time based on changing conditions:

```rust
pub struct DynamicLoadBalancer {
    current_strategy: LoadBalancingStrategy,
    strategy_performance: HashMap<LoadBalancingStrategy, StrategyMetrics>,
    adaptation_engine: AdaptationEngine,
    load_predictor: LoadPredictor,
}

impl DynamicLoadBalancer {
    pub async fn adapt_strategy(&mut self) -> Result<(), AdaptationError> {
        // Analyze current system performance
        let current_metrics = self.collect_current_metrics().await?;
        
        // Predict future load patterns
        let predicted_load = self.load_predictor.predict_next_period().await?;
        
        // Evaluate effectiveness of current strategy
        let current_effectiveness = self.evaluate_current_strategy(&current_metrics).await;
        
        // If current strategy is underperforming, try alternatives
        if current_effectiveness < 0.7 { // 70% effectiveness threshold
            let alternative_strategy = self.select_alternative_strategy(
                &current_metrics,
                &predicted_load
            ).await?;
            
            if alternative_strategy != self.current_strategy {
                self.switch_strategy(alternative_strategy).await?;
            }
        }
        
        Ok(())
    }
    
    async fn select_alternative_strategy(&self,
        current_metrics: &SystemMetrics,
        predicted_load: &LoadPrediction
    ) -> Result<LoadBalancingStrategy, AdaptationError> {
        
        let strategy = match (current_metrics.primary_bottleneck(), predicted_load.load_type) {
            // High latency issues - prioritize latency-based routing
            (Bottleneck::NetworkLatency, _) => LoadBalancingStrategy::LatencyBased,
            
            // CPU bottlenecks - balance by computational load
            (Bottleneck::CPU, _) => LoadBalancingStrategy::CapabilityAware,
            
            // Memory pressure - use least connections to spread memory usage
            (Bottleneck::Memory, _) => LoadBalancingStrategy::LeastConnections,
            
            // Uneven load distribution - use weighted random for better distribution
            (Bottleneck::LoadImbalance, _) => LoadBalancingStrategy::WeightedRandom,
            
            // Predicted high burst load - preemptively use round robin
            (_, LoadType::BurstLoad) => LoadBalancingStrategy::RoundRobin,
            
            // Predicted sustained high load - use adaptive approach
            (_, LoadType::SustainedHigh) => LoadBalancingStrategy::Adaptive,
            
            // Normal conditions - use weighted random for good general performance
            _ => LoadBalancingStrategy::WeightedRandom,
        };
        
        Ok(strategy)
    }
    
    async fn switch_strategy(&mut self, new_strategy: LoadBalancingStrategy) -> Result<(), AdaptationError> {
        println!("Switching load balancing strategy from {:?} to {:?}", 
                self.current_strategy, new_strategy);
        
        // Record performance of old strategy before switching
        let final_metrics = self.collect_current_metrics().await?;
        self.record_strategy_performance(self.current_strategy.clone(), final_metrics).await;
        
        // Switch to new strategy
        let old_strategy = self.current_strategy.clone();
        self.current_strategy = new_strategy.clone();
        
        // Initialize new strategy
        self.initialize_strategy(&new_strategy).await?;
        
        // Monitor strategy transition
        self.monitor_strategy_transition(old_strategy, new_strategy).await;
        
        Ok(())
    }
}

struct LoadPredictor {
    historical_data: VecDeque<LoadSnapshot>,
    prediction_model: PredictionModel,
}

impl LoadPredictor {
    async fn predict_next_period(&self) -> Result<LoadPrediction, PredictionError> {
        // Use historical patterns to predict load
        let time_of_day_pattern = self.analyze_time_of_day_patterns();
        let day_of_week_pattern = self.analyze_weekly_patterns();
        let recent_trend = self.analyze_recent_trends();
        
        // Combine different prediction signals
        let predicted_load = LoadPrediction {
            expected_connection_count: self.predict_connection_count(&time_of_day_pattern, &recent_trend),
            expected_game_starts: self.predict_game_activity(&day_of_week_pattern),
            expected_consensus_load: self.predict_consensus_activity(&recent_trend),
            load_type: self.classify_predicted_load_type(),
            confidence: self.calculate_prediction_confidence(),
        };
        
        Ok(predicted_load)
    }
    
    fn analyze_time_of_day_patterns(&self) -> TimePattern {
        let mut hourly_averages = HashMap::new();
        
        for snapshot in &self.historical_data {
            let hour = snapshot.timestamp.hour();
            let entry = hourly_averages.entry(hour).or_insert_with(Vec::new);
            entry.push(snapshot.total_load);
        }
        
        // Calculate average load for each hour
        let mut patterns = HashMap::new();
        for (hour, loads) in hourly_averages {
            let average_load: f64 = loads.iter().sum::<f64>() / loads.len() as f64;
            patterns.insert(hour, average_load);
        }
        
        TimePattern { hourly_patterns: patterns }
    }
}
```

## Part IV: Geographic Load Distribution

For global BitCraps deployment, geographic distribution is crucial:

```rust
pub struct GeographicLoadBalancer {
    regional_nodes: HashMap<Region, Vec<NodeId>>,
    latency_matrix: LatencyMatrix,
    regional_load_limits: HashMap<Region, LoadLimit>,
    cross_region_balancer: CrossRegionBalancer,
}

impl GeographicLoadBalancer {
    pub async fn select_regional_nodes(&self,
        player_locations: Vec<GeographicLocation>,
        game_requirements: GameRequirements
    ) -> Result<Vec<NodeId>, GeographicBalancingError> {
        
        // Calculate optimal regions for this game based on player locations
        let optimal_regions = self.calculate_optimal_regions(&player_locations).await?;
        
        let mut selected_nodes = Vec::new();
        
        for region in optimal_regions {
            // Get available nodes in this region
            let regional_nodes = self.regional_nodes.get(&region)
                .ok_or(GeographicBalancingError::NoNodesInRegion(region))?;
            
            // Filter by capability and current load
            let suitable_nodes = self.filter_suitable_regional_nodes(
                regional_nodes,
                &game_requirements
            ).await?;
            
            if !suitable_nodes.is_empty() {
                // Select best node in region
                let best_node = self.select_best_regional_node(&suitable_nodes, region).await?;
                selected_nodes.push(best_node);
            } else {
                // No suitable nodes in preferred region, try cross-region assignment
                let cross_region_node = self.cross_region_balancer
                    .find_alternative_node(region, &game_requirements)
                    .await?;
                selected_nodes.push(cross_region_node);
            }
        }
        
        Ok(selected_nodes)
    }
    
    async fn calculate_optimal_regions(&self,
        player_locations: &[GeographicLocation]
    ) -> Result<Vec<Region>, GeographicBalancingError> {
        
        if player_locations.is_empty() {
            return Ok(vec![Region::default()]);
        }
        
        // Calculate geographic center of players
        let center = self.calculate_geographic_center(player_locations);
        
        // Find regions that minimize total latency to all players
        let mut region_scores: Vec<(Region, f64)> = Vec::new();
        
        for region in Region::all() {
            let regional_center = region.get_center_location();
            let total_latency = self.calculate_total_latency_from_region(
                regional_center,
                player_locations
            ).await?;
            
            // Consider regional load when scoring
            let load_penalty = self.get_regional_load_penalty(region).await;
            let adjusted_score = total_latency + load_penalty;
            
            region_scores.push((region, adjusted_score));
        }
        
        // Sort by score (lower is better)
        region_scores.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        
        // Select top regions (usually 1-3 regions for redundancy)
        let optimal_region_count = self.calculate_optimal_region_count(player_locations.len());
        let optimal_regions = region_scores
            .into_iter()
            .take(optimal_region_count)
            .map(|(region, _)| region)
            .collect();
        
        Ok(optimal_regions)
    }
    
    pub async fn rebalance_across_regions(&self) -> Result<Vec<RegionalMigration>, RegionalRebalanceError> {
        let mut migrations = Vec::new();
        
        // Analyze load distribution across regions
        let regional_loads = self.get_regional_load_distribution().await?;
        
        // Identify overloaded and underutilized regions
        for (overloaded_region, load) in regional_loads.iter() {
            if load.is_overloaded() {
                // Find games that could be migrated to other regions
                let migratable_games = self.find_migratable_games(*overloaded_region).await?;
                
                for game in migratable_games {
                    // Find suitable destination region
                    if let Some(target_region) = self.find_migration_target_region(&game, *overloaded_region).await? {
                        migrations.push(RegionalMigration {
                            game_id: game.id,
                            from_region: *overloaded_region,
                            to_region: target_region,
                            estimated_latency_impact: self.calculate_latency_impact(&game, target_region).await,
                            player_consent_required: self.requires_player_consent(&game, target_region),
                        });
                    }
                }
            }
        }
        
        Ok(migrations)
    }
}
```

## Part V: Practical Load Balancing Exercise

Let's implement a simple HTTP load balancer:

**Exercise: Round-Robin HTTP Load Balancer**

```rust
pub struct HttpLoadBalancer {
    servers: Vec<ServerInfo>,
    current_index: AtomicUsize,
    health_checker: HealthChecker,
    metrics: Arc<Mutex<LoadBalancerMetrics>>,
}

impl HttpLoadBalancer {
    pub fn new(servers: Vec<ServerInfo>) -> Self {
        HttpLoadBalancer {
            servers,
            current_index: AtomicUsize::new(0),
            health_checker: HealthChecker::new(),
            metrics: Arc::new(Mutex::new(LoadBalancerMetrics::new())),
        }
    }
    
    pub async fn handle_request(&self, request: HttpRequest) -> Result<HttpResponse, LoadBalancerError> {
        // Get healthy servers
        let healthy_servers = self.get_healthy_servers().await?;
        
        if healthy_servers.is_empty() {
            return Err(LoadBalancerError::NoHealthyServers);
        }
        
        // Select server using round-robin
        let selected_server = self.round_robin_select(&healthy_servers);
        
        // Record request start
        let request_start = Instant::now();
        
        // Forward request to selected server
        let response = match self.forward_request(&selected_server, request).await {
            Ok(response) => {
                // Record successful request
                self.record_successful_request(&selected_server, request_start.elapsed()).await;
                response
            }
            Err(e) => {
                // Record failed request
                self.record_failed_request(&selected_server, e.clone()).await;
                return Err(LoadBalancerError::RequestFailed(e));
            }
        };
        
        Ok(response)
    }
    
    fn round_robin_select(&self, servers: &[ServerInfo]) -> ServerInfo {
        let index = self.current_index.fetch_add(1, Ordering::SeqCst) % servers.len();
        servers[index].clone()
    }
    
    async fn forward_request(&self, server: &ServerInfo, request: HttpRequest) -> Result<HttpResponse, RequestError> {
        let client = reqwest::Client::new();
        
        let url = format!("http://{}:{}{}", server.address, server.port, request.path);
        
        let response = match request.method {
            HttpMethod::GET => client.get(&url),
            HttpMethod::POST => client.post(&url).json(&request.body),
            HttpMethod::PUT => client.put(&url).json(&request.body),
            HttpMethod::DELETE => client.delete(&url),
        }
        .timeout(Duration::from_secs(30))
        .send()
        .await?;
        
        let status = response.status().as_u16();
        let body = response.text().await?;
        
        Ok(HttpResponse {
            status_code: status,
            body,
            headers: HashMap::new(), // Simplified for example
        })
    }
    
    async fn get_healthy_servers(&self) -> Result<Vec<ServerInfo>, LoadBalancerError> {
        let mut healthy_servers = Vec::new();
        
        for server in &self.servers {
            if self.health_checker.is_healthy(server).await {
                healthy_servers.push(server.clone());
            }
        }
        
        Ok(healthy_servers)
    }
    
    async fn record_successful_request(&self, server: &ServerInfo, duration: Duration) {
        let mut metrics = self.metrics.lock().await;
        metrics.record_success(server.clone(), duration);
    }
    
    async fn record_failed_request(&self, server: &ServerInfo, error: RequestError) {
        let mut metrics = self.metrics.lock().await;
        metrics.record_failure(server.clone(), error);
    }
    
    pub async fn get_metrics(&self) -> LoadBalancerMetrics {
        let metrics = self.metrics.lock().await;
        metrics.clone()
    }
}

struct HealthChecker {
    health_cache: Arc<Mutex<HashMap<ServerInfo, (bool, Instant)>>>,
}

impl HealthChecker {
    fn new() -> Self {
        HealthChecker {
            health_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    
    async fn is_healthy(&self, server: &ServerInfo) -> bool {
        // Check cache first
        {
            let cache = self.health_cache.lock().await;
            if let Some((is_healthy, last_check)) = cache.get(server) {
                if last_check.elapsed() < Duration::from_secs(30) {
                    return *is_healthy;
                }
            }
        }
        
        // Perform health check
        let is_healthy = self.perform_health_check(server).await;
        
        // Update cache
        {
            let mut cache = self.health_cache.lock().await;
            cache.insert(server.clone(), (is_healthy, Instant::now()));
        }
        
        is_healthy
    }
    
    async fn perform_health_check(&self, server: &ServerInfo) -> bool {
        let client = reqwest::Client::new();
        let health_url = format!("http://{}:{}/health", server.address, server.port);
        
        match client.get(&health_url)
            .timeout(Duration::from_secs(5))
            .send()
            .await 
        {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        }
    }
}

#[tokio::test]
async fn test_load_balancer() {
    let servers = vec![
        ServerInfo { address: "127.0.0.1".to_string(), port: 8001 },
        ServerInfo { address: "127.0.0.1".to_string(), port: 8002 },
        ServerInfo { address: "127.0.0.1".to_string(), port: 8003 },
    ];
    
    let load_balancer = HttpLoadBalancer::new(servers);
    
    // Send multiple requests
    let mut tasks = Vec::new();
    
    for i in 0..9 {
        let lb = load_balancer.clone();
        let task = tokio::spawn(async move {
            let request = HttpRequest {
                method: HttpMethod::GET,
                path: format!("/test/{}", i),
                body: None,
                headers: HashMap::new(),
            };
            
            lb.handle_request(request).await
        });
        
        tasks.push(task);
    }
    
    // Wait for all requests
    let results = futures::future::join_all(tasks).await;
    
    // Verify round-robin distribution
    let metrics = load_balancer.get_metrics().await;
    println!("Load balancer metrics: {:?}", metrics);
    
    // Each server should have received 3 requests (9 requests / 3 servers)
    assert_eq!(metrics.total_requests(), 9);
}
```

## Conclusion: Load Balancing as System Optimization

Load balancing is the art of optimal resource utilization. It's what allows BitCraps to:

1. **Handle varying loads** - From quiet periods to peak gaming hours
2. **Provide consistent performance** - No matter which node handles your request  
3. **Scale efficiently** - Add capacity where and when it's needed
4. **Maintain reliability** - Distribute risk across multiple nodes
5. **Optimize user experience** - Route requests to the best available resources

The key insights for load balancing:

1. **No single strategy works everywhere** - Different situations need different approaches
2. **Monitor continuously** - Load patterns change, your balancing must adapt
3. **Consider multiple factors** - Not just current load, but latency, capability, geography
4. **Plan for failures** - Load balancing helps, but nodes will still fail
5. **Measure and optimize** - Track strategy effectiveness and adjust

Remember: Good load balancing is invisible to users. They just experience a fast, reliable system. Bad load balancing is very visible - slow responses, timeouts, and frustrated players who can't enjoy their BitCraps games.

In distributed gaming where real money is at stake, load balancing isn't just about performance - it's about fairness, reliability, and trust.