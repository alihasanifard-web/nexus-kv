// ==============================================================================
// 🧹 NEXUS-KV: COMPACTION ENGINE (فشرده‌سازی و ادغام جداول SSTable برای بازیافت فضا)
// Architect: Ali Hasanifard
// ==============================================================================

use crate::lsm::sstable::SSTable;
use std::collections::BTreeMap;
use std::path::Path;

pub struct Compactor;

impl Compactor {
    /// Merge multiple SSTables into a single consolidated SSTable
    /// Older SSTables come first; newer SSTables overwrite older values.
    pub fn compact<P: AsRef<Path>>(
        sstables: &[SSTable],
        output_path: P,
        purge_tombstones: bool,
    ) -> std::io::Result<Option<SSTable>> {
        if sstables.is_empty() {
            return Ok(None);
        }

        let mut merged_map: BTreeMap<Vec<u8>, Option<Vec<u8>>> = BTreeMap::new();

        // Iterate through sstables from oldest to newest
        for sst in sstables {
            let entries = sst.read_all()?;
            for (k, v) in entries {
                merged_map.insert(k, v);
            }
        }

        // If purging tombstones (e.g. major compaction), drop None values
        let final_entries: Vec<(Vec<u8>, Option<Vec<u8>>)> = if purge_tombstones {
            merged_map.into_iter().filter(|(_, v)| v.is_some()).collect()
        } else {
            merged_map.into_iter().collect()
        };

        if final_entries.is_empty() {
            // All entries were deleted tombstones
            return Ok(None);
        }

        let compacted_sst = SSTable::create(output_path, &final_entries)?;
        Ok(Some(compacted_sst))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_compaction_merges_and_purges() {
        let dir = tempdir().unwrap();
        let sst1_path = dir.path().join("001.sst");
        let sst2_path = dir.path().join("002.sst");
        let out_path = dir.path().join("compacted.sst");

        // Old SSTable
        let entries1 = vec![
            (b"k1".to_vec(), Some(b"v1_old".to_vec())),
            (b"k2".to_vec(), Some(b"v2".to_vec())),
            (b"k3".to_vec(), Some(b"v3".to_vec())),
        ];
        let sst1 = SSTable::create(&sst1_path, &entries1).unwrap();

        // New SSTable (overwrites k1, deletes k2)
        let entries2 = vec![
            (b"k1".to_vec(), Some(b"v1_new".to_vec())),
            (b"k2".to_vec(), None),
            (b"k4".to_vec(), Some(b"v4".to_vec())),
        ];
        let sst2 = SSTable::create(&sst2_path, &entries2).unwrap();

        // Compact with purge_tombstones = true
        let compacted = Compactor::compact(&[sst1, sst2], &out_path, true).unwrap().unwrap();
        
        assert_eq!(compacted.get(b"k1").unwrap(), Some(Some(b"v1_new".to_vec())));
        assert_eq!(compacted.get(b"k2").unwrap(), None); // Purged!
        assert_eq!(compacted.get(b"k3").unwrap(), Some(Some(b"v3".to_vec())));
        assert_eq!(compacted.get(b"k4").unwrap(), Some(Some(b"v4".to_vec())));
    }
}
