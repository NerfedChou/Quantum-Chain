//! # Network Adapters (Phase 4)
//!
//! Production-ready network adapters for peer discovery.
//!
//! ## Adapters Provided
//!
//! - `SystemTimeSource` - Production time source using system clock
//! - `UdpNetworkSocket` - UDP-based network I/O (requires "network" feature)
//! - `TomlConfigProvider` - Config file loading (requires "network" feature)
//!
//! ## Feature Flags
//!
//! - `network` - Enables async UDP networking and config file parsing
//!
//! ## Reference
//!
//! SPEC-01-PEER-DISCOVERY.md Section 8 (Phase 4)

use crate::domain::{SocketAddr, Timestamp};
use crate::ports::{NetworkError, NetworkSocket, TimeSource};

// ============================================================================
// SystemTimeSource - Production Time Source
// ============================================================================

/// Production time source using the system clock.
///
/// This adapter implements `TimeSource` using `std::time::SystemTime`.
/// For testing, use the `ControllableTimeSource` from the test utilities.
///
/// # Example
///
/// ```rust
/// use qc_01_peer_discovery::adapters::network::{SystemTimeSource};
/// use qc_01_peer_discovery::ports::TimeSource;
///
/// let time_source = SystemTimeSource::new();
/// let now = time_source.now();
/// assert!(now.as_secs() > 0);
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemTimeSource;

impl SystemTimeSource {
    /// Create a new system time source.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl TimeSource for SystemTimeSource {
    fn now(&self) -> Timestamp {
        use std::time::{SystemTime, UNIX_EPOCH};

        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();

        Timestamp::new(duration.as_secs())
    }
}

// ============================================================================
// NoOpNetworkSocket - Stub for testing without network
// ============================================================================

/// No-operation network socket for testing.
///
/// All operations succeed but don't send any actual packets.
/// Use this for unit testing domain logic without network dependencies.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoOpNetworkSocket;

impl NoOpNetworkSocket {
    /// Create a new no-op socket.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl NetworkSocket for NoOpNetworkSocket {
    fn send_ping(&self, _target: SocketAddr) -> Result<(), NetworkError> {
        Ok(())
    }

    fn send_find_node(
        &self,
        _target: SocketAddr,
        _search_id: crate::domain::NodeId,
    ) -> Result<(), NetworkError> {
        Ok(())
    }

    fn send_pong(&self, _target: SocketAddr) -> Result<(), NetworkError> {
        Ok(())
    }
}

// ============================================================================
// StaticConfigProvider - Hardcoded config for testing/development
// ============================================================================

use crate::domain::KademliaConfig;
use crate::ports::ConfigProvider;

/// Static configuration provider with hardcoded values.
///
/// Useful for testing and development. For production, use `TomlConfigProvider`.
#[derive(Debug, Clone)]
pub struct StaticConfigProvider {
    bootstrap_nodes: Vec<SocketAddr>,
    config: KademliaConfig,
}

impl StaticConfigProvider {
    /// Create with default config and no bootstrap nodes.
    #[must_use]
    pub fn new() -> Self {
        Self {
            bootstrap_nodes: Vec::new(),
            config: KademliaConfig::default(),
        }
    }

    /// Create with specified bootstrap nodes.
    #[must_use]
    pub fn with_bootstrap_nodes(mut self, nodes: Vec<SocketAddr>) -> Self {
        self.bootstrap_nodes = nodes;
        self
    }

    /// Create with specified Kademlia config.
    #[must_use]
    pub fn with_config(mut self, config: KademliaConfig) -> Self {
        self.config = config;
        self
    }
}

impl Default for StaticConfigProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigProvider for StaticConfigProvider {
    fn get_bootstrap_nodes(&self) -> Vec<SocketAddr> {
        self.bootstrap_nodes.clone()
    }

    fn get_kademlia_config(&self) -> KademliaConfig {
        self.config.clone()
    }
}

// ============================================================================
// NoOpNodeIdValidator - Accepts all NodeIds (for testing)
// ============================================================================

use crate::domain::NodeId;
use crate::ports::NodeIdValidator;

/// No-operation NodeId validator that accepts all NodeIds.
///
/// For production, implement a validator that checks proof-of-work.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoOpNodeIdValidator;

impl NoOpNodeIdValidator {
    /// Create a new no-op validator.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl NodeIdValidator for NoOpNodeIdValidator {
    fn validate_node_id(&self, _node_id: NodeId) -> bool {
        true // Accept all NodeIds
    }
}

/// Proof-of-work NodeId validator.
///
/// Requires NodeIds to have a specified number of leading zero bits.
/// This provides Sybil attack resistance per SPEC-01 Section 6.1.
#[derive(Debug, Clone, Copy)]
pub struct ProofOfWorkValidator {
    /// Number of leading zero bits required.
    required_zero_bits: u8,
}

impl ProofOfWorkValidator {
    /// Create a validator requiring specified leading zero bits.
    ///
    /// # Arguments
    ///
    /// * `required_zero_bits` - Number of leading zero bits (e.g., 16 = 2 zero bytes)
    #[must_use]
    pub fn new(required_zero_bits: u8) -> Self {
        Self { required_zero_bits }
    }

    /// Count leading zero bits in a byte slice.
    fn count_leading_zero_bits(bytes: &[u8]) -> u32 {
        let mut count = 0u32;
        for byte in bytes {
            if *byte == 0 {
                count += 8;
            } else {
                count += byte.leading_zeros();
                break;
            }
        }
        count
    }
}

impl NodeIdValidator for ProofOfWorkValidator {
    fn validate_node_id(&self, node_id: NodeId) -> bool {
        let zero_bits = Self::count_leading_zero_bits(node_id.as_bytes());
        zero_bits >= u32::from(self.required_zero_bits)
    }
}

// ============================================================================
// UdpNetworkSocket - Production UDP Socket (requires "network" feature)
// ============================================================================

#[cfg(feature = "network")]
mod udp_socket {
    use super::*;
    use crate::domain::IpAddr;
    use std::net::UdpSocket as StdUdpSocket;
    use std::sync::Arc;

    /// Kademlia message types for UDP protocol.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[repr(u8)]
    pub enum MessageType {
        Ping = 0x01,
        Pong = 0x02,
        FindNode = 0x03,
        Nodes = 0x04,
        Bootstrap = 0x05,
    }

    /// UDP-based network socket for Kademlia protocol.
    ///
    /// This adapter implements the `NetworkSocket` port using standard UDP.
    /// It wraps a `std::net::UdpSocket` for synchronous sends.
    ///
    /// # Wire Protocol
    ///
    /// Messages are simple binary format:
    /// - Byte 0: Message type (PING=0x01, PONG=0x02, FIND_NODE=0x03, NODES=0x04)
    /// - Bytes 1-32: Our NodeId (for PING/PONG/FIND_NODE)
    /// - Bytes 33-64: Target NodeId (for FIND_NODE only)
    /// - For BOOTSTRAP (0x05):
    ///   - Bytes 1-32: NodeId
    ///   - Bytes 33-64: Proof of Work
    ///   - Bytes 65-97: Claimed PubKey
    ///   - Bytes 98-161: Signature (64 bytes)
    ///   - Bytes 162-163: Port
    ///   - Remaining: IP Address (4 or 16 bytes)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use qc_01_peer_discovery::adapters::network::UdpNetworkSocket;
    ///
    /// let socket = UdpNetworkSocket::bind("0.0.0.0:8080", local_node_id)?;
    /// socket.send_ping(peer_addr)?;
    /// ```
    pub struct UdpNetworkSocket {
        socket: Arc<StdUdpSocket>,
        local_node_id: crate::domain::NodeId,
    }

    impl UdpNetworkSocket {
        /// Bind to a local address and create the socket.
        ///
        /// # Arguments
        ///
        /// * `bind_addr` - Local address to bind (e.g., "0.0.0.0:8080")
        /// * `local_node_id` - Our NodeId to include in messages
        ///
        /// # Errors
        ///
        /// Returns error if socket binding fails.
        pub fn bind(
            bind_addr: &str,
            local_node_id: crate::domain::NodeId,
        ) -> std::io::Result<Self> {
            let socket = StdUdpSocket::bind(bind_addr)?;
            socket.set_nonblocking(true)?;
            Ok(Self {
                socket: Arc::new(socket),
                local_node_id,
            })
        }

        /// Get the local address the socket is bound to.
        pub fn local_addr(&self) -> std::io::Result<std::net::SocketAddr> {
            self.socket.local_addr()
        }

        /// Convert domain IpAddr to std::net::IpAddr.
        fn to_std_ip(ip: IpAddr) -> std::net::IpAddr {
            match ip {
                IpAddr::V4(bytes) => std::net::IpAddr::V4(std::net::Ipv4Addr::from(bytes)),
                IpAddr::V6(bytes) => std::net::IpAddr::V6(std::net::Ipv6Addr::from(bytes)),
            }
        }

        /// Convert domain SocketAddr to std::net::SocketAddr.
        fn to_std_addr(addr: SocketAddr) -> std::net::SocketAddr {
            std::net::SocketAddr::new(Self::to_std_ip(addr.ip), addr.port)
        }

        /// Send raw bytes to a target address.
        fn send_to(&self, data: &[u8], target: SocketAddr) -> Result<(), NetworkError> {
            let std_addr = Self::to_std_addr(target);
            match self.socket.send_to(data, std_addr) {
                Ok(_n) => Ok(()),
                Err(e) => match e.kind() {
                    std::io::ErrorKind::WouldBlock => Err(NetworkError::Timeout),
                    std::io::ErrorKind::ConnectionRefused => Err(NetworkError::ConnectionRefused),
                    std::io::ErrorKind::InvalidInput => Err(NetworkError::InvalidAddress),
                    _ => Err(NetworkError::Timeout),
                },
            }
        }
    }

    impl NetworkSocket for UdpNetworkSocket {
        fn send_ping(&self, target: SocketAddr) -> Result<(), NetworkError> {
            // Wire format: [type(1)] [our_node_id(32)]
            let mut msg = [0u8; 33];
            msg[0] = MessageType::Ping as u8;
            msg[1..33].copy_from_slice(self.local_node_id.as_bytes());
            self.send_to(&msg, target)
        }

        fn send_find_node(
            &self,
            target: SocketAddr,
            search_id: crate::domain::NodeId,
        ) -> Result<(), NetworkError> {
            // Wire format: [type(1)] [our_node_id(32)] [search_id(32)]
            let mut msg = [0u8; 65];
            msg[0] = MessageType::FindNode as u8;
            msg[1..33].copy_from_slice(self.local_node_id.as_bytes());
            msg[33..65].copy_from_slice(search_id.as_bytes());
            self.send_to(&msg, target)
        }

        fn send_pong(&self, target: SocketAddr) -> Result<(), NetworkError> {
            // Wire format: [type(1)] [our_node_id(32)]
            let mut msg = [0u8; 33];
            msg[0] = MessageType::Pong as u8;
            msg[1..33].copy_from_slice(self.local_node_id.as_bytes());
            self.send_to(&msg, target)
        }
    }

    // Allow cloning the socket handle
    impl Clone for UdpNetworkSocket {
        fn clone(&self) -> Self {
            Self {
                socket: Arc::clone(&self.socket),
                local_node_id: self.local_node_id,
            }
        }
    }

    /// Parse a raw Bootstrap message.
    ///
    /// # Protocol
    /// \[Type(1)\]\[NodeId(32)\]\[PoW(32)\]\[PubKey(33)\]\[Sig(64)\]\[Port(2)\]\[IP(4/16)\]
    #[cfg(feature = "ipc")]
    pub fn parse_bootstrap_request(
        data: &[u8],
    ) -> Result<crate::ipc::BootstrapRequest, NetworkError> {
        if data.len() < 167 {
            // Min size (IPv4)
            return Err(NetworkError::MessageTooLarge); // Actually TooSmall, but using available error
        }

        if data[0] != MessageType::Bootstrap as u8 {
            return Err(NetworkError::InvalidAddress); // Wrong type
        }

        let mut node_id = [0u8; 32];
        node_id.copy_from_slice(&data[1..33]);

        let mut proof_of_work = [0u8; 32];
        proof_of_work.copy_from_slice(&data[33..65]);

        let mut claimed_pubkey = [0u8; 33];
        claimed_pubkey.copy_from_slice(&data[65..98]);

        let mut signature = [0u8; 64];
        signature.copy_from_slice(&data[98..162]);

        // SECURITY: Bounds check before slice conversion
        // The minimum length check at start (167 bytes) guarantees data[162..164] exists,
        // but we add an explicit check here for defense-in-depth
        if data.len() < 164 {
            return Err(NetworkError::MessageTooLarge); // Data too short for port bytes
        }
        let port_bytes: [u8; 2] = data[162..164]
            .try_into()
            .map_err(|_| NetworkError::InvalidAddress)?;
        let port = u16::from_be_bytes(port_bytes);

        // IP Address (remaining bytes)
        let ip_bytes = &data[164..];
        let ip_address = if ip_bytes.len() == 4 {
            let mut b = [0u8; 4];
            b.copy_from_slice(ip_bytes);
            crate::domain::IpAddr::V4(b)
        } else if ip_bytes.len() == 16 {
            let mut b = [0u8; 16];
            b.copy_from_slice(ip_bytes);
            crate::domain::IpAddr::V6(b)
        } else {
            return Err(NetworkError::InvalidAddress);
        };

        Ok(crate::ipc::BootstrapRequest::new(
            node_id,
            ip_address,
            port,
            proof_of_work,
            claimed_pubkey,
            signature,
        ))
    }
}

#[cfg(feature = "network")]
pub use udp_socket::{MessageType, UdpNetworkSocket};

#[cfg(all(feature = "network", feature = "ipc"))]
pub use udp_socket::parse_bootstrap_request;

// ============================================================================
// TomlConfigProvider - Production Config Loading (requires "network" feature)
// ============================================================================

#[cfg(feature = "network")]
mod toml_config {
    use super::*;
    use crate::domain::IpAddr;
    use serde::Deserialize;
    use std::fs;
    use std::path::Path;

    /// Configuration file structure.
    #[derive(Debug, Deserialize)]
    struct ConfigFile {
        #[serde(default)]
        bootstrap: BootstrapConfig,
        #[serde(default)]
        kademlia: KademliaConfigFile,
    }

    #[derive(Debug, Deserialize, Default)]
    struct BootstrapConfig {
        #[serde(default)]
        nodes: Vec<String>,
    }

    #[derive(Debug, Deserialize, Default)]
    struct KademliaConfigFile {
        k: Option<usize>,
        alpha: Option<usize>,
        max_peers_per_subnet: Option<usize>,
        max_pending_peers: Option<usize>,
        eviction_challenge_timeout_secs: Option<u64>,
        verification_timeout_secs: Option<u64>,
    }

    /// TOML-based configuration provider.
    ///
    /// Loads peer discovery configuration from a TOML file.
    ///
    /// # Config File Format
    ///
    /// ```toml
    /// [bootstrap]
    /// nodes = [
    ///     "192.168.1.100:8080",
    ///     "10.0.0.1:8080"
    /// ]
    ///
    /// [kademlia]
    /// k = 20
    /// alpha = 3
    /// max_peers_per_subnet = 2
    /// max_pending_peers = 1024
    /// eviction_challenge_timeout_secs = 5
    /// verification_timeout_secs = 10
    /// ```
    pub struct TomlConfigProvider {
        bootstrap_nodes: Vec<SocketAddr>,
        config: KademliaConfig,
    }

    impl TomlConfigProvider {
        /// Load configuration from a TOML file.
        ///
        /// # Arguments
        ///
        /// * `path` - Path to the config file
        ///
        /// # Errors
        ///
        /// Returns error if file cannot be read or parsed.
        pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
            let content = fs::read_to_string(path.as_ref()).map_err(|e| ConfigError::Io {
                path: path.as_ref().display().to_string(),
                error: e.to_string(),
            })?;

            Self::parse(&content)
        }

        /// Parse configuration from a TOML string.
        pub fn parse(content: &str) -> Result<Self, ConfigError> {
            let file: ConfigFile =
                toml::from_str(content).map_err(|e| ConfigError::Parse(e.to_string()))?;

            // Parse bootstrap nodes
            let mut bootstrap_nodes = Vec::new();
            for node_str in &file.bootstrap.nodes {
                let Some(addr) = Self::parse_socket_addr(node_str) else {
                    continue;
                };
                bootstrap_nodes.push(addr);
            }

            // Build Kademlia config with defaults
            let kc = file.kademlia;
            let config = KademliaConfig {
                k: kc.k.unwrap_or(20),
                alpha: kc.alpha.unwrap_or(3),
                max_peers_per_subnet: kc.max_peers_per_subnet.unwrap_or(2),
                max_pending_peers: kc.max_pending_peers.unwrap_or(1024),
                eviction_challenge_timeout_secs: kc.eviction_challenge_timeout_secs.unwrap_or(5),
                verification_timeout_secs: kc.verification_timeout_secs.unwrap_or(10),
            };

            Ok(Self {
                bootstrap_nodes,
                config,
            })
        }

        /// Parse a socket address string like "192.168.1.100:8080".
        fn parse_socket_addr(s: &str) -> Option<SocketAddr> {
            let std_addr: std::net::SocketAddr = s.parse().ok()?;
            let ip = match std_addr.ip() {
                std::net::IpAddr::V4(v4) => IpAddr::V4(v4.octets()),
                std::net::IpAddr::V6(v6) => IpAddr::V6(v6.octets()),
            };
            Some(SocketAddr::new(ip, std_addr.port()))
        }
    }

    impl ConfigProvider for TomlConfigProvider {
        fn get_bootstrap_nodes(&self) -> Vec<SocketAddr> {
            self.bootstrap_nodes.clone()
        }

        fn get_kademlia_config(&self) -> KademliaConfig {
            self.config.clone()
        }
    }

    /// Errors that can occur during config loading.
    #[derive(Debug, Clone)]
    pub enum ConfigError {
        /// File I/O error.
        Io { path: String, error: String },
        /// TOML parsing error.
        Parse(String),
    }

    impl std::fmt::Display for ConfigError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::Io { path, error } => write!(f, "Failed to read {}: {}", path, error),
                Self::Parse(e) => write!(f, "Failed to parse config: {}", e),
            }
        }
    }

    impl std::error::Error for ConfigError {}
}

#[cfg(feature = "network")]
pub use toml_config::{ConfigError, TomlConfigProvider};

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::IpAddr;

    #[test]
    fn test_system_time_source_returns_nonzero() {
        let source = SystemTimeSource::new();
        let now = source.now();
        // Should be after Unix epoch (reasonably recent)
        assert!(now.as_secs() > 1_700_000_000); // After ~2024
    }

    #[test]
    fn test_system_time_source_is_monotonic() {
        let source = SystemTimeSource::new();
        let t1 = source.now();
        let t2 = source.now();
        assert!(t2.as_secs() >= t1.as_secs());
    }

    #[test]
    fn test_noop_network_socket() {
        let socket = NoOpNetworkSocket::new();
        let addr = SocketAddr::new(IpAddr::v4(127, 0, 0, 1), 8080);

        assert!(socket.send_ping(addr).is_ok());
        assert!(socket.send_pong(addr).is_ok());
        assert!(socket.send_find_node(addr, NodeId::new([1u8; 32])).is_ok());
    }

    #[test]
    fn test_static_config_provider_defaults() {
        let provider = StaticConfigProvider::new();
        assert!(provider.get_bootstrap_nodes().is_empty());
        assert_eq!(provider.get_kademlia_config().k, 20);
    }

    #[test]
    fn test_static_config_provider_with_bootstrap() {
        let nodes = vec![
            SocketAddr::new(IpAddr::v4(192, 168, 1, 100), 8080),
            SocketAddr::new(IpAddr::v4(10, 0, 0, 1), 8080),
        ];
        let provider = StaticConfigProvider::new().with_bootstrap_nodes(nodes.clone());
        assert_eq!(provider.get_bootstrap_nodes().len(), 2);
    }

    #[test]
    fn test_noop_node_id_validator() {
        let validator = NoOpNodeIdValidator::new();
        assert!(validator.validate_node_id(NodeId::new([0u8; 32])));
        assert!(validator.validate_node_id(NodeId::new([255u8; 32])));
    }

    #[test]
    fn test_proof_of_work_validator() {
        let validator = ProofOfWorkValidator::new(16); // Require 16 leading zero bits (2 bytes)

        // NodeId with 2 zero bytes at start - should pass
        let mut valid_id = [255u8; 32];
        valid_id[0] = 0;
        valid_id[1] = 0;
        assert!(validator.validate_node_id(NodeId::new(valid_id)));

        // NodeId with only 1 zero byte - should fail (only 8 zero bits)
        let mut invalid_id = [255u8; 32];
        invalid_id[0] = 0;
        assert!(!validator.validate_node_id(NodeId::new(invalid_id)));

        // NodeId with no zero bytes - should fail
        assert!(!validator.validate_node_id(NodeId::new([255u8; 32])));
    }

    #[test]
    fn test_proof_of_work_validator_partial_byte() {
        // Require 12 leading zero bits (1 byte + 4 bits)
        let validator = ProofOfWorkValidator::new(12);

        // [0x00, 0x0F, ...] = 8 + 4 = 12 leading zeros - should pass
        let mut id = [255u8; 32];
        id[0] = 0x00;
        id[1] = 0x0F; // 0000_1111 = 4 leading zeros
        assert!(validator.validate_node_id(NodeId::new(id)));

        // [0x00, 0x1F, ...] = 8 + 3 = 11 leading zeros - should fail
        id[1] = 0x1F; // 0001_1111 = 3 leading zeros
        assert!(!validator.validate_node_id(NodeId::new(id)));
    }

    #[cfg(feature = "network")]
    mod network_tests {
        use super::*;

        #[test]
        fn test_toml_config_provider_parse() {
            let toml = r#"
                [bootstrap]
                nodes = ["192.168.1.100:8080", "10.0.0.1:9000"]
                
                [kademlia]
                k = 25
                alpha = 5
                max_pending_peers = 2048
            "#;

            let provider = TomlConfigProvider::parse(toml).unwrap();
            assert_eq!(provider.get_bootstrap_nodes().len(), 2);

            let config = provider.get_kademlia_config();
            assert_eq!(config.k, 25);
            assert_eq!(config.alpha, 5);
            assert_eq!(config.max_pending_peers, 2048);
            assert_eq!(config.max_peers_per_subnet, 2); // default
        }

        #[test]
        fn test_toml_config_provider_empty() {
            let toml = "";
            let provider = TomlConfigProvider::parse(toml).unwrap();
            assert!(provider.get_bootstrap_nodes().is_empty());
            assert_eq!(provider.get_kademlia_config().k, 20); // default
        }
    }
}
