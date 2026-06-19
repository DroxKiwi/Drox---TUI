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
        viewer.render_lines(body_h),
        &viewer.footer(body_h),
    );
}

fn overlay_body_height(area: Rect) -> usize {
    let popup_h = area
        .height
        .saturating_sub(4)
        .max(12)
        .min(area.height.saturating_mul(3) / 4);
    popup_h.saturating_sub(2).saturating_sub(1) as usize
}
