//! Rendu des messages système (`LogEntry::System`).

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SystemKind {
    Info,
    Cancelled,
    Permission,
    Compaction,
    Hook,
}

fn classify(text: &str) -> SystemKind {
    let lower = text.to_ascii_lowercase();
    if lower.contains("annulé") || lower.contains("annule") || lower.contains("cancelled") {
        return SystemKind::Cancelled;
    }
    if lower.contains("permission") || lower.contains("refus") || lower.contains("denied") {
        return SystemKind::Permission;
    }
    if lower.contains("compact") || lower.contains("snip") || lower.contains("contexte") {
        return SystemKind::Compaction;
    }
    if lower.contains("hook") {
        return SystemKind::Hook;
    }
    SystemKind::Info
}

/// Rend un message système avec style selon le contenu.
#[must_use]
pub fn render_system_message(text: &str) -> Vec<Line<'static>> {
    let kind = classify(text);
    let style = match kind {
        SystemKind::Info => Style::default().fg(Color::Gray),
        SystemKind::Cancelled => Style::default().fg(Color::Yellow),
        SystemKind::Permission => Style::default().fg(Color::Red),
        SystemKind::Compaction => Style::default().fg(Color::Blue),
        SystemKind::Hook => Style::default().fg(Color::Cyan),
    };
    let prefix = match kind {
        SystemKind::Cancelled => "⊘ ",
        SystemKind::Permission => "✗ ",
        _ => "· ",
    };
    text.lines()
        .map(|line| {
            Line::from(vec![
                Span::styled(prefix, style.add_modifier(Modifier::BOLD)),
                Span::styled(line.to_string(), style),
            ])
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancelled_style() {
        let lines = render_system_message("Run annulé (Esc).");
        assert!(lines[0].to_string().contains('⊘'));
    }
}
