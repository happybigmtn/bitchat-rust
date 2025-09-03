//! Lock ordering enforcement to prevent deadlocks
//!
//! This module provides a consistent lock ordering mechanism to prevent
//! deadlocks in concurrent code. All locks must be acquired in a predefined
//! order to avoid circular dependencies.

use std::sync::{Arc, Mutex, RwLock};
use crate::utils::task_tracker::{spawn_tracked, TaskType};

use std::collections::HashMap;
use std::thread::ThreadId;
use once_cell::sync::Lazy;

/// Lock priority levels - lower numbers must be acquired first
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LockOrder {
    /// Highest priority - configuration and global state
    Config = 1,
    /// User/identity management
    Identity = 2,
    /// Network transport layer
    Transport = 3,
    /// Message routing and mesh
    Mesh = 4,
    /// Game state and consensus
    GameState = 5,
    /// Token/economics management
    Treasury = 6,
    /// Session management
    Session = 7,
    /// Database operations
    Database = 8,
    /// Monitoring and metrics
    Monitoring = 9,
    /// Lowest priority - caches and buffers
    Cache = 10,
}

/// Thread-local storage for tracking held locks
thread_local! {
    static HELD_LOCKS: std::cell::RefCell<Vec<LockOrder>> = std::cell::RefCell::new(Vec::new());
}

/// Global deadlock detector (only enabled in debug mode)
#[cfg(debug_assertions)]
static DEADLOCK_DETECTOR: Lazy<Arc<Mutex<DeadlockDetector>>> = Lazy::new(|| {
    Arc::new(Mutex::new(DeadlockDetector::new()))
});

#[cfg(debug_assertions)]
struct DeadlockDetector {
    // Maps thread ID to currently held locks
    thread_locks: HashMap<ThreadId, Vec<LockOrder>>,
}

#[cfg(debug_assertions)]
impl DeadlockDetector {
    fn new() -> Self {
        Self {
            thread_locks: HashMap::new(),
        }
    }

    fn check_lock_order(&mut self, thread_id: ThreadId, new_lock: LockOrder, held_locks: &[LockOrder]) {
        // Check if acquiring new_lock would violate ordering
        for held in held_locks {
            if *held > new_lock {
                panic!(
                    "Potential deadlock detected! Thread {:?} attempting to acquire {:?} while holding {:?}. \
                     Locks must be acquired in order: Config < Identity < Transport < Mesh < GameState < Treasury < Session < Database < Monitoring < Cache",
                    thread_id, new_lock, held
                );
            }
        }

        // Update global state
        self.thread_locks.insert(thread_id, held_locks.to_vec());
    }

    fn release_lock(&mut self, thread_id: ThreadId, lock: LockOrder) {
        if let Some(locks) = self.thread_locks.get_mut(&thread_id) {
            locks.retain(|&l| l != lock);
            if locks.is_empty() {
                self.thread_locks.remove(&thread_id);
            }
        }
    }
}

/// Acquire a lock with deadlock detection
pub fn acquire_lock(order: LockOrder) {
    HELD_LOCKS.with(|locks| {
        let mut held = locks.borrow_mut();

        // Check ordering
        #[cfg(debug_assertions)]
        {
            let thread_id = std::thread::current().id();
            let mut detector = DEADLOCK_DETECTOR.lock().unwrap();
            detector.check_lock_order(thread_id, order, &held);
        }

        // In production, just check locally
        #[cfg(not(debug_assertions))]
        {
            for held_lock in held.iter() {
                if *held_lock > order {
                    // Log warning but don't panic in production
                    tracing::warn!(
                        "Lock ordering violation: acquiring {:?} while holding {:?}",
                        order, held_lock
                    );
                }
            }
        }

        held.push(order);
    });
}

/// Release a lock
pub fn release_lock(order: LockOrder) {
    HELD_LOCKS.with(|locks| {
        let mut held = locks.borrow_mut();
        held.retain(|&l| l != order);

        #[cfg(debug_assertions)]
        {
            let thread_id = std::thread::current().id();
            let mut detector = DEADLOCK_DETECTOR.lock().unwrap();
            detector.release_lock(thread_id, order);
        }
    });
}

/// Ordered mutex wrapper that enforces lock ordering
pub struct OrderedMutex<T> {
    inner: Mutex<T>,
    order: LockOrder,
}

impl<T> OrderedMutex<T> {
    pub fn new(value: T, order: LockOrder) -> Self {
        Self {
            inner: Mutex::new(value),
            order,
        }
    }

    pub fn lock(&self) -> Result<OrderedMutexGuard<T>, std::sync::PoisonError<std::sync::MutexGuard<T>>> {
        acquire_lock(self.order);
        self.inner.lock().map(|guard| OrderedMutexGuard {
            guard: Some(guard),
            order: self.order,
        })
    }
}

/// Guard that releases the lock order tracking on drop
pub struct OrderedMutexGuard<'a, T> {
    guard: Option<std::sync::MutexGuard<'a, T>>,
    order: LockOrder,
}

impl<'a, T> Drop for OrderedMutexGuard<'a, T> {
    fn drop(&mut self) {
        self.guard = None; // Drop the actual guard first
        release_lock(self.order);
    }
}

impl<'a, T> std::ops::Deref for OrderedMutexGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.guard.as_ref().unwrap()
    }
}

impl<'a, T> std::ops::DerefMut for OrderedMutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.guard.as_mut().unwrap()
    }
}

/// Ordered RwLock wrapper
pub struct OrderedRwLock<T> {
    inner: RwLock<T>,
    order: LockOrder,
}

impl<T> OrderedRwLock<T> {
    pub fn new(value: T, order: LockOrder) -> Self {
        Self {
            inner: RwLock::new(value),
            order,
        }
    }

    pub fn read(&self) -> Result<OrderedRwLockReadGuard<T>, std::sync::PoisonError<std::sync::RwLockReadGuard<T>>> {
        acquire_lock(self.order);
        self.inner.read().map(|guard| OrderedRwLockReadGuard {
            guard: Some(guard),
            order: self.order,
        })
    }

    pub fn write(&self) -> Result<OrderedRwLockWriteGuard<T>, std::sync::PoisonError<std::sync::RwLockWriteGuard<T>>> {
        acquire_lock(self.order);
        self.inner.write().map(|guard| OrderedRwLockWriteGuard {
            guard: Some(guard),
            order: self.order,
        })
    }
}

pub struct OrderedRwLockReadGuard<'a, T> {
    guard: Option<std::sync::RwLockReadGuard<'a, T>>,
    order: LockOrder,
}

impl<'a, T> Drop for OrderedRwLockReadGuard<'a, T> {
    fn drop(&mut self) {
        self.guard = None;
        release_lock(self.order);
    }
}

impl<'a, T> std::ops::Deref for OrderedRwLockReadGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.guard.as_ref().unwrap()
    }
}

pub struct OrderedRwLockWriteGuard<'a, T> {
    guard: Option<std::sync::RwLockWriteGuard<'a, T>>,
    order: LockOrder,
}

impl<'a, T> Drop for OrderedRwLockWriteGuard<'a, T> {
    fn drop(&mut self) {
        self.guard = None;
        release_lock(self.order);
    }
}

impl<'a, T> std::ops::Deref for OrderedRwLockWriteGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.guard.as_ref().unwrap()
    }
}

impl<'a, T> std::ops::DerefMut for OrderedRwLockWriteGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.guard.as_mut().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_lock_ordering() {
        let config = Arc::new(OrderedMutex::new(1, LockOrder::Config));
        let transport = Arc::new(OrderedMutex::new(2, LockOrder::Transport));

        // This should work - acquiring in order
        {
            let _c = config.lock().unwrap();
            let _t = transport.lock().unwrap();
        }
    }

    #[test]
    #[should_panic(expected = "Potential deadlock detected")]
    #[cfg(debug_assertions)]
    fn test_lock_ordering_violation() {
        let config = Arc::new(OrderedMutex::new(1, LockOrder::Config));
        let transport = Arc::new(OrderedMutex::new(2, LockOrder::Transport));

        // This should panic - acquiring out of order
        {
            let _t = transport.lock().unwrap();
            let _c = config.lock().unwrap(); // Panic here!
        }
    }
}