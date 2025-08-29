-- Game analytics and statistics
-- Version: 004
-- Description: Comprehensive game analytics and player statistics

CREATE TABLE IF NOT EXISTS game_statistics (
    game_id TEXT PRIMARY KEY REFERENCES games(id) ON DELETE CASCADE,
    total_bets INTEGER DEFAULT 0 CHECK(total_bets >= 0),
    total_wagered INTEGER DEFAULT 0 CHECK(total_wagered >= 0),
    total_won INTEGER DEFAULT 0 CHECK(total_won >= 0),
    house_edge REAL CHECK(house_edge BETWEEN 0 AND 1),
    duration_seconds INTEGER CHECK(duration_seconds >= 0),
    player_count INTEGER DEFAULT 0 CHECK(player_count >= 0),
    max_pot_size INTEGER DEFAULT 0,
    average_bet_size REAL DEFAULT 0,
    volatility_index REAL DEFAULT 0,
    fairness_score REAL DEFAULT 1.0,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS player_statistics (
    player_id TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    games_played INTEGER DEFAULT 0 CHECK(games_played >= 0),
    games_won INTEGER DEFAULT 0 CHECK(games_won >= 0),
    games_lost INTEGER DEFAULT 0 CHECK(games_lost >= 0),
    total_wagered INTEGER DEFAULT 0 CHECK(total_wagered >= 0),
    total_won INTEGER DEFAULT 0 CHECK(total_won >= 0),
    net_profit INTEGER DEFAULT 0,
    win_rate REAL DEFAULT 0.0 CHECK(win_rate BETWEEN 0 AND 1),
    avg_bet_size INTEGER DEFAULT 0,
    biggest_win INTEGER DEFAULT 0,
    biggest_loss INTEGER DEFAULT 0,
    longest_winning_streak INTEGER DEFAULT 0,
    longest_losing_streak INTEGER DEFAULT 0,
    current_streak INTEGER DEFAULT 0,
    current_streak_type TEXT CHECK(current_streak_type IN ('win', 'loss', 'none')),
    risk_tolerance REAL DEFAULT 0.5,
    play_style TEXT DEFAULT 'balanced',
    updated_at INTEGER NOT NULL
);

-- Betting pattern analysis
CREATE TABLE IF NOT EXISTS betting_patterns (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    player_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    bet_sequence INTEGER NOT NULL,
    bet_type TEXT NOT NULL,
    bet_amount INTEGER NOT NULL,
    pot_size_at_bet INTEGER,
    player_balance_before INTEGER,
    time_to_bet_ms INTEGER,
    confidence_level REAL,
    is_bluff INTEGER DEFAULT 0,
    created_at INTEGER NOT NULL
);

-- Game outcome analysis
CREATE TABLE IF NOT EXISTS outcome_analysis (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    outcome_type TEXT NOT NULL CHECK(outcome_type IN ('expected', 'upset', 'anomaly')),
    predicted_winner TEXT,
    actual_winner TEXT,
    prediction_confidence REAL,
    upset_factor REAL DEFAULT 0,
    statistical_deviation REAL,
    contributing_factors TEXT,
    created_at INTEGER NOT NULL
);

-- Real-time game metrics
CREATE TABLE IF NOT EXISTS game_metrics (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    metric_name TEXT NOT NULL,
    metric_value REAL NOT NULL,
    metric_unit TEXT,
    measurement_time INTEGER NOT NULL,
    player_context TEXT,
    game_phase TEXT
);

-- Player behavior insights
CREATE TABLE IF NOT EXISTS player_behavior (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    player_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    behavior_type TEXT NOT NULL CHECK(behavior_type IN ('aggressive', 'conservative', 'erratic', 'consistent', 'suspicious')),
    confidence_score REAL NOT NULL CHECK(confidence_score BETWEEN 0 AND 1),
    evidence_data TEXT,
    detected_at INTEGER NOT NULL,
    last_updated INTEGER NOT NULL,
    alert_level INTEGER DEFAULT 0 CHECK(alert_level BETWEEN 0 AND 5)
);

-- Indices for game analytics
CREATE INDEX IF NOT EXISTS idx_game_statistics_created_at ON game_statistics(created_at);
CREATE INDEX IF NOT EXISTS idx_game_statistics_player_count ON game_statistics(player_count);
CREATE INDEX IF NOT EXISTS idx_game_statistics_duration ON game_statistics(duration_seconds);
CREATE INDEX IF NOT EXISTS idx_game_statistics_house_edge ON game_statistics(house_edge);

CREATE INDEX IF NOT EXISTS idx_player_statistics_win_rate ON player_statistics(win_rate DESC);
CREATE INDEX IF NOT EXISTS idx_player_statistics_games_played ON player_statistics(games_played DESC);
CREATE INDEX IF NOT EXISTS idx_player_statistics_net_profit ON player_statistics(net_profit DESC);
CREATE INDEX IF NOT EXISTS idx_player_statistics_updated_at ON player_statistics(updated_at);

CREATE INDEX IF NOT EXISTS idx_betting_patterns_player ON betting_patterns(player_id);
CREATE INDEX IF NOT EXISTS idx_betting_patterns_game ON betting_patterns(game_id);
CREATE INDEX IF NOT EXISTS idx_betting_patterns_sequence ON betting_patterns(bet_sequence);
CREATE INDEX IF NOT EXISTS idx_betting_patterns_created_at ON betting_patterns(created_at);

CREATE INDEX IF NOT EXISTS idx_outcome_analysis_game ON outcome_analysis(game_id);
CREATE INDEX IF NOT EXISTS idx_outcome_analysis_type ON outcome_analysis(outcome_type);
CREATE INDEX IF NOT EXISTS idx_outcome_analysis_created_at ON outcome_analysis(created_at);

CREATE INDEX IF NOT EXISTS idx_game_metrics_game ON game_metrics(game_id);
CREATE INDEX IF NOT EXISTS idx_game_metrics_name ON game_metrics(metric_name);
CREATE INDEX IF NOT EXISTS idx_game_metrics_time ON game_metrics(measurement_time);

CREATE INDEX IF NOT EXISTS idx_player_behavior_player ON player_behavior(player_id);
CREATE INDEX IF NOT EXISTS idx_player_behavior_type ON player_behavior(behavior_type);
CREATE INDEX IF NOT EXISTS idx_player_behavior_alert ON player_behavior(alert_level);
CREATE INDEX IF NOT EXISTS idx_player_behavior_detected ON player_behavior(detected_at);