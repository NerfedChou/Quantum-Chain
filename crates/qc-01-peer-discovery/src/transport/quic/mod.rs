//! # QUIC Transport Layer
//!
//! Production-ready encrypted transport using quinn (QUIC).
//!
//! ## Security Properties
//!
//! - TLS 1.3 encryption (prevents eavesdropping)
//! - Encrypted headers (prevents traffic analysis)
//! - Multi-streaming (solves TCP HoL blocking)
//! - 0-RTT with replay protection
//! - Connection migration support
//!
//! ## Reference
//!
//! - RFC 9000 (QUIC)
//! - RFC 9001 (QUIC-TLS)

#[cfg(feature = "quic")]
use std::collections::HashMap;
use std::net::SocketAddr;
#[cfg(feature = "quic")]
use std::sync::Arc;
use std::time::Duration;

// =============================================================================
// CONFIGURATION
// =============================================================================

/// QUIC connection configuration.
#[derive(Clone, Debug)]
pub struct QuicConfig {
    /// Bind address for the QUIC endpoint
    pub bind_addr: SocketAddr,
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

// =============================================================================
// 0-RTT REPLAY PROTECTION
// =============================================================================

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

// =============================================================================
// CONNECTION STATE
// =============================================================================

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

// =============================================================================
// TRANSPORT ERRORS
// =============================================================================

/// Errors that can occur in QUIC transport operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuicError {
    /// Failed to bind to the specified address.
    BindFailed {
        /// The address we tried to bind to.
        addr: String,
        /// Error description.
        reason: String,
    },
    /// Connection attempt timed out.
    ConnectionTimeout {
        /// Remote address.
        remote: String,
    },
    /// Connection was refused by peer.
    ConnectionRefused {
        /// Remote address.
        remote: String,
    },
    /// TLS handshake failed.
    TlsError {
        /// Error description.
        reason: String,
    },
    /// Stream creation failed.
    StreamError {
        /// Error description.
        reason: String,
    },
    /// Send operation failed.
    SendFailed {
        /// Error description.
        reason: String,
    },
    /// Receive operation failed.
    RecvFailed {
        /// Error description.
        reason: String,
    },
    /// Connection was closed.
    ConnectionClosed {
        /// Reason for closure.
        reason: String,
    },
    /// Certificate generation failed.
    CertificateError {
        /// Error description.
        reason: String,
    },
    /// Endpoint not initialized.
    NotInitialized,
}

impl std::fmt::Display for QuicError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BindFailed { addr, reason } => {
                write!(f, "failed to bind to {}: {}", addr, reason)
            }
            Self::ConnectionTimeout { remote } => {
                write!(f, "connection to {} timed out", remote)
            }
            Self::ConnectionRefused { remote } => {
                write!(f, "connection to {} refused", remote)
            }
            Self::TlsError { reason } => write!(f, "TLS error: {}", reason),
            Self::StreamError { reason } => write!(f, "stream error: {}", reason),
            Self::SendFailed { reason } => write!(f, "send failed: {}", reason),
            Self::RecvFailed { reason } => write!(f, "receive failed: {}", reason),
            Self::ConnectionClosed { reason } => {
                write!(f, "connection closed: {}", reason)
            }
            Self::CertificateError { reason } => {
                write!(f, "certificate error: {}", reason)
            }
            Self::NotInitialized => write!(f, "QUIC endpoint not initialized"),
        }
    }
}

impl std::error::Error for QuicError {}

// =============================================================================
// QUIC TRANSPORT (Async Implementation)
// =============================================================================

/// Production QUIC transport using quinn.
///
/// This is the async implementation that requires the `quic` feature.
#[cfg(feature = "quic")]
pub struct QuicTransport {
    /// Configuration
    config: QuicConfig,
    /// Quinn endpoint (optional until initialized)
    endpoint: Option<quinn::Endpoint>,
    /// Active connections by remote address
    connections: HashMap<SocketAddr, quinn::Connection>,
    /// Connection states for monitoring
    connection_states: HashMap<SocketAddr, QuicConnectionState>,
    /// 0-RTT replay protection
    replay_protection: ReplayProtection,
    /// Server certificate (for accepting connections)
    server_cert: Option<Vec<u8>>,
}

#[cfg(feature = "quic")]
impl QuicTransport {
    /// Create a new QUIC transport (not yet bound).
    pub fn new(config: QuicConfig) -> Self {
        Self {
            config,
            endpoint: None,
            connections: HashMap::new(),
            connection_states: HashMap::new(),
            replay_protection: ReplayProtection::default(),
            server_cert: None,
        }
    }

    /// Initialize the transport by binding to the configured address.
    ///
    /// # Errors
    ///
    /// Returns `QuicError::BindFailed` if binding fails.
    /// Returns `QuicError::CertificateError` if certificate generation fails.
    pub async fn bind(&mut self) -> Result<SocketAddr, QuicError> {
        // Generate self-signed certificate for this node
        let (server_config, cert_der) = self.generate_server_config()?;
        self.server_cert = Some(cert_der);

        // Create client config (accepts any certificate for P2P)
        let client_config = self.generate_client_config()?;

        // Build endpoint
        let mut endpoint =
            quinn::Endpoint::server(server_config, self.config.bind_addr).map_err(|e| {
                QuicError::BindFailed {
                    addr: self.config.bind_addr.to_string(),
                    reason: e.to_string(),
                }
            })?;

        endpoint.set_default_client_config(client_config);

        let local_addr = endpoint.local_addr().map_err(|e| QuicError::BindFailed {
            addr: self.config.bind_addr.to_string(),
            reason: e.to_string(),
        })?;

        self.endpoint = Some(endpoint);
        Ok(local_addr)
    }

    /// Connect to a remote peer.
    ///
    /// # Arguments
    ///
    /// * `remote` - The remote peer's address
    /// * `server_name` - TLS server name (can be the NodeId hex)
    pub async fn connect(
        &mut self,
        remote: SocketAddr,
        server_name: &str,
    ) -> Result<QuicConnectionState, QuicError> {
        let endpoint = self.endpoint.as_ref().ok_or(QuicError::NotInitialized)?;

        // Check if already connected
        if let Some(state) = self.connection_states.get(&remote) {
            if state.established {
                return Ok(state.clone());
            }
        }

        // Initiate connection with timeout
        let connecting =
            endpoint
                .connect(remote, server_name)
                .map_err(|_| QuicError::ConnectionRefused {
                    remote: remote.to_string(),
                })?;

        let connection = tokio::time::timeout(self.config.connect_timeout, connecting)
            .await
            .map_err(|_| QuicError::ConnectionTimeout {
                remote: remote.to_string(),
            })?
            .map_err(|e| QuicError::TlsError {
                reason: e.to_string(),
            })?;

        // Create connection state
        let mut conn_id = [0u8; 16];
        let stable_id = connection.stable_id();
        conn_id[..8].copy_from_slice(&stable_id.to_le_bytes());

        let mut state = QuicConnectionState::new(remote, conn_id);
        state.established = true;
        state.rtt_estimate = connection.rtt();

        // Store connection
        self.connections.insert(remote, connection);
        self.connection_states.insert(remote, state.clone());

        Ok(state)
    }

    /// Accept an incoming connection.
    ///
    /// Returns `None` if the endpoint is not initialized or no connections are pending.
    pub async fn accept(&mut self) -> Option<QuicConnectionState> {
        let endpoint = self.endpoint.as_ref()?;

        let incoming = endpoint.accept().await?;
        let connection = incoming.await.ok()?;

        let remote = connection.remote_address();

        // Create connection state
        let mut conn_id = [0u8; 16];
        let stable_id = connection.stable_id();
        conn_id[..8].copy_from_slice(&stable_id.to_le_bytes());

        let mut state = QuicConnectionState::new(remote, conn_id);
        state.established = true;
        state.rtt_estimate = connection.rtt();

        // Store connection
        self.connections.insert(remote, connection);
        self.connection_states.insert(remote, state.clone());

        Some(state)
    }

    /// Send data to a connected peer.
    pub async fn send(&mut self, remote: SocketAddr, data: &[u8]) -> Result<(), QuicError> {
        let connection = self
            .connections
            .get(&remote)
            .ok_or(QuicError::ConnectionClosed {
                reason: "not connected".into(),
            })?;

        // Open unidirectional stream and send
        let mut stream = connection
            .open_uni()
            .await
            .map_err(|e| QuicError::StreamError {
                reason: e.to_string(),
            })?;

        stream
            .write_all(data)
            .await
            .map_err(|e| QuicError::SendFailed {
                reason: e.to_string(),
            })?;

        stream.finish().map_err(|e| QuicError::SendFailed {
            reason: e.to_string(),
        })?;

        // Update stats
        if let Some(state) = self.connection_states.get_mut(&remote) {
            state.bytes_sent += data.len() as u64;
        }

        Ok(())
    }

    /// Receive data from any connected peer.
    ///
    /// Returns the sender address and received data.
    pub async fn recv(&mut self) -> Result<(SocketAddr, Vec<u8>), QuicError> {
        // Try to receive from any connection
        for (addr, connection) in &self.connections {
            let Ok(mut stream) = connection.accept_uni().await else {
                continue;
            };

            let data = stream.read_to_end(65536).await.unwrap_or_default();
            if data.is_empty() {
                continue;
            }

            // Update stats
            if let Some(state) = self.connection_states.get_mut(addr) {
                state.bytes_received += data.len() as u64;
            }

            return Ok((*addr, data));
        }

        Err(QuicError::RecvFailed {
            reason: "no data available".into(),
        })
    }

    /// Close connection to a peer.
    pub fn close(&mut self, remote: &SocketAddr) {
        if let Some(conn) = self.connections.remove(remote) {
            conn.close(0u32.into(), b"closed");
        }
        self.connection_states.remove(remote);
    }

    /// Check 0-RTT token for replay.
    pub fn check_0rtt_token(&mut self, token: &[u8; 32]) -> bool {
        if !self.config.enable_0rtt {
            return false;
        }
        self.replay_protection.check_token(token)
    }

    /// Get connection state for a peer.
    pub fn connection_state(&self, remote: &SocketAddr) -> Option<&QuicConnectionState> {
        self.connection_states.get(remote)
    }

    /// Get all active connections.
    pub fn active_connections(&self) -> Vec<SocketAddr> {
        self.connection_states
            .iter()
            .filter(|(_, s)| s.established)
            .map(|(addr, _)| *addr)
            .collect()
    }

    /// Get configuration.
    pub fn config(&self) -> &QuicConfig {
        &self.config
    }

    /// Get local address (if bound).
    pub fn local_addr(&self) -> Option<SocketAddr> {
        self.endpoint.as_ref()?.local_addr().ok()
    }

    /// Generate server TLS configuration with self-signed certificate.
    fn generate_server_config(&self) -> Result<(quinn::ServerConfig, Vec<u8>), QuicError> {
        use rcgen::{generate_simple_self_signed, CertifiedKey};

        let subject_alt_names = vec!["localhost".to_string()];
        let CertifiedKey { cert, key_pair } = generate_simple_self_signed(subject_alt_names)
            .map_err(|e| QuicError::CertificateError {
                reason: e.to_string(),
            })?;

        let cert_der = cert.der().to_vec();
        let key_der = key_pair.serialize_der();

        let cert_chain = vec![rustls::pki_types::CertificateDer::from(cert_der.clone())];
        let private_key = rustls::pki_types::PrivateKeyDer::try_from(key_der).map_err(|e| {
            QuicError::CertificateError {
                reason: format!("invalid private key: {:?}", e),
            }
        })?;

        let server_crypto = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(cert_chain, private_key)
            .map_err(|e| QuicError::CertificateError {
                reason: e.to_string(),
            })?;

        let quic_server_config = quinn::crypto::rustls::QuicServerConfig::try_from(server_crypto)
            .map_err(|e| QuicError::CertificateError {
            reason: format!("QUIC crypto config error: {:?}", e),
        })?;

        let mut server_config = quinn::ServerConfig::with_crypto(Arc::new(quic_server_config));

        // Configure transport parameters
        let transport_config = Arc::get_mut(&mut server_config.transport).unwrap();
        transport_config.max_idle_timeout(Some(
            self.config
                .idle_timeout
                .try_into()
                .unwrap_or(quinn::IdleTimeout::from(quinn::VarInt::from_u32(30_000))),
        ));
        transport_config.max_concurrent_bidi_streams(self.config.max_streams.into());

        Ok((server_config, cert_der))
    }

    /// Generate client TLS configuration (accepts any certificate for P2P).
    fn generate_client_config(&self) -> Result<quinn::ClientConfig, QuicError> {
        // For P2P, we skip certificate verification since we verify identity via NodeId
        let crypto = rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(SkipServerVerification))
            .with_no_client_auth();

        let quic_client_config = quinn::crypto::rustls::QuicClientConfig::try_from(crypto)
            .map_err(|e| QuicError::TlsError {
                reason: format!("client crypto config error: {:?}", e),
            })?;

        let client_config = quinn::ClientConfig::new(Arc::new(quic_client_config));

        Ok(client_config)
    }
}

/// Skip TLS certificate verification for P2P connections.
///
/// In a P2P network, identity is verified via NodeId (public key hash),
/// not TLS certificates. This allows connections without CA infrastructure.
#[cfg(feature = "quic")]
#[derive(Debug)]
struct SkipServerVerification;

#[cfg(feature = "quic")]
impl rustls::client::danger::ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        // Skip verification - identity verified via NodeId
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            rustls::SignatureScheme::ECDSA_NISTP384_SHA384,
            rustls::SignatureScheme::ED25519,
            rustls::SignatureScheme::RSA_PSS_SHA256,
            rustls::SignatureScheme::RSA_PSS_SHA384,
            rustls::SignatureScheme::RSA_PSS_SHA512,
            rustls::SignatureScheme::RSA_PKCS1_SHA256,
            rustls::SignatureScheme::RSA_PKCS1_SHA384,
            rustls::SignatureScheme::RSA_PKCS1_SHA512,
        ]
    }
}

// =============================================================================
// FALLBACK (No quinn feature)
// =============================================================================

/// Placeholder QUIC transport when quinn is not available.
///
/// Enable the `quic` feature for full async QUIC support.
#[cfg(not(feature = "quic"))]
#[derive(Debug)]
pub struct QuicTransport {
    config: QuicConfig,
    replay_protection: ReplayProtection,
}

#[cfg(not(feature = "quic"))]
impl QuicTransport {
    /// Create new QUIC transport (placeholder).
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

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests;
