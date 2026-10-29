// ==============================================================================
// 🧠 NEXUS-KV: MEMTABLE (حافظه موقت مرتب برای دسترسی فوق‌سریع بدون دیسک)
// Architect: Ali Hasanifard
// ==============================================================================

use parking_lot::RwLock;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct MemTable {
    map: RwLock<BTreeMap<Vec<u8>, Option<Vec<u8>>>>,
    approximate_size: AtomicUsize,
}

impl MemTable {
    pub fn new() -> Self {
        Self {
            map: RwLock::new(BTreeMap::new()),
            approximate_size: AtomicUsize::new(0),
        }
    }

    /// Put key-value into memtable
    pub fn put(&self, key: Vec<u8>, value: Vec<u8>) {
        let delta = key.len() + value.len();
        let mut map = self.map.write();
        if let Some(old) = map.insert(key, Some(value)) {
            if let Some(old_val) = old {
                self.approximate_size.fetch_sub(old_val.len(), Ordering::Relaxed);
            }
        }
        self.approximate_size.fetch_add(delta, Ordering::Relaxed);
    }

    /// Mark key as deleted with tombstone (None)
    pub fn delete(&self, key: Vec<u8>) {
        let delta = key.len();
        let mut map = self.map.write();
        if let Some(old) = map.insert(key, None) {
            if let Some(old_val) = old {
                self.approximate_size.fetch_sub(old_val.len(), Ordering::Relaxed);
            }
        }
        self.approximate_size.fetch_add(delta, Ordering::Relaxed);
    }

    /// Get value for key. Returns:
    /// - Some(Some(val)) if present
    /// - Some(None) if deleted (tombstone)
    /// - None if not found in this memtable
    pub fn get(&self, key: &[u8]) -> Option<Option<Vec<u8>>> {
        let map = self.map.read();
        map.get(key).cloned()
    }

    /// Get current approximate size in bytes
    pub fn size(&self) -> usize {
        self.approximate_size.load(Ordering::Relaxed)
    }

    /// Drain all entries in sorted order (used for flushing to SSTable)
    pub fn drain_sorted(&self) -> Vec<(Vec<u8>, Option<Vec<u8>>)> {
        let mut map = self.map.write();
        let entries: Vec<_> = map.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        map.clear();
        self.approximate_size.store(0, Ordering::Relaxed);
        entries
    }

    /// Snapshot copy of all entries in sorted order
    pub fn snapshot(&self) -> Vec<(Vec<u8>, Option<Vec<u8>>)> {
        let map = self.map.read();
        map.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    }

    pub fn len(&self) -> usize {
        self.map.read().len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.read().is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memtable_crud() {
        let mem = MemTable::new();
        assert_eq!(mem.get(b"k1"), None);

        mem.put(b"k1".to_vec(), b"v1".to_vec());
        assert_eq!(mem.get(b"k1"), Some(Some(b"v1".to_vec())));

        mem.delete(b"k1".to_vec());
        assert_eq!(mem.get(b"k1"), Some(None)); // Tombstone

        assert!(mem.size() > 0);
        let drained = mem.drain_sorted();
        assert_eq!(drained.len(), 1);
        assert_eq!(mem.size(), 0);
    }
}
