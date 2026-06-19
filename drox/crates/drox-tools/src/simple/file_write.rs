//! Tool `file_write` — écrit un fichier sous le workspace.

use async_trait::async_trait;
use camino::Utf8Path;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::fs;

use crate::context::ToolContext;
use crate::error::ToolError;
use crate::path_util::resolve_path_for_write;
use crate::tool::Tool;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FileWriteInput {
    /// Chemin relatif au workspace ou absolu **sous** le workspace.
    pub path: String,
    /// Contenu UTF-8 à écrire.
    pub content: String,
}

pub struct FileWriteTool;

/// Produit un diff unifié pour une proposition `file_write` (lecture disque sync).
pub fn preview_file_write_diff(workspace: &Utf8Path, input: &Value) -> Result<String, ToolError> {
    let args: FileWriteInput = serde_json::from_value(input.clone())?;
    let resolved = resolve_path_for_write(workspace, &args.path)?;
    let before = std::fs::read_to_string(&resolved).unwrap_or_default();
    Ok(crate::diff_util::unified_line_diff(
        resolved.as_str(),
        &before,
        &args.content,
    ))
}

#[async_trait]
impl Tool for FileWriteTool {
    fn name(&self) -> &str {
        "file_write"
    }

    fn description(&self) -> &str {
        "Écrit un fichier texte sous le workspace. En mode `apply_fs_writes`, écrit sur disque ; sinon retourne une proposition JSON."
    }

    fn input_schema(&self) -> Value {
        serde_json::to_value(schemars::schema_for!(FileWriteInput)).unwrap_or(Value::Null)
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<Value, ToolError> {
        let args: FileWriteInput = serde_json::from_value(input)?;
        let resolved = resolve_path_for_write(&ctx.effective_workspace(), &args.path)?;

        if ctx.plan_mode {
            return Err(ToolError::plan_violation("file_write"));
        }

        if ctx.apply_fs_writes {
            if let Some(parent) = resolved.parent() {
                fs::create_dir_all(parent)
                    .await
                    .map_err(|e| ToolError::io(parent.to_owned(), e))?;
            }
            fs::write(&resolved, args.content.as_bytes())
                .await
                .map_err(|e| ToolError::io(resolved.clone(), e))?;
            Ok(json!({
                "applied": true,
                "path": resolved.as_str(),
                "bytes_written": args.content.len(),
            }))
        } else {
            Ok(json!({
                "applied": false,
                "proposed": true,
                "path": resolved.as_str(),
                "content": args.content,
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::registry::ToolRegistry;

    #[tokio::test]
    async fn propose_mode_does_not_write() {
        let tmp = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        tokio::fs::create_dir_all(root.join("d")).await.unwrap();
        let ctx = ToolContext::new(root.clone(), false);
        let reg = ToolRegistry::with_simple_tools();
        let out = reg
            .execute_named(
                "file_write",
                &ctx,
                json!({ "path": "d/x.txt", "content": "hi" }),
            )
            .await
            .unwrap();
        assert_eq!(out["applied"], false);
        assert!(!root.join("d/x.txt").exists());
    }

    #[tokio::test]
    async fn apply_mode_writes() {
        let tmp = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        tokio::fs::create_dir_all(root.join("d")).await.unwrap();
        let ctx = ToolContext::new(root.clone(), true);
        let reg = ToolRegistry::with_simple_tools();
        reg.execute_named(
            "file_write",
            &ctx,
            json!({ "path": "d/y.txt", "content": "yo" }),
        )
        .await
        .unwrap();
        let text = tokio::fs::read_to_string(root.join("d/y.txt"))
            .await
            .unwrap();
        assert_eq!(text, "yo");
    }
}
