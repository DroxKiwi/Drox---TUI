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
    /// Vérifie si un check auto au boot est dû (opt-in + intervalle).
    #[must_use]
    pub fn should_run_startup_check(&self) -> bool {
        self.enabled && self.check_on_startup && self.is_check_due()
    }

    /// Intervalle minimum écoulé depuis `last_check_at`.
    #[must_use]
    pub fn is_check_due(&self) -> bool {
        let Some(last) = &self.last_check_at else {
            return true;
        };
        let Ok(last_dt) = chrono::DateTime::parse_from_rfc3339(last) else {
            return true;
        };
        let elapsed =
            chrono::Utc::now().signed_duration_since(last_dt.with_timezone(&chrono::Utc));
        elapsed.num_hours() >= i64::from(self.check_interval_hours)
    }

    #[must_use]
    pub fn is_snoozed(&self) -> bool {
        let Some(until) = &self.snooze_until else {
            return false;
        };
        let Ok(until_dt) = chrono::DateTime::parse_from_rfc3339(until) else {
            return false;
        };
        chrono::Utc::now() < until_dt.with_timezone(&chrono::Utc)
    }

    #[must_use]
    pub fn is_version_dismissed(&self, remote_version: &str) -> bool {
        let Some(dismissed) = &self.dismissed_version else {
            return false;
        };
        match (
            semver::Version::parse(remote_version),
            semver::Version::parse(dismissed),
        ) {
            (Ok(remote), Ok(dismissed_v)) => remote <= dismissed_v,
            _ => dismissed == remote_version,
        }
    }

    /// Afficher le bandeau MAJ (snooze / dismiss / opt-in).
    #[must_use]
    pub fn should_show_banner(&self, remote_version: &str, manual_check: bool) -> bool {
        if !manual_check && !self.enabled {
            return false;
        }
        if self.is_snoozed() {
            return false;
        }
        !self.is_version_dismissed(remote_version)
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_check_requires_opt_in() {
        let prefs = UpdatePrefs::default();
        assert!(!prefs.should_run_startup_check());
    }

    #[test]
    fn startup_check_when_enabled_and_due() {
        let prefs = UpdatePrefs {
            enabled: true,
            check_on_startup: true,
            ..UpdatePrefs::default()
        };
        assert!(prefs.should_run_startup_check());
    }

    #[test]
    fn banner_hidden_when_snoozed() {
        let until = (chrono::Utc::now() + chrono::Duration::days(1)).to_rfc3339();
        let prefs = UpdatePrefs {
            enabled: true,
            snooze_until: Some(until),
            ..UpdatePrefs::default()
        };
        assert!(!prefs.should_show_banner("9.9.9", true));
    }

    #[test]
    fn banner_hidden_when_dismissed() {
        let prefs = UpdatePrefs {
            enabled: true,
            dismissed_version: Some("2.0.5".into()),
            ..UpdatePrefs::default()
        };
        assert!(!prefs.should_show_banner("2.0.4", true));
        assert!(prefs.should_show_banner("2.0.6", true));
    }

    #[test]
    fn banner_requires_opt_in_without_manual_check() {
        let prefs = UpdatePrefs::default();
        assert!(!prefs.should_show_banner("9.9.9", false));
        assert!(prefs.should_show_banner("9.9.9", true));
    }
}
