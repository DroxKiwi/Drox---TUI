//! Layout principal ratatui.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::app::{AppPhase, AppState};
use crate::engine::status_bar::StatusBarSnapshot;
use crate::engine::VimComposer;
use crate::widgets::{composer, composer_help, composer_suggestions, copy_selector, course_panel, mcp_panel, message_log, onboarding, prompt_modal, rewind_selector, scroll_viewer, search_bar, slash_palette, status_bar, status_notices, theme_picker, toast, todo_panel};

pub fn draw(
    frame: &mut Frame,
    state: &mut AppState,
    history_search: Option<&crate::view::HistorySearchState>,
    status: &StatusBarSnapshot,
    model: &str,
    workspace: &str,
    permission_mode: &str,
    plan_mode: bool,
    vim: &VimComposer,
) {
    let area = frame.area();
    let notices_h = status_notices::desired_height(state);
    let todo_h = todo_panel::desired_height(state);
    let course_h = course_panel::desired_height(state);
    let mcp_h = mcp_panel::desired_height(state);
    let mut constraints = vec![Constraint::Length(3)];
    if notices_h > 0 {
        constraints.push(Constraint::Length(notices_h));
    }
    constraints.push(Constraint::Min(5));
    if todo_h > 0 {
        constraints.push(Constraint::Length(todo_h));
    }
    if course_h > 0 {
        constraints.push(Constraint::Length(course_h));
    }
    if mcp_h > 0 {
        constraints.push(Constraint::Length(mcp_h));
    }
    constraints.push(Constraint::Length(5));
    constraints.push(Constraint::Length(1));

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    let mut idx = 0usize;
    draw_header(frame, chunks[idx], state, model, workspace, permission_mode, plan_mode);
    idx += 1;
    if notices_h > 0 {
        status_notices::render(frame, chunks[idx], state);
        idx += 1;
    }
    message_log::render(frame, chunks[idx], state);
    idx += 1;
    if todo_h > 0 {
        todo_panel::render(frame, chunks[idx], state);
        idx += 1;
    }
    if course_h > 0 {
        course_panel::render(frame, chunks[idx], state);
        idx += 1;
    }
    if mcp_h > 0 {
        mcp_panel::render(frame, chunks[idx], state);
        idx += 1;
    }
    composer::render(frame, chunks[idx], state, model, permission_mode, plan_mode, vim);
    if let Some(dialog) = state.composer_suggestions.as_ref() {
        composer_suggestions::render(frame, chunks[idx], state, dialog);
    }
    if state.composer_help {
        composer_help::render(frame, area, state);
    }
    if let Some(search) = history_search {
        search_bar::render_history_bar(frame, chunks[idx], state, search);
    }
    idx += 1;
    status_bar::render(frame, chunks[idx], state, permission_mode, status);

    if state.phase == AppPhase::Prompt {
        prompt_modal::render(frame, area, state);
    }
    if state.phase == AppPhase::Rewind {
        rewind_selector::render(frame, area, state);
    }
    if state.phase == AppPhase::Theme {
        theme_picker::render(frame, area, state);
    }
    if state.phase == AppPhase::SlashPalette {
        slash_palette::render(frame, area, state);
    }
    if state.phase == AppPhase::Copy {
        copy_selector::render(frame, area, state);
    }
    if state.phase == AppPhase::Onboarding {
        onboarding::render(frame, area, state);
    }
    if let Some(ref viewer) = state.scroll_viewer {
        scroll_viewer::render(frame, area, state, viewer);
    }
    toast::render(frame, area, state);
}

fn draw_header(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    model: &str,
    workspace: &str,
    permission_mode: &str,
    plan_mode: bool,
) {
    let mode_tag = if plan_mode {
        format!("{permission_mode} · PLAN")
    } else {
        permission_mode.to_string()
    };
    let line = Line::from(vec![
        Span::styled(
            model,
            Style::default()
                .fg(state.palette.header_primary)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" · {mode_tag}"),
            Style::default().fg(state.palette.mode_tag),
        ),
        Span::styled(
            format!(" · {workspace}"),
            Style::default().fg(state.palette.header_muted),
        ),
        Span::raw(" · Ctrl+Q quitter"),
    ]);
    let paragraph = Paragraph::new(line).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Drox TUI ")
            .style(Style::default().fg(state.palette.border)),
    );
    frame.render_widget(paragraph, area);
}
