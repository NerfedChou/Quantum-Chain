/// Subsystem IDs per Architecture.md Section 4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum SubsystemId {
    /// Subsystem 1: Peer Discovery & Routing
    PeerDiscovery = 1,
    /// Subsystem 2: Block Storage Engine
    BlockStorage = 2,
    /// Subsystem 3: Transaction Indexing
    TransactionIndexing = 3,
    /// Subsystem 4: State Management
    StateManagement = 4,
    /// Subsystem 5: Block Propagation
    BlockPropagation = 5,
    /// Subsystem 6: Mempool
    Mempool = 6,
    /// Subsystem 7: Bloom Filters
    BloomFilters = 7,
    /// Subsystem 8: Consensus
    Consensus = 8,
    /// Subsystem 9: Finality
    Finality = 9,
    /// Subsystem 10: Signature Verification
    SignatureVerification = 10,
    /// Subsystem 11: Smart Contracts
    SmartContracts = 11,
    /// Subsystem 12: Transaction Ordering
    TransactionOrdering = 12,
    /// Subsystem 13: Light Clients
    LightClients = 13,
    /// Subsystem 14: Sharding
    Sharding = 14,
    /// Subsystem 15: Cross-Chain
    CrossChain = 15,
}

impl SubsystemId {
    /// Convert from raw u8 value.
    #[must_use]
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::PeerDiscovery),
            2 => Some(Self::BlockStorage),
            3 => Some(Self::TransactionIndexing),
            4 => Some(Self::StateManagement),
            5 => Some(Self::BlockPropagation),
            6 => Some(Self::Mempool),
            7 => Some(Self::BloomFilters),
            8 => Some(Self::Consensus),
            9 => Some(Self::Finality),
            10 => Some(Self::SignatureVerification),
            11 => Some(Self::SmartContracts),
            12 => Some(Self::TransactionOrdering),
            13 => Some(Self::LightClients),
            14 => Some(Self::Sharding),
            15 => Some(Self::CrossChain),
            _ => None,
        }
    }

    /// Get the raw u8 value.
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

impl std::fmt::Display for SubsystemId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PeerDiscovery => write!(f, "PeerDiscovery(1)"),
            Self::BlockStorage => write!(f, "BlockStorage(2)"),
            Self::TransactionIndexing => write!(f, "TransactionIndexing(3)"),
            Self::StateManagement => write!(f, "StateManagement(4)"),
            Self::BlockPropagation => write!(f, "BlockPropagation(5)"),
            Self::Mempool => write!(f, "Mempool(6)"),
            Self::BloomFilters => write!(f, "BloomFilters(7)"),
            Self::Consensus => write!(f, "Consensus(8)"),
            Self::Finality => write!(f, "Finality(9)"),
            Self::SignatureVerification => write!(f, "SignatureVerification(10)"),
            Self::SmartContracts => write!(f, "SmartContracts(11)"),
            Self::TransactionOrdering => write!(f, "TransactionOrdering(12)"),
            Self::LightClients => write!(f, "LightClients(13)"),
            Self::Sharding => write!(f, "Sharding(14)"),
            Self::CrossChain => write!(f, "CrossChain(15)"),
        }
    }
}
