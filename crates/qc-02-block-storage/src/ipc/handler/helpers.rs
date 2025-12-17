//! # Handler Helpers
//!
//! Shared utilities to reduce code duplication in handler methods.

use crate::domain::storage::StoredBlock;
use crate::ipc::payloads::BlockStoredPayload;
use shared_types::Hash;

/// Get current Unix timestamp in seconds.
#[inline]
pub fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Build BlockStoredPayload from StoredBlock and block identifiers.
#[inline]
pub fn build_block_stored_payload(
    stored: &StoredBlock,
    block_hash: Hash,
    block_height: u64,
) -> BlockStoredPayload {
    BlockStoredPayload {
        block_height,
        block_hash,
        merkle_root: stored.merkle_root,
        state_root: stored.state_root,
        stored_at: stored.stored_at,
    }
}
