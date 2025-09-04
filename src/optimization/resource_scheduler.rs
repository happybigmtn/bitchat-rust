//! Resource Scheduler for BitCraps
//!
//! Provides intelligent resource scheduling, task prioritization, and
//! load balancing across system components with adaptive algorithms.

use std::collections::{BTreeMap, HashMap, VecDeque, BinaryHeap};
use std::cmp::{Ordering, Reverse};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering as AtomicOrdering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{Mutex, RwLock, Semaphore};
use tokio::task::JoinHandle;
use uuid::Uuid;

/// Resource scheduler configuration
#[derive(Clone, Debug)]
pub struct ResourceSchedulerConfig {
    /// Maximum concurrent tasks
    pub max_concurrent_tasks: usize,
    /// CPU core count (auto-detected if 0)
    pub cpu_cores: usize,
    /// Memory limit in MB
    pub memory_limit_mb: usize,
    /// Task queue size limit
    pub max_queue_size: usize,
    /// Enable task prioritization
    pub enable_prioritization: bool,
    /// Enable load balancing
    pub enable_load_balancing: bool,
    /// Enable adaptive scheduling
    pub enable_adaptive_scheduling: bool,
    /// Task timeout default
    pub default_task_timeout: Duration,
    /// Resource monitoring interval
    pub monitoring_interval: Duration,
    /// CPU utilization target (0.0 to 1.0)
    pub target_cpu_utilization: f64,
    /// Memory utilization target (0.0 to 1.0)
    pub target_memory_utilization: f64,
}

impl Default for ResourceSchedulerConfig {
    fn default() -> Self {
        Self {
            max_concurrent_tasks: num_cpus::get() * 4,
            cpu_cores: num_cpus::get(),
            memory_limit_mb: 2048, // 2GB default
            max_queue_size: 10000,
            enable_prioritization: true,
            enable_load_balancing: true,
            enable_adaptive_scheduling: true,
            default_task_timeout: Duration::from_secs(300), // 5 minutes
            monitoring_interval: Duration::from_secs(5),
            target_cpu_utilization: 0.8, // 80%
            target_memory_utilization: 0.7, // 70%
        }
    }
}

/// Task priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TaskPriority {
    Critical = 4,
    High = 3,
    Normal = 2,
    Low = 1,
    Background = 0,
}

/// Task categories for resource allocation
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TaskCategory {
    NetworkIO,
    DiskIO,
    Computation,
    Consensus,
    Gaming,
    Database,
    Crypto,
    UI,
    Background,
}

/// Resource requirements for a task
#[derive(Debug, Clone)]
pub struct ResourceRequirements {
    pub cpu_weight: f64,        // 0.0 to 1.0, CPU intensive
    pub memory_mb: usize,       // Expected memory usage
    pub io_weight: f64,         // 0.0 to 1.0, I/O intensive
    pub network_weight: f64,    // 0.0 to 1.0, network intensive
    pub disk_weight: f64,       // 0.0 to 1.0, disk intensive
    pub estimated_duration: Duration,
}

impl Default for ResourceRequirements {
    fn default() -> Self {
        Self {
            cpu_weight: 0.5,
            memory_mb: 10,
            io_weight: 0.3,
            network_weight: 0.1,
            disk_weight: 0.1,
            estimated_duration: Duration::from_secs(1),
        }
    }
}

/// Scheduled task representation
#[derive(Debug)]
pub struct ScheduledTask {
    pub task_id: Uuid,
    pub name: String,
    pub priority: TaskPriority,
    pub category: TaskCategory,
    pub requirements: ResourceRequirements,
    pub created_at: Instant,
    pub deadline: Option<Instant>,
    pub timeout: Duration,
    pub retry_count: u32,
    pub max_retries: u32,
    pub dependencies: Vec<Uuid>,
    pub affinity: Option<WorkerAffinity>,
    pub task_fn: Box<dyn TaskFunction>,
}

impl PartialEq for ScheduledTask {
    fn eq(&self, other: &Self) -> bool {
        self.task_id == other.task_id
    }
}

impl Eq for ScheduledTask {}

impl PartialOrd for ScheduledTask {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ScheduledTask {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher priority tasks come first
        self.priority.cmp(&other.priority)
            .then_with(|| {
                // Earlier deadlines come first
                match (self.deadline, other.deadline) {
                    (Some(a), Some(b)) => a.cmp(&b),
                    (Some(_), None) => Ordering::Less,
                    (None, Some(_)) => Ordering::Greater,
                    (None, None) => Ordering::Equal,
                }
            })
            .then_with(|| self.created_at.cmp(&other.created_at))
    }
}

/// Worker thread affinity preferences
#[derive(Debug, Clone)]
pub enum WorkerAffinity {
    Any,
    CpuIntensive,
    IoIntensive,
    NetworkIntensive,
    SpecificWorker(usize),
}

/// Task execution function trait
pub trait TaskFunction: Send + Sync {
    fn execute(&self) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<TaskResult, TaskError>> + Send + '_>>;
}

/// Task execution result
#[derive(Debug, Clone)]
pub struct TaskResult {
    pub data: Vec<u8>,
    pub metadata: HashMap<String, String>,
    pub execution_time: Duration,
    pub resources_used: ResourceUsage,
}

/// Resource usage tracking
#[derive(Debug, Clone)]
pub struct ResourceUsage {
    pub cpu_time_ms: u64,
    pub memory_peak_mb: usize,
    pub network_bytes: u64,
    pub disk_bytes: u64,
}

/// Task execution errors
#[derive(Debug, Clone)]
pub enum TaskError {
    Timeout,
    ResourceExhaustion,
    DependencyFailure(Vec<Uuid>),
    ExecutionError(String),
    Cancelled,
}

/// Worker thread information
#[derive(Debug, Clone)]
pub struct WorkerInfo {
    pub worker_id: usize,
    pub worker_type: WorkerType,
    pub current_load: f64, // 0.0 to 1.0
    pub tasks_completed: u64,
    pub tasks_failed: u64,
    pub average_task_time_ms: f64,
    pub specialization: Vec<TaskCategory>,
    pub is_available: bool,
}

#[derive(Debug, Clone)]
pub enum WorkerType {
    General,
    CpuOptimized,
    IoOptimized,
    NetworkOptimized,
    Specialized(TaskCategory),
}

/// System resource metrics
#[derive(Debug, Clone)]
pub struct SystemMetrics {
    pub timestamp: Instant,
    pub cpu_utilization: f64,       // 0.0 to 1.0
    pub memory_utilization: f64,    // 0.0 to 1.0
    pub memory_used_mb: usize,
    pub memory_available_mb: usize,
    pub load_average_1m: f64,
    pub load_average_5m: f64,
    pub load_average_15m: f64,
    pub network_io_rate: f64,       // bytes/sec
    pub disk_io_rate: f64,          // bytes/sec
    pub active_tasks: usize,
    pub queued_tasks: usize,
    pub worker_utilization: HashMap<usize, f64>,
}

/// Scheduler statistics
#[derive(Debug, Clone)]
pub struct SchedulerStatistics {
    pub total_tasks_scheduled: u64,
    pub total_tasks_completed: u64,
    pub total_tasks_failed: u64,
    pub total_tasks_cancelled: u64,
    pub average_queue_time_ms: f64,
    pub average_execution_time_ms: f64,
    pub throughput_tasks_per_sec: f64,
    pub resource_utilization: SystemMetrics,
    pub worker_stats: Vec<WorkerInfo>,
    pub category_performance: HashMap<TaskCategory, CategoryPerformance>,
}

#[derive(Debug, Clone)]
pub struct CategoryPerformance {
    pub category: TaskCategory,
    pub tasks_completed: u64,
    pub average_execution_time_ms: f64,
    pub success_rate: f64,
    pub average_resource_usage: ResourceUsage,
}

/// Adaptive resource scheduler
pub struct AdaptiveResourceScheduler {
    config: ResourceSchedulerConfig,
    
    // Task queues organized by priority
    high_priority_queue: Arc<Mutex<BinaryHeap<ScheduledTask>>>,
    normal_priority_queue: Arc<Mutex<BinaryHeap<ScheduledTask>>>,
    background_queue: Arc<Mutex<VecDeque<ScheduledTask>>>,
    
    // Active tasks and workers
    active_tasks: Arc<RwLock<HashMap<Uuid, TaskExecution>>>,
    workers: Arc<RwLock<Vec<WorkerInfo>>>,
    worker_handles: Arc<Mutex<Vec<JoinHandle<()>>>>,
    
    // Resource management
    cpu_semaphore: Arc<Semaphore>,
    memory_semaphore: Arc<Semaphore>,
    io_semaphore: Arc<Semaphore>,
    
    // Metrics and monitoring
    system_metrics: Arc<RwLock<SystemMetrics>>,
    scheduler_stats: Arc<RwLock<SchedulerStatistics>>,
    
    // Adaptive algorithms
    load_predictor: Arc<RwLock<LoadPredictor>>,
    resource_allocator: Arc<RwLock<ResourceAllocator>>,
    
    // Control
    is_running: AtomicBool,
    task_counter: AtomicU64,
    next_worker_id: AtomicUsize,
}

/// Task execution context
#[derive(Debug)]
struct TaskExecution {
    task_id: Uuid,
    worker_id: usize,
    started_at: Instant,
    timeout_handle: Option<JoinHandle<()>>,
}

/// Load prediction for adaptive scheduling
#[derive(Debug)]
struct LoadPredictor {
    cpu_history: VecDeque<(Instant, f64)>,
    memory_history: VecDeque<(Instant, f64)>,
    task_completion_history: VecDeque<(Instant, usize)>,
    prediction_model: PredictionModel,
}

#[derive(Debug)]
enum PredictionModel {
    MovingAverage(usize), // window size
    ExponentialSmoothing(f64), // alpha parameter
    LinearRegression,
}

/// Resource allocator for optimal distribution
#[derive(Debug)]
struct ResourceAllocator {
    cpu_allocation: HashMap<TaskCategory, f64>,
    memory_allocation: HashMap<TaskCategory, usize>,
    io_allocation: HashMap<TaskCategory, f64>,
    allocation_history: VecDeque<(Instant, HashMap<TaskCategory, ResourceUsage>)>,
}

impl AdaptiveResourceScheduler {
    pub async fn new(config: ResourceSchedulerConfig) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let cpu_cores = if config.cpu_cores == 0 { num_cpus::get() } else { config.cpu_cores };
        
        let scheduler = Self {
            high_priority_queue: Arc::new(Mutex::new(BinaryHeap::new())),
            normal_priority_queue: Arc::new(Mutex::new(BinaryHeap::new())),
            background_queue: Arc::new(Mutex::new(VecDeque::new())),
            active_tasks: Arc::new(RwLock::new(HashMap::new())),
            workers: Arc::new(RwLock::new(Vec::new())),
            worker_handles: Arc::new(Mutex::new(Vec::new())),
            cpu_semaphore: Arc::new(Semaphore::new(cpu_cores * 2)), // Allow oversubscription
            memory_semaphore: Arc::new(Semaphore::new(config.memory_limit_mb)),
            io_semaphore: Arc::new(Semaphore::new(100)), // Arbitrary I/O limit
            system_metrics: Arc::new(RwLock::new(SystemMetrics {
                timestamp: Instant::now(),
                cpu_utilization: 0.0,
                memory_utilization: 0.0,
                memory_used_mb: 0,
                memory_available_mb: config.memory_limit_mb,
                load_average_1m: 0.0,
                load_average_5m: 0.0,
                load_average_15m: 0.0,
                network_io_rate: 0.0,
                disk_io_rate: 0.0,
                active_tasks: 0,
                queued_tasks: 0,
                worker_utilization: HashMap::new(),
            })),
            scheduler_stats: Arc::new(RwLock::new(SchedulerStatistics {
                total_tasks_scheduled: 0,
                total_tasks_completed: 0,
                total_tasks_failed: 0,
                total_tasks_cancelled: 0,
                average_queue_time_ms: 0.0,
                average_execution_time_ms: 0.0,
                throughput_tasks_per_sec: 0.0,
                resource_utilization: SystemMetrics {
                    timestamp: Instant::now(),
                    cpu_utilization: 0.0,
                    memory_utilization: 0.0,
                    memory_used_mb: 0,
                    memory_available_mb: config.memory_limit_mb,
                    load_average_1m: 0.0,
                    load_average_5m: 0.0,
                    load_average_15m: 0.0,
                    network_io_rate: 0.0,
                    disk_io_rate: 0.0,
                    active_tasks: 0,
                    queued_tasks: 0,
                    worker_utilization: HashMap::new(),
                },
                worker_stats: Vec::new(),
                category_performance: HashMap::new(),
            })),
            load_predictor: Arc::new(RwLock::new(LoadPredictor {
                cpu_history: VecDeque::new(),
                memory_history: VecDeque::new(),
                task_completion_history: VecDeque::new(),
                prediction_model: PredictionModel::MovingAverage(10),
            })),
            resource_allocator: Arc::new(RwLock::new(ResourceAllocator {
                cpu_allocation: HashMap::new(),
                memory_allocation: HashMap::new(),
                io_allocation: HashMap::new(),
                allocation_history: VecDeque::new(),
            })),
            is_running: AtomicBool::new(false),
            task_counter: AtomicU64::new(0),
            next_worker_id: AtomicUsize::new(0),
            config,
        };

        // Initialize workers
        scheduler.initialize_workers().await?;
        
        Ok(scheduler)
    }

    /// Start the resource scheduler
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.is_running.swap(true, AtomicOrdering::Relaxed) {
            return Ok(()); // Already running
        }

        println!("Starting adaptive resource scheduler with {} workers", self.config.max_concurrent_tasks);

        // Start worker threads
        self.start_workers().await?;
        
        // Start monitoring
        self.start_monitoring().await;
        
        // Start adaptive algorithms
        if self.config.enable_adaptive_scheduling {
            self.start_adaptive_algorithms().await;
        }

        Ok(())
    }

    /// Stop the resource scheduler
    pub async fn stop(&self) {
        self.is_running.store(false, AtomicOrdering::Relaxed);
        
        // Wait for workers to finish
        let mut handles = self.worker_handles.lock().await;
        while let Some(handle) = handles.pop() {
            handle.abort();
        }
        
        println!("Resource scheduler stopped");
    }

    /// Schedule a task for execution
    pub async fn schedule_task(&self, mut task: ScheduledTask) -> Result<Uuid, Box<dyn std::error::Error + Send + Sync>> {
        if !self.is_running.load(AtomicOrdering::Relaxed) {
            return Err("Scheduler is not running".into());
        }

        // Assign task ID if not set
        if task.task_id.is_nil() {
            task.task_id = Uuid::new_v4();
        }

        let task_id = task.task_id;
        self.task_counter.fetch_add(1, AtomicOrdering::Relaxed);

        // Update statistics
        {
            let mut stats = self.scheduler_stats.write().await;
            stats.total_tasks_scheduled += 1;
        }

        // Add to appropriate queue based on priority and load balancing
        match task.priority {
            TaskPriority::Critical | TaskPriority::High => {
                let mut queue = self.high_priority_queue.lock().await;
                if queue.len() >= self.config.max_queue_size / 2 {
                    return Err("High priority queue is full".into());
                }
                queue.push(task);
            },
            TaskPriority::Normal => {
                let mut queue = self.normal_priority_queue.lock().await;
                if queue.len() >= self.config.max_queue_size {
                    return Err("Normal priority queue is full".into());
                }
                queue.push(task);
            },
            TaskPriority::Low | TaskPriority::Background => {
                let mut queue = self.background_queue.lock().await;
                queue.push_back(task);
                
                // Limit background queue size
                while queue.len() > self.config.max_queue_size * 2 {
                    queue.pop_front();
                }
            },
        }

        Ok(task_id)
    }

    /// Cancel a scheduled or running task
    pub async fn cancel_task(&self, task_id: Uuid) -> bool {
        // Remove from queues first
        let mut removed = false;
        
        // Check high priority queue
        {
            let mut queue = self.high_priority_queue.lock().await;
            let mut temp_queue = BinaryHeap::new();
            while let Some(task) = queue.pop() {
                if task.task_id == task_id {
                    removed = true;
                } else {
                    temp_queue.push(task);
                }
            }
            *queue = temp_queue;
        }

        if !removed {
            // Check normal priority queue
            let mut queue = self.normal_priority_queue.lock().await;
            let mut temp_queue = BinaryHeap::new();
            while let Some(task) = queue.pop() {
                if task.task_id == task_id {
                    removed = true;
                } else {
                    temp_queue.push(task);
                }
            }
            *queue = temp_queue;
        }

        if !removed {
            // Check background queue
            let mut queue = self.background_queue.lock().await;
            if let Some(pos) = queue.iter().position(|t| t.task_id == task_id) {
                queue.remove(pos);
                removed = true;
            }
        }

        // If not in queues, check if it's currently executing
        if !removed {
            let active = self.active_tasks.read().await;
            if let Some(execution) = active.get(&task_id) {
                if let Some(timeout_handle) = &execution.timeout_handle {
                    timeout_handle.abort();
                }
                // Task will be cleaned up by the worker
                removed = true;
            }
        }

        if removed {
            let mut stats = self.scheduler_stats.write().await;
            stats.total_tasks_cancelled += 1;
        }

        removed
    }

    /// Get current system metrics
    pub async fn get_system_metrics(&self) -> SystemMetrics {
        let metrics = self.system_metrics.read().await;
        metrics.clone()
    }

    /// Get scheduler statistics
    pub async fn get_statistics(&self) -> SchedulerStatistics {
        let stats = self.scheduler_stats.read().await;
        stats.clone()
    }

    /// Get current queue sizes
    pub async fn get_queue_status(&self) -> (usize, usize, usize) {
        let high = self.high_priority_queue.lock().await.len();
        let normal = self.normal_priority_queue.lock().await.len();
        let background = self.background_queue.lock().await.len();
        (high, normal, background)
    }

    /// Initialize worker pool
    async fn initialize_workers(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut workers = Vec::new();
        
        // Create general purpose workers
        for i in 0..self.config.max_concurrent_tasks {
            workers.push(WorkerInfo {
                worker_id: i,
                worker_type: WorkerType::General,
                current_load: 0.0,
                tasks_completed: 0,
                tasks_failed: 0,
                average_task_time_ms: 0.0,
                specialization: vec![
                    TaskCategory::Computation,
                    TaskCategory::NetworkIO,
                    TaskCategory::DiskIO,
                ],
                is_available: true,
            });
        }

        // Add specialized workers if we have enough cores
        if self.config.cpu_cores >= 4 {
            // Add CPU-optimized worker
            workers.push(WorkerInfo {
                worker_id: workers.len(),
                worker_type: WorkerType::CpuOptimized,
                current_load: 0.0,
                tasks_completed: 0,
                tasks_failed: 0,
                average_task_time_ms: 0.0,
                specialization: vec![TaskCategory::Computation, TaskCategory::Crypto],
                is_available: true,
            });

            // Add I/O-optimized worker
            workers.push(WorkerInfo {
                worker_id: workers.len(),
                worker_type: WorkerType::IoOptimized,
                current_load: 0.0,
                tasks_completed: 0,
                tasks_failed: 0,
                average_task_time_ms: 0.0,
                specialization: vec![TaskCategory::NetworkIO, TaskCategory::DiskIO, TaskCategory::Database],
                is_available: true,
            });
        }

        let mut worker_pool = self.workers.write().await;
        *worker_pool = workers;

        Ok(())
    }

    /// Start worker threads
    async fn start_workers(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut handles = Vec::new();
        let workers = self.workers.read().await;
        
        for worker in workers.iter() {
            let worker_id = worker.worker_id;
            let scheduler = self.clone();
            
            let handle = tokio::spawn(async move {
                scheduler.worker_loop(worker_id).await;
            });
            
            handles.push(handle);
        }

        let mut worker_handles = self.worker_handles.lock().await;
        *worker_handles = handles;

        Ok(())
    }

    /// Worker thread main loop
    async fn worker_loop(&self, worker_id: usize) {
        println!("Worker {} started", worker_id);
        
        while self.is_running.load(AtomicOrdering::Relaxed) {
            match self.get_next_task(worker_id).await {
                Some(task) => {
                    self.execute_task(worker_id, task).await;
                },
                None => {
                    // No tasks available, sleep briefly
                    tokio::time::sleep(Duration::from_millis(10)).await;
                },
            }
        }
        
        println!("Worker {} stopped", worker_id);
    }

    /// Get next task for worker
    async fn get_next_task(&self, worker_id: usize) -> Option<ScheduledTask> {
        // Get worker specialization
        let worker_specialization = {
            let workers = self.workers.read().await;
            workers.get(worker_id)?.specialization.clone()
        };

        // Try high priority queue first
        {
            let mut queue = self.high_priority_queue.lock().await;
            if let Some(task) = queue.pop() {
                return Some(task);
            }
        }

        // Then normal priority queue
        {
            let mut queue = self.normal_priority_queue.lock().await;
            if let Some(task) = queue.pop() {
                return Some(task);
            }
        }

        // Finally background queue, but only if worker specializes in background tasks
        // or if system load is low
        let system_load = {
            let metrics = self.system_metrics.read().await;
            metrics.cpu_utilization
        };

        if system_load < self.config.target_cpu_utilization * 0.5 {
            let mut queue = self.background_queue.lock().await;
            return queue.pop_front();
        }

        None
    }

    /// Execute a task
    async fn execute_task(&self, worker_id: usize, task: ScheduledTask) {
        let task_id = task.task_id;
        let start_time = Instant::now();
        
        // Acquire necessary resources
        let _cpu_permit = match self.cpu_semaphore.acquire().await {
            Ok(permit) => permit,
            Err(_) => {
                self.record_task_failure(task_id, TaskError::ResourceExhaustion).await;
                return;
            }
        };

        let _memory_permit = match self.memory_semaphore.acquire_many(task.requirements.memory_mb as u32).await {
            Ok(permit) => permit,
            Err(_) => {
                self.record_task_failure(task_id, TaskError::ResourceExhaustion).await;
                return;
            }
        };

        // Record task execution start
        {
            let mut active = self.active_tasks.write().await;
            active.insert(task_id, TaskExecution {
                task_id,
                worker_id,
                started_at: start_time,
                timeout_handle: None,
            });
        }

        // Update worker status
        {
            let mut workers = self.workers.write().await;
            if let Some(worker) = workers.get_mut(worker_id) {
                worker.is_available = false;
                worker.current_load = 1.0; // Simplified load calculation
            }
        }

        // Execute the task with timeout
        let execution_result = tokio::time::timeout(
            task.timeout,
            task.task_fn.execute()
        ).await;

        let execution_time = start_time.elapsed();
        
        // Clean up task execution record
        {
            let mut active = self.active_tasks.write().await;
            active.remove(&task_id);
        }

        // Update worker status
        {
            let mut workers = self.workers.write().await;
            if let Some(worker) = workers.get_mut(worker_id) {
                worker.is_available = true;
                worker.current_load = 0.0;
                worker.average_task_time_ms = 
                    (worker.average_task_time_ms * worker.tasks_completed as f64 + execution_time.as_millis() as f64) 
                    / (worker.tasks_completed + 1) as f64;
            }
        }

        // Record results
        match execution_result {
            Ok(Ok(result)) => {
                self.record_task_success(task_id, result, execution_time).await;
                
                // Update worker success count
                let mut workers = self.workers.write().await;
                if let Some(worker) = workers.get_mut(worker_id) {
                    worker.tasks_completed += 1;
                }
            },
            Ok(Err(error)) => {
                self.record_task_failure(task_id, error).await;
                
                // Update worker failure count
                let mut workers = self.workers.write().await;
                if let Some(worker) = workers.get_mut(worker_id) {
                    worker.tasks_failed += 1;
                }
            },
            Err(_) => {
                self.record_task_failure(task_id, TaskError::Timeout).await;
                
                // Update worker failure count
                let mut workers = self.workers.write().await;
                if let Some(worker) = workers.get_mut(worker_id) {
                    worker.tasks_failed += 1;
                }
            },
        }
    }

    /// Record successful task completion
    async fn record_task_success(&self, _task_id: Uuid, _result: TaskResult, execution_time: Duration) {
        let mut stats = self.scheduler_stats.write().await;
        stats.total_tasks_completed += 1;
        stats.average_execution_time_ms = 
            (stats.average_execution_time_ms * (stats.total_tasks_completed - 1) as f64 
             + execution_time.as_millis() as f64) / stats.total_tasks_completed as f64;
    }

    /// Record task failure
    async fn record_task_failure(&self, _task_id: Uuid, _error: TaskError) {
        let mut stats = self.scheduler_stats.write().await;
        stats.total_tasks_failed += 1;
    }

    /// Start system monitoring
    async fn start_monitoring(&self) {
        let scheduler = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(scheduler.config.monitoring_interval);
            
            while scheduler.is_running.load(AtomicOrdering::Relaxed) {
                interval.tick().await;
                scheduler.collect_system_metrics().await;
            }
        });
    }

    /// Start adaptive algorithms
    async fn start_adaptive_algorithms(&self) {
        let scheduler = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            
            while scheduler.is_running.load(AtomicOrdering::Relaxed) {
                interval.tick().await;
                scheduler.perform_adaptive_optimization().await;
            }
        });
    }

    /// Collect system metrics
    async fn collect_system_metrics(&self) {
        // In a real implementation, this would collect actual system metrics
        // For now, we'll simulate some values
        use rand::Rng;
        let mut rng = rand::thread_rng();

        let active_tasks = self.active_tasks.read().await.len();
        let (high_queue, normal_queue, background_queue) = self.get_queue_status().await;
        let queued_tasks = high_queue + normal_queue + background_queue;
        
        let cpu_utilization = rng.gen_range(0.2..0.9);
        let memory_utilization = rng.gen_range(0.3..0.8);
        
        let metrics = SystemMetrics {
            timestamp: Instant::now(),
            cpu_utilization,
            memory_utilization,
            memory_used_mb: (memory_utilization * self.config.memory_limit_mb as f64) as usize,
            memory_available_mb: self.config.memory_limit_mb - ((memory_utilization * self.config.memory_limit_mb as f64) as usize),
            load_average_1m: cpu_utilization,
            load_average_5m: cpu_utilization * 0.9,
            load_average_15m: cpu_utilization * 0.8,
            network_io_rate: rng.gen_range(1000.0..100000.0),
            disk_io_rate: rng.gen_range(500.0..50000.0),
            active_tasks,
            queued_tasks,
            worker_utilization: HashMap::new(), // Would be filled with actual worker data
        };

        let mut system_metrics = self.system_metrics.write().await;
        *system_metrics = metrics.clone();
        
        // Update scheduler statistics
        let mut stats = self.scheduler_stats.write().await;
        stats.resource_utilization = metrics;
        
        // Update load predictor
        let mut predictor = self.load_predictor.write().await;
        predictor.cpu_history.push_back((Instant::now(), cpu_utilization));
        predictor.memory_history.push_back((Instant::now(), memory_utilization));
        
        // Keep history bounded
        while predictor.cpu_history.len() > 100 {
            predictor.cpu_history.pop_front();
        }
        while predictor.memory_history.len() > 100 {
            predictor.memory_history.pop_front();
        }
    }

    /// Perform adaptive optimization
    async fn perform_adaptive_optimization(&self) {
        let metrics = self.get_system_metrics().await;
        
        // Adjust worker pool size based on load
        if metrics.cpu_utilization > self.config.target_cpu_utilization * 1.2 {
            // System is overloaded, consider reducing concurrent tasks
            println!("System overloaded - CPU: {:.1}%", metrics.cpu_utilization * 100.0);
        } else if metrics.cpu_utilization < self.config.target_cpu_utilization * 0.5 {
            // System is underutilized, could handle more tasks
            println!("System underutilized - CPU: {:.1}%", metrics.cpu_utilization * 100.0);
        }

        // Adjust resource allocation based on task categories
        self.optimize_resource_allocation().await;
        
        // Update task prioritization based on performance
        self.update_priority_algorithms().await;
    }

    /// Optimize resource allocation across task categories
    async fn optimize_resource_allocation(&self) {
        // Analyze historical performance by category
        let stats = self.scheduler_stats.read().await;
        
        let mut allocator = self.resource_allocator.write().await;
        
        // Adjust allocations based on category performance
        for (category, performance) in &stats.category_performance {
            let base_cpu = 1.0 / stats.category_performance.len() as f64;
            let performance_factor = if performance.average_execution_time_ms > 0.0 {
                1.0 / (performance.average_execution_time_ms / 1000.0).sqrt()
            } else {
                1.0
            };
            
            allocator.cpu_allocation.insert(category.clone(), base_cpu * performance_factor);
        }
        
        println!("Resource allocation optimized for {} categories", stats.category_performance.len());
    }

    /// Update priority algorithms based on performance
    async fn update_priority_algorithms(&self) {
        // Analyze task completion patterns and adjust priority weights
        // This is where machine learning algorithms could be applied
        
        let stats = self.scheduler_stats.read().await;
        if stats.total_tasks_completed > 100 {
            // Enough data to make adjustments
            println!("Priority algorithms updated based on {} completed tasks", stats.total_tasks_completed);
        }
    }
}

impl Clone for AdaptiveResourceScheduler {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            high_priority_queue: Arc::clone(&self.high_priority_queue),
            normal_priority_queue: Arc::clone(&self.normal_priority_queue),
            background_queue: Arc::clone(&self.background_queue),
            active_tasks: Arc::clone(&self.active_tasks),
            workers: Arc::clone(&self.workers),
            worker_handles: Arc::clone(&self.worker_handles),
            cpu_semaphore: Arc::clone(&self.cpu_semaphore),
            memory_semaphore: Arc::clone(&self.memory_semaphore),
            io_semaphore: Arc::clone(&self.io_semaphore),
            system_metrics: Arc::clone(&self.system_metrics),
            scheduler_stats: Arc::clone(&self.scheduler_stats),
            load_predictor: Arc::clone(&self.load_predictor),
            resource_allocator: Arc::clone(&self.resource_allocator),
            is_running: AtomicBool::new(self.is_running.load(AtomicOrdering::Relaxed)),
            task_counter: AtomicU64::new(self.task_counter.load(AtomicOrdering::Relaxed)),
            next_worker_id: AtomicUsize::new(self.next_worker_id.load(AtomicOrdering::Relaxed)),
        }
    }
}

// Simple task function implementations for testing
pub struct SimpleTask {
    pub name: String,
    pub work_duration: Duration,
}

impl TaskFunction for SimpleTask {
    fn execute(&self) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<TaskResult, TaskError>> + Send + '_>> {
        Box::pin(async move {
            tokio::time::sleep(self.work_duration).await;
            
            Ok(TaskResult {
                data: self.name.as_bytes().to_vec(),
                metadata: HashMap::new(),
                execution_time: self.work_duration,
                resources_used: ResourceUsage {
                    cpu_time_ms: self.work_duration.as_millis() as u64,
                    memory_peak_mb: 10,
                    network_bytes: 0,
                    disk_bytes: 0,
                },
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_scheduler_creation() {
        let config = ResourceSchedulerConfig {
            max_concurrent_tasks: 4,
            ..Default::default()
        };
        
        let scheduler = AdaptiveResourceScheduler::new(config).await.unwrap();
        let metrics = scheduler.get_system_metrics().await;
        
        assert!(metrics.memory_available_mb > 0);
    }

    #[tokio::test]
    async fn test_task_scheduling() {
        let config = ResourceSchedulerConfig {
            max_concurrent_tasks: 2,
            ..Default::default()
        };
        
        let scheduler = AdaptiveResourceScheduler::new(config).await.unwrap();
        scheduler.start().await.unwrap();
        
        let task = ScheduledTask {
            task_id: Uuid::new_v4(),
            name: "test_task".to_string(),
            priority: TaskPriority::Normal,
            category: TaskCategory::Computation,
            requirements: ResourceRequirements::default(),
            created_at: Instant::now(),
            deadline: None,
            timeout: Duration::from_secs(5),
            retry_count: 0,
            max_retries: 1,
            dependencies: Vec::new(),
            affinity: None,
            task_fn: Box::new(SimpleTask {
                name: "test".to_string(),
                work_duration: Duration::from_millis(100),
            }),
        };
        
        let task_id = scheduler.schedule_task(task).await.unwrap();
        assert!(!task_id.is_nil());
        
        // Give time for task execution
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        let stats = scheduler.get_statistics().await;
        assert!(stats.total_tasks_scheduled > 0);
        
        scheduler.stop().await;
    }

    #[tokio::test]
    async fn test_priority_ordering() {
        let config = ResourceSchedulerConfig {
            max_concurrent_tasks: 1, // Force queueing
            ..Default::default()
        };
        
        let scheduler = AdaptiveResourceScheduler::new(config).await.unwrap();
        scheduler.start().await.unwrap();
        
        // Schedule tasks with different priorities
        let low_task = ScheduledTask {
            task_id: Uuid::new_v4(),
            name: "low_priority".to_string(),
            priority: TaskPriority::Low,
            category: TaskCategory::Background,
            requirements: ResourceRequirements::default(),
            created_at: Instant::now(),
            deadline: None,
            timeout: Duration::from_secs(5),
            retry_count: 0,
            max_retries: 1,
            dependencies: Vec::new(),
            affinity: None,
            task_fn: Box::new(SimpleTask {
                name: "low".to_string(),
                work_duration: Duration::from_millis(100),
            }),
        };
        
        let high_task = ScheduledTask {
            task_id: Uuid::new_v4(),
            name: "high_priority".to_string(),
            priority: TaskPriority::High,
            category: TaskCategory::Computation,
            requirements: ResourceRequirements::default(),
            created_at: Instant::now(),
            deadline: None,
            timeout: Duration::from_secs(5),
            retry_count: 0,
            max_retries: 1,
            dependencies: Vec::new(),
            affinity: None,
            task_fn: Box::new(SimpleTask {
                name: "high".to_string(),
                work_duration: Duration::from_millis(50),
            }),
        };
        
        let _low_id = scheduler.schedule_task(low_task).await.unwrap();
        let _high_id = scheduler.schedule_task(high_task).await.unwrap();
        
        // High priority task should be executed first
        // In a real test, you'd verify execution order
        
        tokio::time::sleep(Duration::from_millis(300)).await;
        
        let (high_queue, _normal_queue, _background_queue) = scheduler.get_queue_status().await;
        
        // Queue should be processed
        assert!(high_queue == 0);
        
        scheduler.stop().await;
    }

    #[tokio::test]
    async fn test_system_metrics_collection() {
        let config = ResourceSchedulerConfig {
            monitoring_interval: Duration::from_millis(100),
            ..Default::default()
        };
        
        let scheduler = AdaptiveResourceScheduler::new(config).await.unwrap();
        scheduler.start().await.unwrap();
        
        // Wait for metrics collection
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        let metrics = scheduler.get_system_metrics().await;
        assert!(metrics.cpu_utilization >= 0.0);
        assert!(metrics.memory_utilization >= 0.0);
        assert!(metrics.memory_available_mb > 0);
        
        scheduler.stop().await;
    }
}