//! # Cuckoo Filter
//!
//! Space-efficient probabilistic data structure supporting deletion.
//!
//! ## Advantages over Bloom Filters
//!
//! | Feature | Bloom | Cuckoo |
//! |---------|-------|--------|
//! | Deletion | ❌ No | ✅ Yes |
//! | Space efficiency | Lower | Higher at low FPR |
//! | False positive rate | Fixed | Configurable |
//!
//! ## Use Cases
//!
//! - Anti-replay caches (with expiration)
//! - IP blacklists (dynamic add/remove)
//! - Transaction deduplication

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Default number of buckets.
pub const DEFAULT_BUCKET_COUNT: usize = 1024;

/// Default entries per bucket.
pub const ENTRIES_PER_BUCKET: usize = 4;

/// Fingerprint size in bits.
pub const FINGERPRINT_SIZE: usize = 16;

/// Maximum number of kicks before giving up.
const MAX_KICKS: usize = 500;

/// Fingerprint stored in each slot.
pub type Fingerprint = u16;

/// A bucket containing multiple fingerprints.
#[derive(Clone, Debug)]
pub struct Bucket {
    entries: [Fingerprint; ENTRIES_PER_BUCKET],
}

impl Default for Bucket {
    fn default() -> Self {
        Self {
            entries: [0; ENTRIES_PER_BUCKET],
        }
    }
}

impl Bucket {
    /// Insert fingerprint if there's an empty slot.
    pub fn insert(&mut self, fp: Fingerprint) -> bool {
        for entry in &mut self.entries {
            if *entry == 0 {
                *entry = fp;
                return true;
            }
        }
        false
    }

    /// Check if fingerprint exists.
    pub fn contains(&self, fp: Fingerprint) -> bool {
        self.entries.iter().any(|&e| e == fp)
    }

    /// Delete fingerprint if it exists.
    pub fn delete(&mut self, fp: Fingerprint) -> bool {
        for entry in &mut self.entries {
            if *entry == fp {
                *entry = 0;
                return true;
            }
        }
        false
    }

    /// Swap a fingerprint with an existing one.
    pub fn swap(&mut self, fp: Fingerprint) -> Fingerprint {
        let idx = rand::random::<usize>() % ENTRIES_PER_BUCKET;
        let old = self.entries[idx];
        self.entries[idx] = fp;
        old
    }
}

/// Cuckoo filter for probabilistic membership testing with deletion.
#[derive(Clone, Debug)]
pub struct CuckooFilter {
    buckets: Vec<Bucket>,
    bucket_count: usize,
    count: usize,
}

impl CuckooFilter {
    /// Create a new cuckoo filter with specified capacity.
    pub fn new(capacity: usize) -> Self {
        let bucket_count = (capacity + ENTRIES_PER_BUCKET - 1) / ENTRIES_PER_BUCKET;
        let bucket_count = bucket_count.next_power_of_two().max(4);

        Self {
            buckets: vec![Bucket::default(); bucket_count],
            bucket_count,
            count: 0,
        }
    }

    /// Create with default capacity.
    pub fn with_default_capacity() -> Self {
        Self::new(DEFAULT_BUCKET_COUNT * ENTRIES_PER_BUCKET)
    }

    /// Insert an item.
    ///
    /// Returns `true` if inserted, `false` if filter is full.
    pub fn insert<T: Hash>(&mut self, item: &T) -> bool {
        let (fp, i1, i2) = self.indices(item);

        // Ensure fingerprint is non-zero
        let fp = if fp == 0 { 1 } else { fp };

        // Try first bucket
        if self.buckets[i1].insert(fp) {
            self.count += 1;
            return true;
        }

        // Try second bucket
        if self.buckets[i2].insert(fp) {
            self.count += 1;
            return true;
        }

        // Both full, need to kick
        self.relocate(fp, i1)
    }

    /// Relocate fingerprints to make room.
    fn relocate(&mut self, mut fp: Fingerprint, mut idx: usize) -> bool {
        for _ in 0..MAX_KICKS {
            fp = self.buckets[idx].swap(fp);
            idx = self.alt_index(idx, fp);

            if self.buckets[idx].insert(fp) {
                self.count += 1;
                return true;
            }
        }
        false // Filter is full
    }

    /// Check if item might be in the filter.
    pub fn contains<T: Hash>(&self, item: &T) -> bool {
        let (fp, i1, i2) = self.indices(item);
        let fp = if fp == 0 { 1 } else { fp };

        self.buckets[i1].contains(fp) || self.buckets[i2].contains(fp)
    }

    /// Delete an item from the filter.
    ///
    /// Returns `true` if deleted, `false` if not found.
    pub fn delete<T: Hash>(&mut self, item: &T) -> bool {
        let (fp, i1, i2) = self.indices(item);
        let fp = if fp == 0 { 1 } else { fp };

        if self.buckets[i1].delete(fp) {
            self.count = self.count.saturating_sub(1);
            return true;
        }

        if self.buckets[i2].delete(fp) {
            self.count = self.count.saturating_sub(1);
            return true;
        }

        false
    }

    /// Get number of items in filter.
    pub fn len(&self) -> usize {
        self.count
    }

    /// Check if filter is empty.
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Get load factor.
    pub fn load_factor(&self) -> f64 {
        self.count as f64 / (self.bucket_count * ENTRIES_PER_BUCKET) as f64
    }

    /// Calculate fingerprint and two bucket indices.
    fn indices<T: Hash>(&self, item: &T) -> (Fingerprint, usize, usize) {
        let hash = self.hash(item);
        let fp = (hash >> 48) as Fingerprint;
        let i1 = (hash as usize) % self.bucket_count;
        let i2 = self.alt_index(i1, fp);
        (fp, i1, i2)
    }

    /// Calculate alternate index using partial-key cuckoo hashing.
    fn alt_index(&self, idx: usize, fp: Fingerprint) -> usize {
        let fp_hash = self.hash(&fp) as usize;
        (idx ^ fp_hash) % self.bucket_count
    }

    /// Hash an item.
    fn hash<T: Hash>(&self, item: &T) -> u64 {
        let mut hasher = DefaultHasher::new();
        item.hash(&mut hasher);
        hasher.finish()
    }
}

impl Default for CuckooFilter {
    fn default() -> Self {
        Self::with_default_capacity()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_contains() {
        let mut filter = CuckooFilter::new(100);

        assert!(filter.insert(&"hello"));
        assert!(filter.contains(&"hello"));
        assert!(!filter.contains(&"world"));
    }

    #[test]
    fn test_delete() {
        let mut filter = CuckooFilter::new(100);

        filter.insert(&"item1");
        assert!(filter.contains(&"item1"));

        filter.delete(&"item1");
        assert!(!filter.contains(&"item1"));
    }

    #[test]
    fn test_multiple_items() {
        let mut filter = CuckooFilter::new(1000);

        for i in 0..100 {
            assert!(filter.insert(&i));
        }

        for i in 0..100 {
            assert!(filter.contains(&i));
        }

        assert_eq!(filter.len(), 100);
    }

    #[test]
    fn test_false_positive_rate() {
        let mut filter = CuckooFilter::new(10000);

        // Insert 1000 items
        for i in 0..1000 {
            filter.insert(&i);
        }

        // Check false positives for items not inserted
        let mut false_positives = 0;
        for i in 1000..2000 {
            if filter.contains(&i) {
                false_positives += 1;
            }
        }

        // FPR should be low (< 5%)
        let fpr = false_positives as f64 / 1000.0;
        assert!(fpr < 0.05, "FPR too high: {}", fpr);
    }

    #[test]
    fn test_load_factor() {
        let mut filter = CuckooFilter::new(100);

        for i in 0..50 {
            filter.insert(&i);
        }

        let lf = filter.load_factor();
        assert!(lf > 0.0 && lf <= 1.0);
    }
}
