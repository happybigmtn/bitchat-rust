-- Initial BitCraps database schema
-- Version: 001
-- Description: Core game infrastructure tables

-- Users table
CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY,
    username TEXT UNIQUE NOT NULL,
    public_key BLOB NOT NULL,
    reputation REAL DEFAULT 0.0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    is_active INTEGER DEFAULT 1,
    last_seen_at INTEGER
);

-- Games table
CREATE TABLE IF NOT EXISTS games (
    id TEXT PRIMARY KEY,
    state TEXT NOT NULL CHECK(state IN ('waiting', 'playing', 'completed', 'cancelled')),
    pot_size INTEGER DEFAULT 0 CHECK(pot_size >= 0),
    phase TEXT NOT NULL CHECK(phase IN ('betting', 'rolling', 'resolved')),
    created_at INTEGER NOT NULL,
    completed_at INTEGER,
    winner_id TEXT REFERENCES users(id),
    game_type TEXT NOT NULL DEFAULT 'craps',
    metadata TEXT,
    max_players INTEGER DEFAULT 8,
    min_bet INTEGER DEFAULT 10
);

-- Bets table
CREATE TABLE IF NOT EXISTS bets (
    id BLOB PRIMARY KEY,
    game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    player_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    bet_type TEXT NOT NULL CHECK(bet_type IN ('pass_line', 'dont_pass', 'come', 'dont_come', 'field', 'place_6', 'place_8')),
    amount INTEGER NOT NULL CHECK(amount > 0),
    odds_multiplier REAL DEFAULT 1.0,
    outcome TEXT CHECK(outcome IS NULL OR outcome IN ('win', 'lose', 'push')),
    payout INTEGER DEFAULT 0,
    created_at INTEGER NOT NULL,
    resolved_at INTEGER
);

-- Transactions table
CREATE TABLE IF NOT EXISTS transactions (
    id TEXT PRIMARY KEY,
    from_user_id TEXT REFERENCES users(id),
    to_user_id TEXT REFERENCES users(id),
    amount INTEGER NOT NULL CHECK(amount > 0),
    transaction_type TEXT NOT NULL CHECK(transaction_type IN ('transfer', 'bet_placed', 'bet_payout', 'deposit', 'withdrawal')),
    status TEXT NOT NULL CHECK(status IN ('pending', 'confirmed', 'failed', 'cancelled')) DEFAULT 'pending',
    created_at INTEGER NOT NULL,
    confirmed_at INTEGER,
    block_height INTEGER,
    tx_hash TEXT UNIQUE,
    fee INTEGER DEFAULT 0
);

-- Indices for performance
CREATE INDEX IF NOT EXISTS idx_users_username ON users(username);
CREATE INDEX IF NOT EXISTS idx_users_reputation ON users(reputation DESC);
CREATE INDEX IF NOT EXISTS idx_users_active ON users(is_active);

CREATE INDEX IF NOT EXISTS idx_games_state ON games(state);
CREATE INDEX IF NOT EXISTS idx_games_created_at ON games(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_games_phase ON games(phase);
CREATE INDEX IF NOT EXISTS idx_games_type ON games(game_type);

CREATE INDEX IF NOT EXISTS idx_bets_game_id ON bets(game_id);
CREATE INDEX IF NOT EXISTS idx_bets_player_id ON bets(player_id);
CREATE INDEX IF NOT EXISTS idx_bets_created_at ON bets(created_at);
CREATE INDEX IF NOT EXISTS idx_bets_outcome ON bets(outcome);

CREATE INDEX IF NOT EXISTS idx_transactions_from_user ON transactions(from_user_id);
CREATE INDEX IF NOT EXISTS idx_transactions_to_user ON transactions(to_user_id);
CREATE INDEX IF NOT EXISTS idx_transactions_status ON transactions(status);
CREATE INDEX IF NOT EXISTS idx_transactions_type ON transactions(transaction_type);
CREATE INDEX IF NOT EXISTS idx_transactions_created_at ON transactions(created_at DESC);