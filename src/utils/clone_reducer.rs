//! Clone reduction utilities to improve performance
//!
//! This module provides patterns and utilities to reduce unnecessary cloning
//! in the codebase, using Arc, Cow, and other zero-copy techniques.

use std::borrow::Cow;
use crate::utils::task_tracker::{spawn_tracked, TaskType};

use std::sync::Arc;
use std::rc::Rc;

/// Wrapper for expensive-to-clone types that provides cheap cloning via Arc
#[derive(Debug)]
pub struct CheapClone<T> {
    inner: Arc<T>,
}

impl<T> CheapClone<T> {
    pub fn new(value: T) -> Self {
        Self {
            inner: Arc::new(value),
        }
    }

    /// Get a reference to the inner value
    pub fn get(&self) -> &T {
        &self.inner
    }

    /// Try to unwrap the inner value if this is the only reference
    pub fn try_unwrap(self) -> Result<T, Self> {
        Arc::try_unwrap(self.inner)
            .map_err(|arc| Self { inner: arc })
    }

    /// Get a mutable reference if this is the only reference
    pub fn get_mut(&mut self) -> Option<&mut T> {
        Arc::get_mut(&mut self.inner)
    }

    /// Make a mutable copy if there are other references
    pub fn make_mut(&mut self) -> &mut T
    where
        T: Clone,
    {
        Arc::make_mut(&mut self.inner)
    }
}

impl<T> Clone for CheapClone<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<T> std::ops::Deref for CheapClone<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// String wrapper that avoids cloning for read operations
pub type CowString = Cow<'static, str>;

/// Bytes wrapper that avoids cloning for read operations
pub type CowBytes = Cow<'static, [u8]>;

/// Convert a String to a Cow that can be cheaply passed around
pub trait IntoCow {
    fn into_cow(self) -> CowString;
}

impl IntoCow for String {
    fn into_cow(self) -> CowString {
        Cow::Owned(self)
    }
}

impl IntoCow for &'static str {
    fn into_cow(self) -> CowString {
        Cow::Borrowed(self)
    }
}

/// Shared string type for frequently cloned strings
#[derive(Debug, Clone)]
pub struct SharedString(Arc<String>);

impl SharedString {
    pub fn new(s: String) -> Self {
        Self(Arc::new(s))
    }

    pub fn from_static(s: &'static str) -> Self {
        Self(Arc::new(s.to_string()))
    }
}

impl AsRef<str> for SharedString {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::ops::Deref for SharedString {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Lazy clone - only clones when actually needed
pub struct LazyClone<T> {
    value: Option<T>,
    factory: Arc<dyn Fn() -> T + Send + Sync>,
}

impl<T> LazyClone<T> {
    pub fn new<F>(factory: F) -> Self
    where
        F: Fn() -> T + Send + Sync + 'static,
    {
        Self {
            value: None,
            factory: Arc::new(factory),
        }
    }

    pub fn get_or_create(&mut self) -> &T {
        self.value.get_or_insert_with(|| (self.factory)())
    }
}

impl<T: Clone> Clone for LazyClone<T> {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
            factory: Arc::clone(&self.factory),
        }
    }
}

/// Buffer pool to avoid repeated allocations
pub struct BufferPool {
    pool: Arc<parking_lot::Mutex<Vec<Vec<u8>>>>,
    max_size: usize,
    buffer_capacity: usize,
}

impl BufferPool {
    pub fn new(max_size: usize, buffer_capacity: usize) -> Self {
        Self {
            pool: Arc::new(parking_lot::Mutex::new(Vec::with_capacity(max_size))),
            max_size,
            buffer_capacity,
        }
    }

    /// Get a buffer from the pool or create a new one
    pub fn acquire(&self) -> PooledBuffer {
        let mut pool = self.pool.lock();
        let buffer = pool.pop().unwrap_or_else(|| Vec::with_capacity(self.buffer_capacity));
        PooledBuffer {
            buffer: Some(buffer),
            pool: Arc::clone(&self.pool),
            max_size: self.max_size,
        }
    }
}

/// A buffer that returns to the pool when dropped
pub struct PooledBuffer {
    buffer: Option<Vec<u8>>,
    pool: Arc<parking_lot::Mutex<Vec<Vec<u8>>>>,
    max_size: usize,
}

impl PooledBuffer {
    pub fn as_mut(&mut self) -> &mut Vec<u8> {
        self.buffer.as_mut().unwrap()
    }
}

impl Drop for PooledBuffer {
    fn drop(&mut self) {
        if let Some(mut buffer) = self.buffer.take() {
            buffer.clear(); // Clear contents but keep capacity
            let mut pool = self.pool.lock();
            if pool.len() < self.max_size {
                pool.push(buffer);
            }
        }
    }
}

impl std::ops::Deref for PooledBuffer {
    type Target = Vec<u8>;

    fn deref(&self) -> &Self::Target {
        self.buffer.as_ref().unwrap()
    }
}

impl std::ops::DerefMut for PooledBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.buffer.as_mut().unwrap()
    }
}

/// Macro to convert clone-heavy code to use Arc
#[macro_export]
macro_rules! arc_wrap {
    ($value:expr) => {
        Arc::new($value)
    };
}

/// Macro to create a cheap clone wrapper
#[macro_export]
macro_rules! cheap_clone {
    ($value:expr) => {
        CheapClone::new($value)
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cheap_clone() {
        #[derive(Debug, Clone)]
        struct ExpensiveData {
            data: Vec<u8>,
        }

        let expensive = ExpensiveData {
            data: vec![0; 1000000], // 1MB of data
        };

        let wrapped = CheapClone::new(expensive);
        let clone1 = wrapped.clone(); // Cheap!
        let clone2 = wrapped.clone(); // Cheap!

        assert_eq!(clone1.data.len(), 1000000);
        assert_eq!(clone2.data.len(), 1000000);
    }

    #[test]
    fn test_buffer_pool() {
        let pool = BufferPool::new(10, 1024);

        let mut buffer1 = pool.acquire();
        buffer1.extend_from_slice(b"Hello");
        assert_eq!(&buffer1[..], b"Hello");

        drop(buffer1); // Returns to pool

        let buffer2 = pool.acquire(); // Reuses the buffer
        assert_eq!(buffer2.capacity(), 1024); // Capacity preserved
    }
}