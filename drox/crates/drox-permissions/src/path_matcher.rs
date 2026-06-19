//! Matching de chemins pour règles `file_*` — sémantique gitignore (cf. leak `filesystem.ts`).
//!
//! Patterns supportés (V1) :
//! - relatif au cwd : `**/*.env`, `.env`, `src/**`
//! - absolu projet : `/secrets/**` (racine = répertoire des settings selon `RuleSource`)
//! - home : `~/.ssh/**`
//! - racine système : `//etc/**` (POSIX) ; sur Windows `//c/Users/**` → lecteur `C:\`

use std::path::{Path, PathBuf};

use ignore::gitignore::GitignoreBuilder;

use crate::rule::RuleSource;

/// Contexte de résolution des chemins pour une session / workspace.
#[derive(Debug, Clone)]
pub struct PathMatchContext {
    /// Racine du workspace (cwd agent).
    pub cwd: PathBuf,
    /// Répertoire home utilisateur.
    pub home: PathBuf,
}

impl PathMatchContext {
    /// Construit un contexte à partir de cwd + home.
    #[must_use]
    pub fn new(cwd: impl Into<PathBuf>, home: impl Into<PathBuf>) -> Self {
        Self {
            cwd: cwd.into(),
            home: home.into(),
        }
    }
}

/// Racine + motif relatif (comme `patternWithRoot` côté leak).
#[derive(Debug, Clone)]
struct PatternRoot {
    root: Option<PathBuf>,
    relative_pattern: String,
}

/// Teste si `file_path` matche un motif de permission fichier.
#[must_use]
pub fn path_rule_matches(
    pattern: &str,
    file_path: &str,
    source: RuleSource,
    ctx: &PathMatchContext,
) -> bool {
    let pr = split_pattern_root(pattern, source, ctx);
    let abs = expand_path(file_path, ctx);
    let abs = normalize_separators(&abs);
    let root = pr
        .root
        .as_ref()
        .map(|r| normalize_separators(r))
        .unwrap_or_else(|| normalize_separators(&ctx.cwd));

    let relative = relative_path_posix(&root, &abs);
    let Some(rel) = relative else {
        return false;
    };
    if rel.is_empty() {
        return false;
    }

    let mut adjusted = pr.relative_pattern;
    if adjusted.ends_with("/**") {
        adjusted.truncate(adjusted.len() - 3);
    }

    let mut builder = GitignoreBuilder::new(&root);
    if builder.add_line(None, &adjusted).is_err() {
        return false;
    }
    let Ok(matcher) = builder.build() else {
        return false;
    };
    matcher.matched(&rel, false).is_ignore()
}

fn split_pattern_root(pattern: &str, source: RuleSource, ctx: &PathMatchContext) -> PatternRoot {
    let pattern = pattern.replace('\\', "/");

    if let Some(rest) = pattern.strip_prefix("//") {
        if cfg!(windows) {
            if let Some(drive) = rest.chars().next() {
                if drive.is_ascii_alphabetic() {
                    let after = rest.strip_prefix(&format!("{drive}/")).unwrap_or(rest);
                    let drive_root = PathBuf::from(format!("{}:\\", drive.to_ascii_uppercase()));
                    return PatternRoot {
                        root: Some(drive_root),
                        relative_pattern: after.trim_start_matches('/').to_string(),
                    };
                }
            }
        }
        return PatternRoot {
            root: Some(PathBuf::from(if cfg!(windows) { "\\" } else { "/" })),
            relative_pattern: format!("/{rest}"),
        };
    }

    if let Some(rest) = pattern.strip_prefix("~/") {
        return PatternRoot {
            root: Some(ctx.home.clone()),
            relative_pattern: rest.to_string(),
        };
    }

    if pattern.starts_with('/') {
        return PatternRoot {
            root: Some(settings_root_for_source(source, ctx)),
            relative_pattern: pattern.to_string(),
        };
    }

    let relative_pattern = pattern
        .strip_prefix("./")
        .map(str::to_string)
        .unwrap_or(pattern);

    PatternRoot {
        root: None,
        relative_pattern,
    }
}

fn settings_root_for_source(source: RuleSource, ctx: &PathMatchContext) -> PathBuf {
    match source {
        RuleSource::UserSettings => ctx.home.clone(),
        RuleSource::ProjectSettings | RuleSource::LocalSettings | RuleSource::CliArg
        | RuleSource::Session => ctx.cwd.clone(),
    }
}

fn expand_path(path: &str, ctx: &PathMatchContext) -> PathBuf {
    let trimmed = path.trim();
    if trimmed == "~" {
        return ctx.home.clone();
    }
    if let Some(rest) = trimmed.strip_prefix("~/").or_else(|| trimmed.strip_prefix("~\\")) {
        return ctx.home.join(rest);
    }
    let p = PathBuf::from(trimmed);
    if p.is_absolute() {
        p
    } else {
        ctx.cwd.join(p)
    }
}

fn normalize_separators(path: &Path) -> PathBuf {
    if cfg!(windows) {
        let s = path.to_string_lossy().replace('\\', "/");
        PathBuf::from(s)
    } else {
        path.to_path_buf()
    }
}

/// Chemin relatif POSIX ; `None` si hors racine.
fn relative_path_posix(base: &Path, target: &Path) -> Option<String> {
    let rel = target.strip_prefix(base).ok()?;
    let s = rel.to_string_lossy().replace('\\', "/");
    let trimmed = s.trim_start_matches('/');
    if trimmed.is_empty() || trimmed.starts_with("../") || trimmed.contains("/../") {
        return None;
    }
    Some(trimmed.to_string())
}

/// Fichiers / répertoires sensibles — auto-refus sans règle `Allow` explicite (V1).
#[must_use]
pub fn dangerous_path_reason(path: &str, ctx: &PathMatchContext) -> Option<&'static str> {
    let abs = expand_path(path, ctx);
    let abs = normalize_separators(&abs);
    let lower = abs.to_string_lossy().to_ascii_lowercase();

    const DANGEROUS_FILES: &[&str] = &[
        ".gitconfig",
        ".gitmodules",
        ".bashrc",
        ".bash_profile",
        ".zshrc",
        ".zprofile",
        ".profile",
        ".ripgreprc",
        ".mcp.json",
        ".drox.json",
    ];
    const DANGEROUS_DIRS: &[&str] = &[".git", ".vscode", ".idea", ".drox"];

    if let Some(file_name) = abs.file_name().and_then(|n| n.to_str()) {
        let name_lower = file_name.to_ascii_lowercase();
        if DANGEROUS_FILES.iter().any(|d| name_lower == d.to_ascii_lowercase()) {
            return Some("fichier de configuration sensible");
        }
        if name_lower == ".env" || name_lower.ends_with(".env.local") {
            return Some("fichier d'environnement (.env)");
        }
    }

    for dir in DANGEROUS_DIRS {
        let needle = if cfg!(windows) {
            format!("/{dir}/").to_ascii_lowercase()
        } else {
            format!("/{dir}/")
        };
        let needle_back = format!("\\{dir}\\").to_ascii_lowercase();
        if lower.contains(&needle) || lower.contains(&needle_back) {
            return Some("répertoire sensible");
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rule::RuleSource;

    fn ctx(root: &Path) -> PathMatchContext {
        PathMatchContext::new(root, root.join("home"))
    }

    #[test]
    fn matches_glob_env_in_tree() {
        let root = std::env::temp_dir().join("drox_perm_test_env");
        let _ = std::fs::create_dir_all(root.join("src"));
        let ctx = ctx(&root);
        assert!(path_rule_matches(
            "**/*.env",
            root.join("src/.env").to_str().unwrap(),
            RuleSource::ProjectSettings,
            &ctx,
        ));
        assert!(!path_rule_matches(
            "**/*.env",
            root.join("src/main.rs").to_str().unwrap(),
            RuleSource::ProjectSettings,
            &ctx,
        ));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn matches_root_relative_pattern() {
        let root = std::env::temp_dir().join("drox_perm_test_root");
        let _ = std::fs::create_dir_all(&root);
        let ctx = ctx(&root);
        assert!(path_rule_matches(
            "/.env",
            root.join(".env").to_str().unwrap(),
            RuleSource::ProjectSettings,
            &ctx,
        ));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn dangerous_detects_env() {
        let root = PathBuf::from("/tmp/drox_test");
        let ctx = PathMatchContext::new(&root, "/home/user");
        assert!(dangerous_path_reason(".env", &ctx).is_some());
        assert!(dangerous_path_reason("src/.git/config", &ctx).is_some());
    }
}
