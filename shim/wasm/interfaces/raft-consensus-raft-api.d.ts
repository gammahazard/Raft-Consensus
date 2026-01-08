/** @module Interface raft:consensus/raft-api **/
export function init(nodeId: bigint, nodeIds: BigUint64Array): void;
export function tick(): NodeStatus;
export function onMessage(fromNode: bigint, msg: RaftMessage): void;
export function submitCommand(command: Uint8Array): boolean;
export function getStatus(): NodeStatus;
export type NodeStatus = import('./raft-consensus-types.js').NodeStatus;
export type RaftMessage = import('./raft-consensus-types.js').RaftMessage;
