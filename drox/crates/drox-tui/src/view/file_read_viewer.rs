//! Contenu `file_read` pour le viewer scrollable (leak : expand verbose / `ScrollBox`).

use std::path::Path;

use drox_types::ToolUseId;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use serde_json::Value;

use super::syntax;

const LINE_NUM_WIDTH: usize = 5;

/// Lignes numérotées extraites d'une sortie `file_read`.
#[derive(Debug, Clone)]
pub struct FileReadContent {
    pub path: String,
    pub lines: Vec<(u64, String)>,
    pub truncated: bool,
    pub size_bytes: u64,
}

/// Viewer plein écran pour parcourir une sortie `file_read`.
#[derive(Debug, Clone)]
pub struct FileReadViewerState {
    pub tool_id: ToolUseId,
    pub path: String,
    pub lines: Vec<(u64, String)>,
    pub truncated: bool,
    pub size_bytes: u64,
    pub scroll_top: usize,
    pub lang: String,
}

impl FileReadViewerState {
    #[must_use]
    pub fn from_output(id: ToolUseId, output: &Value) -> Option<Self> {
        let content = parse_file_read_output(output)?;
        let lang = lang_from_path(&content.path);
        Some(Self {
            tool_id: id,
            path: content.path,
            lines: content.lines,
            truncated: content.truncated,
            size_bytes: content.size_bytes,
            scroll_top: 0,
            lang,
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
        let num_style = Style::default().fg(Color::DarkGray);
        let gutter = Style::default().fg(Color::Blue);
        self.lines[self.scroll_top..end]
            .iter()
            .map(|(n, text)| {
                let num = format!("L{n:>LINE_NUM_WIDTH$}| ");
                let code = syntax::highlight_code_line(text, &self.lang);
                let mut spans = vec![
                    Span::styled(num, num_style),
                    Span::styled(" ", gutter),
                ];
                spans.extend(code.spans.into_iter().map(|s| {
                    Span::styled(s.content, s.style)
                }));
                Line::from(spans)
            })
            .collect()
    }
}

#[must_use]
pub fn parse_file_read_output(output: &Value) -> Option<FileReadContent> {
    let path = output.get("path").and_then(Value::as_str)?.to_string();
    let content = output.get("content").and_then(Value::as_str)?;
    let start = output.get("start_line").and_then(Value::as_u64).unwrap_or(1);
    let truncated = output.get("truncated").and_then(Value::as_bool).unwrap_or(false);
    let size = output.get("size_bytes").and_then(Value::as_u64).unwrap_or(0);
    let lines = content
        .lines()
        .enumerate()
        .map(|(i, line)| (start + i as u64, line.to_string()))
        .collect();
    Some(FileReadContent {
        path,
        lines,
        truncated,
        size_bytes: size,
    })
}

#[must_use]
pub fn lang_from_path(path: &str) -> String {
    Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

#[must_use]
pub fn viewer_title(viewer: &FileReadViewerState) -> String {
    let trunc = if viewer.truncated { " · tronqué" } else { "" };
    format!(
        " file_read — {} ({} o, {} ligne(s){trunc}) ",
        viewer.path,
        viewer.size_bytes,
        viewer.lines.len()
    )
}

#[must_use]
pub fn viewer_footer(scroll_top: usize, total: usize, visible: usize) -> String {
    if total == 0 {
        return " Esc fermer ".to_string();
    }
    let end = (scroll_top + visible).min(total);
    format!(
        " {}/{} lignes · PgUp/PgDown · Esc fermer ",
        scroll_top + 1,
        end
    )
}

#[must_use]
pub fn footer_line(text: &str) -> Line<'static> {
    Line::from(Span::styled(
        text.to_string(),
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::ITALIC),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_numbered_lines() {
        let out = serde_json::json!({
            "path": "src/main.rs",
            "content": "fn main() {}\n",
            "start_line": 10,
            "size_bytes": 12,
            "truncated": false
        });
        let content = parse_file_read_output(&out).unwrap();
        assert_eq!(content.lines[0], (10, "fn main() {}".into()));
        assert_eq!(lang_from_path("src/main.rs"), "rs");
    }

    #[test]
    fn scroll_clamps() {
        let mut v = FileReadViewerState {
            tool_id: ToolUseId::new(),
            path: "a.txt".into(),
            lines: (1..=20).map(|n| (n, format!("l{n}"))).collect(),
            truncated: false,
            size_bytes: 0,
            scroll_top: 0,
            lang: String::new(),
        };
        v.scroll_page_down(5, 10);
        assert_eq!(v.scroll_top, 5);
        v.scroll_page_down(100, 10);
        assert_eq!(v.scroll_top, 10);
        v.scroll_page(3, 10);
        assert_eq!(v.scroll_top, 7);
    }
}
