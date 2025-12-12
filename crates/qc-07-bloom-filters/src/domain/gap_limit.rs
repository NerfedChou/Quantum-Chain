//! # Gap Limit Enforcer (Anti-Dusting Protection)
//!
//! Prevents CPU exhaustion from dust transactions matching filters.
//!
//! ## Threat
//!
//! Attacker sends tiny "dust" transactions to thousands of addresses
//! matching client filters, forcing gigabyte downloads and battery drain.
//!
//! ## Defense: Match-Rate Throttling
//!
//! 1. Track `MatchesPerBlock` for each client
//! 2. Calculate expected match rate from filter parameters
//! 3. Disconnect if `ActualMatches > E * 10` for 5 consecutive blocks
//!
//! Reference: SPEC-07 Appendix B.2 - Security Boundaries

use std::collections::HashMap;

/// Expected match rate multiplier for dusting detection.
const MATCH_RATE_THRESHOLD: f64 = 10.0;

/// Number of consecutive blocks before triggering protection.
const CONSECUTIVE_BLOCKS_THRESHOLD: usize = 5;

/// Maximum allowed match rate per block.
const MAX_MATCHES_PER_BLOCK: usize = 1000;

/// Match rate tracking for a single client.
#[derive(Clone, Debug)]
pub struct ClientMatchHistory {
    /// Expected match rate based on filter FPR
    expected_rate: f64,
    /// Recent match counts per block
    recent_matches: Vec<usize>,
    /// Consecutive blocks exceeding threshold
    consecutive_violations: usize,
    /// Total matches served
    total_matches: u64,
    /// Whether client is throttled
    pub is_throttled: bool,
}

impl ClientMatchHistory {
    /// Create new history with expected rate.
    pub fn new(expected_rate: f64) -> Self {
        Self {
            expected_rate,
            recent_matches: Vec::with_capacity(CONSECUTIVE_BLOCKS_THRESHOLD),
            consecutive_violations: 0,
            total_matches: 0,
            is_throttled: false,
        }
    }

    /// Record matches for a block.
    ///
    /// Returns true if client should be throttled.
    pub fn record_block(&mut self, matches: usize) -> bool {
        self.recent_matches.push(matches);
        self.total_matches += matches as u64;

        // Keep only recent blocks
        if self.recent_matches.len() > CONSECUTIVE_BLOCKS_THRESHOLD {
            self.recent_matches.remove(0);
        }

        // Check if exceeds threshold
        let threshold = (self.expected_rate * MATCH_RATE_THRESHOLD).max(1.0) as usize;
        let exceeds = matches > threshold || matches > MAX_MATCHES_PER_BLOCK;

        if exceeds {
            self.consecutive_violations += 1;
        } else {
            self.consecutive_violations = 0;
        }

        // Trigger throttle if exceeded for consecutive blocks
        if self.consecutive_violations >= CONSECUTIVE_BLOCKS_THRESHOLD {
            self.is_throttled = true;
        }

        self.is_throttled
    }

    /// Reset throttle state (e.g., after client reduces filter size).
    pub fn reset(&mut self) {
        self.consecutive_violations = 0;
        self.is_throttled = false;
        self.recent_matches.clear();
    }

    /// Get statistics.
    pub fn stats(&self) -> MatchHistoryStats {
        let recent_avg = if self.recent_matches.is_empty() {
            0.0
        } else {
            self.recent_matches.iter().sum::<usize>() as f64 / self.recent_matches.len() as f64
        };

        MatchHistoryStats {
            expected_rate: self.expected_rate,
            recent_avg,
            consecutive_violations: self.consecutive_violations,
            total_matches: self.total_matches,
            is_throttled: self.is_throttled,
        }
    }
}

/// Statistics for monitoring.
#[derive(Clone, Debug)]
pub struct MatchHistoryStats {
    pub expected_rate: f64,
    pub recent_avg: f64,
    pub consecutive_violations: usize,
    pub total_matches: u64,
    pub is_throttled: bool,
}

/// Gap limit enforcer for multiple clients.
#[derive(Debug, Default)]
pub struct GapLimitEnforcer {
    /// Per-client match history
    clients: HashMap<String, ClientMatchHistory>,
}

impl GapLimitEnforcer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a client with their filter's expected rate.
    ///
    /// Expected rate is calculated from filter FPR and average block size.
    pub fn register_client(&mut self, client_id: String, filter_fpr: f64, avg_tx_per_block: usize) {
        let expected_rate = avg_tx_per_block as f64 * filter_fpr;
        self.clients
            .insert(client_id, ClientMatchHistory::new(expected_rate));
    }

    /// Record matches for a client's block.
    ///
    /// Returns `Err(ThrottleReason)` if client should be disconnected.
    pub fn record_matches(
        &mut self,
        client_id: &str,
        matches: usize,
    ) -> Result<(), ThrottleReason> {
        let history = self
            .clients
            .get_mut(client_id)
            .ok_or(ThrottleReason::ClientNotFound)?;

        if history.record_block(matches) {
            Err(ThrottleReason::MatchRateExceeded {
                expected: history.expected_rate,
                actual_avg: history.stats().recent_avg,
            })
        } else {
            Ok(())
        }
    }

    /// Unregister a client.
    pub fn unregister_client(&mut self, client_id: &str) {
        self.clients.remove(client_id);
    }

    /// Get client statistics.
    pub fn get_stats(&self, client_id: &str) -> Option<MatchHistoryStats> {
        self.clients.get(client_id).map(|h| h.stats())
    }

    /// Get all throttled clients.
    pub fn get_throttled_clients(&self) -> Vec<String> {
        self.clients
            .iter()
            .filter(|(_, h)| h.is_throttled)
            .map(|(id, _)| id.clone())
            .collect()
    }
}

/// Reason for throttling a client.
#[derive(Clone, Debug)]
pub enum ThrottleReason {
    ClientNotFound,
    MatchRateExceeded { expected: f64, actual_avg: f64 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normal_match_rate() {
        let mut history = ClientMatchHistory::new(5.0); // Expect 5 matches/block

        // Normal matching - should not throttle
        for _ in 0..10 {
            assert!(!history.record_block(3));
        }

        assert!(!history.is_throttled);
    }

    #[test]
    fn test_excessive_match_rate_triggers_throttle() {
        let mut history = ClientMatchHistory::new(5.0); // Expect 5 matches/block

        // Excessive matching (>50 = 5 * 10)
        for _ in 0..CONSECUTIVE_BLOCKS_THRESHOLD {
            history.record_block(100);
        }

        assert!(history.is_throttled);
    }

    #[test]
    fn test_intermittent_spikes_dont_throttle() {
        let mut history = ClientMatchHistory::new(5.0);

        // Spike, then normal - should reset counter
        history.record_block(100);
        history.record_block(100);
        history.record_block(3); // Normal - resets counter
        history.record_block(100);
        history.record_block(100);

        assert!(!history.is_throttled);
    }

    #[test]
    fn test_enforcer_multiple_clients() {
        let mut enforcer = GapLimitEnforcer::new();

        enforcer.register_client("alice".to_string(), 0.01, 100);
        enforcer.register_client("bob".to_string(), 0.01, 100);

        // Alice gets dusted
        for _ in 0..10 {
            let _ = enforcer.record_matches("alice", 500);
        }

        // Bob is normal
        for _ in 0..10 {
            enforcer.record_matches("bob", 1).unwrap();
        }

        let throttled = enforcer.get_throttled_clients();
        assert!(throttled.contains(&"alice".to_string()));
        assert!(!throttled.contains(&"bob".to_string()));
    }

    #[test]
    fn test_reset_clears_throttle() {
        let mut history = ClientMatchHistory::new(5.0);

        // Trigger throttle
        for _ in 0..CONSECUTIVE_BLOCKS_THRESHOLD {
            history.record_block(100);
        }
        assert!(history.is_throttled);

        // Reset
        history.reset();
        assert!(!history.is_throttled);
    }
}
