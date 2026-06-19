//! Chargement de `.drox/hooks.json` (format proche leak `settings.hooks`).

use camino::Utf8Path;
use serde::Deserialize;
use tracing::{debug, warn};

/// Fichier hooks déclaratif.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct HooksFile {
    #[serde(default, rename = "PreToolUse")]
    pub pre_tool_use: Vec<HookMatcherEntry>,
    #[serde(default, rename = "PostToolUse")]
    pub post_tool_use: Vec<HookMatcherEntry>,
    #[serde(default)]
    pub settings: HooksSettings,
}

/// Paramètres globaux du fichier hooks.
#[derive(Debug, Clone, Deserialize)]
pub struct HooksSettings {
    /// Timeout par défaut (secondes) pour chaque commande.
    #[serde(default = "default_timeout_secs")]
    pub default_timeout_secs: u64,
    /// Si non vide : seules ces commandes (1er token) sont autorisées.
    #[serde(default)]
    pub allowed_commands: Vec<String>,
}

impl Default for HooksSettings {
    fn default() -> Self {
        Self {
            default_timeout_secs: default_timeout_secs(),
            allowed_commands: Vec::new(),
        }
    }
}

const fn default_timeout_secs() -> u64 {
    60
}

/// Un matcher + liste de hooks à exécuter.
#[derive(Debug, Clone, Deserialize)]
pub struct HookMatcherEntry {
    pub matcher: String,
    #[serde(default)]
    pub hooks: Vec<CommandHook>,
}

/// Hook shell — seul type supporté en V1.
#[derive(Debug, Clone, Deserialize)]
pub struct CommandHook {
    #[serde(rename = "type")]
    pub hook_type: String,
    pub command: String,
    #[serde(default)]
    pub timeout: Option<u64>,
    #[serde(rename = "if", default)]
    pub if_condition: Option<String>,
}

/// Configuration fusionnée prête pour le moteur.
#[derive(Debug, Clone, Default)]
pub struct ToolHooksConfig {
    pub pre_tool_use: Vec<HookMatcherEntry>,
    pub post_tool_use: Vec<HookMatcherEntry>,
    pub settings: HooksSettings,
}

impl ToolHooksConfig {
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn is_enabled(&self) -> bool {
        !self.pre_tool_use.is_empty() || !self.post_tool_use.is_empty()
    }

    pub fn merge(&mut self, other: HooksFile) {
        self.pre_tool_use.extend(other.pre_tool_use);
        self.post_tool_use.extend(other.post_tool_use);
        if !other.settings.allowed_commands.is_empty() {
            self.settings.allowed_commands = other.settings.allowed_commands;
        }
        if other.settings.default_timeout_secs != default_timeout_secs() {
            self.settings.default_timeout_secs = other.settings.default_timeout_secs;
        }
    }
}

/// Charge un fichier `hooks.json` depuis le disque.
pub fn load_hooks_file(path: &Utf8Path) -> Result<HooksFile, String> {
    let raw = std::fs::read_to_string(path).map_err(|e| format!("read {path}: {e}"))?;
    let file: HooksFile =
        serde_json::from_str(&raw).map_err(|e| format!("parse {path}: {e}"))?;
    validate_file(&file)?;
    debug!(path = %path, pre = file.pre_tool_use.len(), post = file.post_tool_use.len(), "hooks loaded");
    Ok(file)
}

fn validate_file(file: &HooksFile) -> Result<(), String> {
    for entry in file
        .pre_tool_use
        .iter()
        .chain(file.post_tool_use.iter())
    {
        for hook in &entry.hooks {
            if hook.hook_type != "command" {
                return Err(format!(
                    "unsupported hook type `{}` (only `command` in V1)",
                    hook.hook_type
                ));
            }
            if hook.command.trim().is_empty() {
                return Err("hook command must not be empty".into());
            }
        }
    }
    Ok(())
}

/// Vérifie que la commande est autorisée par l'allowlist (si configurée).
pub fn command_allowed(settings: &HooksSettings, command: &str) -> Result<(), String> {
    if settings.allowed_commands.is_empty() {
        return Ok(());
    }
    let first = command
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim_matches(|c: char| c == '"' || c == '\'');
    if settings.allowed_commands.iter().any(|a| a == first) {
        Ok(())
    } else {
        warn!(command = first, "hook command blocked by allowed_commands");
        Err(format!(
            "hook command `{first}` is not in allowed_commands {:?}",
            settings.allowed_commands
        ))
    }
}
