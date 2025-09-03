-- PostgreSQL Initial Schema for BitCraps
-- Production-ready with proper indexing, constraints, and performance optimizations

-- Enable required extensions
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "btree_gist";
CREATE EXTENSION IF NOT EXISTS "pg_stat_statements";

-- Users table with comprehensive identity management
CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    username VARCHAR(255) NOT NULL UNIQUE,
    email VARCHAR(255) UNIQUE,
    password_hash BYTEA NOT NULL,
    salt BYTEA NOT NULL,
    public_key BYTEA NOT NULL,
    reputation_score INTEGER NOT NULL DEFAULT 1000,
    total_games_played BIGINT NOT NULL DEFAULT 0,
    total_winnings BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_active TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    account_status VARCHAR(20) NOT NULL DEFAULT 'active' CHECK (account_status IN ('active', 'suspended', 'banned', 'pending')),
    kyc_status VARCHAR(20) NOT NULL DEFAULT 'none' CHECK (kyc_status IN ('none', 'pending', 'verified', 'rejected')),
    preferences JSONB DEFAULT '{}',
    
    -- Constraints
    CONSTRAINT username_length CHECK (char_length(username) >= 3 AND char_length(username) <= 50),
    CONSTRAINT reputation_bounds CHECK (reputation_score >= 0 AND reputation_score <= 10000)
);

-- Optimized indexes for users
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_users_username ON users(username);
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_users_email ON users(email);
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_users_reputation ON users(reputation_score DESC);
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_users_last_active ON users(last_active DESC);
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_users_status ON users(account_status, kyc_status);

-- Games table with comprehensive game state tracking
CREATE TABLE IF NOT EXISTS games (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    game_type VARCHAR(50) NOT NULL DEFAULT 'craps',
    status VARCHAR(20) NOT NULL DEFAULT 'waiting' CHECK (status IN ('waiting', 'active', 'completed', 'cancelled', 'disputed')),
    creator_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    max_players INTEGER NOT NULL DEFAULT 8 CHECK (max_players BETWEEN 2 AND 16),
    current_players INTEGER NOT NULL DEFAULT 1 CHECK (current_players >= 0),
    min_bet BIGINT NOT NULL CHECK (min_bet > 0),
    max_bet BIGINT NOT NULL CHECK (max_bet >= min_bet),
    house_edge DECIMAL(5,4) NOT NULL DEFAULT 0.0136 CHECK (house_edge >= 0 AND house_edge <= 0.1),
    total_pot BIGINT NOT NULL DEFAULT 0,
    game_state JSONB NOT NULL DEFAULT '{}',
    consensus_state JSONB NOT NULL DEFAULT '{}',
    dice_results JSONB DEFAULT '[]',
    round_number INTEGER NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    -- Ensure valid state transitions
    CONSTRAINT valid_completion CHECK (
        (status = 'completed' AND completed_at IS NOT NULL) OR 
        (status != 'completed' AND completed_at IS NULL)
    ),
    CONSTRAINT valid_start CHECK (
        (status IN ('active', 'completed', 'cancelled') AND started_at IS NOT NULL) OR 
        (status = 'waiting' AND started_at IS NULL)
    )
);

-- Optimized indexes for games
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_games_status ON games(status, created_at DESC);
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_games_creator ON games(creator_id, created_at DESC);
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_games_waiting ON games(status, max_players, current_players) WHERE status = 'waiting';
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_games_active ON games(status, updated_at) WHERE status = 'active';
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_games_type_status ON games(game_type, status, created_at DESC);

-- Game participants with comprehensive betting tracking
CREATE TABLE IF NOT EXISTS game_participants (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    game_id UUID NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    position INTEGER NOT NULL CHECK (position BETWEEN 1 AND 16),
    joined_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    left_at TIMESTAMPTZ,
    total_bet BIGINT NOT NULL DEFAULT 0,
    total_winnings BIGINT NOT NULL DEFAULT 0,
    status VARCHAR(20) NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'left', 'disconnected', 'kicked')),
    participant_state JSONB DEFAULT '{}',
    
    -- Unique constraints
    UNIQUE(game_id, user_id),
    UNIQUE(game_id, position),
    
    -- Consistency constraints
    CONSTRAINT valid_departure CHECK (
        (status IN ('left', 'disconnected', 'kicked') AND left_at IS NOT NULL) OR 
        (status = 'active' AND left_at IS NULL)
    )
);

-- Indexes for game participants
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_participants_game ON game_participants(game_id, position);
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_participants_user ON game_participants(user_id, joined_at DESC);
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_participants_status ON game_participants(status, joined_at DESC);

-- Bets table with comprehensive betting information
CREATE TABLE IF NOT EXISTS bets (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    game_id UUID NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    participant_id UUID NOT NULL REFERENCES game_participants(id) ON DELETE CASCADE,
    bet_type VARCHAR(50) NOT NULL,
    bet_amount BIGINT NOT NULL CHECK (bet_amount > 0),
    potential_payout BIGINT NOT NULL CHECK (potential_payout >= 0),
    actual_payout BIGINT DEFAULT 0,
    odds_numerator INTEGER NOT NULL DEFAULT 1,
    odds_denominator INTEGER NOT NULL DEFAULT 1,
    round_number INTEGER NOT NULL DEFAULT 1,
    placed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    resolved_at TIMESTAMPTZ,
    status VARCHAR(20) NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'won', 'lost', 'cancelled', 'pushed')),
    bet_data JSONB DEFAULT '{}',
    
    -- Constraints
    CONSTRAINT valid_odds CHECK (odds_numerator > 0 AND odds_denominator > 0),
    CONSTRAINT valid_resolution CHECK (
        (status IN ('won', 'lost', 'cancelled', 'pushed') AND resolved_at IS NOT NULL) OR 
        (status = 'pending' AND resolved_at IS NULL)
    )
);

-- Optimized indexes for bets
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_bets_game ON bets(game_id, round_number, placed_at);
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_bets_user ON bets(user_id, placed_at DESC);
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_bets_status ON bets(status, placed_at DESC);
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_bets_resolution ON bets(resolved_at DESC) WHERE resolved_at IS NOT NULL;

-- Transactions table for comprehensive financial tracking
CREATE TABLE IF NOT EXISTS transactions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    transaction_type VARCHAR(50) NOT NULL,
    amount BIGINT NOT NULL,
    balance_before BIGINT NOT NULL,
    balance_after BIGINT NOT NULL,
    game_id UUID REFERENCES games(id) ON DELETE SET NULL,
    bet_id UUID REFERENCES bets(id) ON DELETE SET NULL,
    reference_id UUID,
    description TEXT,
    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    confirmed_at TIMESTAMPTZ,
    status VARCHAR(20) NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'confirmed', 'failed', 'cancelled')),
    
    -- Ensure balance consistency
    CONSTRAINT balance_calculation CHECK (balance_before + amount = balance_after),
    CONSTRAINT valid_confirmation CHECK (
        (status = 'confirmed' AND confirmed_at IS NOT NULL) OR 
        (status != 'confirmed' AND confirmed_at IS NULL)
    )
);

-- Indexes for transactions
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_transactions_user ON transactions(user_id, created_at DESC);
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_transactions_type ON transactions(transaction_type, created_at DESC);
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_transactions_game ON transactions(game_id, created_at DESC);
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_transactions_status ON transactions(status, created_at DESC);
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_transactions_reference ON transactions(reference_id) WHERE reference_id IS NOT NULL;

-- Consensus messages for distributed game state
CREATE TABLE IF NOT EXISTS consensus_messages (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    game_id UUID NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    sender_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    message_type VARCHAR(50) NOT NULL,
    sequence_number BIGINT NOT NULL,
    round_number INTEGER NOT NULL,
    message_data JSONB NOT NULL,
    signature BYTEA NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    processed_at TIMESTAMPTZ,
    status VARCHAR(20) NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'processed', 'rejected', 'duplicate')),
    
    -- Unique constraint for consensus ordering
    UNIQUE(game_id, sequence_number),
    
    -- Ensure valid processing
    CONSTRAINT valid_processing CHECK (
        (status = 'processed' AND processed_at IS NOT NULL) OR 
        (status != 'processed' AND processed_at IS NULL)
    )
);

-- Indexes for consensus messages
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_consensus_game_seq ON consensus_messages(game_id, sequence_number);
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_consensus_sender ON consensus_messages(sender_id, created_at DESC);
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_consensus_status ON consensus_messages(status, created_at DESC);
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_consensus_type ON consensus_messages(message_type, game_id, sequence_number);

-- Peer connections for mesh networking
CREATE TABLE IF NOT EXISTS peer_connections (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    local_peer_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    remote_peer_id VARCHAR(255) NOT NULL,
    connection_type VARCHAR(20) NOT NULL CHECK (connection_type IN ('bluetooth', 'tcp', 'websocket')),
    transport_info JSONB NOT NULL DEFAULT '{}',
    connection_status VARCHAR(20) NOT NULL DEFAULT 'connecting' CHECK (connection_status IN ('connecting', 'connected', 'disconnected', 'failed')),
    established_at TIMESTAMPTZ,
    last_seen TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    disconnect_reason VARCHAR(255),
    statistics JSONB DEFAULT '{}',
    
    -- Unique constraint per local peer and remote peer
    UNIQUE(local_peer_id, remote_peer_id, connection_type),
    
    -- Valid connection state
    CONSTRAINT valid_connection CHECK (
        (connection_status = 'connected' AND established_at IS NOT NULL) OR 
        (connection_status != 'connected' AND established_at IS NULL)
    )
);

-- Indexes for peer connections
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_peers_local ON peer_connections(local_peer_id, connection_status);
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_peers_remote ON peer_connections(remote_peer_id, connection_status);
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_peers_status ON peer_connections(connection_status, last_seen DESC);

-- System metrics for monitoring and analytics
CREATE TABLE IF NOT EXISTS system_metrics (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    metric_name VARCHAR(100) NOT NULL,
    metric_value DECIMAL(15,6) NOT NULL,
    metric_unit VARCHAR(20),
    component VARCHAR(100) NOT NULL,
    instance_id VARCHAR(255),
    tags JSONB DEFAULT '{}',
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    -- Performance index
    INDEX USING BRIN (recorded_at)
);

-- Partitioned index for system metrics (time-based)
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_metrics_name_time ON system_metrics(metric_name, recorded_at DESC);
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_metrics_component ON system_metrics(component, recorded_at DESC);

-- Audit log for security and compliance
CREATE TABLE IF NOT EXISTS audit_log (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    action VARCHAR(100) NOT NULL,
    resource_type VARCHAR(50) NOT NULL,
    resource_id UUID,
    old_values JSONB,
    new_values JSONB,
    ip_address INET,
    user_agent TEXT,
    session_id VARCHAR(255),
    success BOOLEAN NOT NULL DEFAULT true,
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    -- Performance index for time-series data
    INDEX USING BRIN (created_at)
);

-- Indexes for audit log
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_audit_user ON audit_log(user_id, created_at DESC);
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_audit_action ON audit_log(action, created_at DESC);
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_audit_resource ON audit_log(resource_type, resource_id, created_at DESC);

-- Create triggers for updated_at columns
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Apply triggers
CREATE TRIGGER update_users_updated_at BEFORE UPDATE ON users FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_games_updated_at BEFORE UPDATE ON games FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Create views for common queries
CREATE OR REPLACE VIEW active_games AS
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

CREATE OR REPLACE VIEW user_statistics AS
SELECT 
    u.id,
    u.username,
    u.reputation_score,
    COUNT(DISTINCT gp.game_id) as games_played,
    COALESCE(SUM(t.amount) FILTER (WHERE t.amount > 0), 0) as total_winnings,
    COALESCE(SUM(t.amount) FILTER (WHERE t.amount < 0), 0) as total_losses,
    COUNT(DISTINCT b.id) as total_bets,
    COUNT(DISTINCT CASE WHEN g.status = 'completed' THEN g.id END) as completed_games
FROM users u
LEFT JOIN game_participants gp ON u.id = gp.user_id
LEFT JOIN games g ON gp.game_id = g.id
LEFT JOIN transactions t ON u.id = t.user_id AND t.transaction_type IN ('bet_win', 'bet_loss')
LEFT JOIN bets b ON u.id = b.user_id
GROUP BY u.id, u.username, u.reputation_score;

-- Performance optimization: Enable parallel query execution
ALTER SYSTEM SET max_parallel_workers_per_gather = 4;
ALTER SYSTEM SET max_parallel_workers = 8;

-- Set optimal work_mem for complex queries
ALTER SYSTEM SET work_mem = '256MB';

-- Enable query plan caching
ALTER SYSTEM SET shared_preload_libraries = 'pg_stat_statements';

-- Create stored procedure for efficient game creation
CREATE OR REPLACE FUNCTION create_game_with_participant(
    p_creator_id UUID,
    p_game_type VARCHAR DEFAULT 'craps',
    p_max_players INTEGER DEFAULT 8,
    p_min_bet BIGINT DEFAULT 10,
    p_max_bet BIGINT DEFAULT 1000
) RETURNS UUID AS $$
DECLARE
    v_game_id UUID;
BEGIN
    -- Insert game
    INSERT INTO games (creator_id, game_type, max_players, min_bet, max_bet)
    VALUES (p_creator_id, p_game_type, p_max_players, p_min_bet, p_max_bet)
    RETURNING id INTO v_game_id;
    
    -- Add creator as first participant
    INSERT INTO game_participants (game_id, user_id, position)
    VALUES (v_game_id, p_creator_id, 1);
    
    RETURN v_game_id;
END;
$$ LANGUAGE plpgsql;

-- Function for atomic bet placement
CREATE OR REPLACE FUNCTION place_bet_atomic(
    p_game_id UUID,
    p_user_id UUID,
    p_bet_type VARCHAR,
    p_bet_amount BIGINT,
    p_potential_payout BIGINT DEFAULT NULL
) RETURNS UUID AS $$
DECLARE
    v_bet_id UUID;
    v_participant_id UUID;
    v_user_balance BIGINT;
BEGIN
    -- Get participant ID
    SELECT id INTO v_participant_id
    FROM game_participants
    WHERE game_id = p_game_id AND user_id = p_user_id AND status = 'active';
    
    IF v_participant_id IS NULL THEN
        RAISE EXCEPTION 'User is not an active participant in this game';
    END IF;
    
    -- Check user balance (simplified - would integrate with balance system)
    -- This would typically check a user_balances table
    
    -- Calculate potential payout if not provided
    IF p_potential_payout IS NULL THEN
        p_potential_payout := p_bet_amount * 2; -- Default 2:1 odds
    END IF;
    
    -- Insert bet
    INSERT INTO bets (game_id, user_id, participant_id, bet_type, bet_amount, potential_payout)
    VALUES (p_game_id, p_user_id, v_participant_id, p_bet_type, p_bet_amount, p_potential_payout)
    RETURNING id INTO v_bet_id;
    
    -- Update participant total bet
    UPDATE game_participants
    SET total_bet = total_bet + p_bet_amount
    WHERE id = v_participant_id;
    
    -- Update game total pot
    UPDATE games
    SET total_pot = total_pot + p_bet_amount
    WHERE id = p_game_id;
    
    RETURN v_bet_id;
END;
$$ LANGUAGE plpgsql;

-- Initial data setup
INSERT INTO users (username, password_hash, salt, public_key) VALUES
    ('system', decode('deadbeef', 'hex'), decode('cafebabe', 'hex'), decode('feedface', 'hex'))
ON CONFLICT (username) DO NOTHING;

-- Create database-specific sequences for high-performance ID generation
CREATE SEQUENCE IF NOT EXISTS game_sequence_numbers START 1 INCREMENT 1;
CREATE SEQUENCE IF NOT EXISTS consensus_sequence_numbers START 1 INCREMENT 1;

COMMIT;