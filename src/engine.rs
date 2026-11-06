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
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Default, Clone)]
pub struct ClientSession {
    pub in_multi: bool,
    pub queue: Vec<Vec<RespValue>>,
    pub authenticated: bool,
}

pub struct NexusEngine {
    pub storage: Arc<LsmEngine>,
    pub raft: Arc<RaftNode>,
    pub expirations: Arc<RwLock<HashMap<Vec<u8>, Instant>>>,
    pub auth_password: Arc<RwLock<Option<String>>>,
    pub start_time: Instant,
}

impl NexusEngine {
    pub fn new(node_id: NodeId, peers: Vec<NodeId>, storage_config: LsmConfig) -> std::io::Result<Self> {
        let storage = Arc::new(LsmEngine::open(storage_config)?);
        let raft = Arc::new(RaftNode::new(node_id, peers));
        let expirations = Arc::new(RwLock::new(HashMap::new()));
        let auth_password = Arc::new(RwLock::new(None));
        let start_time = Instant::now();

        Ok(Self { storage, raft, expirations, auth_password, start_time })
    }

    /// Configure authentication password
    pub fn set_auth_password(&self, password: Option<String>) {
        *self.auth_password.write() = password;
    }

    /// Set an expiration timeout for a key in seconds
    pub fn set_expiration(&self, key: &[u8], ttl_seconds: u64) {
        let mut exp = self.expirations.write();
        exp.insert(key.to_vec(), Instant::now() + Duration::from_secs(ttl_seconds));
    }

    /// Get remaining TTL in seconds (-2 if non-existent, -1 if no expiration, >0 if active)
    pub fn get_ttl(&self, key: &[u8]) -> i64 {
        // First check if key exists in storage
        match self.storage.get(key) {
            Ok(Some(_)) => {
                let mut exp = self.expirations.write();
                if let Some(&expires_at) = exp.get(key) {
                    let now = Instant::now();
                    if now >= expires_at {
                        // Key has expired: delete it from storage and expiration registry
                        exp.remove(key);
                        let _ = self.storage.delete(key);
                        -2
                    } else {
                        let remaining = expires_at.duration_since(now).as_secs();
                        if remaining == 0 { 1 } else { remaining as i64 }
                    }
                } else {
                    -1 // Key exists with no expiration
                }
            }
            _ => -2, // Key does not exist
        }
    }

    /// Remove expiration from a key, making it persistent
    pub fn persist(&self, key: &[u8]) -> i64 {
        let mut exp = self.expirations.write();
        if exp.remove(key).is_some() {
            1
        } else {
            0
        }
    }

    /// Check and clean key if expired
    fn is_expired(&self, key: &[u8]) -> bool {
        let mut exp = self.expirations.write();
        if let Some(&expires_at) = exp.get(key) {
            if Instant::now() >= expires_at {
                exp.remove(key);
                let _ = self.storage.delete(key);
                return true;
            }
        }
        false
    }

    /// Process a parsed Redis RESP command with session state (for MULTI / EXEC / DISCARD)
    pub fn execute_with_session(&self, session: &mut ClientSession, cmd: &[RespValue]) -> RespValue {
        if cmd.is_empty() {
            return RespValue::Error("Empty command".to_string());
        }

        let cmd_name = match &cmd[0] {
            RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_uppercase(),
            RespValue::SimpleString(s) => s.to_uppercase(),
            _ => return RespValue::Error("Invalid command format".to_string()),
        };

        // Authentication enforcement
        let req_pass = self.auth_password.read().clone();
        if let Some(ref expected) = req_pass {
            if !session.authenticated {
                if cmd_name == "AUTH" {
                    if cmd.len() < 2 {
                        return RespValue::Error("ERR wrong number of arguments for 'auth' command".to_string());
                    }
                    let provided = match &cmd[1] {
                        RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                        RespValue::SimpleString(s) => s.clone(),
                        _ => return RespValue::Error("ERR invalid password format".to_string()),
                    };
                    if provided == *expected {
                        session.authenticated = true;
                        return RespValue::SimpleString("OK".to_string());
                    } else {
                        return RespValue::Error("ERR invalid password".to_string());
                    }
                } else if cmd_name != "PING" {
                    return RespValue::Error("NOAUTH Authentication required.".to_string());
                }
            }
        }

        match cmd_name.as_str() {
            "MULTI" => {
                if session.in_multi {
                    RespValue::Error("ERR MULTI calls can not be nested".to_string())
                } else {
                    session.in_multi = true;
                    session.queue.clear();
                    RespValue::SimpleString("OK".to_string())
                }
            }
            "DISCARD" => {
                if !session.in_multi {
                    RespValue::Error("ERR DISCARD without MULTI".to_string())
                } else {
                    session.in_multi = false;
                    session.queue.clear();
                    RespValue::SimpleString("OK".to_string())
                }
            }
            "EXEC" => {
                if !session.in_multi {
                    RespValue::Error("ERR EXEC without MULTI".to_string())
                } else {
                    session.in_multi = false;
                    let queue = std::mem::take(&mut session.queue);
                    let mut results = Vec::with_capacity(queue.len());
                    for queued_cmd in queue {
                        results.push(self.execute_command(&queued_cmd));
                    }
                    RespValue::Array(Some(results))
                }
            }
            _ => {
                if session.in_multi {
                    session.queue.push(cmd.to_vec());
                    RespValue::SimpleString("QUEUED".to_string())
                } else {
                    self.execute_command(cmd)
                }
            }
        }
    }

    /// Process a parsed Redis RESP command
    pub fn execute_command(&self, cmd: &[RespValue]) -> RespValue {
        if cmd.is_empty() {
            return RespValue::Error("Empty command".to_string());
        }

        crate::metrics::Metrics::inc_op();

        let cmd_name = match &cmd[0] {
            RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_uppercase(),
            RespValue::SimpleString(s) => s.to_uppercase(),
            _ => return RespValue::Error("Invalid command format".to_string()),
        };

        match cmd_name.as_str() {
            "PING" => RespValue::SimpleString("PONG".to_string()),

            "SET" => {
                crate::metrics::Metrics::inc_write();
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

                let mut ttl_secs: Option<u64> = None;
                if cmd.len() >= 5 {
                    let opt = match &cmd[3] {
                        RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_uppercase(),
                        RespValue::SimpleString(s) => s.to_uppercase(),
                        _ => String::new(),
                    };
                    if opt == "EX" {
                        if let RespValue::BulkString(Some(b)) = &cmd[4] {
                            if let Ok(s) = String::from_utf8_lossy(b).parse::<u64>() {
                                ttl_secs = Some(s);
                            }
                        }
                    }
                }

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
                        if let Some(s) = ttl_secs {
                            self.set_expiration(&key, s);
                        } else {
                            self.persist(&key);
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
                            if let Some(s) = ttl_secs {
                                self.set_expiration(&key, s);
                            } else {
                                self.persist(&key);
                            }
                            RespValue::SimpleString("OK".to_string())
                        } else {
                            RespValue::Error("Leader election in progress".to_string())
                        }
                    }
                }
            }

            "GET" => {
                crate::metrics::Metrics::inc_read();
                if cmd.len() < 2 {
                    return RespValue::Error("ERR wrong number of arguments for 'get' command".to_string());
                }
                let key = match &cmd[1] {
                    RespValue::BulkString(Some(b)) => b,
                    _ => return RespValue::Error("Invalid key".to_string()),
                };

                if self.is_expired(key) {
                    return RespValue::BulkString(None);
                }

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

                self.persist(&key);

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

            "EXPIRE" => {
                if cmd.len() < 3 {
                    return RespValue::Error("ERR wrong number of arguments for 'expire' command".to_string());
                }
                let key = match &cmd[1] {
                    RespValue::BulkString(Some(b)) => b,
                    _ => return RespValue::Error("Invalid key".to_string()),
                };
                let secs = match &cmd[2] {
                    RespValue::BulkString(Some(b)) => match String::from_utf8_lossy(b).parse::<u64>() {
                        Ok(s) => s,
                        Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
                    },
                    RespValue::Integer(i) => *i as u64,
                    _ => return RespValue::Error("Invalid expiration".to_string()),
                };

                if self.storage.get(key).ok().flatten().is_none() {
                    RespValue::Integer(0)
                } else {
                    self.set_expiration(key, secs);
                    RespValue::Integer(1)
                }
            }

            "TTL" => {
                if cmd.len() < 2 {
                    return RespValue::Error("ERR wrong number of arguments for 'ttl' command".to_string());
                }
                let key = match &cmd[1] {
                    RespValue::BulkString(Some(b)) => b,
                    _ => return RespValue::Error("Invalid key".to_string()),
                };
                let ttl = self.get_ttl(key);
                RespValue::Integer(ttl)
            }

            "PERSIST" => {
                if cmd.len() < 2 {
                    return RespValue::Error("ERR wrong number of arguments for 'persist' command".to_string());
                }
                let key = match &cmd[1] {
                    RespValue::BulkString(Some(b)) => b,
                    _ => return RespValue::Error("Invalid key".to_string()),
                };
                let res = self.persist(key);
                RespValue::Integer(res)
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

            "KEYS" => {
                let pattern = if cmd.len() > 1 {
                    match &cmd[1] {
                        RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                        _ => "*".to_string(),
                    }
                } else {
                    "*".to_string()
                };

                match self.storage.keys(&pattern) {
                    Ok(mut matched_keys) => {
                        matched_keys.retain(|k| {
                            let s = String::from_utf8_lossy(k);
                            !self.is_expired(k) && (!s.starts_with("__set__:") || pattern.starts_with("__set__:"))
                        });
                        let elements = matched_keys.into_iter().map(|k| RespValue::BulkString(Some(k))).collect();
                        RespValue::Array(Some(elements))
                    }
                    Err(e) => RespValue::Error(format!("Storage error: {}", e)),
                }
            }

            "SCAN" => {
                let pattern = if cmd.len() > 2 {
                    match &cmd[2] {
                        RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                        _ => "*".to_string(),
                    }
                } else {
                    "*".to_string()
                };

                match self.storage.keys(&pattern) {
                    Ok(mut matched_keys) => {
                        matched_keys.retain(|k| {
                            let s = String::from_utf8_lossy(k);
                            !self.is_expired(k) && (!s.starts_with("__set__:") || pattern.starts_with("__set__:"))
                        });
                        let elements = matched_keys.into_iter().map(|k| RespValue::BulkString(Some(k))).collect();
                        // Standard Redis SCAN format: [cursor, [keys...]]
                        RespValue::Array(Some(vec![
                            RespValue::BulkString(Some(b"0".to_vec())), // 0 cursor indicates end of scan
                            RespValue::Array(Some(elements)),
                        ]))
                    }
                    Err(e) => RespValue::Error(format!("Storage error: {}", e)),
                }
            }

            "MULTI" => RespValue::SimpleString("OK".to_string()),
            "EXEC" => RespValue::Array(Some(vec![])),
            "DISCARD" => RespValue::SimpleString("OK".to_string()),

            "AUTH" => {
                let req_pass = self.auth_password.read().clone();
                if let Some(ref expected) = req_pass {
                    if cmd.len() < 2 {
                        RespValue::Error("ERR wrong number of arguments for 'auth' command".to_string())
                    } else {
                        let provided = match &cmd[1] {
                            RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                            RespValue::SimpleString(s) => s.clone(),
                            _ => return RespValue::Error("ERR invalid password format".to_string()),
                        };
                        if provided == *expected {
                            RespValue::SimpleString("OK".to_string())
                        } else {
                            RespValue::Error("ERR invalid password".to_string())
                        }
                    }
                } else {
                    RespValue::SimpleString("OK".to_string())
                }
            }

            "INFO" => {
                let uptime = self.start_time.elapsed().as_secs();
                let term = self.raft.state.read().current_term;
                let commit_idx = self.raft.state.read().commit_index;
                let role = self.raft.state.read().role;
                let role_str = match role {
                    NodeRole::Leader => "master",
                    NodeRole::Follower => "slave",
                    NodeRole::Candidate => "candidate",
                };
                let mem_bytes = self.storage.memtable.size();
                let sst_count = self.storage.sstables.read().len();
                let total_ops = crate::metrics::Metrics::total_ops();
                let write_ops = crate::metrics::Metrics::write_ops();
                let read_ops = crate::metrics::Metrics::read_ops();

                let mut s = String::new();
                s.push_str("# Server\r\n");
                s.push_str("nexus_version:0.1.0\r\n");
                s.push_str("nexus_mode:distributed-raft\r\n");
                s.push_str(&format!("node_id:{}\r\n", self.raft.id));
                s.push_str("os:windows\r\n");
                s.push_str("arch:x86_64\r\n");
                s.push_str(&format!("uptime_in_seconds:{}\r\n", uptime));
                s.push_str("\r\n# Clients\r\n");
                s.push_str("connected_clients:1\r\n");
                s.push_str("\r\n# Memory\r\n");
                s.push_str(&format!("memtable_bytes:{}\r\n", mem_bytes));
                s.push_str("\r\n# Persistence\r\n");
                s.push_str("wal_status:active\r\n");
                s.push_str(&format!("sstable_count:{}\r\n", sst_count));
                s.push_str("\r\n# Replication\r\n");
                s.push_str(&format!("role:{}\r\n", role_str));
                s.push_str(&format!("connected_slaves:{}\r\n", self.raft.peers.len()));
                s.push_str(&format!("raft_current_term:{}\r\n", term));
                s.push_str(&format!("raft_commit_index:{}\r\n", commit_idx));
                s.push_str("\r\n# Stats\r\n");
                s.push_str(&format!("total_commands_processed:{}\r\n", total_ops));
                s.push_str(&format!("total_writes:{}\r\n", write_ops));
                s.push_str(&format!("total_reads:{}\r\n", read_ops));

                RespValue::BulkString(Some(s.into_bytes()))
            }

            // Secondary Indexing & Set Operations
            "SADD" => {
                if cmd.len() < 3 {
                    return RespValue::Error("ERR wrong number of arguments for 'sadd' command".to_string());
                }
                let key_str = match &cmd[1] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                    _ => return RespValue::Error("Invalid key".to_string()),
                };

                let mut added = 0;
                for member_val in &cmd[2..] {
                    let mem_str = match member_val {
                        RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                        _ => continue,
                    };
                    let internal_key = format!("__set__:{}:{}", key_str, mem_str).into_bytes();
                    if self.storage.get(&internal_key).ok().flatten().is_none() {
                        let _ = self.storage.put(&internal_key, b"1");
                        added += 1;
                    }
                }
                RespValue::Integer(added)
            }

            "SMEMBERS" => {
                if cmd.len() < 2 {
                    return RespValue::Error("ERR wrong number of arguments for 'smembers' command".to_string());
                }
                let key_str = match &cmd[1] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                    _ => return RespValue::Error("Invalid key".to_string()),
                };
                let prefix = format!("__set__:{}:", key_str);
                let pattern = format!("{}*", prefix);
                match self.storage.keys(&pattern) {
                    Ok(matched) => {
                        let members: Vec<RespValue> = matched.into_iter().filter_map(|k| {
                            let s = String::from_utf8_lossy(&k);
                            if s.starts_with(&prefix) {
                                Some(RespValue::BulkString(Some(s[prefix.len()..].as_bytes().to_vec())))
                            } else {
                                None
                            }
                        }).collect();
                        RespValue::Array(Some(members))
                    }
                    Err(e) => RespValue::Error(format!("Storage error: {}", e)),
                }
            }

            "SREM" => {
                if cmd.len() < 3 {
                    return RespValue::Error("ERR wrong number of arguments for 'srem' command".to_string());
                }
                let key_str = match &cmd[1] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                    _ => return RespValue::Error("Invalid key".to_string()),
                };
                let mut removed = 0;
                for member_val in &cmd[2..] {
                    let mem_str = match member_val {
                        RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                        _ => continue,
                    };
                    let internal_key = format!("__set__:{}:{}", key_str, mem_str).into_bytes();
                    if self.storage.get(&internal_key).ok().flatten().is_some() {
                        let _ = self.storage.delete(&internal_key);
                        removed += 1;
                    }
                }
                RespValue::Integer(removed)
            }

            "SISMEMBER" => {
                if cmd.len() < 3 {
                    return RespValue::Error("ERR wrong number of arguments for 'sismember' command".to_string());
                }
                let key_str = match &cmd[1] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                    _ => return RespValue::Error("Invalid key".to_string()),
                };
                let mem_str = match &cmd[2] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                    _ => return RespValue::Error("Invalid member".to_string()),
                };
                let internal_key = format!("__set__:{}:{}", key_str, mem_str).into_bytes();
                let exists = self.storage.get(&internal_key).ok().flatten().is_some();
                RespValue::Integer(if exists { 1 } else { 0 })
            }

            "SCARD" => {
                if cmd.len() < 2 {
                    return RespValue::Error("ERR wrong number of arguments for 'scard' command".to_string());
                }
                let key_str = match &cmd[1] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                    _ => return RespValue::Error("Invalid key".to_string()),
                };
                let prefix = format!("__set__:{}:", key_str);
                let pattern = format!("{}*", prefix);
                let count = self.storage.keys(&pattern).map(|k| k.len()).unwrap_or(0);
                RespValue::Integer(count as i64)
            }

            "METRICS" => {
                let term = self.raft.state.read().current_term;
                let mem_bytes = self.storage.memtable.size();
                let sst_count = self.storage.sstables.read().len();
                let raw_metrics = crate::metrics::Metrics::render_prometheus(self.raft.id, term, mem_bytes, sst_count);
                RespValue::BulkString(Some(raw_metrics.into_bytes()))
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

        // Test EXPIRE & TTL
        engine.execute_command(&[
            RespValue::BulkString(Some(b"SET".to_vec())),
            RespValue::BulkString(Some(b"temp_key".to_vec())),
            RespValue::BulkString(Some(b"temp_val".to_vec())),
        ]);
        let ttl_init = engine.execute_command(&[
            RespValue::BulkString(Some(b"TTL".to_vec())),
            RespValue::BulkString(Some(b"temp_key".to_vec())),
        ]);
        assert_eq!(ttl_init, RespValue::Integer(-1)); // No expiration set

        let exp_res = engine.execute_command(&[
            RespValue::BulkString(Some(b"EXPIRE".to_vec())),
            RespValue::BulkString(Some(b"temp_key".to_vec())),
            RespValue::BulkString(Some(b"60".to_vec())),
        ]);
        assert_eq!(exp_res, RespValue::Integer(1));

        let ttl_after = engine.execute_command(&[
            RespValue::BulkString(Some(b"TTL".to_vec())),
            RespValue::BulkString(Some(b"temp_key".to_vec())),
        ]);
        if let RespValue::Integer(sec) = ttl_after {
            assert!(sec > 0 && sec <= 60);
        } else {
            panic!("Expected Integer TTL");
        }

        // Test PERSIST
        let persist_res = engine.execute_command(&[
            RespValue::BulkString(Some(b"PERSIST".to_vec())),
            RespValue::BulkString(Some(b"temp_key".to_vec())),
        ]);
        assert_eq!(persist_res, RespValue::Integer(1));

        // Test SET with EX
        engine.execute_command(&[
            RespValue::BulkString(Some(b"SET".to_vec())),
            RespValue::BulkString(Some(b"exp_key".to_vec())),
            RespValue::BulkString(Some(b"exp_val".to_vec())),
            RespValue::BulkString(Some(b"EX".to_vec())),
            RespValue::BulkString(Some(b"100".to_vec())),
        ]);
        let ttl_ex = engine.execute_command(&[
            RespValue::BulkString(Some(b"TTL".to_vec())),
            RespValue::BulkString(Some(b"exp_key".to_vec())),
        ]);
        if let RespValue::Integer(sec) = ttl_ex {
            assert!(sec > 0 && sec <= 100);
        } else {
            panic!("Expected Integer TTL for SET EX");
        }

        // Test MULTI / EXEC / DISCARD Session
        let mut session = ClientSession::default();
        let m_res = engine.execute_with_session(&mut session, &[RespValue::BulkString(Some(b"MULTI".to_vec()))]);
        assert_eq!(m_res, RespValue::SimpleString("OK".to_string()));

        let q1 = engine.execute_with_session(&mut session, &[
            RespValue::BulkString(Some(b"SET".to_vec())),
            RespValue::BulkString(Some(b"tx_k1".to_vec())),
            RespValue::BulkString(Some(b"tx_v1".to_vec())),
        ]);
        assert_eq!(q1, RespValue::SimpleString("QUEUED".to_string()));

        let q2 = engine.execute_with_session(&mut session, &[
            RespValue::BulkString(Some(b"GET".to_vec())),
            RespValue::BulkString(Some(b"tx_k1".to_vec())),
        ]);
        assert_eq!(q2, RespValue::SimpleString("QUEUED".to_string()));

        let exec_res = engine.execute_with_session(&mut session, &[RespValue::BulkString(Some(b"EXEC".to_vec()))]);
        assert_eq!(
            exec_res,
            RespValue::Array(Some(vec![
                RespValue::SimpleString("OK".to_string()),
                RespValue::BulkString(Some(b"tx_v1".to_vec()))
            ]))
        );

        // Test INFO command
        let info_res = engine.execute_command(&[RespValue::BulkString(Some(b"INFO".to_vec()))]);
        if let RespValue::BulkString(Some(b)) = info_res {
            let s = String::from_utf8_lossy(&b);
            assert!(s.contains("nexus_version:0.1.0"));
            assert!(s.contains("# Replication"));
            assert!(s.contains("role:master"));
        } else {
            panic!("Expected BulkString for INFO");
        }

        // Test Sets & Secondary Indexing (SADD / SMEMBERS / SREM / SCARD / SISMEMBER)
        let sadd_res = engine.execute_command(&[
            RespValue::BulkString(Some(b"SADD".to_vec())),
            RespValue::BulkString(Some(b"idx:tags".to_vec())),
            RespValue::BulkString(Some(b"rust".to_vec())),
            RespValue::BulkString(Some(b"database".to_vec())),
            RespValue::BulkString(Some(b"raft".to_vec())),
        ]);
        assert_eq!(sadd_res, RespValue::Integer(3));

        let sismember = engine.execute_command(&[
            RespValue::BulkString(Some(b"SISMEMBER".to_vec())),
            RespValue::BulkString(Some(b"idx:tags".to_vec())),
            RespValue::BulkString(Some(b"rust".to_vec())),
        ]);
        assert_eq!(sismember, RespValue::Integer(1));

        let scard = engine.execute_command(&[
            RespValue::BulkString(Some(b"SCARD".to_vec())),
            RespValue::BulkString(Some(b"idx:tags".to_vec())),
        ]);
        assert_eq!(scard, RespValue::Integer(3));

        let smembers = engine.execute_command(&[
            RespValue::BulkString(Some(b"SMEMBERS".to_vec())),
            RespValue::BulkString(Some(b"idx:tags".to_vec())),
        ]);
        if let RespValue::Array(Some(mems)) = smembers {
            assert_eq!(mems.len(), 3);
        } else {
            panic!("Expected Array for SMEMBERS");
        }

        let srem = engine.execute_command(&[
            RespValue::BulkString(Some(b"SREM".to_vec())),
            RespValue::BulkString(Some(b"idx:tags".to_vec())),
            RespValue::BulkString(Some(b"database".to_vec())),
        ]);
        assert_eq!(srem, RespValue::Integer(1));

        let scard_after = engine.execute_command(&[
            RespValue::BulkString(Some(b"SCARD".to_vec())),
            RespValue::BulkString(Some(b"idx:tags".to_vec())),
        ]);
        assert_eq!(scard_after, RespValue::Integer(2));

        // Test AUTH Protection
        engine.set_auth_password(Some("secret123".to_string()));
        let mut unauth_session = ClientSession::default();

        // Query before auth should fail with NOAUTH
        let unauth_get = engine.execute_with_session(&mut unauth_session, &[
            RespValue::BulkString(Some(b"GET".to_vec())),
            RespValue::BulkString(Some(b"k1".to_vec())),
        ]);
        assert_eq!(unauth_get, RespValue::Error("NOAUTH Authentication required.".to_string()));

        // Wrong password should fail
        let wrong_auth = engine.execute_with_session(&mut unauth_session, &[
            RespValue::BulkString(Some(b"AUTH".to_vec())),
            RespValue::BulkString(Some(b"wrong".to_vec())),
        ]);
        assert_eq!(wrong_auth, RespValue::Error("ERR invalid password".to_string()));

        // Correct password succeeds
        let right_auth = engine.execute_with_session(&mut unauth_session, &[
            RespValue::BulkString(Some(b"AUTH".to_vec())),
            RespValue::BulkString(Some(b"secret123".to_vec())),
        ]);
        assert_eq!(right_auth, RespValue::SimpleString("OK".to_string()));

        // Now queries succeed
        let auth_get = engine.execute_with_session(&mut unauth_session, &[
            RespValue::BulkString(Some(b"PING".to_vec())),
        ]);
        assert_eq!(auth_get, RespValue::SimpleString("PONG".to_string()));
    }
}
