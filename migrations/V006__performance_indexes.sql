-- Performance optimization indexes
-- Version: 006
-- Description: Add composite indexes for hot queries identified in production

-- Composite index for user game history queries
-- Hot query: SELECT * FROM games WHERE winner_id = ? ORDER BY created_at DESC
CREATE INDEX IF NOT EXISTS idx_games_winner_created ON games(winner_id, created_at DESC);

-- Composite index for active user bets
-- Hot query: SELECT * FROM bets WHERE player_id = ? AND outcome IS NULL
CREATE INDEX IF NOT EXISTS idx_bets_player_unresolved ON bets(player_id, outcome);

-- Composite index for transaction history by user
-- Hot query: SELECT * FROM transactions WHERE from_user_id = ? OR to_user_id = ? ORDER BY created_at DESC
CREATE INDEX IF NOT EXISTS idx_transactions_user_history ON transactions(from_user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_transactions_to_user_history ON transactions(to_user_id, created_at DESC);

-- Composite index for game state and phase queries  
-- Hot query: SELECT * FROM games WHERE state = 'waiting' AND phase = 'betting'
CREATE INDEX IF NOT EXISTS idx_games_state_phase ON games(state, phase);

-- Composite index for peer connection quality queries
-- Hot query: SELECT * FROM peer_connections WHERE is_active = 1 AND connection_quality > 0.7
CREATE INDEX IF NOT EXISTS idx_peer_connections_active_quality ON peer_connections(is_active, connection_quality DESC);

-- Composite index for recent active peer discovery
-- Hot query: SELECT * FROM peer_discovery_events WHERE peer_id = ? AND last_seen_at > ?
CREATE INDEX IF NOT EXISTS idx_discovery_events_peer_recent ON peer_discovery_events(peer_id, last_seen_at DESC);

-- Composite index for network topology routing
-- Hot query: SELECT * FROM network_topology WHERE from_peer_id = ? AND expires_at > ?
CREATE INDEX IF NOT EXISTS idx_network_topology_from_expires ON network_topology(from_peer_id, expires_at);

-- Partial indexes for frequently filtered queries
-- Hot query: SELECT * FROM games WHERE state = 'waiting' ORDER BY created_at
CREATE INDEX IF NOT EXISTS idx_games_waiting_created ON games(created_at) WHERE state = 'waiting';

-- Hot query: SELECT * FROM transactions WHERE status = 'pending' ORDER BY created_at
CREATE INDEX IF NOT EXISTS idx_transactions_pending_created ON transactions(created_at) WHERE status = 'pending';

-- Hot query: SELECT * FROM bets WHERE outcome IS NULL ORDER BY created_at
CREATE INDEX IF NOT EXISTS idx_bets_unresolved_created ON bets(created_at) WHERE outcome IS NULL;

-- Covering index for user reputation leaderboard
-- Hot query: SELECT username, reputation FROM users WHERE is_active = 1 ORDER BY reputation DESC LIMIT 100
CREATE INDEX IF NOT EXISTS idx_users_active_reputation_covering ON users(is_active, reputation DESC) 
  WHERE is_active = 1;

-- Storage performance indexes for hot queries
CREATE INDEX IF NOT EXISTS idx_storage_collection_accessed ON storage_records(collection, last_accessed DESC);
CREATE INDEX IF NOT EXISTS idx_storage_collection_created ON storage_records(collection, created_at DESC);

-- Performance metrics query optimization
CREATE INDEX IF NOT EXISTS idx_perf_metrics_name_created ON performance_metrics(metric_name, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_perf_metrics_component_created ON performance_metrics(component, created_at DESC);

-- System health time-series queries
CREATE INDEX IF NOT EXISTS idx_system_health_cpu_created ON system_health(cpu_usage, created_at);
CREATE INDEX IF NOT EXISTS idx_system_health_memory_created ON system_health(memory_usage, created_at);