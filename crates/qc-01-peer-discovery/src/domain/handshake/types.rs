//! Handshake data types.

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
