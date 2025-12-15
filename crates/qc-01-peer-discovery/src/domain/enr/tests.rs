//! Tests for Ethereum Node Records (ENR)
//!
//! Reference: EIP-778 (Ethereum Node Records)

use super::*;
use crate::domain::IpAddr;

fn make_pubkey(byte: u8) -> PublicKey {
    let mut key = [0u8; 33];
    key[0] = 0x02; // Compressed key prefix
    key[1] = byte;
    PublicKey::new(key)
}

fn make_record(seq: u64, port: u16) -> NodeRecord {
    NodeRecord::new_unsigned(NodeRecordConfig {
        seq,
        pubkey: make_pubkey(1),
        ip: IpAddr::v4(192, 168, 1, 100),
        udp_port: port,
        tcp_port: port,
        capabilities: vec![Capability::full_node()],
    })
}

// =============================================================================
// TEST GROUP 1: Record Creation and Signing
// =============================================================================

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

// =============================================================================
// TEST GROUP 2: Node ID Derivation
// =============================================================================

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

// =============================================================================
// TEST GROUP 3: Capabilities
// =============================================================================

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

// =============================================================================
// TEST GROUP 4: ENR Cache
// =============================================================================

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
fn test_cache_find_by_capability() {
    let config = EnrConfig::default();
    let mut cache = EnrCache::new(config);
    let now = 1000u64;

    // Full node
    let mut record1 = make_record(1, 8080);
    record1.sign(&[1u8; 32]);
    cache.insert(record1, now);

    // Light server
    let mut record2 = NodeRecord::new_unsigned(NodeRecordConfig {
        seq: 1,
        pubkey: make_pubkey(2),
        ip: IpAddr::v4(192, 168, 1, 101),
        udp_port: 8081,
        tcp_port: 8081,
        capabilities: vec![Capability::light_server()],
    });
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
