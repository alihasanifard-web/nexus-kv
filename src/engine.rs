// ==============================================================================
// 🚀 NEXUS-KV: UNIFIED DATABASE ENGINE (ترکیب یکپارچه اجماع Raft و ذخیره‌سازی LSM)
// Architect: Ali Hasanifard
// ==============================================================================

use crate::lsm::{LsmConfig, LsmEngine};
use crate::proto::resp::RespValue;
use crate::raft::log::RaftCommand;
use crate::raft::message::NodeId;
use crate::raft::node::RaftNode;
use crate::raft::state::NodeRole;
use std::sync::Arc;

pub struct NexusEngine {
    pub storage: Arc<LsmEngine>,
    pub raft: Arc<RaftNode>,
}

impl NexusEngine {
    pub fn new(node_id: NodeId, peers: Vec<NodeId>, storage_config: LsmConfig) -> std::io::Result<Self> {
        let storage = Arc::new(LsmEngine::open(storage_config)?);
        let raft = Arc::new(RaftNode::new(node_id, peers));

        Ok(Self { storage, raft })
    }

    /// Process a parsed Redis RESP command
    pub fn execute_command(&self, cmd: &[RespValue]) -> RespValue {
        if cmd.is_empty() {
            return RespValue::Error("Empty command".to_string());
        }

        let cmd_name = match &cmd[0] {
            RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_uppercase(),
            RespValue::SimpleString(s) => s.to_uppercase(),
            _ => return RespValue::Error("Invalid command format".to_string()),
        };

        match cmd_name.as_str() {
            "PING" => RespValue::SimpleString("PONG".to_string()),

            "SET" => {
                if cmd.len() < 3 {
                    return RespValue::Error("ERR wrong number of arguments for 'set' command".to_string());
                }
                let key = match &cmd[1] {
                    RespValue::BulkString(Some(b)) => b.clone(),
                    _ => return RespValue::Error("Invalid key".to_string()),
                };
                let val = match &cmd[2] {
                    RespValue::BulkString(Some(b)) => b.clone(),
                    _ => return RespValue::Error("Invalid value".to_string()),
                };

                // Check Raft leadership
                let raft_cmd = RaftCommand::Put {
                    key: key.clone(),
                    value: val.clone(),
                };

                match self.raft.propose(raft_cmd) {
                    Ok(_log_idx) => {
                        // Apply to local LSM storage directly (in standalone/leader mode)
                        if let Err(e) = self.storage.put(&key, &val) {
                            return RespValue::Error(format!("Storage error: {}", e));
                        }
                        RespValue::SimpleString("OK".to_string())
                    }
                    Err(Some(leader_id)) => {
                        RespValue::Error(format!("MOVED to leader node {}", leader_id))
                    }
                    Err(None) => {
                        // Single-node mode or election in progress: apply directly if standalone
                        if self.raft.peers.is_empty() {
                            if let Err(e) = self.storage.put(&key, &val) {
                                return RespValue::Error(format!("Storage error: {}", e));
                            }
                            RespValue::SimpleString("OK".to_string())
                        } else {
                            RespValue::Error("Leader election in progress".to_string())
                        }
                    }
                }
            }

            "GET" => {
                if cmd.len() < 2 {
                    return RespValue::Error("ERR wrong number of arguments for 'get' command".to_string());
                }
                let key = match &cmd[1] {
                    RespValue::BulkString(Some(b)) => b,
                    _ => return RespValue::Error("Invalid key".to_string()),
                };

                match self.storage.get(key) {
                    Ok(Some(val)) => RespValue::BulkString(Some(val)),
                    Ok(None) => RespValue::BulkString(None), // Nil
                    Err(e) => RespValue::Error(format!("Storage error: {}", e)),
                }
            }

            "DEL" => {
                if cmd.len() < 2 {
                    return RespValue::Error("ERR wrong number of arguments for 'del' command".to_string());
                }
                let key = match &cmd[1] {
                    RespValue::BulkString(Some(b)) => b.clone(),
                    _ => return RespValue::Error("Invalid key".to_string()),
                };

                let raft_cmd = RaftCommand::Delete { key: key.clone() };
                match self.raft.propose(raft_cmd) {
                    Ok(_) => {
                        let _ = self.storage.delete(&key);
                        RespValue::Integer(1)
                    }
                    Err(Some(leader_id)) => {
                        RespValue::Error(format!("MOVED to leader node {}", leader_id))
                    }
                    Err(None) => {
                        if self.raft.peers.is_empty() {
                            let _ = self.storage.delete(&key);
                            RespValue::Integer(1)
                        } else {
                            RespValue::Error("Leader election in progress".to_string())
                        }
                    }
                }
            }

            "ROLE" => {
                let role = self.raft.state.read().role;
                let role_str = match role {
                    NodeRole::Leader => "master",
                    NodeRole::Follower => "slave",
                    NodeRole::Candidate => "candidate",
                };
                RespValue::SimpleString(role_str.to_string())
            }

            "FLUSHALL" => {
                let _ = self.storage.flush();
                RespValue::SimpleString("OK".to_string())
            }

            _ => RespValue::Error(format!("ERR unknown command '{}'", cmd_name)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_nexus_engine_redis_commands() {
        let dir = tempdir().unwrap();
        let config = LsmConfig {
            dir: dir.path().to_path_buf(),
            memtable_threshold_bytes: 1024,
            compaction_threshold_ssts: 4,
        };

        let engine = NexusEngine::new(1, vec![], config).unwrap();
        engine.raft.become_leader();

        // PING
        let ping_res = engine.execute_command(&[RespValue::BulkString(Some(b"PING".to_vec()))]);
        assert_eq!(ping_res, RespValue::SimpleString("PONG".to_string()));

        // SET key val
        let set_res = engine.execute_command(&[
            RespValue::BulkString(Some(b"SET".to_vec())),
            RespValue::BulkString(Some(b"framework".to_vec())),
            RespValue::BulkString(Some(b"NexusKV".to_vec())),
        ]);
        assert_eq!(set_res, RespValue::SimpleString("OK".to_string()));

        // GET key
        let get_res = engine.execute_command(&[
            RespValue::BulkString(Some(b"GET".to_vec())),
            RespValue::BulkString(Some(b"framework".to_vec())),
        ]);
        assert_eq!(get_res, RespValue::BulkString(Some(b"NexusKV".to_vec())));

        // DEL key
        let del_res = engine.execute_command(&[
            RespValue::BulkString(Some(b"DEL".to_vec())),
            RespValue::BulkString(Some(b"framework".to_vec())),
        ]);
        assert_eq!(del_res, RespValue::Integer(1));

        // GET after DEL -> Nil
        let get_after = engine.execute_command(&[
            RespValue::BulkString(Some(b"GET".to_vec())),
            RespValue::BulkString(Some(b"framework".to_vec())),
        ]);
        assert_eq!(get_after, RespValue::BulkString(None));
    }
}
