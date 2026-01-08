# Raft Algorithm Specification

<!-- 
why: document the raft algorithm as implemented in this project
relations: implemented by raft-core crate, visualized by dashboard
what: simplified raft explanation, state transitions, message types, PreVote protocol
-->

## Overview

This document describes our implementation of the Raft consensus algorithm, focusing on three core phases: **PreVote** (disruptive server prevention), **Leader Election**, and **Log Replication**.

## Node States

```
              timeout (no heartbeat)
                     │
                     ▼
             ┌───────────────┐
             │ PRE-CANDIDATE │◄────────────────────┐
             │               │                     │
             │ - Gathers     │   majority rejects  │
             │   pre-votes   │   pre-vote          │
             │ - Term stays  │                     │
             │   unchanged   │                     │
             └───────┬───────┘                     │
                     │ majority grants pre-vote    │
                     ▼                             │
┌───────────┐  ┌───────────────┐             ┌────┴────────────────┐
│  FOLLOWER │  │   CANDIDATE   │  majority   │       LEADER        │
│           │  │               │   votes     │                     │
│ - Passive │  │ - Requests    │────────────►│ - Sends heartbeats  │
│ - Votes   │  │   votes       │             │ - Manages log       │
│ - Listens │  │ - Term++      │             │ - Handles commits   │
└───────────┘  └───────────────┘             └─────────────────────┘
     ▲                │                                 │
     │    discovers   │          discovers higher       │
     │    higher term │          term                   │
     └────────────────┴─────────────────────────────────┘
```

## Phase 0: PreVote Protocol (Raft Thesis Section 9.6)

### The Problem: Disruptive Servers

Without PreVote, a disconnected node can wreak havoc:

```
Scenario: Node C gets network-partitioned

Time 0:  [A=leader term=5] ←──heartbeat──► [B=follower term=5] ✓ [C=follower term=5]
                                                                        │
Time 1:  Cable unplugged ──────────────────────────────────────────────┘
                                                                         
Time 2:  [A=leader term=5] ←──heartbeat──► [B=follower term=5]   [C timeout → term=6]
Time 3:  [A=leader term=5] ←──heartbeat──► [B=follower term=5]   [C timeout → term=7]
  ...     (cluster healthy, unaware)                              (C keeps incrementing)
Time N:  [A=leader term=5] ←──heartbeat──► [B=follower term=5]   [C timeout → term=50]

Time N+1: Cable plugged back in
         
         [C sends VoteRequest with term=50]
         [A sees term 50 > 5, STEPS DOWN immediately!]
         [B sees term 50 > 5, STEPS DOWN immediately!]
         
         ❌ CLUSTER DISRUPTED for no good reason!
```

### The Solution: PreVote

Before incrementing term and starting a real election, a node asks: **"If I were to run, would you vote for me?"**

```rust
// PreVote flow (implemented in node.rs)
fn on_election_timeout(&mut self) {
    // Phase 1: Ask permission first
    let msg = self.start_prevote();  // term stays unchanged!
    broadcast(msg);
}

fn handle_prevote_response(&mut self, granted: bool, from: NodeId) {
    if granted {
        self.prevotes_received.push(from);
    }
    
    // Only proceed if majority says "yes, we'd vote for you"
    if self.has_prevote_quorum() {
        // Phase 2: NOW increment term and start real election
        self.start_election();
    }
}
```

### How Healthy Nodes Reject Rogue PreVotes

```rust
fn handle_prevote_request(&mut self, ...) -> PreVoteResponse {
    // KEY INSIGHT: Reject if we've heard from leader recently
    if self.last_heartbeat_time.is_some() {
        return PreVoteResponse { vote_granted: false };
    }
    // ... normal log checks ...
}
```

**Result:** The disconnected node C with term=50:
1. Sends PreVoteRequest to A and B
2. A and B have heard from leader recently → REJECT
3. C never gets pre-vote quorum → never starts real election
4. C's high term never infects the cluster
5. **Cluster stays stable!** ✅

## Phase 1: Leader Election

### Election Timer

Randomized timeout between **150-300ms** (configurable via `RaftConfig`):

```rust
pub struct RaftConfig {
    pub election_timeout_min: u64,  // default: 150ms
    pub election_timeout_max: u64,  // default: 300ms
    pub heartbeat_interval: u64,    // default: 50ms (must be < timeout_min)
}
```

**Why randomized?** Prevents split votes. If all nodes timed out simultaneously, they'd all become candidates and split the vote.

### Vote Request (RequestVote RPC)

```rust
VoteRequest {
    term: u64,           // candidate's term
    candidate_id: u64,   // node requesting vote
    last_log_index: u64, // index of candidate's last log entry
    last_log_term: u64,  // term of candidate's last log entry
}
```

### Voting Rules

A node grants its vote if ALL of these are true:
1. `candidate_term >= my_term`
2. `voted_for` is None OR equals `candidate_id` (one vote per term)
3. Candidate's log is at least as up-to-date as ours:
   - Higher last log term wins, OR
   - Same last log term and longer/equal log wins

### Becoming Leader

When a candidate receives votes from a **majority (quorum)** of nodes:

| Cluster Size | Quorum Needed | Can Tolerate |
|--------------|---------------|--------------|
| 3 nodes      | 2 votes       | 1 failure    |
| 5 nodes      | 3 votes       | 2 failures   |
| 7 nodes      | 4 votes       | 3 failures   |

Upon becoming leader:
1. Initialize `next_index[i] = last_log_index + 1` for all followers
2. Initialize `match_index[i] = 0` for all followers  
3. Send immediate heartbeat to establish authority

## Phase 2: Log Replication

### Log Entry Structure

```rust
pub struct LogEntry {
    pub term: u64,      // term when entry was created
    pub index: u64,     // position in log (1-indexed)
    pub command: Vec<u8>, // serialized command (e.g., "SET key value")
}
```

### AppendEntries RPC

```rust
AppendEntries {
    term: u64,           // leader's term
    leader_id: u64,      // for followers to redirect clients
    prev_log_index: u64, // index of entry just before new ones
    prev_log_term: u64,  // term of prev_log_index entry
    entries: Vec<LogEntry>, // new entries (empty = heartbeat)
    leader_commit: u64,  // leader's commit index
}
```

### Log Consistency Check

Follower accepts AppendEntries only if it has an entry at `prev_log_index` with term matching `prev_log_term`. This ensures log consistency:

```
Leader log:  [1:1][1:2][2:3][2:4][3:5]
             ─────────────────────────
                        ▲
AppendEntries: prev_log_index=4, prev_log_term=2, entries=[3:5]
                        │
Follower log:[1:1][1:2][2:3][2:4] ← Has entry 4 with term 2? ✓ ACCEPT
```

### Commit Process

An entry is **committed** when stored on a majority of nodes:

1. Leader replicates entry to followers
2. Followers acknowledge with `AppendEntriesResponse { success: true }`
3. Leader tracks `match_index` for each follower
4. When entry replicated to majority → advance `commit_index`
5. Leader includes `commit_index` in next heartbeat
6. Followers advance their `commit_index` and apply to state machine

## Safety Guarantees

### Election Safety
> At most one leader can be elected per term.

Guaranteed by: One vote per term per node.

### Log Matching  
> If two logs contain an entry with the same index and term, the logs are identical through that entry.

Guaranteed by: AppendEntries consistency check.

### Leader Completeness
> If an entry is committed in a term, it will be present in all future leaders' logs.

Guaranteed by: Vote restriction (only vote for candidates with logs at least as up-to-date).

### PreVote Safety
> A disconnected node cannot disrupt a stable cluster by having an inflated term.

Guaranteed by: PreVote protocol rejects candidates when a healthy leader exists.

## Our Implementation Highlights

1. **PreVote Enabled**: Prevents disruptive server problem (Raft thesis Section 9.6)
2. **3-Node Cluster**: Tolerates 1 failure, simplest production configuration
3. **Key-Value Commands**: `SET`, `GET`, `DELETE` operations
4. **WASM Portable**: Runs in browser via WASI 0.2 shims
5. **120+ Tests**: Comprehensive coverage of all Raft scenarios
