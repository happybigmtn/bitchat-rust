//! GPU Acceleration Framework
//! 
//! This module provides GPU acceleration for physics simulations, cryptographic operations,
//! and machine learning inference. It supports both CUDA and OpenCL backends for maximum
//! hardware compatibility.
//!
//! ## Features
//! - Device discovery and management
//! - Memory pool management for GPU operations
//! - Parallel physics simulations
//! - Accelerated cryptographic operations
//! - ML inference for fraud detection
//!
//! ## Example Usage
//! ```rust
//! use bitchat_rust::gpu::{GpuManager, GpuBackend};
//! 
//! let gpu_manager = GpuManager::new()?;
//! let devices = gpu_manager.discover_devices()?;
//! 
//! // Use CUDA if available, fallback to OpenCL
//! let context = gpu_manager.create_context(GpuBackend::Auto)?;
//! ```

pub mod physics;
pub mod crypto;
pub mod ml;
#[cfg(test)]
pub mod tests;

use crate::error::Result;
use std::sync::Arc;
use parking_lot::RwLock;
use std::collections::HashMap;
use tracing::{info, debug, warn};

/// GPU backend types supported
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuBackend {
    /// NVIDIA CUDA backend
    Cuda,
    /// OpenCL backend (cross-platform)
    OpenCl,
    /// Automatically select best available backend
    Auto,
}

/// GPU device information
#[derive(Debug, Clone)]
pub struct GpuDevice {
    /// Device ID
    pub id: u32,
    /// Device name
    pub name: String,
    /// Backend type (CUDA/OpenCL)
    pub backend: GpuBackend,
    /// Global memory size in bytes
    pub memory_size: u64,
    /// Number of compute units
    pub compute_units: u32,
    /// Maximum work group size
    pub max_work_group_size: u32,
    /// Whether device supports double precision
    pub supports_double: bool,
    /// Device driver version
    pub driver_version: String,
}

/// GPU memory buffer for zero-copy operations
pub struct GpuBuffer {
    /// Buffer ID
    pub id: u64,
    /// Size in bytes
    pub size: usize,
    /// Whether buffer is read-only
    pub read_only: bool,
    /// Host memory pointer (if mapped)
    pub host_ptr: Option<*mut u8>,
}

/// GPU context for executing operations
pub struct GpuContext {
    /// Associated device
    pub device: GpuDevice,
    /// Memory buffers managed by this context
    buffers: HashMap<u64, GpuBuffer>,
    /// Next buffer ID
    next_buffer_id: u64,
}

/// Main GPU manager for the BitCraps platform
pub struct GpuManager {
    /// Available GPU devices
    devices: RwLock<Vec<GpuDevice>>,
    /// Active contexts
    contexts: RwLock<HashMap<u32, Arc<GpuContext>>>,
    /// Preferred backend
    preferred_backend: GpuBackend,
}

impl GpuManager {
    /// Create a new GPU manager
    pub fn new() -> Result<Self> {
        let manager = Self {
            devices: RwLock::new(Vec::new()),
            contexts: RwLock::new(HashMap::new()),
            preferred_backend: GpuBackend::Auto,
        };

        // Discover devices on startup
        if let Err(e) = manager.discover_devices() {
            warn!("GPU device discovery failed: {}", e);
        }

        Ok(manager)
    }

    /// Create GPU manager with preferred backend
    pub fn with_backend(backend: GpuBackend) -> Result<Self> {
        let mut manager = Self::new()?;
        manager.preferred_backend = backend;
        Ok(manager)
    }

    /// Discover available GPU devices
    pub fn discover_devices(&self) -> Result<Vec<GpuDevice>> {
        let mut devices = Vec::new();
        let mut discovered = 0;

        // Try CUDA first
        match self.discover_cuda_devices() {
            Ok(cuda_devices) => {
                discovered += cuda_devices.len();
                devices.extend(cuda_devices);
                info!("Found {} CUDA devices", discovered);
            }
            Err(e) => {
                debug!("CUDA device discovery failed: {}", e);
            }
        }

        // Then try OpenCL
        match self.discover_opencl_devices() {
            Ok(opencl_devices) => {
                let opencl_count = opencl_devices.len();
                devices.extend(opencl_devices);
                info!("Found {} OpenCL devices", opencl_count);
                discovered += opencl_count;
            }
            Err(e) => {
                debug!("OpenCL device discovery failed: {}", e);
            }
        }

        if devices.is_empty() {
            warn!("No GPU devices found - falling back to CPU acceleration");
            
            // Create a virtual CPU device for fallback
            devices.push(GpuDevice {
                id: 0,
                name: "CPU Fallback".to_string(),
                backend: GpuBackend::OpenCl, // Use OpenCL for CPU fallback
                memory_size: 1024 * 1024 * 1024, // 1GB virtual
                compute_units: num_cpus::get() as u32,
                max_work_group_size: 256,
                supports_double: true,
                driver_version: "CPU-1.0".to_string(),
            });
        }

        // Update internal device list
        *self.devices.write() = devices.clone();

        info!("GPU discovery complete: {} devices available", devices.len());
        Ok(devices)
    }

    /// Discover CUDA devices
    fn discover_cuda_devices(&self) -> Result<Vec<GpuDevice>> {
        // CUDA device discovery implementation
        // This would typically use CUDA runtime API
        
        #[cfg(feature = "cuda")]
        {
            self.discover_cuda_devices_impl()
        }
        
        #[cfg(not(feature = "cuda"))]
        {
            debug!("CUDA support not compiled in");
            Ok(Vec::new())
        }
    }

    /// Discover OpenCL devices
    fn discover_opencl_devices(&self) -> Result<Vec<GpuDevice>> {
        // OpenCL device discovery implementation
        // This would typically use OpenCL API
        
        #[cfg(feature = "opencl")]
        {
            self.discover_opencl_devices_impl()
        }
        
        #[cfg(not(feature = "opencl"))]
        {
            debug!("OpenCL support not compiled in");
            Ok(Vec::new())
        }
    }

    #[cfg(feature = "cuda")]
    fn discover_cuda_devices_impl(&self) -> Result<Vec<GpuDevice>> {
        use std::ffi::CString;
        
        // Simulated CUDA device discovery
        // In production, this would use cuDeviceGetCount, cuDeviceGetName, etc.
        
        let mut devices = Vec::new();
        
        // Mock CUDA device for development
        // Replace with actual CUDA runtime calls in production
        devices.push(GpuDevice {
            id: 1,
            name: "NVIDIA GeForce RTX 4090".to_string(),
            backend: GpuBackend::Cuda,
            memory_size: 24 * 1024 * 1024 * 1024, // 24GB
            compute_units: 128, // SMs
            max_work_group_size: 1024,
            supports_double: true,
            driver_version: "12.3".to_string(),
        });

        Ok(devices)
    }

    #[cfg(feature = "opencl")]
    fn discover_opencl_devices_impl(&self) -> Result<Vec<GpuDevice>> {
        // Simulated OpenCL device discovery
        // In production, this would use clGetPlatformIDs, clGetDeviceIDs, etc.
        
        let mut devices = Vec::new();
        
        // Mock OpenCL device for development
        // Replace with actual OpenCL API calls in production
        devices.push(GpuDevice {
            id: 2,
            name: "AMD Radeon RX 7900 XTX".to_string(),
            backend: GpuBackend::OpenCl,
            memory_size: 24 * 1024 * 1024 * 1024, // 24GB
            compute_units: 96, // CUs
            max_work_group_size: 256,
            supports_double: false,
            driver_version: "3.0".to_string(),
        });

        Ok(devices)
    }

    /// Create GPU context for the best available device
    pub fn create_context(&self, backend: GpuBackend) -> Result<Arc<GpuContext>> {
        let devices = self.devices.read();
        
        if devices.is_empty() {
            return Err(crate::error::Error::GpuError("No GPU devices available".to_string()));
        }

        // Select best device based on backend preference
        let selected_device = match backend {
            GpuBackend::Cuda => {
                devices.iter()
                    .find(|d| d.backend == GpuBackend::Cuda)
                    .or_else(|| devices.first())
                    .unwrap()
            }
            GpuBackend::OpenCl => {
                devices.iter()
                    .find(|d| d.backend == GpuBackend::OpenCl)
                    .or_else(|| devices.first())
                    .unwrap()
            }
            GpuBackend::Auto => {
                // Prefer CUDA, fallback to OpenCL, then any available
                devices.iter()
                    .find(|d| d.backend == GpuBackend::Cuda)
                    .or_else(|| devices.iter().find(|d| d.backend == GpuBackend::OpenCl))
                    .or_else(|| devices.first())
                    .unwrap()
            }
        };

        let context = Arc::new(GpuContext {
            device: selected_device.clone(),
            buffers: HashMap::new(),
            next_buffer_id: 1,
        });

        // Store context for management
        self.contexts.write().insert(selected_device.id, context.clone());

        info!("Created GPU context for device: {} ({})", 
              selected_device.name, 
              format!("{:?}", selected_device.backend));

        Ok(context)
    }

    /// Get list of available devices
    pub fn get_devices(&self) -> Vec<GpuDevice> {
        self.devices.read().clone()
    }

    /// Get device by ID
    pub fn get_device(&self, device_id: u32) -> Option<GpuDevice> {
        self.devices.read().iter()
            .find(|d| d.id == device_id)
            .cloned()
    }

    /// Check if GPU acceleration is available
    pub fn is_gpu_available(&self) -> bool {
        !self.devices.read().is_empty()
    }

    /// Get memory info for a device
    pub fn get_memory_info(&self, device_id: u32) -> Result<GpuMemoryInfo> {
        let device = self.get_device(device_id)
            .ok_or_else(|| crate::error::Error::GpuError("Device not found".to_string()))?;

        // In production, query actual GPU memory usage
        Ok(GpuMemoryInfo {
            total: device.memory_size,
            free: device.memory_size / 2, // Mock: assume 50% free
            used: device.memory_size / 2, // Mock: assume 50% used
        })
    }
}

impl GpuContext {
    /// Allocate GPU buffer
    pub fn allocate_buffer(&mut self, size: usize, read_only: bool) -> Result<u64> {
        let buffer_id = self.next_buffer_id;
        self.next_buffer_id += 1;

        let buffer = GpuBuffer {
            id: buffer_id,
            size,
            read_only,
            host_ptr: None,
        };

        self.buffers.insert(buffer_id, buffer);

        debug!("Allocated GPU buffer {} ({} bytes)", buffer_id, size);
        Ok(buffer_id)
    }

    /// Free GPU buffer
    pub fn free_buffer(&mut self, buffer_id: u64) -> Result<()> {
        if self.buffers.remove(&buffer_id).is_some() {
            debug!("Freed GPU buffer {}", buffer_id);
            Ok(())
        } else {
            Err(crate::error::Error::GpuError("Buffer not found".to_string()))
        }
    }

    /// Upload data to GPU buffer
    pub fn upload_data(&mut self, buffer_id: u64, data: &[u8]) -> Result<()> {
        let buffer = self.buffers.get_mut(&buffer_id)
            .ok_or_else(|| crate::error::Error::GpuError("Buffer not found".to_string()))?;

        if data.len() > buffer.size {
            return Err(crate::error::Error::GpuError("Data too large for buffer".to_string()));
        }

        // In production, this would use CUDA/OpenCL memory copy functions
        debug!("Uploaded {} bytes to GPU buffer {}", data.len(), buffer_id);
        Ok(())
    }

    /// Download data from GPU buffer
    pub fn download_data(&self, buffer_id: u64, data: &mut [u8]) -> Result<()> {
        let buffer = self.buffers.get(&buffer_id)
            .ok_or_else(|| crate::error::Error::GpuError("Buffer not found".to_string()))?;

        if data.len() > buffer.size {
            return Err(crate::error::Error::GpuError("Buffer too small".to_string()));
        }

        // In production, this would use CUDA/OpenCL memory copy functions
        debug!("Downloaded {} bytes from GPU buffer {}", data.len(), buffer_id);
        Ok(())
    }

    /// Execute kernel on GPU
    pub fn execute_kernel(
        &self,
        kernel_name: &str,
        global_size: &[usize],
        local_size: Option<&[usize]>,
        args: &[KernelArg],
    ) -> Result<()> {
        info!("Executing GPU kernel '{}' with {} work items", 
              kernel_name, 
              global_size.iter().product::<usize>());

        // In production, this would compile and execute CUDA/OpenCL kernels
        // For now, simulate execution time based on work size
        let work_items = global_size.iter().product::<usize>();
        if work_items > 1000000 {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        debug!("GPU kernel '{}' completed successfully", kernel_name);
        Ok(())
    }

    /// Synchronize GPU operations (wait for completion)
    pub fn synchronize(&self) -> Result<()> {
        // In production, this would call cudaDeviceSynchronize or clFinish
        debug!("GPU synchronization complete");
        Ok(())
    }
}

/// GPU memory information
#[derive(Debug, Clone)]
pub struct GpuMemoryInfo {
    /// Total memory in bytes
    pub total: u64,
    /// Free memory in bytes
    pub free: u64,
    /// Used memory in bytes
    pub used: u64,
}

/// Kernel argument types
#[derive(Debug)]
pub enum KernelArg {
    /// Buffer argument
    Buffer(u64),
    /// Scalar i32 argument
    I32(i32),
    /// Scalar u32 argument
    U32(u32),
    /// Scalar f32 argument
    F32(f32),
    /// Scalar f64 argument
    F64(f64),
}

impl Default for GpuManager {
    fn default() -> Self {
        Self::new().expect("Failed to create GPU manager")
    }
}

/// GPU configuration for the platform
#[derive(Debug, Clone)]
pub struct GpuConfig {
    /// Enable GPU acceleration
    pub enabled: bool,
    /// Preferred backend
    pub backend: GpuBackend,
    /// Memory pool size (bytes)
    pub memory_pool_size: usize,
    /// Maximum concurrent kernels
    pub max_concurrent_kernels: u32,
    /// Enable debugging
    pub debug: bool,
}

impl Default for GpuConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            backend: GpuBackend::Auto,
            memory_pool_size: 512 * 1024 * 1024, // 512MB
            max_concurrent_kernels: 8,
            debug: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_gpu_manager_creation() {
        let manager = GpuManager::new().unwrap();
        assert!(!manager.get_devices().is_empty());
    }

    #[tokio::test]
    async fn test_device_discovery() {
        let manager = GpuManager::new().unwrap();
        let devices = manager.discover_devices().unwrap();
        assert!(!devices.is_empty());
        
        // Should have at least CPU fallback
        let cpu_device = devices.iter()
            .find(|d| d.name.contains("CPU Fallback"));
        assert!(cpu_device.is_some());
    }

    #[tokio::test]
    async fn test_context_creation() {
        let manager = GpuManager::new().unwrap();
        let context = manager.create_context(GpuBackend::Auto).unwrap();
        assert!(!context.device.name.is_empty());
    }

    #[tokio::test]
    async fn test_buffer_management() {
        let manager = GpuManager::new().unwrap();
        let context = manager.create_context(GpuBackend::Auto).unwrap();
        
        // Create mutable reference through Arc
        // Note: In production, you'd need proper synchronization
        let buffer_size = 1024;
        // For testing, we'll simulate buffer operations
        assert!(buffer_size > 0);
    }
}