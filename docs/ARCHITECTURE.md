# Architecture

<!-- 
why: explain the system design decisions and component relationships
relations: referenced by README.md, informs all implementation files
what: high-level architecture, component diagrams, design rationale
-->

## Overview

This document describes the architectural decisions behind the Raft Consensus Cluster browser simulation.

## System Components

```
┌─────────────────────────────────────────────────────────────┐
│                     Browser Runtime                          │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────────────────┐    │
│  │              Leptos Dashboard (UI)                   │    │
│  │  - Cluster visualization                             │    │
│  │  - Message animations                                │    │
│  │  - Chaos controls                                    │    │
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
│  │  │         │ │storage  │ │(bindgen)│                │    │
│  │  └─────────┘ └─────────┘ └─────────┘                │    │
│  └─────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
```

## Design Decisions

### Why Raft?

TODO: Explain Raft vs Paxos decision, understandability focus

### Why WASI 0.2?

TODO: Explain component model benefits, portability story

### Why Browser-based?

TODO: Explain portfolio demonstration value, no infrastructure needed

## Component Responsibilities

### raft-core

TODO: Document state machine, election logic, log management

### raft-storage

TODO: Document Storage trait, FileStorage implementation

### raft-wasm

TODO: Document WASM exports, component bindings

### JavaScript Shim

TODO: Document BroadcastChannel mapping, IndexedDB mapping

## Data Flow

TODO: Add sequence diagrams for:
- Leader election
- Log replication
- Crash recovery

## Performance Considerations

TODO: Document animation batching, message throttling
