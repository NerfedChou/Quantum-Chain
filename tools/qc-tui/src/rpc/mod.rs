//! RPC module for qc-16 API Gateway communication.

mod client;

pub use client::{BlockInfo, NodeInfo, PeerInfo, RpcClient, SyncStatus, TxInfo, TxPoolStatus};
