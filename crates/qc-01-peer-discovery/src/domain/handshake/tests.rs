//! Tests for Chain-Aware Handshakes
//!
//! Reference: Ethereum's Fork-ID (EIP-2124), Go-Ethereum's handshake

use super::*;

fn make_genesis() -> [u8; 32] {
    let mut hash = [0u8; 32];
    hash[0] = 0xDE;
    hash[1] = 0xAD;
    hash
}

fn make_handshake(
    genesis: [u8; 32],
    network_id: u32,
    height: u64,
    difficulty: u128,
) -> HandshakeData {
    HandshakeData::new(
        ChainInfo::new(network_id, genesis, 1),
        HeadState::new(height, [0u8; 32], difficulty),
    )
}

// =============================================================================
// TEST GROUP 1: Network Matching
// =============================================================================

// =============================================================================
// TEST HELPERS
// =============================================================================

struct HandshakeScenario {
    genesis_ours: u8,
    genesis_theirs: u8,
    network_id_ours: u32,
    network_id_theirs: u32,
    height_ours: u64,
    height_theirs: u64,
    diff_ours: u128,
    diff_theirs: u128,
    protocol_version_theirs: u16,
    config: HandshakeConfig,
}

impl Default for HandshakeScenario {
    fn default() -> Self {
        Self {
            genesis_ours: 0xDE,
            genesis_theirs: 0xDE,
            network_id_ours: 1,
            network_id_theirs: 1,
            height_ours: 100,
            height_theirs: 100,
            diff_ours: 1000,
            diff_theirs: 1000,
            protocol_version_theirs: 1,
            config: HandshakeConfig::default(),
        }
    }
}

impl HandshakeScenario {
    fn run(self) -> HandshakeResult {
        let mut genesis_ours = make_genesis();
        genesis_ours[0] = self.genesis_ours;

        let mut genesis_theirs = make_genesis();
        genesis_theirs[0] = self.genesis_theirs;

        let ours = make_handshake(
            genesis_ours,
            self.network_id_ours,
            self.height_ours,
            self.diff_ours,
        );

        let mut theirs = make_handshake(
            genesis_theirs,
            self.network_id_theirs,
            self.height_theirs,
            self.diff_theirs,
        );
        theirs.chain_info.protocol_version = self.protocol_version_theirs;

        verify_handshake(&ours, &theirs, &self.config)
    }
}

// =============================================================================
// TEST GROUP 1: Network Matching
// =============================================================================

// =============================================================================
// TEST GROUP 1-4: Handshake Logic (Consolidated)
// =============================================================================

#[test]
fn test_handshake_rejections() {
    struct TestCase {
        name: &'static str,
        scenario: HandshakeScenario,
        expected: HandshakeResult,
    }

    let cases = vec![
        // Network Matching
        TestCase {
            name: "Wrong Genesis",
            scenario: HandshakeScenario {
                genesis_theirs: 0xBE,
                ..Default::default()
            },
            expected: HandshakeResult::Reject(RejectReason::WrongNetwork),
        },
        TestCase {
            name: "Wrong Network ID",
            scenario: HandshakeScenario {
                network_id_theirs: 2,
                ..Default::default()
            },
            expected: HandshakeResult::Reject(RejectReason::NetworkIdMismatch),
        },
        // Protocol Version
        TestCase {
            name: "Old Protocol",
            scenario: HandshakeScenario {
                protocol_version_theirs: 0,
                ..Default::default()
            },
            expected: HandshakeResult::Reject(RejectReason::ProtocolMismatch),
        },
        // Fork Check
        TestCase {
            name: "Peer Too Far Behind",
            scenario: HandshakeScenario {
                height_ours: 1000,
                diff_ours: 10000,
                height_theirs: 10,
                diff_theirs: 100,
                config: HandshakeConfig {
                    finalized_height: 500,
                    max_behind_blocks: 100,
                    ..Default::default()
                },
                ..Default::default()
            },
            expected: HandshakeResult::Reject(RejectReason::TooFarBehind),
        },
    ];

    for case in cases {
        assert_eq!(
            case.scenario.run(),
            case.expected,
            "Failed case: {}",
            case.name
        );
    }
}

#[test]
fn test_handshake_acceptance_and_classification() {
    struct TestCase {
        name: &'static str,
        scenario: HandshakeScenario,
        expected: HandshakeResult,
    }

    let cases = vec![
        TestCase {
            name: "Peer Slightly Behind (Accepted)",
            scenario: HandshakeScenario {
                height_ours: 1000,
                diff_ours: 10000,
                height_theirs: 950,
                diff_theirs: 9500,
                config: HandshakeConfig {
                    finalized_height: 500,
                    max_behind_blocks: 100,
                    ..Default::default()
                },
                ..Default::default()
            },
            expected: HandshakeResult::Accept(PeerClassification::SyncTarget), // Changed to Equal as default match
        },
        // Classification
        TestCase {
            name: "Peer Ahead (Sync Source)",
            scenario: HandshakeScenario {
                diff_theirs: 2000,
                ..Default::default()
            },
            expected: HandshakeResult::Accept(PeerClassification::SyncSource),
        },
        TestCase {
            name: "Peer Behind (Sync Target)",
            scenario: HandshakeScenario {
                diff_ours: 2000,
                diff_theirs: 1000,
                ..Default::default()
            },
            expected: HandshakeResult::Accept(PeerClassification::SyncTarget),
        },
        TestCase {
            name: "Peer Equal",
            scenario: HandshakeScenario::default(),
            expected: HandshakeResult::Accept(PeerClassification::Equal),
        },
    ];

    for case in cases {
        assert_eq!(
            case.scenario.run(),
            case.expected,
            "Failed case: {}",
            case.name
        );
    }
}

// =============================================================================
// TEST GROUP 5: Fork ID (EIP-2124)
// =============================================================================

#[test]
fn test_fork_id_hash_mismatch_incompatible() {
    let ours = ForkId::new(0xDEADBEEF, 1000);
    let theirs = ForkId::new(0xCAFEBABE, 1000);

    // Different hashes = different chains
    assert!(!ours.is_compatible(&theirs, 500));
}

#[test]
fn test_fork_id_same_hash_and_next_compatible() {
    let ours = ForkId::new(0xDEADBEEF, 1000);
    let theirs = ForkId::new(0xDEADBEEF, 1000);

    assert!(ours.is_compatible(&theirs, 500));
    assert!(ours.is_compatible(&theirs, 999));
    assert!(ours.is_compatible(&theirs, 1500)); // Even past the fork
}

#[test]
fn test_fork_id_no_future_fork_compatible() {
    // next=0 means no future forks expected
    let ours = ForkId::new(0xDEADBEEF, 0);
    let theirs = ForkId::new(0xDEADBEEF, 1000);

    // One has no future fork, other does - compatible
    assert!(ours.is_compatible(&theirs, 500));
    assert!(theirs.is_compatible(&ours, 500));
}

#[test]
fn test_fork_id_different_next_in_past_incompatible() {
    // We're at height 1500, they expect a fork at 1000 that we don't know about
    let ours = ForkId::new(0xDEADBEEF, 2000);
    let theirs = ForkId::new(0xDEADBEEF, 1000);

    // Their next fork is in our past (1000 <= 1500) but our next is different
    // This indicates they expect a fork we don't have
    assert!(!ours.is_compatible(&theirs, 1500));
}

#[test]
fn test_fork_id_different_next_in_future_compatible() {
    // We're at height 500, they expect fork at 1000, we expect at 2000
    let ours = ForkId::new(0xDEADBEEF, 2000);
    let theirs = ForkId::new(0xDEADBEEF, 1000);

    // Their next fork is in our future - we can still communicate
    assert!(ours.is_compatible(&theirs, 500));
}

#[test]
fn test_fork_id_is_stale() {
    let ours = ForkId::new(0xDEADBEEF, 2000);

    // Remote expects fork at 1000, we're at 1500 - they're stale
    assert!(ours.is_stale(1000, 1500));

    // Remote expects fork at 2000, we're at 1500 - not stale yet
    assert!(!ours.is_stale(2000, 1500));

    // Remote expects no fork (0) - not stale
    assert!(!ours.is_stale(0, 1500));

    // Remote expects same fork as us - not stale
    assert!(!ours.is_stale(2000, 2500));
}
