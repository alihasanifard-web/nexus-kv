// ==============================================================================
// 📜 NEXUS-KV: WRITE-AHEAD LOG (لاگ پیش‌از‌نوشتن با اعتبارسنجی CRC32)
// Architect: Ali Hasanifard
// ==============================================================================

use crc32fast::Hasher;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalOp {
    Put = 1,
    Delete = 2,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalEntry {
    pub op: WalOp,
    pub key: Vec<u8>,
    pub value: Option<Vec<u8>>,
}

pub struct WriteAheadLog {
    path: PathBuf,
    writer: BufWriter<File>,
}

impl WriteAheadLog {
    pub fn open<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .append(true)
            .open(&path)?;
        
        Ok(Self {
            path,
            writer: BufWriter::new(file),
        })
    }

    /// Append a mutation entry to WAL with CRC32 checksum
    pub fn append(&mut self, op: WalOp, key: &[u8], value: Option<&[u8]>) -> std::io::Result<()> {
        let op_byte = op as u8;
        let key_len = key.len() as u32;
        let val_len = value.map(|v| v.len() as u32).unwrap_or(0);

        let mut hasher = Hasher::new();
        hasher.update(&[op_byte]);
        hasher.update(&key_len.to_le_bytes());
        hasher.update(&val_len.to_le_bytes());
        hasher.update(key);
        if let Some(v) = value {
            hasher.update(v);
        }
        let checksum = hasher.finalize();

        // Binary frame layout:
        // [Checksum: 4B][Op: 1B][KeyLen: 4B][ValLen: 4B][Key][Value]
        self.writer.write_all(&checksum.to_le_bytes())?;
        self.writer.write_all(&[op_byte])?;
        self.writer.write_all(&key_len.to_le_bytes())?;
        self.writer.write_all(&val_len.to_le_bytes())?;
        self.writer.write_all(key)?;
        if let Some(v) = value {
            self.writer.write_all(v)?;
        }
        self.writer.flush()?;
        Ok(())
    }

    /// Recover all valid committed records from WAL
    pub fn recover<P: AsRef<Path>>(path: P) -> std::io::Result<Vec<WalEntry>> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        let mut entries = Vec::new();

        loop {
            let mut crc_buf = [0u8; 4];
            if reader.read_exact(&mut crc_buf).is_err() {
                break; // EOF
            }
            let expected_crc = u32::from_le_bytes(crc_buf);

            let mut op_buf = [0u8; 1];
            if reader.read_exact(&mut op_buf).is_err() {
                break;
            }
            let op = match op_buf[0] {
                1 => WalOp::Put,
                2 => WalOp::Delete,
                _ => break, // Corrupt entry
            };

            let mut klen_buf = [0u8; 4];
            let mut vlen_buf = [0u8; 4];
            if reader.read_exact(&mut klen_buf).is_err() || reader.read_exact(&mut vlen_buf).is_err() {
                break;
            }
            let key_len = u32::from_le_bytes(klen_buf) as usize;
            let val_len = u32::from_le_bytes(vlen_buf) as usize;

            let mut key = vec![0u8; key_len];
            if reader.read_exact(&mut key).is_err() {
                break;
            }

            let value = if op == WalOp::Put {
                let mut val = vec![0u8; val_len];
                if reader.read_exact(&mut val).is_err() {
                    break;
                }
                Some(val)
            } else {
                None
            };

            // Verify checksum
            let mut hasher = Hasher::new();
            hasher.update(&[op as u8]);
            hasher.update(&(key_len as u32).to_le_bytes());
            hasher.update(&(val_len as u32).to_le_bytes());
            hasher.update(&key);
            if let Some(ref v) = value {
                hasher.update(v);
            }
            if hasher.finalize() != expected_crc {
                eprintln!("⚠ Corrupt WAL frame detected. Replay truncated cleanly.");
                break;
            }

            entries.push(WalEntry { op, key, value });
        }

        Ok(entries)
    }

    /// Truncate WAL after successful memtable flush
    pub fn reset(&mut self) -> std::io::Result<()> {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.path)?;
        self.writer = BufWriter::new(file);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_wal_append_and_recovery() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();

        {
            let mut wal = WriteAheadLog::open(&path).unwrap();
            wal.append(WalOp::Put, b"user:101", Some(b"Alice")).unwrap();
            wal.append(WalOp::Put, b"user:102", Some(b"Bob")).unwrap();
            wal.append(WalOp::Delete, b"user:101", None).unwrap();
        }

        // Replay
        let recovered = WriteAheadLog::recover(&path).unwrap();
        assert_eq!(recovered.len(), 3);
        assert_eq!(recovered[0].key, b"user:101");
        assert_eq!(recovered[0].value, Some(b"Alice".to_vec()));
        assert_eq!(recovered[1].key, b"user:102");
        assert_eq!(recovered[2].op, WalOp::Delete);
        assert_eq!(recovered[2].key, b"user:101");
        assert_eq!(recovered[2].value, None);
    }
}
