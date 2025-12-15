//! Address table implementation.

use std::collections::HashMap;

use super::bucket::AddressBucket;
use super::security::SubnetKey;
use super::types::AddressEntry;
use crate::domain::NodeId;

/// A table of buckets (either New or Tried)
#[derive(Debug)]
pub struct AddressTable {
    pub(crate) buckets: Vec<AddressBucket>,
    /// Total entries per subnet across all buckets
    pub(crate) subnet_totals: HashMap<SubnetKey, usize>,
    /// Quick lookup: NodeId -> bucket index
    pub(crate) node_to_bucket: HashMap<NodeId, usize>,
}

impl AddressTable {
    /// Create a new table with specified bucket count
    pub fn new(bucket_count: usize) -> Self {
        Self {
            buckets: (0..bucket_count).map(|_| AddressBucket::new()).collect(),
            subnet_totals: HashMap::new(),
            node_to_bucket: HashMap::new(),
        }
    }

    /// Get total entry count
    pub fn len(&self) -> usize {
        self.buckets.iter().map(|b| b.len()).sum()
    }

    /// Check if empty using short-circuit optimization.
    pub fn is_empty(&self) -> bool {
        !self.buckets.iter().any(|b| !b.is_empty())
    }

    /// Check if table contains a node
    pub fn contains(&self, node_id: &NodeId) -> bool {
        self.node_to_bucket.contains_key(node_id)
    }

    /// Get a random entry from the table using proper randomness.
    pub fn random_entry_with<F>(&self, mut random_fn: F) -> Option<&AddressEntry>
    where
        F: FnMut(usize) -> usize,
    {
        let total = self.len();
        if total == 0 {
            return None;
        }

        let random_global_idx = random_fn(total);

        let mut remaining = random_global_idx;
        for bucket in &self.buckets {
            let bucket_len = bucket.len();
            if remaining < bucket_len {
                return bucket.get_entry(remaining);
            }
            remaining -= bucket_len;
        }

        None
    }

    /// DEPRECATED: Get a random entry deterministically.
    #[deprecated(note = "Use random_entry_with() with RandomSource for security")]
    #[allow(deprecated)]
    pub fn random_entry(&self) -> Option<&AddressEntry> {
        self.buckets.iter().find_map(|bucket| bucket.random_entry())
    }
}
