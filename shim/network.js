/**
 * network.js
 * 
 * why: simulate network communication between wasm nodes using broadcastchannel
 * relations: used by host.js, intercepts wasi:sockets calls
 * what: VirtualNetwork class, message routing, latency simulation
 */

// TODO: Implement in Phase 1 (feature/scaffold)

/**
 * Virtual network that maps Rust TcpStream operations to BroadcastChannel.
 * 
 * Each node gets a unique channel for receiving messages.
 * Outgoing messages are broadcast to the cluster channel.
 */
export class VirtualNetwork {
    constructor() {
        /** @type {BroadcastChannel} */
        this.clusterChannel = new BroadcastChannel('raft-cluster');
        /** @type {Map<number, BroadcastChannel>} */
        this.nodeChannels = new Map();
    }

    /**
     * Register a node with the virtual network
     * @param {number} nodeId 
     * @param {function} onMessage - Callback for received messages
     */
    registerNode(nodeId, onMessage) {
        const channel = new BroadcastChannel(`raft-node-${nodeId}`);
        channel.onmessage = onMessage;
        this.nodeChannels.set(nodeId, channel);
    }

    /**
     * Send a message from one node to another
     * @param {number} from - Sender node ID
     * @param {number} to - Receiver node ID  
     * @param {object} message - The Raft message
     */
    send(from, to, message) {
        const channel = this.nodeChannels.get(to);
        if (channel) {
            channel.postMessage({ from, message });
        }
    }

    /**
     * Broadcast a message to all nodes
     * @param {number} from - Sender node ID
     * @param {object} message - The Raft message
     */
    broadcast(from, message) {
        this.clusterChannel.postMessage({ from, message });
    }
}
