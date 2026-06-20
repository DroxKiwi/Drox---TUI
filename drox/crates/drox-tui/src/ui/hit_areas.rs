//! Zones cliquables mises à jour à chaque frame.

use ratatui::layout::Rect;

/// Rectangles écran pour le hit-testing souris.
#[derive(Clone, Copy, Debug, Default)]
pub struct HitAreas {
    pub terminal: Rect,
    pub message_log: Rect,
    pub prompt_body: Option<Rect>,
    pub prompt_footer: Option<Rect>,
    pub ai_server_popup: Option<Rect>,
    pub scroll_viewer_popup: Option<Rect>,
}

impl HitAreas {
    #[must_use]
    pub fn contains(rect: Rect, x: u16, y: u16) -> bool {
        x >= rect.x
            && x < rect.x.saturating_add(rect.width)
            && y >= rect.y
            && y < rect.y.saturating_add(rect.height)
    }
}
