// ==============================================================================
// 💾 NEXUS-KV: SSTABLE (جدول رشته‌های مرتب روی دیسک با شاخص تنک و فیلتر بلوم)
// Architect: Ali Hasanifard
// ==============================================================================

use crate::lsm::bloom::BloomFilter;
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

const SST_MAGIC: &[u8; 8] = b"NEXUSSST";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockIndexEntry {
    pub key: Vec<u8>,
    pub offset: u64,
}

pub struct SSTable {
    path: PathBuf,
    bloom: BloomFilter,
    index: Vec<BlockIndexEntry>,
    file_size: u64,
}

impl SSTable {
    /// Create and write a new immutable SSTable file from sorted memtable entries
    pub fn create<P: AsRef<Path>>(path: P, entries: &[(Vec<u8>, Option<Vec<u8>>)]) -> std::io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&path)?;
        let mut writer = BufWriter::new(file);

        let mut bloom = BloomFilter::new(entries.len(), 0.01);
        let mut index = Vec::with_capacity(entries.len());

        let mut current_offset = 0u64;

        // 1. Write Data Section
        for (k, v) in entries {
            bloom.insert(k);

            index.push(BlockIndexEntry {
                key: k.clone(),
                offset: current_offset,
            });

            let is_tombstone = v.is_none() as u8;
            let val_bytes = v.as_deref().unwrap_or(b"");
            let k_len = k.len() as u32;
            let v_len = val_bytes.len() as u32;

            writer.write_all(&[is_tombstone])?;
            writer.write_all(&k_len.to_le_bytes())?;
            writer.write_all(&v_len.to_le_bytes())?;
            writer.write_all(k)?;
            writer.write_all(val_bytes)?;

            current_offset += 1 + 4 + 4 + k.len() as u64 + val_bytes.len() as u64;
        }

        // 2. Write Index Section
        let index_offset = current_offset;
        let index_bytes = bincode::serialize(&index).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        writer.write_all(&index_bytes)?;
        let index_len = index_bytes.len() as u64;

        // 3. Write Bloom Filter Section
        let bloom_offset = index_offset + index_len;
        let bloom_bytes = bloom.to_bytes();
        writer.write_all(&bloom_bytes)?;
        let bloom_len = bloom_bytes.len() as u64;

        // 4. Write Footer: [IndexOffset: 8B][IndexLen: 8B][BloomOffset: 8B][BloomLen: 8B][Magic: 8B]
        writer.write_all(&index_offset.to_le_bytes())?;
        writer.write_all(&index_len.to_le_bytes())?;
        writer.write_all(&bloom_offset.to_le_bytes())?;
        writer.write_all(&bloom_len.to_le_bytes())?;
        writer.write_all(SST_MAGIC)?;

        writer.flush()?;
        let file_size = bloom_offset + bloom_len + 8 * 4 + 8;

        Ok(Self {
            path,
            bloom,
            index,
            file_size,
        })
    }

    /// Open an existing SSTable by reading its footer, index, and bloom filter into memory
    pub fn open<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        let mut file = File::open(&path)?;
        let meta = file.metadata()?;
        let file_size = meta.len();

        if file_size < 40 {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "SSTable file too small"));
        }

        // Read footer (last 40 bytes)
        file.seek(SeekFrom::End(-40))?;
        let mut footer = [0u8; 40];
        file.read_exact(&mut footer)?;

        let magic = &footer[32..40];
        if magic != SST_MAGIC {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid SSTable magic header"));
        }

        let index_offset = u64::from_le_bytes(footer[0..8].try_into().unwrap());
        let index_len = u64::from_le_bytes(footer[8..16].try_into().unwrap());
        let bloom_offset = u64::from_le_bytes(footer[16..24].try_into().unwrap());
        let bloom_len = u64::from_le_bytes(footer[24..32].try_into().unwrap());

        // Read Index
        file.seek(SeekFrom::Start(index_offset))?;
        let mut index_bytes = vec![0u8; index_len as usize];
        file.read_exact(&mut index_bytes)?;
        let index: Vec<BlockIndexEntry> = bincode::deserialize(&index_bytes)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        // Read Bloom Filter
        file.seek(SeekFrom::Start(bloom_offset))?;
        let mut bloom_bytes = vec![0u8; bloom_len as usize];
        file.read_exact(&mut bloom_bytes)?;
        let bloom = BloomFilter::from_bytes(&bloom_bytes)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        Ok(Self {
            path,
            bloom,
            index,
            file_size,
        })
    }

    /// Fast lookup using Bloom Filter -> Binary Search Index -> Single Disk Read
    pub fn get(&self, key: &[u8]) -> std::io::Result<Option<Option<Vec<u8>>>> {
        // 1. Fast Bloom check (0 disk IO if false)
        if !self.bloom.contains(key) {
            return Ok(None);
        }

        // 2. Binary search in sparse index
        let idx = self.index.binary_search_by(|entry| entry.key.as_slice().cmp(key));
        let entry_idx = match idx {
            Ok(i) => i,
            Err(_) => return Ok(None),
        };

        // 3. Read specific data entry from disk
        let mut file = File::open(&self.path)?;
        file.seek(SeekFrom::Start(self.index[entry_idx].offset))?;

        let mut tombstone_buf = [0u8; 1];
        file.read_exact(&mut tombstone_buf)?;
        let is_tombstone = tombstone_buf[0] != 0;

        let mut klen_buf = [0u8; 4];
        let mut vlen_buf = [0u8; 4];
        file.read_exact(&mut klen_buf)?;
        file.read_exact(&mut vlen_buf)?;
        let k_len = u32::from_le_bytes(klen_buf) as usize;
        let v_len = u32::from_le_bytes(vlen_buf) as usize;

        let mut k_buf = vec![0u8; k_len];
        file.read_exact(&mut k_buf)?;

        if is_tombstone {
            Ok(Some(None))
        } else {
            let mut v_buf = vec![0u8; v_len];
            file.read_exact(&mut v_buf)?;
            Ok(Some(Some(v_buf)))
        }
    }

    /// Read all entries sequentially (used for compaction)
    pub fn read_all(&self) -> std::io::Result<Vec<(Vec<u8>, Option<Vec<u8>>)>> {
        let mut entries = Vec::new();
        let mut file = File::open(&self.path)?;

        for item in &self.index {
            file.seek(SeekFrom::Start(item.offset))?;
            let mut tombstone_buf = [0u8; 1];
            file.read_exact(&mut tombstone_buf)?;
            let is_tombstone = tombstone_buf[0] != 0;

            let mut klen_buf = [0u8; 4];
            let mut vlen_buf = [0u8; 4];
            file.read_exact(&mut klen_buf)?;
            file.read_exact(&mut vlen_buf)?;
            let k_len = u32::from_le_bytes(klen_buf) as usize;
            let v_len = u32::from_le_bytes(vlen_buf) as usize;

            let mut k = vec![0u8; k_len];
            file.read_exact(&mut k)?;

            let v = if is_tombstone {
                None
            } else {
                let mut val = vec![0u8; v_len];
                file.read_exact(&mut val)?;
                Some(val)
            };

            entries.push((k, v));
        }

        Ok(entries)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn size(&self) -> u64 {
        self.file_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_sstable_lifecycle() {
        let dir = tempdir().unwrap();
        let sst_path = dir.path().join("00001.sst");

        let entries = vec![
            (b"apple".to_vec(), Some(b"red".to_vec())),
            (b"banana".to_vec(), Some(b"yellow".to_vec())),
            (b"cherry".to_vec(), None), // Deleted tombstone
            (b"date".to_vec(), Some(b"brown".to_vec())),
        ];

        let sst = SSTable::create(&sst_path, &entries).unwrap();
        assert_eq!(sst.get(b"apple").unwrap(), Some(Some(b"red".to_vec())));
        assert_eq!(sst.get(b"cherry").unwrap(), Some(None));
        assert_eq!(sst.get(b"grape").unwrap(), None);

        // Reopen from disk
        let reopened = SSTable::open(&sst_path).unwrap();
        assert_eq!(reopened.get(b"banana").unwrap(), Some(Some(b"yellow".to_vec())));
        assert_eq!(reopened.get(b"cherry").unwrap(), Some(None));
        assert_eq!(reopened.get(b"unknown").unwrap(), None);
    }
}
