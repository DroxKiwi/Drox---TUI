//! Frames spinner partagées (leak : `Spinner.tsx`).

/// Frames Braille pour indicateurs d'attente.
pub const SPINNER_FRAMES: [&str; 4] = ["⠋", "⠙", "⠹", "⠸"];

#[must_use]
pub fn frame(tick: u8) -> &'static str {
    SPINNER_FRAMES[usize::from(tick % 4)]
}
