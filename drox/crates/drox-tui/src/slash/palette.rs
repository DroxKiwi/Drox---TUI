//! Liste des commandes slash pour la palette (`/`).

/// Entrée palette : commande + description courte.
#[derive(Debug, Clone, Copy)]
pub struct SlashPaletteEntry {
    pub command: &'static str,
    pub description: &'static str,
}

pub const ENTRIES: &[SlashPaletteEntry] = &[
    SlashPaletteEntry { command: "/help", description: "aide — liste des commandes" },
    SlashPaletteEntry { command: "/server", description: "connexion IA Ollama (Ctrl+Shift+L)" },
    SlashPaletteEntry { command: "/workspace", description: "changer workspace (Ctrl+Shift+W)" },
    SlashPaletteEntry { command: "/clear", description: "effacer le fil UI" },
    SlashPaletteEntry { command: "/status", description: "workspace, modèle, session" },
    SlashPaletteEntry { command: "/context", description: "tokens et marge contexte" },
    SlashPaletteEntry { command: "/cost", description: "usage tokens session" },
    SlashPaletteEntry { command: "/compact", description: "compaction LLM transcript" },
    SlashPaletteEntry { command: "/memory", description: "sessions archivées · search" },
    SlashPaletteEntry { command: "/search", description: "recherche mémoire longue" },
    SlashPaletteEntry { command: "/permissions", description: "règles permission" },
    SlashPaletteEntry { command: "/plan", description: "mode plan" },
    SlashPaletteEntry { command: "/config", description: "réglages runtime" },
    SlashPaletteEntry { command: "/doctor", description: "diagnostic environnement" },
    SlashPaletteEntry { command: "/hooks", description: "hooks Pre/Post tool" },
    SlashPaletteEntry { command: "/mcp", description: "serveurs MCP" },
    SlashPaletteEntry { command: "/skills", description: "skills locaux" },
    SlashPaletteEntry { command: "/session", description: "transcript courant" },
    SlashPaletteEntry { command: "/rename", description: "titre personnalisé session" },
    SlashPaletteEntry { command: "/sessions", description: "lister sessions" },
    SlashPaletteEntry { command: "/resume", description: "reprendre ses_…" },
    SlashPaletteEntry { command: "/rewind", description: "rembobiner transcript" },
    SlashPaletteEntry { command: "/copy", description: "copier dernière réponse assistant" },
    SlashPaletteEntry { command: "/add-dir", description: "répertoire de travail additionnel" },
    SlashPaletteEntry { command: "/export", description: "exporter conversation" },
    SlashPaletteEntry { command: "/diff", description: "git diff workspace" },
    SlashPaletteEntry { command: "/files", description: "fichiers vus dans le fil" },
    SlashPaletteEntry { command: "/branch", description: "branche git" },
    SlashPaletteEntry { command: "/theme", description: "palette couleurs TUI" },
    SlashPaletteEntry { command: "/color", description: "accent session" },
    SlashPaletteEntry { command: "/vim", description: "mode vim composer (Esc NORMAL/INSERT)" },
    SlashPaletteEntry { command: "/keybindings", description: "raccourcis · init · reload" },
    SlashPaletteEntry { command: "/terminal-setup", description: "guide terminal + keybindings" },
    SlashPaletteEntry { command: "/settings", description: "préférences TUI" },
    SlashPaletteEntry { command: "/onboarding", description: "guide premier lancement" },
    SlashPaletteEntry { command: "/init", description: "scaffold workspace" },
    SlashPaletteEntry { command: "/sandbox", description: "état sandbox bash" },
    SlashPaletteEntry { command: "/review", description: "revue code (agent)" },
    SlashPaletteEntry { command: "/security-review", description: "revue sécurité (agent)" },
    SlashPaletteEntry { command: "/statusline", description: "barre de statut TUI" },
    SlashPaletteEntry { command: "/exit", description: "quitter" },
];

/// Filtre les entrées par préfixe ou sous-chaîne (insensible à la casse).
#[must_use]
pub fn filter_entries(filter: &str) -> Vec<usize> {
    let raw = filter.trim();
    let needle = raw.strip_prefix('/').unwrap_or(raw).to_ascii_lowercase();
    if needle.is_empty() {
        return (0..ENTRIES.len()).collect();
    }
    ENTRIES
        .iter()
        .enumerate()
        .filter_map(|(i, e)| {
            let cmd = e.command.trim_start_matches('/').to_ascii_lowercase();
            let desc = e.description.to_ascii_lowercase();
            if cmd.starts_with(&needle) || e.command.to_ascii_lowercase().contains(&needle) || desc.contains(&needle) {
                Some(i)
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_by_prefix() {
        let hits = filter_entries("mem");
        assert!(hits.iter().any(|&i| ENTRIES[i].command == "/memory"));
    }

    #[test]
    fn empty_filter_lists_all() {
        assert_eq!(filter_entries("").len(), ENTRIES.len());
    }
}
