//! Modal connexion serveur IA (`/server`) — assistant 3 étapes.

use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{
    AiServerSelectFocus, AiServerStep, AppState, AuthTypeChoice, ConfigureField, DeploymentKind,
    PersonalEngineChoice,
};
use crate::engine::{preset_label, CONTEXT_CUSTOM_INDEX, CONTEXT_PRESETS};
use crate::i18n::{self, keys};
use crate::ui::ThemePalette;
use crate::ui::animation;
use crate::widgets::modal_frame;

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
            i18n::t(keys::MODAL_SERVER_FIELD_CONTEXT_MAX),
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
        format!("{}: ", i18n::t(keys::MODAL_SERVER_FIELD_CONTEXT_MAX)),
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
        AiServerStep::ChooseDeployment => i18n::t(keys::MODAL_SERVER_STEP_DEPLOY),
        AiServerStep::ChoosePersonalEngine => i18n::t(keys::MODAL_SERVER_STEP_ENGINE),
        AiServerStep::ChooseCloudProvider => i18n::t(keys::MODAL_SERVER_STEP_CLOUD),
        AiServerStep::ConfigureConnection => i18n::t(keys::MODAL_SERVER_STEP_CONFIGURE),
        AiServerStep::Testing => i18n::t(keys::MODAL_SERVER_STEP_TESTING),
        AiServerStep::SelectModel => i18n::t(keys::MODAL_SERVER_STEP_MODEL),
        AiServerStep::ConfirmReset => i18n::t(keys::MODAL_SERVER_STEP_RESET),
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
            i18n::t(keys::MODAL_SERVER_TITLE),
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
            if let Some(doc) = dialog.configure_doc_url() {
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("{}: ", i18n::t(keys::MODAL_SERVER_FIELD_DOC)),
                        Style::default().fg(p.header_muted),
                    ),
                    Span::styled(doc.to_string(), Style::default().fg(p.accent_bright)),
                ]));
                lines.push(Line::from(""));
            }
            lines.push(field_line(
                p,
                i18n::t(keys::MODAL_SERVER_FIELD_URL),
                &dialog.server,
                dialog.server_cursor,
                dialog.configure_focus == ConfigureField::Url
                    && dialog.step == AiServerStep::ConfigureConnection,
                false,
            ));
            if !dialog.auth_type_locked() {
                lines.push(auth_type_line(
                    p,
                    dialog.auth_type,
                    dialog.configure_focus == ConfigureField::AuthType
                        && dialog.step == AiServerStep::ConfigureConnection,
                ));
            }
            if dialog.auth_type == AuthTypeChoice::ApiKeyHeader {
                lines.push(field_line(
                    p,
                    i18n::t(keys::MODAL_SERVER_FIELD_HEADER_NAME),
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
                    i18n::t(keys::MODAL_SERVER_FIELD_TOKEN),
                    &dialog.auth_token,
                    dialog.auth_token_cursor,
                    dialog.configure_focus == ConfigureField::AuthToken
                        && dialog.step == AiServerStep::ConfigureConnection,
                    true,
                ));
            }
            if !dialog.extra_headers.is_empty() {
                lines.push(Line::from(Span::styled(
                    i18n::t(keys::MODAL_SERVER_FIELD_EXTRA_HEADERS),
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
                i18n::t(keys::MODAL_SERVER_FIELD_HEADER_PLUS_NAME),
                &dialog.extra_header_name,
                dialog.extra_header_name_cursor,
                dialog.configure_focus == ConfigureField::ExtraHeaderName
                    && dialog.step == AiServerStep::ConfigureConnection,
                false,
            ));
            lines.push(field_line(
                p,
                i18n::t(keys::MODAL_SERVER_FIELD_HEADER_PLUS_VALUE),
                &dialog.extra_header_value,
                dialog.extra_header_value_cursor,
                dialog.configure_focus == ConfigureField::ExtraHeaderValue
                    && dialog.step == AiServerStep::ConfigureConnection,
                false,
            ));
            lines.push(button_line(
                p,
                i18n::t(keys::MODAL_SERVER_BTN_ADD_HEADER),
                dialog.configure_focus == ConfigureField::AddExtraHeader
                    && dialog.step == AiServerStep::ConfigureConnection,
            ));
            lines.push(Line::from(""));
            let testing = dialog.step == AiServerStep::Testing;
            lines.push(button_line(
                p,
                if testing {
                    i18n::t(keys::MODAL_SERVER_BTN_TESTING)
                } else {
                    i18n::t(keys::MODAL_SERVER_BTN_TEST)
                },
                dialog.configure_focus == ConfigureField::TestButton && !testing,
            ));
            lines.push(button_line(
                p,
                i18n::t(keys::MODAL_SERVER_BTN_BACK),
                dialog.configure_focus == ConfigureField::BackButton
                    && dialog.step == AiServerStep::ConfigureConnection,
            ));
            if testing {
                lines.push(Line::from(""));
                let bar = animation::handshake_bar(
                    state.ui_frame_tick,
                    14,
                    state.animations_enabled,
                );
                lines.push(Line::from(Span::styled(
                    bar,
                    Style::default().fg(p.warning),
                )));
            }
        }
        AiServerStep::SelectModel => {
            lines.push(Line::from(Span::styled(
                i18n::t(keys::MODAL_SERVER_MODELS_TITLE),
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
                    i18n::tf(keys::MODAL_SERVER_MODELS_TOTAL, &dialog.models.len().to_string()),
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
                i18n::t(keys::MODAL_SERVER_FIELD_MAX_ITER),
                &dialog.max_iterations,
                dialog.max_iterations_cursor,
                dialog.select_focus == AiServerSelectFocus::MaxIterations,
                false,
            ));
            lines.push(Line::from(""));
            lines.push(button_line(
                p,
                i18n::t(keys::MODAL_SERVER_BTN_NEW_CONFIG),
                dialog.select_focus == AiServerSelectFocus::ResetWizard,
            ));
        }
        AiServerStep::ConfirmReset => {
            lines.push(Line::from(Span::styled(
                i18n::t(keys::MODAL_SERVER_RESET_TITLE),
                Style::default()
                    .fg(p.header_primary)
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                i18n::t(keys::MODAL_SERVER_RESET_LINE1),
                Style::default().fg(p.text_muted),
            )));
            lines.push(Line::from(Span::styled(
                i18n::t(keys::MODAL_SERVER_RESET_LINE2),
                Style::default().fg(p.warning),
            )));
            lines.push(Line::from(""));
            lines.push(button_line(p, i18n::t(keys::MODAL_SERVER_BTN_CONFIRM), true));
            lines.push(button_line(p, i18n::t(keys::MODAL_SERVER_BTN_CANCEL), false));
        }
    }

    if !dialog.status.is_empty() {
        lines.push(Line::from(""));
        let color = if dialog.step == AiServerStep::Testing {
            p.warning
        } else if i18n::server_connection_failed(&dialog.status) {
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
        | AiServerStep::ChooseCloudProvider => i18n::t(keys::MODAL_SERVER_HINT_LIST),
        AiServerStep::ConfigureConnection => i18n::t(keys::MODAL_SERVER_HINT_CONFIGURE),
        AiServerStep::Testing => i18n::t(keys::MODAL_SERVER_HINT_TESTING),
        AiServerStep::SelectModel => i18n::t(keys::MODAL_SERVER_HINT_MODEL),
        AiServerStep::ConfirmReset => i18n::t(keys::MODAL_SERVER_HINT_RESET),
    };
    lines.push(Line::from(Span::styled(
        hints,
        Style::default().fg(p.header_muted),
    )));

    frame.render_widget(
        Paragraph::new(lines)
            .block(modal_frame::pip_boy_block_animated(
                i18n::t(keys::MODAL_SERVER_FRAME),
                p,
                state.modal_anim_tick,
                state.animations_enabled,
            ))
            .wrap(Wrap { trim: true })
            .alignment(Alignment::Left),
        popup,
    );
}

/// Cible interactive d'une ligne du modal `/server`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AiServerHit {
    ListItem(usize),
    TestButton,
    BackButton,
    AddHeader,
    ConfirmReset,
    CancelReset,
    Model(usize),
    ResetWizard,
}

struct ContentLine {
    hit: Option<AiServerHit>,
}

fn push_line(lines: &mut Vec<ContentLine>, hit: Option<AiServerHit>) {
    lines.push(ContentLine { hit });
}

/// Index de ligne (dans le paragraphe) → cible clic.
#[must_use]
pub fn hit_at_line(dialog: &crate::app::AiServerDialog, line_index: usize) -> Option<AiServerHit> {
    let mut lines: Vec<ContentLine> = Vec::new();
    push_line(&mut lines, None);
    push_line(&mut lines, None);
    push_line(&mut lines, None);

    match dialog.step {
        AiServerStep::ChooseDeployment => {
            for (i, _) in DeploymentKind::ALL.iter().enumerate() {
                push_line(&mut lines, Some(AiServerHit::ListItem(i)));
            }
        }
        AiServerStep::ChoosePersonalEngine => {
            for (i, _) in PersonalEngineChoice::ALL.iter().enumerate() {
                push_line(&mut lines, Some(AiServerHit::ListItem(i)));
            }
        }
        AiServerStep::ChooseCloudProvider => {
            for (i, _) in crate::app::CloudProviderChoice::ALL.iter().enumerate() {
                push_line(&mut lines, Some(AiServerHit::ListItem(i)));
            }
        }
        AiServerStep::ConfigureConnection | AiServerStep::Testing => {
            push_line(&mut lines, None);
            push_line(&mut lines, None);
            if dialog.auth_type == AuthTypeChoice::ApiKeyHeader {
                push_line(&mut lines, None);
            }
            if dialog.auth_type != AuthTypeChoice::None {
                push_line(&mut lines, None);
            }
            if !dialog.extra_headers.is_empty() {
                push_line(&mut lines, None);
                for _ in &dialog.extra_headers {
                    push_line(&mut lines, None);
                }
            }
            push_line(&mut lines, None);
            push_line(&mut lines, None);
            push_line(&mut lines, Some(AiServerHit::AddHeader));
            push_line(&mut lines, None);
            push_line(&mut lines, Some(AiServerHit::TestButton));
            push_line(&mut lines, Some(AiServerHit::BackButton));
            if dialog.step == AiServerStep::Testing {
                push_line(&mut lines, None);
                push_line(&mut lines, None);
            }
        }
        AiServerStep::SelectModel => {
            push_line(&mut lines, None);
            push_line(&mut lines, None);
            let max = dialog.models.len().min(8);
            let start = dialog.model_cursor.saturating_sub(max / 2);
            let end = (start + max).min(dialog.models.len());
            for i in start..end {
                push_line(&mut lines, Some(AiServerHit::Model(i)));
            }
            if dialog.models.len() > max {
                push_line(&mut lines, None);
            }
            push_line(&mut lines, None);
            push_line(&mut lines, None);
            push_line(&mut lines, None);
            push_line(&mut lines, None);
            push_line(&mut lines, Some(AiServerHit::ResetWizard));
        }
        AiServerStep::ConfirmReset => {
            push_line(&mut lines, None);
            push_line(&mut lines, None);
            push_line(&mut lines, None);
            push_line(&mut lines, None);
            push_line(&mut lines, Some(AiServerHit::ConfirmReset));
            push_line(&mut lines, Some(AiServerHit::CancelReset));
        }
    }

    if !dialog.status.is_empty() {
        push_line(&mut lines, None);
        push_line(&mut lines, None);
    }
    push_line(&mut lines, None);
    push_line(&mut lines, None);

    lines.get(line_index).and_then(|l| l.hit)
}

#[must_use]
pub fn popup_rect(area: Rect) -> Rect {
    modal_frame::centered_popup(area, 76, 28)
}

#[must_use]
pub fn popup_inner(area: Rect) -> Rect {
    modal_frame::bordered_inner(popup_rect(area))
}
