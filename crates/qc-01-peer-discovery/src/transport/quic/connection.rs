use std::net::SocketAddr;
use std::time::Duration;

/// Connection state for a QUIC peer.
#[derive(Clone, Debug)]
pub struct QuicConnectionState {
    /// Remote peer address
    pub remote_addr: SocketAddr,
    /// Connection ID (first 16 bytes of QUIC connection ID)
    pub connection_id: [u8; 16],
    /// Is connection fully established (handshake complete)
    pub established: bool,
    /// Smoothed RTT estimate
    pub rtt_estimate: Duration,
    /// Total bytes sent
    pub bytes_sent: u64,
    /// Total bytes received
    pub bytes_received: u64,
    /// When connection was established
    pub connected_at: std::time::Instant,
    /// Number of active streams
    pub active_streams: u32,
}

impl QuicConnectionState {
    /// Create new connection state.
    pub fn new(remote_addr: SocketAddr, connection_id: [u8; 16]) -> Self {
        Self {
            remote_addr,
            connection_id,
            established: false,
            rtt_estimate: Duration::from_millis(100), // Initial estimate
            bytes_sent: 0,
            bytes_received: 0,
            connected_at: std::time::Instant::now(),
            active_streams: 0,
        }
    }

    /// Check if connection is healthy (not stale).
    pub fn is_healthy(&self, max_idle: Duration) -> bool {
        self.established && self.connected_at.elapsed() < max_idle
    }
}
