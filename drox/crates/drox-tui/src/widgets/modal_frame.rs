//! Cadre modal style Pip-Boy (coins arrondis, fond panel).

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
