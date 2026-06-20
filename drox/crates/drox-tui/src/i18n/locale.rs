//! Locale interface TUI (FR / EN).

use serde::{Deserialize, Serialize};

/// Langue affichée dans le TUI (indépendante de la langue agent).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UiLocale {
    Fr,
    En,
}

impl Default for UiLocale {
    fn default() -> Self {
        Self::detect_os()
    }
}

impl UiLocale {
    /// Détecte la locale depuis les variables d'environnement (`LANG`, `LC_*`).
    #[must_use]
    pub fn detect_os() -> Self {
        for key in ["LC_ALL", "LC_MESSAGES", "LANG", "LANGUAGE"] {
            if let Ok(v) = std::env::var(key) {
                if let Some(locale) = Self::parse(&v) {
                    return locale;
                }
            }
        }
        Self::En
    }

    /// Parse `fr`, `en`, `fr-FR`, `en_US`, etc.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        let lower = s.trim().to_ascii_lowercase();
        let code = lower.split(['-', '_', '.']).next().unwrap_or(&lower);
        match code {
            "fr" => Some(Self::Fr),
            "en" => Some(Self::En),
            _ => None,
        }
    }

    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Fr => "fr",
            Self::En => "en",
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Fr => "Français",
            Self::En => "English",
        }
    }
}
