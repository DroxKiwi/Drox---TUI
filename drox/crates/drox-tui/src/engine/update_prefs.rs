//! Préférences mises à jour opt-in (`/update`).

use serde::{Deserialize, Serialize};

/// Réglages vérification / installation MAJ TUI (dépôt OR).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdatePrefs {
    /// Master switch — aucune requête auto sans consentement.
    #[serde(default)]
    pub enabled: bool,
    /// Si `enabled`, vérifier au lancement (respecte `check_interval_hours`).
    #[serde(default)]
    pub check_on_startup: bool,
    #[serde(default = "default_check_interval_hours")]
    pub check_interval_hours: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snooze_until: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dismissed_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub install_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_check_at: Option<String>,
}

fn default_check_interval_hours() -> u32 {
    24
}

impl Default for UpdatePrefs {
    fn default() -> Self {
        Self {
            enabled: false,
            check_on_startup: false,
            check_interval_hours: default_check_interval_hours(),
            snooze_until: None,
            dismissed_version: None,
            install_path: None,
            last_check_at: None,
        }
    }
}

impl UpdatePrefs {
    /// Lignes `/update` (état local, sans requête réseau).
    #[must_use]
    pub fn format_status_lines(&self) -> Vec<String> {
        use crate::i18n::{self, keys_update as u};

        let version = env!("CARGO_PKG_VERSION");
        let mut lines = vec![
            i18n::t(u::UPDATE_STATUS_HEADER).into(),
            i18n::tf(u::UPDATE_STATUS_VERSION, version),
            i18n::t(if self.enabled {
                u::UPDATE_STATUS_ENABLED
            } else {
                u::UPDATE_STATUS_DISABLED
            })
            .into(),
        ];
        if let Some(ref v) = self.dismissed_version {
            lines.push(i18n::tf(u::UPDATE_STATUS_DISMISSED, v));
        }
        if let Some(ref until) = self.snooze_until {
            lines.push(i18n::tf(u::UPDATE_STATUS_SNOOZE, until));
        }
        match &self.last_check_at {
            Some(t) => lines.push(i18n::tf(u::UPDATE_STATUS_LAST_CHECK, t)),
            None => lines.push(i18n::t(u::UPDATE_STATUS_NEVER_CHECKED).into()),
        }
        lines.push(i18n::t(u::UPDATE_STATUS_HINTS).into());
        lines
    }
}
