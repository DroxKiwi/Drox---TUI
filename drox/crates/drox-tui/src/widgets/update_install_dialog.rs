//! Modale confirmation installation MAJ (`Ctrl+Shift+U` / `/update install`).

use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::AppState;
use crate::i18n::{self, keys_update as u};
use crate::widgets::modal_frame;

fn sha_preview(hash: &str) -> String {
    let h = hash.trim();
    if h.len() <= 16 {
        h.to_string()
    } else {
        format!("{}…", &h[..16])
    }
}

pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let Some(dialog) = state.update_install.as_ref() else {
        return;
    };

    let popup_w = area.width.saturating_sub(2).min(62);
    let popup_h = 16.min(area.height.saturating_sub(2));
    let popup = modal_frame::centered_popup(area, popup_w, popup_h);
    frame.render_widget(Clear, popup);

    let mut lines = vec![
        Line::from(Span::styled(
            i18n::t(u::UPDATE_INSTALL_MODAL_TITLE),
            Style::default()
                .fg(state.palette.header_primary)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            i18n::tf2(
                u::UPDATE_INSTALL_MODAL_BODY,
                &dialog.from_version,
                &dialog.to_version,
            ),
            Style::default().fg(state.palette.text_muted),
        )),
        Line::from(Span::styled(
            i18n::tf(u::UPDATE_INSTALL_MODAL_SHA, &sha_preview(&dialog.sha256)),
            Style::default().fg(state.palette.header_muted),
        )),
    ];
    if let Some(ref notes) = dialog.release_notes {
        let short = if notes.len() > 80 {
            format!("{}…", &notes[..80])
        } else {
            notes.clone()
        };
        lines.push(Line::from(Span::styled(
            i18n::tf(u::UPDATE_INSTALL_MODAL_NOTES, &short),
            Style::default().fg(state.palette.header_muted),
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        i18n::t(u::UPDATE_INSTALL_MODAL_FOOTER),
        Style::default()
            .fg(state.palette.warning)
            .add_modifier(Modifier::BOLD),
    )));

    frame.render_widget(
        Paragraph::new(lines)
            .block(modal_frame::pip_boy_block_animated(
                "/update install",
                &state.palette,
                state.modal_anim_tick,
                state.animations_enabled,
            ))
            .wrap(Wrap { trim: true })
            .alignment(Alignment::Left),
        popup,
    );
}
