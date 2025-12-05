//! Domain models for the admin panel.

mod app;
mod subsystem;

pub use app::{App, AppState, PeerDisplayInfo, PendingAssemblyInfo};
pub use subsystem::{NodeStatus, SubsystemId, SubsystemInfo, SubsystemStatus, SystemHealth};
