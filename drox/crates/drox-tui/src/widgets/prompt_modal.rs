//! Modal question / permission (outil `ask_user_question` ou gate Ask).

use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{AppPhase, AppState};
use crate::view::{
    bash_kind_color, bash_kind_label, preview_body_line_count, PermissionPreview,
    PermissionPreviewBody,
};

pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let Some(dialog) = state.prompt.as_ref() else {
        return;
    };

    let is_permission = is_permission_dialog(dialog);
    let has_rich_preview = dialog
        .file_preview
        .as_ref()
        .is_some_and(|p| preview_body_line_count(p) > 0 || p.error.is_some());

    let popup_w = area.width.saturating_sub(2).min(80);
    let extra_h = dialog
        .file_preview
        .as_ref()
        .map(|p| preview_body_line_count(p) as u16)
        .unwrap_or(0);
    let popup_h = if has_rich_preview {
        (14 + extra_h).min(area.height.saturating_sub(1))
    } else {
        area.height.saturating_sub(4).min(18)
    };
    let x = area.x + (area.width.saturating_sub(popup_w)) / 2;
    let y = area.y + (area.height.saturating_sub(popup_h)) / 2;
    let popup = Rect::new(x, y, popup_w, popup_h);

    frame.render_widget(Clear, popup);

    let mut lines: Vec<Line> = Vec::new();
    append_prompt_head(&mut lines, &dialog.question.prompt, is_permission, has_rich_preview);

    if let Some(ref preview) = dialog.file_preview {
        append_permission_preview(&mut lines, preview);
    }

    lines.push(Line::from(""));

    if dialog.question.choices.is_empty() {
        lines.push(Line::from(Span::styled(
            "Réponse libre — Entrée valider · Esc ignorer",
            Style::default().fg(Color::DarkGray),
        )));
        if !dialog.buffer.is_empty() {
            lines.push(Line::from(Span::styled(
                format!("> {}", dialog.buffer),
                Style::default().fg(Color::Yellow),
            )));
        }
    } else {
        for (i, choice) in dialog.question.choices.iter().enumerate() {
            let marker = if i == dialog.choice_index {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };
            lines.push(Line::from(vec![
                Span::styled(format!(" {}. ", i + 1), marker),
                Span::raw(choice.clone()),
            ]));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "↑↓ choisir · Entrée valider · y/n si oui/non · Esc ignorer",
            Style::default().fg(Color::DarkGray),
        )));
        if dialog.question.allow_free_text && !dialog.buffer.is_empty() {
            lines.push(Line::from(Span::styled(
                format!("+ {}", dialog.buffer),
                Style::default().fg(Color::Cyan),
            )));
        }
    }

    let title = if is_permission {
        if state.permission_queue_waiting > 0 {
            format!(" Permission (+{} en attente) ", state.permission_queue_waiting)
        } else {
            " Permission ".to_string()
        }
    } else if state.permission_queue_waiting > 0 {
        format!(" Question (+{} en attente) ", state.permission_queue_waiting)
    } else {
        " Question ".to_string()
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .title_alignment(Alignment::Center)
        .style(Style::default().fg(Color::Magenta));

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, popup);
}

fn is_permission_dialog(dialog: &crate::app::PromptDialog) -> bool {
    dialog.question.choices.len() == 2
        && dialog.question.choices[0].eq_ignore_ascii_case("yes")
        && dialog.question.prompt.contains("Allow this `")
}

fn append_prompt_head(
    lines: &mut Vec<Line>,
    prompt: &str,
    is_permission: bool,
    has_rich_preview: bool,
) {
    if let Some((head, args)) = prompt.split_once("\nArgs:\n") {
        for line in head.lines() {
            lines.push(Line::from(Span::styled(
                line.to_string(),
                Style::default().fg(Color::White),
            )));
        }
        if is_permission && !has_rich_preview && !args.trim().is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Arguments :",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )));
            for line in args.lines().take(12) {
                lines.push(Line::from(Span::styled(
                    format!("  {line}"),
                    Style::default().fg(Color::Cyan),
                )));
            }
        }
    } else {
        for line in prompt.lines() {
            lines.push(Line::from(Span::styled(
                line.to_string(),
                Style::default().fg(Color::White),
            )));
        }
    }
}

fn append_permission_preview(lines: &mut Vec<Line>, preview: &PermissionPreview) {
    if let Some(ref err) = preview.error {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("Preview {} — erreur", preview.tool_name),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(Span::styled(
            format!("  ⚠ {err}"),
            Style::default().fg(Color::Red),
        )));
        return;
    }

    match &preview.kind {
        PermissionPreviewBody::FileDiff { path_label, lines: diff } => {
            lines.push(Line::from(""));
            let path = path_label.as_deref().unwrap_or("(fichier)");
            lines.push(Line::from(Span::styled(
                format!("Preview {} — {path}", preview.tool_name),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )));
            for line in diff {
                lines.push(style_diff_line(line));
            }
        }
        PermissionPreviewBody::Bash(body) => {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Preview bash",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )));
            if let Some(ref desc) = body.description {
                lines.push(Line::from(Span::styled(
                    format!("  {desc}"),
                    Style::default().fg(Color::DarkGray),
                )));
            }
            lines.push(Line::from(Span::styled(
                format!("  $ {}", body.command),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )));
            if body.segments.len() > 1 {
                lines.push(Line::from(Span::styled(
                    format!("  {} sous-commandes :", body.segments.len()),
                    Style::default().fg(Color::DarkGray),
                )));
            }
            for (i, seg) in body.segments.iter().enumerate() {
                let prefix = if body.segments.len() > 1 {
                    format!("  [{}] ", i + 1)
                } else {
                    "  ".to_string()
                };
                let kind = bash_kind_label(seg.kind);
                let color = bash_kind_color(seg.kind);
                lines.push(Line::from(vec![
                    Span::styled(prefix, Style::default().fg(Color::DarkGray)),
                    Span::styled(format!("[{kind}] "), Style::default().fg(color)),
                    Span::styled(seg.text.clone(), Style::default().fg(Color::Cyan)),
                ]));
                if let Some(ref warn) = seg.warning {
                    lines.push(Line::from(Span::styled(
                        format!("      ⚠ {warn}"),
                        Style::default().fg(Color::Red),
                    )));
                }
            }
        }
        PermissionPreviewBody::Url { url } => {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                format!("Preview {} — URL", preview.tool_name),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(Span::styled(
                format!("  {url}"),
                Style::default().fg(Color::Cyan),
            )));
        }
        PermissionPreviewBody::PathAction {
            action,
            path,
            detail,
        } => {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                format!("Preview {} — {action}", preview.tool_name),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(Span::styled(
                format!("  {path}"),
                Style::default().fg(Color::Red),
            )));
            if let Some(d) = detail {
                lines.push(Line::from(Span::styled(
                    format!("  ({d})"),
                    Style::default().fg(Color::DarkGray),
                )));
            }
        }
        PermissionPreviewBody::PathPair {
            source,
            destination,
        } => {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                format!("Preview {}", preview.tool_name),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(Span::styled(
                format!("  {source}"),
                Style::default().fg(Color::Cyan),
            )));
            lines.push(Line::from(Span::styled(
                format!("  → {destination}"),
                Style::default().fg(Color::Green),
            )));
        }
        PermissionPreviewBody::TextBlock { title, lines: body } => {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                title.clone(),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )));
            for line in body {
                lines.push(Line::from(Span::styled(
                    format!("  {line}"),
                    Style::default().fg(Color::Gray),
                )));
            }
        }
    }
}

fn style_diff_line(line: &str) -> Line<'static> {
    let style = if line.starts_with("+++") || line.starts_with("---") {
        Style::default().fg(Color::DarkGray)
    } else if line.starts_with("@@") {
        Style::default().fg(Color::Cyan)
    } else if line.starts_with('+') {
        Style::default().fg(Color::Green)
    } else if line.starts_with('-') {
        Style::default().fg(Color::Red)
    } else {
        Style::default().fg(Color::Gray)
    };
    Line::from(Span::styled(format!("  {line}"), style))
}

#[must_use]
pub fn composer_blocked(state: &AppState) -> bool {
    state.phase == AppPhase::Prompt || state.pending_ask
}
