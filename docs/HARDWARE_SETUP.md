# 🔧 Hardware Setup: 3-Node Raspberry Pi Cluster

> **Status**: Coming Soon — Hardware integration planned for future release

This guide documents the planned hardware demonstration of the Unsinkable Raft Cluster running on a 3-node Raspberry Pi cluster with real network communication.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                       RAFT CLUSTER - HARDWARE DEMO                          │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │                         WiFi Network                                 │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
│           │                      │                      │                   │
│           ▼                      ▼                      ▼                   │
│   ┌───────────────┐      ┌───────────────┐      ┌───────────────┐          │
│   │  Pi 4         │      │  Pi Zero #1   │      │  Pi Zero #2   │          │
│   │  (Node 1)     │◄────►│  (Node 2)     │◄────►│  (Node 3)     │          │
│   │               │      │               │      │               │          │
│   │ ┌───────────┐ │      │ ┌───────────┐ │      │ ┌───────────┐ │          │
│   │ │raft.wasm  │ │      │ │raft.wasm  │ │      │ │raft.wasm  │ │          │
│   │ │(LEADER)   │ │      │ │(FOLLOWER) │ │      │ │(FOLLOWER) │ │          │
│   │ │    🔵     │ │      │ │    🟢     │ │      │ │    🟢     │ │          │
│   │ └───────────┘ │      │ └───────────┘ │      │ └───────────┘ │          │
│   │               │      │               │      │               │          │
│   │ Log: [1,2,3]  │      │ Log: [1,2,3]  │      │ Log: [1,2,3]  │          │
│   └───────────────┘      └───────────────┘      └───────────────┘          │
│                                                                             │
│   Demo: Unplug Node 3 → Cluster continues → Plug back in → Auto-sync       │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Hardware Requirements

| Component | Model | Quantity | Purpose |
|-----------|-------|:--------:|---------|
| **Primary Node** | Raspberry Pi 4 (4GB) | 1 | Leader node (usually) |
| **Follower Nodes** | Raspberry Pi Zero 2 W | 2 | Lightweight cluster nodes |
| **Storage** | MicroSD Card (8GB+) | 3 | Boot + state persistence |
| **Power** | USB-C / Micro USB adapters | 3 | Node power |
| **Indicator** | WS2812B LED Strip | 1 | Cluster status visualization |
| **Alert** | Cylewet Buzzer | 1 | Audio on leader election |

## Network Configuration

All nodes communicate over the **Industrial Zone** network (isolated from management network):

| Node | Hostname | IP Address | Role |
|------|----------|------------|------|
| Pi 4 | `guardian-node-1` | `192.168.40.4` | Primary (Leader) |
| Pi Zero #1 | `guardian-node-2` | `192.168.40.X` | Follower |
| Pi Zero #2 | `guardian-node-3` | `192.168.40.X` | Follower |

> **Note:** Pi Zero 2W nodes arriving soon. IPs will be assigned in the 192.168.40.x industrial zone, segmented via UniFi Switch Lite 8 PoE.

### Static IP Setup (each node)

Edit `/etc/dhcpcd.conf`:

```bash
interface wlan0
static ip_address=192.168.40.X/24
static routers=192.168.40.1
static domain_name_servers=8.8.8.8
```

## Pi Zero 2 W Headless Setup

### Step 1: Flash Raspberry Pi OS Lite

```bash
# Use Raspberry Pi Imager
# Choose: Raspberry Pi OS Lite (64-bit)
# Configure: hostname, WiFi, SSH enabled
```

### Step 2: Configure WiFi (before first boot)

Create `wpa_supplicant.conf` on boot partition:

```conf
country=CA
ctrl_interface=DIR=/var/run/wpa_supplicant GROUP=netdev
update_config=1

network={
    ssid="YourWiFiName"
    psk="YourWiFiPassword"
    key_mgmt=WPA-PSK
}
```

### Step 3: Enable SSH

Create empty file named `ssh` (no extension) on boot partition.

### Step 4: First Boot

```bash
# Find Pi on network
ping guardian-node-2.local

# SSH in
ssh pi@guardian-node-2.local
```

## Software Setup

### On Each Node

```bash
# Install wasmtime
curl https://wasmtime.dev/install.sh -sSf | bash

# Clone project
git clone https://github.com/gammahazard/Raft-Consensus
cd raft-consensus

# Build pi-host
cd pi-host
cargo build --release
```

### Run Cluster

```bash
# On Node 1 (Pi 4)
./target/release/raft-node --id 1 --peers 192.168.1.102:5000,192.168.1.103:5000

# On Node 2 (Pi Zero #1)
./target/release/raft-node --id 2 --peers 192.168.1.101:5000,192.168.1.103:5000

# On Node 3 (Pi Zero #2)
./target/release/raft-node --id 3 --peers 192.168.1.101:5000,192.168.1.102:5000
```

## Project Structure

```
raft-consensus/
├── crates/
│   ├── raft-core/          # ← NO CHANGES NEEDED
│   ├── raft-storage/       # ← NO CHANGES NEEDED
│   └── raft-wasm/          # ← NO CHANGES NEEDED (same .wasm)
│
├── pi-host/                # ← NEW: Pi-specific host
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs         # Wasmtime loader + cluster coordinator
│       ├── shim/
│       │   ├── mod.rs
│       │   ├── filesystem.rs # Real std::fs (log persistence)
│       │   └── network.rs    # Real TCP sockets (peer communication)
│       ├── cluster.rs      # Peer discovery and management
│       └── led_status.rs   # WS2812B status display
```

| File | Purpose |
|------|---------|
| `pi-host/src/main.rs` | Load `raft.wasm`, connect to peers, run event loop |
| `pi-host/src/shim/filesystem.rs` | Persist term, voted_for, log to disk |
| `pi-host/src/shim/network.rs` | TCP connections for Raft RPCs |
| `pi-host/src/cluster.rs` | Maintain peer connections, handle reconnects |
| `pi-host/src/led_status.rs` | Show leader/follower/offline status |

## LED Status Visualization

| LED Color | Node State |
|:---------:|------------|
| 🔵 Blue | Leader |
| 🟢 Green | Follower (healthy) |
| 🟡 Yellow | Candidate (election in progress) |
| 🔴 Red | Offline / unreachable |

### Wiring (Pi 4 only — controls shared LED strip)

```
LED Strip (WS2812B)           Pi 4 GPIO
───────────────────           ─────────
VCC (Red)    ◄──────────────  5V (Pin 2)
GND (White)  ◄──────────────  GND (Pin 6)
DIN (Green)  ◄──────────────  GPIO18 (Pin 12)
```

## The Demo Script

### Demo 1: Normal Operation

1. Power on all 3 nodes
2. Wait for leader election (~300ms)
3. LED shows: 🔵🟢🟢
4. Log entries replicate to all nodes

### Demo 2: Single Node Failure

1. Unplug Pi Zero #2 (Node 3)
2. LED shows: 🔵🟢🔴
3. Cluster continues (2/3 quorum maintained)
4. New log entries still replicate to 2 nodes

### Demo 3: Majority Failure

1. Unplug Pi Zero #1 (Node 2) as well
2. LED shows: 🔵🔴🔴
3. Cluster **halts** — no quorum (1/3)
4. Safety: No new commits possible

### Demo 4: Recovery

1. Plug Pi Zero #2 back in
2. LED shows: 🔵🔴🟢
3. Cluster resumes (2/3 quorum)
4. Node 3 auto-syncs missed log entries

### Demo 5: Leader Failover

1. Unplug Pi 4 (current leader)
2. Pi Zeros detect timeout, start election
3. One becomes new leader
4. LED shows: 🔴🔵🟢 (new leader elected)

## What This Demonstrates

1. **Same WASM**: The exact `raft.wasm` from browser demo runs unmodified
2. **Real Consensus**: TCP-based Raft RPCs between physical devices
3. **Fault Tolerance**: Cluster survives minority failures
4. **Auto-Recovery**: Rejoining nodes automatically synchronize
5. **Visual Proof**: LED strip shows cluster state in real-time

---

*This hardware integration validates that WASI 0.2 components are truly portable: the same Raft binary runs identically in browsers (via BroadcastChannel) and on distributed edge devices (via TCP).*
