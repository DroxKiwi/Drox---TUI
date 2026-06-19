//! Modal sélecteur `/theme`.

use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::AppState;
use crate::ui::TuiThemeSetting;

pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let Some(dialog) = state.theme_dialog.as_ref() else {
        return;
    };

    let popup_w = area.width.saturating_sub(2).min(56);
    let popup_h = 14.min(area.height.saturating_sub(2));
    let x = area.x + (area.width.saturating_sub(popup_w)) / 2;
    let y = area.y + (area.height.saturating_sub(popup_h)) / 2;
    let popup = Rect::new(x, y, popup_w, popup_h);

    frame.render_widget(Clear, popup);

    let mut lines = vec![
        Line::from(Span::styled(
            "Choisir un thème",
            Style::default()
                .fg(state.palette.header_primary)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "Persisté dans ~/.drox/tui-preferences.json",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
    ];

    for (i, theme) in TuiThemeSetting::ALL.iter().enumerate() {
        let marker = if i == dialog.cursor {
            Style::default()
                .fg(Color::Black)
                .bg(state.palette.composer_border)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        lines.push(Line::from(Span::styled(format!(" {} ", theme.label()), marker)));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "↑↓ choisir · Entrée appliquer · Esc annuler",
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" /theme ")
                .style(Style::default().fg(state.palette.border)),
        )
        .wrap(Wrap { trim: true })
        .alignment(Alignment::Left);

    frame.render_widget(paragraph, popup);
}
