//! Per-subsystem detail renderers.
//!
//! Each subsystem has its own dedicated renderer file that knows how to
//! display subsystem-specific metrics and information.

mod not_implemented;
mod qc_01_peers;
mod qc_02_storage;

use ratatui::{layout::Rect, Frame};

use crate::domain::{App, SubsystemId, SubsystemInfo};

/// Dispatch to the appropriate subsystem renderer.
pub fn render(frame: &mut Frame, area: Rect, id: SubsystemId, info: &SubsystemInfo, app: &App) {
    match id {
        SubsystemId::PeerDiscovery => qc_01_peers::render(frame, area, info, app),
        SubsystemId::BlockStorage => qc_02_storage::render(frame, area, info, app),
        SubsystemId::TransactionIndexing => not_implemented::render_placeholder(frame, area, id),
        SubsystemId::StateManagement => not_implemented::render_placeholder(frame, area, id),
        SubsystemId::BlockPropagation => not_implemented::render_placeholder(frame, area, id),
        SubsystemId::Mempool => not_implemented::render_placeholder(frame, area, id),
        SubsystemId::BloomFilters => not_implemented::render_not_implemented(frame, area, id),
        SubsystemId::Consensus => not_implemented::render_placeholder(frame, area, id),
        SubsystemId::Finality => not_implemented::render_placeholder(frame, area, id),
        SubsystemId::SignatureVerification => not_implemented::render_placeholder(frame, area, id),
        SubsystemId::SmartContracts => not_implemented::render_not_implemented(frame, area, id),
        SubsystemId::TransactionOrdering => not_implemented::render_not_implemented(frame, area, id),
        SubsystemId::LightClientSync => not_implemented::render_not_implemented(frame, area, id),
        SubsystemId::Sharding => not_implemented::render_not_implemented(frame, area, id),
        SubsystemId::CrossChain => not_implemented::render_not_implemented(frame, area, id),
        SubsystemId::ApiGateway => not_implemented::render_placeholder(frame, area, id),
    }
}
