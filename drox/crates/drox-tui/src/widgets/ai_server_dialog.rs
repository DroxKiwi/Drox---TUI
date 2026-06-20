//! Modal connexion serveur IA (`/server`) — assistant 3 étapes.

use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{
    AiServerSelectFocus, AiServerStep, AppState, AuthTypeChoice, ConfigureField, DeploymentKind,
    PersonalEngineChoice,
};
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

fn list_line(label: &str, selected: bool, accent: Color) -> Line<'static> {
    let marker = if selected { ">" } else { " " };
    let style = if selected {
        Style::default()
            .fg(Color::Black)
            .bg(accent)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    };
    Line::from(Span::styled(format!(" {marker} {label} "), style))
}

fn button_line(label: &str, focused: bool, accent: Color) -> Line<'static> {
    let style = if focused {
        Style::default()
            .fg(Color::Black)
            .bg(accent)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    };
    Line::from(Span::styled(format!(" {label} "), style))
}

fn auth_type_line(auth: AuthTypeChoice, focused: bool, accent: Color) -> Line<'static> {
    let value = auth.label();
    let mut spans = vec![Span::styled("Auth: ", Style::default().fg(Color::DarkGray))];
    if focused {
        spans.push(Span::styled(
            value.to_string(),
            Style::default()
                .fg(Color::Black)
                .bg(accent)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            "  <-/->",
            Style::default().fg(Color::DarkGray),
        ));
    } else {
        spans.push(Span::styled(value.to_string(), Style::default().fg(Color::Gray)));
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
    let mut spans = vec![Span::styled(
        "Context max: ",
        Style::default().fg(Color::DarkGray),
    )];
    if focused {
        spans.push(Span::styled(
            value,
            Style::default()
                .fg(Color::Black)
                .bg(accent)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            "  <-/->",
            Style::default().fg(Color::DarkGray),
        ));
    } else {
        spans.push(Span::styled(value, Style::default().fg(Color::Gray)));
    }
    Line::from(spans)
}

fn step_title(step: AiServerStep) -> &'static str {
    match step {
        AiServerStep::ChooseDeployment => "Etape 1/3 — Perso ou cloud",
        AiServerStep::ChoosePersonalEngine => "Etape 2/3 — Moteur d'inference",
        AiServerStep::ChooseCloudProvider => "Etape 2/3 — Prestataire cloud",
        AiServerStep::ConfigureConnection => "Etape 3/3 — Connexion",
        AiServerStep::Testing => "Test connexion",
        AiServerStep::SelectModel => "Configuration modele",
        AiServerStep::ConfirmReset => "Nouvelle configuration",
    }
}

pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let Some(dialog) = state.ai_server.as_ref() else {
        return;
    };

    let popup_w = area.width.saturating_sub(4).min(76);
    let popup_h = 28u16.min(area.height.saturating_sub(4));
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
            step_title(dialog.step),
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
    ];

    match dialog.step {
        AiServerStep::ChooseDeployment => {
            for (i, kind) in DeploymentKind::ALL.iter().enumerate() {
                lines.push(list_line(kind.label(), i == dialog.list_cursor, accent));
            }
        }
        AiServerStep::ChoosePersonalEngine => {
            for (i, engine) in PersonalEngineChoice::ALL.iter().enumerate() {
                lines.push(list_line(engine.label(), i == dialog.list_cursor, accent));
            }
        }
        AiServerStep::ChooseCloudProvider => {
            for (i, provider) in crate::app::CloudProviderChoice::ALL.iter().enumerate() {
                lines.push(list_line(provider.label(), i == dialog.list_cursor, accent));
            }
        }
        AiServerStep::ConfigureConnection | AiServerStep::Testing => {
            lines.push(field_line(
                "URL",
                &dialog.server,
                dialog.server_cursor,
                dialog.configure_focus == ConfigureField::Url
                    && dialog.step == AiServerStep::ConfigureConnection,
                false,
            ));
            lines.push(auth_type_line(
                dialog.auth_type,
                dialog.configure_focus == ConfigureField::AuthType
                    && dialog.step == AiServerStep::ConfigureConnection,
                accent,
            ));
            if dialog.auth_type == AuthTypeChoice::ApiKeyHeader {
                lines.push(field_line(
                    "Nom header",
                    &dialog.auth_header_name,
                    dialog.auth_header_name_cursor,
                    dialog.configure_focus == ConfigureField::AuthHeaderName
                        && dialog.step == AiServerStep::ConfigureConnection,
                    false,
                ));
            }
            if dialog.auth_type != AuthTypeChoice::None {
                lines.push(field_line(
                    "Token / cle",
                    &dialog.auth_token,
                    dialog.auth_token_cursor,
                    dialog.configure_focus == ConfigureField::AuthToken
                        && dialog.step == AiServerStep::ConfigureConnection,
                    true,
                ));
            }
            if !dialog.extra_headers.is_empty() {
                lines.push(Line::from(Span::styled(
                    "Headers supplementaires:",
                    Style::default().fg(Color::DarkGray),
                )));
                for (name, _) in &dialog.extra_headers {
                    lines.push(Line::from(Span::styled(
                        format!("  · {name}"),
                        Style::default().fg(Color::Gray),
                    )));
                }
            }
            lines.push(field_line(
                "Header + nom",
                &dialog.extra_header_name,
                dialog.extra_header_name_cursor,
                dialog.configure_focus == ConfigureField::ExtraHeaderName
                    && dialog.step == AiServerStep::ConfigureConnection,
                false,
            ));
            lines.push(field_line(
                "Header + valeur",
                &dialog.extra_header_value,
                dialog.extra_header_value_cursor,
                dialog.configure_focus == ConfigureField::ExtraHeaderValue
                    && dialog.step == AiServerStep::ConfigureConnection,
                false,
            ));
            lines.push(button_line(
                "[+] Ajouter header",
                dialog.configure_focus == ConfigureField::AddExtraHeader
                    && dialog.step == AiServerStep::ConfigureConnection,
                accent,
            ));
            lines.push(Line::from(""));
            let testing = dialog.step == AiServerStep::Testing;
            lines.push(button_line(
                if testing {
                    " Tester connexion… "
                } else {
                    " Tester connexion "
                },
                dialog.configure_focus == ConfigureField::TestButton && !testing,
                accent,
            ));
            lines.push(button_line(
                " <- Retour ",
                dialog.configure_focus == ConfigureField::BackButton
                    && dialog.step == AiServerStep::ConfigureConnection,
                accent,
            ));
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
                lines.push(list_line(
                    name,
                    idx == dialog.model_cursor
                        && dialog.select_focus == AiServerSelectFocus::ModelList,
                    accent,
                ));
            }
            if dialog.models.len() > max {
                lines.push(Line::from(Span::styled(
                    format!("… {} modele(s) au total", dialog.models.len()),
                    Style::default().fg(Color::DarkGray),
                )));
            }
            lines.push(Line::from(""));
            lines.push(context_line(
                dialog,
                dialog.select_focus == AiServerSelectFocus::NumCtx,
                accent,
            ));
            lines.push(field_line(
                "Max iterations",
                &dialog.max_iterations,
                dialog.max_iterations_cursor,
                dialog.select_focus == AiServerSelectFocus::MaxIterations,
                false,
            ));
            lines.push(Line::from(""));
            lines.push(button_line(
                " Nouvelle configuration ",
                dialog.select_focus == AiServerSelectFocus::ResetWizard,
                accent,
            ));
        }
        AiServerStep::ConfirmReset => {
            lines.push(Line::from(Span::styled(
                "Recommencer la configuration ?",
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "L'assistant repartira de l'etape 1 (perso ou cloud).",
                Style::default().fg(Color::Gray),
            )));
            lines.push(Line::from(Span::styled(
                "La connexion actuelle reste active tant que la nouvelle n'est pas validee.",
                Style::default().fg(Color::Yellow),
            )));
            lines.push(Line::from(""));
            lines.push(button_line(" Confirmer ", true, accent));
            lines.push(button_line(" <- Annuler (Esc) ", false, accent));
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
        lines.push(Line::from(Span::styled(
            dialog.status.clone(),
            Style::default().fg(color),
        )));
    }

    lines.push(Line::from(""));
    let hints = match dialog.step {
        AiServerStep::ChooseDeployment
        | AiServerStep::ChoosePersonalEngine
        | AiServerStep::ChooseCloudProvider => "fleches · Entree · Esc annuler/retour",
        AiServerStep::ConfigureConnection => "Tab · <-/-> auth · Entree tester · Esc retour",
        AiServerStep::Testing => "Patientez… · Esc annuler",
        AiServerStep::SelectModel => "Tab · fleches · Entree appliquer · Esc retour connexion",
        AiServerStep::ConfirmReset => "Entree confirmer · Esc annuler",
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
