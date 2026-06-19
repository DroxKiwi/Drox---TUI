//! Indicateur de progression bash pendant l'exécution (§12.3).

use std::time::Instant;

use drox_bash::{kind_of_segment, split_command_segments, BashCommandKind};
use drox_types::ToolUseId;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use serde_json::Value;

use super::bash_output::bash_kind_label;

use super::spinner;

/// Lignes éphémères affichées sous le fil pendant un bash actif.
#[derive(Debug, Clone)]
pub struct ActiveBashRun {
    pub id: ToolUseId,
    pub command: String,
    pub primary_kind: BashCommandKind,
    pub started: Instant,
    /// Dernières lignes streamées (stdout+stderr).
    pub partial_output: String,
    pub total_lines: usize,
}

impl ActiveBashRun {
    #[must_use]
    pub fn from_tool_start(id: ToolUseId, arguments: &Value) -> Option<Self> {
        let command = arguments
            .get("command")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())?
            .to_string();
        let primary_kind = split_command_segments(&command)
            .ok()
            .and_then(|segs| segs.first().cloned())
            .map(|s| kind_of_segment(&s))
            .unwrap_or(BashCommandKind::Unknown);
        Some(Self {
            id,
            command,
            primary_kind,
            started: Instant::now(),
            partial_output: String::new(),
            total_lines: 0,
        })
    }

    pub fn apply_progress(&mut self, output: &str, total_lines: usize) {
        self.partial_output = output.to_string();
        self.total_lines = total_lines;
    }

    /// Crée un run bash mode `!` (sans événement agent).
    #[must_use]
    pub fn from_user_command(command: impl Into<String>) -> Self {
        let command = command.into();
        let primary_kind = drox_bash::split_command_segments(&command)
            .ok()
            .and_then(|segs| segs.first().map(|s| drox_bash::kind_of_segment(s)))
            .unwrap_or(drox_bash::BashCommandKind::Unknown);
        Self {
            id: ToolUseId::new(),
            command,
            primary_kind,
            started: Instant::now(),
            partial_output: String::new(),
            total_lines: 0,
        }
    }
}

/// Lignes éphémères affichées sous le fil pendant un bash actif.
#[must_use]
pub fn progress_lines(active: &ActiveBashRun, frame_tick: u8) -> Vec<Line<'static>> {
    let elapsed = active.started.elapsed();
    let spin = spinner::frame(frame_tick);
    let secs = format!("{:.1}s", elapsed.as_secs_f32());
    let kind = bash_kind_label(active.primary_kind);
    let kind_color = kind_color(active.primary_kind);

    let cmd_preview = truncate_middle(&active.command, 72);

    let mut lines = vec![
        Line::from(vec![
            Span::styled(
                format!("◂ bash ({}) ", active.id),
                Style::default().fg(Color::Cyan),
            ),
            Span::styled(
                format!("{spin} en cours "),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(secs, Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!(" · {kind}"),
                Style::default().fg(kind_color),
            ),
        ]),
        Line::from(Span::styled(
            format!("  $ {cmd_preview}"),
            Style::default().fg(Color::Gray),
        )),
    ];

    if active.partial_output.is_empty() {
        lines.push(Line::from(Span::styled(
            "  Running…",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        let tail: Vec<&str> = active
            .partial_output
            .lines()
            .rev()
            .take(5)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        for line in tail {
            let styled = if line.contains('\x1b') {
                super::ansi::line_from_ansi(&format!("  {line}"), Style::default().fg(Color::Gray))
            } else {
                Line::from(Span::styled(
                    format!("  {line}"),
                    Style::default().fg(Color::Gray),
                ))
            };
            lines.push(styled);
        }
        if active.total_lines > 5 {
            lines.push(Line::from(Span::styled(
                format!("  … ~{} lignes", active.total_lines),
                Style::default().fg(Color::DarkGray),
            )));
        }
    }

    lines
}

#[must_use]
pub fn status_hint(active: &ActiveBashRun) -> String {
    let elapsed = active.started.elapsed();
    format!(
        "bash en cours ({:.0}s) — {}",
        elapsed.as_secs_f32(),
        truncate_middle(&active.command, 48)
    )
}

fn kind_color(kind: BashCommandKind) -> Color {
    match kind {
        BashCommandKind::ReadOnly => Color::Green,
        BashCommandKind::Mutating => Color::Yellow,
        BashCommandKind::Network => Color::Cyan,
        BashCommandKind::Destructive => Color::Red,
        BashCommandKind::Unknown => Color::Gray,
        _ => Color::Gray,
    }
}

fn truncate_middle(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let keep = max.saturating_sub(3) / 2;
    let chars: Vec<char> = s.chars().collect();
    let left: String = chars.iter().take(keep).collect();
    let right: String = chars.iter().skip(chars.len().saturating_sub(keep)).collect();
    format!("{left}…{right}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use drox_types::ToolUseId;
    use serde_json::json;

    #[test]
    fn builds_from_arguments() {
        let run = ActiveBashRun::from_tool_start(
            ToolUseId::new(),
            &json!({ "command": "ls -la" }),
        )
        .unwrap();
        assert_eq!(run.primary_kind, BashCommandKind::ReadOnly);
        assert!(!progress_lines(&run, 0).is_empty());
    }

    #[test]
    fn truncates_long_command() {
        let long = "a".repeat(100);
        let t = truncate_middle(&long, 20);
        assert!(t.contains('…'));
        assert!(t.chars().count() <= 20);
    }
}
