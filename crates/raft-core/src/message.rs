//! # message
//!
//! why: define all raft rpc message types for node communication
//! relations: used by node.rs for state transitions, serialized for network
//! what: VoteRequest, VoteResponse, AppendEntries, Heartbeat messages

use serde::{Deserialize, Serialize};

/// All possible Raft messages between nodes
/// 
/// TODO: Implement full message handling in Phase 2 (feature/raft-core)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RaftMessage {
    /// Request a vote during leader election
    VoteRequest {
        term: u64,
        candidate_id: u64,
        last_log_index: u64,
        last_log_term: u64,
    },
    /// Response to a vote request
    VoteResponse {
        term: u64,
        vote_granted: bool,
    },
    /// Replicate log entries (also serves as heartbeat when entries is empty)
    AppendEntries {
        term: u64,
        leader_id: u64,
        prev_log_index: u64,
        prev_log_term: u64,
        entries: Vec<crate::LogEntry>,
        leader_commit: u64,
    },
    /// Response to AppendEntries
    AppendEntriesResponse {
        term: u64,
        success: bool,
    },
}
