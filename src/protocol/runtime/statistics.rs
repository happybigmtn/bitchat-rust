//! Statistics Tracker
//! 
//! Collects and manages runtime statistics.

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};

/// Runtime statistics
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GameStats {
    pub total_games_created: u64,
    pub active_games: usize,
    pub total_bets_placed: u64,
    pub total_volume: u64,
    pub treasury_rake_collected: u64,
    pub largest_pot: u64,
    pub longest_game_rounds: u32,
    pub total_players: usize,
    pub peak_concurrent_players: usize,
    pub average_game_duration: Duration,
}

/// Tracks runtime statistics
pub struct StatisticsTracker {
    /// Current statistics
    stats: Arc<RwLock<GameStats>>,
    
    /// Start time for uptime tracking
    start_time: Instant,
    
    /// Game duration samples for averaging
    game_durations: Arc<RwLock<Vec<Duration>>>,
}

impl StatisticsTracker {
    /// Create a new statistics tracker
    pub fn new() -> Self {
        Self {
            stats: Arc::new(RwLock::new(GameStats::default())),
            start_time: Instant::now(),
            game_durations: Arc::new(RwLock::new(Vec::new())),
        }
    }
    
    /// Record a game creation
    pub async fn record_game_created(&self) {
        let mut stats = self.stats.write().await;
        stats.total_games_created += 1;
        stats.active_games += 1;
    }
    
    /// Record a game ending
    pub async fn record_game_ended(&self, duration: Duration, rounds: u32) {
        let mut stats = self.stats.write().await;
        stats.active_games = stats.active_games.saturating_sub(1);
        
        if rounds > stats.longest_game_rounds {
            stats.longest_game_rounds = rounds;
        }
        
        // Track duration for averaging
        let mut durations = self.game_durations.write().await;
        durations.push(duration);
        
        // Keep only last 100 samples
        if durations.len() > 100 {
            durations.remove(0);
        }
        
        // Update average
        if !durations.is_empty() {
            let total: Duration = durations.iter().sum();
            stats.average_game_duration = total / durations.len() as u32;
        }
    }
    
    /// Record a bet
    pub async fn record_bet(&self, amount: u64) {
        let mut stats = self.stats.write().await;
        stats.total_bets_placed += 1;
        stats.total_volume = stats.total_volume.saturating_add(amount);
    }
    
    /// Record rake collection
    pub async fn record_rake(&self, amount: u64) {
        let mut stats = self.stats.write().await;
        stats.treasury_rake_collected = stats.treasury_rake_collected.saturating_add(amount);
    }
    
    /// Record a pot size
    pub async fn record_pot(&self, amount: u64) {
        let mut stats = self.stats.write().await;
        if amount > stats.largest_pot {
            stats.largest_pot = amount;
        }
    }
    
    /// Update player count
    pub async fn update_player_count(&self, count: usize) {
        let mut stats = self.stats.write().await;
        stats.total_players = count;
        
        if count > stats.peak_concurrent_players {
            stats.peak_concurrent_players = count;
        }
    }
    
    /// Get current statistics
    pub async fn get_stats(&self) -> GameStats {
        self.stats.read().await.clone()
    }
    
    /// Get uptime
    pub fn get_uptime(&self) -> Duration {
        self.start_time.elapsed()
    }
    
    /// Reset statistics
    pub async fn reset(&self) {
        let mut stats = self.stats.write().await;
        *stats = GameStats::default();
        
        let mut durations = self.game_durations.write().await;
        durations.clear();
    }
    
    /// Start metrics collector
    pub async fn start_metrics_collector(&self) {
        let stats = self.stats.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            
            loop {
                interval.tick().await;
                
                // Log periodic statistics
                let current_stats = stats.read().await;
                log::info!(
                    "Runtime Stats: {} active games, {} total bets, {} volume",
                    current_stats.active_games,
                    current_stats.total_bets_placed,
                    current_stats.total_volume
                );
            }
        });
    }
    
    /// Export statistics as JSON
    pub async fn export_json(&self) -> String {
        let stats = self.stats.read().await;
        serde_json::to_string_pretty(&*stats).unwrap_or_else(|_| "{}".to_string())
    }
}