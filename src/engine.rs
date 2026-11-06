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
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;

#[derive(Default, Clone)]
pub struct ClientSession {
    pub in_multi: bool,
    pub queue: Vec<Vec<RespValue>>,
    pub authenticated: bool,
    pub db: u8,
}

pub struct NexusEngine {
    pub storage: Arc<LsmEngine>,
    pub raft: Arc<RaftNode>,
    pub expirations: Arc<RwLock<HashMap<Vec<u8>, Instant>>>,
    pub auth_password: Arc<RwLock<Option<String>>>,
    pub start_time: Instant,
    pub pubsub: Arc<RwLock<HashMap<String, broadcast::Sender<Vec<u8>>>>>,
    pub last_save_time: Arc<AtomicU64>,
}

impl NexusEngine {
    pub fn new(node_id: NodeId, peers: Vec<NodeId>, storage_config: LsmConfig) -> std::io::Result<Self> {
        let storage = Arc::new(LsmEngine::open(storage_config)?);
        let raft = Arc::new(RaftNode::new(node_id, peers));
        let expirations = Arc::new(RwLock::new(HashMap::new()));
        let auth_password = Arc::new(RwLock::new(None));
        let start_time = Instant::now();
        let pubsub = Arc::new(RwLock::new(HashMap::new()));
        let last_save_time = Arc::new(AtomicU64::new(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        ));

        Ok(Self {
            storage,
            raft,
            expirations,
            auth_password,
            start_time,
            pubsub,
            last_save_time,
        })
    }

    /// Get or register a pubsub broadcast channel
    pub fn get_or_create_channel(&self, channel: &str) -> broadcast::Sender<Vec<u8>> {
        let mut ps = self.pubsub.write();
        if let Some(sender) = ps.get(channel) {
            sender.clone()
        } else {
            let (tx, _rx) = broadcast::channel(1024);
            ps.insert(channel.to_string(), tx.clone());
            tx
        }
    }

    /// Publish message to all active subscribers of a channel
    pub fn publish_message(&self, channel: &str, message: &[u8]) -> usize {
        let ps = self.pubsub.read();
        if let Some(sender) = ps.get(channel) {
            let count = sender.receiver_count();
            let _ = sender.send(message.to_vec());
            count
        } else {
            0
        }
    }

    // Helper for List operations (stored in LSM engine)
    pub fn get_list(&self, key: &str) -> Vec<Vec<u8>> {
        let internal_key = format!("__list__:{}", key).into_bytes();
        match self.storage.get(&internal_key) {
            Ok(Some(raw)) => bincode::deserialize(&raw).unwrap_or_default(),
            _ => Vec::new(),
        }
    }

    pub fn save_list(&self, key: &str, list: &[Vec<u8>]) -> std::io::Result<()> {
        let internal_key = format!("__list__:{}", key).into_bytes();
        if list.is_empty() {
            self.storage.delete(&internal_key)
        } else {
            let raw = bincode::serialize(list).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            self.storage.put(&internal_key, &raw)
        }
    }

    /// Perform a point-in-time snapshot dump of the database (SAVE / BGSAVE)
    pub fn save_dump(&self) -> std::io::Result<usize> {
        let _ = self.storage.flush();
        let dump_path = self.storage.config.dir.join("nexus.dump");

        let all_keys = self.storage.keys("*")?;
        let mut dump_entries: Vec<(Vec<u8>, Vec<u8>)> = Vec::with_capacity(all_keys.len());
        for k in all_keys {
            if !self.is_expired(&k) {
                if let Ok(Some(v)) = self.storage.get(&k) {
                    dump_entries.push((k, v));
                }
            }
        }

        let count = dump_entries.len();
        let serialized = bincode::serialize(&dump_entries)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(&dump_path, serialized)?;

        self.last_save_time.store(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            Ordering::Relaxed,
        );
        Ok(count)
    }

    /// Restore database state from a point-in-time snapshot dump
    pub fn restore_dump(&self, path: Option<&Path>) -> std::io::Result<usize> {
        let dump_path = match path {
            Some(p) => p.to_path_buf(),
            None => self.storage.config.dir.join("nexus.dump"),
        };

        if !dump_path.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Dump file not found at {}", dump_path.display()),
            ));
        }

        let raw = std::fs::read(&dump_path)?;
        let dump_entries: Vec<(Vec<u8>, Vec<u8>)> = bincode::deserialize(&raw)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        let count = dump_entries.len();
        for (k, v) in dump_entries {
            self.storage.put(&k, &v)?;
        }
        self.storage.flush()?;
        Ok(count)
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
            "SELECT" => {
                if cmd.len() < 2 {
                    return RespValue::Error("ERR wrong number of arguments for 'select' command".to_string());
                }
                let db_idx = match &cmd[1] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).parse::<u8>().unwrap_or(0),
                    RespValue::Integer(i) => *i as u8,
                    _ => 0,
                };
                if db_idx > 15 {
                    return RespValue::Error("ERR DB index is out of range".to_string());
                }
                session.db = db_idx;
                RespValue::SimpleString("OK".to_string())
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
                        if self.raft.peers.read().is_empty() {
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
                        if self.raft.peers.read().is_empty() {
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
                            let is_internal = s.starts_with("__set__:") || s.starts_with("__list__:") || s.starts_with("__hash__:") || s.starts_with("__zset_score__:") || s.starts_with("__geo__:") || s.starts_with("__ratelimit__:") || s.starts_with("__hll__:") || s.starts_with("__stream_") || s.starts_with("__db");
                            !self.is_expired(k) && (!is_internal || pattern.starts_with("__"))
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
                            let is_internal = s.starts_with("__set__:") || s.starts_with("__list__:") || s.starts_with("__hash__:") || s.starts_with("__zset_score__:") || s.starts_with("__geo__:") || s.starts_with("__ratelimit__:") || s.starts_with("__hll__:") || s.starts_with("__stream_") || s.starts_with("__db");
                            !self.is_expired(k) && (!is_internal || pattern.starts_with("__"))
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
                s.push_str(&format!("connected_slaves:{}\r\n", self.raft.peers.read().len()));
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

            // Pub/Sub Message Broker Engine
            "PUBLISH" => {
                if cmd.len() < 3 {
                    return RespValue::Error("ERR wrong number of arguments for 'publish' command".to_string());
                }
                let channel = match &cmd[1] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                    _ => return RespValue::Error("Invalid channel".to_string()),
                };
                let msg = match &cmd[2] {
                    RespValue::BulkString(Some(b)) => b.clone(),
                    _ => return RespValue::Error("Invalid message".to_string()),
                };

                let receivers = self.publish_message(&channel, &msg);
                RespValue::Integer(receivers as i64)
            }

            // Lists & FIFO Task Queues Engine
            "LPUSH" => {
                if cmd.len() < 3 {
                    return RespValue::Error("ERR wrong number of arguments for 'lpush' command".to_string());
                }
                let key_str = match &cmd[1] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                    _ => return RespValue::Error("Invalid key".to_string()),
                };
                let mut list = self.get_list(&key_str);
                for v in &cmd[2..] {
                    if let RespValue::BulkString(Some(b)) = v {
                        list.insert(0, b.clone());
                    }
                }
                let len = list.len();
                let _ = self.save_list(&key_str, &list);
                RespValue::Integer(len as i64)
            }

            "RPUSH" => {
                if cmd.len() < 3 {
                    return RespValue::Error("ERR wrong number of arguments for 'rpush' command".to_string());
                }
                let key_str = match &cmd[1] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                    _ => return RespValue::Error("Invalid key".to_string()),
                };
                let mut list = self.get_list(&key_str);
                for v in &cmd[2..] {
                    if let RespValue::BulkString(Some(b)) = v {
                        list.push(b.clone());
                    }
                }
                let len = list.len();
                let _ = self.save_list(&key_str, &list);
                RespValue::Integer(len as i64)
            }

            "LPOP" => {
                if cmd.len() < 2 {
                    return RespValue::Error("ERR wrong number of arguments for 'lpop' command".to_string());
                }
                let key_str = match &cmd[1] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                    _ => return RespValue::Error("Invalid key".to_string()),
                };
                let mut list = self.get_list(&key_str);
                if list.is_empty() {
                    RespValue::BulkString(None)
                } else {
                    let popped = list.remove(0);
                    let _ = self.save_list(&key_str, &list);
                    RespValue::BulkString(Some(popped))
                }
            }

            "RPOP" => {
                if cmd.len() < 2 {
                    return RespValue::Error("ERR wrong number of arguments for 'rpop' command".to_string());
                }
                let key_str = match &cmd[1] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                    _ => return RespValue::Error("Invalid key".to_string()),
                };
                let mut list = self.get_list(&key_str);
                if list.is_empty() {
                    RespValue::BulkString(None)
                } else {
                    let popped = list.pop().unwrap();
                    let _ = self.save_list(&key_str, &list);
                    RespValue::BulkString(Some(popped))
                }
            }

            "LLEN" => {
                if cmd.len() < 2 {
                    return RespValue::Error("ERR wrong number of arguments for 'llen' command".to_string());
                }
                let key_str = match &cmd[1] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                    _ => return RespValue::Error("Invalid key".to_string()),
                };
                let list = self.get_list(&key_str);
                RespValue::Integer(list.len() as i64)
            }

            "LRANGE" => {
                if cmd.len() < 4 {
                    return RespValue::Error("ERR wrong number of arguments for 'lrange' command".to_string());
                }
                let key_str = match &cmd[1] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                    _ => return RespValue::Error("Invalid key".to_string()),
                };
                let start_idx = match &cmd[2] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).parse::<i64>().unwrap_or(0),
                    RespValue::Integer(i) => *i,
                    _ => 0,
                };
                let stop_idx = match &cmd[3] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).parse::<i64>().unwrap_or(-1),
                    RespValue::Integer(i) => *i,
                    _ => -1,
                };

                let list = self.get_list(&key_str);
                let len = list.len() as i64;
                if len == 0 {
                    return RespValue::Array(Some(vec![]));
                }

                let s = if start_idx < 0 { (len + start_idx).max(0) } else { start_idx };
                let mut e = if stop_idx < 0 { (len + stop_idx).max(0) } else { stop_idx };

                if s >= len || s > e {
                    return RespValue::Array(Some(vec![]));
                }
                e = e.min(len - 1);

                let slice: Vec<RespValue> = list[s as usize..=e as usize]
                    .iter()
                    .map(|v| RespValue::BulkString(Some(v.clone())))
                    .collect();
                RespValue::Array(Some(slice))
            }

            // Hashes Data Structure Engine (HSET, HGET, HGETALL, HDEL, HLEN, HEXISTS)
            "HSET" => {
                if cmd.len() < 4 || (cmd.len() - 2) % 2 != 0 {
                    return RespValue::Error("ERR wrong number of arguments for 'hset' command".to_string());
                }
                let key_str = match &cmd[1] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                    _ => return RespValue::Error("Invalid key".to_string()),
                };

                let mut new_fields = 0;
                let mut i = 2;
                while i < cmd.len() {
                    let field = match &cmd[i] {
                        RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                        _ => return RespValue::Error("Invalid field".to_string()),
                    };
                    let val = match &cmd[i + 1] {
                        RespValue::BulkString(Some(b)) => b.clone(),
                        _ => return RespValue::Error("Invalid value".to_string()),
                    };

                    let internal_key = format!("__hash__:{}:{}", key_str, field).into_bytes();
                    if let Ok(None) = self.storage.get(&internal_key) {
                        new_fields += 1;
                    }
                    let _ = self.storage.put(&internal_key, &val);
                    i += 2;
                }
                RespValue::Integer(new_fields as i64)
            }

            "HGET" => {
                if cmd.len() < 3 {
                    return RespValue::Error("ERR wrong number of arguments for 'hget' command".to_string());
                }
                let key_str = match &cmd[1] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                    _ => return RespValue::Error("Invalid key".to_string()),
                };
                let field = match &cmd[2] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                    _ => return RespValue::Error("Invalid field".to_string()),
                };

                let internal_key = format!("__hash__:{}:{}", key_str, field).into_bytes();
                match self.storage.get(&internal_key) {
                    Ok(Some(v)) => RespValue::BulkString(Some(v)),
                    _ => RespValue::BulkString(None),
                }
            }

            "HGETALL" => {
                if cmd.len() < 2 {
                    return RespValue::Error("ERR wrong number of arguments for 'hgetall' command".to_string());
                }
                let key_str = match &cmd[1] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                    _ => return RespValue::Error("Invalid key".to_string()),
                };

                let prefix = format!("__hash__:{}:", key_str);
                let pattern = format!("{}*", prefix);
                match self.storage.keys(&pattern) {
                    Ok(keys) => {
                        let mut pairs = Vec::with_capacity(keys.len() * 2);
                        for k in keys {
                            if let Ok(Some(v)) = self.storage.get(&k) {
                                let k_str = String::from_utf8_lossy(&k);
                                if let Some(field) = k_str.strip_prefix(&prefix) {
                                    pairs.push(RespValue::BulkString(Some(field.as_bytes().to_vec())));
                                    pairs.push(RespValue::BulkString(Some(v)));
                                }
                            }
                        }
                        RespValue::Array(Some(pairs))
                    }
                    Err(e) => RespValue::Error(format!("Storage error: {}", e)),
                }
            }

            "HDEL" => {
                if cmd.len() < 3 {
                    return RespValue::Error("ERR wrong number of arguments for 'hdel' command".to_string());
                }
                let key_str = match &cmd[1] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                    _ => return RespValue::Error("Invalid key".to_string()),
                };

                let mut deleted_count = 0;
                for f_val in &cmd[2..] {
                    let field = match f_val {
                        RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                        _ => continue,
                    };
                    let internal_key = format!("__hash__:{}:{}", key_str, field).into_bytes();
                    if let Ok(Some(_)) = self.storage.get(&internal_key) {
                        let _ = self.storage.delete(&internal_key);
                        deleted_count += 1;
                    }
                }
                RespValue::Integer(deleted_count as i64)
            }

            "HLEN" => {
                if cmd.len() < 2 {
                    return RespValue::Error("ERR wrong number of arguments for 'hlen' command".to_string());
                }
                let key_str = match &cmd[1] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                    _ => return RespValue::Error("Invalid key".to_string()),
                };
                let prefix = format!("__hash__:{}:", key_str);
                let pattern = format!("{}*", prefix);
                let count = self.storage.keys(&pattern).map(|k| k.len()).unwrap_or(0);
                RespValue::Integer(count as i64)
            }

            "HEXISTS" => {
                if cmd.len() < 3 {
                    return RespValue::Error("ERR wrong number of arguments for 'hexists' command".to_string());
                }
                let key_str = match &cmd[1] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                    _ => return RespValue::Error("Invalid key".to_string()),
                };
                let field = match &cmd[2] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                    _ => return RespValue::Error("Invalid field".to_string()),
                };
                let internal_key = format!("__hash__:{}:{}", key_str, field).into_bytes();
                let exists = self.storage.get(&internal_key).map(|opt| opt.is_some()).unwrap_or(false);
                RespValue::Integer(if exists { 1 } else { 0 })
            }

            // Sorted Sets Data Structure Engine (ZADD, ZRANGE, ZSCORE, ZCARD, ZREM)
            "ZADD" => {
                if cmd.len() < 4 || (cmd.len() - 2) % 2 != 0 {
                    return RespValue::Error("ERR wrong number of arguments for 'zadd' command".to_string());
                }
                let key_str = match &cmd[1] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                    _ => return RespValue::Error("Invalid key".to_string()),
                };

                let mut new_members = 0;
                let mut i = 2;
                while i < cmd.len() {
                    let score_val = match &cmd[i] {
                        RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).parse::<f64>().unwrap_or(0.0),
                        _ => 0.0,
                    };
                    let member_bytes = match &cmd[i + 1] {
                        RespValue::BulkString(Some(b)) => b.clone(),
                        _ => return RespValue::Error("Invalid member".to_string()),
                    };

                    let member_str = String::from_utf8_lossy(&member_bytes);
                    let internal_key = format!("__zset_score__:{}:{}", key_str, member_str).into_bytes();
                    if let Ok(None) = self.storage.get(&internal_key) {
                        new_members += 1;
                    }
                    let _ = self.storage.put(&internal_key, score_val.to_string().as_bytes());
                    i += 2;
                }
                RespValue::Integer(new_members as i64)
            }

            "ZSCORE" => {
                if cmd.len() < 3 {
                    return RespValue::Error("ERR wrong number of arguments for 'zscore' command".to_string());
                }
                let key_str = match &cmd[1] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                    _ => return RespValue::Error("Invalid key".to_string()),
                };
                let member = match &cmd[2] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                    _ => return RespValue::Error("Invalid member".to_string()),
                };
                let internal_key = format!("__zset_score__:{}:{}", key_str, member).into_bytes();
                match self.storage.get(&internal_key) {
                    Ok(Some(v)) => RespValue::BulkString(Some(v)),
                    _ => RespValue::BulkString(None),
                }
            }

            "ZCARD" => {
                if cmd.len() < 2 {
                    return RespValue::Error("ERR wrong number of arguments for 'zcard' command".to_string());
                }
                let key_str = match &cmd[1] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                    _ => return RespValue::Error("Invalid key".to_string()),
                };
                let prefix = format!("__zset_score__:{}:", key_str);
                let pattern = format!("{}*", prefix);
                let count = self.storage.keys(&pattern).map(|k| k.len()).unwrap_or(0);
                RespValue::Integer(count as i64)
            }

            "ZREM" => {
                if cmd.len() < 3 {
                    return RespValue::Error("ERR wrong number of arguments for 'zrem' command".to_string());
                }
                let key_str = match &cmd[1] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                    _ => return RespValue::Error("Invalid key".to_string()),
                };
                let mut removed = 0;
                for m_val in &cmd[2..] {
                    let member = match m_val {
                        RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                        _ => continue,
                    };
                    let internal_key = format!("__zset_score__:{}:{}", key_str, member).into_bytes();
                    if let Ok(Some(_)) = self.storage.get(&internal_key) {
                        let _ = self.storage.delete(&internal_key);
                        removed += 1;
                    }
                }
                RespValue::Integer(removed as i64)
            }

            "ZRANGE" => {
                if cmd.len() < 4 {
                    return RespValue::Error("ERR wrong number of arguments for 'zrange' command".to_string());
                }
                let key_str = match &cmd[1] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                    _ => return RespValue::Error("Invalid key".to_string()),
                };
                let start_idx = match &cmd[2] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).parse::<i64>().unwrap_or(0),
                    RespValue::Integer(i) => *i,
                    _ => 0,
                };
                let stop_idx = match &cmd[3] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).parse::<i64>().unwrap_or(-1),
                    RespValue::Integer(i) => *i,
                    _ => -1,
                };

                let with_scores = cmd.get(4).map(|arg| match arg {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_uppercase() == "WITHSCORES",
                    _ => false,
                }).unwrap_or(false);

                let prefix = format!("__zset_score__:{}:", key_str);
                let pattern = format!("{}*", prefix);
                let mut entries: Vec<(Vec<u8>, f64)> = Vec::new();

                if let Ok(keys) = self.storage.keys(&pattern) {
                    for k in keys {
                        let k_str = String::from_utf8_lossy(&k);
                        if let Some(member) = k_str.strip_prefix(&prefix) {
                            if let Ok(Some(score_raw)) = self.storage.get(&k) {
                                let score_num = String::from_utf8_lossy(&score_raw).parse::<f64>().unwrap_or(0.0);
                                entries.push((member.as_bytes().to_vec(), score_num));
                            }
                        }
                    }
                }

                entries.sort_by(|a, b| {
                    a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| a.0.cmp(&b.0))
                });

                let len = entries.len() as i64;
                if len == 0 {
                    return RespValue::Array(Some(vec![]));
                }

                let s = if start_idx < 0 { (len + start_idx).max(0) } else { start_idx };
                let mut e = if stop_idx < 0 { (len + stop_idx).max(0) } else { stop_idx };

                if s >= len || s > e {
                    return RespValue::Array(Some(vec![]));
                }
                e = e.min(len - 1);

                let slice = &entries[s as usize..=e as usize];
                let mut result = Vec::new();
                for (member, score) in slice {
                    result.push(RespValue::BulkString(Some(member.clone())));
                    if with_scores {
                        result.push(RespValue::BulkString(Some(score.to_string().into_bytes())));
                    }
                }
                RespValue::Array(Some(result))
            }

            // Snapshotting & Persistence (SAVE / BGSAVE / LASTSAVE / RESTORE_DUMP)
            "SAVE" => {
                match self.save_dump() {
                    Ok(_) => RespValue::SimpleString("OK".to_string()),
                    Err(e) => RespValue::Error(format!("ERR save failed: {}", e)),
                }
            }

            "BGSAVE" => {
                let dump_res = self.save_dump();
                match dump_res {
                    Ok(_) => RespValue::SimpleString("Background saving started".to_string()),
                    Err(e) => RespValue::Error(format!("ERR bgsave failed: {}", e)),
                }
            }

            "LASTSAVE" => {
                let ts = self.last_save_time.load(Ordering::Relaxed);
                RespValue::Integer(ts as i64)
            }

            "RESTORE_DUMP" => {
                let custom_path = if cmd.len() > 1 {
                    match &cmd[1] {
                        RespValue::BulkString(Some(b)) => Some(PathBuf::from(String::from_utf8_lossy(b).to_string())),
                        _ => None,
                    }
                } else {
                    None
                };

                match self.restore_dump(custom_path.as_deref()) {
                    Ok(count) => RespValue::Integer(count as i64),
                    Err(e) => RespValue::Error(format!("ERR restore failed: {}", e)),
                }
            }

            // Geospatial Engine (GEOADD, GEODIST, GEORADIUS)
            "GEOADD" => {
                if cmd.len() < 5 || (cmd.len() - 2) % 3 != 0 {
                    return RespValue::Error("ERR wrong number of arguments for 'geoadd' command".to_string());
                }
                let key_str = match &cmd[1] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                    _ => return RespValue::Error("Invalid key".to_string()),
                };

                let mut added = 0;
                let mut i = 2;
                while i < cmd.len() {
                    let lon = match &cmd[i] {
                        RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).parse::<f64>().unwrap_or(0.0),
                        _ => return RespValue::Error("Invalid longitude".to_string()),
                    };
                    let lat = match &cmd[i + 1] {
                        RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).parse::<f64>().unwrap_or(0.0),
                        _ => return RespValue::Error("Invalid latitude".to_string()),
                    };
                    let member = match &cmd[i + 2] {
                        RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                        _ => return RespValue::Error("Invalid member".to_string()),
                    };

                    if !(-180.0..=180.0).contains(&lon) || !(-90.0..=90.0).contains(&lat) {
                        return RespValue::Error("ERR coordinates out of range".to_string());
                    }

                    let internal_key = format!("__geo__:{}:{}", key_str, member).into_bytes();
                    if let Ok(None) = self.storage.get(&internal_key) {
                        added += 1;
                    }
                    let val = format!("{},{}", lon, lat).into_bytes();
                    let _ = self.storage.put(&internal_key, &val);
                    i += 3;
                }
                RespValue::Integer(added as i64)
            }

            "GEODIST" => {
                if cmd.len() < 4 {
                    return RespValue::Error("ERR wrong number of arguments for 'geodist' command".to_string());
                }
                let key_str = match &cmd[1] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                    _ => return RespValue::Error("Invalid key".to_string()),
                };
                let m1 = match &cmd[2] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                    _ => return RespValue::Error("Invalid member 1".to_string()),
                };
                let m2 = match &cmd[3] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                    _ => return RespValue::Error("Invalid member 2".to_string()),
                };
                let unit = cmd.get(4).map(|u| match u {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                    _ => "m".to_string(),
                }).unwrap_or_else(|| "m".to_string());

                let k1 = format!("__geo__:{}:{}", key_str, m1).into_bytes();
                let k2 = format!("__geo__:{}:{}", key_str, m2).into_bytes();

                let (p1, p2) = match (self.storage.get(&k1), self.storage.get(&k2)) {
                    (Ok(Some(v1)), Ok(Some(v2))) => (v1, v2),
                    _ => return RespValue::BulkString(None),
                };

                let parse_point = |b: &[u8]| -> Option<(f64, f64)> {
                    let s = String::from_utf8_lossy(b);
                    let mut parts = s.split(',');
                    let lon = parts.next()?.parse::<f64>().ok()?;
                    let lat = parts.next()?.parse::<f64>().ok()?;
                    Some((lon, lat))
                };

                let (lon1, lat1) = match parse_point(&p1) {
                    Some(p) => p,
                    None => return RespValue::BulkString(None),
                };
                let (lon2, lat2) = match parse_point(&p2) {
                    Some(p) => p,
                    None => return RespValue::BulkString(None),
                };

                let dist_m = haversine_distance(lon1, lat1, lon2, lat2);
                let converted = convert_distance_from_meters(dist_m, &unit);
                RespValue::BulkString(Some(format!("{:.4}", converted).into_bytes()))
            }

            "GEORADIUS" => {
                if cmd.len() < 5 {
                    return RespValue::Error("ERR wrong number of arguments for 'georadius' command".to_string());
                }
                let key_str = match &cmd[1] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                    _ => return RespValue::Error("Invalid key".to_string()),
                };
                let center_lon = match &cmd[2] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).parse::<f64>().unwrap_or(0.0),
                    _ => return RespValue::Error("Invalid longitude".to_string()),
                };
                let center_lat = match &cmd[3] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).parse::<f64>().unwrap_or(0.0),
                    _ => return RespValue::Error("Invalid latitude".to_string()),
                };
                let radius_num = match &cmd[4] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).parse::<f64>().unwrap_or(0.0),
                    _ => return RespValue::Error("Invalid radius".to_string()),
                };

                let mut unit = "m";
                let mut with_dist = false;
                for arg in &cmd[5..] {
                    if let RespValue::BulkString(Some(b)) = arg {
                        let s = String::from_utf8_lossy(b).to_lowercase();
                        if s == "withdist" {
                            with_dist = true;
                        } else if s == "km" || s == "m" || s == "mi" || s == "ft" {
                            unit = if s == "km" { "km" } else if s == "mi" { "mi" } else if s == "ft" { "ft" } else { "m" };
                        }
                    }
                }

                let max_meters = convert_distance_to_meters(radius_num, unit);
                let prefix = format!("__geo__:{}:", key_str);
                let pattern = format!("{}*", prefix);
                let mut matches: Vec<(String, f64)> = Vec::new();

                if let Ok(keys) = self.storage.keys(&pattern) {
                    for k in keys {
                        let k_str = String::from_utf8_lossy(&k);
                        if let Some(member) = k_str.strip_prefix(&prefix) {
                            if let Ok(Some(raw)) = self.storage.get(&k) {
                                let s = String::from_utf8_lossy(&raw);
                                if let Some((lon_str, lat_str)) = s.split_once(',') {
                                    if let (Ok(m_lon), Ok(m_lat)) = (lon_str.parse::<f64>(), lat_str.parse::<f64>()) {
                                        let dist = haversine_distance(center_lon, center_lat, m_lon, m_lat);
                                        if dist <= max_meters {
                                            matches.push((member.to_string(), dist));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                matches.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
                let mut out = Vec::new();
                for (member, dist_m) in matches {
                    if with_dist {
                        let converted = convert_distance_from_meters(dist_m, unit);
                        out.push(RespValue::Array(Some(vec![
                            RespValue::BulkString(Some(member.into_bytes())),
                            RespValue::BulkString(Some(format!("{:.4}", converted).into_bytes())),
                        ])));
                    } else {
                        out.push(RespValue::BulkString(Some(member.into_bytes())));
                    }
                }
                RespValue::Array(Some(out))
            }

            // High-Speed Atomic Sliding-Window Rate Limiter
            "RATELIMIT" => {
                if cmd.len() < 4 {
                    return RespValue::Error("ERR wrong number of arguments for 'ratelimit' command".to_string());
                }
                let key_str = match &cmd[1] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                    _ => return RespValue::Error("Invalid key".to_string()),
                };
                let max_reqs = match &cmd[2] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).parse::<usize>().unwrap_or(10),
                    RespValue::Integer(i) => *i as usize,
                    _ => 10,
                };
                let window_secs = match &cmd[3] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).parse::<u64>().unwrap_or(60),
                    RespValue::Integer(i) => *i as u64,
                    _ => 60,
                };

                let now_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);
                let window_ms = window_secs * 1000;
                let cutoff_ms = now_ms.saturating_sub(window_ms);

                let internal_key = format!("__ratelimit__:{}", key_str).into_bytes();
                let mut timestamps: Vec<u64> = match self.storage.get(&internal_key) {
                    Ok(Some(raw)) => bincode::deserialize(&raw).unwrap_or_default(),
                    _ => Vec::new(),
                };

                timestamps.retain(|&ts| ts >= cutoff_ms);
                let current_count = timestamps.len();

                if current_count < max_reqs {
                    timestamps.push(now_ms);
                    if let Ok(raw) = bincode::serialize(&timestamps) {
                        let _ = self.storage.put(&internal_key, &raw);
                    }
                    let remaining = (max_reqs - current_count - 1) as i64;
                    RespValue::Array(Some(vec![
                        RespValue::Integer(1), // Allowed
                        RespValue::Integer(remaining),
                        RespValue::Integer(window_secs as i64),
                    ]))
                } else {
                    let oldest = timestamps.first().copied().unwrap_or(now_ms);
                    let reset_secs = ((oldest + window_ms).saturating_sub(now_ms) / 1000) + 1;
                    RespValue::Array(Some(vec![
                        RespValue::Integer(0), // Blocked
                        RespValue::Integer(0),
                        RespValue::Integer(reset_secs as i64),
                    ]))
                }
            }

            // Dynamic Cluster Membership (CLUSTER MEET / CLUSTER NODES)
            "CLUSTER" => {
                if cmd.len() < 2 {
                    return RespValue::Error("ERR wrong number of arguments for 'cluster' command".to_string());
                }
                let sub_cmd = match &cmd[1] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_uppercase(),
                    _ => return RespValue::Error("Invalid cluster subcommand".to_string()),
                };

                match sub_cmd.as_str() {
                    "MEET" => {
                        if cmd.len() < 4 {
                            return RespValue::Error("ERR wrong number of arguments for 'cluster meet' command".to_string());
                        }
                        let _ip = match &cmd[2] {
                            RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                            _ => return RespValue::Error("Invalid IP".to_string()),
                        };
                        let port = match &cmd[3] {
                            RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).parse::<u64>().unwrap_or(0),
                            RespValue::Integer(i) => *i as u64,
                            _ => return RespValue::Error("Invalid port".to_string()),
                        };
                        let target_node_id = if port >= 6379 { (port - 6378) as u64 } else { port };
                        self.raft.add_peer(target_node_id);
                        RespValue::SimpleString("OK".to_string())
                    }
                    "NODES" => {
                        let self_id = self.raft.id;
                        let role = match self.raft.state.read().role {
                            NodeRole::Leader => "master",
                            NodeRole::Follower => "slave",
                            NodeRole::Candidate => "candidate",
                        };
                        let mut out = format!("{}-self 127.0.0.1:{} myself,{} 0 0 1 connected\n", self_id, 6378 + self_id, role);
                        for peer_id in self.raft.peers.read().iter() {
                            out.push_str(&format!("{}-node 127.0.0.1:{} slave 0 0 1 connected\n", peer_id, 6378 + peer_id));
                        }
                        RespValue::BulkString(Some(out.into_bytes()))
                    }
                    _ => RespValue::Error(format!("ERR unknown cluster subcommand '{}'", sub_cmd)),
                }
            }

            // Bitmaps Engine (SETBIT, GETBIT, BITCOUNT)
            "SETBIT" => {
                if cmd.len() < 4 {
                    return RespValue::Error("ERR wrong number of arguments for 'setbit' command".to_string());
                }
                let key = match &cmd[1] {
                    RespValue::BulkString(Some(b)) => b.clone(),
                    _ => return RespValue::Error("Invalid key".to_string()),
                };
                let offset: usize = match &cmd[2] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).parse().unwrap_or(0),
                    RespValue::Integer(i) => *i as usize,
                    _ => 0,
                };
                let val: u8 = match &cmd[3] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).parse().unwrap_or(0),
                    RespValue::Integer(i) => *i as u8,
                    _ => 0,
                };
                if val > 1 {
                    return RespValue::Error("ERR bit is not an integer or out of range".to_string());
                }

                let byte_idx = offset / 8;
                let bit_idx = 7 - (offset % 8);

                let mut bytes = self.storage.get(&key).ok().flatten().unwrap_or_default();
                if byte_idx >= bytes.len() {
                    bytes.resize(byte_idx + 1, 0);
                }
                let prev_bit = (bytes[byte_idx] >> bit_idx) & 1;
                if val == 1 {
                    bytes[byte_idx] |= 1 << bit_idx;
                } else {
                    bytes[byte_idx] &= !(1 << bit_idx);
                }
                let _ = self.storage.put(&key, &bytes);
                RespValue::Integer(prev_bit as i64)
            }

            "GETBIT" => {
                if cmd.len() < 3 {
                    return RespValue::Error("ERR wrong number of arguments for 'getbit' command".to_string());
                }
                let key = match &cmd[1] {
                    RespValue::BulkString(Some(b)) => b.clone(),
                    _ => return RespValue::Error("Invalid key".to_string()),
                };
                let offset: usize = match &cmd[2] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).parse().unwrap_or(0),
                    RespValue::Integer(i) => *i as usize,
                    _ => 0,
                };

                let byte_idx = offset / 8;
                let bit_idx = 7 - (offset % 8);
                let bit = match self.storage.get(&key).ok().flatten() {
                    Some(bytes) if byte_idx < bytes.len() => (bytes[byte_idx] >> bit_idx) & 1,
                    _ => 0,
                };
                RespValue::Integer(bit as i64)
            }

            "BITCOUNT" => {
                if cmd.len() < 2 {
                    return RespValue::Error("ERR wrong number of arguments for 'bitcount' command".to_string());
                }
                let key = match &cmd[1] {
                    RespValue::BulkString(Some(b)) => b.clone(),
                    _ => return RespValue::Error("Invalid key".to_string()),
                };
                let bytes = match self.storage.get(&key).ok().flatten() {
                    Some(b) => b,
                    None => return RespValue::Integer(0),
                };
                let len = bytes.len() as i64;
                if len == 0 {
                    return RespValue::Integer(0);
                }

                let (start, stop) = if cmd.len() >= 4 {
                    let s = match &cmd[2] {
                        RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).parse::<i64>().unwrap_or(0),
                        RespValue::Integer(i) => *i,
                        _ => 0,
                    };
                    let e = match &cmd[3] {
                        RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).parse::<i64>().unwrap_or(-1),
                        RespValue::Integer(i) => *i,
                        _ => -1,
                    };
                    let s_norm = if s < 0 { (len + s).max(0) } else { s };
                    let e_norm = if e < 0 { (len + e).max(0) } else { e };
                    (s_norm as usize, (e_norm as usize).min(bytes.len().saturating_sub(1)))
                } else {
                    (0, bytes.len().saturating_sub(1))
                };

                let count: u64 = if start <= stop && start < bytes.len() {
                    bytes[start..=stop].iter().map(|&b| b.count_ones() as u64).sum()
                } else {
                    0
                };
                RespValue::Integer(count as i64)
            }

            // HyperLogLog Cardinality Estimation (PFADD, PFCOUNT)
            "PFADD" => {
                if cmd.len() < 2 {
                    return RespValue::Error("ERR wrong number of arguments for 'pfadd' command".to_string());
                }
                let key_str = match &cmd[1] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                    _ => return RespValue::Error("Invalid key".to_string()),
                };
                let internal_key = format!("__hll__:{}", key_str).into_bytes();
                let mut registers = match self.storage.get(&internal_key).ok().flatten() {
                    Some(r) if r.len() == 16384 => r,
                    _ => vec![0u8; 16384],
                };

                let mut updated = false;
                for arg in &cmd[2..] {
                    if let RespValue::BulkString(Some(b)) = arg {
                        let hash = fnv1a_64(b);
                        let idx = (hash & 0x3FFF) as usize; // first 14 bits -> 16384 registers
                        let w = hash >> 14;
                        let rho = (w.leading_zeros() + 1).min(64) as u8;
                        if rho > registers[idx] {
                            registers[idx] = rho;
                            updated = true;
                        }
                    }
                }

                if updated {
                    let _ = self.storage.put(&internal_key, &registers);
                    RespValue::Integer(1)
                } else {
                    RespValue::Integer(0)
                }
            }

            "PFCOUNT" => {
                if cmd.len() < 2 {
                    return RespValue::Error("ERR wrong number of arguments for 'pfcount' command".to_string());
                }
                let mut merged = vec![0u8; 16384];
                let mut any_found = false;

                for k_arg in &cmd[1..] {
                    if let RespValue::BulkString(Some(b)) = k_arg {
                        let key_str = String::from_utf8_lossy(b);
                        let internal_key = format!("__hll__:{}", key_str).into_bytes();
                        if let Some(r) = self.storage.get(&internal_key).ok().flatten() {
                            if r.len() == 16384 {
                                any_found = true;
                                for i in 0..16384 {
                                    merged[i] = merged[i].max(r[i]);
                                }
                            }
                        }
                    }
                }

                if !any_found {
                    return RespValue::Integer(0);
                }

                let m = 16384.0;
                let alpha_m = 0.7213 / (1.0 + 1.079 / m);
                let sum: f64 = merged.iter().map(|&val| 2.0f64.powi(-(val as i32))).sum();
                let mut estimate = alpha_m * (m * m) / sum;

                if estimate <= 2.5 * m {
                    let zeros = merged.iter().filter(|&&v| v == 0).count() as f64;
                    if zeros > 0.0 {
                        estimate = m * (m / zeros).ln();
                    }
                }
                RespValue::Integer(estimate.round() as i64)
            }

            // Redis Streams & Event Sourcing Engine (XADD, XRANGE, XLEN)
            "XADD" => {
                if cmd.len() < 5 || (cmd.len() - 3) % 2 != 0 {
                    return RespValue::Error("ERR wrong number of arguments for 'xadd' command".to_string());
                }
                let key_str = match &cmd[1] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                    _ => return RespValue::Error("Invalid stream key".to_string()),
                };
                let id_spec = match &cmd[2] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                    _ => "*".to_string(),
                };

                let now_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);

                let seq_key = format!("__stream_seq__:{}", key_str).into_bytes();
                let (last_ms, last_seq): (u64, u64) = match self.storage.get(&seq_key).ok().flatten() {
                    Some(raw) => bincode::deserialize(&raw).unwrap_or((0, 0)),
                    None => (0, 0),
                };

                let entry_id = if id_spec == "*" {
                    let seq = if now_ms == last_ms { last_seq + 1 } else { 0 };
                    format!("{}-{}", now_ms, seq)
                } else {
                    id_spec
                };

                let mut fields: Vec<(String, Vec<u8>)> = Vec::new();
                let mut i = 3;
                while i < cmd.len() {
                    let f = match &cmd[i] {
                        RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                        _ => return RespValue::Error("Invalid field".to_string()),
                    };
                    let v = match &cmd[i + 1] {
                        RespValue::BulkString(Some(b)) => b.clone(),
                        _ => return RespValue::Error("Invalid value".to_string()),
                    };
                    fields.push((f, v));
                    i += 2;
                }

                let entry_key = format!("__stream_entry__:{}:{}", key_str, entry_id).into_bytes();
                if let Ok(raw) = bincode::serialize(&fields) {
                    let _ = self.storage.put(&entry_key, &raw);
                }

                let ids_key = format!("__stream_ids__:{}", key_str).into_bytes();
                let mut ids: Vec<String> = match self.storage.get(&ids_key).ok().flatten() {
                    Some(raw) => bincode::deserialize(&raw).unwrap_or_default(),
                    None => Vec::new(),
                };
                ids.push(entry_id.clone());
                if let Ok(raw) = bincode::serialize(&ids) {
                    let _ = self.storage.put(&ids_key, &raw);
                }

                if let Ok(raw) = bincode::serialize(&(now_ms, if now_ms == last_ms { last_seq + 1 } else { 0 })) {
                    let _ = self.storage.put(&seq_key, &raw);
                }

                RespValue::BulkString(Some(entry_id.into_bytes()))
            }

            "XRANGE" => {
                if cmd.len() < 4 {
                    return RespValue::Error("ERR wrong number of arguments for 'xrange' command".to_string());
                }
                let key_str = match &cmd[1] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                    _ => return RespValue::Error("Invalid stream key".to_string()),
                };
                let start_id = match &cmd[2] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                    _ => "-".to_string(),
                };
                let stop_id = match &cmd[3] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                    _ => "+".to_string(),
                };

                let ids_key = format!("__stream_ids__:{}", key_str).into_bytes();
                let ids: Vec<String> = match self.storage.get(&ids_key).ok().flatten() {
                    Some(raw) => bincode::deserialize(&raw).unwrap_or_default(),
                    None => Vec::new(),
                };

                let mut entries = Vec::new();
                for id in ids {
                    let after_start = start_id == "-" || id.as_str() >= start_id.as_str();
                    let before_stop = stop_id == "+" || id.as_str() <= stop_id.as_str();
                    if after_start && before_stop {
                        let entry_key = format!("__stream_entry__:{}:{}", key_str, id).into_bytes();
                        if let Some(raw) = self.storage.get(&entry_key).ok().flatten() {
                            if let Ok(fields) = bincode::deserialize::<Vec<(String, Vec<u8>)>>(&raw) {
                                let mut f_arr = Vec::new();
                                for (k, v) in fields {
                                    f_arr.push(RespValue::BulkString(Some(k.into_bytes())));
                                    f_arr.push(RespValue::BulkString(Some(v)));
                                }
                                entries.push(RespValue::Array(Some(vec![
                                    RespValue::BulkString(Some(id.into_bytes())),
                                    RespValue::Array(Some(f_arr)),
                                ])));
                            }
                        }
                    }
                }
                RespValue::Array(Some(entries))
            }

            "XLEN" => {
                if cmd.len() < 2 {
                    return RespValue::Error("ERR wrong number of arguments for 'xlen' command".to_string());
                }
                let key_str = match &cmd[1] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                    _ => return RespValue::Error("Invalid stream key".to_string()),
                };
                let ids_key = format!("__stream_ids__:{}", key_str).into_bytes();
                let count = match self.storage.get(&ids_key).ok().flatten() {
                    Some(raw) => bincode::deserialize::<Vec<String>>(&raw).map(|v| v.len()).unwrap_or(0),
                    None => 0,
                };
                RespValue::Integer(count as i64)
            }

            "SELECT" => {
                if cmd.len() < 2 {
                    return RespValue::Error("ERR wrong number of arguments for 'select' command".to_string());
                }
                let db_idx = match &cmd[1] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).parse::<u8>().unwrap_or(0),
                    RespValue::Integer(i) => *i as u8,
                    _ => 0,
                };
                if db_idx > 15 {
                    return RespValue::Error("ERR DB index is out of range".to_string());
                }
                RespValue::SimpleString("OK".to_string())
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

// -----------------------------------------------------------------------------
// Geospatial Math Helpers (Haversine Spherical Distance Formula)
// -----------------------------------------------------------------------------
fn haversine_distance(lon1: f64, lat1: f64, lon2: f64, lat2: f64) -> f64 {
    let r = 6_372_795.0; // Earth mean radius in meters
    let dlat = (lat2 - lat1).to_radians();
    let dlon = (lon2 - lon1).to_radians();
    let a = (dlat / 2.0).sin().powi(2)
        + lat1.to_radians().cos() * lat2.to_radians().cos() * (dlon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
    r * c
}

fn convert_distance_from_meters(meters: f64, unit: &str) -> f64 {
    match unit.to_lowercase().as_str() {
        "km" => meters / 1000.0,
        "mi" => meters / 1609.344,
        "ft" => meters * 3.28084,
        _ => meters,
    }
}

fn convert_distance_to_meters(dist: f64, unit: &str) -> f64 {
    match unit.to_lowercase().as_str() {
        "km" => dist * 1000.0,
        "mi" => dist * 1609.344,
        "ft" => dist / 3.28084,
        _ => dist,
    }
}

// -----------------------------------------------------------------------------
// HyperLogLog FNV-1a 64-bit Non-Cryptographic Fast Hash
// -----------------------------------------------------------------------------
fn fnv1a_64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
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

        // Test Lists & Queues (LPUSH, RPUSH, LPOP, RPOP, LLEN, LRANGE)
        let rpush_res = engine.execute_command(&[
            RespValue::BulkString(Some(b"RPUSH".to_vec())),
            RespValue::BulkString(Some(b"task_queue".to_vec())),
            RespValue::BulkString(Some(b"job1".to_vec())),
            RespValue::BulkString(Some(b"job2".to_vec())),
        ]);
        assert_eq!(rpush_res, RespValue::Integer(2));

        let lpush_res = engine.execute_command(&[
            RespValue::BulkString(Some(b"LPUSH".to_vec())),
            RespValue::BulkString(Some(b"task_queue".to_vec())),
            RespValue::BulkString(Some(b"job0".to_vec())),
        ]);
        assert_eq!(lpush_res, RespValue::Integer(3));

        let llen_res = engine.execute_command(&[
            RespValue::BulkString(Some(b"LLEN".to_vec())),
            RespValue::BulkString(Some(b"task_queue".to_vec())),
        ]);
        assert_eq!(llen_res, RespValue::Integer(3));

        let lrange_all = engine.execute_command(&[
            RespValue::BulkString(Some(b"LRANGE".to_vec())),
            RespValue::BulkString(Some(b"task_queue".to_vec())),
            RespValue::BulkString(Some(b"0".to_vec())),
            RespValue::BulkString(Some(b"-1".to_vec())),
        ]);
        assert_eq!(
            lrange_all,
            RespValue::Array(Some(vec![
                RespValue::BulkString(Some(b"job0".to_vec())),
                RespValue::BulkString(Some(b"job1".to_vec())),
                RespValue::BulkString(Some(b"job2".to_vec())),
            ]))
        );

        let lpop_res = engine.execute_command(&[
            RespValue::BulkString(Some(b"LPOP".to_vec())),
            RespValue::BulkString(Some(b"task_queue".to_vec())),
        ]);
        assert_eq!(lpop_res, RespValue::BulkString(Some(b"job0".to_vec())));

        let rpop_res = engine.execute_command(&[
            RespValue::BulkString(Some(b"RPOP".to_vec())),
            RespValue::BulkString(Some(b"task_queue".to_vec())),
        ]);
        assert_eq!(rpop_res, RespValue::BulkString(Some(b"job2".to_vec())));

        // Test Pub/Sub Engine
        let _rx = engine.get_or_create_channel("news").subscribe();
        let pub_res = engine.execute_command(&[
            RespValue::BulkString(Some(b"PUBLISH".to_vec())),
            RespValue::BulkString(Some(b"news".to_vec())),
            RespValue::BulkString(Some(b"breaking_news_event".to_vec())),
        ]);
        assert_eq!(pub_res, RespValue::Integer(1));

        // Test SAVE & Snapshot Dump
        let save_res = engine.execute_command(&[RespValue::BulkString(Some(b"SAVE".to_vec()))]);
        assert_eq!(save_res, RespValue::SimpleString("OK".to_string()));

        let bgsave_res = engine.execute_command(&[RespValue::BulkString(Some(b"BGSAVE".to_vec()))]);
        assert_eq!(bgsave_res, RespValue::SimpleString("Background saving started".to_string()));

        let lastsave_res = engine.execute_command(&[RespValue::BulkString(Some(b"LASTSAVE".to_vec()))]);
        if let RespValue::Integer(ts) = lastsave_res {
            assert!(ts > 0);
        } else {
            panic!("Expected Integer for LASTSAVE");
        }

        // Test Hashes Data Structure (HSET, HGET, HGETALL, HDEL, HLEN, HEXISTS)
        let hset_res = engine.execute_command(&[
            RespValue::BulkString(Some(b"HSET".to_vec())),
            RespValue::BulkString(Some(b"user:101".to_vec())),
            RespValue::BulkString(Some(b"name".to_vec())),
            RespValue::BulkString(Some(b"Ali Hasanifard".to_vec())),
            RespValue::BulkString(Some(b"role".to_vec())),
            RespValue::BulkString(Some(b"system-architect".to_vec())),
        ]);
        assert_eq!(hset_res, RespValue::Integer(2));

        let hget_name = engine.execute_command(&[
            RespValue::BulkString(Some(b"HGET".to_vec())),
            RespValue::BulkString(Some(b"user:101".to_vec())),
            RespValue::BulkString(Some(b"name".to_vec())),
        ]);
        assert_eq!(hget_name, RespValue::BulkString(Some(b"Ali Hasanifard".to_vec())));

        let hexists_role = engine.execute_command(&[
            RespValue::BulkString(Some(b"HEXISTS".to_vec())),
            RespValue::BulkString(Some(b"user:101".to_vec())),
            RespValue::BulkString(Some(b"role".to_vec())),
        ]);
        assert_eq!(hexists_role, RespValue::Integer(1));

        let hlen_res = engine.execute_command(&[
            RespValue::BulkString(Some(b"HLEN".to_vec())),
            RespValue::BulkString(Some(b"user:101".to_vec())),
        ]);
        assert_eq!(hlen_res, RespValue::Integer(2));

        let hdel_role = engine.execute_command(&[
            RespValue::BulkString(Some(b"HDEL".to_vec())),
            RespValue::BulkString(Some(b"user:101".to_vec())),
            RespValue::BulkString(Some(b"role".to_vec())),
        ]);
        assert_eq!(hdel_role, RespValue::Integer(1));

        let hlen_after = engine.execute_command(&[
            RespValue::BulkString(Some(b"HLEN".to_vec())),
            RespValue::BulkString(Some(b"user:101".to_vec())),
        ]);
        assert_eq!(hlen_after, RespValue::Integer(1));

        // Test Sorted Sets (ZADD, ZRANGE, ZSCORE, ZCARD, ZREM)
        let zadd_res = engine.execute_command(&[
            RespValue::BulkString(Some(b"ZADD".to_vec())),
            RespValue::BulkString(Some(b"leaderboard".to_vec())),
            RespValue::BulkString(Some(b"1500.5".to_vec())),
            RespValue::BulkString(Some(b"player_c".to_vec())),
            RespValue::BulkString(Some(b"500.0".to_vec())),
            RespValue::BulkString(Some(b"player_a".to_vec())),
            RespValue::BulkString(Some(b"1000.0".to_vec())),
            RespValue::BulkString(Some(b"player_b".to_vec())),
        ]);
        assert_eq!(zadd_res, RespValue::Integer(3));

        let zscore_res = engine.execute_command(&[
            RespValue::BulkString(Some(b"ZSCORE".to_vec())),
            RespValue::BulkString(Some(b"leaderboard".to_vec())),
            RespValue::BulkString(Some(b"player_b".to_vec())),
        ]);
        assert_eq!(zscore_res, RespValue::BulkString(Some(b"1000".to_vec())));

        let zcard_res = engine.execute_command(&[
            RespValue::BulkString(Some(b"ZCARD".to_vec())),
            RespValue::BulkString(Some(b"leaderboard".to_vec())),
        ]);
        assert_eq!(zcard_res, RespValue::Integer(3));

        let zrange_res = engine.execute_command(&[
            RespValue::BulkString(Some(b"ZRANGE".to_vec())),
            RespValue::BulkString(Some(b"leaderboard".to_vec())),
            RespValue::BulkString(Some(b"0".to_vec())),
            RespValue::BulkString(Some(b"-1".to_vec())),
        ]);
        assert_eq!(
            zrange_res,
            RespValue::Array(Some(vec![
                RespValue::BulkString(Some(b"player_a".to_vec())),
                RespValue::BulkString(Some(b"player_b".to_vec())),
                RespValue::BulkString(Some(b"player_c".to_vec())),
            ]))
        );

        let zrem_res = engine.execute_command(&[
            RespValue::BulkString(Some(b"ZREM".to_vec())),
            RespValue::BulkString(Some(b"leaderboard".to_vec())),
            RespValue::BulkString(Some(b"player_b".to_vec())),
        ]);
        assert_eq!(zrem_res, RespValue::Integer(1));

        let zcard_after = engine.execute_command(&[
            RespValue::BulkString(Some(b"ZCARD".to_vec())),
            RespValue::BulkString(Some(b"leaderboard".to_vec())),
        ]);
        assert_eq!(zcard_after, RespValue::Integer(2));

        // Test Point-in-Time Dump & RESTORE_DUMP
        let _ = engine.execute_command(&[RespValue::BulkString(Some(b"SAVE".to_vec()))]);
        let restore_res = engine.execute_command(&[RespValue::BulkString(Some(b"RESTORE_DUMP".to_vec()))]);
        if let RespValue::Integer(count) = restore_res {
            assert!(count > 0);
        } else {
            panic!("Expected Integer count for RESTORE_DUMP");
        }

        // Test Geospatial Engine (GEOADD, GEODIST, GEORADIUS)
        let geoadd_res = engine.execute_command(&[
            RespValue::BulkString(Some(b"GEOADD".to_vec())),
            RespValue::BulkString(Some(b"tehran_fleet".to_vec())),
            RespValue::BulkString(Some(b"51.3890".to_vec())),
            RespValue::BulkString(Some(b"35.6892".to_vec())),
            RespValue::BulkString(Some(b"driver_azadi".to_vec())),
            RespValue::BulkString(Some(b"51.4200".to_vec())),
            RespValue::BulkString(Some(b"35.7000".to_vec())),
            RespValue::BulkString(Some(b"driver_enghelab".to_vec())),
        ]);
        assert_eq!(geoadd_res, RespValue::Integer(2));

        let geodist_res = engine.execute_command(&[
            RespValue::BulkString(Some(b"GEODIST".to_vec())),
            RespValue::BulkString(Some(b"tehran_fleet".to_vec())),
            RespValue::BulkString(Some(b"driver_azadi".to_vec())),
            RespValue::BulkString(Some(b"driver_enghelab".to_vec())),
            RespValue::BulkString(Some(b"km".to_vec())),
        ]);
        if let RespValue::BulkString(Some(b)) = geodist_res {
            let s = String::from_utf8_lossy(&b);
            let dist: f64 = s.parse().unwrap();
            assert!(dist > 1.0 && dist < 10.0);
        } else {
            panic!("Expected BulkString for GEODIST");
        }

        let georadius_res = engine.execute_command(&[
            RespValue::BulkString(Some(b"GEORADIUS".to_vec())),
            RespValue::BulkString(Some(b"tehran_fleet".to_vec())),
            RespValue::BulkString(Some(b"51.3890".to_vec())),
            RespValue::BulkString(Some(b"35.6892".to_vec())),
            RespValue::BulkString(Some(b"5".to_vec())),
            RespValue::BulkString(Some(b"km".to_vec())),
        ]);
        if let RespValue::Array(Some(drivers)) = georadius_res {
            assert!(!drivers.is_empty());
        } else {
            panic!("Expected Array for GEORADIUS");
        }

        // Test Native Atomic Rate Limiter (RATELIMIT)
        let rl1 = engine.execute_command(&[
            RespValue::BulkString(Some(b"RATELIMIT".to_vec())),
            RespValue::BulkString(Some(b"user:login:127.0.0.1".to_vec())),
            RespValue::BulkString(Some(b"2".to_vec())),
            RespValue::BulkString(Some(b"10".to_vec())),
        ]);
        assert_eq!(rl1, RespValue::Array(Some(vec![RespValue::Integer(1), RespValue::Integer(1), RespValue::Integer(10)])));

        let rl2 = engine.execute_command(&[
            RespValue::BulkString(Some(b"RATELIMIT".to_vec())),
            RespValue::BulkString(Some(b"user:login:127.0.0.1".to_vec())),
            RespValue::BulkString(Some(b"2".to_vec())),
            RespValue::BulkString(Some(b"10".to_vec())),
        ]);
        assert_eq!(rl2, RespValue::Array(Some(vec![RespValue::Integer(1), RespValue::Integer(0), RespValue::Integer(10)])));

        let rl3 = engine.execute_command(&[
            RespValue::BulkString(Some(b"RATELIMIT".to_vec())),
            RespValue::BulkString(Some(b"user:login:127.0.0.1".to_vec())),
            RespValue::BulkString(Some(b"2".to_vec())),
            RespValue::BulkString(Some(b"10".to_vec())),
        ]);
        if let RespValue::Array(Some(arr)) = rl3 {
            assert_eq!(arr[0], RespValue::Integer(0)); // Blocked
        } else {
            panic!("Expected Array for RATELIMIT");
        }

        // Test CLUSTER MEET
        let meet_res = engine.execute_command(&[
            RespValue::BulkString(Some(b"CLUSTER".to_vec())),
            RespValue::BulkString(Some(b"MEET".to_vec())),
            RespValue::BulkString(Some(b"127.0.0.1".to_vec())),
            RespValue::BulkString(Some(b"6382".to_vec())),
        ]);
        assert_eq!(meet_res, RespValue::SimpleString("OK".to_string()));

        // Test Bitmaps (SETBIT, GETBIT, BITCOUNT)
        let sb1 = engine.execute_command(&[
            RespValue::BulkString(Some(b"SETBIT".to_vec())),
            RespValue::BulkString(Some(b"user:dau:20261106".to_vec())),
            RespValue::BulkString(Some(b"7".to_vec())),
            RespValue::BulkString(Some(b"1".to_vec())),
        ]);
        assert_eq!(sb1, RespValue::Integer(0));

        let gb1 = engine.execute_command(&[
            RespValue::BulkString(Some(b"GETBIT".to_vec())),
            RespValue::BulkString(Some(b"user:dau:20261106".to_vec())),
            RespValue::BulkString(Some(b"7".to_vec())),
        ]);
        assert_eq!(gb1, RespValue::Integer(1));

        let bc = engine.execute_command(&[
            RespValue::BulkString(Some(b"BITCOUNT".to_vec())),
            RespValue::BulkString(Some(b"user:dau:20261106".to_vec())),
        ]);
        assert_eq!(bc, RespValue::Integer(1));

        // Test HyperLogLog (PFADD, PFCOUNT)
        let pf1 = engine.execute_command(&[
            RespValue::BulkString(Some(b"PFADD".to_vec())),
            RespValue::BulkString(Some(b"hll:visitors".to_vec())),
            RespValue::BulkString(Some(b"user_1".to_vec())),
            RespValue::BulkString(Some(b"user_2".to_vec())),
            RespValue::BulkString(Some(b"user_3".to_vec())),
        ]);
        assert_eq!(pf1, RespValue::Integer(1));

        let pfc = engine.execute_command(&[
            RespValue::BulkString(Some(b"PFCOUNT".to_vec())),
            RespValue::BulkString(Some(b"hll:visitors".to_vec())),
        ]);
        if let RespValue::Integer(card) = pfc {
            assert!(card >= 2 && card <= 4);
        } else {
            panic!("Expected Integer for PFCOUNT");
        }

        // Test Streams (XADD, XRANGE, XLEN)
        let xadd_res = engine.execute_command(&[
            RespValue::BulkString(Some(b"XADD".to_vec())),
            RespValue::BulkString(Some(b"events:orders".to_vec())),
            RespValue::BulkString(Some(b"*".to_vec())),
            RespValue::BulkString(Some(b"item".to_vec())),
            RespValue::BulkString(Some(b"laptop".to_vec())),
            RespValue::BulkString(Some(b"price".to_vec())),
            RespValue::BulkString(Some(b"1200".to_vec())),
        ]);
        if let RespValue::BulkString(Some(id)) = xadd_res {
            assert!(!id.is_empty());
        } else {
            panic!("Expected BulkString ID for XADD");
        }

        let xlen_res = engine.execute_command(&[
            RespValue::BulkString(Some(b"XLEN".to_vec())),
            RespValue::BulkString(Some(b"events:orders".to_vec())),
        ]);
        assert_eq!(xlen_res, RespValue::Integer(1));

        let xrange_res = engine.execute_command(&[
            RespValue::BulkString(Some(b"XRANGE".to_vec())),
            RespValue::BulkString(Some(b"events:orders".to_vec())),
            RespValue::BulkString(Some(b"-".to_vec())),
            RespValue::BulkString(Some(b"+".to_vec())),
        ]);
        if let RespValue::Array(Some(entries)) = xrange_res {
            assert_eq!(entries.len(), 1);
        } else {
            panic!("Expected Array for XRANGE");
        }

        // Test SELECT command
        session.authenticated = true;
        let sel_res = engine.execute_with_session(&mut session, &[
            RespValue::BulkString(Some(b"SELECT".to_vec())),
            RespValue::BulkString(Some(b"3".to_vec())),
        ]);
        assert_eq!(sel_res, RespValue::SimpleString("OK".to_string()));
        assert_eq!(session.db, 3);
    }
}
