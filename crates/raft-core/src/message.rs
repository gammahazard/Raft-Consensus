//! # message
//!
//! why: define all raft rpc message types for node communication
//! relations: used by node.rs for state transitions, serialized for network
//! what: VoteRequest, VoteResponse, AppendEntries, PreVote messages

use serde::{Deserialize, Serialize};

/// All possible Raft messages between nodes
/// 
/// Includes PreVote messages (Raft thesis Section 9.6) to prevent the
/// "disruptive server" problem where a disconnected node raises its term
/// and disrupts the cluster when it rejoins.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RaftMessage {
    // -- PreVote Phase (prevents disruptive server problem) --
    
    /// Pre-vote request: "Would you vote for me IF I ran for election?"
    /// 
    /// This is sent BEFORE incrementing term. If a majority says no,
    /// the node doesn't start a real election, preventing term inflation
    /// from disconnected nodes.
    PreVoteRequest {
        /// The term the candidate WOULD use if it started an election
        /// (current_term + 1, but NOT actually incremented yet)
        term: u64,
        candidate_id: u64,
        last_log_index: u64,
        last_log_term: u64,
    },
    /// Response to a pre-vote request
    PreVoteResponse {
        term: u64,
        /// Would this node vote for the candidate if a real election started?
        vote_granted: bool,
    },
    
    // -- Standard Raft Messages --
    
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

