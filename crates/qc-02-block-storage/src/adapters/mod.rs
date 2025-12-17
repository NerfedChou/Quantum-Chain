//! # Adapters Module
//!
//! Contains adapter implementations for the Block Storage subsystem.
//!
//! ## Modules
//!
//! - `api_handler/`: API Gateway integration (requires `ipc` feature)
//! - `lock/`: Database process locking (requires `locking` feature)
//! - `security/`: Subsystem-level security validations

#[cfg(feature = "api")]
pub mod api_handler;

#[cfg(feature = "locking")]
pub mod lock;

pub mod filesystem;
pub mod infra;
pub mod security;
pub mod serializer;
pub mod storage;

#[cfg(feature = "api")]
pub use api_handler::{
    handle_api_query, ApiGatewayHandler, ApiQueryError, Qc02Metrics, RpcPendingAssembly,
};

#[cfg(feature = "locking")]
pub use lock::{DatabaseLock, LockError};

pub use security::{validate_batch_count, validate_block_height, validate_method_name};

// Re-exports for convenience
pub use filesystem::MockFileSystemAdapter;
pub use infra::{DefaultChecksumProvider, SystemTimeSource};
pub use serializer::BincodeBlockSerializer;
pub use storage::{FileBackedKVStore, InMemoryKVStore};
