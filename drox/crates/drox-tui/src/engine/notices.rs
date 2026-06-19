//! Notices de statut au démarrage (§11.4) + tips (§11.5).

use camino::Utf8Path;

use crate::app::AppConfig;
use crate::engine::bootstrap::EngineRuntime;

/// Niveau d'une notice affichée sous le header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoticeLevel {
    Info,
    Warn,
    Tip,
}

/// Notice compacte (une ligne).
#[derive(Debug, Clone)]
pub struct StatusNotice {
    pub level: NoticeLevel,
    pub text: String,
}

const TIPS: &[&str] = &[
    "Astuce : `/` ouvre la palette des commandes slash.",
    "Astuce : `Ctrl+F` recherche dans le fil, `Ctrl+R` dans l'historique.",
    "Astuce : `!` passe en mode bash (shell direct sans agent).",
    "Astuce : `e` ouvre le viewer scrollable du dernier outil expansible (bash, grep, diff, LSP, MCP…).",
    "Astuce : `/context` affiche la marge tokens restante.",
    "Astuce : `/rewind` rembobine le transcript à un message passé.",
    "Astuce : `/theme` et `/color` personnalisent l'apparence.",
    "Astuce : `Esc` annule un run agent en cours.",
];

impl EngineRuntime {
    /// Notices contextuelles affichées au boot (et jusqu'à `/clear`).
    #[must_use]
    pub fn collect_startup_notices(&self, config: &AppConfig) -> Vec<StatusNotice> {
        let mut out = Vec::new();

        if !config.apply {
            out.push(StatusNotice {
                level: NoticeLevel::Warn,
                text: "Mode lecture seule — relancez avec --apply pour autoriser les écritures fichier.".into(),
            });
        }

        if config.plan_mode || self.plan_mode() {
            out.push(StatusNotice {
                level: NoticeLevel::Info,
                text: "Mode plan actif — le moteur ne modifiera pas les fichiers sans validation (/plan off).".into(),
            });
        }

        if !workspace_has_init_marker(&self.workspace) {
            out.push(StatusNotice {
                level: NoticeLevel::Tip,
                text: "Workspace non initialisé — essayez `/init` puis `/init run`.".into(),
            });
        }

        if !self.workspace.join(".drox").join("hooks.json").exists() {
            out.push(StatusNotice {
                level: NoticeLevel::Tip,
                text: "Aucun hook Pre/Post — voir `/hooks` et `.drox/hooks.json`.".into(),
            });
        }

        let tip_idx = tip_index_for_session(&self.session_id());
        out.push(StatusNotice {
            level: NoticeLevel::Tip,
            text: TIPS[tip_idx % TIPS.len()].to_string(),
        });

        out
    }
}

fn workspace_has_init_marker(workspace: &Utf8Path) -> bool {
    workspace.join(".drox").exists()
}

fn tip_index_for_session(session_id: &str) -> usize {
    session_id.bytes().fold(0usize, |acc, b| acc.wrapping_add(usize::from(b)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tip_index_deterministic() {
        assert_eq!(
            tip_index_for_session("ses_abc"),
            tip_index_for_session("ses_abc")
        );
    }
}
