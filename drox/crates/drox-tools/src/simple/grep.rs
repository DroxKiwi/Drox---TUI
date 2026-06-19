//! Tool `grep` — recherche regex dans les fichiers texte sous le workspace.

use async_trait::async_trait;
use ignore::WalkBuilder;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::context::ToolContext;
use crate::error::ToolError;
use crate::path_util::resolve_under_workspace;
use crate::tool::Tool;

const MAX_MATCHES: usize = 300;
const MAX_FILE_BYTES: u64 = 512 * 1024;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GrepInput {
    /// Expression régulière Rust (`regex` crate).
    pub pattern: String,
    /// Répertoire ou fichier relatif au workspace (défaut : racine workspace).
    #[serde(default)]
    pub path: Option<String>,
    /// Filtre `glob` sur le chemin **relatif au workspace** (ex. `**/*.rs`, `*.tsx`).
    /// Slash `/` accepté même sous Windows.
    #[serde(default)]
    pub glob: Option<String>,
}

pub struct GrepTool;

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }

    fn description(&self) -> &str {
        "Recherche une regex dans les fichiers du workspace (respecte `.gitignore` et **`.droxignore`**, limite 300 occurrences). \
         Lecture UTF-8 **permissive** (fichiers partiellement binaires ignorés sans faire échouer toute la recherche). \
         Option `glob` : limite aux chemins relatifs qui matchent (ex. `**/*.ts`)."
    }

    fn input_schema(&self) -> Value {
        serde_json::to_value(schemars::schema_for!(GrepInput)).unwrap_or(Value::Null)
    }

    fn is_read_only(&self) -> bool {
        true
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<Value, ToolError> {
        let args: GrepInput = serde_json::from_value(input)?;
        let re = regex::Regex::new(&args.pattern)?;

        let ws = ctx.effective_workspace();
        let base = match &args.path {
            None => ws.clone(),
            Some(p) => resolve_under_workspace(&ws, p)?,
        };

        if let Some(ref ignore) = ctx.drox_ignore {
            if ignore.is_ignored(base.as_std_path()) {
                return Err(ToolError::drox_ignore(base));
            }
        }

        let glob_pat = match &args.glob {
            None => None,
            Some(g) => {
                let trimmed = g.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(
                        glob::Pattern::new(trimmed).map_err(|e| {
                            ToolError::invalid_args(format!("grep: `glob` invalide ({e})"))
                        })?,
                    )
                }
            }
        };

        let mut matches = Vec::new();
        let mut truncated = false;

        let mut binding = WalkBuilder::new(base.as_std_path());
        let mut walker = binding.hidden(false);
        if let Some(ref ignore) = ctx.drox_ignore {
            ignore.configure_walk(&mut walker);
        } else {
            walker.git_ignore(true);
        }
        let walker = walker.build();

        'outer: for result in walker {
            let Ok(entry) = result else { continue };
            if matches.len() >= MAX_MATCHES {
                truncated = true;
                break;
            }
            let path = entry.path();
            let Ok(meta) = tokio::fs::metadata(path).await else {
                continue;
            };
            if !meta.is_file() || meta.len() > MAX_FILE_BYTES {
                continue;
            }

            let Ok(utf8_path) = camino::Utf8PathBuf::from_path_buf(path.to_path_buf()) else {
                continue;
            };

            if let Some(ref pat) = glob_pat {
                let rel_std = match path.strip_prefix(ws.as_std_path()) {
                    Ok(r) => r,
                    Err(_) => path,
                };
                let rel_norm = rel_std.to_string_lossy().replace('\\', "/");
                if !pat.matches(rel_norm.as_ref()) {
                    continue;
                }
            }

            let Ok(bytes) = tokio::fs::read(path).await else {
                continue;
            };
            if bytes.len() > MAX_FILE_BYTES as usize {
                continue;
            }
            let text = String::from_utf8_lossy(&bytes);
            for (idx, line) in text.lines().enumerate() {
                let line_no = (idx + 1) as u64;
                if re.is_match(line) {
                    matches.push(json!({
                        "path": utf8_path.as_str(),
                        "line_number": line_no,
                        "line": line,
                    }));
                    if matches.len() >= MAX_MATCHES {
                        truncated = true;
                        break 'outer;
                    }
                }
            }
        }

        Ok(json!({ "matches": matches, "truncated": truncated }))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::registry::ToolRegistry;

    #[tokio::test]
    async fn finds_pattern() {
        let tmp = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        tokio::fs::write(root.join("a.rs"), "fn main() {\n    let x = 1;\n}\n")
            .await
            .unwrap();
        let ctx = ToolContext::new(root.clone(), false);
        let reg = ToolRegistry::with_simple_tools();
        let out = reg
            .execute_named("grep", &ctx, json!({ "pattern": "let x", "path": "." }))
            .await
            .unwrap();
        let arr = out["matches"].as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert!(arr[0]["line"].as_str().unwrap().contains("let x"));
    }

    #[tokio::test]
    async fn glob_filters_extensions() {
        let tmp = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        tokio::fs::write(root.join("a.rs"), "fn foo() {}\n")
            .await
            .unwrap();
        tokio::fs::write(root.join("b.txt"), "fn foo() {}\n")
            .await
            .unwrap();
        let ctx = ToolContext::new(root.clone(), false);
        let reg = ToolRegistry::with_simple_tools();
        let out = reg
            .execute_named(
                "grep",
                &ctx,
                json!({ "pattern": "foo", "path": ".", "glob": "*.rs" }),
            )
            .await
            .unwrap();
        let arr = out["matches"].as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert!(arr[0]["path"].as_str().unwrap().ends_with("a.rs"));
    }

    #[tokio::test]
    async fn skips_invalid_utf8_without_failing() {
        let tmp = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        let mut bad = b"good line\n".to_vec();
        bad.extend_from_slice(&[0xff, 0xfe, 0xfd]);
        bad.extend_from_slice(b"\nanother good\n");
        tokio::fs::write(root.join("mix.txt"), &bad).await.unwrap();
        let ctx = ToolContext::new(root.clone(), false);
        let reg = ToolRegistry::with_simple_tools();
        let out = reg
            .execute_named("grep", &ctx, json!({ "pattern": "good", "path": "." }))
            .await
            .unwrap();
        let arr = out["matches"].as_array().unwrap();
        assert!(arr.len() >= 2, "matches={arr:?}");
    }
}
