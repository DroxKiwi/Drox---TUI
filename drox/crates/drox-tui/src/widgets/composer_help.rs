//! Menu d'aide composer (`?`) — raccourcis essentiels.

use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::AppState;
use crate::i18n::{self, keys_p1 as k, COMPOSER_HELP_LINES};

pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let h = (COMPOSER_HELP_LINES.len() as u16 + 2).min(area.height.saturating_sub(2));
    let w = area.width.min(72);
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h + 2);
    let popup = Rect::new(x, y, w, h);
    frame.render_widget(Clear, popup);

    let body: Vec<Line> = COMPOSER_HELP_LINES
        .iter()
        .map(|key| {
            Line::from(Span::styled(
                i18n::t(key),
                Style::default().fg(state.palette.header_muted),
            ))
        })
        .collect();

    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {} ", i18n::t(k::COMPOSER_HELP_TITLE)))
        .style(Style::default().fg(state.palette.composer_border));

    frame.render_widget(Paragraph::new(body).block(block).wrap(Wrap { trim: true }), popup);
}
