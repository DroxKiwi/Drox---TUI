//! Viewer scrollable générique (diff, LSP, skill, MCP, rapports).

use drox_types::ToolUseId;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinesViewerStyle {
    Plain,
    UnifiedDiff,
}

#[derive(Debug, Clone)]
pub struct LinesViewerState {
    pub tool_id: ToolUseId,
    pub title: String,
    pub lines: Vec<String>,
    pub scroll_top: usize,
    pub style: LinesViewerStyle,
}

impl LinesViewerState {
    #[must_use]
    pub fn new(
        id: ToolUseId,
        title: impl Into<String>,
        lines: Vec<String>,
        style: LinesViewerStyle,
    ) -> Self {
        Self {
            tool_id: id,
            title: title.into(),
            lines,
            scroll_top: 0,
            style,
        }
    }

    #[must_use]
    pub fn from_diff(
        id: ToolUseId,
        tool: &str,
        path: &str,
        diff: &str,
        tag: &str,
    ) -> Self {
        let title = format!(" {tool} — {path} [{tag}] ");
        let lines: Vec<String> = diff.lines().map(str::to_string).collect();
        Self::new(id, title, lines, LinesViewerStyle::UnifiedDiff)
    }

    #[must_use]
    pub fn from_plain(id: ToolUseId, title: impl Into<String>, lines: Vec<String>) -> Self {
        Self::new(id, title, lines, LinesViewerStyle::Plain)
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
            .map(|line| match self.style {
                LinesViewerStyle::Plain => Line::from(Span::styled(
                    line.clone(),
                    Style::default().fg(Color::Gray),
                )),
                LinesViewerStyle::UnifiedDiff => render_diff_line(line),
            })
            .collect()
    }
}

fn render_diff_line(line: &str) -> Line<'static> {
    let style = if line.starts_with("+++") || line.starts_with("---") {
        Style::default().fg(Color::Cyan)
    } else if line.starts_with('+') {
        Style::default().fg(Color::Green)
    } else if line.starts_with('-') {
        Style::default().fg(Color::Red)
    } else if line.starts_with('@') {
        Style::default().fg(Color::Magenta)
    } else {
        Style::default().fg(Color::Gray)
    };
    Line::from(Span::styled(line.to_string(), style))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_lines_colored() {
        let v = LinesViewerState::from_diff(
            ToolUseId::new(),
            "file_edit",
            "a.rs",
            "--- a\n+++ b\n@@\n-old\n+new",
            "appliqué",
        );
        assert_eq!(v.line_count(), 5);
        let rendered = v.render_lines(10);
        assert!(!rendered.is_empty());
    }
}
