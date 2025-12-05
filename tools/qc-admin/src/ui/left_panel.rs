//! Left panel: Subsystem list + System health.
//!
//! ```text
//! ┌──────────────────────┐
//! │   SUBSYSTEMS         │
//! │                      │
//! │  [1] qc-01 ● RUN     │
//! │  [2] qc-02 ● RUN     │
//! │  ...                 │
//! ├──────────────────────┤
//! │  SYSTEM HEALTH       │
//! │  CPU:  ████░░ 65%    │
//! │  MEM:  ███░░░ 48%    │
//! │  Status: RUNNING     │
//! └──────────────────────┘
//! ```

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

use crate::domain::{App, NodeStatus, SubsystemId, SubsystemStatus};

/// Render the left panel.
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    // Vertical split: subsystem list (flexible) + system health (fixed)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(10),   // Subsystem list
            Constraint::Length(7), // System health
        ])
        .split(area);

    render_subsystem_list(frame, chunks[0], app);
    render_system_health(frame, chunks[1], app);
}

/// Render the subsystem list.
fn render_subsystem_list(frame: &mut Frame, area: Rect, app: &App) {
    let items: Vec<ListItem> = SubsystemId::ALL
        .iter()
        .map(|&id| {
            let info = app.subsystems.get(&id);
            let status = info.map(|i| i.status).unwrap_or_default();
            let is_selected = id == app.selected_subsystem;

            // Build the line: [hotkey] code indicator label
            let hotkey = id.hotkey().map(|c| c.to_string()).unwrap_or("-".to_string());
            let indicator = status.indicator();
            let label = status.label();

            // Status color
            let status_color = match status {
                SubsystemStatus::Running => Color::Green,
                SubsystemStatus::Warning => Color::Yellow,
                SubsystemStatus::Stopped => Color::Red,
                SubsystemStatus::NotImplemented => Color::DarkGray,
            };

            // Dim not-implemented subsystems
            let text_style = if status == SubsystemStatus::NotImplemented {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default()
            };

            // Highlight selected
            let line_style = if is_selected {
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let spans = vec![
                Span::styled(format!("[{}] ", hotkey), text_style),
                Span::styled(format!("{} ", id.code()), text_style),
                Span::styled(format!("{} ", indicator), Style::default().fg(status_color)),
                Span::styled(label, Style::default().fg(status_color)),
            ];

            ListItem::new(Line::from(spans)).style(line_style)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .title(" SUBSYSTEMS ")
            .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    frame.render_widget(list, area);
}

/// Render system health panel.
fn render_system_health(frame: &mut Frame, area: Rect, app: &App) {
    let health = &app.system_health;

    // CPU bar
    let cpu_bar = render_progress_bar(health.cpu_percent, 10);
    let cpu_line = Line::from(vec![
        Span::raw("CPU: "),
        Span::styled(cpu_bar, progress_bar_color(health.cpu_percent)),
        Span::raw(format!(" {:>3.0}%", health.cpu_percent)),
    ]);

    // Memory bar
    let mem_bar = render_progress_bar(health.memory_percent, 10);
    let mem_line = Line::from(vec![
        Span::raw("MEM: "),
        Span::styled(mem_bar, progress_bar_color(health.memory_percent)),
        Span::raw(format!(" {:>3.0}%", health.memory_percent)),
    ]);

    // Status line
    let status_color = match health.node_status {
        NodeStatus::Running => Color::Green,
        NodeStatus::Warning => Color::Yellow,
        NodeStatus::Stopped => Color::Red,
    };
    let status_line = Line::from(vec![
        Span::raw("Status: "),
        Span::styled(health.node_status.label(), Style::default().fg(status_color)),
    ]);

    let text = vec![cpu_line, mem_line, Line::raw(""), status_line];

    let paragraph = Paragraph::new(text).block(
        Block::default()
            .title(" SYSTEM HEALTH ")
            .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    frame.render_widget(paragraph, area);
}

/// Render a simple progress bar.
fn render_progress_bar(percent: f32, width: usize) -> String {
    let filled = ((percent / 100.0) * width as f32).round() as usize;
    let empty = width.saturating_sub(filled);
    format!("{}{}", "█".repeat(filled), "░".repeat(empty))
}

/// Get color for progress bar based on percentage.
fn progress_bar_color(percent: f32) -> Style {
    let color = if percent >= 90.0 {
        Color::Red
    } else if percent >= 70.0 {
        Color::Yellow
    } else {
        Color::Green
    };
    Style::default().fg(color)
}
