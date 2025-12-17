//! Storage Adapters
//! 
//! Implementations of the `KeyValueStore` trait.

mod memory;
mod file;

pub use memory::InMemoryKVStore;
pub use file::FileBackedKVStore;
