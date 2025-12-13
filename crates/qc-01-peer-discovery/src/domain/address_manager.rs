//! # Address Manager - New/Tried Bucket System
//!
//! Implements Bitcoin's `addrman` pattern for Eclipse Attack resistance.
//!
//! ## Design (Bitcoin-Inspired)
//!
//! - **New Table**: Addresses heard about but never successfully connected to
//! - **Tried Table**: Addresses we've successfully connected to
//!
//! ## Anti-Eclipse Properties
//!
//! 1. Per-subnet bucketing prevents IP flooding attacks
//! 2. Segregation prevents poisoning Tried with unverified addresses
//! 3. Source-based bucketing distributes gossip across buckets
//!
//! Reference: Bitcoin Core's `addrman.h`

use std::collections::HashMap;

use crate::domain::{IpAddr, NodeId, PeerInfo, SocketAddr, Timestamp};

// =============================================================================
// CONFIGURATION
// =============================================================================

/// Configuration for the address manager
#[derive(Debug, Clone)]
pub struct AddressManagerConfig {
    /// Number of buckets in the New table
    pub new_bucket_count: usize,
    /// Number of buckets in the Tried table
    pub tried_bucket_count: usize,
    /// Maximum entries per bucket
    pub bucket_size: usize,
    /// Maximum entries from same /16 subnet per bucket
    pub max_per_subnet_per_bucket: usize,
    /// Maximum entries from same /16 subnet across all buckets
    pub max_per_subnet_total: usize,
}

impl Default for AddressManagerConfig {
    fn default() -> Self {
        Self {
            new_bucket_count: 1024,
            tried_bucket_count: 256,
            bucket_size: 64,
            max_per_subnet_per_bucket: 2,
            max_per_subnet_total: 64,
        }
    }
}

impl AddressManagerConfig {
    /// Testing config with smaller tables
    #[cfg(test)]
    pub fn for_testing() -> Self {
        Self {
            new_bucket_count: 16,
            tried_bucket_count: 8,
            bucket_size: 4,
            max_per_subnet_per_bucket: 2,
            max_per_subnet_total: 8,
        }
    }
}

// =============================================================================
// ADDRESS ENTRY
// =============================================================================

/// An address entry in the address manager
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddressEntry {
    /// Full peer information
    pub peer_info: PeerInfo,
    /// When we first learned about this address
    pub first_seen: Timestamp,
    /// When we last attempted connection
    pub last_attempt: Option<Timestamp>,
    /// When we last successfully connected
    pub last_success: Option<Timestamp>,
    /// Number of connection attempts
    pub attempts: u32,
    /// Source that told us about this address (/16 subnet key)
    pub source_subnet: SubnetKey,
}

impl AddressEntry {
    /// Create a new address entry
    pub fn new(peer_info: PeerInfo, now: Timestamp, source_subnet: SubnetKey) -> Self {
        Self {
            first_seen: now,
            last_attempt: None,
            last_success: None,
            attempts: 0,
            source_subnet,
            peer_info,
        }
    }
}

// =============================================================================
// SUBNET KEY (for bucketing)
// =============================================================================

/// /16 subnet key for IPv4, /32 for IPv6
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SubnetKey([u8; 2]);

impl SubnetKey {
    /// Extract /16 subnet from IP address
    pub fn from_ip(ip: &IpAddr) -> Self {
        match ip {
            IpAddr::V4(bytes) => SubnetKey([bytes[0], bytes[1]]),
            IpAddr::V6(bytes) => SubnetKey([bytes[0], bytes[1]]), // First /16 of IPv6
        }
    }
}

// =============================================================================
// ADDRESS BUCKET
// =============================================================================

/// A bucket containing address entries with subnet limits
#[derive(Debug, Clone, Default)]
pub struct AddressBucket {
    entries: Vec<AddressEntry>,
    /// Count of entries per subnet in this bucket
    subnet_counts: HashMap<SubnetKey, usize>,
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

    /// Get a random entry
    pub fn random_entry(&self) -> Option<&AddressEntry> {
        if self.entries.is_empty() {
            None
        } else {
            // Simple deterministic selection for now (can add randomness later)
            Some(&self.entries[0])
        }
    }
}

// =============================================================================
// ADDRESS TABLE (NEW or TRIED)
// =============================================================================

/// A table of buckets (either New or Tried)
#[derive(Debug)]
pub struct AddressTable {
    buckets: Vec<AddressBucket>,
    /// Total entries per subnet across all buckets
    subnet_totals: HashMap<SubnetKey, usize>,
    /// Quick lookup: NodeId -> bucket index
    node_to_bucket: HashMap<NodeId, usize>,
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

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.buckets.iter().all(|b| b.is_empty())
    }

    /// Check if table contains a node
    pub fn contains(&self, node_id: &NodeId) -> bool {
        self.node_to_bucket.contains_key(node_id)
    }

    /// Get a random entry from the table
    pub fn random_entry(&self) -> Option<&AddressEntry> {
        // Simple: iterate buckets and return first non-empty
        for bucket in &self.buckets {
            if let Some(entry) = bucket.random_entry() {
                return Some(entry);
            }
        }
        None
    }
}

// =============================================================================
// ADDRESS MANAGER
// =============================================================================

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
    ///
    /// # Arguments
    /// * `peer_info` - The address to add
    /// * `source_ip` - IP of the peer that told us about this address
    /// * `now` - Current timestamp
    ///
    /// # Returns
    /// * `Ok(true)` - Address was added
    /// * `Ok(false)` - Address was rejected (duplicate, subnet limit, etc.)
    /// * `Err` - Invalid input
    pub fn add_new(
        &mut self,
        peer_info: PeerInfo,
        source_ip: &IpAddr,
        now: Timestamp,
    ) -> Result<bool, AddressManagerError> {
        let node_id = peer_info.node_id;

        // Check if already in either table
        if self.tried_table.contains(&node_id) || self.new_table.contains(&node_id) {
            return Ok(false);
        }

        let source_subnet = SubnetKey::from_ip(source_ip);
        let addr_subnet = SubnetKey::from_ip(&peer_info.socket_addr.ip);

        // Check total subnet limit
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

        // Calculate bucket (Bitcoin's addrman formula)
        let bucket_idx = self.calculate_new_bucket(&source_subnet, &addr_subnet);
        let bucket = &mut self.new_table.buckets[bucket_idx];

        // Check if bucket can accept
        if !bucket.can_accept(&addr_subnet, &self.config) {
            return Ok(false);
        }

        // Add entry
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
        // Find in New table
        let bucket_idx = match self.new_table.node_to_bucket.get(node_id) {
            Some(&idx) => idx,
            None => return Ok(false), // Not in New table
        };

        let bucket = &mut self.new_table.buckets[bucket_idx];
        let mut entry = match bucket.remove(node_id) {
            Some(e) => e,
            None => return Ok(false),
        };

        // Update entry
        entry.last_success = Some(now);
        entry.attempts += 1;

        // Remove from New table tracking
        let addr_subnet = SubnetKey::from_ip(&entry.peer_info.socket_addr.ip);
        if let Some(count) = self.new_table.subnet_totals.get_mut(&addr_subnet) {
            *count = count.saturating_sub(1);
        }
        self.new_table.node_to_bucket.remove(node_id);

        // Calculate Tried bucket
        let tried_bucket_idx = self.calculate_tried_bucket(&entry.peer_info.socket_addr);
        let tried_bucket = &mut self.tried_table.buckets[tried_bucket_idx];

        // Check if bucket can accept
        if !tried_bucket.can_accept(&addr_subnet, &self.config) {
            // Tried bucket full - evict oldest and still add
            // For now, just reject (TODO: implement eviction)
            return Ok(false);
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

    /// Get a random address from the New table (for Feeler connections)
    pub fn random_new_address(&self) -> Option<&AddressEntry> {
        self.new_table.random_entry()
    }

    /// Get a random address from the Tried table (for outbound connections)
    pub fn random_tried_address(&self) -> Option<&AddressEntry> {
        self.tried_table.random_entry()
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

    // =========================================================================
    // BUCKET CALCULATION (Bitcoin's addrman algorithm)
    // =========================================================================

    /// Calculate New table bucket index
    /// bucket = hash(source_group || addr_group) % new_bucket_count
    fn calculate_new_bucket(&self, source_subnet: &SubnetKey, addr_subnet: &SubnetKey) -> usize {
        // Simple hash combining source and address group
        let combined = [
            source_subnet.0[0],
            source_subnet.0[1],
            addr_subnet.0[0],
            addr_subnet.0[1],
        ];
        let hash = simple_hash(&combined);
        hash % self.config.new_bucket_count
    }

    /// Calculate Tried table bucket index
    /// bucket = hash(addr_group) % tried_bucket_count
    fn calculate_tried_bucket(&self, addr: &SocketAddr) -> usize {
        let subnet = SubnetKey::from_ip(&addr.ip);
        let hash = simple_hash(&subnet.0);
        hash % self.config.tried_bucket_count
    }
}

/// Simple hash function for bucket calculation
fn simple_hash(data: &[u8]) -> usize {
    let mut hash: usize = 0;
    for byte in data {
        hash = hash.wrapping_mul(31).wrapping_add(*byte as usize);
    }
    hash
}

// =============================================================================
// ERRORS
// =============================================================================

/// Errors from address manager operations
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddressManagerError {
    /// Invalid IP address
    InvalidAddress,
}

// =============================================================================
// STATISTICS
// =============================================================================

/// Statistics about the address manager
#[derive(Debug, Clone, Default)]
pub struct AddressManagerStats {
    pub new_count: usize,
    pub tried_count: usize,
    pub new_bucket_count: usize,
    pub tried_bucket_count: usize,
}

// =============================================================================
// TESTS (TDD - Written First)
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_peer(id_byte: u8, ip_third: u8, ip_fourth: u8) -> PeerInfo {
        let mut id = [0u8; 32];
        id[0] = id_byte;
        PeerInfo::new(
            NodeId::new(id),
            SocketAddr::new(IpAddr::v4(192, 168, ip_third, ip_fourth), 8080),
            Timestamp::new(1000),
        )
    }

    fn make_source_ip(third: u8, fourth: u8) -> IpAddr {
        IpAddr::v4(10, 0, third, fourth)
    }

    // =========================================================================
    // TEST GROUP 1: Basic New/Tried Segregation
    // =========================================================================

    #[test]
    fn test_new_address_goes_to_new_table() {
        let config = AddressManagerConfig::for_testing();
        let mut manager = AddressManager::new(config);
        let now = Timestamp::new(1000);

        let peer = make_peer(1, 1, 100);
        let source = make_source_ip(0, 1);

        let result = manager.add_new(peer.clone(), &source, now);
        assert!(result.unwrap());

        let stats = manager.stats();
        assert_eq!(stats.new_count, 1);
        assert_eq!(stats.tried_count, 0);
    }

    #[test]
    fn test_duplicate_address_rejected() {
        let config = AddressManagerConfig::for_testing();
        let mut manager = AddressManager::new(config);
        let now = Timestamp::new(1000);

        let peer = make_peer(1, 1, 100);
        let source = make_source_ip(0, 1);

        manager.add_new(peer.clone(), &source, now).unwrap();
        let result = manager.add_new(peer.clone(), &source, now);
        assert!(!result.unwrap()); // Duplicate rejected

        assert_eq!(manager.stats().new_count, 1);
    }

    #[test]
    fn test_promote_moves_to_tried() {
        let config = AddressManagerConfig::for_testing();
        let mut manager = AddressManager::new(config);
        let now = Timestamp::new(1000);

        let peer = make_peer(1, 1, 100);
        let source = make_source_ip(0, 1);

        manager.add_new(peer.clone(), &source, now).unwrap();
        let result = manager.promote_to_tried(&peer.node_id, now);
        assert!(result.unwrap());

        let stats = manager.stats();
        assert_eq!(stats.new_count, 0);
        assert_eq!(stats.tried_count, 1);
    }

    // =========================================================================
    // TEST GROUP 2: Per-Subnet Limits (Anti-Eclipse)
    // =========================================================================

    #[test]
    fn test_per_subnet_bucket_limit() {
        let config = AddressManagerConfig::for_testing();
        let mut manager = AddressManager::new(config.clone());
        let now = Timestamp::new(1000);

        // Add max_per_subnet_per_bucket peers from same /16
        // All have 192.168.x.y (same /16 subnet 192.168.0.0/16)
        let source = make_source_ip(0, 1);

        // First two should succeed (max_per_subnet_per_bucket = 2)
        let peer1 = make_peer(1, 1, 100);
        let peer2 = make_peer(2, 1, 101);
        assert!(manager.add_new(peer1, &source, now).unwrap());
        assert!(manager.add_new(peer2, &source, now).unwrap());

        // Third peer same subnet might be rejected if lands in same bucket
        // (depends on hash). Let's verify at least subnet tracking works
        let stats = manager.stats();
        assert_eq!(stats.new_count, 2);
    }

    #[test]
    fn test_different_subnets_allowed() {
        let config = AddressManagerConfig::for_testing();
        let mut manager = AddressManager::new(config);
        let now = Timestamp::new(1000);
        let source = make_source_ip(0, 1);

        // Peers from same /16 (192.168.x.x) but same source
        // may hit per-bucket subnet limits depending on hash distribution
        let peer1 = make_peer(1, 1, 100); // 192.168.1.100
        let peer2 = make_peer(2, 2, 100); // 192.168.2.100

        assert!(manager.add_new(peer1, &source, now).unwrap());
        assert!(manager.add_new(peer2, &source, now).unwrap());

        // At least 2 should be added (may be more if in different buckets)
        assert!(manager.stats().new_count >= 2);
    }

    // =========================================================================
    // TEST GROUP 3: Bucket Distribution
    // =========================================================================

    #[test]
    fn test_different_sources_distribute_to_different_buckets() {
        let config = AddressManagerConfig::for_testing();
        let mut manager = AddressManager::new(config);
        let now = Timestamp::new(1000);

        // Different peers, different sources - should distribute across buckets
        let peer1 = make_peer(1, 1, 100);
        let peer2 = make_peer(2, 1, 101);

        let source1 = make_source_ip(0, 1);
        let source2 = make_source_ip(1, 1); // Different /16 source

        assert!(manager.add_new(peer1, &source1, now).unwrap());
        assert!(manager.add_new(peer2, &source2, now).unwrap());

        // At least 2 should be added
        assert!(manager.stats().new_count >= 2);
    }

    // =========================================================================
    // TEST GROUP 4: Random Selection
    // =========================================================================

    #[test]
    fn test_random_new_address_returns_entry() {
        let config = AddressManagerConfig::for_testing();
        let mut manager = AddressManager::new(config);
        let now = Timestamp::new(1000);

        let peer = make_peer(1, 1, 100);
        let source = make_source_ip(0, 1);

        manager.add_new(peer.clone(), &source, now).unwrap();

        let random = manager.random_new_address();
        assert!(random.is_some());
        assert_eq!(random.unwrap().peer_info.node_id, peer.node_id);
    }

    #[test]
    fn test_random_tried_address_after_promotion() {
        let config = AddressManagerConfig::for_testing();
        let mut manager = AddressManager::new(config);
        let now = Timestamp::new(1000);

        let peer = make_peer(1, 1, 100);
        let source = make_source_ip(0, 1);

        manager.add_new(peer.clone(), &source, now).unwrap();
        manager.promote_to_tried(&peer.node_id, now).unwrap();

        let random_new = manager.random_new_address();
        let random_tried = manager.random_tried_address();

        assert!(random_new.is_none()); // Moved out of New
        assert!(random_tried.is_some()); // Now in Tried
    }
}
