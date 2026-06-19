//! Viewer scrollable pour sortie `web_search` (Sprint 4.8).

use drox_types::ToolUseId;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use serde_json::Value;

const LINE_MAX: usize = 200;
const SNIPPET_MAX: usize = 300;

#[derive(Debug, Clone)]
pub struct WebSearchViewerState {
    pub tool_id: ToolUseId,
    pub query: String,
    pub provider: String,
    pub result_count: usize,
    pub truncated: bool,
    pub lines: Vec<SearchLine>,
    pub scroll_top: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchLine {
    Title { index: usize, text: String },
    Url(String),
    Snippet(String),
    Blank,
}

impl WebSearchViewerState {
    #[must_use]
    pub fn from_output(id: ToolUseId, output: &Value) -> Option<Self> {
        let query = output
            .get("query")
            .and_then(Value::as_str)
            .unwrap_or("?")
            .to_string();
        let provider = output
            .get("provider")
            .and_then(Value::as_str)
            .unwrap_or("?")
            .to_string();
        let truncated = output.get("truncated").and_then(Value::as_bool).unwrap_or(false);
        let results = output.get("results")?.as_array()?;
        let result_count = results.len();
        let mut lines = Vec::new();
        for (i, r) in results.iter().enumerate() {
            let title = r.get("title").and_then(Value::as_str).unwrap_or("?");
            let url = r.get("url").and_then(Value::as_str).unwrap_or("?");
            let snippet = r
                .get("snippet")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim();
            lines.push(SearchLine::Title {
                index: i + 1,
                text: truncate(title, LINE_MAX),
            });
            lines.push(SearchLine::Url(truncate(url, LINE_MAX)));
            if !snippet.is_empty() {
                lines.push(SearchLine::Snippet(truncate(snippet, SNIPPET_MAX)));
            }
            lines.push(SearchLine::Blank);
        }
        if lines.last() == Some(&SearchLine::Blank) {
            lines.pop();
        }
        Some(Self {
            tool_id: id,
            query,
            provider,
            result_count,
            truncated,
            lines,
            scroll_top: 0,
        })
    }

    #[must_use]
    pub fn line_count(&self) -> usize {
        self.lines.len()
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
        self.lines[self.scroll_top..end]
            .iter()
            .map(render_search_line)
            .collect()
    }
}

fn render_search_line(line: &SearchLine) -> Line<'static> {
    match line {
        SearchLine::Title { index, text } => Line::from(vec![
            Span::styled(format!("  {index}. "), Style::default().fg(Color::Magenta)),
            Span::styled(text.clone(), Style::default().fg(Color::White)),
        ]),
        SearchLine::Url(url) => Line::from(Span::styled(
            format!("     {url}"),
            Style::default().fg(Color::Cyan),
        )),
        SearchLine::Snippet(snippet) => Line::from(Span::styled(
            format!("     {snippet}"),
            Style::default().fg(Color::DarkGray),
        )),
        SearchLine::Blank => Line::from(""),
    }
}

#[must_use]
pub fn viewer_title(viewer: &WebSearchViewerState) -> String {
    let trunc = if viewer.truncated { " · tronqué" } else { "" };
    format!(
        " web_search — «{}» — {} résultat(s) [{}]{} ",
        truncate(&viewer.query, 48),
        viewer.result_count,
        viewer.provider,
        trunc
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
    fn parses_results() {
        let out = serde_json::json!({
            "query": "rust ratatui",
            "provider": "test",
            "results": [{
                "title": "Ratatui",
                "url": "https://ratatui.rs",
                "snippet": "TUI crate"
            }],
            "truncated": false
        });
        let v = WebSearchViewerState::from_output(ToolUseId::new(), &out).unwrap();
        assert!(v.line_count() >= 3);
    }
}
