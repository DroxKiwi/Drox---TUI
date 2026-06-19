//! Découpe une ligne Bash en segments pour permissions (listes, pipelines,
//! substitutions de commande).
//!
//! On combine (1) les unités `command` directement sous des `list` /
//! `pipeline` au premier niveau du `program` et (2) les `simple_command`
//! imbriqués dans des `command_substitution` (`$(…)`), dédoublonnés par
//! `(start_byte, texte)`.

use std::collections::BTreeSet;

use tree_sitter::Node;

use crate::error::{BashError, MAX_SUBCOMMANDS};
use crate::parse::parse_bash;

/// Découpe `source` en segments analysables (sous-commandes).
///
/// - Parse tree-sitter ; si l'arbre porte des erreurs → un seul segment = source trim.
/// - Si plus de [`MAX_SUBCOMMANDS`] segments → [`BashError::TooManySubcommands`].
pub fn split_command_segments(source: &str) -> Result<Vec<String>, BashError> {
    let trimmed = source.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    let tree = parse_bash(trimmed)?;
    let root = tree.root_node();
    if root.has_error() {
        return Ok(vec![trimmed.to_string()]);
    }

    let bytes = trimmed.as_bytes();
    let mut spans: Vec<(usize, String)> = Vec::new();
    let mut seen: BTreeSet<(usize, String)> = BTreeSet::new();

    for i in 0..root.named_child_count() {
        let Some(ch) = root.named_child(i) else {
            continue;
        };
        match ch.kind() {
            "list" => collect_from_list(ch, bytes, &mut spans, &mut seen)?,
            "pipeline" => collect_from_pipeline(ch, bytes, &mut spans, &mut seen)?,
            "command" => push_command_span(ch, bytes, &mut spans, &mut seen)?,
            "subshell" | "compound_statement" | "redirected_statement" => {
                collect_nested_top_level(ch, bytes, &mut spans, &mut seen)?;
            }
            _ => {}
        }
    }

    collect_substitution_simple_commands(root, bytes, &mut spans, &mut seen)?;

    if spans.is_empty() {
        spans.push((0, trimmed.to_string()));
    }

    spans.sort_by_key(|(start, _)| *start);
    let out: Vec<String> = spans.into_iter().map(|(_, t)| t).collect();

    if out.len() > MAX_SUBCOMMANDS {
        return Err(BashError::TooManySubcommands(out.len()));
    }

    Ok(out)
}

fn push_span(
    start: usize,
    text: String,
    spans: &mut Vec<(usize, String)>,
    seen: &mut BTreeSet<(usize, String)>,
) {
    let t = text.trim();
    if t.is_empty() || t.starts_with('#') {
        return;
    }
    let key = (start, t.to_string());
    if seen.insert(key.clone()) {
        spans.push((start, key.1));
    }
}

fn push_command_span(
    cmd: Node<'_>,
    src: &[u8],
    spans: &mut Vec<(usize, String)>,
    seen: &mut BTreeSet<(usize, String)>,
) -> Result<(), BashError> {
    let t = cmd.utf8_text(src)?.to_string();
    push_span(cmd.start_byte(), t, spans, seen);
    Ok(())
}

fn collect_from_list(
    list: Node<'_>,
    src: &[u8],
    spans: &mut Vec<(usize, String)>,
    seen: &mut BTreeSet<(usize, String)>,
) -> Result<(), BashError> {
    let mut cursor = list.walk();
    for ch in list.children(&mut cursor) {
        match ch.kind() {
            "list" => collect_from_list(ch, src, spans, seen)?,
            "pipeline" => collect_from_pipeline(ch, src, spans, seen)?,
            "command" => push_command_span(ch, src, spans, seen)?,
            _ => {}
        }
    }
    Ok(())
}

fn collect_from_pipeline(
    pipe: Node<'_>,
    src: &[u8],
    spans: &mut Vec<(usize, String)>,
    seen: &mut BTreeSet<(usize, String)>,
) -> Result<(), BashError> {
    let mut cursor = pipe.walk();
    for ch in pipe.children(&mut cursor) {
        if ch.kind() == "command" {
            push_command_span(ch, src, spans, seen)?;
        }
    }
    Ok(())
}

/// Reprend des constructions `subshell`, `compound_statement`, etc.
fn collect_nested_top_level(
    node: Node<'_>,
    src: &[u8],
    spans: &mut Vec<(usize, String)>,
    seen: &mut BTreeSet<(usize, String)>,
) -> Result<(), BashError> {
    let mut cursor = node.walk();
    for ch in node.children(&mut cursor) {
        match ch.kind() {
            "list" => collect_from_list(ch, src, spans, seen)?,
            "pipeline" => collect_from_pipeline(ch, src, spans, seen)?,
            "command" => push_command_span(ch, src, spans, seen)?,
            _ => {}
        }
    }
    Ok(())
}

fn collect_substitution_simple_commands(
    node: Node<'_>,
    src: &[u8],
    spans: &mut Vec<(usize, String)>,
    seen: &mut BTreeSet<(usize, String)>,
) -> Result<(), BashError> {
    if node.kind() == "command_substitution" {
        let mut cursor = node.walk();
        for ch in node.children(&mut cursor) {
            if ch.kind() == "simple_command" {
                let t = ch.utf8_text(src)?.to_string();
                push_span(ch.start_byte(), t, spans, seen);
            }
        }
    }
    let mut cursor = node.walk();
    for ch in node.children(&mut cursor) {
        collect_substitution_simple_commands(ch, src, spans, seen)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_double_ampersand() {
        let segs = split_command_segments("echo a && echo b").unwrap();
        assert_eq!(segs, vec!["echo a", "echo b"]);
    }

    #[test]
    fn splits_pipeline() {
        let segs = split_command_segments("echo a | grep b").unwrap();
        assert_eq!(segs, vec!["echo a", "grep b"]);
    }

    #[test]
    fn command_substitution_yields_inner_segment() {
        let segs = split_command_segments("echo $(ls -la)").unwrap();
        assert!(segs.iter().any(|s| s.contains("echo")));
        assert!(segs.iter().any(|s| s.contains("ls -la")));
    }

    #[test]
    fn malformed_falls_back_single_segment() {
        let bad = "echo ((( unclosed";
        let segs = split_command_segments(bad).unwrap();
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0], bad);
    }

    #[test]
    fn too_many_simple_commands_errors() {
        let mut parts = Vec::new();
        for i in 0..=MAX_SUBCOMMANDS {
            parts.push(format!("echo x{i}"));
        }
        let cmd = parts.join(" && ");
        assert!(matches!(
            split_command_segments(&cmd),
            Err(BashError::TooManySubcommands(_))
        ));
    }
}
