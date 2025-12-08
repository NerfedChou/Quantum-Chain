//! Gateway configuration with validation.
//!
//! Configuration follows SPEC-16 Section 10.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

/// Main gateway configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GatewayConfig {
    /// HTTP server configuration
    pub http: HttpConfig,
    /// WebSocket server configuration
    pub websocket: WebSocketConfig,
    /// Admin server configuration (localhost only by default)
    pub admin: AdminConfig,
    /// Rate limiting configuration
    pub rate_limit: RateLimitConfig,
    /// Request validation limits
    pub limits: LimitsConfig,
    /// Timeout configuration
    pub timeouts: TimeoutConfig,
    /// CORS configuration
    pub cors: CorsConfig,
    /// Chain information
    pub chain: ChainConfig,
    /// Security configuration
    pub security: SecurityConfig,
    /// Method whitelist configuration
    pub methods: MethodsConfig,
    /// Circuit breaker configuration for downstream resilience
    pub circuit_breaker: CircuitBreakerConfig,
    /// TLS configuration (optional)
    pub tls: Option<TlsConfig>,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            http: HttpConfig::default(),
            websocket: WebSocketConfig::default(),
            admin: AdminConfig::default(),
            rate_limit: RateLimitConfig::default(),
            limits: LimitsConfig::default(),
            timeouts: TimeoutConfig::default(),
            cors: CorsConfig::default(),
            chain: ChainConfig::default(),
            security: SecurityConfig::default(),
            methods: MethodsConfig::default(),
            circuit_breaker: CircuitBreakerConfig::default(),
            tls: None,
        }
    }
}

impl GatewayConfig {
    /// Validate configuration
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Validate ports are different
        let ports = [self.http.port, self.websocket.port, self.admin.port];
        let unique_ports: HashSet<_> = ports.iter().collect();
        if unique_ports.len() != ports.len() {
            return Err(ConfigError::DuplicatePorts);
        }

        // Validate rate limits
        if self.rate_limit.requests_per_second == 0 {
            return Err(ConfigError::InvalidRateLimit(
                "requests_per_second cannot be 0".into(),
            ));
        }

        // Validate limits
        if self.limits.max_request_size == 0 {
            return Err(ConfigError::InvalidLimit(
                "max_request_size cannot be 0".into(),
            ));
        }

        if self.limits.max_batch_size == 0 {
            return Err(ConfigError::InvalidLimit(
                "max_batch_size cannot be 0".into(),
            ));
        }

        // Validate timeouts
        if self.timeouts.default.as_millis() == 0 {
            return Err(ConfigError::InvalidTimeout(
                "default timeout cannot be 0".into(),
            ));
        }

        Ok(())
    }

    /// Get HTTP server bind address
    pub fn http_addr(&self) -> SocketAddr {
        SocketAddr::new(self.http.host, self.http.port)
    }

    /// Get WebSocket server bind address
    pub fn ws_addr(&self) -> SocketAddr {
        SocketAddr::new(self.websocket.host, self.websocket.port)
    }

    /// Get Admin server bind address
    pub fn admin_addr(&self) -> SocketAddr {
        SocketAddr::new(self.admin.host, self.admin.port)
    }
}

/// HTTP server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HttpConfig {
    /// Bind address
    pub host: IpAddr,
    /// Port (default: 8545)
    pub port: u16,
    /// Enable HTTP server
    pub enabled: bool,
    /// Keep-alive timeout
    #[serde(with = "humantime_serde")]
    pub keep_alive: Duration,
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            host: IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
            port: 8545,
            enabled: true,
            keep_alive: Duration::from_secs(75),
        }
    }
}

/// WebSocket server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WebSocketConfig {
    /// Bind address
    pub host: IpAddr,
    /// Port (default: 8546)
    pub port: u16,
    /// Enable WebSocket server
    pub enabled: bool,
    /// Max connections per IP
    pub max_connections_per_ip: u32,
    /// Max subscriptions per connection
    pub max_subscriptions_per_connection: u32,
    /// Ping interval
    #[serde(with = "humantime_serde")]
    pub ping_interval: Duration,
    /// Message buffer size
    pub message_buffer_size: usize,
}

impl Default for WebSocketConfig {
    fn default() -> Self {
        Self {
            host: IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
            port: 8546,
            enabled: true,
            max_connections_per_ip: 10,
            max_subscriptions_per_connection: 100,
            ping_interval: Duration::from_secs(30),
            message_buffer_size: 1024,
        }
    }
}

/// Admin server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AdminConfig {
    /// Bind address (localhost only by default for security)
    pub host: IpAddr,
    /// Port (default: 8080)
    pub port: u16,
    /// Enable admin server
    pub enabled: bool,
    /// Required API key (None = no auth required, only localhost check)
    pub api_key: Option<String>,
    /// Allow non-localhost connections (DANGER)
    pub allow_external: bool,
}

impl Default for AdminConfig {
    fn default() -> Self {
        Self {
            host: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port: 8080,
            enabled: true,
            api_key: None,
            allow_external: false,
        }
    }
}

/// Rate limiting configuration per SPEC-16 Section 7.1
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RateLimitConfig {
    /// Requests per second per IP (reads)
    pub requests_per_second: u32,
    /// Write requests per second per IP (sendRawTransaction)
    pub writes_per_second: u32,
    /// Burst allowance (token bucket)
    pub burst_size: u32,
    /// Enable rate limiting
    pub enabled: bool,
    /// IPs to whitelist from rate limiting
    pub whitelist: Vec<IpAddr>,
    /// Window for sliding window rate limit
    #[serde(with = "humantime_serde")]
    pub window: Duration,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            requests_per_second: 100,
            writes_per_second: 10,
            burst_size: 200,
            enabled: true,
            whitelist: vec![IpAddr::V4(Ipv4Addr::LOCALHOST)],
            window: Duration::from_secs(1),
        }
    }
}

/// Request limits configuration per SPEC-16 Section 7.2
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LimitsConfig {
    /// Max request body size in bytes (default: 1MB)
    pub max_request_size: usize,
    /// Max batch size (number of requests in batch)
    pub max_batch_size: usize,
    /// Max response size in bytes (default: 10MB)
    pub max_response_size: usize,
    /// Max block range for eth_getLogs
    pub max_log_block_range: u64,
    /// Max results for eth_getLogs
    pub max_log_results: usize,
}

impl Default for LimitsConfig {
    fn default() -> Self {
        Self {
            max_request_size: 1024 * 1024, // 1MB
            max_batch_size: 100,
            max_response_size: 10 * 1024 * 1024, // 10MB
            max_log_block_range: 10_000,
            max_log_results: 10_000,
        }
    }
}

/// Timeout configuration per SPEC-16 Section 7.3
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TimeoutConfig {
    /// Default timeout for most requests
    #[serde(with = "humantime_serde")]
    pub default: Duration,
    /// Timeout for eth_call (contract simulation)
    #[serde(with = "humantime_serde")]
    pub eth_call: Duration,
    /// Timeout for simple lookups
    #[serde(with = "humantime_serde")]
    pub simple: Duration,
    /// Timeout for eth_getLogs
    #[serde(with = "humantime_serde")]
    pub get_logs: Duration,
    /// Connection timeout
    #[serde(with = "humantime_serde")]
    pub connection: Duration,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            default: Duration::from_secs(10),
            eth_call: Duration::from_secs(30),
            simple: Duration::from_secs(5),
            get_logs: Duration::from_secs(60),
            connection: Duration::from_secs(30),
        }
    }
}

/// CORS configuration per SPEC-16 Section 7.5
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CorsConfig {
    /// Enable CORS
    pub enabled: bool,
    /// Allowed origins ("*" for all)
    pub allowed_origins: Vec<String>,
    /// Allowed methods
    pub allowed_methods: Vec<String>,
    /// Allowed headers
    pub allowed_headers: Vec<String>,
    /// Expose headers
    pub expose_headers: Vec<String>,
    /// Max age for preflight cache
    pub max_age: u64,
    /// Allow credentials
    pub allow_credentials: bool,
}

impl Default for CorsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            allowed_origins: vec!["*".to_string()],
            allowed_methods: vec!["GET".to_string(), "POST".to_string(), "OPTIONS".to_string()],
            allowed_headers: vec!["Content-Type".to_string(), "Authorization".to_string()],
            expose_headers: vec![],
            max_age: 86400, // 24 hours
            allow_credentials: false,
        }
    }
}

/// Chain configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ChainConfig {
    /// Chain ID
    pub chain_id: u64,
    /// Network name
    pub network_name: String,
    /// Client version string
    pub client_version: String,
}

impl Default for ChainConfig {
    fn default() -> Self {
        Self {
            chain_id: 1,
            network_name: "quantum-chain".to_string(),
            client_version: format!(
                "QuantumChain/v{}/linux/rust-{}",
                env!("CARGO_PKG_VERSION"),
                rustc_version()
            ),
        }
    }
}

/// TLS configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    /// Path to certificate file
    pub cert_path: String,
    /// Path to key file
    pub key_path: String,
}

/// Security configuration per SPEC-16 Section 7.5
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SecurityConfig {
    /// List of trusted proxy IPs
    pub trusted_proxies: Vec<IpAddr>,
    /// Trust private IPs (10.x, 172.16.x, 192.168.x)
    pub trust_private_ips: bool,
    /// Number of proxies in chain (for X-Forwarded-For parsing)
    pub proxy_count: usize,
    /// Enable request signing verification
    pub verify_request_signatures: bool,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            trusted_proxies: Vec::new(),
            trust_private_ips: false, // Security-first default
            proxy_count: 1,
            verify_request_signatures: false,
        }
    }
}

/// Method whitelist configuration per SPEC-16 Section 7.3
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MethodsConfig {
    /// Allow unknown methods (for forward compatibility)
    pub allow_unknown: bool,
    /// Additional methods to allow (not in standard registry)
    pub extra_methods: Vec<String>,
    /// Disabled methods (override to deny even standard methods)
    pub disabled_methods: Vec<String>,
}

/// Circuit breaker configuration for downstream subsystem resilience
///
/// The circuit breaker pattern prevents cascading failures when downstream
/// subsystems become unhealthy. When a subsystem fails repeatedly, the circuit
/// opens and requests are rejected immediately until the subsystem recovers.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CircuitBreakerConfig {
    /// Enable circuit breaker
    pub enabled: bool,
    /// Number of failures before opening the circuit
    pub failure_threshold: u32,
    /// Number of successes in half-open state before closing
    pub success_threshold: u32,
    /// Duration before half-open from open state (in seconds)
    pub open_timeout_secs: u64,
    /// Duration to track failure rate (in seconds)
    pub failure_window_secs: u64,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            failure_threshold: 5,
            success_threshold: 3,
            open_timeout_secs: 30,
            failure_window_secs: 60,
        }
    }
}

impl CircuitBreakerConfig {
    /// Convert to the middleware CircuitBreakerConfig
    pub fn to_middleware_config(&self) -> crate::middleware::CircuitBreakerConfig {
        crate::middleware::CircuitBreakerConfig {
            enabled: self.enabled,
            failure_threshold: self.failure_threshold,
            success_threshold: self.success_threshold,
            open_timeout: Duration::from_secs(self.open_timeout_secs),
            failure_window: Duration::from_secs(self.failure_window_secs),
        }
    }
}

/// Configuration errors
#[derive(Debug, Clone, thiserror::Error)]
pub enum ConfigError {
    /// Multiple servers using the same port
    #[error("duplicate ports configured")]
    DuplicatePorts,
    /// Invalid rate limiting configuration
    #[error("invalid rate limit: {0}")]
    InvalidRateLimit(String),
    /// Invalid size or count limit
    #[error("invalid limit: {0}")]
    InvalidLimit(String),
    /// Invalid timeout value
    #[error("invalid timeout: {0}")]
    InvalidTimeout(String),
    /// General configuration error
    #[error("invalid configuration: {0}")]
    Invalid(String),
}

/// Get rustc version for client version string
fn rustc_version() -> &'static str {
    // In real implementation, this would be set at compile time
    "1.75.0"
}

/// Humantime serde module for Duration serialization
mod humantime_serde {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("{}s", duration.as_secs()))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        parse_duration(&s).map_err(serde::de::Error::custom)
    }

    fn parse_duration(s: &str) -> Result<Duration, &'static str> {
        let s = s.trim();
        if let Some(secs) = s.strip_suffix('s') {
            secs.trim()
                .parse::<u64>()
                .map(Duration::from_secs)
                .map_err(|_| "invalid seconds")
        } else if let Some(ms) = s.strip_suffix("ms") {
            ms.trim()
                .parse::<u64>()
                .map(Duration::from_millis)
                .map_err(|_| "invalid milliseconds")
        } else if let Some(mins) = s.strip_suffix('m') {
            mins.trim()
                .parse::<u64>()
                .map(|m| Duration::from_secs(m * 60))
                .map_err(|_| "invalid minutes")
        } else {
            // Try parsing as plain seconds
            s.parse::<u64>()
                .map(Duration::from_secs)
                .map_err(|_| "invalid duration format")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = GatewayConfig::default();
        assert!(config.validate().is_ok());
        assert_eq!(config.http.port, 8545);
        assert_eq!(config.websocket.port, 8546);
        assert_eq!(config.admin.port, 8080);
    }

    #[test]
    fn test_duplicate_ports() {
        let mut config = GatewayConfig::default();
        config.websocket.port = config.http.port;
        assert!(matches!(
            config.validate(),
            Err(ConfigError::DuplicatePorts)
        ));
    }

    #[test]
    fn test_config_addresses() {
        let config = GatewayConfig::default();
        assert_eq!(config.http_addr().port(), 8545);
        assert_eq!(config.ws_addr().port(), 8546);
        assert_eq!(config.admin_addr().port(), 8080);
    }

    #[test]
    fn test_rate_limit_validation() {
        let mut config = GatewayConfig::default();
        config.rate_limit.requests_per_second = 0;
        assert!(matches!(
            config.validate(),
            Err(ConfigError::InvalidRateLimit(_))
        ));
    }
}
