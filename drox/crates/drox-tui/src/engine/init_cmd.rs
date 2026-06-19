//! `/init` — bootstrap workspace Drox.

use super::EngineRuntime;

/// Prompt agent pour `/init run` (version courte du leak `init.ts`).
pub const INIT_AGENT_PROMPT: &str = "Initialise ce dépôt pour Drox :\n\
1. Analyse le projet (manifest, README, structure).\n\
2. Crée ou améliore DROX.md — instructions concises pour les futures sessions.\n\
3. Si pertinent : `.drox/settings.json` minimal, skills ou hooks.\n\
4. Documente build / test / lint essentiels dans DROX.md.\n\
Reste concis — pas de généralités évidentes.";

impl EngineRuntime {
    /// État du scaffold `.drox/` + fichiers racine.
    #[must_use]
    pub fn format_init_status_lines(&self) -> Vec<String> {
        let ws = &self.workspace;
        let checks = [
            ("DROX.md", ws.join("DROX.md")),
            ("MEMORY.md", ws.join("MEMORY.md")),
            (".drox/", ws.join(".drox")),
            ("settings.json", ws.join(".drox/settings.json")),
            ("hooks.json", ws.join(".drox/hooks.json")),
            ("skills/", ws.join(".drox/skills")),
            (".mcp.json", ws.join(".mcp.json")),
        ];
        let mut lines = vec![
            "Init Drox — état workspace".into(),
            format!("  path : {ws}"),
        ];
        for (label, path) in checks {
            let status = if path.is_file() {
                "fichier"
            } else if path.is_dir() {
                "dossier"
            } else {
                "absent"
            };
            lines.push(format!("  {label:<16} {path} ({status})"));
        }
        lines.push("Lancer l'agent d'init : `/init run` (ou `/init` puis accepter).".into());
        lines
    }
}
