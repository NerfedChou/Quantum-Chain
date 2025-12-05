//! UI module - TUI rendering components.
//!
//! The UI follows a component-based architecture:
//! - `layout.rs`: Main layout orchestration
//! - `left_panel.rs`: Subsystem list + System health
//! - `right_panel.rs`: Dispatches to subsystem-specific renderers
//! - `widgets/`: Reusable UI components
//! - `subsystems/`: Per-subsystem detail renderers

mod layout;
mod left_panel;
mod right_panel;

pub mod subsystems;
pub mod widgets;

pub use layout::render;
