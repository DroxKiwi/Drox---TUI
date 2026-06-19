//! Conversion `LogEntry` → lignes ratatui stylées.

use std::collections::HashSet;

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use super::log_entry::LogEntry;
use super::markdown;
use drox_types::ToolUseId;

#[must_use]
pub fn style_for_plain_line(line: &str) -> Style {
    if line.starts_with('▸') {
        Style::default().fg(Color::Green)
    } else if line.starts_with('◂') {
        Style::default().fg(Color::Cyan)
    } else if line.starts_with('✗') {
        Style::default().fg(Color::Red)
    } else if line.starts_with("──") {
        Style::default().fg(Color::Blue)
    } else if line.starts_with('·') {
        Style::default().fg(Color::Gray)
    } else if line.starts_with("    L") || line.starts_with("    f ") || line.starts_with("    d ")
        || line.starts_with("    type:")
        || line.starts_with("       http")
        || line.starts_with("       https")
        || (line.starts_with("    ") && line.contains(':'))
        || (line.starts_with("    ") && line.chars().nth(4) == Some('.'))
        || line.starts_with("    [")
        || line.starts_with("    +")
        || line.starts_with("    -")
        || line.starts_with("    →")
        || line.starts_with("    ·")
        || line.starts_with("    réponse")
    {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default().fg(Color::Gray)
    }
}

#[must_use]
pub fn plain_line(line: String) -> Line<'static> {
    let style = style_for_plain_line(&line);
    if line.contains('\x1b') {
        return super::ansi::line_from_ansi(&line, style);
    }
    Line::from(Span::styled(line, style))
}

#[must_use]
pub fn render_entry(entry: &LogEntry, expanded_tools: &HashSet<ToolUseId>) -> Vec<Line<'static>> {
    super::message_router::render_entry(entry, expanded_tools)
}

#[must_use]
pub fn render_entries(
    entries: &[LogEntry],
    expanded_tools: &HashSet<ToolUseId>,
    collapsed_phases: &HashSet<usize>,
) -> Vec<Line<'static>> {
    super::message_router::render_entries(entries, expanded_tools, collapsed_phases)
}

pub fn plan_line(line: String) -> Line<'static> {
    let style = if line.starts_with("── [PLAN]") {
        Style::default()
            .fg(Color::Magenta)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::LightMagenta)
    };
    Line::from(Span::styled(line, style))
}

#[must_use]
pub fn render_streaming(text: &str, thinking: bool) -> Vec<Line<'static>> {
    if thinking {
        super::run_spinner::thinking_stream_lines(text)
    } else {
        markdown::assistant_lines(text)
    }
}
