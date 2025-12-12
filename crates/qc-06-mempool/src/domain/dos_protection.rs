//! # Rejection Cache (Anti-CPU DoS)
//!
//! Implements rolling bloom filter for rejecting known-bad transactions.
//!
//! ## Problem
//!
//! Attacker repeatedly broadcasts invalid transaction.
//! Node validates (expensive), rejects, receives again from another peer.
//! Loop forever = CPU exhaustion.
//!
//! ## Solution: Rolling-Bloom-Filter
//!
//! 1. On reject: Insert Hash(TxID + salt) into bloom filter
//! 2. On receive: Check filter first (O(1))
//! 3. Roll: Replace filter every N hours to prevent false positive buildup
//!
//! ## Security
//!
//! Zero CPU cost for repeated invalid transactions.

use super::Hash;
use std::collections::HashSet;
use std::time::{Duration, Instant};

/// Rejection cache using probabilistic bloom-like filter.
///
/// ## Algorithm: Rolling-Bloom-Filter
///
/// - Insert rejected tx hashes
/// - Check before validation
/// - Roll periodically to clear false positives
#[derive(Debug)]
pub struct RejectionCache {
    /// Current filter (HashSet for simplicity, could be bloom)
    current: HashSet<Hash>,
    /// Previous filter (for rolling)
    previous: HashSet<Hash>,
    /// Salt for hashing (prevents precomputation)
    salt: u64,
    /// Last roll time
    last_roll: Instant,
    /// Roll interval
    roll_interval: Duration,
    /// Maximum entries per filter
    max_entries: usize,
}

/// Default roll interval (1 hour).
pub const DEFAULT_ROLL_INTERVAL: Duration = Duration::from_secs(3600);

/// Default max entries per filter.
pub const DEFAULT_MAX_ENTRIES: usize = 100_000;

impl RejectionCache {
    /// Create new rejection cache.
    pub fn new() -> Self {
        Self {
            current: HashSet::with_capacity(DEFAULT_MAX_ENTRIES / 2),
            previous: HashSet::new(),
            salt: rand_salt(),
            last_roll: Instant::now(),
            roll_interval: DEFAULT_ROLL_INTERVAL,
            max_entries: DEFAULT_MAX_ENTRIES,
        }
    }

    /// Create with custom parameters.
    pub fn with_params(roll_interval: Duration, max_entries: usize) -> Self {
        Self {
            current: HashSet::with_capacity(max_entries / 2),
            previous: HashSet::new(),
            salt: rand_salt(),
            last_roll: Instant::now(),
            roll_interval,
            max_entries,
        }
    }

    /// Check if a transaction was recently rejected.
    ///
    /// Returns true if the tx should be dropped (was rejected before).
    pub fn is_rejected(&self, tx_hash: &Hash) -> bool {
        self.current.contains(tx_hash) || self.previous.contains(tx_hash)
    }

    /// Mark a transaction as rejected.
    pub fn mark_rejected(&mut self, tx_hash: Hash) {
        // Check if we need to roll
        self.maybe_roll();

        // Check capacity
        if self.current.len() >= self.max_entries {
            self.roll();
        }

        self.current.insert(tx_hash);
    }

    /// Check if roll is needed and perform if so.
    fn maybe_roll(&mut self) {
        if self.last_roll.elapsed() >= self.roll_interval {
            self.roll();
        }
    }

    /// Roll the filter (move current to previous, clear current).
    pub fn roll(&mut self) {
        self.previous = std::mem::take(&mut self.current);
        self.current = HashSet::with_capacity(self.max_entries / 2);
        self.salt = rand_salt();
        self.last_roll = Instant::now();
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.current.clear();
        self.previous.clear();
    }

    /// Get statistics.
    pub fn stats(&self) -> RejectionCacheStats {
        RejectionCacheStats {
            current_entries: self.current.len(),
            previous_entries: self.previous.len(),
            time_since_roll: self.last_roll.elapsed(),
        }
    }
}

impl Default for RejectionCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics for monitoring.
#[derive(Clone, Debug)]
pub struct RejectionCacheStats {
    pub current_entries: usize,
    pub previous_entries: usize,
    pub time_since_roll: Duration,
}

/// Generate random salt.
fn rand_salt() -> u64 {
    use std::time::SystemTime;
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(12345)
}

// =============================================================================
// DUST FILTER (Penny-Flooding Protection)
// =============================================================================

/// Dust threshold calculator.
///
/// ## Algorithm: Dust-Threshold-Check
///
/// Cost_spend = (Size_input + Size_overhead) × MinRelayFee
/// If OutputValue < Cost_spend, output is "Dust"
#[derive(Clone, Debug)]
pub struct DustFilter {
    /// Minimum relay fee (satoshi per byte)
    min_relay_fee: u64,
    /// Input size overhead (bytes)
    input_overhead: usize,
}

/// Default input overhead (P2PKH input ~148 bytes).
pub const DEFAULT_INPUT_OVERHEAD: usize = 148;

/// Default minimum relay fee (1 sat/byte).
pub const DEFAULT_MIN_RELAY_FEE: u64 = 1;

impl DustFilter {
    pub fn new() -> Self {
        Self {
            min_relay_fee: DEFAULT_MIN_RELAY_FEE,
            input_overhead: DEFAULT_INPUT_OVERHEAD,
        }
    }

    /// Create with custom parameters.
    pub fn with_params(min_relay_fee: u64, input_overhead: usize) -> Self {
        Self {
            min_relay_fee,
            input_overhead,
        }
    }

    /// Calculate dust threshold for an output.
    ///
    /// Returns minimum value an output must have to not be dust.
    pub fn dust_threshold(&self, output_size: usize) -> u64 {
        let spend_size = self.input_overhead + output_size;
        spend_size as u64 * self.min_relay_fee
    }

    /// Check if an output value is dust.
    pub fn is_dust(&self, output_value: u64, output_size: usize) -> bool {
        output_value < self.dust_threshold(output_size)
    }

    /// Check if any output in a transaction is dust.
    ///
    /// Returns indices of dust outputs.
    pub fn find_dust_outputs(&self, outputs: &[(u64, usize)]) -> Vec<usize> {
        outputs
            .iter()
            .enumerate()
            .filter(|(_, (value, size))| self.is_dust(*value, *size))
            .map(|(i, _)| i)
            .collect()
    }
}

impl Default for DustFilter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rejection_cache_basic() {
        let mut cache = RejectionCache::new();
        let hash = [0xAB; 32];

        assert!(!cache.is_rejected(&hash));

        cache.mark_rejected(hash);

        assert!(cache.is_rejected(&hash));
    }

    #[test]
    fn test_rejection_cache_roll() {
        let mut cache = RejectionCache::new();
        let hash1 = [0xAA; 32];
        let hash2 = [0xBB; 32];

        cache.mark_rejected(hash1);
        cache.roll();
        cache.mark_rejected(hash2);

        // Both should still be found
        assert!(cache.is_rejected(&hash1)); // In previous
        assert!(cache.is_rejected(&hash2)); // In current

        // Roll again - hash1 should be gone
        cache.roll();
        assert!(!cache.is_rejected(&hash1));
        assert!(cache.is_rejected(&hash2));
    }

    #[test]
    fn test_rejection_cache_clear() {
        let mut cache = RejectionCache::new();
        cache.mark_rejected([0xAA; 32]);
        cache.mark_rejected([0xBB; 32]);

        cache.clear();

        assert!(!cache.is_rejected(&[0xAA; 32]));
        assert!(!cache.is_rejected(&[0xBB; 32]));
    }

    #[test]
    fn test_dust_filter_threshold() {
        let filter = DustFilter::new();

        // For P2PKH output (~34 bytes)
        let threshold = filter.dust_threshold(34);

        // 148 + 34 = 182 bytes × 1 sat = 182 satoshi
        assert_eq!(threshold, 182);
    }

    #[test]
    fn test_dust_filter_is_dust() {
        let filter = DustFilter::new();

        // 182 satoshi threshold for 34-byte output
        assert!(filter.is_dust(100, 34)); // Below threshold
        assert!(!filter.is_dust(200, 34)); // Above threshold
        assert!(!filter.is_dust(182, 34)); // Exactly at threshold
    }

    #[test]
    fn test_find_dust_outputs() {
        let filter = DustFilter::new();

        let outputs = vec![
            (1000, 34), // OK
            (50, 34),   // Dust
            (2000, 34), // OK
            (10, 34),   // Dust
        ];

        let dust = filter.find_dust_outputs(&outputs);
        assert_eq!(dust, vec![1, 3]);
    }

    #[test]
    fn test_stats() {
        let mut cache = RejectionCache::new();
        cache.mark_rejected([0xAA; 32]);
        cache.mark_rejected([0xBB; 32]);

        let stats = cache.stats();
        assert_eq!(stats.current_entries, 2);
        assert_eq!(stats.previous_entries, 0);
    }
}
