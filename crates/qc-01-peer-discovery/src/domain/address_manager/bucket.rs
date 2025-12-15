//! Address bucket implementation.

use std::collections::HashMap;

use super::config::AddressManagerConfig;
use super::security::SubnetKey;
use super::types::AddressEntry;
use crate::domain::NodeId;

/// A bucket containing address entries with subnet limits
#[derive(Debug, Clone, Default)]
pub struct AddressBucket {
    pub(crate) entries: Vec<AddressEntry>,
    /// Count of entries per subnet in this bucket
    pub(crate) subnet_counts: HashMap<SubnetKey, usize>,
}

impl AddressBucket {
    /// Create a new empty bucket
    pub fn new() -> Self {
        Self::default()
    }

    /// Get number of entries
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Check if bucket can accept an entry from given subnet
    pub fn can_accept(&self, subnet: &SubnetKey, config: &AddressManagerConfig) -> bool {
        if self.entries.len() >= config.bucket_size {
            return false;
        }
        let subnet_count = self.subnet_counts.get(subnet).copied().unwrap_or(0);
        subnet_count < config.max_per_subnet_per_bucket
    }

    /// Add an entry to the bucket
    pub fn add(&mut self, entry: AddressEntry) {
        let subnet = SubnetKey::from_ip(&entry.peer_info.socket_addr.ip);
        *self.subnet_counts.entry(subnet).or_insert(0) += 1;
        self.entries.push(entry);
    }

    /// Remove an entry by NodeId
    pub fn remove(&mut self, node_id: &NodeId) -> Option<AddressEntry> {
        let pos = self
            .entries
            .iter()
            .position(|e| &e.peer_info.node_id == node_id)?;

        let entry = self.entries.remove(pos);
        let subnet = SubnetKey::from_ip(&entry.peer_info.socket_addr.ip);

        if let Some(count) = self.subnet_counts.get_mut(&subnet) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                self.subnet_counts.remove(&subnet);
            }
        }
        Some(entry)
    }

    /// Get all entries
    pub fn entries(&self) -> &[AddressEntry] {
        &self.entries
    }

    /// Get entry at a specific index.
    pub fn get_entry(&self, index: usize) -> Option<&AddressEntry> {
        self.entries.get(index)
    }

    /// Get a random entry using an externally-provided random index.
    pub fn random_entry_at(&self, random_index: usize) -> Option<&AddressEntry> {
        if self.entries.is_empty() {
            None
        } else {
            let safe_index = random_index % self.entries.len();
            Some(&self.entries[safe_index])
        }
    }

    /// DEPRECATED: Deterministic entry selection (returns first entry).
    #[deprecated(note = "Use random_entry_at() with RandomSource for security")]
    pub fn random_entry(&self) -> Option<&AddressEntry> {
        self.entries.first()
    }

    /// Evict the oldest entry (by last_success timestamp) from the bucket.
    pub fn evict_oldest(&mut self) -> Option<AddressEntry> {
        if self.entries.is_empty() {
            return None;
        }

        let oldest_idx = self
            .entries
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                let a_time = a.last_success.map(|t| t.as_secs()).unwrap_or(0);
                let b_time = b.last_success.map(|t| t.as_secs()).unwrap_or(0);
                a_time.cmp(&b_time)
            })
            .map(|(idx, _)| idx)
            .unwrap_or(0);

        let entry = self.entries.remove(oldest_idx);
        let subnet = SubnetKey::from_ip(&entry.peer_info.socket_addr.ip);

        if let Some(count) = self.subnet_counts.get_mut(&subnet) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                self.subnet_counts.remove(&subnet);
            }
        }

        Some(entry)
    }
}
