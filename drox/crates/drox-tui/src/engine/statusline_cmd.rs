//! `/statusline` — introspection barre de statut TUI.

use super::status_bar::{format_elapsed, StatusBarSnapshot};
use super::EngineRuntime;

/// Affiche le contenu actuel de la status line (sans lancer l'agent).
#[must_use]
pub fn format_statusline_lines(
    runtime: &EngineRuntime,
    snapshot: &StatusBarSnapshot,
) -> Vec<String> {
    let mut lines = vec![
        "Barre de statut Drox TUI (bas d'écran)".into(),
        format!("  modèle      : {}", snapshot.model),
        format!("  workspace   : {}", snapshot.workspace_short),
    ];
    if let Some(ref branch) = snapshot.branch {
        lines.push(format!("  git branch  : {branch}"));
    }
    if let Some(pct) = snapshot.ctx_pct {
        lines.push(format!(
            "  contexte    : ↑{} ↓{} · ctx {} — {pct}% fenêtre",
            snapshot.stats.total_in, snapshot.stats.total_out, snapshot.stats.ctx
        ));
    } else {
        lines.push(format!(
            "  tokens run  : ↑{} ↓{} (ctx {})",
            snapshot.stats.total_in, snapshot.stats.total_out, snapshot.stats.ctx
        ));
    }
    lines.push(format!(
        "  durée       : {}",
        format_elapsed(snapshot.session_elapsed)
    ));
    lines.push(format!("  session     : {}", runtime.session_id()));
    lines.push("Personnaliser : `/theme` · `/color` · `/keybindings init`".into());
    lines.push("Assistant setup : `/statusline run`".into());
    lines
}
