use crate::domain::errors::SerializationError;
use crate::domain::storage::StoredBlock;
use crate::ports::outbound::BlockSerializer;

/// Default block serializer using bincode.
#[derive(Default)]
pub struct BincodeBlockSerializer;

impl BlockSerializer for BincodeBlockSerializer {
    fn serialize(&self, block: &StoredBlock) -> Result<Vec<u8>, SerializationError> {
        bincode::serialize(block).map_err(|e| SerializationError {
            message: e.to_string(),
        })
    }

    fn deserialize(&self, data: &[u8]) -> Result<StoredBlock, SerializationError> {
        bincode::deserialize(data).map_err(|e| SerializationError {
            message: e.to_string(),
        })
    }

    fn estimate_size(&self, block: &StoredBlock) -> usize {
        // Rough estimate: header + transactions + overhead
        std::mem::size_of::<StoredBlock>() + block.block.transactions.len() * 256
    }
}
