//! Formatage fil pour `file_read`, `grep`, `glob`, `web_fetch`, `web_search`.

use drox_types::ToolUseId;
use serde_json::Value;

use super::lines_viewer::LinesViewerState;

const MAX_BODY_LINES: usize = 24;
const MAX_BODY_LINES_EXPANDED: usize = 120;
const MAX_GREP_MATCHES: usize = 25;
const MAX_GREP_MATCHES_EXPANDED: usize = 80;
const MAX_GLOB_ENTRIES: usize = 30;
const MAX_WEB_TEXT_LINES: usize = 20;
const MAX_WEB_TEXT_LINES_EXPANDED: usize = 100;
const MAX_SEARCH_RESULTS: usize = 8;
const LINE_MAX: usize = 120;
const LINE_MAX_EXPANDED: usize = 200;
const SNIPPET_MAX: usize = 100;

fn line_limit(expanded: bool, normal: usize, expanded_max: usize) -> usize {
    if expanded { expanded_max } else { normal }
}

fn char_limit(expanded: bool) -> usize {
    if expanded { LINE_MAX_EXPANDED } else { LINE_MAX }
}

pub fn format_file_read_finish(
    id: &ToolUseId,
    output: &Value,
    is_error: bool,
    expanded: bool,
) -> Vec<String> {
    if is_error {
        return format_error("file_read", id, output);
    }
    let path = output.get("path").and_then(Value::as_str).unwrap_or("?");
    let truncated = output.get("truncated").and_then(Value::as_bool).unwrap_or(false);
    let size = output.get("size_bytes").and_then(Value::as_u64).unwrap_or(0);
    let mut lines = vec![format!(
        "◂ file_read ({id}) {path} ({size} o{})",
        if truncated { ", tronqué" } else { "" }
    )];
    if let Some(content) = output.get("content").and_then(Value::as_str) {
        let start = output.get("start_line").and_then(Value::as_u64).unwrap_or(1);
        let max_lines = line_limit(expanded, MAX_BODY_LINES, MAX_BODY_LINES_EXPANDED);
        let line_max = char_limit(expanded);
        for (i, line) in content.lines().take(max_lines).enumerate() {
            let n = start + i as u64;
            lines.push(format!("    L{n:>4}| {}", truncate(line, line_max)));
        }
        let total = content.lines().count();
        if total > max_lines {
            lines.push(format!("    … +{} lignes (e pour parcourir)", total - max_lines));
        }
    }
    lines
}

pub fn format_grep_finish(
    id: &ToolUseId,
    output: &Value,
    is_error: bool,
    expanded: bool,
) -> Vec<String> {
    if is_error {
        return format_error("grep", id, output);
    }
    let matches = output
        .get("matches")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let truncated = output.get("truncated").and_then(Value::as_bool).unwrap_or(false);
    let mut lines = vec![format!(
        "◂ grep ({id}) {} correspondance(s){}",
        matches.len(),
        if truncated { " (tronqué)" } else { "" }
    )];
    let max_matches = line_limit(expanded, MAX_GREP_MATCHES, MAX_GREP_MATCHES_EXPANDED);
    let line_max = char_limit(expanded);
    for m in matches.iter().take(max_matches) {
        let path = m.get("path").and_then(Value::as_str).unwrap_or("?");
        let no = m.get("line_number").and_then(Value::as_u64).unwrap_or(0);
        let line = m.get("line").and_then(Value::as_str).unwrap_or("");
        lines.push(format!(
            "    {path}:{no}: {}",
            truncate(line, line_max)
        ));
    }
    if matches.len() > max_matches {
        lines.push(format!(
            "    … +{} correspondances (e pour parcourir)",
            matches.len() - max_matches
        ));
    }
    lines
}

pub fn format_glob_finish(id: &ToolUseId, output: &Value, is_error: bool) -> Vec<String> {
    if is_error {
        return format_error("glob", id, output);
    }
    let files = output
        .get("files")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let dirs = output
        .get("directories")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let truncated = output.get("truncated").and_then(Value::as_bool).unwrap_or(false);
    let mut lines = vec![format!(
        "◂ glob ({id}) {} fichier(s), {} dossier(s){}",
        files.len(),
        dirs.len(),
        if truncated { " (tronqué)" } else { "" }
    )];
    let mut shown = 0usize;
    for f in files {
        if shown >= MAX_GLOB_ENTRIES {
            break;
        }
        if let Some(p) = f.as_str() {
            lines.push(format!("    f {p}"));
            shown += 1;
        }
    }
    for d in dirs {
        if shown >= MAX_GLOB_ENTRIES {
            break;
        }
        if let Some(p) = d.as_str() {
            lines.push(format!("    d {p}"));
            shown += 1;
        }
    }
    let total = files.len() + dirs.len();
    if total > shown {
        lines.push(format!(
            "    … +{} entrées (e pour parcourir)",
            total - shown
        ));
    }
    lines
}

pub fn format_web_fetch_finish(
    id: &ToolUseId,
    output: &Value,
    is_error: bool,
    expanded: bool,
) -> Vec<String> {
    if is_error {
        return format_error("web_fetch", id, output);
    }
    let url = output.get("url").and_then(Value::as_str).unwrap_or("?");
    let status = output.get("status").and_then(Value::as_u64).unwrap_or(0);
    let kind = output.get("kind").and_then(Value::as_str).unwrap_or("?");
    let bytes = output.get("bytes").and_then(Value::as_u64).unwrap_or(0);
    let truncated_dl = output
        .get("truncated_download")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let truncated_txt = output
        .get("truncated_text")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let content_type = output
        .get("content_type")
        .and_then(Value::as_str)
        .unwrap_or("?");
    let mut flags = Vec::new();
    if truncated_dl {
        flags.push("dl tronqué");
    }
    if truncated_txt {
        flags.push("texte tronqué");
    }
    let flag_suffix = if flags.is_empty() {
        String::new()
    } else {
        format!(", {}", flags.join(", "))
    };
    let mut lines = vec![format!(
        "◂ web_fetch ({id}) HTTP {status} {url} [{kind}, {bytes} o{flag_suffix}]"
    )];
    lines.push(format!("    type: {content_type}"));
    if let Some(text) = output.get("text").and_then(Value::as_str) {
        let max_lines = line_limit(expanded, MAX_WEB_TEXT_LINES, MAX_WEB_TEXT_LINES_EXPANDED);
        let line_max = char_limit(expanded);
        let (body, total) = split_display_lines(text, line_max, max_lines);
        for line in body {
            if line.is_empty() {
                lines.push("    ".to_string());
            } else {
                lines.push(format!("    {line}"));
            }
        }
        if total > max_lines {
            lines.push(format!("    … +{} lignes (e pour parcourir)", total - max_lines));
        }
    }
    lines
}

pub fn format_web_search_finish(id: &ToolUseId, output: &Value, is_error: bool) -> Vec<String> {
    if is_error {
        return format_error("web_search", id, output);
    }
    let query = output.get("query").and_then(Value::as_str).unwrap_or("?");
    let provider = output
        .get("provider")
        .and_then(Value::as_str)
        .unwrap_or("?");
    let count = output
        .get("result_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let truncated = output.get("truncated").and_then(Value::as_bool).unwrap_or(false);
    let mut lines = vec![format!(
        "◂ web_search ({id}) «{query}» — {count} résultat(s) [{provider}]{}",
        if truncated { " (tronqué)" } else { "" }
    )];
    let results = output
        .get("results")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    for (i, r) in results.iter().take(MAX_SEARCH_RESULTS).enumerate() {
        let title = r.get("title").and_then(Value::as_str).unwrap_or("?");
        let url = r.get("url").and_then(Value::as_str).unwrap_or("?");
        let snippet = r
            .get("snippet")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        lines.push(format!("    {}. {}", i + 1, truncate(title, LINE_MAX)));
        lines.push(format!("       {}", truncate(url, LINE_MAX)));
        if !snippet.is_empty() {
            lines.push(format!("       {}", truncate(snippet, SNIPPET_MAX)));
        }
    }
    if results.len() > MAX_SEARCH_RESULTS {
        lines.push(format!(
            "    … +{} résultats (e pour parcourir)",
            results.len() - MAX_SEARCH_RESULTS
        ));
    }
    lines
}

fn split_display_lines(text: &str, line_max: usize, max_lines: usize) -> (Vec<String>, usize) {
    let mut lines = Vec::new();
    let mut total = 0usize;
    for para in text.lines() {
        let mut rest = para;
        loop {
            if rest.is_empty() {
                break;
            }
            let (chunk, next) = take_chars_prefix(rest, line_max);
            total += 1;
            if lines.len() < max_lines {
                lines.push(chunk);
            }
            rest = next;
        }
    }
    (lines, total)
}

fn take_chars_prefix(s: &str, max: usize) -> (String, &str) {
    if s.chars().count() <= max {
        return (s.to_string(), "");
    }
    let mut count = 0usize;
    let mut end = s.len();
    for (i, _) in s.char_indices() {
        if count == max {
            end = i;
            break;
        }
        count += 1;
    }
    (s[..end].to_string(), &s[end..])
}

pub(crate) fn format_error(name: &str, id: &ToolUseId, output: &Value) -> Vec<String> {
    let body = serde_json::to_string(output).unwrap_or_else(|_| "{}".into());
    vec![format!(
        "◂ {name} ({id}) [erreur] {}",
        truncate(&body, 200)
    )]
}

pub(crate) fn push_wrapped_body(lines: &mut Vec<String>, text: &str, max_lines: usize) {
    push_wrapped_body_inner(lines, text, max_lines, false);
}

pub(crate) fn push_wrapped_body_expandable(lines: &mut Vec<String>, text: &str, max_lines: usize) {
    push_wrapped_body_inner(lines, text, max_lines, true);
}

fn push_wrapped_body_inner(
    lines: &mut Vec<String>,
    text: &str,
    max_lines: usize,
    expandable: bool,
) {
    let (body, total) = split_display_lines(text, LINE_MAX, max_lines);
    for line in body {
        if line.is_empty() {
            lines.push("    ".to_string());
        } else {
            lines.push(format!("    {line}"));
        }
    }
    if total > max_lines {
        let hint = if expandable {
            " (e pour parcourir)"
        } else {
            ""
        };
        lines.push(format!("    … +{} lignes{hint}", total - max_lines));
    }
}

pub fn format_file_edit_finish(id: &ToolUseId, output: &Value, is_error: bool) -> Vec<String> {
    if is_error {
        return format_error("file_edit", id, output);
    }
    let path = output.get("path").and_then(Value::as_str).unwrap_or("?");
    let applied = output.get("applied").and_then(Value::as_bool).unwrap_or(false);
    let proposed = output.get("proposed").and_then(Value::as_bool).unwrap_or(false);
    let tag = file_edit_tag(applied, proposed);
    let edits = output
        .get("edits_applied")
        .and_then(Value::as_u64)
        .map(|n| format!(", {n} edit(s)"))
        .unwrap_or_default();
    let mut lines = vec![format!("◂ file_edit ({id}) {path} [{tag}{edits}]")];
    if let Some(diff) = output.get("diff").and_then(Value::as_str) {
        append_unified_diff_block(&mut lines, diff);
    }
    lines
}

pub fn format_file_write_finish(id: &ToolUseId, output: &Value, is_error: bool) -> Vec<String> {
    if is_error {
        return format_error("file_write", id, output);
    }
    let path = output.get("path").and_then(Value::as_str).unwrap_or("?");
    let applied = output.get("applied").and_then(Value::as_bool).unwrap_or(false);
    let proposed = output.get("proposed").and_then(Value::as_bool).unwrap_or(false);
    let tag = file_edit_tag(applied, proposed);
    let mut lines = vec![format!("◂ file_write ({id}) {path} [{tag}]")];
    if applied {
        if let Some(bytes) = output.get("bytes_written").and_then(Value::as_u64) {
            lines.push(format!("    {bytes} octet(s) écrits"));
        }
    } else if proposed {
        if let Some(content) = output.get("content").and_then(Value::as_str) {
            push_wrapped_body_expandable(&mut lines, content, 12);
        }
    }
    lines
}

pub fn format_delete_path_finish(id: &ToolUseId, output: &Value, is_error: bool) -> Vec<String> {
    if is_error {
        return format_error("delete_path", id, output);
    }
    let path = output.get("path").and_then(Value::as_str).unwrap_or("?");
    let kind = output.get("kind").and_then(Value::as_str).unwrap_or("?");
    let deleted = output.get("deleted").and_then(Value::as_bool).unwrap_or(false);
    let proposed = output.get("proposed").and_then(Value::as_bool).unwrap_or(false);
    let recursive = output
        .get("recursive")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let tag = if deleted {
        "supprimé"
    } else if proposed {
        "proposé"
    } else {
        "ok"
    };
    let rec = if recursive { " · récursif" } else { "" };
    vec![format!("◂ delete_path ({id}) {path} [{kind}{rec}] [{tag}]")]
}

pub fn format_copy_path_finish(id: &ToolUseId, output: &Value, is_error: bool) -> Vec<String> {
    if is_error {
        return format_error("copy_path", id, output);
    }
    let source = output.get("source").and_then(Value::as_str).unwrap_or("?");
    let dest = output
        .get("destination")
        .and_then(Value::as_str)
        .unwrap_or("?");
    let copied = output.get("copied").and_then(Value::as_bool).unwrap_or(false);
    let proposed = output.get("proposed").and_then(Value::as_bool).unwrap_or(false);
    let tag = if copied {
        "copié"
    } else if proposed {
        "proposé"
    } else {
        "ok"
    };
    let mut lines = vec![format!("◂ copy_path ({id}) {source} → {dest} [{tag}]")];
    if let Some(bytes) = output.get("bytes").and_then(Value::as_u64) {
        lines.push(format!("    {bytes} octet(s)"));
    }
    lines
}

fn file_edit_tag(applied: bool, proposed: bool) -> &'static str {
    if applied {
        "appliqué"
    } else if proposed {
        "proposé"
    } else {
        "ok"
    }
}

const MAX_DIFF_LINES: usize = 16;

#[must_use]
pub fn diff_is_expandable(output: &Value) -> bool {
    output
        .get("diff")
        .and_then(Value::as_str)
        .is_some_and(|diff| diff.lines().count() > MAX_DIFF_LINES)
}

#[must_use]
pub fn diff_viewer_from_output(
    tool: &str,
    id: ToolUseId,
    output: &Value,
) -> Option<LinesViewerState> {
    let diff = output.get("diff").and_then(Value::as_str)?;
    if diff.is_empty() {
        return None;
    }
    let path = output.get("path").and_then(Value::as_str).unwrap_or("?");
    let applied = output.get("applied").and_then(Value::as_bool).unwrap_or(false);
    let proposed = output.get("proposed").and_then(Value::as_bool).unwrap_or(false);
    let tag = file_edit_tag(applied, proposed);
    Some(LinesViewerState::from_diff(id, tool, path, diff, tag))
}

fn append_unified_diff_block(lines: &mut Vec<String>, diff: &str) {
    for line in diff.lines().take(MAX_DIFF_LINES) {
        lines.push(format!("    {line}"));
    }
    let total = diff.lines().count();
    if total > MAX_DIFF_LINES {
        lines.push(format!(
            "    … +{} lignes diff (e pour parcourir)",
            total - MAX_DIFF_LINES
        ));
    }
}

pub fn format_notebook_edit_finish(id: &ToolUseId, output: &Value, is_error: bool) -> Vec<String> {
    if is_error {
        return format_error("notebook_edit", id, output);
    }
    let path = output.get("path").and_then(Value::as_str).unwrap_or("?");
    let applied = output.get("applied").and_then(Value::as_bool).unwrap_or(false);
    let proposed = output.get("proposed").and_then(Value::as_bool).unwrap_or(false);
    let tag = file_edit_tag(applied, proposed);
    let edits = output
        .get("cell_edits_applied")
        .and_then(Value::as_u64)
        .map(|n| format!(", {n} cellule(s)"))
        .unwrap_or_default();
    let mut lines = vec![format!("◂ notebook_edit ({id}) {path} [{tag}{edits}]")];
    if let Some(diff) = output.get("diff").and_then(Value::as_str) {
        append_unified_diff_block(&mut lines, diff);
    }
    lines
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use drox_types::ToolUseId;

    #[test]
    fn file_read_shows_line_numbers() {
        let id = ToolUseId::new();
        let out = serde_json::json!({
            "path": "a.rs",
            "content": "line1\nline2",
            "size_bytes": 10,
            "truncated": false
        });
        let lines = format_file_read_finish(&id, &out, false, false);
        assert!(lines.iter().any(|l| l.contains("L   1|")));
    }

    #[test]
    fn file_read_expanded_shows_more_lines() {
        let id = ToolUseId::new();
        let content: String = (1..=50).map(|n| format!("line{n}")).collect::<Vec<_>>().join("\n");
        let out = serde_json::json!({
            "path": "big.txt",
            "content": content,
            "size_bytes": 500,
            "truncated": false
        });
        let compact = format_file_read_finish(&id, &out, false, false);
        let expanded = format_file_read_finish(&id, &out, false, true);
        assert!(expanded.len() > compact.len());
    }

    #[test]
    fn web_fetch_shows_url_and_body() {
        let id = ToolUseId::new();
        let out = serde_json::json!({
            "url": "https://example.com",
            "status": 200,
            "content_type": "text/html",
            "kind": "html",
            "bytes": 1024,
            "truncated_download": false,
            "truncated_text": false,
            "text": "Hello world"
        });
        let lines = format_web_fetch_finish(&id, &out, false, false);
        assert!(lines.iter().any(|l| l.contains("example.com")));
        assert!(lines.iter().any(|l| l.contains("Hello world")));
    }

    #[test]
    fn web_search_lists_results() {
        let id = ToolUseId::new();
        let out = serde_json::json!({
            "query": "rust book",
            "provider": "duckduckgo",
            "result_count": 1,
            "truncated": false,
            "results": [{
                "title": "The Rust Book",
                "url": "https://doc.rust-lang.org/book/",
                "snippet": "Learn Rust"
            }]
        });
        let lines = format_web_search_finish(&id, &out, false);
        assert!(lines.iter().any(|l| l.contains("rust book")));
        assert!(lines.iter().any(|l| l.contains("The Rust Book")));
        assert!(lines.iter().any(|l| l.contains("doc.rust-lang.org")));
    }

    #[test]
    fn split_display_lines_wraps_long_paragraph() {
        let text = "a".repeat(250);
        let (lines, total) = split_display_lines(&text, 100, 5);
        assert_eq!(lines.len(), 3);
        assert!(total >= 3);
    }

    #[test]
    fn delete_path_shows_kind_and_tag() {
        let id = ToolUseId::new();
        let out = serde_json::json!({
            "deleted": true,
            "path": "old.txt",
            "kind": "file",
            "recursive": false
        });
        let lines = format_delete_path_finish(&id, &out, false);
        assert!(lines.iter().any(|l| l.contains("delete_path")));
        assert!(lines.iter().any(|l| l.contains("supprimé")));
    }

    #[test]
    fn copy_path_shows_source_dest() {
        let id = ToolUseId::new();
        let out = serde_json::json!({
            "copied": true,
            "source": "a.txt",
            "destination": "b.txt",
            "bytes": 42
        });
        let lines = format_copy_path_finish(&id, &out, false);
        assert!(lines.iter().any(|l| l.contains("a.txt → b.txt")));
        assert!(lines.iter().any(|l| l.contains("42 octet")));
    }

    #[test]
    fn file_edit_shows_diff() {
        let id = ToolUseId::new();
        let out = serde_json::json!({
            "applied": true,
            "path": "a.rs",
            "edits_applied": 1,
            "diff": "--- a.rs\n+++ a.rs\n@@ -1 +1 @@\n-old\n+new"
        });
        let lines = format_file_edit_finish(&id, &out, false);
        assert!(lines.iter().any(|l| l.contains("file_edit")));
        assert!(lines.iter().any(|l| l.contains("-old")));
        assert!(lines.iter().any(|l| l.contains("+new")));
    }

    #[test]
    fn file_write_proposed_shows_content() {
        let id = ToolUseId::new();
        let out = serde_json::json!({
            "proposed": true,
            "path": "new.txt",
            "content": "hello\nworld"
        });
        let lines = format_file_write_finish(&id, &out, false);
        assert!(lines.iter().any(|l| l.contains("proposé")));
        assert!(lines.iter().any(|l| l.contains("hello")));
    }

    #[test]
    fn notebook_edit_shows_diff() {
        let id = ToolUseId::new();
        let out = serde_json::json!({
            "applied": true,
            "path": "nb.ipynb",
            "cell_edits_applied": 1,
            "diff": "@@ -1 +1 @@\n-old\n+new"
        });
        let lines = format_notebook_edit_finish(&id, &out, false);
        assert!(lines.iter().any(|l| l.contains("nb.ipynb")));
        assert!(lines.iter().any(|l| l.contains("+new")));
    }
}
