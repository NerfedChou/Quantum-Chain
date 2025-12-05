//! Per-subsystem detail renderers.
//!
//! Each subsystem has its own dedicated renderer file that knows how to
//! display subsystem-specific metrics and information.

mod not_implemented;
mod qc_01_peers;
mod qc_02_storage;
mod qc_03_indexing;
mod qc_04_state;
mod qc_05_propagation;
mod qc_06_mempool;
mod qc_08_consensus;
mod qc_09_finality;
mod qc_10_signature;
mod qc_16_gateway;

use ratatui::{layout::Rect, Frame};

use crate::domain::{App, SubsystemId, SubsystemInfo};

/// Dispatch to the appropriate subsystem renderer.
pub fn render(frame: &mut Frame, area: Rect, id: SubsystemId, info: &SubsystemInfo, app: &App) {
    match id {
        SubsystemId::PeerDiscovery => qc_01_peers::render(frame, area, info, app),
        SubsystemId::BlockStorage => qc_02_storage::render(frame, area, info, app),
        SubsystemId::TransactionIndexing => qc_03_indexing::render(frame, area, info),
        SubsystemId::StateManagement => qc_04_state::render(frame, area, info),
        SubsystemId::BlockPropagation => qc_05_propagation::render(frame, area, info),
        SubsystemId::Mempool => qc_06_mempool::render(frame, area, info),
        SubsystemId::BloomFilters => not_implemented::render_not_implemented(frame, area, id),
        SubsystemId::Consensus => qc_08_consensus::render(frame, area, info),
        SubsystemId::Finality => qc_09_finality::render(frame, area, info),
        SubsystemId::SignatureVerification => qc_10_signature::render(frame, area, info),
        SubsystemId::SmartContracts => not_implemented::render_not_implemented(frame, area, id),
        SubsystemId::TransactionOrdering => not_implemented::render_not_implemented(frame, area, id),
        SubsystemId::LightClientSync => not_implemented::render_not_implemented(frame, area, id),
        SubsystemId::Sharding => not_implemented::render_not_implemented(frame, area, id),
        SubsystemId::CrossChain => not_implemented::render_not_implemented(frame, area, id),
        SubsystemId::ApiGateway => qc_16_gateway::render(frame, area, info),
    }
}
