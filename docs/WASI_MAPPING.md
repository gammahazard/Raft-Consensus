# WASI Capability Mapping

<!-- 
why: document how standard rust apis map to browser apis via wasi
relations: implemented by shim/*.js, used by raft-storage and raft-wasm
what: std::fs -> indexeddb, std::net -> broadcastchannel mappings
-->

## Overview

This document explains how we use WASI 0.2 to run standard Rust backend code in the browser by mapping system capabilities to browser APIs.

## The Magic

```
┌─────────────────────────────────────────────────────────────┐
│                    Rust Code (Portable)                      │
│                                                              │
│   std::fs::write("state.log", data)?;                       │
│   socket.send(message)?;                                     │
│                                                              │
└──────────────────────────┬──────────────────────────────────┘
                           │ compiles to
                           ▼
┌─────────────────────────────────────────────────────────────┐
│                    WASM Component                            │
│                                                              │
│   wasi:filesystem/types.write(...)                          │
│   wasi:sockets/tcp.send(...)                                │
│                                                              │
└──────────────────────────┬──────────────────────────────────┘
                           │ intercepted by
                           ▼
┌─────────────────────────────────────────────────────────────┐
│                    JavaScript Shim                           │
│                                                              │
│   indexedDB.put(...)                                         │
│   broadcastChannel.postMessage(...)                          │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

## Filesystem Mapping

### std::fs → IndexedDB

| Rust API | WASI Interface | Browser Implementation |
|----------|----------------|------------------------|
| `std::fs::write()` | `wasi:filesystem/types.write` | `IDBObjectStore.put()` |
| `std::fs::read()` | `wasi:filesystem/types.read` | `IDBObjectStore.get()` |
| `std::fs::remove_file()` | `wasi:filesystem/types.unlink` | `IDBObjectStore.delete()` |

### Implementation Details

The browser shim uses IndexedDB with a per-node database namespace:

```javascript
// filesystem.js
const dbName = `raft-node-${nodeId}`;
const storeName = 'state';

// Write: stores term, votedFor, and log entries
await db.put(storeName, { key: 'currentTerm', value: term });
await db.put(storeName, { key: 'votedFor', value: nodeId });
await db.put(storeName, { key: 'log', value: entries });
```

## Networking Mapping

### std::net → BroadcastChannel

| Rust API | WASI Interface | Browser Implementation |
|----------|----------------|------------------------|
| `TcpStream::connect()` | `wasi:sockets/tcp.connect` | `new BroadcastChannel()` |
| `stream.write()` | `wasi:sockets/tcp.send` | `channel.postMessage()` |
| `stream.read()` | `wasi:sockets/tcp.receive` | `channel.onmessage` |

### Implementation Details

Node addressing uses a shared `raft-cluster` channel with message routing:

```javascript
// network.js
const channel = new BroadcastChannel('raft-cluster');

// Send to specific node
channel.postMessage({
  from: myNodeId,
  to: targetNodeId,
  payload: raftMessage
});

// Receive (filter by destination)
channel.onmessage = (event) => {
  if (event.data.to === myNodeId) {
    handleMessage(event.data.payload);
  }
};
```

## jco Configuration

The WASM component is transpiled using jco:

```bash
jco transpile target/wasm32-wasip1/debug/raft_wasm.wasm -o shim/wasm --name raft
```

This generates:
- `shim/wasm/raft.js` — JavaScript bindings (89KB)
- `shim/wasm/raft.d.ts` — TypeScript declarations
- `shim/wasm/interfaces/` — WASI interface stubs

## Component Model Interfaces

### WIT Definitions

Our custom `wit/raft.wit` defines the component interface:

```wit
interface host {
  send-message: func(to: u64, msg: raft-message);
  persist-state: func(term: u64, voted-for: option<u64>);
  now-ms: func() -> u64;
  random-timeout: func(min: u64, max: u64) -> u64;
}

interface raft-api {
  init: func(node-id: u64, node-ids: list<u64>);
  tick: func() -> node-status;
  on-message: func(from-node: u64, msg: raft-message);
  submit-command: func(command: list<u8>) -> bool;
  get-status: func() -> node-status;
}
```

## Limitations

**Not Supported in Browser:**
- Actual TCP sockets — using BroadcastChannel instead
- File system seeking — IndexedDB is key-value only
- Blocking I/O — all operations are async
- Multiple files — single "database" per node

**Production (Raspberry Pi) will use:**
- Real TCP sockets via Wasmtime
- Real filesystem via std::fs
- Same WASM binary, different host implementation
