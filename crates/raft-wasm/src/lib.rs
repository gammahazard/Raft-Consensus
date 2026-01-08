//! # raft-wasm
//!
//! why: expose raft node functionality to javascript via wasm component model
//! relations: wraps raft-core and raft-storage, called by js host shim
//! what: wasm exports for node lifecycle, wit-bindgen bindings

// TODO: Implement in Phase 1 (feature/scaffold) after shim is ready

pub use raft_core::{NodeState, RaftNode, RaftMessage, LogEntry};
pub use raft_storage::{Storage, FileStorage};

// TODO: Add wit-bindgen exports
// wit_bindgen::generate!({
//     world: "raft-node",
// });
