//! Plugin Registry for BitCraps
//!
//! This module manages plugin registration, version tracking, dependency
//! resolution, and plugin metadata storage.

use std::collections::{HashMap, HashSet};
use std::time::SystemTime;
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error};

use super::core::{PluginInfo, GameType};

/// Plugin registry that maintains all registered plugins
pub struct PluginRegistry {
    config: RegistryConfig,
    plugins: HashMap<String, PluginEntry>,
    dependencies: HashMap<String, HashSet<String>>,
    reverse_dependencies: HashMap<String, HashSet<String>>,
}

impl PluginRegistry {
    /// Create new plugin registry
    pub fn new(config: RegistryConfig) -> Result<Self, RegistryError> {
        Ok(Self {
            config,
            plugins: HashMap::new(),
            dependencies: HashMap::new(),
            reverse_dependencies: HashMap::new(),
        })
    }

    /// Register a plugin
    pub fn register(&mut self, metadata: PluginInfo) -> Result<String, RegistryError> {
        let plugin_id = metadata.id.clone();

        // Check if plugin already registered
        if self.plugins.contains_key(&plugin_id) {
            if self.config.allow_updates {
                warn!("Updating existing plugin: {}", plugin_id);
            } else {
                return Err(RegistryError::PluginAlreadyExists(plugin_id));
            }
        }

        // Validate plugin metadata
        self.validate_plugin_metadata(&metadata)?;

        // Check dependencies
        self.validate_dependencies(&metadata)?;

        // Create plugin entry
        let entry = PluginEntry {
            metadata: metadata.clone(),
            registered_at: SystemTime::now(),
            status: PluginStatus::Registered,
            load_count: 0,
            last_loaded: None,
            capabilities: self.determine_capabilities(&metadata),
        };

        // Update dependency tracking
        self.update_dependencies(&plugin_id, &metadata.dependencies);

        // Insert into registry
        self.plugins.insert(plugin_id.clone(), entry);

        info!("Registered plugin: {} v{}", metadata.name, metadata.version);
        Ok(plugin_id)
    }

    /// Unregister a plugin
    pub fn unregister(&mut self, plugin_id: &str) -> Result<(), RegistryError> {
        // Check if plugin exists
        if !self.plugins.contains_key(plugin_id) {
            return Err(RegistryError::PluginNotFound(plugin_id.to_string()));
        }

        // Check for reverse dependencies
        if let Some(dependents) = self.reverse_dependencies.get(plugin_id) {
            if !dependents.is_empty() && !self.config.allow_force_unregister {
                return Err(RegistryError::HasDependents(
                    plugin_id.to_string(),
                    dependents.clone()
                ));
            }
        }

        // Remove from all tracking
        self.plugins.remove(plugin_id);
        self.dependencies.remove(plugin_id);
        self.reverse_dependencies.remove(plugin_id);

        // Clean up reverse dependencies
        for deps in self.reverse_dependencies.values_mut() {
            deps.remove(plugin_id);
        }

        info!("Unregistered plugin: {}", plugin_id);
        Ok(())
    }

    /// Get plugin information
    pub fn get_plugin(&self, plugin_id: &str) -> Option<&PluginEntry> {
        self.plugins.get(plugin_id)
    }

    /// List all plugins
    pub fn list_plugins(&self) -> Vec<PluginInfo> {
        self.plugins.values().map(|entry| entry.metadata.clone()).collect()
    }

    /// List plugins by game type
    pub fn list_plugins_by_type(&self, game_type: &GameType) -> Vec<PluginInfo> {
        self.plugins
            .values()
            .filter(|entry| &entry.metadata.game_type == game_type)
            .map(|entry| entry.metadata.clone())
            .collect()
    }

    /// Get plugin count
    pub fn count(&self) -> usize {
        self.plugins.len()
    }

    /// Mark plugin as loaded
    pub fn mark_loaded(&mut self, plugin_id: &str) -> Result<(), RegistryError> {
        let entry = self.plugins.get_mut(plugin_id)
            .ok_or_else(|| RegistryError::PluginNotFound(plugin_id.to_string()))?;

        entry.status = PluginStatus::Loaded;
        entry.load_count += 1;
        entry.last_loaded = Some(SystemTime::now());

        Ok(())
    }

    /// Mark plugin as unloaded
    pub fn mark_unloaded(&mut self, plugin_id: &str) -> Result<(), RegistryError> {
        let entry = self.plugins.get_mut(plugin_id)
            .ok_or_else(|| RegistryError::PluginNotFound(plugin_id.to_string()))?;

        entry.status = PluginStatus::Registered;
        Ok(())
    }

    /// Get plugin dependencies
    pub fn get_dependencies(&self, plugin_id: &str) -> Vec<String> {
        self.dependencies.get(plugin_id).cloned()
            .unwrap_or_default()
            .into_iter()
            .collect()
    }

    /// Get plugins that depend on this plugin
    pub fn get_dependents(&self, plugin_id: &str) -> Vec<String> {
        self.reverse_dependencies.get(plugin_id).cloned()
            .unwrap_or_default()
            .into_iter()
            .collect()
    }

    /// Resolve load order based on dependencies
    pub fn resolve_load_order(&self, plugin_ids: &[String]) -> Result<Vec<String>, RegistryError> {
        let mut resolved = Vec::new();
        let mut visiting = HashSet::new();
        let mut visited = HashSet::new();

        for plugin_id in plugin_ids {
            if !visited.contains(plugin_id) {
                self.dependency_sort(plugin_id, &mut resolved, &mut visiting, &mut visited)?;
            }
        }

        Ok(resolved)
    }

    /// Check if plugin update is available
    pub fn check_update(&self, plugin_id: &str, current_version: &str) -> Option<PluginVersion> {
        if let Some(entry) = self.plugins.get(plugin_id) {
            if self.is_newer_version(&entry.metadata.version, current_version) {
                return Some(PluginVersion {
                    plugin_id: plugin_id.to_string(),
                    version: entry.metadata.version.clone(),
                    available: true,
                });
            }
        }
        None
    }

    /// Get registry statistics
    pub fn get_statistics(&self) -> RegistryStatistics {
        let mut by_type = HashMap::new();
        let mut by_status = HashMap::new();

        for entry in self.plugins.values() {
            *by_type.entry(entry.metadata.game_type.clone()).or_insert(0) += 1;
            *by_status.entry(entry.status.clone()).or_insert(0) += 1;
        }

        RegistryStatistics {
            total_plugins: self.plugins.len(),
            plugins_by_type: by_type,
            plugins_by_status: by_status,
            total_load_count: self.plugins.values().map(|e| e.load_count).sum(),
        }
    }

    /// Validate plugin metadata
    fn validate_plugin_metadata(&self, metadata: &PluginInfo) -> Result<(), RegistryError> {
        // Validate required fields
        if metadata.id.is_empty() {
            return Err(RegistryError::InvalidMetadata("Plugin ID cannot be empty".to_string()));
        }

        if metadata.name.is_empty() {
            return Err(RegistryError::InvalidMetadata("Plugin name cannot be empty".to_string()));
        }

        if metadata.version.is_empty() {
            return Err(RegistryError::InvalidMetadata("Plugin version cannot be empty".to_string()));
        }

        // Validate version format (basic semver check)
        if !self.is_valid_version(&metadata.version) {
            return Err(RegistryError::InvalidMetadata(
                format!("Invalid version format: {}", metadata.version)
            ));
        }

        // Validate API version compatibility
        if !self.is_api_compatible(&metadata.api_version) {
            return Err(RegistryError::IncompatibleApiVersion(metadata.api_version.clone()));
        }

        Ok(())
    }

    /// Validate plugin dependencies
    fn validate_dependencies(&self, metadata: &PluginInfo) -> Result<(), RegistryError> {
        for dependency in &metadata.dependencies {
            if dependency.required {
                if let Some(dep_entry) = self.plugins.get(&dependency.plugin_id) {
                    // Check version compatibility
                    if !self.is_version_compatible(
                        &dep_entry.metadata.version,
                        &dependency.minimum_version
                    ) {
                        return Err(RegistryError::IncompatibleDependency(
                            format!("Dependency {} version {} is not compatible with required minimum {}",
                                dependency.plugin_id, dep_entry.metadata.version, dependency.minimum_version)
                        ));
                    }
                } else {
                    return Err(RegistryError::MissingDependency(dependency.plugin_id.clone()));
                }
            }
        }

        Ok(())
    }

    /// Update dependency tracking
    fn update_dependencies(&mut self, plugin_id: &str, dependencies: &[super::core::PluginDependency]) {
        let mut deps = HashSet::new();

        for dependency in dependencies {
            deps.insert(dependency.plugin_id.clone());

            // Update reverse dependencies
            self.reverse_dependencies
                .entry(dependency.plugin_id.clone())
                .or_insert_with(HashSet::new)
                .insert(plugin_id.to_string());
        }

        self.dependencies.insert(plugin_id.to_string(), deps);
    }

    /// Determine plugin capabilities based on metadata
    fn determine_capabilities(&self, metadata: &PluginInfo) -> Vec<PluginCapabilityInfo> {
        let mut capabilities = Vec::new();

        // Add capabilities based on supported features
        for feature in &metadata.supported_features {
            match feature.as_str() {
                "real_money" => capabilities.push(PluginCapabilityInfo {
                    capability: super::core::PluginCapability::RealMoneyGaming,
                    granted: false, // Must be explicitly granted
                }),
                "network" => capabilities.push(PluginCapabilityInfo {
                    capability: super::core::PluginCapability::NetworkAccess,
                    granted: true,
                }),
                "storage" => capabilities.push(PluginCapabilityInfo {
                    capability: super::core::PluginCapability::DataStorage,
                    granted: true,
                }),
                "crypto" => capabilities.push(PluginCapabilityInfo {
                    capability: super::core::PluginCapability::Cryptography,
                    granted: true,
                }),
                _ => {}
            }
        }

        capabilities
    }

    /// Perform topological sort for dependency resolution
    fn dependency_sort(
        &self,
        plugin_id: &str,
        resolved: &mut Vec<String>,
        visiting: &mut HashSet<String>,
        visited: &mut HashSet<String>,
    ) -> Result<(), RegistryError> {
        if visiting.contains(plugin_id) {
            return Err(RegistryError::CircularDependency(plugin_id.to_string()));
        }

        if visited.contains(plugin_id) {
            return Ok(());
        }

        visiting.insert(plugin_id.to_string());

        if let Some(deps) = self.dependencies.get(plugin_id) {
            for dep_id in deps {
                self.dependency_sort(dep_id, resolved, visiting, visited)?;
            }
        }

        visiting.remove(plugin_id);
        visited.insert(plugin_id.to_string());
        resolved.push(plugin_id.to_string());

        Ok(())
    }

    /// Check if version string is valid
    fn is_valid_version(&self, version: &str) -> bool {
        // Basic semver validation
        let parts: Vec<&str> = version.split('.').collect();
        parts.len() >= 3 && parts.iter().all(|part| part.parse::<u32>().is_ok())
    }

    /// Check if API version is compatible
    fn is_api_compatible(&self, api_version: &str) -> bool {
        // Check against supported API versions
        self.config.supported_api_versions.contains(api_version)
    }

    /// Check if version is compatible
    fn is_version_compatible(&self, available: &str, required: &str) -> bool {
        // Simple version comparison - would use proper semver in practice
        available >= required
    }

    /// Check if first version is newer than second
    fn is_newer_version(&self, version1: &str, version2: &str) -> bool {
        // Simple comparison - would use proper semver parsing
        version1 > version2
    }
}

/// Registry configuration
#[derive(Debug, Clone)]
pub struct RegistryConfig {
    pub allow_updates: bool,
    pub allow_force_unregister: bool,
    pub supported_api_versions: Vec<String>,
    pub max_plugins: usize,
}

impl Default for RegistryConfig {
    fn default() -> Self {
        Self {
            allow_updates: true,
            allow_force_unregister: false,
            supported_api_versions: vec!["1.0".to_string(), "1.1".to_string()],
            max_plugins: 1000,
        }
    }
}

/// Plugin entry in registry
#[derive(Debug, Clone)]
pub struct PluginEntry {
    pub metadata: PluginInfo,
    pub registered_at: SystemTime,
    pub status: PluginStatus,
    pub load_count: u64,
    pub last_loaded: Option<SystemTime>,
    pub capabilities: Vec<PluginCapabilityInfo>,
}

/// Plugin status
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PluginStatus {
    Registered,
    Loaded,
    Error(String),
    Deprecated,
}

/// Plugin capability with grant status
#[derive(Debug, Clone)]
pub struct PluginCapabilityInfo {
    pub capability: super::core::PluginCapability,
    pub granted: bool,
}

/// Plugin version information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginVersion {
    pub plugin_id: String,
    pub version: String,
    pub available: bool,
}

/// Registry statistics
#[derive(Debug, Clone)]
pub struct RegistryStatistics {
    pub total_plugins: usize,
    pub plugins_by_type: HashMap<GameType, usize>,
    pub plugins_by_status: HashMap<PluginStatus, usize>,
    pub total_load_count: u64,
}

/// Registry error types
#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("Plugin already exists: {0}")]
    PluginAlreadyExists(String),
    
    #[error("Plugin not found: {0}")]
    PluginNotFound(String),
    
    #[error("Invalid plugin metadata: {0}")]
    InvalidMetadata(String),
    
    #[error("Missing dependency: {0}")]
    MissingDependency(String),
    
    #[error("Incompatible dependency: {0}")]
    IncompatibleDependency(String),
    
    #[error("Incompatible API version: {0}")]
    IncompatibleApiVersion(String),
    
    #[error("Circular dependency detected: {0}")]
    CircularDependency(String),
    
    #[error("Plugin has dependents: {0} -> {1:?}")]
    HasDependents(String, HashSet<String>),
    
    #[error("Registry full: maximum {0} plugins allowed")]
    RegistryFull(usize),
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::core::PluginDependency;

    fn create_test_plugin_info(id: &str, name: &str, version: &str) -> PluginInfo {
        PluginInfo {
            id: id.to_string(),
            name: name.to_string(),
            version: version.to_string(),
            description: "Test plugin".to_string(),
            author: "Test Author".to_string(),
            license: "MIT".to_string(),
            website: None,
            api_version: "1.0".to_string(),
            minimum_platform_version: "1.0.0".to_string(),
            game_type: GameType::Blackjack,
            supported_features: vec!["network".to_string()],
            dependencies: vec![],
        }
    }

    #[test]
    fn test_registry_creation() {
        let config = RegistryConfig::default();
        let registry = PluginRegistry::new(config).unwrap();
        
        assert_eq!(registry.count(), 0);
    }

    #[test]
    fn test_plugin_registration() {
        let config = RegistryConfig::default();
        let mut registry = PluginRegistry::new(config).unwrap();
        
        let plugin_info = create_test_plugin_info("test-plugin", "Test Plugin", "1.0.0");
        let plugin_id = registry.register(plugin_info).unwrap();
        
        assert_eq!(plugin_id, "test-plugin");
        assert_eq!(registry.count(), 1);
        
        let entry = registry.get_plugin(&plugin_id).unwrap();
        assert_eq!(entry.metadata.name, "Test Plugin");
        assert_eq!(entry.status, PluginStatus::Registered);
    }

    #[test]
    fn test_plugin_unregistration() {
        let config = RegistryConfig::default();
        let mut registry = PluginRegistry::new(config).unwrap();
        
        let plugin_info = create_test_plugin_info("test-plugin", "Test Plugin", "1.0.0");
        let plugin_id = registry.register(plugin_info).unwrap();
        
        registry.unregister(&plugin_id).unwrap();
        assert_eq!(registry.count(), 0);
        assert!(registry.get_plugin(&plugin_id).is_none());
    }

    #[test]
    fn test_dependency_resolution() {
        let config = RegistryConfig::default();
        let mut registry = PluginRegistry::new(config).unwrap();
        
        // Register plugin A
        let plugin_a = create_test_plugin_info("plugin-a", "Plugin A", "1.0.0");
        registry.register(plugin_a).unwrap();
        
        // Register plugin B that depends on A
        let mut plugin_b = create_test_plugin_info("plugin-b", "Plugin B", "1.0.0");
        plugin_b.dependencies.push(PluginDependency {
            plugin_id: "plugin-a".to_string(),
            minimum_version: "1.0.0".to_string(),
            required: true,
        });
        registry.register(plugin_b).unwrap();
        
        // Resolve load order
        let order = registry.resolve_load_order(&["plugin-b".to_string()]).unwrap();
        assert_eq!(order, vec!["plugin-a", "plugin-b"]);
    }

    #[test]
    fn test_plugin_by_type() {
        let config = RegistryConfig::default();
        let mut registry = PluginRegistry::new(config).unwrap();
        
        let mut blackjack_plugin = create_test_plugin_info("blackjack", "Blackjack", "1.0.0");
        blackjack_plugin.game_type = GameType::Blackjack;
        registry.register(blackjack_plugin).unwrap();
        
        let mut poker_plugin = create_test_plugin_info("poker", "Poker", "1.0.0");
        poker_plugin.game_type = GameType::Poker;
        registry.register(poker_plugin).unwrap();
        
        let blackjack_plugins = registry.list_plugins_by_type(&GameType::Blackjack);
        assert_eq!(blackjack_plugins.len(), 1);
        assert_eq!(blackjack_plugins[0].id, "blackjack");
        
        let poker_plugins = registry.list_plugins_by_type(&GameType::Poker);
        assert_eq!(poker_plugins.len(), 1);
        assert_eq!(poker_plugins[0].id, "poker");
    }
}