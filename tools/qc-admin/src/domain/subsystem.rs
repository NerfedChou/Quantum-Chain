//! Subsystem domain models.

use serde::{Deserialize, Serialize};

/// Unique identifier for each subsystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SubsystemId {
    /// qc-01: Peer Discovery & Routing
    PeerDiscovery,
    /// qc-02: Block Storage Engine
    BlockStorage,
    /// qc-03: Transaction Indexing
    TransactionIndexing,
    /// qc-04: State Management
    StateManagement,
    /// qc-05: Block Propagation
    BlockPropagation,
    /// qc-06: Mempool
    Mempool,
    /// qc-07: Bloom Filters (NOT IMPLEMENTED)
    BloomFilters,
    /// qc-08: Consensus
    Consensus,
    /// qc-09: Finality
    Finality,
    /// qc-10: Signature Verification
    SignatureVerification,
    /// qc-11: Smart Contracts (NOT IMPLEMENTED)
    SmartContracts,
    /// qc-12: Transaction Ordering (NOT IMPLEMENTED)
    TransactionOrdering,
    /// qc-13: Light Client Sync (NOT IMPLEMENTED)
    LightClientSync,
    /// qc-14: Sharding (NOT IMPLEMENTED)
    Sharding,
    /// qc-15: Cross-Chain (NOT IMPLEMENTED)
    CrossChain,
    /// qc-16: API Gateway
    ApiGateway,
}

impl SubsystemId {
    /// All subsystem IDs in order.
    pub const ALL: [SubsystemId; 16] = [
        SubsystemId::PeerDiscovery,
        SubsystemId::BlockStorage,
        SubsystemId::TransactionIndexing,
        SubsystemId::StateManagement,
        SubsystemId::BlockPropagation,
        SubsystemId::Mempool,
        SubsystemId::BloomFilters,
        SubsystemId::Consensus,
        SubsystemId::Finality,
        SubsystemId::SignatureVerification,
        SubsystemId::SmartContracts,
        SubsystemId::TransactionOrdering,
        SubsystemId::LightClientSync,
        SubsystemId::Sharding,
        SubsystemId::CrossChain,
        SubsystemId::ApiGateway,
    ];

    /// Get the numeric ID (1-16).
    pub fn number(&self) -> u8 {
        match self {
            SubsystemId::PeerDiscovery => 1,
            SubsystemId::BlockStorage => 2,
            SubsystemId::TransactionIndexing => 3,
            SubsystemId::StateManagement => 4,
            SubsystemId::BlockPropagation => 5,
            SubsystemId::Mempool => 6,
            SubsystemId::BloomFilters => 7,
            SubsystemId::Consensus => 8,
            SubsystemId::Finality => 9,
            SubsystemId::SignatureVerification => 10,
            SubsystemId::SmartContracts => 11,
            SubsystemId::TransactionOrdering => 12,
            SubsystemId::LightClientSync => 13,
            SubsystemId::Sharding => 14,
            SubsystemId::CrossChain => 15,
            SubsystemId::ApiGateway => 16,
        }
    }

    /// Get the short code (qc-01, qc-02, etc.).
    pub fn code(&self) -> &'static str {
        match self {
            SubsystemId::PeerDiscovery => "qc-01",
            SubsystemId::BlockStorage => "qc-02",
            SubsystemId::TransactionIndexing => "qc-03",
            SubsystemId::StateManagement => "qc-04",
            SubsystemId::BlockPropagation => "qc-05",
            SubsystemId::Mempool => "qc-06",
            SubsystemId::BloomFilters => "qc-07",
            SubsystemId::Consensus => "qc-08",
            SubsystemId::Finality => "qc-09",
            SubsystemId::SignatureVerification => "qc-10",
            SubsystemId::SmartContracts => "qc-11",
            SubsystemId::TransactionOrdering => "qc-12",
            SubsystemId::LightClientSync => "qc-13",
            SubsystemId::Sharding => "qc-14",
            SubsystemId::CrossChain => "qc-15",
            SubsystemId::ApiGateway => "qc-16",
        }
    }

    /// Get the display name.
    pub fn name(&self) -> &'static str {
        match self {
            SubsystemId::PeerDiscovery => "Peer Discovery",
            SubsystemId::BlockStorage => "Block Storage",
            SubsystemId::TransactionIndexing => "Transaction Indexing",
            SubsystemId::StateManagement => "State Management",
            SubsystemId::BlockPropagation => "Block Propagation",
            SubsystemId::Mempool => "Mempool",
            SubsystemId::BloomFilters => "Bloom Filters",
            SubsystemId::Consensus => "Consensus",
            SubsystemId::Finality => "Finality",
            SubsystemId::SignatureVerification => "Signature Verification",
            SubsystemId::SmartContracts => "Smart Contracts",
            SubsystemId::TransactionOrdering => "Transaction Ordering",
            SubsystemId::LightClientSync => "Light Client Sync",
            SubsystemId::Sharding => "Sharding",
            SubsystemId::CrossChain => "Cross-Chain",
            SubsystemId::ApiGateway => "API Gateway",
        }
    }

    /// Check if this subsystem is implemented.
    pub fn is_implemented(&self) -> bool {
        !matches!(
            self,
            SubsystemId::BloomFilters
                | SubsystemId::SmartContracts
                | SubsystemId::TransactionOrdering
                | SubsystemId::LightClientSync
                | SubsystemId::Sharding
                | SubsystemId::CrossChain
        )
    }

    /// Get the keyboard shortcut for this subsystem.
    pub fn hotkey(&self) -> Option<char> {
        match self.number() {
            1..=9 => Some((b'0' + self.number()) as char),
            10 => Some('0'),
            16 => Some('g'),
            _ => None,
        }
    }

    /// Get subsystem by hotkey.
    pub fn from_hotkey(key: char) -> Option<SubsystemId> {
        match key {
            '1' => Some(SubsystemId::PeerDiscovery),
            '2' => Some(SubsystemId::BlockStorage),
            '3' => Some(SubsystemId::TransactionIndexing),
            '4' => Some(SubsystemId::StateManagement),
            '5' => Some(SubsystemId::BlockPropagation),
            '6' => Some(SubsystemId::Mempool),
            '7' => Some(SubsystemId::BloomFilters),
            '8' => Some(SubsystemId::Consensus),
            '9' => Some(SubsystemId::Finality),
            '0' => Some(SubsystemId::SignatureVerification),
            'g' | 'G' => Some(SubsystemId::ApiGateway),
            _ => None,
        }
    }
}

/// Status of a subsystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SubsystemStatus {
    /// Subsystem is running and healthy.
    Running,
    /// Subsystem is running but has a warning (e.g., dependency down).
    Warning,
    /// Subsystem is stopped or not responding.
    Stopped,
    /// Subsystem is not implemented.
    #[default]
    NotImplemented,
}

impl SubsystemStatus {
    /// Get the indicator character.
    pub fn indicator(&self) -> char {
        match self {
            SubsystemStatus::Running => '●',
            SubsystemStatus::Warning => '●',
            SubsystemStatus::Stopped => '●',
            SubsystemStatus::NotImplemented => '○',
        }
    }

    /// Get the short label.
    pub fn label(&self) -> &'static str {
        match self {
            SubsystemStatus::Running => "RUN",
            SubsystemStatus::Warning => "WARN",
            SubsystemStatus::Stopped => "STOP",
            SubsystemStatus::NotImplemented => "N/I",
        }
    }
}

/// Information about a subsystem's current state.
#[derive(Debug, Clone, Default)]
pub struct SubsystemInfo {
    pub id: SubsystemId,
    pub status: SubsystemStatus,
    /// Optional warning message if status is Warning.
    pub warning_message: Option<String>,
    /// Subsystem-specific metrics as JSON value.
    pub metrics: Option<serde_json::Value>,
}

impl Default for SubsystemId {
    fn default() -> Self {
        SubsystemId::PeerDiscovery
    }
}

/// Overall system health metrics.
#[derive(Debug, Clone, Default)]
pub struct SystemHealth {
    /// CPU usage as percentage (0-100).
    pub cpu_percent: f32,
    /// Memory usage as percentage (0-100).
    pub memory_percent: f32,
    /// Overall node status.
    pub node_status: NodeStatus,
}

/// Overall node status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NodeStatus {
    /// Node is running normally.
    #[default]
    Running,
    /// Node has warnings.
    Warning,
    /// Node is stopped.
    Stopped,
}

impl NodeStatus {
    pub fn label(&self) -> &'static str {
        match self {
            NodeStatus::Running => "RUNNING",
            NodeStatus::Warning => "WARNING",
            NodeStatus::Stopped => "STOPPED",
        }
    }
}
