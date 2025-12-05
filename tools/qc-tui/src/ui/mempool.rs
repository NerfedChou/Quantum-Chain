//! Mempool view UI rendering.

use crate::app::App;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Row, Table},
    Frame,
};

/// Render the mempool view.
pub fn render(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Length(5), // Stats
            Constraint::Min(10),   // Transaction list
            Constraint::Length(3), // Footer
        ])
        .split(frame.area());

    render_header(frame, app, chunks[0]);
    render_stats(frame, app, chunks[1]);
    render_transactions(frame, app, chunks[2]);
    render_footer(frame, chunks[3]);
}

/// Render the header.
fn render_header(frame: &mut Frame, app: &App, area: Rect) {
    let header = Paragraph::new(Line::from(vec![
        Span::styled(" MEMPOOL ", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw("│ "),
        Span::styled(
            format!("Pending: {}", app.txpool_status.pending),
            Style::default().fg(Color::Yellow),
        ),
        Span::raw(" │ "),
        Span::styled(
            format!("Queued: {}", app.txpool_status.queued),
            Style::default().fg(Color::Cyan),
        ),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Transaction Pool "),
    );

    frame.render_widget(header, area);
}

/// Render mempool stats.
fn render_stats(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Pending stats
    let pending_text = vec![
        Line::from(vec![
            Span::raw(" Count: "),
            Span::styled(
                format!("{}", app.txpool_pending.len()),
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw(" Status: "),
            Span::styled("Ready for inclusion", Style::default().fg(Color::Green)),
        ]),
    ];

    let pending_block = Block::default()
        .borders(Borders::ALL)
        .title(" Pending ")
        .border_style(Style::default().fg(Color::Yellow));

    frame.render_widget(Paragraph::new(pending_text).block(pending_block), chunks[0]);

    // Queued stats
    let queued_text = vec![
        Line::from(vec![
            Span::raw(" Count: "),
            Span::styled(
                format!("{}", app.txpool_queued.len()),
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw(" Status: "),
            Span::styled("Waiting (nonce gaps)", Style::default().fg(Color::Cyan)),
        ]),
    ];

    let queued_block = Block::default()
        .borders(Borders::ALL)
        .title(" Queued ")
        .border_style(Style::default().fg(Color::Cyan));

    frame.render_widget(Paragraph::new(queued_text).block(queued_block), chunks[1]);
}

/// Render transaction list.
fn render_transactions(frame: &mut Frame, app: &App, area: Rect) {
    let header = Row::new(vec![
        " Hash",
        "From",
        "To",
        "Value",
        "Gas Price",
        "Nonce",
    ])
    .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
    .height(1);

    let rows: Vec<Row> = app
        .txpool_pending
        .iter()
        .enumerate()
        .map(|(i, tx)| {
            let style = if i == app.mempool_selected {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default()
            };

            Row::new(vec![
                format!(" {}", tx.short_hash()),
                tx.short_from(),
                tx.short_to(),
                format!("{:.4} ETH", tx.value_eth()),
                format!("{:.1} gwei", tx.gas_price_gwei()),
                format!("{}", tx.nonce_u64()),
            ])
            .style(style)
        })
        .collect();

    let widths = [
        Constraint::Length(14),
        Constraint::Length(12),
        Constraint::Length(12),
        Constraint::Length(12),
        Constraint::Length(12),
        Constraint::Length(8),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Pending Transactions ")
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
