//! # Subsystem Trait - True Plug-and-Play Architecture
//!
//! Defines the contract that ALL subsystems must implement to participate
//! in the Quantum-Chain event-driven architecture.
//!
//! ## Design Philosophy (EDA + Hexagonal)
//!
//! - **No compile-time coupling**: Subsystems are discovered at runtime
//! - **Event-only communication**: No direct function calls between subsystems
//! - **Graceful degradation**: Missing subsystems don't crash the node
//! - **Hot-swappable**: Subsystems can be replaced without restart (future)
//!
//! ## Example Implementation
//!
//! ```rust,ignore
//! use shared_types::{Subsystem, SubsystemId, SubsystemStatus};
//! use async_trait::async_trait;
//!
//! pub struct MySubsystem { /* ... */ }
//!
//! #[async_trait]
//! impl Subsystem for MySubsystem {
//!     fn id(&self) -> SubsystemId { SubsystemId::MySubsystem }
//!     fn name(&self) -> &'static str { "My Subsystem" }
//!     async fn start(&self) -> Result<(), SubsystemError> { Ok(()) }
//!     async fn stop(&self) -> Result<(), SubsystemError> { Ok(()) }
//!     async fn health_check(&self) -> SubsystemStatus { SubsystemStatus::Healthy }
//! }
//! ```

use crate::entities::SubsystemId;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Error type for subsystem operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubsystemError {
    /// The subsystem that encountered the error.
    pub subsystem_id: SubsystemId,
    /// Error kind.
    pub kind: SubsystemErrorKind,
    /// Human-readable error message.
    pub message: String,
}

impl fmt::Display for SubsystemError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{:?}] {}: {}",
            self.subsystem_id, self.kind, self.message
        )
    }
}

impl std::error::Error for SubsystemError {}

/// Categories of subsystem errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubsystemErrorKind {
    /// Subsystem failed to initialize.
    InitializationFailed,
    /// Subsystem is not available (disabled or missing dependency).
    NotAvailable,
    /// Subsystem encountered a runtime error.
    RuntimeError,
    /// Subsystem failed to shut down gracefully.
    ShutdownFailed,
    /// Subsystem dependency is missing.
    MissingDependency,
    /// Configuration error.
    ConfigurationError,
}

impl fmt::Display for SubsystemErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InitializationFailed => write!(f, "InitializationFailed"),
            Self::NotAvailable => write!(f, "NotAvailable"),
            Self::RuntimeError => write!(f, "RuntimeError"),
            Self::ShutdownFailed => write!(f, "ShutdownFailed"),
            Self::MissingDependency => write!(f, "MissingDependency"),
            Self::ConfigurationError => write!(f, "ConfigurationError"),
        }
    }
}

/// Health status of a subsystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubsystemStatus {
    /// Subsystem is running normally.
    Healthy,
    /// Subsystem is running but degraded (e.g., high latency).
    Degraded,
    /// Subsystem is not running.
    Stopped,
    /// Subsystem encountered an error.
    Error,
    /// Subsystem is starting up.
    Starting,
    /// Subsystem is shutting down.
    ShuttingDown,
    /// Subsystem is disabled in configuration.
    Disabled,
}

/// Metadata about a subsystem for discovery and monitoring.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubsystemInfo {
    /// Unique identifier.
    pub id: SubsystemId,
    /// Human-readable name.
    pub name: String,
    /// Version string.
    pub version: String,
    /// Brief description.
    pub description: String,
    /// Subsystems this depends on (must be started first).
    pub dependencies: Vec<SubsystemId>,
    /// Events this subsystem publishes.
    pub publishes: Vec<String>,
    /// Events this subsystem subscribes to.
    pub subscribes: Vec<String>,
    /// Whether this subsystem is required for basic operation.
    pub required: bool,
}

impl SubsystemInfo {
    /// Create a new subsystem info with required fields.
    pub fn new(id: SubsystemId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            version: "0.1.0".to_string(),
            description: String::new(),
            dependencies: Vec::new(),
            publishes: Vec::new(),
            subscribes: Vec::new(),
            required: false,
        }
    }

    /// Mark as required subsystem.
    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    /// Add dependencies.
    pub fn depends_on(mut self, deps: Vec<SubsystemId>) -> Self {
        self.dependencies = deps;
        self
    }

    /// Set events this subsystem publishes.
    pub fn publishes_events(mut self, events: Vec<&str>) -> Self {
        self.publishes = events.into_iter().map(String::from).collect();
        self
    }

    /// Set events this subsystem subscribes to.
    pub fn subscribes_to(mut self, events: Vec<&str>) -> Self {
        self.subscribes = events.into_iter().map(String::from).collect();
        self
    }
}

/// The core trait that ALL subsystems must implement.
///
/// This enables true plug-and-play architecture where:
/// - Subsystems can be enabled/disabled at runtime
/// - Missing subsystems don't crash the node
/// - Subsystems communicate ONLY through the event bus
#[async_trait]
pub trait Subsystem: Send + Sync {
    /// Get the unique identifier for this subsystem.
    fn id(&self) -> SubsystemId;

    /// Get the human-readable name.
    fn name(&self) -> &'static str;

    /// Get detailed information about this subsystem.
    fn info(&self) -> SubsystemInfo {
        SubsystemInfo::new(self.id(), self.name())
    }

    /// Initialize and start the subsystem.
    ///
    /// Called by the runtime during startup. The subsystem should:
    /// 1. Validate its configuration
    /// 2. Connect to the event bus
    /// 3. Subscribe to relevant events
    /// 4. Start any background tasks
    async fn start(&self) -> Result<(), SubsystemError>;

    /// Stop the subsystem gracefully.
    ///
    /// Called during shutdown. The subsystem should:
    /// 1. Stop accepting new work
    /// 2. Complete in-flight operations (with timeout)
    /// 3. Persist any state
    /// 4. Unsubscribe from events
    async fn stop(&self) -> Result<(), SubsystemError>;

    /// Check the health of the subsystem.
    ///
    /// Called periodically by the runtime for monitoring.
    async fn health_check(&self) -> SubsystemStatus;

    /// Handle a configuration reload.
    ///
    /// Called when the operator triggers a config reload.
    /// Default implementation does nothing.
    async fn reload_config(&self) -> Result<(), SubsystemError> {
        Ok(())
    }

    /// Get current metrics for this subsystem.
    ///
    /// Returns a JSON-serializable object with subsystem-specific metrics.
    fn metrics(&self) -> serde_json::Value {
        serde_json::json!({
            "subsystem_id": self.id().as_u8(),
            "status": "no_metrics"
        })
    }
}

/// A type-erased subsystem handle for the registry.
pub type DynSubsystem = Box<dyn Subsystem>;

/// Factory function type for creating subsystems.
///
/// Used by the registry to lazily instantiate subsystems.
pub type SubsystemFactory = Box<dyn Fn() -> Result<DynSubsystem, SubsystemError> + Send + Sync>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subsystem_info_builder() {
        let info = SubsystemInfo::new(SubsystemId::Consensus, "Consensus")
            .required()
            .depends_on(vec![SubsystemId::SignatureVerification])
            .publishes_events(vec!["BlockValidated", "BlockRejected"])
            .subscribes_to(vec!["BlockProduced"]);

        assert_eq!(info.id, SubsystemId::Consensus);
        assert!(info.required);
        assert_eq!(info.dependencies, vec![SubsystemId::SignatureVerification]);
        assert_eq!(info.publishes.len(), 2);
        assert_eq!(info.subscribes.len(), 1);
    }

    #[test]
    fn test_subsystem_error_display() {
        let err = SubsystemError {
            subsystem_id: SubsystemId::BlockStorage,
            kind: SubsystemErrorKind::InitializationFailed,
            message: "RocksDB not found".to_string(),
        };

        let display = format!("{}", err);
        assert!(display.contains("BlockStorage"));
        assert!(display.contains("InitializationFailed"));
        assert!(display.contains("RocksDB not found"));
    }
}
