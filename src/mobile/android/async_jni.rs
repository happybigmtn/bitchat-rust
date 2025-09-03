//! Async JNI Helper Module
//!
//! This module provides utilities for handling async operations in JNI without blocking
//! the Android UI thread, preventing ANR (Application Not Responding) errors.
//!
//! # ANR Prevention Strategy
//!
//! Android apps must respond to user interaction within 5 seconds or the system will
//! show an ANR dialog. JNI calls come from the UI thread, so any blocking operations
//! will cause ANRs.
//!
//! ## Solution Patterns:
//!
//! 1. **Immediate Return with Async Processing**: Start async operation and return immediately
//! 2. **Polling Pattern**: Android polls for completion using separate JNI calls
//! 3. **Callback Pattern**: Use JNI callbacks to notify Android when operations complete
//! 4. **Timeout Protection**: All async operations have maximum 5-second timeouts
//!
//! ## Usage Example:
//!
//! ```rust
//! // Instead of blocking:
//! // let result = rt.block_on(async_operation());
//!
//! // Use async pattern:
//! let handle = AsyncJNIManager::start_operation(async_operation());
//! // Return immediately to Android
//! // Android polls with: AsyncJNIManager::check_completion(handle)
//! ```

use crate::error::BitCrapsError;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::oneshot;
use tokio::time::timeout;

/// Handle for tracking async operations
pub type AsyncHandle = u64;

/// Result of an async operation
pub enum AsyncResult<T> {
    Pending,
    Complete(Result<T, BitCrapsError>),
    TimedOut,
}

/// Manager for async JNI operations
pub struct AsyncJNIManager<T> {
    operations: Arc<Mutex<HashMap<AsyncHandle, oneshot::Receiver<Result<T, BitCrapsError>>>>>,
    next_handle: Arc<Mutex<AsyncHandle>>,
}

impl<T> AsyncJNIManager<T> {
    pub fn new() -> Self {
        Self {
            operations: Arc::new(Mutex::new(HashMap::new())),
            next_handle: Arc::new(Mutex::new(1)),
        }
    }

    /// Start an async operation and return a handle immediately
    pub fn start_operation<F, Fut>(&self, rt: &tokio::runtime::Runtime, operation: F) -> AsyncHandle
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<T, BitCrapsError>> + Send + 'static,
        T: Send + 'static,
    {
        let handle = {
            let mut next = self.next_handle.lock().unwrap();
            let current = *next;
            *next += 1;
            current
        };

        let (tx, rx) = oneshot::channel();

        // Store receiver for polling
        {
            let mut ops = self.operations.lock().unwrap();
            ops.insert(handle, rx);
        }

        // Start async operation with timeout
        rt.spawn(async move {
            let result = timeout(Duration::from_secs(5), operation()).await;
            let final_result = match result {
                Ok(ok_result) => ok_result,
                Err(_) => Err(BitCrapsError::Timeout),
            };
            let _ = tx.send(final_result);
        });

        handle
    }

    /// Check if an async operation is complete
    pub fn check_completion(&self, handle: AsyncHandle) -> AsyncResult<T> {
        let mut ops = match self.operations.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                log::error!("Operations mutex poisoned in check_completion, recovering");
                poisoned.into_inner()
            }
        };

        if let Some(mut rx) = ops.remove(&handle) {
            match rx.try_recv() {
                Ok(result) => AsyncResult::Complete(result),
                Err(oneshot::error::TryRecvError::Empty) => {
                    // Put it back and return pending
                    ops.insert(handle, rx);
                    AsyncResult::Pending
                }
                Err(oneshot::error::TryRecvError::Closed) => AsyncResult::TimedOut,
            }
        } else {
            AsyncResult::TimedOut // Handle not found or already consumed
        }
    }

    /// Remove a completed or timed out operation
    pub fn cleanup_operation(&self, handle: AsyncHandle) {
        let mut ops = match self.operations.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                log::error!("Operations mutex poisoned in cleanup_operation, recovering");
                poisoned.into_inner()
            }
        };
        ops.remove(&handle);
    }
}

/// Macro for creating non-blocking JNI functions
#[macro_export]
macro_rules! async_jni_fn {
    ($fn_name:ident, $operation:expr, $manager:expr, $rt:expr) => {
        #[no_mangle]
        pub extern "C" fn $fn_name(
            env: jni::JNIEnv,
            _class: jni::objects::JClass,
        ) -> jni::sys::jlong {
            let handle = $manager.start_operation(&$rt, || $operation);
            handle as jni::sys::jlong
        }
    };
}

/// Global async managers for different operation types
use once_cell::sync::Lazy;

pub static BLE_ASYNC_MANAGER: Lazy<AsyncJNIManager<()>> = Lazy::new(|| AsyncJNIManager::new());

pub static GAME_ASYNC_MANAGER: Lazy<AsyncJNIManager<String>> = Lazy::new(|| AsyncJNIManager::new());

/// Helper function to convert AsyncResult to JNI boolean
pub fn async_result_to_jboolean<T>(result: AsyncResult<T>) -> jni::sys::jboolean {
    match result {
        AsyncResult::Complete(Ok(_)) => true as jni::sys::jboolean,
        _ => false as jni::sys::jboolean,
    }
}

/// Helper function to convert AsyncResult to JNI string
pub fn async_result_to_jstring<T>(
    env: &jni::JNIEnv,
    result: AsyncResult<T>,
    success_message: &str,
) -> jni::sys::jstring
where
    T: std::fmt::Debug,
{
    match result {
        AsyncResult::Complete(Ok(_)) => match env.new_string(success_message) {
            Ok(jstr) => jstr.into_raw(),
            Err(_) => std::ptr::null_mut(),
        },
        AsyncResult::Complete(Err(e)) => match env.new_string(format!("Error: {}", e)) {
            Ok(jstr) => jstr.into_raw(),
            Err(_) => std::ptr::null_mut(),
        },
        AsyncResult::Pending => match env.new_string("PENDING") {
            Ok(jstr) => jstr.into_raw(),
            Err(_) => std::ptr::null_mut(),
        },
        AsyncResult::TimedOut => match env.new_string("TIMEOUT") {
            Ok(jstr) => jstr.into_raw(),
            Err(_) => std::ptr::null_mut(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_async_manager() {
        let manager = AsyncJNIManager::new();
        let rt = tokio::runtime::Runtime::new().unwrap();

        // Start a quick operation
        let handle = manager.start_operation(&rt, || async {
            sleep(Duration::from_millis(10)).await;
            Ok::<(), BitCrapsError>(())
        });

        // Should be pending initially
        matches!(manager.check_completion(handle), AsyncResult::Pending);

        // Wait a bit and check again
        sleep(Duration::from_millis(20)).await;
        matches!(
            manager.check_completion(handle),
            AsyncResult::Complete(Ok(()))
        );
    }

    #[tokio::test]
    async fn test_timeout() {
        let manager = AsyncJNIManager::new();
        let rt = tokio::runtime::Runtime::new().unwrap();

        // Start an operation that takes too long
        let handle = manager.start_operation(&rt, || async {
            sleep(Duration::from_secs(10)).await; // Longer than 5s timeout
            Ok::<(), BitCrapsError>(())
        });

        // Wait for timeout
        sleep(Duration::from_secs(6)).await;
        matches!(
            manager.check_completion(handle),
            AsyncResult::Complete(Err(BitCrapsError::Timeout))
        );
    }
}
