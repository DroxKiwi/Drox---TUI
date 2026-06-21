//! Modale réglages TUI (`/settings`).

use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{AppState, SettingsRowKind};
use crate::engine::preferences::preferences_path;
use crate::i18n::{self, keys, keys_update as ku};
use crate::widgets::modal_frame;

fn row_label(kind: SettingsRowKind) -> &'static str {
    match kind {
        SettingsRowKind::Language => i18n::t(keys::SETTINGS_LANGUAGE),
        SettingsRowKind::Animations => i18n::t(keys::SETTINGS_ANIMATIONS),
        SettingsRowKind::Mouse => i18n::t(keys::SETTINGS_MOUSE),
        SettingsRowKind::Vim => i18n::t(keys::SETTINGS_VIM),
        SettingsRowKind::Updates => i18n::t(ku::SETTINGS_UPDATE),
    }
}

fn row_value(state: &AppState, kind: SettingsRowKind, prefs: &crate::engine::preferences::TuiPreferences) -> String {
    match kind {
        SettingsRowKind::Language => {
            format!("{} ({})", prefs.ui_locale.label(), prefs.ui_locale.code())
        }
        SettingsRowKind::Animations => bool_label(state.animations_enabled),
        SettingsRowKind::Mouse => bool_label(state.mouse_enabled),
        SettingsRowKind::Vim => bool_label(prefs.vim_enabled),
        SettingsRowKind::Updates => bool_label(prefs.update.enabled),
    }
}

fn bool_label(on: bool) -> String {
    if on {
        i18n::t(keys::SETTINGS_VALUE_ON).into()
    } else {
        i18n::t(keys::SETTINGS_VALUE_OFF).into()
    }
}

pub fn render(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    prefs: &crate::engine::preferences::TuiPreferences,
) {
    let Some(dialog) = state.settings_dialog.as_ref() else {
        return;
    };

    let row_count = SettingsRowKind::ALL.len();
    let popup_h = (row_count as u16 + 8).min(area.height.saturating_sub(2));
    let popup_w = area.width.saturating_sub(2).min(58);
    let popup = modal_frame::centered_popup(area, popup_w, popup_h);
    frame.render_widget(Clear, popup);

    let mut lines = vec![
        Line::from(Span::styled(
            i18n::t(keys::SETTINGS_TITLE),
            Style::default()
                .fg(state.palette.header_primary)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            format!("{} : {}", i18n::t(keys::SETTINGS_FILE), preferences_path()),
            Style::default().fg(state.palette.header_muted),
        )),
        Line::from(""),
    ];

    for (i, kind) in SettingsRowKind::ALL.iter().enumerate() {
        let selected = i == dialog.cursor;
        let label = row_label(*kind);
        let value = row_value(state, *kind, prefs);
        let marker = if selected {
            Style::default()
                .fg(state.palette.selection_fg)
                .bg(state.palette.accent_bright)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(state.palette.text_muted)
        };
        lines.push(Line::from(vec![
            Span::styled(format!(" {label:<18} "), marker),
            Span::styled(value, marker),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        i18n::t(keys::SETTINGS_MODAL_FOOTER),
        Style::default().fg(state.palette.header_muted),
    )));

    frame.render_widget(
        Paragraph::new(lines)
            .block(modal_frame::pip_boy_block_animated(
                "/settings",
                &state.palette,
                state.modal_anim_tick,
                state.animations_enabled,
            ))
            .wrap(Wrap { trim: true })
            .alignment(Alignment::Left),
        popup,
    );
}
