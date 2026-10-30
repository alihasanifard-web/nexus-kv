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
    pub last_included_index: u64,
    pub last_included_term: u64,
}

impl RaftLog {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            last_included_index: 0,
            last_included_term: 0,
        }
    }

    pub fn last_log_index(&self) -> u64 {
        self.entries.last().map(|e| e.index).unwrap_or(self.last_included_index)
    }

    pub fn last_log_term(&self) -> u64 {
        self.entries.last().map(|e| e.term).unwrap_or(self.last_included_term)
    }

    pub fn get(&self, index: u64) -> Option<&LogEntry> {
        if index <= self.last_included_index {
            None
        } else {
            let offset = (index - self.last_included_index - 1) as usize;
            self.entries.get(offset)
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

    /// Prune/compact all log entries up through last_included_index
    pub fn compact_up_to(&mut self, last_included_index: u64, last_included_term: u64) {
        if last_included_index <= self.last_included_index {
            return;
        }

        let retain_from_offset = if last_included_index >= self.last_log_index() {
            self.entries.len()
        } else {
            (last_included_index - self.last_included_index) as usize
        };

        if retain_from_offset <= self.entries.len() {
            self.entries.drain(0..retain_from_offset);
        } else {
            self.entries.clear();
        }

        self.last_included_index = last_included_index;
        self.last_included_term = last_included_term;
    }

    /// Truncate log starting from given index (e.g. if leader has diverging entries)
    pub fn truncate_from(&mut self, index: u64) {
        if index > self.last_included_index {
            let offset = (index - self.last_included_index - 1) as usize;
            if offset < self.entries.len() {
                self.entries.truncate(offset);
            }
        }
    }

    pub fn slice_from(&self, from_index: u64) -> Vec<LogEntry> {
        if from_index <= self.last_included_index {
            self.entries.clone()
        } else {
            let offset = (from_index - self.last_included_index - 1) as usize;
            if offset < self.entries.len() {
                self.entries[offset..].to_vec()
            } else {
                Vec::new()
            }
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
