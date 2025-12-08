//! # Bootstrap Request
//!
//! Inbound request from external nodes joining the network.
//!
//! ## Flow (IPC-MATRIX.md lines 66-72)
//!
//! When an external peer wants to join the network:
//! 1. They send a `BootstrapRequest` with their identity and proof-of-work
//! 2. Peer Discovery validates the PoW (anti-Sybil)
//! 3. Peer Discovery stages the peer in `pending_verification`
//! 4. Peer Discovery sends `VerifyNodeIdentityRequest` to Subsystem 10
//! 5. Upon receiving `NodeIdentityVerificationResult`:
//!    - If valid: promote to routing table, send PeerList response
//!    - If invalid: reject, add to temporary ban list
//!
//! ## Security
//!
//! - Proof-of-work prevents Sybil attacks (cheap identity creation)
//! - Signature verification (via Subsystem 10) prevents identity spoofing
//! - Peer is NEVER added to routing table until BOTH checks pass

use crate::domain::IpAddr;

/// Bootstrap request from an external node joining the network.
///
/// This is the entry point for new peers. The request includes proof-of-work
/// and a signature to enable identity verification before system entry.
///
/// # Wire Format (IPC-MATRIX.md lines 66-72)
///
/// ```rust,ignore
/// struct BootstrapRequest {
///     version: u16,
///     node_id: [u8; 32],
///     ip_address: IpAddr,
///     port: u16,
///     proof_of_work: [u8; 32],   // Anti-Sybil
/// }
/// ```
///
/// # Extended Format
///
/// We extend the base format to include signature fields needed for
/// identity verification via Subsystem 10.
///
/// # Serialization
///
/// Serialization is handled at the `AuthenticatedMessage` envelope level.
/// Arrays larger than 32 bytes require custom serialization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapRequest {
    /// The 256-bit Kademlia node ID.
    pub node_id: [u8; 32],
    /// IP address of the requesting node.
    pub ip_address: IpAddr,
    /// Port number for P2P communication.
    pub port: u16,
    /// Proof-of-work hash demonstrating computational effort.
    /// Must have sufficient leading zero bits per network difficulty.
    pub proof_of_work: [u8; 32],
    /// Compressed public key (33 bytes for secp256k1).
    pub claimed_pubkey: [u8; 33],
    /// Signature of node_id by the private key.
    /// Format: [r: 32 bytes][s: 32 bytes] = 64 bytes total.
    pub signature: [u8; 64],
}

impl BootstrapRequest {
    /// Create a new bootstrap request.
    #[must_use]
    pub fn new(
        node_id: [u8; 32],
        ip_address: IpAddr,
        port: u16,
        proof_of_work: [u8; 32],
        claimed_pubkey: [u8; 33],
        signature: [u8; 64],
    ) -> Self {
        Self {
            node_id,
            ip_address,
            port,
            proof_of_work,
            claimed_pubkey,
            signature,
        }
    }

    /// Extract the verification request to send to Subsystem 10.
    #[must_use]
    pub fn to_verification_request(&self) -> super::VerifyNodeIdentityRequest {
        super::VerifyNodeIdentityRequest::new(self.node_id, self.claimed_pubkey, self.signature)
    }
}

/// Result of processing a bootstrap request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BootstrapResult {
    /// Peer was staged for verification, awaiting Subsystem 10 response.
    PendingVerification {
        /// Correlation ID to match the verification response.
        correlation_id: [u8; 16],
    },
    /// Proof-of-work validation failed.
    InvalidProofOfWork,
    /// Staging area is full (Memory Bomb Defense).
    StagingFull,
    /// Peer is currently banned.
    Banned,
    /// Peer's IP is from an over-represented subnet.
    SubnetLimitReached,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_request() -> BootstrapRequest {
        BootstrapRequest::new(
            [1u8; 32],
            IpAddr::v4(192, 168, 1, 100),
            8080,
            [0u8; 32], // PoW with leading zeros
            [2u8; 33],
            [3u8; 64],
        )
    }

    #[test]
    fn test_bootstrap_request_new() {
        let req = make_request();
        assert_eq!(req.node_id, [1u8; 32]);
        assert_eq!(req.port, 8080);
    }

    #[test]
    fn test_to_verification_request() {
        let req = make_request();
        let verify_req = req.to_verification_request();

        assert_eq!(verify_req.node_id, req.node_id);
        assert_eq!(verify_req.claimed_pubkey, req.claimed_pubkey);
        assert_eq!(verify_req.signature, req.signature);
    }

    #[test]
    fn test_bootstrap_request_clone() {
        let req = make_request();
        let cloned = req.clone();
        assert_eq!(req, cloned);
    }
}
