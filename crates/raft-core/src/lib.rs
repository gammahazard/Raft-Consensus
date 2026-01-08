//! # raft-core
//!
//! why: implement the core raft consensus algorithm in pure, portable rust
//! relations: used by raft-wasm for browser execution, raft-storage for persistence
//! what: state machine, election logic, log management, message types

// TODO: Implement in Phase 2 (feature/raft-core)

pub mod log;
pub mod message;
pub mod node;

pub use node::{NodeState, RaftNode};
pub use message::RaftMessage;
pub use log::LogEntry;
