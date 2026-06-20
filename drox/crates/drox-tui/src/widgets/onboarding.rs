//! Modal premier lancement (`/onboarding`).

use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::AppState;
use crate::i18n::{self, keys, ONBOARDING_STEPS};

pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let Some(dialog) = state.onboarding.as_ref() else {
        return;
    };

    let popup_w = area.width.saturating_sub(4).min(68);
    let popup_h = 16u16.min(area.height.saturating_sub(4));
    let x = area.x + (area.width.saturating_sub(popup_w)) / 2;
    let y = area.y + (area.height.saturating_sub(popup_h)) / 2;
    let popup = Rect::new(x, y, popup_w, popup_h);
    frame.render_widget(Clear, popup);

    let step = dialog.step.min(ONBOARDING_STEPS.len().saturating_sub(1));
    let lines = vec![
        Line::from(Span::styled(
            i18n::t(keys::ONBOARDING_TITLE),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            i18n::tf2(
                keys::ONBOARDING_STEP,
                &(step + 1).to_string(),
                &ONBOARDING_STEPS.len().to_string(),
            ),
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        Line::from(i18n::t(ONBOARDING_STEPS[step])),
        Line::from(""),
        Line::from(Span::styled(
            if step + 1 >= ONBOARDING_STEPS.len() {
                i18n::t(keys::ONBOARDING_FOOTER_DONE)
            } else {
                i18n::t(keys::ONBOARDING_FOOTER_NEXT)
            },
            Style::default().fg(Color::DarkGray),
        )),
    ];

    frame.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" {} ", i18n::t(keys::ONBOARDING_FRAME)))
                    .style(Style::default().fg(Color::Cyan)),
            )
            .wrap(Wrap { trim: true })
            .alignment(Alignment::Left),
        popup,
    );
}
