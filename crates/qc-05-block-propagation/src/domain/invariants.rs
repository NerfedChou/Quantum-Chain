//! Security invariants for block propagation (SPEC-05 Section 2.3).

use super::{PeerPropagationState, PropagationConfig, PropagationState, SeenBlockCache};
use shared_types::Hash;

/// INVARIANT-1: Deduplication
/// The same block hash is never processed/validated twice.
pub fn invariant_no_duplicate_processing(cache: &SeenBlockCache, hash: &Hash) -> bool {
    if let Some(state) = cache.get_state(hash) {
        // If we've already fully processed, don't process again
        !matches!(
            state,
            PropagationState::Complete | PropagationState::Validated
        )
    } else {
        true
    }
}

/// INVARIANT-2: Rate Limiting
/// No peer can send more than max_announcements_per_second.
pub fn invariant_rate_limit(peer: &PeerPropagationState, config: &PropagationConfig) -> bool {
    peer.announcement_count < config.max_announcements_per_second
}

/// INVARIANT-3: Size Limit
/// No block larger than max_block_size is accepted.
pub fn invariant_size_limit(block_size: usize, config: &PropagationConfig) -> bool {
    block_size <= config.max_block_size_bytes
}

/// Security check result.
#[derive(Debug, PartialEq, Eq)]
pub enum InvariantViolation {
    DuplicateBlock,
    RateLimitExceeded,
    BlockTooLarge,
}

/// Check all invariants for incoming block.
pub fn check_all_invariants(
    cache: &SeenBlockCache,
    hash: &Hash,
    block_size: usize,
    peer: &PeerPropagationState,
    config: &PropagationConfig,
) -> Result<(), InvariantViolation> {
    if !invariant_no_duplicate_processing(cache, hash) {
        return Err(InvariantViolation::DuplicateBlock);
    }

    if !invariant_rate_limit(peer, config) {
        return Err(InvariantViolation::RateLimitExceeded);
    }

    if !invariant_size_limit(block_size, config) {
        return Err(InvariantViolation::BlockTooLarge);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::PeerId;

    #[test]
    fn test_invariant_deduplication() {
        let cache = SeenBlockCache::new(100);
        let hash = [1u8; 32];

        // New block should pass
        assert!(invariant_no_duplicate_processing(&cache, &hash));

        // Mark as seen
        cache.mark_seen(hash, None);
        assert!(invariant_no_duplicate_processing(&cache, &hash));

        // Mark as complete - should fail
        cache.update_state(&hash, PropagationState::Complete);
        assert!(!invariant_no_duplicate_processing(&cache, &hash));
    }

    #[test]
    fn test_invariant_rate_limit() {
        let config = PropagationConfig {
            max_announcements_per_second: 2,
            ..Default::default()
        };
        let peer_id = PeerId::new([1u8; 32]);
        let mut peer = PeerPropagationState::new(peer_id);

        assert!(invariant_rate_limit(&peer, &config));
        peer.record_announcement();
        assert!(invariant_rate_limit(&peer, &config));
        peer.record_announcement();
        assert!(!invariant_rate_limit(&peer, &config));
    }

    #[test]
    fn test_invariant_size_limit() {
        let config = PropagationConfig {
            max_block_size_bytes: 1000,
            ..Default::default()
        };

        assert!(invariant_size_limit(500, &config));
        assert!(invariant_size_limit(1000, &config));
        assert!(!invariant_size_limit(1001, &config));
    }

    #[test]
    fn test_check_all_invariants() {
        let cache = SeenBlockCache::new(100);
        let config = PropagationConfig {
            max_block_size_bytes: 1000,
            max_announcements_per_second: 2,
            ..Default::default()
        };
        let peer_id = PeerId::new([1u8; 32]);
        let peer = PeerPropagationState::new(peer_id);
        let hash = [1u8; 32];

        // All invariants pass
        assert!(check_all_invariants(&cache, &hash, 500, &peer, &config).is_ok());

        // Size violation
        assert_eq!(
            check_all_invariants(&cache, &hash, 2000, &peer, &config),
            Err(InvariantViolation::BlockTooLarge)
        );
    }
}
