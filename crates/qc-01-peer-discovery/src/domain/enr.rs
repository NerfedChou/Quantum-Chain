//! # Ethereum Node Records (ENR)
//!
//! Implements EIP-778 inspired self-signed node identity records.
//!
//! ## Purpose
//!
//! ENR allows nodes to share signed identity records containing:
//! - Public key (for identity verification)
//! - Network addresses (IP, UDP port, TCP port)
//! - Capabilities (full node, light server, shard ranges, etc.)
//!
//! ## Security Properties
//!
//! - Self-signed: Record is signed by the node's private key
//! - Sequence number: Prevents replay of old records
//! - Compact: Efficient wire format for gossip
//!
//! Reference: EIP-778 (Ethereum Node Records)

use std::collections::HashMap;

use crate::domain::{IpAddr, NodeId, SocketAddr};

// =============================================================================
// CONFIGURATION
// =============================================================================

/// ENR configuration
#[derive(Debug, Clone)]
pub struct EnrConfig {
    /// Maximum size of an ENR record (bytes)
    pub max_record_size: usize,
    /// Maximum age of a record before considered stale (seconds)
    pub max_record_age_secs: u64,
    /// Maximum capabilities per record
    pub max_capabilities: usize,
}

impl Default for EnrConfig {
    fn default() -> Self {
        Self {
            max_record_size: 300,
            max_record_age_secs: 86400, // 24 hours
            max_capabilities: 16,
        }
    }
}

// =============================================================================
// NODE RECORD
// =============================================================================

/// Ethereum Node Record (EIP-778 inspired)
///
/// A self-signed record containing node identity and capabilities.
#[derive(Debug, Clone)]
pub struct NodeRecord {
    /// Sequence number (increment on ANY change)
    pub seq: u64,
    /// Node's public key (33 bytes compressed secp256k1)
    pub pubkey: PublicKey,
    /// IP address
    pub ip: IpAddr,
    /// UDP port for discovery
    pub udp_port: u16,
    /// TCP port for data (optional, 0 if same as UDP)
    pub tcp_port: u16,
    /// Capabilities
    pub capabilities: Vec<Capability>,
    /// Signature over the record (64 bytes)
    pub signature: Signature,
}

impl NodeRecord {
    /// Create a new unsigned record (for building)
    pub fn new_unsigned(
        seq: u64,
        pubkey: PublicKey,
        ip: IpAddr,
        udp_port: u16,
        tcp_port: u16,
        capabilities: Vec<Capability>,
    ) -> Self {
        Self {
            seq,
            pubkey,
            ip,
            udp_port,
            tcp_port,
            capabilities,
            signature: Signature::empty(),
        }
    }

    /// Get the Node ID derived from public key
    pub fn node_id(&self) -> NodeId {
        // Node ID = Keccak256(pubkey)[12..32] or just hash of pubkey
        // Simplified: use first 32 bytes of pubkey hash
        let mut id = [0u8; 32];
        let hash = simple_hash(&self.pubkey.0);
        id[0] = (hash >> 24) as u8;
        id[1] = (hash >> 16) as u8;
        id[2] = (hash >> 8) as u8;
        id[3] = hash as u8;
        // Fill rest with pubkey bytes
        let copy_len = 28.min(self.pubkey.0.len());
        id[4..4 + copy_len].copy_from_slice(&self.pubkey.0[..copy_len]);
        NodeId::new(id)
    }

    /// Get socket address
    pub fn socket_addr(&self) -> SocketAddr {
        SocketAddr::new(self.ip, self.udp_port)
    }

    /// Get the signing payload (everything except signature)
    pub fn signing_payload(&self) -> Vec<u8> {
        let mut payload = Vec::new();

        // Sequence number (8 bytes)
        payload.extend_from_slice(&self.seq.to_be_bytes());

        // Public key (33 bytes)
        payload.extend_from_slice(&self.pubkey.0);

        // IP address (4 or 16 bytes)
        match &self.ip {
            IpAddr::V4(bytes) => {
                payload.push(4);
                payload.extend_from_slice(bytes);
            }
            IpAddr::V6(bytes) => {
                payload.push(16);
                payload.extend_from_slice(bytes);
            }
        }

        // Ports (4 bytes)
        payload.extend_from_slice(&self.udp_port.to_be_bytes());
        payload.extend_from_slice(&self.tcp_port.to_be_bytes());

        // Capabilities
        payload.push(self.capabilities.len() as u8);
        for cap in &self.capabilities {
            payload.extend_from_slice(&cap.to_bytes());
        }

        payload
    }

    /// Verify the signature is valid for this record
    ///
    /// Returns true if signature is valid for the signing payload
    pub fn verify_signature(&self) -> bool {
        // In production: verify using secp256k1
        // Simplified: check signature is valid hash of payload
        let payload = self.signing_payload();
        let expected_hash = simple_hash(&payload);

        // Check if signature contains the expected hash
        if self.signature.0.len() < 4 {
            return false;
        }

        let sig_hash = u32::from_be_bytes([
            self.signature.0[0],
            self.signature.0[1],
            self.signature.0[2],
            self.signature.0[3],
        ]);

        sig_hash == expected_hash
    }

    /// Sign the record with a private key
    ///
    /// In production: use secp256k1 ECDSA
    pub fn sign(&mut self, _private_key: &[u8; 32]) {
        let payload = self.signing_payload();
        let hash = simple_hash(&payload);

        // Simplified signature: embed hash
        let mut sig = [0u8; 64];
        sig[0..4].copy_from_slice(&hash.to_be_bytes());
        self.signature = Signature(sig);
    }

    /// Check if record has a specific capability
    pub fn has_capability(&self, cap_type: CapabilityType) -> bool {
        self.capabilities.iter().any(|c| c.cap_type == cap_type)
    }

    /// Get all capabilities of a specific type
    pub fn get_capabilities(&self, cap_type: CapabilityType) -> Vec<&Capability> {
        self.capabilities
            .iter()
            .filter(|c| c.cap_type == cap_type)
            .collect()
    }
}

// =============================================================================
// PUBLIC KEY
// =============================================================================

/// Compressed secp256k1 public key (33 bytes)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicKey(pub [u8; 33]);

impl PublicKey {
    /// Create from bytes
    pub fn new(bytes: [u8; 33]) -> Self {
        Self(bytes)
    }

    /// Create an empty public key
    pub fn empty() -> Self {
        Self([0u8; 33])
    }

    /// Get as bytes
    pub fn as_bytes(&self) -> &[u8; 33] {
        &self.0
    }
}

// =============================================================================
// SIGNATURE
// =============================================================================

/// ECDSA signature (64 bytes: r + s)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature(pub [u8; 64]);

impl Signature {
    /// Create from bytes
    pub fn new(bytes: [u8; 64]) -> Self {
        Self(bytes)
    }

    /// Create an empty signature
    pub fn empty() -> Self {
        Self([0u8; 64])
    }

    /// Get as bytes
    pub fn as_bytes(&self) -> &[u8; 64] {
        &self.0
    }
}

// =============================================================================
// CAPABILITY
// =============================================================================

/// Node capability advertisement
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capability {
    /// Type of capability
    pub cap_type: CapabilityType,
    /// Capability-specific data
    pub data: CapabilityData,
}

impl Capability {
    /// Create a new capability
    pub fn new(cap_type: CapabilityType, data: CapabilityData) -> Self {
        Self { cap_type, data }
    }

    /// Create a "full node" capability
    pub fn full_node() -> Self {
        Self::new(CapabilityType::FullNode, CapabilityData::None)
    }

    /// Create a "light server" capability
    pub fn light_server() -> Self {
        Self::new(CapabilityType::LightServer, CapabilityData::None)
    }

    /// Create a "shard" capability
    pub fn shard(shard_id: u16) -> Self {
        Self::new(CapabilityType::Shard, CapabilityData::ShardId(shard_id))
    }

    /// Create a "shard range" capability
    pub fn shard_range(start: u16, end: u16) -> Self {
        Self::new(
            CapabilityType::ShardRange,
            CapabilityData::ShardRange { start, end },
        )
    }

    /// Serialize to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.push(self.cap_type as u8);
        match &self.data {
            CapabilityData::None => {}
            CapabilityData::ShardId(id) => {
                bytes.extend_from_slice(&id.to_be_bytes());
            }
            CapabilityData::ShardRange { start, end } => {
                bytes.extend_from_slice(&start.to_be_bytes());
                bytes.extend_from_slice(&end.to_be_bytes());
            }
            CapabilityData::Custom(data) => {
                bytes.push(data.len() as u8);
                bytes.extend_from_slice(data);
            }
        }
        bytes
    }
}

/// Types of node capabilities
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum CapabilityType {
    /// Full node - stores all data
    FullNode = 1,
    /// Light client server - serves light client proofs
    LightServer = 2,
    /// Specific shard
    Shard = 3,
    /// Range of shards
    ShardRange = 4,
    /// Archive node - stores historical state
    Archive = 5,
    /// Custom capability
    Custom = 255,
}

/// Capability-specific data
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityData {
    /// No additional data
    None,
    /// Single shard ID
    ShardId(u16),
    /// Range of shards (inclusive)
    ShardRange { start: u16, end: u16 },
    /// Custom data
    Custom(Vec<u8>),
}

// =============================================================================
// ENR CACHE
// =============================================================================

/// Cache of known ENR records
#[derive(Debug)]
pub struct EnrCache {
    /// Records by Node ID
    records: HashMap<NodeId, CachedRecord>,
    /// Configuration
    config: EnrConfig,
}

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

impl EnrCache {
    /// Create a new cache
    pub fn new(config: EnrConfig) -> Self {
        Self {
            records: HashMap::new(),
            config,
        }
    }

    /// Insert or update a record
    ///
    /// Returns true if record was accepted, false if rejected
    pub fn insert(&mut self, record: NodeRecord, now_secs: u64) -> bool {
        // Verify signature
        if !record.verify_signature() {
            return false;
        }

        let node_id = record.node_id();

        // Check if we have an existing record
        if let Some(existing) = self.records.get(&node_id) {
            // Only accept if sequence number is higher
            if record.seq <= existing.record.seq {
                return false;
            }
        }

        // Check capabilities limit
        if record.capabilities.len() > self.config.max_capabilities {
            return false;
        }

        // Insert
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

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/// Simple hash function (for demonstration - use Keccak256 in production)
fn simple_hash(data: &[u8]) -> u32 {
    let mut hash: u32 = 0;
    for byte in data {
        hash = hash.wrapping_mul(31).wrapping_add(*byte as u32);
    }
    hash
}

// =============================================================================
// TESTS (TDD)
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_pubkey(byte: u8) -> PublicKey {
        let mut key = [0u8; 33];
        key[0] = 0x02; // Compressed key prefix
        key[1] = byte;
        PublicKey::new(key)
    }

    fn make_record(seq: u64, port: u16) -> NodeRecord {
        NodeRecord::new_unsigned(
            seq,
            make_pubkey(1),
            IpAddr::v4(192, 168, 1, 100),
            port,
            port,
            vec![Capability::full_node()],
        )
    }

    // =========================================================================
    // TEST GROUP 1: Record Creation and Signing
    // =========================================================================

    #[test]
    fn test_record_creation() {
        let record = make_record(1, 8080);

        assert_eq!(record.seq, 1);
        assert_eq!(record.udp_port, 8080);
        assert_eq!(record.capabilities.len(), 1);
    }

    #[test]
    fn test_record_signing_and_verification() {
        let mut record = make_record(1, 8080);
        let private_key = [1u8; 32];

        // Before signing, verification should fail
        assert!(!record.verify_signature());

        // Sign the record
        record.sign(&private_key);

        // After signing, verification should succeed
        assert!(record.verify_signature());
    }

    #[test]
    fn test_modified_record_fails_verification() {
        let mut record = make_record(1, 8080);
        let private_key = [1u8; 32];
        record.sign(&private_key);

        // Modify the record
        record.seq = 2;

        // Verification should now fail
        assert!(!record.verify_signature());
    }

    // =========================================================================
    // TEST GROUP 2: Node ID Derivation
    // =========================================================================

    #[test]
    fn test_node_id_derived_from_pubkey() {
        let record1 = make_record(1, 8080);
        let record2 = make_record(2, 9000); // Same pubkey

        // Same pubkey should produce same node ID
        assert_eq!(record1.node_id(), record2.node_id());

        // Different pubkey should produce different node ID
        let mut record3 = make_record(1, 8080);
        record3.pubkey = make_pubkey(2);
        assert_ne!(record1.node_id(), record3.node_id());
    }

    // =========================================================================
    // TEST GROUP 3: Capabilities
    // =========================================================================

    #[test]
    fn test_capability_full_node() {
        let cap = Capability::full_node();
        assert_eq!(cap.cap_type, CapabilityType::FullNode);
    }

    #[test]
    fn test_capability_shard_range() {
        let cap = Capability::shard_range(0, 10);
        assert_eq!(cap.cap_type, CapabilityType::ShardRange);

        if let CapabilityData::ShardRange { start, end } = cap.data {
            assert_eq!(start, 0);
            assert_eq!(end, 10);
        } else {
            panic!("Expected ShardRange data");
        }
    }

    #[test]
    fn test_has_capability() {
        let mut record = make_record(1, 8080);
        record.capabilities.push(Capability::light_server());

        assert!(record.has_capability(CapabilityType::FullNode));
        assert!(record.has_capability(CapabilityType::LightServer));
        assert!(!record.has_capability(CapabilityType::Archive));
    }

    // =========================================================================
    // TEST GROUP 4: ENR Cache
    // =========================================================================

    #[test]
    fn test_cache_insert_and_get() {
        let config = EnrConfig::default();
        let mut cache = EnrCache::new(config);

        let mut record = make_record(1, 8080);
        record.sign(&[1u8; 32]);

        let node_id = record.node_id();
        let now = 1000u64;

        assert!(cache.insert(record, now));
        assert!(cache.get(&node_id).is_some());
    }

    #[test]
    fn test_cache_rejects_lower_seq() {
        let config = EnrConfig::default();
        let mut cache = EnrCache::new(config);
        let now = 1000u64;

        // Insert seq=2
        let mut record2 = make_record(2, 8080);
        record2.sign(&[1u8; 32]);
        assert!(cache.insert(record2.clone(), now));

        // Try to insert seq=1 (should be rejected)
        let mut record1 = make_record(1, 8080);
        record1.sign(&[1u8; 32]);
        assert!(!cache.insert(record1, now));

        // Stored record should still be seq=2
        let node_id = record2.node_id();
        assert_eq!(cache.get(&node_id).unwrap().seq, 2);
    }

    #[test]
    fn test_cache_accepts_higher_seq() {
        let config = EnrConfig::default();
        let mut cache = EnrCache::new(config);
        let now = 1000u64;

        // Insert seq=1
        let mut record1 = make_record(1, 8080);
        record1.sign(&[1u8; 32]);
        let node_id = record1.node_id();
        assert!(cache.insert(record1, now));

        // Insert seq=2 (should replace)
        let mut record2 = make_record(2, 9000);
        record2.sign(&[1u8; 32]);
        assert!(cache.insert(record2, now));

        // Stored record should be seq=2 with new port
        assert_eq!(cache.get(&node_id).unwrap().seq, 2);
        assert_eq!(cache.get(&node_id).unwrap().udp_port, 9000);
    }

    #[test]
    fn test_cache_find_by_capability() {
        let config = EnrConfig::default();
        let mut cache = EnrCache::new(config);
        let now = 1000u64;

        // Full node
        let mut record1 = make_record(1, 8080);
        record1.sign(&[1u8; 32]);
        cache.insert(record1, now);

        // Light server
        let mut record2 = NodeRecord::new_unsigned(
            1,
            make_pubkey(2),
            IpAddr::v4(192, 168, 1, 101),
            8081,
            8081,
            vec![Capability::light_server()],
        );
        record2.sign(&[2u8; 32]);
        cache.insert(record2, now);

        let full_nodes = cache.find_by_capability(CapabilityType::FullNode);
        let light_servers = cache.find_by_capability(CapabilityType::LightServer);

        assert_eq!(full_nodes.len(), 1);
        assert_eq!(light_servers.len(), 1);
    }

    #[test]
    fn test_cache_gc_stale() {
        let config = EnrConfig {
            max_record_age_secs: 100,
            ..Default::default()
        };
        let mut cache = EnrCache::new(config);

        // Insert at time 0
        let mut record = make_record(1, 8080);
        record.sign(&[1u8; 32]);
        cache.insert(record, 0);

        assert_eq!(cache.len(), 1);

        // GC at time 50 - should keep
        let removed = cache.gc_stale(50);
        assert_eq!(removed, 0);
        assert_eq!(cache.len(), 1);

        // GC at time 150 - should remove (age > 100)
        let removed = cache.gc_stale(150);
        assert_eq!(removed, 1);
        assert_eq!(cache.len(), 0);
    }
}
