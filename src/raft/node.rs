// ==============================================================================
// 🏛️ NEXUS-KV: RAFT CONSENSUS NODE (گره اجماع توزیع‌شده با الگوریتم Raft)
// Architect: Ali Hasanifard
// ==============================================================================

use crate::raft::log::{RaftCommand, RaftLog};
use crate::raft::message::*;
use crate::raft::state::{NodeRole, RaftState};
use parking_lot::RwLock;
use std::collections::HashMap;

pub struct RaftNode {
    pub id: NodeId,
    pub peers: Vec<NodeId>,
    pub state: RwLock<RaftState>,
    pub log: RwLock<RaftLog>,
    // Volatile state on leaders: next index to send to each peer, match index
    next_index: RwLock<HashMap<NodeId, u64>>,
    match_index: RwLock<HashMap<NodeId, u64>>,
}

impl RaftNode {
    pub fn new(id: NodeId, peers: Vec<NodeId>) -> Self {
        Self {
            id,
            peers,
            state: RwLock::new(RaftState::new()),
            log: RwLock::new(RaftLog::new()),
            next_index: RwLock::new(HashMap::new()),
            match_index: RwLock::new(HashMap::new()),
        }
    }

    /// Handle incoming RequestVote RPC
    pub fn handle_request_vote(&self, args: &RequestVoteArgs) -> RequestVoteReply {
        let mut state = self.state.write();
        let log = self.log.read();

        // 1. If term < current_term, reject
        if args.term < state.current_term {
            return RequestVoteReply {
                term: state.current_term,
                vote_granted: false,
            };
        }

        // 2. If term > current_term, step down to Follower
        if args.term > state.current_term {
            state.current_term = args.term;
            state.role = NodeRole::Follower;
            state.voted_for = None;
            state.leader_id = None;
        }

        // 3. Check if we already voted for another candidate in this term
        let can_vote = match state.voted_for {
            None => true,
            Some(voted) => voted == args.candidate_id,
        };

        // 4. Candidate's log must be at least as up-to-date as receiver's log
        let my_last_index = log.last_log_index();
        let my_last_term = log.last_log_term();

        let log_ok = args.last_log_term > my_last_term
            || (args.last_log_term == my_last_term && args.last_log_index >= my_last_index);

        if can_vote && log_ok {
            state.voted_for = Some(args.candidate_id);
            RequestVoteReply {
                term: state.current_term,
                vote_granted: true,
            }
        } else {
            RequestVoteReply {
                term: state.current_term,
                vote_granted: false,
            }
        }
    }

    /// Handle incoming AppendEntries RPC (Heartbeat & Log Replication)
    pub fn handle_append_entries(&self, args: &AppendEntriesArgs) -> AppendEntriesReply {
        let mut state = self.state.write();
        let mut log = self.log.write();

        // 1. Reply false if term < current_term
        if args.term < state.current_term {
            return AppendEntriesReply {
                term: state.current_term,
                success: false,
                match_index: 0,
            };
        }

        // 2. If term >= current_term, recognize leader and stay/become Follower
        if args.term > state.current_term || state.role != NodeRole::Follower {
            state.current_term = args.term;
            state.role = NodeRole::Follower;
            state.voted_for = None;
        }
        state.leader_id = Some(args.leader_id);

        // 3. Reply false if log doesn't contain entry at prev_log_index matching prev_log_term
        if args.prev_log_index > 0 {
            match log.get(args.prev_log_index) {
                Some(entry) if entry.term == args.prev_log_term => {}
                _ => {
                    return AppendEntriesReply {
                        term: state.current_term,
                        success: false,
                        match_index: log.last_log_index(),
                    };
                }
            }
        }

        // 4. Append any new entries not already in the log
        for entry in &args.entries {
            if let Some(existing) = log.get(entry.index) {
                if existing.term != entry.term {
                    // Conflict: delete existing entry and all that follow it
                    log.truncate_from(entry.index);
                    log.append(entry.term, entry.command.clone());
                }
            } else {
                log.append(entry.term, entry.command.clone());
            }
        }

        // 5. Update commit_index if leader_commit > commit_index
        if args.leader_commit > state.commit_index {
            let last_new_entry = args.entries.last().map(|e| e.index).unwrap_or(args.prev_log_index);
            state.commit_index = args.leader_commit.min(last_new_entry);
        }

        AppendEntriesReply {
            term: state.current_term,
            success: true,
            match_index: log.last_log_index(),
        }
    }

    /// Promote to Candidate and construct RequestVoteArgs
    pub fn start_election(&self) -> (RequestVoteArgs, usize) {
        let mut state = self.state.write();
        let log = self.log.read();

        state.role = NodeRole::Candidate;
        state.current_term += 1;
        state.voted_for = Some(self.id);
        state.leader_id = None;

        let args = RequestVoteArgs {
            term: state.current_term,
            candidate_id: self.id,
            last_log_index: log.last_log_index(),
            last_log_term: log.last_log_term(),
        };

        (args, 1) // 1 vote from self
    }

    /// Transition to Leader when majority is won
    pub fn become_leader(&self) {
        let mut state = self.state.write();
        state.role = NodeRole::Leader;
        state.leader_id = Some(self.id);

        let last_log = self.log.read().last_log_index();
        let mut next_idx = self.next_index.write();
        let mut match_idx = self.match_index.write();

        for peer in &self.peers {
            next_idx.insert(*peer, last_log + 1);
            match_idx.insert(*peer, 0);
        }
    }

    /// Propose client command to the cluster (Leader only)
    pub fn propose(&self, cmd: RaftCommand) -> Result<u64, Option<NodeId>> {
        let state = self.state.read();
        if state.role != NodeRole::Leader {
            return Err(state.leader_id);
        }

        let mut log = self.log.write();
        let idx = log.append(state.current_term, cmd);
        Ok(idx)
    }

    /// Create heartbeat message for peer
    pub fn create_heartbeat(&self, peer_id: NodeId) -> AppendEntriesArgs {
        let state = self.state.read();
        let log = self.log.read();

        let next_idx = self.next_index.read().get(&peer_id).copied().unwrap_or(log.last_log_index() + 1);
        let prev_log_index = next_idx.saturating_sub(1);
        let prev_log_term = if prev_log_index > 0 {
            log.get(prev_log_index).map(|e| e.term).unwrap_or(0)
        } else {
            0
        };

        let entries = log.slice_from(next_idx);

        AppendEntriesArgs {
            term: state.current_term,
            leader_id: self.id,
            prev_log_index,
            prev_log_term,
            entries,
            leader_commit: state.commit_index,
        }
    }

    /// Process AppendEntries reply from peer
    pub fn handle_peer_append_reply(&self, peer_id: NodeId, reply: &AppendEntriesReply) -> bool {
        let mut state = self.state.write();
        if reply.term > state.current_term {
            state.current_term = reply.term;
            state.role = NodeRole::Follower;
            state.voted_for = None;
            state.leader_id = None;
            return false;
        }

        if state.role != NodeRole::Leader {
            return false;
        }

        if reply.success {
            self.next_index.write().insert(peer_id, reply.match_index + 1);
            self.match_index.write().insert(peer_id, reply.match_index);

            // Check if commit_index can be advanced (quorum agreement)
            let total_nodes = self.peers.len() + 1;
            let majority = total_nodes / 2 + 1;

            let log = self.log.read();
            let mut matches: Vec<u64> = self.match_index.read().values().copied().collect();
            matches.push(log.last_log_index());
            matches.sort_unstable();

            // Median of matches gives the highest index replicated on majority
            let median_match = matches[matches.len() - majority];
            if median_match > state.commit_index {
                if let Some(entry) = log.get(median_match) {
                    if entry.term == state.current_term {
                        state.commit_index = median_match;
                        return true; // Newly committed!
                    }
                }
            }
        } else {
            // Decrement next_index on conflict
            let mut next_idx = self.next_index.write();
            if let Some(idx) = next_idx.get_mut(&peer_id) {
                if *idx > 1 {
                    *idx -= 1;
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_raft_election_and_quorum_replication() {
        let node1 = Arc::new(RaftNode::new(1, vec![2, 3]));
        let node2 = Arc::new(RaftNode::new(2, vec![1, 3]));
        let node3 = Arc::new(RaftNode::new(3, vec![1, 2]));

        // 1. Node 1 starts election
        let (vote_args, mut votes) = node1.start_election();
        assert_eq!(node1.state.read().role, NodeRole::Candidate);

        // 2. Peers vote
        let r2 = node2.handle_request_vote(&vote_args);
        let r3 = node3.handle_request_vote(&vote_args);

        if r2.vote_granted { votes += 1; }
        if r3.vote_granted { votes += 1; }

        assert!(votes >= 2); // Won majority
        node1.become_leader();
        assert_eq!(node1.state.read().role, NodeRole::Leader);

        // 3. Propose command
        let log_idx = node1.propose(RaftCommand::Put {
            key: b"user:1".to_vec(),
            value: b"Ali".to_vec(),
        }).unwrap();
        assert_eq!(log_idx, 1);

        // 4. Replicate to Node 2
        let heartbeat_n2 = node1.create_heartbeat(2);
        let reply_n2 = node2.handle_append_entries(&heartbeat_n2);
        assert!(reply_n2.success);

        // 5. Update match and verify commit
        let committed = node1.handle_peer_append_reply(2, &reply_n2);
        assert!(committed);
        assert_eq!(node1.state.read().commit_index, 1);
    }
}
