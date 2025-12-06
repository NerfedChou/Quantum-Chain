//! Compute backends
//!
//! All backends compile cleanly - no heavy build-time dependencies.
//! GPU backends detect hardware at runtime and gracefully fall back to CPU.

#[cfg(feature = "cpu")]
pub mod cpu;

#[cfg(feature = "opencl")]
pub mod opencl;

// NOTE: Vulkan backend removed - vulkano-shaders requires shaderc/cmake
// which breaks compilation on systems without these tools.
// Use OpenCL for GPU acceleration instead (more portable anyway)
