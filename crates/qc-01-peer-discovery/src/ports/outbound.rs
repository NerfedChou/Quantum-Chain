//! # Driven Ports (Outbound SPI)
//!
//! These are the interfaces this subsystem **requires** the host application to implement.
//!
//! Per SPEC-01-PEER-DISCOVERY.md Section 3.2

use crate::domain::{KademliaConfig, NodeId, SocketAddr, Timestamp};

/// Abstract interface for network I/O.
///
/// The host must provide a concrete implementation (e.g., using tokio UDP).
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` to support concurrent access
/// from multiple async tasks.
///
/// # Example Implementation
///
/// ```rust,ignore
/// struct UdpNetworkSocket {
///     socket: tokio::net::UdpSocket,
/// }
///
/// impl NetworkSocket for UdpNetworkSocket {
///     fn send_ping(&self, target: SocketAddr) -> Result<(), NetworkError> {
///         // Serialize PING message and send via UDP
///         todo!()
///     }
///     // ...
/// }
/// ```
pub trait NetworkSocket: Send + Sync {
    /// Send a PING message to a peer.
    ///
    /// Used for liveness checks and eviction challenges (INVARIANT-10).
    fn send_ping(&self, target: SocketAddr) -> Result<(), NetworkError>;

    /// Send a FIND_NODE query to a peer.
    ///
    /// Used for iterative Kademlia lookups.
    ///
    /// # Arguments
    ///
    /// * `target` - The peer to query
    /// * `search_id` - The NodeId we're looking for
    fn send_find_node(&self, target: SocketAddr, search_id: NodeId) -> Result<(), NetworkError>;

    /// Send a PONG response to a peer.
    ///
    /// Response to a PING message.
    fn send_pong(&self, target: SocketAddr) -> Result<(), NetworkError>;
}

/// Errors from network operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkError {
    /// Operation timed out waiting for response
    Timeout,
    /// Remote peer refused connection
    ConnectionRefused,
    /// Invalid socket address
    InvalidAddress,
    /// Message exceeds maximum allowed size
    MessageTooLarge,
}

impl std::fmt::Display for NetworkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NetworkError::Timeout => write!(f, "network operation timed out"),
            NetworkError::ConnectionRefused => write!(f, "connection refused by peer"),
            NetworkError::InvalidAddress => write!(f, "invalid socket address"),
            NetworkError::MessageTooLarge => write!(f, "message exceeds maximum size"),
        }
    }
}

impl std::error::Error for NetworkError {}

/// Abstract interface for time-related operations.
///
/// Enables deterministic testing by injecting controllable time sources.
/// Production implementations use system time; tests use fixed timestamps.
///
/// Reference: SPEC-01 Section 3.2 (`TimeSource` driven port)
///
/// # Example Implementation
///
/// ```rust,ignore
/// struct SystemTimeSource;
///
/// impl TimeSource for SystemTimeSource {
///     fn now(&self) -> Timestamp {
///         Timestamp::new(
///             std::time::SystemTime::now()
///                 .duration_since(std::time::UNIX_EPOCH)
///                 .unwrap()
///                 .as_secs()
///         )
///     }
/// }
/// ```
pub trait TimeSource: Send + Sync {
    /// Get the current timestamp.
    fn now(&self) -> Timestamp;
}

/// Abstract interface for configuration loading.
///
/// Allows different configuration sources (file, environment, etc.)
pub trait ConfigProvider: Send + Sync {
    /// Get list of bootstrap nodes to connect to initially.
    ///
    /// Bootstrap nodes are well-known, stable nodes that help
    /// new nodes discover the network.
    fn get_bootstrap_nodes(&self) -> Vec<SocketAddr>;

    /// Get Kademlia configuration parameters.
    ///
    /// Includes bucket size (k), parallelism (alpha), and security limits.
    fn get_kademlia_config(&self) -> KademliaConfig;
}

/// Abstract interface for proof-of-work validation (Sybil resistance).
///
/// # Security
///
/// NodeId validation helps prevent Sybil attacks by requiring computational
/// work to generate valid NodeIds (e.g., leading zeros in hash).
pub trait NodeIdValidator: Send + Sync {
    /// Verify that a NodeId has sufficient proof-of-work.
    ///
    /// # Arguments
    ///
    /// * `node_id` - The NodeId to validate
    ///
    /// # Returns
    ///
    /// `true` if the NodeId meets the proof-of-work requirements.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Require 16 leading zero bits
    /// fn validate_node_id(&self, id: NodeId) -> bool {
    ///     let bytes = id.as_bytes();
    ///     bytes[0] == 0 && bytes[1] == 0
    /// }
    /// ```
    fn validate_node_id(&self, node_id: NodeId) -> bool;
}



#[cfg(test)]
mod tests {
    use super::*;

    /// Test-only TimeSource returning a fixed timestamp for deterministic assertions.
    struct FixedTimeSource(u64);

    impl TimeSource for FixedTimeSource {
        fn now(&self) -> Timestamp {
            Timestamp::new(self.0)
        }
    }

    #[test]
    fn test_fixed_time_source_returns_configured_value() {
        let source = FixedTimeSource(1000);
        assert_eq!(source.now().as_secs(), 1000);
    }

    #[test]
    fn test_network_error_display() {
        assert_eq!(
            NetworkError::Timeout.to_string(),
            "network operation timed out"
        );
        assert_eq!(
            NetworkError::ConnectionRefused.to_string(),
            "connection refused by peer"
        );
        assert_eq!(
            NetworkError::InvalidAddress.to_string(),
            "invalid socket address"
        );
        assert_eq!(
            NetworkError::MessageTooLarge.to_string(),
            "message exceeds maximum size"
        );
    }
}
