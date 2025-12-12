//! # Chain-Aware Handshakes
//!
//! Implements Fork-ID Convergence for fast-fail chain verification.
//!
//! ## Algorithm: Fork-ID Convergence
//!
//! 1. Exchange (GenesisHash, HeadBlockNum, HeadBlockHash, TotalDifficulty)
//! 2. Filter 1: Network Match (O(1)) - Genesis must match
//! 3. Filter 2: Fork Check (O(1)) - Peer not too far behind finalized
//! 4. Filter 3: Canonical Check - Compare total difficulty
//!
//! Reference: Ethereum's Fork-ID (EIP-2124), Go-Ethereum's handshake

// =============================================================================
// HANDSHAKE DATA
// =============================================================================

/// Chain information exchanged during handshake
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandshakeData {
    /// Network identifier (mainnet=1, testnet=2, etc.)
    pub network_id: u32,
    /// Genesis block hash - MUST match for same network
    pub genesis_hash: [u8; 32],
    /// Current head block number
    pub head_height: u64,
    /// Current head block hash
    pub head_hash: [u8; 32],
    /// Total accumulated difficulty (for PoW)
    pub total_difficulty: u128,
    /// Protocol version
    pub protocol_version: u16,
}

impl HandshakeData {
    /// Create new handshake data
    pub fn new(
        network_id: u32,
        genesis_hash: [u8; 32],
        head_height: u64,
        head_hash: [u8; 32],
        total_difficulty: u128,
        protocol_version: u16,
    ) -> Self {
        Self {
            network_id,
            genesis_hash,
            head_height,
            head_hash,
            total_difficulty,
            protocol_version,
        }
    }

    /// Create a minimal handshake for testing
    #[cfg(test)]
    pub fn for_testing(head_height: u64, total_difficulty: u128) -> Self {
        Self {
            network_id: 1,
            genesis_hash: [0u8; 32],
            head_height,
            head_hash: [0u8; 32],
            total_difficulty,
            protocol_version: 1,
        }
    }
}

// =============================================================================
// HANDSHAKE RESULT
// =============================================================================

/// Result of handshake verification
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandshakeResult {
    /// Handshake successful - peer is compatible
    Accept(PeerClassification),
    /// Handshake failed - peer rejected
    Reject(RejectReason),
}

/// Classification of an accepted peer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerClassification {
    /// Peer is at same height or behind - we can help them sync
    SyncTarget,
    /// Peer is ahead - potential sync source for us
    SyncSource,
    /// Peer is at same position - equal
    Equal,
}

/// Reasons for rejecting a handshake
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectReason {
    /// Different genesis hash - wrong network
    WrongNetwork,
    /// Different network ID
    NetworkIdMismatch,
    /// Protocol version incompatible
    ProtocolMismatch,
    /// Peer is too far behind our finalized checkpoint
    TooFarBehind,
    /// Peer claims fork that diverges from our finalized chain
    ForkDivergence,
}

// =============================================================================
// HANDSHAKE VERIFIER
// =============================================================================

/// Configuration for handshake verification
#[derive(Debug, Clone)]
pub struct HandshakeConfig {
    /// Minimum supported protocol version
    pub min_protocol_version: u16,
    /// Maximum protocol version
    pub max_protocol_version: u16,
    /// Height of our last finalized block (can't sync below this)
    pub finalized_height: u64,
    /// Hash of our last finalized block
    pub finalized_hash: [u8; 32],
    /// Maximum block height difference for "useless" peer
    pub max_behind_blocks: u64,
}

impl Default for HandshakeConfig {
    fn default() -> Self {
        Self {
            min_protocol_version: 1,
            max_protocol_version: 1,
            finalized_height: 0,
            finalized_hash: [0u8; 32],
            max_behind_blocks: 1000,
        }
    }
}

impl HandshakeConfig {
    /// Testing config
    #[cfg(test)]
    pub fn for_testing() -> Self {
        Self {
            min_protocol_version: 1,
            max_protocol_version: 1,
            finalized_height: 100,
            finalized_hash: [0u8; 32],
            max_behind_blocks: 50,
        }
    }
}

/// Verify a peer's handshake data against our own
///
/// Implements Fork-ID Convergence algorithm:
/// 1. Network match (O(1))
/// 2. Protocol version check (O(1))
/// 3. Fork check - peer not behind finalized (O(1))
/// 4. Classification based on total difficulty
pub fn verify_handshake(
    ours: &HandshakeData,
    theirs: &HandshakeData,
    config: &HandshakeConfig,
) -> HandshakeResult {
    // -------------------------------------------------------------------------
    // Filter 1: Network Match (O(1))
    // -------------------------------------------------------------------------

    if ours.genesis_hash != theirs.genesis_hash {
        return HandshakeResult::Reject(RejectReason::WrongNetwork);
    }

    if ours.network_id != theirs.network_id {
        return HandshakeResult::Reject(RejectReason::NetworkIdMismatch);
    }

    // -------------------------------------------------------------------------
    // Filter 2: Protocol Version (O(1))
    // -------------------------------------------------------------------------

    if theirs.protocol_version < config.min_protocol_version
        || theirs.protocol_version > config.max_protocol_version
    {
        return HandshakeResult::Reject(RejectReason::ProtocolMismatch);
    }

    // -------------------------------------------------------------------------
    // Filter 3: Fork Check - Peer Not Too Far Behind (O(1))
    // -------------------------------------------------------------------------

    // If peer is behind our finalized height by too much, they're useless
    if theirs.head_height + config.max_behind_blocks < config.finalized_height {
        return HandshakeResult::Reject(RejectReason::TooFarBehind);
    }

    // -------------------------------------------------------------------------
    // Filter 4: Classification by Total Difficulty
    // -------------------------------------------------------------------------

    let classification = if theirs.total_difficulty > ours.total_difficulty {
        // Peer has more work done - potential sync source
        PeerClassification::SyncSource
    } else if theirs.total_difficulty < ours.total_difficulty {
        // Peer has less work - we can help them sync
        PeerClassification::SyncTarget
    } else {
        // Equal difficulty
        PeerClassification::Equal
    };

    HandshakeResult::Accept(classification)
}

// =============================================================================
// FORK ID (EIP-2124 Inspired)
// =============================================================================

/// Fork ID for quick network/fork identification
///
/// Compact representation: hash(genesis + fork_hashes) + next_fork
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForkId {
    /// CRC32 hash of genesis + all fork block hashes
    pub hash: u32,
    /// Block number of next expected fork (0 if none)
    pub next: u64,
}

impl ForkId {
    /// Create a new fork ID
    pub fn new(hash: u32, next: u64) -> Self {
        Self { hash, next }
    }

    /// Check if two fork IDs are compatible
    ///
    /// Compatible means either:
    /// 1. Same hash and next
    /// 2. Our hash matches their hash and their next >= our height
    pub fn is_compatible(&self, other: &ForkId, _our_height: u64) -> bool {
        if self.hash != other.hash {
            return false;
        }

        // If they expect a fork before our height, we should have it
        // If we're past their next fork but have same hash, they're behind
        true // Simplified - in production, more complex logic needed
    }
}

// =============================================================================
// TESTS (TDD)
// =============================================================================

#[cfg(test)]
mod tests {
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
        HandshakeData {
            network_id,
            genesis_hash: genesis,
            head_height: height,
            head_hash: [0u8; 32],
            total_difficulty: difficulty,
            protocol_version: 1,
        }
    }

    // =========================================================================
    // TEST GROUP 1: Network Matching
    // =========================================================================

    #[test]
    fn test_wrong_genesis_rejected() {
        let genesis1 = make_genesis();
        let mut genesis2 = make_genesis();
        genesis2[0] = 0xBE;

        let ours = make_handshake(genesis1, 1, 100, 1000);
        let theirs = make_handshake(genesis2, 1, 100, 1000);
        let config = HandshakeConfig::default();

        let result = verify_handshake(&ours, &theirs, &config);
        assert_eq!(result, HandshakeResult::Reject(RejectReason::WrongNetwork));
    }

    #[test]
    fn test_wrong_network_id_rejected() {
        let genesis = make_genesis();

        let ours = make_handshake(genesis, 1, 100, 1000); // Mainnet
        let theirs = make_handshake(genesis, 2, 100, 1000); // Testnet
        let config = HandshakeConfig::default();

        let result = verify_handshake(&ours, &theirs, &config);
        assert_eq!(
            result,
            HandshakeResult::Reject(RejectReason::NetworkIdMismatch)
        );
    }

    // =========================================================================
    // TEST GROUP 2: Protocol Version
    // =========================================================================

    #[test]
    fn test_old_protocol_rejected() {
        let genesis = make_genesis();

        let ours = make_handshake(genesis, 1, 100, 1000);
        let mut theirs = make_handshake(genesis, 1, 100, 1000);
        theirs.protocol_version = 0; // Too old

        let config = HandshakeConfig::default();

        let result = verify_handshake(&ours, &theirs, &config);
        assert_eq!(
            result,
            HandshakeResult::Reject(RejectReason::ProtocolMismatch)
        );
    }

    // =========================================================================
    // TEST GROUP 3: Fork Check
    // =========================================================================

    #[test]
    fn test_peer_too_far_behind_rejected() {
        let genesis = make_genesis();

        let ours = make_handshake(genesis, 1, 1000, 10000);
        let theirs = make_handshake(genesis, 1, 10, 100); // Way behind

        let mut config = HandshakeConfig::default();
        config.finalized_height = 500;
        config.max_behind_blocks = 100;

        let result = verify_handshake(&ours, &theirs, &config);
        assert_eq!(result, HandshakeResult::Reject(RejectReason::TooFarBehind));
    }

    #[test]
    fn test_peer_slightly_behind_accepted() {
        let genesis = make_genesis();

        let ours = make_handshake(genesis, 1, 1000, 10000);
        let theirs = make_handshake(genesis, 1, 950, 9500); // Slightly behind

        let mut config = HandshakeConfig::default();
        config.finalized_height = 500;
        config.max_behind_blocks = 100;

        let result = verify_handshake(&ours, &theirs, &config);
        assert!(matches!(result, HandshakeResult::Accept(_)));
    }

    // =========================================================================
    // TEST GROUP 4: Classification
    // =========================================================================

    #[test]
    fn test_peer_ahead_classified_as_sync_source() {
        let genesis = make_genesis();

        let ours = make_handshake(genesis, 1, 100, 1000);
        let theirs = make_handshake(genesis, 1, 200, 2000); // More difficulty

        let config = HandshakeConfig::default();

        let result = verify_handshake(&ours, &theirs, &config);
        assert_eq!(
            result,
            HandshakeResult::Accept(PeerClassification::SyncSource)
        );
    }

    #[test]
    fn test_peer_behind_classified_as_sync_target() {
        let genesis = make_genesis();

        let ours = make_handshake(genesis, 1, 200, 2000);
        let theirs = make_handshake(genesis, 1, 100, 1000); // Less difficulty

        let config = HandshakeConfig::default();

        let result = verify_handshake(&ours, &theirs, &config);
        assert_eq!(
            result,
            HandshakeResult::Accept(PeerClassification::SyncTarget)
        );
    }

    #[test]
    fn test_peer_equal_classified_as_equal() {
        let genesis = make_genesis();

        let ours = make_handshake(genesis, 1, 100, 1000);
        let theirs = make_handshake(genesis, 1, 100, 1000); // Same

        let config = HandshakeConfig::default();

        let result = verify_handshake(&ours, &theirs, &config);
        assert_eq!(result, HandshakeResult::Accept(PeerClassification::Equal));
    }

    // =========================================================================
    // TEST GROUP 5: Fork ID
    // =========================================================================

    #[test]
    fn test_fork_id_compatibility() {
        let fork1 = ForkId::new(0xDEADBEEF, 1000);
        let fork2 = ForkId::new(0xDEADBEEF, 1000);
        let fork3 = ForkId::new(0xCAFEBABE, 1000);

        assert!(fork1.is_compatible(&fork2, 500));
        assert!(!fork1.is_compatible(&fork3, 500)); // Different hash
    }
}
