use std::time::Duration;

/// 0-RTT replay protection using idempotency tokens.
///
/// Prevents replay attacks on 0-RTT data by tracking seen tokens
/// within a sliding time window.
#[derive(Clone, Debug)]
pub struct ReplayProtection {
    /// Idempotency tokens seen in current window
    seen_tokens: std::collections::HashSet<[u8; 32]>,
    /// Window start time
    window_start: std::time::Instant,
    /// Window duration
    window_duration: Duration,
}

impl ReplayProtection {
    /// Create new replay protection with specified window.
    pub fn new(window_duration: Duration) -> Self {
        Self {
            seen_tokens: std::collections::HashSet::new(),
            window_start: std::time::Instant::now(),
            window_duration,
        }
    }

    /// Check if a 0-RTT token is valid (not replayed).
    ///
    /// Returns `true` if token is fresh, `false` if replayed.
    pub fn check_token(&mut self, token: &[u8; 32]) -> bool {
        // Rotate window if expired
        if self.window_start.elapsed() > self.window_duration {
            self.seen_tokens.clear();
            self.window_start = std::time::Instant::now();
        }

        // Check if seen before
        if self.seen_tokens.contains(token) {
            return false;
        }

        self.seen_tokens.insert(*token);
        true
    }

    /// Clear all tokens (e.g., on key rotation).
    pub fn clear(&mut self) {
        self.seen_tokens.clear();
        self.window_start = std::time::Instant::now();
    }

    /// Get number of tracked tokens.
    pub fn token_count(&self) -> usize {
        self.seen_tokens.len()
    }
}

impl Default for ReplayProtection {
    fn default() -> Self {
        Self::new(Duration::from_secs(60))
    }
}
