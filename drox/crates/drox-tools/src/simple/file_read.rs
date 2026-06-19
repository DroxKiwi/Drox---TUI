//! Tool `file_read` — lit un fichier texte sous le workspace (entier ou plage de lignes).

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::fs;

use crate::context::ToolContext;
use crate::error::ToolError;
use crate::path_util::resolve_under_workspace;
use crate::tool::Tool;

/// Lecture « tête de fichier » : même plafond qu’historiquement.
const MAX_BYTES_FULL: u64 = 512 * 1024;
/// Borne sur le nombre de lignes renvoyées en mode plage (1-based inclusif).
const MAX_RANGE_LINES: u32 = 400;
/// Borne sur la taille du fragment renvoyé en mode plage.
const MAX_RANGE_OUTPUT_BYTES: usize = 128 * 1024;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FileReadInput {
    /// Chemin relatif au workspace ou absolu **sous** le workspace.
    pub path: String,
    /// Première ligne à inclure (**1-based**). Si renseignée, `end_line` doit l’être aussi.
    #[serde(default)]
    pub start_line: Option<u32>,
    /// Dernière ligne incluse (**1-based**), ≥ `start_line`. Fenêtre max. `MAX_RANGE_LINES` lignes.
    #[serde(default)]
    pub end_line: Option<u32>,
}

#[derive(Debug, Serialize)]
struct FileReadOutput {
    path: String,
    content: String,
    truncated: bool,
    size_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    start_line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    end_line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lines_returned: Option<u32>,
}

pub struct FileReadTool;

#[async_trait]
impl Tool for FileReadTool {
    fn name(&self) -> &str {
        "file_read"
    }

    fn description(&self) -> &str {
        "Lit le contenu UTF-8 d'un fichier sous la racine workspace. Sans `start_line`/`end_line` : \
         lecture depuis le début (tronquée au-delà de 512 KiB). Avec les deux : **fenêtre de lignes** \
         1-based inclusive (max 400 lignes, sortie max ~128 KiB) — idéal après un `grep` pour ne pas \
         charger tout le fichier."
    }

    fn input_schema(&self) -> Value {
        serde_json::to_value(schemars::schema_for!(FileReadInput)).unwrap_or(Value::Null)
    }

    fn is_read_only(&self) -> bool {
        true
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<Value, ToolError> {
        let args: FileReadInput = serde_json::from_value(input)?;
        let resolved = resolve_under_workspace(&ctx.effective_workspace(), &args.path)?;
        ctx.deny_if_drox_ignored(&resolved)?;
        let meta = fs::metadata(&resolved)
            .await
            .map_err(|e| ToolError::io(resolved.clone(), e))?;
        if !meta.is_file() {
            return Err(ToolError::invalid_args(format!(
                "not a regular file: {resolved}",
            )));
        }
        let len = meta.len();

        match (args.start_line, args.end_line) {
            (None, None) => read_full(&resolved, len).await,
            (Some(start), Some(end)) => {
                if start == 0 {
                    return Err(ToolError::invalid_args(
                        "file_read: `start_line` doit être ≥ 1 (numérotation 1-based).",
                    ));
                }
                if end < start {
                    return Err(ToolError::invalid_args(
                        "file_read: `end_line` doit être ≥ `start_line`.",
                    ));
                }
                let n_lines = end - start + 1;
                if n_lines > MAX_RANGE_LINES {
                    return Err(ToolError::invalid_args(format!(
                        "file_read: fenêtre trop large ({n_lines} lignes, max {MAX_RANGE_LINES}). \
                         Réduis la plage ou découpe en plusieurs appels.",
                    )));
                }
                read_line_range(&resolved, len, start, end).await
            }
            _ => Err(ToolError::invalid_args(
                "file_read: fournir **les deux** `start_line` et `end_line`, ou aucun des deux.",
            )),
        }
    }
}

async fn read_full(resolved: &camino::Utf8Path, len: u64) -> Result<Value, ToolError> {
    let truncated = len > MAX_BYTES_FULL;
    let capped = len.min(MAX_BYTES_FULL);
    let read_len = usize::try_from(capped).unwrap_or(0);
    let bytes = fs::read(resolved)
        .await
        .map_err(|e| ToolError::io(resolved.to_owned(), e))?;
    let slice = bytes.get(..read_len).unwrap_or(&bytes);
    let content = String::from_utf8_lossy(slice).into_owned();
    let out = FileReadOutput {
        path: resolved.to_string(),
        content,
        truncated,
        size_bytes: len,
        start_line: None,
        end_line: None,
        lines_returned: None,
    };
    Ok(serde_json::to_value(out)?)
}

async fn read_line_range(
    resolved: &camino::Utf8Path,
    len: u64,
    start: u32,
    end: u32,
) -> Result<Value, ToolError> {
    let file = File::open(resolved.as_std_path())
        .await
        .map_err(|e| ToolError::io(resolved.to_owned(), e))?;
    let reader = BufReader::new(file);
    let mut lines = reader.lines();

    let mut current: u32 = 0;
    let mut out = String::new();
    let mut truncated = false;
    let mut count_returned: u32 = 0;

    while let Some(line) = lines
        .next_line()
        .await
        .map_err(|e| ToolError::io(resolved.to_owned(), e))?
    {
        current += 1;
        if current < start {
            continue;
        }
        if current > end {
            break;
        }
        out.push_str(&line);
        out.push('\n');
        count_returned += 1;
        if out.len() > MAX_RANGE_OUTPUT_BYTES {
            truncated = true;
            break;
        }
    }

    if current < start {
        return Err(ToolError::invalid_args(format!(
            "file_read: `start_line` ({start}) dépasse la fin du fichier ({current} lignes).",
        )));
    }

    let out = FileReadOutput {
        path: resolved.to_string(),
        content: out,
        truncated,
        size_bytes: len,
        start_line: Some(start),
        end_line: Some(end),
        lines_returned: Some(count_returned),
    };
    Ok(serde_json::to_value(out)?)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::registry::ToolRegistry;

    #[tokio::test]
    async fn reads_utf8_file() {
        let tmp = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        let p = root.join("hello.txt");
        tokio::fs::write(&p, "world").await.unwrap();
        let ctx = ToolContext::new(root.clone(), false);
        let reg = ToolRegistry::with_simple_tools();
        let out = reg
            .execute_named("file_read", &ctx, json!({ "path": "hello.txt" }))
            .await
            .unwrap();
        assert_eq!(out["content"], "world");
        assert_eq!(out["truncated"], false);
        assert!(out["start_line"].is_null());
    }

    #[tokio::test]
    async fn reads_line_range() {
        let tmp = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        let body = "L1\nL2\nL3\nL4\n";
        tokio::fs::write(root.join("x.txt"), body).await.unwrap();
        let ctx = ToolContext::new(root.clone(), false);
        let reg = ToolRegistry::with_simple_tools();
        let out = reg
            .execute_named(
                "file_read",
                &ctx,
                json!({ "path": "x.txt", "start_line": 2, "end_line": 3 }),
            )
            .await
            .unwrap();
        assert_eq!(out["content"], "L2\nL3\n");
        assert_eq!(out["start_line"], 2);
        assert_eq!(out["end_line"], 3);
        assert_eq!(out["lines_returned"], 2);
        assert_eq!(out["truncated"], false);
    }

    #[tokio::test]
    async fn rejects_partial_range_args() {
        let tmp = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        tokio::fs::write(root.join("a.txt"), "x\n").await.unwrap();
        let ctx = ToolContext::new(root, false);
        let reg = ToolRegistry::with_simple_tools();
        let err = reg
            .execute_named(
                "file_read",
                &ctx,
                json!({ "path": "a.txt", "start_line": 1 }),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidArgs(_)), "{err:?}");
    }

    #[tokio::test]
    async fn droxignore_blocks_file_read() {
        let tmp = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        tokio::fs::write(root.join(".env"), "SECRET=1").await.unwrap();
        let ignore = drox_session::DroxIgnoreMatcher::load_or_create(root.clone())
            .await
            .unwrap();
        let ctx = ToolContext::new(root, false).with_drox_ignore(ignore);
        let reg = ToolRegistry::with_simple_tools();
        let err = reg
            .execute_named("file_read", &ctx, json!({ "path": ".env" }))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::DroxIgnore { .. }), "{err:?}");
    }

    #[tokio::test]
    async fn rejects_start_beyond_eof() {
        let tmp = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        tokio::fs::write(root.join("b.txt"), "one\n").await.unwrap();
        let ctx = ToolContext::new(root, false);
        let reg = ToolRegistry::with_simple_tools();
        let err = reg
            .execute_named(
                "file_read",
                &ctx,
                json!({ "path": "b.txt", "start_line": 5, "end_line": 10 }),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidArgs(_)), "{err:?}");
    }
}
