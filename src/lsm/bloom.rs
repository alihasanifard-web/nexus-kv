// ==============================================================================
// 🌸 NEXUS-KV: BLOOM FILTER (فیلتر بلوم برای تسریع جستجوی منفی بدون دیسک)
// Architect: Ali Hasanifard
// ==============================================================================

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BloomFilter {
    bitset: Vec<u8>,
    num_bits: usize,
    num_hashes: usize,
}

impl BloomFilter {
    /// Create a new Bloom Filter with estimated items and target false positive rate
    pub fn new(estimated_items: usize, false_positive_rate: f64) -> Self {
        let items = estimated_items.max(1) as f64;
        let fp = false_positive_rate.clamp(0.0001, 0.5);

        // Optimal bits: m = - (n * ln(p)) / (ln(2)^2)
        let m = (-(items * fp.ln()) / (2.0f64.ln().powi(2))).ceil() as usize;
        let num_bits = m.max(64);

        // Optimal hash functions: k = (m / n) * ln(2)
        let k = ((num_bits as f64 / items) * 2.0f64.ln()).round() as usize;
        let num_hashes = k.clamp(1, 16);

        let byte_len = (num_bits + 7) / 8;
        Self {
            bitset: vec![0u8; byte_len],
            num_bits,
            num_hashes,
        }
    }

    /// Hash helper using double hashing (Kirsch-Mitzenmacher-Technique)
    fn get_hash_pair(key: &[u8]) -> (u64, u64) {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::Hasher;

        let mut h1 = DefaultHasher::new();
        h1.write(key);
        let hash1 = h1.finish();

        let mut h2 = DefaultHasher::new();
        h2.write(key);
        h2.write(b"nexus_bloom_salt");
        let hash2 = h2.finish();

        (hash1, hash2.rotate_left(17) ^ 0x9e3779b97f4a7c15)
    }

    /// Add a key to the bloom filter
    pub fn insert(&mut self, key: &[u8]) {
        let (h1, h2) = Self::get_hash_pair(key);
        for i in 0..self.num_hashes {
            let combined = h1.wrapping_add((i as u64).wrapping_mul(h2));
            let bit_idx = (combined as usize) % self.num_bits;
            self.bitset[bit_idx / 8] |= 1 << (bit_idx % 8);
        }
    }

    /// Check if a key might be in the set (true: might contain, false: definitely does NOT contain)
    pub fn contains(&self, key: &[u8]) -> bool {
        if self.num_bits == 0 {
            return false;
        }
        let (h1, h2) = Self::get_hash_pair(key);
        for i in 0..self.num_hashes {
            let combined = h1.wrapping_add((i as u64).wrapping_mul(h2));
            let bit_idx = (combined as usize) % self.num_bits;
            if (self.bitset[bit_idx / 8] & (1 << (bit_idx % 8))) == 0 {
                return false;
            }
        }
        true
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        bincode::serialize(self).unwrap_or_default()
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, bincode::Error> {
        bincode::deserialize(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bloom_filter_accuracy() {
        let mut filter = BloomFilter::new(1000, 0.01);
        
        for i in 0..500 {
            let key = format!("user_key_{}", i);
            filter.insert(key.as_bytes());
        }

        // Must contain all inserted keys (0 false negatives)
        for i in 0..500 {
            let key = format!("user_key_{}", i);
            assert!(filter.contains(key.as_bytes()));
        }

        // Keys not inserted should mostly return false
        let mut false_positives = 0;
        for i in 1000..1500 {
            let key = format!("unknown_key_{}", i);
            if filter.contains(key.as_bytes()) {
                false_positives += 1;
            }
        }

        // FP rate should be around 1% (allow up to 5% due to small sample size)
        assert!(false_positives <= 25, "False positives too high: {}", false_positives);
    }
}
