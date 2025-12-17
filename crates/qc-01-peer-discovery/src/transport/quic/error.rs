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
