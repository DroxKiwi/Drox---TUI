//! Carte structure workspace persistante (§2.23) — `.drox/workspace-map.json`.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use camino::{Utf8Path, Utf8PathBuf};
use chrono::Utc;
use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::drox_ignore::DroxIgnoreMatcher;
use crate::error::SessionError;

const MAP_FILE: &str = "workspace-map.json";
const MAP_VERSION: u32 = 1;
const MAX_NODES: usize = 120;
const MAX_CHILDREN_PER_NODE: usize = 15;
const MAX_PIVOTS_PER_NODE: usize = 8;
const MAX_SUMMARY_LEN: usize = 200;
const SCAN_MAX_DEPTH: usize = 2;
const SCAN_MAX_ENTRIES: usize = 80;
const PROMPT_MAX_LINES: usize = 28;

/// Nœud de la carte (répertoire ou fichier pivot).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MapNode {
    /// Chemin relatif POSIX sous le workspace (`""` = racine logique).
    pub path: String,
    /// `dir` ou `file`.
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default)]
    pub explored: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pivots: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<String>,
}

/// Artefact JSON persisté.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceMapV1 {
    pub version: u32,
    pub workspace_fingerprint: String,
    pub updated_at: String,
    pub stale: bool,
    pub nodes: Vec<MapNode>,
}

impl Default for WorkspaceMapV1 {
    fn default() -> Self {
        Self {
            version: MAP_VERSION,
            workspace_fingerprint: String::new(),
            updated_at: Utc::now().to_rfc3339(),
            stale: false,
            nodes: Vec::new(),
        }
    }
}

/// Store thread-safe + persistance disque pour un run / workspace.
#[derive(Clone)]
pub struct WorkspaceMapStore {
    inner: Arc<Mutex<WorkspaceMapV1>>,
    workspace: Utf8PathBuf,
    map_path: Utf8PathBuf,
    drox_ignore: Option<DroxIgnoreMatcher>,
}

impl WorkspaceMapStore {
    /// Charge la carte ou en crée une (scan léger si absente / empreinte différente).
    pub async fn load_or_create(
        workspace: Utf8PathBuf,
        fingerprint: String,
        drox_ignore: Option<DroxIgnoreMatcher>,
    ) -> Result<Self, SessionError> {
        let drox_dir = workspace.join(".drox");
        tokio::fs::create_dir_all(drox_dir.as_std_path()).await?;
        let map_path = drox_dir.join(MAP_FILE);

        let mut map = match tokio::fs::read_to_string(map_path.as_std_path()).await {
            Ok(raw) => serde_json::from_str::<WorkspaceMapV1>(&raw).unwrap_or_default(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => WorkspaceMapV1::default(),
            Err(e) => return Err(e.into()),
        };

        if map.workspace_fingerprint != fingerprint {
            map.stale = true;
            map.workspace_fingerprint = fingerprint.clone();
        }

        let needs_scan = map.nodes.is_empty() || map.version != MAP_VERSION;
        if needs_scan {
            map.version = MAP_VERSION;
            map.workspace_fingerprint = fingerprint.clone();
            map.nodes = initial_snapshot(&workspace, drox_ignore.as_ref());
            map.stale = false;
            map.updated_at = Utc::now().to_rfc3339();
        }

        let store = Self {
            inner: Arc::new(Mutex::new(map)),
            workspace: workspace.clone(),
            map_path,
            drox_ignore,
        };
        store.save().await?;
        Ok(store)
    }

    #[must_use]
    pub fn workspace(&self) -> &Utf8Path {
        &self.workspace
    }

    #[must_use]
    pub fn snapshot(&self) -> WorkspaceMapV1 {
        self.inner
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default()
    }

    /// Note sémantique sur un nœud (`path` vide = racine `.`).
    pub fn note(&self, path: Option<&str>, summary: String) -> Result<MapNode, String> {
        let summary = summary.trim().to_string();
        if summary.is_empty() {
            return Err("summary must be non-empty".into());
        }
        if summary.chars().count() > MAX_SUMMARY_LEN {
            return Err(format!(
                "summary too long (max {MAX_SUMMARY_LEN} chars)"
            ));
        }
        let rel = normalize_rel_path(path.unwrap_or(""), &self.workspace)?;
        let mut guard = self.inner.lock().expect("workspace_map mutex poisoned");
        let kind = if rel.is_empty() { "dir" } else { infer_kind(&rel) };
        let node = upsert_node(&mut guard.nodes, &rel, kind);
        node.summary = Some(summary);
        node.explored = true;
        let out = node.clone();
        guard.updated_at = Utc::now().to_rfc3339();
        guard.stale = false;
        Ok(out)
    }

    /// Fusionne le résultat d'un `glob` réussi.
    pub fn ingest_glob(&self, output: &Value) {
        let files: Vec<String> = output
            .get("files")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        let directories: Vec<String> = output
            .get("directories")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        let mut guard = self.inner.lock().expect("workspace_map mutex poisoned");
        for d in directories {
            if self.is_path_ignored_for_map(&d) {
                continue;
            }
            if let Ok(rel) = normalize_rel_path(&d, &self.workspace) {
                let node = upsert_node(&mut guard.nodes, &rel, "dir");
                node.explored = true;
                mark_ancestors_explored(&mut guard.nodes, &rel);
            }
        }
        for f in files {
            if self.is_path_ignored_for_map(&f) {
                continue;
            }
            if let Ok(rel) = normalize_rel_path(&f, &self.workspace) {
                let node = upsert_node(&mut guard.nodes, &rel, "file");
                node.explored = true;
                push_pivot(&mut guard.nodes, &rel);
                mark_ancestors_explored(&mut guard.nodes, &rel);
            }
        }
        guard.updated_at = Utc::now().to_rfc3339();
    }

    /// Marque un fichier lu comme pivot.
    pub fn ingest_file_read(&self, output: &Value) {
        let Some(path) = output.get("path").and_then(|v| v.as_str()) else {
            return;
        };
        if self.is_path_ignored_for_map(path) {
            return;
        }
        let Ok(rel) = normalize_rel_path(path, &self.workspace) else {
            return;
        };
        let mut guard = self.inner.lock().expect("workspace_map mutex poisoned");
        let node = upsert_node(&mut guard.nodes, &rel, "file");
        node.explored = true;
        push_pivot(&mut guard.nodes, &rel);
        mark_ancestors_explored(&mut guard.nodes, &rel);
        guard.updated_at = Utc::now().to_rfc3339();
    }

    /// Marque un fichier cible LSP (diagnostics, etc.).
    pub fn ingest_lsp(&self, output: &Value) {
        let path = output
            .get("path")
            .or_else(|| output.get("file"))
            .and_then(|v| v.as_str());
        let Some(path) = path else {
            return;
        };
        if self.is_path_ignored_for_map(path) {
            return;
        }
        let Ok(rel) = normalize_rel_path(path, &self.workspace) else {
            return;
        };
        let mut guard = self.inner.lock().expect("workspace_map mutex poisoned");
        let node = upsert_node(&mut guard.nodes, &rel, "file");
        node.explored = true;
        push_pivot(&mut guard.nodes, &rel);
        mark_ancestors_explored(&mut guard.nodes, &rel);
        guard.updated_at = Utc::now().to_rfc3339();
    }

    fn is_path_ignored_for_map(&self, path: &str) -> bool {
        self.drox_ignore
            .as_ref()
            .is_some_and(|m| m.is_ignored(Path::new(path)))
    }

    pub async fn save(&self) -> Result<(), SessionError> {
        let json = {
            let guard = self.inner.lock().expect("workspace_map mutex poisoned");
            serde_json::to_string_pretty(&*guard)?
        };
        tokio::fs::write(self.map_path.as_std_path(), json).await?;
        Ok(())
    }

    pub async fn save_if_dirty(&self) {
        if let Err(e) = self.save().await {
            tracing::warn!(error = %e, path = %self.map_path, "workspace_map persist failed");
        }
    }

    /// Extrait compact pour le system prompt (≤ ~28 lignes).
    #[must_use]
    pub fn format_for_prompt(&self) -> Option<String> {
        let guard = self.inner.lock().ok()?;
        format_workspace_map_for_prompt(&guard)
    }
}

/// Scan initial : 1–2 niveaux, respect `.gitignore` + `.droxignore`.
#[must_use]
pub fn initial_snapshot(workspace: &Utf8Path, drox_ignore: Option<&DroxIgnoreMatcher>) -> Vec<MapNode> {
    let mut nodes: Vec<MapNode> = Vec::new();
    let mut seen = HashMap::new();

    let mut binding = WalkBuilder::new(workspace.as_std_path());
    let mut walker = binding
        .hidden(false)
        .max_depth(Some(SCAN_MAX_DEPTH));
    if let Some(drox) = drox_ignore {
        drox.configure_walk(&mut walker);
    } else {
        walker.git_ignore(true);
    }
    let walker = walker.build();

    for entry in walker.flatten() {
        if nodes.len() >= SCAN_MAX_ENTRIES {
            break;
        }
        let path = entry.path();
        let Ok(rel) = path.strip_prefix(workspace.as_std_path()) else {
            continue;
        };
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        if rel_str.is_empty() {
            continue;
        }
        if drox_ignore.is_some_and(|d| d.is_ignored(path)) {
            continue;
        }
        let kind = if entry.file_type().is_some_and(|t| t.is_dir()) {
            "dir"
        } else {
            "file"
        };
        if seen.insert(rel_str.clone(), ()).is_some() {
            continue;
        }
        let depth = rel_str.matches('/').count();
        let is_pivot_file = kind == "file" && is_notable_root_file(Path::new(&rel_str));
        nodes.push(MapNode {
            path: rel_str,
            kind: kind.to_string(),
            summary: None,
            explored: kind == "dir" && depth == 0,
            pivots: Vec::new(),
            children: Vec::new(),
        });
        if is_pivot_file {
            if let Some(last) = nodes.last_mut() {
                last.explored = true;
            }
        }
    }

    attach_children_lists(&mut nodes);
    nodes.sort_by(|a, b| a.path.cmp(&b.path));
    if nodes.len() > MAX_NODES {
        nodes.truncate(MAX_NODES);
    }
    nodes
}

/// Rendu texte injecté au `agent.run`.
#[must_use]
pub fn format_workspace_map_for_prompt(map: &WorkspaceMapV1) -> Option<String> {
    if map.nodes.is_empty() {
        return None;
    }
    let mut lines = Vec::new();
    if map.stale {
        lines.push(
            "[Workspace map] (stale — empreinte changée : explore les zones touchées, \
             mets à jour via workspace_map_note)"
            .to_string(),
        );
    } else {
        lines.push(
            "[Workspace map] (fresh — ne refais pas un inventaire racine complet ; \
             cible les pivots listés ou workspace_map_read pour le détail)"
                .to_string(),
        );
    }

    let mut candidates: Vec<&MapNode> = map
        .nodes
        .iter()
        .filter(|n| n.path.matches('/').count() <= 1 || n.explored || !n.pivots.is_empty())
        .collect();
    candidates.sort_by(|a, b| {
        let score_a = (a.explored as u8) + (a.pivots.len() as u8) * 2 + (!a.summary.is_none() as u8);
        let score_b = (b.explored as u8) + (b.pivots.len() as u8) * 2 + (!b.summary.is_none() as u8);
        score_b.cmp(&score_a).then_with(|| a.path.cmp(&b.path))
    });

    for node in candidates.into_iter().take(PROMPT_MAX_LINES) {
        let label = if node.path.is_empty() { "." } else { &node.path };
        let mut parts = vec![format!("· {label}")];
        if node.kind == "dir" {
            parts.push("(dir)".into());
        }
        if let Some(s) = &node.summary {
            parts.push(format!("— {s}"));
        } else if node.explored && node.kind == "dir" {
            parts.push("— exploré".into());
        }
        if !node.pivots.is_empty() {
            let pivots: Vec<_> = node.pivots.iter().take(3).cloned().collect();
            parts.push(format!("pivots: {}", pivots.join(", ")));
        }
        if !node.children.is_empty() {
            let kids: Vec<_> = node.children.iter().take(4).cloned().collect();
            parts.push(format!("[{}{}]", kids.join(", "), if node.children.len() > 4 { "…" } else { "" }));
        }
        lines.push(parts.join(" "));
    }
    Some(lines.join("\n"))
}

fn attach_children_lists(nodes: &mut [MapNode]) {
    let paths: Vec<String> = nodes.iter().map(|n| n.path.clone()).collect();
    for i in 0..nodes.len() {
        let parent = nodes[i].path.clone();
        if nodes[i].kind != "dir" {
            continue;
        }
        let prefix = if parent.is_empty() {
            String::new()
        } else {
            format!("{parent}/")
        };
        let mut kids: Vec<String> = paths
            .iter()
            .filter(|p| {
                p.starts_with(&prefix)
                    && !p.is_empty()
                    && p.len() > prefix.len()
                    && !p[prefix.len()..].contains('/')
            })
            .cloned()
            .collect();
        kids.sort();
        if kids.len() > MAX_CHILDREN_PER_NODE {
            kids.truncate(MAX_CHILDREN_PER_NODE);
        }
        nodes[i].children = kids;
    }
}

fn is_notable_root_file(path: &Path) -> bool {
    let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
    matches!(
        name.to_ascii_lowercase().as_str(),
        "readme.md" | "readme" | "package.json" | "cargo.toml" | "pyproject.toml" | "go.mod"
    ) || name.eq_ignore_ascii_case("MEMORY.md")
        || name.eq_ignore_ascii_case("DROX.md")
}

fn infer_kind(rel: &str) -> &'static str {
    if rel.is_empty() || rel.ends_with('/') {
        "dir"
    } else if Path::new(rel).extension().is_none() && !rel.contains('.') {
        "dir"
    } else {
        "file"
    }
}

fn normalize_rel_path(path: &str, workspace: &Utf8Path) -> Result<String, String> {
    let trimmed = path.trim().replace('\\', "/");
    if trimmed.is_empty() {
        return Ok(String::new());
    }
    let p = Utf8Path::new(&trimmed);
    let rel = if p.is_absolute() {
        p.strip_prefix(workspace)
            .map_err(|_| format!("path not under workspace: {path}"))?
            .to_string()
    } else {
        trimmed
            .trim_start_matches("./")
            .to_string()
    };
    Ok(rel)
}

fn upsert_node<'a>(nodes: &'a mut Vec<MapNode>, path: &str, kind: &str) -> &'a mut MapNode {
    if let Some(idx) = nodes.iter().position(|n| n.path == path) {
        if nodes[idx].kind != "file" {
            nodes[idx].kind = kind.to_string();
        }
        return &mut nodes[idx];
    }
    if nodes.len() >= MAX_NODES {
        nodes.remove(0);
    }
    nodes.push(MapNode {
        path: path.to_string(),
        kind: kind.to_string(),
        summary: None,
        explored: false,
        pivots: Vec::new(),
        children: Vec::new(),
    });
    let idx = nodes.len() - 1;
    &mut nodes[idx]
}

fn push_pivot(nodes: &mut [MapNode], file_path: &str) {
    let parent = Utf8Path::new(file_path)
        .parent()
        .map(|p| p.as_str().to_string())
        .unwrap_or_default();
    let file_name = Utf8Path::new(file_path)
        .file_name()
        .map(|s| s.to_string())
        .unwrap_or_else(|| file_path.to_string());

    if let Some(node) = nodes.iter_mut().find(|n| n.path == parent) {
        if !node.pivots.iter().any(|p| p == &file_name || p == file_path) {
            if node.pivots.len() < MAX_PIVOTS_PER_NODE {
                node.pivots.push(file_name);
            }
        }
    }
    if let Some(node) = nodes.iter_mut().find(|n| n.path == file_path) {
        node.explored = true;
    }
}

fn mark_ancestors_explored(nodes: &mut [MapNode], rel: &str) {
    let parts: Vec<&str> = rel.split('/').filter(|s| !s.is_empty()).collect();
    for i in 0..parts.len() {
        let ancestor = parts[..i].join("/");
        if let Some(n) = nodes.iter_mut().find(|n| n.path == ancestor) {
            n.explored = true;
        } else if ancestor.is_empty() {
            continue;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn initial_snapshot_lists_top_level() {
        let dir = tempdir().unwrap();
        let root = Utf8Path::from_path(dir.path()).unwrap();
        std::fs::create_dir(root.join("drox").as_std_path()).unwrap();
        std::fs::write(root.join("README.md").as_std_path(), "hi").unwrap();
        let nodes = initial_snapshot(root, None);
        assert!(nodes.iter().any(|n| n.path == "drox"));
        assert!(nodes.iter().any(|n| n.path == "README.md"));
    }

    #[test]
    fn format_prompt_includes_summary() {
        let map = WorkspaceMapV1 {
            nodes: vec![MapNode {
                path: "drox".into(),
                kind: "dir".into(),
                summary: Some("moteur Rust".into()),
                explored: true,
                pivots: vec!["agent.rs".into()],
                children: vec!["crates".into()],
            }],
            ..Default::default()
        };
        let block = format_workspace_map_for_prompt(&map).unwrap();
        assert!(block.contains("moteur Rust"));
        assert!(block.contains("agent.rs"));
    }

    #[tokio::test]
    async fn store_round_trip() {
        let dir = tempdir().unwrap();
        let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        std::fs::create_dir(root.join("src").as_std_path()).unwrap();
        let store =
            WorkspaceMapStore::load_or_create(root.clone(), "/test/fp".into(), None)
                .await
                .unwrap();
        store
            .note(Some("src"), "code applicatif".into())
            .unwrap();
        store.save().await.unwrap();
        let store2 = WorkspaceMapStore::load_or_create(root, "/test/fp".into(), None)
            .await
            .unwrap();
        let snap = store2.snapshot();
        assert!(snap.nodes.iter().any(|n| n.summary.as_deref() == Some("code applicatif")));
    }

    #[test]
    fn ingest_glob_merges_paths() {
        let dir = tempdir().unwrap();
        let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let store = WorkspaceMapStore {
            inner: Arc::new(Mutex::new(WorkspaceMapV1::default())),
            workspace: root.clone(),
            map_path: root.join(".drox").join(MAP_FILE),
            drox_ignore: None,
        };
        store.ingest_glob(&serde_json::json!({
            "files": ["README.md"],
            "directories": ["drox", "extension-vscode"]
        }));
        let snap = store.snapshot();
        assert!(snap.nodes.iter().any(|n| n.path == "drox" && n.explored));
        assert!(snap.nodes.iter().any(|n| n.path == "README.md"));
    }
}
