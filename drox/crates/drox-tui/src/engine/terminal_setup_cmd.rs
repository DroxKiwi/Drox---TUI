//! `/terminal-setup` — guide d'intégration terminal pour le TUI Drox.

use crate::engine::keybindings_cmd::keybindings_path;

const EXPANDABLE_TOOLS: &[&str] = &[];

/// Lignes affichées par `/terminal-setup`.
#[must_use]
pub fn format_terminal_setup_lines(init_keybindings: bool) -> Vec<String> {
    let term = detect_terminal_label();
    let kb_path = keybindings_path();
    let mut lines = vec![
        "── Configuration terminal Drox TUI ──".into(),
        format!("Terminal détecté : {term}"),
        String::new(),
        "Raccourcis intégrés (surchargeables via keybindings.json) :".into(),
        "  Entrée          envoyer".into(),
        "  !               mode bash (shell direct)".into(),
        "  Shift+Entrée    nouvelle ligne dans le composer".into(),
        "  Ctrl+F          recherche dans le fil".into(),
        "  Ctrl+R          recherche historique prompts".into(),
        "  e               développer / réduire la dernière sortie outil".into(),
        format!("                  ({})", EXPANDABLE_TOOLS.join(", ")),
        "  /               palette commandes slash".into(),
        "  Esc (run)       annuler le run agent".into(),
        "  Ctrl+Q          quitter".into(),
        String::new(),
        format!("Fichier keybindings : {kb_path}"),
        "  /keybindings init    — crée le template JSON".into(),
        "  /keybindings reload  — recharge à chaud".into(),
    ];

    if init_keybindings {
        lines.push(String::new());
        lines.push("✓ Template keybindings créé ou déjà présent.".into());
    }

    lines.extend(terminal_specific_hints(&term));
    lines
}

fn detect_terminal_label() -> String {
    if std::env::var("WT_SESSION").is_ok() {
        return "Windows Terminal".into();
    }
    if let Ok(v) = std::env::var("TERM_PROGRAM") {
        return v;
    }
    if let Ok(v) = std::env::var("TERM") {
        return format!("TERM={v}");
    }
    "terminal inconnu (crossterm)".into()
}

fn terminal_specific_hints(term: &str) -> Vec<String> {
    let lower = term.to_ascii_lowercase();
    let mut out = vec![String::new(), "Conseils :".into()];

    if lower.contains("windows terminal") || cfg!(windows) {
        out.push(
            "  Windows : Shift+Entrée est mappé via keybindings.json (défaut Drox).".into(),
        );
        out.push(
            "  Paramètres WT → Actions → ajouter Shift+Entrée si un autre programme capture la touche.".into(),
        );
    } else if lower.contains("iterm") || lower.contains("wezterm") || lower.contains("kitty") {
        out.push(
            "  Terminal moderne : les séquences Shift+Entrée sont en général transmises telles quelles.".into(),
        );
    } else {
        out.push(
            "  Si Shift+Entrée n'insère pas de saut de ligne, lancez /keybindings init puis ajustez multiline.".into(),
        );
    }

    out.push(
        "  Le TUI tourne en mode brut : copier/coller dépend du terminal hôte.".into(),
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_not_empty() {
        assert!(format_terminal_setup_lines(false).len() > 8);
    }
}
