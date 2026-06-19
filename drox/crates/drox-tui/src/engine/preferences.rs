//! Préférences TUI persistantes (`~/.drox/tui-preferences.json`).

use std::io;

use anyhow::Context;
use camino::Utf8PathBuf;

use crate::ui::{SessionAccent, TuiThemeSetting};

/// Préférences utilisateur TUI.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TuiPreferences {
    #[serde(default)]
    pub theme: TuiThemeSetting,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_color: Option<SessionAccent>,
    #[serde(default = "default_true")]
    pub terminal_title_from_rename: bool,
    /// Copier la réponse complète sans sélecteur de blocs.
    #[serde(default)]
    pub copy_full_response: bool,
    /// Mode vim dans le composer (`/vim` pour basculer).
    #[serde(default)]
    pub vim_enabled: bool,
    /// Onboarding TUI terminé (premier lancement).
    #[serde(default)]
    pub onboarding_done: bool,
}

fn default_true() -> bool {
    true
}

impl Default for TuiPreferences {
    fn default() -> Self {
        Self {
            theme: TuiThemeSetting::Dark,
            session_color: None,
            terminal_title_from_rename: true,
            copy_full_response: false,
            vim_enabled: false,
            onboarding_done: false,
        }
    }
}

#[must_use]
pub fn preferences_path() -> Utf8PathBuf {
    dirs::home_dir()
        .and_then(|h| Utf8PathBuf::from_path_buf(h.join(".drox").join("tui-preferences.json")).ok())
        .unwrap_or_else(|| Utf8PathBuf::from(".drox/tui-preferences.json"))
}

pub fn load_preferences() -> TuiPreferences {
    let path = preferences_path();
    match std::fs::read_to_string(path.as_std_path()) {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
        Err(e) if e.kind() == io::ErrorKind::NotFound => TuiPreferences::default(),
        Err(_) => TuiPreferences::default(),
    }
}

pub fn save_preferences(prefs: &TuiPreferences) -> anyhow::Result<()> {
    let path = preferences_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent.as_std_path()).context("création ~/.drox")?;
    }
    let body = serde_json::to_string_pretty(prefs).context("sérialisation préférences")?;
    std::fs::write(path.as_std_path(), body).context("écriture tui-preferences.json")?;
    Ok(())
}

pub fn mark_onboarding_done() -> anyhow::Result<()> {
    let mut prefs = load_preferences();
    prefs.onboarding_done = true;
    save_preferences(&prefs)
}

/// Lignes `/settings` — préférences TUI utilisateur.
#[must_use]
pub fn format_settings_lines(prefs: &TuiPreferences) -> Vec<String> {
    let path = preferences_path();
    vec![
        "Réglages TUI (`~/.drox/tui-preferences.json`)".into(),
        format!("  fichier : {path}"),
        format!("  thème : {:?}", prefs.theme),
        format!(
            "  accent session : {}",
            prefs
                .session_color
                .map(|c| format!("{c:?}"))
                .unwrap_or_else(|| "défaut".into())
        ),
        format!("  vim composer : {}", prefs.vim_enabled),
        format!("  /copy réponse complète : {}", prefs.copy_full_response),
        format!("  titre terminal depuis /rename : {}", prefs.terminal_title_from_rename),
        format!("  onboarding vu : {}", prefs.onboarding_done),
        "Modifier : `/theme` · `/color` · `/vim` · `/onboarding`".into(),
    ]
}

pub fn persist_from_state(state: &crate::app::AppState) -> anyhow::Result<()> {
    let existing = load_preferences();
    save_preferences(&TuiPreferences {
        theme: state.theme,
        session_color: state.session_accent,
        terminal_title_from_rename: existing.terminal_title_from_rename,
        copy_full_response: existing.copy_full_response,
        vim_enabled: existing.vim_enabled,
        onboarding_done: existing.onboarding_done,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_preferences_json() {
        let prefs = TuiPreferences::default();
        let json = serde_json::to_string(&prefs).unwrap();
        assert!(json.contains("dark"));
    }
}
