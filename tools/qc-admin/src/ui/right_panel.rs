//! Right panel: Subsystem detail view.
//!
//! Dispatches to the appropriate subsystem-specific renderer based on selection.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders},
    Frame,
};

use crate::domain::App;

use super::subsystems;

/// Render the right panel (subsystem detail).
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let selected = app.selected_subsystem;
    let info = app.selected_info();

    // Create the container block with subsystem name as title
    let title = format!(" {} {} ", selected.code().to_uppercase(), selected.name());
    let block = Block::default()
        .title(title)
        .title_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    // Calculate inner area for content
    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    // Dispatch to subsystem-specific renderer
    subsystems::render(frame, inner_area, selected, info, app);
}
