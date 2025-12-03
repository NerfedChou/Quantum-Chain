//! Core domain entities for block propagation.

use shared_types::Hash;

/// Block announcement for header-first propagation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockAnnouncement {
    pub block_hash: Hash,
    pub block_height: u64,
    pub parent_hash: Hash,
    pub timestamp: u64,
    pub difficulty: u128,
}

impl BlockAnnouncement {
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
/// Calculated via SipHash(nonce || tx_hash)[0:6].
pub type ShortTxId = [u8; 6];

/// Prefilled transaction with index in block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrefilledTx {
    pub index: u16,
    pub tx_hash: Hash,
    pub tx_data: Vec<u8>,
}

/// Compact block for bandwidth-efficient relay (BIP152-style).
#[derive(Clone, Debug)]
pub struct CompactBlock {
    pub header_hash: Hash,
    pub block_height: u64,
    pub parent_hash: Hash,
    pub timestamp: u64,
    /// Short transaction IDs (first 6 bytes of SipHash)
    pub short_txids: Vec<ShortTxId>,
    /// Prefilled transactions (coinbase + any sender thinks receiver lacks)
    pub prefilled_txs: Vec<PrefilledTx>,
    /// Salt for short ID calculation (random per block)
    pub nonce: u64,
}

impl CompactBlock {
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

    pub fn with_short_txids(mut self, short_txids: Vec<ShortTxId>) -> Self {
        self.short_txids = short_txids;
        self
    }

    pub fn with_prefilled_txs(mut self, prefilled_txs: Vec<PrefilledTx>) -> Self {
        self.prefilled_txs = prefilled_txs;
        self
    }

    /// Returns indices of transactions that need to be fetched.
    pub fn missing_indices(&self, found_txids: &[Option<Hash>]) -> Vec<u16> {
        found_txids
            .iter()
            .enumerate()
            .filter_map(|(i, opt)| if opt.is_none() { Some(i as u16) } else { None })
            .collect()
    }
}

/// Full block request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockRequest {
    pub block_hash: Hash,
    pub request_id: u64,
}

/// Full block response.
#[derive(Clone, Debug)]
pub struct BlockResponse {
    pub request_id: u64,
    pub block_hash: Option<Hash>,
    pub block_data: Option<Vec<u8>>,
}

/// Request for missing transactions during compact block reconstruction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GetBlockTxnRequest {
    pub block_hash: Hash,
    pub indices: Vec<u16>,
}

/// Response with missing transactions.
#[derive(Clone, Debug)]
pub struct BlockTxnResponse {
    pub block_hash: Hash,
    pub transactions: Vec<(u16, Vec<u8>)>,
}

/// Peer identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PeerId(pub [u8; 32]);

impl PeerId {
    pub fn new(id: [u8; 32]) -> Self {
        Self(id)
    }

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
