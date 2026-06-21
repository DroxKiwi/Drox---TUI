//! Viewer scrollable générique (diff, LSP, skill, MCP, rapports).

use std::collections::HashMap;

use drox_types::ToolUseId;
use ratatui::text::Line;

use crate::ui::theme::ThemePalette;

use super::diff_render::{
    self, build_diff_line_meta, render_diff_line, word_diff_pair, DiffColors, DiffLineMeta,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinesViewerStyle {
    Plain,
    UnifiedDiff,
}

/// Navigation git workspace (`/diff`) : lignes status cliquables + sélection clavier.
#[derive(Debug, Clone, Default)]
pub struct GitWorkspaceNav {
    pub selected_path: Option<String>,
    /// Index dans `lines` → chemin relatif au dépôt.
    pub status_line_paths: HashMap<usize, String>,
}

impl GitWorkspaceNav {
    #[must_use]
    pub fn index_body(body: &[String], preset_path: Option<String>) -> Self {
        let mut status_line_paths = HashMap::new();
        let mut in_status = false;
        for (i, line) in body.iter().enumerate() {
            if line.as_str() == "— status —" {
                in_status = true;
                continue;
            }
            if line.starts_with('—') && line.ends_with('—') && line.as_str() != "— status —" {
                in_status = false;
                continue;
            }
            if in_status {
                if let Some(path) = parse_status_line_path(line) {
                    status_line_paths.insert(i, path);
                }
            }
        }
        Self {
            selected_path: preset_path,
            status_line_paths,
        }
    }

    #[must_use]
    pub fn path_at_line(&self, line_idx: usize) -> Option<&str> {
        self.status_line_paths.get(&line_idx).map(String::as_str)
    }

    pub fn cycle_selection(&mut self, delta: i32) {
        let mut indices: Vec<usize> = self.status_line_paths.keys().copied().collect();
        indices.sort_unstable();
        if indices.is_empty() {
            return;
        }
        let current_pos = self.selected_path.as_ref().and_then(|sel| {
            indices.iter().position(|&idx| {
                self.status_line_paths
                    .get(&idx)
                    .is_some_and(|p| p == sel)
            })
        });
        let next_pos = match current_pos {
            None => {
                if delta < 0 {
                    indices.len() - 1
                } else {
                    0
                }
            }
            Some(pos) => {
                let len = indices.len() as i32;
                (pos as i32 + delta).rem_euclid(len) as usize
            }
        };
        self.selected_path = self
            .status_line_paths
            .get(&indices[next_pos])
            .cloned();
    }
}

/// Parse une ligne `git status --short` (ex. ` M src/lib.rs`).
#[must_use]
pub fn parse_status_line_path(line: &str) -> Option<String> {
    let line = line.trim_end();
    let bytes = line.as_bytes();
    if bytes.len() < 4 || bytes[2] != b' ' {
        return None;
    }
    let rest = line.get(3..)?.trim();
    if rest.is_empty() {
        return None;
    }
    let path = if let Some((_, new)) = rest.split_once(" -> ") {
        new.trim()
    } else {
        rest
    };
    if path.is_empty() {
        None
    } else {
        Some(path.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct LinesViewerState {
    pub tool_id: ToolUseId,
    pub title: String,
    pub lines: Vec<String>,
    pub scroll_top: usize,
    pub style: LinesViewerStyle,
    pub git_nav: Option<GitWorkspaceNav>,
    diff_meta: Option<Vec<DiffLineMeta>>,
}

impl LinesViewerState {
    #[must_use]
    pub fn new(
        id: ToolUseId,
        title: impl Into<String>,
        lines: Vec<String>,
        style: LinesViewerStyle,
    ) -> Self {
        let mut viewer = Self {
            tool_id: id,
            title: title.into(),
            lines,
            scroll_top: 0,
            style,
            git_nav: None,
            diff_meta: None,
        };
        viewer.refresh_diff_meta();
        viewer
    }

    fn refresh_diff_meta(&mut self) {
        self.diff_meta = if self.style == LinesViewerStyle::UnifiedDiff {
            Some(build_diff_line_meta(&self.lines))
        } else {
            None
        };
    }

    #[must_use]
    pub fn from_workspace_git(
        id: ToolUseId,
        workspace: &camino::Utf8Path,
        lines: Vec<String>,
        nav: GitWorkspaceNav,
    ) -> Self {
        let title = if let Some(ref path) = nav.selected_path {
            format!(" git — {} — {path} ", workspace)
        } else {
            format!(" git — {} ", workspace)
        };
        let mut viewer = Self::new(id, title, lines, LinesViewerStyle::UnifiedDiff);
        viewer.git_nav = Some(nav);
        viewer
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
    pub fn render_lines(&self, visible: usize, palette: &ThemePalette) -> Vec<Line<'static>> {
        let colors = DiffColors::from_palette(palette);
        let end = (self.scroll_top + visible).min(self.lines.len());
        self.lines[self.scroll_top..end]
            .iter()
            .enumerate()
            .map(|(row, line)| {
                let abs = self.scroll_top + row;
                let git_highlight = self.git_nav.as_ref().and_then(|nav| {
                    let path = nav.path_at_line(abs)?;
                    let selected = nav.selected_path.as_deref() == Some(path);
                    Some((selected, nav.status_line_paths.contains_key(&abs)))
                });
                match self.style {
                    LinesViewerStyle::Plain => Line::from(ratatui::text::Span::styled(
                        line.clone(),
                        ratatui::style::Style::default().fg(palette.text_muted),
                    )),
                    LinesViewerStyle::UnifiedDiff => {
                        let meta = self
                            .diff_meta
                            .as_ref()
                            .and_then(|m| m.get(abs).copied())
                            .unwrap_or_default();
                        let bounds = diff_render::hunk_bounds_for_line(&self.lines, abs);
                        let pair = word_diff_pair(&self.lines, abs, bounds);
                        render_diff_line(
                            line,
                            meta,
                            &colors,
                            git_highlight,
                            pair,
                        )
                    }
                }
            })
            .collect()
    }

    /// Chemin cible pour `o` (sélection status ou en-tête `+++ b/`).
    #[must_use]
    pub fn git_open_path(&self) -> Option<String> {
        if let Some(nav) = &self.git_nav {
            if let Some(path) = &nav.selected_path {
                return Some(path.clone());
            }
        }
        for line in &self.lines {
            if let Some(path) = line.strip_prefix("+++ b/") {
                return Some(path.to_string());
            }
        }
        None
    }
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

#[must_use]
pub fn viewer_footer_git(
    scroll_top: usize,
    total: usize,
    visible: usize,
    _has_selection: bool,
) -> String {
    if total == 0 {
        return format!(" {} ", crate::i18n::t(crate::i18n::keys_p1::DIFF_VIEWER_FOOTER_EMPTY));
    }
    let base = viewer_footer(scroll_top, total, visible);
    format!(
        "{}· {} ",
        base.trim_end(),
        crate::i18n::t(crate::i18n::keys_p1::DIFF_VIEWER_FOOTER_GIT_EXTRA)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_status_paths() {
        assert_eq!(
            parse_status_line_path(" M src/lib.rs").as_deref(),
            Some("src/lib.rs")
        );
        assert_eq!(
            parse_status_line_path("?? tmp.log").as_deref(),
            Some("tmp.log")
        );
        assert_eq!(
            parse_status_line_path("R  old.rs -> new.rs").as_deref(),
            Some("new.rs")
        );
    }

    #[test]
    fn git_nav_indexes_status_section() {
        let body = vec![
            "— status —".into(),
            " M src/a.rs".into(),
            "?? b.txt".into(),
            "— diff (HEAD) —".into(),
            "+++ b/src/a.rs".into(),
        ];
        let nav = GitWorkspaceNav::index_body(&body, None);
        assert_eq!(nav.status_line_paths.len(), 2);
        assert_eq!(nav.path_at_line(1), Some("src/a.rs"));
    }

    #[test]
    fn diff_lines_colored() {
        let palette = crate::ui::theme::TuiThemeSetting::Drox.palette();
        let v = LinesViewerState::from_diff(
            ToolUseId::new(),
            "file_edit",
            "a.rs",
            "--- a\n+++ b\n@@ -1,1 +1,1 @@\n-old\n+new",
            "appliqué",
        );
        assert_eq!(v.line_count(), 5);
        let rendered = v.render_lines(10, &palette);
        assert!(!rendered.is_empty());
        assert!(v.diff_meta.is_some());
    }
}
