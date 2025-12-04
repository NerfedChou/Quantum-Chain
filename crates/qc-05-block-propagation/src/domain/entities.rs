//! # Core Domain Entities
//!
//! Defines the fundamental data structures for block propagation.
//!
//! ## Entities
//!
//! - [`BlockAnnouncement`]: Header-first block announcement (gossip trigger)
//! - [`CompactBlock`]: BIP152-style bandwidth-efficient block (short tx IDs)
//! - [`PrefilledTx`]: Transaction included in compact block (e.g., coinbase)
//! - [`PeerId`]: 32-byte peer identifier for P2P communication
//!
//! ## Wire Format Reference
//!
//! See SPEC-05 Appendix D for compact block wire format details.

use shared_types::Hash;

/// Block announcement for header-first propagation.
///
/// When a new block is produced, the node first broadcasts an announcement
/// containing only the header metadata. Peers can then request the full
/// block or compact block based on this announcement.
///
/// # Fields
///
/// - `block_hash`: SHA-256 hash of the block header
/// - `block_height`: Height in the blockchain (0 = genesis)
/// - `parent_hash`: Hash of the parent block (chain linkage)
/// - `timestamp`: Unix timestamp when block was produced
/// - `difficulty`: Mining/staking difficulty target
///
/// # Example
///
/// ```rust
/// use qc_05_block_propagation::BlockAnnouncement;
///
/// let announcement = BlockAnnouncement::new(
///     [0xAB; 32],  // block_hash
///     100,         // height
///     [0x00; 32],  // parent_hash
///     1701705600,  // timestamp
///     1000,        // difficulty
/// );
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockAnnouncement {
    /// SHA-256 hash of the block header.
    pub block_hash: Hash,
    /// Height in the blockchain (0 = genesis).
    pub block_height: u64,
    /// Hash of the parent block.
    pub parent_hash: Hash,
    /// Unix timestamp (seconds since epoch).
    pub timestamp: u64,
    /// Mining/staking difficulty target.
    pub difficulty: u128,
}

impl BlockAnnouncement {
    /// Creates a new block announcement.
    pub fn new(
        block_hash: Hash,
        block_height: u64,
        parent_hash: Hash,
        timestamp: u64,
        difficulty: u128,
    ) -> Self {
        Self {
            block_hash,
            block_height,
            parent_hash,
            timestamp,
            difficulty,
        }
    }
}

/// Short transaction ID (6 bytes) for compact block relay.
///
/// Calculated as: `SipHash-1-3(nonce, tx_hash)[0:6]`
///
/// The 6-byte size provides a good balance between collision resistance
/// (1 in 281 trillion for random transactions) and bandwidth savings.
///
/// # Reference
///
/// BIP152 (Bitcoin Improvement Proposal 152) - Compact Block Relay
pub type ShortTxId = [u8; 6];

/// Prefilled transaction included in a compact block.
///
/// Some transactions are included directly in the compact block rather
/// than as short IDs. This is used for:
///
/// 1. **Coinbase transaction**: Always prefilled (index 0)
/// 2. **Low-entropy transactions**: Transactions the sender believes
///    the receiver is unlikely to have in their mempool
///
/// # Fields
///
/// - `index`: Position in the block's transaction list
/// - `tx_hash`: Full 32-byte transaction hash
/// - `tx_data`: Serialized transaction data
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrefilledTx {
    /// Position in the block's transaction list.
    pub index: u16,
    /// Full 32-byte transaction hash.
    pub tx_hash: Hash,
    /// Serialized transaction data.
    pub tx_data: Vec<u8>,
}

/// Compact block for bandwidth-efficient relay (BIP152-style).
///
/// Instead of transmitting full transaction data, compact blocks contain:
/// 1. Block header
/// 2. Short transaction IDs (6 bytes each vs 32 bytes)
/// 3. A few prefilled transactions (coinbase, low-entropy)
///
/// The receiver reconstructs the full block by:
/// 1. Looking up transactions in their mempool using short IDs
/// 2. Requesting any missing transactions from the sender
///
/// # Bandwidth Savings
///
/// For a block with 2000 transactions:
/// - Full block: ~2000 × 250 bytes = 500 KB
/// - Compact block: ~2000 × 6 bytes = 12 KB (97% reduction)
///
/// # Wire Format
///
/// ```text
/// [header_hash: 32][height: 8][nonce: 8][count: 2][short_ids: 6*N][prefilled...]
/// ```
#[derive(Clone, Debug)]
pub struct CompactBlock {
    /// SHA-256 hash of the block header.
    pub header_hash: Hash,
    /// Height in the blockchain.
    pub block_height: u64,
    /// Hash of the parent block.
    pub parent_hash: Hash,
    /// Unix timestamp (seconds since epoch).
    pub timestamp: u64,
    /// Short transaction IDs (6 bytes each).
    pub short_txids: Vec<ShortTxId>,
    /// Prefilled transactions (coinbase + sender's choice).
    pub prefilled_txs: Vec<PrefilledTx>,
    /// Random nonce for short ID calculation (prevents precomputation attacks).
    pub nonce: u64,
}

impl CompactBlock {
    /// Creates a new compact block with empty transaction lists.
    pub fn new(
        header_hash: Hash,
        block_height: u64,
        parent_hash: Hash,
        timestamp: u64,
        nonce: u64,
    ) -> Self {
        Self {
            header_hash,
            block_height,
            parent_hash,
            timestamp,
            short_txids: Vec::new(),
            prefilled_txs: Vec::new(),
            nonce,
        }
    }


    /// Builder method: set short transaction IDs.
    pub fn with_short_txids(mut self, short_txids: Vec<ShortTxId>) -> Self {
        self.short_txids = short_txids;
        self
    }

    /// Builder method: set prefilled transactions.
    pub fn with_prefilled_txs(mut self, prefilled_txs: Vec<PrefilledTx>) -> Self {
        self.prefilled_txs = prefilled_txs;
        self
    }

    /// Returns indices of transactions not found in mempool.
    ///
    /// Used during compact block reconstruction to identify which
    /// transactions need to be requested from the sender.
    pub fn missing_indices(&self, found_txids: &[Option<Hash>]) -> Vec<u16> {
        found_txids
            .iter()
            .enumerate()
            .filter_map(|(i, opt)| if opt.is_none() { Some(i as u16) } else { None })
            .collect()
    }
}

/// Request for a full block by hash.
///
/// Sent when compact block reconstruction fails or when the receiver
/// doesn't support compact blocks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockRequest {
    /// Hash of the requested block.
    pub block_hash: Hash,
    /// Request ID for correlating responses.
    pub request_id: u64,
}

/// Response containing a full block.
#[derive(Clone, Debug)]
pub struct BlockResponse {
    /// Request ID from the original request.
    pub request_id: u64,
    /// Block hash (None if not found).
    pub block_hash: Option<Hash>,
    /// Serialized block data (None if not found).
    pub block_data: Option<Vec<u8>>,
}

/// Request for missing transactions during compact block reconstruction.
///
/// Sent after attempting to reconstruct a compact block when some
/// transactions are not found in the local mempool.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GetBlockTxnRequest {
    /// Hash of the block being reconstructed.
    pub block_hash: Hash,
    /// Indices of missing transactions in the block.
    pub indices: Vec<u16>,
}

/// Response with missing transactions for compact block reconstruction.
#[derive(Clone, Debug)]
pub struct BlockTxnResponse {
    /// Hash of the block being reconstructed.
    pub block_hash: Hash,
    /// Missing transactions: (index, serialized_tx_data).
    pub transactions: Vec<(u16, Vec<u8>)>,
}

/// Peer identifier for P2P network communication.
///
/// A 32-byte identifier derived from the peer's public key or
/// Kademlia node ID. Used for:
/// - Routing messages to specific peers
/// - Tracking per-peer state (rate limiting, reputation)
/// - Deduplication (which peer sent which block first)
///
/// # Example
///
/// ```rust
/// use qc_05_block_propagation::PeerId;
///
/// let peer = PeerId::new([0xAB; 32]);
/// let peer_from_bytes = PeerId::from_bytes(&[0xAB; 32]).unwrap();
/// assert_eq!(peer, peer_from_bytes);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PeerId(pub [u8; 32]);

impl PeerId {
    /// Creates a new peer ID from a 32-byte array.
    pub fn new(id: [u8; 32]) -> Self {
        Self(id)
    }

    /// Creates a peer ID from a byte slice.
    ///
    /// Returns `None` if the slice is shorter than 32 bytes.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() >= 32 {
            let mut id = [0u8; 32];
            id.copy_from_slice(&bytes[..32]);
            Some(Self(id))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_announcement_creation() {
        let ann = BlockAnnouncement::new([1u8; 32], 100, [0u8; 32], 1234567890, 1000);
        assert_eq!(ann.block_height, 100);
        assert_eq!(ann.block_hash, [1u8; 32]);
    }

    #[test]
    fn test_compact_block_missing_indices() {
        let compact = CompactBlock::new([1u8; 32], 100, [0u8; 32], 0, 42);
        let found = vec![Some([1u8; 32]), None, Some([2u8; 32]), None];
        let missing = compact.missing_indices(&found);
        assert_eq!(missing, vec![1, 3]);
    }

    #[test]
    fn test_peer_id_from_bytes() {
        let bytes = [0xABu8; 32];
        let peer = PeerId::from_bytes(&bytes);
        assert!(peer.is_some());
        assert_eq!(peer.unwrap().0, bytes);
    }
}
