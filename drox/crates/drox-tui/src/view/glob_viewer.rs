//! Viewer scrollable pour sortie `glob` (Sprint 4.6).

use drox_types::ToolUseId;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlobEntryKind {
    File,
    Dir,
}

#[derive(Debug, Clone)]
pub struct GlobViewerState {
    pub tool_id: ToolUseId,
    pub pattern: Option<String>,
    pub file_count: usize,
    pub dir_count: usize,
    pub truncated: bool,
    pub entries: Vec<(GlobEntryKind, String)>,
    pub scroll_top: usize,
}

impl GlobViewerState {
    #[must_use]
    pub fn from_output(id: ToolUseId, output: &Value, pattern: Option<String>) -> Option<Self> {
        let files = output.get("files")?.as_array()?;
        let dirs = output
            .get("directories")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let truncated = output.get("truncated").and_then(Value::as_bool).unwrap_or(false);
        let mut entries = Vec::new();
        for f in files {
            if let Some(p) = f.as_str() {
                entries.push((GlobEntryKind::File, p.to_string()));
            }
        }
        for d in dirs {
            if let Some(p) = d.as_str() {
                entries.push((GlobEntryKind::Dir, p.to_string()));
            }
        }
        Some(Self {
            tool_id: id,
            pattern,
            file_count: files.len(),
            dir_count: dirs.len(),
            truncated,
            entries,
            scroll_top: 0,
        })
    }

    #[must_use]
    pub fn line_count(&self) -> usize {
        self.entries.len()
    }

    pub fn scroll_page(&mut self, delta_lines: usize, visible: usize) {
        if delta_lines == 0 || self.entries.is_empty() {
            return;
        }
        let max_top = self.entries.len().saturating_sub(visible.max(1));
        self.scroll_top = self.scroll_top.saturating_sub(delta_lines).min(max_top);
    }

    pub fn scroll_page_down(&mut self, delta_lines: usize, visible: usize) {
        if delta_lines == 0 || self.entries.is_empty() {
            return;
        }
        let max_top = self.entries.len().saturating_sub(visible.max(1));
        self.scroll_top = (self.scroll_top + delta_lines).min(max_top);
    }

    #[must_use]
    pub fn render_lines(&self, visible: usize) -> Vec<Line<'static>> {
        let end = (self.scroll_top + visible).min(self.entries.len());
        self.entries[self.scroll_top..end]
            .iter()
            .map(|(kind, path)| {
                let (tag, tag_style, path_style) = match kind {
                    GlobEntryKind::File => (
                        "f",
                        Style::default().fg(Color::Green),
                        Style::default().fg(Color::Gray),
                    ),
                    GlobEntryKind::Dir => (
                        "d",
                        Style::default().fg(Color::Blue),
                        Style::default().fg(Color::Cyan),
                    ),
                };
                Line::from(vec![
                    Span::styled(format!("  {tag} "), tag_style),
                    Span::styled(path.clone(), path_style),
                ])
            })
            .collect()
    }
}

#[must_use]
pub fn viewer_title(viewer: &GlobViewerState) -> String {
    let trunc = if viewer.truncated { " · tronqué" } else { "" };
    let pattern = viewer
        .pattern
        .as_deref()
        .map(|p| format!(" `{p}`"))
        .unwrap_or_default();
    format!(
        " glob{pattern} — {} fichier(s), {} dossier(s){trunc} ",
        viewer.file_count, viewer.dir_count
    )
}

#[must_use]
pub fn viewer_footer(scroll_top: usize, total: usize, visible: usize) -> String {
    if total == 0 {
        return " Esc / e fermer ".to_string();
    }
    let end = (scroll_top + visible).min(total);
    format!(
        " {}/{} entrées · PgUp/PgDown · Esc fermer ",
        scroll_top + 1,
        end
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_files_and_dirs() {
        let out = serde_json::json!({
            "files": ["a.rs"],
            "directories": ["src"],
            "truncated": false
        });
        let v = GlobViewerState::from_output(ToolUseId::new(), &out, Some("*.rs".into())).unwrap();
        assert_eq!(v.entries.len(), 2);
        assert_eq!(v.entries[0].0, GlobEntryKind::File);
        assert_eq!(v.entries[1].0, GlobEntryKind::Dir);
    }
}
