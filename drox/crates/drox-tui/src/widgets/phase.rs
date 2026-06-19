//! Bloc phase repliable (équivalent `.phase-block` webview / trace Ink).

use drox_engine::Phase;

/// Vue d'une phase dans le fil (placeholder — rendu ratatui à brancher).
#[derive(Debug, Clone)]
pub struct PhaseBlock {
    pub phase: Phase,
    pub lines: Vec<String>,
    pub collapsed: bool,
}

impl PhaseBlock {
    #[must_use]
    pub fn new(phase: Phase) -> Self {
        Self {
            phase,
            lines: Vec::new(),
            collapsed: false,
        }
    }

    pub fn push_line(&mut self, line: impl Into<String>) {
        self.lines.push(line.into());
    }
}
