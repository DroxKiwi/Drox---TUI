//! Tool `copy_path` — copie un fichier sous le workspace (cross-platform).
//!
//! Préféré à `bash copy` / `cp` / `robocopy` : chemins résolus, pas de quoting Windows.

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::fs;

use crate::context::ToolContext;
use crate::error::ToolError;
use crate::path_util::{resolve_path_for_write, resolve_under_workspace};
use crate::tool::Tool;

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CopyPathInput {
    /// Fichier source (relatif au workspace ou absolu sous le workspace).
    pub source: String,
    /// Fichier destination (relatif ou absolu sous le workspace).
    pub destination: String,
    /// Crée les répertoires parents de la destination si besoin (défaut `true`).
    #[serde(default = "default_create_dirs")]
    pub create_dirs: bool,
}

const fn default_create_dirs() -> bool {
    true
}

pub struct CopyPathTool;

#[async_trait]
impl Tool for CopyPathTool {
    fn name(&self) -> &str {
        "copy_path"
    }

    fn description(&self) -> &str {
        "Copie un **fichier** vers une autre destination strictement sous le workspace. \
         Préfère cet outil à `bash copy` / `cp` / `robocopy`. \
         Format : {\"source\": \"…\", \"destination\": \"…\", \"createDirs\"?: true}. \
         Ne copie pas de dossiers entiers (utilise plusieurs appels ou `glob` + copie fichier par fichier)."
    }

    fn input_schema(&self) -> Value {
        serde_json::to_value(schemars::schema_for!(CopyPathInput)).unwrap_or(Value::Null)
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<Value, ToolError> {
        let args: CopyPathInput = serde_json::from_value(input).map_err(|e| {
            ToolError::invalid_args(format!(
                "copy_path: JSON invalide ({e}). Attendu : {{\"source\":\"…\",\"destination\":\"…\"}}."
            ))
        })?;

        if ctx.plan_mode {
            return Err(ToolError::plan_violation("copy_path"));
        }

        let ws = ctx.effective_workspace();
        let src = resolve_under_workspace(&ws, &args.source)?;
        let dest = resolve_path_for_write(&ws, &args.destination)?;

        let src_meta = fs::metadata(&src)
            .await
            .map_err(|e| ToolError::io(src.clone(), e))?;
        if !src_meta.is_file() {
            return Err(ToolError::invalid_args(
                "copy_path: source must be a regular file (not a directory)",
            ));
        }

        if args.create_dirs {
            if let Some(parent) = dest.parent() {
                if !parent.as_str().is_empty() {
                    fs::create_dir_all(parent)
                        .await
                        .map_err(|e| ToolError::io(parent.to_owned(), e))?;
                }
            }
        }

        if ctx.apply_fs_writes {
            fs::copy(&src, &dest)
                .await
                .map_err(|e| ToolError::io(dest.clone(), e))?;
            Ok(json!({
                "copied": true,
                "source": src.as_str(),
                "destination": dest.as_str(),
                "bytes": src_meta.len(),
            }))
        } else {
            Ok(json!({
                "copied": false,
                "proposed": true,
                "source": src.as_str(),
                "destination": dest.as_str(),
                "bytes": src_meta.len(),
            }))
        }
    }
}
