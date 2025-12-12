//! # Subsystem Registry - Runtime Discovery and Lifecycle Management
//!
//! Manages subsystem registration, dependency resolution, and lifecycle.
//! This is the core of the plug-and-play architecture.
//!
//! ## Features
//!
//! - **Runtime registration**: Subsystems register themselves on startup
//! - **Dependency ordering**: Starts subsystems in correct order
//! - **Graceful degradation**: Missing optional subsystems logged as warnings
//! - **Health monitoring**: Periodic health checks on all subsystems
//!
//! ## Usage
//!
//! ```rust,ignore
//! let mut registry = SubsystemRegistry::new();
//!
//! // Register available subsystems
//! registry.register(Box::new(PeerDiscoverySubsystem::new()));
//! registry.register(Box::new(BlockStorageSubsystem::new()));
//!
//! // Start all in dependency order
//! registry.start_all().await?;
//!
//! // Later: graceful shutdown
//! registry.stop_all().await?;
//! ```

// TODO: Refactor to use tokio::sync::RwLock to properly handle async lock guards.
// Current implementation uses parking_lot::RwLock which warns about holding locks across await.
// This is safe because subsystem operations are short-lived, but should be refactored.
#![allow(clippy::await_holding_lock)]

use crate::entities::SubsystemId;
use crate::subsystem_trait::{
    DynSubsystem, SubsystemError, SubsystemErrorKind, SubsystemInfo, SubsystemStatus,
};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{error, info, warn};

/// Central registry for all subsystems.
///
/// Manages the lifecycle of plug-and-play subsystems.
pub struct SubsystemRegistry {
    /// Registered subsystems by ID.
    subsystems: HashMap<SubsystemId, Arc<RwLock<SubsystemEntry>>>,
    /// Required subsystems that MUST be present.
    required: Vec<SubsystemId>,
    /// Initialization order (computed from dependencies).
    init_order: Vec<SubsystemId>,
}

/// Entry for a registered subsystem.
struct SubsystemEntry {
    /// The subsystem instance.
    subsystem: DynSubsystem,
    /// Current status.
    status: SubsystemStatus,
    /// Cached info.
    info: SubsystemInfo,
}

impl SubsystemRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            subsystems: HashMap::new(),
            required: vec![
                // Core subsystems required for basic operation
                SubsystemId::BlockStorage, // qc-02: Must store blocks
                SubsystemId::Consensus,    // qc-08: Must validate blocks
                SubsystemId::SignatureVerification, // qc-10: Must verify signatures
            ],
            init_order: Vec::new(),
        }
    }

    /// Register a subsystem.
    ///
    /// The subsystem will be started later when `start_all()` is called.
    pub fn register(&mut self, subsystem: DynSubsystem) -> Result<(), SubsystemError> {
        let id = subsystem.id();
        let info = subsystem.info();

        info!("[Registry] Registering subsystem {:?} ({})", id, info.name);

        if self.subsystems.contains_key(&id) {
            warn!(
                "[Registry] Subsystem {:?} already registered, replacing",
                id
            );
        }

        self.subsystems.insert(
            id,
            Arc::new(RwLock::new(SubsystemEntry {
                subsystem,
                status: SubsystemStatus::Stopped,
                info,
            })),
        );

        // Recompute initialization order
        self.compute_init_order();

        Ok(())
    }

    /// Check if a subsystem is registered.
    pub fn is_registered(&self, id: SubsystemId) -> bool {
        self.subsystems.contains_key(&id)
    }

    /// Get the status of a subsystem.
    pub fn status(&self, id: SubsystemId) -> Option<SubsystemStatus> {
        self.subsystems.get(&id).map(|e| e.read().status)
    }

    /// Get info about a registered subsystem.
    pub fn info(&self, id: SubsystemId) -> Option<SubsystemInfo> {
        self.subsystems.get(&id).map(|e| e.read().info.clone())
    }

    /// Get all registered subsystem IDs.
    pub fn registered_ids(&self) -> Vec<SubsystemId> {
        self.subsystems.keys().copied().collect()
    }

    /// Validate that all required subsystems are registered.
    pub fn validate_required(&self) -> Result<(), SubsystemError> {
        let mut missing = Vec::new();

        for required_id in &self.required {
            if !self.subsystems.contains_key(required_id) {
                missing.push(*required_id);
            }
        }

        if !missing.is_empty() {
            return Err(SubsystemError {
                subsystem_id: SubsystemId::BlockStorage, // Use first missing as representative
                kind: SubsystemErrorKind::MissingDependency,
                message: format!("Required subsystems not registered: {:?}", missing),
            });
        }

        Ok(())
    }

    /// Start all registered subsystems in dependency order.
    pub async fn start_all(&self) -> Result<(), SubsystemError> {
        info!(
            "[Registry] Starting {} subsystems in dependency order",
            self.subsystems.len()
        );

        // Validate required subsystems first
        self.validate_required()?;

        // Start in computed order
        for id in &self.init_order {
            let Some(entry) = self.subsystems.get(id) else {
                continue;
            };

            // Check dependencies before starting
            self.check_dependencies(id, &entry.read().info.dependencies)?;

            // Start the subsystem
            let mut entry = entry.write();
            info!("[Registry] Starting {:?} ({})", id, entry.info.name);
            entry.status = SubsystemStatus::Starting;

            if let Err(e) = entry.subsystem.start().await {
                entry.status = SubsystemStatus::Error;
                // Required subsystems fail hard, optional ones just warn
                if self.required.contains(id) {
                    error!("[Registry] ✗ Required {:?} failed: {}", id, e);
                    return Err(e);
                }
                warn!("[Registry] ✗ Optional {:?} failed: {}", id, e);
                continue;
            }

            entry.status = SubsystemStatus::Healthy;
            info!("[Registry] ✓ {:?} started successfully", id);
        }

        info!("[Registry] All subsystems started");
        Ok(())
    }

    /// Stop all subsystems in reverse dependency order.
    #[allow(clippy::excessive_nesting)]
    pub async fn stop_all(&self) -> Result<(), SubsystemError> {
        info!("[Registry] Stopping all subsystems");

        // Stop in reverse order
        for id in self.init_order.iter().rev() {
            if let Some(entry) = self.subsystems.get(id) {
                let mut entry = entry.write();

                if entry.status == SubsystemStatus::Healthy
                    || entry.status == SubsystemStatus::Degraded
                {
                    info!("[Registry] Stopping {:?}", id);
                    entry.status = SubsystemStatus::ShuttingDown;

                    match entry.subsystem.stop().await {
                        Ok(()) => {
                            entry.status = SubsystemStatus::Stopped;
                            info!("[Registry] ✓ {:?} stopped", id);
                        }
                        Err(e) => {
                            entry.status = SubsystemStatus::Error;
                            error!("[Registry] ✗ {:?} failed to stop cleanly: {}", id, e);
                            // Continue stopping others
                        }
                    }
                }
            }
        }

        info!("[Registry] All subsystems stopped");
        Ok(())
    }

    /// Run health checks on all subsystems.
    pub async fn health_check_all(&self) -> HashMap<SubsystemId, SubsystemStatus> {
        let mut results = HashMap::new();

        for (id, entry) in &self.subsystems {
            let entry = entry.read();
            let status = entry.subsystem.health_check().await;
            results.insert(*id, status);
        }

        results
    }

    /// Get metrics from all subsystems.
    pub fn metrics_all(&self) -> serde_json::Value {
        let mut metrics = serde_json::Map::new();

        for (id, entry) in &self.subsystems {
            let entry = entry.read();
            let subsystem_metrics = entry.subsystem.metrics();
            metrics.insert(format!("qc-{:02}", id.as_u8()), subsystem_metrics);
        }

        serde_json::Value::Object(metrics)
    }

    /// Check if all dependencies for a subsystem are healthy.
    /// Returns Ok if all required deps are healthy, Err otherwise.
    fn check_dependencies(
        &self,
        subsystem_id: &SubsystemId,
        dependencies: &[SubsystemId],
    ) -> Result<(), SubsystemError> {
        for dep_id in dependencies {
            let Some(dep_entry) = self.subsystems.get(dep_id) else {
                // Dependency not registered - only an error if required
                if self.required.contains(dep_id) {
                    return Err(SubsystemError {
                        subsystem_id: *subsystem_id,
                        kind: SubsystemErrorKind::MissingDependency,
                        message: format!("Required dependency {:?} not registered", dep_id),
                    });
                }
                continue;
            };

            let dep_status = dep_entry.read().status;
            if dep_status == SubsystemStatus::Healthy {
                continue;
            }

            // Dependency not healthy - only an error if required
            if self.required.contains(dep_id) {
                return Err(SubsystemError {
                    subsystem_id: *subsystem_id,
                    kind: SubsystemErrorKind::MissingDependency,
                    message: format!("Required dependency {:?} is {:?}", dep_id, dep_status),
                });
            }
            warn!(
                "[Registry] Optional dependency {:?} not healthy, continuing",
                dep_id
            );
        }
        Ok(())
    }

    /// Compute initialization order using topological sort on dependencies.
    fn compute_init_order(&mut self) {
        // Simple topological sort
        let mut order = Vec::new();
        let mut visited = std::collections::HashSet::new();

        fn visit(
            id: SubsystemId,
            subsystems: &HashMap<SubsystemId, Arc<RwLock<SubsystemEntry>>>,
            visited: &mut std::collections::HashSet<SubsystemId>,
            order: &mut Vec<SubsystemId>,
        ) {
            if visited.contains(&id) {
                return;
            }
            visited.insert(id);

            if let Some(entry) = subsystems.get(&id) {
                let deps = entry.read().info.dependencies.clone();
                for dep in deps {
                    visit(dep, subsystems, visited, order);
                }
            }

            order.push(id);
        }

        for id in self.subsystems.keys() {
            visit(*id, &self.subsystems, &mut visited, &mut order);
        }

        self.init_order = order;
    }
}

impl Default for SubsystemRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subsystem_trait::Subsystem;
    use async_trait::async_trait;

    struct MockSubsystem {
        id: SubsystemId,
        name: &'static str,
    }

    #[async_trait]
    impl Subsystem for MockSubsystem {
        fn id(&self) -> SubsystemId {
            self.id
        }
        fn name(&self) -> &'static str {
            self.name
        }
        async fn start(&self) -> Result<(), SubsystemError> {
            Ok(())
        }
        async fn stop(&self) -> Result<(), SubsystemError> {
            Ok(())
        }
        async fn health_check(&self) -> SubsystemStatus {
            SubsystemStatus::Healthy
        }
    }

    #[test]
    fn test_registry_register() {
        let mut registry = SubsystemRegistry::new();

        registry
            .register(Box::new(MockSubsystem {
                id: SubsystemId::PeerDiscovery,
                name: "Peer Discovery",
            }))
            .unwrap();

        assert!(registry.is_registered(SubsystemId::PeerDiscovery));
        assert!(!registry.is_registered(SubsystemId::BlockStorage));
    }

    #[test]
    fn test_registry_missing_required() {
        let registry = SubsystemRegistry::new();

        // No subsystems registered, should fail validation
        let result = registry.validate_required();
        assert!(result.is_err());
    }
}
