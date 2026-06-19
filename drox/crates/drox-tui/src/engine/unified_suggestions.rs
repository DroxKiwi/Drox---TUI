//! Suggestions unifiées du composer — fichiers `@`, commandes `/`, skills.
//!
//! Inspiré du leak `unifiedSuggestions.ts` : une seule popup, priorité au token actif.

use crate::engine::at_typeahead::{self, AtFileIndex, AtQuery, MAX_SUGGESTIONS as MAX_UNIFIED};
use crate::slash::palette::{filter_entries, SlashPaletteEntry, ENTRIES as SLASH_ENTRIES};

/// Type de suggestion (affichage + application Tab).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuggestionKind {
    File,
    Slash,
    Skill,
}

/// Une ligne de la popup composer.
#[derive(Debug, Clone)]
pub struct ComposerSuggestionItem {
    pub kind: SuggestionKind,
    /// Texte principal (ex. `@src/foo.rs`, `/memory`).
    pub label: String,
    /// Sous-titre optionnel (description slash / skill).
    pub detail: Option<String>,
    /// Valeur utile à l'application (chemin relatif, commande slash, nom skill).
    pub payload: String,
}

/// État de la popup suggestions (remplace l'ancien typeahead `@` seul).
#[derive(Debug, Clone)]
pub struct ComposerSuggestionDialog {
    pub items: Vec<ComposerSuggestionItem>,
    pub cursor: usize,
    /// Présent uniquement pour les suggestions fichier — contexte `apply_completion`.
    pub file_ctx: Option<AtQuery>,
    /// Titre court pour la barre de la popup.
    pub title: String,
}

/// Nom + description courte d'un skill (cache session).
#[derive(Debug, Clone)]
pub struct SkillSuggestionEntry {
    pub name: String,
    pub description: String,
}

/// Détecte un préfixe slash en cours (`/mem` → `mem`).
#[must_use]
pub fn active_slash_prefix(buffer: &str) -> Option<String> {
    if !buffer.starts_with('/') {
        return None;
    }
    // Uniquement le premier token : `/memory` ok, `/memory foo` → pas de popup slash.
    let rest = buffer.strip_prefix('/')?;
    if rest.contains(' ') {
        return None;
    }
    Some(rest.to_string())
}

/// Détecte la saisie après `/skills` pour compléter un nom de skill.
#[must_use]
pub fn active_skill_prefix(buffer: &str) -> Option<String> {
    if !buffer.starts_with("/skills") {
        return None;
    }
    let rest = buffer.strip_prefix("/skills")?;
    if !rest.is_empty() && !rest.starts_with(' ') {
        return None;
    }
    let name = rest.trim_start();
    if name.contains(' ') {
        return None;
    }
    Some(name.to_string())
}

/// Construit la liste unifiée selon le buffer courant (priorité : `@` > `/skills` > `/`).
#[must_use]
pub fn build_suggestions(
    buffer: &str,
    file_index: Option<&AtFileIndex>,
    skills: &[SkillSuggestionEntry],
) -> Option<ComposerSuggestionDialog> {
    // 1) Token `@fichier` en fin de buffer.
    if let Some(file_query) = at_typeahead::active_at_query(buffer) {
        let paths = file_index
            .map(|idx| idx.query(&file_query.prefix, MAX_UNIFIED))
            .unwrap_or_default();
        let items: Vec<ComposerSuggestionItem> = paths
            .into_iter()
            .map(|p| ComposerSuggestionItem {
                kind: SuggestionKind::File,
                label: format!("@{p}"),
                detail: None,
                payload: p,
            })
            .collect();
        let title = if file_query.prefix.is_empty() {
            format!(" @fichier — {} ", items.len())
        } else {
            format!(" @{} — {} ", file_query.prefix, items.len())
        };
        return Some(ComposerSuggestionDialog {
            items,
            cursor: 0,
            file_ctx: Some(file_query),
            title,
        });
    }

    // 2) Complétion skill : `/skills rust`
    if let Some(skill_prefix) = active_skill_prefix(buffer) {
        let items = filter_skills(skills, &skill_prefix);
        if items.is_empty() && !skill_prefix.is_empty() {
            return None;
        }
        return Some(ComposerSuggestionDialog {
            title: format!(" /skills — {} ", items.len()),
            cursor: 0,
            items,
            file_ctx: None,
        });
    }

    // 3) Complétion commande slash : `/mem`
    if let Some(slash_prefix) = active_slash_prefix(buffer) {
        let items = filter_slash_commands(&slash_prefix);
        if items.is_empty() && !slash_prefix.is_empty() {
            return None;
        }
        return Some(ComposerSuggestionDialog {
            title: format!(" /{} — {} ", slash_prefix, items.len()),
            cursor: 0,
            items,
            file_ctx: None,
        });
    }

    None
}

fn filter_slash_commands(prefix: &str) -> Vec<ComposerSuggestionItem> {
    let indices = filter_entries(prefix);
    indices
        .into_iter()
        .take(MAX_UNIFIED)
        .filter_map(|i| {
            let e: &SlashPaletteEntry = SLASH_ENTRIES.get(i)?;
            Some(ComposerSuggestionItem {
                kind: SuggestionKind::Slash,
                label: e.command.to_string(),
                detail: Some(e.description.to_string()),
                payload: e.command.to_string(),
            })
        })
        .collect()
}

fn filter_skills(skills: &[SkillSuggestionEntry], prefix: &str) -> Vec<ComposerSuggestionItem> {
    let needle = prefix.to_ascii_lowercase();
    let mut out: Vec<ComposerSuggestionItem> = skills
        .iter()
        .filter(|s| {
            needle.is_empty()
                || s.name.to_ascii_lowercase().starts_with(&needle)
                || s.description.to_ascii_lowercase().contains(&needle)
        })
        .take(MAX_UNIFIED)
        .map(|s| ComposerSuggestionItem {
            kind: SuggestionKind::Skill,
            label: s.name.clone(),
            detail: if s.description.is_empty() {
                None
            } else {
                Some(s.description.clone())
            },
            payload: s.name.clone(),
        })
        .collect();
    if prefix.is_empty() {
        out.truncate(MAX_UNIFIED);
    }
    out
}

/// Préserve le curseur si la liste se prolonge (même contexte fichier ou préfixe slash).
#[must_use]
pub fn preserve_cursor(
    prev: Option<&ComposerSuggestionDialog>,
    next: &ComposerSuggestionDialog,
) -> usize {
    let Some(prev) = prev else {
        return 0;
    };
    if prev.items.is_empty() {
        return 0;
    }
    if let (Some(old), Some(new)) = (&prev.file_ctx, &next.file_ctx) {
        if old.at_start == new.at_start && new.prefix.starts_with(&old.prefix) {
            return prev.cursor.min(next.items.len().saturating_sub(1));
        }
    }
    if prev.file_ctx.is_none() && next.file_ctx.is_none() && !prev.items.is_empty() {
        let old_label = prev.items.get(prev.cursor).map(|i| i.label.as_str());
        if let Some(lbl) = old_label {
            if let Some(idx) = next.items.iter().position(|i| i.label == lbl) {
                return idx;
            }
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_slash_prefix() {
        assert_eq!(active_slash_prefix("/mem").as_deref(), Some("mem"));
        assert!(active_slash_prefix("/memory ok").is_none());
    }

    #[test]
    fn detects_skill_prefix() {
        assert_eq!(active_skill_prefix("/skills ru").as_deref(), Some("ru"));
        assert_eq!(active_skill_prefix("/skills ").as_deref(), Some(""));
        assert_eq!(active_skill_prefix("/skills").as_deref(), Some(""));
    }

    #[test]
    fn builds_slash_suggestions() {
        let dialog = build_suggestions("/mem", None, &[]).expect("slash");
        assert!(dialog.items.iter().any(|i| i.label == "/memory"));
    }
}
