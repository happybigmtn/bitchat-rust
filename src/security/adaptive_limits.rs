//! Adaptive Rate Limiting and Quota Tuning
//!
//! This module provides intelligent, self-tuning rate limits and quotas that adapt
//! based on network conditions, peer behavior, and security threat levels.

use crate::error::{Error, Result};
use crate::protocol::PeerId;
use crate::security::resource_quotas::{QuotaConfig, ResourceQuotaManager, ResourceType};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Adaptive configuration that adjusts based on conditions
#[derive(Debug, Clone)]
pub struct AdaptiveQuotaConfig {
    /// Base configuration (conservative defaults)
    pub base_config: QuotaConfig,
    /// Scaling factors for different conditions
    pub scaling_factors: ScalingFactors,
    /// Auto-tuning parameters
    pub auto_tuning: AutoTuningConfig,
}

/// Scaling factors for different network/security conditions
#[derive(Debug, Clone)]
pub struct ScalingFactors {
    /// Factor when network is under heavy load (0.5 = half limits)
    pub high_load_factor: f32,
    /// Factor when security threats detected (0.3 = very restrictive)
    pub threat_factor: f32,
    /// Factor for trusted peers (2.0 = double limits)
    pub trusted_peer_factor: f32,
    /// Factor during peak hours
    pub peak_hours_factor: f32,
    /// Factor for new/unverified peers
    pub new_peer_factor: f32,
}

impl Default for ScalingFactors {
    fn default() -> Self {
        Self {
            high_load_factor: 0.6,
            threat_factor: 0.3,
            trusted_peer_factor: 2.0,
            peak_hours_factor: 0.8,
            new_peer_factor: 0.5,
        }
    }
}

/// Auto-tuning configuration
#[derive(Debug, Clone)]
pub struct AutoTuningConfig {
    /// Enable automatic adjustment of limits
    pub enabled: bool,
    /// Minimum interval between adjustments
    pub adjustment_interval: Duration,
    /// Maximum percentage change per adjustment
    pub max_adjustment_rate: f32,
    /// Target system utilization (0.0 to 1.0)
    pub target_utilization: f32,
    /// Learning rate for adjustments (0.0 to 1.0)
    pub learning_rate: f32,
}

impl Default for AutoTuningConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            adjustment_interval: Duration::from_secs(60), // 1 minute
            max_adjustment_rate: 0.1,                     // 10% max change
            target_utilization: 0.7,                      // 70% target
            learning_rate: 0.05,                          // 5% learning rate
        }
    }
}

/// Current system conditions affecting rate limits
#[derive(Debug, Clone)]
pub struct SystemConditions {
    /// Current CPU utilization (0.0 to 1.0)
    pub cpu_usage: f32,
    /// Current memory utilization (0.0 to 1.0)
    pub memory_usage: f32,
    /// Current network utilization (0.0 to 1.0)
    pub network_usage: f32,
    /// Number of active connections
    pub active_connections: usize,
    /// Recent error rate (0.0 to 1.0)
    pub error_rate: f32,
    /// Security threat level (0 = none, 5 = critical)
    pub threat_level: u8,
    /// Time of day factor for peak hours
    pub time_of_day_factor: f32,
}

impl Default for SystemConditions {
    fn default() -> Self {
        Self {
            cpu_usage: 0.0,
            memory_usage: 0.0,
            network_usage: 0.0,
            active_connections: 0,
            error_rate: 0.0,
            threat_level: 0,
            time_of_day_factor: 1.0,
        }
    }
}

/// Peer reputation and behavior tracking
#[derive(Debug, Clone)]
pub struct PeerProfile {
    /// Peer trust level (0.0 = untrusted, 1.0 = fully trusted)
    pub trust_level: f32,
    /// How long we've known this peer
    pub relationship_duration: Duration,
    /// Recent violation count
    pub recent_violations: usize,
    /// Historical compliance rate (0.0 to 1.0)
    pub compliance_rate: f32,
    /// Peer's behavior classification
    pub behavior_class: BehaviorClass,
    /// Last time profile was updated
    pub last_updated: Instant,
}

/// Peer behavior classification
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BehaviorClass {
    /// Brand new peer, unknown behavior
    New,
    /// Well-behaved peer with good history
    Good,
    /// Peer with some violations but generally okay
    Suspicious,
    /// Peer with frequent violations
    Problematic,
    /// Peer engaging in clear attacks
    Malicious,
    /// Verified trusted peer (e.g., game server)
    Trusted,
}

/// Performance metrics for auto-tuning
#[derive(Debug, Clone)]
struct PerformanceMetrics {
    /// Request processing latency samples
    latency_samples: VecDeque<Duration>,
    /// Success rate over time
    success_rate_history: VecDeque<f32>,
    /// Resource utilization history
    utilization_history: VecDeque<f32>,
    /// Throughput measurements
    throughput_history: VecDeque<f32>,
}

impl PerformanceMetrics {
    fn new() -> Self {
        Self {
            latency_samples: VecDeque::with_capacity(100),
            success_rate_history: VecDeque::with_capacity(60),
            utilization_history: VecDeque::with_capacity(60),
            throughput_history: VecDeque::with_capacity(60),
        }
    }

    fn add_latency_sample(&mut self, latency: Duration) {
        self.latency_samples.push_back(latency);
        if self.latency_samples.len() > 100 {
            self.latency_samples.pop_front();
        }
    }

    fn get_average_latency(&self) -> Option<Duration> {
        if self.latency_samples.is_empty() {
            return None;
        }

        let total_nanos: u128 = self.latency_samples.iter().map(|d| d.as_nanos()).sum();
        let average_nanos = total_nanos / self.latency_samples.len() as u128;

        Some(Duration::from_nanos(average_nanos as u64))
    }
}

/// Adaptive rate limiter that adjusts based on conditions
pub struct AdaptiveRateLimiter {
    /// Base quota manager
    quota_manager: Arc<ResourceQuotaManager>,
    /// Adaptive configuration
    config: AdaptiveQuotaConfig,
    /// Current system conditions
    conditions: Arc<RwLock<SystemConditions>>,
    /// Per-peer profiles for behavior-based limits
    peer_profiles: Arc<RwLock<HashMap<PeerId, PeerProfile>>>,
    /// Performance metrics for auto-tuning
    metrics: Arc<RwLock<PerformanceMetrics>>,
    /// Last adjustment time
    last_adjustment: Arc<RwLock<Instant>>,
    /// Current effective scaling factor
    current_scaling: Arc<RwLock<f32>>,
}

impl AdaptiveRateLimiter {
    /// Create new adaptive rate limiter
    pub fn new(config: AdaptiveQuotaConfig) -> Self {
        let quota_manager = Arc::new(ResourceQuotaManager::with_config(
            config.base_config.clone(),
        ));

        Self {
            quota_manager,
            config,
            conditions: Arc::new(RwLock::new(SystemConditions::default())),
            peer_profiles: Arc::new(RwLock::new(HashMap::with_capacity(1000))),
            metrics: Arc::new(RwLock::new(PerformanceMetrics::new())),
            last_adjustment: Arc::new(RwLock::new(Instant::now())),
            current_scaling: Arc::new(RwLock::new(1.0)),
        }
    }

    /// Update system conditions (called periodically)
    pub async fn update_conditions(&self, conditions: SystemConditions) {
        let mut current_conditions = self.conditions.write().await;
        *current_conditions = conditions;

        // Trigger auto-tuning if enabled
        if self.config.auto_tuning.enabled {
            self.auto_tune().await;
        }
    }

    /// Check quota with adaptive limits
    pub async fn check_adaptive_quota(
        &self,
        peer_id: &PeerId,
        resource: ResourceType,
        amount: u64,
    ) -> std::result::Result<(), crate::security::resource_quotas::QuotaViolation> {
        // Get adaptive scaling factor for this peer
        let scaling_factor = self.calculate_peer_scaling_factor(peer_id).await;

        // Apply scaling to the requested amount (inverse scaling)
        // If limits are scaled down 0.5x, we scale the request up 2x for comparison
        let scaled_amount = if scaling_factor > 0.0 {
            ((amount as f64) / (scaling_factor as f64)).ceil() as u64
        } else {
            amount * 10 // Very restrictive if scaling factor is 0
        };

        // Check against base quota manager
        let result = self
            .quota_manager
            .check_quota(peer_id, resource, scaled_amount)
            .await;

        // Record performance metrics
        if result.is_err() {
            self.record_rejection(peer_id).await;
        } else {
            self.record_success(peer_id).await;
        }

        result
    }

    /// Calculate adaptive scaling factor for a specific peer
    async fn calculate_peer_scaling_factor(&self, peer_id: &PeerId) -> f32 {
        let conditions = self.conditions.read().await;
        let peer_profiles = self.peer_profiles.read().await;
        let base_scaling = *self.current_scaling.read().await;

        // Start with system-wide scaling
        let mut peer_scaling = base_scaling;

        // Apply peer-specific adjustments
        if let Some(profile) = peer_profiles.get(peer_id) {
            peer_scaling *= match profile.behavior_class {
                BehaviorClass::Trusted => self.config.scaling_factors.trusted_peer_factor,
                BehaviorClass::Good => 1.0,
                BehaviorClass::New => self.config.scaling_factors.new_peer_factor,
                BehaviorClass::Suspicious => 0.7,
                BehaviorClass::Problematic => 0.4,
                BehaviorClass::Malicious => 0.1,
            };

            // Apply trust level scaling
            let trust_factor = 0.5 + (profile.trust_level * 0.5);
            peer_scaling *= trust_factor;
        } else {
            // Unknown peer - apply new peer factor
            peer_scaling *= self.config.scaling_factors.new_peer_factor;
        }

        // Apply security threat adjustment
        if conditions.threat_level > 0 {
            let threat_factor = self
                .config
                .scaling_factors
                .threat_factor
                .powf(conditions.threat_level as f32 / 5.0);
            peer_scaling *= threat_factor;
        }

        // Ensure scaling factor is within reasonable bounds
        peer_scaling.clamp(0.01, 10.0)
    }

    /// Auto-tune limits based on system performance
    async fn auto_tune(&self) {
        let mut last_adjustment = self.last_adjustment.write().await;
        let now = Instant::now();

        // Check if enough time has passed
        if now.duration_since(*last_adjustment) < self.config.auto_tuning.adjustment_interval {
            return;
        }

        *last_adjustment = now;
        drop(last_adjustment);

        // Analyze current performance
        let conditions = self.conditions.read().await;
        let metrics = self.metrics.read().await;

        // Calculate target utilization vs actual
        let current_utilization =
            (conditions.cpu_usage + conditions.memory_usage + conditions.network_usage) / 3.0;
        let target = self.config.auto_tuning.target_utilization;

        // Calculate adjustment needed
        let utilization_error = current_utilization - target;
        let adjustment = -utilization_error * self.config.auto_tuning.learning_rate;

        // Apply maximum adjustment rate limit
        let clamped_adjustment = adjustment.clamp(
            -self.config.auto_tuning.max_adjustment_rate,
            self.config.auto_tuning.max_adjustment_rate,
        );

        // Update scaling factor
        let mut current_scaling = self.current_scaling.write().await;
        let new_scaling = (*current_scaling * (1.0 + clamped_adjustment)).clamp(0.1, 5.0);
        *current_scaling = new_scaling;

        log::info!(
            "Auto-tuned rate limits: utilization={:.2}, adjustment={:.3}, new_scaling={:.3}",
            current_utilization,
            clamped_adjustment,
            new_scaling
        );
    }

    /// Update peer profile based on behavior
    pub async fn update_peer_profile(&self, peer_id: PeerId, behavior_update: BehaviorUpdate) {
        let mut profiles = self.peer_profiles.write().await;
        let profile = profiles.entry(peer_id).or_insert_with(|| PeerProfile {
            trust_level: 0.5,
            relationship_duration: Duration::from_secs(0),
            recent_violations: 0,
            compliance_rate: 1.0,
            behavior_class: BehaviorClass::New,
            last_updated: Instant::now(),
        });

        let now = Instant::now();
        profile.relationship_duration += now.duration_since(profile.last_updated);
        profile.last_updated = now;

        match behavior_update {
            BehaviorUpdate::Violation => {
                profile.recent_violations += 1;
                profile.trust_level = (profile.trust_level - 0.1).max(0.0);
                profile.compliance_rate = profile.compliance_rate * 0.95;

                // Update behavior class based on violations
                profile.behavior_class = match profile.recent_violations {
                    0..=2 => BehaviorClass::Good,
                    3..=5 => BehaviorClass::Suspicious,
                    6..=10 => BehaviorClass::Problematic,
                    _ => BehaviorClass::Malicious,
                };
            }
            BehaviorUpdate::GoodBehavior => {
                profile.recent_violations = profile.recent_violations.saturating_sub(1);
                profile.trust_level = (profile.trust_level + 0.01).min(1.0);
                profile.compliance_rate = (profile.compliance_rate * 0.99 + 0.01).min(1.0);

                // Improve behavior class over time
                if profile.recent_violations == 0 && profile.trust_level > 0.8 {
                    profile.behavior_class = BehaviorClass::Good;
                }
            }
            BehaviorUpdate::PromoteToTrusted => {
                profile.behavior_class = BehaviorClass::Trusted;
                profile.trust_level = 1.0;
                profile.recent_violations = 0;
                profile.compliance_rate = 1.0;
            }
        }
    }

    /// Record successful request for metrics
    async fn record_success(&self, _peer_id: &PeerId) {
        // Implementation would record success metrics
    }

    /// Record rejected request for metrics
    async fn record_rejection(&self, peer_id: &PeerId) {
        // Update peer profile with violation
        self.update_peer_profile(*peer_id, BehaviorUpdate::Violation)
            .await;
    }

    /// Get current adaptive limits for monitoring
    pub async fn get_current_limits(&self, peer_id: &PeerId) -> AdaptiveLimits {
        let scaling_factor = self.calculate_peer_scaling_factor(peer_id).await;
        let base_config = &self.config.base_config;

        AdaptiveLimits {
            max_bandwidth: ((base_config.max_bandwidth as f64) * (scaling_factor as f64)) as u64,
            max_connections: ((base_config.max_connections as f64) * (scaling_factor as f64))
                as usize,
            max_message_rate: ((base_config.max_message_rate as f64) * (scaling_factor as f64))
                as u32,
            max_memory: ((base_config.max_memory as f64) * (scaling_factor as f64)) as usize,
            scaling_factor,
        }
    }

    /// Get system-wide statistics
    pub async fn get_adaptive_stats(&self) -> AdaptiveStats {
        let conditions = self.conditions.read().await;
        let profiles = self.peer_profiles.read().await;
        let current_scaling = *self.current_scaling.read().await;

        let peer_count_by_class = profiles.values().fold(HashMap::new(), |mut acc, profile| {
            *acc.entry(profile.behavior_class.clone()).or_insert(0) += 1;
            acc
        });

        AdaptiveStats {
            current_scaling_factor: current_scaling,
            system_utilization: (conditions.cpu_usage
                + conditions.memory_usage
                + conditions.network_usage)
                / 3.0,
            active_connections: conditions.active_connections,
            threat_level: conditions.threat_level,
            total_peers: profiles.len(),
            peer_count_by_class,
            auto_tuning_enabled: self.config.auto_tuning.enabled,
        }
    }
}

/// Behavior update types for peer profiles
#[derive(Debug, Clone)]
pub enum BehaviorUpdate {
    Violation,
    GoodBehavior,
    PromoteToTrusted,
}

/// Current adaptive limits for a peer
#[derive(Debug, Clone)]
pub struct AdaptiveLimits {
    pub max_bandwidth: u64,
    pub max_connections: usize,
    pub max_message_rate: u32,
    pub max_memory: usize,
    pub scaling_factor: f32,
}

/// System-wide adaptive statistics
#[derive(Debug, Clone)]
pub struct AdaptiveStats {
    pub current_scaling_factor: f32,
    pub system_utilization: f32,
    pub active_connections: usize,
    pub threat_level: u8,
    pub total_peers: usize,
    pub peer_count_by_class: HashMap<BehaviorClass, usize>,
    pub auto_tuning_enabled: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_adaptive_scaling() {
        let config = AdaptiveQuotaConfig {
            base_config: QuotaConfig::default(),
            scaling_factors: ScalingFactors::default(),
            auto_tuning: AutoTuningConfig::default(),
        };

        let limiter = AdaptiveRateLimiter::new(config);
        let peer_id: PeerId = [1u8; 32];

        // Test basic scaling calculation
        let scaling = limiter.calculate_peer_scaling_factor(&peer_id).await;

        // New peer should have reduced limits
        assert!(scaling < 1.0);
    }

    #[tokio::test]
    async fn test_peer_profile_updates() {
        let config = AdaptiveQuotaConfig {
            base_config: QuotaConfig::default(),
            scaling_factors: ScalingFactors::default(),
            auto_tuning: AutoTuningConfig::default(),
        };

        let limiter = AdaptiveRateLimiter::new(config);
        let peer_id: PeerId = [1u8; 32];

        // Update peer with violation
        limiter
            .update_peer_profile(peer_id, BehaviorUpdate::Violation)
            .await;

        // Check profile was updated
        let profiles = limiter.peer_profiles.read().await;
        let profile = profiles.get(&peer_id).unwrap();

        assert_eq!(profile.recent_violations, 1);
        assert!(profile.trust_level < 0.5);
    }

    #[tokio::test]
    async fn test_auto_tuning() {
        let mut config = AdaptiveQuotaConfig {
            base_config: QuotaConfig::default(),
            scaling_factors: ScalingFactors::default(),
            auto_tuning: AutoTuningConfig::default(),
        };
        config.auto_tuning.adjustment_interval = Duration::from_millis(1);

        let limiter = AdaptiveRateLimiter::new(config);

        // Set high utilization conditions
        let high_util_conditions = SystemConditions {
            cpu_usage: 0.9,
            memory_usage: 0.8,
            network_usage: 0.9,
            ..Default::default()
        };

        limiter.update_conditions(high_util_conditions).await;

        // Wait for auto-tuning
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Should have reduced scaling factor due to high utilization
        let scaling = *limiter.current_scaling.read().await;
        assert!(scaling < 1.0);
    }
}
