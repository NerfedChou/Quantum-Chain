//! QC-Admin: Quantum-Chain Admin Control Panel
//!
//! A TUI-based admin panel for monitoring and debugging Quantum-Chain subsystems.
//!
//! ## Architecture
//!
//! The admin panel follows a component-based architecture where each subsystem
//! has its own dedicated renderer. This mirrors the hexagonal architecture of
//! the main Quantum-Chain codebase.
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │  QC-ADMIN                                                       │
//! ├────────────────────────┬────────────────────────────────────────┤
//! │  SUBSYSTEMS            │  SUBSYSTEM DETAIL PANEL                │
//! │  [1] qc-01 ● RUN       │  (Component-based renderer)            │
//! │  [2] qc-02 ● RUN       │                                        │
//! │  ...                   │                                        │
//! ├────────────────────────┤                                        │
//! │  SYSTEM HEALTH         │                                        │
//! │  CPU: ████░░ 65%       │                                        │
//! │  MEM: ███░░░ 48%       │                                        │
//! └────────────────────────┴────────────────────────────────────────┘
//! ```

pub mod api;
pub mod domain;
pub mod ui;

pub use domain::{App, AppState, NodeStatus, PeerDisplayInfo, SubsystemId, SubsystemInfo, SubsystemStatus, SystemHealth};
