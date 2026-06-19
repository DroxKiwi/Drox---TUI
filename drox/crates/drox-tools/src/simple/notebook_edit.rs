//! Tool `notebook_edit` — édition ciblée des cellules d’un `.ipynb`.
//!
//! Trois modes par entrée dans `edits` (inspiré du leak `NotebookEditTool`) :
//! - **`replace`** (défaut) — `old_string` → `new_string` dans la source de la cellule
//!   (même règles que `file_edit`). Si `old_string` est vide, remplace toute la source.
//! - **`insert`** — insère une cellule à `cell_index` (`new_string` = source, `cell_type` :
//!   `code` | `markdown`, défaut `code`).
//! - **`delete`** — supprime la cellule à `cell_index`.
//!
//! Les `edits` s’appliquent **dans l’ordre** ; les indices sont relatifs à l’état courant
//! après chaque opération.

use async_trait::async_trait;
use camino::Utf8PathBuf;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Map, Value, json};
use similar::TextDiff;
use tokio::fs;

use super::file_edit::{EditOp, apply_edits};
use crate::context::ToolContext;
use crate::error::ToolError;
use crate::path_util::resolve_under_workspace;
use crate::tool::Tool;

const MAX_FILE_SIZE: u64 = 512 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CellEditMode {
    #[default]
    Replace,
    Insert,
    Delete,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct NotebookEditInput {
    /// Chemin (relatif au workspace) du notebook `.ipynb`.
    #[serde(alias = "file_path", alias = "notebook_path")]
    pub path: String,
    pub edits: Vec<NotebookCellEdit>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct NotebookCellEdit {
    /// Index de cellule (0-based) dans le tableau `cells` **après** les éditions précédentes.
    #[serde(alias = "cellIndex", alias = "cell_number")]
    pub cell_index: usize,
    #[serde(default)]
    pub edit_mode: CellEditMode,
    #[serde(default, alias = "old", alias = "oldString")]
    pub old_string: String,
    #[serde(default, alias = "new", alias = "newString", alias = "new_source")]
    pub new_string: String,
    #[serde(default)]
    pub replace_all: bool,
    /// Pour `insert` : `code` (défaut) ou `markdown`.
    #[serde(default, alias = "cell_type")]
    pub cell_type: Option<String>,
}

pub struct NotebookEditTool;

fn source_to_string(v: &Value) -> Result<String, ToolError> {
    match v {
        Value::String(s) => Ok(s.clone()),
        Value::Array(parts) => {
            let mut out = String::new();
            for p in parts {
                let s = p.as_str().ok_or_else(|| {
                    ToolError::edit_failed("cell.source array must contain only strings")
                })?;
                out.push_str(s);
            }
            Ok(out)
        }
        Value::Null => Ok(String::new()),
        _ => Err(ToolError::edit_failed(
            "cell.source must be a string or array of strings",
        )),
    }
}

fn set_cell_source(cell: &mut Value, new_src: String) {
    if let Some(obj) = cell.as_object_mut() {
        obj.insert("source".into(), Value::String(new_src));
    }
}

fn reset_code_cell_execution(cell: &mut Value) {
    if let Some(obj) = cell.as_object_mut() {
        if obj.get("cell_type").and_then(|v| v.as_str()) == Some("code") {
            obj.insert("execution_count".into(), Value::Null);
            obj.insert("outputs".into(), json!([]));
        }
    }
}

fn new_cell_value(cell_type: &str, source: String) -> Value {
    let mut obj = Map::new();
    let ct = if cell_type.eq_ignore_ascii_case("markdown") {
        "markdown"
    } else {
        "code"
    };
    obj.insert("cell_type".into(), Value::String(ct.into()));
    obj.insert("metadata".into(), json!({}));
    let id = format!("drox-{:x}", edit_index_hash(&source));
    obj.insert("source".into(), Value::String(source));
    if ct == "code" {
        obj.insert("execution_count".into(), Value::Null);
        obj.insert("outputs".into(), json!([]));
    }
    obj.insert("id".into(), Value::String(id));
    Value::Object(obj)
}

fn edit_index_hash(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Rattrape les payloads mal formés (modèles locaux, format leak plat).
pub(crate) fn normalize_input(input: Value) -> Result<Value, ToolError> {
    if serde_json::from_value::<NotebookEditInput>(input.clone()).is_ok() {
        return Ok(input);
    }

    let Some(mut obj) = input.as_object().cloned() else {
        return Err(ToolError::invalid_args(
            "notebook_edit: input must be a JSON object",
        ));
    };

    let path = take_string(&mut obj, &["path", "file_path", "notebook_path"])
        .ok_or_else(|| ToolError::invalid_args("notebook_edit: missing path (or notebook_path)"))?;

    // Format leak plat : { notebook_path, new_source, cell_number?, edit_mode? }
    if obj.contains_key("new_source") {
        let new_source = take_string(&mut obj, &["new_source", "new_string", "new"])
            .unwrap_or_default();
        let cell_index = take_usize(&mut obj, &["cell_index", "cell_number", "cellIndex"])
            .unwrap_or(0);
        let edit_mode = take_edit_mode(&mut obj);
        let cell_type = take_string(&mut obj, &["cell_type"]);
        return Ok(single_edit_payload(
            path,
            cell_index,
            edit_mode,
            new_source,
            String::new(),
            cell_type,
        ));
    }

    // Objet plat avec cell_index + old/new mais sans tableau edits
    if obj.contains_key("cell_index") || obj.contains_key("cell_number") {
        let cell_index = take_usize(&mut obj, &["cell_index", "cell_number", "cellIndex"])
            .unwrap_or(0);
        let edit_mode = take_edit_mode(&mut obj);
        let old_string = take_string(&mut obj, &["old_string", "old", "oldString"]).unwrap_or_default();
        let new_string =
            take_string(&mut obj, &["new_string", "new", "newString", "new_source"]).unwrap_or_default();
        let cell_type = take_string(&mut obj, &["cell_type"]);
        return Ok(single_edit_payload(
            path,
            cell_index,
            edit_mode,
            new_string,
            old_string,
            cell_type,
        ));
    }

    // { path, edit: { ... } } singulier
    if let Some(edit_val) = obj.remove("edit") {
        return Ok(json!({
            "path": path,
            "edits": [edit_val]
        }));
    }

    Err(ToolError::invalid_args(
        "notebook_edit: expected {\"path\":\"notebook.ipynb\",\"edits\":[{\"cell_index\":0,\"old_string\":\"…\",\"new_string\":\"…\"}]}. \
         For insert: {\"edit_mode\":\"insert\",\"new_string\":\"…\",\"cell_type\":\"code\"}. For delete: {\"edit_mode\":\"delete\"}. \
         Leak-style flat: {\"notebook_path\":\"…\",\"cell_number\":0,\"new_source\":\"…\",\"edit_mode\":\"replace\"}.",
    ))
}

fn single_edit_payload(
    path: String,
    cell_index: usize,
    edit_mode: CellEditMode,
    new_string: String,
    old_string: String,
    cell_type: Option<String>,
) -> Value {
    let mode_str = match edit_mode {
        CellEditMode::Insert => "insert",
        CellEditMode::Delete => "delete",
        CellEditMode::Replace => "replace",
    };
    let mut edit = json!({
        "cell_index": cell_index,
        "edit_mode": mode_str,
        "old_string": old_string,
        "new_string": new_string,
    });
    if let Some(ct) = cell_type {
        edit["cell_type"] = Value::String(ct);
    }
    json!({ "path": path, "edits": [edit] })
}

fn take_string(obj: &mut Map<String, Value>, keys: &[&str]) -> Option<String> {
    for k in keys {
        if let Some(v) = obj.remove(*k) {
            if let Some(s) = v.as_str() {
                return Some(s.to_string());
            }
        }
    }
    None
}

fn take_usize(obj: &mut Map<String, Value>, keys: &[&str]) -> Option<usize> {
    for k in keys {
        if let Some(v) = obj.remove(*k) {
            if let Some(n) = v.as_u64() {
                return Some(n as usize);
            }
            if let Some(s) = v.as_str() {
                if let Ok(n) = s.parse::<usize>() {
                    return Some(n);
                }
            }
        }
    }
    None
}

fn take_edit_mode(obj: &mut Map<String, Value>) -> CellEditMode {
    let mode = obj
        .remove("edit_mode")
        .and_then(|v| v.as_str().map(str::to_string));
    parse_edit_mode(mode.as_deref())
}

fn parse_edit_mode(s: Option<&str>) -> CellEditMode {
    match s.map(str::to_ascii_lowercase).as_deref() {
        Some("insert") => CellEditMode::Insert,
        Some("delete") => CellEditMode::Delete,
        _ => CellEditMode::Replace,
    }
}

fn apply_cell_edits(cells: &mut Vec<Value>, edits: &[NotebookCellEdit]) -> Result<(), ToolError> {
    for (idx, edit) in edits.iter().enumerate() {
        match edit.edit_mode {
            CellEditMode::Delete => {
                if edit.cell_index >= cells.len() {
                    return Err(ToolError::edit_failed(format!(
                        "edit #{idx}: cell_index {} out of range (len={})",
                        edit.cell_index,
                        cells.len()
                    )));
                }
                cells.remove(edit.cell_index);
            }
            CellEditMode::Insert => {
                if edit.new_string.is_empty() {
                    return Err(ToolError::edit_failed(format!(
                        "edit #{idx}: insert requires non-empty new_string"
                    )));
                }
                let ct = edit.cell_type.as_deref().unwrap_or("code");
                let new_cell = new_cell_value(ct, edit.new_string.clone());
                if edit.cell_index > cells.len() {
                    return Err(ToolError::edit_failed(format!(
                        "edit #{idx}: cell_index {} out of range for insert (len={})",
                        edit.cell_index,
                        cells.len()
                    )));
                }
                cells.insert(edit.cell_index, new_cell);
            }
            CellEditMode::Replace => {
                let n_cells = cells.len();
                let cell = cells.get_mut(edit.cell_index).ok_or_else(|| {
                    ToolError::edit_failed(format!(
                        "edit #{idx}: cell_index {} out of range (len={n_cells})",
                        edit.cell_index,
                    ))
                })?;
                let updated_src = if edit.old_string.is_empty() {
                    edit.new_string.clone()
                } else {
                    let cell_obj = cell.as_object().ok_or_else(|| {
                        ToolError::edit_failed(format!("edit #{idx}: cell is not an object"))
                    })?;
                    let src_val = cell_obj.get("source").ok_or_else(|| {
                        ToolError::edit_failed(format!(
                            "edit #{idx}: cell {} has no \"source\" field",
                            edit.cell_index
                        ))
                    })?;
                    let src_str = source_to_string(src_val)?;
                    let ops = [EditOp {
                        old_string: edit.old_string.clone(),
                        new_string: edit.new_string.clone(),
                        replace_all: edit.replace_all,
                    }];
                    apply_edits(&src_str, &ops).map_err(|e| {
                        ToolError::edit_failed(format!(
                            "edit #{idx} on cell {}: {e}",
                            edit.cell_index
                        ))
                    })?
                };
                set_cell_source(cell, updated_src);
                reset_code_cell_execution(cell);
            }
        }
    }
    Ok(())
}

#[async_trait]
impl Tool for NotebookEditTool {
    fn name(&self) -> &str {
        "notebook_edit"
    }

    fn description(&self) -> &str {
        "Édite un notebook Jupyter (.ipynb). Chaque entrée de `edits` cible une cellule par \
         `cell_index` (0-based). Mode `replace` (défaut) : old_string → new_string dans la source \
         (comme file_edit ; old_string vide = remplacer toute la source). Mode `insert` : \
         nouvelle cellule à l’index (`new_string`, `cell_type` code|markdown). Mode `delete` : \
         supprime la cellule. Exemple : \
         {\"path\":\"nb.ipynb\",\"edits\":[{\"cell_index\":1,\"old_string\":\"a\",\"new_string\":\"b\"}]}"
    }

    fn input_schema(&self) -> Value {
        serde_json::to_value(schemars::schema_for!(NotebookEditInput)).unwrap_or(Value::Null)
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<Value, ToolError> {
        let input = normalize_input(input)?;
        let args: NotebookEditInput = serde_json::from_value(input).map_err(|e| {
            ToolError::invalid_args(format!(
                "notebook_edit: invalid schema after normalization — use \
                 {{\"path\":\"file.ipynb\",\"edits\":[{{\"cell_index\":0,\"old_string\":\"x\",\"new_string\":\"y\"}}]}}. \
                 Detail: {e}"
            ))
        })?;
        if args.edits.is_empty() {
            return Err(ToolError::invalid_args("edits must not be empty"));
        }

        let resolved = resolve_under_workspace(&ctx.effective_workspace(), &args.path)?;
        ctx.deny_if_drox_ignored(&resolved)?;
        if !resolved.as_str().to_ascii_lowercase().ends_with(".ipynb") {
            return Err(ToolError::invalid_args(
                "notebook_edit: path must end with .ipynb",
            ));
        }

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

        let mut nb: Value = serde_json::from_str(&original).map_err(|e| {
            ToolError::edit_failed(format!("invalid JSON (not a notebook?): {e}"))
        })?;

        let cells = nb
            .get_mut("cells")
            .and_then(|c| c.as_array_mut())
            .ok_or_else(|| {
                ToolError::edit_failed("notebook missing top-level \"cells\" array")
            })?;

        apply_cell_edits(cells, &args.edits)?;

        let updated = serde_json::to_string_pretty(&nb).map_err(|e| {
            ToolError::edit_failed(format!("serialize notebook failed: {e}"))
        })?;
        let updated = format!("{updated}\n");

        if updated == original {
            return Err(ToolError::edit_failed(
                "edits produced no change".to_string(),
            ));
        }

        let diff = unified_diff(&resolved, &original, &updated);

        if ctx.plan_mode {
            return Err(ToolError::plan_violation("notebook_edit"));
        }

        if ctx.apply_fs_writes {
            fs::write(&resolved, updated.as_bytes())
                .await
                .map_err(|e| ToolError::io(resolved.clone(), e))?;
            Ok(json!({
                "applied": true,
                "path": resolved.as_str(),
                "cell_edits_applied": args.edits.len(),
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

/// Diff unifié pour preview permission TUI (lecture disque sync).
pub fn preview_notebook_edit_diff(workspace: &camino::Utf8Path, input: &Value) -> Result<String, ToolError> {
    let input = normalize_input(input.clone())?;
    let args: NotebookEditInput = serde_json::from_value(input).map_err(|e| {
        ToolError::invalid_args(format!("notebook_edit preview: invalid schema — {e}"))
    })?;
    if args.edits.is_empty() {
        return Err(ToolError::invalid_args("edits must not be empty"));
    }

    let resolved = resolve_under_workspace(workspace, &args.path)?;
    if !resolved.as_str().to_ascii_lowercase().ends_with(".ipynb") {
        return Err(ToolError::invalid_args(
            "notebook_edit: path must end with .ipynb",
        ));
    }

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
    let mut nb: Value = serde_json::from_str(&original).map_err(|e| {
        ToolError::edit_failed(format!("invalid JSON (not a notebook?): {e}"))
    })?;
    let cells = nb
        .get_mut("cells")
        .and_then(|c| c.as_array_mut())
        .ok_or_else(|| ToolError::edit_failed("notebook missing top-level \"cells\" array"))?;

    apply_cell_edits(cells, &args.edits)?;
    let updated = serde_json::to_string_pretty(&nb).map_err(|e| {
        ToolError::edit_failed(format!("serialize notebook failed: {e}"))
    })?;
    let updated = format!("{updated}\n");
    if updated == original {
        return Err(ToolError::edit_failed("edits produced no change"));
    }
    Ok(unified_diff(&resolved, &original, &updated))
}

fn unified_diff(path: &Utf8PathBuf, before: &str, after: &str) -> String {
    let diff = TextDiff::from_lines(before, after);
    let label = path.as_str();
    diff.unified_diff()
        .context_radius(3)
        .header(label, label)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::ToolRegistry;

    fn sample_nb() -> String {
        json!({
            "nbformat": 4,
            "nbformat_minor": 5,
            "cells": [
                {
                    "cell_type": "markdown",
                    "metadata": {},
                    "source": ["# Hi\n", "there"]
                },
                {
                    "cell_type": "code",
                    "metadata": {},
                    "source": "print(1)\n",
                    "execution_count": 3,
                    "outputs": [{"output_type": "stream", "name": "stdout", "text": ["1\n"]}]
                }
            ],
            "metadata": {}
        })
        .to_string()
    }

    fn make_ctx(root: &camino::Utf8Path, apply: bool, plan: bool) -> ToolContext {
        ToolContext::new(root.to_owned(), apply).with_plan_mode(plan)
    }

    #[tokio::test]
    async fn applies_to_string_source() {
        let tmp = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        let p = root.join("n.ipynb");
        tokio::fs::write(&p, sample_nb()).await.unwrap();
        let ctx = make_ctx(&root, true, false);
        let reg = ToolRegistry::with_simple_tools();
        let out = reg
            .execute_named(
                "notebook_edit",
                &ctx,
                json!({
                    "path": "n.ipynb",
                    "edits": [{ "cell_index": 1, "old_string": "1", "new_string": "2" }],
                }),
            )
            .await
            .unwrap();
        assert_eq!(out["applied"], true);
        let raw = tokio::fs::read_to_string(&p).await.unwrap();
        assert!(raw.contains("print(2)"));
        assert!(raw.contains("\"execution_count\": null"));
    }

    #[tokio::test]
    async fn applies_to_array_source() {
        let tmp = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        let p = root.join("n.ipynb");
        tokio::fs::write(&p, sample_nb()).await.unwrap();
        let ctx = make_ctx(&root, true, false);
        let reg = ToolRegistry::with_simple_tools();
        let out = reg
            .execute_named(
                "notebook_edit",
                &ctx,
                json!({
                    "file_path": "n.ipynb",
                    "edits": [{ "cell_index": 0, "old": "Hi", "new": "Hello" }],
                }),
            )
            .await
            .unwrap();
        assert_eq!(out["applied"], true);
        let raw = tokio::fs::read_to_string(&p).await.unwrap();
        assert!(raw.contains("# Hello"));
    }

    #[tokio::test]
    async fn inserts_cell_at_index() {
        let tmp = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        let p = root.join("n.ipynb");
        tokio::fs::write(&p, sample_nb()).await.unwrap();
        let ctx = make_ctx(&root, true, false);
        let reg = ToolRegistry::with_simple_tools();
        reg.execute_named(
            "notebook_edit",
            &ctx,
            json!({
                "path": "n.ipynb",
                "edits": [{
                    "cell_index": 1,
                    "edit_mode": "insert",
                    "new_string": "# inserted\n",
                    "cell_type": "markdown"
                }],
            }),
        )
        .await
        .unwrap();
        let raw = tokio::fs::read_to_string(&p).await.unwrap();
        let nb: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(nb["cells"].as_array().unwrap().len(), 3);
        assert!(raw.contains("# inserted"));
    }

    #[tokio::test]
    async fn deletes_cell() {
        let tmp = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        let p = root.join("n.ipynb");
        tokio::fs::write(&p, sample_nb()).await.unwrap();
        let ctx = make_ctx(&root, true, false);
        let reg = ToolRegistry::with_simple_tools();
        reg.execute_named(
            "notebook_edit",
            &ctx,
            json!({
                "path": "n.ipynb",
                "edits": [{ "cell_index": 0, "edit_mode": "delete" }],
            }),
        )
        .await
        .unwrap();
        let raw = tokio::fs::read_to_string(&p).await.unwrap();
        let nb: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(nb["cells"].as_array().unwrap().len(), 1);
        assert!(!raw.contains("# Hi"));
    }

    #[test]
    fn normalize_leak_flat_replace() {
        let out = normalize_input(json!({
            "notebook_path": "nb.ipynb",
            "cell_number": 1,
            "new_source": "print(99)\n",
            "edit_mode": "replace"
        }))
        .unwrap();
        let parsed: NotebookEditInput = serde_json::from_value(out).unwrap();
        assert_eq!(parsed.path, "nb.ipynb");
        assert_eq!(parsed.edits.len(), 1);
        assert_eq!(parsed.edits[0].cell_index, 1);
        assert_eq!(parsed.edits[0].new_string, "print(99)\n");
        assert_eq!(parsed.edits[0].edit_mode, CellEditMode::Replace);
    }

    #[tokio::test]
    async fn rejects_non_ipynb_extension() {
        let tmp = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        tokio::fs::write(root.join("x.json"), "{}").await.unwrap();
        let ctx = make_ctx(&root, true, false);
        let reg = ToolRegistry::with_simple_tools();
        let err = reg
            .execute_named(
                "notebook_edit",
                &ctx,
                json!({
                    "path": "x.json",
                    "edits": [{ "cell_index": 0, "old_string": "a", "new_string": "b" }],
                }),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidArgs(_)));
    }
}
