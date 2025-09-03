//! Task lifecycle tracking for proper cleanup
//!
//! This module provides a centralized task tracker that ensures all spawned
//! tasks are properly cleaned up and don't leak resources.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tracing::{debug, info, warn, error};

/// Global task tracker instance
static TASK_TRACKER: once_cell::sync::Lazy<Arc<TaskTracker>> = once_cell::sync::Lazy::new(|| {
    Arc::new(TaskTracker::new())
});

/// Get the global task tracker
pub fn global_tracker() -> Arc<TaskTracker> {
    Arc::clone(&TASK_TRACKER)
}

/// Task identifier type
pub type TaskId = u64;

/// Information about a tracked task
#[derive(Debug, Clone)]
pub struct TaskInfo {
    pub id: TaskId,
    pub name: String,
    pub spawn_time: Instant,
    pub task_type: TaskType,
    pub parent_id: Option<TaskId>,
}

/// Type of task for categorization
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskType {
    /// Network I/O tasks
    Network,
    /// Database operations
    Database,
    /// Game logic processing
    GameLogic,
    /// Consensus operations
    Consensus,
    /// Background maintenance
    Maintenance,
    /// User interface updates
    UI,
    /// General purpose
    General,
}

/// Task lifecycle state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// Centralized task tracker
pub struct TaskTracker {
    /// Counter for generating unique task IDs
    next_id: AtomicU64,
    /// Currently running tasks
    tasks: Arc<RwLock<HashMap<TaskId, TrackedTask>>>,
    /// Statistics
    stats: TaskStats,
}

struct TrackedTask {
    info: TaskInfo,
    state: TaskState,
    handle: Option<JoinHandle<()>>,
}

/// Task tracking statistics
pub struct TaskStats {
    pub total_spawned: AtomicUsize,
    pub currently_running: AtomicUsize,
    pub total_completed: AtomicUsize,
    pub total_failed: AtomicUsize,
    pub total_cancelled: AtomicUsize,
}

impl TaskTracker {
    pub fn new() -> Self {
        let tracker = Self {
            next_id: AtomicU64::new(1),
            tasks: Arc::new(RwLock::new(HashMap::with_capacity(100))),
            stats: TaskStats {
                total_spawned: AtomicUsize::new(0),
                currently_running: AtomicUsize::new(0),
                total_completed: AtomicUsize::new(0),
                total_failed: AtomicUsize::new(0),
                total_cancelled: AtomicUsize::new(0),
            },
        };

        // Start cleanup task
        tracker.start_cleanup_task();
        tracker
    }

    /// Register a new task
    pub async fn register_task(
        &self,
        name: String,
        task_type: TaskType,
        handle: JoinHandle<()>,
    ) -> TaskId {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);

        let info = TaskInfo {
            id,
            name: name.clone(),
            spawn_time: Instant::now(),
            task_type,
            parent_id: None, // Could track parent context if needed
        };

        let tracked = TrackedTask {
            info: info.clone(),
            state: TaskState::Running,
            handle: Some(handle),
        };

        let mut tasks = self.tasks.write().await;
        tasks.insert(id, tracked);

        self.stats.total_spawned.fetch_add(1, Ordering::Relaxed);
        self.stats.currently_running.fetch_add(1, Ordering::Relaxed);

        debug!("Task registered: {} (ID: {}, Type: {:?})", name, id, task_type);

        id
    }

    /// Mark a task as completed
    pub async fn complete_task(&self, id: TaskId) {
        let mut tasks = self.tasks.write().await;
        if let Some(task) = tasks.get_mut(&id) {
            task.state = TaskState::Completed;
            task.handle = None; // Release the handle

            self.stats.currently_running.fetch_sub(1, Ordering::Relaxed);
            self.stats.total_completed.fetch_add(1, Ordering::Relaxed);

            debug!("Task completed: {} (ID: {})", task.info.name, id);
        }
    }

    /// Mark a task as failed
    pub async fn fail_task(&self, id: TaskId, error: &str) {
        let mut tasks = self.tasks.write().await;
        if let Some(task) = tasks.get_mut(&id) {
            task.state = TaskState::Failed;
            task.handle = None;

            self.stats.currently_running.fetch_sub(1, Ordering::Relaxed);
            self.stats.total_failed.fetch_add(1, Ordering::Relaxed);

            warn!("Task failed: {} (ID: {}): {}", task.info.name, id, error);
        }
    }

    /// Cancel a task
    pub async fn cancel_task(&self, id: TaskId) -> bool {
        let mut tasks = self.tasks.write().await;
        if let Some(task) = tasks.get_mut(&id) {
            if let Some(handle) = task.handle.take() {
                handle.abort();
                task.state = TaskState::Cancelled;

                self.stats.currently_running.fetch_sub(1, Ordering::Relaxed);
                self.stats.total_cancelled.fetch_add(1, Ordering::Relaxed);

                info!("Task cancelled: {} (ID: {})", task.info.name, id);
                return true;
            }
        }
        false
    }

    /// Cancel all tasks of a specific type
    pub async fn cancel_tasks_by_type(&self, task_type: TaskType) -> usize {
        let mut tasks = self.tasks.write().await;
        let mut cancelled = 0;

        for (_, task) in tasks.iter_mut() {
            if task.info.task_type == task_type && task.state == TaskState::Running {
                if let Some(handle) = task.handle.take() {
                    handle.abort();
                    task.state = TaskState::Cancelled;
                    cancelled += 1;

                    self.stats.currently_running.fetch_sub(1, Ordering::Relaxed);
                    self.stats.total_cancelled.fetch_add(1, Ordering::Relaxed);
                }
            }
        }

        if cancelled > 0 {
            info!("Cancelled {} tasks of type {:?}", cancelled, task_type);
        }

        cancelled
    }

    /// Get current task statistics
    pub fn get_stats(&self) -> TaskStatsSnapshot {
        TaskStatsSnapshot {
            total_spawned: self.stats.total_spawned.load(Ordering::Relaxed),
            currently_running: self.stats.currently_running.load(Ordering::Relaxed),
            total_completed: self.stats.total_completed.load(Ordering::Relaxed),
            total_failed: self.stats.total_failed.load(Ordering::Relaxed),
            total_cancelled: self.stats.total_cancelled.load(Ordering::Relaxed),
        }
    }

    /// Get information about running tasks
    pub async fn get_running_tasks(&self) -> Vec<TaskInfo> {
        let tasks = self.tasks.read().await;
        tasks
            .values()
            .filter(|t| t.state == TaskState::Running)
            .map(|t| t.info.clone())
            .collect()
    }

    /// Cleanup completed/failed tasks older than the given duration
    pub async fn cleanup_old_tasks(&self, older_than: Duration) -> usize {
        let mut tasks = self.tasks.write().await;
        let now = Instant::now();
        let mut removed = 0;

        tasks.retain(|_id, task| {
            let should_keep = task.state == TaskState::Running ||
                              now.duration_since(task.info.spawn_time) < older_than;
            if !should_keep {
                removed += 1;
            }
            should_keep
        });

        if removed > 0 {
            debug!("Cleaned up {} old tasks", removed);
        }

        removed
    }

    /// Start periodic cleanup task
    fn start_cleanup_task(&self) {
        let tasks = Arc::clone(&self.tasks);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            loop {
                interval.tick().await;

                // Cleanup tasks older than 5 minutes
                let mut tasks_guard = tasks.write().await;
                let now = Instant::now();
                let before_count = tasks_guard.len();

                tasks_guard.retain(|_id, task| {
                    task.state == TaskState::Running ||
                    now.duration_since(task.info.spawn_time) < Duration::from_secs(300)
                });

                let removed = before_count - tasks_guard.len();
                if removed > 0 {
                    debug!("Cleanup removed {} completed tasks", removed);
                }
            }
        });
    }

    /// Shutdown tracker and cancel all running tasks
    pub async fn shutdown(&self) {
        info!("Shutting down task tracker");

        let mut tasks = self.tasks.write().await;
        let mut cancelled = 0;

        for (_, task) in tasks.iter_mut() {
            if let Some(handle) = task.handle.take() {
                handle.abort();
                cancelled += 1;
            }
        }

        tasks.clear();

        if cancelled > 0 {
            info!("Cancelled {} running tasks during shutdown", cancelled);
        }
    }
}

/// Snapshot of task statistics
#[derive(Debug, Clone)]
pub struct TaskStatsSnapshot {
    pub total_spawned: usize,
    pub currently_running: usize,
    pub total_completed: usize,
    pub total_failed: usize,
    pub total_cancelled: usize,
}

/// Extension trait for spawning tracked tasks
pub trait TrackedSpawn {
    /// Spawn a tracked task
    fn spawn_tracked<F>(
        &self,
        name: String,
        task_type: TaskType,
        future: F,
    ) -> impl std::future::Future<Output = TaskId>
    where
        F: std::future::Future<Output = ()> + Send + 'static;
}

impl TrackedSpawn for TaskTracker {
    async fn spawn_tracked<F>(
        &self,
        name: String,
        task_type: TaskType,
        future: F,
    ) -> TaskId
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        let tracker = global_tracker();
        let id_clone = Arc::new(AtomicU64::new(0));
        let id_ref = Arc::clone(&id_clone);

        let handle = tokio::spawn(async move {
            future.await;
            let id = id_ref.load(Ordering::SeqCst);
            if id != 0 {
                tracker.complete_task(id).await;
            }
        });

        let id = self.register_task(name, task_type, handle).await;
        id_clone.store(id, Ordering::SeqCst);
        id
    }
}

/// Convenience function to spawn a tracked task
pub async fn spawn_tracked<F>(
    name: impl Into<String>,
    task_type: TaskType,
    future: F,
) -> TaskId
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    global_tracker().spawn_tracked(name.into(), task_type, future).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_task_tracking() {
        let tracker = TaskTracker::new();

        let handle = tokio::spawn(async {
            tokio::time::sleep(Duration::from_millis(10)).await;
        });

        let id = tracker.register_task(
            "test_task".to_string(),
            TaskType::General,
            handle,
        ).await;

        let running = tracker.get_running_tasks().await;
        assert_eq!(running.len(), 1);
        assert_eq!(running[0].id, id);

        tokio::time::sleep(Duration::from_millis(20)).await;
        tracker.complete_task(id).await;

        let stats = tracker.get_stats();
        assert_eq!(stats.total_spawned, 1);
        assert_eq!(stats.total_completed, 1);
    }

    #[tokio::test]
    async fn test_task_cancellation() {
        let tracker = TaskTracker::new();

        let handle = tokio::spawn(async {
            tokio::time::sleep(Duration::from_secs(10)).await;
        });

        let id = tracker.register_task(
            "long_task".to_string(),
            TaskType::Network,
            handle,
        ).await;

        let cancelled = tracker.cancel_task(id).await;
        assert!(cancelled);

        let stats = tracker.get_stats();
        assert_eq!(stats.total_cancelled, 1);
    }
}