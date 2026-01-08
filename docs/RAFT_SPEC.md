# Raft Algorithm Specification

<!-- 
why: document the raft algorithm as implemented in this project
relations: implemented by raft-core crate, visualized by dashboard
what: simplified raft explanation, state transitions, message types
-->

## Overview

This document describes our implementation of the Raft consensus algorithm, focusing on the two core phases: **Leader Election** and **Log Replication**.

## Node States

```
         timeout              receive majority votes
    ┌──────────────┐         ┌──────────────────────┐
    │              ▼         │                      ▼
┌───────────┐  ┌───────────────┐  ┌─────────────────────┐
│  FOLLOWER │  │   CANDIDATE   │  │       LEADER        │
│           │  │               │  │                     │
│ - Passive │  │ - Requests    │  │ - Sends heartbeats  │
│ - Votes   │  │   votes       │  │ - Manages log       │
│ - Listens │  │ - May become  │  │ - Handles commits   │
└───────────┘  │   leader      │  └─────────────────────┘
    ▲          └───────────────┘           │
    │                   │                  │
    │    discovers      │   discovers      │
    │    higher term    │   higher term    │
    └───────────────────┴──────────────────┘
```

## Phase 1: Leader Election

### Election Timer

TODO: Document randomized timeout (150-300ms), why randomization matters

### Vote Request

TODO: Document RequestVote RPC structure, term comparison

### Vote Response

TODO: Document voting rules, one vote per term

### Becoming Leader

TODO: Document majority requirement (quorum), immediate heartbeat

## Phase 2: Log Replication

### Log Entry Structure

TODO: Document (term, index, command) tuple

### AppendEntries RPC

TODO: Document heartbeats, log consistency check

### Commit Process

TODO: Document majority acknowledgment, commit index advancement

## Safety Guarantees

### Election Safety

TODO: At most one leader per term

### Log Matching

TODO: If two logs contain entry with same index and term, logs are identical up to that point

### Leader Completeness

TODO: Committed entries appear in all future leaders' logs

## Our Implementation Notes

TODO: Document any simplifications or variations from the Raft paper
