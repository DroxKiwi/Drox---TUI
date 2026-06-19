//! Catalogue skills locaux (`/skills`).

use drox_tools::skills::{
    SKILLS_SUBDIR, find_skill, load_skills_catalog, model_visible_skills, read_skill_file,
};

use super::EngineRuntime;

const DISPLAY_MAX_CHARS: usize = 4_000;

impl EngineRuntime {
    /// `/skills` (liste) ou `/skills <name>` (lecture SKILL.md).
    pub async fn run_skills_command(&self, args: &str) -> Vec<String> {
        let name = args.trim();
        if name.is_empty() {
            self.format_skills_list().await
        } else {
            self.read_skill_lines(name).await
        }
    }

    async fn format_skills_list(&self) -> Vec<String> {
        let root = self.workspace.join(SKILLS_SUBDIR);
        match load_skills_catalog(&self.workspace).await {
            Ok(catalog) if catalog.is_empty() => vec![
                format!("Skills locaux ({root})"),
                "  (aucun — créer `.drox/skills/<name>/SKILL.md`)".into(),
            ],
            Ok(catalog) => {
                let visible = model_visible_skills(&catalog);
                let mut lines = vec![
                    format!("Skills locaux ({root})"),
                    format!(
                        "  {} skill(s) — {} visible(s) pour le modèle",
                        catalog.len(),
                        visible.len()
                    ),
                ];
                for entry in &catalog {
                    let mut desc = entry.description.clone();
                    if let Some(w) = &entry.when_to_use {
                        if !desc.is_empty() {
                            desc.push_str(" — ");
                        }
                        desc.push_str(w);
                    }
                    if desc.is_empty() {
                        desc = "(sans description)".into();
                    }
                    if desc.len() > 120 {
                        desc.truncate(119);
                        desc.push('…');
                    }
                    let tag = if entry.disable_model_invocation {
                        " [user-only]"
                    } else {
                        ""
                    };
                    lines.push(format!("  • {} — {desc}{tag}", entry.name));
                }
                lines.push("Astuce : /skills <name> pour lire SKILL.md complet.".into());
                lines
            }
            Err(e) => vec![format!("Skills : {e}")],
        }
    }

    async fn read_skill_lines(&self, name: &str) -> Vec<String> {
        let catalog = match load_skills_catalog(&self.workspace).await {
            Ok(c) => c,
            Err(e) => return vec![format!("Skills : {e}")],
        };
        let Some(entry) = find_skill(&catalog, name) else {
            return vec![format!("Skill inconnu : `{name}` (voir /skills)")];
        };
        let content = match read_skill_file(&entry.path).await {
            Ok(c) => c,
            Err(e) => return vec![format!("Lecture {} : {e}", entry.path)],
        };

        let mut lines = vec![
            format!("── skill [{}] ──", entry.name),
            format!("  path : {}", entry.path),
        ];
        if entry.disable_model_invocation {
            lines.push("  note : disable-model-invocation (masqué au modèle, lisible ici)".into());
        }
        if !entry.description.is_empty() {
            lines.push(format!("  description : {}", entry.description));
        }
        let body = if content.len() > DISPLAY_MAX_CHARS {
            format!(
                "{}…\n\n[tronqué — {DISPLAY_MAX_CHARS} car. affichés]",
                &content[..DISPLAY_MAX_CHARS]
            )
        } else {
            content
        };
        for line in body.lines() {
            lines.push(line.to_string());
        }
        lines
    }
}
