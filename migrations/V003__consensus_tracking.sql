-- Consensus mechanism tracking
-- Version: 003
-- Description: Byzantine fault tolerant consensus state and voting

CREATE TABLE IF NOT EXISTS consensus_rounds (
    round_number INTEGER PRIMARY KEY,
    game_id TEXT REFERENCES games(id) ON DELETE CASCADE,
    proposer_id TEXT NOT NULL,
    proposal_hash BLOB NOT NULL,
    proposal_data TEXT,
    vote_threshold INTEGER NOT NULL DEFAULT 3,
    vote_count INTEGER DEFAULT 0,
    positive_votes INTEGER DEFAULT 0,
    negative_votes INTEGER DEFAULT 0,
    finalized INTEGER DEFAULT 0,
    consensus_type TEXT NOT NULL CHECK(consensus_type IN ('game_state', 'bet_resolution', 'payout', 'dispute')),
    created_at INTEGER NOT NULL,
    voting_deadline INTEGER,
    finalized_at INTEGER
);

CREATE TABLE IF NOT EXISTS consensus_votes (
    id TEXT PRIMARY KEY,
    round_number INTEGER REFERENCES consensus_rounds(round_number) ON DELETE CASCADE,
    voter_id TEXT NOT NULL,
    vote_type TEXT NOT NULL CHECK(vote_type IN ('approve', 'reject', 'abstain')),
    vote_hash BLOB NOT NULL,
    signature BLOB NOT NULL,
    vote_weight REAL DEFAULT 1.0,
    reasoning TEXT,
    created_at INTEGER NOT NULL,
    UNIQUE(round_number, voter_id)
);

-- Validator reputation and history
CREATE TABLE IF NOT EXISTS validator_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    validator_id TEXT NOT NULL,
    round_number INTEGER REFERENCES consensus_rounds(round_number),
    participation_type TEXT CHECK(participation_type IN ('proposer', 'voter', 'observer')),
    was_correct INTEGER,
    reputation_change REAL DEFAULT 0,
    stake_amount INTEGER DEFAULT 0,
    created_at INTEGER NOT NULL
);

-- Byzantine behavior detection
CREATE TABLE IF NOT EXISTS byzantine_incidents (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    accused_peer_id TEXT NOT NULL,
    incident_type TEXT NOT NULL CHECK(incident_type IN ('double_vote', 'equivocation', 'invalid_proposal', 'timeout')),
    round_number INTEGER REFERENCES consensus_rounds(round_number),
    evidence TEXT,
    reporter_id TEXT,
    severity REAL DEFAULT 1.0,
    confirmed INTEGER DEFAULT 0,
    created_at INTEGER NOT NULL
);

-- Consensus performance metrics
CREATE TABLE IF NOT EXISTS consensus_metrics (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    round_number INTEGER REFERENCES consensus_rounds(round_number),
    total_validators INTEGER NOT NULL,
    participating_validators INTEGER NOT NULL,
    consensus_time_ms INTEGER,
    network_rounds INTEGER DEFAULT 1,
    created_at INTEGER NOT NULL
);

-- Indices for consensus tracking
CREATE INDEX IF NOT EXISTS idx_consensus_rounds_game_id ON consensus_rounds(game_id);
CREATE INDEX IF NOT EXISTS idx_consensus_rounds_proposer ON consensus_rounds(proposer_id);
CREATE INDEX IF NOT EXISTS idx_consensus_rounds_finalized ON consensus_rounds(finalized);
CREATE INDEX IF NOT EXISTS idx_consensus_rounds_type ON consensus_rounds(consensus_type);
CREATE INDEX IF NOT EXISTS idx_consensus_rounds_created_at ON consensus_rounds(created_at);

CREATE INDEX IF NOT EXISTS idx_consensus_votes_round ON consensus_votes(round_number);
CREATE INDEX IF NOT EXISTS idx_consensus_votes_voter ON consensus_votes(voter_id);
CREATE INDEX IF NOT EXISTS idx_consensus_votes_type ON consensus_votes(vote_type);

CREATE INDEX IF NOT EXISTS idx_validator_history_validator ON validator_history(validator_id);
CREATE INDEX IF NOT EXISTS idx_validator_history_round ON validator_history(round_number);
CREATE INDEX IF NOT EXISTS idx_validator_history_correct ON validator_history(was_correct);

CREATE INDEX IF NOT EXISTS idx_byzantine_incidents_accused ON byzantine_incidents(accused_peer_id);
CREATE INDEX IF NOT EXISTS idx_byzantine_incidents_type ON byzantine_incidents(incident_type);
CREATE INDEX IF NOT EXISTS idx_byzantine_incidents_confirmed ON byzantine_incidents(confirmed);

CREATE INDEX IF NOT EXISTS idx_consensus_metrics_round ON consensus_metrics(round_number);
CREATE INDEX IF NOT EXISTS idx_consensus_metrics_created_at ON consensus_metrics(created_at);