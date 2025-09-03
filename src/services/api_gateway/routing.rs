//! Request Routing
//!
//! Route matching and service resolution for the API gateway.

use super::{RouteConfig};
use crate::error::Result;
use std::collections::HashMap;
use tokio::sync::RwLock;

/// Router for matching incoming requests to services
pub struct Router {
    routes: RwLock<HashMap<String, RouteConfig>>,
}

impl Router {
    /// Create a new router
    pub fn new() -> Self {
        Self {
            routes: RwLock::new(HashMap::new()),
        }
    }
    
    /// Add a route configuration
    pub async fn add_route(&self, route: RouteConfig) {
        let mut routes = self.routes.write().await;
        routes.insert(route.path.clone(), route);
    }
    
    /// Remove a route
    pub async fn remove_route(&self, path: &str) {
        let mut routes = self.routes.write().await;
        routes.remove(path);
    }
    
    /// Find a matching route for the given path and method
    pub async fn find_route(&self, path: &str, method: &str) -> Option<RouteConfig> {
        let routes = self.routes.read().await;
        
        // Try exact match first
        if let Some(route) = routes.get(path) {
            if route.methods.contains(&method.to_string()) {
                return Some(route.clone());
            }
        }
        
        // Try pattern matching for parameterized routes
        for (pattern, route) in routes.iter() {
            if self.matches_pattern(pattern, path) && route.methods.contains(&method.to_string()) {
                return Some(route.clone());
            }
        }
        
        None
    }
    
    /// Get all registered routes
    pub async fn list_routes(&self) -> Vec<RouteConfig> {
        let routes = self.routes.read().await;
        routes.values().cloned().collect()
    }
    
    /// Check if a path pattern matches the given path
    fn matches_pattern(&self, pattern: &str, path: &str) -> bool {
        let pattern_parts: Vec<&str> = pattern.split('/').collect();
        let path_parts: Vec<&str> = path.split('/').collect();
        
        if pattern_parts.len() != path_parts.len() {
            return false;
        }
        
        for (pattern_part, path_part) in pattern_parts.iter().zip(path_parts.iter()) {
            if pattern_part.starts_with('{') && pattern_part.ends_with('}') {
                // This is a parameter, it matches any non-empty value
                if path_part.is_empty() {
                    return false;
                }
                continue;
            }
            
            if pattern_part != path_part {
                return false;
            }
        }
        
        true
    }
    
    /// Extract path parameters from a matched route
    pub fn extract_params(&self, pattern: &str, path: &str) -> HashMap<String, String> {
        let mut params = HashMap::new();
        
        let pattern_parts: Vec<&str> = pattern.split('/').collect();
        let path_parts: Vec<&str> = path.split('/').collect();
        
        if pattern_parts.len() != path_parts.len() {
            return params;
        }
        
        for (pattern_part, path_part) in pattern_parts.iter().zip(path_parts.iter()) {
            if pattern_part.starts_with('{') && pattern_part.ends_with('}') {
                let param_name = &pattern_part[1..pattern_part.len()-1];
                params.insert(param_name.to_string(), path_part.to_string());
            }
        }
        
        params
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::api_gateway::RouteConfig;
    use std::time::Duration;
    
    #[tokio::test]
    async fn test_exact_route_matching() {
        let router = Router::new();
        
        let route = RouteConfig {
            path: "/api/v1/games".to_string(),
            service: "game-engine".to_string(),
            methods: vec!["GET".to_string(), "POST".to_string()],
            auth_required: true,
            rate_limit_override: None,
            timeout_override: None,
        };
        
        router.add_route(route).await;
        
        // Test exact match
        let matched = router.find_route("/api/v1/games", "GET").await;
        assert!(matched.is_some());
        assert_eq!(matched.unwrap().service, "game-engine");
        
        // Test wrong method
        let not_matched = router.find_route("/api/v1/games", "DELETE").await;
        assert!(not_matched.is_none());
        
        // Test wrong path
        let not_matched = router.find_route("/api/v1/players", "GET").await;
        assert!(not_matched.is_none());
    }
    
    #[tokio::test]
    async fn test_parameterized_route_matching() {
        let router = Router::new();
        
        let route = RouteConfig {
            path: "/api/v1/games/{id}".to_string(),
            service: "game-engine".to_string(),
            methods: vec!["GET".to_string()],
            auth_required: true,
            rate_limit_override: None,
            timeout_override: None,
        };
        
        router.add_route(route).await;
        
        // Test parameterized match
        let matched = router.find_route("/api/v1/games/123", "GET").await;
        assert!(matched.is_some());
        assert_eq!(matched.unwrap().service, "game-engine");
        
        // Test parameter extraction
        let params = router.extract_params("/api/v1/games/{id}", "/api/v1/games/123");
        assert_eq!(params.get("id"), Some(&"123".to_string()));
    }
    
    #[tokio::test]
    async fn test_multiple_parameters() {
        let router = Router::new();
        
        let route = RouteConfig {
            path: "/api/v1/games/{game_id}/players/{player_id}".to_string(),
            service: "game-engine".to_string(),
            methods: vec!["GET".to_string()],
            auth_required: true,
            rate_limit_override: None,
            timeout_override: None,
        };
        
        router.add_route(route).await;
        
        // Test multiple parameter match
        let matched = router.find_route("/api/v1/games/123/players/456", "GET").await;
        assert!(matched.is_some());
        
        // Test parameter extraction
        let params = router.extract_params(
            "/api/v1/games/{game_id}/players/{player_id}", 
            "/api/v1/games/123/players/456"
        );
        assert_eq!(params.get("game_id"), Some(&"123".to_string()));
        assert_eq!(params.get("player_id"), Some(&"456".to_string()));
    }
    
    #[tokio::test]
    async fn test_route_priority() {
        let router = Router::new();
        
        // Add specific route first
        let specific_route = RouteConfig {
            path: "/api/v1/games/health".to_string(),
            service: "health-service".to_string(),
            methods: vec!["GET".to_string()],
            auth_required: false,
            rate_limit_override: None,
            timeout_override: None,
        };
        
        // Add parameterized route second
        let param_route = RouteConfig {
            path: "/api/v1/games/{id}".to_string(),
            service: "game-engine".to_string(),
            methods: vec!["GET".to_string()],
            auth_required: true,
            rate_limit_override: None,
            timeout_override: None,
        };
        
        router.add_route(specific_route).await;
        router.add_route(param_route).await;
        
        // Specific route should match first
        let matched = router.find_route("/api/v1/games/health", "GET").await;
        assert!(matched.is_some());
        assert_eq!(matched.unwrap().service, "health-service");
        
        // Parameterized route should match other paths
        let matched = router.find_route("/api/v1/games/123", "GET").await;
        assert!(matched.is_some());
        assert_eq!(matched.unwrap().service, "game-engine");
    }
}