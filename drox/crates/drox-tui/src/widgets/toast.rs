//! Toast éphémère (notifications UX §14.7).

use std::time::{Duration, Instant};

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::AppState;

const TOAST_TTL: Duration = Duration::from_secs(4);

/// Notification courte affichée au-dessus de la status line.
#[derive(Debug, Clone)]
pub struct Toast {
    pub text: String,
    pub until: Instant,
}

impl AppState {
    pub fn push_toast(&mut self, text: impl Into<String>) {
        self.toast = Some(Toast {
            text: text.into(),
            until: Instant::now() + TOAST_TTL,
        });
    }

    pub fn clear_expired_toast(&mut self) {
        if self
            .toast
            .as_ref()
            .is_some_and(|t| Instant::now() >= t.until)
        {
            self.toast = None;
        }
    }
}

pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let Some(toast) = state.toast.as_ref() else {
        return;
    };
    let w = toast.text.chars().count().saturating_add(6) as u16;
    let popup_w = w.min(area.width.saturating_sub(2)).max(12);
    let popup_h = 3u16;
    let x = area.x + area.width.saturating_sub(popup_w + 1);
    let y = area.y + area.height.saturating_sub(popup_h + 1);
    let popup = Rect::new(x, y, popup_w, popup_h);
    frame.render_widget(Clear, popup);
    let line = Line::from(Span::styled(
        toast.text.clone(),
        Style::default()
            .fg(Color::Black)
            .bg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    ));
    frame.render_widget(
        Paragraph::new(vec![line]).block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Yellow)),
        ),
        popup,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toast_expires() {
        let mut state = AppState::new();
        state.push_toast("ok");
        assert!(state.toast.is_some());
        if let Some(t) = &mut state.toast {
            t.until = Instant::now();
        }
        state.clear_expired_toast();
        assert!(state.toast.is_none());
    }
}
