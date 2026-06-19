//! Layout principal ratatui.

pub mod layout;
pub mod theme;

pub use layout::draw;
pub use theme::{resolve_palette, SessionAccent, ThemePalette, TuiThemeSetting};
