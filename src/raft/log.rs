// ==============================================================================
// 📋 NEXUS-KV: RAFT LOG (لاگ وقایع الگوریتم اجماع توزیع‌شده)
// Architect: Ali Hasanifard
// ==============================================================================

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RaftCommand {
    Put { key: Vec<u8>, value: Vec<u8> },
    Delete { key: Vec<u8> },
    Noop,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogEntry {
    pub term: u64,
    pub index: u64,
    pub command: RaftCommand,
}

#[derive(Debug, Default)]
pub struct RaftLog {
    entries: Vec<LogEntry>,
}

impl RaftLog {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn last_log_index(&self) -> u64 {
        self.entries.last().map(|e| e.index).unwrap_or(0)
    }

    pub fn last_log_term(&self) -> u64 {
        self.entries.last().map(|e| e.term).unwrap_or(0)
    }

    pub fn get(&self, index: u64) -> Option<&LogEntry> {
        if index == 0 || index > self.entries.len() as u64 {
            None
        } else {
            self.entries.get((index - 1) as usize)
        }
    }

    pub fn append(&mut self, term: u64, command: RaftCommand) -> u64 {
        let index = self.last_log_index() + 1;
        self.entries.push(LogEntry {
            term,
            index,
            command,
        });
        index
    }

    /// Truncate log starting from given index (e.g. if leader has diverging entries)
    pub fn truncate_from(&mut self, index: u64) {
        if index > 0 && index <= self.entries.len() as u64 {
            self.entries.truncate((index - 1) as usize);
        }
    }

    pub fn slice_from(&self, from_index: u64) -> Vec<LogEntry> {
        if from_index == 0 {
            self.entries.clone()
        } else if from_index <= self.entries.len() as u64 {
            self.entries[(from_index - 1) as usize..].to_vec()
        } else {
            Vec::new()
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
