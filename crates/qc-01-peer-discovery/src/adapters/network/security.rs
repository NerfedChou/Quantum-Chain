use crate::domain::NodeId;
use crate::ports::NodeIdValidator;

// ============================================================================
// NoOpNodeIdValidator - Accepts all NodeIds (for testing)
// ============================================================================

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

#[cfg(feature = "ipc")]
mod parser {
    use crate::adapters::network::MessageType;
    use crate::ports::NetworkError;

    /// Parse a raw Bootstrap message.
    ///
    /// # Protocol
    /// \[Type(1)\]\[NodeId(32)\]\[PoW(32)\]\[PubKey(33)\]\[Sig(64)\]\[Port(2)\]\[IP(4/16)\]
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

        let mut node_id_bytes = [0u8; 32];
        node_id_bytes.copy_from_slice(&data[1..33]);
        let node_id = crate::domain::NodeId::new(node_id_bytes);

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
            crate::ipc::BootstrapRequestConfig {
                node_id: node_id.0, // Unwrap raw bytes.
                ip_address,
                port,
                proof_of_work,
                claimed_pubkey,
                signature,
            },
        ))
    }
}

#[cfg(feature = "ipc")]
pub use parser::parse_bootstrap_request;
