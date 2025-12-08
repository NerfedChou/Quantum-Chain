//! # Verify Node Identity Request
//!
//! Outbound request to Subsystem 10 (Signature Verification) for DDoS defense.
//!
//! ## Flow (IPC-MATRIX.md lines 42-51, 94-100)
//!
//! ```text
//! External Peer ──BootstrapRequest──→ [Peer Discovery (1)]
//!                                            │
//!                                            ↓ stage in pending_verification
//!                                            │
//!                                     VerifyNodeIdentityRequest ──→ [Signature Verification (10)]
//!                                            │
//!                                            ← NodeIdentityVerificationResult
//!                                            │
//!                                            ↓ if identity_valid: promote to routing table
//!                                              else: reject peer
//! ```
//!
//! ## Security
//!
//! - This request can ONLY be sent BY Subsystem 1 (Peer Discovery)
//! - This request can ONLY be received BY Subsystem 10 (Signature Verification)
//! - Per Architecture.md v2.2, payload contains NO identity fields (envelope authority)

/// Request to Subsystem 10 for node identity verification.
///
/// Sent when a new external peer attempts to join via `BootstrapRequest`.
/// Peer Discovery stages the peer in `pending_verification` and asks
/// Subsystem 10 to verify the signature before allowing table entry.
///
/// # Wire Format (IPC-MATRIX.md lines 42-51)
///
/// ```rust,ignore
/// struct VerifyNodeIdentityRequest {
///     version: u16,
///     requester_id: SubsystemId,  // Must be 1 (set in envelope)
///     correlation_id: [u8; 16],   // Set in envelope
///     reply_to: Topic,            // Set in envelope
///     node_id: [u8; 32],
///     claimed_pubkey: [u8; 33],
///     signature: Signature,
/// }
/// ```
///
/// # Security Note
///
/// The `requester_id`, `correlation_id`, and `reply_to` fields are part of the
/// `AuthenticatedMessage` envelope, NOT this payload. Per Architecture.md v2.2
/// "Envelope-Only Identity" principle.
///
/// # Serialization
///
/// Serialization is handled at the `AuthenticatedMessage` envelope level,
/// not by this struct directly. Arrays larger than 32 bytes require custom
/// serialization via the envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyNodeIdentityRequest {
    /// The 256-bit Kademlia node ID to verify.
    pub node_id: [u8; 32],
    /// The claimed compressed public key (33 bytes for secp256k1).
    pub claimed_pubkey: [u8; 33],
    /// Signature of the node_id by the private key corresponding to claimed_pubkey.
    /// Format: [r: 32 bytes][s: 32 bytes] = 64 bytes total.
    pub signature: [u8; 64],
}

impl VerifyNodeIdentityRequest {
    /// Create a new verification request.
    ///
    /// # Arguments
    ///
    /// * `node_id` - The 256-bit Kademlia node ID
    /// * `claimed_pubkey` - Compressed public key (33 bytes)
    /// * `signature` - 64-byte signature of node_id
    #[must_use]
    pub fn new(node_id: [u8; 32], claimed_pubkey: [u8; 33], signature: [u8; 64]) -> Self {
        Self {
            node_id,
            claimed_pubkey,
            signature,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_node_identity_request_new() {
        let node_id = [1u8; 32];
        let pubkey = [2u8; 33];
        let sig = [3u8; 64];

        let req = VerifyNodeIdentityRequest::new(node_id, pubkey, sig);

        assert_eq!(req.node_id, node_id);
        assert_eq!(req.claimed_pubkey, pubkey);
        assert_eq!(req.signature, sig);
    }

    #[test]
    fn test_verify_node_identity_request_clone() {
        let req = VerifyNodeIdentityRequest::new([1u8; 32], [2u8; 33], [3u8; 64]);
        let cloned = req.clone();
        assert_eq!(req, cloned);
    }
}
