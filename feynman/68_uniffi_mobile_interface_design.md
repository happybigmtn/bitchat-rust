# Chapter 68: UniFFI Mobile Interface Design

## Introduction: Bridging Rust to Mobile Worlds

Imagine you've built an incredible machine that speaks only ancient Sanskrit, and now you need it to communicate with people who speak English, Spanish, and Mandarin. You need not just translators, but a complete communication framework that preserves the meaning, handles cultural differences, and works reliably across all languages. This is the challenge of exposing Rust code to mobile platforms.

UniFFI (Uniform FFI) is Mozilla's solution to this challenge, providing a unified way to expose Rust libraries to multiple languages including Kotlin (Android) and Swift (iOS). This chapter explores how BitCraps uses UniFFI to create a seamless mobile experience while maintaining Rust's safety guarantees.

## The Fundamentals: Understanding Foreign Function Interfaces

### What is FFI?

Foreign Function Interface (FFI) is the mechanism by which code written in one programming language can call code written in another language. It's like building a universal adapter for programming languages.

```rust
// Traditional C-style FFI (complex and error-prone)
#[no_mangle]
pub extern "C" fn add_numbers(a: i32, b: i32) -> i32 {
    a + b
}

// UniFFI approach (declarative and safe)
#[uniffi::export]
fn add_numbers(a: i32, b: i32) -> i32 {
    a + b
}
```

### The UniFFI Definition Language (UDL)

UniFFI uses an Interface Definition Language to describe the API surface:

```idl
// bitcraps.udl
namespace bitcraps {
    // Initialize the BitCraps system
    [Throws=BitcrapsError]
    void initialize(string config_path);
    
    // Get the current player balance
    [Throws=BitcrapsError]
    u64 get_balance(string player_id);
};

// Custom types exposed to mobile
dictionary PlayerInfo {
    string id;
    string display_name;
    u64 balance;
    sequence<string> active_games;
};

// Rust enums become mobile enums
enum GamePhase {
    "ComeOut",
    "Point",
    "Ended",
};

// Error types for proper error handling
[Error]
enum BitcrapsError {
    "Network",
    "InvalidInput",
    "InsufficientBalance",
    "GameNotFound",
};
```

## Deep Dive: Mobile Bridge Architecture

### The BitCraps Mobile Interface

```rust
// src/mobile/ffi.rs

use uniffi::Object;

/// Main interface for mobile platforms
#[derive(Object)]
pub struct BitcrapsMobile {
    runtime: Arc<Runtime>,
    state: Arc<RwLock<MobileState>>,
    event_queue: Arc<Mutex<VecDeque<MobileEvent>>>,
}

#[uniffi::export]
impl BitcrapsMobile {
    /// Create a new mobile interface instance
    #[uniffi::constructor]
    pub fn new(config: MobileConfig) -> Result<Arc<Self>, BitcrapsError> {
        // Initialize tokio runtime for async operations
        let runtime = Runtime::new()
            .map_err(|e| BitcrapsError::Initialization(e.to_string()))?;
        
        // Create mobile-optimized state manager
        let state = Arc::new(RwLock::new(MobileState::new(config)));
        
        // Event queue for mobile UI updates
        let event_queue = Arc::new(Mutex::new(VecDeque::new()));
        
        Ok(Arc::new(Self {
            runtime,
            state,
            event_queue,
        }))
    }
    
    /// Connect to the BitCraps network
    pub fn connect(&self) -> Result<(), BitcrapsError> {
        self.runtime.block_on(async {
            let mut state = self.state.write().await;
            state.connect().await
        })
    }
    
    /// Place a bet (mobile-friendly synchronous wrapper)
    pub fn place_bet(
        &self,
        game_id: String,
        bet_type: String,
        amount: u64,
    ) -> Result<BetReceipt, BitcrapsError> {
        // Convert string to typed bet
        let bet_type = BetType::from_str(&bet_type)?;
        
        // Execute async operation synchronously for mobile
        self.runtime.block_on(async {
            let mut state = self.state.write().await;
            state.place_bet(game_id, bet_type, amount).await
        })
    }
    
    /// Poll for events (mobile UI pattern)
    pub fn poll_events(&self) -> Vec<MobileEvent> {
        let mut events = self.event_queue.lock().unwrap();
        events.drain(..).collect()
    }
}
```

### Handling Async in Mobile Environments

Mobile platforms have different threading models than Rust. We need to bridge async Rust to mobile-friendly patterns:

```rust
/// Mobile-friendly async executor
pub struct MobileExecutor {
    runtime: Arc<Runtime>,
    active_tasks: Arc<RwLock<HashMap<TaskId, JoinHandle<()>>>>,
}

impl MobileExecutor {
    /// Execute async operation with callback
    pub fn execute_with_callback<F, T>(
        &self,
        future: F,
        callback: Box<dyn MobileCallback<T>>,
    ) -> TaskId
    where
        F: Future<Output = Result<T, BitcrapsError>> + Send + 'static,
        T: Send + 'static,
    {
        let task_id = TaskId::new();
        let handle = self.runtime.spawn(async move {
            match future.await {
                Ok(result) => callback.on_success(result),
                Err(error) => callback.on_error(error),
            }
        });
        
        self.active_tasks.write().unwrap().insert(task_id, handle);
        task_id
    }
    
    /// Cancel a running task
    pub fn cancel_task(&self, task_id: TaskId) -> bool {
        if let Some(handle) = self.active_tasks.write().unwrap().remove(&task_id) {
            handle.abort();
            true
        } else {
            false
        }
    }
}

/// Callback interface for mobile platforms
pub trait MobileCallback<T>: Send {
    fn on_success(&self, result: T);
    fn on_error(&self, error: BitcrapsError);
}
```

### Memory Management Across Language Boundaries

```rust
/// Reference-counted wrapper for safe memory sharing
#[derive(Clone)]
pub struct SharedGameState {
    inner: Arc<RwLock<GameState>>,
}

#[uniffi::export]
impl SharedGameState {
    /// Get current phase (safe read)
    pub fn get_phase(&self) -> GamePhase {
        self.inner.read().unwrap().phase
    }
    
    /// Get player list (returns owned copy for safety)
    pub fn get_players(&self) -> Vec<PlayerInfo> {
        self.inner
            .read()
            .unwrap()
            .players
            .iter()
            .map(|p| PlayerInfo::from(p))
            .collect()
    }
    
    /// Update state (synchronized write)
    pub fn update(&self, update: GameStateUpdate) -> Result<(), BitcrapsError> {
        let mut state = self.inner.write().unwrap();
        state.apply_update(update)
    }
}

/// Ensure proper cleanup when objects cross boundaries
impl Drop for SharedGameState {
    fn drop(&mut self) {
        // Log cleanup for debugging
        tracing::debug!("SharedGameState dropped, refs: {}", Arc::strong_count(&self.inner));
    }
}
```

## Mobile Platform Specifics

### Android Integration via Kotlin

```kotlin
// Generated Kotlin code from UniFFI
class BitcrapsMobile {
    companion object {
        init {
            // Load the Rust library
            System.loadLibrary("bitcraps")
        }
    }
    
    private val handle: Long
    
    constructor(config: MobileConfig) {
        handle = rustConstructor(config)
    }
    
    fun connect() {
        rustConnect(handle)
    }
    
    fun placeBet(gameId: String, betType: String, amount: ULong): BetReceipt {
        return rustPlaceBet(handle, gameId, betType, amount)
    }
    
    fun pollEvents(): List<MobileEvent> {
        return rustPollEvents(handle)
    }
    
    // Native method declarations
    private external fun rustConstructor(config: MobileConfig): Long
    private external fun rustConnect(handle: Long)
    private external fun rustPlaceBet(
        handle: Long,
        gameId: String,
        betType: String,
        amount: ULong
    ): BetReceipt
    private external fun rustPollEvents(handle: Long): List<MobileEvent>
}
```

### iOS Integration via Swift

```swift
// Generated Swift code from UniFFI
public class BitcrapsMobile {
    private let handle: UnsafeMutableRawPointer
    
    public init(config: MobileConfig) throws {
        self.handle = try bitcraps_mobile_new(config)
    }
    
    deinit {
        bitcraps_mobile_free(handle)
    }
    
    public func connect() throws {
        try bitcraps_mobile_connect(handle)
    }
    
    public func placeBet(
        gameId: String,
        betType: String,
        amount: UInt64
    ) throws -> BetReceipt {
        return try bitcraps_mobile_place_bet(handle, gameId, betType, amount)
    }
    
    public func pollEvents() -> [MobileEvent] {
        return bitcraps_mobile_poll_events(handle)
    }
}
```

## Event Handling and UI Updates

### Event Polling Pattern for Mobile UIs

```rust
/// Events pushed to mobile UI
#[derive(Debug, Clone)]
pub enum MobileEvent {
    /// Connection state changed
    ConnectionStateChanged {
        connected: bool,
        peer_count: u32,
    },
    
    /// Game state updated
    GameStateUpdated {
        game_id: String,
        phase: GamePhase,
        point: Option<u8>,
    },
    
    /// Bet resolved
    BetResolved {
        bet_id: String,
        won: bool,
        payout: Option<u64>,
    },
    
    /// Balance changed
    BalanceChanged {
        new_balance: u64,
        change: i64,
    },
    
    /// Error occurred
    Error {
        code: ErrorCode,
        message: String,
        recoverable: bool,
    },
}

/// Event dispatcher for mobile platforms
pub struct EventDispatcher {
    subscribers: Arc<RwLock<HashMap<EventType, Vec<EventHandler>>>>,
    mobile_queue: Arc<Mutex<VecDeque<MobileEvent>>>,
    max_queue_size: usize,
}

impl EventDispatcher {
    /// Dispatch event to mobile UI
    pub fn dispatch(&self, event: MobileEvent) {
        let mut queue = self.mobile_queue.lock().unwrap();
        
        // Prevent queue overflow
        if queue.len() >= self.max_queue_size {
            queue.pop_front();
            tracing::warn!("Mobile event queue overflow, dropping oldest event");
        }
        
        queue.push_back(event.clone());
        
        // Also notify any registered handlers
        if let Some(handlers) = self.get_handlers(&event) {
            for handler in handlers {
                handler.handle(event.clone());
            }
        }
    }
    
    /// Register event handler
    pub fn register_handler(&self, event_type: EventType, handler: EventHandler) {
        let mut subscribers = self.subscribers.write().unwrap();
        subscribers.entry(event_type).or_insert_with(Vec::new).push(handler);
    }
}
```

### Reactive State Management

```rust
/// Observable state for mobile UI binding
#[derive(Clone)]
pub struct ObservableState<T: Clone + Send + Sync> {
    value: Arc<RwLock<T>>,
    observers: Arc<RwLock<Vec<StateObserver<T>>>>,
}

impl<T: Clone + Send + Sync + 'static> ObservableState<T> {
    pub fn new(initial: T) -> Self {
        Self {
            value: Arc::new(RwLock::new(initial)),
            observers: Arc::new(RwLock::new(Vec::new())),
        }
    }
    
    /// Update state and notify observers
    pub fn update<F>(&self, updater: F)
    where
        F: FnOnce(&mut T),
    {
        let mut value = self.value.write().unwrap();
        let old_value = value.clone();
        updater(&mut *value);
        let new_value = value.clone();
        drop(value);
        
        // Notify all observers
        let observers = self.observers.read().unwrap();
        for observer in observers.iter() {
            observer.on_change(&old_value, &new_value);
        }
    }
    
    /// Subscribe to state changes
    pub fn observe(&self, observer: StateObserver<T>) {
        self.observers.write().unwrap().push(observer);
    }
    
    /// Get current value
    pub fn get(&self) -> T {
        self.value.read().unwrap().clone()
    }
}

/// Observer for state changes
pub trait StateObserver<T>: Send + Sync {
    fn on_change(&self, old: &T, new: &T);
}
```

## Performance Optimization for Mobile

### Minimizing FFI Overhead

```rust
/// Batch multiple operations to reduce FFI calls
pub struct BatchedOperations {
    operations: Vec<Operation>,
    max_batch_size: usize,
}

#[uniffi::export]
impl BatchedOperations {
    pub fn new(max_size: u32) -> Self {
        Self {
            operations: Vec::new(),
            max_batch_size: max_size as usize,
        }
    }
    
    /// Add operation to batch
    pub fn add_operation(&mut self, op: Operation) {
        self.operations.push(op);
    }
    
    /// Execute all operations at once
    pub fn execute_batch(&mut self) -> BatchResult {
        let operations = std::mem::take(&mut self.operations);
        
        // Process all operations in single FFI call
        let results: Vec<OperationResult> = operations
            .into_iter()
            .map(|op| self.execute_single(op))
            .collect();
        
        BatchResult {
            successful: results.iter().filter(|r| r.is_ok()).count() as u32,
            failed: results.iter().filter(|r| r.is_err()).count() as u32,
            results,
        }
    }
}
```

### Lazy Loading and Caching

```rust
/// Cache frequently accessed data to minimize FFI calls
pub struct MobileCache {
    player_info: Arc<RwLock<Option<CachedData<PlayerInfo>>>>,
    game_states: Arc<RwLock<HashMap<String, CachedData<GameState>>>>,
    cache_ttl: Duration,
}

pub struct CachedData<T> {
    data: T,
    cached_at: Instant,
}

impl MobileCache {
    /// Get player info with caching
    pub fn get_player_info(&self, player_id: &str) -> Result<PlayerInfo, BitcrapsError> {
        let cache = self.player_info.read().unwrap();
        
        if let Some(cached) = &*cache {
            if cached.cached_at.elapsed() < self.cache_ttl {
                return Ok(cached.data.clone());
            }
        }
        drop(cache);
        
        // Cache miss or expired, fetch fresh data
        let fresh_data = self.fetch_player_info(player_id)?;
        
        let mut cache = self.player_info.write().unwrap();
        *cache = Some(CachedData {
            data: fresh_data.clone(),
            cached_at: Instant::now(),
        });
        
        Ok(fresh_data)
    }
}
```

### Mobile-Specific Memory Management

```rust
/// Memory-aware buffer management for mobile
pub struct MobileBufferPool {
    pools: HashMap<usize, Vec<Vec<u8>>>,
    max_memory: usize,
    current_usage: Arc<AtomicUsize>,
}

impl MobileBufferPool {
    /// Get buffer of specified size
    pub fn get_buffer(&mut self, size: usize) -> Vec<u8> {
        // Round up to nearest power of 2 for pooling
        let pool_size = size.next_power_of_two();
        
        if let Some(pool) = self.pools.get_mut(&pool_size) {
            if let Some(buffer) = pool.pop() {
                return buffer;
            }
        }
        
        // Check memory limit
        let new_usage = self.current_usage.fetch_add(pool_size, Ordering::Relaxed) + pool_size;
        if new_usage > self.max_memory {
            self.current_usage.fetch_sub(pool_size, Ordering::Relaxed);
            panic!("Mobile memory limit exceeded");
        }
        
        vec![0u8; pool_size]
    }
    
    /// Return buffer to pool
    pub fn return_buffer(&mut self, mut buffer: Vec<u8>) {
        buffer.clear();
        let size = buffer.capacity();
        
        self.pools
            .entry(size)
            .or_insert_with(Vec::new)
            .push(buffer);
    }
}
```

## Error Handling Across Boundaries

### Structured Error Types

```rust
/// Mobile-friendly error representation
#[derive(Debug, Clone)]
pub enum BitcrapsError {
    Network { message: String, retry_after: Option<u32> },
    InvalidInput { field: String, reason: String },
    InsufficientBalance { required: u64, available: u64 },
    GameNotFound { game_id: String },
    PermissionDenied { action: String },
    Internal { code: u32, debug_info: Option<String> },
}

impl BitcrapsError {
    /// Convert to mobile-friendly error code
    pub fn error_code(&self) -> u32 {
        match self {
            Self::Network { .. } => 1000,
            Self::InvalidInput { .. } => 2000,
            Self::InsufficientBalance { .. } => 3000,
            Self::GameNotFound { .. } => 4000,
            Self::PermissionDenied { .. } => 5000,
            Self::Internal { code, .. } => 9000 + code,
        }
    }
    
    /// User-friendly error message
    pub fn user_message(&self) -> String {
        match self {
            Self::Network { message, .. } => {
                format!("Network error: {}", message)
            }
            Self::InvalidInput { field, reason } => {
                format!("Invalid {}: {}", field, reason)
            }
            Self::InsufficientBalance { required, available } => {
                format!("Insufficient balance. Need {}, have {}", required, available)
            }
            Self::GameNotFound { game_id } => {
                format!("Game '{}' not found", game_id)
            }
            Self::PermissionDenied { action } => {
                format!("Permission denied for action: {}", action)
            }
            Self::Internal { .. } => {
                "An internal error occurred. Please try again.".to_string()
            }
        }
    }
}
```

### Error Recovery Strategies

```rust
/// Mobile-specific error recovery
pub struct ErrorRecovery {
    retry_policies: HashMap<u32, RetryPolicy>,
    error_handlers: HashMap<u32, Box<dyn ErrorHandler>>,
}

pub struct RetryPolicy {
    max_attempts: u32,
    backoff: BackoffStrategy,
    retryable: Box<dyn Fn(&BitcrapsError) -> bool>,
}

pub enum BackoffStrategy {
    Fixed { delay_ms: u32 },
    Exponential { base_ms: u32, max_ms: u32 },
    Linear { increment_ms: u32 },
}

impl ErrorRecovery {
    pub async fn handle_with_retry<F, T>(
        &self,
        operation: F,
        error_code: u32,
    ) -> Result<T, BitcrapsError>
    where
        F: Fn() -> Future<Output = Result<T, BitcrapsError>>,
    {
        let policy = self.retry_policies.get(&error_code)
            .ok_or(BitcrapsError::Internal { 
                code: error_code, 
                debug_info: Some("No retry policy".to_string()) 
            })?;
        
        let mut attempt = 0;
        let mut last_error = None;
        
        while attempt < policy.max_attempts {
            match operation().await {
                Ok(result) => return Ok(result),
                Err(error) => {
                    if !(policy.retryable)(&error) {
                        return Err(error);
                    }
                    
                    last_error = Some(error);
                    let delay = policy.backoff.calculate_delay(attempt);
                    tokio::time::sleep(delay).await;
                    attempt += 1;
                }
            }
        }
        
        Err(last_error.unwrap_or(BitcrapsError::Internal {
            code: error_code,
            debug_info: Some("Max retries exceeded".to_string()),
        }))
    }
}
```

## Testing Mobile Interfaces

### Unit Testing FFI Boundaries

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_uniffi_types_serialization() {
        let player = PlayerInfo {
            id: "player1".to_string(),
            display_name: "Alice".to_string(),
            balance: 1000,
            active_games: vec!["game1".to_string()],
        };
        
        // Simulate FFI serialization
        let serialized = uniffi::serialize(&player).unwrap();
        let deserialized: PlayerInfo = uniffi::deserialize(&serialized).unwrap();
        
        assert_eq!(player.id, deserialized.id);
        assert_eq!(player.balance, deserialized.balance);
    }
    
    #[test]
    fn test_event_polling() {
        let mobile = BitcrapsMobile::new(MobileConfig::default()).unwrap();
        
        // Trigger some events
        mobile.internal_dispatch(MobileEvent::ConnectionStateChanged {
            connected: true,
            peer_count: 5,
        });
        
        mobile.internal_dispatch(MobileEvent::BalanceChanged {
            new_balance: 1500,
            change: 500,
        });
        
        // Poll events
        let events = mobile.poll_events();
        assert_eq!(events.len(), 2);
        
        // Verify queue is cleared
        let events2 = mobile.poll_events();
        assert_eq!(events2.len(), 0);
    }
}
```

### Integration Testing with Mobile Simulators

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_full_game_flow() {
        let config = MobileConfig {
            player_id: "test_player".to_string(),
            server_url: "localhost:8080".to_string(),
            cache_size: 1024 * 1024, // 1MB
        };
        
        let mobile = BitcrapsMobile::new(config).unwrap();
        
        // Connect to network
        mobile.connect().unwrap();
        
        // Join game
        let game_id = mobile.create_or_join_game().unwrap();
        
        // Place bet
        let receipt = mobile.place_bet(
            game_id.clone(),
            "Pass".to_string(),
            100,
        ).unwrap();
        
        assert_eq!(receipt.amount, 100);
        
        // Simulate dice roll
        mobile.test_trigger_dice_roll(7);
        
        // Poll for events
        let events = mobile.poll_events();
        
        // Verify bet won event
        assert!(events.iter().any(|e| matches!(e, 
            MobileEvent::BetResolved { won: true, .. }
        )));
    }
}
```

## Platform-Specific Optimizations

### Android-Specific Considerations

```rust
/// Android-specific optimizations
pub struct AndroidOptimizations {
    /// JNI environment handle
    jni_env: *mut JNIEnv,
    
    /// Cached method IDs for performance
    method_cache: HashMap<String, jmethodID>,
    
    /// Android-specific memory limits
    memory_class: u32,
}

impl AndroidOptimizations {
    /// Optimize for Android's memory constraints
    pub fn configure_for_device(&mut self) {
        // Get device memory class
        let memory_class = self.get_memory_class();
        
        // Adjust cache sizes based on available memory
        let cache_size = match memory_class {
            0..=64 => 1024 * 1024,      // 1MB for low-end devices
            65..=128 => 4 * 1024 * 1024, // 4MB for mid-range
            _ => 8 * 1024 * 1024,        // 8MB for high-end
        };
        
        self.configure_cache(cache_size);
    }
    
    /// Use Android's native logging
    pub fn log_to_logcat(&self, level: LogLevel, message: &str) {
        unsafe {
            // Call Android's __android_log_print
            android_log(level as i32, "BitCraps", message);
        }
    }
}
```

### iOS-Specific Considerations

```rust
/// iOS-specific optimizations
pub struct IosOptimizations {
    /// Objective-C runtime integration
    objc_runtime: ObjcRuntime,
    
    /// iOS-specific background task handling
    background_task_id: Option<UIBackgroundTaskIdentifier>,
    
    /// Memory pressure handler
    memory_warning_handler: Option<Box<dyn Fn()>>,
}

impl IosOptimizations {
    /// Handle iOS background mode
    pub fn enter_background(&mut self) {
        // Request background time
        self.background_task_id = Some(self.begin_background_task());
        
        // Reduce resource usage
        self.reduce_activity();
        
        // Persist critical state
        self.persist_state();
    }
    
    /// Handle iOS memory warnings
    pub fn handle_memory_warning(&mut self) {
        // Clear caches
        self.clear_caches();
        
        // Reduce buffer sizes
        self.reduce_buffers();
        
        // Notify app of memory pressure
        if let Some(handler) = &self.memory_warning_handler {
            handler();
        }
    }
}
```

## Conclusion

UniFFI mobile interface design represents the critical bridge between Rust's performance and safety guarantees and the mobile platforms that billions of users interact with daily. Through BitCraps' implementation, we've seen how to build robust, efficient, and maintainable mobile interfaces that preserve Rust's advantages while providing native experiences on iOS and Android.

Key insights from this chapter:

1. **UniFFI** provides a declarative, safe way to expose Rust to mobile platforms
2. **Event polling** patterns work well for mobile UI updates
3. **Caching and batching** minimize expensive FFI calls
4. **Platform-specific optimizations** ensure optimal performance
5. **Proper error handling** provides good user experience across language boundaries

Remember: The best mobile interface is invisible to users—they should feel like they're using a native app, not a foreign library wrapped in bindings. Success is measured not in how much Rust code you expose, but in how naturally it integrates with each platform's idioms and expectations.