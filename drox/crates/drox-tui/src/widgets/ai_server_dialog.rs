//! Modal connexion serveur IA (`/server`).

use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{AiServerField, AiServerSelectFocus, AiServerStep, AppState};
use crate::engine::{preset_label, CONTEXT_CUSTOM_INDEX, CONTEXT_PRESETS};

fn field_line(label: &str, value: &str, cursor: usize, focused: bool, secret: bool) -> Line<'static> {
    let display = if secret && !value.is_empty() {
        "*".repeat(value.len())
    } else {
        value.to_string()
    };
    let mut spans = vec![
        Span::styled(format!("{label}: "), Style::default().fg(Color::DarkGray)),
    ];
    if focused {
        let before = display.get(..cursor.min(display.len())).unwrap_or("");
        let at = display.chars().nth(cursor).map(|c| c.to_string()).unwrap_or_else(|| " ".into());
        let after = display.get(cursor.saturating_add(at.len())..).unwrap_or("");
        spans.push(Span::styled(before.to_string(), Style::default().fg(Color::White)));
        spans.push(Span::styled(
            at,
            Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(after.to_string(), Style::default().fg(Color::White)));
    } else {
        spans.push(Span::styled(display, Style::default().fg(Color::Gray)));
    }
    Line::from(spans)
}

fn context_line(
    dialog: &crate::app::AiServerDialog,
    focused: bool,
    accent: Color,
) -> Line<'static> {
    if dialog.context_preset_index == CONTEXT_CUSTOM_INDEX {
        return field_line(
            "Context max",
            &dialog.context_custom,
            dialog.context_custom_cursor,
            focused,
            false,
        );
    }

    let preset = preset_label(dialog.context_preset_index, "");
    let tokens = CONTEXT_PRESETS[dialog.context_preset_index].1;
    let value = format!("{preset} ({tokens} tokens)");
    let mut spans = vec![
        Span::styled("Context max: ", Style::default().fg(Color::DarkGray)),
    ];
    if focused {
        spans.push(Span::styled(
            value,
            Style::default().fg(Color::Black).bg(accent).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            "  <-/-> changer",
            Style::default().fg(Color::DarkGray),
        ));
    } else {
        spans.push(Span::styled(value, Style::default().fg(Color::Gray)));
    }
    Line::from(spans)
}

pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let Some(dialog) = state.ai_server.as_ref() else {
        return;
    };

    let popup_w = area.width.saturating_sub(4).min(72);
    let popup_h = 24u16.min(area.height.saturating_sub(4));
    let x = area.x + (area.width.saturating_sub(popup_w)) / 2;
    let y = area.y + (area.height.saturating_sub(popup_h)) / 2;
    let popup = Rect::new(x, y, popup_w, popup_h);
    frame.render_widget(Clear, popup);

    let accent = state.palette.header_primary;
    let mut lines = vec![
        Line::from(Span::styled(
            "Connexion serveur IA",
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "Persiste dans ~/.drox/tui-preferences.json",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("Moteur : {}", dialog.engine.label()),
            Style::default().fg(Color::Gray),
        )),
        Line::from(""),
    ];

    match dialog.step {
        AiServerStep::Configure | AiServerStep::Testing => {
            lines.push(field_line(
                "Adresse",
                &dialog.server,
                dialog.server_cursor,
                dialog.focus == AiServerField::Server && dialog.step == AiServerStep::Configure,
                false,
            ));
            lines.push(field_line(
                "x-api-key",
                &dialog.api_key,
                dialog.api_key_cursor,
                dialog.focus == AiServerField::ApiKey && dialog.step == AiServerStep::Configure,
                true,
            ));
            lines.push(context_line(
                dialog,
                dialog.focus == AiServerField::NumCtx && dialog.step == AiServerStep::Configure,
                accent,
            ));
            lines.push(Line::from(""));
            let test_style = if dialog.focus == AiServerField::TestButton {
                Style::default().fg(Color::Black).bg(accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };
            let test_label = if dialog.step == AiServerStep::Testing {
                " Tester connexion… "
            } else {
                " Tester connexion "
            };
            lines.push(Line::from(Span::styled(test_label, test_style)));
        }
        AiServerStep::SelectModel => {
            lines.push(Line::from(Span::styled(
                "Modeles disponibles",
                Style::default().fg(accent),
            )));
            lines.push(Line::from(""));
            let max = dialog.models.len().min(8);
            let start = dialog.model_cursor.saturating_sub(max / 2);
            let end = (start + max).min(dialog.models.len());
            for (i, name) in dialog.models[start..end].iter().enumerate() {
                let idx = start + i;
                let marker = if idx == dialog.model_cursor
                    && dialog.select_focus == AiServerSelectFocus::ModelList
                {
                    Style::default().fg(Color::Black).bg(accent).add_modifier(Modifier::BOLD)
                } else if idx == dialog.model_cursor {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default().fg(Color::Gray)
                };
                lines.push(Line::from(Span::styled(format!(" {name} "), marker)));
            }
            if dialog.models.len() > max {
                lines.push(Line::from(Span::styled(
                    format!("… {} modele(s) au total", dialog.models.len()),
                    Style::default().fg(Color::DarkGray),
                )));
            }
            lines.push(Line::from(""));
            lines.push(field_line(
                "Max iterations",
                &dialog.max_iterations,
                dialog.max_iterations_cursor,
                dialog.select_focus == AiServerSelectFocus::MaxIterations,
                false,
            ));
        }
    }

    if !dialog.status.is_empty() {
        lines.push(Line::from(""));
        let color = if dialog.step == AiServerStep::Testing {
            Color::Yellow
        } else if dialog.status.starts_with("Connexion echouee") {
            Color::Red
        } else {
            Color::DarkGray
        };
        lines.push(Line::from(Span::styled(dialog.status.clone(), Style::default().fg(color))));
    }

    lines.push(Line::from(""));
    let hints = match dialog.step {
        AiServerStep::Configure => "Tab · fleches · Entree tester · Esc annuler",
        AiServerStep::Testing => "Patientez… · Esc annuler",
        AiServerStep::SelectModel => "Tab · fleches modele · Entree appliquer · Esc retour",
    };
    lines.push(Line::from(Span::styled(hints, Style::default().fg(Color::DarkGray))));

    frame.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" /server · Ctrl+Shift+L ")
                    .style(Style::default().fg(state.palette.border)),
            )
            .wrap(Wrap { trim: true })
            .alignment(Alignment::Left),
        popup,
    );
}
