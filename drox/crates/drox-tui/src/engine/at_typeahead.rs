//! Complétion `@chemin` dans le composer — inspiré du leak `fileSuggestions.ts`.
//!
//! Flux TUI :
//! 1. `active_at_query` détecte un token `@…` en fin de buffer ;
//! 2. `AtFileIndex` indexe une fois les fichiers du workspace (borné) ;
//! 3. `query` filtre l'index par préfixe ; Tab remplace le token par `@chemin/complet`.

use camino::{Utf8Path, Utf8PathBuf};
use drox_session::DroxIgnoreMatcher;

/// Nombre max de suggestions affichées dans la popup (partagé avec `unified_suggestions`).
pub const MAX_SUGGESTIONS: usize = 12;

/// Profondeur max de parcours depuis la racine workspace.
const MAX_WALK_DEPTH: usize = 6;

/// Plafond d'entrées dans l'index (évite de scanner des monorepos entiers à chaque `@`).
const MAX_INDEX_ENTRIES: usize = 4_000;

/// Dossiers systématiquement ignorés pendant l'indexation (perf — indépendant de `.droxignore`).
const SKIP_DIR_NAMES: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    "__pycache__",
    "vendor",
    ".next",
    ".turbo",
    "coverage",
    ".cargo",
];

/// Token `@fichier` actif en fin de saisie.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtQuery {
    /// Position byte du caractère `@` dans le buffer composer.
    pub at_start: usize,
    /// Texte saisi après `@` (préfixe partiel du chemin).
    pub prefix: String,
}

/// Index fichiers workspace — construit une fois, filtré à chaque frappe.
#[derive(Debug, Clone)]
pub struct AtFileIndex {
    /// Chemins relatifs au workspace, triés lexicographiquement (séparateur `/`).
    paths: Vec<String>,
}

impl AtFileIndex {
    /// Parcourt le workspace et collecte les fichiers non ignorés.
    #[must_use]
    pub fn build(workspace: &Utf8Path, ignore: &DroxIgnoreMatcher) -> Self {
        Self::build_roots(&[workspace.to_path_buf()], ignore)
    }

    /// Indexe une ou plusieurs racines (`workspace` + `/add-dir`).
    #[must_use]
    pub fn build_roots(roots: &[Utf8PathBuf], ignore: &DroxIgnoreMatcher) -> Self {
        let mut paths = Vec::with_capacity(512);
        for root in roots {
            walk_files(
                root,
                ignore,
                Utf8Path::new(""),
                root.as_std_path(),
                0,
                &mut paths,
            );
        }
        paths.sort();
        paths.dedup();
        Self { paths }
    }

    /// Filtre l'index selon le préfixe après `@`, trié par pertinence.
    #[must_use]
    pub fn query(&self, prefix: &str, limit: usize) -> Vec<String> {
        let norm_prefix = normalize_seps(prefix);
        let mut scored: Vec<(u8, usize, &str)> = self
            .paths
            .iter()
            .filter_map(|p| {
                let rank = score_match(p, &norm_prefix)?;
                Some((rank.0, rank.1, p.as_str()))
            })
            .collect();
        scored.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(b.2)));
        scored
            .into_iter()
            .take(limit)
            .map(|(_, _, p)| p.to_string())
            .collect()
    }
}

/// Détecte un token `@chemin` en cours de frappe à la **fin** du buffer.
///
/// Règles alignées sur le leak (`HAS_AT_SYMBOL`) :
/// - le `@` doit être en début de ligne ou précédé d'un espace / saut de ligne ;
/// - tout le texte après `@` jusqu'à la fin du buffer forme le préfixe partiel ;
/// - un espace dans ce suffixe invalide le token (pas de `@` au milieu d'une phrase).
#[must_use]
pub fn active_at_query(buffer: &str) -> Option<AtQuery> {
    let at_start = find_at_token_start(buffer)?;
    let prefix = buffer.get(at_start + 1..)?.to_string();
    if prefix.contains(' ') {
        return None;
    }
    if !prefix.is_empty() && !prefix.chars().all(is_path_char) {
        return None;
    }
    Some(AtQuery { at_start, prefix })
}

/// Remplace le préfixe partiel par le chemin choisi (`@src/foo` → `@src/foo/bar.rs`).
#[must_use]
pub fn apply_completion(buffer: &str, query: &AtQuery, relative_path: &str) -> String {
    let mut out = String::with_capacity(buffer.len() + relative_path.len());
    out.push_str(&buffer[..query.at_start]);
    out.push('@');
    out.push_str(relative_path);
    out
}

/// Cherche le `@` du token actif : le dernier `@` valide tel que tout le suffixe en découle.
fn find_at_token_start(buffer: &str) -> Option<usize> {
    let mut last: Option<usize> = None;
    for (idx, ch) in buffer.char_indices() {
        if ch != '@' {
            continue;
        }
        let valid_before = idx == 0
            || buffer[..idx]
                .chars()
                .last()
                .is_some_and(|c| c.is_whitespace());
        if valid_before {
            last = Some(idx);
        }
    }
    last
}

/// Caractères autorisés dans un chemin partiel pendant la frappe.
fn is_path_char(c: char) -> bool {
    c.is_alphanumeric()
        || matches!(
            c,
            '_' | '-' | '.' | '/' | '\\' | '(' | ')' | '[' | ']' | '~' | ':'
        )
}

/// Normalise les séparateurs pour comparer chemins Windows / Unix.
fn normalize_seps(s: &str) -> String {
    s.replace('\\', "/")
}

/// Score de correspondance — plus petit = meilleur. `None` = pas de match.
fn score_match(path: &str, prefix: &str) -> Option<(u8, usize)> {
    let path_norm = normalize_seps(path);
    let path_lower = path_norm.to_ascii_lowercase();
    let prefix_lower = normalize_seps(prefix).to_ascii_lowercase();

    if prefix_lower.is_empty() {
        // `@` seul : favoriser les entrées peu profondes (racine puis 1er niveau).
        let depth = path_norm.matches('/').count();
        return Some((0, depth * 100 + path_norm.len()));
    }

    if path_lower.starts_with(&prefix_lower) {
        return Some((1, path_norm.len()));
    }

    // Dernier segment du préfixe (ex. `@foo` peut matcher `src/foo/bar.rs`).
    let last_seg = prefix_lower
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())?;
    if path_lower.contains(last_seg) {
        return Some((2, path_norm.len()));
    }

    None
}

/// Parcours récursif borné — fichiers uniquement, respecte `.droxignore`.
fn walk_files(
    _workspace: &Utf8Path,
    ignore: &DroxIgnoreMatcher,
    rel_dir: &Utf8Path,
    abs_dir: &std::path::Path,
    depth: usize,
    out: &mut Vec<String>,
) {
    if depth > MAX_WALK_DEPTH || out.len() >= MAX_INDEX_ENTRIES {
        return;
    }
    let entries = match std::fs::read_dir(abs_dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        if out.len() >= MAX_INDEX_ENTRIES {
            break;
        }
        let path = entry.path();
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();

        if path.is_dir() {
            if SKIP_DIR_NAMES.iter().any(|skip| *skip == name.as_ref()) {
                continue;
            }
        }

        if ignore.is_ignored(&path) {
            continue;
        }

        let rel = if rel_dir.as_str().is_empty() {
            Utf8PathBuf::from(name.as_ref())
        } else {
            rel_dir.join(name.as_ref())
        };

        if path.is_file() {
            out.push(rel.as_str().replace('\\', "/"));
        } else if path.is_dir() {
            walk_files(_workspace, ignore, &rel, &path, depth + 1, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn detects_trailing_at_token() {
        assert_eq!(
            active_at_query("voir @src/ma"),
            Some(AtQuery {
                at_start: 5,
                prefix: "src/ma".into(),
            })
        );
        assert!(active_at_query("email@host.com").is_none());
        assert!(active_at_query("@src/foo bar").is_none());
    }

    #[test]
    fn apply_completion_replaces_suffix() {
        let q = AtQuery {
            at_start: 0,
            prefix: "src/".into(),
        };
        assert_eq!(
            apply_completion("@src/", &q, "src/lib.rs"),
            "@src/lib.rs"
        );
    }

    #[tokio::test]
    async fn index_and_query_prefix() {
        let dir = tempdir().unwrap();
        let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/main.rs"), "").unwrap();
        std::fs::write(root.join("README.md"), "").unwrap();

        let ignore = DroxIgnoreMatcher::load_or_create(root.clone())
            .await
            .unwrap();
        let index = AtFileIndex::build(&root, &ignore);
        let hits = index.query("src/", 5);
        assert!(hits.iter().any(|p| p.ends_with("main.rs")));
    }
}
