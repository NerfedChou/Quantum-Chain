//! Node capability types.

/// Node capability advertisement
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capability {
    /// Type of capability
    pub cap_type: CapabilityType,
    /// Capability-specific data
    pub data: CapabilityData,
}

impl Capability {
    /// Create a new capability
    pub fn new(cap_type: CapabilityType, data: CapabilityData) -> Self {
        Self { cap_type, data }
    }

    /// Create a "full node" capability
    pub fn full_node() -> Self {
        Self::new(CapabilityType::FullNode, CapabilityData::None)
    }

    /// Create a "light server" capability
    pub fn light_server() -> Self {
        Self::new(CapabilityType::LightServer, CapabilityData::None)
    }

    /// Create a "shard" capability
    pub fn shard(shard_id: u16) -> Self {
        Self::new(CapabilityType::Shard, CapabilityData::ShardId(shard_id))
    }

    /// Create a "shard range" capability
    pub fn shard_range(start: u16, end: u16) -> Self {
        Self::new(
            CapabilityType::ShardRange,
            CapabilityData::ShardRange { start, end },
        )
    }

    /// Serialize to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.push(self.cap_type as u8);
        match &self.data {
            CapabilityData::None => {}
            CapabilityData::ShardId(id) => {
                bytes.extend_from_slice(&id.to_be_bytes());
            }
            CapabilityData::ShardRange { start, end } => {
                bytes.extend_from_slice(&start.to_be_bytes());
                bytes.extend_from_slice(&end.to_be_bytes());
            }
            CapabilityData::Custom(data) => {
                bytes.push(data.len() as u8);
                bytes.extend_from_slice(data);
            }
        }
        bytes
    }
}

/// Types of node capabilities
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum CapabilityType {
    /// Full node - stores all data
    FullNode = 1,
    /// Light client server
    LightServer = 2,
    /// Specific shard
    Shard = 3,
    /// Range of shards
    ShardRange = 4,
    /// Archive node
    Archive = 5,
    /// Custom capability
    Custom = 255,
}

/// Capability-specific data
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityData {
    /// No additional data
    None,
    /// Single shard ID
    ShardId(u16),
    /// Range of shards (inclusive)
    ShardRange {
        /// Start of shard range (inclusive).
        start: u16,
        /// End of shard range (inclusive).
        end: u16,
    },
    /// Custom data
    Custom(Vec<u8>),
}
