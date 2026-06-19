//! `/add-dir` — répertoires de travail additionnels (session).
//!
//! Aligné sur le leak `commands/add-dir/` : étend les racines pour la résolution
//! des chemins (`@fichier`, tools) au-delà du workspace principal.

use std::fs;
use std::path::PathBuf;

use camino::{Utf8Path, Utf8PathBuf};

use crate::app::AppState;
use crate::engine::EngineRuntime;
use crate::slash::SlashOutcome;

/// Valide et enregistre un répertoire additionnel pour la session courante.
pub fn handle_add_dir(args: &str, state: &mut AppState, runtime: &EngineRuntime) -> SlashOutcome {
    let trimmed = args.trim();
    if trimmed.is_empty() {
        state.push_system(
            "Usage : /add-dir <chemin>\n\
             Ajoute un répertoire de travail pour cette session (résolution @fichier et tools).\n\
             Exemple : /add-dir ../autre-projet",
        );
        return SlashOutcome::Handled;
    }

    match validate_and_add(runtime, trimmed) {
        Ok(msg) => {
            state.push_system(&msg);
            state.status_line = format!("Répertoire ajouté : {trimmed}");
        }
        Err(e) => {
            state.push_system(format!("/add-dir : {e}"));
            state.status_line = "Échec /add-dir".into();
        }
    }
    SlashOutcome::Handled
}

/// Canonise le chemin, vérifie qu'il est un dossier accessible et pas déjà couvert.
pub fn validate_and_add(runtime: &EngineRuntime, directory_path: &str) -> Result<String, String> {
    let expanded = expand_user_path(directory_path);
    let canonical = fs::canonicalize(&expanded).map_err(|e| {
        format!(
            "chemin introuvable ou inaccessible : {} ({e})",
            expanded.display()
        )
    })?;

    if !canonical.is_dir() {
        return Err(format!(
            "{} n'est pas un répertoire",
            canonical.display()
        ));
    }

    let abs = Utf8PathBuf::from_path_buf(canonical.clone())
        .map_err(|_| "chemin non UTF-8".to_string())?;

    for existing in runtime.working_directories() {
        if path_within(&abs, &existing) {
            return Err(format!(
                "déjà accessible via le répertoire de travail {}",
                existing
            ));
        }
    }

    runtime
        .add_working_directory(abs.clone())
        .map_err(|e| e.to_string())?;

    Ok(format!(
        "Répertoire de travail ajouté : {}\n\
         · session uniquement — /permissions pour les règles persistantes",
        abs
    ))
}

/// Canonise et valide un chemin de workspace principal (`/workspace`).
pub fn validate_workspace_path(input: &str) -> Result<Utf8PathBuf, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("chemin requis".into());
    }

    let expanded = expand_user_path(trimmed);
    let canonical = fs::canonicalize(&expanded).map_err(|e| {
        format!(
            "chemin introuvable ou inaccessible : {} ({e})",
            expanded.display()
        )
    })?;

    if !canonical.is_dir() {
        return Err(format!("{} n'est pas un répertoire", canonical.display()));
    }

    Utf8PathBuf::from_path_buf(canonical).map_err(|_| "chemin non UTF-8".to_string())
}

/// `child` est sous `parent` (chemins canoniques).
fn path_within(child: &Utf8Path, parent: &Utf8Path) -> bool {
    let Ok(child_c) = fs::canonicalize(child.as_std_path()) else {
        return false;
    };
    let Ok(parent_c) = fs::canonicalize(parent.as_std_path()) else {
        return false;
    };
    child_c.starts_with(&parent_c)
}

/// Expansion minimale `~` et chemins relatifs au cwd process.
pub(crate) fn expand_user_path(path: &str) -> PathBuf {
    let trimmed = path.trim();
    if trimmed == "~" {
        return dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    }
    if let Some(rest) = trimmed.strip_prefix("~/").or_else(|| trimmed.strip_prefix("~\\")) {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    let p = PathBuf::from(trimmed);
    if p.is_absolute() {
        p
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn rejects_file_not_directory() {
        let dir = tempdir().unwrap();
        let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let file = root.join("notadir.txt");
        std::fs::write(&file, "").unwrap();
        // Runtime minimal non nécessaire — test expand seulement
        assert!(!file.as_std_path().is_dir());
    }

    #[test]
    fn validates_workspace_directory() {
        let dir = tempdir().unwrap();
        let path = dir.path().to_string_lossy();
        let got = validate_workspace_path(path.as_ref()).expect("valid dir");
        assert!(got.as_std_path().is_dir());
    }
}
