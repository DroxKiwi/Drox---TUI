//! Bandeau notices sous le header (§11.4).

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::app::AppState;
use crate::engine::notices::NoticeLevel;

#[must_use]
pub fn desired_height(state: &AppState) -> u16 {
    if state.status_notices.is_empty() {
        0
    } else {
        state.status_notices.len().min(3) as u16 + 2
    }
}

pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    if state.status_notices.is_empty() {
        return;
    }

    let lines: Vec<Line> = state
        .status_notices
        .iter()
        .take(3)
        .map(|n| {
            let (prefix, color) = match n.level {
                NoticeLevel::Warn => ("⚠ ", Color::Yellow),
                NoticeLevel::Tip => ("💡 ", Color::Cyan),
                NoticeLevel::Info => ("ℹ ", Color::Blue),
            };
            Line::from(vec![
                Span::styled(prefix, Style::default().fg(color).add_modifier(Modifier::BOLD)),
                Span::styled(&n.text, Style::default().fg(state.palette.header_muted)),
            ])
        })
        .collect();

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Notices ")
        .style(Style::default().fg(state.palette.border));

    frame.render_widget(Paragraph::new(lines).block(block), area);
}
