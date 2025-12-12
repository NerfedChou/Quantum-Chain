//! # Feeler Connection Service
//!
//! Implements stochastic probing to promote New Table addresses to Tried Table.
//!
//! ## Algorithm: Poisson-Process Probing
//!
//! 1. Wake every `t` seconds (+ random jitter)
//! 2. Select address from "stale" New Table bucket
//! 3. Open short-lived connection, verify chain compatibility
//! 4. Promote on success, increment failure count on failure
//!
//! Reference: Bitcoin Core's `-feeler` connection logic

use std::collections::HashMap;

use crate::domain::{NodeId, SocketAddr, Timestamp};

// =============================================================================
// CONFIGURATION
// =============================================================================

/// Feeler service configuration
#[derive(Debug, Clone)]
pub struct FeelerConfig {
    /// Base interval between feeler probes (seconds)
    pub probe_interval_secs: u64,
    /// Maximum random jitter added to interval (seconds)
    pub jitter_max_secs: u64,
    /// Connection timeout for feeler probe (seconds)
    pub connection_timeout_secs: u64,
    /// Maximum failures before address is removed from New table
    pub max_failures: u32,
    /// Maximum active feeler connections at once
    pub max_concurrent_probes: usize,
}

impl Default for FeelerConfig {
    fn default() -> Self {
        Self {
            probe_interval_secs: 120,    // 2 minutes
            jitter_max_secs: 30,         // 0-30s jitter
            connection_timeout_secs: 10, // 10 second timeout
            max_failures: 3,
            max_concurrent_probes: 2,
        }
    }
}

impl FeelerConfig {
    /// Testing config with faster intervals
    #[cfg(test)]
    pub fn for_testing() -> Self {
        Self {
            probe_interval_secs: 5,
            jitter_max_secs: 2,
            connection_timeout_secs: 2,
            max_failures: 2,
            max_concurrent_probes: 1,
        }
    }
}

// =============================================================================
// FEELER PROBE STATE
// =============================================================================

/// State of a pending feeler probe
#[derive(Debug, Clone)]
pub struct FeelerProbe {
    /// Target address being probed
    pub target: SocketAddr,
    /// Target node ID (if known)
    pub node_id: Option<NodeId>,
    /// When the probe started
    pub started_at: Timestamp,
    /// Deadline for connection
    pub deadline: Timestamp,
}

impl FeelerProbe {
    /// Create a new feeler probe
    pub fn new(
        target: SocketAddr,
        node_id: Option<NodeId>,
        now: Timestamp,
        timeout_secs: u64,
    ) -> Self {
        Self {
            target,
            node_id,
            started_at: now,
            deadline: Timestamp::new(now.as_secs() + timeout_secs),
        }
    }

    /// Check if probe has timed out
    pub fn is_timed_out(&self, now: Timestamp) -> bool {
        now.as_secs() >= self.deadline.as_secs()
    }
}

/// Result of a feeler probe
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeelerResult {
    /// Probe successful - chain compatible, promote to Tried
    Success,
    /// Probe failed - connection error or timeout
    ConnectionFailed,
    /// Probe failed - wrong chain (different genesis)
    WrongChain,
    /// Probe failed - peer too far behind
    TooFarBehind,
    /// No address available to probe
    NoAddressAvailable,
}

// =============================================================================
// FEELER SERVICE STATE (Pure Domain Logic)
// =============================================================================

/// Feeler service domain state
///
/// This is the pure domain logic. Actual network I/O is handled by adapters.
#[derive(Debug)]
pub struct FeelerState {
    /// Active feeler probes
    active_probes: HashMap<SocketAddr, FeelerProbe>,
    /// Failure counts per address
    failure_counts: HashMap<SocketAddr, u32>,
    /// When next probe should occur
    next_probe_at: Timestamp,
    /// Configuration
    config: FeelerConfig,
    /// Simple counter for deterministic "jitter" in tests
    probe_counter: u64,
}

impl FeelerState {
    /// Create new feeler state
    pub fn new(config: FeelerConfig, now: Timestamp) -> Self {
        Self {
            active_probes: HashMap::new(),
            failure_counts: HashMap::new(),
            next_probe_at: Timestamp::new(now.as_secs() + config.probe_interval_secs),
            config,
            probe_counter: 0,
        }
    }

    /// Check if it's time to start a new probe
    pub fn should_probe(&self, now: Timestamp) -> bool {
        now.as_secs() >= self.next_probe_at.as_secs()
            && self.active_probes.len() < self.config.max_concurrent_probes
    }

    /// Start a new feeler probe
    ///
    /// Returns the probe if started, None if at capacity
    pub fn start_probe(
        &mut self,
        target: SocketAddr,
        node_id: Option<NodeId>,
        now: Timestamp,
    ) -> Option<FeelerProbe> {
        if self.active_probes.len() >= self.config.max_concurrent_probes {
            return None;
        }

        if self.active_probes.contains_key(&target) {
            return None;
        }

        let probe = FeelerProbe::new(target, node_id, now, self.config.connection_timeout_secs);
        self.active_probes.insert(target, probe.clone());

        // Schedule next probe with jitter
        self.probe_counter += 1;
        let jitter = self.probe_counter % (self.config.jitter_max_secs + 1);
        self.next_probe_at =
            Timestamp::new(now.as_secs() + self.config.probe_interval_secs + jitter);

        Some(probe)
    }

    /// Complete a probe with success
    pub fn on_probe_success(&mut self, target: &SocketAddr) {
        self.active_probes.remove(target);
        self.failure_counts.remove(target); // Reset failures on success
    }

    /// Complete a probe with failure
    ///
    /// Returns true if address should be removed from New table (max failures reached)
    pub fn on_probe_failure(&mut self, target: &SocketAddr) -> bool {
        self.active_probes.remove(target);

        let count = self.failure_counts.entry(*target).or_insert(0);
        *count += 1;

        *count >= self.config.max_failures
    }

    /// Get timed-out probes
    pub fn get_timed_out_probes(&self, now: Timestamp) -> Vec<SocketAddr> {
        self.active_probes
            .iter()
            .filter(|(_, p)| p.is_timed_out(now))
            .map(|(addr, _)| *addr)
            .collect()
    }

    /// Cancel a probe (e.g., on timeout)
    pub fn cancel_probe(&mut self, target: &SocketAddr) {
        self.active_probes.remove(target);
    }

    /// Get current failure count for an address
    pub fn failure_count(&self, target: &SocketAddr) -> u32 {
        self.failure_counts.get(target).copied().unwrap_or(0)
    }

    /// Get number of active probes
    pub fn active_probe_count(&self) -> usize {
        self.active_probes.len()
    }
}

// =============================================================================
// BUCKET SELECTION ALGORITHM
// =============================================================================

/// Bucket freshness tracking for stochastic selection
#[derive(Debug, Clone, Default)]
pub struct BucketFreshness {
    /// Last probe time per bucket
    last_probe: HashMap<usize, Timestamp>,
}

impl BucketFreshness {
    /// Create new bucket freshness tracker
    pub fn new() -> Self {
        Self::default()
    }

    /// Record that a bucket was probed
    pub fn record_probe(&mut self, bucket_idx: usize, now: Timestamp) {
        self.last_probe.insert(bucket_idx, now);
    }

    /// Select the stalest bucket (longest since last probe)
    ///
    /// If bucket_counts is provided, only considers buckets with entries
    pub fn select_stalest_bucket(&self, bucket_counts: &[usize], now: Timestamp) -> Option<usize> {
        let mut stalest_idx = None;
        let mut stalest_time = now.as_secs();

        for (idx, &count) in bucket_counts.iter().enumerate() {
            if count == 0 {
                continue;
            }

            let last = self.last_probe.get(&idx).map(|t| t.as_secs()).unwrap_or(0);
            if last < stalest_time {
                stalest_time = last;
                stalest_idx = Some(idx);
            }
        }

        stalest_idx
    }
}

// =============================================================================
// TESTS (TDD)
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::IpAddr;

    fn make_socket(port: u16) -> SocketAddr {
        SocketAddr::new(IpAddr::v4(192, 168, 1, 1), port)
    }

    // =========================================================================
    // TEST GROUP 1: Probe Scheduling
    // =========================================================================

    #[test]
    fn test_should_probe_after_interval() {
        let config = FeelerConfig::for_testing();
        let now = Timestamp::new(1000);
        let state = FeelerState::new(config.clone(), now);

        // Should not probe immediately
        assert!(!state.should_probe(now));

        // Should probe after interval
        let later = Timestamp::new(now.as_secs() + config.probe_interval_secs);
        assert!(state.should_probe(later));
    }

    #[test]
    fn test_probe_respects_max_concurrent() {
        let config = FeelerConfig::for_testing();
        let now = Timestamp::new(1000);
        let mut state = FeelerState::new(config.clone(), now);

        // Start first probe
        let target1 = make_socket(8080);
        assert!(state.start_probe(target1, None, now).is_some());

        // Can't start another (max_concurrent_probes = 1)
        let target2 = make_socket(8081);
        assert!(state.start_probe(target2, None, now).is_none());
    }

    // =========================================================================
    // TEST GROUP 2: Probe Completion
    // =========================================================================

    #[test]
    fn test_probe_success_resets_failures() {
        let config = FeelerConfig::for_testing();
        let now = Timestamp::new(1000);
        let mut state = FeelerState::new(config, now);

        let target = make_socket(8080);

        // Simulate some failures
        state.start_probe(target, None, now);
        state.on_probe_failure(&target);
        assert_eq!(state.failure_count(&target), 1);

        // Success resets
        state.start_probe(target, None, now);
        state.on_probe_success(&target);
        assert_eq!(state.failure_count(&target), 0);
    }

    #[test]
    fn test_max_failures_triggers_removal() {
        let config = FeelerConfig::for_testing();
        let now = Timestamp::new(1000);
        let mut state = FeelerState::new(config.clone(), now);

        let target = make_socket(8080);

        // First failure
        state.start_probe(target, None, now);
        assert!(!state.on_probe_failure(&target));

        // Second failure (max_failures = 2) - should trigger removal
        state.start_probe(target, None, now);
        assert!(state.on_probe_failure(&target));
    }

    // =========================================================================
    // TEST GROUP 3: Timeout Handling
    // =========================================================================

    #[test]
    fn test_probe_timeout_detection() {
        let config = FeelerConfig::for_testing();
        let now = Timestamp::new(1000);
        let mut state = FeelerState::new(config.clone(), now);

        let target = make_socket(8080);
        state.start_probe(target, None, now);

        // Not timed out yet
        assert!(state.get_timed_out_probes(now).is_empty());

        // After timeout
        let later = Timestamp::new(now.as_secs() + config.connection_timeout_secs + 1);
        let timed_out = state.get_timed_out_probes(later);
        assert_eq!(timed_out.len(), 1);
        assert_eq!(timed_out[0], target);
    }

    // =========================================================================
    // TEST GROUP 4: Bucket Selection
    // =========================================================================

    #[test]
    fn test_stalest_bucket_selection() {
        let now = Timestamp::new(1000);
        let mut freshness = BucketFreshness::new();

        // Bucket 0 probed at 500, bucket 1 probed at 800, bucket 2 never probed
        freshness.record_probe(0, Timestamp::new(500));
        freshness.record_probe(1, Timestamp::new(800));

        // Bucket counts: all have entries
        let counts = vec![5, 3, 7];

        // Should select bucket 2 (never probed = stalest)
        let selected = freshness.select_stalest_bucket(&counts, now);
        assert_eq!(selected, Some(2));
    }

    #[test]
    fn test_stalest_bucket_skips_empty() {
        let now = Timestamp::new(1000);
        let freshness = BucketFreshness::new();

        // Bucket 0 and 2 have entries, bucket 1 is empty
        let counts = vec![5, 0, 7];

        let selected = freshness.select_stalest_bucket(&counts, now);
        // Should select 0 or 2, not 1
        assert!(selected == Some(0) || selected == Some(2));
    }
}
