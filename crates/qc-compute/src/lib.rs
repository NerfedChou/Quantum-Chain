//! # QC-Compute: Portable GPU/CPU Compute Abstraction
//!
//! This crate provides a vendor-agnostic compute layer for Quantum-Chain.
//! It automatically selects the best available backend at **runtime**:
//!
//! 1. **OpenCL** - Wide GPU support (NVIDIA, AMD, Intel, Apple)
//! 2. **CPU/Rayon** - Fallback, always works, zero dependencies
//!
//! ## Philosophy: No Vendor Lock-in, No Build Failures
//!
//! - **No CUDA**: Locks you into NVIDIA
//! - **No Vulkan shaders**: Requires shaderc/cmake, breaks CI
//! - **OpenCL**: Compiles anywhere, detects GPU at runtime
//! - **CPU**: Always works, parallel via Rayon
//!
//! ## Subsystem Compute Requirements
//!
//! | Subsystem | Workload Type | Best Backend | Why |
//! |-----------|---------------|--------------|-----|
//! | QC-17 (Mining) | SHA256 hashing | GPU/OpenCL | Embarrassingly parallel |
//! | QC-10 (Signatures) | ECDSA/BLS verify | GPU/OpenCL | Batch verification |
//! | QC-03 (Merkle) | SHA256 tree | GPU/OpenCL | Parallel hashing |
//! | QC-04 (State) | Trie operations | CPU | Memory-bound, branching |
//! | QC-08 (Consensus) | Validation | CPU | Logic-heavy |
//! | QC-06 (Mempool) | Sorting/filtering | CPU | Memory-bound |
//! | QC-02 (Storage) | I/O operations | CPU | Disk-bound |
//!
//! ## Usage
//!
//! ```rust,ignore
//! use qc_compute::auto_detect;
//!
//! // Auto-detect best backend (OpenCL GPU or CPU)
//! let engine = auto_detect()?;
//! println!("Using: {}", engine.backend());
//! ```

pub mod backends;
pub mod tasks;

use primitive_types::U256;
use std::sync::Arc;
use thiserror::Error;

/// Compute backend capabilities
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// CPU with Rayon parallelism
    Cpu,
    /// OpenCL (portable GPU)
    OpenCL,
}

impl std::fmt::Display for Backend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Backend::Cpu => write!(f, "CPU (Rayon)"),
            Backend::OpenCL => write!(f, "OpenCL GPU"),
        }
    }
}

/// Compute engine errors
#[derive(Error, Debug)]
pub enum ComputeError {
    #[error("No compute backend available")]
    NoBackendAvailable,

    #[error("Backend initialization failed: {0}")]
    InitializationFailed(String),

    #[error("Compute task failed: {0}")]
    TaskFailed(String),

    #[error("Timeout waiting for result")]
    Timeout,

    #[error("Invalid input: {0}")]
    InvalidInput(String),
}

/// Device information
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub name: String,
    pub backend: Backend,
    pub compute_units: u32,
    pub memory_bytes: u64,
    pub supports_f64: bool,
}

/// Compute engine trait - implemented by all backends
#[async_trait::async_trait]
pub trait ComputeEngine: Send + Sync {
    /// Get backend type
    fn backend(&self) -> Backend;

    /// Get device info
    fn device_info(&self) -> &DeviceInfo;

    /// Batch SHA256 hashing (for mining, merkle trees)
    async fn batch_sha256(&self, inputs: &[Vec<u8>]) -> Result<Vec<[u8; 32]>, ComputeError>;

    /// PoW mining - find nonce that produces hash below target
    async fn pow_mine(
        &self,
        header_template: &[u8],
        target: U256,
        nonce_start: u64,
        nonce_count: u64,
    ) -> Result<Option<(u64, [u8; 32])>, ComputeError>;

    /// Batch ECDSA signature verification
    async fn batch_verify_ecdsa(
        &self,
        messages: &[[u8; 32]],
        signatures: &[[u8; 65]],
        public_keys: &[[u8; 33]],
    ) -> Result<Vec<bool>, ComputeError>;
}

/// Auto-detect and create the best available compute engine
pub fn auto_detect() -> Result<Arc<dyn ComputeEngine>, ComputeError> {
    // Try backends in order of preference: GPU first, then CPU

    #[cfg(feature = "opencl")]
    {
        match backends::opencl::OpenCLEngine::new() {
            Ok(engine) => {
                tracing::info!("✓ GPU detected: {} (OpenCL)", engine.device_info().name);
                return Ok(Arc::new(engine));
            }
            Err(e) => {
                tracing::debug!("OpenCL not available: {}", e);
            }
        }
    }

    #[cfg(feature = "cpu")]
    {
        let engine = backends::cpu::CpuEngine::new();
        tracing::info!(
            "Using CPU compute: {} cores (Rayon)",
            engine.device_info().compute_units
        );
        return Ok(Arc::new(engine));
    }

    #[cfg(not(feature = "cpu"))]
    {
        Err(ComputeError::NoBackendAvailable)
    }
}

/// Create a specific backend
pub fn create_backend(backend: Backend) -> Result<Arc<dyn ComputeEngine>, ComputeError> {
    match backend {
        Backend::Cpu => {
            #[cfg(feature = "cpu")]
            {
                Ok(Arc::new(backends::cpu::CpuEngine::new()))
            }
            #[cfg(not(feature = "cpu"))]
            {
                Err(ComputeError::NoBackendAvailable)
            }
        }
        Backend::OpenCL => {
            #[cfg(feature = "opencl")]
            {
                backends::opencl::OpenCLEngine::new().map(|e| Arc::new(e) as Arc<dyn ComputeEngine>)
            }
            #[cfg(not(feature = "opencl"))]
            {
                Err(ComputeError::NoBackendAvailable)
            }
        }
    }
}

/// Recommended backend for each subsystem workload
pub fn recommended_backend_for(subsystem: &str) -> Backend {
    match subsystem {
        // GPU-accelerated (embarrassingly parallel)
        "qc-17" | "qc-17-block-production" => Backend::OpenCL, // PoW mining
        "qc-10" | "qc-10-signature-verification" => Backend::OpenCL, // Batch signatures
        "qc-03" | "qc-03-transaction-indexing" => Backend::OpenCL, // Merkle trees

        // CPU-preferred (memory-bound or logic-heavy)
        _ => Backend::Cpu,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recommended_backends() {
        // GPU-accelerated subsystems
        assert_eq!(recommended_backend_for("qc-17"), Backend::OpenCL);
        assert_eq!(recommended_backend_for("qc-10"), Backend::OpenCL);
        assert_eq!(recommended_backend_for("qc-03"), Backend::OpenCL);

        // CPU-preferred subsystems
        assert_eq!(recommended_backend_for("qc-04"), Backend::Cpu);
        assert_eq!(recommended_backend_for("qc-08"), Backend::Cpu);
        assert_eq!(recommended_backend_for("unknown"), Backend::Cpu);
    }
}
