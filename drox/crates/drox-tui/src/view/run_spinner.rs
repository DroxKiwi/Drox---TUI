//! Spinner / indicateur d'activité agent pendant un run (§3.7).

use std::time::Instant;

use drox_engine::Phase;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use super::spinner;

/// Lignes éphémères affichées en bas du fil pendant `AppPhase::Running`.
#[must_use]
pub fn activity_lines(
    tick: u8,
    started: Option<Instant>,
    active_phase: Option<Phase>,
    has_streaming: bool,
    bash_active: bool,
) -> Vec<Line<'static>> {
    if bash_active {
        return Vec::new();
    }
    let Some(started) = started else {
        return Vec::new();
    };

    // Pendant le streaming assistant classique, le texte suffit comme feedback.
    if has_streaming && active_phase != Some(Phase::InternalReasoning) {
        return Vec::new();
    }

    let spin = spinner::frame(tick);
    let secs = format!("{:.0}s", started.elapsed().as_secs_f32());
    let (label, color) = activity_label(active_phase);

    vec![Line::from(vec![
        Span::styled(
            spin,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {label} "),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(secs, Style::default().fg(Color::DarkGray)),
    ])]
}

fn activity_label(phase: Option<Phase>) -> (&'static str, Color) {
    match phase {
        Some(Phase::InternalReasoning) => ("réflexion", Color::DarkGray),
        Some(Phase::Reading) | Some(Phase::Analyzing) => ("lecture", Color::Blue),
        Some(Phase::Planning) => ("planification", Color::Magenta),
        Some(Phase::Acting) | Some(Phase::Testing) | Some(Phase::Verifying) => {
            ("exécution", Color::Yellow)
        }
        Some(Phase::Answering) => ("réponse", Color::Cyan),
        _ => ("agent", Color::Cyan),
    }
}

/// Rendu streaming pour la phase réflexion native (Ollama thinking).
#[must_use]
pub fn thinking_stream_lines(text: &str) -> Vec<Line<'static>> {
    let style = Style::default()
        .fg(Color::DarkGray)
        .add_modifier(Modifier::ITALIC);
    if text.trim().is_empty() {
        return Vec::new();
    }
    super::ansi::lines_from_text(text, style, Some("◂ "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shows_agent_spinner() {
        let lines = activity_lines(0, Some(Instant::now()), None, false, false);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].to_string().contains("agent"));
    }

    #[test]
    fn hidden_when_bash_active() {
        assert!(activity_lines(0, Some(Instant::now()), None, false, true).is_empty());
    }
}
