//! Palettes TUI (`/theme`, `/color`).
//!
//! Tokens normatifs : voir `docs/THEME.md` à la racine du dépôt.

use ratatui::style::Color;
use serde::{Deserialize, Serialize};

/// Thème global du terminal Drox.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum TuiThemeSetting {
    /// Vert phosphore sur fond noir — défaut produit 2.0.2+
    #[default]
    Drox,
    /// Fallback 16 couleurs du thème Drox.
    #[serde(rename = "drox-ansi")]
    DroxAnsi,
    Dark,
    Light,
    #[serde(rename = "dark-ansi")]
    DarkAnsi,
    #[serde(rename = "light-ansi")]
    LightAnsi,
    Dim,
}

/// Accent session (`/color`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionAccent {
    Red,
    Blue,
    Green,
    Yellow,
    Purple,
    Orange,
    Pink,
    Cyan,
}

/// Couleurs dérivées pour le rendu ratatui (mapping sémantique).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemePalette {
    pub bg: Color,
    pub bg_panel: Color,
    pub bg_elevated: Color,
    pub text: Color,
    pub text_muted: Color,
    pub header_primary: Color,
    pub header_muted: Color,
    pub accent_bright: Color,
    pub accent_glow: Color,
    pub border: Color,
    pub border_inactive: Color,
    pub composer_border: Color,
    pub composer_border_blocked: Color,
    pub status_muted: Color,
    pub mode_tag: Color,
    pub warning: Color,
    pub error: Color,
    pub selection_fg: Color,
    pub selection_bg: Color,
    pub diff_add: Color,
    pub diff_remove: Color,
    pub diff_hunk: Color,
    pub diff_meta: Color,
    pub diff_ctx: Color,
    pub diff_gutter: Color,
    pub diff_status: Color,
}

/// Tokens couleur normatifs Drox (truecolor).
pub mod drox {
    use ratatui::style::Color;

    pub const BG_DEEP: Color = Color::Rgb(0x0a, 0x0e, 0x0a);
    pub const BG_PANEL: Color = Color::Rgb(0x0f, 0x14, 0x10);
    pub const BG_ELEVATED: Color = Color::Rgb(0x14, 0x1a, 0x14);
    pub const PHOSPHOR_PRIMARY: Color = Color::Rgb(0x33, 0xff, 0x66);
    pub const PHOSPHOR_DIM: Color = Color::Rgb(0x1a, 0x99, 0x33);
    pub const PHOSPHOR_BRIGHT: Color = Color::Rgb(0x66, 0xff, 0x99);
    pub const PHOSPHOR_GLOW: Color = Color::Rgb(0x00, 0xff, 0x41);
    pub const AMBER_ALERT: Color = Color::Rgb(0xff, 0xb0, 0x00);
    pub const RED_CRITICAL: Color = Color::Rgb(0xff, 0x33, 0x33);
    pub const CYAN_META: Color = Color::Rgb(0x66, 0xcc, 0xff);
    pub const MAGENTA_HUNK: Color = Color::Rgb(0xcc, 0x66, 0xff);
    pub const GUTTER_MUTED: Color = Color::Rgb(0x55, 0x77, 0x55);
}

impl TuiThemeSetting {
    pub const ALL: &'static [Self] = &[
        Self::Drox,
        Self::DroxAnsi,
        Self::Dark,
        Self::Light,
        Self::DarkAnsi,
        Self::LightAnsi,
        Self::Dim,
    ];

    #[must_use]
    pub const fn is_drox_family(self) -> bool {
        matches!(self, Self::Drox | Self::DroxAnsi)
    }

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Drox => "drox",
            Self::DroxAnsi => "drox-ansi",
            Self::Dark => "dark",
            Self::Light => "light",
            Self::DarkAnsi => "dark-ansi",
            Self::LightAnsi => "light-ansi",
            Self::Dim => "dim",
        }
    }

    #[must_use]
    pub fn from_slug(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "drox" => Some(Self::Drox),
            "drox-ansi" | "drox_ansi" => Some(Self::DroxAnsi),
            "dark" => Some(Self::Dark),
            "light" => Some(Self::Light),
            "dark-ansi" | "dark_ansi" => Some(Self::DarkAnsi),
            "light-ansi" | "light_ansi" => Some(Self::LightAnsi),
            "dim" => Some(Self::Dim),
            _ => None,
        }
    }

    #[must_use]
    pub fn palette(self) -> ThemePalette {
        match self {
            Self::Drox => ThemePalette {
                    bg: drox::BG_DEEP,
                    bg_panel: drox::BG_PANEL,
                    bg_elevated: drox::BG_ELEVATED,
                    text: drox::PHOSPHOR_PRIMARY,
                    text_muted: drox::PHOSPHOR_DIM,
                    header_primary: drox::PHOSPHOR_PRIMARY,
                    header_muted: drox::PHOSPHOR_DIM,
                    accent_bright: drox::PHOSPHOR_BRIGHT,
                    accent_glow: drox::PHOSPHOR_GLOW,
                    border: drox::PHOSPHOR_BRIGHT,
                    border_inactive: drox::PHOSPHOR_DIM,
                    composer_border: drox::PHOSPHOR_GLOW,
                    composer_border_blocked: drox::PHOSPHOR_DIM,
                    status_muted: drox::PHOSPHOR_DIM,
                    mode_tag: drox::AMBER_ALERT,
                    warning: drox::AMBER_ALERT,
                    error: drox::RED_CRITICAL,
                    selection_fg: drox::BG_DEEP,
                    selection_bg: drox::PHOSPHOR_BRIGHT,
                    diff_add: drox::PHOSPHOR_BRIGHT,
                    diff_remove: drox::RED_CRITICAL,
                    diff_hunk: drox::MAGENTA_HUNK,
                    diff_meta: drox::CYAN_META,
                    diff_ctx: drox::PHOSPHOR_DIM,
                    diff_gutter: drox::GUTTER_MUTED,
                    diff_status: drox::AMBER_ALERT,
                },
            Self::DroxAnsi => ThemePalette {
                    bg: Color::Black,
                    bg_panel: Color::Black,
                    bg_elevated: Color::Black,
                    text: Color::Green,
                    text_muted: Color::Rgb(0, 128, 0),
                    header_primary: Color::Green,
                    header_muted: Color::Rgb(0, 128, 0),
                    accent_bright: Color::LightGreen,
                    accent_glow: Color::LightGreen,
                    border: Color::Green,
                    border_inactive: Color::Rgb(0, 128, 0),
                    composer_border: Color::LightGreen,
                    composer_border_blocked: Color::Rgb(0, 128, 0),
                    status_muted: Color::Rgb(0, 128, 0),
                    mode_tag: Color::Yellow,
                    warning: Color::Yellow,
                    error: Color::Red,
                    selection_fg: Color::Black,
                    selection_bg: Color::LightGreen,
                    diff_add: Color::LightGreen,
                    diff_remove: Color::Red,
                    diff_hunk: Color::Magenta,
                    diff_meta: Color::Cyan,
                    diff_ctx: Color::Rgb(0, 128, 0),
                    diff_gutter: Color::DarkGray,
                    diff_status: Color::Yellow,
                },
            Self::Dark => ThemePalette {
                    bg: Color::Black,
                    bg_panel: Color::Rgb(16, 16, 16),
                    bg_elevated: Color::Rgb(24, 24, 24),
                    text: Color::White,
                    text_muted: Color::DarkGray,
                    header_primary: Color::Cyan,
                    header_muted: Color::DarkGray,
                    accent_bright: Color::Cyan,
                    accent_glow: Color::Yellow,
                    border: Color::Cyan,
                    border_inactive: Color::DarkGray,
                    composer_border: Color::Yellow,
                    composer_border_blocked: Color::DarkGray,
                    status_muted: Color::DarkGray,
                    mode_tag: Color::Yellow,
                    warning: Color::Yellow,
                    error: Color::Red,
                    selection_fg: Color::Black,
                    selection_bg: Color::Cyan,
                    diff_add: Color::Green,
                    diff_remove: Color::Red,
                    diff_hunk: Color::Magenta,
                    diff_meta: Color::Cyan,
                    diff_ctx: Color::DarkGray,
                    diff_gutter: Color::Rgb(64, 64, 64),
                    diff_status: Color::Yellow,
                },
            Self::Light => ThemePalette {
                    bg: Color::White,
                    bg_panel: Color::Rgb(245, 245, 245),
                    bg_elevated: Color::Rgb(235, 235, 235),
                    text: Color::Black,
                    text_muted: Color::Gray,
                    header_primary: Color::Blue,
                    header_muted: Color::Gray,
                    accent_bright: Color::Blue,
                    accent_glow: Color::Rgb(180, 100, 0),
                    border: Color::Blue,
                    border_inactive: Color::Gray,
                    composer_border: Color::Rgb(180, 100, 0),
                    composer_border_blocked: Color::Gray,
                    status_muted: Color::Gray,
                    mode_tag: Color::Rgb(180, 100, 0),
                    warning: Color::Rgb(180, 100, 0),
                    error: Color::Red,
                    selection_fg: Color::White,
                    selection_bg: Color::Blue,
                    diff_add: Color::Rgb(0, 120, 0),
                    diff_remove: Color::Rgb(180, 0, 0),
                    diff_hunk: Color::Magenta,
                    diff_meta: Color::Blue,
                    diff_ctx: Color::Gray,
                    diff_gutter: Color::Rgb(160, 160, 160),
                    diff_status: Color::Rgb(180, 100, 0),
                },
            Self::DarkAnsi => ThemePalette {
                    bg: Color::Black,
                    bg_panel: Color::Black,
                    bg_elevated: Color::Black,
                    text: Color::White,
                    text_muted: Color::DarkGray,
                    header_primary: Color::Green,
                    header_muted: Color::DarkGray,
                    accent_bright: Color::Green,
                    accent_glow: Color::Cyan,
                    border: Color::Green,
                    border_inactive: Color::DarkGray,
                    composer_border: Color::Cyan,
                    composer_border_blocked: Color::DarkGray,
                    status_muted: Color::DarkGray,
                    mode_tag: Color::Cyan,
                    warning: Color::Yellow,
                    error: Color::Red,
                    selection_fg: Color::Black,
                    selection_bg: Color::Green,
                    diff_add: Color::Green,
                    diff_remove: Color::Red,
                    diff_hunk: Color::Magenta,
                    diff_meta: Color::Cyan,
                    diff_ctx: Color::DarkGray,
                    diff_gutter: Color::Rgb(64, 64, 64),
                    diff_status: Color::Yellow,
                },
            Self::LightAnsi => ThemePalette {
                    bg: Color::White,
                    bg_panel: Color::White,
                    bg_elevated: Color::White,
                    text: Color::Black,
                    text_muted: Color::Gray,
                    header_primary: Color::Magenta,
                    header_muted: Color::Gray,
                    accent_bright: Color::Magenta,
                    accent_glow: Color::Blue,
                    border: Color::Magenta,
                    border_inactive: Color::Gray,
                    composer_border: Color::Blue,
                    composer_border_blocked: Color::Gray,
                    status_muted: Color::Gray,
                    mode_tag: Color::Blue,
                    warning: Color::Yellow,
                    error: Color::Red,
                    selection_fg: Color::White,
                    selection_bg: Color::Magenta,
                    diff_add: Color::Green,
                    diff_remove: Color::Red,
                    diff_hunk: Color::Magenta,
                    diff_meta: Color::Blue,
                    diff_ctx: Color::Gray,
                    diff_gutter: Color::Rgb(160, 160, 160),
                    diff_status: Color::Blue,
                },
            Self::Dim => ThemePalette {
                    bg: Color::Rgb(12, 12, 12),
                    bg_panel: Color::Rgb(18, 18, 18),
                    bg_elevated: Color::Rgb(28, 28, 28),
                    text: Color::Rgb(200, 200, 200),
                    text_muted: Color::Rgb(90, 90, 90),
                    header_primary: Color::Rgb(120, 160, 200),
                    header_muted: Color::Rgb(90, 90, 90),
                    accent_bright: Color::Rgb(140, 180, 220),
                    accent_glow: Color::Rgb(160, 140, 80),
                    border: Color::Rgb(100, 120, 140),
                    border_inactive: Color::Rgb(70, 70, 70),
                    composer_border: Color::Rgb(160, 140, 80),
                    composer_border_blocked: Color::Rgb(70, 70, 70),
                    status_muted: Color::Rgb(90, 90, 90),
                    mode_tag: Color::Rgb(160, 140, 80),
                    warning: Color::Rgb(200, 160, 60),
                    error: Color::Rgb(200, 80, 80),
                    selection_fg: Color::Rgb(12, 12, 12),
                    selection_bg: Color::Rgb(120, 160, 200),
                    diff_add: Color::Rgb(120, 200, 140),
                    diff_remove: Color::Rgb(200, 80, 80),
                    diff_hunk: Color::Rgb(180, 120, 200),
                    diff_meta: Color::Rgb(120, 160, 200),
                    diff_ctx: Color::Rgb(90, 90, 90),
                    diff_gutter: Color::Rgb(70, 70, 70),
                    diff_status: Color::Rgb(200, 160, 60),
                },
        }
    }
}

impl SessionAccent {
    pub const ALL: &'static [Self] = &[
        Self::Red,
        Self::Blue,
        Self::Green,
        Self::Yellow,
        Self::Purple,
        Self::Orange,
        Self::Pink,
        Self::Cyan,
    ];

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Red => "red",
            Self::Blue => "blue",
            Self::Green => "green",
            Self::Yellow => "yellow",
            Self::Purple => "purple",
            Self::Orange => "orange",
            Self::Pink => "pink",
            Self::Cyan => "cyan",
        }
    }

    #[must_use]
    pub fn from_slug(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "red" => Some(Self::Red),
            "blue" => Some(Self::Blue),
            "green" => Some(Self::Green),
            "yellow" => Some(Self::Yellow),
            "purple" => Some(Self::Purple),
            "orange" => Some(Self::Orange),
            "pink" => Some(Self::Pink),
            "cyan" => Some(Self::Cyan),
            _ => None,
        }
    }

    #[must_use]
    pub fn color(self) -> Color {
        match self {
            Self::Red => Color::Red,
            Self::Blue => Color::Blue,
            Self::Green => Color::Green,
            Self::Yellow => Color::Yellow,
            Self::Purple => Color::Magenta,
            Self::Orange => Color::Rgb(255, 140, 0),
            Self::Pink => Color::Rgb(255, 105, 180),
            Self::Cyan => Color::Cyan,
        }
    }
}

#[must_use]
pub fn resolve_palette(theme: TuiThemeSetting, accent: Option<SessionAccent>) -> ThemePalette {
    let mut palette = theme.palette();
    let Some(accent) = accent else {
        return palette;
    };
    if theme.is_drox_family() {
        // Thème Drox strict : l'accent ne recolore que les tags / badges.
        palette.mode_tag = accent.color();
        return palette;
    }
    palette.header_primary = accent.color();
    palette.border = accent.color();
    palette.composer_border = accent.color();
    palette.accent_bright = accent.color();
    palette
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_slug_round_trip() {
        for theme in TuiThemeSetting::ALL {
            assert_eq!(TuiThemeSetting::from_slug(theme.label()), Some(*theme));
        }
    }

    #[test]
    fn default_theme_is_drox() {
        assert_eq!(TuiThemeSetting::default(), TuiThemeSetting::Drox);
    }

    #[test]
    fn accent_overrides_primary_on_dark() {
        let p = resolve_palette(TuiThemeSetting::Dark, Some(SessionAccent::Orange));
        assert_eq!(p.header_primary, SessionAccent::Orange.color());
    }

    #[test]
    fn accent_limited_on_drox() {
        let base = TuiThemeSetting::Drox.palette();
        let p = resolve_palette(TuiThemeSetting::Drox, Some(SessionAccent::Orange));
        assert_eq!(p.header_primary, base.header_primary);
        assert_eq!(p.mode_tag, SessionAccent::Orange.color());
    }
}
