//! Blocks view UI rendering.

use crate::app::App;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Row, Table},
    Frame,
};

/// Render the blocks view.
pub fn render(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(10),   // Blocks table
            Constraint::Length(3), // Footer
        ])
        .split(frame.area());

    render_header(frame, app, chunks[0]);
    render_blocks_table(frame, app, chunks[1]);
    render_footer(frame, chunks[2]);
}

/// Render the header.
fn render_header(frame: &mut Frame, app: &App, area: Rect) {
    let finalized = app.block_height.saturating_sub(64); // Approximate finalized

    let header = Paragraph::new(Line::from(vec![
        Span::styled(" BLOCKS ", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw("│ "),
        Span::styled(
            format!("Latest: #{}", format_number(app.block_height)),
            Style::default().fg(Color::White),
        ),
        Span::raw(" │ "),
        Span::styled(
            format!("Finalized: ~#{}", format_number(finalized)),
            Style::default().fg(Color::Green),
        ),
        Span::raw(" │ "),
        Span::styled(
            format!("Showing: {}", app.blocks_list.len()),
            Style::default().fg(Color::Cyan),
        ),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Block Explorer "),
    );

    frame.render_widget(header, area);
}

/// Render the blocks table.
fn render_blocks_table(frame: &mut Frame, app: &App, area: Rect) {
    let header = Row::new(vec![
        " Height",
        "Hash",
        "Txs",
        "Gas Used",
        "Time",
        "Status",
    ])
    .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
    .height(1);

    let finalized_height = app.block_height.saturating_sub(64);

    let rows: Vec<Row> = app
        .blocks_list
        .iter()
        .enumerate()
        .map(|(i, block)| {
            let style = if i == app.blocks_selected {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default()
            };

            let block_num = block.block_number();
            let is_finalized = block_num <= finalized_height;
            let status = if is_finalized { "✓ Finalized" } else { "● Confirmed" };

            let age = format_block_age(app.block_height, block_num);

            Row::new(vec![
                format!(" #{}", format_number(block_num)),
                block.short_hash(),
                format!("{:>4}", block.tx_count()),
                format_gas(block.gas_used_u64()),
                age,
                status.to_string(),
            ])
            .style(style)
        })
        .collect();

    let widths = [
        Constraint::Length(14),
        Constraint::Length(14),
        Constraint::Length(6),
        Constraint::Length(12),
        Constraint::Length(12),
        Constraint::Length(12),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Recent Blocks ")
                .border_style(Style::default().fg(Color::Blue)),
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

/// Format a number with thousand separators.
fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    let chars: Vec<char> = s.chars().rev().collect();

    for (i, c) in chars.iter().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(*c);
    }

    result.chars().rev().collect()
}

/// Format gas used.
fn format_gas(gas: u64) -> String {
    if gas >= 1_000_000 {
        format!("{:.1}M", gas as f64 / 1_000_000.0)
    } else if gas >= 1_000 {
        format!("{:.1}K", gas as f64 / 1_000.0)
    } else {
        format!("{}", gas)
    }
}

/// Format block age based on height difference.
fn format_block_age(current: u64, block: u64) -> String {
    let diff = current.saturating_sub(block);
    let seconds = diff * 12; // ~12 second block time

    if seconds < 60 {
        format!("{}s ago", seconds)
    } else if seconds < 3600 {
        format!("{}m ago", seconds / 60)
    } else {
        format!("{}h ago", seconds / 3600)
    }
}
