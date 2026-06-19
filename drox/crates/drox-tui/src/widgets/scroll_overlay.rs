//! Overlay scrollable générique (viewers outils Sprint 4).

use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    border_color: Color,
    title: &str,
    lines: Vec<Line<'static>>,
    footer: &str,
) {
    let popup_w = area.width.saturating_sub(4).min(110);
    let popup_h = area
        .height
        .saturating_sub(4)
        .max(12)
        .min(area.height.saturating_mul(3) / 4);
    let x = area.x + (area.width.saturating_sub(popup_w)) / 2;
    let y = area.y + (area.height.saturating_sub(popup_h)) / 2;
    let popup = Rect::new(x, y, popup_w, popup_h);

    frame.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .style(Style::default().fg(border_color));

    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let footer_h = 1u16;
    let body_area = Rect {
        height: inner.height.saturating_sub(footer_h),
        ..inner
    };
    let footer_area = Rect {
        y: inner.y + inner.height.saturating_sub(footer_h),
        height: footer_h,
        ..inner
    };

    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .style(Style::default().fg(Color::Gray));
    frame.render_widget(paragraph, body_area);

    let footer = Paragraph::new(Line::from(Span::styled(
        footer.to_string(),
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::ITALIC),
    )))
    .alignment(Alignment::Center);
    frame.render_widget(footer, footer_area);
}
