//! Modal changement de workspace (`/workspace`) — explorateur de dossiers.

use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{AppState, WorkspaceField, WorkspaceStep};
use crate::engine::dir_browser::BrowseEntryKind;
use crate::i18n::{self, keys};
use crate::widgets::modal_frame;

fn field_line(
    palette: &crate::ui::ThemePalette,
    label: &str,
    value: &str,
    cursor: usize,
    focused: bool,
) -> Line<'static> {
    let display = value.to_string();
    let mut spans = vec![Span::styled(
        format!("{label}: "),
        Style::default().fg(palette.header_muted),
    )];
    if focused {
        let before = display.get(..cursor.min(display.len())).unwrap_or("");
        let at = display
            .chars()
            .nth(cursor)
            .map(|c| c.to_string())
            .unwrap_or_else(|| " ".into());
        let after = display.get(cursor.saturating_add(at.len())..).unwrap_or("");
        spans.push(Span::styled(
            before.to_string(),
            Style::default().fg(palette.text),
        ));
        spans.push(Span::styled(
            at,
            Style::default()
                .fg(palette.selection_fg)
                .bg(palette.selection_bg)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            after.to_string(),
            Style::default().fg(palette.text),
        ));
    } else {
        spans.push(Span::styled(
            display,
            Style::default().fg(palette.text_muted),
        ));
    }
    Line::from(spans)
}

fn list_line(
    palette: &crate::ui::ThemePalette,
    label: &str,
    selected: bool,
    is_parent: bool,
) -> Line<'static> {
    let marker = if is_parent { ".." } else { ">" };
    let style = if selected {
        Style::default()
            .fg(palette.selection_fg)
            .bg(palette.accent_bright)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(palette.text_muted)
    };
    Line::from(Span::styled(format!(" {marker} {label} "), style))
}

fn workspace_status_is_error(status: &str) -> bool {
    [
        keys::WORKSPACE_STATUS_PATH_REQUIRED,
        keys::WORKSPACE_STATUS_ALREADY_CURRENT,
        keys::WORKSPACE_VALIDATE_PATH_REQUIRED,
        keys::WORKSPACE_VALIDATE_NOT_UTF8,
    ]
    .iter()
    .any(|k| status == i18n::t(k))
        || status.contains("introuvable")
        || status.contains("not found")
        || status.contains("n'est pas un répertoire")
        || status.contains("is not a directory")
}

pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let Some(dialog) = state.workspace_dialog.as_ref() else {
        return;
    };

    let popup = modal_frame::centered_popup(area, 78, 26);
    frame.render_widget(Clear, popup);

    let p = &state.palette;
    let mut lines = vec![
        Line::from(Span::styled(
            i18n::t(keys::MODAL_WORKSPACE_TITLE),
            Style::default()
                .fg(p.header_primary)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            i18n::t(keys::MODAL_WORKSPACE_SUBTITLE),
            Style::default().fg(p.header_muted),
        )),
        Line::from(""),
    ];

    match dialog.step {
        WorkspaceStep::Edit => {
            lines.push(Line::from(Span::styled(
                i18n::tf(keys::MODAL_WORKSPACE_CURRENT_DIR, &dialog.location.display_path()),
                Style::default().fg(p.accent_glow),
            )));
            lines.push(Line::from(""));

            if dialog.focus == WorkspaceField::Browser {
                lines.push(Line::from(Span::styled(
                    i18n::t(keys::MODAL_WORKSPACE_EXPLORER),
                    Style::default().fg(p.header_primary),
                )));
            } else {
                lines.push(Line::from(Span::styled(
                    i18n::t(keys::MODAL_WORKSPACE_EXPLORER),
                    Style::default().fg(p.header_muted),
                )));
            }

            let max = dialog.entries.len().min(8);
            let start = dialog.browse_cursor.saturating_sub(max / 2);
            let end = (start + max).min(dialog.entries.len());
            for (i, entry) in dialog.entries[start..end].iter().enumerate() {
                let idx = start + i;
                let selected =
                    dialog.focus == WorkspaceField::Browser && idx == dialog.browse_cursor;
                lines.push(list_line(
                    p,
                    &entry.label,
                    selected,
                    entry.kind == BrowseEntryKind::Parent,
                ));
            }
            if dialog.entries.len() > max {
                lines.push(Line::from(Span::styled(
                    i18n::tf(
                        keys::MODAL_WORKSPACE_ENTRIES_TOTAL,
                        &dialog.entries.len().to_string(),
                    ),
                    Style::default().fg(p.header_muted),
                )));
            }

            lines.push(Line::from(""));
            let btn_style = if dialog.focus == WorkspaceField::SelectButton {
                Style::default()
                    .fg(p.selection_fg)
                    .bg(p.accent_bright)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(p.text_muted)
            };
            lines.push(Line::from(Span::styled(
                i18n::t(keys::MODAL_WORKSPACE_BTN_SELECT),
                btn_style,
            )));

            lines.push(Line::from(""));
            lines.push(field_line(
                p,
                i18n::t(keys::MODAL_WORKSPACE_FIELD_PATH),
                &dialog.path,
                dialog.path_cursor,
                dialog.focus == WorkspaceField::Path,
            ));
        }
        WorkspaceStep::Confirm => {
            if let Some(ref path) = dialog.validated {
                lines.push(Line::from(Span::styled(
                    i18n::t(keys::MODAL_WORKSPACE_VALID),
                    Style::default().fg(p.header_primary),
                )));
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    path.as_str(),
                    Style::default().fg(p.text),
                )));
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    i18n::t(keys::MODAL_WORKSPACE_CONFIRM_WARN),
                    Style::default().fg(p.warning),
                )));
            }
        }
    }

    if !dialog.status.is_empty() {
        lines.push(Line::from(""));
        let color = if workspace_status_is_error(&dialog.status) {
            p.error
        } else if dialog.step == WorkspaceStep::Confirm {
            p.warning
        } else {
            p.header_muted
        };
        lines.push(Line::from(Span::styled(
            dialog.status.clone(),
            Style::default().fg(color),
        )));
    }

    lines.push(Line::from(""));
    let hints = match dialog.step {
        WorkspaceStep::Edit => i18n::t(keys::MODAL_WORKSPACE_HINT_EDIT),
        WorkspaceStep::Confirm => i18n::t(keys::MODAL_WORKSPACE_HINT_CONFIRM),
    };
    lines.push(Line::from(Span::styled(
        hints,
        Style::default().fg(p.header_muted),
    )));

    frame.render_widget(
        Paragraph::new(lines)
            .block(modal_frame::pip_boy_block_animated(
                i18n::t(keys::MODAL_WORKSPACE_FRAME),
                p,
                state.modal_anim_tick,
                state.animations_enabled,
            ))
            .wrap(Wrap { trim: true })
            .alignment(Alignment::Left),
        popup,
    );
}
