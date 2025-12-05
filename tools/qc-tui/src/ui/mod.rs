//! UI module for TUI rendering.

pub mod blocks;
pub mod dashboard;
pub mod mempool;
pub mod peers;

use crate::app::{App, Tab};
use ratatui::Frame;

/// Render the appropriate view based on active tab.
pub fn render(frame: &mut Frame, app: &App) {
    match app.active_tab {
        Tab::Dashboard => dashboard::render(frame, app),
        Tab::Mempool => mempool::render(frame, app),
        Tab::Blocks => blocks::render(frame, app),
        Tab::Peers => peers::render(frame, app),
    }
}
