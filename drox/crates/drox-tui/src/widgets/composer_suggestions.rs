//! Popup suggestions unifiées (`@` · `/` · skills) au-dessus du composer.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem};
use ratatui::Frame;

use crate::app::{AppState, ComposerSuggestionDialog};
use crate::engine::unified_suggestions::SuggestionKind;

const MAX_VISIBLE_ROWS: u16 = 8;

pub fn render(frame: &mut Frame, composer_area: Rect, state: &AppState, dialog: &ComposerSuggestionDialog) {
    let row_count = dialog.items.len().max(1) as u16;
    let popup_h = (row_count + 2).min(MAX_VISIBLE_ROWS + 2);
    let y = composer_area.y.saturating_sub(popup_h);
    let popup = Rect::new(composer_area.x, y, composer_area.width, popup_h);
    frame.render_widget(Clear, popup);

    if dialog.items.is_empty() {
        let hint = Line::from(Span::styled(
            "Aucune suggestion — Esc fermer",
            Style::default().fg(state.palette.header_muted),
        ));
        let block = Block::default()
            .borders(Borders::ALL)
            .title(dialog.title.clone())
            .style(Style::default().fg(state.palette.composer_border));
        frame.render_widget(ratatui::widgets::Paragraph::new(hint).block(block), popup);
        return;
    }

    let items: Vec<ListItem> = dialog
        .items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let active = i == dialog.cursor;
            let style = if active {
                Style::default()
                    .fg(state.palette.header_primary)
                    .add_modifier(Modifier::BOLD | Modifier::REVERSED)
            } else {
                Style::default().fg(state.palette.header_muted)
            };
            let kind_tag = match item.kind {
                SuggestionKind::File => "@",
                SuggestionKind::Slash => "/",
                SuggestionKind::Skill => "skill",
            };
            let mut spans = vec![
                Span::styled(format!("[{kind_tag}] "), Style::default().fg(state.palette.mode_tag)),
                Span::styled(item.label.clone(), style),
            ];
            if let Some(detail) = &item.detail {
                spans.push(Span::styled(
                    format!(" — {detail}"),
                    Style::default().fg(state.palette.header_muted),
                ));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!("{}· Tab · ↑↓", dialog.title))
            .style(Style::default().fg(state.palette.composer_border)),
    );
    frame.render_widget(list, popup);
}
