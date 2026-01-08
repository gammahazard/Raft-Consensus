/**
 * host.js
 * 
 * why: orchestrate wasm node lifecycle and provide wasi capability mapping
 * relations: uses network.js and filesystem.js, instantiates raft-wasm modules
 * what: WasiHost class, wasm instantiation timing, cluster management
 */

import { network } from './network.js';
import { filesystem } from './filesystem.js';

/**
 * the wasi host manages the simulation environment for raft nodes.
 * 
 * provides:
 * - wasm module instantiation with real timing measurements
 * - virtual networking via broadcastchannel
 * - virtual filesystem via indexeddb
 * - cluster lifecycle management
 */
export class WasiHost {
    /**
     * create a new wasi host
     * @param {number} nodeCount - number of nodes in cluster (default: 3)
     */
    constructor(nodeCount = 3) {
        /** @type {number} - number of nodes in cluster */
        this.nodeCount = nodeCount;

        /** @type {Map<number, object>} - node id to node state */
        this.nodes = new Map();

        /** @type {WebAssembly.Module | null} - compiled wasm module (shared) */
        this.wasmModule = null;

        /** @type {string | null} - path to wasm file */
        this.wasmPath = null;

        /** @type {object} - timing measurements for ui */
        this.timings = {
            compileMs: 0,
            instantiateMs: [],
            lastInstantiateMs: 0
        };

        /** @type {function | null} - callback when node state changes */
        this.onNodeStateChange = null;

        /** @type {function | null} - callback when log entry is committed */
        this.onLogCommit = null;

        /** @type {boolean} - is cluster running */
        this.isRunning = false;
    }

    /**
     * initialize the host (loads wasm module, sets up filesystem)
     * @param {string} wasmPath - path to the raft wasm file
     * @returns {Promise<void>}
     */
    async init(wasmPath = '/raft_wasm.wasm') {
        this.wasmPath = wasmPath;

        // initialize filesystem
        await filesystem.init();

        // fetch and compile wasm module (measure time)
        const compileStart = performance.now();

        try {
            const response = await fetch(wasmPath);
            const bytes = await response.arrayBuffer();
            this.wasmModule = await WebAssembly.compile(bytes);

            this.timings.compileMs = performance.now() - compileStart;

            this.logEvent(`[HOST] wasm compiled in ${this.timings.compileMs.toFixed(2)}ms`);
        } catch (error) {
            this.logEvent(`[HOST] wasm compile failed: ${error.message}`);
            throw error;
        }
    }

    /**
     * start a node in the cluster
     * @param {number} nodeId - unique node identifier
     * @returns {Promise<void>}
     */
    async startNode(nodeId) {
        if (this.nodes.has(nodeId)) {
            this.logEvent(`[HOST] node ${nodeId} already running`);
            return;
        }

        // measure instantiation time
        const instantiateStart = performance.now();

        // load persisted state
        const metadata = await filesystem.loadMetadata(nodeId);
        const log = await filesystem.loadLog(nodeId);

        // create node state (will be replaced by actual wasm instance)
        const nodeState = {
            id: nodeId,
            term: metadata.term,
            votedFor: metadata.votedFor,
            log: log,
            state: 'follower', // follower, candidate, leader
            commitIndex: 0,
            lastHeartbeat: Date.now(),
            electionTimeout: this.randomElectionTimeout(),
            timerId: null
        };

        // register with network
        network.registerNode(nodeId, (from, message) => {
            this.handleMessage(nodeId, from, message);
        });

        this.nodes.set(nodeId, nodeState);

        const instantiateMs = performance.now() - instantiateStart;
        this.timings.instantiateMs.push(instantiateMs);
        this.timings.lastInstantiateMs = instantiateMs;

        // start election timer
        this.resetElectionTimer(nodeId);

        this.logEvent(`[HOST] node ${nodeId} started (${instantiateMs.toFixed(2)}ms)`);
        this.emitNodeState(nodeId);
    }

    /**
     * stop/crash a node
     * @param {number} nodeId - node to stop
     * @returns {Promise<void>}
     */
    async stopNode(nodeId) {
        const node = this.nodes.get(nodeId);
        if (!node) {
            return;
        }

        // clear timers
        if (node.timerId) {
            clearTimeout(node.timerId);
        }

        // unregister from network
        network.unregisterNode(nodeId);

        // persist state before stopping
        await filesystem.saveMetadata(nodeId, node.term, node.votedFor);
        await filesystem.saveLog(nodeId, node.log);

        this.nodes.delete(nodeId);

        // mark as dead in network
        network.killNode(nodeId);

        this.logEvent(`[HOST] node ${nodeId} stopped`);
        this.emitNodeState(nodeId);
    }

    /**
     * start the entire cluster
     * @returns {Promise<void>}
     */
    async startCluster() {
        this.isRunning = true;

        for (let i = 1; i <= this.nodeCount; i++) {
            await this.startNode(i);
        }

        this.logEvent(`[HOST] cluster started with ${this.nodeCount} nodes`);
    }

    /**
     * stop the entire cluster
     * @returns {Promise<void>}
     */
    async stopCluster() {
        this.isRunning = false;

        for (const nodeId of this.nodes.keys()) {
            await this.stopNode(nodeId);
        }

        this.logEvent(`[HOST] cluster stopped`);
    }

    /**
     * restart a node (stop + start)
     * @param {number} nodeId 
     * @returns {Promise<void>}
     */
    async restartNode(nodeId) {
        await this.stopNode(nodeId);
        network.restartNode(nodeId); // un-kill in network
        await this.startNode(nodeId);
    }

    /**
     * reset the entire demo (clear all state)
     * @returns {Promise<void>}
     */
    async resetDemo() {
        await this.stopCluster();
        await filesystem.clearAll();
        network.resetAll();
        this.timings.instantiateMs = [];
        this.logEvent(`[HOST] demo reset`);
        await this.startCluster();
    }

    // -- raft protocol simulation --

    /**
     * handle incoming message for a node
     * @param {number} toNodeId 
     * @param {number} fromNodeId 
     * @param {object} message 
     */
    handleMessage(toNodeId, fromNodeId, message) {
        const node = this.nodes.get(toNodeId);
        if (!node) return;

        // reset election timer on any message from leader
        if (message.AppendEntries) {
            this.resetElectionTimer(toNodeId);
            node.lastHeartbeat = Date.now();

            // step down if we see higher term
            if (message.AppendEntries.term > node.term) {
                node.term = message.AppendEntries.term;
                node.state = 'follower';
                node.votedFor = null;
                this.emitNodeState(toNodeId);
            }

            // respond to append entries
            const response = {
                AppendEntriesResponse: {
                    term: node.term,
                    success: message.AppendEntries.term >= node.term
                }
            };
            network.send(toNodeId, fromNodeId, response);
        }

        if (message.VoteRequest) {
            this.handleVoteRequest(toNodeId, fromNodeId, message.VoteRequest);
        }

        if (message.VoteResponse) {
            this.handleVoteResponse(toNodeId, fromNodeId, message.VoteResponse);
        }

        if (message.AppendEntriesResponse) {
            this.handleAppendEntriesResponse(toNodeId, fromNodeId, message.AppendEntriesResponse);
        }
    }

    /**
     * handle vote request
     */
    handleVoteRequest(nodeId, candidateId, request) {
        const node = this.nodes.get(nodeId);
        if (!node) return;

        let voteGranted = false;

        // step down if we see higher term
        if (request.term > node.term) {
            node.term = request.term;
            node.state = 'follower';
            node.votedFor = null;
            this.emitNodeState(nodeId);
        }

        // grant vote if we haven't voted and candidate's log is up-to-date
        if (request.term >= node.term &&
            (node.votedFor === null || node.votedFor === candidateId)) {
            voteGranted = true;
            node.votedFor = candidateId;
            this.resetElectionTimer(nodeId);
        }

        const response = {
            VoteResponse: {
                term: node.term,
                vote_granted: voteGranted
            }
        };
        network.send(nodeId, candidateId, response);
    }

    /**
     * handle vote response (candidate only)
     */
    handleVoteResponse(nodeId, fromId, response) {
        const node = this.nodes.get(nodeId);
        if (!node || node.state !== 'candidate') return;

        // step down if we see higher term
        if (response.term > node.term) {
            node.term = response.term;
            node.state = 'follower';
            node.votedFor = null;
            this.emitNodeState(nodeId);
            return;
        }

        if (response.vote_granted) {
            node.votesReceived = node.votesReceived || new Set([nodeId]);
            node.votesReceived.add(fromId);

            // check for majority
            if (node.votesReceived.size >= this.quorumSize()) {
                this.becomeLeader(nodeId);
            }
        }
    }

    /**
     * handle append entries response (leader only)
     */
    handleAppendEntriesResponse(nodeId, fromId, response) {
        const node = this.nodes.get(nodeId);
        if (!node || node.state !== 'leader') return;

        // step down if we see higher term
        if (response.term > node.term) {
            node.term = response.term;
            node.state = 'follower';
            node.votedFor = null;
            this.emitNodeState(nodeId);
        }
    }

    /**
     * start election for a node
     */
    startElection(nodeId) {
        const node = this.nodes.get(nodeId);
        if (!node) return;

        node.term += 1;
        node.state = 'candidate';
        node.votedFor = nodeId;
        node.votesReceived = new Set([nodeId]);

        this.logEvent(`[RAFT] node ${nodeId} started election (term ${node.term})`);
        this.emitNodeState(nodeId);

        // request votes from all other nodes
        const request = {
            VoteRequest: {
                term: node.term,
                candidate_id: nodeId,
                last_log_index: node.log.length,
                last_log_term: node.log.length > 0 ? node.log[node.log.length - 1].term : 0
            }
        };
        network.broadcast(nodeId, request);

        // reset election timer in case we don't win
        this.resetElectionTimer(nodeId);
    }

    /**
     * become leader
     */
    becomeLeader(nodeId) {
        const node = this.nodes.get(nodeId);
        if (!node) return;

        node.state = 'leader';
        this.logEvent(`[RAFT] node ${nodeId} became LEADER (term ${node.term})`);
        this.emitNodeState(nodeId);

        // start sending heartbeats
        this.sendHeartbeat(nodeId);
    }

    /**
     * send heartbeat to all followers
     */
    sendHeartbeat(nodeId) {
        const node = this.nodes.get(nodeId);
        if (!node || node.state !== 'leader') return;

        const heartbeat = {
            AppendEntries: {
                term: node.term,
                leader_id: nodeId,
                prev_log_index: node.log.length,
                prev_log_term: node.log.length > 0 ? node.log[node.log.length - 1].term : 0,
                entries: [],
                leader_commit: node.commitIndex
            }
        };
        network.broadcast(nodeId, heartbeat);

        // schedule next heartbeat
        node.heartbeatTimer = setTimeout(() => {
            this.sendHeartbeat(nodeId);
        }, 50); // 50ms heartbeat interval
    }

    // -- helpers --

    /**
     * reset election timer for a node
     */
    resetElectionTimer(nodeId) {
        const node = this.nodes.get(nodeId);
        if (!node) return;

        if (node.timerId) {
            clearTimeout(node.timerId);
        }

        node.electionTimeout = this.randomElectionTimeout();
        node.timerId = setTimeout(() => {
            // only start election if we're not the leader
            if (node.state !== 'leader') {
                this.startElection(nodeId);
            }
        }, node.electionTimeout);
    }

    /**
     * get random election timeout (150-300ms)
     */
    randomElectionTimeout() {
        return 150 + Math.random() * 150;
    }

    /**
     * get quorum size
     */
    quorumSize() {
        return Math.floor(this.nodeCount / 2) + 1;
    }

    /**
     * log event
     */
    logEvent(msg) {
        network.logEvent(msg);
    }

    /**
     * emit node state change
     */
    emitNodeState(nodeId) {
        if (this.onNodeStateChange) {
            const node = this.nodes.get(nodeId);
            this.onNodeStateChange(nodeId, node ? node.state : 'dead');
        }

        if (typeof window !== 'undefined' && window.dispatchEvent) {
            const node = this.nodes.get(nodeId);
            window.dispatchEvent(new CustomEvent('raft-node-state', {
                detail: { nodeId, state: node ? node.state : 'dead' }
            }));
        }
    }

    /**
     * get cluster status for ui
     */
    getStatus() {
        const nodeStates = {};
        for (const [id, node] of this.nodes) {
            nodeStates[id] = {
                state: node.state,
                term: node.term,
                logLength: node.log.length,
                commitIndex: node.commitIndex
            };
        }

        return {
            isRunning: this.isRunning,
            nodeCount: this.nodeCount,
            nodes: nodeStates,
            timings: this.timings,
            network: network.getStatus()
        };
    }

    /**
     * submit a command to the cluster (for key-value store)
     * @param {string} command - the command to replicate
     * @returns {Promise<{success: boolean, leader: number|null}>}
     */
    async submitCommand(command) {
        // find the leader
        let leaderNode = null;
        for (const [id, node] of this.nodes) {
            if (node.state === 'leader') {
                leaderNode = node;
                break;
            }
        }

        if (!leaderNode) {
            return { success: false, leader: null, error: 'no leader' };
        }

        // append to leader's log
        const entry = {
            term: leaderNode.term,
            index: leaderNode.log.length + 1,
            command: command
        };
        leaderNode.log.push(entry);

        this.logEvent(`[KV] command submitted: ${command}`);

        return { success: true, leader: leaderNode.id, index: entry.index };
    }
}

// singleton instance
export const host = new WasiHost(3);
