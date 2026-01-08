//! # node
//!
//! why: define the raft node state machine and state transitions
//! relations: uses message.rs for rpc types, log.rs for entry management
//! what: NodeState enum, RaftNode struct, election/heartbeat timers

use serde::{Deserialize, Serialize};
use crate::{LogEntry, RaftMessage};
use std::collections::HashMap;

/// the three possible states a raft node can be in
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeState {
    /// passive state - listens for heartbeats, votes when asked
    Follower,
    /// transitional state - requesting votes to become leader
    Candidate,
    /// active state - manages log replication, sends heartbeats
    Leader,
}

impl Default for NodeState {
    fn default() -> Self {
        Self::Follower
    }
}

/// configuration for raft timing (in milliseconds)
#[derive(Debug, Clone)]
pub struct RaftConfig {
    /// minimum election timeout in ms (default: 150)
    pub election_timeout_min: u64,
    /// maximum election timeout in ms (default: 300)
    pub election_timeout_max: u64,
    /// heartbeat interval in ms (default: 50)
    pub heartbeat_interval: u64,
}

impl Default for RaftConfig {
    fn default() -> Self {
        Self {
            election_timeout_min: 150,
            election_timeout_max: 300,
            heartbeat_interval: 50,
        }
    }
}

/// a single raft node in the cluster
/// 
/// implements the raft consensus algorithm including:
/// - leader election with randomized timeouts
/// - log replication with consistency checks
/// - commit index management
#[derive(Debug)]
pub struct RaftNode {
    // -- persistent state (must survive restarts) --
    
    /// unique identifier for this node
    pub id: u64,
    /// current term number (monotonically increasing)
    pub current_term: u64,
    /// node id that received our vote in current term (if any)
    pub voted_for: Option<u64>,
    /// the replicated log entries
    pub log: Vec<LogEntry>,
    
    // -- volatile state (all nodes) --
    
    /// current state (follower, candidate, or leader)
    pub state: NodeState,
    /// index of highest log entry known to be committed
    pub commit_index: u64,
    /// index of highest log entry applied to state machine
    pub last_applied: u64,
    
    // -- volatile state (leaders only, reinitialized after election) --
    
    /// for each server, index of next log entry to send (leader only)
    pub next_index: HashMap<u64, u64>,
    /// for each server, index of highest log entry known to be replicated (leader only)
    pub match_index: HashMap<u64, u64>,
    
    // -- cluster configuration --
    
    /// list of all node ids in the cluster (including self)
    pub cluster_nodes: Vec<u64>,
    /// timing configuration
    pub config: RaftConfig,
    
    // -- election state --
    
    /// votes received in current election (candidate only)
    pub votes_received: Vec<u64>,
}

impl RaftNode {
    /// create a new raft node in follower state
    pub fn new(id: u64, cluster_nodes: Vec<u64>) -> Self {
        Self {
            id,
            current_term: 0,
            voted_for: None,
            log: Vec::new(),
            state: NodeState::Follower,
            commit_index: 0,
            last_applied: 0,
            next_index: HashMap::new(),
            match_index: HashMap::new(),
            cluster_nodes,
            config: RaftConfig::default(),
            votes_received: Vec::new(),
        }
    }
    
    /// create a node with custom configuration
    pub fn with_config(id: u64, cluster_nodes: Vec<u64>, config: RaftConfig) -> Self {
        let mut node = Self::new(id, cluster_nodes);
        node.config = config;
        node
    }
    
    // -- state transitions --
    
    /// get the number of nodes needed for quorum (majority)
    pub fn quorum_size(&self) -> usize {
        (self.cluster_nodes.len() / 2) + 1
    }
    
    /// check if we have enough votes to become leader
    pub fn has_quorum(&self) -> bool {
        self.votes_received.len() >= self.quorum_size()
    }
    
    /// start an election: become candidate, increment term, vote for self
    pub fn start_election(&mut self) -> RaftMessage {
        self.state = NodeState::Candidate;
        self.current_term += 1;
        self.voted_for = Some(self.id);
        self.votes_received = vec![self.id]; // vote for ourselves
        
        // create vote request to send to all peers
        RaftMessage::VoteRequest {
            term: self.current_term,
            candidate_id: self.id,
            last_log_index: self.last_log_index(),
            last_log_term: self.last_log_term(),
        }
    }
    
    /// become leader: initialize leader state
    pub fn become_leader(&mut self) {
        self.state = NodeState::Leader;
        self.votes_received.clear();
        
        // initialize next_index and match_index for all peers
        let last_log_idx = self.last_log_index();
        for &node_id in &self.cluster_nodes {
            if node_id != self.id {
                self.next_index.insert(node_id, last_log_idx + 1);
                self.match_index.insert(node_id, 0);
            }
        }
    }
    
    /// step down to follower (e.g., when seeing higher term)
    pub fn become_follower(&mut self, term: u64) {
        self.state = NodeState::Follower;
        self.current_term = term;
        self.voted_for = None;
        self.votes_received.clear();
    }
    
    // -- log helpers --
    
    /// get the index of the last log entry (0 if log is empty)
    pub fn last_log_index(&self) -> u64 {
        self.log.last().map(|e| e.index).unwrap_or(0)
    }
    
    /// get the term of the last log entry (0 if log is empty)
    pub fn last_log_term(&self) -> u64 {
        self.log.last().map(|e| e.term).unwrap_or(0)
    }
    
    /// get log entry at a specific index (1-indexed)
    pub fn get_entry(&self, index: u64) -> Option<&LogEntry> {
        if index == 0 {
            return None;
        }
        self.log.iter().find(|e| e.index == index)
    }
    
    /// get the term of entry at a specific index (0 if not found)
    pub fn get_term_at(&self, index: u64) -> u64 {
        self.get_entry(index).map(|e| e.term).unwrap_or(0)
    }
    
    /// append a new entry to the log (leader only)
    pub fn append_entry(&mut self, command: Vec<u8>) -> &LogEntry {
        let entry = LogEntry::new(
            self.current_term,
            self.last_log_index() + 1,
            command,
        );
        self.log.push(entry);
        self.log.last().unwrap()
    }
    
    // -- message handling --
    
    /// handle a vote request from a candidate
    /// returns (response, should_reset_election_timer)
    pub fn handle_vote_request(
        &mut self,
        term: u64,
        candidate_id: u64,
        last_log_index: u64,
        last_log_term: u64,
    ) -> (RaftMessage, bool) {
        // if candidate's term is less than ours, reject
        if term < self.current_term {
            return (
                RaftMessage::VoteResponse {
                    term: self.current_term,
                    vote_granted: false,
                },
                false,
            );
        }
        
        // if we see a higher term, become follower
        if term > self.current_term {
            self.become_follower(term);
        }
        
        // check if we can grant the vote:
        // 1. we haven't voted for anyone else this term
        // 2. candidate's log is at least as up-to-date as ours
        let can_vote = self.voted_for.is_none() || self.voted_for == Some(candidate_id);
        let log_ok = self.is_log_up_to_date(last_log_index, last_log_term);
        
        let vote_granted = can_vote && log_ok;
        
        if vote_granted {
            self.voted_for = Some(candidate_id);
        }
        
        (
            RaftMessage::VoteResponse {
                term: self.current_term,
                vote_granted,
            },
            vote_granted, // reset election timer if we granted vote
        )
    }
    
    /// handle a vote response (candidate only)
    /// returns true if we just became leader
    pub fn handle_vote_response(&mut self, term: u64, vote_granted: bool, from: u64) -> bool {
        // if we see a higher term, step down
        if term > self.current_term {
            self.become_follower(term);
            return false;
        }
        
        // ignore if we're not a candidate anymore
        if self.state != NodeState::Candidate {
            return false;
        }
        
        // ignore if term doesn't match (stale response)
        if term != self.current_term {
            return false;
        }
        
        if vote_granted && !self.votes_received.contains(&from) {
            self.votes_received.push(from);
            
            // check if we have quorum
            if self.has_quorum() {
                self.become_leader();
                return true;
            }
        }
        
        false
    }
    
    /// check if a candidate's log is at least as up-to-date as ours
    /// (raft paper section 5.4.1)
    fn is_log_up_to_date(&self, last_log_index: u64, last_log_term: u64) -> bool {
        let our_last_term = self.last_log_term();
        let our_last_index = self.last_log_index();
        
        // compare by term first, then by index
        if last_log_term != our_last_term {
            last_log_term > our_last_term
        } else {
            last_log_index >= our_last_index
        }
    }
    
    /// create an append entries message for a follower (leader only)
    pub fn create_append_entries(&self, follower_id: u64) -> Option<RaftMessage> {
        if self.state != NodeState::Leader {
            return None;
        }
        
        let next_idx = *self.next_index.get(&follower_id)?;
        let prev_log_index = if next_idx > 1 { next_idx - 1 } else { 0 };
        let prev_log_term = self.get_term_at(prev_log_index);
        
        // get entries starting from next_index
        let entries: Vec<LogEntry> = self.log
            .iter()
            .filter(|e| e.index >= next_idx)
            .cloned()
            .collect();
        
        Some(RaftMessage::AppendEntries {
            term: self.current_term,
            leader_id: self.id,
            prev_log_index,
            prev_log_term,
            entries,
            leader_commit: self.commit_index,
        })
    }
    
    /// create a heartbeat (empty append entries) for all followers
    pub fn create_heartbeat(&self) -> Option<RaftMessage> {
        if self.state != NodeState::Leader {
            return None;
        }
        
        Some(RaftMessage::AppendEntries {
            term: self.current_term,
            leader_id: self.id,
            prev_log_index: self.last_log_index(),
            prev_log_term: self.last_log_term(),
            entries: Vec::new(),
            leader_commit: self.commit_index,
        })
    }
    
    /// handle an append entries request (follower/candidate)
    /// returns (response, should_reset_election_timer)
    pub fn handle_append_entries(
        &mut self,
        term: u64,
        _leader_id: u64,
        prev_log_index: u64,
        prev_log_term: u64,
        entries: Vec<LogEntry>,
        leader_commit: u64,
    ) -> (RaftMessage, bool) {
        // reject if term is less than ours
        if term < self.current_term {
            return (
                RaftMessage::AppendEntriesResponse {
                    term: self.current_term,
                    success: false,
                },
                false,
            );
        }
        
        // if we see higher or equal term from a leader, become follower
        if term >= self.current_term {
            self.become_follower(term);
        }
        
        // log consistency check: we must have an entry at prev_log_index
        // with term == prev_log_term (or prev_log_index == 0)
        let log_consistent = if prev_log_index == 0 {
            true
        } else {
            self.get_term_at(prev_log_index) == prev_log_term
        };
        
        if !log_consistent {
            return (
                RaftMessage::AppendEntriesResponse {
                    term: self.current_term,
                    success: false,
                },
                true, // still reset timer, we heard from a leader
            );
        }
        
        // append entries (if any)
        for entry in entries {
            // if we have a conflicting entry, delete it and all following
            if let Some(existing) = self.get_entry(entry.index) {
                if existing.term != entry.term {
                    // remove conflicting entry and all after it
                    self.log.retain(|e| e.index < entry.index);
                }
            }
            // append if we don't have this entry
            if self.get_entry(entry.index).is_none() {
                self.log.push(entry);
            }
        }
        
        // update commit index
        if leader_commit > self.commit_index {
            self.commit_index = std::cmp::min(leader_commit, self.last_log_index());
        }
        
        (
            RaftMessage::AppendEntriesResponse {
                term: self.current_term,
                success: true,
            },
            true, // reset election timer
        )
    }
    
    /// handle an append entries response (leader only)
    /// returns true if commit_index was updated
    pub fn handle_append_entries_response(
        &mut self,
        term: u64,
        success: bool,
        from: u64,
        match_index_hint: u64,
    ) -> bool {
        // if we see a higher term, step down
        if term > self.current_term {
            self.become_follower(term);
            return false;
        }
        
        // ignore if we're not the leader
        if self.state != NodeState::Leader {
            return false;
        }
        
        if success {
            // update next_index and match_index for follower
            if let Some(next) = self.next_index.get_mut(&from) {
                *next = match_index_hint + 1;
            }
            if let Some(match_idx) = self.match_index.get_mut(&from) {
                *match_idx = match_index_hint;
            }
            
            // try to advance commit_index
            return self.try_advance_commit_index();
        } else {
            // decrement next_index and retry
            if let Some(next) = self.next_index.get_mut(&from) {
                if *next > 1 {
                    *next -= 1;
                }
            }
        }
        
        false
    }
    
    /// try to advance commit_index based on match_index from followers
    /// returns true if commit_index was advanced
    fn try_advance_commit_index(&mut self) -> bool {
        // find the highest N such that:
        // 1. N > commit_index
        // 2. a majority of match_index[i] >= N
        // 3. log[N].term == current_term
        
        let old_commit = self.commit_index;
        
        for n in (self.commit_index + 1)..=self.last_log_index() {
            // check that entry at N has current term (leader can only commit own entries)
            if self.get_term_at(n) != self.current_term {
                continue;
            }
            
            // count how many servers have this entry
            let mut count = 1; // count ourselves
            for (&node_id, &match_idx) in &self.match_index {
                if node_id != self.id && match_idx >= n {
                    count += 1;
                }
            }
            
            if count >= self.quorum_size() {
                self.commit_index = n;
            }
        }
        
        self.commit_index > old_commit
    }
    
    /// apply committed entries to state machine
    /// returns the entries that should be applied
    pub fn get_entries_to_apply(&mut self) -> Vec<LogEntry> {
        let mut entries = Vec::new();
        
        while self.last_applied < self.commit_index {
            self.last_applied += 1;
            if let Some(entry) = self.get_entry(self.last_applied) {
                entries.push(entry.clone());
            }
        }
        
        entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_node_starts_as_follower() {
        let node = RaftNode::new(1, vec![1, 2, 3]);
        assert_eq!(node.state, NodeState::Follower);
        assert_eq!(node.current_term, 0);
        assert_eq!(node.voted_for, None);
    }
    
    #[test]
    fn quorum_calculation() {
        // 3 nodes: quorum = 2
        let node3 = RaftNode::new(1, vec![1, 2, 3]);
        assert_eq!(node3.quorum_size(), 2);
        
        // 5 nodes: quorum = 3
        let node5 = RaftNode::new(1, vec![1, 2, 3, 4, 5]);
        assert_eq!(node5.quorum_size(), 3);
    }
    
    #[test]
    fn election_timeout_triggers_candidacy() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        assert_eq!(node.state, NodeState::Follower);
        
        // simulate election timeout
        let vote_request = node.start_election();
        
        assert_eq!(node.state, NodeState::Candidate);
        assert_eq!(node.current_term, 1);
        assert_eq!(node.voted_for, Some(1)); // voted for self
        assert_eq!(node.votes_received, vec![1]); // has own vote
        
        // verify vote request format
        match vote_request {
            RaftMessage::VoteRequest { term, candidate_id, .. } => {
                assert_eq!(term, 1);
                assert_eq!(candidate_id, 1);
            }
            _ => panic!("expected VoteRequest"),
        }
    }
    
    #[test]
    fn majority_vote_wins_election() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        node.start_election();
        
        // receive vote from node 2
        let became_leader = node.handle_vote_response(1, true, 2);
        
        // with 2/3 votes (self + node 2), we have quorum
        assert!(became_leader);
        assert_eq!(node.state, NodeState::Leader);
    }
    
    #[test]
    fn higher_term_forces_step_down() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        node.start_election();
        node.handle_vote_response(1, true, 2); // become leader
        assert_eq!(node.state, NodeState::Leader);
        assert_eq!(node.current_term, 1);
        
        // receive message with higher term
        node.handle_vote_response(5, false, 2);
        
        assert_eq!(node.state, NodeState::Follower);
        assert_eq!(node.current_term, 5);
    }
    
    #[test]
    fn log_consistency_check_rejects_stale_entries() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        
        // manually add some log entries
        node.log.push(LogEntry::new(1, 1, vec![1]));
        node.log.push(LogEntry::new(1, 2, vec![2]));
        node.current_term = 1;
        
        // append entries with wrong prev_log_term should fail
        let (response, _) = node.handle_append_entries(
            2,    // term
            2,    // leader_id
            2,    // prev_log_index
            99,   // prev_log_term (wrong!)
            vec![LogEntry::new(2, 3, vec![3])],
            0,    // leader_commit
        );
        
        match response {
            RaftMessage::AppendEntriesResponse { success, .. } => {
                assert!(!success, "should reject due to log inconsistency");
            }
            _ => panic!("expected AppendEntriesResponse"),
        }
    }
    
    #[test]
    fn follower_grants_vote_to_valid_candidate() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        
        let (response, should_reset_timer) = node.handle_vote_request(
            1,  // term
            2,  // candidate_id
            0,  // last_log_index
            0,  // last_log_term
        );
        
        match response {
            RaftMessage::VoteResponse { vote_granted, .. } => {
                assert!(vote_granted);
                assert!(should_reset_timer);
                assert_eq!(node.voted_for, Some(2));
            }
            _ => panic!("expected VoteResponse"),
        }
    }
    
    #[test]
    fn follower_rejects_vote_for_lower_term() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        node.current_term = 5;
        
        let (response, _) = node.handle_vote_request(
            3,  // term (lower than current)
            2,  // candidate_id
            0,  // last_log_index
            0,  // last_log_term
        );
        
        match response {
            RaftMessage::VoteResponse { vote_granted, term } => {
                assert!(!vote_granted);
                assert_eq!(term, 5); // should return our higher term
            }
            _ => panic!("expected VoteResponse"),
        }
    }
}
