use crate::domain::{KademliaConfig, SocketAddr};
use crate::ports::ConfigProvider;

// ============================================================================
// StaticConfigProvider - Hardcoded config for testing/development
// ============================================================================

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
            let bootstrap_nodes: Vec<_> = file
                .bootstrap
                .nodes
                .iter()
                .filter_map(|node_str| Self::parse_socket_addr(node_str))
                .collect();

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
        Io {
            /// Path of the file that failed to load.
            path: String,
            /// Error message from the I/O operation.
            error: String,
        },
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
