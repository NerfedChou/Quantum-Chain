//! # Versioned State Cache (Reorg-Aware LRU)
//!
//! A height-aware cache that handles chain reorganizations safely.
//!
//! ## Problem
//!
//! Standard LRU cache is dangerous in blockchain:
//! - Cache state at Block 100
//! - Chain reorgs to different Block 100 (fork)
//! - Cache contains "dirty" state from abandoned chain
//!
//! ## Solution: Epoch-Tagged Cache
//!
//! Tag cache entries with block hash/height and flush on reorg.

use super::{AccountState, Address, Hash as BlockHash, MAX_CACHED_ACCOUNTS};
use lru::LruCache;
use std::num::NonZeroUsize;

/// Cache key includes block hash for reorg safety.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CacheKey {
    pub address: Address,
    pub block_hash: BlockHash,
}

impl CacheKey {
    pub fn new(address: Address, block_hash: BlockHash) -> Self {
        Self { address, block_hash }
    }
}

/// Versioned account cache with reorg awareness.
///
/// ## Algorithm: Epoch-Tagged Cache
///
/// - Read: Checks current head block hash
/// - Write: Tags entry with current head
/// - Reorg: Marks abandoned chain entries as stale
pub struct VersionedAccountCache {
    /// LRU cache with (address, block_hash) keys
    cache: LruCache<CacheKey, AccountState>,
    /// Current chain head hash
    current_head: BlockHash,
    /// Current chain head height
    current_height: u64,
    /// Stale block hashes (from abandoned forks)
    stale_hashes: Vec<BlockHash>,
}

impl VersionedAccountCache {
    /// Create a new versioned cache.
    pub fn new() -> Self {
        let cap = NonZeroUsize::new(MAX_CACHED_ACCOUNTS).unwrap();
        Self {
            cache: LruCache::new(cap),
            current_head: [0; 32],
            current_height: 0,
            stale_hashes: Vec::new(),
        }
    }

    /// Create with custom capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        let cap = NonZeroUsize::new(capacity.max(1)).unwrap();
        Self {
            cache: LruCache::new(cap),
            current_head: [0; 32],
            current_height: 0,
            stale_hashes: Vec::new(),
        }
    }

    /// Update chain head (called on new block).
    pub fn set_head(&mut self, block_hash: BlockHash, height: u64) {
        self.current_head = block_hash;
        self.current_height = height;
    }

    /// Get account state at current head.
    pub fn get(&mut self, address: &Address) -> Option<&AccountState> {
        let key = CacheKey::new(*address, self.current_head);
        self.cache.get(&key)
    }

    /// Put account state tagged with current head.
    pub fn put(&mut self, address: Address, state: AccountState) {
        let key = CacheKey::new(address, self.current_head);
        self.cache.put(key, state);
    }

    /// Handle chain reorganization.
    ///
    /// Marks entries from the abandoned chain as stale.
    /// They will be lazily removed on next access.
    pub fn handle_reorg(&mut self, old_head: BlockHash, new_head: BlockHash) {
        self.stale_hashes.push(old_head);
        self.current_head = new_head;
        
        // Prune stale entries if list grows too large
        if self.stale_hashes.len() > 100 {
            self.flush_stale();
        }
    }

    /// Flush all stale entries from abandoned forks.
    pub fn flush_stale(&mut self) {
        // Collect keys to remove
        let keys_to_remove: Vec<CacheKey> = self.cache
            .iter()
            .filter(|(k, _)| self.stale_hashes.contains(&k.block_hash))
            .map(|(k, _)| k.clone())
            .collect();

        for key in keys_to_remove {
            self.cache.pop(&key);
        }
        
        self.stale_hashes.clear();
    }

    /// Clear entire cache.
    pub fn clear(&mut self) {
        self.cache.clear();
        self.stale_hashes.clear();
    }

    /// Get cache statistics.
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            entries: self.cache.len(),
            capacity: self.cache.cap().get(),
            stale_hashes: self.stale_hashes.len(),
            current_height: self.current_height,
        }
    }
}

impl Default for VersionedAccountCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Cache statistics for monitoring.
#[derive(Clone, Debug)]
pub struct CacheStats {
    pub entries: usize,
    pub capacity: usize,
    pub stale_hashes: usize,
    pub current_height: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_account() -> AccountState {
        AccountState::new(1000)
    }

    #[test]
    fn test_cache_put_get() {
        let mut cache = VersionedAccountCache::new();
        cache.set_head([0x01; 32], 100);
        
        let addr = [0xAA; 20];
        cache.put(addr, test_account());
        
        let retrieved = cache.get(&addr);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().balance, 1000);
    }

    #[test]
    fn test_cache_miss_on_different_head() {
        let mut cache = VersionedAccountCache::with_capacity(100);
        cache.set_head([0x01; 32], 100);
        
        let addr = [0xAA; 20];
        cache.put(addr, test_account());
        
        // Change head - entry should miss
        cache.set_head([0x02; 32], 101);
        assert!(cache.get(&addr).is_none());
    }

    #[test]
    fn test_cache_reorg_stale() {
        let mut cache = VersionedAccountCache::with_capacity(100);
        
        // Cache at head A
        cache.set_head([0x01; 32], 100);
        cache.put([0xAA; 20], test_account());
        
        // Reorg to head B
        cache.handle_reorg([0x01; 32], [0x02; 32]);
        
        // Old entry is stale
        assert_eq!(cache.stats().stale_hashes, 1);
        
        // Flush stale
        cache.flush_stale();
        assert_eq!(cache.stats().entries, 0);
    }

    #[test]
    fn test_cache_stats() {
        let mut cache = VersionedAccountCache::with_capacity(50);
        cache.set_head([0x01; 32], 100);
        
        for i in 0..10u8 {
            let mut addr = [0; 20];
            addr[0] = i;
            cache.put(addr, test_account());
        }
        
        let stats = cache.stats();
        assert_eq!(stats.entries, 10);
        assert_eq!(stats.capacity, 50);
        assert_eq!(stats.current_height, 100);
    }
}
