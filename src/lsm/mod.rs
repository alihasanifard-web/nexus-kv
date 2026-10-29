// ==============================================================================
// 🏛️ NEXUS-KV: LSM-TREE ENGINE (موتور ذخیره‌سازی محلی بر پایه درخت LSM)
// Architect: Ali Hasanifard
// ==============================================================================

pub mod bloom;
pub mod compaction;
pub mod memtable;
pub mod sstable;
pub mod wal;

use compaction::Compactor;
use memtable::MemTable;
use parking_lot::RwLock;
use sstable::SSTable;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use wal::{WalOp, WriteAheadLog};

pub struct LsmConfig {
    pub dir: PathBuf,
    pub memtable_threshold_bytes: usize,
    pub compaction_threshold_ssts: usize,
}

impl Default for LsmConfig {
    fn default() -> Self {
        Self {
            dir: PathBuf::from("./data/lsm"),
            memtable_threshold_bytes: 4 * 1024 * 1024, // 4MB
            compaction_threshold_ssts: 4,
        }
    }
}

pub struct LsmEngine {
    pub config: LsmConfig,
    pub memtable: Arc<MemTable>,
    pub wal: Arc<RwLock<WriteAheadLog>>,
    pub sstables: Arc<RwLock<Vec<SSTable>>>, // Newest SSTable at index 0
    pub next_sst_id: AtomicU64,
}

impl LsmEngine {
    pub fn open(config: LsmConfig) -> std::io::Result<Self> {
        fs::create_dir_all(&config.dir)?;

        let wal_path = config.dir.join("current.wal");
        let recovered_entries = WriteAheadLog::recover(&wal_path)?;

        let memtable = Arc::new(MemTable::new());
        for entry in recovered_entries {
            match entry.op {
                WalOp::Put => {
                    if let Some(v) = entry.value {
                        memtable.put(entry.key, v);
                    }
                }
                WalOp::Delete => {
                    memtable.delete(entry.key);
                }
            }
        }

        let wal = Arc::new(RwLock::new(WriteAheadLog::open(&wal_path)?));

        // Load existing SSTables sorted by ID ascending
        let mut sst_files: Vec<(u64, PathBuf)> = Vec::new();
        for entry in fs::read_dir(&config.dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("sst") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if let Ok(id) = stem.parse::<u64>() {
                        sst_files.push((id, path));
                    }
                }
            }
        }
        sst_files.sort_by_key(|(id, _)| *id);

        let max_id = sst_files.last().map(|(id, _)| *id).unwrap_or(0);
        let next_sst_id = AtomicU64::new(max_id + 1);

        // In memory, we keep newest first for fast lookups
        let mut sstables = Vec::new();
        for (_, path) in sst_files.into_iter().rev() {
            if let Ok(sst) = SSTable::open(&path) {
                sstables.push(sst);
            }
        }

        Ok(Self {
            config,
            memtable,
            wal,
            sstables: Arc::new(RwLock::new(sstables)),
            next_sst_id,
        })
    }

    /// Write key-value pair
    pub fn put(&self, key: &[u8], value: &[u8]) -> std::io::Result<()> {
        // 1. Append to WAL
        {
            let mut wal = self.wal.write();
            wal.append(WalOp::Put, key, Some(value))?;
        }

        // 2. Put into MemTable
        self.memtable.put(key.to_vec(), value.to_vec());

        // 3. Trigger Flush if size exceeds threshold
        if self.memtable.size() >= self.config.memtable_threshold_bytes {
            self.flush()?;
        }

        Ok(())
    }

    /// Read key value
    pub fn get(&self, key: &[u8]) -> std::io::Result<Option<Vec<u8>>> {
        // 1. Check MemTable
        if let Some(mem_res) = self.memtable.get(key) {
            return Ok(mem_res); // If Some(None), it was deleted (tombstone)
        }

        // 2. Search SSTables (from newest to oldest)
        let ssts = self.sstables.read();
        for sst in ssts.iter() {
            if let Some(res) = sst.get(key)? {
                return Ok(res);
            }
        }

        Ok(None)
    }

    /// Delete key (writes tombstone)
    pub fn delete(&self, key: &[u8]) -> std::io::Result<()> {
        // 1. Append tombstone to WAL
        {
            let mut wal = self.wal.write();
            wal.append(WalOp::Delete, key, None)?;
        }

        // 2. Insert tombstone into MemTable
        self.memtable.delete(key.to_vec());

        if self.memtable.size() >= self.config.memtable_threshold_bytes {
            self.flush()?;
        }

        Ok(())
    }

    /// Flush MemTable to a new SSTable and reset WAL
    pub fn flush(&self) -> std::io::Result<()> {
        if self.memtable.is_empty() {
            return Ok(());
        }

        let entries = self.memtable.drain_sorted();
        if entries.is_empty() {
            return Ok(());
        }

        let sst_id = self.next_sst_id.fetch_add(1, Ordering::SeqCst);
        let sst_path = self.config.dir.join(format!("{:06}.sst", sst_id));

        let new_sst = SSTable::create(&sst_path, &entries)?;

        // Prepend new SSTable to list (index 0 is newest)
        {
            let mut ssts = self.sstables.write();
            ssts.insert(0, new_sst);
        }

        // Reset WAL
        {
            let mut wal = self.wal.write();
            wal.reset()?;
        }

        // Check if compaction is needed
        let sst_count = self.sstables.read().len();
        if sst_count >= self.config.compaction_threshold_ssts {
            self.compact()?;
        }

        Ok(())
    }

    /// Compact existing SSTables
    pub fn compact(&self) -> std::io::Result<()> {
        let mut ssts_guard = self.sstables.write();
        if ssts_guard.len() < 2 {
            return Ok(());
        }

        // Pass sstables in chronological order (oldest to newest)
        let oldest_to_newest: Vec<SSTable> = ssts_guard.iter().rev().map(|s| SSTable::open(s.path()).unwrap()).collect();

        let sst_id = self.next_sst_id.fetch_add(1, Ordering::SeqCst);
        let compacted_path = self.config.dir.join(format!("{:06}.sst", sst_id));

        if let Some(compacted) = Compactor::compact(&oldest_to_newest, &compacted_path, true)? {
            // Remove old SSTable files
            for old_sst in ssts_guard.drain(..) {
                let _ = fs::remove_file(old_sst.path());
            }
            ssts_guard.push(compacted);
        }

        Ok(())
    }

    /// Range scan over keys, merging MemTable and all SSTables (tombstones filtered out)
    pub fn scan(&self, start: Option<&[u8]>, end: Option<&[u8]>) -> std::io::Result<Vec<(Vec<u8>, Vec<u8>)>> {
        use std::collections::BTreeMap;
        let mut merged: BTreeMap<Vec<u8>, Option<Vec<u8>>> = BTreeMap::new();

        // 1. Read SSTables from oldest to newest
        let ssts = self.sstables.read();
        for sst in ssts.iter().rev() {
            let entries = sst.scan_range(start, end)?;
            for (k, v) in entries {
                merged.insert(k, v);
            }
        }

        // 2. Overlay MemTable (newest mutations)
        let mem_entries = self.memtable.scan_range(start, end);
        for (k, v) in mem_entries {
            merged.insert(k, v);
        }

        // 3. Filter out tombstones
        let active = merged.into_iter().filter_map(|(k, v)| v.map(|val| (k, val))).collect();
        Ok(active)
    }

    /// Match keys by wildcard pattern (e.g. "user:*" or "*")
    pub fn keys(&self, pattern: &str) -> std::io::Result<Vec<Vec<u8>>> {
        let all = self.scan(None, None)?;
        let is_wildcard_all = pattern == "*";
        let prefix = if pattern.ends_with('*') {
            &pattern[..pattern.len() - 1]
        } else {
            pattern
        };

        let matches = all.into_iter()
            .map(|(k, _)| k)
            .filter(|k| {
                if is_wildcard_all {
                    return true;
                }
                let k_str = String::from_utf8_lossy(k);
                if pattern.ends_with('*') {
                    k_str.starts_with(prefix)
                } else {
                    k_str == pattern
                }
            })
            .collect();

        Ok(matches)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_lsm_full_lifecycle_and_recovery() {
        let dir = tempdir().unwrap();
        let config = LsmConfig {
            dir: dir.path().to_path_buf(),
            memtable_threshold_bytes: 256, // small for test flushing
            compaction_threshold_ssts: 3,
        };

        {
            let engine = LsmEngine::open(config).unwrap();
            engine.put(b"name", b"Ali Hasanifard").unwrap();
            engine.put(b"role", b"Lead Systems Architect").unwrap();
            engine.put(b"engine", b"NexusKV").unwrap();

            assert_eq!(engine.get(b"name").unwrap(), Some(b"Ali Hasanifard".to_vec()));
            assert_eq!(engine.get(b"role").unwrap(), Some(b"Lead Systems Architect".to_vec()));

            // Trigger flush
            engine.flush().unwrap();
            assert_eq!(engine.get(b"name").unwrap(), Some(b"Ali Hasanifard".to_vec()));

            // Delete key
            engine.delete(b"role").unwrap();
            assert_eq!(engine.get(b"role").unwrap(), None);
        }

        // Crash and Reopen
        let reopen_config = LsmConfig {
            dir: dir.path().to_path_buf(),
            memtable_threshold_bytes: 256,
            compaction_threshold_ssts: 3,
        };
        let reopened = LsmEngine::open(reopen_config).unwrap();
        assert_eq!(reopened.get(b"name").unwrap(), Some(b"Ali Hasanifard".to_vec()));
        assert_eq!(reopened.get(b"role").unwrap(), None); // Still deleted!
        assert_eq!(reopened.get(b"engine").unwrap(), Some(b"NexusKV".to_vec()));
    }
}
