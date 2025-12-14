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

// =============================================================================
// SECURITY HARDENING PORTS (V2.5)
// =============================================================================

/// Abstract interface for cryptographically secure randomness.
///
/// # Security (V2.5 - Anti-Eclipse Hardening)
///
/// This port enables true random peer selection, preventing attackers from
/// predicting which peer will be selected for outbound connections.
///
/// **Without this:** Deterministic selection (e.g., always `entries[0]`)
/// allows attackers to position malicious peers for guaranteed selection.
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` for concurrent access.
pub trait RandomSource: Send + Sync {
    /// Generate a random usize in range [0, max).
    ///
    /// # Panics
    ///
    /// May panic if `max == 0` (implementation-defined).
    fn random_usize(&self, max: usize) -> usize;

    /// Shuffle a slice in-place using Fisher-Yates algorithm.
    ///
    /// # Security
    ///
    /// Must use CSPRNG for shuffle decisions. Weak PRNGs allow
    /// attackers to predict shuffle outcomes.
    fn shuffle_slice(&self, slice: &mut [u8]);
}

/// Abstract interface for DoS-resistant hashing.
///
/// # Security (V2.5 - Hash Collision Defense)
///
/// Provides keyed hashing (SipHash or similar) to prevent attackers
/// from crafting inputs that collide, overwhelming specific buckets.
///
/// **Key management:** Key should be:
/// - Generated randomly on node startup
/// - NOT derived from predictable values (NodeId, IP, etc.)
/// - Kept secret (not transmitted on network)
pub trait SecureHasher: Send + Sync {
    /// Compute keyed hash of data.
    ///
    /// # Returns
    ///
    /// Hash value suitable for bucket indexing.
    fn hash(&self, data: &[u8]) -> u64;

    /// Compute keyed hash combining two byte slices.
    ///
    /// Used for bucket calculation: `hash(source_subnet || addr_subnet)`
    fn hash_combined(&self, a: &[u8], b: &[u8]) -> u64;
}

/// Abstract interface for domain-level rate limiting.
///
/// # Security (V2.5 - Defense in Depth)
///
/// Provides a backstop rate limiter in the domain layer.
/// Even if adapters misconfigure their rate limiting, the domain
/// provides a final defense.
///
/// # Implementation Notes
///
/// - Rate limits should be configurable per operation type
/// - Window-based limiting (e.g., 10 requests per second)
/// - Key should include relevant context (IP, NodeId, operation)
pub trait RateLimiter: Send + Sync {
    /// Check if an operation should be allowed.
    ///
    /// # Arguments
    ///
    /// * `key` - Unique identifier for the rate limit bucket (e.g., IP bytes)
    /// * `limit` - Maximum operations allowed in window
    /// * `window_secs` - Time window in seconds
    ///
    /// # Returns
    ///
    /// `true` if operation is allowed, `false` if rate limited.
    fn check_rate(&self, key: &[u8], limit: u32, window_secs: u64) -> bool;
}

/// Abstract interface for ENR signature verification.
///
/// # Security (ENR Signature Verification)
///
/// Verifies secp256k1 ECDSA signatures on Ethereum Node Records (ENR).
/// This ensures that ENR data actually came from the claimed public key owner.
///
/// **Without proper verification:** Attackers can forge ENR records claiming
/// any identity, poisoning peer discovery with fake nodes.
///
/// # Implementation Notes
///
/// Production implementations should use a vetted secp256k1 library
/// (e.g., `k256`, `secp256k1`) for the actual ECDSA verification.
pub trait EnrSignatureVerifier: Send + Sync {
    /// Verify a secp256k1 ECDSA signature.
    ///
    /// # Arguments
    ///
    /// * `message` - The signing payload (ENR content hash)
    /// * `signature` - 64-byte ECDSA signature (r || s)
    /// * `public_key` - 33-byte compressed secp256k1 public key
    ///
    /// # Returns
    ///
    /// `true` if signature is valid for the message and public key.
    fn verify_signature(
        &self,
        message: &[u8],
        signature: &[u8; 64],
        public_key: &[u8; 33],
    ) -> bool;

    /// Compute signing message hash for ENR.
    ///
    /// # Arguments
    ///
    /// * `payload` - Raw ENR signing payload bytes
    ///
    /// # Returns
    ///
    /// 32-byte hash suitable for ECDSA signing (typically Keccak256 for Ethereum).
    fn hash_signing_payload(&self, payload: &[u8]) -> [u8; 32];
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
