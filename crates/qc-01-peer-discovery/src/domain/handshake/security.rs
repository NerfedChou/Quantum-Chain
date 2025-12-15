//! Handshake verification (security-critical).
//!
//! SECURITY-CRITICAL: Contains chain verification logic.
//! Isolate for security audits.

use super::config::HandshakeConfig;
use super::types::{HandshakeData, HandshakeResult, PeerClassification, RejectReason};

/// Verify a peer's handshake data against our own
///
/// # Security
/// This function is the gatekeeper for chain compatibility.
/// It prevents:
/// - Connecting to wrong network (genesis mismatch)
/// - Connecting to incompatible protocol versions
/// - Accepting peers too far behind (useless for sync)
///
/// # Algorithm: Fork-ID Convergence
/// 1. Network match (O(1))
/// 2. Protocol version check (O(1))
/// 3. Fork check - peer not behind finalized (O(1))
/// 4. Classification based on total difficulty
pub fn verify_handshake(
    ours: &HandshakeData,
    theirs: &HandshakeData,
    config: &HandshakeConfig,
) -> HandshakeResult {
    // Filter 1: Network Match (O(1))
    if ours.chain_info.genesis_hash != theirs.chain_info.genesis_hash {
        return HandshakeResult::Reject(RejectReason::WrongNetwork);
    }

    if ours.chain_info.network_id != theirs.chain_info.network_id {
        return HandshakeResult::Reject(RejectReason::NetworkIdMismatch);
    }

    // Filter 2: Protocol Version (O(1))
    if theirs.chain_info.protocol_version < config.min_protocol_version
        || theirs.chain_info.protocol_version > config.max_protocol_version
    {
        return HandshakeResult::Reject(RejectReason::ProtocolMismatch);
    }

    // Filter 3: Fork Check - Peer Not Too Far Behind (O(1))
    if theirs.head_state.height + config.max_behind_blocks < config.finalized_height {
        return HandshakeResult::Reject(RejectReason::TooFarBehind);
    }

    // Filter 4: Classification by Total Difficulty
    let classification = if theirs.head_state.total_difficulty > ours.head_state.total_difficulty {
        PeerClassification::SyncSource
    } else if theirs.head_state.total_difficulty < ours.head_state.total_difficulty {
        PeerClassification::SyncTarget
    } else {
        PeerClassification::Equal
    };

    HandshakeResult::Accept(classification)
}
