//! Tool `delete_path` — supprime un fichier ou un répertoire sous le workspace.
//!
//! Aligné leak `fsOperations.rm` / garde-fous `isDangerousRemovalPath` (cf. §2.33).

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::fs;

use crate::context::ToolContext;
use crate::error::ToolError;
use crate::path_util::{
    dangerous_removal_resolved_path, dangerous_removal_user_path_reason,
    is_protected_workspace_entry, resolve_under_workspace,
};
use crate::tool::Tool;

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeletePathInput {
    /// Chemin relatif au workspace ou absolu **sous** le workspace (fichier ou dossier).
    pub path: String,
    /// Pour un répertoire : `true` (défaut) = suppression récursive ; `false` = uniquement si vide.
    #[serde(default = "default_recursive")]
    pub recursive: bool,
}

const fn default_recursive() -> bool {
    true
}

pub struct DeletePathTool;

fn normalize_input(input: Value) -> Value {
    let Value::Object(mut obj) = input else {
        return input;
    };
    if !obj.contains_key("path") {
        for key in ["file_path", "filePath", "target", "file"] {
            if let Some(v) = obj.remove(key) {
                obj.insert("path".into(), v);
                break;
            }
        }
    }
    if let Some(Value::Bool(b)) = obj.remove("recursive") {
        obj.insert("recursive".into(), Value::Bool(b));
    }
    Value::Object(obj)
}

fn relative_under_workspace(root: &camino::Utf8Path, resolved: &camino::Utf8Path) -> Option<String> {
    let root_canon = std::fs::canonicalize(root.as_std_path()).ok()?;
    let target_canon = std::fs::canonicalize(resolved.as_std_path()).ok()?;
    let rel = target_canon.strip_prefix(&root_canon).ok()?;
    camino::Utf8PathBuf::from_path_buf(rel.to_path_buf())
        .ok()
        .map(|p| p.as_str().replace('\\', "/"))
}

#[async_trait]
impl Tool for DeletePathTool {
    fn name(&self) -> &str {
        "delete_path"
    }

    fn description(&self) -> &str {
        "Supprime un fichier ou un répertoire strictement sous le workspace. \
         Préfère cet outil à `bash rm -rf` (chemins, quoting Windows). \
         Format : {\"path\":\"chemin/relatif\",\"recursive\"?:true}. \
         `recursive:false` sur un dossier ne supprime que s'il est vide."
    }

    fn input_schema(&self) -> Value {
        serde_json::to_value(schemars::schema_for!(DeletePathInput)).unwrap_or(Value::Null)
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<Value, ToolError> {
        let normalized = normalize_input(input);
        let args: DeletePathInput = serde_json::from_value(normalized).map_err(|e| {
            ToolError::invalid_args(format!(
                "delete_path: JSON invalide ({e}). Attendu : {{\"path\":\"…\",\"recursive\"?:true}}.",
            ))
        })?;

        if let Some(reason) = dangerous_removal_user_path_reason(&args.path) {
            return Err(ToolError::invalid_args(format!("delete_path: {reason}")));
        }

        if ctx.plan_mode {
            return Err(ToolError::plan_violation("delete_path"));
        }

        let ws = ctx.effective_workspace();
        let resolved = resolve_under_workspace(&ws, &args.path)?;

        if dangerous_removal_resolved_path(&resolved) {
            return Err(ToolError::invalid_args(
                "delete_path: chemin système sensible — suppression refusée.",
            ));
        }

        let root_canon = std::fs::canonicalize(ws.as_std_path())
            .map_err(|e| ToolError::io(ws.to_owned(), e))?;
        if resolved.as_std_path() == root_canon.as_path() {
            return Err(ToolError::invalid_args(
                "delete_path: suppression de la racine du workspace interdite.",
            ));
        }

        if let Some(rel) = relative_under_workspace(&ws, &resolved) {
            if is_protected_workspace_entry(&rel) {
                return Err(ToolError::invalid_args(format!(
                    "delete_path: suppression de `{rel}` interdite (répertoire sensible du projet).",
                )));
            }
        }

        let meta = fs::metadata(&resolved)
            .await
            .map_err(|e| ToolError::io(resolved.clone(), e))?;
        let file_type = meta.file_type();
        let kind = if file_type.is_symlink() {
            "symlink"
        } else if file_type.is_dir() {
            "directory"
        } else {
            "file"
        };

        if ctx.apply_fs_writes {
            if file_type.is_symlink() || file_type.is_file() {
                fs::remove_file(&resolved)
                    .await
                    .map_err(|e| ToolError::io(resolved.clone(), e))?;
            } else if file_type.is_dir() {
                if args.recursive {
                    fs::remove_dir_all(&resolved)
                        .await
                        .map_err(|e| ToolError::io(resolved.clone(), e))?;
                } else {
                    fs::remove_dir(&resolved)
                        .await
                        .map_err(|e| ToolError::io(resolved.clone(), e))?;
                }
            } else {
                return Err(ToolError::invalid_args(
                    "delete_path: type de fichier non pris en charge pour cette cible.",
                ));
            }
            Ok(json!({
                "deleted": true,
                "path": resolved.as_str(),
                "kind": kind,
                "recursive": args.recursive,
            }))
        } else {
            Ok(json!({
                "deleted": false,
                "proposed": true,
                "path": resolved.as_str(),
                "kind": kind,
                "recursive": args.recursive,
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
    async fn propose_mode_does_not_delete() {
        let tmp = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        let f = root.join("keep.txt");
        tokio::fs::write(&f, b"x").await.unwrap();
        let ctx = ToolContext::new(root.clone(), false);
        let reg = ToolRegistry::with_simple_tools();
        let out = reg
            .execute_named(
                "delete_path",
                &ctx,
                json!({ "path": "keep.txt" }),
            )
            .await
            .unwrap();
        assert_eq!(out["deleted"], false);
        assert_eq!(out["proposed"], true);
        assert!(f.exists());
    }

    #[tokio::test]
    async fn apply_mode_deletes_file() {
        let tmp = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        let f = root.join("gone.txt");
        tokio::fs::write(&f, b"x").await.unwrap();
        let ctx = ToolContext::new(root.clone(), true);
        let reg = ToolRegistry::with_simple_tools();
        let out = reg
            .execute_named(
                "delete_path",
                &ctx,
                json!({ "path": "gone.txt" }),
            )
            .await
            .unwrap();
        assert_eq!(out["deleted"], true);
        assert_eq!(out["kind"], "file");
        assert!(!f.exists());
    }

    #[tokio::test]
    async fn apply_mode_deletes_directory_recursive() {
        let tmp = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        let d = root.join("tree").join("nested");
        tokio::fs::create_dir_all(&d).await.unwrap();
        tokio::fs::write(d.join("a.txt"), b"a").await.unwrap();
        let ctx = ToolContext::new(root.clone(), true);
        let reg = ToolRegistry::with_simple_tools();
        let out = reg
            .execute_named(
                "delete_path",
                &ctx,
                json!({ "path": "tree" }),
            )
            .await
            .unwrap();
        assert_eq!(out["deleted"], true);
        assert_eq!(out["kind"], "directory");
        assert!(!root.join("tree").exists());
    }

    #[tokio::test]
    async fn non_recursive_rejects_nonempty_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        let d = root.join("tree");
        tokio::fs::create_dir_all(&d).await.unwrap();
        tokio::fs::write(d.join("a.txt"), b"a").await.unwrap();
        let ctx = ToolContext::new(root.clone(), true);
        let reg = ToolRegistry::with_simple_tools();
        let err = reg
            .execute_named(
                "delete_path",
                &ctx,
                json!({ "path": "tree", "recursive": false }),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::Io { .. }), "got {err:?}");
        assert!(d.exists());
    }

    #[tokio::test]
    async fn non_recursive_deletes_empty_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        let d = root.join("empty");
        tokio::fs::create_dir_all(&d).await.unwrap();
        let ctx = ToolContext::new(root.clone(), true);
        let reg = ToolRegistry::with_simple_tools();
        reg.execute_named(
            "delete_path",
            &ctx,
            json!({ "path": "empty", "recursive": false }),
        )
        .await
        .unwrap();
        assert!(!d.exists());
    }

    #[tokio::test]
    async fn rejects_workspace_root() {
        let tmp = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        tokio::fs::write(root.join("marker.txt"), b"x").await.unwrap();
        let ctx = ToolContext::new(root.clone(), true);
        let reg = ToolRegistry::with_simple_tools();
        let err = reg
            .execute_named("delete_path", &ctx, json!({ "path": "." }))
            .await
            .unwrap_err();
        assert!(
            matches!(err, ToolError::InvalidArgs(ref m) if m.contains("racine")),
            "got {err:?}"
        );
    }

    #[tokio::test]
    async fn rejects_glob_path() {
        let tmp = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        let ctx = ToolContext::new(root, true);
        let reg = ToolRegistry::with_simple_tools();
        let err = reg
            .execute_named("delete_path", &ctx, json!({ "path": "src/*" }))
            .await
            .unwrap_err();
        assert!(
            matches!(err, ToolError::InvalidArgs(ref m) if m.contains("wildcard")),
            "got {err:?}"
        );
    }

    #[tokio::test]
    async fn rejects_dot_git_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        tokio::fs::create_dir_all(root.join(".git")).await.unwrap();
        let ctx = ToolContext::new(root, true);
        let reg = ToolRegistry::with_simple_tools();
        let err = reg
            .execute_named("delete_path", &ctx, json!({ "path": ".git" }))
            .await
            .unwrap_err();
        assert!(
            matches!(err, ToolError::InvalidArgs(ref m) if m.contains(".git")),
            "got {err:?}"
        );
    }

    #[tokio::test]
    async fn normalizes_file_path_alias() {
        let tmp = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        let f = root.join("gone.txt");
        tokio::fs::write(&f, b"x").await.unwrap();
        let ctx = ToolContext::new(root, true);
        let reg = ToolRegistry::with_simple_tools();
        reg.execute_named(
            "delete_path",
            &ctx,
            json!({ "file_path": "gone.txt" }),
        )
        .await
        .unwrap();
        assert!(!f.exists());
    }

    #[tokio::test]
    async fn errors_when_path_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        let ctx = ToolContext::new(root, true);
        let reg = ToolRegistry::with_simple_tools();
        let err = reg
            .execute_named(
                "delete_path",
                &ctx,
                json!({ "path": "nope.txt" }),
            )
            .await
            .unwrap_err();
        assert!(
            matches!(err, ToolError::Io { .. } | ToolError::InvalidArgs { .. }),
            "got {err:?}"
        );
    }
}
