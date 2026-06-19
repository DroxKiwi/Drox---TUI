//! Enum unifié des viewers scrollables outil.

use drox_types::ToolUseId;
use ratatui::text::Line;

use super::bash_viewer::{self, BashViewerState};
use super::file_read_viewer::{self, FileReadViewerState};
use super::glob_viewer::{self, GlobViewerState};
use super::grep_viewer::{self, GrepViewerState};
use super::lines_viewer::{self, LinesViewerState};
use super::web_fetch_viewer::{self, WebFetchViewerState};
use super::web_search_viewer::{self, WebSearchViewerState};

#[derive(Debug, Clone)]
pub enum ScrollViewerState {
    FileRead(FileReadViewerState),
    Bash(BashViewerState),
    Grep(GrepViewerState),
    Glob(GlobViewerState),
    WebFetch(WebFetchViewerState),
    WebSearch(WebSearchViewerState),
    Lines(LinesViewerState),
}

impl ScrollViewerState {
    #[must_use]
    pub fn tool_id(&self) -> &ToolUseId {
        match self {
            Self::FileRead(v) => &v.tool_id,
            Self::Bash(v) => &v.tool_id,
            Self::Grep(v) => &v.tool_id,
            Self::Glob(v) => &v.tool_id,
            Self::WebFetch(v) => &v.tool_id,
            Self::WebSearch(v) => &v.tool_id,
            Self::Lines(v) => &v.tool_id,
        }
    }

    #[must_use]
    pub fn line_count(&self) -> usize {
        match self {
            Self::FileRead(v) => v.lines.len(),
            Self::Bash(v) => v.lines.len(),
            Self::Grep(v) => v.lines.len(),
            Self::Glob(v) => v.line_count(),
            Self::WebFetch(v) => v.lines.len(),
            Self::WebSearch(v) => v.line_count(),
            Self::Lines(v) => v.line_count(),
        }
    }

    #[must_use]
    pub fn scroll_top(&self) -> usize {
        match self {
            Self::FileRead(v) => v.scroll_top,
            Self::Bash(v) => v.scroll_top,
            Self::Grep(v) => v.scroll_top,
            Self::Glob(v) => v.scroll_top,
            Self::WebFetch(v) => v.scroll_top,
            Self::WebSearch(v) => v.scroll_top,
            Self::Lines(v) => v.scroll_top,
        }
    }

    pub fn scroll_page(&mut self, delta_lines: usize, visible: usize) {
        match self {
            Self::FileRead(v) => v.scroll_page(delta_lines, visible),
            Self::Bash(v) => v.scroll_page(delta_lines, visible),
            Self::Grep(v) => v.scroll_page(delta_lines, visible),
            Self::Glob(v) => v.scroll_page(delta_lines, visible),
            Self::WebFetch(v) => v.scroll_page(delta_lines, visible),
            Self::WebSearch(v) => v.scroll_page(delta_lines, visible),
            Self::Lines(v) => v.scroll_page(delta_lines, visible),
        }
    }

    pub fn scroll_page_down(&mut self, delta_lines: usize, visible: usize) {
        match self {
            Self::FileRead(v) => v.scroll_page_down(delta_lines, visible),
            Self::Bash(v) => v.scroll_page_down(delta_lines, visible),
            Self::Grep(v) => v.scroll_page_down(delta_lines, visible),
            Self::Glob(v) => v.scroll_page_down(delta_lines, visible),
            Self::WebFetch(v) => v.scroll_page_down(delta_lines, visible),
            Self::WebSearch(v) => v.scroll_page_down(delta_lines, visible),
            Self::Lines(v) => v.scroll_page_down(delta_lines, visible),
        }
    }

    #[must_use]
    pub fn render_lines(&self, visible: usize) -> Vec<Line<'static>> {
        match self {
            Self::FileRead(v) => v.render_lines(visible),
            Self::Bash(v) => v.render_lines(visible),
            Self::Grep(v) => v.render_lines(visible),
            Self::Glob(v) => v.render_lines(visible),
            Self::WebFetch(v) => v.render_lines(visible),
            Self::WebSearch(v) => v.render_lines(visible),
            Self::Lines(v) => v.render_lines(visible),
        }
    }

    #[must_use]
    pub fn title(&self) -> String {
        match self {
            Self::FileRead(v) => file_read_viewer::viewer_title(v),
            Self::Bash(v) => bash_viewer::viewer_title(v),
            Self::Grep(v) => grep_viewer::viewer_title(v),
            Self::Glob(v) => glob_viewer::viewer_title(v),
            Self::WebFetch(v) => web_fetch_viewer::viewer_title(v),
            Self::WebSearch(v) => web_search_viewer::viewer_title(v),
            Self::Lines(v) => v.title.clone(),
        }
    }

    #[must_use]
    pub fn footer(&self, visible: usize) -> String {
        let top = self.scroll_top();
        let total = self.line_count();
        match self {
            Self::FileRead(_) => file_read_viewer::viewer_footer(top, total, visible),
            Self::Bash(_) => bash_viewer::viewer_footer(top, total, visible),
            Self::Grep(_) => grep_viewer::viewer_footer(top, total, visible),
            Self::Glob(_) => glob_viewer::viewer_footer(top, total, visible),
            Self::WebFetch(_) => web_fetch_viewer::viewer_footer(top, total, visible),
            Self::WebSearch(_) => web_search_viewer::viewer_footer(top, total, visible),
            Self::Lines(_) => lines_viewer::viewer_footer(top, total, visible),
        }
    }
}
