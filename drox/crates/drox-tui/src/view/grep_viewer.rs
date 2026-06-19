//! Viewer scrollable pour sortie `grep` (Sprint 4.5).

use drox_types::ToolUseId;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use serde_json::Value;

const LINE_MAX: usize = 200;

#[derive(Debug, Clone)]
pub struct GrepViewerState {
    pub tool_id: ToolUseId,
    pub match_count: usize,
    pub truncated: bool,
    pub lines: Vec<String>,
    pub scroll_top: usize,
}

impl GrepViewerState {
    #[must_use]
    pub fn from_output(id: ToolUseId, output: &Value) -> Option<Self> {
        let matches = output.get("matches")?.as_array()?;
        let truncated = output.get("truncated").and_then(Value::as_bool).unwrap_or(false);
        let match_count = matches.len();
        let lines = matches
            .iter()
            .filter_map(|m| {
                let path = m.get("path").and_then(Value::as_str)?;
                let no = m.get("line_number").and_then(Value::as_u64).unwrap_or(0);
                let line = m.get("line").and_then(Value::as_str).unwrap_or("");
                Some(format!("{path}:{no}: {}", truncate(line, LINE_MAX)))
            })
            .collect();
        Some(Self {
            tool_id: id,
            match_count,
            truncated,
            lines,
            scroll_top: 0,
        })
    }

    pub fn scroll_page(&mut self, delta_lines: usize, visible: usize) {
        if delta_lines == 0 || self.lines.is_empty() {
            return;
        }
        let max_top = self.lines.len().saturating_sub(visible.max(1));
        self.scroll_top = self.scroll_top.saturating_sub(delta_lines).min(max_top);
    }

    pub fn scroll_page_down(&mut self, delta_lines: usize, visible: usize) {
        if delta_lines == 0 || self.lines.is_empty() {
            return;
        }
        let max_top = self.lines.len().saturating_sub(visible.max(1));
        self.scroll_top = (self.scroll_top + delta_lines).min(max_top);
    }

    #[must_use]
    pub fn render_lines(&self, visible: usize) -> Vec<Line<'static>> {
        let end = (self.scroll_top + visible).min(self.lines.len());
        let path_style = Style::default().fg(Color::Cyan);
        let text_style = Style::default().fg(Color::Gray);
        self.lines[self.scroll_top..end]
            .iter()
            .map(|line| {
                if let Some((head, rest)) = line.split_once(": ") {
                    if let Some((path, tail)) = head.rsplit_once(':') {
                        if tail.chars().all(|c| c.is_ascii_digit()) {
                            return Line::from(vec![
                                Span::styled(format!("{path}:{tail}:"), path_style),
                                Span::styled(format!(" {rest}"), text_style),
                            ]);
                        }
                    }
                }
                Line::from(Span::styled(line.clone(), text_style))
            })
            .collect()
    }
}

#[must_use]
pub fn viewer_title(viewer: &GrepViewerState) -> String {
    let trunc = if viewer.truncated { " · tronqué" } else { "" };
    format!(
        " grep — {} correspondance(s){trunc} ",
        viewer.match_count
    )
}

#[must_use]
pub fn viewer_footer(scroll_top: usize, total: usize, visible: usize) -> String {
    if total == 0 {
        return " Esc / e fermer ".to_string();
    }
    let end = (scroll_top + visible).min(total);
    format!(
        " {}/{} lignes · PgUp/PgDown · Esc fermer ",
        scroll_top + 1,
        end
    )
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    format!("{}…", &s[..max])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_matches() {
        let out = serde_json::json!({
            "matches": [
                {"path": "a.rs", "line_number": 3, "line": "fn main() {}"}
            ],
            "truncated": false
        });
        let v = GrepViewerState::from_output(ToolUseId::new(), &out).unwrap();
        assert_eq!(v.lines.len(), 1);
        assert!(v.lines[0].contains("a.rs:3:"));
    }
}
