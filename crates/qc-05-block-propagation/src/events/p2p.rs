//! P2P message types for Block Propagation subsystem.

use crate::domain::ShortTxId;
use shared_types::Hash;

/// P2P network message types for block propagation.
#[derive(Clone, Debug)]
pub enum BlockPropagationMessage {
    /// Block announcement (header-first)
    Announce(BlockAnnouncementMsg),
    /// Compact block
    CompactBlock(CompactBlockMsg),
    /// Full block request
    GetBlock(GetBlockMsg),
    /// Full block response
    Block(BlockMsg),
    /// Request missing transactions for compact block
    GetBlockTxn(GetBlockTxnMsg),
    /// Missing transactions response
    BlockTxn(BlockTxnMsg),
}

#[derive(Clone, Debug)]
pub struct BlockAnnouncementMsg {
    pub block_hash: Hash,
    pub block_height: u64,
    pub parent_hash: Hash,
    pub timestamp: u64,
    pub difficulty: u128,
}

#[derive(Clone, Debug)]
pub struct CompactBlockMsg {
    pub header_hash: Hash,
    pub block_height: u64,
    pub parent_hash: Hash,
    pub timestamp: u64,
    pub short_txids: Vec<ShortTxId>,
    pub prefilled_txs: Vec<PrefilledTxMsg>,
    pub nonce: u64,
}

#[derive(Clone, Debug)]
pub struct PrefilledTxMsg {
    pub index: u16,
    pub tx_data: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct GetBlockMsg {
    pub block_hash: Hash,
    pub request_id: u64,
}

#[derive(Clone, Debug)]
pub struct BlockMsg {
    pub request_id: u64,
    pub block_data: Option<Vec<u8>>,
}

#[derive(Clone, Debug)]
pub struct GetBlockTxnMsg {
    pub block_hash: Hash,
    pub indices: Vec<u16>,
}

#[derive(Clone, Debug)]
pub struct BlockTxnMsg {
    pub block_hash: Hash,
    pub transactions: Vec<Vec<u8>>,
}
