//! `/diff` — état git du workspace.

use camino::Utf8Path;

use super::EngineRuntime;

impl EngineRuntime {
    /// Lignes `git status` + `git diff --stat`.
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
        lines.push("Astuce : `git diff` dans un shell pour le patch complet.".into());
        lines
    }
}

async fn append_git_output(
    lines: &mut Vec<String>,
    label: &str,
    workspace: &Utf8Path,
    args: &[&str],
) {
    match tokio::process::Command::new("git")
        .args(args)
        .current_dir(workspace.as_std_path())
        .output()
        .await
    {
        Ok(out) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout);
            let trimmed = text.trim();
            if trimmed.is_empty() {
                lines.push(format!("— {label} : (vide)"));
            } else {
                lines.push(format!("— {label}"));
                for line in trimmed.lines().take(40) {
                    lines.push(format!("  {line}"));
                }
                if trimmed.lines().count() > 40 {
                    lines.push("  …".into());
                }
            }
        }
        Ok(out) => {
            let err = String::from_utf8_lossy(&out.stderr);
            lines.push(format!("— {label} : git erreur ({})", out.status));
            if !err.trim().is_empty() {
                lines.push(format!("  {err}"));
            }
        }
        Err(e) => lines.push(format!("— {label} : {e}")),
    }
}
