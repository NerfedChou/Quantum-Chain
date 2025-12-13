//! Domain services for block propagation.

use shared_types::Hash;
use siphasher::sip::SipHasher13;
use std::hash::Hasher;

use super::{CompactBlock, PeerPropagationState, PrefilledTx, PropagationConfig, ShortTxId};

/// Calculate short transaction ID using SipHash.
///
/// Formula: short_id = SipHash-1-3(nonce, tx_hash)\[0:6\]
///
/// Reference: BIP152 (Bitcoin Improvement Proposal 152)
pub fn calculate_short_id(tx_hash: &Hash, nonce: u64) -> ShortTxId {
    let mut hasher = SipHasher13::new_with_keys(nonce, 0);
    hasher.write(tx_hash);
    let full_hash = hasher.finish();

    let mut short_id = [0u8; 6];
    short_id.copy_from_slice(&full_hash.to_le_bytes()[..6]);
    short_id
}

/// Parameters for creating a compact block.
///
/// Groups the arguments needed for `create_compact_block` to reduce parameter count.
#[derive(Clone, Copy)]
pub struct CompactBlockParams<'a> {
    /// Block header hash
    pub header_hash: Hash,
    /// Block height
    pub block_height: u64,
    /// Parent block hash
    pub parent_hash: Hash,
    /// Block timestamp
    pub timestamp: u64,
    /// Transaction hashes in the block
    pub tx_hashes: &'a [Hash],
    /// Nonce for short ID calculation
    pub nonce: u64,
    /// Indices of transactions to prefill
    pub prefill_indices: &'a [usize],
}

/// Create a compact block from full block data.
pub fn create_compact_block(params: CompactBlockParams<'_>) -> CompactBlock {
    let short_txids: Vec<ShortTxId> = params
        .tx_hashes
        .iter()
        .map(|hash| calculate_short_id(hash, params.nonce))
        .collect();

    let prefilled_txs: Vec<PrefilledTx> = params
        .prefill_indices
        .iter()
        .filter_map(|&i| {
            if i < params.tx_hashes.len() {
                Some(PrefilledTx {
                    index: i as u16,
                    tx_hash: params.tx_hashes[i],
                    tx_data: Vec::new(), // Actual tx data would be provided
                })
            } else {
                None
            }
        })
        .collect();

    CompactBlock::new(
        params.header_hash,
        params.block_height,
        params.parent_hash,
        params.timestamp,
        params.nonce,
    )
    .with_short_txids(short_txids)
    .with_prefilled_txs(prefilled_txs)
}

/// Result of compact block reconstruction.
#[derive(Debug)]
pub enum ReconstructionResult {
    /// Block successfully reconstructed
    Success {
        block_hash: Hash,
        tx_hashes: Vec<Hash>,
    },
    /// Missing transactions at these indices
    MissingTransactions { indices: Vec<u16> },
}

/// Reconstruct block from compact block and mempool lookup.
pub fn reconstruct_block<F>(compact: &CompactBlock, mut mempool_lookup: F) -> ReconstructionResult
where
    F: FnMut(&[ShortTxId], u64) -> Vec<Option<Hash>>,
{
    // Get transactions from mempool by short IDs
    let found_txs = mempool_lookup(&compact.short_txids, compact.nonce);

    // Check for missing transactions
    let missing: Vec<u16> = found_txs
        .iter()
        .enumerate()
        .filter_map(|(i, opt)| {
            // Check if this index is prefilled
            let is_prefilled = compact.prefilled_txs.iter().any(|p| p.index == i as u16);
            if opt.is_none() && !is_prefilled {
                Some(i as u16)
            } else {
                None
            }
        })
        .collect();

    if missing.is_empty() {
        // Reconstruct full transaction list
        let tx_hashes: Vec<Hash> = found_txs
            .into_iter()
            .enumerate()
            .map(|(i, opt)| {
                // Use prefilled tx if available, otherwise use mempool tx
                compact
                    .prefilled_txs
                    .iter()
                    .find(|p| p.index == i as u16)
                    .map(|p| p.tx_hash)
                    .or(opt)
                    .unwrap_or([0u8; 32])
            })
            .collect();

        ReconstructionResult::Success {
            block_hash: compact.header_hash,
            tx_hashes,
        }
    } else {
        ReconstructionResult::MissingTransactions { indices: missing }
    }
}

/// Check if peer is within rate limit.
pub fn check_rate_limit(peer: &PeerPropagationState, config: &PropagationConfig) -> bool {
    peer.announcement_count < config.max_announcements_per_second
}

/// Select peers for block propagation based on reputation.
/// Returns the top N peers by reputation score.
pub fn select_peers_for_propagation(
    peers: &[PeerPropagationState],
    fanout: usize,
) -> Vec<PeerPropagationState> {
    let mut sorted_peers = peers.to_vec();
    // Use total_cmp for safe f64 comparison (handles NaN gracefully)
    sorted_peers.sort_by(|a, b| b.reputation.total_cmp(&a.reputation));
    sorted_peers.truncate(fanout);
    sorted_peers
}

/// Validate block size against configuration.
pub fn validate_block_size(block_size: usize, config: &PropagationConfig) -> bool {
    block_size <= config.max_block_size_bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::PeerId;

    #[test]
    fn test_short_id_calculation() {
        let tx_hash = [0xABu8; 32];
        let nonce = 12345u64;

        let short_id = calculate_short_id(&tx_hash, nonce);

        // Short ID should be 6 bytes
        assert_eq!(short_id.len(), 6);

        // Same inputs should produce same output
        let short_id2 = calculate_short_id(&tx_hash, nonce);
        assert_eq!(short_id, short_id2);

        // Different nonce should produce different output
        let short_id3 = calculate_short_id(&tx_hash, nonce + 1);
        assert_ne!(short_id, short_id3);
    }

    #[test]
    fn test_short_id_collision_resistance() {
        use std::collections::HashSet;

        let mut short_ids = HashSet::new();
        let nonce = 42u64;

        // Generate 10,000 short IDs and check for collisions
        for i in 0..10_000u32 {
            let mut tx_hash = [0u8; 32];
            tx_hash[..4].copy_from_slice(&i.to_le_bytes());
            let short_id = calculate_short_id(&tx_hash, nonce);
            assert!(short_ids.insert(short_id), "Collision at index {}", i);
        }
    }

    #[test]
    fn test_create_compact_block() {
        let tx_hashes = vec![[1u8; 32], [2u8; 32], [3u8; 32]];
        let compact = create_compact_block(CompactBlockParams {
            header_hash: [0xAAu8; 32],
            block_height: 100,
            parent_hash: [0u8; 32],
            timestamp: 1234567890,
            tx_hashes: &tx_hashes,
            nonce: 42,
            prefill_indices: &[0], // Prefill first tx (coinbase)
        });

        assert_eq!(compact.block_height, 100);
        assert_eq!(compact.short_txids.len(), 3);
        assert_eq!(compact.prefilled_txs.len(), 1);
        assert_eq!(compact.prefilled_txs[0].index, 0);
    }

    #[test]
    fn test_reconstruct_block_success() {
        let tx_hashes = vec![[1u8; 32], [2u8; 32], [3u8; 32]];
        let compact = create_compact_block(CompactBlockParams {
            header_hash: [0xAAu8; 32],
            block_height: 100,
            parent_hash: [0u8; 32],
            timestamp: 0,
            tx_hashes: &tx_hashes,
            nonce: 42,
            prefill_indices: &[],
        });

        // Mock mempool that has all transactions
        let mempool: std::collections::HashMap<ShortTxId, Hash> = tx_hashes
            .iter()
            .map(|h| (calculate_short_id(h, 42), *h))
            .collect();

        let result = reconstruct_block(&compact, |ids, _nonce| {
            ids.iter().map(|id| mempool.get(id).copied()).collect()
        });

        match result {
            ReconstructionResult::Success {
                block_hash,
                tx_hashes: reconstructed,
            } => {
                assert_eq!(block_hash, [0xAAu8; 32]);
                assert_eq!(reconstructed.len(), 3);
            }
            _ => panic!("Expected successful reconstruction"),
        }
    }

    #[test]
    fn test_reconstruct_block_missing_tx() {
        let tx_hashes = vec![[1u8; 32], [2u8; 32], [3u8; 32]];
        let compact = create_compact_block(CompactBlockParams {
            header_hash: [0xAAu8; 32],
            block_height: 100,
            parent_hash: [0u8; 32],
            timestamp: 0,
            tx_hashes: &tx_hashes,
            nonce: 42,
            prefill_indices: &[],
        });

        // Mock mempool missing one transaction
        let mempool: std::collections::HashMap<ShortTxId, Hash> = tx_hashes[..2]
            .iter()
            .map(|h| (calculate_short_id(h, 42), *h))
            .collect();

        let result = reconstruct_block(&compact, |ids, _nonce| {
            ids.iter().map(|id| mempool.get(id).copied()).collect()
        });

        match result {
            ReconstructionResult::MissingTransactions { indices } => {
                assert_eq!(indices, vec![2]);
            }
            _ => panic!("Expected missing transactions"),
        }
    }

    #[test]
    fn test_rate_limit_check() {
        let config = PropagationConfig {
            max_announcements_per_second: 2,
            ..Default::default()
        };

        let peer_id = PeerId::new([1u8; 32]);
        let mut peer = PeerPropagationState::new(peer_id);

        assert!(check_rate_limit(&peer, &config));
        peer.record_announcement();
        assert!(check_rate_limit(&peer, &config));
        peer.record_announcement();
        assert!(!check_rate_limit(&peer, &config));
    }

    #[test]
    fn test_peer_selection_by_reputation() {
        let peers = vec![
            {
                let mut p = PeerPropagationState::new(PeerId::new([1u8; 32]));
                p.reputation = 0.3;
                p
            },
            {
                let mut p = PeerPropagationState::new(PeerId::new([2u8; 32]));
                p.reputation = 0.9;
                p
            },
            {
                let mut p = PeerPropagationState::new(PeerId::new([3u8; 32]));
                p.reputation = 0.6;
                p
            },
        ];

        let selected = select_peers_for_propagation(&peers, 2);

        assert_eq!(selected.len(), 2);
        assert_eq!(selected[0].reputation, 0.9);
        assert_eq!(selected[1].reputation, 0.6);
    }

    #[test]
    fn test_block_size_validation() {
        let config = PropagationConfig {
            max_block_size_bytes: 1000,
            ..Default::default()
        };

        assert!(validate_block_size(500, &config));
        assert!(validate_block_size(1000, &config));
        assert!(!validate_block_size(1001, &config));
    }
}
