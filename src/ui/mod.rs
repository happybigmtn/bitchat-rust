//! User Interface module for BitCraps
//! 
//! This module provides the complete user interface for BitCraps including:
//! - CLI interface using clap for command-line operations
//! - TUI interface using ratatui for terminal-based interaction (work in progress)
//! - Specialized casino widgets and components (work in progress)
//! - Application state management and event handling

pub mod simple;

// Complex UI modules (re-enabled for comprehensive debugging)
pub mod cli;
pub mod tui;
pub mod app;

// Export the working simple UI for now
pub use simple::*;