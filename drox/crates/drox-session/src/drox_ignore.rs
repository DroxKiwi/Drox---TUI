//! `.droxignore` — chemins interdits à toute lecture agent (§2.34).

use std::path::Path;
use std::sync::Arc;

use camino::{Utf8Path, Utf8PathBuf};
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use ignore::WalkBuilder;

use crate::error::SessionError;

pub const DROXIGNORE_FILENAME: &str = ".droxignore";

/// Template généré si le fichier est absent à la première utilisation.
pub const DEFAULT_DROXIGNORE_TEMPLATE: &str = r"# Drox — chemins interdits à la lecture agent (obligatoire)
# Syntaxe gitignore. Ne pas retirer les motifs secrets sans revue équipe.

.env
.env.*
!.env.example
**/*.pem
**/*.key
**/*.p12
**/*.pfx
**/secrets/
**/.aws/credentials
**/.ssh/id_*

# Dumps / artefacts volumineux (adapter)
**/*.sql
**/*.dump
**/backups/
";

const ALWAYS_ALLOW: &[&str] = &[".droxignore", ".gitignore"];

/// Matcher chargé depuis `<workspace>/.droxignore`.
#[derive(Clone)]
pub struct DroxIgnoreMatcher {
    workspace: Utf8PathBuf,
    droxignore_path: Utf8PathBuf,
    matcher: Arc<Gitignore>,
    pattern_lines: usize,
}

impl DroxIgnoreMatcher {
    /// Charge `.droxignore` ou crée le fichier avec le template par défaut.
    pub async fn load_or_create(workspace: Utf8PathBuf) -> Result<Self, SessionError> {
        let droxignore_path = workspace.join(DROXIGNORE_FILENAME);
        if !droxignore_path.is_file() {
            tokio::fs::write(droxignore_path.as_std_path(), DEFAULT_DROXIGNORE_TEMPLATE).await?;
        }
        Self::load(workspace).await
    }

    /// Charge un `.droxignore` existant (erreur si absent).
    pub async fn load(workspace: Utf8PathBuf) -> Result<Self, SessionError> {
        let droxignore_path = workspace.join(DROXIGNORE_FILENAME);
        let (matcher, pattern_lines) = build_matcher(workspace.as_std_path(), &droxignore_path)?;
        Ok(Self {
            workspace,
            droxignore_path,
            matcher: Arc::new(matcher),
            pattern_lines,
        })
    }

    #[must_use]
    pub fn workspace(&self) -> &Utf8Path {
        &self.workspace
    }

    #[must_use]
    pub fn droxignore_path(&self) -> &Utf8Path {
        &self.droxignore_path
    }

    #[must_use]
    pub fn pattern_lines(&self) -> usize {
        self.pattern_lines
    }

    /// Chemin relatif POSIX sous le workspace ; `None` si hors racine.
    #[must_use]
    pub fn relative_path(&self, path: &Path) -> Option<String> {
        let workspace = std::fs::canonicalize(self.workspace.as_std_path())
            .unwrap_or_else(|_| self.workspace.as_std_path().to_path_buf());
        let abs = if path.is_absolute() {
            std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
        } else {
            std::fs::canonicalize(workspace.join(path))
                .unwrap_or_else(|_| workspace.join(path))
        };
        let rel = abs.strip_prefix(&workspace).ok()?;
        let s = rel.to_string_lossy();
        let trimmed = s.trim_start_matches(['/', '\\']);
        if trimmed.contains("..") {
            return None;
        }
        Some(trimmed.to_string())
    }

    /// `true` si le chemin (absolu ou relatif au workspace) est interdit.
    #[must_use]
    pub fn is_ignored(&self, path: &Path) -> bool {
        let Some(rel) = self.relative_path(path) else {
            return false;
        };
        if ALWAYS_ALLOW.iter().any(|p| *p == rel) {
            return false;
        }
        let is_dir = {
            let workspace = std::fs::canonicalize(self.workspace.as_std_path())
                .unwrap_or_else(|_| self.workspace.as_std_path().to_path_buf());
            let abs = if path.is_absolute() {
                std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
            } else {
                std::fs::canonicalize(workspace.join(path))
                    .unwrap_or_else(|_| workspace.join(path))
            };
            abs.is_dir()
        };
        self.matcher.matched(rel.as_str(), is_dir).is_ignore()
    }

    /// Filtre une liste de chemins (absolus ou relatifs) ; retourne gardés, omis, échantillon omis.
    #[must_use]
    pub fn filter_paths(&self, paths: Vec<String>) -> (Vec<String>, usize, Vec<String>) {
        let mut kept = Vec::new();
        let mut omitted = 0usize;
        let mut sample = Vec::new();
        for p in paths {
            if self.is_ignored(Path::new(&p)) {
                omitted += 1;
                if sample.len() < 5 {
                    sample.push(p);
                }
            } else {
                kept.push(p);
            }
        }
        (kept, omitted, sample)
    }

    /// Configure un `WalkBuilder` avec `.gitignore` + `.droxignore`.
    pub fn configure_walk(&self, builder: &mut WalkBuilder) {
        builder.git_ignore(true);
        if self.droxignore_path.is_file() {
            let _ = builder.add_ignore(self.droxignore_path.as_std_path());
        }
    }

    /// Rappel court pour le system prompt.
    #[must_use]
    pub fn format_for_prompt(&self) -> String {
        format!(
            "[Drox ignore] {n} motif(s) actifs dans `{file}` — lecture agent **interdite** \
             sur ces chemins (`file_read`, `glob`, `grep`, carte workspace, …). \
             Le moteur filtre ou refuse ; ne pas contourner via chemins absolus hors workspace.",
            n = self.pattern_lines,
            file = DROXIGNORE_FILENAME,
        )
    }
}

fn build_matcher(
    workspace: &Path,
    droxignore_path: &Utf8Path,
) -> Result<(Gitignore, usize), SessionError> {
    let mut builder = GitignoreBuilder::new(workspace);
    let mut pattern_lines = 0usize;
    let content = std::fs::read_to_string(droxignore_path.as_std_path())?;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        builder
            .add_line(Some(droxignore_path.as_std_path().to_path_buf()), trimmed)
            .map_err(|_| SessionError::InvalidPath)?;
        pattern_lines += 1;
    }
    let matcher = builder.build().map_err(|_| SessionError::InvalidPath)?;
    Ok((matcher, pattern_lines))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn blocks_env_and_allows_readme() {
        let dir = tempdir().unwrap();
        let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        std::fs::write(root.join(".env"), "SECRET=1").unwrap();
        std::fs::write(root.join("README.md"), "ok").unwrap();
        let m = DroxIgnoreMatcher::load_or_create(root.clone()).await.unwrap();
        assert!(m.is_ignored(root.join(".env").as_std_path()));
        assert!(!m.is_ignored(root.join("README.md").as_std_path()));
    }

    #[tokio::test]
    async fn filter_paths_omits_env() {
        let dir = tempdir().unwrap();
        let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        std::fs::write(root.join(".env"), "").unwrap();
        let m = DroxIgnoreMatcher::load_or_create(root.clone()).await.unwrap();
        let paths = vec![
            root.join(".env").to_string(),
            root.join("src/main.rs").to_string(),
        ];
        let (kept, omitted, sample) = m.filter_paths(paths);
        assert_eq!(omitted, 1);
        assert_eq!(kept.len(), 1);
        assert!(kept[0].contains("main.rs"));
        assert!(sample[0].contains(".env"));
    }

    #[tokio::test]
    async fn always_allows_droxignore_file() {
        let dir = tempdir().unwrap();
        let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let m = DroxIgnoreMatcher::load_or_create(root.clone()).await.unwrap();
        assert!(!m.is_ignored(root.join(DROXIGNORE_FILENAME).as_std_path()));
    }
}
