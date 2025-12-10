//! # QUIC Transport Layer
//!
//! Encrypted transport with HTTP/3 capabilities.
//!
//! ## Security Properties
//!
//! - Encrypted headers (prevents traffic analysis)
//! - Multi-streaming (solves TCP HoL blocking)
//! - 0-RTT with replay protection
//!
//! ## Reference
//!
//! - RFC 9000 (QUIC)
//! - RFC 9001 (QUIC-TLS)

use std::net::SocketAddr;
use std::time::Duration;

/// QUIC connection configuration.
#[derive(Clone, Debug)]
pub struct QuicConfig {
    /// Server address
    pub server_addr: SocketAddr,
    /// Connection timeout
    pub connect_timeout: Duration,
    /// Idle timeout before connection close
    pub idle_timeout: Duration,
    /// Maximum concurrent streams
    pub max_streams: u32,
    /// Enable 0-RTT (with replay protection)
    pub enable_0rtt: bool,
    /// Maximum datagram size
    pub max_datagram_size: u16,
}

impl Default for QuicConfig {
    fn default() -> Self {
        Self {
            server_addr: "0.0.0.0:8443".parse().unwrap(),
            connect_timeout: Duration::from_secs(10),
            idle_timeout: Duration::from_secs(30),
            max_streams: 100,
            enable_0rtt: true,
            max_datagram_size: 1350,
        }
    }
}

/// 0-RTT replay protection using idempotency tokens.
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
    /// Create new replay protection.
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

    /// Clear all tokens.
    pub fn clear(&mut self) {
        self.seen_tokens.clear();
        self.window_start = std::time::Instant::now();
    }
}

impl Default for ReplayProtection {
    fn default() -> Self {
        Self::new(Duration::from_secs(60))
    }
}

/// Connection state for a QUIC peer.
#[derive(Clone, Debug)]
pub struct QuicConnectionState {
    /// Remote peer address
    pub remote_addr: SocketAddr,
    /// Connection ID
    pub connection_id: [u8; 16],
    /// Is connection established
    pub established: bool,
    /// RTT estimate
    pub rtt_estimate: Duration,
    /// Bytes sent
    pub bytes_sent: u64,
    /// Bytes received
    pub bytes_received: u64,
}

/// QUIC transport handle (placeholder for actual quinn integration).
#[derive(Debug)]
pub struct QuicTransport {
    config: QuicConfig,
    replay_protection: ReplayProtection,
}

impl QuicTransport {
    /// Create new QUIC transport.
    pub fn new(config: QuicConfig) -> Self {
        Self {
            config,
            replay_protection: ReplayProtection::default(),
        }
    }

    /// Get config.
    pub fn config(&self) -> &QuicConfig {
        &self.config
    }

    /// Check 0-RTT token for replay.
    pub fn check_0rtt_token(&mut self, token: &[u8; 32]) -> bool {
        if !self.config.enable_0rtt {
            return false;
        }
        self.replay_protection.check_token(token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = QuicConfig::default();
        assert_eq!(config.max_streams, 100);
        assert!(config.enable_0rtt);
    }

    #[test]
    fn test_replay_protection_fresh_token() {
        let mut rp = ReplayProtection::new(Duration::from_secs(60));
        let token = [1u8; 32];

        assert!(rp.check_token(&token));
    }

    #[test]
    fn test_replay_protection_rejects_duplicate() {
        let mut rp = ReplayProtection::new(Duration::from_secs(60));
        let token = [2u8; 32];

        assert!(rp.check_token(&token));
        assert!(!rp.check_token(&token)); // Replay rejected
    }

    #[test]
    fn test_replay_protection_different_tokens() {
        let mut rp = ReplayProtection::new(Duration::from_secs(60));

        assert!(rp.check_token(&[1u8; 32]));
        assert!(rp.check_token(&[2u8; 32]));
        assert!(rp.check_token(&[3u8; 32]));
    }

    #[test]
    fn test_quic_transport_0rtt() {
        let mut transport = QuicTransport::new(QuicConfig::default());
        let token = [0xAB; 32];

        assert!(transport.check_0rtt_token(&token));
        assert!(!transport.check_0rtt_token(&token));
    }
}
