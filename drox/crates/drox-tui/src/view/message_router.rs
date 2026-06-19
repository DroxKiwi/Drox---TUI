//! Routeur fil → widgets ratatui (leak : `Messages.tsx`, `MessageRow.tsx`).

use std::collections::HashSet;

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use drox_engine::Phase;
use drox_types::ToolUseId;

use super::log_entry::LogEntry;
use super::api_error;
use super::markdown;
use super::render::{plain_line, plan_line};
use super::system_message;
use super::user_message;

/// Outils toujours rendus en détail (start + finish), même sans expand `e`.
const DETAIL_TOOLS: &[&str] = &[
    "bash",
    "file_read",
    "grep",
    "glob",
    "web_fetch",
    "web_search",
    "notebook_edit",
    "lsp",
];

/// Rend une séquence d'entrées (avec groupement tool start/finish optionnel).
#[must_use]
pub fn render_entries(
    entries: &[LogEntry],
    expanded_tools: &HashSet<ToolUseId>,
    collapsed_phases: &HashSet<usize>,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let mut i = 0usize;
    while i < entries.len() {
        if let Some(next_i) =
            try_render_collapsed_phase(entries, i, collapsed_phases, &mut lines)
        {
            i = next_i;
            continue;
        }
        if let Some(next_i) = try_render_grouped_tool(entries, i, expanded_tools, &mut lines) {
            i = next_i;
            continue;
        }
        lines.extend(render_entry(&entries[i], expanded_tools));
        i += 1;
    }
    lines
}

/// Rend une entrée unique.
#[must_use]
pub fn render_entry(entry: &LogEntry, expanded_tools: &HashSet<ToolUseId>) -> Vec<Line<'static>> {
    match entry {
        LogEntry::User { text } => user_message::render_user_message(text),
        LogEntry::System { text } => system_message::render_system_message(text),
        LogEntry::Assistant { text } => markdown::assistant_lines(text),
        LogEntry::PhaseOpen { phase } => render_phase_open(*phase, false),
        LogEntry::PhaseLine { text } => render_phase_line(text),
        LogEntry::Error { text } => render_error(text),
        LogEntry::ApiError(view) => api_error::render_api_error(view),
        LogEntry::RunCancelled { reason } => render_cancelled(reason),
        LogEntry::PlanActivated { .. }
        | LogEntry::PlanDocument { .. }
        | LogEntry::PlanApproval { .. } => entry
            .display_lines(expanded_tools)
            .into_iter()
            .map(plan_line)
            .collect(),
        _ => {
            if let LogEntry::ToolFinish {
                name,
                id,
                output,
                is_error,
            } = entry
            {
                render_tool_finish(name, id, output, *is_error, expanded_tools)
            } else {
                entry
                    .display_lines(expanded_tools)
                    .into_iter()
                    .map(plain_line)
                    .collect()
            }
        }
    }
}

/// Nombre de lignes produites par une entrée (pour scroll / layout).
#[must_use]
pub fn entry_line_count(entry: &LogEntry, expanded_tools: &HashSet<ToolUseId>) -> usize {
    render_entry(entry, expanded_tools).len().max(1)
}

fn render_phase_line(text: &str) -> Vec<Line<'static>> {
    let style = Style::default()
        .fg(Color::DarkGray)
        .add_modifier(Modifier::ITALIC);
    text.lines()
        .map(|line| {
            Line::from(vec![
                Span::styled("  ", style),
                Span::styled(line.to_string(), style),
            ])
        })
        .collect()
}

fn render_cancelled(reason: &str) -> Vec<Line<'static>> {
    let style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    vec![Line::from(vec![
        Span::styled("⊘ ", style),
        Span::styled(format!("run annulé — {reason}"), style),
    ])]
}

fn render_tool_finish(
    name: &str,
    id: &ToolUseId,
    output: &serde_json::Value,
    is_error: bool,
    expanded_tools: &HashSet<ToolUseId>,
) -> Vec<Line<'static>> {
    let lines = LogEntry::ToolFinish {
        id: id.clone(),
        name: name.to_string(),
        output: output.clone(),
        is_error,
    }
    .display_lines(expanded_tools);
    let style = if is_error {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    lines
        .into_iter()
        .map(|line| {
            if is_error {
                Line::from(Span::styled(line, style))
            } else if line.contains('\x1b') {
                super::ansi::line_from_ansi(&line, style)
            } else {
                Line::from(Span::styled(line, style))
            }
        })
        .collect()
}

fn render_phase_open(phase: Phase, collapsed: bool) -> Vec<Line<'static>> {
    let (label, style) = match phase {
        Phase::InternalReasoning => (
            "réflexion",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        ),
        _ => (
            phase.as_marker(),
            Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD),
        ),
    };
    let marker = if collapsed { "▶" } else { "──" };
    vec![Line::from(Span::styled(
        format!("{marker} [{label}] {marker}"),
        style,
    ))]
}

/// Index après le dernier `PhaseLine` consécutif à `open_idx`.
#[must_use]
pub fn phase_block_end(entries: &[LogEntry], open_idx: usize) -> usize {
    let mut i = open_idx + 1;
    while i < entries.len() {
        if matches!(entries[i], LogEntry::PhaseLine { .. }) {
            i += 1;
        } else {
            break;
        }
    }
    i
}

fn count_phase_content_lines(entries: &[LogEntry], open_idx: usize, end: usize) -> usize {
    entries[open_idx + 1..end]
        .iter()
        .filter_map(|e| match e {
            LogEntry::PhaseLine { text } => Some(text.lines().count().max(1)),
            _ => None,
        })
        .sum()
}

fn try_render_collapsed_phase(
    entries: &[LogEntry],
    i: usize,
    collapsed_phases: &HashSet<usize>,
    out: &mut Vec<Line<'static>>,
) -> Option<usize> {
    let LogEntry::PhaseOpen { phase } = &entries[i] else {
        return None;
    };
    if !collapsed_phases.contains(&i) {
        return None;
    }
    let end = phase_block_end(entries, i);
    let hidden = count_phase_content_lines(entries, i, end);
    out.extend(render_phase_open(*phase, true));
    if hidden > 0 {
        let style = Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::ITALIC);
        out.push(Line::from(Span::styled(
            format!("  … {hidden} ligne(s) — e pour développer"),
            style,
        )));
    }
    Some(end)
}

fn render_error(text: &str) -> Vec<Line<'static>> {
    let style = Style::default().fg(Color::Red).add_modifier(Modifier::BOLD);
  let prefix = if text.to_ascii_lowercase().contains("permission")
        || text.to_ascii_lowercase().contains("refus")
    {
        "✗ permission "
    } else {
        "✗ "
    };
    text.lines()
        .map(|line| Line::from(Span::styled(format!("{prefix}{line}"), style)))
        .collect()
}

/// Regroupe `ToolStart` + `ToolFinish` en une ligne compacte si l'outil est « simple ».
fn try_render_grouped_tool(
    entries: &[LogEntry],
    i: usize,
    expanded: &HashSet<ToolUseId>,
    out: &mut Vec<Line<'static>>,
) -> Option<usize> {
    let LogEntry::ToolStart { id, name, arguments } = &entries[i] else {
        return None;
    };
    if DETAIL_TOOLS.contains(&name.as_str()) || expanded.contains(id) {
        return None;
    }
    let Some(LogEntry::ToolFinish {
        id: fid,
        name: fname,
        output,
        is_error,
    }) = entries.get(i + 1)
    else {
        return None;
    };
    if fid != id || fname != name {
        return None;
    }

    let status = if *is_error { "✗" } else { "✓" };
    let style = if *is_error {
        Style::default().fg(Color::Red)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let summary = tool_summary_line(name, arguments, output, *is_error);
    out.push(Line::from(vec![
        Span::styled(format!("▸ {name} ({id}) "), style),
        Span::styled(status.to_string(), style.add_modifier(Modifier::BOLD)),
        Span::styled(format!(" {summary}"), style),
    ]));
    Some(i + 2)
}

fn tool_summary_line(
    name: &str,
    arguments: &serde_json::Value,
    output: &serde_json::Value,
    is_error: bool,
) -> String {
    if is_error {
        return output
            .as_str()
            .or_else(|| output.get("error").and_then(|v| v.as_str()))
            .unwrap_or("erreur")
            .chars()
            .take(80)
            .collect();
    }
    match name {
        "file_write" | "file_edit" | "delete_path" | "copy_path" => arguments
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("…")
            .to_string(),
        "todo_write" => output
            .get("summary")
            .and_then(|v| v.as_str())
            .unwrap_or("ok")
            .to_string(),
        _ => output
            .get("summary")
            .or_else(|| output.get("message"))
            .and_then(|v| v.as_str())
            .map(|s| {
                if s.len() > 60 {
                    format!("{}…", &s[..60])
                } else {
                    s.to_string()
                }
            })
            .unwrap_or_else(|| "ok".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use drox_types::ToolUseId;

    #[test]
    fn groups_simple_tool_pair() {
        let id = ToolUseId::from_string("tu_test".into());
        let entries = vec![
            LogEntry::ToolStart {
                id: id.clone(),
                name: "delete_path".into(),
                arguments: serde_json::json!({"path": "tmp.txt"}),
            },
            LogEntry::ToolFinish {
                id,
                name: "delete_path".into(),
                output: serde_json::json!({"summary": "deleted"}),
                is_error: false,
            },
        ];
        let lines = render_entries(&entries, &HashSet::new(), &HashSet::new());
        assert_eq!(lines.len(), 1);
        assert!(lines[0].to_string().contains('✓'));
    }

    #[test]
    fn collapses_phase_block() {
        let entries = vec![
            LogEntry::PhaseOpen {
                phase: Phase::InternalReasoning,
            },
            LogEntry::PhaseLine {
                text: "ligne 1\nligne 2".into(),
            },
        ];
        let collapsed = HashSet::from([0]);
        let lines = render_entries(&entries, &HashSet::new(), &collapsed);
        assert!(lines.iter().any(|l| l.to_string().contains('▶')));
        assert!(lines.iter().any(|l| l.to_string().contains("2 ligne")));
        let expanded = render_entries(&entries, &HashSet::new(), &HashSet::new());
        assert!(expanded.len() > lines.len());
    }
}
