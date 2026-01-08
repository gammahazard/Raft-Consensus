/**
 * network.js
 * 
 * why: simulate network communication between wasm nodes using broadcastchannel
 * relations: used by host.js, intercepts wasi:sockets calls
 * what: VirtualNetwork class, message routing, latency simulation, partition simulation
 */

/**
 * virtual network that maps raft message passing to broadcastchannel.
 * 
 * each node gets a unique channel for receiving messages.
 * supports latency simulation and network partitioning for chaos demos.
 */
export class VirtualNetwork {
    constructor() {
        /** @type {Map<number, BroadcastChannel>} - node id to channel */
        this.nodeChannels = new Map();

        /** @type {Map<number, function>} - node id to message callback */
        this.messageCallbacks = new Map();

        /** @type {Set<number>} - node ids in partition group A */
        this.partitionA = new Set();

        /** @type {Set<number>} - node ids in partition group B */
        this.partitionB = new Set();

        /** @type {boolean} - is network partitioned? */
        this.isPartitioned = false;

        /** @type {number} - simulated latency in ms (0 = instant) */
        this.latencyMs = 0;

        /** @type {Set<number>} - dead node ids (no messages in/out) */
        this.deadNodes = new Set();

        /** @type {Array} - event log for ui display */
        this.eventLog = [];
    }

    /**
     * register a node with the virtual network
     * @param {number} nodeId 
     * @param {function} onMessage - callback for received messages
     */
    registerNode(nodeId, onMessage) {
        const channel = new BroadcastChannel(`raft-node-${nodeId}`);

        channel.onmessage = (event) => {
            // ignore if this node is dead
            if (this.deadNodes.has(nodeId)) {
                this.logEvent(`[NET] message to dead node ${nodeId} dropped`);
                return;
            }

            const { from, message, timestamp } = event.data;

            // check partition: can from reach to?
            if (!this.canCommunicate(from, nodeId)) {
                this.logEvent(`[NET] partition: ${from} → ${nodeId} blocked`);
                return;
            }

            // add latency if configured
            if (this.latencyMs > 0) {
                setTimeout(() => onMessage(from, message), this.latencyMs);
            } else {
                onMessage(from, message);
            }
        };

        this.nodeChannels.set(nodeId, channel);
        this.messageCallbacks.set(nodeId, onMessage);
        this.logEvent(`[NET] node ${nodeId} registered`);
    }

    /**
     * unregister a node (cleanup)
     * @param {number} nodeId 
     */
    unregisterNode(nodeId) {
        const channel = this.nodeChannels.get(nodeId);
        if (channel) {
            channel.close();
            this.nodeChannels.delete(nodeId);
            this.messageCallbacks.delete(nodeId);
            this.logEvent(`[NET] node ${nodeId} unregistered`);
        }
    }

    /**
     * send a message from one node to another
     * @param {number} from - sender node id
     * @param {number} to - receiver node id  
     * @param {object} message - the raft message
     */
    send(from, to, message) {
        // ignore if sender is dead
        if (this.deadNodes.has(from)) {
            return;
        }

        // ignore if receiver is dead
        if (this.deadNodes.has(to)) {
            this.logEvent(`[NET] message to dead node ${to} dropped`);
            return;
        }

        // check partition
        if (!this.canCommunicate(from, to)) {
            this.logEvent(`[NET] partition: ${from} → ${to} blocked`);
            return;
        }

        const channel = this.nodeChannels.get(to);
        if (channel) {
            const payload = {
                from,
                message,
                timestamp: Date.now()
            };

            // add latency if configured
            if (this.latencyMs > 0) {
                setTimeout(() => channel.postMessage(payload), this.latencyMs);
            } else {
                channel.postMessage(payload);
            }

            this.logEvent(`[NET] ${from} → ${to}: ${this.messageType(message)}`);
        }
    }

    /**
     * broadcast a message to all nodes except sender
     * @param {number} from - sender node id
     * @param {object} message - the raft message
     */
    broadcast(from, message) {
        for (const [nodeId] of this.nodeChannels) {
            if (nodeId !== from) {
                this.send(from, nodeId, message);
            }
        }
    }

    // -- chaos controls --

    /**
     * kill a node (stop all messages in/out)
     * @param {number} nodeId 
     */
    killNode(nodeId) {
        this.deadNodes.add(nodeId);
        this.logEvent(`[CHAOS] node ${nodeId} killed`);
    }

    /**
     * restart a dead node
     * @param {number} nodeId 
     */
    restartNode(nodeId) {
        this.deadNodes.delete(nodeId);
        this.logEvent(`[CHAOS] node ${nodeId} restarted`);
    }

    /**
     * create a network partition
     * nodes in group A can only talk to group A
     * nodes in group B can only talk to group B
     * @param {number[]} groupA - node ids in partition A
     * @param {number[]} groupB - node ids in partition B
     */
    partition(groupA, groupB) {
        this.partitionA = new Set(groupA);
        this.partitionB = new Set(groupB);
        this.isPartitioned = true;
        this.logEvent(`[CHAOS] partition: [${groupA}] vs [${groupB}]`);
    }

    /**
     * heal the network partition
     */
    healPartition() {
        this.partitionA.clear();
        this.partitionB.clear();
        this.isPartitioned = false;
        this.logEvent(`[CHAOS] partition healed`);
    }

    /**
     * set simulated network latency
     * @param {number} ms - latency in milliseconds
     */
    setLatency(ms) {
        this.latencyMs = ms;
        this.logEvent(`[NET] latency set to ${ms}ms`);
    }

    /**
     * restart all nodes and heal network
     */
    resetAll() {
        this.deadNodes.clear();
        this.healPartition();
        this.setLatency(0);
        this.logEvent(`[CHAOS] all reset`);
    }

    // -- helper methods --

    /**
     * check if two nodes can communicate (not partitioned)
     * @param {number} from 
     * @param {number} to 
     * @returns {boolean}
     */
    canCommunicate(from, to) {
        if (!this.isPartitioned) {
            return true;
        }

        // both in same partition?
        const fromInA = this.partitionA.has(from);
        const toInA = this.partitionA.has(to);
        const fromInB = this.partitionB.has(from);
        const toInB = this.partitionB.has(to);

        return (fromInA && toInA) || (fromInB && toInB);
    }

    /**
     * get human-readable message type
     * @param {object} message 
     * @returns {string}
     */
    messageType(message) {
        if (!message) return 'unknown';
        if (message.VoteRequest) return 'VoteRequest';
        if (message.VoteResponse) return `VoteResponse(${message.VoteResponse.vote_granted ? 'yes' : 'no'})`;
        if (message.AppendEntries) {
            const count = message.AppendEntries.entries?.length || 0;
            return count === 0 ? 'Heartbeat' : `AppendEntries(${count})`;
        }
        if (message.AppendEntriesResponse) {
            return `AppendEntriesResponse(${message.AppendEntriesResponse.success ? 'ok' : 'fail'})`;
        }
        return JSON.stringify(message).slice(0, 30);
    }

    /**
     * log an event for ui display
     * @param {string} msg 
     */
    logEvent(msg) {
        const entry = {
            timestamp: new Date().toISOString().slice(11, 23),
            message: msg
        };
        this.eventLog.push(entry);

        // keep only last 100 events
        if (this.eventLog.length > 100) {
            this.eventLog.shift();
        }

        // emit for ui
        if (typeof window !== 'undefined' && window.dispatchEvent) {
            window.dispatchEvent(new CustomEvent('raft-network-event', { detail: entry }));
        }
    }

    /**
     * get cluster status for ui
     * @returns {object}
     */
    getStatus() {
        return {
            registeredNodes: Array.from(this.nodeChannels.keys()),
            deadNodes: Array.from(this.deadNodes),
            isPartitioned: this.isPartitioned,
            partitionA: Array.from(this.partitionA),
            partitionB: Array.from(this.partitionB),
            latencyMs: this.latencyMs,
            eventLog: this.eventLog.slice(-20)
        };
    }
}

// singleton instance for the cluster
export const network = new VirtualNetwork();
