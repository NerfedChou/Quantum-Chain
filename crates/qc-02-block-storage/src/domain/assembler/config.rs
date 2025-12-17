//! # Assembler Configuration
//!
//! Configuration for the block assembly buffer.
//!
//! ## SPEC-02 Section 2.4
//!
//! - `assembly_timeout_secs`: Maximum time to wait for all components
//! - `max_pending_assemblies`: Maximum buffer size for memory safety

/// Configuration for the assembly buffer.
///
/// ## SPEC-02 Section 2.4
///
/// - `assembly_timeout_secs`: Maximum time to wait for all components (default: 30s)
/// - `max_pending_assemblies`: Maximum buffer size to prevent memory exhaustion (default: 1000)
#[derive(Debug, Clone)]
pub struct AssemblyConfig {
    /// Maximum time to wait for all components before purging (default: 30 seconds).
    ///
    /// SECURITY (INVARIANT-7): This prevents memory exhaustion from orphaned partial blocks.
    pub assembly_timeout_secs: u64,

    /// Maximum number of pending assemblies (default: 1000).
    ///
    /// SECURITY (INVARIANT-8): Bounds memory usage. If exceeded, oldest entries are purged.
    pub max_pending_assemblies: usize,
}

impl Default for AssemblyConfig {
    fn default() -> Self {
        Self {
            assembly_timeout_secs: 30,
            max_pending_assemblies: 1000,
        }
    }
}

impl AssemblyConfig {
    /// Create a new configuration with custom values.
    pub fn new(assembly_timeout_secs: u64, max_pending_assemblies: usize) -> Self {
        Self {
            assembly_timeout_secs,
            max_pending_assemblies,
        }
    }

    /// Validate configuration values.
    ///
    /// Returns `true` if all values are within acceptable bounds.
    pub fn is_valid(&self) -> bool {
        self.assembly_timeout_secs > 0
            && self.assembly_timeout_secs <= 300 // Max 5 minutes
            && self.max_pending_assemblies > 0
            && self.max_pending_assemblies <= 10_000 // Max 10k
    }
}
