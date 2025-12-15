//! Feeler service state.

use std::collections::HashMap;

use super::config::FeelerConfig;
use super::types::FeelerProbe;
use crate::domain::{NodeId, SocketAddr, Timestamp};

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
