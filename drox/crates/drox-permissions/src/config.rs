//! Chargement des règles depuis un fichier `settings.json`.
//!
//! Le format attendu est volontairement minimaliste (proche du fichier
//! `~/.drox/settings.json` que les utilisateurs vont écrire à la main) :
//!
//! ```json
//! {
//!   "permissions": {
//!     "allow": ["Bash(npm:*)", "FileRead"],
//!     "ask":   ["Bash(git push:*)"],
//!     "deny":  ["Bash(rm -rf *)"]
//!   },
//!   "mode": "default"
//! }
//! ```
//!
//! Plusieurs fichiers peuvent être chargés successivement et fusionnés dans
//! le même `RuleSet` (chacun avec sa propre `RuleSource`).

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::PermissionError;
use crate::mode::PermissionMode;
use crate::rule::{PermissionBehavior, Rule, RuleSet, RuleSource, parse_rule};

/// Représentation JSON d'un fichier de settings (permissions uniquement).
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsFile {
    /// Bloc permissions (allow / ask / deny).
    #[serde(default)]
    pub permissions: PermissionsBlock,
    /// Mode par défaut. `None` signifie "ne pas écraser le mode courant".
    #[serde(default)]
    pub mode: Option<PermissionMode>,
}

/// Sous-bloc `permissions` d'un settings file.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PermissionsBlock {
    #[serde(default)]
    pub allow: Vec<String>,
    #[serde(default)]
    pub ask: Vec<String>,
    #[serde(default)]
    pub deny: Vec<String>,
}

impl SettingsFile {
    /// Charge un settings file depuis un chemin.
    ///
    /// - Renvoie `Ok(default)` si le fichier n'existe pas.
    /// - Renvoie `Err` si le fichier existe mais ne parse pas.
    pub fn load(path: &Path) -> Result<Self, PermissionError> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(path)?;
        let file: Self = serde_json::from_str(&content)?;
        Ok(file)
    }

    /// Pousse les règles du fichier dans un `RuleSet` en les taggant avec
    /// `source`.
    pub fn merge_into(&self, set: &mut RuleSet, source: RuleSource) {
        for raw in &self.permissions.allow {
            set.push(Rule {
                value: parse_rule(raw),
                behavior: PermissionBehavior::Allow,
                source,
            });
        }
        for raw in &self.permissions.ask {
            set.push(Rule {
                value: parse_rule(raw),
                behavior: PermissionBehavior::Ask,
                source,
            });
        }
        for raw in &self.permissions.deny {
            set.push(Rule {
                value: parse_rule(raw),
                behavior: PermissionBehavior::Deny,
                source,
            });
        }
    }
}

/// Construit un `RuleSet` à partir d'un certain nombre de fichiers (user,
/// project, local) — chacun avec sa propre source. Les fichiers absents
/// sont ignorés silencieusement.
///
/// Renvoie aussi le `mode` retenu (priorité : local > project > user, si
/// défini ; sinon `None`).
#[derive(Debug, Default)]
pub struct LayeredConfig {
    pub user: Option<SettingsFile>,
    pub project: Option<SettingsFile>,
    pub local: Option<SettingsFile>,
}

impl LayeredConfig {
    /// Charge les trois fichiers attendus à des chemins explicites.
    ///
    /// Chaque chemin est optionnel ; les fichiers absents sont traités
    /// comme un settings vide.
    pub fn load(
        user_path: Option<&Path>,
        project_path: Option<&Path>,
        local_path: Option<&Path>,
    ) -> Result<Self, PermissionError> {
        Ok(Self {
            user: user_path.map(SettingsFile::load).transpose()?,
            project: project_path.map(SettingsFile::load).transpose()?,
            local: local_path.map(SettingsFile::load).transpose()?,
        })
    }

    /// Construit le `RuleSet` agrégé. L'ordre d'insertion (user → project →
    /// local) n'a **pas** d'impact fonctionnel : le moteur applique son
    /// pipeline (`Deny > Ask > Allow`) globalement.
    #[must_use]
    pub fn build_rule_set(&self) -> RuleSet {
        let mut set = RuleSet::new();
        if let Some(user) = &self.user {
            user.merge_into(&mut set, RuleSource::UserSettings);
        }
        if let Some(project) = &self.project {
            project.merge_into(&mut set, RuleSource::ProjectSettings);
        }
        if let Some(local) = &self.local {
            local.merge_into(&mut set, RuleSource::LocalSettings);
        }
        set
    }

    /// Mode retenu, priorité local > project > user.
    #[must_use]
    pub fn effective_mode(&self) -> Option<PermissionMode> {
        self.local
            .as_ref()
            .and_then(|f| f.mode)
            .or_else(|| self.project.as_ref().and_then(|f| f.mode))
            .or_else(|| self.user.as_ref().and_then(|f| f.mode))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn loads_minimal_file() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"{{
              "permissions": {{
                "allow": ["Bash(npm:*)"],
                "deny":  ["Bash(rm -rf *)"]
              }},
              "mode": "acceptEdits"
            }}"#
        )
        .unwrap();
        let settings = SettingsFile::load(file.path()).unwrap();
        assert_eq!(settings.permissions.allow.len(), 1);
        assert_eq!(settings.permissions.deny.len(), 1);
        assert_eq!(settings.mode, Some(PermissionMode::AcceptEdits));
    }

    #[test]
    fn missing_file_returns_default() {
        let path = Path::new("/tmp/__drox_definitely_not_a_real_file.json");
        let settings = SettingsFile::load(path).unwrap();
        assert!(settings.permissions.allow.is_empty());
        assert!(settings.permissions.deny.is_empty());
        assert!(settings.mode.is_none());
    }

    #[test]
    fn merge_into_pushes_with_correct_source() {
        let settings = SettingsFile {
            permissions: PermissionsBlock {
                allow: vec!["Bash(echo *)".into()],
                ask: vec!["Bash(git push:*)".into()],
                deny: vec!["Bash(rm -rf *)".into()],
            },
            mode: None,
        };
        let mut set = RuleSet::new();
        settings.merge_into(&mut set, RuleSource::ProjectSettings);
        assert_eq!(set.len(), 3);
    }

    #[test]
    fn layered_mode_priority() {
        let user = Some(SettingsFile {
            permissions: PermissionsBlock::default(),
            mode: Some(PermissionMode::Default),
        });
        let project = Some(SettingsFile {
            permissions: PermissionsBlock::default(),
            mode: Some(PermissionMode::Plan),
        });
        let layered = LayeredConfig {
            user,
            project,
            local: None,
        };
        assert_eq!(layered.effective_mode(), Some(PermissionMode::Plan));
    }

    #[test]
    fn layered_local_overrides_project_rules() {
        let project = SettingsFile {
            permissions: PermissionsBlock {
                allow: vec!["bash(echo *)".into()],
                ask: vec![],
                deny: vec![],
            },
            mode: None,
        };
        let local = SettingsFile {
            permissions: PermissionsBlock {
                allow: vec![],
                ask: vec![],
                deny: vec!["bash".into()],
            },
            mode: Some(PermissionMode::Default),
        };
        let layered = LayeredConfig {
            user: None,
            project: Some(project),
            local: Some(local),
        };
        let set = layered.build_rule_set();
        assert!(set.iter_tool_wide(PermissionBehavior::Deny).any(|r| r.value.tool_name == "bash"));
        assert_eq!(layered.effective_mode(), Some(PermissionMode::Default));
    }
}
