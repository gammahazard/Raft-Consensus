# Raft Consensus Cluster

> **"Unsinkable" Distributed Consensus in the Browser via WASI 0.2**

[![Status](https://img.shields.io/badge/status-in_development-yellow)]()
[![Rust](https://img.shields.io/badge/rust-1.75+-orange)]()
[![WASI](https://img.shields.io/badge/WASI-0.2-blue)]()

## Overview

A browser-based High-Availability Control Plane simulation demonstrating **distributed consensus** (Raft algorithm) using Rust compiled to WASM with WASI 0.2 polyfills.

**The Flex**: Standard backend Rust (using File I/O and Networking) runs unchanged in the browser via WASI capability mapping.

## Features

- 🟢 **Leader Election**: Automatic failover when leaders crash
- 💾 **Durable Storage**: State persisted to IndexedDB via WASI fs
- 🌐 **Virtual Network**: BroadcastChannel simulates node communication
- 🎮 **Chaos Controls**: Kill nodes and watch the cluster self-heal
- 📊 **Real-time Visualization**: See elections and heartbeats in action

## Project Structure

```
raft-consensus/
├── crates/
│   ├── raft-core/      # Pure Raft algorithm logic
│   ├── raft-storage/   # Persistence layer (std::fs)
│   └── raft-wasm/      # WASM bindings and exports
├── shim/               # JavaScript WASI polyfills
├── src/                # Leptos dashboard
└── docs/               # Architecture documentation
```

## Tech Stack

| Layer | Technology |
|-------|------------|
| Logic | Rust (no_std compatible) |
| Binary | WebAssembly (wasm32-wasip1) |
| Interface | WASI 0.2 Component Model |
| UI | Leptos + Trunk |
| Polyfills | jco + custom shims |

## Documentation

- [ARCHITECTURE.md](docs/ARCHITECTURE.md) - System design and rationale
- [RAFT_SPEC.md](docs/RAFT_SPEC.md) - Algorithm implementation details
- [WASI_MAPPING.md](docs/WASI_MAPPING.md) - How std maps to browser APIs

## Getting Started

```bash
# Install dependencies
cargo install trunk
npm install

# Run development server
trunk serve

# Run tests
cargo test --workspace
```

## Portfolio Context

This project is part of a **Reliability Triad** demonstrating systems engineering:

| Project | Domain | Reliability Story |
|---------|--------|-------------------|
| ICS Guardian | Security | "I ensure the connection is safe." |
| Protocol Gateway | Edge | "I ensure the parser is crash-proof." |
| **Raft Cluster** | Distributed | "I ensure the system state is consistent." |

## License

MIT
