//! Peers view UI rendering.

use crate::app::App;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Row, Table},
    Frame,
};

/// Render the peers view.
pub fn render(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Length(5), // Node info
            Constraint::Min(10),   // Peers table
            Constraint::Length(3), // Footer
        ])
        .split(frame.area());

    render_header(frame, app, chunks[0]);
    render_node_info(frame, app, chunks[1]);
    render_peers_table(frame, app, chunks[2]);
    render_footer(frame, chunks[3]);
}

/// Render the header.
fn render_header(frame: &mut Frame, app: &App, area: Rect) {
    let peer_count = app.peers_list.len();
    let trusted_count = app.peers_list.iter().filter(|p| p.is_trusted()).count();
    let inbound_count = app.peers_list.iter().filter(|p| p.network.inbound).count();
    let outbound_count = peer_count - inbound_count;

    let header = Paragraph::new(Line::from(vec![
        Span::styled(" PEERS ", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw("│ "),
        Span::styled(
            format!("Connected: {}", peer_count),
            Style::default().fg(Color::White),
        ),
        Span::raw(" │ "),
        Span::styled(
            format!("In: {}", inbound_count),
            Style::default().fg(Color::Green),
        ),
        Span::raw(" / "),
        Span::styled(
            format!("Out: {}", outbound_count),
            Style::default().fg(Color::Cyan),
        ),
        Span::raw(" │ "),
        Span::styled(
            format!("Trusted: {}", trusted_count),
            Style::default().fg(Color::Yellow),
        ),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Peer Management "),
    );

    frame.render_widget(header, area);
}

/// Render node info.
fn render_node_info(frame: &mut Frame, app: &App, area: Rect) {
    let text = if let Some(ref info) = app.node_info {
        vec![
            Line::from(vec![
                Span::raw(" Node ID: "),
                Span::styled(
                    if info.id.len() > 20 {
                        format!("{}...", &info.id[..20])
                    } else {
                        info.id.clone()
                    },
                    Style::default().fg(Color::Cyan),
                ),
            ]),
            Line::from(vec![
                Span::raw(" Name:    "),
                Span::styled(&info.name, Style::default().fg(Color::White)),
            ]),
        ]
    } else {
        vec![Line::from(Span::styled(
            " Node info unavailable (admin API may be disabled)",
            Style::default().fg(Color::DarkGray),
        ))]
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" This Node ")
        .border_style(Style::default().fg(Color::Blue));

    frame.render_widget(Paragraph::new(text).block(block), area);
}

/// Render peers table.
fn render_peers_table(frame: &mut Frame, app: &App, area: Rect) {
    let header = Row::new(vec![
        " Peer ID",
        "Name",
        "Remote Address",
        "Direction",
        "Trusted",
    ])
    .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
    .height(1);

    let rows: Vec<Row> = app
        .peers_list
        .iter()
        .enumerate()
        .map(|(i, peer)| {
            let style = if i == app.peers_selected {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default()
            };

            let trusted_symbol = if peer.is_trusted() { "✓" } else { "-" };

            Row::new(vec![
                format!(" {}", peer.short_id()),
                peer.display_name().to_string(),
                peer.remote_addr().to_string(),
                peer.direction().to_string(),
                trusted_symbol.to_string(),
            ])
            .style(style)
        })
        .collect();

    let widths = [
        Constraint::Length(20),
        Constraint::Length(20),
        Constraint::Length(24),
        Constraint::Length(10),
        Constraint::Length(8),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Connected Peers ")
                .border_style(Style::default().fg(Color::Green)),
        )
        .row_highlight_style(Style::default().add_modifier(Modifier::BOLD));

    frame.render_widget(table, area);
}

/// Render the footer.
fn render_footer(frame: &mut Frame, area: Rect) {
    let footer = Paragraph::new(Line::from(vec![
        Span::styled(" [D] ", Style::default().fg(Color::Yellow)),
        Span::raw("Dashboard  "),
        Span::styled("[M] ", Style::default().fg(Color::Yellow)),
        Span::raw("Mempool  "),
        Span::styled("[B] ", Style::default().fg(Color::Yellow)),
        Span::raw("Blocks  "),
        Span::styled("[P] ", Style::default().fg(Color::Yellow)),
        Span::raw("Peers  "),
        Span::raw("│ "),
        Span::styled("[↑/↓] ", Style::default().fg(Color::Yellow)),
        Span::raw("Navigate  "),
        Span::styled("[R] ", Style::default().fg(Color::Yellow)),
        Span::raw("Refresh  "),
        Span::styled("[Q] ", Style::default().fg(Color::Yellow)),
        Span::raw("Quit"),
    ]))
    .block(Block::default().borders(Borders::ALL));

    frame.render_widget(footer, area);
}
