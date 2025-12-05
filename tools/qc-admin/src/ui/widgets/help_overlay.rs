//! Help overlay widget.

use ratatui::{
    layout::{Constraint, Flex, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

/// Render a centered help overlay.
pub fn render_help_overlay(frame: &mut Frame) {
    let area = frame.area();

    // Center a box in the middle of the screen
    let popup_area = centered_rect(60, 70, area);

    // Clear the background
    frame.render_widget(Clear, popup_area);

    let help_text = vec![
        Line::from(vec![
            Span::styled(
                "QC-ADMIN HELP",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::styled("Navigation", Style::default().add_modifier(Modifier::BOLD)),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::styled("  1-9    ", Style::default().fg(Color::Yellow)),
            Span::raw("Select subsystem 01-09"),
        ]),
        Line::from(vec![
            Span::styled("  0      ", Style::default().fg(Color::Yellow)),
            Span::raw("Select subsystem 10 (Signature Verification)"),
        ]),
        Line::from(vec![
            Span::styled("  G      ", Style::default().fg(Color::Yellow)),
            Span::raw("Select subsystem 16 (API Gateway)"),
        ]),
        Line::from(vec![
            Span::styled("  ↑/↓    ", Style::default().fg(Color::Yellow)),
            Span::raw("Navigate subsystem list"),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::styled("Actions", Style::default().add_modifier(Modifier::BOLD)),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::styled("  Enter  ", Style::default().fg(Color::Yellow)),
            Span::raw("Drill down into subsystem"),
        ]),
        Line::from(vec![
            Span::styled("  B      ", Style::default().fg(Color::Yellow)),
            Span::raw("Back to previous view"),
        ]),
        Line::from(vec![
            Span::styled("  R      ", Style::default().fg(Color::Yellow)),
            Span::raw("Refresh data"),
        ]),
        Line::from(vec![
            Span::styled("  Q      ", Style::default().fg(Color::Yellow)),
            Span::raw("Quit"),
        ]),
        Line::from(vec![
            Span::styled("  ?      ", Style::default().fg(Color::Yellow)),
            Span::raw("Toggle this help"),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::styled("Status Indicators", Style::default().add_modifier(Modifier::BOLD)),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::styled("  ● RUN  ", Style::default().fg(Color::Green)),
            Span::raw("Subsystem running and healthy"),
        ]),
        Line::from(vec![
            Span::styled("  ● WARN ", Style::default().fg(Color::Yellow)),
            Span::raw("Running but dependency is down"),
        ]),
        Line::from(vec![
            Span::styled("  ● STOP ", Style::default().fg(Color::Red)),
            Span::raw("Subsystem stopped or error"),
        ]),
        Line::from(vec![
            Span::styled("  ○ N/I  ", Style::default().fg(Color::DarkGray)),
            Span::raw("Not implemented"),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::styled(
                "Press any key to close",
                Style::default().fg(Color::DarkGray),
            ),
        ]),
    ];

    let paragraph = Paragraph::new(help_text).block(
        Block::default()
            .title(" Help ")
            .title_style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );

    frame.render_widget(paragraph, popup_area);
}

/// Create a centered rectangle.
fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::vertical([Constraint::Percentage(percent_y)]).flex(Flex::Center);
    let horizontal = Layout::horizontal([Constraint::Percentage(percent_x)]).flex(Flex::Center);

    let [area] = vertical.areas(area);
    let [area] = horizontal.areas(area);
    area
}
