//! Indicateur éphémère pendant l'exécution des hooks Pre/Post tool.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use drox_types::ToolUseId;

use super::spinner;

/// Hook en cours pour un appel outil.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveHookProgress {
    pub tool_use_id: ToolUseId,
    pub hook_event: String,
    pub in_progress: usize,
}

impl ActiveHookProgress {
    #[must_use]
    pub fn from_event(
        tool_use_id: ToolUseId,
        hook_event: String,
        in_progress: usize,
    ) -> Option<Self> {
        if in_progress == 0 {
            return None;
        }
        Some(Self {
            tool_use_id,
            hook_event,
            in_progress,
        })
    }
}

#[must_use]
pub fn progress_lines(progress: &ActiveHookProgress, tick: u8) -> Vec<Line<'static>> {
    let frame = spinner::frame(tick);
    let style = Style::default().fg(Color::Cyan);
    let label = if progress.in_progress == 1 {
        "hook".to_string()
    } else {
        format!("{} hooks", progress.in_progress)
    };
    vec![Line::from(vec![
        Span::styled(format!("{frame} "), style),
        Span::styled(
            format!(
                "{} {} — {}",
                progress.in_progress, label, progress.hook_event
            ),
            style.add_modifier(Modifier::ITALIC),
        ),
    ])]
}
