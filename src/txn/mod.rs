// ==============================================================================
// 💳 NEXUS-KV: DISTRIBUTED ACID TRANSACTIONS (پروتکل دو فازی 2PC و مدل Percolator)
// Architect: Ali Hasanifard
// ==============================================================================

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

pub static GLOBAL_HLC: AtomicU64 = AtomicU64::new(1000);

/// Hybrid Logical Clock timestamp generator
pub fn next_timestamp() -> u64 {
    GLOBAL_HLC.fetch_add(1, Ordering::SeqCst)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TxnStatus {
    Pending,
    Committed,
    RolledBack,
}

#[derive(Debug, Clone)]
pub struct TxnMutation {
    pub key: Vec<u8>,
    pub value: Option<Vec<u8>>, // None = Delete
}

pub struct Transaction {
    pub start_ts: u64,
    pub commit_ts: Option<u64>,
    pub mutations: HashMap<Vec<u8>, Option<Vec<u8>>>,
    pub status: TxnStatus,
}

impl Transaction {
    pub fn begin() -> Self {
        Self {
            start_ts: next_timestamp(),
            commit_ts: None,
            mutations: HashMap::new(),
            status: TxnStatus::Pending,
        }
    }

    pub fn set(&mut self, key: Vec<u8>, value: Vec<u8>) {
        self.mutations.insert(key, Some(value));
    }

    pub fn delete(&mut self, key: Vec<u8>) {
        self.mutations.insert(key, None);
    }
}

/// Two-Phase Commit Transaction Coordinator
pub struct TxnCoordinator {
    active_locks: parking_lot::RwLock<HashMap<Vec<u8>, u64>>, // key -> locked_by_start_ts
}

impl TxnCoordinator {
    pub fn new() -> Self {
        Self {
            active_locks: parking_lot::RwLock::new(HashMap::new()),
        }
    }

    /// Phase 1: Prewrite (Acquire locks and check write-write conflicts)
    pub fn prewrite(&self, txn: &Transaction) -> Result<(), String> {
        let mut locks = self.active_locks.write();

        // Check if any key is already locked by another transaction
        for (k, _) in &txn.mutations {
            if let Some(&holder_ts) = locks.get(k) {
                if holder_ts != txn.start_ts {
                    return Err(format!("Write-Write conflict on key: already locked by txn {}", holder_ts));
                }
            }
        }

        // Acquire locks
        for (k, _) in &txn.mutations {
            locks.insert(k.clone(), txn.start_ts);
        }

        Ok(())
    }

    /// Phase 2: Commit (Assign commit timestamp and release locks)
    pub fn commit(&self, txn: &mut Transaction) -> Result<u64, String> {
        if txn.status != TxnStatus::Pending {
            return Err("Transaction is not pending".to_string());
        }

        let commit_ts = next_timestamp();
        txn.commit_ts = Some(commit_ts);
        txn.status = TxnStatus::Committed;

        // Release locks
        let mut locks = self.active_locks.write();
        for (k, _) in &txn.mutations {
            if let Some(&holder_ts) = locks.get(k) {
                if holder_ts == txn.start_ts {
                    locks.remove(k);
                }
            }
        }

        Ok(commit_ts)
    }

    /// Rollback: abort and release all locks
    pub fn rollback(&self, txn: &mut Transaction) {
        txn.status = TxnStatus::RolledBack;
        let mut locks = self.active_locks.write();
        for (k, _) in &txn.mutations {
            if let Some(&holder_ts) = locks.get(k) {
                if holder_ts == txn.start_ts {
                    locks.remove(k);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_2pc_acid_transaction_lifecycle() {
        let coordinator = TxnCoordinator::new();

        let mut txn1 = Transaction::begin();
        txn1.set(b"account:alice".to_vec(), b"900".to_vec());
        txn1.set(b"account:bob".to_vec(), b"1100".to_vec());

        // Prewrite Phase 1
        assert!(coordinator.prewrite(&txn1).is_ok());

        // Conflicting transaction trying to modify alice's account concurrently
        let mut txn2 = Transaction::begin();
        txn2.set(b"account:alice".to_vec(), b"850".to_vec());
        let conflict = coordinator.prewrite(&txn2);
        assert!(conflict.is_err()); // Conflict detected and blocked!

        // Commit Phase 2
        let commit_ts = coordinator.commit(&mut txn1).unwrap();
        assert!(commit_ts > txn1.start_ts);
        assert_eq!(txn1.status, TxnStatus::Committed);

        // Now txn2 can acquire lock
        assert!(coordinator.prewrite(&txn2).is_ok());
        coordinator.rollback(&mut txn2);
        assert_eq!(txn2.status, TxnStatus::RolledBack);
    }
}
