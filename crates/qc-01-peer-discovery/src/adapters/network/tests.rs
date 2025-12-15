//! Tests for Network Adapters
//!
//! Reference: SPEC-01-PEER-DISCOVERY.md Section 8 (Phase 4)

use super::*;
use crate::domain::{IpAddr, NodeId, SocketAddr};
use crate::ports::{ConfigProvider, NetworkSocket, NodeIdValidator, TimeSource};

#[test]
fn test_system_time_source_returns_nonzero() {
    let source = SystemTimeSource::new();
    let now = source.now();
    // Should be after Unix epoch (reasonably recent)
    assert!(now.as_secs() > 1_700_000_000); // After ~2024
}

#[test]
fn test_system_time_source_is_monotonic() {
    let source = SystemTimeSource::new();
    let t1 = source.now();
    let t2 = source.now();
    assert!(t2.as_secs() >= t1.as_secs());
}

#[test]
fn test_noop_network_socket() {
    let socket = NoOpNetworkSocket::new();
    let addr = SocketAddr::new(IpAddr::v4(127, 0, 0, 1), 8080);

    assert!(socket.send_ping(addr).is_ok());
    assert!(socket.send_pong(addr).is_ok());
    assert!(socket.send_find_node(addr, NodeId::new([1u8; 32])).is_ok());
}

#[test]
fn test_static_config_provider_defaults() {
    let provider = StaticConfigProvider::new();
    assert!(provider.get_bootstrap_nodes().is_empty());
    assert_eq!(provider.get_kademlia_config().k, 20);
}

#[test]
fn test_static_config_provider_with_bootstrap() {
    let nodes = vec![
        SocketAddr::new(IpAddr::v4(192, 168, 1, 100), 8080),
        SocketAddr::new(IpAddr::v4(10, 0, 0, 1), 8080),
    ];
    let provider = StaticConfigProvider::new().with_bootstrap_nodes(nodes.clone());
    assert_eq!(provider.get_bootstrap_nodes().len(), 2);
}

#[test]
fn test_noop_node_id_validator() {
    let validator = NoOpNodeIdValidator::new();
    assert!(validator.validate_node_id(NodeId::new([0u8; 32])));
    assert!(validator.validate_node_id(NodeId::new([255u8; 32])));
}

#[test]
fn test_proof_of_work_validator() {
    let validator = ProofOfWorkValidator::new(16); // Require 16 leading zero bits (2 bytes)

    // NodeId with 2 zero bytes at start - should pass
    let mut valid_id = [255u8; 32];
    valid_id[0] = 0;
    valid_id[1] = 0;
    assert!(validator.validate_node_id(NodeId::new(valid_id)));

    // NodeId with only 1 zero byte - should fail (only 8 zero bits)
    let mut invalid_id = [255u8; 32];
    invalid_id[0] = 0;
    assert!(!validator.validate_node_id(NodeId::new(invalid_id)));

    // NodeId with no zero bytes - should fail
    assert!(!validator.validate_node_id(NodeId::new([255u8; 32])));
}

#[test]
fn test_proof_of_work_validator_partial_byte() {
    // Require 12 leading zero bits (1 byte + 4 bits)
    let validator = ProofOfWorkValidator::new(12);

    // [0x00, 0x0F, ...] = 8 + 4 = 12 leading zeros - should pass
    let mut id = [255u8; 32];
    id[0] = 0x00;
    id[1] = 0x0F; // 0000_1111 = 4 leading zeros
    assert!(validator.validate_node_id(NodeId::new(id)));

    // [0x00, 0x1F, ...] = 8 + 3 = 11 leading zeros - should fail
    id[1] = 0x1F; // 0001_1111 = 3 leading zeros
    assert!(!validator.validate_node_id(NodeId::new(id)));
}

#[cfg(feature = "network")]
mod network_tests {
    use super::*;

    #[test]
    fn test_toml_config_provider_parse() {
        let toml = r#"
            [bootstrap]
            nodes = ["192.168.1.100:8080", "10.0.0.1:9000"]
            
            [kademlia]
            k = 25
            alpha = 5
            max_pending_peers = 2048
        "#;

        let provider = TomlConfigProvider::parse(toml).unwrap();
        assert_eq!(provider.get_bootstrap_nodes().len(), 2);

        let config = provider.get_kademlia_config();
        assert_eq!(config.k, 25);
        assert_eq!(config.alpha, 5);
        assert_eq!(config.max_pending_peers, 2048);
        assert_eq!(config.max_peers_per_subnet, 2); // default
    }

    #[test]
    fn test_toml_config_provider_empty() {
        let toml = "";
        let provider = TomlConfigProvider::parse(toml).unwrap();
        assert!(provider.get_bootstrap_nodes().is_empty());
        assert_eq!(provider.get_kademlia_config().k, 20); // default
    }
}
