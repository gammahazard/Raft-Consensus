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

TODO: Document IndexedDB schema, per-node namespacing

## Networking Mapping

### std::net → BroadcastChannel

| Rust API | WASI Interface | Browser Implementation |
|----------|----------------|------------------------|
| `TcpStream::connect()` | `wasi:sockets/tcp.connect` | `new BroadcastChannel()` |
| `stream.write()` | `wasi:sockets/tcp.send` | `channel.postMessage()` |
| `stream.read()` | `wasi:sockets/tcp.receive` | `channel.onmessage` |

### Implementation Details

TODO: Document message routing, node addressing scheme

## jco Configuration

TODO: Document jco transpile settings, WASI shim imports

## Component Model Interfaces

### WIT Definitions

TODO: Document our custom WIT interfaces if any

## Limitations

TODO: Document what's NOT supported (e.g., actual TCP, file seek)
