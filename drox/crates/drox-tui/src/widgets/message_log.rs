//! Fil de discussion scrollable.

use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::AppState;
use crate::widgets::search_bar;

pub fn render(frame: &mut Frame, area: Rect, state: &mut AppState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(if state.transcript_search.is_some() {
            " Session · recherche "
        } else {
            " Session "
        })
        .style(Style::default().fg(Color::White));

    let lines = state.flattened_log_lines_display();
    let inner_h = area.height.saturating_sub(2) as usize;
    let total = lines.len();
    let scroll = state.scroll as usize;
    let end = total.saturating_sub(scroll);
    let start = end.saturating_sub(inner_h);
    let visible = &lines[start..end];

    let paragraph = Paragraph::new(visible.to_vec())
        .block(block)
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);

    if let Some(ref search) = state.transcript_search {
        search_bar::render_transcript_bar(frame, area, state, search);
    }
}
