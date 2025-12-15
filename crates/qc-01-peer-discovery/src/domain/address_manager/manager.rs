//! Main AddressManager implementation.
//!
//! Reference: Bitcoin Core's `addrman.h`

use super::config::AddressManagerConfig;
use super::security::{secure_bucket_hash, AddressManagerError, SubnetKey};
use super::table::AddressTable;
use super::types::{AddressEntry, AddressManagerStats};
use crate::domain::{IpAddr, NodeId, PeerInfo, SocketAddr, Timestamp};

/// Address manager with New/Tried segregation
///
/// # Anti-Eclipse Defense
///
/// 1. New addresses go to New table, bucketed by source+address subnet
/// 2. Only after successful connection do addresses move to Tried table
/// 3. Per-subnet limits prevent flooding from single IP range
#[derive(Debug)]
pub struct AddressManager {
    /// Addresses we've heard about but never connected to
    new_table: AddressTable,
    /// Addresses we've successfully connected to
    tried_table: AddressTable,
    /// Configuration
    config: AddressManagerConfig,
}

impl AddressManager {
    /// Create a new address manager
    pub fn new(config: AddressManagerConfig) -> Self {
        Self {
            new_table: AddressTable::new(config.new_bucket_count),
            tried_table: AddressTable::new(config.tried_bucket_count),
            config,
        }
    }

    /// Add a new address learned from a peer
    pub fn add_new(
        &mut self,
        peer_info: PeerInfo,
        source_ip: &IpAddr,
        now: Timestamp,
    ) -> Result<bool, AddressManagerError> {
        let node_id = peer_info.node_id;

        if self.tried_table.contains(&node_id) || self.new_table.contains(&node_id) {
            return Ok(false);
        }

        let source_subnet = SubnetKey::from_ip(source_ip);
        let addr_subnet = SubnetKey::from_ip(&peer_info.socket_addr.ip);

        let total_count = self
            .new_table
            .subnet_totals
            .get(&addr_subnet)
            .copied()
            .unwrap_or(0)
            + self
                .tried_table
                .subnet_totals
                .get(&addr_subnet)
                .copied()
                .unwrap_or(0);
        if total_count >= self.config.max_per_subnet_total {
            return Ok(false);
        }

        let bucket_idx = self.calculate_new_bucket(&source_subnet, &addr_subnet);
        let bucket = &mut self.new_table.buckets[bucket_idx];

        if !bucket.can_accept(&addr_subnet, &self.config) {
            return Ok(false);
        }

        let entry = AddressEntry::new(peer_info, now, source_subnet);
        bucket.add(entry);
        *self.new_table.subnet_totals.entry(addr_subnet).or_insert(0) += 1;
        self.new_table.node_to_bucket.insert(node_id, bucket_idx);

        Ok(true)
    }

    /// Promote an address from New to Tried after successful connection
    pub fn promote_to_tried(
        &mut self,
        node_id: &NodeId,
        now: Timestamp,
    ) -> Result<bool, AddressManagerError> {
        let bucket_idx = match self.new_table.node_to_bucket.get(node_id) {
            Some(&idx) => idx,
            None => return Ok(false),
        };

        let bucket = &mut self.new_table.buckets[bucket_idx];
        let mut entry = match bucket.remove(node_id) {
            Some(e) => e,
            None => return Ok(false),
        };

        entry.last_success = Some(now);
        entry.attempts += 1;

        let addr_subnet = SubnetKey::from_ip(&entry.peer_info.socket_addr.ip);
        if let Some(count) = self.new_table.subnet_totals.get_mut(&addr_subnet) {
            *count = count.saturating_sub(1);
        }
        self.new_table.node_to_bucket.remove(node_id);

        let tried_bucket_idx = self.calculate_tried_bucket(&entry.peer_info.socket_addr);
        let tried_bucket = &mut self.tried_table.buckets[tried_bucket_idx];

        if !tried_bucket.can_accept(&addr_subnet, &self.config) {
            let Some(evicted) = tried_bucket.evict_oldest() else {
                return Ok(false);
            };
            let evicted_subnet = SubnetKey::from_ip(&evicted.peer_info.socket_addr.ip);
            if let Some(count) = self.tried_table.subnet_totals.get_mut(&evicted_subnet) {
                *count = count.saturating_sub(1);
            }
            self.tried_table
                .node_to_bucket
                .remove(&evicted.peer_info.node_id);
        }

        tried_bucket.add(entry);
        *self
            .tried_table
            .subnet_totals
            .entry(addr_subnet)
            .or_insert(0) += 1;
        self.tried_table
            .node_to_bucket
            .insert(*node_id, tried_bucket_idx);

        Ok(true)
    }

    /// Get a random address from the New table.
    #[allow(deprecated)]
    pub fn random_new_address(&self) -> Option<&AddressEntry> {
        self.new_table.random_entry()
    }

    /// Get a random address from the Tried table.
    #[allow(deprecated)]
    pub fn random_tried_address(&self) -> Option<&AddressEntry> {
        self.tried_table.random_entry()
    }

    /// Get a random address from New table with proper randomness.
    pub fn random_new_address_with<F>(&self, random_fn: F) -> Option<&AddressEntry>
    where
        F: FnMut(usize) -> usize,
    {
        self.new_table.random_entry_with(random_fn)
    }

    /// Get a random address from Tried table with proper randomness.
    pub fn random_tried_address_with<F>(&self, random_fn: F) -> Option<&AddressEntry>
    where
        F: FnMut(usize) -> usize,
    {
        self.tried_table.random_entry_with(random_fn)
    }

    /// Get statistics
    pub fn stats(&self) -> AddressManagerStats {
        AddressManagerStats {
            new_count: self.new_table.len(),
            tried_count: self.tried_table.len(),
            new_bucket_count: self.config.new_bucket_count,
            tried_bucket_count: self.config.tried_bucket_count,
        }
    }

    /// Calculate New table bucket index
    ///
    /// # Security
    /// Combines source and address subnets using secure keyed hash
    /// to prevent predictable bucket placement attacks.
    fn calculate_new_bucket(&self, source_subnet: &SubnetKey, addr_subnet: &SubnetKey) -> usize {
        // Hash both full SubnetKeys together for unpredictable distribution
        let hash = secure_bucket_hash(&(source_subnet, addr_subnet));
        (hash as usize) % self.config.new_bucket_count
    }

    /// Calculate Tried table bucket index
    ///
    /// # Security
    /// Uses secure keyed hash for unpredictable distribution.
    fn calculate_tried_bucket(&self, addr: &SocketAddr) -> usize {
        let subnet = SubnetKey::from_ip(&addr.ip);
        let hash = secure_bucket_hash(&subnet);
        (hash as usize) % self.config.tried_bucket_count
    }
}
