//! Main layout orchestration.
//!
//! Renders the overall dashboard structure:
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │  QC-ADMIN v0.1.0                        [R]efresh [Q]uit [?]Help│
//! ├────────────────────────┬────────────────────────────────────────┤
//! │  SUBSYSTEMS            │  SUBSYSTEM DETAIL PANEL                │
//! │  ...                   │  ...                                   │
//! ├────────────────────────┤                                        │
//! │  SYSTEM HEALTH         │                                        │
//! │  ...                   │                                        │
//! └────────────────────────┴────────────────────────────────────────┘
//! │  [1-9,0,G] Select   [↑↓] Navigate   [Enter] Drill   [B] Back    │
//! └─────────────────────────────────────────────────────────────────┘
//! ```

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::domain::{App, AppState};

use super::{left_panel, right_panel, widgets};

/// Render the entire UI.
pub fn render(frame: &mut Frame, app: &App) {
    let size = frame.area();

    // Main vertical layout: header, body, footer
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(10),   // Body
            Constraint::Length(3), // Footer (keybinds)
        ])
        .split(size);

    render_header(frame, main_chunks[0], app);
    render_body(frame, main_chunks[1], app);
    render_footer(frame, main_chunks[2]);

    // Render help overlay if active
    if app.state == AppState::Help {
        widgets::render_help_overlay(frame);
    }
}

/// Render the header bar.
fn render_header(frame: &mut Frame, area: Rect, app: &App) {
    let title = vec![
        Span::styled(
            " QC-ADMIN ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("v0.1.0", Style::default().fg(Color::DarkGray)),
    ];

    // Show last refresh time or error
    let status = if let Some(err) = &app.error_message {
        Span::styled(
            format!(" ⚠ {} ", err),
            Style::default().fg(Color::Red),
        )
    } else if let Some(time) = app.last_refresh {
        Span::styled(
            format!(" Last refresh: {} ", time.format("%H:%M:%S")),
            Style::default().fg(Color::DarkGray),
        )
    } else {
        Span::styled(" No data ", Style::default().fg(Color::DarkGray))
    };

    let hints = vec![
        Span::styled("[R]", Style::default().fg(Color::Yellow)),
        Span::raw("efresh "),
        Span::styled("[Q]", Style::default().fg(Color::Yellow)),
        Span::raw("uit "),
        Span::styled("[?]", Style::default().fg(Color::Yellow)),
        Span::raw("Help "),
    ];

    // Calculate spacing
    let title_len: usize = title.iter().map(|s| s.content.len()).sum();
    let status_len = status.content.len();
    let hints_len: usize = hints.iter().map(|s| s.content.len()).sum();
    let padding = area
        .width
        .saturating_sub((title_len + status_len + hints_len) as u16);

    let mut spans = title;
    spans.push(status);
    spans.push(Span::raw(" ".repeat(padding as usize)));
    spans.extend(hints);

    let header = Paragraph::new(Line::from(spans)).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    frame.render_widget(header, area);
}

/// Render the main body (left panel + right panel).
fn render_body(frame: &mut Frame, area: Rect, app: &App) {
    // Horizontal split: left panel (fixed 26 chars) + right panel (rest)
    let body_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(28), // Left panel (subsystem list + health)
            Constraint::Min(40),    // Right panel (detail)
        ])
        .split(area);

    left_panel::render(frame, body_chunks[0], app);
    right_panel::render(frame, body_chunks[1], app);
}

/// Render the footer with keyboard shortcuts.
fn render_footer(frame: &mut Frame, area: Rect) {
    let keybinds = vec![
        Span::styled("[1-9,0,G]", Style::default().fg(Color::Yellow)),
        Span::raw(" Select  "),
        Span::styled("[↑↓]", Style::default().fg(Color::Yellow)),
        Span::raw(" Navigate  "),
        Span::styled("[Enter]", Style::default().fg(Color::Yellow)),
        Span::raw(" Drill Down  "),
        Span::styled("[B]", Style::default().fg(Color::Yellow)),
        Span::raw(" Back  "),
        Span::styled("[R]", Style::default().fg(Color::Yellow)),
        Span::raw(" Refresh  "),
    ];

    let footer = Paragraph::new(Line::from(keybinds))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .centered();

    frame.render_widget(footer, area);
}
