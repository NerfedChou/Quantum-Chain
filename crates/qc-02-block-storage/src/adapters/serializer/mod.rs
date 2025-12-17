//! Serializer Adapters
//! 
//! Implementations of the `BlockSerializer` trait.

mod bincode;

pub use self::bincode::BincodeBlockSerializer;
