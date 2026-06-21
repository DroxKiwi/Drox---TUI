//! Préférences TUI persistantes (`~/.drox/tui-preferences.json`).

use std::collections::HashSet;
use std::io;
use std::sync::Mutex;

use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};

use crate::i18n::UiLocale;
use crate::ui::{SessionAccent, TuiThemeSetting};

use super::connection_library::{
    migrate_library_from_legacy, profile_to_legacy_prefs, ConnectionLibrary,
};
use super::validate_workspace_path;

use super::update_prefs::UpdatePrefs;

static PREFS_PATH_OVERRIDE: Mutex<Option<Utf8PathBuf>> = Mutex::new(None);

#[cfg(test)]
static PREFS_TEST_LOCK: Mutex<()> = Mutex::new(());

/// Moteur IA supporté (extensible).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LlmEngineKind {
    #[default]
    Ollama,
}

impl LlmEngineKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Ollama => "Ollama",
        }
    }
}

/// Connexion serveur IA persistée.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LlmConnectionPrefs {
    #[serde(default)]
    pub engine: LlmEngineKind,
    pub server: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    pub model: String,
    #[serde(default = "default_num_ctx_pref")]
    pub num_ctx: i64,
    #[serde(default = "default_max_iterations_pref")]
    pub max_iterations: usize,
}

pub(crate) fn default_max_iterations_pref() -> usize {
    crate::engine::default_max_iterations()
}

pub(crate) fn default_num_ctx_pref() -> i64 {
    crate::engine::default_num_ctx()
}

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
    /// Animations UI (curseur, pulse, modales).
    #[serde(default = "default_true")]
    pub animations_enabled: bool,
    /// Souris (scroll fil, clic modales).
    #[serde(default = "default_true")]
    pub mouse_enabled: bool,
    /// Connexion serveur IA (`/server`) — legacy, synchronisé avec `connection_library`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llm_connection: Option<LlmConnectionPrefs>,
    /// Bibliothèque de profils LLM (presets + custom).
    #[serde(default)]
    pub connection_library: ConnectionLibrary,
    /// Workspaces récents (`/workspace`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recent_workspaces: Vec<String>,
    /// Version schéma prefs UI (migrations charte 2.0.2+).
    #[serde(default)]
    pub ui_prefs_version: u32,
    /// Langue interface (`fr` / `en`).
    #[serde(default)]
    pub ui_locale: UiLocale,
    /// Vérification MAJ TUI opt-in (`/update`).
    #[serde(default)]
    pub update: UpdatePrefs,
}

/// Version courante du schéma `tui-preferences.json`.
pub const UI_PREFS_VERSION: u32 = 2;

/// Résultat de chargement (migration thème éventuelle).
pub struct LoadedPreferences {
    pub prefs: TuiPreferences,
    /// `dark`/`dark-ansi` migré vers `drox` à l'ouverture.
    pub theme_migrated_to_drox: bool,
}

fn default_true() -> bool {
    true
}

impl Default for TuiPreferences {
    fn default() -> Self {
        Self {
            theme: TuiThemeSetting::default(),
            session_color: None,
            terminal_title_from_rename: true,
            copy_full_response: false,
            vim_enabled: false,
            onboarding_done: false,
            animations_enabled: true,
            mouse_enabled: true,
            llm_connection: None,
            connection_library: ConnectionLibrary::default(),
            recent_workspaces: Vec::new(),
            ui_prefs_version: UI_PREFS_VERSION,
            ui_locale: UiLocale::default(),
            update: UpdatePrefs::default(),
        }
    }
}

/// URL Ollama préremplie dans le modal `/server` (pas un modèle implicite).
pub const OLLAMA_DEFAULT_SERVER: &str = "http://localhost:11434";

/// Placeholder interne — le moteur boot sans modèle réel tant que l'utilisateur n'a pas configuré.
pub const LLM_BOOT_PLACEHOLDER_MODEL: &str = "-";

/// Applique les préférences LLM persistées à la config de démarrage.
pub fn apply_llm_prefs_to_config(config: &mut crate::app::AppConfig, prefs: &LlmConnectionPrefs) {
    config.server = prefs.server.clone();
    config.model = prefs.model.clone();
    config.api_key = prefs.api_key.clone();
    config.num_ctx = prefs.num_ctx;
    config.max_iterations = prefs.max_iterations;
}

/// Normalise prefs (migration bibliothèque, sync legacy ↔ profil actif).
#[must_use]
pub fn normalize_preferences(prefs: TuiPreferences) -> TuiPreferences {
    normalize_preferences_inner(prefs).0
}

fn normalize_preferences_inner(mut prefs: TuiPreferences) -> (TuiPreferences, bool, bool) {
    let mut theme_migrated = false;

    if prefs.ui_prefs_version < UI_PREFS_VERSION {
        if matches!(
            prefs.theme,
            TuiThemeSetting::Dark | TuiThemeSetting::DarkAnsi
        ) {
            prefs.theme = TuiThemeSetting::Drox;
            theme_migrated = true;
        }
        prefs.ui_prefs_version = UI_PREFS_VERSION;
    }

    let (recents, recents_pruned) = sanitize_recent_workspaces(prefs.recent_workspaces);
    prefs.recent_workspaces = recents;

    prefs.connection_library =
        migrate_library_from_legacy(prefs.connection_library.clone(), prefs.llm_connection.as_ref());
    prefs.connection_library.ensure_builtin_presets();

    if let Some(active_id) = prefs.connection_library.active_profile_id.clone() {
        if let Some(active) = prefs
            .connection_library
            .profiles
            .iter_mut()
            .find(|p| p.id == active_id)
        {
            if active.is_usable() && !active.connection_verified {
                active.connection_verified = true;
            }
        }
    }

    if let Some(active) = prefs.connection_library.active_profile() {
        if let Some(legacy) = profile_to_legacy_prefs(active) {
            prefs.llm_connection = Some(legacy);
        }
    }

    (prefs, theme_migrated, recents_pruned)
}

/// Garde uniquement les répertoires existants (chemins canoniques), ordre préservé.
#[must_use]
pub fn usable_recent_workspaces(recents: &[String], exclude: Option<&str>) -> Vec<String> {
    let exclude_key = exclude.and_then(|p| validate_workspace_path(p).ok().map(|c| c.to_string()));
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for p in recents {
        let Ok(canonical) = validate_workspace_path(p) else {
            continue;
        };
        let key = canonical.to_string();
        if exclude_key.as_ref() == Some(&key) {
            continue;
        }
        if seen.insert(key.clone()) {
            out.push(key);
        }
    }
    out.truncate(MAX_RECENT_WORKSPACES);
    out
}

fn sanitize_recent_workspaces(recents: Vec<String>) -> (Vec<String>, bool) {
    let cleaned = usable_recent_workspaces(&recents, None);
    let changed = cleaned.len() != recents.len()
        || cleaned
            .iter()
            .zip(recents.iter())
            .any(|(a, b)| a != b);
    (cleaned, changed)
}

/// Résout la connexion IA au démarrage TUI.
///
/// Priorité : préférences persistées → flags/env CLI explicites → non configuré.
#[must_use]
pub fn resolve_llm_startup(
    config: &mut crate::app::AppConfig,
    prefs: &TuiPreferences,
) -> bool {
    if let Some(ref conn) = prefs.llm_connection {
        if is_valid_saved_connection(conn) {
            apply_llm_prefs_to_config(config, conn);
            return true;
        }
    }

    let model_explicit = !config.model.trim().is_empty();
    let server_explicit = !config.server.trim().is_empty();

    if model_explicit {
        if !server_explicit {
            config.server = OLLAMA_DEFAULT_SERVER.to_string();
        }
        return true;
    }

    config.server = OLLAMA_DEFAULT_SERVER.to_string();
    config.model = LLM_BOOT_PLACEHOLDER_MODEL.to_string();
    config.api_key = None;
    config.num_ctx = crate::engine::default_num_ctx();
    false
}

pub(crate) fn is_valid_saved_connection(conn: &LlmConnectionPrefs) -> bool {
    !conn.server.trim().is_empty()
        && !conn.model.trim().is_empty()
        && conn.model != LLM_BOOT_PLACEHOLDER_MODEL
}

pub fn save_llm_connection(prefs: &LlmConnectionPrefs) -> anyhow::Result<()> {
    let mut all = load_preferences();
    all.llm_connection = Some(prefs.clone());
    all.connection_library
        .sync_from_legacy_fields(
            &prefs.server,
            prefs.api_key.as_deref(),
            &prefs.model,
            prefs.num_ctx,
            prefs.max_iterations,
        );
    save_preferences(&all)
}

/// Enregistre la bibliothèque complète (profils + actif).
pub fn save_connection_library(library: &ConnectionLibrary) -> anyhow::Result<()> {
    let mut all = load_preferences();
    all.connection_library = library.clone();
    all.connection_library.ensure_builtin_presets();
    if let Some(active) = all.connection_library.active_profile() {
        all.llm_connection = profile_to_legacy_prefs(active);
    }
    save_preferences(&all)
}

/// Construit une entrée persistable depuis la config runtime courante.
#[must_use]
pub fn llm_connection_from_config(config: &crate::app::AppConfig) -> Option<LlmConnectionPrefs> {
    if !is_valid_saved_connection(&LlmConnectionPrefs {
        engine: LlmEngineKind::Ollama,
        server: config.server.clone(),
        api_key: config.api_key.clone(),
        model: config.model.clone(),
        num_ctx: config.num_ctx,
        max_iterations: config.max_iterations,
    }) {
        return None;
    }
    Some(LlmConnectionPrefs {
        engine: LlmEngineKind::Ollama,
        server: config.server.clone(),
        api_key: config.api_key.clone(),
        model: config.model.clone(),
        num_ctx: config.num_ctx,
        max_iterations: config.max_iterations,
    })
}

/// Fusionne avec le disque pour ne pas ecraser `llm_connection` par erreur.
fn merge_with_disk(mut prefs: TuiPreferences) -> TuiPreferences {
    let path = preferences_path();
    let Ok(raw) = std::fs::read_to_string(path.as_std_path()) else {
        return prefs;
    };
    let Ok(disk) = serde_json::from_str::<TuiPreferences>(&raw) else {
        return prefs;
    };
    if prefs.llm_connection.is_none() {
        prefs.llm_connection = disk.llm_connection;
    }
    if prefs.connection_library.profiles.is_empty()
        && !disk.connection_library.profiles.is_empty()
    {
        prefs.connection_library = disk.connection_library.clone();
    }
    if prefs.recent_workspaces.is_empty() && !disk.recent_workspaces.is_empty() {
        prefs.recent_workspaces = disk.recent_workspaces;
    }
    prefs
}

const MAX_RECENT_WORKSPACES: usize = 10;

/// Enregistre un workspace dans l'historique récent (répertoire existant uniquement).
pub fn record_recent_workspace(path: &str) -> anyhow::Result<()> {
    let canonical = validate_workspace_path(path).map_err(anyhow::Error::msg)?;
    let path_str = canonical.to_string();
    let mut prefs = load_preferences();
    prefs.recent_workspaces.retain(|p| p != &path_str);
    prefs.recent_workspaces.insert(0, path_str);
    prefs.recent_workspaces.truncate(MAX_RECENT_WORKSPACES);
    save_preferences(&prefs)
}

#[must_use]
pub fn preferences_path() -> Utf8PathBuf {
    if let Ok(guard) = PREFS_PATH_OVERRIDE.lock() {
        if let Some(path) = guard.as_ref() {
            return path.clone();
        }
    }
    if let Ok(raw) = std::env::var("DROX_TUI_PREFS") {
        if let Ok(path) = Utf8PathBuf::from_path_buf(std::path::PathBuf::from(raw)) {
            return path;
        }
    }
    dirs::home_dir()
        .and_then(|h| Utf8PathBuf::from_path_buf(h.join(".drox").join("tui-preferences.json")).ok())
        .unwrap_or_else(|| Utf8PathBuf::from(".drox/tui-preferences.json"))
}

pub fn load_preferences() -> TuiPreferences {
    load_preferences_detailed().prefs
}

/// Charge les prefs et persiste les migrations one-shot (ex. thème Drox 2.0.2).
#[must_use]
pub fn load_preferences_detailed() -> LoadedPreferences {
    let path = preferences_path();
    let raw_prefs = match std::fs::read_to_string(path.as_std_path()) {
        Ok(raw) => match serde_json::from_str::<TuiPreferences>(&raw) {
            Ok(prefs) => Some(prefs),
            Err(e) => {
                tracing::warn!(
                    path = %path,
                    error = %e,
                    "tui-preferences.json illisible — valeurs par defaut"
                );
                None
            }
        },
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Err(e) => {
            tracing::warn!(
                path = %path,
                error = %e,
                "lecture tui-preferences.json impossible"
            );
            None
        }
    };

    let base = raw_prefs.unwrap_or_default();
    let (prefs, theme_migrated, recents_pruned) = normalize_preferences_inner(base);
    if theme_migrated || recents_pruned {
        let _ = save_preferences(&prefs);
    }
    LoadedPreferences {
        prefs,
        theme_migrated_to_drox: theme_migrated,
    }
}

pub fn save_preferences(prefs: &TuiPreferences) -> anyhow::Result<()> {
    let path = preferences_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent.as_std_path()).context("creation ~/.drox")?;
    }
    let merged = merge_with_disk(normalize_preferences(prefs.clone()));
    let body = serde_json::to_string_pretty(&merged).context("serialisation preferences")?;
    write_atomic(&path, body.as_bytes()).context("ecriture tui-preferences.json")?;
    tracing::info!(
        path = %path,
        llm = merged.llm_connection.is_some(),
        "tui-preferences.json enregistre"
    );
    Ok(())
}

fn write_atomic(path: &Utf8Path, body: &[u8]) -> io::Result<()> {
    let tmp = path.with_extension("json.tmp");
    std::fs::write(tmp.as_std_path(), body)?;
    if path.exists() {
        let bak = path.with_extension("json.bak");
        let _ = std::fs::remove_file(bak.as_std_path());
        let _ = std::fs::rename(path.as_std_path(), bak.as_std_path());
    }
    std::fs::rename(tmp.as_std_path(), path.as_std_path())?;
    Ok(())
}

#[cfg(test)]
fn set_preferences_path_for_tests(path: Utf8PathBuf) {
    *PREFS_PATH_OVERRIDE.lock().unwrap() = Some(path);
}

#[cfg(test)]
fn clear_preferences_path_for_tests() {
    *PREFS_PATH_OVERRIDE.lock().unwrap() = None;
}

pub fn mark_onboarding_done() -> anyhow::Result<()> {
    let mut prefs = load_preferences();
    prefs.onboarding_done = true;
    save_preferences(&prefs)
}

/// Lignes `/settings` — préférences TUI utilisateur.
#[must_use]
pub fn format_settings_lines(prefs: &TuiPreferences) -> Vec<String> {
    use crate::i18n::{self, keys, keys_p1 as k};

    let path = preferences_path();
    let mut lines = vec![
        i18n::t(keys::SETTINGS_TITLE).into(),
        format!("  {} : {path}", i18n::t(keys::SETTINGS_FILE)),
        format!("  {} : {:?}", i18n::t(keys::SETTINGS_THEME), prefs.theme),
        format!(
            "  {} : {}",
            i18n::t(keys::SETTINGS_ACCENT),
            prefs
                .session_color
                .map(|c| format!("{c:?}"))
                .unwrap_or_else(|| i18n::t(keys::SETTINGS_ACCENT_DEFAULT).to_string())
        ),
        format!("  {} : {}", i18n::t(keys::SETTINGS_VIM), prefs.vim_enabled),
        format!(
            "  {} : {}",
            i18n::t(keys::SETTINGS_ANIMATIONS),
            prefs.animations_enabled
        ),
        format!("  {} : {}", i18n::t(keys::SETTINGS_MOUSE), prefs.mouse_enabled),
        format!(
            "  {} : {}",
            i18n::t(keys::SETTINGS_COPY_FULL),
            prefs.copy_full_response
        ),
        format!(
            "  {} : {}",
            i18n::t(keys::SETTINGS_TITLE_RENAME),
            prefs.terminal_title_from_rename
        ),
        format!(
            "  {} : {}",
            i18n::t(keys::SETTINGS_ONBOARDING),
            prefs.onboarding_done
        ),
        format!(
            "  {} : {} ({})",
            i18n::t(keys::SETTINGS_LANGUAGE),
            prefs.ui_locale.label(),
            prefs.ui_locale.code()
        ),
        format!(
            "  {} : {}",
            i18n::t(crate::i18n::keys_update::SETTINGS_UPDATE),
            prefs.update.enabled
        ),
        i18n::t(keys::SETTINGS_HINTS).into(),
    ];
    if !prefs.recent_workspaces.is_empty() {
        lines.push(format!(
            "  {} : {} (via `/workspace`)",
            i18n::t(keys::SETTINGS_RECENT_WS),
            prefs.recent_workspaces.len()
        ));
    }
    if let Some(ref llm) = prefs.llm_connection {
        lines.push(i18n::t(keys::SETTINGS_LLM_SECTION).into());
        lines.push(format!(
            "  {} : {}",
            i18n::t(keys::SETTINGS_LLM_ENGINE),
            llm.engine.label()
        ));
        lines.push(format!(
            "  {} : {}",
            i18n::t(keys::SETTINGS_LLM_SERVER),
            llm.server
        ));
        lines.push(format!(
            "  {} : {}",
            i18n::t(keys::SETTINGS_LLM_MODEL),
            llm.model
        ));
        lines.push(format!(
            "  {} : {}",
            i18n::t(keys::SETTINGS_LLM_NUM_CTX),
            llm.num_ctx
        ));
        lines.push(format!(
            "  {} : {}",
            i18n::t(keys::SETTINGS_LLM_MAX_ITER),
            llm.max_iterations
        ));
        lines.push(format!(
            "  x-api-key : {}",
            if llm.api_key.as_ref().is_some_and(|k| !k.is_empty()) {
                i18n::t(k::SETTINGS_API_KEY_SET)
            } else {
                i18n::t(k::SETTINGS_API_KEY_ABSENT)
            }
        ));
    } else {
        lines.push(i18n::t(k::SETTINGS_LLM_UNCONFIGURED).into());
    }
    let mut lib = prefs.connection_library.clone();
    lib.ensure_builtin_presets();
    if !lib.profiles.is_empty() {
        lines.push(i18n::tf2(
            k::SETTINGS_PROFILES,
            &lib.profiles.len().to_string(),
            lib.active_profile()
                .map(|p| p.name.as_str())
                .unwrap_or("aucun"),
        ));
    }
    lines
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
        animations_enabled: state.animations_enabled,
        mouse_enabled: state.mouse_enabled,
        llm_connection: existing.llm_connection.clone(),
        connection_library: existing.connection_library.clone(),
        recent_workspaces: existing.recent_workspaces.clone(),
        ui_prefs_version: existing.ui_prefs_version.max(UI_PREFS_VERSION),
        ui_locale: existing.ui_locale,
        update: existing.update.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_update_prefs_opt_in_off() {
        let prefs = TuiPreferences::default();
        assert!(!prefs.update.enabled);
        assert!(!prefs.update.check_on_startup);
        let json = serde_json::to_string(&prefs).unwrap();
        assert!(json.contains("\"update\""));
    }

    #[test]
    fn default_preferences_json() {
        let prefs = TuiPreferences::default();
        let json = serde_json::to_string(&prefs).unwrap();
        assert!(json.contains("drox"));
    }

    #[test]
    fn resolve_unconfigured_without_prefs_or_cli() {
        let mut config = crate::app::AppConfig {
            server: String::new(),
            model: String::new(),
            workspace: camino::Utf8PathBuf::from("."),
            apply: false,
            plan_mode: false,
            mode: None,
            allow: Vec::new(),
            ask: Vec::new(),
            deny: Vec::new(),
            no_settings: false,
            max_iterations: 12,
            api_key: None,
            num_ctx: crate::engine::default_num_ctx(),
            session: None,
            session_dir: None,
        };
        assert!(!resolve_llm_startup(&mut config, &TuiPreferences::default()));
        assert_eq!(config.model, LLM_BOOT_PLACEHOLDER_MODEL);
        assert_eq!(config.server, OLLAMA_DEFAULT_SERVER);
    }

    #[test]
    fn resolve_from_saved_prefs() {
        let mut config = crate::app::AppConfig {
            server: String::new(),
            model: String::new(),
            workspace: camino::Utf8PathBuf::from("."),
            apply: false,
            plan_mode: false,
            mode: None,
            allow: Vec::new(),
            ask: Vec::new(),
            deny: Vec::new(),
            no_settings: false,
            max_iterations: 12,
            api_key: None,
            num_ctx: crate::engine::default_num_ctx(),
            session: None,
            session_dir: None,
        };
        let prefs = TuiPreferences {
            llm_connection: Some(LlmConnectionPrefs {
                engine: LlmEngineKind::Ollama,
                server: "http://127.0.0.1:11434".into(),
                api_key: None,
                model: "qwen2.5".into(),
                num_ctx: crate::engine::default_num_ctx(),
                max_iterations: crate::engine::default_max_iterations(),
            }),
            ..TuiPreferences::default()
        };
        assert!(resolve_llm_startup(&mut config, &prefs));
        assert_eq!(config.model, "qwen2.5");
    }

    #[test]
    fn resolve_from_explicit_cli_model() {
        let mut config = crate::app::AppConfig {
            server: String::new(),
            model: "mistral".into(),
            workspace: camino::Utf8PathBuf::from("."),
            apply: false,
            plan_mode: false,
            mode: None,
            allow: Vec::new(),
            ask: Vec::new(),
            deny: Vec::new(),
            no_settings: false,
            max_iterations: 12,
            api_key: None,
            num_ctx: crate::engine::default_num_ctx(),
            session: None,
            session_dir: None,
        };
        assert!(resolve_llm_startup(&mut config, &TuiPreferences::default()));
        assert_eq!(config.model, "mistral");
        assert_eq!(config.server, OLLAMA_DEFAULT_SERVER);
    }

    #[test]
    fn record_recent_workspace_dedupes_and_caps() {
        let _lock = PREFS_TEST_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = Utf8PathBuf::from_path_buf(dir.path().join("tui-preferences.json")).unwrap();
        set_preferences_path_for_tests(path);

        let mut prefs = TuiPreferences::default();
        for i in 0..12 {
            let ws = dir.path().join(format!("w{i}"));
            std::fs::create_dir_all(&ws).unwrap();
            prefs
                .recent_workspaces
                .push(ws.to_string_lossy().into_owned());
        }
        save_preferences(&prefs).unwrap();
        let new_ws = dir.path().join("new");
        std::fs::create_dir_all(&new_ws).unwrap();
        record_recent_workspace(new_ws.to_str().unwrap()).unwrap();
        let loaded = load_preferences();
        let first = loaded.recent_workspaces.first().expect("recent");
        assert_eq!(
            validate_workspace_path(first).expect("canonical"),
            validate_workspace_path(new_ws.to_str().unwrap()).expect("canonical new")
        );
        assert!(loaded.recent_workspaces.len() <= MAX_RECENT_WORKSPACES);

        clear_preferences_path_for_tests();
    }

    #[test]
    fn sanitize_recent_workspaces_drops_missing_paths() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("proj");
        std::fs::create_dir_all(&real).unwrap();
        let real_str = real.to_string_lossy().into_owned();
        let prefs = normalize_preferences(TuiPreferences {
            recent_workspaces: vec![
                "/tmp/ghost".into(),
                real_str.clone(),
                "/tmp/w0".into(),
            ],
            ..Default::default()
        });
        assert_eq!(prefs.recent_workspaces.len(), 1);
        assert_eq!(
            validate_workspace_path(&prefs.recent_workspaces[0]).expect("canonical"),
            validate_workspace_path(&real_str).expect("canonical real")
        );
    }

    #[test]
    fn save_and_reload_llm_connection_roundtrip() {
        let _lock = PREFS_TEST_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = Utf8PathBuf::from_path_buf(dir.path().join("tui-preferences.json")).unwrap();
        set_preferences_path_for_tests(path);

        let conn = LlmConnectionPrefs {
            engine: LlmEngineKind::Ollama,
            server: "http://127.0.0.1:11434".into(),
            api_key: None,
            model: "qwen2.5".into(),
            num_ctx: 65_536,
            max_iterations: 40,
        };
        save_llm_connection(&conn).unwrap();
        let loaded = load_preferences();
        assert_eq!(loaded.llm_connection.as_ref(), Some(&conn));

        clear_preferences_path_for_tests();
    }

    #[test]
    fn save_llm_connection_updates_library() {
        let _lock = PREFS_TEST_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = Utf8PathBuf::from_path_buf(dir.path().join("tui-preferences.json")).unwrap();
        set_preferences_path_for_tests(path);

        let conn = LlmConnectionPrefs {
            engine: LlmEngineKind::Ollama,
            server: "http://127.0.0.1:11434".into(),
            api_key: Some("secret".into()),
            model: "qwen2.5".into(),
            num_ctx: 65_536,
            max_iterations: 40,
        };
        save_llm_connection(&conn).unwrap();
        let loaded = load_preferences();
        assert_eq!(loaded.llm_connection.as_ref(), Some(&conn));
        assert!(loaded.connection_library.active_profile().is_some());
        assert_eq!(
            loaded
                .connection_library
                .active_profile()
                .and_then(|p| p.default_model.as_deref()),
            Some("qwen2.5")
        );

        clear_preferences_path_for_tests();
    }

    #[test]
    fn merge_with_disk_preserves_llm_connection() {
        let _lock = PREFS_TEST_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = Utf8PathBuf::from_path_buf(dir.path().join("tui-preferences.json")).unwrap();
        set_preferences_path_for_tests(path);

        let conn = LlmConnectionPrefs {
            engine: LlmEngineKind::Ollama,
            server: "http://127.0.0.1:11434".into(),
            api_key: None,
            model: "qwen2.5".into(),
            num_ctx: 32_768,
            max_iterations: 12,
        };
        save_llm_connection(&conn).unwrap();

        let mut theme_only = TuiPreferences::default();
        theme_only.theme = TuiThemeSetting::Light;
        save_preferences(&theme_only).unwrap();

        let loaded = load_preferences();
        assert_eq!(loaded.llm_connection.as_ref(), Some(&conn));
        assert_eq!(loaded.theme, TuiThemeSetting::Light);

        clear_preferences_path_for_tests();
    }

    #[test]
    fn migrates_legacy_dark_theme_to_drox() {
        let _lock = PREFS_TEST_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = Utf8PathBuf::from_path_buf(dir.path().join("tui-preferences.json")).unwrap();
        set_preferences_path_for_tests(path.clone());

        let legacy = r#"{"theme":"dark","onboarding_done":true}"#;
        std::fs::write(path.as_std_path(), legacy).unwrap();

        let loaded = load_preferences_detailed();
        assert_eq!(loaded.prefs.theme, TuiThemeSetting::Drox);
        assert!(loaded.theme_migrated_to_drox);
        assert_eq!(loaded.prefs.ui_prefs_version, UI_PREFS_VERSION);

        let on_disk = load_preferences();
        assert_eq!(on_disk.theme, TuiThemeSetting::Drox);

        clear_preferences_path_for_tests();
    }
}
