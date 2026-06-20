//! Helpers animation TUI (charte Drox — cursor-blink, phosphor-pulse, modal-in, handshake).

use ratatui::style::Color;

/// Période clignotement curseur (~530 ms à ~80 ms/frame).
const CURSOR_BLINK_PERIOD: u8 = 7;

/// Période pulse bordure composer (~560 ms).
const PULSE_PERIOD: u8 = 14;

#[must_use]
pub fn cursor_visible(frame_tick: u8, enabled: bool) -> bool {
    if !enabled {
        return true;
    }
    (frame_tick / CURSOR_BLINK_PERIOD) % 2 == 0
}

#[must_use]
pub fn phosphor_pulse(primary: Color, glow: Color, frame_tick: u8, enabled: bool) -> Color {
    if !enabled {
        return primary;
    }
    if frame_tick % PULSE_PERIOD < PULSE_PERIOD / 2 {
        primary
    } else {
        glow
    }
}

/// Intensité bordure modal-in (0–4 frames d'apparition).
#[must_use]
pub fn modal_border_color(
    dim: Color,
    bright: Color,
    modal_anim_tick: u8,
    enabled: bool,
) -> Color {
    if !enabled || modal_anim_tick >= 4 {
        return bright;
    }
    match modal_anim_tick {
        0 => dim,
        1 => dim,
        2 => bright,
        _ => bright,
    }
}

/// Barre progression test connexion `/server`.
#[must_use]
pub fn handshake_bar(frame_tick: u8, width: usize, enabled: bool) -> String {
    let width = width.max(8);
    let pos = if enabled {
        frame_tick as usize % (width * 2)
    } else {
        width
    };
    let filled = pos.min(width);
    let bar: String = (0..width)
        .map(|i| if i < filled { '=' } else if i == filled { '>' } else { ' ' })
        .collect();
    format!("[{bar}] PROBE…")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_blinks_when_enabled() {
        assert!(cursor_visible(0, true));
        assert!(!cursor_visible(CURSOR_BLINK_PERIOD, true));
    }

    #[test]
    fn cursor_always_on_when_disabled() {
        assert!(cursor_visible(99, false));
    }

    #[test]
    fn handshake_bar_has_brackets() {
        let bar = handshake_bar(3, 12, true);
        assert!(bar.starts_with('['));
        assert!(bar.contains("PROBE"));
    }
}
