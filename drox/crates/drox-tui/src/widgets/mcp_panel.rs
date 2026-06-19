//! Panneau serveurs MCP (config `.mcp.json`).

use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::app::AppState;

pub fn desired_height(state: &AppState) -> u16 {
    let Some(snapshot) = &state.mcp_snapshot else {
        return 0;
    };
    if snapshot.servers.is_empty() {
        return 0;
    }
    (snapshot.servers.len() as u16).min(4) + 2
}

pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let Some(snapshot) = &state.mcp_snapshot else {
        return;
    };
    if snapshot.servers.is_empty() {
        return;
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" MCP {} serveur(s) ", snapshot.servers.len()))
        .style(Style::default().fg(Color::Cyan));

    let mut lines = vec![Line::from(Span::styled(
        truncate_path(&snapshot.config_path),
        Style::default().fg(Color::DarkGray),
    ))];
    for srv in snapshot.servers.iter().take(4) {
        lines.push(Line::from(vec![
            Span::styled(format!(" · {} ", srv.name), Style::default().fg(Color::Yellow)),
            Span::styled(srv.summary.clone(), Style::default().fg(Color::Gray)),
        ]));
    }
    if snapshot.servers.len() > 4 {
        lines.push(Line::from(Span::styled(
            format!("  … +{} — /mcp tools", snapshot.servers.len() - 4),
            Style::default().fg(Color::DarkGray),
        )));
    }

    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn truncate_path(path: &str) -> String {
    if path.len() <= 48 {
        path.to_string()
    } else {
        format!("…{}", &path[path.len().saturating_sub(44)..])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{McpPanelSnapshot, McpServerLine};

    #[test]
    fn height_zero_without_mcp() {
        let state = AppState::new();
        assert_eq!(desired_height(&state), 0);
    }

    #[test]
    fn height_scales_with_servers() {
        let mut state = AppState::new();
        state.mcp_snapshot = Some(McpPanelSnapshot {
            config_path: ".mcp.json".into(),
            servers: vec![
                McpServerLine {
                    name: "a".into(),
                    summary: "stdio".into(),
                },
                McpServerLine {
                    name: "b".into(),
                    summary: "remote".into(),
                },
            ],
        });
        assert_eq!(desired_height(&state), 4);
    }
}
