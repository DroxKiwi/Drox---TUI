//! Hooks pre/post tool — V1 commandes shell (§2.6).
//!
//! Config : `<workspace>/.drox/hooks.json` (+ optionnel `~/.drox/hooks.json`).
//! Événements : `PreToolUse`, `PostToolUse`. Sémantique exit code alignée leak :
//! `0` = OK, `2` = bloquant (stderr au modèle), autre = avertissement utilisateur.

mod config;
mod exec;
mod matcher;
mod runner;

pub use config::{CommandHook, HookMatcherEntry, HooksFile, ToolHooksConfig, load_hooks_file};
pub use runner::{PostHookOutcome, PreHookOutcome, ToolHookContext};

/// Charge et fusionne les hooks projet + utilisateur (si présents).
#[must_use]
pub fn load_merged(workspace: &camino::Utf8Path) -> ToolHooksConfig {
    let mut merged = ToolHooksConfig::empty();
    if let Some(home) = dirs::home_dir() {
        if let Ok(user_utf) = camino::Utf8PathBuf::from_path_buf(home.join(".drox").join("hooks.json")) {
            if let Ok(user) = config::load_hooks_file(&user_utf) {
                merged.merge(user);
            }
        }
    }
    let project_path = workspace.join(".drox").join("hooks.json");
    if let Ok(project) = config::load_hooks_file(&project_path) {
        merged.merge(project);
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8Path;
    use std::fs;

    #[test]
    fn parses_hooks_file() {
        let dir = tempfile::tempdir().unwrap();
        let hooks_path = dir.path().join(".drox");
        fs::create_dir_all(&hooks_path).unwrap();
        fs::write(
            hooks_path.join("hooks.json"),
            r#"{
              "PreToolUse": [
                {
                  "matcher": "bash",
                  "hooks": [{ "type": "command", "command": "echo ok", "timeout": 5 }]
                }
              ]
            }"#,
        )
        .unwrap();
        let cfg = load_merged(Utf8Path::from_path(dir.path()).unwrap());
        assert_eq!(cfg.pre_tool_use.len(), 1);
        assert_eq!(cfg.pre_tool_use[0].matcher, "bash");
    }
}
