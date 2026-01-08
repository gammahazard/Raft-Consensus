//! # raft-core
//!
//! why: implement the core raft consensus algorithm in pure, portable rust
//! relations: used by raft-wasm for browser execution, raft-storage for persistence
//! what: state machine, election logic, log management, message types

pub mod log;
pub mod message;
pub mod node;

pub use node::{NodeState, RaftNode, RaftConfig};
pub use message::RaftMessage;
pub use log::LogEntry;
