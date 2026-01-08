/**
 * Raft WASI Component Bridge
 * 
 * This module provides JavaScript host functions for the transpiled
 * WASI component and exposes the Raft API to the browser dashboard.
 * 
 * Uses jco-transpiled raft.wasm with WASI 0.2 shims.
 */

// Import the transpiled component
import * as raft from './wasm/raft.js';

// Message queue for inter-node communication
const messageQueues = new Map(); // nodeId -> Array<{from, msg}>

// Storage (in-memory for browser demo)
const nodeState = new Map(); // nodeId -> {term, votedFor}
const nodeLogs = new Map();  // nodeId -> Array<LogEntry>

// Active nodes
const nodes = new Map(); // nodeId -> status

/**
 * Initialize a Raft node
 * @param {number} nodeId - This node's ID
 * @param {number[]} clusterNodes - All node IDs in cluster
 */
export function initNode(nodeId, clusterNodes) {
    const nodeIds = new BigUint64Array(clusterNodes.map(id => BigInt(id)));
    raft.raftApi.init(BigInt(nodeId), nodeIds);
    nodes.set(nodeId, { initialized: true });
    messageQueues.set(nodeId, []);
    nodeState.set(nodeId, { term: 0n, votedFor: null });
    nodeLogs.set(nodeId, []);
    console.log(`[WASI] Node ${nodeId} initialized with cluster:`, clusterNodes);
}

/**
 * Tick a node's state machine
 * @param {number} nodeId 
 * @returns {NodeStatus}
 */
export function tickNode(nodeId) {
    const status = raft.raftApi.tick();
    return {
        id: Number(status.id),
        state: status.state,
        term: Number(status.term),
        logLength: Number(status.logLength),
        commitIndex: Number(status.commitIndex)
    };
}

/**
 * Get node status without advancing state
 * @returns {NodeStatus}
 */
export function getNodeStatus() {
    const status = raft.raftApi.getStatus();
    return {
        id: Number(status.id),
        state: status.state,
        term: Number(status.term),
        logLength: Number(status.logLength),
        commitIndex: Number(status.commitIndex)
    };
}

/**
 * Submit a command to the cluster (only works on leader)
 * @param {string} command - The command (e.g., "SET key value")
 * @returns {boolean} - True if accepted
 */
export function submitCommand(command) {
    const encoder = new TextEncoder();
    const bytes = encoder.encode(command);
    return raft.raftApi.submitCommand(bytes);
}

/**
 * Deliver a message to a node
 * @param {number} fromNode 
 * @param {object} message - RaftMessage
 */
export function deliverMessage(toNode, fromNode, message) {
    const queue = messageQueues.get(toNode);
    if (queue) {
        queue.push({ from: fromNode, msg: message });
    }
}

/**
 * Process pending messages for a node
 * @param {number} nodeId 
 */
export function processPendingMessages(nodeId) {
    const queue = messageQueues.get(nodeId);
    if (!queue || queue.length === 0) return;

    while (queue.length > 0) {
        const { from, msg } = queue.shift();
        raft.raftApi.onMessage(BigInt(from), msg);
    }
}

// Host function implementations (called by WASM via imports)
// These need to be provided to jco when we have a custom host setup

/**
 * Create a PreVote request message
 */
export function createPreVoteRequest(term, candidateId, lastLogIndex, lastLogTerm) {
    return {
        tag: 'pre-vote-req',
        val: {
            term: BigInt(term),
            candidateId: BigInt(candidateId),
            lastLogIndex: BigInt(lastLogIndex),
            lastLogTerm: BigInt(lastLogTerm)
        }
    };
}

/**
 * Create a VoteRequest message
 */
export function createVoteRequest(term, candidateId, lastLogIndex, lastLogTerm) {
    return {
        tag: 'vote-req',
        val: {
            term: BigInt(term),
            candidateId: BigInt(candidateId),
            lastLogIndex: BigInt(lastLogIndex),
            lastLogTerm: BigInt(lastLogTerm)
        }
    };
}

/**
 * Create a VoteResponse message
 */
export function createVoteResponse(term, voteGranted) {
    return {
        tag: 'vote-res',
        val: {
            term: BigInt(term),
            voteGranted: voteGranted
        }
    };
}

// Export types for documentation
export const NodeStates = {
    FOLLOWER: 'follower',
    CANDIDATE: 'candidate',
    LEADER: 'leader',
    DEAD: 'dead'
};

export default {
    initNode,
    tickNode,
    getNodeStatus,
    submitCommand,
    deliverMessage,
    processPendingMessages,
    createPreVoteRequest,
    createVoteRequest,
    createVoteResponse,
    NodeStates
};
