//! Panneau to-do live (sorties `todo_write`).

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::app::{AppState, TodoSnapshot};

pub fn desired_height(state: &AppState) -> u16 {
    let Some(snapshot) = &state.todo_snapshot else {
        return 0;
    };
    if snapshot.items.is_empty() {
        return 0;
    }
    (snapshot.items.len() as u16).min(6) + 2
}

pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let Some(snapshot) = &state.todo_snapshot else {
        return;
    };
    if snapshot.items.is_empty() {
        return;
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .title(todo_title(snapshot))
        .style(Style::default().fg(Color::Yellow));

    let mut lines = vec![Line::from(Span::styled(
        snapshot.summary.clone(),
        Style::default().fg(Color::DarkGray),
    ))];

    for item in snapshot.items.iter().take(6) {
        let (marker, color) = status_style(&item.status);
        lines.push(Line::from(vec![
            Span::styled(format!(" {marker} "), Style::default().fg(color)),
            Span::styled(
                item.content.clone(),
                item_style(&item.status),
            ),
        ]));
    }
    if snapshot.items.len() > 6 {
        lines.push(Line::from(Span::styled(
            format!("  … +{} tâches", snapshot.items.len() - 6),
            Style::default().fg(Color::DarkGray),
        )));
    }

    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn todo_title(snapshot: &TodoSnapshot) -> String {
    let total = snapshot.items.len();
    let done = snapshot
        .items
        .iter()
        .filter(|i| i.status == "completed")
        .count();
    let active = snapshot
        .items
        .iter()
        .filter(|i| i.status == "in_progress")
        .count();
    if active > 0 {
        format!(" Todos {done}/{total} · {active} en cours ")
    } else {
        format!(" Todos {done}/{total} ")
    }
}

fn status_style(status: &str) -> (&'static str, Color) {
    match status {
        "in_progress" => ("▶", Color::Yellow),
        "completed" => ("✓", Color::Green),
        "cancelled" => ("✗", Color::DarkGray),
        _ => ("○", Color::Gray),
    }
}

fn item_style(status: &str) -> Style {
    match status {
        "completed" => Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::CROSSED_OUT),
        "cancelled" => Style::default().fg(Color::DarkGray),
        "in_progress" => Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
        _ => Style::default().fg(Color::Gray),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{TodoItemView, TodoSnapshot};

    #[test]
    fn height_zero_without_todos() {
        let state = AppState::new();
        assert_eq!(desired_height(&state), 0);
    }

    #[test]
    fn height_scales_with_items() {
        let mut state = AppState::new();
        state.todo_snapshot = Some(TodoSnapshot {
            summary: "2 todos".into(),
            items: vec![
                TodoItemView {
                    id: "1".into(),
                    content: "a".into(),
                    status: "pending".into(),
                },
                TodoItemView {
                    id: "2".into(),
                    content: "b".into(),
                    status: "pending".into(),
                },
            ],
        });
        assert_eq!(desired_height(&state), 4);
    }

    #[test]
    fn title_shows_progress() {
        let snapshot = TodoSnapshot {
            summary: "x".into(),
            items: vec![
                TodoItemView {
                    id: "1".into(),
                    content: "a".into(),
                    status: "completed".into(),
                },
                TodoItemView {
                    id: "2".into(),
                    content: "b".into(),
                    status: "in_progress".into(),
                },
            ],
        };
        let title = todo_title(&snapshot);
        assert!(title.contains("1/2"));
        assert!(title.contains("en cours"));
    }
}
