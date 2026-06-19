//! Résolution de chemins sous la racine workspace.

use std::fs;
use std::path::Path;

use camino::{Utf8Path, Utf8PathBuf};

use crate::error::ToolError;

fn utf8_path_buf_from_std(path: std::path::PathBuf) -> Result<Utf8PathBuf, ToolError> {
    Utf8PathBuf::from_path_buf(path).map_err(|_| ToolError::invalid_args("path is not valid UTF-8"))
}

/// Résout `user_path` en chemin absolu UTF-8 **contenu** dans `workspace_root`.
pub fn resolve_under_workspace(
    workspace_root: &Utf8Path,
    user_path: &str,
) -> Result<Utf8PathBuf, ToolError> {
    let trimmed = user_path.trim();
    if trimmed.is_empty() {
        return Err(ToolError::invalid_args("path must not be empty"));
    }

    let joined = if Path::new(trimmed).is_absolute() {
        Utf8PathBuf::from(trimmed)
    } else {
        workspace_root.join(trimmed)
    };

    let root_canon = fs::canonicalize(workspace_root.as_std_path())
        .map_err(|e| ToolError::io(workspace_root.to_owned(), e))?;
    let target_canon =
        fs::canonicalize(joined.as_std_path()).map_err(|e| ToolError::io(joined.clone(), e))?;

    target_canon
        .strip_prefix(&root_canon)
        .map_err(|_| ToolError::PathEscape {
            path: utf8_path_buf_from_std(target_canon.clone()).unwrap_or_else(|_| joined.clone()),
        })?;

    utf8_path_buf_from_std(target_canon)
}

/// Résout un chemin cible pour **écriture** : le fichier peut ne pas exister,
/// mais le répertoire parent doit exister et être sous `workspace_root`.
pub fn resolve_path_for_write(
    workspace_root: &Utf8Path,
    user_path: &str,
) -> Result<Utf8PathBuf, ToolError> {
    let trimmed = user_path.trim();
    if trimmed.is_empty() {
        return Err(ToolError::invalid_args("path must not be empty"));
    }

    let joined: Utf8PathBuf = if Path::new(trimmed).is_absolute() {
        Utf8PathBuf::from(trimmed)
    } else {
        workspace_root.join(trimmed)
    };

    let root_canon = fs::canonicalize(workspace_root.as_std_path())
        .map_err(|e| ToolError::io(workspace_root.to_owned(), e))?;

    let parent_utf8 = joined
        .parent()
        .filter(|p| !p.as_str().is_empty())
        .unwrap_or(workspace_root);

    let parent_canon = fs::canonicalize(parent_utf8.as_std_path())
        .map_err(|e| ToolError::io(parent_utf8.to_owned(), e))?;

    parent_canon
        .strip_prefix(&root_canon)
        .map_err(|_| ToolError::PathEscape {
            path: utf8_path_buf_from_std(parent_canon.clone())
                .unwrap_or_else(|_| parent_utf8.to_owned()),
        })?;

    let file_name = joined
        .file_name()
        .ok_or_else(|| ToolError::invalid_args("path must include a file name"))?;

    let full_std = parent_canon.join(file_name);
    utf8_path_buf_from_std(full_std)
}

/// Chemins interdits pour une suppression (aligné leak `isDangerousRemovalPath`).
///
/// Bloque wildcards, racines système et enfants directs de `/` ou `C:\`.
#[must_use]
pub fn dangerous_removal_user_path_reason(user_path: &str) -> Option<&'static str> {
    let trimmed = user_path.trim().trim_matches(|c| c == '"' || c == '\'');
    if trimmed.is_empty() {
        return Some("path must not be empty");
    }
    let forward = trimmed.replace('\\', "/");
    if forward == "*" || forward.ends_with("/*") {
        return Some("wildcards are not allowed for delete_path");
    }
    if forward == "/" || forward == "~" {
        return Some("cannot delete root or home path");
    }
    if forward.len() == 2 && forward.as_bytes()[1] == b':' {
        return Some("cannot delete a drive root");
    }
    None
}

/// Vérifie un chemin **résolu** (absolu) pour opérations de suppression.
#[must_use]
pub fn dangerous_removal_resolved_path(resolved: &Utf8Path) -> bool {
    let forward = resolved.as_str().replace('\\', "/");
    let normalized = if forward == "/" {
        forward
    } else {
        forward.trim_end_matches('/').to_string()
    };

    if normalized == "/" {
        return true;
    }
    if normalized.len() == 2 && normalized.as_bytes().get(1) == Some(&b':') {
        return true;
    }
    if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
        let home_fwd = home.replace('\\', "/").trim_end_matches('/').to_string();
        if normalized == home_fwd {
            return true;
        }
    }
    if let Some(parent) = Utf8Path::new(&normalized).parent() {
        if parent.as_str() == "/" || parent.as_str().is_empty() {
            return true;
        }
        if parent.as_str().len() == 2
            && parent.as_str().as_bytes().get(1) == Some(&b':')
            && !normalized.contains('/')
        {
            return true;
        }
    }
    false
}

/// Segments workspace à ne pas supprimer sans réflexion (`.git`, config IDE).
#[must_use]
pub fn is_protected_workspace_entry(rel_under_root: &str) -> bool {
    let rel = rel_under_root.trim_start_matches('/').trim_start_matches('\\');
    matches!(
        rel,
        ".git" | ".drox" | ".vscode" | ".idea" | "node_modules" | "target"
    ) || rel.starts_with(".git/")
        || rel.starts_with(".drox/")
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;

    use super::*;
    use crate::error::ToolError;

    #[test]
    fn relative_stays_under_root() {
        let tmp = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        fs::create_dir_all(root.join("foo")).unwrap();
        let f = root.join("foo").join("bar.txt");
        fs::File::create(&f).unwrap().write_all(b"x").unwrap();
        let resolved = resolve_under_workspace(&root, "foo/bar.txt").unwrap();
        assert!(resolved.as_str().contains("bar.txt"));
    }

    #[test]
    fn dangerous_removal_rejects_glob() {
        assert!(dangerous_removal_user_path_reason("src/*").is_some());
        assert!(dangerous_removal_user_path_reason("*").is_some());
        assert!(dangerous_removal_user_path_reason("foo.txt").is_none());
    }

    #[test]
    fn protected_workspace_entries() {
        assert!(is_protected_workspace_entry(".git"));
        assert!(is_protected_workspace_entry(".git/objects"));
        assert!(!is_protected_workspace_entry("src/main.rs"));
    }

    #[test]
    fn absolute_outside_workspace_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        let outside = std::env::temp_dir().join("drox_path_escape_test.txt");
        fs::write(&outside, b"x").unwrap();
        let outside_utf8 = Utf8PathBuf::from_path_buf(outside.clone()).unwrap();
        let err = resolve_under_workspace(&root, outside_utf8.as_str()).unwrap_err();
        assert!(matches!(err, ToolError::PathEscape { .. }));
        let _ = fs::remove_file(&outside);
    }
}
