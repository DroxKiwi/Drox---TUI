//! Barre de statut enrichie (modèle, git, tokens, run).

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::{AppPhase, AppState, RunStatus};
use crate::engine::status_bar::{format_elapsed, StatusBarSnapshot};
use crate::view::spinner;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    permission_mode: &str,
    snapshot: &StatusBarSnapshot,
) {
    let run = match (state.phase, state.last_run) {
        (AppPhase::Running, _) => {
            let spin = spinner::frame(state.ui_frame_tick);
            format!("{spin} run")
        }
        (_, RunStatus::Completed) => "ok".into(),
        (_, RunStatus::Cancelled) => "annulé".into(),
        (_, RunStatus::Error) => "erreur".into(),
        _ => "—".into(),
    };
    let queue = if state.queued_messages > 0 {
        format!(" · file {}", state.queued_messages)
    } else {
        String::new()
    };

    let branch = snapshot
        .branch
        .as_deref()
        .map(|b| format!(" · {b}"))
        .unwrap_or_default();

    let tokens = if snapshot.stats.total_in > 0 || snapshot.stats.total_out > 0 {
        let ctx = if snapshot.stats.ctx > 0 {
            let pct = snapshot
                .ctx_pct
                .map(|p| format!(" {p}%"))
                .unwrap_or_default();
            format!(" · ctx {}{}", snapshot.stats.ctx, pct)
        } else {
            String::new()
        };
        format!(
            " · ↑{} ↓{}{}",
            snapshot.stats.total_in, snapshot.stats.total_out, ctx
        )
    } else {
        String::new()
    };

    let elapsed = format_elapsed(snapshot.session_elapsed);
    let session = if state.session_title.is_empty() {
        String::new()
    } else {
        format!(" · {}", state.session_title)
    };
    let workspace = if snapshot.workspace_short.is_empty() {
        String::new()
    } else {
        format!(" · {}", snapshot.workspace_short)
    };

    let hint = if state.status_line.len() < 40 {
        format!(" · {}", state.status_line)
    } else {
        String::new()
    };

    let line = Line::from(vec![
        Span::styled(
            &snapshot.model,
            Style::default()
                .fg(state.palette.header_primary)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(workspace, Style::default().fg(state.palette.header_muted)),
        Span::styled(session, Style::default().fg(Color::Magenta)),
        Span::styled(branch, Style::default().fg(Color::Cyan)),
        Span::styled(tokens, Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!(" · {permission_mode} · {run}{queue} · {elapsed}{hint}"),
            Style::default().fg(state.palette.status_muted),
        ),
    ]);

    frame.render_widget(Paragraph::new(line), area);
}
