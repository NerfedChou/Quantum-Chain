//! Block-level Bloom filter
//!
//! Reference: SPEC-07 Section 2.1 - BlockFilter

use serde::{Deserialize, Serialize};
use shared_types::Hash;

use super::bloom_filter::BloomFilter;
use super::config::BloomConfig;

/// Bloom filter for a specific block
///
/// Contains all addresses involved in transactions within the block.
/// Used by light clients to quickly check if a block contains relevant txs.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockFilter {
    /// Hash of the block
    pub block_hash: Hash,
    /// Height of the block
    pub block_height: u64,
    /// The Bloom filter containing transaction addresses
    pub filter: BloomFilter,
    /// Number of transactions in the block
    pub transaction_count: u32,
}

impl BlockFilter {
    /// Create a new block filter
    pub fn new(
        block_hash: Hash,
        block_height: u64,
        addresses: &[[u8; 20]],
        config: &BloomConfig,
    ) -> Self {
        let mut filter = BloomFilter::new_with_fpr(addresses.len().max(1), config.target_fpr);

        for addr in addresses {
            filter.insert(addr);
        }

        Self {
            block_hash,
            block_height,
            filter,
            transaction_count: 0,
        }
    }

    /// Create a block filter with transaction count
    pub fn with_transaction_count(mut self, count: u32) -> Self {
        self.transaction_count = count;
        self
    }

    /// Check if an address might be in this block's transactions
    pub fn might_contain_address(&self, address: &[u8; 20]) -> bool {
        self.filter.contains(address)
    }

    /// Check if any of the given addresses might be in this block
    pub fn might_contain_any(&self, addresses: &[[u8; 20]]) -> bool {
        addresses.iter().any(|addr| self.filter.contains(addr))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> BloomConfig {
        BloomConfig {
            target_fpr: 0.05, // Valid FPR for testing
            ..Default::default()
        }
    }

    #[test]
    fn test_block_filter_creation() {
        let block_hash = [0xAB; 32];
        let addresses = vec![[0x11; 20], [0x22; 20], [0x33; 20]];

        let filter = BlockFilter::new(block_hash, 100, &addresses, &test_config());

        assert_eq!(filter.block_hash, block_hash);
        assert_eq!(filter.block_height, 100);
        assert!(filter.might_contain_address(&[0x11; 20]));
        assert!(filter.might_contain_address(&[0x22; 20]));
        assert!(filter.might_contain_address(&[0x33; 20]));
    }

    #[test]
    fn test_block_filter_might_contain_any() {
        let addresses = vec![[0x11; 20], [0x22; 20]];
        let filter = BlockFilter::new([0; 32], 1, &addresses, &test_config());

        // Should match if any address matches
        assert!(filter.might_contain_any(&[[0x11; 20], [0xFF; 20]]));
        assert!(filter.might_contain_any(&[[0x22; 20]]));
    }
}
