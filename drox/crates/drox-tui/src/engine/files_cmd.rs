//! `/files` — fichiers touchés dans le fil (tools).

use std::collections::BTreeSet;

use serde_json::Value;

use crate::view::LogEntry;

use super::EngineRuntime;

impl EngineRuntime {
    /// Liste les chemins fichier vus dans les `ToolStart` du fil UI.
    #[must_use]
    pub fn format_context_files_lines(&self, entries: &[LogEntry]) -> Vec<String> {
        let paths = collect_context_files(entries);
        let mut lines = vec!["Fichiers dans le contexte (fil TUI)".into()];
        if paths.is_empty() {
            lines.push("  (aucun — file_read / file_edit / grep / glob…)".into());
            return lines;
        }
        for p in &paths {
            lines.push(format!("  {p}"));
        }
        lines.push(format!("Total : {} chemin(s)", paths.len()));
        lines
    }
}

fn collect_context_files(entries: &[LogEntry]) -> Vec<String> {
    let mut set = BTreeSet::new();
    for entry in entries {
        let LogEntry::ToolStart { name, arguments, .. } = entry else {
            continue;
        };
        match name.as_str() {
            "file_read" | "file_write" | "file_edit" | "notebook_edit" | "copy_path"
            | "delete_path" => {
                push_path_arg(arguments, &["path", "file_path", "target", "source"], &mut set);
            }
            "grep" | "glob" => {
                push_path_arg(arguments, &["path"], &mut set);
            }
            _ => {}
        }
    }
    set.into_iter().collect()
}

fn push_path_arg(args: &Value, keys: &[&str], set: &mut BTreeSet<String>) {
    for key in keys {
        if let Some(p) = args.get(*key).and_then(Value::as_str) {
            let t = p.trim();
            if !t.is_empty() {
                set.insert(t.to_string());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use drox_types::ToolUseId;

    #[test]
    fn collects_file_read_paths() {
        let entries = vec![LogEntry::ToolStart {
            id: ToolUseId::new(),
            name: "file_read".into(),
            arguments: serde_json::json!({ "path": "src/main.rs" }),
        }];
        let paths = collect_context_files(&entries);
        assert_eq!(paths, vec!["src/main.rs"]);
    }
}
