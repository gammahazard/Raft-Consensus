//! # raft-storage
//!
//! why: provide durable persistence for raft state using standard rust fs apis
//! relations: used by raft-core for state persistence, mapped to indexeddb via wasi
//! what: Storage trait, FileStorage implementation, crash recovery

// TODO: Implement in Phase 3 (feature/storage-layer)

use raft_core::LogEntry;
use std::io;

/// Trait for durable storage of Raft state
/// 
/// This abstraction allows the same code to work with:
/// - Real filesystem (native)  
/// - IndexedDB (browser via WASI)
/// - In-memory (testing)
pub trait Storage {
    /// Persist the current term and voted_for
    fn save_term_and_vote(&mut self, term: u64, voted_for: Option<u64>) -> io::Result<()>;
    
    /// Load the persisted term and voted_for
    fn load_term_and_vote(&self) -> io::Result<(u64, Option<u64>)>;
    
    /// Append entries to the log
    fn append_entries(&mut self, entries: &[LogEntry]) -> io::Result<()>;
    
    /// Load all log entries (for crash recovery)
    fn load_log(&self) -> io::Result<Vec<LogEntry>>;
}

/// File-based storage implementation using std::fs
/// 
/// TODO: Implement in Phase 3 (feature/storage-layer)
pub struct FileStorage {
    /// Directory path for storing state files
    pub dir: std::path::PathBuf,
}

impl FileStorage {
    /// Create a new FileStorage at the given directory
    pub fn new(dir: impl Into<std::path::PathBuf>) -> Self {
        Self { dir: dir.into() }
    }
}
