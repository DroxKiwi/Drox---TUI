//! Branche git courante (`/branch`).

use camino::Utf8Path;
use std::process::Command;

use super::EngineRuntime;

impl EngineRuntime {
    #[must_use]
    pub fn format_branch_lines(&self) -> Vec<String> {
        let mut lines = vec!["Branche git".into(), format!("  workspace : {}", self.workspace)];
        match git_branch(&self.workspace) {
            Ok(branch) => {
                lines.push(format!("  branche : {branch}"));
            }
            Err(msg) => lines.push(format!("  {msg}")),
        }
        lines
    }
}

pub(crate) fn git_branch(workspace: &Utf8Path) -> Result<String, String> {
    let out = Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(workspace.as_std_path())
        .output()
        .map_err(|e| format!("git indisponible : {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(format!(
            "pas un dépôt git ou erreur : {}",
            err.trim()
        ));
    }
    let branch = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if branch.is_empty() {
        Ok("(détachée ou vide)".into())
    } else {
        Ok(branch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_branch_on_non_repo_fails_gracefully() {
        let tmp = camino::Utf8PathBuf::from(std::env::temp_dir().to_string_lossy().as_ref());
        let err = git_branch(&tmp);
        assert!(err.is_err());
    }
}
