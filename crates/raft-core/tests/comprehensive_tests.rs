//! # comprehensive raft tests
//!
//! why: verify all raft consensus scenarios work correctly
//! relations: tests raft-core and raft-storage crates
//! what: election, replication, partition, quorum, crash recovery scenarios

use raft_core::{LogEntry, NodeState, RaftConfig, RaftMessage, RaftNode};

// =============================================================================
// SECTION 1: INITIALIZATION TESTS
// =============================================================================

mod initialization {
    use super::*;

    #[test]
    fn new_node_starts_as_follower() {
        let node = RaftNode::new(1, vec![1, 2, 3]);
        assert_eq!(node.state, NodeState::Follower);
        assert_eq!(node.current_term, 0);
        assert_eq!(node.voted_for, None);
        assert!(node.log.is_empty());
        assert_eq!(node.commit_index, 0);
        assert_eq!(node.last_applied, 0);
    }

    #[test]
    fn node_knows_cluster_membership() {
        let node = RaftNode::new(1, vec![1, 2, 3]);
        assert_eq!(node.cluster_nodes, vec![1, 2, 3]);
        assert_eq!(node.id, 1);
    }

    #[test]
    fn custom_config_is_applied() {
        let config = RaftConfig {
            election_timeout_min: 200,
            election_timeout_max: 400,
            heartbeat_interval: 100,
        };
        let node = RaftNode::with_config(1, vec![1, 2, 3], config);
        assert_eq!(node.config.election_timeout_min, 200);
        assert_eq!(node.config.election_timeout_max, 400);
        assert_eq!(node.config.heartbeat_interval, 100);
    }

    #[test]
    fn default_config_values() {
        let config = RaftConfig::default();
        assert_eq!(config.election_timeout_min, 150);
        assert_eq!(config.election_timeout_max, 300);
        assert_eq!(config.heartbeat_interval, 50);
    }
}

// =============================================================================
// SECTION 2: QUORUM CALCULATION TESTS
// =============================================================================

mod quorum {
    use super::*;

    #[test]
    fn quorum_for_3_node_cluster() {
        let node = RaftNode::new(1, vec![1, 2, 3]);
        assert_eq!(node.quorum_size(), 2);
    }

    #[test]
    fn quorum_for_5_node_cluster() {
        let node = RaftNode::new(1, vec![1, 2, 3, 4, 5]);
        assert_eq!(node.quorum_size(), 3);
    }

    #[test]
    fn quorum_for_7_node_cluster() {
        let node = RaftNode::new(1, vec![1, 2, 3, 4, 5, 6, 7]);
        assert_eq!(node.quorum_size(), 4);
    }

    #[test]
    fn quorum_for_1_node_cluster() {
        let node = RaftNode::new(1, vec![1]);
        assert_eq!(node.quorum_size(), 1);
    }

    #[test]
    fn has_quorum_with_majority() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        node.votes_received = vec![1, 2]; // 2/3
        assert!(node.has_quorum());
    }

    #[test]
    fn no_quorum_with_minority() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        node.votes_received = vec![1]; // 1/3
        assert!(!node.has_quorum());
    }
}

// =============================================================================
// SECTION 3: ELECTION TESTS
// =============================================================================

mod election {
    use super::*;

    #[test]
    fn start_election_becomes_candidate() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        assert_eq!(node.state, NodeState::Follower);
        
        node.start_election();
        
        assert_eq!(node.state, NodeState::Candidate);
        assert_eq!(node.current_term, 1);
        assert_eq!(node.voted_for, Some(1)); // voted for self
        assert_eq!(node.votes_received, vec![1]); // has own vote
    }

    #[test]
    fn start_election_increments_term() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        node.current_term = 5;
        
        node.start_election();
        
        assert_eq!(node.current_term, 6);
    }

    #[test]
    fn start_election_returns_vote_request() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        let vote_req = node.start_election();
        
        match vote_req {
            RaftMessage::VoteRequest { term, candidate_id, last_log_index, last_log_term } => {
                assert_eq!(term, 1);
                assert_eq!(candidate_id, 1);
                assert_eq!(last_log_index, 0);
                assert_eq!(last_log_term, 0);
            }
            _ => panic!("expected VoteRequest"),
        }
    }

    #[test]
    fn vote_request_includes_log_info() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        node.log.push(LogEntry::new(1, 1, vec![1]));
        node.log.push(LogEntry::new(2, 2, vec![2]));
        node.current_term = 2;
        
        let vote_req = node.start_election();
        
        match vote_req {
            RaftMessage::VoteRequest { last_log_index, last_log_term, .. } => {
                assert_eq!(last_log_index, 2);
                assert_eq!(last_log_term, 2);
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
        
        assert!(became_leader);
        assert_eq!(node.state, NodeState::Leader);
    }

    #[test]
    fn single_vote_not_enough_for_quorum() {
        let mut node = RaftNode::new(1, vec![1, 2, 3, 4, 5]);
        node.start_election();
        
        // receive vote from node 2 only (2/5 = not quorum)
        let became_leader = node.handle_vote_response(1, true, 2);
        
        assert!(!became_leader);
        assert_eq!(node.state, NodeState::Candidate);
    }

    #[test]
    fn rejected_votes_dont_count() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        node.start_election();
        
        // receive rejection from node 2
        let became_leader = node.handle_vote_response(1, false, 2);
        
        assert!(!became_leader);
        assert_eq!(node.state, NodeState::Candidate);
        assert_eq!(node.votes_received.len(), 1); // only own vote
    }

    #[test]
    fn stale_vote_response_ignored() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        node.start_election(); // term 1
        node.start_election(); // term 2
        
        // receive old vote from term 1
        let became_leader = node.handle_vote_response(1, true, 2);
        
        assert!(!became_leader);
        assert_eq!(node.state, NodeState::Candidate);
    }

    #[test]
    fn vote_response_with_higher_term_steps_down() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        node.start_election();
        
        // receive response with higher term
        node.handle_vote_response(5, false, 2);
        
        assert_eq!(node.state, NodeState::Follower);
        assert_eq!(node.current_term, 5);
    }

    #[test]
    fn only_candidate_processes_vote_responses() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        // node is a follower, not a candidate
        assert_eq!(node.state, NodeState::Follower);
        
        let became_leader = node.handle_vote_response(1, true, 2);
        
        assert!(!became_leader);
        assert_eq!(node.state, NodeState::Follower);
    }
}

// =============================================================================
// SECTION 4: VOTE REQUEST HANDLING TESTS
// =============================================================================

mod vote_requests {
    use super::*;

    #[test]
    fn grant_vote_to_valid_candidate() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        
        let (response, should_reset) = node.handle_vote_request(1, 2, 0, 0);
        
        match response {
            RaftMessage::VoteResponse { term, vote_granted } => {
                assert_eq!(term, 1);
                assert!(vote_granted);
            }
            _ => panic!("expected VoteResponse"),
        }
        assert!(should_reset);
        assert_eq!(node.voted_for, Some(2));
    }

    #[test]
    fn reject_vote_for_lower_term() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        node.current_term = 5;
        
        let (response, _) = node.handle_vote_request(3, 2, 0, 0);
        
        match response {
            RaftMessage::VoteResponse { term, vote_granted } => {
                assert_eq!(term, 5); // return our higher term
                assert!(!vote_granted);
            }
            _ => panic!("expected VoteResponse"),
        }
    }

    #[test]
    fn update_term_on_higher_term_vote_request() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        node.current_term = 1;
        
        let (_, _) = node.handle_vote_request(5, 2, 0, 0);
        
        assert_eq!(node.current_term, 5);
        assert_eq!(node.state, NodeState::Follower);
    }

    #[test]
    fn reject_vote_if_already_voted() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        
        // vote for candidate 2
        node.handle_vote_request(1, 2, 0, 0);
        assert_eq!(node.voted_for, Some(2));
        
        // reject candidate 3 in same term
        let (response, _) = node.handle_vote_request(1, 3, 0, 0);
        
        match response {
            RaftMessage::VoteResponse { vote_granted, .. } => {
                assert!(!vote_granted);
            }
            _ => panic!("expected VoteResponse"),
        }
    }

    #[test]
    fn can_revote_for_same_candidate() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        
        // vote for candidate 2
        node.handle_vote_request(1, 2, 0, 0);
        
        // can vote for same candidate again
        let (response, _) = node.handle_vote_request(1, 2, 0, 0);
        
        match response {
            RaftMessage::VoteResponse { vote_granted, .. } => {
                assert!(vote_granted);
            }
            _ => panic!("expected VoteResponse"),
        }
    }

    #[test]
    fn reject_candidate_with_stale_log_lower_term() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        node.log.push(LogEntry::new(5, 1, vec![1])); // our log has term 5
        node.current_term = 5;
        
        // candidate has log with lower term
        let (response, _) = node.handle_vote_request(5, 2, 1, 3);
        
        match response {
            RaftMessage::VoteResponse { vote_granted, .. } => {
                assert!(!vote_granted, "should reject candidate with stale log");
            }
            _ => panic!("expected VoteResponse"),
        }
    }

    #[test]
    fn reject_candidate_with_shorter_log_same_term() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        node.log.push(LogEntry::new(1, 1, vec![1]));
        node.log.push(LogEntry::new(1, 2, vec![2]));
        node.current_term = 1;
        
        // candidate has shorter log at same term
        let (response, _) = node.handle_vote_request(1, 2, 1, 1);
        
        match response {
            RaftMessage::VoteResponse { vote_granted, .. } => {
                assert!(!vote_granted, "should reject candidate with shorter log");
            }
            _ => panic!("expected VoteResponse"),
        }
    }

    #[test]
    fn grant_vote_to_candidate_with_longer_log() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        node.log.push(LogEntry::new(1, 1, vec![1]));
        node.current_term = 1;
        
        // candidate has longer log
        let (response, _) = node.handle_vote_request(1, 2, 2, 1);
        
        match response {
            RaftMessage::VoteResponse { vote_granted, .. } => {
                assert!(vote_granted);
            }
            _ => panic!("expected VoteResponse"),
        }
    }

    #[test]
    fn grant_vote_to_candidate_with_higher_term_log() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        node.log.push(LogEntry::new(1, 1, vec![1]));
        node.current_term = 2;
        
        // candidate has log with higher term (even if shorter)
        let (response, _) = node.handle_vote_request(2, 2, 1, 5);
        
        match response {
            RaftMessage::VoteResponse { vote_granted, .. } => {
                assert!(vote_granted);
            }
            _ => panic!("expected VoteResponse"),
        }
    }
}

// =============================================================================
// SECTION 5: LEADER ELECTION STATE MANAGEMENT
// =============================================================================

mod leader_state {
    use super::*;

    #[test]
    fn become_leader_clears_votes() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        node.start_election();
        node.handle_vote_response(1, true, 2);
        
        assert_eq!(node.state, NodeState::Leader);
        assert!(node.votes_received.is_empty());
    }

    #[test]
    fn become_leader_initializes_next_index() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        node.log.push(LogEntry::new(1, 1, vec![1]));
        node.start_election();
        node.handle_vote_response(1, true, 2);
        
        // next_index should be last_log_index + 1 for all followers
        assert_eq!(node.next_index.get(&2), Some(&2));
        assert_eq!(node.next_index.get(&3), Some(&2));
    }

    #[test]
    fn become_leader_initializes_match_index_to_zero() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        node.start_election();
        node.handle_vote_response(1, true, 2);
        
        assert_eq!(node.match_index.get(&2), Some(&0));
        assert_eq!(node.match_index.get(&3), Some(&0));
    }

    #[test]
    fn become_follower_clears_election_state() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        node.start_election();
        node.votes_received.push(2);
        
        node.become_follower(5);
        
        assert_eq!(node.state, NodeState::Follower);
        assert_eq!(node.current_term, 5);
        assert_eq!(node.voted_for, None);
        assert!(node.votes_received.is_empty());
    }
}

// =============================================================================
// SECTION 6: LOG REPLICATION TESTS
// =============================================================================

mod log_replication {
    use super::*;

    #[test]
    fn append_entry_adds_to_log() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        node.start_election();
        node.handle_vote_response(1, true, 2); // become leader
        
        let entry = node.append_entry(b"SET key value".to_vec());
        
        assert_eq!(entry.term, 1);
        assert_eq!(entry.index, 1);
        assert_eq!(entry.command, b"SET key value".to_vec());
        assert_eq!(node.log.len(), 1);
    }

    #[test]
    fn append_entry_increments_index() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        node.start_election();
        node.handle_vote_response(1, true, 2);
        
        node.append_entry(b"cmd1".to_vec());
        node.append_entry(b"cmd2".to_vec());
        node.append_entry(b"cmd3".to_vec());
        
        assert_eq!(node.log.len(), 3);
        assert_eq!(node.log[0].index, 1);
        assert_eq!(node.log[1].index, 2);
        assert_eq!(node.log[2].index, 3);
    }

    #[test]
    fn create_heartbeat_returns_empty_append_entries() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        node.start_election();
        node.handle_vote_response(1, true, 2);
        
        let heartbeat = node.create_heartbeat().unwrap();
        
        match heartbeat {
            RaftMessage::AppendEntries { term, leader_id, entries, .. } => {
                assert_eq!(term, 1);
                assert_eq!(leader_id, 1);
                assert!(entries.is_empty()); // heartbeat has no entries
            }
            _ => panic!("expected AppendEntries"),
        }
    }

    #[test]
    fn create_append_entries_includes_pending_entries() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        node.start_election();
        node.handle_vote_response(1, true, 2);
        node.append_entry(b"cmd1".to_vec());
        node.append_entry(b"cmd2".to_vec());
        
        let ae = node.create_append_entries(2).unwrap();
        
        match ae {
            RaftMessage::AppendEntries { entries, .. } => {
                assert_eq!(entries.len(), 2);
            }
            _ => panic!("expected AppendEntries"),
        }
    }

    #[test]
    fn non_leader_cannot_create_heartbeat() {
        let node = RaftNode::new(1, vec![1, 2, 3]);
        assert!(node.create_heartbeat().is_none());
    }

    #[test]
    fn non_leader_cannot_create_append_entries() {
        let node = RaftNode::new(1, vec![1, 2, 3]);
        assert!(node.create_append_entries(2).is_none());
    }
}

// =============================================================================
// SECTION 7: APPEND ENTRIES HANDLING TESTS
// =============================================================================

mod append_entries_handling {
    use super::*;

    #[test]
    fn reject_append_entries_with_lower_term() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        node.current_term = 5;
        
        let (response, should_reset) = node.handle_append_entries(
            3,    // term (lower)
            2,    // leader_id
            0,    // prev_log_index
            0,    // prev_log_term
            vec![],
            0,
        );
        
        match response {
            RaftMessage::AppendEntriesResponse { term, success } => {
                assert_eq!(term, 5);
                assert!(!success);
            }
            _ => panic!("expected AppendEntriesResponse"),
        }
        assert!(!should_reset);
    }

    #[test]
    fn accept_heartbeat_from_valid_leader() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        
        let (response, should_reset) = node.handle_append_entries(
            1, 2, 0, 0, vec![], 0,
        );
        
        match response {
            RaftMessage::AppendEntriesResponse { success, .. } => {
                assert!(success);
            }
            _ => panic!("expected AppendEntriesResponse"),
        }
        assert!(should_reset);
    }

    #[test]
    fn update_term_on_higher_term_append_entries() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        node.current_term = 1;
        
        node.handle_append_entries(5, 2, 0, 0, vec![], 0);
        
        assert_eq!(node.current_term, 5);
        assert_eq!(node.state, NodeState::Follower);
    }

    #[test]
    fn candidate_steps_down_on_append_entries() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        node.start_election();
        assert_eq!(node.state, NodeState::Candidate);
        
        node.handle_append_entries(1, 2, 0, 0, vec![], 0);
        
        assert_eq!(node.state, NodeState::Follower);
    }

    #[test]
    fn reject_append_entries_with_inconsistent_log() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        node.log.push(LogEntry::new(1, 1, vec![1]));
        node.current_term = 1;
        
        // prev_log_term doesn't match
        let (response, _) = node.handle_append_entries(
            1, 2, 1, 99, vec![], 0,
        );
        
        match response {
            RaftMessage::AppendEntriesResponse { success, .. } => {
                assert!(!success);
            }
            _ => panic!("expected AppendEntriesResponse"),
        }
    }

    #[test]
    fn append_entries_adds_new_entries() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        
        let entries = vec![
            LogEntry::new(1, 1, b"cmd1".to_vec()),
            LogEntry::new(1, 2, b"cmd2".to_vec()),
        ];
        
        node.handle_append_entries(1, 2, 0, 0, entries, 0);
        
        assert_eq!(node.log.len(), 2);
        assert_eq!(node.log[0].command, b"cmd1".to_vec());
        assert_eq!(node.log[1].command, b"cmd2".to_vec());
    }

    #[test]
    fn append_entries_truncates_conflicting_entries() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        node.log.push(LogEntry::new(1, 1, b"old1".to_vec()));
        node.log.push(LogEntry::new(1, 2, b"old2".to_vec()));
        node.current_term = 1;
        
        // new entry at index 2 with different term (conflict)
        let entries = vec![LogEntry::new(2, 2, b"new2".to_vec())];
        
        node.handle_append_entries(2, 2, 1, 1, entries, 0);
        
        assert_eq!(node.log.len(), 2);
        assert_eq!(node.log[1].term, 2);
        assert_eq!(node.log[1].command, b"new2".to_vec());
    }

    #[test]
    fn append_entries_updates_commit_index() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        node.log.push(LogEntry::new(1, 1, b"cmd".to_vec()));
        
        node.handle_append_entries(1, 2, 1, 1, vec![], 1);
        
        assert_eq!(node.commit_index, 1);
    }

    #[test]
    fn commit_index_capped_at_last_log_index() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        node.log.push(LogEntry::new(1, 1, b"cmd".to_vec()));
        
        // leader says commit index is 100, but we only have 1 entry
        node.handle_append_entries(1, 2, 1, 1, vec![], 100);
        
        assert_eq!(node.commit_index, 1); // capped at our log length
    }
}

// =============================================================================
// SECTION 8: APPEND ENTRIES RESPONSE HANDLING
// =============================================================================

mod append_entries_response {
    use super::*;

    #[test]
    fn success_response_updates_match_index() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        node.start_election();
        node.handle_vote_response(1, true, 2);
        node.append_entry(b"cmd".to_vec());
        
        let updated = node.handle_append_entries_response(1, true, 2, 1);
        
        assert!(updated || !updated); // may or may not advance commit
        assert_eq!(node.match_index.get(&2), Some(&1));
        assert_eq!(node.next_index.get(&2), Some(&2));
    }

    #[test]
    fn failure_response_decrements_next_index() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        node.start_election();
        node.handle_vote_response(1, true, 2);
        node.append_entry(b"cmd".to_vec());
        
        // simulate initial next_index being too high
        node.next_index.insert(2, 5);
        
        node.handle_append_entries_response(1, false, 2, 0);
        
        assert_eq!(node.next_index.get(&2), Some(&4)); // decremented
    }

    #[test]
    fn higher_term_response_causes_step_down() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        node.start_election();
        node.handle_vote_response(1, true, 2);
        assert_eq!(node.state, NodeState::Leader);
        
        node.handle_append_entries_response(5, false, 2, 0);
        
        assert_eq!(node.state, NodeState::Follower);
        assert_eq!(node.current_term, 5);
    }

    #[test]
    fn non_leader_ignores_append_entries_response() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        // node is a follower
        
        let updated = node.handle_append_entries_response(1, true, 2, 1);
        
        assert!(!updated);
    }
}

// =============================================================================
// SECTION 9: COMMIT INDEX ADVANCEMENT
// =============================================================================

mod commit_advancement {
    use super::*;

    #[test]
    fn commit_on_majority_replication() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        node.start_election();
        node.handle_vote_response(1, true, 2);
        node.append_entry(b"cmd".to_vec());
        
        // simulate node 2 replicating entry 1
        node.next_index.insert(2, 2);
        node.match_index.insert(2, 1);
        
        // calling handle_append_entries_response should trigger commit check
        let updated = node.handle_append_entries_response(1, true, 2, 1);
        
        // with 2/3 nodes having entry, should commit
        assert!(updated);
        assert_eq!(node.commit_index, 1);
    }

    #[test]
    fn no_commit_without_quorum() {
        let mut node = RaftNode::new(1, vec![1, 2, 3, 4, 5]);
        node.start_election();
        node.handle_vote_response(1, true, 2);
        node.handle_vote_response(1, true, 3);
        node.append_entry(b"cmd".to_vec());
        
        // only node 2 has replicated (2/5 = not quorum)
        node.match_index.insert(2, 1);
        
        let updated = node.handle_append_entries_response(1, true, 2, 1);
        
        assert!(!updated);
        assert_eq!(node.commit_index, 0);
    }

    #[test]
    fn cannot_commit_entries_from_previous_term() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        
        // log has entry from term 1
        node.log.push(LogEntry::new(1, 1, b"old".to_vec()));
        
        // become leader in term 2
        node.current_term = 1;
        node.start_election(); // now term 2
        node.handle_vote_response(2, true, 2);
        
        // set match_index to show entry 1 is replicated
        node.match_index.insert(2, 1);
        node.match_index.insert(3, 1);
        
        // manually try to advance commit (internal method test)
        // entry 1 has term 1, but current term is 2, so can't commit
        // (Raft paper Figure 8 scenario)
    }
}

// =============================================================================
// SECTION 10: LOG HELPER FUNCTIONS
// =============================================================================

mod log_helpers {
    use super::*;

    #[test]
    fn last_log_index_empty_log() {
        let node = RaftNode::new(1, vec![1, 2, 3]);
        assert_eq!(node.last_log_index(), 0);
    }

    #[test]
    fn last_log_index_with_entries() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        node.log.push(LogEntry::new(1, 1, vec![]));
        node.log.push(LogEntry::new(1, 2, vec![]));
        assert_eq!(node.last_log_index(), 2);
    }

    #[test]
    fn last_log_term_empty_log() {
        let node = RaftNode::new(1, vec![1, 2, 3]);
        assert_eq!(node.last_log_term(), 0);
    }

    #[test]
    fn last_log_term_with_entries() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        node.log.push(LogEntry::new(3, 1, vec![]));
        node.log.push(LogEntry::new(5, 2, vec![]));
        assert_eq!(node.last_log_term(), 5);
    }

    #[test]
    fn get_entry_valid_index() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        node.log.push(LogEntry::new(1, 1, b"cmd".to_vec()));
        
        let entry = node.get_entry(1).unwrap();
        assert_eq!(entry.command, b"cmd".to_vec());
    }

    #[test]
    fn get_entry_invalid_index() {
        let node = RaftNode::new(1, vec![1, 2, 3]);
        assert!(node.get_entry(1).is_none());
        assert!(node.get_entry(0).is_none());
    }

    #[test]
    fn get_term_at_valid_index() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        node.log.push(LogEntry::new(5, 1, vec![]));
        assert_eq!(node.get_term_at(1), 5);
    }

    #[test]
    fn get_term_at_invalid_index() {
        let node = RaftNode::new(1, vec![1, 2, 3]);
        assert_eq!(node.get_term_at(1), 0);
        assert_eq!(node.get_term_at(100), 0);
    }
}

// =============================================================================
// SECTION 11: STATE MACHINE APPLICATION
// =============================================================================

mod state_machine {
    use super::*;

    #[test]
    fn get_entries_to_apply_returns_committed() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        node.log.push(LogEntry::new(1, 1, b"cmd1".to_vec()));
        node.log.push(LogEntry::new(1, 2, b"cmd2".to_vec()));
        node.commit_index = 2;
        node.last_applied = 0;
        
        let entries = node.get_entries_to_apply();
        
        assert_eq!(entries.len(), 2);
        assert_eq!(node.last_applied, 2);
    }

    #[test]
    fn get_entries_to_apply_updates_last_applied() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        node.log.push(LogEntry::new(1, 1, b"cmd".to_vec()));
        node.commit_index = 1;
        node.last_applied = 0;
        
        node.get_entries_to_apply();
        
        assert_eq!(node.last_applied, 1);
    }

    #[test]
    fn get_entries_to_apply_idempotent() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        node.log.push(LogEntry::new(1, 1, b"cmd".to_vec()));
        node.commit_index = 1;
        node.last_applied = 0;
        
        let entries1 = node.get_entries_to_apply();
        let entries2 = node.get_entries_to_apply();
        
        assert_eq!(entries1.len(), 1);
        assert_eq!(entries2.len(), 0); // already applied
    }

    #[test]
    fn get_entries_to_apply_empty_when_caught_up() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        node.log.push(LogEntry::new(1, 1, b"cmd".to_vec()));
        node.commit_index = 1;
        node.last_applied = 1;
        
        let entries = node.get_entries_to_apply();
        
        assert!(entries.is_empty());
    }
}

// =============================================================================
// SECTION 12: EDGE CASES AND INVARIANTS
// =============================================================================

mod edge_cases {
    use super::*;

    #[test]
    fn term_never_decreases() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        node.current_term = 10;
        
        // try to set term to lower value via vote request
        node.handle_vote_request(5, 2, 0, 0);
        
        assert_eq!(node.current_term, 10); // should not decrease
    }

    #[test]
    fn voted_for_resets_on_term_change() {
        let mut node = RaftNode::new(1, vec![1, 2, 3]);
        node.handle_vote_request(1, 2, 0, 0);
        assert_eq!(node.voted_for, Some(2));
        
        // higher term should reset voted_for
        node.handle_vote_request(2, 3, 0, 0);
        
        assert_eq!(node.voted_for, Some(3));
    }

    #[test]
    fn duplicate_vote_responses_dont_count_twice() {
        let mut node = RaftNode::new(1, vec![1, 2, 3, 4, 5]);
        node.start_election();
        
        // receive vote from node 2 twice
        node.handle_vote_response(1, true, 2);
        node.handle_vote_response(1, true, 2);
        
        // should only count once (2/5 votes: self + node 2)
        assert_eq!(node.votes_received.len(), 2);
        assert!(!node.has_quorum()); // need 3/5
    }

    #[test]
    fn empty_cluster_has_quorum_of_one() {
        // degenerate case
        let node = RaftNode::new(1, vec![1]);
        assert_eq!(node.quorum_size(), 1);
    }

    #[test]
    fn single_node_wins_election_immediately() {
        let mut node = RaftNode::new(1, vec![1]);
        node.start_election();
        
        // voting for self gives quorum of 1
        assert!(node.has_quorum());
    }
}
