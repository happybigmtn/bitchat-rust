//! iOS-specific mobile implementation
//! 
//! This module provides the iOS-specific implementation for CoreBluetooth
//! integration, state management, and FFI bridges to Swift/Objective-C.

pub mod ble_peripheral;
pub mod ffi;
pub mod state_manager;
pub mod memory_bridge;

// Re-export the main types
pub use ble_peripheral::*;
pub use ffi::*;
pub use state_manager::*;
pub use memory_bridge::*;