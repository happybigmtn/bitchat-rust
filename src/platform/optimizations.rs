//! Platform-specific optimizations for maximum performance

use crate::error::Result;

/// Platform optimizer facade
pub struct PlatformOptimizer {
    thread_count: usize,
}

impl PlatformOptimizer {
    pub fn new() -> Result<Self> {
        Ok(Self {
            thread_count: num_cpus::get(),
        })
    }

    pub fn optimize_for_platform(&self) -> Result<()> {
        // Set rayon thread pool size
        rayon::ThreadPoolBuilder::new()
            .num_threads(self.thread_count)
            .build_global()
            .ok();

        Ok(())
    }

    pub fn get_thread_count(&self) -> usize {
        self.thread_count
    }
}

/// Cross-platform SIMD optimizations
pub mod simd {

    /// Optimized memory copy using standard library
    pub unsafe fn simd_memcpy(dst: *mut u8, src: *const u8, len: usize) {
        std::ptr::copy_nonoverlapping(src, dst, len);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_optimizer_creation() {
        let optimizer = PlatformOptimizer::new();
        assert!(optimizer.is_ok());
    }

    #[test]
    fn test_simd_memcpy() {
        let src = vec![1u8; 1024];
        let mut dst = vec![0u8; 1024];

        unsafe {
            simd::simd_memcpy(dst.as_mut_ptr(), src.as_ptr(), 1024);
        }

        assert_eq!(src, dst);
    }
}
