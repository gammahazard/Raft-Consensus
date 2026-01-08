//! # node
//!
//! why: define the raft node state machine and state transitions
//! relations: uses message.rs for rpc types, log.rs for entry management
//! what: NodeState enum, RaftNode struct, election/heartbeat timers

use serde::{Deserialize, Serialize};

/// The three possible states a Raft node can be in
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeState {
    /// Passive state - listens for heartbeats, votes when asked
    Follower,
    /// Transitional state - requesting votes to become leader
    Candidate,
    /// Active state - manages log replication, sends heartbeats
    Leader,
}

impl Default for NodeState {
    fn default() -> Self {
        Self::Follower
    }
}

/// A single Raft node in the cluster
/// 
/// TODO: Implement in Phase 2 (feature/raft-core)
#[derive(Debug)]
pub struct RaftNode {
    /// Unique identifier for this node
    pub id: u64,
    /// Current state (Follower, Candidate, or Leader)
    pub state: NodeState,
    /// Current term number
    pub current_term: u64,
    /// Node ID that received our vote in current term (if any)
    pub voted_for: Option<u64>,
}

impl RaftNode {
    /// Create a new Raft node in Follower state
    pub fn new(id: u64) -> Self {
        Self {
            id,
            state: NodeState::Follower,
            current_term: 0,
            voted_for: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_node_starts_as_follower() {
        let node = RaftNode::new(1);
        assert_eq!(node.state, NodeState::Follower);
        assert_eq!(node.current_term, 0);
        assert_eq!(node.voted_for, None);
    }
}
