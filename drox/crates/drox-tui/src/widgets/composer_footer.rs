//! Pied du composer — modèle, mode, suggestions contextuelles (leak : `PromptInputFooter.tsx`).

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::{AppPhase, AppState, ComposerMode};
use crate::engine::{VimComposer, VimMode};

/// Rend la ligne sous la zone de saisie (1 ligne de hauteur).
pub fn render(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    model: &str,
    permission_mode: &str,
    plan_mode: bool,
    vim: &VimComposer,
) {
    if area.height == 0 {
        return;
    }

    let model_label = if state.llm_configured {
        shorten_model_name(model)
    } else {
        "IA non configurée".into()
    };
    let mut left = vec![
        Span::styled(
            if state.llm_configured { "● " } else { "⚠ " },
            Style::default().fg(if state.llm_configured {
                state.palette.header_primary
            } else {
                Color::Yellow
            }),
        ),
        Span::styled(
            model_label,
            Style::default()
                .fg(if state.llm_configured {
                    state.palette.header_primary
                } else {
                    Color::Yellow
                })
                .add_modifier(Modifier::BOLD),
        ),
    ];
    if plan_mode {
        left.push(Span::styled(
            " · PLAN",
            Style::default().fg(state.palette.mode_tag),
        ));
    } else {
        left.push(Span::styled(
            format!(" · {permission_mode}"),
            Style::default().fg(state.palette.mode_tag),
        ));
    }
    if vim.enabled {
        let tag = match vim.mode {
            VimMode::Insert => "INSERT",
            VimMode::Normal => "NORMAL",
        };
        left.push(Span::styled(
            format!(" · {tag}"),
            Style::default()
                .fg(state.palette.header_muted)
                .add_modifier(Modifier::BOLD),
        ));
    }

    let hint = footer_hint(state, vim);
    let line = if hint.is_empty() {
        Line::from(left)
    } else {
        // Remplir l'espace entre gauche et droite pour aligner les hints à droite.
        let left_plain = plain_width(&left, &hint, area.width);
        let pad = area.width.saturating_sub(left_plain).max(1);
        let mut spans = left;
        spans.push(Span::raw(" ".repeat(pad as usize)));
        spans.push(Span::styled(
            hint,
            Style::default().fg(state.palette.header_muted),
        ));
        Line::from(spans)
    };

    frame.render_widget(Paragraph::new(line), area);
}

fn footer_hint(state: &AppState, vim: &VimComposer) -> String {
    if state.phase == AppPhase::Running {
        return String::new();
    }
    if !state.llm_configured {
        return "Ctrl+Shift+L · /server — configurer Ollama".into();
    }
    if vim.enabled && vim.mode == VimMode::Normal {
        return "h/j/k/l · i/a · dd · y/p".into();
    }
    if state.composer_suggestions.is_some() {
        return "↑↓ · Tab accepter · Esc fermer".into();
    }
    if !state.composer_buffer.is_empty() {
        return String::new();
    }
    match state.composer_mode {
        ComposerMode::Bash => "Entrée exécuter · Esc quitter bash".into(),
        _ => "? aide · @ fichier · / cmd · ! bash".into(),
    }
}

/// Nom court du modèle (segment après `/` ou `:`).
#[must_use]
pub fn shorten_model_name(model: &str) -> String {
    let trimmed = model.trim();
    if trimmed.is_empty() {
        return "modèle".into();
    }
    trimmed
        .rsplit(['/', ':'])
        .next()
        .unwrap_or(trimmed)
        .to_string()
}

fn plain_width(left: &[Span<'_>], hint: &str, total_width: u16) -> u16 {
    let left_len: usize = left.iter().map(|s| s.content.len()).sum();
    let hint_len = hint.len();
    let sum = left_len + hint_len;
    if sum >= total_width as usize {
        total_width
    } else {
        sum as u16
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortens_ollama_tag() {
        assert_eq!(shorten_model_name("ollama/qwen2.5-coder:7b"), "7b");
        assert_eq!(shorten_model_name("claude-sonnet"), "claude-sonnet");
    }
}
