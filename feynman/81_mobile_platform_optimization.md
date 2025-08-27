# Chapter 81: Mobile Platform Optimization - Making BitCraps Fast on Phones

## Understanding Mobile Optimization Through Real Code
*"Optimizing for mobile isn't just about making things faster - it's about making them work in someone's pocket."*

---

## Part I: Why Mobile Is Different

Imagine trying to play a demanding PC game on a calculator. That's what running distributed systems on mobile devices can feel like. Mobile devices have:

- **Limited battery** - Every computation costs precious energy
- **Variable CPU** - Performance changes based on temperature and battery
- **Limited memory** - Apps get killed if they use too much RAM  
- **Unreliable networks** - WiFi, cellular, Bluetooth all come and go
- **Background restrictions** - OS aggressively suspends apps to save battery

BitCraps faces all these challenges. Players want to bet real tokens on dice games, but their phones might have 1% battery left or spotty cell service. Let's see how the `src/mobile/` directory tackles these problems.

## Part II: Battery Optimization - The Silent Killer

Battery drain is the #1 reason users uninstall apps. BitCraps uses several strategies to be battery-friendly:

### 1. Adaptive CPU Scaling

```rust
// From src/mobile/cpu_optimizer.rs
pub struct CpuOptimizer {
    current_frequency: AtomicU64,
    thermal_throttling: AtomicBool,
    battery_level: AtomicU8,
}

impl CpuOptimizer {
    pub async fn optimize_for_battery(&self) -> Result<CpuProfile, OptimizationError> {
        let battery_level = self.get_battery_level().await?;
        let thermal_state = self.get_thermal_state().await?;
        
        let profile = match (battery_level, thermal_state) {
            // Critical battery - minimal processing
            (0..=10, _) => CpuProfile::PowerSaver {
                max_frequency: 800_000_000, // 800MHz
                background_processing: false,
                consensus_participation: false, // Just receive updates
            },
            
            // Low battery - reduce consensus work
            (11..=25, ThermalState::Normal) => CpuProfile::Balanced {
                max_frequency: 1_200_000_000, // 1.2GHz
                background_processing: true,
                consensus_participation: true,
                mining_disabled: true, // Don't mine new blocks
            },
            
            // Good battery, hot device - throttle CPU
            (26..=100, ThermalState::Hot) => CpuProfile::ThermalThrottle {
                max_frequency: 1_000_000_000, // 1GHz
                background_processing: false,
                consensus_participation: true,
            },
            
            // Good battery, cool device - full performance
            (26..=100, ThermalState::Normal) => CpuProfile::Performance {
                max_frequency: 2_400_000_000, // 2.4GHz
                background_processing: true,
                consensus_participation: true,
                mining_enabled: true,
            },
        };
        
        self.apply_profile(profile.clone()).await?;
        Ok(profile)
    }
    
    // Monitor battery drain and adjust automatically
    pub async fn start_adaptive_monitoring(&self) -> Result<(), OptimizationError> {
        let battery_monitor = BatteryMonitor::new().await?;
        let thermal_monitor = ThermalMonitor::new().await?;
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            
            loop {
                interval.tick().await;
                
                // Check if we're draining battery too fast
                let drain_rate = battery_monitor.get_drain_rate().await;
                if drain_rate > 2.0 { // More than 2% per minute
                    self.reduce_performance().await;
                }
                
                // Check thermal throttling
                if thermal_monitor.is_overheating().await {
                    self.enable_thermal_throttling().await;
                }
            }
        });
        
        Ok(())
    }
}
```

### 2. Smart Background Processing

Mobile OSes aggressively kill background apps. BitCraps needs to continue participating in the mesh network even when backgrounded:

```rust
// From src/mobile/power_manager.rs
pub struct PowerManager {
    wake_locks: HashMap<String, WakeLock>,
    background_tasks: Vec<BackgroundTask>,
    battery_optimization_detected: AtomicBool,
}

impl PowerManager {
    pub async fn optimize_for_background(&self) -> Result<(), PowerError> {
        // Detect if user has battery optimization enabled for our app
        if self.is_battery_optimized().await? {
            return self.request_battery_optimization_whitelist().await;
        }
        
        // Request minimal wake locks for critical operations
        let consensus_lock = WakeLock::new(WakeLockType::PartialWakeLock)
            .duration(Duration::from_secs(30)) // Short bursts only
            .reason("Consensus participation");
        
        self.acquire_wake_lock("consensus", consensus_lock).await?;
        
        // Schedule essential work in small bursts
        self.schedule_burst_work().await?;
        
        Ok(())
    }
    
    async fn schedule_burst_work(&self) -> Result<(), PowerError> {
        // Instead of continuous processing, work in 30-second bursts every 5 minutes
        let mut interval = tokio::time::interval(Duration::from_secs(300)); // 5 minutes
        
        tokio::spawn(async move {
            loop {
                interval.tick().await;
                
                // Wake up, do critical work, then sleep
                let work_session = WorkSession::new(Duration::from_secs(30));
                
                // Process pending consensus messages
                work_session.process_consensus_queue().await?;
                
                // Sync game state with peers
                work_session.sync_game_state().await?;
                
                // Update local database
                work_session.flush_pending_writes().await?;
                
                // Go back to sleep
                work_session.finish().await;
            }
        });
        
        Ok(())
    }
}
```

## Part III: Memory Optimization - Staying Under the Radar

Mobile apps get killed if they use too much memory. BitCraps implements aggressive memory management:

### 1. Tiered Memory Management

```rust
// From src/mobile/memory_manager.rs
pub struct MemoryManager {
    game_states: LruCache<GameId, GameState>,
    peer_connections: LruCache<PeerId, Connection>,
    message_cache: LruCache<MessageId, Message>,
    memory_pressure_receiver: Receiver<MemoryPressure>,
}

impl MemoryManager {
    pub async fn handle_memory_pressure(&mut self, pressure: MemoryPressure) {
        match pressure {
            MemoryPressure::Low => {
                // Trim caches by 25%
                self.game_states.resize(self.game_states.cap() * 3 / 4);
                self.message_cache.resize(self.message_cache.cap() * 3 / 4);
            }
            
            MemoryPressure::Medium => {
                // Trim caches by 50% 
                self.game_states.resize(self.game_states.cap() / 2);
                self.message_cache.resize(self.message_cache.cap() / 2);
                
                // Disconnect idle peer connections
                self.disconnect_idle_peers(Duration::from_secs(60)).await;
            }
            
            MemoryPressure::Critical => {
                // Emergency: Keep only current game and active connections
                self.keep_only_active_game().await;
                self.keep_only_active_connections().await;
                
                // Force garbage collection
                self.force_gc().await;
            }
        }
    }
    
    pub async fn monitor_memory_usage(&self) -> Result<(), MemoryError> {
        let mut interval = tokio::time::interval(Duration::from_secs(10));
        
        tokio::spawn(async move {
            loop {
                interval.tick().await;
                
                let memory_info = self.get_memory_info().await?;
                let pressure = self.calculate_pressure(memory_info);
                
                if pressure != MemoryPressure::None {
                    self.handle_memory_pressure(pressure).await;
                }
            }
        });
        
        Ok(())
    }
}
```

### 2. Lazy Loading Game States

Instead of keeping all game history in memory, BitCraps loads game states on-demand:

```rust
// From src/mobile/performance.rs
pub struct LazyGameStateLoader {
    local_cache: LruCache<GameId, GameState>,
    database: Arc<Database>,
    peer_network: Arc<MeshNetwork>,
}

impl LazyGameStateLoader {
    pub async fn get_game_state(&self, game_id: GameId) -> Result<GameState, LoadError> {
        // Try memory cache first (fastest)
        if let Some(state) = self.local_cache.get(&game_id) {
            return Ok(state.clone());
        }
        
        // Try local database (fast)
        if let Ok(state) = self.database.get_game_state(game_id).await {
            self.local_cache.put(game_id, state.clone());
            return Ok(state);
        }
        
        // Request from peers (slow, but necessary)
        let state = self.peer_network.request_game_state(game_id).await?;
        
        // Cache for future use
        self.local_cache.put(game_id, state.clone());
        self.database.store_game_state(game_id, &state).await?;
        
        Ok(state)
    }
    
    // Preload states we think the user will need
    pub async fn preload_likely_states(&self) -> Result<(), LoadError> {
        // Load states for games user has bets in
        let user_games = self.database.get_user_active_games().await?;
        
        for game_id in user_games {
            tokio::spawn({
                let loader = self.clone();
                async move {
                    // Load in background, ignore errors (it's just optimization)
                    let _ = loader.get_game_state(game_id).await;
                }
            });
        }
        
        Ok(())
    }
}
```

## Part IV: Network Optimization - Dealing with Bad Connections

Mobile networks are unreliable. WiFi cuts out, cellular has dead zones, Bluetooth is flaky. BitCraps adapts to these conditions:

### 1. Adaptive Protocol Selection

```rust
// From src/mobile/network_optimizer.rs
pub struct NetworkOptimizer {
    bluetooth_transport: BluetoothTransport,
    wifi_transport: WifiTransport,
    cellular_transport: CellularTransport,
    current_conditions: NetworkConditions,
}

impl NetworkOptimizer {
    pub async fn select_optimal_transport(&self) -> Result<TransportType, NetworkError> {
        let conditions = self.assess_network_conditions().await?;
        
        let transport = match conditions {
            NetworkConditions {
                bluetooth_available: true,
                wifi_available: false,
                cellular_available: false,
                ..
            } => {
                // Only Bluetooth - use mesh networking
                TransportType::BluetoothMesh {
                    max_hops: 3,
                    message_ttl: Duration::from_secs(30),
                }
            }
            
            NetworkConditions {
                wifi_available: true,
                wifi_strength: strength,
                ..
            } if strength > 0.7 => {
                // Strong WiFi - use direct internet connection
                TransportType::DirectInternet {
                    use_compression: false, // High bandwidth
                    batch_messages: false,
                }
            }
            
            NetworkConditions {
                cellular_available: true,
                cellular_strength: strength,
                data_limit_approaching: false,
                ..
            } if strength > 0.5 => {
                // Good cellular - use with optimization
                TransportType::CellularOptimized {
                    use_compression: true,  // Save data
                    batch_messages: true,   // Reduce radio wakeups
                    message_priority: MessagePriority::GameCritical,
                }
            }
            
            _ => {
                // Poor conditions - use hybrid approach
                TransportType::Hybrid {
                    primary: Box::new(TransportType::CellularOptimized {
                        use_compression: true,
                        batch_messages: true,
                        message_priority: MessagePriority::Essential,
                    }),
                    fallback: Box::new(TransportType::BluetoothMesh {
                        max_hops: 5, // More hops for poor conditions
                        message_ttl: Duration::from_secs(60),
                    }),
                }
            }
        };
        
        Ok(transport)
    }
    
    pub async fn adapt_to_conditions(&self) -> Result<(), NetworkError> {
        let mut interval = tokio::time::interval(Duration::from_secs(15));
        
        tokio::spawn(async move {
            loop {
                interval.tick().await;
                
                let new_transport = self.select_optimal_transport().await?;
                
                if new_transport != self.current_transport {
                    // Gracefully switch transports
                    self.switch_transport(new_transport).await?;
                }
            }
        });
        
        Ok(())
    }
}
```

### 2. Message Compression and Batching

```rust
// From src/mobile/compression.rs
pub struct MessageOptimizer {
    compression_stats: CompressionStats,
    batching_queue: VecDeque<PendingMessage>,
}

impl MessageOptimizer {
    pub async fn optimize_message(&self, msg: Message) -> Result<OptimizedMessage, OptimizationError> {
        let msg_size = msg.serialized_size();
        
        // Small messages: don't compress (overhead > savings)
        if msg_size < 100 {
            return Ok(OptimizedMessage::Raw(msg));
        }
        
        // Try different compression algorithms
        let compressed_lz4 = lz4::compress(&msg.serialize()?)?;
        let compressed_zstd = zstd::compress(&msg.serialize()?)?;
        
        // Pick the best compression ratio
        let best_compressed = if compressed_lz4.len() < compressed_zstd.len() {
            (compressed_lz4, CompressionType::Lz4)
        } else {
            (compressed_zstd, CompressionType::Zstd)
        };
        
        // Only use compression if it actually saves space
        if best_compressed.0.len() < msg_size {
            Ok(OptimizedMessage::Compressed {
                data: best_compressed.0,
                compression_type: best_compressed.1,
                original_size: msg_size,
            })
        } else {
            Ok(OptimizedMessage::Raw(msg))
        }
    }
    
    pub async fn batch_messages(&mut self, max_wait: Duration) -> Result<Vec<Message>, BatchError> {
        // Collect messages for up to max_wait time
        let start = Instant::now();
        let mut batch = Vec::new();
        
        while start.elapsed() < max_wait && batch.len() < 50 {
            if let Some(msg) = self.batching_queue.pop_front() {
                batch.push(msg.message);
                
                // If we have a high-priority message, send immediately
                if msg.priority == MessagePriority::Urgent {
                    break;
                }
            } else {
                // No messages pending, wait a bit
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }
        
        Ok(batch)
    }
}
```

## Part V: Platform-Specific Optimizations

### Android Optimizations

```rust
// From src/mobile/android/mod.rs
pub struct AndroidOptimizations {
    doze_mode_detector: DozeModeDetector,
    battery_optimization_detector: BatteryOptimizationDetector,
    background_task_scheduler: WorkManager,
}

impl AndroidOptimizations {
    pub async fn handle_doze_mode(&self) -> Result<(), AndroidError> {
        // Android Doze mode severely limits network access
        if self.doze_mode_detector.is_in_doze().await? {
            // Schedule work for next maintenance window
            let work_request = OneTimeWorkRequest::builder()
                .set_constraints(
                    Constraints::builder()
                        .set_required_network_type(NetworkType::CONNECTED)
                        .set_requires_battery_not_low(true)
                        .build()
                )
                .build();
            
            self.background_task_scheduler.enqueue(work_request).await?;
        }
        
        Ok(())
    }
    
    pub async fn optimize_for_android_specific(&self) -> Result<(), AndroidError> {
        // Use Android-specific optimizations
        
        // 1. Use JobScheduler for background work
        self.schedule_periodic_sync().await?;
        
        // 2. Request battery optimization whitelist
        if self.battery_optimization_detector.is_optimized().await? {
            self.request_battery_whitelist().await?;
        }
        
        // 3. Use foreground service for critical operations
        self.start_foreground_service().await?;
        
        Ok(())
    }
}
```

### iOS Optimizations

```rust
// From src/mobile/ios/mod.rs  
pub struct IosOptimizations {
    background_task_manager: BackgroundTaskManager,
    memory_warning_observer: MemoryWarningObserver,
}

impl IosOptimizations {
    pub async fn handle_ios_background(&self) -> Result<(), IosError> {
        // iOS gives very limited background time
        let background_task = self.background_task_manager
            .begin_background_task(Duration::from_secs(30))
            .await?;
            
        // Do essential work quickly
        self.sync_critical_game_state().await?;
        self.flush_pending_database_writes().await?;
        
        // Clean up before time expires
        background_task.end().await;
        
        Ok(())
    }
    
    pub async fn optimize_for_ios_specific(&self) -> Result<(), IosError> {
        // iOS-specific optimizations
        
        // 1. Use background app refresh efficiently
        if BackgroundAppRefresh::is_available().await {
            self.schedule_background_refresh().await?;
        }
        
        // 2. Handle memory warnings aggressively
        self.memory_warning_observer.on_memory_warning({
            let cache_manager = self.cache_manager.clone();
            move || {
                cache_manager.emergency_clear();
            }
        }).await;
        
        // 3. Use iOS push notifications for game updates
        self.setup_push_notifications().await?;
        
        Ok(())
    }
}
```

## Part VI: Performance Monitoring and Adaptation

### Real-Time Performance Monitoring

```rust
// From src/mobile/performance.rs
pub struct PerformanceMonitor {
    cpu_usage: MovingAverage<f64>,
    memory_usage: MovingAverage<usize>,
    battery_drain_rate: MovingAverage<f64>,
    network_quality: MovingAverage<f64>,
}

impl PerformanceMonitor {
    pub async fn start_monitoring(&self) -> Result<(), MonitoringError> {
        // Monitor every 30 seconds
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        
        tokio::spawn(async move {
            loop {
                interval.tick().await;
                
                let metrics = self.collect_metrics().await?;
                
                // Update moving averages
                self.cpu_usage.add_sample(metrics.cpu_usage);
                self.memory_usage.add_sample(metrics.memory_usage);
                self.battery_drain_rate.add_sample(metrics.battery_drain);
                self.network_quality.add_sample(metrics.network_quality);
                
                // Adapt performance based on metrics
                self.adapt_performance(metrics).await?;
            }
        });
        
        Ok(())
    }
    
    async fn adapt_performance(&self, metrics: PerformanceMetrics) -> Result<(), MonitoringError> {
        // If performance is degrading, reduce load
        if metrics.cpu_usage > 0.8 {
            // Reduce consensus participation frequency
            self.reduce_consensus_frequency().await?;
        }
        
        if metrics.memory_usage > self.get_memory_limit() * 0.8 {
            // Clear caches more aggressively  
            self.increase_cache_pressure().await?;
        }
        
        if metrics.battery_drain > 2.0 { // 2% per minute
            // Enable power saving mode
            self.enable_power_saving().await?;
        }
        
        if metrics.network_quality < 0.3 {
            // Switch to offline-first mode
            self.enable_offline_mode().await?;
        }
        
        Ok(())
    }
}
```

## Part VII: Practical Mobile Optimization Exercise

Let's build a mobile-optimized feature:

**Exercise: Smart Game State Caching**

```rust
pub struct SmartGameStateCache {
    hot_cache: LruCache<GameId, GameState>,    // Fast access
    warm_cache: LruCache<GameId, CompressedGameState>, // Compressed
    cold_storage: Database,                     // Persistent
    access_predictor: AccessPredictor,
}

impl SmartGameStateCache {
    pub async fn get_game_state(&self, game_id: GameId) -> Result<GameState, CacheError> {
        // Try hot cache first (uncompressed, instant access)
        if let Some(state) = self.hot_cache.get(&game_id) {
            return Ok(state.clone());
        }
        
        // Try warm cache (compressed, fast decompression)
        if let Some(compressed) = self.warm_cache.get(&game_id) {
            let state = compressed.decompress()?;
            
            // Promote to hot cache if frequently accessed
            if self.access_predictor.is_hot(&game_id) {
                self.hot_cache.put(game_id, state.clone());
            }
            
            return Ok(state);
        }
        
        // Load from cold storage (database)
        let state = self.cold_storage.get_game_state(game_id).await?;
        
        // Decide which cache tier to put it in
        match self.access_predictor.predict_access_pattern(&game_id) {
            AccessPattern::VeryHot => {
                self.hot_cache.put(game_id, state.clone());
            }
            AccessPattern::Warm => {
                let compressed = CompressedGameState::compress(&state)?;
                self.warm_cache.put(game_id, compressed);
            }
            AccessPattern::Cold => {
                // Keep only in database
            }
        }
        
        Ok(state)
    }
    
    pub async fn handle_memory_pressure(&mut self, pressure: MemoryPressure) {
        match pressure {
            MemoryPressure::Low => {
                // Move some hot cache items to warm cache
                while self.hot_cache.len() > self.hot_cache.cap() * 3 / 4 {
                    if let Some((game_id, state)) = self.hot_cache.pop_lru() {
                        let compressed = CompressedGameState::compress(&state).unwrap();
                        self.warm_cache.put(game_id, compressed);
                    }
                }
            }
            
            MemoryPressure::Medium => {
                // Clear hot cache completely, halve warm cache
                self.hot_cache.clear();
                self.warm_cache.resize(self.warm_cache.cap() / 2);
            }
            
            MemoryPressure::Critical => {
                // Clear everything - rely on database only
                self.hot_cache.clear();
                self.warm_cache.clear();
            }
        }
    }
}

struct AccessPredictor {
    access_history: HashMap<GameId, VecDeque<Instant>>,
    user_games: HashSet<GameId>,
    friend_games: HashSet<GameId>,
}

impl AccessPredictor {
    fn predict_access_pattern(&self, game_id: &GameId) -> AccessPattern {
        // Games user is actively playing - very hot
        if self.user_games.contains(game_id) {
            return AccessPattern::VeryHot;
        }
        
        // Games with friends - warm  
        if self.friend_games.contains(game_id) {
            return AccessPattern::Warm;
        }
        
        // Check recent access history
        if let Some(history) = self.access_history.get(game_id) {
            let recent_accesses = history.iter()
                .filter(|&&time| time.elapsed() < Duration::from_secs(300)) // 5 minutes
                .count();
            
            if recent_accesses > 3 {
                return AccessPattern::VeryHot;
            } else if recent_accesses > 0 {
                return AccessPattern::Warm;
            }
        }
        
        AccessPattern::Cold
    }
}
```

## Part VIII: Testing Mobile Optimizations

Mobile optimizations need special testing:

```rust
#[tokio::test]
async fn test_battery_optimization() {
    let optimizer = CpuOptimizer::new();
    
    // Simulate low battery
    optimizer.set_battery_level(15).await;
    
    let profile = optimizer.optimize_for_battery().await.unwrap();
    
    match profile {
        CpuProfile::PowerSaver { consensus_participation, .. } => {
            assert!(!consensus_participation); // Should disable consensus
        }
        _ => panic!("Should use power saver profile"),
    }
}

#[tokio::test]
async fn test_memory_pressure_response() {
    let mut cache = SmartGameStateCache::new();
    
    // Fill cache with test data
    for i in 0..1000 {
        let game_id = GameId::new(i);
        let state = GameState::new_test();
        cache.hot_cache.put(game_id, state);
    }
    
    assert_eq!(cache.hot_cache.len(), 1000);
    
    // Simulate memory pressure
    cache.handle_memory_pressure(MemoryPressure::Critical).await;
    
    // Should clear all caches
    assert_eq!(cache.hot_cache.len(), 0);
    assert_eq!(cache.warm_cache.len(), 0);
}
```

## Conclusion: Mobile Optimization as User Experience

Mobile optimization isn't just about performance - it's about user experience. When someone is betting real tokens on a dice game, they need:

- **Reliability**: The app works even on poor networks
- **Responsiveness**: Actions feel instant despite complex consensus
- **Battery Life**: They can play for hours without draining battery
- **Data Efficiency**: Works within cellular data limits

The key insights for mobile optimization:

1. **Adapt continuously** - Network, battery, and thermal conditions change constantly
2. **Use platform features** - Each platform has specific optimizations available
3. **Cache intelligently** - Predict what users need and preload it
4. **Fail gracefully** - When resources are constrained, degrade features, don't crash
5. **Monitor and respond** - Track real metrics and adapt behavior automatically

Remember: A distributed system that works great on desktop but fails on mobile is useless in today's world. Mobile optimization isn't optional - it's the difference between an app users love and one they delete.