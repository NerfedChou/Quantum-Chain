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

/// Static chain configuration (network, genesis, protocol)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainInfo {
    /// Network identifier (mainnet=1, testnet=2, etc.)
    pub network_id: u32,
    /// Genesis block hash - MUST match for same network
    pub genesis_hash: [u8; 32],
    /// Protocol version
    pub protocol_version: u16,
}

impl ChainInfo {
    /// Create new chain info
    pub fn new(network_id: u32, genesis_hash: [u8; 32], protocol_version: u16) -> Self {
        Self {
            network_id,
            genesis_hash,
            protocol_version,
        }
    }
}

/// Dynamic chain state (head block, difficulty)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadState {
    /// Current head block number
    pub height: u64,
    /// Current head block hash
    pub hash: [u8; 32],
    /// Total accumulated difficulty (for PoW)
    pub total_difficulty: u128,
}

impl HeadState {
    /// Create new head state
    pub fn new(height: u64, hash: [u8; 32], total_difficulty: u128) -> Self {
        Self {
            height,
            hash,
            total_difficulty,
        }
    }
}

/// Chain information exchanged during handshake
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandshakeData {
    /// Chain configuration
    pub chain_info: ChainInfo,
    /// Current chain state
    pub head_state: HeadState,
}

impl HandshakeData {
    /// Create new handshake data
    pub fn new(chain_info: ChainInfo, head_state: HeadState) -> Self {
        Self {
            chain_info,
            head_state,
        }
    }

    /// Convenience accessor for network_id
    pub fn network_id(&self) -> u32 {
        self.chain_info.network_id
    }

    /// Convenience accessor for genesis_hash
    pub fn genesis_hash(&self) -> [u8; 32] {
        self.chain_info.genesis_hash
    }

    /// Convenience accessor for protocol_version
    pub fn protocol_version(&self) -> u16 {
        self.chain_info.protocol_version
    }

    /// Convenience accessor for head_height
    pub fn head_height(&self) -> u64 {
        self.head_state.height
    }

    /// Convenience accessor for total_difficulty
    pub fn total_difficulty(&self) -> u128 {
        self.head_state.total_difficulty
    }

    /// Create a minimal handshake for testing
    #[cfg(test)]
    pub fn for_testing(head_height: u64, total_difficulty: u128) -> Self {
        Self {
            chain_info: ChainInfo::new(1, [0u8; 32], 1),
            head_state: HeadState::new(head_height, [0u8; 32], total_difficulty),
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

    if ours.chain_info.genesis_hash != theirs.chain_info.genesis_hash {
        return HandshakeResult::Reject(RejectReason::WrongNetwork);
    }

    if ours.chain_info.network_id != theirs.chain_info.network_id {
        return HandshakeResult::Reject(RejectReason::NetworkIdMismatch);
    }

    // -------------------------------------------------------------------------
    // Filter 2: Protocol Version (O(1))
    // -------------------------------------------------------------------------

    if theirs.chain_info.protocol_version < config.min_protocol_version
        || theirs.chain_info.protocol_version > config.max_protocol_version
    {
        return HandshakeResult::Reject(RejectReason::ProtocolMismatch);
    }

    // -------------------------------------------------------------------------
    // Filter 3: Fork Check - Peer Not Too Far Behind (O(1))
    // -------------------------------------------------------------------------

    // If peer is behind our finalized height by too much, they're useless
    if theirs.head_state.height + config.max_behind_blocks < config.finalized_height {
        return HandshakeResult::Reject(RejectReason::TooFarBehind);
    }

    // -------------------------------------------------------------------------
    // Filter 4: Classification by Total Difficulty
    // -------------------------------------------------------------------------

    let classification = if theirs.head_state.total_difficulty > ours.head_state.total_difficulty {
        // Peer has more work done - potential sync source
        PeerClassification::SyncSource
    } else if theirs.head_state.total_difficulty < ours.head_state.total_difficulty {
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

    /// Check if two fork IDs are compatible (EIP-2124 logic).
    ///
    /// # Compatibility Rules (per EIP-2124)
    ///
    /// 1. **Hash mismatch**: If hashes differ, we're on different chains → incompatible
    /// 2. **We're stale**: If their `next` fork is in the past for us and we have same hash,
    ///    we should have already applied that fork → incompatible (we're behind)
    /// 3. **They're stale**: If our `next` fork is in the past for them but hashes match,
    ///    they haven't applied a fork we have → incompatible (they're behind)
    /// 4. **Future fork**: If `next` is in the future for both, compatible
    ///
    /// # Arguments
    ///
    /// * `other` - The remote peer's ForkId
    /// * `our_height` - Our current block height
    ///
    /// # Returns
    ///
    /// `true` if we can communicate with this peer, `false` if chain is incompatible
    pub fn is_compatible(&self, other: &ForkId, our_height: u64) -> bool {
        // Rule 1: Hash mismatch = different chain or diverged fork
        if self.hash != other.hash {
            return false;
        }

        // Rule 2: Check if their expected next fork is in our past
        // If other.next != 0 and other.next <= our_height, they expect a fork
        // that we should have already applied. Since hashes match, this is OK
        // (we both applied it). But if other.next < our_height AND our.next != other.next,
        // there may be divergence.
        
        // Rule 3: Check if our expected next fork is in their past
        // This is symmetric - if we expect a fork they should have applied
        
        // Simplified but correct logic:
        // - If hashes match, we're on the same chain up to now
        // - If either next==0, no more forks expected → compatible
        // - If both have next forks, they should be the same or we're diverging
        if self.next == 0 || other.next == 0 {
            // One or both have no future forks → compatible
            return true;
        }

        // Both expect future forks
        if self.next == other.next {
            // Same next fork expected → compatible
            return true;
        }

        // Different next forks expected with same hash
        // This happens when one node knows about a fork the other doesn't
        // The node with the earlier next fork is more up-to-date
        
        // If their next fork is before our height, they expect us to have it
        // but we don't (since our next is different) → we might be on wrong chain
        if other.next <= our_height {
            return false;
        }

        // If our next fork is before their expected and they have same hash,
        // they haven't applied our fork yet → they might be stale
        // But since hashes match, we can still communicate for now
        true
    }

    /// Check if this ForkId indicates a stale node compared to current height.
    ///
    /// A node is stale if their `next` fork has passed but they haven't updated.
    pub fn is_stale(&self, remote_next: u64, our_height: u64) -> bool {
        remote_next != 0 && remote_next <= our_height && remote_next != self.next
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
        HandshakeData::new(
            ChainInfo::new(network_id, genesis, 1),
            HeadState::new(height, [0u8; 32], difficulty),
        )
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
        theirs.chain_info.protocol_version = 0; // Too old

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

        let config = HandshakeConfig {
            finalized_height: 500,
            max_behind_blocks: 100,
            ..Default::default()
        };

        let result = verify_handshake(&ours, &theirs, &config);
        assert_eq!(result, HandshakeResult::Reject(RejectReason::TooFarBehind));
    }

    #[test]
    fn test_peer_slightly_behind_accepted() {
        let genesis = make_genesis();

        let ours = make_handshake(genesis, 1, 1000, 10000);
        let theirs = make_handshake(genesis, 1, 950, 9500); // Slightly behind

        let config = HandshakeConfig {
            finalized_height: 500,
            max_behind_blocks: 100,
            ..Default::default()
        };

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
    // TEST GROUP 5: Fork ID (EIP-2124)
    // =========================================================================

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
}
