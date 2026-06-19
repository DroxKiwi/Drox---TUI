//! Modal changement de workspace (`/workspace`).

use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{AppState, WorkspaceField, WorkspaceStep};

fn field_line(label: &str, value: &str, cursor: usize, focused: bool) -> Line<'static> {
    let display = value.to_string();
    let mut spans = vec![
        Span::styled(format!("{label}: "), Style::default().fg(Color::DarkGray)),
    ];
    if focused {
        let before = display.get(..cursor.min(display.len())).unwrap_or("");
        let at = display
            .chars()
            .nth(cursor)
            .map(|c| c.to_string())
            .unwrap_or_else(|| " ".into());
        let after = display.get(cursor.saturating_add(at.len())..).unwrap_or("");
        spans.push(Span::styled(before.to_string(), Style::default().fg(Color::White)));
        spans.push(Span::styled(
            at,
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(after.to_string(), Style::default().fg(Color::White)));
    } else {
        spans.push(Span::styled(display, Style::default().fg(Color::Gray)));
    }
    Line::from(spans)
}

pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let Some(dialog) = state.workspace_dialog.as_ref() else {
        return;
    };

    let popup_w = area.width.saturating_sub(4).min(72);
    let popup_h = 22u16.min(area.height.saturating_sub(4));
    let x = area.x + (area.width.saturating_sub(popup_w)) / 2;
    let y = area.y + (area.height.saturating_sub(popup_h)) / 2;
    let popup = Rect::new(x, y, popup_w, popup_h);
    frame.render_widget(Clear, popup);

    let accent = state.palette.header_primary;
    let mut lines = vec![
        Line::from(Span::styled(
            "Changer de workspace",
            Style::default()
                .fg(accent)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "Nouvelle session - fil efface - re-bootstrap moteur",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
    ];

    match dialog.step {
        WorkspaceStep::Edit => {
            if !dialog.recents.is_empty() {
                lines.push(Line::from(Span::styled(
                    "Recents",
                    Style::default().fg(accent),
                )));
                let max = dialog.recents.len().min(5);
                let start = dialog.recent_cursor.saturating_sub(max / 2);
                let end = (start + max).min(dialog.recents.len());
                for (i, path) in dialog.recents[start..end].iter().enumerate() {
                    let idx = start + i;
                    let marker = if dialog.focus == WorkspaceField::Recents && idx == dialog.recent_cursor {
                        Style::default()
                            .fg(Color::Black)
                            .bg(accent)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::Gray)
                    };
                    let short = shorten_path(path, 58);
                    lines.push(Line::from(Span::styled(format!(" {short} "), marker)));
                }
                lines.push(Line::from(""));
            }
            lines.push(field_line(
                "Chemin",
                &dialog.path,
                dialog.path_cursor,
                dialog.focus == WorkspaceField::Path,
            ));
            lines.push(Line::from(""));
            let btn_style = if dialog.focus == WorkspaceField::ValidateButton {
                Style::default()
                    .fg(Color::Black)
                    .bg(accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };
            lines.push(Line::from(Span::styled(" Verifier ", btn_style)));
        }
        WorkspaceStep::Confirm => {
            if let Some(ref path) = dialog.validated {
                lines.push(Line::from(Span::styled(
                    "Workspace valide",
                    Style::default().fg(accent),
                )));
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    path.as_str(),
                    Style::default().fg(Color::White),
                )));
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "Une nouvelle session sera creee. Le fil courant sera efface.",
                    Style::default().fg(Color::Yellow),
                )));
            }
        }
    }

    if !dialog.status.is_empty() {
        lines.push(Line::from(""));
        let color = if dialog.status.starts_with("chemin") || dialog.status.contains("echou") {
            Color::Red
        } else if dialog.step == WorkspaceStep::Confirm {
            Color::Yellow
        } else {
            Color::DarkGray
        };
        lines.push(Line::from(Span::styled(
            dialog.status.clone(),
            Style::default().fg(color),
        )));
    }

    lines.push(Line::from(""));
    let hints = match dialog.step {
        WorkspaceStep::Edit => "Tab - fleches - Entree verifier - Esc annuler",
        WorkspaceStep::Confirm => "Entree confirmer - Esc retour",
    };
    lines.push(Line::from(Span::styled(
        hints,
        Style::default().fg(Color::DarkGray),
    )));

    frame.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" /workspace - Ctrl+Shift+W ")
                    .style(Style::default().fg(state.palette.border)),
            )
            .wrap(Wrap { trim: true })
            .alignment(Alignment::Left),
        popup,
    );
}

fn shorten_path(path: &str, max: usize) -> String {
    if path.len() <= max {
        return path.to_string();
    }
    format!("...{}", &path[path.len().saturating_sub(max.saturating_sub(3))..])
}