//! ENR cache implementation.

use std::collections::HashMap;

use super::capability::CapabilityType;
use super::config::EnrConfig;
use super::record::NodeRecord;
use crate::domain::NodeId;

/// A cached ENR record with metadata
#[derive(Debug, Clone)]
pub struct CachedRecord {
    /// The record
    pub record: NodeRecord,
    /// When we received this record
    pub received_at: u64,
    /// Last time we verified the node was alive
    pub last_verified: Option<u64>,
}

/// Cache of known ENR records
#[derive(Debug)]
pub struct EnrCache {
    /// Records by Node ID
    records: HashMap<NodeId, CachedRecord>,
    /// Configuration
    config: EnrConfig,
}

impl EnrCache {
    /// Create a new cache
    pub fn new(config: EnrConfig) -> Self {
        Self {
            records: HashMap::new(),
            config,
        }
    }

    /// Insert or update a record
    pub fn insert(&mut self, record: NodeRecord, now_secs: u64) -> bool {
        if !record.verify_signature() {
            return false;
        }

        let node_id = record.node_id();

        if let Some(existing) = self.records.get(&node_id) {
            if record.seq <= existing.record.seq {
                return false;
            }
        }

        if record.capabilities.len() > self.config.max_capabilities {
            return false;
        }

        self.records.insert(
            node_id,
            CachedRecord {
                record,
                received_at: now_secs,
                last_verified: None,
            },
        );

        true
    }

    /// Get a record by Node ID
    pub fn get(&self, node_id: &NodeId) -> Option<&NodeRecord> {
        self.records.get(node_id).map(|c| &c.record)
    }

    /// Find nodes with a specific capability
    pub fn find_by_capability(&self, cap_type: CapabilityType) -> Vec<&NodeRecord> {
        self.records
            .values()
            .filter(|c| c.record.has_capability(cap_type))
            .map(|c| &c.record)
            .collect()
    }

    /// Remove stale records
    pub fn gc_stale(&mut self, now_secs: u64) -> usize {
        let max_age = self.config.max_record_age_secs;
        let before = self.records.len();

        self.records
            .retain(|_, cached| now_secs - cached.received_at < max_age);

        before - self.records.len()
    }

    /// Get number of cached records
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Mark a node as verified
    pub fn mark_verified(&mut self, node_id: &NodeId, now_secs: u64) {
        if let Some(cached) = self.records.get_mut(node_id) {
            cached.last_verified = Some(now_secs);
        }
    }
}
