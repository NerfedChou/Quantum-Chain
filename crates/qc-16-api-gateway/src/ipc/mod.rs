//! IPC module for internal communication with subsystems.
//!
//! Per SPEC-16 Section 6, the API Gateway communicates with other subsystems
//! through the shared event bus using a query-response pattern.

pub mod bus_adapter;
pub mod handler;
pub mod requests;
pub mod responses;
pub mod validation;

pub use bus_adapter::{
    BlockQuery, EventBusReceiver, EventBusSender, MempoolQuery, PeerDiscoveryQuery, QueryRouter,
    ResponseRouter, StateQuery, TxIndexQuery,
};
pub use handler::{
    IpcError, IpcHandler, IpcReceiver, IpcSender, ResilientIpcHandler, ResponseListener,
};
pub use requests::{IpcRequest, RequestPayload, SubmitTransactionRequest};
pub use responses::{IpcResponse, ResponsePayload, SuccessData};
pub use validation::{create_submit_request, validate_raw_transaction, ValidatedTransaction};
