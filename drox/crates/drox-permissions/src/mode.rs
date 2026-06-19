//! Modes de permission.
//!
//! Pendant un sprint, l'agent fonctionne dans **un seul** mode à la fois.
//! Le mode influence la décision finale du `PermissionEngine` :
//!
//! - `Default` : pipeline complet (deny → ask → allow), demande à l'humain
//!   si aucune règle ne décide.
//! - `Plan` : interdit toute écriture (l'agent ne peut que lire / réfléchir).
//!   En pratique, les tools d'écriture renvoient `Deny` même si une règle
//!   allow correspond ; lecture et grep restent `Allow`.
//! - `AcceptEdits` : auto-allow les écritures fichier dans le workspace, mais
//!   continue à demander pour le reste (Bash arbitraire, etc.).
//! - `BypassPermissions` : équivalent du "yolo" — auto-allow tout sauf
//!   les rules `Deny` explicites. **Risqué**, à réserver aux sandboxes.

use serde::{Deserialize, Serialize};

/// Modes de permission supportés (sous-ensemble du système TS d'origine).
///
/// Note : `dontAsk`, `auto`, `bubble` ne sont pas portés pour ce sprint
/// (ils nécessitent classifier YOLO + hooks, prévus plus tard).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum PermissionMode {
    /// Mode par défaut : pipeline complet allow → ask → deny.
    #[default]
    Default,
    /// Mode plan : interdit toute écriture / commande qui change le système.
    Plan,
    /// Auto-allow les écritures fichier dans le workspace.
    AcceptEdits,
    /// Bypass complet (sauf rules deny explicites).
    BypassPermissions,
    /// Mode professeur : tuteur — écritures autorisées uniquement via les
    /// gates moteur (`workArea` + étape `exercise`/`checkpoint` active).
    Professor,
}

impl PermissionMode {
    /// Parse le mode depuis une chaîne libre (case-insensitive).
    ///
    /// Renvoie `Default` si la chaîne ne correspond à rien.
    #[must_use]
    pub fn from_str_lossy(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "plan" => Self::Plan,
            "acceptedits" | "accept-edits" | "accept_edits" => Self::AcceptEdits,
            "bypasspermissions" | "bypass-permissions" | "bypass_permissions" | "yolo" => {
                Self::BypassPermissions
            }
            "professor" | "professeur" | "teacher" => Self::Professor,
            _ => Self::Default,
        }
    }

    /// Identifiant court adapté à l'affichage CLI.
    #[must_use]
    pub const fn short_title(self) -> &'static str {
        match self {
            Self::Default => "Default",
            Self::Plan => "Plan",
            Self::AcceptEdits => "Accept",
            Self::BypassPermissions => "Bypass",
            Self::Professor => "Professeur",
        }
    }

    /// `true` si le mode autorise l'auto-allow sur les écritures fichier.
    #[must_use]
    pub const fn auto_allows_writes(self) -> bool {
        matches!(
            self,
            Self::AcceptEdits | Self::BypassPermissions | Self::Professor
        )
    }

    /// `true` si le mode interdit toute écriture (mode plan).
    #[must_use]
    pub const fn blocks_writes(self) -> bool {
        matches!(self, Self::Plan)
    }

    /// Mode pédagogique (plan de cours, pas de todo technique).
    #[must_use]
    pub const fn is_professor(self) -> bool {
        matches!(self, Self::Professor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_camelcase_and_aliases() {
        assert_eq!(PermissionMode::from_str_lossy("plan"), PermissionMode::Plan);
        assert_eq!(
            PermissionMode::from_str_lossy("acceptEdits"),
            PermissionMode::AcceptEdits
        );
        assert_eq!(
            PermissionMode::from_str_lossy("ACCEPT-EDITS"),
            PermissionMode::AcceptEdits
        );
        assert_eq!(
            PermissionMode::from_str_lossy("yolo"),
            PermissionMode::BypassPermissions
        );
        assert_eq!(
            PermissionMode::from_str_lossy("bypassPermissions"),
            PermissionMode::BypassPermissions
        );
        assert_eq!(
            PermissionMode::from_str_lossy("professor"),
            PermissionMode::Professor
        );
        assert_eq!(
            PermissionMode::from_str_lossy("unknown-mode"),
            PermissionMode::Default
        );
    }

    #[test]
    fn behavior_helpers_are_consistent() {
        assert!(PermissionMode::Plan.blocks_writes());
        assert!(!PermissionMode::Professor.blocks_writes());
        assert!(PermissionMode::Professor.auto_allows_writes());
        assert!(PermissionMode::Professor.is_professor());
        assert!(!PermissionMode::Plan.auto_allows_writes());
        assert!(PermissionMode::AcceptEdits.auto_allows_writes());
        assert!(PermissionMode::BypassPermissions.auto_allows_writes());
        assert!(!PermissionMode::Default.auto_allows_writes());
        assert!(!PermissionMode::Default.blocks_writes());
    }

    #[test]
    fn round_trip_json() {
        let m = PermissionMode::AcceptEdits;
        let s = serde_json::to_string(&m).unwrap();
        assert_eq!(s, "\"acceptEdits\"");
        let parsed: PermissionMode = serde_json::from_str(&s).unwrap();
        assert_eq!(parsed, m);
    }
}
