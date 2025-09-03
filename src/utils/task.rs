//! Safe task spawning utilities with panic handling
//!
//! Provides wrappers around tokio::spawn that ensure panics are caught
//! and logged rather than silently failing.

use std::future::Future;
use crate::utils::task_tracker::{spawn_tracked, TaskType};

use std::panic::{catch_unwind, AssertUnwindSafe};
use tokio::task::{JoinHandle, JoinError};
use futures::FutureExt;

/// Extension trait for safe task spawning
pub trait SpawnExt {
    /// Spawn a task with panic handling
    fn spawn_safe<F>(future: F) -> JoinHandle<Result<F::Output, TaskError>>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static;

    /// Spawn a task with panic handling and a name
    fn spawn_named<F>(name: &str, future: F) -> JoinHandle<Result<F::Output, TaskError>>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static;

    /// Spawn a detached task that logs errors
    fn spawn_detached<F>(future: F)
    where
        F: Future<Output = ()> + Send + 'static;

    /// Spawn a critical task that should restart on failure
    fn spawn_critical<F, R>(name: &str, factory: F) -> JoinHandle<()>
    where
        F: Fn() -> R + Send + Sync + 'static,
        R: Future<Output = ()> + Send + 'static;
}

#[derive(Debug, thiserror::Error)]
pub enum TaskError {
    #[error("Task panicked: {0}")]
    Panic(String),
    #[error("Task join failed: {0}")]
    JoinError(String),
}

/// Safe task spawner implementation
pub struct SafeSpawner;

impl SpawnExt for SafeSpawner {
    fn spawn_safe<F>(future: F) -> JoinHandle<Result<F::Output, TaskError>>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        tokio::spawn(async move {
            // Wrap the future to catch panics
            match AssertUnwindSafe(future).catch_unwind().await {
                Ok(result) => Ok(result),
                Err(panic) => {
                    let msg = if let Some(s) = panic.downcast_ref::<String>() {
                        s.clone()
                    } else if let Some(s) = panic.downcast_ref::<&str>() {
                        s.to_string()
                    } else {
                        "Unknown panic".to_string()
                    };

                    log::error!("Task panicked: {}", msg);
                    Err(TaskError::Panic(msg))
                }
            }
        })
    }

    fn spawn_named<F>(name: &str, future: F) -> JoinHandle<Result<F::Output, TaskError>>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let task_name = name.to_string();

        tokio::spawn(async move {
            log::debug!("Starting task: {}", task_name);

            match AssertUnwindSafe(future).catch_unwind().await {
                Ok(result) => {
                    log::debug!("Task completed successfully: {}", task_name);
                    Ok(result)
                }
                Err(panic) => {
                    let msg = if let Some(s) = panic.downcast_ref::<String>() {
                        s.clone()
                    } else if let Some(s) = panic.downcast_ref::<&str>() {
                        s.to_string()
                    } else {
                        "Unknown panic".to_string()
                    };

                    log::error!("Task '{}' panicked: {}", task_name, msg);
                    Err(TaskError::Panic(format!("{}: {}", task_name, msg)))
                }
            }
        })
    }

    fn spawn_detached<F>(future: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        tokio::spawn(async move {
            if let Err(panic) = AssertUnwindSafe(future).catch_unwind().await {
                let msg = if let Some(s) = panic.downcast_ref::<String>() {
                    s.clone()
                } else if let Some(s) = panic.downcast_ref::<&str>() {
                    s.to_string()
                } else {
                    "Unknown panic".to_string()
                };

                log::error!("Detached task panicked: {}", msg);
            }
        });
    }

    fn spawn_critical<F, R>(name: &str, factory: F) -> JoinHandle<()>
    where
        F: Fn() -> R + Send + Sync + 'static,
        R: Future<Output = ()> + Send + 'static,
    {
        let task_name = name.to_string();

        tokio::spawn(async move {
            let mut restart_count = 0;
            let max_restarts = 5;
            let mut backoff = std::time::Duration::from_secs(1);

            loop {
                log::info!("Starting critical task: {} (attempt {})", task_name, restart_count + 1);

                let future = factory();
                match AssertUnwindSafe(future).catch_unwind().await {
                    Ok(()) => {
                        log::info!("Critical task '{}' completed normally", task_name);
                        break;
                    }
                    Err(panic) => {
                        let msg = if let Some(s) = panic.downcast_ref::<String>() {
                            s.clone()
                        } else if let Some(s) = panic.downcast_ref::<&str>() {
                            s.to_string()
                        } else {
                            "Unknown panic".to_string()
                        };

                        log::error!(
                            "Critical task '{}' panicked (attempt {}): {}",
                            task_name, restart_count + 1, msg
                        );

                        restart_count += 1;
                        if restart_count >= max_restarts {
                            log::error!(
                                "Critical task '{}' exceeded max restarts ({}). Giving up.",
                                task_name, max_restarts
                            );
                            break;
                        }

                        // Exponential backoff
                        tokio::time::sleep(backoff).await;
                        backoff = std::cmp::min(backoff * 2, std::time::Duration::from_secs(60));
                    }
                }
            }
        })
    }
}

/// Convenience function to spawn a safe task
pub fn spawn_safe<F>(future: F) -> JoinHandle<Result<F::Output, TaskError>>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    SafeSpawner::spawn_safe(future)
}

/// Convenience function to spawn a named task
pub fn spawn_named<F>(name: &str, future: F) -> JoinHandle<Result<F::Output, TaskError>>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    SafeSpawner::spawn_named(name, future)
}

/// Convenience function to spawn a detached task
pub fn spawn_detached<F>(future: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    SafeSpawner::spawn_detached(future)
}

/// Convenience function to spawn a critical task
pub fn spawn_critical<F, R>(name: &str, factory: F) -> JoinHandle<()>
where
    F: Fn() -> R + Send + Sync + 'static,
    R: Future<Output = ()> + Send + 'static,
{
    SafeSpawner::spawn_critical(name, factory)
}

/// Task supervisor for managing multiple tasks
pub struct TaskSupervisor {
    tasks: Vec<(String, JoinHandle<Result<(), TaskError>>)>,
}

impl TaskSupervisor {
    pub fn new() -> Self {
        Self {
            tasks: Vec::new(),
        }
    }

    /// Add a task to supervise
    pub fn add_task<F>(&mut self, name: &str, future: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let handle = spawn_named(name, future);
        self.tasks.push((name.to_string(), handle));
    }

    /// Wait for all tasks to complete
    pub async fn wait_all(self) -> Vec<(String, Result<(), TaskError>)> {
        let mut results = Vec::new();

        for (name, handle) in self.tasks {
            match handle.await {
                Ok(result) => results.push((name, result)),
                Err(e) => results.push((name, Err(TaskError::JoinError(e.to_string())))),
            }
        }

        results
    }

    /// Abort all tasks
    pub fn abort_all(&mut self) {
        for (name, handle) in &self.tasks {
            log::info!("Aborting task: {}", name);
            handle.abort();
        }
        self.tasks.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_spawn_safe_success() {
        let handle = spawn_safe(async { 42 });
        let result = handle.await.unwrap();
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_spawn_safe_panic() {
        let handle = spawn_safe(async {
            panic!("Test panic");
        });

        let result = handle.await.unwrap();
        assert!(result.is_err());
        assert!(matches!(result, Err(TaskError::Panic(_))));
    }

    #[tokio::test]
    async fn test_spawn_critical_restart() {
        let counter = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let counter_clone = counter.clone();

        let handle = spawn_critical("test_critical", move || {
            let counter = counter_clone.clone();
            async move {
                let count = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if count < 2 {
                    panic!("Simulated failure");
                }
                // Success on third attempt
            }
        });

        // Give it time to retry
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        handle.abort();

        assert!(counter.load(std::sync::atomic::Ordering::SeqCst) >= 2);
    }
}