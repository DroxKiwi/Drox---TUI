//! Barres de recherche (transcript + historique).

use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::AppState;
use crate::view::{HistorySearchState, TranscriptSearchState};

pub fn render_transcript_bar(frame: &mut Frame, area: Rect, state: &AppState, search: &TranscriptSearchState) {
    let bar_h = 3u16.min(area.height);
    let y = area.y + area.height.saturating_sub(bar_h);
    let popup = Rect::new(area.x, y, area.width, bar_h);
    frame.render_widget(Clear, popup);

    let line = Line::from(vec![
        Span::styled("/ ", Style::default().fg(Color::Yellow)),
        Span::styled(&search.query, Style::default().fg(Color::White)),
        Span::styled(
            format!("  {}  ", search.counter_label()),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(
            "Entrée fermer · Ctrl+n/p suiv/préc · Esc annuler",
            Style::default().fg(Color::DarkGray),
        ),
    ]);

    let paragraph = Paragraph::new(line)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Recherche Ctrl+F ")
                .style(Style::default().fg(state.palette.border)),
        )
        .alignment(Alignment::Left);

    frame.render_widget(paragraph, popup);
}

pub fn render_history_bar(frame: &mut Frame, area: Rect, state: &AppState, search: &HistorySearchState) {
    let bar_h = 3u16.min(area.height);
    let y = area.y + area.height.saturating_sub(bar_h);
    let popup = Rect::new(area.x, y, area.width, bar_h);
    frame.render_widget(Clear, popup);

    let line = Line::from(vec![
        Span::styled(
            "reverse-i-search`",
            Style::default().fg(state.palette.mode_tag),
        ),
        Span::raw(&search.query),
        Span::styled(
            "` ",
            Style::default().fg(state.palette.mode_tag),
        ),
        Span::styled(
            search.counter_label(),
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        ),
        Span::styled(
            " · Entrée valider · Ctrl+R suivant · Esc annuler",
            Style::default().fg(Color::DarkGray),
        ),
    ]);

    let paragraph = Paragraph::new(line)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Historique Ctrl+R ")
                .style(Style::default().fg(state.palette.composer_border)),
        )
        .alignment(Alignment::Left);

    frame.render_widget(paragraph, popup);
}
