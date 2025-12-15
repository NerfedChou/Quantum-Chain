use std::time::Duration;

/// QUIC connection configuration.
#[derive(Clone, Debug)]
pub struct QuicConfig {
    /// Bind address for the QUIC endpoint
    pub bind_addr: std::net::SocketAddr,
    /// Connection timeout
    pub connect_timeout: Duration,
    /// Idle timeout before connection close
    pub idle_timeout: Duration,
    /// Maximum concurrent bidirectional streams per connection
    pub max_streams: u32,
    /// Enable 0-RTT (with replay protection)
    pub enable_0rtt: bool,
    /// Maximum datagram size (MTU - headers)
    pub max_datagram_size: u16,
    /// Keep-alive interval (0 to disable)
    pub keep_alive_interval: Option<Duration>,
}

impl Default for QuicConfig {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0:0".parse().expect("valid default bind addr"),
            connect_timeout: Duration::from_secs(10),
            idle_timeout: Duration::from_secs(30),
            max_streams: 100,
            enable_0rtt: true,
            max_datagram_size: 1350,
            keep_alive_interval: Some(Duration::from_secs(15)),
        }
    }
}

impl QuicConfig {
    /// Create config for testing with shorter timeouts.
    #[cfg(test)]
    pub fn for_testing() -> Self {
        Self {
            bind_addr: "127.0.0.1:0".parse().expect("valid test bind addr"),
            connect_timeout: Duration::from_secs(2),
            idle_timeout: Duration::from_secs(5),
            max_streams: 10,
            enable_0rtt: false, // Simpler for tests
            max_datagram_size: 1350,
            keep_alive_interval: None,
        }
    }
}
