//! Couche mémoire de session — branche la compaction + la persistance dans
//! la boucle agent.
//!
//! Sprint M1 (architecture mémoire unifiée). Ce module reste **pur orchestration** :
//! il ne touche pas à l'historique en RAM, n'altère pas le contexte, et
//! n'émet rien tant que le run n'est pas accepté comme terminé.
//!
//! Il expose :
//!
//! - [`MemoryRuntime`] : config installée dans `AgentConfig::memory`.
//!   Encapsule le client LLM utilisé pour la compaction, le prompt système
//!   de compaction (cf. `drox-cli/prompts::COMPACTION_PROMPT`), le handle
//!   partagé pour les `session_note`, la conf compaction (température,
//!   tokens), et le workspace de destination.
//! - [`MemoryTracker`] : compteur d'éligibilité maintenu pendant le run.
//!   Le moteur l'incrémente à chaque tool mutateur réussi.
//! - [`persist_run`] : la fonction appelée par l'agent à `[phase: done]`
//!   accepté, qui orchestre la compaction et l'écriture du `.md`.

use std::sync::Arc;

use camino::Utf8PathBuf;
use chrono::Utc;
use drox_llm::LlmClient;
use drox_session::{
    SessionFrontMatter, memory_sessions::reserve_session_path, write_session,
};
use drox_tools::{SessionNote, SessionNotesHandle};
use drox_types::Message;
use tracing::{debug, warn};

use crate::compaction::{CompactionConfig, CompactionResult, summarize_run};
use crate::error::EngineError;

/// Configuration installée par l'appelant pour activer la mémoire de session.
///
/// Cheap to clone (les ressources lourdes sont derrière `Arc`).
#[derive(Clone)]
pub struct MemoryRuntime {
    /// Workspace racine — c'est sous `<root>/.drox/memory/sessions/` que
    /// les `.md` sont écrits.
    pub workspace_root: Utf8PathBuf,
    /// Client LLM utilisé pour le tour de compaction. Peut être le même
    /// client que celui de la boucle agent ou un autre (ex. un modèle plus
    /// petit / plus rapide dédié au résumé).
    pub llm: Arc<dyn LlmClient>,
    /// Prompt système pour la compaction (cf. `drox-cli/prompts::COMPACTION_PROMPT`).
    /// Stocké en `String` plutôt qu'`&'static str` pour autoriser des
    /// surcharges runtime (config utilisateur, A/B testing).
    pub compaction_prompt: String,
    /// Paramètres LLM de la compaction (`temperature`, `max_tokens`, …).
    pub compaction_config: CompactionConfig,
    /// Stock partagé des notes épinglées via `session_note`. Drainé par
    /// [`persist_run`] pour les injecter dans le payload de compaction.
    pub notes: SessionNotesHandle,
    /// Nom du modèle, écrit dans le front-matter pour le diagnostic. Format
    /// libre (ex. `"glm-4.7-flash-q5_K_M"`). Sert uniquement à l'archive,
    /// pas au runtime.
    pub model_label: String,
}

impl std::fmt::Debug for MemoryRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemoryRuntime")
            .field("workspace_root", &self.workspace_root)
            .field("compaction_prompt_len", &self.compaction_prompt.len())
            .field("compaction_config", &self.compaction_config)
            .field("notes_count", &self.notes.len())
            .field("model_label", &self.model_label)
            .finish_non_exhaustive()
    }
}

/// État d'éligibilité du run vis-à-vis de la persistance.
///
/// Un run est **non trivial** (= mérite d'être persisté) s'il a déclenché
/// au moins un de ces signaux :
///
/// - tool **mutateur** (`file_edit`, `file_write`, `notebook_edit`, `delete_path`, `bash`) exécuté avec succès ;
/// - `todo_write` réussi (indique au minimum une intention de travail) ;
/// - au moins une `session_note` épinglée par le modèle.
///
/// Pour les runs purement conversationnels (salut, merci, question triviale)
/// → aucun de ces signaux n'est levé → on n'écrit pas de `.md` (sinon le
/// dossier `sessions/` se remplit de bruit).
#[derive(Debug, Default, Clone, Copy)]
pub struct MemoryTracker {
    mutating_count: u32,
    todo_writes: u32,
    session_notes: u32,
}

impl MemoryTracker {
    /// Nouveau tracker vierge.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            mutating_count: 0,
            todo_writes: 0,
            session_notes: 0,
        }
    }

    /// Appelé par l'agent à chaque tool réussi.
    pub fn record_tool(&mut self, name: &str) {
        match name {
            "file_edit" | "file_write" | "notebook_edit" | "delete_path" | "bash" => self.mutating_count += 1,
            "todo_write" | "course_plan_write" => self.todo_writes += 1,
            "session_note" => self.session_notes += 1,
            _ => {}
        }
    }

    /// Vrai si le run mérite une persistance.
    ///
    /// Borne basse : un `todo_write` seul suffit (le user a posé un plan,
    /// même s'il n'a pas (encore) muté → la session a une valeur d'archive
    /// si elle aboutit à `[phase: done]`).
    #[must_use]
    pub const fn is_non_trivial(&self) -> bool {
        self.mutating_count > 0 || self.todo_writes > 0 || self.session_notes > 0
    }
}

/// Résultat de [`persist_run`].
#[derive(Debug, Clone)]
pub struct PersistedRun {
    /// Slug court (préfixe date exclu) — clé pour `memory_read`.
    pub slug: String,
    /// Chemin absolu du `.md` produit.
    pub path: Utf8PathBuf,
    /// Métadonnées + résumé renvoyés par la compaction.
    pub result: CompactionResult,
}

/// Orchestration : compaction + écriture sur disque.
///
/// `messages` est l'historique complet du run (system, user, et tours
/// assistant/outils). On l'envoie tel quel au modèle de compaction qui en
/// extrait le résumé.
///
/// `objective_fallback` est utilisé pour le slug et le front-matter quand
/// la compaction ne livre pas de section `## Objective` exploitable. En
/// pratique : la première ligne non vide du message user d'origine, ou
/// `"session"` si même ça manque.
///
/// **Échecs tolérés** : si l'écriture disque rate (permission, FS plein,
/// …), on log et on renvoie `Err`. L'agent décide alors s'il propage
/// l'erreur ou la swallow (V1 : swallow + log, on ne casse pas le run
/// pour un problème d'archivage).
pub async fn persist_run(
    runtime: &MemoryRuntime,
    messages: &[Message],
    objective_fallback: &str,
) -> Result<PersistedRun, EngineError> {
    let notes_snapshot: Vec<SessionNote> = runtime.notes.drain();
    debug!(
        notes = notes_snapshot.len(),
        msg_count = messages.len(),
        "memory: starting compaction for persistence"
    );

    let result = summarize_run(
        runtime.llm.as_ref(),
        &runtime.compaction_prompt,
        messages,
        &notes_snapshot,
        &runtime.compaction_config,
    )
    .await?;

    let objective_for_slug = if result.objective.is_empty() {
        objective_fallback
    } else {
        result.objective.as_str()
    };
    let slug = drox_session::slugify(objective_for_slug);
    let now = Utc::now();
    let path = reserve_session_path(&runtime.workspace_root, now, &slug);
    let front = SessionFrontMatter {
        slug: slug.clone(),
        objective: result.objective.clone(),
        date: now,
        model: runtime.model_label.clone(),
        files_touched: result.files_touched.clone(),
    };
    if let Err(e) = write_session(&path, &front, &result.summary).await {
        warn!(error = %e, path = %path, "memory: failed to write session .md");
        return Err(EngineError::Memory(format!(
            "cannot write session {path}: {e}"
        )));
    }
    debug!(slug = %slug, path = %path, "memory: session persisted");
    Ok(PersistedRun { slug, path, result })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tracker_marks_run_non_trivial_after_mutation() {
        let mut t = MemoryTracker::new();
        assert!(!t.is_non_trivial());
        t.record_tool("file_edit");
        assert!(t.is_non_trivial());
    }

    #[test]
    fn tracker_marks_run_non_trivial_after_todo_write_only() {
        let mut t = MemoryTracker::new();
        t.record_tool("todo_write");
        assert!(t.is_non_trivial());
    }

    #[test]
    fn tracker_marks_run_non_trivial_with_session_note() {
        let mut t = MemoryTracker::new();
        t.record_tool("session_note");
        assert!(t.is_non_trivial());
    }

    #[test]
    fn tracker_ignores_read_only_tools() {
        let mut t = MemoryTracker::new();
        for tool in ["glob", "grep", "file_read", "lsp", "web_search"] {
            t.record_tool(tool);
        }
        assert!(!t.is_non_trivial());
    }
}
