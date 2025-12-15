use parking_lot::RwLock;
use crate::domain::{BlockHeader, ChainState};

/// Encapsulates the mutable state of the Consensus Service.
/// This includes the blockchain state (chain head, blocks) and the consensus view state.
pub struct ConsensusState {
    pub chain: RwLock<ChainState>,
    pub current_view: RwLock<u64>,
}

impl ConsensusState {
    pub fn new() -> Self {
        Self {
            chain: RwLock::new(ChainState::new()),
            current_view: RwLock::new(0),
        }
    }

    pub fn with_genesis(genesis: BlockHeader) -> Self {
        Self {
            chain: RwLock::new(ChainState::with_genesis(genesis)),
            current_view: RwLock::new(0),
        }
    }

    pub fn current_view(&self) -> u64 {
        *self.current_view.read()
    }

    pub fn set_view(&self, view: u64) {
        *self.current_view.write() = view;
    }
}
