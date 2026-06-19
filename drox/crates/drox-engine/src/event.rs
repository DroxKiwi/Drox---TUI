//! Événements émis par la boucle agent vers son consommateur.

use drox_tools::ScopeDeferredItem;
use drox_types::{StopReason, ToolUseId, Usage};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::long_memory::ContextChunkSummaryV1;

/// Phase courante du protocole agent (Sprint A — refonte « phases + objectif »).
///
/// Le modèle est invité à structurer ses réponses comme une suite de phases ;
/// chaque transition est annoncée par une ligne `[phase: nom]` au début d'un
/// segment de réponse, parsée et **retirée** du texte assistant par
/// `consume_stream`, puis exposée à l'UI via `AgentEvent::PhaseEnter`.
///
/// L'ordre indicatif documenté pour le prompt est : `Reading → Clarifying? →
/// Planning? → (Acting → Verifying)+ → Answering → Done`. Toutes les phases
/// ne sont pas requises ; **seul `Done` est le signal de clôture exploité
/// par le moteur** (Sprint A.2 — done-driven completion). `Answering` est la
/// phase qui sépare le contenu interne (trace UI repliée) de la
/// réponse finale destinée à l'utilisateur (rendu plein dans la bulle).
///
/// Les marqueurs historiques `[phase: reasoning]` et `[phase: next-move]` sont
/// **ignorés** par le parseur (ligne consommée sans effet) pour compatibilité
/// avec d'anciens prompts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    /// Raisonnement natif du fournisseur (ex. Ollama `message.thinking`) —
    /// **non** issu des marqueurs `[phase: …]` ; émis uniquement par le moteur.
    #[serde(rename = "internal_reasoning")]
    InternalReasoning,
    /// Cartographie structurée du workspace (§2.18) — distinct du `reading` ciblé.
    Analyzing,
    Reading,
    Clarifying,
    Planning,
    Acting,
    /// Vérification exécutable après mutation de code (§2.11) — tests, build, lint.
    Testing,
    Verifying,
    Answering,
    Done,
}

impl Phase {
    /// Identifiant ASCII utilisé dans le marqueur ligne `[phase: ...]`.
    #[must_use]
    pub const fn as_marker(self) -> &'static str {
        match self {
            Self::InternalReasoning => "internal_reasoning",
            Self::Analyzing => "analyzing",
            Self::Reading => "reading",
            Self::Clarifying => "clarifying",
            Self::Planning => "planning",
            Self::Acting => "acting",
            Self::Testing => "testing",
            Self::Verifying => "verifying",
            Self::Answering => "answering",
            Self::Done => "done",
        }
    }
}

/// Événement de haut niveau émis par l'agent.
///
/// `#[non_exhaustive]` : de nouveaux variants pourront être ajoutés
/// (ex. `Thinking`, `PlanStep`, `PermissionPrompt`) sans casser l'API.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum AgentEvent {
    /// Le modèle entre dans une phase (cf. protocole §`Phase`). Émis dès qu'un
    /// marqueur `[phase: ...]` est détecté en début de ligne dans le stream
    /// texte. L'UI s'en sert pour ouvrir un bloc collapsible dédié.
    PhaseEnter { phase: Phase },
    /// Ferme le bloc de phase repliable courant **sans** en ouvrir un nouveau
    /// (ex. fin du flux `thinking` Ollama avant le texte `content`).
    PhaseClose,
    /// Token(s) de texte produits par l'assistant.
    TextDelta { text: String },
    /// Le modèle a décidé d'invoquer un tool. Émis dès la réception de la
    /// décision, avant exécution.
    ToolStart {
        id: ToolUseId,
        name: String,
        arguments: Value,
    },
    /// Résultat d'exécution d'un tool, fourni au modèle au tour suivant.
    ToolFinish {
        id: ToolUseId,
        output: Value,
        #[serde(default)]
        is_error: bool,
    },
    /// Progression partielle d'un tool long (ex. bash stream stdout/stderr).
    ToolProgress {
        id: ToolUseId,
        name: String,
        /// Dernières lignes affichables.
        output: String,
        full_output: String,
        elapsed_ms: u64,
        total_lines: usize,
    },
    /// Fin du tour agent (succès final, plus aucun tool call à exécuter).
    Stop { reason: StopReason, usage: Usage },
    /// Une passe de snip a été appliquée à l'historique pour libérer du
    /// contexte. `tokens_freed` est une estimation ; `blocks_snipped` est le
    /// nombre de `tool_result` réécrits.
    ContextSnip {
        tokens_freed: usize,
        blocks_snipped: usize,
        tokens_used_after: usize,
    },
    /// Compaction LLM **en cours de run** : une portion ancienne de
    /// l'historique a été résumée et remplacée par un message `system`
    /// « checkpoint » (M2 — compaction proactive). Nécessite
    /// `AgentConfig::memory` pour réutiliser le client + prompt de
    /// compaction ; sinon seul le snip synchrone s'applique.
    ContextCompacted {
        /// Estimation jetons avant réécriture.
        tokens_before: usize,
        /// Estimation jetons après réécriture.
        tokens_after: usize,
        /// Nombre de messages retirés (hors le checkpoint inséré).
        messages_removed: usize,
        /// Coût du tour LLM de compaction, si le provider l'expose.
        #[serde(default)]
        usage: Option<Usage>,
        /// Données pour indexation mémoire longue côté client (extension).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        context_chunk_summary: Option<ContextChunkSummaryV1>,
    },
    /// Sprint M1 — un résumé du run vient d'être persisté dans
    /// `.drox/memory/sessions/`. L'UI peut afficher un chip discret
    /// « Session archivée : <slug> » et proposer un lien vers le fichier.
    ///
    /// `usage` est le coût LLM du tour de compaction (input/output tokens).
    MemoryPersisted {
        /// Slug court de la session (utilisé par `memory_read`).
        slug: String,
        /// Chemin absolu du `.md` produit.
        path: String,
        /// Objectif extrait du résumé (1 ligne, identique au front-matter).
        objective: String,
        /// Coût LLM du tour de compaction. `None` si le provider n'a pas
        /// remonté de `usage` (rare).
        #[serde(default)]
        usage: Option<Usage>,
    },
    /// Sprint §2.25 — objectif verrouillé du run (heuristique côté client).
    RunObjective { text: String },
    /// Sprint §2.25 — mise à jour du parking hors scope (`scope_defer`).
    ScopeParkingUpdate { items: Vec<ScopeDeferredItem> },
    /// Exécution hook Pre/Post tool en cours (`HookProgressMessage` leak).
    HookProgress {
        tool_use_id: ToolUseId,
        hook_event: String,
        /// Nombre de hooks encore en cours (0 = terminé pour cette phase).
        in_progress: usize,
    },
}
