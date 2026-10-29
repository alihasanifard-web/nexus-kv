// ==============================================================================
// 🌐 NEXUS-KV: MULTI-RAFT & DYNAMIC SHARDING (معماری چندگروهه و افراز داده‌ها)
// Architect: Ali Hasanifard
// ==============================================================================

use crate::raft::message::NodeId;
use parking_lot::RwLock;
use std::collections::BTreeMap;

pub type RegionId = u64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Region {
    pub id: RegionId,
    pub start_key: Vec<u8>,
    pub end_key: Vec<u8>, // empty means unbounded infinity
    pub peers: Vec<NodeId>,
    pub leader: Option<NodeId>,
}

pub struct ShardRouter {
    // start_key -> Region
    regions: RwLock<BTreeMap<Vec<u8>, Region>>,
}

impl ShardRouter {
    pub fn new() -> Self {
        let mut regions = BTreeMap::new();
        // Default initial region covering whole universe ["" .. infinity)
        regions.insert(Vec::new(), Region {
            id: 1,
            start_key: Vec::new(),
            end_key: Vec::new(),
            peers: vec![1, 2, 3],
            leader: Some(1),
        });

        Self {
            regions: RwLock::new(regions),
        }
    }

    /// Locate the region responsible for a given key
    pub fn locate_region(&self, key: &[u8]) -> Option<Region> {
        let regions = self.regions.read();
        // Find the region with the largest start_key <= key
        let mut target = None;
        for (start, region) in regions.iter() {
            if start.as_slice() <= key {
                if region.end_key.is_empty() || key < region.end_key.as_slice() {
                    target = Some(region.clone());
                    break;
                }
            }
        }
        target
    }

    /// Split a region into two halves at split_key (Multi-Raft split)
    pub fn split_region(&self, region_id: RegionId, split_key: Vec<u8>, new_region_id: RegionId) -> Result<(), String> {
        let mut regions = self.regions.write();
        
        let mut found_start = None;
        for (start, reg) in regions.iter() {
            if reg.id == region_id {
                found_start = Some(start.clone());
                break;
            }
        }

        let start_key = found_start.ok_or_else(|| format!("Region {} not found", region_id))?;
        let old_region = regions.remove(&start_key).unwrap();

        // Left region: [old_start .. split_key)
        let left = Region {
            id: old_region.id,
            start_key: old_region.start_key.clone(),
            end_key: split_key.clone(),
            peers: old_region.peers.clone(),
            leader: old_region.leader,
        };

        // Right region: [split_key .. old_end)
        let right = Region {
            id: new_region_id,
            start_key: split_key.clone(),
            end_key: old_region.end_key,
            peers: old_region.peers,
            leader: old_region.leader,
        };

        regions.insert(left.start_key.clone(), left);
        regions.insert(right.start_key.clone(), right);
        Ok(())
    }

    pub fn region_count(&self) -> usize {
        self.regions.read().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dynamic_sharding_and_region_split() {
        let router = ShardRouter::new();
        assert_eq!(router.region_count(), 1);

        // Initial key routes to region 1
        let reg1 = router.locate_region(b"user:100").unwrap();
        assert_eq!(reg1.id, 1);

        // Split at "m"
        router.split_region(1, b"m".to_vec(), 2).unwrap();
        assert_eq!(router.region_count(), 2);

        // Keys before "m" route to region 1
        let reg_apple = router.locate_region(b"apple").unwrap();
        assert_eq!(reg_apple.id, 1);

        // Keys after "m" route to region 2
        let reg_zebra = router.locate_region(b"zebra").unwrap();
        assert_eq!(reg_zebra.id, 2);
    }
}
