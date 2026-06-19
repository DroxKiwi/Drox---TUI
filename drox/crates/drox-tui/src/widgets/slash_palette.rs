//! Palette commandes slash (`/`).

use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::AppState;
use crate::slash::palette::ENTRIES;

pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let Some(dialog) = state.slash_palette.as_ref() else {
        return;
    };

    let popup_w = area.width.saturating_sub(2).min(72);
    let visible = dialog.matches.len().min(10) as u16;
    let popup_h = (8 + visible).min(area.height.saturating_sub(4));
    let x = area.x + (area.width.saturating_sub(popup_w)) / 2;
    let y = area.y + area.height.saturating_sub(popup_h).saturating_sub(6);
    let popup = Rect::new(x, y, popup_w, popup_h);

    frame.render_widget(Clear, popup);

    let mut lines = vec![
        Line::from(Span::styled(
            "Commandes slash",
            Style::default()
                .fg(state.palette.header_primary)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            format!("Filtre : /{}", dialog.filter),
            Style::default().fg(Color::White),
        )),
        Line::from(""),
    ];

    if dialog.matches.is_empty() {
        lines.push(Line::from(Span::styled(
            "Aucune commande correspondante",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for (row, &idx) in dialog.matches.iter().enumerate().take(10) {
            let entry = &ENTRIES[idx];
            let marker = if row == dialog.cursor {
                Style::default()
                    .fg(Color::Black)
                    .bg(state.palette.composer_border)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };
            lines.push(Line::from(vec![
                Span::styled(format!(" {:<14}", entry.command), marker),
                Span::styled(entry.description, Style::default().fg(Color::DarkGray)),
            ]));
        }
        if dialog.matches.len() > 10 {
            lines.push(Line::from(Span::styled(
                format!(" … +{} autres", dialog.matches.len() - 10),
                Style::default().fg(Color::DarkGray),
            )));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "↑↓ choisir · Entrée insérer · Esc fermer",
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" / ")
                .style(Style::default().fg(state.palette.border)),
        )
        .wrap(Wrap { trim: true })
        .alignment(Alignment::Left);

    frame.render_widget(paragraph, popup);
}
