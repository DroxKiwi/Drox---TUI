//! Git worktrees locaux (sprint §2.32) — `.drox/worktrees/<slug>/`.
//!
//! Inspiré leak `EnterWorktreeTool` / `ExitWorktreeTool` + `worktree.ts`.

mod session;

use std::path::Path;
use std::process::Stdio;

use camino::{Utf8Path, Utf8PathBuf};
use session::{WorktreeSession, load_session, save_session};
use thiserror::Error;
use tokio::process::Command;

pub use session::SESSION_FILENAME;

const MAX_SLUG_LEN: usize = 64;
const VALID_SEGMENT: &str = r"^[a-zA-Z0-9._-]+$";

#[derive(Debug, Error)]
pub enum WorktreeError {
    #[error("git worktree: {0}")]
    Msg(String),
    #[error("git worktree: io ({path}): {source}")]
    Io {
        path: Utf8PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("git worktree: git failed ({args}): {stderr}")]
    Git { args: String, stderr: String },
}

/// Racine effective pour les tools fs/bash quand une session worktree est active.
#[must_use]
pub fn effective_workspace_root(workspace: &Utf8Path) -> Utf8PathBuf {
    let Ok(Some(session)) = load_session(workspace) else {
        return workspace.to_path_buf();
    };
    if !session.active {
        return workspace.to_path_buf();
    }
    let Ok(ws) = canonical_utf8(workspace) else {
        return workspace.to_path_buf();
    };
    let Ok(main) = canonical_utf8(&session.main_repo_root) else {
        return workspace.to_path_buf();
    };
    let Ok(wt) = canonical_utf8(&session.worktree_path) else {
        return workspace.to_path_buf();
    };
    if ws == main || ws == wt {
        return session.worktree_path.clone();
    }
    workspace.to_path_buf()
}

pub fn validate_slug(slug: &str) -> Result<(), WorktreeError> {
    if slug.is_empty() {
        return Err(WorktreeError::Msg("worktree name must not be empty".into()));
    }
    if slug.len() > MAX_SLUG_LEN {
        return Err(WorktreeError::Msg(format!(
            "worktree name must be at most {MAX_SLUG_LEN} characters"
        )));
    }
    for segment in slug.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." {
            return Err(WorktreeError::Msg(format!(
                "invalid worktree name `{slug}`: forbidden path segment"
            )));
        }
        let re = regex::Regex::new(VALID_SEGMENT).expect("slug segment regex");
        if !re.is_match(segment) {
            return Err(WorktreeError::Msg(format!(
                "invalid worktree name `{slug}`: segments may only contain letters, digits, ., _, -"
            )));
        }
    }
    Ok(())
}

fn flatten_slug(slug: &str) -> String {
    slug.replace('/', "+")
}

fn branch_name(slug: &str) -> String {
    format!("worktree-{}", flatten_slug(slug))
}

fn worktree_dir(main_repo: &Utf8Path, slug: &str) -> Utf8PathBuf {
    main_repo.join(".drox").join("worktrees").join(flatten_slug(slug))
}

fn canonical_utf8(path: &Utf8Path) -> Result<Utf8PathBuf, std::io::Error> {
    let canon = std::fs::canonicalize(path.as_std_path())?;
    Utf8PathBuf::from_path_buf(canon).map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, "path is not UTF-8")
    })
}

/// Cherche la racine git en remontant depuis `start`.
pub fn find_git_root(start: &Utf8Path) -> Option<Utf8PathBuf> {
    let mut dir = start;
    loop {
        if dir.join(".git").exists() {
            return Some(dir.to_path_buf());
        }
        dir = dir.parent()?;
    }
}

async fn git(cwd: &Utf8Path, args: &[&str]) -> Result<String, WorktreeError> {
    let display = args.join(" ");
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd.as_std_path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_ASKPASS", "")
        .output()
        .await
        .map_err(|e| WorktreeError::Io {
            path: cwd.to_path_buf(),
            source: e,
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(WorktreeError::Git {
            args: display,
            stderr,
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

async fn main_repo_root(git_root: &Utf8Path) -> Result<Utf8PathBuf, WorktreeError> {
    let common = git(git_root, &["rev-parse", "--git-common-dir"]).await?;
    let common_path = if Path::new(&common).is_absolute() {
        Utf8PathBuf::from(common)
    } else {
        git_root.join(common)
    };
    let canon = canonical_utf8(&common_path).map_err(|e| WorktreeError::Io {
        path: common_path.clone(),
        source: e,
    })?;
    canon
        .parent()
        .map(|p| p.to_path_buf())
        .ok_or_else(|| WorktreeError::Msg("cannot resolve main repository root".into()))
}

struct ChangeSummary {
    changed_files: usize,
    commits: usize,
}

async fn count_changes(
    worktree_path: &Utf8Path,
    original_head: &str,
) -> Result<ChangeSummary, WorktreeError> {
    let status = git(worktree_path, &["status", "--porcelain"]).await?;
    let changed_files = status.lines().filter(|l| !l.trim().is_empty()).count();
    let rev = git(
        worktree_path,
        &["rev-list", "--count", &format!("{original_head}..HEAD")],
    )
    .await?;
    let commits = rev.parse::<usize>().unwrap_or(0);
    Ok(ChangeSummary {
        changed_files,
        commits,
    })
}

/// Crée ou reprend un worktree et active la session (fichier `.drox/worktree-session.json`).
pub async fn enter_worktree(
    workspace: &Utf8Path,
    name: Option<&str>,
) -> Result<WorktreeSession, WorktreeError> {
    if let Ok(Some(s)) = load_session(workspace) {
        if s.active {
            return Err(WorktreeError::Msg(
                "already in an active worktree session — call git_worktree_exit first".into(),
            ));
        }
    }

    let git_root = find_git_root(workspace).ok_or_else(|| {
        WorktreeError::Msg("not inside a git repository (no .git found)".into())
    })?;
    let main_repo = main_repo_root(&git_root).await?;

    let slug = match name {
        Some(n) => {
            validate_slug(n)?;
            n.to_string()
        }
        None => {
            let id = uuid::Uuid::new_v4();
            format!("run-{}", &id.simple().to_string()[..8])
        }
    };

    let wt_path = worktree_dir(&main_repo, &slug);
    let branch = branch_name(&slug);

    let head_commit = if wt_path.is_dir() {
        git(&wt_path, &["rev-parse", "HEAD"]).await?
    } else {
        let worktrees_parent = main_repo.join(".drox").join("worktrees");
        std::fs::create_dir_all(worktrees_parent.as_std_path()).map_err(|e| WorktreeError::Io {
            path: worktrees_parent.clone(),
            source: e,
        })?;
        let base = git(&main_repo, &["rev-parse", "HEAD"]).await?;
        let wt_arg = wt_path
            .strip_prefix(&main_repo)
            .map(|p| p.as_str())
            .unwrap_or(wt_path.as_str());
        git(
            &main_repo,
            &["worktree", "add", "-B", &branch, wt_arg, "HEAD"],
        )
        .await?;
        base
    };

    let session = WorktreeSession {
        active: true,
        main_repo_root: main_repo.clone(),
        worktree_path: wt_path.clone(),
        worktree_name: slug,
        worktree_branch: branch,
        original_head_commit: head_commit,
    };
    save_session(&main_repo, &session)?;
    Ok(session)
}

/// Sortie worktree : `keep` conserve le dossier ; `remove` supprime worktree + branche.
pub async fn exit_worktree(
    workspace: &Utf8Path,
    action: &str,
    discard_changes: bool,
) -> Result<WorktreeSession, WorktreeError> {
    let session = load_session(workspace)?
        .filter(|s| s.active)
        .ok_or_else(|| {
            WorktreeError::Msg(
                "no active worktree session — use git_worktree_enter first".into(),
            )
        })?;

    if action == "remove" && !discard_changes {
        let summary = count_changes(&session.worktree_path, &session.original_head_commit).await?;
        if summary.changed_files > 0 || summary.commits > 0 {
            return Err(WorktreeError::Msg(format!(
                "worktree has {} uncommitted file(s) and {} commit(s) — re-invoke with \
                 discard_changes: true after user confirmation, or action: \"keep\"",
                summary.changed_files, summary.commits
            )));
        }
    }

    if action == "keep" {
        let mut kept = session.clone();
        kept.active = false;
        save_session(&session.main_repo_root, &kept)?;
        return Ok(session);
    }

    if action != "remove" {
        return Err(WorktreeError::Msg(
            "action must be \"keep\" or \"remove\"".into(),
        ));
    }

    let wt_arg = session
        .worktree_path
        .strip_prefix(&session.main_repo_root)
        .map(|p| p.as_str())
        .unwrap_or(session.worktree_path.as_str());
    let _ = git(
        &session.main_repo_root,
        &["worktree", "remove", "--force", wt_arg],
    )
    .await;
    let _ = git(
        &session.main_repo_root,
        &["branch", "-D", &session.worktree_branch],
    )
    .await;

    let mut cleared = session.clone();
    cleared.active = false;
    save_session(&session.main_repo_root, &cleared)?;

    Ok(session)
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8PathBuf;
    use tempfile::tempdir;

    #[test]
    fn validate_slug_rejects_traversal() {
        assert!(validate_slug("ok/name").is_ok());
        assert!(validate_slug("../escape").is_err());
        assert!(validate_slug("a/../b").is_err());
    }

    #[test]
    fn flatten_slug_maps_slashes() {
        assert_eq!(flatten_slug("feat/foo"), "feat+foo");
        assert_eq!(branch_name("feat/foo"), "worktree-feat+foo");
    }

    #[tokio::test]
    async fn enter_and_exit_worktree_roundtrip() {
        if std::process::Command::new("git")
            .arg("--version")
            .status()
            .is_err()
        {
            return;
        }
        let dir = tempdir().unwrap();
        let ws = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(ws.as_std_path())
            .status()
            .unwrap();
        std::process::Command::new("git")
            .args(["config", "user.email", "t@drox.dev"])
            .current_dir(ws.as_std_path())
            .status()
            .unwrap();
        std::process::Command::new("git")
            .args(["config", "user.name", "drox"])
            .current_dir(ws.as_std_path())
            .status()
            .unwrap();
        std::fs::write(ws.join("README.md"), "hi").unwrap();
        std::process::Command::new("git")
            .args(["add", "README.md"])
            .current_dir(ws.as_std_path())
            .status()
            .unwrap();
        std::process::Command::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(ws.as_std_path())
            .status()
            .unwrap();

        let session = enter_worktree(&ws, Some("feat")).await.unwrap();
        assert!(session.active);
        assert!(
            session
                .worktree_path
                .as_str()
                .replace('\\', "/")
                .contains(".drox/worktrees/feat")
        );

        let effective = effective_workspace_root(&ws);
        assert_eq!(effective, session.worktree_path);

        exit_worktree(&ws, "remove", true).await.unwrap();
        assert!(!session.worktree_path.is_dir() || !session.worktree_path.join(".git").exists());
    }
}
