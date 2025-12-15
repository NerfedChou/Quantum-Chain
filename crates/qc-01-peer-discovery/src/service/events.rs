use crate::domain::{NodeId, PeerDiscoveryError};
use crate::ports::VerificationHandler;
use crate::service::PeerDiscoveryService;

impl PeerDiscoveryService {
    /// Handle verification result from Subsystem 10.
    ///
    /// This is called after Subsystem 10 verifies a peer's signature.
    ///
    /// # Arguments
    ///
    /// * `node_id` - The peer that was verified
    /// * `identity_valid` - true if signature verified, false if failed
    ///
    /// # Returns
    ///
    /// If a peer needs to be challenged (bucket full), returns the challenged peer's NodeId.
    /// The caller should send a PING to this peer via `NetworkSocket`.
    pub fn on_verification_result(
        &mut self,
        node_id: &NodeId,
        identity_valid: bool,
    ) -> Result<Option<NodeId>, PeerDiscoveryError> {
        let now = self.now();
        self.routing_table
            .on_verification_result(node_id, identity_valid, now)
    }

    /// Handle challenge response (PING/PONG result).
    ///
    /// # Arguments
    ///
    /// * `challenged_peer` - The peer that was challenged
    /// * `is_alive` - true if peer responded, false if timed out
    pub fn on_challenge_response(
        &mut self,
        challenged_peer: &NodeId,
        is_alive: bool,
    ) -> Result<(), PeerDiscoveryError> {
        let now = self.now();
        self.routing_table
            .on_challenge_response(challenged_peer, is_alive, now)
    }
}

// EDA Integration: Implement VerificationHandler for event-driven processing
impl VerificationHandler for PeerDiscoveryService {
    fn handle_verification(
        &mut self,
        node_id: &NodeId,
        identity_valid: bool,
    ) -> Result<Option<NodeId>, PeerDiscoveryError> {
        self.on_verification_result(node_id, identity_valid)
    }
}
