//! Tool `memory_list` — liste les sessions archivées du workspace.
//!
//! Sprint M1. Le moteur injecte déjà un listing court (≤ 10 dernières
//! sessions) en début de system prompt à chaque run. Ce tool sert aux
//! cas où le modèle veut explicitement re-scanner — par exemple à
//! mi-conversation, après avoir épuisé les 10 premières, ou pour
//! filtrer / paginer.
//!
//! Read-only. Renvoie une liste structurée (slug, date, objectif, fichiers
//! touchés). Pas de body — pour ça il faut appeler `memory_read`.

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use drox_session::memory_sessions;

use crate::context::ToolContext;
use crate::error::ToolError;
use crate::tool::Tool;

/// Limite par défaut pour rester compatible avec la limite du listing
/// injecté au prompt. Le modèle peut demander plus en passant `limit`.
const DEFAULT_LIMIT: usize = 10;
/// Plafond dur — au-delà, le modèle scrolle pour rien et bouffe son
/// contexte.
const MAX_LIMIT: usize = 50;

/// Payload reçu par le tool `memory_list`.
#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct MemoryListInput {
    /// Nombre max d'entrées retournées (1..=50). Défaut : 10.
    #[serde(default)]
    pub limit: Option<usize>,
}

/// Tool local `memory_list`.
pub struct MemoryListTool;

#[async_trait]
impl Tool for MemoryListTool {
    fn name(&self) -> &str {
        "memory_list"
    }

    fn description(&self) -> &str {
        "Liste les sessions archivées du workspace (`.drox/memory/sessions/`), \
         triées du plus récent au plus ancien. Lecture seule. Renvoie \
         {slug, date, objective, files_touched, model} pour chaque entrée, \
         pas le body — utilise `memory_read` pour recharger une session \
         précise. Format : {\"limit\": 10} (optionnel, défaut 10, max 50)."
    }

    fn input_schema(&self) -> Value {
        serde_json::to_value(schemars::schema_for!(MemoryListInput)).unwrap_or(Value::Null)
    }

    fn is_read_only(&self) -> bool {
        true
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<Value, ToolError> {
        let args: MemoryListInput = if input.is_null() {
            MemoryListInput::default()
        } else {
            serde_json::from_value(input).map_err(|e| {
                ToolError::invalid_args(format!(
                    "memory_list: payload JSON invalide ({e}). Format attendu : \
                     {{\"limit\": 10}} (ou {{}})."
                ))
            })?
        };
        let limit = args.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
        let entries = memory_sessions::load_sessions_listing(&ctx.workspace_root, limit)
            .await
            .map_err(|e| {
                ToolError::invalid_args(format!(
                    "memory_list: cannot list sessions in workspace ({e})",
                ))
            })?;
        let total = entries.len();
        let items: Vec<Value> = entries
            .into_iter()
            .map(|e| {
                json!({
                    "slug": e.slug,
                    "date": e.date.to_rfc3339(),
                    "objective": e.objective,
                    "files_touched": e.files_touched,
                    "model": e.model,
                })
            })
            .collect();
        Ok(json!({
            "sessions": items,
            "count": total,
            "limit": limit,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8PathBuf;
    use chrono::{TimeZone, Utc};
    use drox_session::memory_sessions::{
        SessionFrontMatter, compute_session_path, write_session,
    };
    use tempfile::tempdir;

    async fn seed(ws: &camino::Utf8Path, slug: &str, year: i32, month: u32, day: u32) {
        let date = Utc.with_ymd_and_hms(year, month, day, 12, 0, 0).unwrap();
        let path = compute_session_path(ws, date, slug);
        let front = SessionFrontMatter {
            slug: slug.into(),
            objective: format!("obj {slug}"),
            date,
            model: "test".into(),
            files_touched: vec![format!("src/{slug}.rs")],
        };
        write_session(&path, &front, "").await.unwrap();
    }

    #[tokio::test]
    async fn lists_sessions_in_descending_order() {
        let dir = tempdir().unwrap();
        let ws = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        seed(&ws, "older", 2026, 5, 10).await;
        seed(&ws, "newer", 2026, 5, 13).await;

        let ctx = ToolContext::new(ws, false);
        let out = MemoryListTool.execute(&ctx, json!({})).await.unwrap();
        let sessions = out["sessions"].as_array().unwrap();
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0]["slug"], "newer");
        assert_eq!(sessions[1]["slug"], "older");
        assert_eq!(out["count"], 2);
    }

    #[tokio::test]
    async fn respects_limit_and_clamps_to_max() {
        let dir = tempdir().unwrap();
        let ws = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        for i in 1u32..=5 {
            seed(&ws, &format!("s{i}"), 2026, 5, i).await;
        }
        let ctx = ToolContext::new(ws, false);

        let out = MemoryListTool
            .execute(&ctx, json!({ "limit": 2 }))
            .await
            .unwrap();
        assert_eq!(out["sessions"].as_array().unwrap().len(), 2);

        let out_clamped = MemoryListTool
            .execute(&ctx, json!({ "limit": 9999 }))
            .await
            .unwrap();
        assert_eq!(out_clamped["limit"], MAX_LIMIT);
    }

    #[tokio::test]
    async fn returns_empty_when_no_session() {
        let dir = tempdir().unwrap();
        let ws = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let ctx = ToolContext::new(ws, false);
        let out = MemoryListTool.execute(&ctx, Value::Null).await.unwrap();
        assert_eq!(out["count"], 0);
        assert!(out["sessions"].as_array().unwrap().is_empty());
    }
}
