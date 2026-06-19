//! Panneau plan de cours live (`course_plan_write`).

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::app::{AppState, CourseSnapshot};

pub fn desired_height(state: &AppState) -> u16 {
    let Some(snapshot) = &state.course_snapshot else {
        return 0;
    };
    if snapshot.steps.is_empty() {
        return 0;
    }
    (snapshot.steps.len() as u16).min(5) + 3
}

pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let Some(snapshot) = &state.course_snapshot else {
        return;
    };
    if snapshot.steps.is_empty() {
        return;
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .title(course_title(snapshot))
        .style(Style::default().fg(Color::Magenta));

    let mut lines = vec![Line::from(Span::styled(
        snapshot.summary.clone(),
        Style::default().fg(Color::DarkGray),
    ))];

    for step in snapshot.steps.iter().take(5) {
        let (marker, color) = step_marker(&step.kind, &step.status);
        lines.push(Line::from(vec![
            Span::styled(format!(" {marker} "), Style::default().fg(color)),
            Span::styled(step.title.clone(), step_style(&step.status)),
        ]));
    }
    if snapshot.steps.len() > 5 {
        lines.push(Line::from(Span::styled(
            format!("  … +{} étapes", snapshot.steps.len() - 5),
            Style::default().fg(Color::DarkGray),
        )));
    }

    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn course_title(snapshot: &CourseSnapshot) -> String {
    let total = snapshot.steps.len();
    let mastered = snapshot
        .steps
        .iter()
        .filter(|s| s.status == "mastered")
        .count();
    let active = snapshot
        .steps
        .iter()
        .filter(|s| s.status == "active")
        .count();
    let title = truncate_chars(&snapshot.title, 28);
    if active > 0 {
        format!(" Cours {mastered}/{total} · actif · {title} ")
    } else {
        format!(" Cours {mastered}/{total} · {title} ")
    }
}

fn truncate_chars(s: &str, max_chars: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max_chars {
        return s.to_string();
    }
    let take = max_chars.saturating_sub(1);
    format!("{}…", s.chars().take(take).collect::<String>())
}

fn step_marker(kind: &str, status: &str) -> (&'static str, Color) {
    let base = match kind {
        "lesson" => "L",
        "exercise" => "E",
        "checkpoint" => "C",
        _ => "·",
    };
    let color = match status {
        "active" => Color::Yellow,
        "mastered" => Color::Green,
        "skipped" => Color::DarkGray,
        _ => Color::Gray,
    };
    (base, color)
}

fn step_style(status: &str) -> Style {
    match status {
        "mastered" => Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::CROSSED_OUT),
        "skipped" => Style::default().fg(Color::DarkGray),
        "active" => Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
        _ => Style::default().fg(Color::Gray),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{CourseSnapshot, CourseStepView};

    #[test]
    fn height_zero_without_course() {
        let state = AppState::new();
        assert_eq!(desired_height(&state), 0);
    }

    #[test]
    fn title_shows_progress_and_active() {
        let snapshot = CourseSnapshot {
            title: "Introduction à Rust".into(),
            summary: "2/3".into(),
            steps: vec![
                CourseStepView {
                    id: "1".into(),
                    title: "L1".into(),
                    kind: "lesson".into(),
                    status: "mastered".into(),
                },
                CourseStepView {
                    id: "2".into(),
                    title: "L2".into(),
                    kind: "lesson".into(),
                    status: "active".into(),
                },
                CourseStepView {
                    id: "3".into(),
                    title: "L3".into(),
                    kind: "exercise".into(),
                    status: "pending".into(),
                },
            ],
        };
        let title = course_title(&snapshot);
        assert!(title.contains("1/3"));
        assert!(title.contains("actif"));
        assert!(title.contains("Introduction"));
    }

    #[test]
    fn title_truncates_long_course_name() {
        let snapshot = CourseSnapshot {
            title: "Un titre de cours extrêmement long pour le panneau".into(),
            summary: "".into(),
            steps: vec![CourseStepView {
                id: "1".into(),
                title: "x".into(),
                kind: "lesson".into(),
                status: "pending".into(),
            }],
        };
        let title = course_title(&snapshot);
        assert!(title.contains('…'));
        assert!(title.chars().count() < 60);
    }
}
