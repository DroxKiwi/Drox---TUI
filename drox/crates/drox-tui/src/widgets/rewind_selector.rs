//! Modal sélecteur `/rewind`.

use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::AppState;

pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let Some(dialog) = state.rewind.as_ref() else {
        return;
    };

    let popup_w = area.width.saturating_sub(2).min(72);
    let visible = dialog.choices.len().min(8) as u16;
    let popup_h = (8 + visible).min(area.height.saturating_sub(2));
    let x = area.x + (area.width.saturating_sub(popup_w)) / 2;
    let y = area.y + (area.height.saturating_sub(popup_h)) / 2;
    let popup = Rect::new(x, y, popup_w, popup_h);

    frame.render_widget(Clear, popup);

    let mut lines = vec![
        Line::from(Span::styled(
            "Rembobiner la conversation",
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "Choisir un message — tout après sera supprimé du transcript",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
    ];

    if dialog.choices.is_empty() {
        lines.push(Line::from("Aucun message utilisateur dans le transcript."));
    } else {
        for (i, choice) in dialog.choices.iter().enumerate() {
            let marker = if i == dialog.cursor {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Magenta)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };
            lines.push(Line::from(Span::styled(format!(" {} ", choice.label), marker)));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "↑↓ choisir · Entrée rembobiner · Esc annuler",
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" /rewind ")
                .style(Style::default().fg(Color::Magenta)),
        )
        .wrap(Wrap { trim: true })
        .alignment(Alignment::Left);

    frame.render_widget(paragraph, popup);
}
