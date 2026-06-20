//! Modal connexion serveur IA (`/server`) — assistant 3 étapes.

use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{
    AiServerSelectFocus, AiServerStep, AppState, AuthTypeChoice, ConfigureField, DeploymentKind,
    PersonalEngineChoice,
};
use crate::engine::{preset_label, CONTEXT_CUSTOM_INDEX, CONTEXT_PRESETS};
use crate::ui::ThemePalette;

fn field_line(
    palette: &ThemePalette,
    label: &str,
    value: &str,
    cursor: usize,
    focused: bool,
    secret: bool,
) -> Line<'static> {
    let display = if secret && !value.is_empty() {
        "*".repeat(value.len())
    } else {
        value.to_string()
    };
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

fn list_line(palette: &ThemePalette, label: &str, selected: bool) -> Line<'static> {
    let marker = if selected { ">" } else { " " };
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

fn button_line(palette: &ThemePalette, label: &str, focused: bool) -> Line<'static> {
    let style = if focused {
        Style::default()
            .fg(palette.selection_fg)
            .bg(palette.accent_bright)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(palette.text_muted)
    };
    Line::from(Span::styled(format!(" {label} "), style))
}

fn auth_type_line(palette: &ThemePalette, auth: AuthTypeChoice, focused: bool) -> Line<'static> {
    let value = auth.label();
    let mut spans = vec![Span::styled(
        "Auth: ",
        Style::default().fg(palette.header_muted),
    )];
    if focused {
        spans.push(Span::styled(
            value.to_string(),
            Style::default()
                .fg(palette.selection_fg)
                .bg(palette.accent_bright)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            "  <-/->",
            Style::default().fg(palette.header_muted),
        ));
    } else {
        spans.push(Span::styled(
            value.to_string(),
            Style::default().fg(palette.text_muted),
        ));
    }
    Line::from(spans)
}

fn context_line(palette: &ThemePalette, dialog: &crate::app::AiServerDialog, focused: bool) -> Line<'static> {
    if dialog.context_preset_index == CONTEXT_CUSTOM_INDEX {
        return field_line(
            palette,
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
        Style::default().fg(palette.header_muted),
    )];
    if focused {
        spans.push(Span::styled(
            value,
            Style::default()
                .fg(palette.selection_fg)
                .bg(palette.accent_bright)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            "  <-/->",
            Style::default().fg(palette.header_muted),
        ));
    } else {
        spans.push(Span::styled(
            value,
            Style::default().fg(palette.text_muted),
        ));
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

    let p = &state.palette;
    let mut lines = vec![
        Line::from(Span::styled(
            "CONNEXION SERVEUR IA",
            Style::default()
                .fg(p.header_primary)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            step_title(dialog.step),
            Style::default().fg(p.header_muted),
        )),
        Line::from(""),
    ];

    match dialog.step {
        AiServerStep::ChooseDeployment => {
            for (i, kind) in DeploymentKind::ALL.iter().enumerate() {
                lines.push(list_line(p, kind.label(), i == dialog.list_cursor));
            }
        }
        AiServerStep::ChoosePersonalEngine => {
            for (i, engine) in PersonalEngineChoice::ALL.iter().enumerate() {
                lines.push(list_line(p, engine.label(), i == dialog.list_cursor));
            }
        }
        AiServerStep::ChooseCloudProvider => {
            for (i, provider) in crate::app::CloudProviderChoice::ALL.iter().enumerate() {
                lines.push(list_line(p, provider.label(), i == dialog.list_cursor));
            }
        }
        AiServerStep::ConfigureConnection | AiServerStep::Testing => {
            lines.push(field_line(
                p,
                "URL",
                &dialog.server,
                dialog.server_cursor,
                dialog.configure_focus == ConfigureField::Url
                    && dialog.step == AiServerStep::ConfigureConnection,
                false,
            ));
            lines.push(auth_type_line(
                p,
                dialog.auth_type,
                dialog.configure_focus == ConfigureField::AuthType
                    && dialog.step == AiServerStep::ConfigureConnection,
            ));
            if dialog.auth_type == AuthTypeChoice::ApiKeyHeader {
                lines.push(field_line(
                    p,
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
                    p,
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
                    Style::default().fg(p.header_muted),
                )));
                for (name, _) in &dialog.extra_headers {
                    lines.push(Line::from(Span::styled(
                        format!("  · {name}"),
                        Style::default().fg(p.text_muted),
                    )));
                }
            }
            lines.push(field_line(
                p,
                "Header + nom",
                &dialog.extra_header_name,
                dialog.extra_header_name_cursor,
                dialog.configure_focus == ConfigureField::ExtraHeaderName
                    && dialog.step == AiServerStep::ConfigureConnection,
                false,
            ));
            lines.push(field_line(
                p,
                "Header + valeur",
                &dialog.extra_header_value,
                dialog.extra_header_value_cursor,
                dialog.configure_focus == ConfigureField::ExtraHeaderValue
                    && dialog.step == AiServerStep::ConfigureConnection,
                false,
            ));
            lines.push(button_line(
                p,
                "[+] Ajouter header",
                dialog.configure_focus == ConfigureField::AddExtraHeader
                    && dialog.step == AiServerStep::ConfigureConnection,
            ));
            lines.push(Line::from(""));
            let testing = dialog.step == AiServerStep::Testing;
            lines.push(button_line(
                p,
                if testing {
                    " Tester connexion… "
                } else {
                    " Tester connexion "
                },
                dialog.configure_focus == ConfigureField::TestButton && !testing,
            ));
            lines.push(button_line(
                p,
                " <- Retour ",
                dialog.configure_focus == ConfigureField::BackButton
                    && dialog.step == AiServerStep::ConfigureConnection,
            ));
        }
        AiServerStep::SelectModel => {
            lines.push(Line::from(Span::styled(
                "Modeles disponibles",
                Style::default().fg(p.header_primary),
            )));
            lines.push(Line::from(""));
            let max = dialog.models.len().min(8);
            let start = dialog.model_cursor.saturating_sub(max / 2);
            let end = (start + max).min(dialog.models.len());
            for (i, name) in dialog.models[start..end].iter().enumerate() {
                let idx = start + i;
                lines.push(list_line(
                    p,
                    name,
                    idx == dialog.model_cursor
                        && dialog.select_focus == AiServerSelectFocus::ModelList,
                ));
            }
            if dialog.models.len() > max {
                lines.push(Line::from(Span::styled(
                    format!("… {} modele(s) au total", dialog.models.len()),
                    Style::default().fg(p.header_muted),
                )));
            }
            lines.push(Line::from(""));
            lines.push(context_line(
                p,
                dialog,
                dialog.select_focus == AiServerSelectFocus::NumCtx,
            ));
            lines.push(field_line(
                p,
                "Max iterations",
                &dialog.max_iterations,
                dialog.max_iterations_cursor,
                dialog.select_focus == AiServerSelectFocus::MaxIterations,
                false,
            ));
            lines.push(Line::from(""));
            lines.push(button_line(
                p,
                " Nouvelle configuration ",
                dialog.select_focus == AiServerSelectFocus::ResetWizard,
            ));
        }
        AiServerStep::ConfirmReset => {
            lines.push(Line::from(Span::styled(
                "Recommencer la configuration ?",
                Style::default()
                    .fg(p.header_primary)
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "L'assistant repartira de l'etape 1 (perso ou cloud).",
                Style::default().fg(p.text_muted),
            )));
            lines.push(Line::from(Span::styled(
                "La connexion actuelle reste active tant que la nouvelle n'est pas validee.",
                Style::default().fg(p.warning),
            )));
            lines.push(Line::from(""));
            lines.push(button_line(p, " Confirmer ", true));
            lines.push(button_line(p, " <- Annuler (Esc) ", false));
        }
    }

    if !dialog.status.is_empty() {
        lines.push(Line::from(""));
        let color = if dialog.step == AiServerStep::Testing {
            p.warning
        } else if dialog.status.starts_with("Connexion echouee") {
            p.error
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
        AiServerStep::ChooseDeployment
        | AiServerStep::ChoosePersonalEngine
        | AiServerStep::ChooseCloudProvider => "fleches · Entree · Esc annuler/retour",
        AiServerStep::ConfigureConnection => "Tab · <-/-> auth · Entree tester · Esc retour",
        AiServerStep::Testing => "Patientez… · Esc annuler",
        AiServerStep::SelectModel => "Tab · fleches · Entree appliquer · Esc retour connexion",
        AiServerStep::ConfirmReset => "Entree confirmer · Esc annuler",
    };
    lines.push(Line::from(Span::styled(
        hints,
        Style::default().fg(p.header_muted),
    )));

    frame.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" /server · Ctrl+Shift+L ")
                    .style(
                        Style::default()
                            .fg(p.border)
                            .bg(p.bg_panel),
                    ),
            )
            .wrap(Wrap { trim: true })
            .alignment(Alignment::Left),
        popup,
    );
}
