//! Cache des lignes rendues — évite de re-parser tout le fil à chaque frame.

use std::collections::HashSet;

use ratatui::text::Line;
use drox_types::ToolUseId;

use super::log_entry::LogEntry;
use super::message_router;

/// Snapshot pour invalider le cache.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogRenderCacheKey {
    pub revision: u64,
    pub entries_len: usize,
    pub expanded: Vec<ToolUseId>,
    pub collapsed_phases: Vec<usize>,
}

/// Lignes stables (hors streaming / bash live).
#[derive(Debug, Clone)]
pub struct LogRenderCache {
    pub key: LogRenderCacheKey,
    pub lines: Vec<Line<'static>>,
}

impl LogRenderCache {
    #[must_use]
    pub fn build(
        revision: u64,
        entries: &[LogEntry],
        expanded_tools: &HashSet<ToolUseId>,
        collapsed_phases: &HashSet<usize>,
    ) -> Self {
        let mut expanded: Vec<_> = expanded_tools.iter().cloned().collect();
        expanded.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        let mut collapsed: Vec<_> = collapsed_phases.iter().copied().collect();
        collapsed.sort_unstable();
        Self {
            key: LogRenderCacheKey {
                revision,
                entries_len: entries.len(),
                expanded,
                collapsed_phases: collapsed,
            },
            lines: message_router::render_entries(entries, expanded_tools, collapsed_phases),
        }
    }
}

#[must_use]
pub fn cache_key(
    revision: u64,
    entries: &[LogEntry],
    expanded_tools: &HashSet<ToolUseId>,
    collapsed_phases: &HashSet<usize>,
) -> LogRenderCacheKey {
    let mut expanded: Vec<_> = expanded_tools.iter().cloned().collect();
    expanded.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    let mut collapsed: Vec<_> = collapsed_phases.iter().copied().collect();
    collapsed.sort_unstable();
    LogRenderCacheKey {
        revision,
        entries_len: entries.len(),
        expanded,
        collapsed_phases: collapsed,
    }
}
