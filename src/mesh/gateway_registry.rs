//! In-memory registry for gateways with region and weights

use std::collections::HashMap;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct GatewayInfo {
    pub id: String,
    pub region: String,
    pub weight: u32,
    pub capacity_score: u32,
    pub ws_topics_supported: Vec<String>,
    pub last_heartbeat: Instant,
    pub healthy: bool,
}

impl GatewayInfo {
    pub fn new(id: String, region: String) -> Self {
        Self {
            id,
            region,
            weight: 100,
            capacity_score: 100,
            ws_topics_supported: Vec::new(),
            last_heartbeat: Instant::now(),
            healthy: true,
        }
    }
}

#[derive(Default)]
pub struct GatewayRegistry {
    by_id: HashMap<String, GatewayInfo>,
    by_region: HashMap<String, Vec<String>>, // region -> ids
}

impl GatewayRegistry {
    pub fn new() -> Self { Self::default() }

    pub fn register(&mut self, gw: GatewayInfo) {
        let region = gw.region.clone();
        let id = gw.id.clone();
        self.by_id.insert(id.clone(), gw);
        self.by_region.entry(region).or_default().push(id);
    }

    pub fn deregister(&mut self, id: &str) {
        if let Some(info) = self.by_id.remove(id) {
            if let Some(vec) = self.by_region.get_mut(&info.region) {
                vec.retain(|x| x != id);
                if vec.is_empty() { self.by_region.remove(&info.region); }
            }
        }
    }

    pub fn list_by_region(&self, region: &str) -> Vec<GatewayInfo> {
        self.by_region.get(region)
            .into_iter()
            .flat_map(|ids| ids.iter())
            .filter_map(|id| self.by_id.get(id).cloned())
            .collect()
    }

    pub fn get(&self, id: &str) -> Option<&GatewayInfo> { self.by_id.get(id) }

    pub fn update_health(&mut self, id: &str, healthy: bool) {
        if let Some(info) = self.by_id.get_mut(id) {
            info.healthy = healthy;
            info.last_heartbeat = Instant::now();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_basic() {
        let mut reg = GatewayRegistry::new();
        reg.register(GatewayInfo::new("gw1".into(), "iad".into()));
        reg.register(GatewayInfo::new("gw2".into(), "iad".into()));
        reg.register(GatewayInfo::new("gw3".into(), "sfo".into()));

        assert_eq!(reg.list_by_region("iad").len(), 2);
        reg.deregister("gw1");
        assert_eq!(reg.list_by_region("iad").len(), 1);
        assert!(reg.get("gw2").is_some());
    }
}

