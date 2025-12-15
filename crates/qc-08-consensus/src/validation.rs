use crate::domain::{Block, BlockHeader, ConsensusError, ConsensusResult, ConsensusConfig};
use crate::ports::TimeSource;
use crate::state::ConsensusState;

/// Stateless validation logic for blocks.
pub struct BlockValidator;

impl BlockValidator {
    /// Validate block structure (size, gas, tx count)
    pub fn validate_structure(block: &Block, config: &ConsensusConfig) -> ConsensusResult<()> {
        // Check transaction count
        if block.transactions.len() > config.max_txs_per_block {
            return Err(ConsensusError::TooManyTransactions {
                count: block.transactions.len(),
                limit: config.max_txs_per_block,
            });
        }

        // Check gas limit
        let total_gas: u64 = block.transactions.iter().map(|tx| tx.gas_cost()).sum();

        if total_gas > config.max_block_gas {
            return Err(ConsensusError::GasLimitExceeded {
                used: total_gas,
                limit: config.max_block_gas,
            });
        }

        // Check header gas used matches transactions
        if block.header.gas_used > block.header.gas_limit {
            return Err(ConsensusError::GasLimitExceeded {
                used: block.header.gas_used,
                limit: block.header.gas_limit,
            });
        }

        // Check extra_data size limit (prevent DoS via oversized blocks)
        // Default limit: 32 bytes (Ethereum standard)
        const MAX_EXTRA_DATA_SIZE: usize = 32;
        if block.header.extra_data.len() > MAX_EXTRA_DATA_SIZE {
            return Err(ConsensusError::ExtraDataTooLarge {
                size: block.header.extra_data.len(),
                limit: MAX_EXTRA_DATA_SIZE,
            });
        }

        Ok(())
    }

    /// Validate parent chain linkage
    pub fn validate_parent(header: &BlockHeader, state: &ConsensusState) -> ConsensusResult<()> {
        let chain = state.chain.read();

        if header.is_genesis() {
            if chain.block_count() > 0 {
                return Err(ConsensusError::GenesisWithParent);
            }
            return Ok(());
        }

        if !chain.has_block(&header.parent_hash) {
            return Err(ConsensusError::UnknownParent(header.parent_hash));
        }

        Ok(())
    }

    /// Validate sequential height
    pub fn validate_height(header: &BlockHeader, state: &ConsensusState) -> ConsensusResult<()> {
        if header.is_genesis() {
            if header.block_height != 0 {
                return Err(ConsensusError::InvalidHeight {
                    expected: 0,
                    actual: header.block_height,
                });
            }
            return Ok(());
        }

        let chain = state.chain.read();
        if let Some(parent) = chain.get_block(&header.parent_hash) {
            let expected = parent.block_height + 1;
            if header.block_height != expected {
                return Err(ConsensusError::InvalidHeight {
                    expected,
                    actual: header.block_height,
                });
            }
        }
        Ok(())
    }

    /// Validate timestamp ordering
    pub fn validate_timestamp(
        header: &BlockHeader, 
        state: &ConsensusState, 
        time_source: &dyn TimeSource,
        config: &ConsensusConfig
    ) -> ConsensusResult<()> {
        let now = time_source.now();

        // Check not too far in future
        if header.timestamp > now + config.max_timestamp_drift_secs {
            return Err(ConsensusError::FutureTimestamp {
                timestamp: header.timestamp,
                current: now,
            });
        }

        if header.is_genesis() {
            return Ok(());
        }

        let chain = state.chain.read();
        if let Some(parent) = chain.get_block(&header.parent_hash) {
            if header.timestamp <= parent.timestamp {
                return Err(ConsensusError::InvalidTimestamp {
                    block: header.timestamp,
                    parent: parent.timestamp,
                });
            }
        }

        Ok(())
    }
}
