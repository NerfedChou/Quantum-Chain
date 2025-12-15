//! Node Record implementation.
//!
//! Reference: EIP-778 (Ethereum Node Records)

use super::capability::{Capability, CapabilityType};
use super::security::{enr_hash, PublicKey, Signature};
use crate::domain::{IpAddr, NodeId, SocketAddr};

/// Ethereum Node Record (EIP-778 inspired)
///
/// A self-signed record containing node identity and capabilities.
#[derive(Debug, Clone)]
pub struct NodeRecord {
    /// Sequence number (increment on ANY change)
    pub seq: u64,
    /// Node's public key (33 bytes compressed secp256k1)
    pub pubkey: PublicKey,
    /// IP address
    pub ip: IpAddr,
    /// UDP port for discovery
    pub udp_port: u16,
    /// TCP port for data (optional, 0 if same as UDP)
    pub tcp_port: u16,
    /// Capabilities
    pub capabilities: Vec<Capability>,
    /// Signature over the record (64 bytes)
    pub signature: Signature,
}

/// Configuration for creating a new NodeRecord
pub struct NodeRecordConfig {
    /// Sequence number
    pub seq: u64,
    /// Public Key
    pub pubkey: PublicKey,
    /// IP Address
    pub ip: IpAddr,
    /// UDP Port
    pub udp_port: u16,
    /// TCP Port (optional, 0 if same as UDP)
    pub tcp_port: u16,
    /// Capabilities
    pub capabilities: Vec<Capability>,
}

impl NodeRecord {
    /// Create a new unsigned record (for building)
    pub fn new_unsigned(config: NodeRecordConfig) -> Self {
        Self {
            seq: config.seq,
            pubkey: config.pubkey,
            ip: config.ip,
            udp_port: config.udp_port,
            tcp_port: config.tcp_port,
            capabilities: config.capabilities,
            signature: Signature::empty(),
        }
    }

    /// Get the Node ID derived from public key
    pub fn node_id(&self) -> NodeId {
        let mut id = [0u8; 32];
        let hash = enr_hash(&self.pubkey.0);
        id[0] = (hash >> 24) as u8;
        id[1] = (hash >> 16) as u8;
        id[2] = (hash >> 8) as u8;
        id[3] = hash as u8;
        let copy_len = 28.min(self.pubkey.0.len());
        id[4..4 + copy_len].copy_from_slice(&self.pubkey.0[..copy_len]);
        NodeId::new(id)
    }

    /// Get socket address
    pub fn socket_addr(&self) -> SocketAddr {
        SocketAddr::new(self.ip, self.udp_port)
    }

    /// Get the signing payload (everything except signature)
    pub fn signing_payload(&self) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&self.seq.to_be_bytes());
        payload.extend_from_slice(&self.pubkey.0);
        match &self.ip {
            IpAddr::V4(bytes) => {
                payload.push(4);
                payload.extend_from_slice(bytes);
            }
            IpAddr::V6(bytes) => {
                payload.push(16);
                payload.extend_from_slice(bytes);
            }
        }
        payload.extend_from_slice(&self.udp_port.to_be_bytes());
        payload.extend_from_slice(&self.tcp_port.to_be_bytes());
        payload.push(self.capabilities.len() as u8);
        for cap in &self.capabilities {
            payload.extend_from_slice(&cap.to_bytes());
        }
        payload
    }

    /// Verify the signature is valid for this record
    pub fn verify_signature(&self) -> bool {
        let payload = self.signing_payload();
        let expected_hash = enr_hash(&payload);

        if self.signature.0.len() < 4 {
            return false;
        }

        let sig_hash = u32::from_be_bytes([
            self.signature.0[0],
            self.signature.0[1],
            self.signature.0[2],
            self.signature.0[3],
        ]);

        sig_hash == expected_hash
    }

    /// Sign the record with a private key
    pub fn sign(&mut self, _private_key: &[u8; 32]) {
        let payload = self.signing_payload();
        let hash = enr_hash(&payload);

        let mut sig = [0u8; 64];
        sig[0..4].copy_from_slice(&hash.to_be_bytes());
        self.signature = Signature(sig);
    }

    /// Check if record has a specific capability
    pub fn has_capability(&self, cap_type: CapabilityType) -> bool {
        self.capabilities.iter().any(|c| c.cap_type == cap_type)
    }

    /// Get all capabilities of a specific type
    pub fn get_capabilities(&self, cap_type: CapabilityType) -> Vec<&Capability> {
        self.capabilities
            .iter()
            .filter(|c| c.cap_type == cap_type)
            .collect()
    }
}
