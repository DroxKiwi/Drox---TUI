//! Introspection configuration (`/config`).

use std::path::Path;

use camino::{Utf8Path, Utf8PathBuf};
use drox_permissions::{LayeredConfig, SettingsFile};

use crate::app::AppConfig;

use super::EngineRuntime;

impl EngineRuntime {
    /// Lignes pour `/config` — état effectif + fichiers sur disque.
    #[must_use]
    pub fn format_config_lines(&self) -> Vec<String> {
        format_runtime_config(self.boot_config(), self)
    }
}

/// Formate la configuration pour affichage TUI.
#[must_use]
pub fn format_runtime_config(boot: &AppConfig, rt: &EngineRuntime) -> Vec<String> {
    let mut lines = vec!["Configuration Drox (session TUI)".into()];

    lines.push("— LLM".into());
    lines.push(format!("  server : {}", boot.server));
    lines.push(format!("  model : {}", rt.model_label));
    lines.push(format!("  num_ctx : {}", rt.num_ctx));
    lines.push(format!(
        "  api_key : {}",
        if boot.api_key.is_some() || env_set("DROX_API_KEY") {
            "définie"
        } else {
            "absente"
        }
    ));
    append_env_llm(&mut lines);

    lines.push("— Workspace".into());
    lines.push(format!("  path : {}", rt.workspace));
    lines.push(format!("  apply (--apply) : {}", rt.apply_fs_writes));
    lines.push(format!("  max_iterations : {}", rt.max_iterations));

    lines.push("— Permissions".into());
    lines.push(format!(
        "  mode session : {} (plan={})",
        rt.permission_mode().short_title(),
        rt.plan_mode()
    ));
    if boot.no_settings {
        lines.push("  settings fichiers : ignorés (--no-settings)".into());
    } else {
        let paths = settings_paths(&rt.workspace);
        lines.push("  settings fichiers :".into());
        lines.push(summarize_settings_file("utilisateur", &paths.user));
        lines.push(summarize_settings_file("projet", &paths.project));
        lines.push(summarize_settings_file("local", &paths.local));
        if let Ok(layered) = LayeredConfig::load(
            Some(paths.user.as_path()),
            Some(paths.project.as_path()),
            Some(paths.local.as_path()),
        ) {
            if let Some(m) = layered.effective_mode() {
                lines.push(format!("  mode fichier effectif : {m:?}"));
            }
        }
    }
    let cli_rules = boot.allow.len() + boot.ask.len() + boot.deny.len();
    if cli_rules > 0 || boot.mode.is_some() || boot.plan_mode {
        lines.push(format!(
            "  CLI : allow={} ask={} deny={}{}{}",
            boot.allow.len(),
            boot.ask.len(),
            boot.deny.len(),
            boot.mode
                .as_ref()
                .map(|m| format!(" --mode {m}"))
                .unwrap_or_default(),
            if boot.plan_mode { " --plan" } else { "" },
        ));
    }

    lines.push("— Fichiers env (chargés au boot, shell prioritaire)".into());
    for p in env_file_paths(&rt.workspace) {
        lines.push(format_file_status(&p));
    }

    lines.push("— Hooks".into());
    lines.push(format!(
        "  runtime : {}",
        if rt.hooks_active() {
            "actifs"
        } else {
            "inactifs"
        }
    ));
    lines.push(format_file_status(&rt.workspace.join(".drox/hooks.json")));
    if let Some(user) = user_hooks_path() {
        lines.push(format_file_status(&user));
    }

    lines.push("— Sessions".into());
    lines.push(format!("  répertoire : {}", rt.sessions_dir));
    lines.push(format!("  courante : {}", rt.session_id()));

    lines.push("Astuce : éditer settings.json puis relancer ; /permissions pour les règles effectives.".into());
    lines
}

struct SettingsPaths {
    user: std::path::PathBuf,
    project: std::path::PathBuf,
    local: std::path::PathBuf,
}

fn settings_paths(workspace: &Utf8Path) -> SettingsPaths {
    let user = dirs::home_dir()
        .map(|h| h.join(".drox").join("settings.json"))
        .unwrap_or_else(|| Path::new("/nonexistent").to_path_buf());
    let drox = workspace.as_std_path().join(".drox");
    SettingsPaths {
        user,
        project: drox.join("settings.json"),
        local: drox.join("settings.local.json"),
    }
}

fn summarize_settings_file(label: &str, path: &Path) -> String {
    if !path.is_file() {
        return format!("    {label} : {} (absent)", path.display());
    }
    match SettingsFile::load(path) {
        Ok(s) => {
            let mode = s
                .mode
                .map(|m| format!(" · mode={m:?}"))
                .unwrap_or_default();
            format!(
                "    {label} : {} — allow {} · ask {} · deny {}{mode}",
                path.display(),
                s.permissions.allow.len(),
                s.permissions.ask.len(),
                s.permissions.deny.len(),
            )
        }
        Err(e) => format!("    {label} : {} (parse : {e})", path.display()),
    }
}

fn env_file_paths(workspace: &Utf8Path) -> Vec<Utf8PathBuf> {
    let mut paths = vec![
        workspace.join(".drox/.env"),
        workspace.join(".drox/env"),
    ];
    if let Some(home) = dirs::home_dir() {
        if let Ok(p) = Utf8PathBuf::from_path_buf(home.join(".drox").join(".env")) {
            paths.push(p);
        }
        if let Ok(p) = Utf8PathBuf::from_path_buf(home.join(".drox").join("env")) {
            paths.push(p);
        }
    }
    paths
}

fn user_hooks_path() -> Option<Utf8PathBuf> {
    dirs::home_dir().and_then(|h| Utf8PathBuf::from_path_buf(h.join(".drox/hooks.json")).ok())
}

fn format_file_status(path: &Utf8Path) -> String {
    let status = if path.is_file() { "présent" } else { "absent" };
    format!("  {path} ({status})")
}

fn env_set(key: &str) -> bool {
    std::env::var(key)
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false)
}

fn append_env_llm(lines: &mut Vec<String>) {
    const KEYS: &[&str] = &[
        "DROX_NUM_CTX",
        "DROX_NUM_PREDICT",
        "DROX_TOP_P",
        "DROX_TOP_K",
        "DROX_REPEAT_PENALTY",
        "DROX_SEED",
        "DROX_KEEP_ALIVE",
    ];
    let mut any = false;
    for key in KEYS {
        if let Ok(v) = std::env::var(key) {
            if !v.trim().is_empty() {
                if !any {
                    lines.push("  env LLM :".into());
                    any = true;
                }
                lines.push(format!("    {key}={v}"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_settings_line() {
        let line = summarize_settings_file("projet", Path::new("/tmp/__drox_no_settings__.json"));
        assert!(line.contains("absent"));
    }
}
