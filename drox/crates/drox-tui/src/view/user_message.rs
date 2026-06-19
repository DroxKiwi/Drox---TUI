//! Classification et rendu des messages utilisateur (leak : `UserTextMessage.tsx`).

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use crate::engine::image_paste::parse_image_refs;

/// Type de message utilisateur affiché dans le fil.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserMessageKind {
    /// Commande slash (`/compact`, `/help`, …).
    SlashCommand,
    /// Texte avec références `[Image #N]`.
    WithImages,
    /// Mode bash intégré (`!cmd`) — entrée dédiée `BashModeInput`.
    Bash,
    /// Prompt texte standard.
    Text,
}

/// Déduit le type d'affichage à partir du texte stocké dans `LogEntry::User`.
#[must_use]
pub fn classify_user_text(text: &str) -> UserMessageKind {
    let trimmed = text.trim();
    if trimmed.starts_with('!') {
        return UserMessageKind::Bash;
    }
    if trimmed.starts_with('/') {
        return UserMessageKind::SlashCommand;
    }
    if !parse_image_refs(text).is_empty() {
        return UserMessageKind::WithImages;
    }
    UserMessageKind::Text
}

/// Rend un message utilisateur en lignes ratatui stylées.
#[must_use]
pub fn render_user_message(text: &str) -> Vec<Line<'static>> {
    match classify_user_text(text) {
        UserMessageKind::SlashCommand => render_slash_command(text),
        UserMessageKind::WithImages => render_with_images(text),
        UserMessageKind::Bash | UserMessageKind::Text => render_plain_user(text),
    }
}

fn render_plain_user(text: &str) -> Vec<Line<'static>> {
    let style = Style::default().fg(Color::Green);
    text.lines()
        .map(|line| {
            Line::from(vec![
                Span::styled("▸ ", style.add_modifier(Modifier::BOLD)),
                Span::styled(line.to_string(), style),
            ])
        })
        .collect()
}

fn render_slash_command(text: &str) -> Vec<Line<'static>> {
    let style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    vec![Line::from(vec![
        Span::styled("▸ ", style),
        Span::styled(text.trim().to_string(), style),
    ])]
}

/// Surligne les pilules `[Image #N]` dans le texte utilisateur.
fn render_with_images(text: &str) -> Vec<Line<'static>> {
    let base = Style::default().fg(Color::Green);
    let pill = Style::default()
        .fg(Color::Magenta)
        .add_modifier(Modifier::BOLD);
    let mut spans = vec![Span::styled("▸ ", base.add_modifier(Modifier::BOLD))];
    let mut rest = text;
    while let Some(idx) = rest.find("[Image #") {
        if idx > 0 {
            spans.push(Span::styled(rest[..idx].to_string(), base));
        }
        rest = &rest[idx..];
        let close = rest.find(']').map(|i| i + 1).unwrap_or(rest.len());
        spans.push(Span::styled(rest[..close].to_string(), pill));
        rest = &rest[close..];
    }
    if !rest.is_empty() {
        spans.push(Span::styled(rest.to_string(), base));
    }
    vec![Line::from(spans)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_slash() {
        assert_eq!(
            classify_user_text("/compact"),
            UserMessageKind::SlashCommand
        );
    }

    #[test]
    fn classifies_image_ref() {
        assert_eq!(
            classify_user_text("voir [Image #1]"),
            UserMessageKind::WithImages
        );
    }
}
