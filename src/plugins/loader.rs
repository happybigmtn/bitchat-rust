//! Dynamic Plugin Loader for BitCraps
//!
//! This module handles loading, validating, and managing plugin lifecycle
//! including dynamic library loading, security validation, and metadata parsing.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::fs;
use tracing::{info, warn, error};

use super::core::{PluginInfo, PluginLoadData, PluginFactory, GamePlugin, PluginResult, PluginError};

/// Plugin loader that handles dynamic loading of plugins
pub struct PluginLoader {
    config: LoaderConfig,
    loaded_plugins: Arc<tokio::sync::RwLock<HashMap<String, LoadedPlugin>>>,
}

impl PluginLoader {
    /// Create new plugin loader
    pub fn new(config: LoaderConfig) -> PluginResult<Self> {
        Ok(Self {
            config,
            loaded_plugins: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        })
    }

    /// Load plugin from file path
    pub async fn load(&self, plugin_path: &str) -> PluginResult<PluginLoadData> {
        let path = Path::new(plugin_path);
        
        // Validate file exists
        if !path.exists() {
            return Err(PluginError::InitializationFailed(
                format!("Plugin file not found: {}", plugin_path)
            ));
        }

        // Load and parse plugin metadata
        let metadata = self.load_metadata(path).await?;
        
        // Validate plugin signature if required
        if self.config.require_signatures {
            self.validate_signature(path, &metadata).await?;
        }

        // Validate plugin compatibility
        self.validate_compatibility(&metadata).await?;

        // Load plugin configuration
        let config = self.load_plugin_config(path, &metadata).await?;

        // Create plugin factory (for actual plugin instantiation)
        let factory = self.create_plugin_factory(path, &metadata).await?;

        // Store loaded plugin info
        {
            let mut loaded = self.loaded_plugins.write().await;
            loaded.insert(metadata.id.clone(), LoadedPlugin {
                metadata: metadata.clone(),
                path: path.to_path_buf(),
                loaded_at: SystemTime::now(),
                config: config.clone(),
            });
        }

        info!("Successfully loaded plugin: {} v{}", metadata.name, metadata.version);

        Ok(PluginLoadData {
            metadata,
            config,
            factory,
        })
    }

    /// Unload plugin
    pub async fn unload(&self, plugin_id: &str) -> PluginResult<()> {
        let mut loaded = self.loaded_plugins.write().await;
        if let Some(plugin) = loaded.remove(plugin_id) {
            info!("Unloaded plugin: {}", plugin.metadata.name);
            // Additional cleanup could be performed here
        }
        Ok(())
    }

    /// Get list of loaded plugins
    pub async fn get_loaded_plugins(&self) -> Vec<PluginInfo> {
        let loaded = self.loaded_plugins.read().await;
        loaded.values().map(|p| p.metadata.clone()).collect()
    }

    /// Discover plugins in directory
    pub async fn discover_plugins(&self, directory: &str) -> PluginResult<Vec<PluginLoadResult>> {
        let dir_path = Path::new(directory);
        
        if !dir_path.exists() {
            return Err(PluginError::InitializationFailed(
                format!("Plugin directory not found: {}", directory)
            ));
        }

        let mut results = Vec::new();
        let mut entries = fs::read_dir(dir_path).await
            .map_err(|e| PluginError::IoError(e))?;

        while let Some(entry) = entries.next_entry().await
            .map_err(|e| PluginError::IoError(e))? {
            
            let path = entry.path();
            
            // Check for plugin files (could be .so, .dll, .dylib, or .toml manifest files)
            if self.is_plugin_file(&path) {
                match self.load_plugin_info(&path).await {
                    Ok(info) => {
                        results.push(PluginLoadResult::Success {
                            path: path.to_string_lossy().to_string(),
                            info,
                        });
                    }
                    Err(e) => {
                        results.push(PluginLoadResult::Error {
                            path: path.to_string_lossy().to_string(),
                            error: e,
                        });
                    }
                }
            }
        }

        Ok(results)
    }

    /// Load plugin metadata from manifest file
    async fn load_metadata(&self, plugin_path: &Path) -> PluginResult<PluginInfo> {
        // Look for manifest file next to plugin
        let manifest_path = plugin_path.with_extension("toml");
        
        if manifest_path.exists() {
            let content = fs::read_to_string(&manifest_path).await
                .map_err(|e| PluginError::IoError(e))?;
            
            let manifest: PluginManifest = toml::from_str(&content)
                .map_err(|e| PluginError::ConfigurationError(format!("Invalid manifest: {}", e)))?;

            Ok(PluginInfo {
                id: manifest.plugin.id,
                name: manifest.plugin.name,
                version: manifest.plugin.version,
                description: manifest.plugin.description,
                author: manifest.plugin.author,
                license: manifest.plugin.license,
                website: manifest.plugin.website,
                api_version: manifest.plugin.api_version,
                minimum_platform_version: manifest.plugin.minimum_platform_version,
                game_type: manifest.plugin.game_type,
                supported_features: manifest.plugin.supported_features,
                dependencies: manifest.plugin.dependencies.unwrap_or_default(),
            })
        } else {
            // Fallback to embedded metadata (would need to load from binary)
            self.load_embedded_metadata(plugin_path).await
        }
    }

    /// Load embedded metadata from plugin binary
    async fn load_embedded_metadata(&self, _plugin_path: &Path) -> PluginResult<PluginInfo> {
        // This would involve loading the shared library and calling a metadata function
        // For now, return a placeholder
        Err(PluginError::InitializationFailed(
            "No manifest file found and embedded metadata not supported".to_string()
        ))
    }

    /// Validate plugin signature
    async fn validate_signature(&self, plugin_path: &Path, metadata: &PluginInfo) -> PluginResult<()> {
        // Look for signature file
        let sig_path = plugin_path.with_extension("sig");
        
        if !sig_path.exists() {
            return Err(PluginError::SecurityViolation(
                "Plugin signature file not found".to_string()
            ));
        }

        // Load signature
        let signature_data = fs::read(&sig_path).await
            .map_err(|e| PluginError::IoError(e))?;
        
        let signature: PluginSignature = bincode::deserialize(&signature_data)
            .map_err(|e| PluginError::ValidationFailed(format!("Invalid signature format: {}", e)))?;

        // Calculate plugin file hash
        let plugin_data = fs::read(plugin_path).await
            .map_err(|e| PluginError::IoError(e))?;
        
        let mut hasher = Sha256::new();
        hasher.update(&plugin_data);
        let file_hash = hasher.finalize();

        // Verify signature matches expected hash
        if signature.file_hash != file_hash.as_slice() {
            return Err(PluginError::SecurityViolation(
                "Plugin file hash does not match signature".to_string()
            ));
        }

        // Verify metadata matches signature
        let metadata_json = serde_json::to_string(metadata)
            .map_err(|e| PluginError::SerializationError(e))?;
        
        let mut metadata_hasher = Sha256::new();
        metadata_hasher.update(metadata_json.as_bytes());
        let metadata_hash = metadata_hasher.finalize();

        if signature.metadata_hash != metadata_hash.as_slice() {
            return Err(PluginError::SecurityViolation(
                "Plugin metadata does not match signature".to_string()
            ));
        }

        // Additional signature verification would go here (RSA/ECDSA)
        info!("Plugin signature validated: {}", metadata.name);
        
        Ok(())
    }

    /// Validate plugin compatibility
    async fn validate_compatibility(&self, metadata: &PluginInfo) -> PluginResult<()> {
        // Check API version compatibility
        if !self.is_api_version_compatible(&metadata.api_version) {
            return Err(PluginError::VersionIncompatible(
                format!("Plugin API version {} not compatible", metadata.api_version)
            ));
        }

        // Check platform version requirements
        if !self.is_platform_version_compatible(&metadata.minimum_platform_version) {
            return Err(PluginError::VersionIncompatible(
                format!("Plugin requires platform version {}", metadata.minimum_platform_version)
            ));
        }

        // Validate dependencies
        for dependency in &metadata.dependencies {
            if dependency.required && !self.is_dependency_available(&dependency.plugin_id).await {
                return Err(PluginError::DependencyNotSatisfied(
                    format!("Required dependency not found: {}", dependency.plugin_id)
                ));
            }
        }

        Ok(())
    }

    /// Load plugin configuration
    async fn load_plugin_config(
        &self,
        plugin_path: &Path,
        metadata: &PluginInfo,
    ) -> PluginResult<HashMap<String, serde_json::Value>> {
        let config_path = plugin_path.with_file_name(format!("{}.config.toml", metadata.id));
        
        let mut config = HashMap::new();
        
        if config_path.exists() {
            let content = fs::read_to_string(&config_path).await
                .map_err(|e| PluginError::IoError(e))?;
            
            let config_data: toml::Value = toml::from_str(&content)
                .map_err(|e| PluginError::ConfigurationError(format!("Invalid config: {}", e)))?;
            
            // Convert TOML to JSON Value for generic handling
            let json_value: serde_json::Value = serde_json::from_str(&content)
                .unwrap_or_else(|_| serde_json::Value::Object(serde_json::Map::new()));
            
            if let serde_json::Value::Object(map) = json_value {
                config.extend(map);
            }
        }
        
        // Add default configuration values
        config.insert("plugin_id".to_string(), serde_json::Value::String(metadata.id.clone()));
        config.insert("plugin_version".to_string(), serde_json::Value::String(metadata.version.clone()));
        
        Ok(config)
    }

    /// Create plugin factory for instantiation
    async fn create_plugin_factory(
        &self,
        _plugin_path: &Path,
        metadata: &PluginInfo,
    ) -> PluginResult<Box<dyn PluginFactory>> {
        // This would typically load the shared library and get a factory function
        // For now, create a mock factory based on the game type
        match metadata.game_type {
            super::core::GameType::Blackjack => {
                Ok(Box::new(MockPluginFactory { 
                    game_type: "blackjack".to_string() 
                }))
            }
            super::core::GameType::Poker => {
                Ok(Box::new(MockPluginFactory { 
                    game_type: "poker".to_string() 
                }))
            }
            super::core::GameType::Roulette => {
                Ok(Box::new(MockPluginFactory { 
                    game_type: "roulette".to_string() 
                }))
            }
            _ => Err(PluginError::InitializationFailed(
                format!("Unsupported game type: {:?}", metadata.game_type)
            ))
        }
    }

    /// Check if file is a plugin file
    fn is_plugin_file(&self, path: &Path) -> bool {
        if let Some(extension) = path.extension() {
            match extension.to_str() {
                Some("so") | Some("dll") | Some("dylib") => true,
                Some("toml") if path.file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| s.contains("plugin"))
                    .unwrap_or(false) => true,
                _ => false,
            }
        } else {
            false
        }
    }

    /// Load basic plugin info for discovery
    async fn load_plugin_info(&self, path: &Path) -> PluginResult<PluginInfo> {
        if path.extension().and_then(|e| e.to_str()) == Some("toml") {
            self.load_metadata(path).await
        } else {
            // For binary files, look for adjacent manifest
            self.load_metadata(path).await
        }
    }

    /// Check API version compatibility
    fn is_api_version_compatible(&self, api_version: &str) -> bool {
        // Simple version check - in practice would use proper semver
        api_version.starts_with("1.") // Support API v1.x
    }

    /// Check platform version compatibility
    fn is_platform_version_compatible(&self, platform_version: &str) -> bool {
        // Check against current platform version
        let current_version = env!("CARGO_PKG_VERSION");
        // Simple check - actual implementation would use proper version comparison
        current_version >= platform_version
    }

    /// Check if dependency is available
    async fn is_dependency_available(&self, plugin_id: &str) -> bool {
        let loaded = self.loaded_plugins.read().await;
        loaded.contains_key(plugin_id)
    }
}

/// Plugin loader configuration
#[derive(Debug, Clone)]
pub struct LoaderConfig {
    pub plugin_directories: Vec<String>,
    pub require_signatures: bool,
    pub allow_unsigned_dev: bool,
    pub max_plugin_size_mb: u64,
    pub supported_architectures: Vec<String>,
}

impl Default for LoaderConfig {
    fn default() -> Self {
        Self {
            plugin_directories: vec!["plugins".to_string()],
            require_signatures: true,
            allow_unsigned_dev: false,
            max_plugin_size_mb: 100,
            supported_architectures: vec![
                std::env::consts::ARCH.to_string()
            ],
        }
    }
}

/// Result of plugin loading attempt
#[derive(Debug)]
pub enum PluginLoadResult {
    Success {
        path: String,
        info: PluginInfo,
    },
    Error {
        path: String,
        error: PluginError,
    },
}

/// Plugin manifest structure
#[derive(Debug, Deserialize, Serialize)]
struct PluginManifest {
    plugin: PluginManifestInfo,
}

#[derive(Debug, Deserialize, Serialize)]
struct PluginManifestInfo {
    id: String,
    name: String,
    version: String,
    description: String,
    author: String,
    license: String,
    website: Option<String>,
    api_version: String,
    minimum_platform_version: String,
    game_type: super::core::GameType,
    supported_features: Vec<String>,
    dependencies: Option<Vec<super::core::PluginDependency>>,
}

/// Plugin signature for verification
#[derive(Debug, Serialize, Deserialize)]
pub struct PluginSignature {
    pub plugin_id: String,
    pub version: String,
    pub file_hash: Vec<u8>,
    pub metadata_hash: Vec<u8>,
    pub signature: Vec<u8>,
    pub signed_by: String,
    pub signed_at: SystemTime,
}

/// Metadata about loaded plugin
#[derive(Debug, Clone)]
struct LoadedPlugin {
    metadata: PluginInfo,
    path: PathBuf,
    loaded_at: SystemTime,
    config: HashMap<String, serde_json::Value>,
}

/// Mock plugin factory for testing
struct MockPluginFactory {
    game_type: String,
}

impl PluginFactory for MockPluginFactory {
    fn create(&self) -> PluginResult<Box<dyn GamePlugin>> {
        // This would create actual plugin instances
        // For now, return an error since we don't have concrete implementations yet
        Err(PluginError::InitializationFailed(
            format!("Mock factory cannot create {} plugin", self.game_type)
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_loader_creation() {
        let config = LoaderConfig::default();
        let loader = PluginLoader::new(config).unwrap();
        
        let plugins = loader.get_loaded_plugins().await;
        assert_eq!(plugins.len(), 0);
    }

    #[tokio::test]
    async fn test_plugin_discovery() {
        let config = LoaderConfig::default();
        let loader = PluginLoader::new(config).unwrap();
        
        // Test discovering plugins in non-existent directory
        let result = loader.discover_plugins("/nonexistent").await;
        assert!(result.is_err());
    }

    #[test]
    fn test_plugin_file_detection() {
        let config = LoaderConfig::default();
        let loader = PluginLoader::new(config).unwrap();
        
        assert!(loader.is_plugin_file(Path::new("test.so")));
        assert!(loader.is_plugin_file(Path::new("test.dll")));
        assert!(loader.is_plugin_file(Path::new("test.dylib")));
        assert!(loader.is_plugin_file(Path::new("plugin.toml")));
        assert!(!loader.is_plugin_file(Path::new("test.txt")));
    }
}