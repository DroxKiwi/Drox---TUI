//! Modal sélecteur `/copy` (réponse complète ou bloc de code).

use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::AppState;
use crate::engine::copy_cmd::CopyChoice;

pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let Some(dialog) = state.copy.as_ref() else {
        return;
    };

    let popup_w = area.width.saturating_sub(2).min(72);
    let visible = dialog.choices.len().min(10) as u16;
    let popup_h = (8 + visible).min(area.height.saturating_sub(2));
    let x = area.x + (area.width.saturating_sub(popup_w)) / 2;
    let y = area.y + (area.height.saturating_sub(popup_h)) / 2;
    let popup = Rect::new(x, y, popup_w, popup_h);

    frame.render_widget(Clear, popup);

    let mut lines = vec![
        Line::from(Span::styled(
            "Copier la réponse",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "Choisir le contenu à copier",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
    ];

    for (i, choice) in dialog.choices.iter().enumerate() {
        let marker = if i == dialog.cursor {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        let (label, desc) = choice_label(choice, &dialog.full_text, &dialog.blocks);
        let line = if desc.is_empty() {
            format!(" {label}")
        } else {
            format!(" {label} — {desc}")
        };
        lines.push(Line::from(Span::styled(line, marker)));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "↑↓ choisir · Entrée copier · w fichier seul · Esc annuler",
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" /copy ")
                .style(Style::default().fg(Color::Cyan)),
        )
        .wrap(Wrap { trim: true })
        .alignment(Alignment::Left);

    frame.render_widget(paragraph, popup);
}

fn choice_label(
    choice: &CopyChoice,
    full_text: &str,
    blocks: &[crate::engine::copy_cmd::CopyCodeBlock],
) -> (String, String) {
    match choice {
        CopyChoice::FullResponse => {
            let lines = full_text.lines().count();
            let chars = full_text.chars().count();
            (
                "Réponse complète".into(),
                format!("{chars} car., {lines} lignes"),
            )
        }
        CopyChoice::CodeBlock(idx) => {
            let block = &blocks[*idx];
            let preview: String = block
                .code
                .lines()
                .next()
                .unwrap_or("")
                .chars()
                .take(48)
                .collect();
            let lines = block.code.lines().count();
            let lang = block.lang.as_deref().unwrap_or("text");
            (
                truncate_preview(&preview),
                format!("{lang}, {lines} lignes"),
            )
        }
        CopyChoice::AlwaysFullResponse => (
            "Toujours copier la réponse complète".into(),
            "ne plus afficher ce sélecteur".into(),
        ),
    }
}

fn truncate_preview(s: &str) -> String {
    if s.chars().count() <= 52 {
        return s.to_string();
    }
    let t: String = s.chars().take(49).collect();
    format!("{t}…")
}
