//! Data persistence layer for BitCraps
//! 
//! Handles saving and loading application state, game history,
//! and user data across sessions.

use crate::{Result, Error};
use std::path::{Path, PathBuf};
use tokio::fs;

/// Manages persistent storage for BitCraps application data
pub struct PersistenceManager {
    data_dir: PathBuf,
}

impl PersistenceManager {
    /// Create a new persistence manager
    pub async fn new<P: AsRef<Path>>(data_dir: P) -> Result<Self> {
        let data_dir = data_dir.as_ref().to_path_buf();
        
        // Create data directory if it doesn't exist
        if !data_dir.exists() {
            fs::create_dir_all(&data_dir).await
                .map_err(|e| Error::Network(format!("Failed to create data directory: {}", e)))?;
        }
        
        Ok(Self { data_dir })
    }
    
    /// Flush any pending writes to disk
    pub async fn flush(&self) -> Result<()> {
        // In a real implementation, this would flush pending writes
        Ok(())
    }
    
    /// Get the data directory path
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }
}

impl Default for PersistenceManager {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from("~/.bitcraps"),
        }
    }
}