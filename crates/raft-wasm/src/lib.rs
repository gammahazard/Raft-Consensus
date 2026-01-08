//! # raft-wasm
//!
//! WASI 0.2 Component Model wrapper for raft-core.
//! This component can run in:
//! - Browser (via jco transpilation → JavaScript shims)
//! - Raspberry Pi (via Wasmtime → real TCP/filesystem)

use std::cell::RefCell;

// Re-export core types
pub use raft_core::{NodeState, RaftNode, RaftMessage, LogEntry, RaftConfig};
pub use raft_storage::InMemoryStorage;

// Include generated bindings
#[allow(warnings)]
mod bindings;

// Use the generated types
use bindings::raft::consensus::types::{
    NodeState as WitNodeState,
    NodeStatus,
    RaftMessage as WitRaftMessage,
    PreVoteRequest,
    PreVoteResponse,
    VoteRequest,
    VoteResponse,
    AppendEntries,
    AppendEntriesResponse,
    LogEntry as WitLogEntry,
};

use bindings::exports::raft::consensus::raft_api::Guest;

// Thread-local storage for the Raft node instance
thread_local! {
    static NODE: RefCell<Option<RaftNode>> = RefCell::new(None);
}

// Convert between WIT types and internal types
fn to_wit_state(state: NodeState) -> WitNodeState {
    match state {
        NodeState::Follower => WitNodeState::Follower,
        NodeState::Candidate | NodeState::PreCandidate => WitNodeState::Candidate,
        NodeState::Leader => WitNodeState::Leader,
    }
}

fn to_wit_log_entry(entry: &LogEntry) -> WitLogEntry {
    WitLogEntry {
        term: entry.term,
        index: entry.index,
        command: entry.command.clone(),
    }
}

fn from_wit_log_entry(entry: &WitLogEntry) -> LogEntry {
    LogEntry {
        term: entry.term,
        index: entry.index,
        command: entry.command.clone(),
    }
}

// Implementation of the component exports
struct RaftNodeComponent;

impl Guest for RaftNodeComponent {
    fn init(node_id: u64, node_ids: Vec<u64>) {
        let config = RaftConfig::default();
        let node = RaftNode::with_config(node_id, node_ids, config);
        
        NODE.with(|n| {
            *n.borrow_mut() = Some(node);
        });
    }

    fn tick() -> NodeStatus {
        NODE.with(|n| {
            let node_ref = n.borrow();
            if let Some(ref node) = *node_ref {
                get_node_status(node)
            } else {
                dead_status()
            }
        })
    }

    fn on_message(from_node: u64, msg: WitRaftMessage) {
        NODE.with(|n| {
            let mut node_ref = n.borrow_mut();
            if let Some(ref mut node) = *node_ref {
                let internal_msg = from_wit_message(msg);
                
                // Dispatch to appropriate handler based on message type
                match internal_msg {
                    RaftMessage::PreVoteRequest { term, candidate_id, last_log_index, last_log_term } => {
                        let (response, _reset_timer) = node.handle_prevote_request(
                            term, candidate_id, last_log_index, last_log_term
                        );
                        // Response would be sent via host.send_message() import
                        let _ = (from_node, response); // Suppress unused for now
                    }
                    RaftMessage::PreVoteResponse { term, vote_granted } => {
                        let _should_start_election = node.handle_prevote_response(term, vote_granted, from_node);
                    }
                    RaftMessage::VoteRequest { term, candidate_id, last_log_index, last_log_term } => {
                        let (response, _reset_timer) = node.handle_vote_request(
                            term, candidate_id, last_log_index, last_log_term
                        );
                        let _ = (from_node, response);
                    }
                    RaftMessage::VoteResponse { term, vote_granted } => {
                        let _became_leader = node.handle_vote_response(term, vote_granted, from_node);
                    }
                    RaftMessage::AppendEntries { term, leader_id, prev_log_index, prev_log_term, entries, leader_commit } => {
                        let (response, _reset_timer) = node.handle_append_entries(
                            term, leader_id, prev_log_index, prev_log_term, entries, leader_commit
                        );
                        let _ = (from_node, response);
                    }
                    RaftMessage::AppendEntriesResponse { term, success } => {
                        // Note: match_index_hint would come from message if enhanced,
                        // for now we use 0 and let leader track via next_index
                        let _commit_advanced = node.handle_append_entries_response(term, success, from_node, 0);
                    }
                }
            }
        });
    }

    fn submit_command(command: Vec<u8>) -> bool {
        NODE.with(|n| {
            let node_ref = n.borrow();
            if let Some(ref node) = *node_ref {
                // Only leader can accept commands
                node.state == NodeState::Leader && !command.is_empty()
            } else {
                false
            }
        })
    }

    fn get_status() -> NodeStatus {
        NODE.with(|n| {
            let node_ref = n.borrow();
            if let Some(ref node) = *node_ref {
                get_node_status(node)
            } else {
                dead_status()
            }
        })
    }
}

fn dead_status() -> NodeStatus {
    NodeStatus {
        id: 0,
        state: WitNodeState::Dead,
        term: 0,
        log_length: 0,
        commit_index: 0,
    }
}

fn get_node_status(node: &RaftNode) -> NodeStatus {
    NodeStatus {
        id: node.id,
        state: to_wit_state(node.state),
        term: node.current_term,
        log_length: if node.log.is_empty() { 0 } else { node.log.len() as u64 },
        commit_index: node.commit_index,
    }
}

#[allow(dead_code)]
fn to_wit_message(msg: &RaftMessage) -> WitRaftMessage {
    match msg {
        RaftMessage::PreVoteRequest { term, candidate_id, last_log_index, last_log_term } => {
            WitRaftMessage::PreVoteReq(PreVoteRequest {
                term: *term,
                candidate_id: *candidate_id,
                last_log_index: *last_log_index,
                last_log_term: *last_log_term,
            })
        }
        RaftMessage::PreVoteResponse { term, vote_granted } => {
            WitRaftMessage::PreVoteRes(PreVoteResponse {
                term: *term,
                vote_granted: *vote_granted,
            })
        }
        RaftMessage::VoteRequest { term, candidate_id, last_log_index, last_log_term } => {
            WitRaftMessage::VoteReq(VoteRequest {
                term: *term,
                candidate_id: *candidate_id,
                last_log_index: *last_log_index,
                last_log_term: *last_log_term,
            })
        }
        RaftMessage::VoteResponse { term, vote_granted } => {
            WitRaftMessage::VoteRes(VoteResponse {
                term: *term,
                vote_granted: *vote_granted,
            })
        }
        RaftMessage::AppendEntries { term, leader_id, prev_log_index, prev_log_term, entries, leader_commit } => {
            WitRaftMessage::AppendReq(AppendEntries {
                term: *term,
                leader_id: *leader_id,
                prev_log_index: *prev_log_index,
                prev_log_term: *prev_log_term,
                entries: entries.iter().map(to_wit_log_entry).collect(),
                leader_commit: *leader_commit,
            })
        }
        RaftMessage::AppendEntriesResponse { term, success } => {
            WitRaftMessage::AppendRes(AppendEntriesResponse {
                term: *term,
                success: *success,
            })
        }
    }
}

fn from_wit_message(msg: WitRaftMessage) -> RaftMessage {
    match msg {
        WitRaftMessage::PreVoteReq(req) => RaftMessage::PreVoteRequest {
            term: req.term,
            candidate_id: req.candidate_id,
            last_log_index: req.last_log_index,
            last_log_term: req.last_log_term,
        },
        WitRaftMessage::PreVoteRes(res) => RaftMessage::PreVoteResponse {
            term: res.term,
            vote_granted: res.vote_granted,
        },
        WitRaftMessage::VoteReq(req) => RaftMessage::VoteRequest {
            term: req.term,
            candidate_id: req.candidate_id,
            last_log_index: req.last_log_index,
            last_log_term: req.last_log_term,
        },
        WitRaftMessage::VoteRes(res) => RaftMessage::VoteResponse {
            term: res.term,
            vote_granted: res.vote_granted,
        },
        WitRaftMessage::AppendReq(req) => RaftMessage::AppendEntries {
            term: req.term,
            leader_id: req.leader_id,
            prev_log_index: req.prev_log_index,
            prev_log_term: req.prev_log_term,
            entries: req.entries.iter().map(from_wit_log_entry).collect(),
            leader_commit: req.leader_commit,
        },
        WitRaftMessage::AppendRes(res) => RaftMessage::AppendEntriesResponse {
            term: res.term,
            success: res.success,
        },
    }
}

// Export the component
bindings::export!(RaftNodeComponent with_types_in bindings);
