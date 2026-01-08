//! # log
//!
//! why: manage the append-only log of commands that raft replicates
//! relations: used by node.rs for replication, persisted via raft-storage
//! what: LogEntry struct, log consistency checking, commit index management

use serde::{Deserialize, Serialize};

/// A single entry in the replicated log
/// 
/// TODO: Implement log management in Phase 2 (feature/raft-core)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// The term when this entry was created
    pub term: u64,
    /// The index of this entry in the log (1-indexed)
    pub index: u64,
    /// The command to be applied to the state machine
    pub command: Vec<u8>,
}

impl LogEntry {
    /// Create a new log entry
    pub fn new(term: u64, index: u64, command: Vec<u8>) -> Self {
        Self { term, index, command }
    }
}
