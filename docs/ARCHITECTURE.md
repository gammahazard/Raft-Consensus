# Architecture

## Overview

The Raft Consensus Cluster demonstrates distributed consensus in the browser using WebAssembly (WASM). The same Raft implementation can run on real hardware (Raspberry Pi) with minimal changes to the host layer.

## System Components

```
┌─────────────────────────────────────────────────────────────┐
│                     Browser Runtime                          │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────────────────┐    │
│  │              Leptos Dashboard (UI)                   │    │
│  │  - Cluster visualization (3 nodes with state)        │    │
│  │  - Per-node log tracking (Log: X/Y)                  │    │
│  │  - Chaos controls (kill, restart, partition)         │    │
│  └─────────────────────────────────────────────────────┘    │
│                           │                                  │
│                           ▼                                  │
│  ┌─────────────────────────────────────────────────────┐    │
│  │            JavaScript Host Shim                      │    │
│  │  ┌──────────────────┐  ┌──────────────────┐         │    │
│  │  │  BroadcastChannel │  │    IndexedDB     │         │    │
│  │  │  (Virtual Network)│  │  (Virtual FS)    │         │    │
│  │  └──────────────────┘  └──────────────────┘         │    │
│  └─────────────────────────────────────────────────────┘    │
│                           │                                  │
│                           ▼                                  │
│  ┌─────────────────────────────────────────────────────┐    │
│  │              WASM Nodes (x3)                         │    │
│  │  ┌─────────┐ ┌─────────┐ ┌─────────┐                │    │
│  │  │raft-core│ │raft-    │ │raft-wasm│                │    │
│  │  │         │ │storage  │ │  (WASI) │                │    │
│  │  └─────────┘ └─────────┘ └─────────┘                │    │
│  └─────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
```

## Design Decisions

### Why Raft?

Raft was chosen over Paxos because:
- **Understandability**: Raft was designed to be easier to understand and implement correctly
- **Leader-based**: Single leader simplifies log replication logic
- **Well-documented**: The Raft paper and thesis provide clear implementation guidance
- **Industry adoption**: Used in etcd, CockroachDB, TiKV, and other production systems

### Why WASI 0.2?

WASI 0.2 (Component Model) provides:
- **Portability**: Same `.wasm` binary runs in browser AND on Raspberry Pi
- **Capability-based security**: Explicit grants for filesystem and network access
- **Size efficiency**: ~500KB vs 50-200MB for containers
- **Crash isolation**: WASM sandbox prevents consensus bugs from crashing host

### Why Browser-based?

- **Zero infrastructure**: No servers, databases, or Docker required
- **Portfolio-friendly**: Runs in any browser for demos
- **Educational**: Watch Raft state machine in real-time
- **Same binary**: Proves WASI portability claim

## Component Responsibilities

### raft-core

The pure Raft algorithm implementation:
- **State machine**: Leader, Follower, Candidate, PreCandidate states
- **Election logic**: Randomized timeouts, vote collection, term management
- **Log management**: Append, commit, truncate operations
- **PreVote protocol**: Prevents disruptive servers (Raft Thesis Section 9.6)

### raft-storage

Persistence abstraction:
- **Storage trait**: Generic interface for term, votedFor, and log persistence
- **FileStorage**: Uses std::fs for real filesystem (Pi deployment)
- **InMemoryStorage**: For testing and browser simulation

### raft-wasm

WASM bindings layer:
- **wasm-bindgen exports**: JavaScript-callable node lifecycle
- **Component bindings**: WASI 0.2 interface stubs
- **Host imports**: Time, random, network functions

### JavaScript Shim (shim/)

Browser API implementations:
- **network.js**: BroadcastChannel for inter-node messaging
- **filesystem.js**: IndexedDB for log persistence
- **host.js**: WASM instantiation and lifecycle management

## Data Flow

### Leader Election

```
1. Follower timeout (150-300ms without heartbeat)
2. Transition to PreCandidate
3. Send PreVoteRequest to all nodes
4. If majority grants pre-vote → become Candidate
5. Increment term, send VoteRequest to all nodes
6. If majority grants vote → become Leader
7. Send immediate heartbeat to establish authority
```

### Log Replication

```
1. Client sends command to Leader
2. Leader appends to local log
3. Leader sends AppendEntries to all Followers
4. Followers append and reply with success
5. When majority acknowledge → entry is committed
6. Leader advances commitIndex
7. Next heartbeat includes new commitIndex
8. Followers apply committed entries to state machine
```

### Crash Recovery

```
1. Node restarts → initializes as Follower
2. Loads persisted: currentTerm, votedFor, log entries
3. Receives heartbeat from Leader
4. Leader detects node's log is behind
5. Leader sends missing entries in AppendEntries
6. Node catches up to Leader's log
7. Node applies all committed entries
```

## Performance Considerations

- **No animations during catch-up**: Log sync happens instantly
- **Event log limited to 50 entries**: Prevents memory growth
- **Timeout measurements use performance.now()**: Sub-millisecond precision
- **Leptos signals for reactive updates**: Only re-render changed components
