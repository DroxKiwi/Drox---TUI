//! Zone de saisie utilisateur.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{AppPhase, AppState, ComposerMode};
use crate::engine::VimComposer;
use crate::widgets::{composer_footer, prompt_modal};

pub fn render(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    model: &str,
    permission_mode: &str,
    plan_mode: bool,
    vim: &VimComposer,
) {
    let blocked = prompt_modal::composer_blocked(state) || state.phase == AppPhase::Running;
    let title = if blocked {
        " Composer (bloqué) "
    } else if !state.llm_configured {
        " Composer — Ctrl+Shift+L configurer IA "
    } else if state.composer_mode == ComposerMode::Bash {
        " Composer (! bash · Entrée exécuter · Esc quitter) "
    } else if state.composer_mode == ComposerMode::Multiline {
        " Composer (multiligne · Entrée envoyer) "
    } else {
        " Composer (Entrée envoyer · ! bash · Shift+Entrée ligne) "
    };

    let border_color = if blocked {
        state.palette.composer_border_blocked
    } else if state.composer_mode == ComposerMode::Bash {
        Color::Yellow
    } else {
        state.palette.composer_border
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .style(Style::default().fg(border_color));

    let placeholder = if blocked {
        "En attente…"
    } else if state.composer_mode == ComposerMode::Bash {
        "Commande shell (sans agent)…"
    } else if state.composer_buffer.is_empty() {
        if !state.llm_configured {
            "Ctrl+Shift+L ou /server — configurez Ollama, puis écrivez…"
        } else {
            "Écrivez votre message, ? aide, @fichier, /help ou ! pour bash…"
        }
    } else {
        ""
    };

    let display = if state.composer_mode == ComposerMode::Bash && !state.composer_buffer.is_empty() {
        format!("!{}", state.composer_buffer)
    } else if state.composer_mode == ComposerMode::Bash {
        "!".to_string()
    } else if state.composer_buffer.is_empty() {
        placeholder.to_string()
    } else {
        state.composer_buffer.clone()
    };

    let style = if state.composer_buffer.is_empty()
        && state.composer_mode != ComposerMode::Bash
        && !blocked
    {
        Style::default()
            .fg(state.palette.header_muted)
            .add_modifier(Modifier::ITALIC)
    } else if state.composer_mode == ComposerMode::Bash {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::White)
    };

    let inner = block.inner(area);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(2), Constraint::Length(1)])
        .split(inner);

    frame.render_widget(
        render_input_line(&display, &style, state, vim, blocked),
        chunks[0],
    );
    composer_footer::render(
        frame,
        chunks[1],
        state,
        model,
        permission_mode,
        plan_mode,
        vim,
    );
    frame.render_widget(block, area);
}

fn render_input_line(
    display: &str,
    style: &Style,
    state: &AppState,
    vim: &VimComposer,
    blocked: bool,
) -> Paragraph<'static> {
    let show_cursor = vim.enabled && !blocked && state.composer_mode != ComposerMode::Bash;
    if !show_cursor || display.is_empty() {
        return Paragraph::new(display.to_string()).style(*style).wrap(Wrap { trim: false });
    }

    let cursor = vim.cursor.index;
    let chars: Vec<char> = display.chars().collect();
    let mut spans = Vec::new();
    for (i, ch) in chars.iter().enumerate() {
        let mut s = *style;
        if i == cursor {
            s = s.add_modifier(Modifier::REVERSED);
        }
        spans.push(Span::styled(ch.to_string(), s));
    }
    if cursor == chars.len() {
        spans.push(Span::styled(
            " ",
            style.add_modifier(Modifier::REVERSED),
        ));
    }
    Paragraph::new(Line::from(spans)).wrap(Wrap { trim: false })
}
