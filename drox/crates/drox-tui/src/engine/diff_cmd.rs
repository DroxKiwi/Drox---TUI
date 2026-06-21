//! `/diff` — modifications git du workspace (résumé ou viewer visuel).

use camino::Utf8Path;
use drox_types::ToolUseId;
use drox_tools::unified_line_diff;

use crate::view::lines_viewer::{GitWorkspaceNav, LinesViewerState};

use super::EngineRuntime;

const MAX_FILE_BYTES: u64 = 512 * 1024;

/// Résultat de `/diff` (mode visuel).
#[derive(Debug, Clone)]
pub enum GitDiffVisual {
    NotRepo,
    Empty,
    Viewer(LinesViewerState),
}

impl EngineRuntime {
    /// Lignes `git status` + `git diff --stat` (mode `/diff --stat`).
    pub async fn format_git_diff_lines(&self) -> Vec<String> {
        let mut lines = vec![format!("Git — {}", self.workspace)];
        if !self.workspace.join(".git").exists() {
            lines.push("  (pas un dépôt git)".into());
            return lines;
        }
        append_git_output(
            &mut lines,
            "status",
            &self.workspace,
            &["status", "--short"],
        )
        .await;
        append_git_output(
            &mut lines,
            "diff --stat",
            &self.workspace,
            &["diff", "--stat", "HEAD"],
        )
        .await;
        lines.push("Astuce : `/diff` sans option pour le patch visuel.".into());
        lines
    }

    /// Charge le viewer coloré (`git status` + `git diff HEAD`, optionnellement un fichier).
    pub async fn load_git_diff_visual(&self, file: Option<&str>) -> GitDiffVisual {
        if !self.workspace.join(".git").exists() {
            return GitDiffVisual::NotRepo;
        }

        let file_rel = file.map(|raw| normalize_git_path(&self.workspace, raw));

        let mut body = Vec::new();
        let status_nonempty = if let Some(ref rel) = file_rel {
            append_git_body(
                &mut body,
                "status",
                &self.workspace,
                &["status", "--short", "--", rel.as_str()],
            )
            .await
        } else {
            append_git_body(
                &mut body,
                "status",
                &self.workspace,
                &["status", "--short"],
            )
            .await
        };

        let diff_nonempty = if let Some(ref rel) = file_rel {
            append_git_file_diff(&mut body, &self.workspace, rel).await
        } else {
            append_git_body(
                &mut body,
                "diff (HEAD)",
                &self.workspace,
                &["diff", "HEAD"],
            )
            .await
        };

        if !status_nonempty && !diff_nonempty {
            return GitDiffVisual::Empty;
        }

        let nav = GitWorkspaceNav::index_body(&body, file_rel.clone());
        GitDiffVisual::Viewer(LinesViewerState::from_workspace_git(
            ToolUseId::new(),
            &self.workspace,
            body,
            nav,
        ))
    }
}

fn normalize_git_path(workspace: &Utf8Path, raw: &str) -> String {
    let trimmed = raw.trim().trim_start_matches("./");
    let path = Utf8Path::new(trimmed);
    if path.is_absolute() {
        if let Ok(rel) = path.strip_prefix(workspace) {
            return rel.to_string();
        }
    }
    trimmed.to_string()
}

async fn append_git_output(
    lines: &mut Vec<String>,
    label: &str,
    workspace: &Utf8Path,
    args: &[&str],
) {
    match run_git(workspace, args).await {
        GitRun::Ok(text) if text.trim().is_empty() => {
            lines.push(format!("— {label} : (vide)"));
        }
        GitRun::Ok(text) => {
            lines.push(format!("— {label}"));
            for line in text.lines().take(40) {
                lines.push(format!("  {line}"));
            }
            if text.lines().count() > 40 {
                lines.push("  …".into());
            }
        }
        GitRun::Err(msg) => lines.push(format!("— {label} : {msg}")),
    }
}

/// Ajoute une section au corps du viewer ; retourne `true` si non vide.
async fn append_git_body(
    body: &mut Vec<String>,
    label: &str,
    workspace: &Utf8Path,
    args: &[&str],
) -> bool {
    match run_git(workspace, args).await {
        GitRun::Ok(text) if text.trim().is_empty() => false,
        GitRun::Ok(text) => {
            if !body.is_empty() {
                body.push(String::new());
            }
            body.push(format!("— {label} —"));
            body.extend(text.lines().map(str::to_string));
            true
        }
        GitRun::Err(msg) => {
            if !body.is_empty() {
                body.push(String::new());
            }
            body.push(format!("— {label} : {msg}"));
            true
        }
    }
}

async fn append_git_file_diff(body: &mut Vec<String>, workspace: &Utf8Path, rel: &str) -> bool {
    let args = ["diff", "HEAD", "--", rel];
    match run_git(workspace, &args).await {
        GitRun::Ok(text) if text.trim().is_empty() => append_untracked_diff(body, workspace, rel).await,
        GitRun::Ok(text) => {
            if !body.is_empty() {
                body.push(String::new());
            }
            body.push(format!("— diff (HEAD) — {rel}"));
            body.extend(text.lines().map(str::to_string));
            true
        }
        GitRun::Err(msg) => {
            if !body.is_empty() {
                body.push(String::new());
            }
            body.push(format!("— diff : {msg}"));
            true
        }
    }
}

async fn append_untracked_diff(body: &mut Vec<String>, workspace: &Utf8Path, rel: &str) -> bool {
    let abs = workspace.join(rel);
    if !abs.exists() {
        if !body.is_empty() {
            body.push(String::new());
        }
        body.push(format!("— fichier introuvable : {rel} —"));
        return true;
    }

    let content = match tokio::fs::read_to_string(abs.as_std_path()).await {
        Ok(text) => text,
        Err(e) => {
            if !body.is_empty() {
                body.push(String::new());
            }
            body.push(format!("— lecture impossible : {e} —"));
            return true;
        }
    };

    let capped = if content.len() as u64 > MAX_FILE_BYTES {
        content
            .chars()
            .take(MAX_FILE_BYTES as usize)
            .collect::<String>()
    } else {
        content
    };

    let patch = unified_line_diff(rel, "", &capped);
    if patch.trim().is_empty() {
        return false;
    }

    if !body.is_empty() {
        body.push(String::new());
    }
    body.push(format!("— diff (non suivi) — {rel}"));
    body.extend(patch.lines().map(str::to_string));
    true
}

enum GitRun {
    Ok(String),
    Err(String),
}

async fn run_git(workspace: &Utf8Path, args: &[&str]) -> GitRun {
    match tokio::process::Command::new("git")
        .args(args)
        .current_dir(workspace.as_std_path())
        .output()
        .await
    {
        Ok(out) if out.status.success() => {
            GitRun::Ok(String::from_utf8_lossy(&out.stdout).into_owned())
        }
        Ok(out) => {
            let err = String::from_utf8_lossy(&out.stderr);
            GitRun::Err(format!(
                "git erreur ({}){}",
                out.status,
                if err.trim().is_empty() {
                    String::new()
                } else {
                    format!(" — {err}")
                }
            ))
        }
        Err(e) => GitRun::Err(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8Path;
    use crate::view::lines_viewer::{GitWorkspaceNav, LinesViewerStyle};

    #[test]
    fn normalize_relative_path() {
        let ws = Utf8Path::new("/tmp/proj");
        assert_eq!(
            normalize_git_path(&ws, "./src/a.rs"),
            "src/a.rs"
        );
    }

    #[test]
    fn workspace_viewer_uses_unified_diff_style() {
        let body = vec![
            "— status —".into(),
            " M src/lib.rs".into(),
            "— diff (HEAD) —".into(),
            "--- a/src/lib.rs".into(),
            "+++ b/src/lib.rs".into(),
            "@@ -1 +1 @@".into(),
            "-old".into(),
            "+new".into(),
        ];
        let nav = GitWorkspaceNav::index_body(&body, None);
        let v = LinesViewerState::from_workspace_git(
            ToolUseId::new(),
            Utf8Path::new("/tmp/proj"),
            body,
            nav,
        );
        assert!(v.title.contains("proj"));
        assert_eq!(v.style, LinesViewerStyle::UnifiedDiff);
        assert!(v.line_count() >= 6);
        assert!(v.git_nav.is_some());
    }
}
