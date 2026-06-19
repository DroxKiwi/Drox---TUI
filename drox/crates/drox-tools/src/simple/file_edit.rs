//! Tool `file_edit` — édition structurée par couples (`old_string`, `new_string`).
//!
//! Mode `apply` : applique et écrit sur disque.
//! Mode `propose` : retourne le nouveau contenu + diff unifié.
//!
//! Contraintes appliquées (alignées sur l'outil de l'agent TS d'origine) :
//! - `old_string` doit apparaître **exactement une fois** sauf si
//!   `replace_all = true`.
//! - `old_string` et `new_string` doivent différer.
//! - Le fichier doit exister.

use async_trait::async_trait;
use camino::{Utf8Path, Utf8PathBuf};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::fs;

use crate::context::ToolContext;
use crate::error::ToolError;
use crate::path_util::resolve_under_workspace;
use crate::tool::Tool;

const MAX_FILE_SIZE: u64 = 512 * 1024;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FileEditInput {
    /// Chemin (relatif au workspace) du fichier à modifier.
    #[serde(alias = "file_path")]
    pub path: String,
    /// Liste séquentielle d'éditions ; chaque édition s'applique au texte
    /// résultant de la précédente.
    pub edits: Vec<EditOp>,
}

#[derive(Debug, Deserialize, JsonSchema, Clone)]
pub struct EditOp {
    /// Texte exact à remplacer (doit être unique sauf si `replace_all`).
    #[serde(alias = "old", alias = "oldString")]
    pub old_string: String,
    /// Texte de remplacement.
    #[serde(alias = "new", alias = "newString")]
    pub new_string: String,
    /// Si `true`, remplace toutes les occurrences sans contrainte d'unicité.
    #[serde(default)]
    pub replace_all: bool,
}

pub struct FileEditTool;

/// Produit un diff unifié pour une proposition `file_edit` (lecture disque sync).
pub fn preview_file_edit_diff(workspace: &Utf8Path, input: &Value) -> Result<String, ToolError> {
    let args: FileEditInput = serde_json::from_value(input.clone())?;
    if args.edits.is_empty() {
        return Err(ToolError::invalid_args("edits must not be empty"));
    }

    let resolved = resolve_under_workspace(workspace, &args.path)?;
    let meta = std::fs::metadata(&resolved).map_err(|e| ToolError::io(resolved.clone(), e))?;
    if !meta.is_file() {
        return Err(ToolError::invalid_args(format!(
            "not a regular file: {resolved}"
        )));
    }
    if meta.len() > MAX_FILE_SIZE {
        return Err(ToolError::edit_failed(format!(
            "file too large ({} bytes > {} bytes limit)",
            meta.len(),
            MAX_FILE_SIZE
        )));
    }

    let bytes = std::fs::read(&resolved).map_err(|e| ToolError::io(resolved.clone(), e))?;
    let original = String::from_utf8_lossy(&bytes).into_owned();
    let updated = apply_edits(&original, &args.edits)?;
    if updated == original {
        return Err(ToolError::edit_failed("edits produced no change"));
    }
    Ok(unified_diff(&resolved, &original, &updated))
}

#[async_trait]
impl Tool for FileEditTool {
    fn name(&self) -> &str {
        "file_edit"
    }

    fn description(&self) -> &str {
        "Édite un fichier texte par couples (old_string, new_string). \
         old_string doit être présent une seule fois sauf si replace_all=true. \
         En mode apply, écrit sur disque ; sinon retourne contenu+diff."
    }

    fn input_schema(&self) -> Value {
        serde_json::to_value(schemars::schema_for!(FileEditInput)).unwrap_or(Value::Null)
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<Value, ToolError> {
        let args: FileEditInput = serde_json::from_value(input)?;
        if args.edits.is_empty() {
            return Err(ToolError::invalid_args("edits must not be empty"));
        }

        let resolved = resolve_under_workspace(&ctx.effective_workspace(), &args.path)?;
        let meta = fs::metadata(&resolved)
            .await
            .map_err(|e| ToolError::io(resolved.clone(), e))?;
        if !meta.is_file() {
            return Err(ToolError::invalid_args(format!(
                "not a regular file: {resolved}"
            )));
        }
        if meta.len() > MAX_FILE_SIZE {
            return Err(ToolError::edit_failed(format!(
                "file too large ({} bytes > {} bytes limit)",
                meta.len(),
                MAX_FILE_SIZE
            )));
        }

        let bytes = fs::read(&resolved)
            .await
            .map_err(|e| ToolError::io(resolved.clone(), e))?;
        let original = String::from_utf8_lossy(&bytes).into_owned();
        let updated = apply_edits(&original, &args.edits)?;

        if updated == original {
            return Err(ToolError::edit_failed(
                "edits produced no change".to_string(),
            ));
        }

        let diff = unified_diff(&resolved, &original, &updated);

        if ctx.plan_mode {
            return Err(ToolError::plan_violation("file_edit"));
        }

        if ctx.apply_fs_writes {
            fs::write(&resolved, updated.as_bytes())
                .await
                .map_err(|e| ToolError::io(resolved.clone(), e))?;
            Ok(json!({
                "applied": true,
                "path": resolved.as_str(),
                "edits_applied": args.edits.len(),
                "diff": diff,
            }))
        } else {
            Ok(json!({
                "applied": false,
                "proposed": true,
                "path": resolved.as_str(),
                "new_content": updated,
                "diff": diff,
            }))
        }
    }
}

/// Applique les éditions séquentiellement. Erreur si une `old_string` est
/// introuvable ou ambiguë.
///
/// Exposé au module `notebook_edit` pour réutiliser la même sémantique.
pub(super) fn apply_edits(original: &str, edits: &[EditOp]) -> Result<String, ToolError> {
    let mut current = original.to_string();
    for (idx, edit) in edits.iter().enumerate() {
        if edit.old_string == edit.new_string {
            return Err(ToolError::edit_failed(format!(
                "edit #{idx}: old_string == new_string"
            )));
        }
        if edit.old_string.is_empty() {
            return Err(ToolError::edit_failed(format!(
                "edit #{idx}: old_string is empty"
            )));
        }
        if edit.replace_all {
            if !current.contains(&edit.old_string) {
                return Err(ToolError::edit_failed(format!(
                    "edit #{idx}: old_string not found"
                )));
            }
            current = current.replace(&edit.old_string, &edit.new_string);
        } else {
            let count = current.matches(&edit.old_string).count();
            match count {
                0 => {
                    return Err(ToolError::edit_failed(format!(
                        "edit #{idx}: old_string not found"
                    )));
                }
                1 => {
                    current = current.replacen(&edit.old_string, &edit.new_string, 1);
                }
                n => {
                    return Err(ToolError::edit_failed(format!(
                        "edit #{idx}: old_string found {n} times (need unique or replace_all=true)"
                    )));
                }
            }
        }
    }
    Ok(current)
}

/// Produit un diff unifié court (`--unified=3`) entre deux contenus.
fn unified_diff(path: &Utf8PathBuf, before: &str, after: &str) -> String {
    crate::diff_util::unified_line_diff(path.as_str(), before, after)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::registry::ToolRegistry;

    fn make_ctx(root: &camino::Utf8Path, apply: bool, plan: bool) -> ToolContext {
        ToolContext::new(root.to_owned(), apply).with_plan_mode(plan)
    }

    async fn write_file(root: &camino::Utf8Path, name: &str, content: &str) {
        let path = root.join(name);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await.unwrap();
        }
        tokio::fs::write(&path, content).await.unwrap();
    }

    #[tokio::test]
    async fn edit_accepts_old_new_aliases() {
        let tmp = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        write_file(&root, "alias.txt", "one two\n").await;
        let ctx = make_ctx(&root, true, false);
        let reg = ToolRegistry::with_simple_tools();
        let out = reg
            .execute_named(
                "file_edit",
                &ctx,
                json!({
                    "file_path": "alias.txt",
                    "edits": [{ "old": "two", "new": "deux" }],
                }),
            )
            .await
            .unwrap();
        assert_eq!(out["applied"], true);
        let after = tokio::fs::read_to_string(root.join("alias.txt")).await.unwrap();
        assert_eq!(after, "one deux\n");
    }

    #[tokio::test]
    async fn unique_replace_applies() {
        let tmp = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        write_file(&root, "a.txt", "hello world\n").await;
        let ctx = make_ctx(&root, true, false);
        let reg = ToolRegistry::with_simple_tools();
        let out = reg
            .execute_named(
                "file_edit",
                &ctx,
                json!({
                    "path": "a.txt",
                    "edits": [{ "old_string": "world", "new_string": "drox" }],
                }),
            )
            .await
            .unwrap();
        assert_eq!(out["applied"], true);
        let after = tokio::fs::read_to_string(root.join("a.txt")).await.unwrap();
        assert_eq!(after, "hello drox\n");
        assert!(out["diff"].as_str().unwrap().contains("-hello world"));
    }

    #[tokio::test]
    async fn propose_mode_does_not_write() {
        let tmp = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        write_file(&root, "b.txt", "alpha beta\n").await;
        let ctx = make_ctx(&root, false, false);
        let reg = ToolRegistry::with_simple_tools();
        let out = reg
            .execute_named(
                "file_edit",
                &ctx,
                json!({
                    "path": "b.txt",
                    "edits": [{ "old_string": "beta", "new_string": "gamma" }],
                }),
            )
            .await
            .unwrap();
        assert_eq!(out["applied"], false);
        assert_eq!(out["proposed"], true);
        let on_disk = tokio::fs::read_to_string(root.join("b.txt")).await.unwrap();
        assert_eq!(on_disk, "alpha beta\n");
    }

    #[tokio::test]
    async fn duplicate_old_string_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        write_file(&root, "c.txt", "x\nx\n").await;
        let ctx = make_ctx(&root, true, false);
        let reg = ToolRegistry::with_simple_tools();
        let err = reg
            .execute_named(
                "file_edit",
                &ctx,
                json!({
                    "path": "c.txt",
                    "edits": [{ "old_string": "x", "new_string": "y" }],
                }),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::EditFailed(_)));
    }

    #[tokio::test]
    async fn replace_all_works() {
        let tmp = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        write_file(&root, "d.txt", "ab\nab\n").await;
        let ctx = make_ctx(&root, true, false);
        let reg = ToolRegistry::with_simple_tools();
        let out = reg
            .execute_named(
                "file_edit",
                &ctx,
                json!({
                    "path": "d.txt",
                    "edits": [{ "old_string": "ab", "new_string": "Z", "replace_all": true }],
                }),
            )
            .await
            .unwrap();
        assert_eq!(out["applied"], true);
        let after = tokio::fs::read_to_string(root.join("d.txt")).await.unwrap();
        assert_eq!(after, "Z\nZ\n");
    }

    #[tokio::test]
    async fn plan_mode_blocks_edit() {
        let tmp = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        write_file(&root, "e.txt", "foo\n").await;
        let ctx = make_ctx(&root, true, true);
        let reg = ToolRegistry::with_simple_tools();
        let err = reg
            .execute_named(
                "file_edit",
                &ctx,
                json!({
                    "path": "e.txt",
                    "edits": [{ "old_string": "foo", "new_string": "bar" }],
                }),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::PlanModeViolation(_)));
    }

    #[test]
    fn preview_diff_shows_changes() {
        let tmp = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        std::fs::write(root.join("p.txt"), "alpha\nbeta\n").unwrap();
        let diff = preview_file_edit_diff(
            &root,
            &json!({
                "path": "p.txt",
                "edits": [{ "old_string": "beta", "new_string": "gamma" }],
            }),
        )
        .unwrap();
        assert!(diff.contains("-beta"));
        assert!(diff.contains("+gamma"));
    }
}
