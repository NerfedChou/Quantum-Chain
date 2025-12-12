//! # Subsystem Registry - True Plug-and-Play Architecture
//!
//! This module implements a runtime-based subsystem registry that allows
//! subsystems to be enabled/disabled WITHOUT recompilation.
//!
//! ## Architectural Principles
//!
//! 1. **EDA (Event-Driven Architecture)**: Subsystems communicate ONLY via Event Bus
//! 2. **DDD (Domain-Driven Design)**: Each subsystem owns its domain logic
//! 3. **Hexagonal Architecture**: Ports define contracts, Adapters implement them
//! 4. **Plug-and-Play**: Subsystems can be added/removed at runtime config level
//!
//! ## How It Works
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                     SubsystemRegistry                          │
//! │                                                                 │
//! │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐       │
//! │  │  qc-01   │  │  qc-02   │  │  qc-03   │  │   ...    │       │
//! │  │ ENABLED  │  │ ENABLED  │  │ DISABLED │  │          │       │
//! │  └────┬─────┘  └────┬─────┘  └──────────┘  └──────────┘       │
//! │       │             │                                          │
//! │       └─────────────┴──────────────┐                          │
//! │                                    ▼                          │
//! │                          ┌─────────────────┐                  │
//! │                          │   Event Bus     │                  │
//! │                          │ (shared-bus)    │                  │
//! │                          └─────────────────┘                  │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Configuration
//!
//! ```toml
//! [subsystems]
//! qc-01-peer-discovery = true
//! qc-02-block-storage = true
//! qc-03-transaction-indexing = true
//! qc-04-state-management = true
//! qc-05-block-propagation = false  # Disabled - no P2P yet
//! qc-06-mempool = true
//! qc-07-bloom-filters = false      # Optional optimization
//! qc-08-consensus = true
//! qc-09-finality = true
//! qc-10-signature-verification = true
//! qc-16-api-gateway = true
//! qc-17-block-production = true
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::RwLock;
use shared_bus::InMemoryEventBus;
use tracing::{info, warn};

/// Subsystem identifier following the QC naming convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SubsystemId {
    /// QC-01: Peer Discovery
    PeerDiscovery = 1,
    /// QC-02: Block Storage
    BlockStorage = 2,
    /// QC-03: Transaction Indexing
    TransactionIndexing = 3,
    /// QC-04: State Management
    StateManagement = 4,
    /// QC-05: Block Propagation
    BlockPropagation = 5,
    /// QC-06: Mempool
    Mempool = 6,
    /// QC-07: Bloom Filters
    BloomFilters = 7,
    /// QC-08: Consensus
    Consensus = 8,
    /// QC-09: Finality
    Finality = 9,
    /// QC-10: Signature Verification
    SignatureVerification = 10,
    /// QC-16: API Gateway
    ApiGateway = 16,
    /// QC-17: Block Production
    BlockProduction = 17,
}

impl SubsystemId {
    /// Get the subsystem name.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::PeerDiscovery => "qc-01-peer-discovery",
            Self::BlockStorage => "qc-02-block-storage",
            Self::TransactionIndexing => "qc-03-transaction-indexing",
            Self::StateManagement => "qc-04-state-management",
            Self::BlockPropagation => "qc-05-block-propagation",
            Self::Mempool => "qc-06-mempool",
            Self::BloomFilters => "qc-07-bloom-filters",
            Self::Consensus => "qc-08-consensus",
            Self::Finality => "qc-09-finality",
            Self::SignatureVerification => "qc-10-signature-verification",
            Self::ApiGateway => "qc-16-api-gateway",
            Self::BlockProduction => "qc-17-block-production",
        }
    }

    /// Get subsystem dependencies.
    /// Returns subsystems that MUST be enabled for this one to work.
    #[must_use]
    pub fn dependencies(&self) -> Vec<SubsystemId> {
        match self {
            // Level 0: No dependencies
            Self::SignatureVerification => vec![],

            // Level 1: Depends on Level 0
            Self::PeerDiscovery => vec![Self::SignatureVerification],
            Self::Mempool => vec![Self::SignatureVerification],

            // Level 2: Depends on Level 0-1
            Self::TransactionIndexing => vec![],
            Self::StateManagement => vec![],
            Self::BlockPropagation => vec![Self::PeerDiscovery],
            Self::BloomFilters => vec![],

            // Level 3: Depends on Level 0-2
            Self::Consensus => vec![Self::SignatureVerification],

            // Level 4: Depends on Level 0-3
            Self::BlockStorage => vec![],
            Self::Finality => vec![Self::BlockStorage, Self::Consensus],

            // Level 5: Optional/External
            Self::ApiGateway => vec![],
            Self::BlockProduction => vec![Self::Consensus],
        }
    }

    /// Check if this subsystem is required for the choreography flow.
    #[must_use]
    pub fn is_core(&self) -> bool {
        matches!(
            self,
            Self::BlockStorage
                | Self::TransactionIndexing
                | Self::StateManagement
                | Self::Consensus
                | Self::Finality
        )
    }

    /// Get all subsystem IDs.
    #[must_use]
    pub fn all() -> Vec<SubsystemId> {
        vec![
            Self::PeerDiscovery,
            Self::BlockStorage,
            Self::TransactionIndexing,
            Self::StateManagement,
            Self::BlockPropagation,
            Self::Mempool,
            Self::BloomFilters,
            Self::Consensus,
            Self::Finality,
            Self::SignatureVerification,
            Self::ApiGateway,
            Self::BlockProduction,
        ]
    }
}

/// Subsystem status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubsystemStatus {
    /// Not registered.
    NotRegistered,
    /// Registered but not started.
    Registered,
    /// Starting up.
    Starting,
    /// Running normally.
    Running,
    /// Stopped gracefully.
    Stopped,
    /// Failed with error.
    Failed,
    /// Disabled by configuration.
    Disabled,
}

/// Trait that all subsystems must implement for plug-and-play.
#[async_trait::async_trait]
pub trait Subsystem: Send + Sync {
    /// Get the subsystem ID.
    fn id(&self) -> SubsystemId;

    /// Get the subsystem name.
    fn name(&self) -> &'static str {
        self.id().name()
    }

    /// Initialize the subsystem.
    async fn init(&self) -> Result<(), SubsystemError>;

    /// Start the subsystem (begins processing events).
    async fn start(&self) -> Result<(), SubsystemError>;

    /// Stop the subsystem gracefully.
    async fn stop(&self) -> Result<(), SubsystemError>;

    /// Get the current status.
    fn status(&self) -> SubsystemStatus;

    /// Health check - returns true if healthy.
    async fn health_check(&self) -> bool {
        self.status() == SubsystemStatus::Running
    }
}

/// Subsystem error type.
#[derive(Debug, Clone)]
pub struct SubsystemError {
    pub subsystem: SubsystemId,
    pub message: String,
}

impl std::fmt::Display for SubsystemError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.subsystem.name(), self.message)
    }
}

impl std::error::Error for SubsystemError {}

/// Configuration for which subsystems are enabled.
#[derive(Debug, Clone)]
pub struct SubsystemConfig {
    /// Map of subsystem ID to enabled status.
    pub enabled: HashMap<SubsystemId, bool>,
}

impl Default for SubsystemConfig {
    fn default() -> Self {
        let mut enabled = HashMap::new();

        // Core subsystems (always enabled by default)
        enabled.insert(SubsystemId::SignatureVerification, true);
        enabled.insert(SubsystemId::BlockStorage, true);
        enabled.insert(SubsystemId::TransactionIndexing, true);
        enabled.insert(SubsystemId::StateManagement, true);
        enabled.insert(SubsystemId::Consensus, true);
        enabled.insert(SubsystemId::Finality, true);
        enabled.insert(SubsystemId::Mempool, true);
        enabled.insert(SubsystemId::BlockProduction, true);

        // Optional subsystems (disabled by default)
        enabled.insert(SubsystemId::PeerDiscovery, true);
        enabled.insert(SubsystemId::BlockPropagation, false); // No P2P yet
        enabled.insert(SubsystemId::BloomFilters, false); // Optional optimization
        enabled.insert(SubsystemId::ApiGateway, true);

        Self { enabled }
    }
}

impl SubsystemConfig {
    /// Check if a subsystem is enabled.
    #[must_use]
    pub fn is_enabled(&self, id: SubsystemId) -> bool {
        self.enabled.get(&id).copied().unwrap_or(false)
    }

    /// Enable a subsystem.
    pub fn enable(&mut self, id: SubsystemId) {
        self.enabled.insert(id, true);
    }

    /// Disable a subsystem.
    pub fn disable(&mut self, id: SubsystemId) {
        self.enabled.insert(id, false);
    }

    /// Validate dependencies are satisfied.
    pub fn validate(&self) -> Result<(), Vec<SubsystemError>> {
        let mut errors = Vec::new();

        for id in SubsystemId::all() {
            if self.is_enabled(id) {
                for dep in id.dependencies() {
                    if !self.is_enabled(dep) {
                        errors.push(SubsystemError {
                            subsystem: id,
                            message: format!("Requires {} but it is disabled", dep.name()),
                        });
                    }
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Load from environment variables.
    pub fn from_env() -> Self {
        let mut config = Self::default();

        // Check for each subsystem
        for id in SubsystemId::all() {
            let env_key = format!(
                "QC_SUBSYSTEM_{}",
                id.name().to_uppercase().replace('-', "_")
            );
            if let Ok(val) = std::env::var(&env_key) {
                let enabled = val == "1" || val.to_lowercase() == "true";
                config.enabled.insert(id, enabled);
            }
        }

        config
    }
}

/// The central subsystem registry.
///
/// This is the plug-and-play heart of the system. Subsystems register
/// themselves here and communicate ONLY through the event bus.
pub struct SubsystemRegistry {
    /// Registered subsystems.
    subsystems: RwLock<HashMap<SubsystemId, Arc<dyn Subsystem>>>,
    /// Subsystem status.
    status: RwLock<HashMap<SubsystemId, SubsystemStatus>>,
    /// Configuration.
    config: SubsystemConfig,
    /// Shared event bus - the ONLY way subsystems communicate.
    event_bus: Arc<InMemoryEventBus>,
}

impl SubsystemRegistry {
    /// Create a new registry with the given configuration.
    pub fn new(config: SubsystemConfig, event_bus: Arc<InMemoryEventBus>) -> Self {
        Self {
            subsystems: RwLock::new(HashMap::new()),
            status: RwLock::new(HashMap::new()),
            config,
            event_bus,
        }
    }

    /// Get the event bus.
    pub fn event_bus(&self) -> Arc<InMemoryEventBus> {
        Arc::clone(&self.event_bus)
    }

    /// Register a subsystem.
    pub fn register(&self, subsystem: Arc<dyn Subsystem>) -> Result<(), SubsystemError> {
        let id = subsystem.id();

        // Check if enabled
        if !self.config.is_enabled(id) {
            info!("[Registry] Skipping disabled subsystem: {}", id.name());
            self.status.write().insert(id, SubsystemStatus::Disabled);
            return Ok(());
        }

        // Check dependencies
        for dep in id.dependencies() {
            if !self.config.is_enabled(dep) {
                return Err(SubsystemError {
                    subsystem: id,
                    message: format!("Dependency {} is disabled", dep.name()),
                });
            }
        }

        info!("[Registry] Registering subsystem: {}", id.name());
        self.subsystems.write().insert(id, subsystem);
        self.status.write().insert(id, SubsystemStatus::Registered);

        Ok(())
    }

    /// Initialize all registered subsystems.
    pub async fn init_all(&self) -> Result<(), Vec<SubsystemError>> {
        let mut errors = Vec::new();
        let subsystems = self.subsystems.read().clone();

        for (id, subsystem) in &subsystems {
            info!("[Registry] Initializing {}", id.name());
            self.status.write().insert(*id, SubsystemStatus::Starting);

            if let Err(e) = subsystem.init().await {
                errors.push(e);
                self.status.write().insert(*id, SubsystemStatus::Failed);
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Start all registered subsystems.
    pub async fn start_all(&self) -> Result<(), Vec<SubsystemError>> {
        let mut errors = Vec::new();
        let subsystems = self.subsystems.read().clone();

        for (id, subsystem) in &subsystems {
            if self.status.read().get(id) == Some(&SubsystemStatus::Failed) {
                warn!("[Registry] Skipping failed subsystem: {}", id.name());
                continue;
            }

            info!("[Registry] Starting {}", id.name());

            if let Err(e) = subsystem.start().await {
                errors.push(e);
                self.status.write().insert(*id, SubsystemStatus::Failed);
            } else {
                self.status.write().insert(*id, SubsystemStatus::Running);
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Stop all registered subsystems.
    pub async fn stop_all(&self) -> Result<(), Vec<SubsystemError>> {
        let mut errors = Vec::new();
        let subsystems = self.subsystems.read().clone();

        for (id, subsystem) in &subsystems {
            info!("[Registry] Stopping {}", id.name());

            if let Err(e) = subsystem.stop().await {
                errors.push(e);
            } else {
                self.status.write().insert(*id, SubsystemStatus::Stopped);
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Get status of a subsystem.
    pub fn get_status(&self, id: SubsystemId) -> SubsystemStatus {
        self.status
            .read()
            .get(&id)
            .copied()
            .unwrap_or(SubsystemStatus::NotRegistered)
    }

    /// Get all statuses.
    pub fn get_all_status(&self) -> HashMap<SubsystemId, SubsystemStatus> {
        self.status.read().clone()
    }

    /// Check if all core subsystems are running.
    pub fn is_healthy(&self) -> bool {
        let status = self.status.read();

        for id in SubsystemId::all() {
            if id.is_core()
                && self.config.is_enabled(id)
                && status.get(&id) != Some(&SubsystemStatus::Running)
            {
                return false;
            }
        }

        true
    }

    /// Get a subsystem by ID.
    pub fn get(&self, id: SubsystemId) -> Option<Arc<dyn Subsystem>> {
        self.subsystems.read().get(&id).cloned()
    }

    /// Print registry status.
    pub fn print_status(&self) {
        info!("===========================================");
        info!("  SUBSYSTEM REGISTRY STATUS");
        info!("===========================================");

        let status = self.status.read();

        for id in SubsystemId::all() {
            let state = status.get(&id).unwrap_or(&SubsystemStatus::NotRegistered);
            let icon = match state {
                SubsystemStatus::Running => "✅",
                SubsystemStatus::Disabled => "⏸️ ",
                SubsystemStatus::Failed => "❌",
                SubsystemStatus::Stopped => "⏹️ ",
                _ => "⏳",
            };

            let core_marker = if id.is_core() { " [CORE]" } else { "" };
            info!("  {} {:30} {:?}{}", icon, id.name(), state, core_marker);
        }

        info!("===========================================");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = SubsystemConfig::default();

        // Core subsystems should be enabled
        assert!(config.is_enabled(SubsystemId::BlockStorage));
        assert!(config.is_enabled(SubsystemId::Consensus));
        assert!(config.is_enabled(SubsystemId::Finality));

        // Optional should be disabled
        assert!(!config.is_enabled(SubsystemId::BloomFilters));
    }

    #[test]
    fn test_dependency_validation() {
        let mut config = SubsystemConfig::default();

        // Enable finality but disable its dependency
        config.enable(SubsystemId::Finality);
        config.disable(SubsystemId::BlockStorage);

        let result = config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_subsystem_dependencies() {
        // Finality depends on BlockStorage and Consensus
        let deps = SubsystemId::Finality.dependencies();
        assert!(deps.contains(&SubsystemId::BlockStorage));
        assert!(deps.contains(&SubsystemId::Consensus));
    }
}
