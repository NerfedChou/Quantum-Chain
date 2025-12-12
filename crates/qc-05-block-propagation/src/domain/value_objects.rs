//! Value objects for block propagation configuration and state.

use parking_lot::RwLock;
use shared_types::Hash;
use std::collections::{HashMap, VecDeque};
use std::time::Instant;

use super::PeerId;

/// Propagation state for a block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropagationState {
    /// Header/announcement received
    Announced,
    /// Compact block received
    CompactReceived,
    /// Waiting for missing transactions
    Reconstructing,
    /// Full block available
    Complete,
    /// Consensus validated
    Validated,
    /// Failed validation
    Invalid,
}

/// Block propagation configuration.
#[derive(Clone, Debug)]
pub struct PropagationConfig {
    /// Number of peers to gossip to (fan-out)
    pub fanout: usize,
    /// Maximum announcements per peer per second
    pub max_announcements_per_second: u32,
    /// Maximum block size in bytes
    pub max_block_size_bytes: usize,
    /// Seen block cache size
    pub seen_cache_size: usize,
    /// Compact block reconstruction timeout in ms
    pub reconstruction_timeout_ms: u64,
    /// Full block request timeout in ms
    pub request_timeout_ms: u64,
    /// Enable compact block relay
    pub enable_compact_blocks: bool,
}

impl Default for PropagationConfig {
    fn default() -> Self {
        Self {
            fanout: 8,
            max_announcements_per_second: 1,
            max_block_size_bytes: 10 * 1024 * 1024, // 10 MB
            seen_cache_size: 10_000,
            reconstruction_timeout_ms: 5_000,
            request_timeout_ms: 10_000,
            enable_compact_blocks: true,
        }
    }
}

/// Per-peer propagation state.
///
/// ## Security: Reputation Decay
///
/// - Reputation decays 5% per minute to prevent accumulation  
/// - Reset to 0 after 3 rate limit violations
/// - Used for gossip peer selection (higher = priority)
#[derive(Clone, Debug)]
pub struct PeerPropagationState {
    pub peer_id: PeerId,
    /// Last announcement timestamp
    pub last_announcement: Instant,
    /// Announcement count in current window
    pub announcement_count: u32,
    /// Window start time
    pub window_start: Instant,
    /// Peer latency estimate in ms
    pub latency_ms: u64,
    /// Reputation score (0.0 to 1.0)
    pub reputation: f64,
    /// Rate limit violations count
    pub rate_violations: u32,
    /// Total blocks received
    pub blocks_received: u64,
    /// Total invalid blocks (PoW failures)
    pub invalid_blocks: u64,
}

/// Reputation decay rate per minute (5%).
pub const REPUTATION_DECAY_RATE: f64 = 0.95;

/// Maximum rate violations before reputation reset.
pub const MAX_RATE_VIOLATIONS: u32 = 3;

impl PeerPropagationState {
    pub fn new(peer_id: PeerId) -> Self {
        Self {
            peer_id,
            last_announcement: Instant::now(),
            announcement_count: 0,
            window_start: Instant::now(),
            latency_ms: 100,
            reputation: 0.5,
            rate_violations: 0,
            blocks_received: 0,
            invalid_blocks: 0,
        }
    }

    /// Record an announcement and update count.
    pub fn record_announcement(&mut self) {
        let now = Instant::now();
        // Reset window if more than 1 second has passed
        if now.duration_since(self.window_start).as_secs() >= 1 {
            self.window_start = now;
            self.announcement_count = 0;
        }
        self.announcement_count += 1;
        self.last_announcement = now;
    }

    /// Reset rate limit window.
    pub fn reset_rate_limit(&mut self) {
        self.window_start = Instant::now();
        self.announcement_count = 0;
    }

    /// Update reputation based on behavior.
    pub fn update_reputation(&mut self, delta: f64) {
        self.reputation = (self.reputation + delta).clamp(0.0, 1.0);
    }

    /// Apply reputation decay (5% per minute).
    pub fn apply_decay(&mut self, elapsed_minutes: u32) {
        for _ in 0..elapsed_minutes.min(60) {
            self.reputation *= REPUTATION_DECAY_RATE;
        }
    }

    /// Record a rate limit violation. Returns true if peer should be deprioritized.
    pub fn record_rate_violation(&mut self) -> bool {
        self.rate_violations += 1;
        if self.rate_violations >= MAX_RATE_VIOLATIONS {
            self.reputation = 0.0;
            true
        } else {
            false
        }
    }

    /// Record a valid block received.
    pub fn record_valid_block(&mut self) {
        self.blocks_received += 1;
        self.reputation = (self.reputation + 0.01).min(1.0);
    }

    /// Record an invalid block (PoW failure).
    pub fn record_invalid_block(&mut self) {
        self.invalid_blocks += 1;
        self.reputation = (self.reputation - 0.1).max(0.0);
    }

    /// Check if peer is eligible for gossip.
    pub fn is_eligible(&self) -> bool {
        self.reputation > 0.0 && self.rate_violations < MAX_RATE_VIOLATIONS
    }
}

/// Information about a seen block.
#[derive(Clone, Debug)]
pub struct SeenBlockInfo {
    pub first_seen: Instant,
    pub first_peer: Option<PeerId>,
    pub propagation_state: PropagationState,
}

/// LRU cache for seen blocks (deduplication).
/// LRU cache for seen blocks (deduplication).
///
/// Uses VecDeque for O(1) eviction of oldest entries.
pub struct SeenBlockCache {
    cache: RwLock<HashMap<Hash, SeenBlockInfo>>,
    max_size: usize,
    /// Insertion order tracking using VecDeque for O(1) pop_front.
    insertion_order: RwLock<VecDeque<Hash>>,
}

impl SeenBlockCache {
    pub fn new(max_size: usize) -> Self {
        Self {
            cache: RwLock::new(HashMap::with_capacity(max_size)),
            max_size,
            insertion_order: RwLock::new(VecDeque::with_capacity(max_size)),
        }
    }

    /// Check if block has been seen.
    pub fn has_seen(&self, hash: &Hash) -> bool {
        self.cache.read().contains_key(hash)
    }

    /// Mark block as seen.
    pub fn mark_seen(&self, hash: Hash, peer: Option<PeerId>) {
        let mut cache = self.cache.write();
        let mut order = self.insertion_order.write();

        // Evict oldest if at capacity - O(1) with VecDeque
        if cache.len() >= self.max_size && !cache.contains_key(&hash) {
            if let Some(oldest) = order.pop_front() {
                cache.remove(&oldest);
            }
        }

        if let std::collections::hash_map::Entry::Vacant(e) = cache.entry(hash) {
            e.insert(SeenBlockInfo {
                first_seen: Instant::now(),
                first_peer: peer,
                propagation_state: PropagationState::Announced,
            });
            order.push_back(hash);
        }
    }

    /// Update propagation state for a block.
    pub fn update_state(&self, hash: &Hash, state: PropagationState) {
        if let Some(info) = self.cache.write().get_mut(hash) {
            info.propagation_state = state;
        }
    }

    /// Get propagation state for a block.
    pub fn get_state(&self, hash: &Hash) -> Option<PropagationState> {
        self.cache
            .read()
            .get(hash)
            .map(|info| info.propagation_state)
    }

    /// Check if block can be processed (not already complete/validated).
    pub fn can_process(&self, hash: &Hash) -> bool {
        match self.cache.read().get(hash) {
            None => true,
            Some(info) => !matches!(
                info.propagation_state,
                PropagationState::Complete
                    | PropagationState::Validated
                    | PropagationState::Invalid
            ),
        }
    }

    /// Get cache size.
    pub fn len(&self) -> usize {
        self.cache.read().len()
    }

    /// Check if cache is empty.
    pub fn is_empty(&self) -> bool {
        self.cache.read().is_empty()
    }
}

/// Propagation statistics.
#[derive(Clone, Debug)]
pub struct PropagationStats {
    pub block_hash: Hash,
    pub peers_reached: usize,
    pub propagation_start_ms: u64,
    pub first_ack_time_ms: Option<u64>,
}

/// Network propagation metrics.
#[derive(Clone, Debug, Default)]
pub struct PropagationMetrics {
    pub average_propagation_time_ms: f64,
    pub blocks_propagated_last_hour: u64,
    pub compact_block_success_rate: f64,
    pub average_missing_txs: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_propagation_config_defaults() {
        let config = PropagationConfig::default();
        assert_eq!(config.fanout, 8);
        assert_eq!(config.max_block_size_bytes, 10 * 1024 * 1024);
    }

    #[test]
    fn test_peer_state_rate_limiting() {
        let peer_id = PeerId::new([1u8; 32]);
        let mut state = PeerPropagationState::new(peer_id);

        assert_eq!(state.announcement_count, 0);
        state.record_announcement();
        assert_eq!(state.announcement_count, 1);
        state.record_announcement();
        assert_eq!(state.announcement_count, 2);

        state.reset_rate_limit();
        assert_eq!(state.announcement_count, 0);
    }

    #[test]
    fn test_seen_block_cache() {
        let cache = SeenBlockCache::new(100);
        let hash = [0xABu8; 32];

        assert!(!cache.has_seen(&hash));
        cache.mark_seen(hash, None);
        assert!(cache.has_seen(&hash));
        assert!(cache.can_process(&hash));

        cache.update_state(&hash, PropagationState::Complete);
        assert!(!cache.can_process(&hash));
    }

    #[test]
    fn test_seen_cache_eviction() {
        let cache = SeenBlockCache::new(3);

        let hash1 = [1u8; 32];
        let hash2 = [2u8; 32];
        let hash3 = [3u8; 32];
        let hash4 = [4u8; 32];

        cache.mark_seen(hash1, None);
        cache.mark_seen(hash2, None);
        cache.mark_seen(hash3, None);
        assert_eq!(cache.len(), 3);

        // Adding 4th should evict first
        cache.mark_seen(hash4, None);
        assert_eq!(cache.len(), 3);
        assert!(!cache.has_seen(&hash1));
        assert!(cache.has_seen(&hash4));
    }

    #[test]
    fn test_reputation_bounds() {
        let peer_id = PeerId::new([1u8; 32]);
        let mut state = PeerPropagationState::new(peer_id);

        state.update_reputation(1.0);
        assert_eq!(state.reputation, 1.0);

        state.update_reputation(0.5);
        assert_eq!(state.reputation, 1.0); // Clamped

        state.update_reputation(-2.0);
        assert_eq!(state.reputation, 0.0); // Clamped
    }

    #[test]
    fn test_reputation_decay() {
        let peer_id = PeerId::new([1u8; 32]);
        let mut state = PeerPropagationState::new(peer_id);

        let initial = state.reputation;
        state.apply_decay(1); // 1 minute

        // Should decay by 5%
        assert!((state.reputation - initial * REPUTATION_DECAY_RATE).abs() < 0.001);
    }

    #[test]
    fn test_rate_violation_threshold() {
        let peer_id = PeerId::new([1u8; 32]);
        let mut state = PeerPropagationState::new(peer_id);

        assert!(!state.record_rate_violation());
        assert!(!state.record_rate_violation());
        assert!(state.record_rate_violation()); // 3rd violation resets

        assert_eq!(state.reputation, 0.0);
        assert!(!state.is_eligible());
    }

    #[test]
    fn test_valid_block_reputation_increase() {
        let peer_id = PeerId::new([1u8; 32]);
        let mut state = PeerPropagationState::new(peer_id);

        let initial = state.reputation;
        state.record_valid_block();

        assert!(state.reputation > initial);
        assert_eq!(state.blocks_received, 1);
    }

    #[test]
    fn test_invalid_block_reputation_penalty() {
        let peer_id = PeerId::new([1u8; 32]);
        let mut state = PeerPropagationState::new(peer_id);

        let initial = state.reputation;
        state.record_invalid_block();

        assert!(state.reputation < initial);
        assert_eq!(state.invalid_blocks, 1);
    }
}
