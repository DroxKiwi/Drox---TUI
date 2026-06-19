//! Tool `glob` — liste les chemins correspondant à un motif glob.
//!
//! L'output expose deux champs : `files` et `directories`. Les deux sont
//! peuplés selon le motif :
//!
//! - `*` à la racine → quelques fichiers + tous les sous-dossiers de premier
//!   niveau (cas d'usage : « qu'y a-t-il dans ce repo ? »).
//! - `*.rs` → uniquement des fichiers `.rs` (dossiers ignorés car ne matchent
//!   pas l'extension).
//! - `**/*` → fichiers + dossiers récursivement, bornés à `MAX_ENTRIES` au
//!   total ; puis **plafond par dossier parent** (`MAX_CHILDREN_PER_PARENT`) :
//!   si un même répertoire a trop d'enfants directs dans le résultat, seuls
//!   les premiers (tri lexicographique) sont listés et le reste est signalé
//!   dans `directory_fanout_caps` (indépendamment du nom du dossier :
//!   `node_modules`, `vendor`, `target`, etc.).
//!
//! Sans la séparation `files` / `directories`, des projets multi-crates /
//! multi-packages ressemblent à un dossier vide avec un seul `README.md`
//! parce que les sous-dossiers sont filtrés silencieusement.

use std::collections::HashMap;

use async_trait::async_trait;
use camino::Utf8Path;
use glob::glob;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::context::ToolContext;
use crate::error::ToolError;
use crate::path_util::resolve_under_workspace;
use crate::tool::Tool;

/// Plafond cumulé sur `files` + `directories` pendant le scan (garde-fou dur).
const MAX_ENTRIES: usize = 10_000;

/// Nombre max d'entrées **par dossier parent** (fichiers + sous-dossiers
/// immédiats) dans la sortie finale. Au-delà, troncature + entrée dans
/// `directory_fanout_caps`.
const MAX_CHILDREN_PER_PARENT: usize = 20;

/// Regroupe les chemins par parent immédiat, trie chaque groupe, et si un
/// parent a plus de `max_per_parent` enfants dans le résultat, ne garde que
/// les `max_per_parent` premiers et enregistre une note explicite.
fn apply_fanout_limit(
    files: &mut Vec<String>,
    directories: &mut Vec<String>,
    max_per_parent: usize,
) -> Vec<Value> {
    if max_per_parent == 0 {
        return Vec::new();
    }

    let mut by_parent: HashMap<String, Vec<(String, bool)>> = HashMap::new();

    for p in files.iter() {
        let parent = Utf8Path::new(p)
            .parent()
            .map(|x| x.as_str().to_string())
            .unwrap_or_default();
        by_parent.entry(parent).or_default().push((p.clone(), false));
    }
    for p in directories.iter() {
        let parent = Utf8Path::new(p)
            .parent()
            .map(|x| x.as_str().to_string())
            .unwrap_or_default();
        by_parent.entry(parent).or_default().push((p.clone(), true));
    }

    let mut new_files = Vec::new();
    let mut new_dirs = Vec::new();
    let mut notes = Vec::new();

    let mut parents: Vec<String> = by_parent.keys().cloned().collect();
    parents.sort();

    for parent in parents {
        let mut group = by_parent.remove(&parent).unwrap_or_default();
        group.sort_by(|a, b| a.0.cmp(&b.0));
        let total = group.len();
        if total <= max_per_parent {
            for (path, is_dir) in group {
                if is_dir {
                    new_dirs.push(path);
                } else {
                    new_files.push(path);
                }
            }
        } else {
            for (path, is_dir) in group.into_iter().take(max_per_parent) {
                if is_dir {
                    new_dirs.push(path);
                } else {
                    new_files.push(path);
                }
            }
            let omitted = total - max_per_parent;
            let parent_display = if parent.is_empty() {
                "(parent indéterminé)"
            } else {
                parent.as_str()
            };
            let hint = format!(
                "Sous «{parent_display}», {total} entrées (fichiers + dossiers) correspondaient au motif ; \
                 seules les {max_per_parent} premières (ordre lexicographique) sont listées. \
                 {omitted} entrée(s) supplémentaire(s) non listées — affine le glob (sous-dossier ou motif plus étroit, ex. `*.ts`) pour voir la suite."
            );
            notes.push(json!({
                "parent_directory": parent_display,
                "entries_total": total,
                "entries_listed": max_per_parent,
                "entries_omitted": omitted,
                "hint": hint,
            }));
        }
    }

    *files = new_files;
    *directories = new_dirs;
    files.sort();
    directories.sort();
    notes.sort_by(|a, b| {
        let pa = a.get("parent_directory").and_then(|x| x.as_str()).unwrap_or("");
        let pb = b.get("parent_directory").and_then(|x| x.as_str()).unwrap_or("");
        pa.cmp(pb)
    });
    notes
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GlobInput {
    /// Motif glob (ex: `**/*.rs`, `*.toml`), relatif au répertoire `path`.
    pub pattern: String,
    /// Répertoire de base relatif au workspace (défaut : racine workspace).
    #[serde(default)]
    pub path: Option<String>,
}

pub struct GlobTool;

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str {
        "glob"
    }

    fn description(&self) -> &str {
        "Liste les chemins (fichiers ET dossiers) qui correspondent à un motif glob sous le workspace. \
         Respecte `.gitignore` indirectement via post-filtre **`.droxignore`** (chemins omis dans `droxignore_omitted`). \
         Sortie : `files`, `directories`, `truncated`, `directory_fanout_caps`, `droxignore_omitted`. \
         Utilise `*` pour le premier niveau, `**/*.ext` pour une extension."
    }

    fn input_schema(&self) -> Value {
        serde_json::to_value(schemars::schema_for!(GlobInput)).unwrap_or(Value::Null)
    }

    fn is_read_only(&self) -> bool {
        true
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<Value, ToolError> {
        let args: GlobInput = serde_json::from_value(input)?;

        let ws = ctx.effective_workspace();
        let base = match &args.path {
            None => ws.clone(),
            Some(p) => resolve_under_workspace(&ws, p)?,
        };

        if !base.is_dir() {
            return Err(ToolError::invalid_args("`path` must be a directory"));
        }

        let joined = base.join(&args.pattern);
        let mut pattern_str = joined.as_str().to_owned();
        if pattern_str.contains('\\') {
            pattern_str = pattern_str.replace('\\', "/");
        }

        let mut files = Vec::new();
        let mut directories = Vec::new();
        let mut truncated = false;

        for entry in glob(&pattern_str).map_err(|e| ToolError::Glob(e.to_string()))? {
            let path = entry.map_err(|e| ToolError::Glob(e.to_string()))?;
            let utf = camino::Utf8PathBuf::from_path_buf(path)
                .map_err(|_| ToolError::invalid_args("matched path is not UTF-8"))?;
            if utf.is_file() {
                files.push(utf.to_string());
            } else if utf.is_dir() {
                directories.push(utf.to_string());
            }
            if files.len() + directories.len() >= MAX_ENTRIES {
                truncated = true;
                break;
            }
        }

        files.sort();
        directories.sort();

        let fanout_caps = apply_fanout_limit(&mut files, &mut directories, MAX_CHILDREN_PER_PARENT);

        let (files, directories, drox_omitted, drox_sample) =
            if let Some(ref ignore) = ctx.drox_ignore {
                let (f, n1, mut s1) = ignore.filter_paths(files);
                let (d, n2, s2) = ignore.filter_paths(directories);
                s1.extend(s2);
                (f, d, n1 + n2, s1)
            } else {
                (files, directories, 0, Vec::new())
            };

        Ok(json!({
            "files": files,
            "directories": directories,
            "truncated": truncated,
            "directory_fanout_caps": fanout_caps,
            "droxignore_omitted": drox_omitted,
            "droxignore_omitted_sample": drox_sample,
        }))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::registry::ToolRegistry;

    #[tokio::test]
    async fn lists_rs_files() {
        let tmp = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        tokio::fs::write(root.join("a.rs"), "").await.unwrap();
        tokio::fs::write(root.join("b.txt"), "").await.unwrap();
        let ctx = ToolContext::new(root.clone(), false);
        let reg = ToolRegistry::with_simple_tools();
        let out = reg
            .execute_named("glob", &ctx, json!({ "pattern": "*.rs" }))
            .await
            .unwrap();
        let files = out["files"].as_array().unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0].as_str().unwrap().ends_with("a.rs"));
        // Pattern `*.rs` : aucun dossier ne devrait matcher.
        assert_eq!(out["directories"].as_array().unwrap().len(), 0);
        assert_eq!(
            out["directory_fanout_caps"].as_array().unwrap().len(),
            0
        );
    }

    #[tokio::test]
    async fn star_pattern_lists_files_and_directories() {
        // Régression : `glob *` à la racine doit montrer les sous-dossiers,
        // pas seulement le fichier README. Sans ça, un modèle conclut à
        // tort « le workspace ne contient qu'un README ».
        let tmp = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        tokio::fs::create_dir(root.join("crate-a")).await.unwrap();
        tokio::fs::create_dir(root.join("crate-b")).await.unwrap();
        tokio::fs::write(root.join("README.md"), "").await.unwrap();
        let ctx = ToolContext::new(root.clone(), false);
        let reg = ToolRegistry::with_simple_tools();
        let out = reg
            .execute_named("glob", &ctx, json!({ "pattern": "*" }))
            .await
            .unwrap();
        let files = out["files"].as_array().unwrap();
        let dirs = out["directories"].as_array().unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0].as_str().unwrap().ends_with("README.md"));
        assert_eq!(dirs.len(), 2);
        assert!(dirs.iter().any(|d| d.as_str().unwrap().ends_with("crate-a")));
        assert!(dirs.iter().any(|d| d.as_str().unwrap().ends_with("crate-b")));
        assert_eq!(
            out["directory_fanout_caps"].as_array().unwrap().len(),
            0
        );
    }

    #[tokio::test]
    async fn fanout_caps_when_single_parent_has_many_children() {
        let tmp = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        let heavy = root.join("heavy");
        tokio::fs::create_dir(&heavy).await.unwrap();
        for i in 0..25 {
            tokio::fs::write(heavy.join(format!("f{i}.txt")), "")
                .await
                .unwrap();
        }
        let ctx = ToolContext::new(root.clone(), false);
        let reg = ToolRegistry::with_simple_tools();
        let out = reg
            .execute_named("glob", &ctx, json!({ "pattern": "heavy/*.txt" }))
            .await
            .unwrap();
        let files = out["files"].as_array().unwrap();
        assert_eq!(files.len(), MAX_CHILDREN_PER_PARENT);
        let caps = out["directory_fanout_caps"].as_array().unwrap();
        assert_eq!(caps.len(), 1);
        assert_eq!(caps[0]["entries_total"], 25);
        assert_eq!(caps[0]["entries_listed"], MAX_CHILDREN_PER_PARENT);
        assert_eq!(caps[0]["entries_omitted"], 5);
        assert!(caps[0]["hint"].as_str().unwrap().contains("non listées"));
    }

    #[tokio::test]
    async fn fanout_lists_all_when_at_or_below_cap() {
        let tmp = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        let d = root.join("ok");
        tokio::fs::create_dir(&d).await.unwrap();
        for i in 0..20 {
            tokio::fs::write(d.join(format!("x{i}.txt")), "").await.unwrap();
        }
        let ctx = ToolContext::new(root.clone(), false);
        let reg = ToolRegistry::with_simple_tools();
        let out = reg
            .execute_named("glob", &ctx, json!({ "pattern": "ok/*.txt" }))
            .await
            .unwrap();
        assert_eq!(out["files"].as_array().unwrap().len(), 20);
        assert_eq!(
            out["directory_fanout_caps"].as_array().unwrap().len(),
            0
        );
    }

    #[tokio::test]
    async fn droxignore_omits_env_from_glob() {
        let tmp = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        tokio::fs::write(root.join(".env"), "SECRET=1").await.unwrap();
        tokio::fs::write(root.join("ok.txt"), "").await.unwrap();
        let ignore = drox_session::DroxIgnoreMatcher::load_or_create(root.clone())
            .await
            .unwrap();
        let ctx = ToolContext::new(root.clone(), false).with_drox_ignore(ignore);
        let reg = ToolRegistry::with_simple_tools();
        let out = reg
            .execute_named("glob", &ctx, json!({ "pattern": "**/*" }))
            .await
            .unwrap();
        let files = out["files"].as_array().unwrap();
        assert!(
            files
                .iter()
                .all(|f| !f.as_str().unwrap().replace('\\', "/").ends_with(".env")),
            "`.env` must be omitted by .droxignore"
        );
        assert!(files.iter().any(|f| f.as_str().unwrap().ends_with("ok.txt")));
        assert!(out["droxignore_omitted"].as_u64().unwrap_or(0) >= 1);
    }

    #[tokio::test]
    async fn recursive_glob_lists_node_modules_child_with_fanout_note() {
        let tmp = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        tokio::fs::create_dir_all(root.join("src")).await.unwrap();
        tokio::fs::write(root.join("src/lib.rs"), "").await.unwrap();
        let nm = root.join("node_modules/pkg");
        tokio::fs::create_dir_all(&nm).await.unwrap();
        for i in 0..22 {
            tokio::fs::write(nm.join(format!("m{i}.js")), "").await.unwrap();
        }
        let ctx = ToolContext::new(root.clone(), false);
        let reg = ToolRegistry::with_simple_tools();
        let out = reg
            .execute_named("glob", &ctx, json!({ "pattern": "**/*" }))
            .await
            .unwrap();
        let files = out["files"].as_array().unwrap();
        assert!(files.iter().any(|f| f.as_str().unwrap().ends_with("lib.rs")));
        let norm = |s: &str| s.replace('\\', "/");
        let under_pkg = files
            .iter()
            .filter(|f| norm(f.as_str().unwrap()).contains("node_modules/pkg"))
            .count();
        assert_eq!(under_pkg, MAX_CHILDREN_PER_PARENT);
        let caps = out["directory_fanout_caps"].as_array().unwrap();
        assert!(caps.iter().any(|c| {
            norm(c["parent_directory"].as_str().unwrap()).contains("node_modules")
        }));
    }
}
