//! Viewer scrollable pour sortie `web_fetch` (Sprint 4.7).

use drox_types::ToolUseId;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use serde_json::Value;

const LINE_MAX: usize = 200;

#[derive(Debug, Clone)]
pub struct WebFetchViewerState {
    pub tool_id: ToolUseId,
    pub url: String,
    pub status: u64,
    pub kind: String,
    pub content_type: String,
    pub bytes: u64,
    pub truncated: bool,
    pub lines: Vec<String>,
    pub scroll_top: usize,
}

impl WebFetchViewerState {
    #[must_use]
    pub fn from_output(id: ToolUseId, output: &Value) -> Option<Self> {
        let url = output.get("url").and_then(Value::as_str)?.to_string();
        let status = output.get("status").and_then(Value::as_u64).unwrap_or(0);
        let kind = output
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or("?")
            .to_string();
        let content_type = output
            .get("content_type")
            .and_then(Value::as_str)
            .unwrap_or("?")
            .to_string();
        let bytes = output.get("bytes").and_then(Value::as_u64).unwrap_or(0);
        let truncated_dl = output
            .get("truncated_download")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let truncated_txt = output
            .get("truncated_text")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let text = output.get("text").and_then(Value::as_str).unwrap_or("");
        let mut lines = vec![format!("type: {content_type}")];
        for line in text.lines() {
            lines.push(truncate(line, LINE_MAX));
        }
        Some(Self {
            tool_id: id,
            url,
            status,
            kind,
            content_type,
            bytes,
            truncated: truncated_dl || truncated_txt,
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
        let meta = Style::default().fg(Color::DarkGray);
        let body = Style::default().fg(Color::Gray);
        self.lines[self.scroll_top..end]
            .iter()
            .map(|line| {
                if line.starts_with("type:") {
                    Line::from(Span::styled(line.clone(), meta))
                } else {
                    Line::from(Span::styled(line.clone(), body))
                }
            })
            .collect()
    }
}

#[must_use]
pub fn viewer_title(viewer: &WebFetchViewerState) -> String {
    let trunc = if viewer.truncated { " · tronqué" } else { "" };
    format!(
        " web_fetch — HTTP {} {} [{}, {} o{trunc}] ",
        viewer.status,
        truncate_url(&viewer.url, 48),
        viewer.kind,
        viewer.bytes
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

fn truncate_url(url: &str, max: usize) -> String {
    if url.len() <= max {
        return url.to_string();
    }
    format!("{}…", &url[..max])
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
    fn parses_web_body() {
        let out = serde_json::json!({
            "url": "https://example.com",
            "status": 200,
            "kind": "html",
            "content_type": "text/html",
            "bytes": 100,
            "text": "Hello\nWorld"
        });
        let v = WebFetchViewerState::from_output(ToolUseId::new(), &out).unwrap();
        assert_eq!(v.lines.len(), 3);
        assert!(v.lines[0].starts_with("type:"));
    }
}
