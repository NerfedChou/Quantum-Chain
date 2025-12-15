use crate::domain::{NodeId, PeerInfo};
use crate::service::PeerDiscoveryService;

impl PeerDiscoveryService {
    /// Run garbage collection to clean expired entries.
    ///
    /// Call from a timer task at 60-second intervals to remove:
    /// - Expired pending verifications (INVARIANT-8)
    /// - Expired ban entries
    ///
    /// Reference: SPEC-01 Section 2.4 (INVARIANT-8: Verification Timeout)
    pub fn gc(&mut self) -> usize {
        let now = self.now();
        self.routing_table.gc_expired(now)
    }

    /// Check for expired eviction challenges and complete pending insertions.
    ///
    /// Call from a timer task at 1-second intervals. Challenges that exceed
    /// `eviction_challenge_timeout_secs` are treated as PONG timeout (peer dead).
    ///
    /// Reference: SPEC-01 Section 2.4 (INVARIANT-10: Eviction-on-Failure)
    pub fn check_expired_challenges(&mut self) -> Vec<(usize, PeerInfo, NodeId)> {
        let now = self.now();
        self.routing_table.check_expired_challenges(now)
    }
}
