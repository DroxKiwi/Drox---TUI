//! Rendu overlay pour `ScrollViewerState`.

use ratatui::layout::Rect;
use ratatui::Frame;

use crate::app::AppState;
use crate::view::ScrollViewerState;
use crate::widgets::scroll_overlay;

pub fn render(frame: &mut Frame, area: Rect, state: &AppState, viewer: &ScrollViewerState) {
    let body_h = overlay_body_height(area);
    scroll_overlay::render(
        frame,
        area,
        state.palette.border,
        &viewer.title(),
        viewer.render_lines(body_h, &state.palette),
        &viewer.footer(body_h),
    );
}

pub fn overlay_body_height(area: Rect) -> usize {
    let popup_h = area
        .height
        .saturating_sub(4)
        .max(12)
        .min(area.height.saturating_mul(3) / 4);
    popup_h.saturating_sub(2).saturating_sub(1) as usize
}

/// Index de ligne absolu dans le viewer selon un clic dans le popup.
#[must_use]
pub fn line_index_at_click(popup: Rect, scroll_top: usize, x: u16, y: u16) -> Option<usize> {
    if x < popup.x || x >= popup.x + popup.width || y < popup.y || y >= popup.y + popup.height {
        return None;
    }
    let inner_top = popup.y + 1;
    let inner_h = popup.height.saturating_sub(2);
    let footer_h = 1u16;
    let body_h = inner_h.saturating_sub(footer_h);
    if y < inner_top || y >= inner_top + body_h {
        return None;
    }
    let row = (y - inner_top) as usize;
    Some(scroll_top + row)
}
