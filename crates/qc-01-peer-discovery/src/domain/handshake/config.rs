//! Handshake configuration.

/// Configuration for handshake verification
#[derive(Debug, Clone)]
pub struct HandshakeConfig {
    /// Minimum supported protocol version
    pub min_protocol_version: u16,
    /// Maximum protocol version
    pub max_protocol_version: u16,
    /// Height of our last finalized block (can't sync below this)
    pub finalized_height: u64,
    /// Hash of our last finalized block
    pub finalized_hash: [u8; 32],
    /// Maximum block height difference for "useless" peer
    pub max_behind_blocks: u64,
}

impl Default for HandshakeConfig {
    fn default() -> Self {
        Self {
            min_protocol_version: 1,
            max_protocol_version: 1,
            finalized_height: 0,
            finalized_hash: [0u8; 32],
            max_behind_blocks: 1000,
        }
    }
}

impl HandshakeConfig {
    /// Testing config
    #[cfg(test)]
    pub fn for_testing() -> Self {
        Self {
            min_protocol_version: 1,
            max_protocol_version: 1,
            finalized_height: 100,
            finalized_hash: [0u8; 32],
            max_behind_blocks: 50,
        }
    }
}
