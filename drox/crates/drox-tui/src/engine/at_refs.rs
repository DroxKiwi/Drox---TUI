//! Expansion des références `@fichier` avant envoi à l'agent.
//!
//! Inspiré du leak (`attachments.ts` : `extractAtMentionedFiles`, `processAtMentionedFiles`).
//! Le TUI n'a pas encore de blocs « attachment » séparés : on annexe le contenu lu
//! sous le message utilisateur, tout en conservant le texte original avec les `@`.

use std::fs;

use camino::{Utf8Path, Utf8PathBuf};
use drox_session::DroxIgnoreMatcher;

/// Taille max lue par fichier référencé (évite de saturer le contexte).
const MAX_FILE_BYTES: usize = 80_000;

/// Plafond d'entrées listées pour un répertoire `@dossier/`.
const MAX_DIR_ENTRIES: usize = 200;

/// Résultat de l'expansion — `agent_prompt` est ce qui part vers le moteur.
#[derive(Debug, Clone)]
pub struct AtRefExpansion {
    /// Texte enrichi envoyé à l'agent (message + annexes fichiers).
    pub agent_prompt: String,
    /// Messages système optionnels (fichier introuvable, ignoré, …).
    pub notes: Vec<String>,
}

/// Extrait les mentions `@fichier` d'un message (dédupliquées, ordre conservé).
///
/// Exclut volontairement :
/// - ressources MCP `@serveur:uri`
/// - agents `@agent-…` et `@"… (agent)"`
/// - adresses e-mail (`user@host`)
#[must_use]
pub fn extract_at_file_refs(content: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let bytes = content.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] != b'@' {
            i += 1;
            continue;
        }
        // `@` doit être en début de token (début ou espace).
        if i > 0 && !content[..i].chars().last().is_some_and(|c| c.is_whitespace()) {
            i += 1;
            continue;
        }

        let start = i + 1;
        if start >= bytes.len() {
            break;
        }

        let mention = if bytes[start] == b'"' {
            // @"chemin avec espaces"
            let mut end = start + 1;
            while end < bytes.len() && bytes[end] != b'"' {
                end += 1;
            }
            if end >= bytes.len() {
                break;
            }
            let inner = &content[start + 1..end];
            i = end + 1;
            if inner.ends_with(" (agent)") {
                continue;
            }
            inner.to_string()
        } else {
            // @chemin jusqu'au prochain espace
            let mut end = start;
            while end < bytes.len() && !bytes[end].is_ascii_whitespace() {
                end += 1;
            }
            let raw = &content[start..end];
            i = end;
            if is_skipped_mention(raw) {
                continue;
            }
            // `user@host.com` — `@` interne sans séparateur de chemin → e-mail, pas un fichier.
            if raw.contains('@') && !raw.contains('/') && !raw.contains('\\') {
                continue;
            }
            raw.to_string()
        };

        if seen.insert(mention.clone()) {
            out.push(mention);
        }
    }
    out
}

/// Découpe une mention en chemin + plage de lignes optionnelle (`#L10`, `#L10-20`).
#[must_use]
pub fn parse_at_file_lines(mention: &str) -> AtFileLines {
    let (path_part, line_start, line_end) = if let Some(hash) = mention.find("#L") {
        let (file, rest) = mention.split_at(hash);
        let rest = &rest[2..]; // après #L
        if let Some(dash) = rest.find('-') {
            let (a, b) = rest.split_at(dash);
            let end = &b[1..];
            let start = a.parse::<usize>().ok();
            let end = end
                .split(|c: char| !c.is_ascii_digit())
                .next()
                .and_then(|s| s.parse::<usize>().ok());
            (file, start, end)
        } else {
            let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            (file, digits.parse().ok(), None)
        }
    } else {
        (mention, None, None)
    };

    AtFileLines {
        filename: path_part.to_string(),
        line_start,
        line_end,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtFileLines {
    pub filename: String,
    pub line_start: Option<usize>,
    pub line_end: Option<usize>,
}

/// Lit les `@fichier` et construit le prompt agent avec annexes markdown.
#[must_use]
pub fn expand_at_refs(
    input: &str,
    working_roots: &[Utf8PathBuf],
    ignore: &DroxIgnoreMatcher,
) -> AtRefExpansion {
    let refs = extract_at_file_refs(input);
    if refs.is_empty() {
        return AtRefExpansion {
            agent_prompt: input.to_string(),
            notes: Vec::new(),
        };
    }

    let mut notes = Vec::new();
    let mut annexes = Vec::new();

    for mention in refs {
        let parsed = parse_at_file_lines(&mention);
        let Some(abs) = resolve_in_working_roots(working_roots, &parsed.filename) else {
            notes.push(format!("@{} : chemin introuvable", parsed.filename));
            continue;
        };

        if ignore.is_ignored(abs.as_std_path()) {
            notes.push(format!(
                "@{} : lecture interdite (.droxignore)",
                parsed.filename
            ));
            continue;
        }

        let display = relative_display_path(working_roots, &abs);
        if abs.is_dir() {
            match read_directory_listing(&abs) {
                Ok(body) => {
                    annexes.push(format_directory_annex(&display, &body));
                }
                Err(e) => {
                    notes.push(format!("@{} : impossible de lister — {e}", parsed.filename));
                }
            }
            continue;
        }

        match read_file_excerpt(&abs, parsed.line_start, parsed.line_end) {
            Ok(body) => {
                annexes.push(format_file_annex(&display, parsed.line_start, parsed.line_end, &body));
            }
            Err(e) => {
                notes.push(format!("@{} : lecture impossible — {e}", parsed.filename));
            }
        }
    }

    let agent_prompt = if annexes.is_empty() {
        input.to_string()
    } else {
        format!(
            "{input}\n\n---\n**Contenu référencé via @ :**\n\n{}",
            annexes.join("\n\n")
        )
    };

    AtRefExpansion {
        agent_prompt,
        notes,
    }
}

/// Mentions à ignorer (MCP, agents).
fn is_skipped_mention(raw: &str) -> bool {
    if raw.starts_with("agent-") {
        return true;
    }
    // Ressource MCP `server:uri` — un seul `:` sans `/` dans le segment serveur.
    if let Some((server, _uri)) = raw.split_once(':') {
        if !server.is_empty() && !server.contains('/') && !server.contains('\\') {
            return true;
        }
    }
    false
}

/// Résout un chemin utilisateur contre chaque racine de travail (workspace + `/add-dir`).
fn resolve_in_working_roots(
    roots: &[Utf8PathBuf],
    user_path: &str,
) -> Option<Utf8PathBuf> {
    let trimmed = user_path.trim();
    if trimmed.is_empty() {
        return None;
    }
    for root in roots {
        let joined = if std::path::Path::new(trimmed).is_absolute() {
            Utf8PathBuf::from(trimmed)
        } else {
            root.join(trimmed)
        };
        let Ok(root_canon) = fs::canonicalize(root.as_std_path()) else {
            continue;
        };
        let Ok(target_canon) = fs::canonicalize(joined.as_std_path()) else {
            continue;
        };
        if target_canon.strip_prefix(&root_canon).is_ok() {
            return Utf8PathBuf::from_path_buf(target_canon).ok();
        }
    }
    None
}

/// Chemin affichable (relatif à la racine workspace principale si possible).
fn relative_display_path(roots: &[Utf8PathBuf], abs: &Utf8Path) -> String {
    let primary = roots.first().map(|r| r.as_std_path());
    if let Some(root) = primary {
        if let Ok(canon_root) = fs::canonicalize(root) {
            if let Ok(canon_abs) = fs::canonicalize(abs.as_std_path()) {
                if let Ok(rel) = canon_abs.strip_prefix(&canon_root) {
                    return rel.to_string_lossy().replace('\\', "/");
                }
            }
        }
    }
    abs.as_str().replace('\\', "/")
}

fn read_directory_listing(dir: &Utf8Path) -> Result<String, String> {
    let entries = fs::read_dir(dir.as_std_path()).map_err(|e| e.to_string())?;
    let mut names = Vec::new();
    for entry in entries.flatten() {
        if names.len() >= MAX_DIR_ENTRIES {
            names.push(format!("… et plus de {MAX_DIR_ENTRIES} entrées"));
            break;
        }
        names.push(entry.file_name().to_string_lossy().into_owned());
    }
    names.sort();
    Ok(names.join("\n"))
}

fn read_file_excerpt(
    path: &Utf8Path,
    line_start: Option<usize>,
    line_end: Option<usize>,
) -> Result<String, String> {
    let bytes = fs::read(path.as_std_path()).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_FILE_BYTES {
        return Err(format!(
            "fichier trop volumineux (>{MAX_FILE_BYTES} o) — utilisez file_read"
        ));
    }
    let text = String::from_utf8(bytes).map_err(|_| "fichier binaire ou non UTF-8".to_string())?;

    let Some(start) = line_start else {
        return Ok(text);
    };

    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return Ok(String::new());
    }
    let start_idx = start.saturating_sub(1).min(lines.len().saturating_sub(1));
    let end_idx = line_end
        .unwrap_or(start)
        .max(start)
        .min(lines.len());
    Ok(lines[start_idx..end_idx].join("\n"))
}

fn format_file_annex(
    display_path: &str,
    line_start: Option<usize>,
    line_end: Option<usize>,
    body: &str,
) -> String {
    let range = match (line_start, line_end) {
        (Some(a), Some(b)) if a != b => format!(" (lignes {a}-{b})"),
        (Some(a), _) => format!(" (ligne {a})"),
        _ => String::new(),
    };
    format!(
        "### `{display_path}`{range}\n```\n{body}\n```"
    )
}

fn format_directory_annex(display_path: &str, listing: &str) -> String {
    format!(
        "### répertoire `{display_path}/`\n```\n{listing}\n```"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn extracts_quoted_and_plain_refs() {
        let s = r#"voir @"src/a b.rs" et @lib.rs#L10-12"#;
        let refs = extract_at_file_refs(s);
        assert!(refs.contains(&"src/a b.rs".to_string()));
        assert!(refs.iter().any(|r| r.starts_with("lib.rs")));
    }

    #[test]
    fn skips_mcp_and_agent() {
        let s = "@mcp:tool/path @agent-code @user@host.com ok @src/x.rs";
        let refs = extract_at_file_refs(s);
        assert_eq!(refs, vec!["src/x.rs".to_string()]);
    }

    #[test]
    fn parses_line_range() {
        let p = parse_at_file_lines("foo.rs#L10-20");
        assert_eq!(p.filename, "foo.rs");
        assert_eq!(p.line_start, Some(10));
        assert_eq!(p.line_end, Some(20));
    }

    #[tokio::test]
    async fn expands_file_content() {
        let dir = tempdir().unwrap();
        let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        std::fs::write(root.join("hello.txt"), "bonjour\nmonde").unwrap();
        let ignore = drox_session::DroxIgnoreMatcher::load_or_create(root.clone())
            .await
            .unwrap();
        let exp = expand_at_refs("explique @hello.txt", &[root], &ignore);
        assert!(exp.agent_prompt.contains("bonjour"));
        assert!(exp.agent_prompt.contains("explique @hello.txt"));
    }
}
