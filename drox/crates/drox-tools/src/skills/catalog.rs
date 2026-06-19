//! Découverte et formatage des skills locaux (`.drox/skills/<name>/SKILL.md`).
//!
//! Sprint §2.31 — équivalent compact du listing leak `SkillTool/prompt.ts` :
//! front-matter seulement au prompt, contenu complet via `skill_read`.

use std::fmt::Write as _;

use camino::{Utf8Path, Utf8PathBuf};
use thiserror::Error;

/// Sous-dossier des skills dans le workspace (leak : `projectSettings` → `.drox/skills`).
pub const SKILLS_SUBDIR: &str = ".drox/skills";

/// Budget caractères pour le listing injecté (leak : 1 % contexte ≈ 8k chars).
pub const DEFAULT_LISTING_CHAR_BUDGET: usize = 8_000;

/// Troncature par entrée (leak `MAX_LISTING_DESC_CHARS`).
pub const MAX_LISTING_DESC_CHARS: usize = 250;

/// Taille max renvoyée par `skill_read` (aligné `memory_read`).
pub const MAX_SKILL_BYTES_RETURNED: usize = 64 * 1024;

/// Entrée catalogue — métadonnées + chemin canonique.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillEntry {
    /// Nom d'invocation (dossier ou champ `name` du front-matter).
    pub name: String,
    pub description: String,
    pub when_to_use: Option<String>,
    /// Chemin absolu UTF-8 vers `SKILL.md`.
    pub path: Utf8PathBuf,
    /// Si `true`, exclu du listing LLM (leak `disable-model-invocation`).
    pub disable_model_invocation: bool,
}

#[derive(Debug, Error)]
pub enum SkillError {
    #[error("skills: io on {path}: {source}")]
    Io {
        path: Utf8PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("skills: {0}")]
    Other(String),
}

/// Charge le catalogue des skills du workspace (tri alphabétique par nom).
pub async fn load_skills_catalog(workspace: &Utf8Path) -> Result<Vec<SkillEntry>, SkillError> {
    let skills_root = workspace.join(SKILLS_SUBDIR);
    let mut entries = Vec::new();
    let Ok(mut dir) = tokio::fs::read_dir(skills_root.as_std_path()).await else {
        return Ok(entries);
    };
    while let Ok(Some(entry)) = dir.next_entry().await {
        let path = entry.path();
        let Ok(path) = Utf8PathBuf::from_path_buf(path) else {
            continue;
        };
        if !entry
            .file_type()
            .await
            .map(|t| t.is_dir())
            .unwrap_or(false)
        {
            continue;
        }
        let skill_md = path.join("SKILL.md");
        if !skill_md.is_file() {
            continue;
        }
        let dir_name = path
            .file_name()
            .map(str::to_string)
            .unwrap_or_default();
        if dir_name.is_empty() {
            continue;
        }
        let raw = tokio::fs::read_to_string(skill_md.as_std_path())
            .await
            .map_err(|e| SkillError::Io {
                path: skill_md.clone(),
                source: e,
            })?;
        let parsed = parse_skill_markdown(&raw, &dir_name);
        entries.push(SkillEntry {
            name: parsed.name,
            description: parsed.description,
            when_to_use: parsed.when_to_use,
            path: skill_md,
            disable_model_invocation: parsed.disable_model_invocation,
        });
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}

/// Entrées visibles du modèle (sans `disable-model-invocation`).
#[must_use]
pub fn model_visible_skills(entries: &[SkillEntry]) -> Vec<&SkillEntry> {
    entries
        .iter()
        .filter(|e| !e.disable_model_invocation)
        .collect()
}

/// Lit le fichier `SKILL.md` complet.
pub async fn read_skill_file(path: &Utf8Path) -> Result<String, SkillError> {
    tokio::fs::read_to_string(path.as_std_path())
        .await
        .map_err(|e| SkillError::Io {
            path: path.to_path_buf(),
            source: e,
        })
}

/// Trouve une entrée par nom (insensible à la casse).
#[must_use]
pub fn find_skill<'a>(entries: &'a [SkillEntry], name: &str) -> Option<&'a SkillEntry> {
    let key = name.trim();
    entries
        .iter()
        .find(|e| e.name.eq_ignore_ascii_case(key))
}

/// Bloc texte injecté au system prompt (listing compact).
#[must_use]
pub fn format_skills_listing_for_prompt(entries: &[SkillEntry]) -> Option<String> {
    let visible = model_visible_skills(entries);
    if visible.is_empty() {
        return None;
    }
    let mut out = String::new();
    out.push_str(
        "[Skills — prompts réutilisables locaux (`.drox/skills/<name>/SKILL.md`)]\n",
    );
    let budget = DEFAULT_LISTING_CHAR_BUDGET;
    let mut lines_added = 0usize;
    let total_visible = visible.len();
    for entry in &visible {
        let line = format_listing_line(entry);
        let line_len = line.len() + 1;
        if used_budget(&out) + line_len > budget && lines_added > 0 {
            let omitted = total_visible - lines_added;
            let _ = writeln!(
                out,
                "(… {omitted} skill(s) omitted — use `skill_list` or `skill_read`)"
            );
            break;
        }
        let _ = writeln!(out, "{line}");
        lines_added += 1;
    }
    out.push_str(
        "Use `skill_read { \"name\": \"…\" }` to load full instructions before following a skill.\n",
    );
    Some(out)
}

fn used_budget(s: &str) -> usize {
    s.len()
}

fn format_listing_line(entry: &SkillEntry) -> String {
    let mut desc = entry.description.clone();
    if let Some(w) = &entry.when_to_use {
        if !w.is_empty() {
            if !desc.is_empty() {
                desc.push_str(" — ");
            }
            desc.push_str(w);
        }
    }
    if desc.is_empty() {
        desc = "(no description)".to_string();
    }
    if desc.len() > MAX_LISTING_DESC_CHARS {
        desc.truncate(MAX_LISTING_DESC_CHARS.saturating_sub(1));
        desc.push('…');
    }
    format!("- {}: {desc}", entry.name)
}

struct ParsedSkillMeta {
    name: String,
    description: String,
    when_to_use: Option<String>,
    disable_model_invocation: bool,
}

fn parse_skill_markdown(raw: &str, dir_name: &str) -> ParsedSkillMeta {
    let (fm, body) = split_frontmatter(raw);
    let mut name = dir_name.to_string();
    let mut description = String::new();
    let mut when_to_use = None;
    let mut disable_model_invocation = false;

    for line in fm.lines() {
            let trimmed = line.trim_end();
            if let Some((k, v)) = trimmed.split_once(':') {
                let key = k.trim();
                let val = v.trim().trim_matches('"');
                match key {
                    "name" if !val.is_empty() => name = val.to_string(),
                    "description" if !val.is_empty() => description = val.to_string(),
                    "when_to_use" | "whenToUse" if !val.is_empty() => {
                        when_to_use = Some(val.to_string());
                    }
                    "disable-model-invocation" | "disable_model_invocation" => {
                        disable_model_invocation =
                            matches!(val.to_ascii_lowercase().as_str(), "true" | "yes" | "1");
                    }
                    _ => {}
                }
            }
    }
    if description.is_empty() {
        description = first_nonempty_body_line(&body);
    }
    ParsedSkillMeta {
        name,
        description,
        when_to_use,
        disable_model_invocation,
    }
}

fn split_frontmatter(raw: &str) -> (String, String) {
    let mut lines = raw.lines();
    if lines.next().map(str::trim) != Some("---") {
        return (String::new(), raw.to_string());
    }
    let mut fm_lines = Vec::new();
    let mut rest = Vec::new();
    let mut in_fm = true;
    for line in lines {
        if in_fm && line.trim() == "---" {
            in_fm = false;
            continue;
        }
        if in_fm {
            fm_lines.push(line);
        } else {
            rest.push(line);
        }
    }
    (fm_lines.join("\n"), rest.join("\n"))
}

fn first_nonempty_body_line(body: &str) -> String {
    body.lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with('#'))
        .unwrap_or_default()
        .chars()
        .take(200)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    async fn write_skill(ws: &Utf8Path, dir: &str, content: &str) {
        let dir_path = ws.join(SKILLS_SUBDIR).join(dir);
        tokio::fs::create_dir_all(dir_path.as_std_path())
            .await
            .unwrap();
        tokio::fs::write(dir_path.join("SKILL.md").as_std_path(), content)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn discovers_and_sorts_skills() {
        let dir = tempdir().unwrap();
        let ws = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        write_skill(
            &ws,
            "beta",
            "---\nname: beta\ndescription: B skill\n---\n\nDo B.",
        )
        .await;
        write_skill(
            &ws,
            "alpha",
            "---\ndescription: A skill\nwhen_to_use: when A\n---\n",
        )
        .await;

        let entries = load_skills_catalog(&ws).await.unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "alpha");
        assert_eq!(entries[1].name, "beta");
        assert_eq!(entries[0].when_to_use.as_deref(), Some("when A"));
    }

    #[tokio::test]
    async fn excludes_disable_model_invocation_from_listing() {
        let dir = tempdir().unwrap();
        let ws = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        write_skill(
            &ws,
            "deploy",
            "---\ndescription: deploy\ndisable-model-invocation: true\n---\n",
        )
        .await;
        write_skill(&ws, "lint", "---\ndescription: lint\n---\n").await;

        let entries = load_skills_catalog(&ws).await.unwrap();
        let block = format_skills_listing_for_prompt(&entries).unwrap();
        assert!(block.contains("lint"));
        assert!(!block.contains("deploy"));
    }

    #[tokio::test]
    async fn find_skill_is_case_insensitive() {
        let dir = tempdir().unwrap();
        let ws = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        write_skill(&ws, "Foo", "---\ndescription: x\n---\n").await;
        let entries = load_skills_catalog(&ws).await.unwrap();
        assert!(find_skill(&entries, "foo").is_some());
    }
}
