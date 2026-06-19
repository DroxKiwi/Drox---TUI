//! Persistance de la session worktree (`.drox/worktree-session.json`).

use std::fs;

use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};

use super::WorktreeError;

pub const SESSION_FILENAME: &str = ".drox/worktree-session.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorktreeSession {
    pub active: bool,
    pub main_repo_root: Utf8PathBuf,
    pub worktree_path: Utf8PathBuf,
    pub worktree_name: String,
    pub worktree_branch: String,
    pub original_head_commit: String,
}

fn session_path(main_repo: &Utf8Path) -> Utf8PathBuf {
    main_repo.join(SESSION_FILENAME)
}

pub fn load_session(workspace: &Utf8Path) -> Result<Option<WorktreeSession>, WorktreeError> {
    let Some(git_root) = super::find_git_root(workspace) else {
        return Ok(None);
    };
    for candidate in session_lookup_roots(&git_root) {
        let path = session_path(&candidate);
        if path.is_file() {
            return read_session_file(&path);
        }
    }
    Ok(None)
}

/// Chemins où le fichier session peut vivre (dépôt principal, pas worktree lié).
fn session_lookup_roots(git_root: &Utf8Path) -> Vec<Utf8PathBuf> {
    let mut roots = vec![git_root.to_path_buf()];
    if let Some(main) = main_repo_root_sync(git_root) {
        if main != git_root {
            roots.push(main);
        }
    }
    roots
}

fn main_repo_root_sync(git_root: &Utf8Path) -> Option<Utf8PathBuf> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--git-common-dir"])
        .current_dir(git_root.as_std_path())
        .stdin(std::process::Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let common = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let common_path = if std::path::Path::new(&common).is_absolute() {
        Utf8PathBuf::from(common)
    } else {
        git_root.join(common)
    };
    let canon = std::fs::canonicalize(common_path.as_std_path()).ok()?;
    let utf8 = Utf8PathBuf::from_path_buf(canon).ok()?;
    utf8.parent().map(|p| p.to_path_buf())
}

fn read_session_file(path: &Utf8Path) -> Result<Option<WorktreeSession>, WorktreeError> {
    let raw = fs::read_to_string(path.as_std_path()).map_err(|e| WorktreeError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    let session: WorktreeSession =
        serde_json::from_str(&raw).map_err(|e| WorktreeError::Msg(e.to_string()))?;
    Ok(Some(session))
}

pub fn save_session(main_repo: &Utf8Path, session: &WorktreeSession) -> Result<(), WorktreeError> {
    let path = session_path(main_repo);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent.as_std_path()).map_err(|e| WorktreeError::Io {
            path: parent.to_path_buf(),
            source: e,
        })?;
    }
    let raw = serde_json::to_string_pretty(session).map_err(|e| WorktreeError::Msg(e.to_string()))?;
    fs::write(path.as_std_path(), raw).map_err(|e| WorktreeError::Io {
        path: path.to_path_buf(),
        source: e,
    })
}
