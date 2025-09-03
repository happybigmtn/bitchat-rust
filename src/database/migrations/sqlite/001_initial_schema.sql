-- SQLite Initial Schema for BitCraps
-- Optimized for development and small-scale deployments

-- Enable foreign key constraints
PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
PRAGMA cache_size = -64000; -- 64MB cache
PRAGMA temp_store = MEMORY;

-- Users table with comprehensive identity management
CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    username TEXT NOT NULL UNIQUE,
    email TEXT UNIQUE,
    password_hash BLOB NOT NULL,
    salt BLOB NOT NULL,
    public_key BLOB NOT NULL,
    reputation_score INTEGER NOT NULL DEFAULT 1000,
    total_games_played INTEGER NOT NULL DEFAULT 0,
    total_winnings INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at INTEGER NOT NULL DEFAULT (unixepoch()),
    last_active INTEGER NOT NULL DEFAULT (unixepoch()),
    account_status TEXT NOT NULL DEFAULT 'active' CHECK (account_status IN ('active', 'suspended', 'banned', 'pending')),
    kyc_status TEXT NOT NULL DEFAULT 'none' CHECK (kyc_status IN ('none', 'pending', 'verified', 'rejected')),
    preferences TEXT DEFAULT '{}',
    
    -- Constraints
    CHECK (length(username) >= 3 AND length(username) <= 50),
    CHECK (reputation_score >= 0 AND reputation_score <= 10000)
);

-- Indexes for users
CREATE INDEX IF NOT EXISTS idx_users_username ON users(username);
CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);
CREATE INDEX IF NOT EXISTS idx_users_reputation ON users(reputation_score DESC);
CREATE INDEX IF NOT EXISTS idx_users_last_active ON users(last_active DESC);
CREATE INDEX IF NOT EXISTS idx_users_status ON users(account_status, kyc_status);

-- Games table with comprehensive game state tracking
CREATE TABLE IF NOT EXISTS games (
    id TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    game_type TEXT NOT NULL DEFAULT 'craps',
    status TEXT NOT NULL DEFAULT 'waiting' CHECK (status IN ('waiting', 'active', 'completed', 'cancelled', 'disputed')),
    creator_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    max_players INTEGER NOT NULL DEFAULT 8 CHECK (max_players BETWEEN 2 AND 16),
    current_players INTEGER NOT NULL DEFAULT 1 CHECK (current_players >= 0),
    min_bet INTEGER NOT NULL CHECK (min_bet > 0),
    max_bet INTEGER NOT NULL CHECK (max_bet >= min_bet),
    house_edge REAL NOT NULL DEFAULT 0.0136 CHECK (house_edge >= 0 AND house_edge <= 0.1),
    total_pot INTEGER NOT NULL DEFAULT 0,
    game_state TEXT NOT NULL DEFAULT '{}',
    consensus_state TEXT NOT NULL DEFAULT '{}',
    dice_results TEXT DEFAULT '[]',
    round_number INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    started_at INTEGER,
    completed_at INTEGER,
    updated_at INTEGER NOT NULL DEFAULT (unixepoch()),
    
    -- Ensure valid state transitions
    CHECK (
        (status = 'completed' AND completed_at IS NOT NULL) OR 
        (status != 'completed' AND completed_at IS NULL)
    ),
    CHECK (
        (status IN ('active', 'completed', 'cancelled') AND started_at IS NOT NULL) OR 
        (status = 'waiting' AND started_at IS NULL)
    )
);

-- Indexes for games
CREATE INDEX IF NOT EXISTS idx_games_status ON games(status, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_games_creator ON games(creator_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_games_waiting ON games(status, max_players, current_players) WHERE status = 'waiting';
CREATE INDEX IF NOT EXISTS idx_games_active ON games(status, updated_at) WHERE status = 'active';
CREATE INDEX IF NOT EXISTS idx_games_type_status ON games(game_type, status, created_at DESC);

-- Game participants with comprehensive betting tracking
CREATE TABLE IF NOT EXISTS game_participants (
    id TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    position INTEGER NOT NULL CHECK (position BETWEEN 1 AND 16),
    joined_at INTEGER NOT NULL DEFAULT (unixepoch()),
    left_at INTEGER,
    total_bet INTEGER NOT NULL DEFAULT 0,
    total_winnings INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'left', 'disconnected', 'kicked')),
    participant_state TEXT DEFAULT '{}',
    
    -- Unique constraints
    UNIQUE(game_id, user_id),
    UNIQUE(game_id, position),
    
    -- Consistency constraints
    CHECK (
        (status IN ('left', 'disconnected', 'kicked') AND left_at IS NOT NULL) OR 
        (status = 'active' AND left_at IS NULL)
    )
);

-- Indexes for game participants
CREATE INDEX IF NOT EXISTS idx_participants_game ON game_participants(game_id, position);
CREATE INDEX IF NOT EXISTS idx_participants_user ON game_participants(user_id, joined_at DESC);
CREATE INDEX IF NOT EXISTS idx_participants_status ON game_participants(status, joined_at DESC);

-- Bets table with comprehensive betting information
CREATE TABLE IF NOT EXISTS bets (
    id TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    participant_id TEXT NOT NULL REFERENCES game_participants(id) ON DELETE CASCADE,
    bet_type TEXT NOT NULL,
    bet_amount INTEGER NOT NULL CHECK (bet_amount > 0),
    potential_payout INTEGER NOT NULL CHECK (potential_payout >= 0),
    actual_payout INTEGER DEFAULT 0,
    odds_numerator INTEGER NOT NULL DEFAULT 1,
    odds_denominator INTEGER NOT NULL DEFAULT 1,
    round_number INTEGER NOT NULL DEFAULT 1,
    placed_at INTEGER NOT NULL DEFAULT (unixepoch()),
    resolved_at INTEGER,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'won', 'lost', 'cancelled', 'pushed')),
    bet_data TEXT DEFAULT '{}',
    
    -- Constraints
    CHECK (odds_numerator > 0 AND odds_denominator > 0),
    CHECK (
        (status IN ('won', 'lost', 'cancelled', 'pushed') AND resolved_at IS NOT NULL) OR 
        (status = 'pending' AND resolved_at IS NULL)
    )
);

-- Indexes for bets
CREATE INDEX IF NOT EXISTS idx_bets_game ON bets(game_id, round_number, placed_at);
CREATE INDEX IF NOT EXISTS idx_bets_user ON bets(user_id, placed_at DESC);
CREATE INDEX IF NOT EXISTS idx_bets_status ON bets(status, placed_at DESC);
CREATE INDEX IF NOT EXISTS idx_bets_resolution ON bets(resolved_at DESC) WHERE resolved_at IS NOT NULL;

-- Transactions table for comprehensive financial tracking
CREATE TABLE IF NOT EXISTS transactions (
    id TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    transaction_type TEXT NOT NULL,
    amount INTEGER NOT NULL,
    balance_before INTEGER NOT NULL,
    balance_after INTEGER NOT NULL,
    game_id TEXT REFERENCES games(id) ON DELETE SET NULL,
    bet_id TEXT REFERENCES bets(id) ON DELETE SET NULL,
    reference_id TEXT,
    description TEXT,
    metadata TEXT DEFAULT '{}',
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    confirmed_at INTEGER,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'confirmed', 'failed', 'cancelled')),
    
    -- Ensure balance consistency
    CHECK (balance_before + amount = balance_after),
    CHECK (
        (status = 'confirmed' AND confirmed_at IS NOT NULL) OR 
        (status != 'confirmed' AND confirmed_at IS NULL)
    )
);

-- Indexes for transactions
CREATE INDEX IF NOT EXISTS idx_transactions_user ON transactions(user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_transactions_type ON transactions(transaction_type, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_transactions_game ON transactions(game_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_transactions_status ON transactions(status, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_transactions_reference ON transactions(reference_id) WHERE reference_id IS NOT NULL;

-- Consensus messages for distributed game state
CREATE TABLE IF NOT EXISTS consensus_messages (
    id TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    sender_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    message_type TEXT NOT NULL,
    sequence_number INTEGER NOT NULL,
    round_number INTEGER NOT NULL,
    message_data TEXT NOT NULL,
    signature BLOB NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    processed_at INTEGER,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'processed', 'rejected', 'duplicate')),
    
    -- Unique constraint for consensus ordering
    UNIQUE(game_id, sequence_number),
    
    -- Ensure valid processing
    CHECK (
        (status = 'processed' AND processed_at IS NOT NULL) OR 
        (status != 'processed' AND processed_at IS NULL)
    )
);

-- Indexes for consensus messages
CREATE INDEX IF NOT EXISTS idx_consensus_game_seq ON consensus_messages(game_id, sequence_number);
CREATE INDEX IF NOT EXISTS idx_consensus_sender ON consensus_messages(sender_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_consensus_status ON consensus_messages(status, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_consensus_type ON consensus_messages(message_type, game_id, sequence_number);

-- Peer connections for mesh networking
CREATE TABLE IF NOT EXISTS peer_connections (
    id TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    local_peer_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    remote_peer_id TEXT NOT NULL,
    connection_type TEXT NOT NULL CHECK (connection_type IN ('bluetooth', 'tcp', 'websocket')),
    transport_info TEXT NOT NULL DEFAULT '{}',
    connection_status TEXT NOT NULL DEFAULT 'connecting' CHECK (connection_status IN ('connecting', 'connected', 'disconnected', 'failed')),
    established_at INTEGER,
    last_seen INTEGER NOT NULL DEFAULT (unixepoch()),
    disconnect_reason TEXT,
    statistics TEXT DEFAULT '{}',
    
    -- Unique constraint per local peer and remote peer
    UNIQUE(local_peer_id, remote_peer_id, connection_type),
    
    -- Valid connection state
    CHECK (
        (connection_status = 'connected' AND established_at IS NOT NULL) OR 
        (connection_status != 'connected' AND established_at IS NULL)
    )
);

-- Indexes for peer connections
CREATE INDEX IF NOT EXISTS idx_peers_local ON peer_connections(local_peer_id, connection_status);
CREATE INDEX IF NOT EXISTS idx_peers_remote ON peer_connections(remote_peer_id, connection_status);
CREATE INDEX IF NOT EXISTS idx_peers_status ON peer_connections(connection_status, last_seen DESC);

-- System metrics for monitoring and analytics
CREATE TABLE IF NOT EXISTS system_metrics (
    id TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    metric_name TEXT NOT NULL,
    metric_value REAL NOT NULL,
    metric_unit TEXT,
    component TEXT NOT NULL,
    instance_id TEXT,
    tags TEXT DEFAULT '{}',
    recorded_at INTEGER NOT NULL DEFAULT (unixepoch())
);

-- Indexes for system metrics
CREATE INDEX IF NOT EXISTS idx_metrics_name_time ON system_metrics(metric_name, recorded_at DESC);
CREATE INDEX IF NOT EXISTS idx_metrics_component ON system_metrics(component, recorded_at DESC);

-- Audit log for security and compliance
CREATE TABLE IF NOT EXISTS audit_log (
    id TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
    action TEXT NOT NULL,
    resource_type TEXT NOT NULL,
    resource_id TEXT,
    old_values TEXT,
    new_values TEXT,
    ip_address TEXT,
    user_agent TEXT,
    session_id TEXT,
    success INTEGER NOT NULL DEFAULT 1,
    error_message TEXT,
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
);

-- Indexes for audit log
CREATE INDEX IF NOT EXISTS idx_audit_user ON audit_log(user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_audit_action ON audit_log(action, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_audit_resource ON audit_log(resource_type, resource_id, created_at DESC);

-- Create triggers for updated_at columns
CREATE TRIGGER IF NOT EXISTS update_users_updated_at
    AFTER UPDATE ON users
BEGIN
    UPDATE users SET updated_at = unixepoch() WHERE id = NEW.id;
END;

CREATE TRIGGER IF NOT EXISTS update_games_updated_at
    AFTER UPDATE ON games
BEGIN
    UPDATE games SET updated_at = unixepoch() WHERE id = NEW.id;
END;

-- Create views for common queries
CREATE VIEW IF NOT EXISTS active_games AS
SELECT 
    g.*,
    u.username as creator_username,
    COUNT(gp.user_id) as participant_count,
    SUM(gp.total_bet) as total_game_bets
FROM games g
JOIN users u ON g.creator_id = u.id
LEFT JOIN game_participants gp ON g.id = gp.game_id AND gp.status = 'active'
WHERE g.status IN ('waiting', 'active')
GROUP BY g.id, u.username;

CREATE VIEW IF NOT EXISTS user_statistics AS
SELECT 
    u.id,
    u.username,
    u.reputation_score,
    COUNT(DISTINCT gp.game_id) as games_played,
    COALESCE(SUM(CASE WHEN t.amount > 0 THEN t.amount ELSE 0 END), 0) as total_winnings,
    COALESCE(SUM(CASE WHEN t.amount < 0 THEN t.amount ELSE 0 END), 0) as total_losses,
    COUNT(DISTINCT b.id) as total_bets,
    COUNT(DISTINCT CASE WHEN g.status = 'completed' THEN g.id END) as completed_games
FROM users u
LEFT JOIN game_participants gp ON u.id = gp.user_id
LEFT JOIN games g ON gp.game_id = g.id
LEFT JOIN transactions t ON u.id = t.user_id AND t.transaction_type IN ('bet_win', 'bet_loss')
LEFT JOIN bets b ON u.id = b.user_id
GROUP BY u.id, u.username, u.reputation_score;

-- Initial data setup
INSERT OR IGNORE INTO users (username, password_hash, salt, public_key) VALUES
    ('system', x'deadbeef', x'cafebabe', x'feedface');

-- Performance optimization: Analyze tables for query planning
ANALYZE users;
ANALYZE games;
ANALYZE game_participants;
ANALYZE bets;
ANALYZE transactions;
ANALYZE consensus_messages;
ANALYZE peer_connections;
ANALYZE system_metrics;
ANALYZE audit_log;

-- Enable query plan optimization
PRAGMA optimize;