//! Palettes TUI (`/theme`, `/color`).

use ratatui::style::Color;
use serde::{Deserialize, Serialize};

/// Thème global du terminal Drox.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum TuiThemeSetting {
    #[default]
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

/// Couleurs dérivées pour le rendu ratatui.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemePalette {
    pub header_primary: Color,
    pub header_muted: Color,
    pub border: Color,
    pub composer_border: Color,
    pub composer_border_blocked: Color,
    pub status_muted: Color,
    pub mode_tag: Color,
}

impl TuiThemeSetting {
    pub const ALL: &'static [Self] = &[
        Self::Dark,
        Self::Light,
        Self::DarkAnsi,
        Self::LightAnsi,
        Self::Dim,
    ];

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
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
            Self::Dark => ThemePalette {
                header_primary: Color::Cyan,
                header_muted: Color::DarkGray,
                border: Color::Cyan,
                composer_border: Color::Yellow,
                composer_border_blocked: Color::DarkGray,
                status_muted: Color::DarkGray,
                mode_tag: Color::Yellow,
            },
            Self::Light => ThemePalette {
                header_primary: Color::Blue,
                header_muted: Color::Gray,
                border: Color::Blue,
                composer_border: Color::Rgb(180, 100, 0),
                composer_border_blocked: Color::Gray,
                status_muted: Color::Gray,
                mode_tag: Color::Rgb(180, 100, 0),
            },
            Self::DarkAnsi => ThemePalette {
                header_primary: Color::Green,
                header_muted: Color::DarkGray,
                border: Color::Green,
                composer_border: Color::Cyan,
                composer_border_blocked: Color::DarkGray,
                status_muted: Color::DarkGray,
                mode_tag: Color::Cyan,
            },
            Self::LightAnsi => ThemePalette {
                header_primary: Color::Magenta,
                header_muted: Color::Gray,
                border: Color::Magenta,
                composer_border: Color::Blue,
                composer_border_blocked: Color::Gray,
                status_muted: Color::Gray,
                mode_tag: Color::Blue,
            },
            Self::Dim => ThemePalette {
                header_primary: Color::Rgb(120, 160, 200),
                header_muted: Color::Rgb(90, 90, 90),
                border: Color::Rgb(100, 120, 140),
                composer_border: Color::Rgb(160, 140, 80),
                composer_border_blocked: Color::Rgb(70, 70, 70),
                status_muted: Color::Rgb(90, 90, 90),
                mode_tag: Color::Rgb(160, 140, 80),
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
    if let Some(accent) = accent {
        palette.header_primary = accent.color();
        palette.border = accent.color();
        palette.composer_border = accent.color();
    }
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
    fn accent_overrides_primary() {
        let p = resolve_palette(TuiThemeSetting::Dark, Some(SessionAccent::Orange));
        assert_eq!(p.header_primary, SessionAccent::Orange.color());
    }
}
