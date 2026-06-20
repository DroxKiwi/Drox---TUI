//! Cadre modal style Pip-Boy (coins arrondis, fond panel).

use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, BorderType, Borders};

use crate::ui::{animation, ThemePalette};

/// Bloc modal centré avec bordures arrondies et animation d'entrée.
#[must_use]
pub fn pip_boy_block(
    title: impl Into<String>,
    palette: &ThemePalette,
    border_color: ratatui::style::Color,
) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(format!(" {} ", title.into()))
        .style(
            Style::default()
                .fg(border_color)
                .bg(palette.bg_panel),
        )
}

/// Bordure modal avec animation modal-in (4 frames).
#[must_use]
pub fn pip_boy_block_animated(
    title: impl Into<String>,
    palette: &ThemePalette,
    modal_anim_tick: u8,
    animations_enabled: bool,
) -> Block<'static> {
    let border = animation::modal_border_color(
        palette.border_inactive,
        palette.border,
        modal_anim_tick,
        animations_enabled,
    );
    pip_boy_block(title, palette, border)
}

/// Popup centré avec marges standard.
#[must_use]
pub fn centered_popup(area: Rect, width: u16, height: u16) -> Rect {
    let popup_w = area.width.saturating_sub(4).min(width);
    let popup_h = height.min(area.height.saturating_sub(4));
    let x = area.x + (area.width.saturating_sub(popup_w)) / 2;
    let y = area.y + (area.height.saturating_sub(popup_h)) / 2;
    Rect::new(x, y, popup_w, popup_h)
}

/// Zone intérieure d'un bloc à bordures (sans titre).
#[must_use]
pub fn bordered_inner(popup: Rect) -> Rect {
    Block::default().borders(Borders::ALL).inner(popup)
}
