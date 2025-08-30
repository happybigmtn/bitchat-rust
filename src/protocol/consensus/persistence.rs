//! Consensus State Persistence
//!
//! Provides persistent storage for consensus state with crash recovery,
//! write-ahead logging, and checkpoint management.

use crate::error::{Error, Result};
use crate::protocol::{Hash256, PeerId};
use rusqlite::{params, Connection, Result as SqlResult};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// Consensus state version for upgrade compatibility
pub const CONSENSUS_DB_VERSION: u32 = 1;

/// Checkpoint interval (every N rounds)
pub const CHECKPOINT_INTERVAL: u64 = 100;

/// Maximum WAL size before checkpoint
pub const MAX_WAL_SIZE: usize = 10_000_000; // 10MB

/// Consensus state checkpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusCheckpoint {
    pub round_id: u64,
    pub state_hash: Hash256,
    pub participant_signatures: Vec<(PeerId, Vec<u8>)>, // Signatures as Vec<u8>
    pub timestamp: u64,
    pub game_state: Vec<u8>, // Serialized game state
    pub version: u32,
}

/// Write-ahead log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalEntry {
    pub sequence: u64,
    pub operation: ConsensusOperation,
    pub timestamp: u64,
    pub hash: Hash256,
}

/// Consensus operations for WAL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConsensusOperation {
    StateUpdate {
        round_id: u64,
        state: Vec<u8>,
    },
    VoteReceived {
        round_id: u64,
        voter: PeerId,
        vote: Vec<u8>,
    },
    ProposalSubmitted {
        round_id: u64,
        proposal: Vec<u8>,
    },
    DisputeRaised {
        dispute_id: Hash256,
        claim: Vec<u8>,
    },
    CheckpointCreated {
        checkpoint: ConsensusCheckpoint,
    },
}

/// Consensus persistence manager
pub struct ConsensusPersistence {
    /// SQLite database connection
    db: Arc<Mutex<Connection>>,

    /// Write-ahead log
    wal: Arc<Mutex<WriteAheadLog>>,

    /// Storage path
    _storage_path: PathBuf,

    /// Current sequence number
    sequence: Arc<Mutex<u64>>,
}

impl ConsensusPersistence {
    /// Create or open consensus persistence
    pub fn new<P: AsRef<Path>>(storage_path: P) -> Result<Self> {
        let storage_path = storage_path.as_ref().to_path_buf();

        // Ensure directory exists
        if let Some(parent) = storage_path.parent() {
            fs::create_dir_all(parent).map_err(|e| Error::IoError(e.to_string()))?;
        }

        // Open SQLite database with WAL mode
        let db = Connection::open(&storage_path).map_err(|e| Error::IoError(e.to_string()))?;

        // Enable WAL mode for better concurrency
        db.execute("PRAGMA journal_mode=WAL", [])
            .map_err(|e| Error::IoError(e.to_string()))?;

        // Create tables
        Self::create_tables(&db)?;

        // Initialize WAL
        let wal_path = storage_path.with_extension("wal");
        let wal = WriteAheadLog::new(wal_path)?;

        // Get current sequence number
        let sequence = Self::get_max_sequence(&db)?;

        Ok(Self {
            db: Arc::new(Mutex::new(db)),
            wal: Arc::new(Mutex::new(wal)),
            _storage_path: storage_path,
            sequence: Arc::new(Mutex::new(sequence)),
        })
    }

    /// Create database tables
    fn create_tables(db: &Connection) -> Result<()> {
        // Consensus state table
        db.execute(
            "CREATE TABLE IF NOT EXISTS consensus_state (
                round_id INTEGER PRIMARY KEY,
                state_hash BLOB NOT NULL,
                state_data BLOB NOT NULL,
                timestamp INTEGER NOT NULL,
                confirmations INTEGER DEFAULT 0,
                is_finalized INTEGER DEFAULT 0
            )",
            [],
        )
        .map_err(|e| Error::IoError(e.to_string()))?;

        // Votes table
        db.execute(
            "CREATE TABLE IF NOT EXISTS consensus_votes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                round_id INTEGER NOT NULL,
                voter BLOB NOT NULL,
                vote_data BLOB NOT NULL,
                signature BLOB NOT NULL,
                timestamp INTEGER NOT NULL,
                FOREIGN KEY(round_id) REFERENCES consensus_state(round_id)
            )",
            [],
        )
        .map_err(|e| Error::IoError(e.to_string()))?;

        // Checkpoints table
        db.execute(
            "CREATE TABLE IF NOT EXISTS consensus_checkpoints (
                checkpoint_id INTEGER PRIMARY KEY,
                round_id INTEGER NOT NULL,
                state_hash BLOB NOT NULL,
                checkpoint_data BLOB NOT NULL,
                timestamp INTEGER NOT NULL,
                version INTEGER NOT NULL
            )",
            [],
        )
        .map_err(|e| Error::IoError(e.to_string()))?;

        // Create indices
        db.execute(
            "CREATE INDEX IF NOT EXISTS idx_votes_round ON consensus_votes(round_id)",
            [],
        )
        .map_err(|e| Error::IoError(e.to_string()))?;

        Ok(())
    }

    /// Get maximum sequence number
    fn get_max_sequence(db: &Connection) -> Result<u64> {
        let result: SqlResult<i64> =
            db.query_row("SELECT MAX(round_id) FROM consensus_state", [], |row| {
                row.get(0)
            });

        Ok(result.unwrap_or(0) as u64)
    }

    /// Store consensus state
    pub fn store_consensus_state(
        &self,
        round_id: u64,
        state_hash: Hash256,
        state_data: &[u8],
    ) -> Result<()> {
        // Write to WAL first
        let wal_entry = WalEntry {
            sequence: self.next_sequence(),
            operation: ConsensusOperation::StateUpdate {
                round_id,
                state: state_data.to_vec(),
            },
            timestamp: current_timestamp(),
            hash: state_hash,
        };

        self.wal.lock().unwrap().append(wal_entry)?;

        // Write to database
        let db = self.db.lock().unwrap();
        db.execute(
            "INSERT OR REPLACE INTO consensus_state 
             (round_id, state_hash, state_data, timestamp, confirmations, is_finalized)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                round_id as i64,
                state_hash.as_ref(),
                state_data,
                current_timestamp() as i64,
                0i64,
                0i64
            ],
        )
        .map_err(|e| Error::IoError(e.to_string()))?;

        // Check if checkpoint needed
        if round_id % CHECKPOINT_INTERVAL == 0 {
            self.create_checkpoint(round_id)?;
        }

        Ok(())
    }

    /// Load consensus state
    pub fn load_consensus_state(&self, round_id: u64) -> Result<Option<Vec<u8>>> {
        let db = self.db.lock().unwrap();

        let result: SqlResult<Vec<u8>> = db.query_row(
            "SELECT state_data FROM consensus_state WHERE round_id = ?1",
            params![round_id as i64],
            |row| row.get(0),
        );

        match result {
            Ok(data) => Ok(Some(data)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(Error::IoError(e.to_string())),
        }
    }

    /// Get latest consensus state
    pub fn load_latest_state(&self) -> Result<Option<(u64, Vec<u8>)>> {
        let db = self.db.lock().unwrap();

        let result: SqlResult<(i64, Vec<u8>)> = db.query_row(
            "SELECT round_id, state_data FROM consensus_state 
             ORDER BY round_id DESC LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        );

        match result {
            Ok((round_id, data)) => Ok(Some((round_id as u64, data))),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(Error::IoError(e.to_string())),
        }
    }

    /// Store consensus vote
    pub fn store_vote(
        &self,
        round_id: u64,
        voter: PeerId,
        vote_data: &[u8],
        signature: &[u8; 64],
    ) -> Result<()> {
        // Write to WAL
        let wal_entry = WalEntry {
            sequence: self.next_sequence(),
            operation: ConsensusOperation::VoteReceived {
                round_id,
                voter,
                vote: vote_data.to_vec(),
            },
            timestamp: current_timestamp(),
            hash: crate::crypto::GameCrypto::hash(vote_data),
        };

        self.wal.lock().unwrap().append(wal_entry)?;

        // Write to database
        let db = self.db.lock().unwrap();
        db.execute(
            "INSERT INTO consensus_votes 
             (round_id, voter, vote_data, signature, timestamp)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                round_id as i64,
                voter.as_ref(),
                vote_data,
                signature.as_ref(),
                current_timestamp() as i64
            ],
        )
        .map_err(|e| Error::IoError(e.to_string()))?;

        Ok(())
    }

    /// Get votes for a round
    pub fn get_votes(&self, round_id: u64) -> Result<Vec<(PeerId, Vec<u8>, [u8; 64])>> {
        let db = self.db.lock().unwrap();

        let mut stmt = db
            .prepare(
                "SELECT voter, vote_data, signature FROM consensus_votes 
             WHERE round_id = ?1 ORDER BY timestamp",
            )
            .map_err(|e| Error::IoError(e.to_string()))?;

        let votes = stmt
            .query_map(params![round_id as i64], |row| {
                let voter_bytes: Vec<u8> = row.get(0)?;
                let vote_data: Vec<u8> = row.get(1)?;
                let sig_bytes: Vec<u8> = row.get(2)?;

                let mut voter = [0u8; 32];
                voter.copy_from_slice(&voter_bytes[..32]);

                let mut signature = [0u8; 64];
                signature.copy_from_slice(&sig_bytes[..64]);

                Ok((voter, vote_data, signature))
            })
            .map_err(|e| Error::IoError(e.to_string()))?;

        votes
            .collect::<SqlResult<Vec<_>>>()
            .map_err(|e| Error::IoError(e.to_string()))
    }

    /// Create checkpoint
    pub fn create_checkpoint(&self, round_id: u64) -> Result<()> {
        let state = self
            .load_consensus_state(round_id)?
            .ok_or_else(|| Error::InvalidState("No state to checkpoint".into()))?;

        let checkpoint = ConsensusCheckpoint {
            round_id,
            state_hash: crate::crypto::GameCrypto::hash(&state),
            participant_signatures: Vec::new(), // Would be filled with actual signatures
            timestamp: current_timestamp(),
            game_state: state,
            version: CONSENSUS_DB_VERSION,
        };

        // Store checkpoint
        let db = self.db.lock().unwrap();
        let checkpoint_data =
            bincode::serialize(&checkpoint).map_err(|e| Error::Serialization(e.to_string()))?;

        db.execute(
            "INSERT INTO consensus_checkpoints 
             (checkpoint_id, round_id, state_hash, checkpoint_data, timestamp, version)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                (round_id / CHECKPOINT_INTERVAL) as i64,
                round_id as i64,
                checkpoint.state_hash.as_ref(),
                checkpoint_data,
                checkpoint.timestamp as i64,
                CONSENSUS_DB_VERSION as i64
            ],
        )
        .map_err(|e| Error::IoError(e.to_string()))?;

        // Prune old data
        self.prune_old_data(round_id)?;

        Ok(())
    }

    /// Load latest checkpoint
    pub fn load_latest_checkpoint(&self) -> Result<Option<ConsensusCheckpoint>> {
        let db = self.db.lock().unwrap();

        let result: SqlResult<Vec<u8>> = db.query_row(
            "SELECT checkpoint_data FROM consensus_checkpoints 
             ORDER BY checkpoint_id DESC LIMIT 1",
            [],
            |row| row.get(0),
        );

        match result {
            Ok(data) => {
                let checkpoint = bincode::deserialize(&data)
                    .map_err(|e| Error::DeserializationError(e.to_string()))?;
                Ok(Some(checkpoint))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(Error::IoError(e.to_string())),
        }
    }

    /// Prune old data before checkpoint
    fn prune_old_data(&self, checkpoint_round: u64) -> Result<()> {
        let cutoff = checkpoint_round.saturating_sub(CHECKPOINT_INTERVAL * 2);

        let db = self.db.lock().unwrap();

        // Delete old votes
        db.execute(
            "DELETE FROM consensus_votes WHERE round_id < ?1",
            params![cutoff as i64],
        )
        .map_err(|e| Error::IoError(e.to_string()))?;

        // Delete old states (keep checkpoint states)
        db.execute(
            "DELETE FROM consensus_state WHERE round_id < ?1 
             AND round_id NOT IN (SELECT round_id FROM consensus_checkpoints)",
            params![cutoff as i64],
        )
        .map_err(|e| Error::IoError(e.to_string()))?;

        Ok(())
    }

    /// Get next sequence number
    fn next_sequence(&self) -> u64 {
        let mut seq = self.sequence.lock().unwrap();
        *seq += 1;
        *seq
    }

    /// Recover from crash using WAL
    pub fn recover(&self) -> Result<()> {
        let wal = self.wal.lock().unwrap();
        let entries = wal.read_all()?;

        for entry in entries {
            match entry.operation {
                ConsensusOperation::StateUpdate { round_id, state } => {
                    self.store_consensus_state(round_id, entry.hash, &state)?;
                }
                ConsensusOperation::VoteReceived {
                    round_id,
                    voter,
                    vote,
                } => {
                    // Extract signature from vote data (simplified)
                    let signature = [0u8; 64];
                    self.store_vote(round_id, voter, &vote, &signature)?;
                }
                _ => {
                    // Handle other operations
                }
            }
        }

        Ok(())
    }
}

/// Write-ahead log implementation
struct WriteAheadLog {
    path: PathBuf,
    current_size: usize,
}

impl WriteAheadLog {
    fn new(path: PathBuf) -> Result<Self> {
        Ok(Self {
            path,
            current_size: 0,
        })
    }

    fn append(&mut self, entry: WalEntry) -> Result<()> {
        let data = bincode::serialize(&entry).map_err(|e| Error::Serialization(e.to_string()))?;

        let file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|e| Error::IoError(e.to_string()))?;

        let mut writer = BufWriter::new(file);

        // Write length prefix
        writer
            .write_all(&(data.len() as u32).to_le_bytes())
            .map_err(|e| Error::IoError(e.to_string()))?;

        // Write data
        writer
            .write_all(&data)
            .map_err(|e| Error::IoError(e.to_string()))?;

        writer.flush().map_err(|e| Error::IoError(e.to_string()))?;

        self.current_size += data.len() + 4;

        // Check if rotation needed
        if self.current_size > MAX_WAL_SIZE {
            self.rotate()?;
        }

        Ok(())
    }

    fn read_all(&self) -> Result<Vec<WalEntry>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }

        let data = fs::read(&self.path).map_err(|e| Error::IoError(e.to_string()))?;

        let mut entries = Vec::new();
        let mut cursor = 0;

        while cursor < data.len() {
            // Read length
            if cursor + 4 > data.len() {
                break;
            }

            let mut len_bytes = [0u8; 4];
            len_bytes.copy_from_slice(&data[cursor..cursor + 4]);
            let len = u32::from_le_bytes(len_bytes) as usize;
            cursor += 4;

            // Read entry
            if cursor + len > data.len() {
                break;
            }

            let entry: WalEntry = bincode::deserialize(&data[cursor..cursor + len])
                .map_err(|e| Error::DeserializationError(e.to_string()))?;

            entries.push(entry);
            cursor += len;
        }

        Ok(entries)
    }

    fn rotate(&mut self) -> Result<()> {
        let backup_path = self.path.with_extension("wal.old");

        if self.path.exists() {
            fs::rename(&self.path, backup_path).map_err(|e| Error::IoError(e.to_string()))?;
        }

        self.current_size = 0;
        Ok(())
    }
}

/// Get current timestamp
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_persistence_create() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("consensus.db");

        let persistence = ConsensusPersistence::new(&path).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn test_store_and_load_state() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("consensus.db");

        let persistence = ConsensusPersistence::new(&path).unwrap();

        let state_data = b"test state data";
        let state_hash = crate::crypto::GameCrypto::hash(state_data);

        persistence
            .store_consensus_state(1, state_hash, state_data)
            .unwrap();

        let loaded = persistence.load_consensus_state(1).unwrap();
        assert_eq!(loaded, Some(state_data.to_vec()));
    }

    #[test]
    fn test_checkpoint_creation() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("consensus.db");

        let persistence = ConsensusPersistence::new(&path).unwrap();

        // Store states up to checkpoint
        for i in 1..=CHECKPOINT_INTERVAL {
            let state = format!("state {}", i);
            let hash = crate::crypto::GameCrypto::hash(state.as_bytes());
            persistence
                .store_consensus_state(i, hash, state.as_bytes())
                .unwrap();
        }

        // Checkpoint should have been created automatically
        let checkpoint = persistence.load_latest_checkpoint().unwrap();
        assert!(checkpoint.is_some());
        assert_eq!(checkpoint.unwrap().round_id, CHECKPOINT_INTERVAL);
    }
}
