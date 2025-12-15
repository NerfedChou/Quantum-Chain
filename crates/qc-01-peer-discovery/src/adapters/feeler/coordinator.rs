use super::port::{FeelerError, FeelerPort};
use crate::domain::{
    feeler::{BucketFreshness, FeelerConfig, FeelerResult, FeelerState},
    handshake::ForkId,
    NodeId, SocketAddr,
};
use crate::ports::TimeSource;
use std::time::Duration;

// =============================================================================
// FEELER COORDINATOR (Application Service)
// =============================================================================

/// Coordinates feeler probing between domain state and network adapter.
///
/// This is the application-layer service that ties together:
/// - `FeelerState` (domain logic)
/// - `FeelerPort` (network adapter)
/// - `AddressManager` (peer address source)
pub struct FeelerCoordinator<T: TimeSource, P: FeelerPort> {
    /// Domain state
    state: FeelerState,
    /// Bucket freshness tracker
    freshness: BucketFreshness,
    /// Network adapter
    port: P,
    /// Time source
    time_source: T,
    /// Our ForkId for compatibility checks
    our_fork_id: ForkId,
}

impl<T: TimeSource, P: FeelerPort> FeelerCoordinator<T, P> {
    /// Create a new feeler coordinator.
    pub fn new(config: FeelerConfig, port: P, time_source: T, fork_id: ForkId) -> Self {
        let now = time_source.now();
        Self {
            state: FeelerState::new(config, now),
            freshness: BucketFreshness::new(),
            port,
            time_source,
            our_fork_id: fork_id,
        }
    }

    /// Check if it's time to probe and execute if needed.
    ///
    /// # Arguments
    ///
    /// * `bucket_counts` - Number of addresses in each New table bucket
    /// * `get_address` - Closure to get a random address from a bucket
    ///
    /// # Returns
    ///
    /// Result of probe if one was executed, None otherwise.
    pub fn maybe_probe<F>(
        &mut self,
        bucket_counts: &[usize],
        get_address: F,
    ) -> Option<(SocketAddr, FeelerResult)>
    where
        F: FnOnce(usize) -> Option<(SocketAddr, Option<NodeId>)>,
    {
        let now = self.time_source.now();

        if !self.state.should_probe(now) {
            return None;
        }

        // Select stalest bucket
        let bucket_idx = self.freshness.select_stalest_bucket(bucket_counts, now)?;

        // Get address from that bucket
        let (target_addr, node_id) = get_address(bucket_idx)?;

        // Start probe in domain
        let probe = self.state.start_probe(target_addr, node_id, now)?;

        // Record bucket probe
        self.freshness.record_probe(bucket_idx, now);

        // Execute probe via network adapter
        let timeout = Duration::from_secs(probe.deadline.as_secs() - now.as_secs());
        let result = self
            .port
            .probe(&target_addr, timeout, &self.our_fork_id)
            .unwrap_or(FeelerResult::ConnectionFailed);

        // Update domain state
        match &result {
            FeelerResult::Success => self.state.on_probe_success(&target_addr),
            _ => {
                let should_remove = self.state.on_probe_failure(&target_addr);
                if should_remove {
                    // Caller should remove from New table
                }
            }
        }

        Some((target_addr, result))
    }

    /// Check for timed-out probes and mark them as failed.
    pub fn gc_timed_out(&mut self) -> Vec<SocketAddr> {
        let now = self.time_source.now();
        let timed_out = self.state.get_timed_out_probes(now);

        for addr in &timed_out {
            self.state.cancel_probe(addr);
            self.state.on_probe_failure(addr);
        }

        timed_out
    }

    /// Get number of active probes.
    pub fn active_probe_count(&self) -> usize {
        self.state.active_probe_count()
    }
}
