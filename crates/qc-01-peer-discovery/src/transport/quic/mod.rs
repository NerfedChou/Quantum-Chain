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

mod config;
mod connection;
mod error;
mod replay;
#[cfg(feature = "quic")]
mod verifier;

pub use config::QuicConfig;
pub use connection::QuicConnectionState;
pub use error::QuicError;
pub use replay::ReplayProtection;
#[cfg(feature = "quic")]
use verifier::SkipServerVerification;

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests;

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
    #[allow(dead_code)] // Used in future for cert rotation
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
