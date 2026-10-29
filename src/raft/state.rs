// ==============================================================================
// 🚦 NEXUS-KV: RAFT NODE STATE (وضعیت ماشین حالت گره توزیع‌شده)
// Architect: Ali Hasanifard
// ==============================================================================

use crate::raft::message::NodeId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeRole {
    Follower,
    Candidate,
    Leader,
}

#[derive(Debug, Clone)]
pub struct RaftState {
    pub current_term: u64,
    pub voted_for: Option<NodeId>,
    pub role: NodeRole,
    pub commit_index: u64,
    pub last_applied: u64,
    pub leader_id: Option<NodeId>,
}

impl RaftState {
    pub fn new() -> Self {
        Self {
            current_term: 0,
            voted_for: None,
            role: NodeRole::Follower,
            commit_index: 0,
            last_applied: 0,
            leader_id: None,
        }
    }
}
