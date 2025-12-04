//! RPC method handlers for JSON-RPC API.

pub mod admin;
pub mod debug;
pub mod eth;
pub mod net;
pub mod txpool;
pub mod web3;

pub use admin::AdminRpc;
pub use debug::DebugRpc;
pub use eth::EthRpc;
pub use net::NetRpc;
pub use txpool::TxPoolRpc;
pub use web3::Web3Rpc;

use crate::domain::config::GatewayConfig;
use crate::ipc::handler::IpcHandler;
use std::path::PathBuf;
use std::sync::Arc;

/// All RPC handlers
pub struct RpcHandlers {
    pub eth: EthRpc,
    pub web3: Web3Rpc,
    pub net: NetRpc,
    pub txpool: TxPoolRpc,
    pub admin: AdminRpc,
    pub debug: DebugRpc,
}

impl RpcHandlers {
    /// Create all RPC handlers from config and IPC handler
    pub fn new(config: &GatewayConfig, ipc: Arc<IpcHandler>, data_dir: PathBuf) -> Self {
        Self {
            eth: EthRpc::new(Arc::clone(&ipc), config.chain.chain_id),
            web3: Web3Rpc::new(config.chain.client_version.clone()),
            net: NetRpc::new(Arc::clone(&ipc), config.chain.chain_id),
            txpool: TxPoolRpc::new(Arc::clone(&ipc)),
            admin: AdminRpc::new(Arc::clone(&ipc), data_dir),
            debug: DebugRpc::new(ipc),
        }
    }
}
