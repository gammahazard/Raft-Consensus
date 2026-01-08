/** @module Interface raft:consensus/types **/
/**
 * # Variants
 * 
 * ## `"follower"`
 * 
 * ## `"candidate"`
 * 
 * ## `"leader"`
 * 
 * ## `"dead"`
 */
export type NodeState = 'follower' | 'candidate' | 'leader' | 'dead';
export interface NodeStatus {
  id: bigint,
  state: NodeState,
  term: bigint,
  logLength: bigint,
  commitIndex: bigint,
}
export interface PreVoteRequest {
  term: bigint,
  candidateId: bigint,
  lastLogIndex: bigint,
  lastLogTerm: bigint,
}
export interface PreVoteResponse {
  term: bigint,
  voteGranted: boolean,
}
export interface VoteRequest {
  term: bigint,
  candidateId: bigint,
  lastLogIndex: bigint,
  lastLogTerm: bigint,
}
export interface VoteResponse {
  term: bigint,
  voteGranted: boolean,
}
export interface LogEntry {
  term: bigint,
  index: bigint,
  command: Uint8Array,
}
export interface AppendEntries {
  term: bigint,
  leaderId: bigint,
  prevLogIndex: bigint,
  prevLogTerm: bigint,
  entries: Array<LogEntry>,
  leaderCommit: bigint,
}
export interface AppendEntriesResponse {
  term: bigint,
  success: boolean,
}
export type RaftMessage = RaftMessagePreVoteReq | RaftMessagePreVoteRes | RaftMessageVoteReq | RaftMessageVoteRes | RaftMessageAppendReq | RaftMessageAppendRes;
export interface RaftMessagePreVoteReq {
  tag: 'pre-vote-req',
  val: PreVoteRequest,
}
export interface RaftMessagePreVoteRes {
  tag: 'pre-vote-res',
  val: PreVoteResponse,
}
export interface RaftMessageVoteReq {
  tag: 'vote-req',
  val: VoteRequest,
}
export interface RaftMessageVoteRes {
  tag: 'vote-res',
  val: VoteResponse,
}
export interface RaftMessageAppendReq {
  tag: 'append-req',
  val: AppendEntries,
}
export interface RaftMessageAppendRes {
  tag: 'append-res',
  val: AppendEntriesResponse,
}
