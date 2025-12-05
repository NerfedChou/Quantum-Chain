//! Placeholder and "not implemented" renderers.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::domain::SubsystemId;

/// Render a "not implemented" message for subsystems that don't exist in the codebase.
pub fn render_not_implemented(frame: &mut Frame, area: Rect, id: SubsystemId) {
    let text = vec![
        Line::raw(""),
        Line::raw(""),
        Line::from(vec![Span::styled(
            "╔═══════════════════════════════════╗",
            Style::default().fg(Color::DarkGray),
        )]),
        Line::from(vec![Span::styled(
            "║                                   ║",
            Style::default().fg(Color::DarkGray),
        )]),
        Line::from(vec![Span::styled(
            "║        NOT IMPLEMENTED            ║",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![Span::styled(
            "║                                   ║",
            Style::default().fg(Color::DarkGray),
        )]),
        Line::from(vec![Span::styled(
            "║   This subsystem is not yet       ║",
            Style::default().fg(Color::DarkGray),
        )]),
        Line::from(vec![Span::styled(
            "║   implemented in the Quantum-     ║",
            Style::default().fg(Color::DarkGray),
        )]),
        Line::from(vec![Span::styled(
            "║   Chain codebase.                 ║",
            Style::default().fg(Color::DarkGray),
        )]),
        Line::from(vec![Span::styled(
            "║                                   ║",
            Style::default().fg(Color::DarkGray),
        )]),
        Line::from(vec![Span::styled(
            "╚═══════════════════════════════════╝",
            Style::default().fg(Color::DarkGray),
        )]),
        Line::raw(""),
        Line::raw(""),
        Line::from(vec![Span::styled(
            format!("Subsystem: {} ({})", id.code(), id.name()),
            Style::default().fg(Color::DarkGray),
        )]),
    ];

    let paragraph = Paragraph::new(text).centered();
    frame.render_widget(paragraph, area);
}

/// Render a placeholder for implemented subsystems whose panel UI is not yet built.
pub fn render_placeholder(frame: &mut Frame, area: Rect, id: SubsystemId) {
    let text = vec![
        Line::raw(""),
        Line::from(vec![
            Span::styled(
                "Panel UI Coming Soon",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::raw(""),
        Line::from(vec![Span::styled(
            format!("The {} panel is under development.", id.name()),
            Style::default().fg(Color::DarkGray),
        )]),
        Line::raw(""),
        Line::from(vec![Span::styled(
            "This subsystem is implemented in the codebase but",
            Style::default().fg(Color::DarkGray),
        )]),
        Line::from(vec![Span::styled(
            "the admin panel UI has not been created yet.",
            Style::default().fg(Color::DarkGray),
        )]),
        Line::raw(""),
        Line::from(vec![Span::styled(
            format!("Subsystem: {} ({})", id.code(), id.name()),
            Style::default().fg(Color::DarkGray),
        )]),
    ];

    let paragraph = Paragraph::new(text).centered();
    frame.render_widget(paragraph, area);
}
