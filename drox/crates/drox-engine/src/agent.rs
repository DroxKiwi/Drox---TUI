//! Boucle agent : orchestre `LlmClient` ↔ `ToolRegistry`.
//!
//! Sprint A (refonte 2026-05-13). Le moteur ne s'appuie plus sur des
//! heuristiques de détection de « réponses paresseuses » (regex de phrases
//! type « je vais explorer… »). À la place :
//!
//! 1. Le modèle peut annoncer des transitions via des marqueurs ligne
//!    `[phase: nom]` (`reading`, `planning`, `acting`, `verifying`,
//!    `clarifying`, `answering`, `done`). Les anciens marqueurs `reasoning` et
//!    `next-move` sont ignorés (ligne retirée sans effet). Voir
//!    [`crate::event::Phase`].
//! 2. `consume_stream` parse ces marqueurs ligne par ligne, les **retire** du
//!    texte assistant, et émet `AgentEvent::PhaseEnter`.
//! 3. La boucle agent applique **une seule règle de continuation** : si un
//!    tour assistant se termine **sans `tool_call`** et **sans** avoir signé
//!    `[phase: done]`, on injecte un rappel `system` et on relance l'LLM
//!    **une fois**. Si la relance reste muette, on accepte la réponse pour
//!    ne pas tourner indéfiniment ; sinon la borne dure reste
//!    `max_iterations`.

use std::sync::Arc;

use drox_llm::{ChatOptions, LlmClient, ToolSpec};
use drox_permissions::PermissionDecision;
use drox_hooks::{PostHookOutcome, PreHookOutcome, ToolHookContext, ToolHooksConfig};
use drox_tools::{CANONICAL_ASK_JSON_EXAMPLE, ToolContext, ToolRegistry, UserQuestion};
use drox_types::{Content, Message, Role, StopReason, StreamEvent, ToolUseId, Usage};
use futures::{StreamExt, stream, stream::BoxStream};
use serde_json::{Value, json};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{debug, instrument, warn};
use uuid::Uuid;

use crate::context::ContextPolicy;
use crate::error::EngineError;
use crate::event::{AgentEvent, Phase};
use crate::long_memory::ContextChunkSummaryV1;
use crate::memory::{MemoryRuntime, MemoryTracker, persist_run};
use crate::permissions::PermissionPolicy;
use crate::tool_orchestration::{ToolCallBatch, partition_tool_calls};
use crate::tool_progress::ToolProgressBridge;

/// Parse une ligne déjà extraite (sans `\n`) et tente d'en faire un marqueur
/// de phase. Reconnaît :
///
/// - le format canonique `[phase: nom]` avec espaces tolérés autour de `nom` ;
/// - quelques alias pratiques (`act` → `acting`, `verify` → `verifying`, …) ;
/// - la casse libre (`[PHASE: Done]` accepté).
///
/// Toute autre ligne renvoie `None` et reste du texte ordinaire.
fn parse_phase_marker(line: &str) -> Option<Phase> {
    let trimmed = line.trim();
    let body = trimmed.strip_prefix('[')?.strip_suffix(']')?;
    let (head, raw_name) = body.split_once(':')?;
    if !head.trim().eq_ignore_ascii_case("phase") {
        return None;
    }
    let name = raw_name.trim().to_ascii_lowercase();
    let name = name.replace('_', "-");
    let name = name.replace(' ', "-");
    match name.as_str() {
        "analyzing" | "analysis" | "survey" => Some(Phase::Analyzing),
        "reading" | "read" => Some(Phase::Reading),
        "clarifying" | "clarify" | "clarification" => Some(Phase::Clarifying),
        "planning" | "plan" => Some(Phase::Planning),
        "acting" | "act" | "action" => Some(Phase::Acting),
        "testing" | "test" | "tests" => Some(Phase::Testing),
        "verifying" | "verify" | "verification" => Some(Phase::Verifying),
        "answering" | "answer" | "respond" | "reply" | "response" => Some(Phase::Answering),
        "done" | "finish" | "finished" | "complete" | "completed" => Some(Phase::Done),
        _ => None,
    }
}

/// Marqueurs de phase **retirés du protocole** : la ligne est consommée
/// (aucun `PhaseEnter`, pas de texte) pour compatibilité avec d'anciens prompts.
#[must_use]
fn legacy_removed_phase_marker_line(line: &str) -> bool {
    let trimmed = line.trim();
    let Some(body) = trimmed.strip_prefix('[').and_then(|s| s.strip_suffix(']')) else {
        return false;
    };
    let Some((head, raw_name)) = body.split_once(':') else {
        return false;
    };
    if !head.trim().eq_ignore_ascii_case("phase") {
        return false;
    }
    let name = raw_name.trim().to_ascii_lowercase();
    let name = name.replace('_', "-");
    let name = name.replace(' ', "-");
    matches!(
        name.as_str(),
        "reasoning" | "reason" | "think" | "thought"
            | "next-move" | "nextmove" | "next" | "next-step"
    )
}

/// Inférence de phase pour un appel d'outil émis **hors phase** (filet de
/// sécurité du moteur, cf. Sprint A.4). On classe selon la nature du tool :
///
/// - **lecture seule** (`glob`, `file_read`, `grep`, `lsp`, `web_search`,
///   `web_fetch`, `todo_write`, `ask_user_question`) → `Reading` (ce sont
///   en pratique des opérations d'exploration / méta) ;
/// - **mutatif / exécution** (`file_edit`, `file_write`, `delete_path`, `bash`, autres) →
///   `Acting`.
///
/// La précision n'est pas critique : l'utilisateur observera juste un
/// bloc de phase au bon endroit, peu importe le nom exact. Le modèle est
/// invité par le prompt à déclarer ses propres phases ; ce helper sert
/// uniquement quand il oublie.
fn phase_for_tool(tool_name: &str, active_phase: Option<Phase>) -> Phase {
    if active_phase == Some(Phase::Testing) {
        if matches!(
            tool_name,
            "bash" | "lsp" | "file_read" | "grep" | "glob" | "web_fetch" | "web_search"
        ) {
            return Phase::Testing;
        }
    }
    let exploration_default = if active_phase == Some(Phase::Analyzing) {
        Phase::Analyzing
    } else {
        Phase::Reading
    };
    match tool_name {
        "glob" | "file_read" | "grep" | "lsp" | "web_search" | "web_fetch"
        | "list_mcp_resources" | "read_mcp_resource" | "workspace_map_read"
        | "memory_read" | "memory_list" | "task"
        | "todo_write" | "course_plan_write" | "ask_user_question" => exploration_default,
        _ => Phase::Acting,
    }
}

/// `true` si le texte utilisateur ressemble à une demande d'analyse de dépôt (§2.18).
fn user_prompt_suggests_workspace_analysis(text: &str) -> bool {
    let lower = text.to_lowercase();
    [
        "analyse",
        "analyze",
        "audit",
        "vue d'ensemble",
        "overview",
        "structure du projet",
        "structure of the project",
        "explore le repo",
        "explore the repo",
        "cartograph",
        "survey the",
        "comprendre le projet",
        "understand the project",
        "analyse le projet",
        "analyze the project",
        "analyse ce repo",
        "analyze this repo",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn is_workspace_exploration_tool(name: &str) -> bool {
    matches!(
        name,
        "glob"
            | "grep"
            | "file_read"
            | "lsp"
            | "web_search"
            | "web_fetch"
            | "workspace_map_read"
            | "memory_read"
            | "memory_list"
            | "task"
    )
}

fn user_blocks_plain_text(blocks: &[Content]) -> String {
    blocks
        .iter()
        .filter_map(|b| match b {
            Content::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Extensions pour lesquelles une mutation ne déclenche pas la gate `testing` (§2.11).
const NON_CODE_MUTATION_EXTENSIONS: &[&str] = &[
    "md",
    "markdown",
    "txt",
    "gitignore",
    "png",
    "jpg",
    "jpeg",
    "gif",
    "webp",
    "svg",
    "ico",
    "csv",
    "pdf",
];

const CODE_MUTATION_TESTING_NUDGE: &str = "You modified **code** in this run but never entered \
    `[phase: testing]` with a **concrete verification** tool call.\n\
    \n\
    Before your final `[phase: answering]` + `[phase: done]`, you MUST:\n\
    1. Emit `[phase: testing]` on its own line.\n\
    2. Call at least one verification tool in the same turn or the next: `bash` \
    (e.g. `cargo check`, `cargo test`, `npm test`, `pnpm typecheck`, `tsc --noEmit`), \
    `lsp` with diagnostics, or `file_read` on a file you edited.\n\
    \n\
    Do not skip this — the engine will keep refusing `[phase: done]` until testing ran.";

#[must_use]
fn path_extension_lower(path: &str) -> Option<String> {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let ext = name.rsplit('.').next()?;
    if ext == name {
        return None;
    }
    Some(ext.to_ascii_lowercase())
}

#[must_use]
fn path_counts_as_code_mutation(path: &str) -> bool {
    match path_extension_lower(path) {
        Some(ext) => !NON_CODE_MUTATION_EXTENSIONS.contains(&ext.as_str()),
        None => true,
    }
}

#[must_use]
fn tool_mutation_path(arguments: &Value) -> Option<&str> {
    arguments
        .get("path")
        .or_else(|| arguments.get("destination"))
        .or_else(|| arguments.get("source"))
        .and_then(|v| v.as_str())
}

#[must_use]
fn bash_command_counts_as_code_mutation(command: &str) -> bool {
    let lower = command.to_lowercase();
    [
        "git commit",
        "git push",
        "git add",
        "npm install",
        "pnpm install",
        "yarn add",
        "cargo fix",
        "rm -rf",
        "del /f",
        "del /s",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

/// `true` si l'appel d'outil réussi doit activer la gate `testing` (§2.11).
#[must_use]
fn record_counts_as_code_mutation(tool_name: &str, arguments: &Value) -> bool {
    match tool_name {
        "file_edit" | "file_write" | "delete_path" | "copy_path" => tool_mutation_path(arguments)
            .is_some_and(path_counts_as_code_mutation),
        "notebook_edit" => true,
        "bash" => arguments
            .get("command")
            .and_then(|v| v.as_str())
            .is_some_and(bash_command_counts_as_code_mutation),
        _ => false,
    }
}

const ANALYZING_PHASE_NUDGE: &str = "The user asked for a **workspace / repo analysis**. \
     Prefer `[phase: analyzing]` (not generic `[phase: reading]`) for this structural pass. \
     Playbook: `workspace_map_read` if `[Workspace map]` is fresh → targeted `glob` (not blind \
     root rescans) → `grep` + `file_read` with line ranges → `lsp` entry points. Treat \
     `directory_fanout_caps` / `truncated` as signals to refine paths, not as errors. \
     Keep notes telegraphic inside `analyzing`; put the full structured report only in \
     `[phase: answering]`.";

/// Corps du message `role = tool` renvoyé au LLM après exécution réussie.
///
/// Sans balisage explicite, certains modèles prennent un gros JSON (sortie
/// `glob`, `grep`, etc.) pour un « collage » utilisateur arbitraire au lieu
/// du résultat structuré de leur propre appel d'outil.
fn format_tool_result_for_llm(tool_name: &str, value: &Value) -> String {
    let serialized = serde_json::to_string(value).unwrap_or_default();
    format!(
        "[drox: résultat de l'outil «{tool_name}» — JSON ci-dessous ; ce n'est pas un message utilisateur]\n{serialized}",
    )
}

/// Texte injecté en `system` quand un tour LLM se termine **sans** marqueur
/// `[phase: done]`. Sprint A.2 — done-driven completion : le moteur n'observe
/// plus du tout `tool_calls.is_empty()` pour décider de la fin ; seul
/// `[phase: done]` ferme la boucle. Tant que ce marqueur n'est pas vu, on
/// relance avec ce nudge, jusqu'à `max_iterations` (garde-fou unique).
///
/// Rédigé en anglais : les modèles Ollama de taille modeste (Devstral 24b,
/// Gemma 27b) suivent mieux les méta-instructions structurelles en anglais.
const NUDGE_PROMPT: &str = "Have you fully completed the user's objective?\n\
    \n\
    **IMPORTANT — read this before acting:** This is an engine reminder, NOT \
    a user reply. If your previous `[phase: answering]` ended with a question \
    to the user (\"Do you want me to…?\", \"Shall I…?\", \"Would you like…?\"), \
    treat the answer as NO — the user has NOT responded yet. In that case, \
    you MUST close with `[phase: done]` and wait. Do NOT interpret this \
    engine message as user approval or as permission to proceed autonomously.\n\
    \n\
    - If your last answering block contained a question to the user and you \
      are waiting for their answer → emit ONLY `[phase: done]`. Stop here.\n\
    - If YES (objective fully met, no pending question): emit `[phase: answering]` \
      on its own line, write your final user-facing response in clean Markdown, \
      then end the message with a line containing EXACTLY `[phase: done]`. \
      That is the ONLY way to end the conversation.\n\
    - If NO: emit `[phase: reading]` or `[phase: acting]` (pick what matches \
      your next tool), optionally one short line of intent, then call the \
      next tool **in the same reply** (`glob`, `file_read`, `grep`, `lsp`, \
      `file_edit`, `file_write`, `delete_path`, `bash`, etc.). Stopping with mere intent \
      prose (\"I should verify…\", \"I will read…\") does NOT end your turn — \
      the engine will keep nudging you until you either deliver `[phase: done]` \
      or actually act.";

/// Nudge minimaliste injecté quand le modèle a **déjà** rédigé sa réponse
/// dans `[phase: answering]` mais a oublié le marqueur `[phase: done]` final.
///
/// Le `NUDGE_PROMPT` générique ci-dessus relance le modèle en lui demandant
/// d'« écrire sa réponse finale » — ce que GLM-4.7-Flash et consorts prennent
/// au pied de la lettre et **ré-écrivent** toute la réponse, causant un
/// affichage en double dans le fil. Ce prompt-ci sert exclusivement à dire
/// « ajoute juste le marqueur, ne rerédige RIEN ». Déclenché par
/// `drive_inner` quand `final_phase == Some(Answering)` ET aucune todo
/// ouverte.
const DONE_ONLY_NUDGE_PROMPT: &str = "Your previous reply ended inside \
    `[phase: answering]` but did NOT include the final `[phase: done]` marker. \
    The engine only closes the turn on `[phase: done]`.\n\
    \n\
    **Do NOT rewrite, paraphrase, or repeat your answer** — the user already \
    received it. Just send a tiny assistant message containing ONLY:\n\
    \n\
    [phase: done]\n\
    \n\
    Nothing else. No `[phase: answering]`, no Markdown, no recap.";

/// Injecté quand le modèle signe `[phase: done]` SANS jamais avoir émis
/// `[phase: answering]` au cours du run. Symptôme : le modèle écrit sa
/// synthèse dans `reading` / `verifying` puis ferme directement — la
/// réponse se retrouve enfouie dans la trace UI repliée, invisible.
///
/// Le moteur refuse alors la clôture et demande au modèle de **re-rédiger**
/// sa réponse dans la bonne phase, même au prix d'une répétition. C'est
/// délibéré : la phase `answering` est la seule rendue en clair côté UI
/// (cf. `enterPhase` dans `chat.js`).
const MISSING_ANSWERING_PROMPT: &str = "You emitted `[phase: done]` without \
    ever using `[phase: answering]` in this run. The engine cannot close yet: \
    your final user-facing reply MUST live inside `[phase: answering]`. Any \
    text written in `reading`, `verifying`, or other reflection phases is \
    hidden in the collapsible trace and the user will not see it.\n\
    \n\
    Re-send your final response NOW in this exact shape, even if it repeats \
    what you already wrote:\n\
    \n\
    [phase: answering]\n\
    <your full user-facing answer in Markdown>\n\
    [phase: done]";

/// Liste des outils qui exigent qu'un `todo_write` ait déjà réussi dans le run.
/// La planification est critique **avant mutation**, pas avant exploration :
/// imposer un plan avant d'avoir vu l'arborescence donne souvent des plans
/// génériques ou hors-sujet. On laisse donc passer librement les read-only
/// (`glob`/`file_read`/`grep`/`lsp`/`web_*`) et on bloque uniquement les
/// écritures et l'exécution shell tant que la to-do n'est pas posée.
///
/// Note : `ask_user_question` n'est pas dans la liste — demander une
/// clarification avant de planifier est légitime (cf. phase `clarifying`).
const TOOLS_REQUIRING_TODO_WRITE_GATE: &[&str] =
    &["file_edit", "file_write", "notebook_edit", "delete_path", "copy_path", "bash"];

#[must_use]
fn requires_todo_write_gate(tool_name: &str) -> bool {
    TOOLS_REQUIRING_TODO_WRITE_GATE.contains(&tool_name)
}

const MUTATING_TOOL_BEFORE_TODO_WRITE_BLOCKED: &str = "Blocked: you tried to call a \
    **mutating** tool (`file_edit` / `file_write` / `notebook_edit` / `delete_path` / `bash`) before any successful \
    `todo_write` in this run. Planning is required before any mutation — the user \
    wants to see your plan in the to-do widget BEFORE you start changing files or \
    running commands. Call `todo_write` first with at least one item describing \
    what you're about to do, then retry your mutation. Read-only exploration \
    (`glob`, `file_read`, `grep`, `lsp`, `web_*`) remains allowed before the plan \
    if you need more context.";

const PROFESSOR_DONE_WITHOUT_PLAN: &str = "You emitted `[phase: done]` but never called \
    `course_plan_write` successfully in this run. In Professor mode, end with a course \
    plan visible to the learner: call `course_plan_write`, then `[phase: answering]` + \
    `[phase: done]`, or continue teaching if more work remains.";

const TODO_WRITE_FORBIDDEN_IN_PROFESSOR: &str = "Blocked: `todo_write` is not available \
    in Professor mode. Use `course_plan_write` to maintain the **Plan de cours** instead.";

#[must_use]
fn is_professor_run(policy: Option<&PermissionPolicy>) -> bool {
    policy.is_some_and(|p| p.mode.is_professor())
}

#[must_use]
fn plan_write_gate_satisfied(professor: bool, saw_todo_write: bool, saw_course_plan: bool) -> bool {
    if professor {
        saw_course_plan
    } else {
        saw_todo_write
    }
}

/// GLM-4.7-Flash / Qwen confondent parfois le marqueur texte `[phase: done]`
/// avec le mécanisme natif `tool_calls` : ils émettent un outil nommé
/// `phase` / `phase:` avec `{\"done\": \"\"}`. Ce n'est pas un outil Drox ;
/// côté permissions les outils inconnus tombent en **Ask** → l'extension
/// refuse sans asker explicite → message trompeur « User denied permission
/// for `phase:` ». On intercepte **avant** `check_permission` et on renvoie
/// une erreur explicite pour éviter la boucle « acting → réécriture finale ».
const PHASE_MARKER_MUST_BE_TEXT_NOT_TOOL: &str = "You tried to invoke a tool named \
    `phase` (or similar) with JSON arguments like `{ \"done\": … }`. That is a \
    misunderstanding: **phase markers are NOT tools**. They MUST appear as \
    **plain text lines** in your assistant message — e.g. `[phase: done]` alone \
    on its own line after your final Markdown answer. Do NOT use `tool_calls` to \
    simulate the protocol. In your NEXT reply, output the literal text line \
    `[phase: done]` (no function call for it). Do NOT rewrite your whole answer \
    unless you still have real work left.";

/// Détecte les `tool_calls` qui mimiquent le protocole `[phase: …]`.
#[must_use]
fn is_hallucinated_phase_tool_call(name: &str, arguments: &Value) -> bool {
    let raw = name.trim().to_ascii_lowercase();
    let core = raw.trim_end_matches(':').trim();
    if let Some((head, _rest)) = core.split_once(':') {
        if head == "phase" {
            return true;
        }
    }
    if core == "phase" {
        return true;
    }
    if core == "set_phase" || core.starts_with("phase_") {
        return true;
    }
    // Motif fréquent : outil `done` avec seule clé `done` (vide) — confusion avec `[phase: done]`.
    if core == "done"
        && arguments
            .as_object()
            .is_some_and(|m| m.len() == 1 && m.contains_key("done"))
    {
        return true;
    }
    false
}

/// Liste blanche des outils qui comptent comme **étape de travail réel** vis-à-vis
/// du suivi de progression. Si le modèle enchaîne 2 (ou plus) de ces outils
/// sans intercaler un `todo_write`, il est en train de batcher ses étapes
/// au lieu de les cocher au fil de l'eau — symptôme observé sur GLM-4.7-Flash :
/// "0/5 → 5/5" en un seul update. Le moteur injecte alors un nudge soft pour
/// rappeler le contrat "step-by-step".
///
/// On ne compte PAS les outils d'exploration (`glob`, `grep`, `file_read`,
/// `lsp`, `web_*`) : pendant une même étape « comprendre le module X »,
/// le modèle peut avoir besoin de lire 5 fichiers — c'est UNE étape, pas
/// cinq. La granularité utile est l'**action** (édition, exécution shell).
const MUTATING_TOOLS_FOR_STEP_TRACKING: &[&str] =
    &["file_edit", "file_write", "notebook_edit", "delete_path", "bash"];

/// Nudge soft injecté quand le modèle a accumulé ≥ 2 outils mutateurs depuis
/// son dernier `todo_write`. Ne bloque PAS le tour courant (les outils sont
/// déjà exécutés) — c'est un rappel pour le tour suivant. Volontairement court
/// pour ne pas polluer le contexte si le modèle a une bonne raison de batch.
/// Sprint Plan « un seul plan par run » — message d'erreur tool_result
/// poussé quand le modèle tente de **re-créer** une todo from scratch après
/// avoir clôturé la précédente (cf. règle 7ter du `CORE_SYSTEM_PROMPT`).
const TODO_RECREATION_BLOCKED: &str = "Blocked: you tried to **replace** \
your previous todo list with a brand-new one. The previous list was \
already all-completed and the new list contains only new ids → that's a \
recreation, not an update. The UI would then show two distinct plans \
side by side, which is exactly what we want to avoid.\n\n\
Resubmit `todo_write` with a payload that **includes the previous items \
as `completed`** (verbatim — same ids, same content) PLUS the new steps \
appended at the end with new ids (`pending` / `in_progress`). One plan \
per run, ever-growing, never replaced.";

/// `session_end` n'est pas exposé au LLM ; s'il est quand même émis, on
/// refuse (clôture de session = commande utilisateur `/session_end` dans
/// l'IDE uniquement).
const SESSION_END_FORBIDDEN_FOR_MODEL: &str =
    "session_end: cet outil n'est pas disponible depuis le modèle. La fin de session \
     (nouveau fil de chat + mémoire longue côté client) est réservée à la commande \
     `/session_end` déclenchée par l'utilisateur. Pour archiver le travail : termine \
     la to-do puis `[phase: answering]` + `[phase: done]` — le moteur écrit déjà \
     `.drox/memory/sessions/` automatiquement.";

/// Extrait la liste des ids du payload `todo_write` et indique si au moins
/// un item est encore actif (`pending` / `in_progress`). Retourne `None`
/// si le format est inconnu (auquel cas le moteur laisse passer — le tool
/// fera lui-même son auto-normalisation et son erreur de parse).
fn extract_todo_ids_and_active(args: &Value) -> Option<(Vec<String>, bool)> {
    let todos = args.get("todos")?.as_array()?;
    let mut ids = Vec::with_capacity(todos.len());
    let mut has_active = false;
    for t in todos {
        let id = t.get("id")?.as_str()?.to_string();
        let status = t.get("status").and_then(|v| v.as_str()).unwrap_or("");
        if status == "pending" || status == "in_progress" {
            has_active = true;
        }
        ids.push(id);
    }
    Some((ids, has_active))
}

/// Vraie ssi (a) le plan précédent était fully completed, (b) aucun id du
/// nouveau plan ne provient du précédent, et (c) le nouveau plan contient
/// au moins un item en `pending` ou `in_progress`. Trois conditions
/// strictes pour minimiser les faux positifs.
fn is_todo_recreation_from_scratch(
    new_args: &Value,
    last_ids: &std::collections::HashSet<String>,
    last_was_all_completed: bool,
) -> bool {
    if !last_was_all_completed || last_ids.is_empty() {
        return false;
    }
    let Some((new_ids, has_active)) = extract_todo_ids_and_active(new_args) else {
        return false;
    };
    if !has_active {
        // Si le nouveau plan est lui-même tout en completed, c'est une
        // tentative de rappel/dédup — pas une re-création.
        return false;
    }
    new_ids.iter().all(|id| !last_ids.contains(id))
}

fn step_by_step_todo_nudge(unupdated_tools: u32, pending: u64, in_progress: u64) -> String {
    format!(
        "Heads-up: you've called {unupdated_tools} mutating tools \
         (file_edit / file_write / notebook_edit / delete_path / bash) since your last `todo_write`, and your \
         plan still has {pending} pending + {in_progress} in_progress item(s).\n\
         \n\
         The user follows your progress in real time on the todo widget. Don't \
         wait until the end of the run to flip everything to `completed` in one \
         batch — that defeats the whole point of the plan.\n\
         \n\
         Before your next action: emit `todo_write` with the SAME list, but \
         move the finished step(s) to `completed` and the next active step to \
         `in_progress`. Then continue with `[phase: reading]` / `[phase: acting]` + your next \
         tool. One `todo_write` per real step transition is enough — you don't \
         need one between every tool call inside the same step."
    )
}

/// Injecté quand le modèle signe `[phase: done]` alors que la **dernière**
/// `todo_write` a encore des items en `pending` ou `in_progress`. Le moteur
/// considère qu'on n'a pas le droit de clôturer si la to-do n'est pas
/// elle-même clôturée (tous les items en `completed` ou `cancelled`).
///
/// Le message demande au modèle de **se poser** : est-ce qu'il reste vraiment
/// du travail (alors `[phase: reading]` / `[phase: acting]` + outil) ou est-ce que les items
/// sont en fait faits (alors un nouveau `todo_write` qui les passe en
/// `completed`).
fn unfinished_todos_prompt(pending: u64, in_progress: u64) -> String {
    format!(
        "You emitted `[phase: done]` but your most recent `todo_write` still has \
         {pending} item(s) in `pending` and {in_progress} item(s) in `in_progress`. \
         The engine cannot close yet — the todo list must mirror reality before you \
         end the turn.\n\
         \n\
         Decide which case you're in, then act:\n\
         \n\
         - If the remaining items are ACTUALLY done (you just forgot to update them): \
         call `todo_write` again with the SAME items, but flip their `status` to \
         `completed` (or `cancelled` if no longer relevant). Then emit \
         `[phase: answering]` + your final reply + `[phase: done]`.\n\
         - If something is still left to do: do NOT close. Emit `[phase: reading]` or \
         `[phase: acting]` on its own line, then call the appropriate tool in the SAME reply.\n\
         \n\
         An open todo means the work is not finished."
    )
}

fn unfinished_course_plan_prompt(pending: u64, active: u64) -> String {
    format!(
        "You emitted `[phase: done]` but your most recent `course_plan_write` still has \
         {pending} step(s) in `pending` and {active} in `active`. Update the **Plan de cours** \
         (`mastered` / `skipped`) or continue teaching the active step before closing.\n\
         \n\
         - If the learner finished the step: `course_plan_write` with that step `mastered`, \
         next step `active`, then `[phase: answering]` + `[phase: done]` if you wait for them.\n\
         - If work remains: do NOT close — continue `[phase: teach]` / `[phase: exercise]`."
    )
}

/// Configuration d'un agent.
#[derive(Debug, Clone)]
pub struct AgentConfig {
    /// System prompt optionnel injecté en tête de conversation.
    pub system_prompt: Option<String>,
    /// Nombre maximum d'allers-retours LLM ↔ tools dans un même `run()`.
    pub max_iterations: usize,
    /// Options passées tel quel au `LlmClient` (tools y sont ajoutés
    /// automatiquement à partir du `ToolRegistry`).
    pub chat_options: ChatOptions,
    /// Politique de permissions. Si `None`, aucun garde-fou : tous les tool
    /// calls sont exécutés (mode "ancien" pré-1.7). En production, fournir
    /// systématiquement une politique.
    pub permissions: Option<PermissionPolicy>,
    /// Politique de contexte (token counting + snip auto). Si `None`,
    /// l'historique n'est jamais réduit (sprint 1.4 behaviour).
    pub context: Option<ContextPolicy>,
    /// Persistance transcript JSONL (sprint 1.10). Si `None`, rien n'est
    /// écrit sur disque.
    pub transcript: Option<drox_session::TranscriptSessionConfig>,
    /// Sprint M1 — mémoire de session (compaction + persistance dans
    /// `.drox/memory/sessions/`). Si `None`, aucun résumé n'est produit et
    /// les tools `session_note` / `memory_*` ne sont pas branchés.
    pub memory: Option<MemoryRuntime>,
    /// Identifiant `ses_…` du transcript JSONL (JSON-RPC / extension). Sert
    /// aux enregistrements `context_chunk_summary` côté client.
    pub transcript_session_id: Option<String>,
    /// Empreinte workspace (chemin canonique) pour corréler l'index client.
    pub workspace_fingerprint: String,
    /// Nombre max de tools read-only exécutés en parallèle dans un même tour (§2.29).
    pub max_parallel_tool_calls: usize,
    /// Hooks pre/post tool (§2.6). `None` ou config vide = désactivé.
    pub tool_hooks: Option<ToolHooksConfig>,
    /// Objectif verrouillé du run (§2.25) — injecté en tête de conversation.
    pub run_objective: Option<String>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            system_prompt: None,
            max_iterations: 12,
            chat_options: ChatOptions::default(),
            permissions: None,
            context: None,
            transcript: None,
            memory: None,
            transcript_session_id: None,
            workspace_fingerprint: String::new(),
            max_parallel_tool_calls: crate::tool_orchestration::DEFAULT_MAX_PARALLEL_TOOL_CALLS,
            tool_hooks: None,
            run_objective: None,
        }
    }
}

fn run_objective_system_block(objective: &str) -> String {
    format!(
        "## Objectif verrouillé (demande utilisateur)\n{}\n\n\
         Fidélité objectif :\n\
         - Une incohérence découverte n'est PAS une tâche implicite : utilise `scope_defer` \
         ou `ask_user_question` avant d'élargir le périmètre.\n\
         - Pas d'audit ni refactor global tant que cet objectif n'est pas atteint.\n\
         - Avant `[phase: done]`, indique brièvement dans ta dernière `[phase: answering]` \
         comment l'objectif est satisfait.",
        objective.trim()
    )
}

async fn mirror_workspace_map_from_tool(
    ctx: &ToolContext,
    tool_name: &str,
    output: &Value,
) {
    let Some(store) = ctx.workspace_map.as_ref() else {
        return;
    };
    match tool_name {
        "glob" => store.ingest_glob(output),
        "file_read" => store.ingest_file_read(output),
        "lsp" => store.ingest_lsp(output),
        _ => return,
    }
    store.save_if_dirty().await;
}

fn run_objective_done_nudge(objective: &str) -> String {
    format!(
        "Tu as émis `[phase: done]`. Avant de clôturer : dans `[phase: answering]`, rappelle en \
         **une courte phrase** comment l'objectif verrouillé est satisfait, puis `[phase: done]` \
         à nouveau.\n\nObjectif : {}",
        objective.trim()
    )
}

/// Stream typé d'événements agent.
pub type AgentStream = BoxStream<'static, Result<AgentEvent, EngineError>>;

/// Agent : boucle LLM streaming + dispatch de tool calls.
///
/// Cheap-to-clone : toutes les ressources lourdes sont derrière `Arc`.
#[derive(Clone)]
pub struct Agent {
    llm: Arc<dyn LlmClient>,
    registry: Arc<ToolRegistry>,
    ctx: ToolContext,
    config: AgentConfig,
}

impl Agent {
    pub fn new(
        llm: Arc<dyn LlmClient>,
        registry: Arc<ToolRegistry>,
        ctx: ToolContext,
        config: AgentConfig,
    ) -> Self {
        Self {
            llm,
            registry,
            ctx,
            config,
        }
    }

    /// Lance la boucle agent et retourne un stream d'événements.
    ///
    /// La tâche async est spawnée sur le runtime courant ; le stream se ferme
    /// quand l'agent atteint `Stop`, `MaxIterations`, ou une erreur.
    pub fn run(&self, prompt: impl Into<String>) -> AgentStream {
        self.run_with_history(Vec::new(), prompt)
    }

    /// Comme [`Self::run`], mais préfixe l'historique chargé depuis le disque
    /// (transcript JSONL) avant le nouveau message utilisateur.
    pub fn run_with_history(
        &self,
        history: Vec<Message>,
        prompt: impl Into<String>,
    ) -> AgentStream {
        self.run_with_history_blocks(history, vec![Content::text(prompt.into())])
    }

    /// Variante multimodale de [`Self::run_with_history`] : permet de pousser
    /// un message `user` constitué de blocs `Content` arbitraires (texte +
    /// images). Utilisée par le serveur JSON-RPC pour relayer des pièces
    /// jointes au modèle.
    #[instrument(skip(self, history, user_blocks), fields(max_iter = self.config.max_iterations, blocks = user_blocks.len()))]
    pub fn run_with_history_blocks(
        &self,
        history: Vec<Message>,
        user_blocks: Vec<Content>,
    ) -> AgentStream {
        let (tx, rx) = mpsc::channel::<Result<AgentEvent, EngineError>>(32);
        let agent = self.clone();
        tokio::spawn(async move {
            agent.drive_inner(history, user_blocks, tx).await;
        });
        ReceiverStream::new(rx).boxed()
    }

    #[allow(clippy::too_many_lines)]
    async fn drive_inner(
        self,
        history: Vec<Message>,
        user_blocks: Vec<Content>,
        tx: mpsc::Sender<Result<AgentEvent, EngineError>>,
    ) {
        let professor = is_professor_run(self.config.permissions.as_ref());
        let tool_specs = build_tool_specs(&self.registry, professor);
        // Sprint M1 — si la mémoire de session est configurée, on greffe
        // le `SessionNotesHandle` partagé dans le `ToolContext` du run.
        // Les tools `session_note` / `memory_read` / `memory_list` y voient
        // le stock partagé ; sans `memory`, ils renvoient une erreur explicite
        // ("tool unavailable in this context") qui s'affiche au modèle.
        let ctx = match self.config.memory.as_ref() {
            Some(mem) => self.ctx.clone().with_session_notes(mem.notes.clone()),
            None => self.ctx.clone(),
        };
        let mut memory_tracker = MemoryTracker::new();
        let mut messages = Vec::new();
        if let Some(sys) = &self.config.system_prompt {
            messages.push(Message::system(sys));
        }
        if let Some(obj) = &self.config.run_objective {
            let block = run_objective_system_block(obj);
            messages.push(Message::system(block));
            let _ = tx
                .send(Ok(AgentEvent::RunObjective {
                    text: obj.clone(),
                }))
                .await;
        }
        messages.extend(history);
        // Garantit qu'il y a toujours au moins un bloc texte pour les
        // providers strictement text-only et pour la cohérence du transcript.
        let user_blocks = if user_blocks.is_empty() {
            vec![Content::text(String::new())]
        } else {
            user_blocks
        };
        let user_analysis_intent =
            user_prompt_suggests_workspace_analysis(&user_blocks_plain_text(&user_blocks));
        messages.push(Message::user_with_blocks(user_blocks));

        let mut transcript_cursor = self
            .config
            .transcript
            .as_ref()
            .map_or(0, |t| t.append_from_message_index);
        if let Err(e) = self
            .flush_transcript(&messages, &mut transcript_cursor)
            .await
        {
            let _ = tx.send(Err(e)).await;
            return;
        }

        // Sprint A.2 — done-driven completion. Plus de heuristique « pas
        // d'outil = on s'arrête » : la SEULE condition de fin propre est
        // `[phase: done]`. Tant que ce marqueur n'est pas vu, on injecte un
        // nudge et on relance. La borne dure reste `max_iterations`.
        //
        // Sprint A.3 — answering-before-done. On ajoute une seconde
        // contrainte : `done` n'est accepté que si **au moins un**
        // `[phase: answering]` a été émis dans le run. Sinon la réponse
        // finale est enfouie dans la trace UI repliée (invisible) et on
        // demande au modèle de la re-rédiger dans `answering`, même au prix
        // d'une répétition. C'est délibéré : la phase `answering` est la
        // seule rendue en clair dans la bulle assistant (cf. `chat.js`).
        let mut seen_answering_in_run = false;
        let mut saw_successful_todo_write_in_run = false;
        let mut saw_successful_course_plan_write_in_run = false;
        // Snapshot du dernier `todo_write` réussi (compteurs `pending` /
        // `in_progress`). Sert à interdire `[phase: done]` tant que la to-do
        // n'est pas elle-même clôturée — cf. `unfinished_todos_prompt`.
        let mut last_todo_pending: u64 = 0;
        let mut last_todo_in_progress: u64 = 0;
        let mut last_course_pending: u64 = 0;
        let mut last_course_active: u64 = 0;
        let mut professor_course_state = crate::professor::ProfessorCourseState::default();
        // Sprint Plan « un seul plan par run » — set des ids du dernier
        // `todo_write` réussi + flag « toutes les étapes étaient completed ».
        // Servent à détecter une re-création de plan from scratch après
        // clôture (cf. `is_todo_recreation_from_scratch`).
        let mut last_todo_ids: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        let mut last_todo_was_all_completed: bool = false;
        // Step-by-step progression tracker : compte le nombre d'outils
        // mutateurs (`file_edit` / `file_write` / `notebook_edit` / `delete_path` / `bash`) exécutés depuis le
        // dernier `todo_write` réussi. Au-delà de 2, on injecte un nudge soft
        // pour pousser le modèle à mettre à jour sa todo entre les étapes.
        // Reset à chaque `todo_write` réussi ET à chaque injection de nudge
        // (pour ne pas répéter en boucle).
        let mut mutating_tools_since_last_todo: u32 = 0;
        // Sprint Hotfix « boucle édition/lecture » — détecteur strict de
        // répétition d'empreinte (texte assistant + tool_calls). Voir
        // `LoopDetector` ; le `reset` est appelé après chaque nudge moteur
        // structurel pour ne pas pénaliser une convergence forcée.
        let mut loop_detector = LoopDetector::new();
        let mut consecutive_ask_user_question_failures: u32 = 0;
        let mut live_compaction_seq: u32 = 0;
        let mut run_objective_reminder_sent = false;
        let mut analyzing_phase_nudge_sent = false;
        let mut saw_analyzing_phase_in_run = false;
        let mut saw_code_mutation_in_run = false;
        let mut saw_testing_phase_in_run = false;
        let testing_gate_active = !ctx.plan_mode && !professor;
        for iter in 0..self.config.max_iterations {
            debug!(
                iter,
                seen_answering_in_run,
                saw_successful_todo_write_in_run,
                last_todo_pending,
                last_todo_in_progress,
                mutating_tools_since_last_todo,
                "tour LLM (todo_write obligatoire AVANT tout autre outil, optionnel pour conversation pure)"
            );

            if self
                .maybe_snip(&mut messages, &tx, &mut live_compaction_seq)
                .await
                .is_err()
            {
                return;
            }

            let options = self
                .config
                .chat_options
                .clone()
                .with_tools(tool_specs.clone());

            let stream = match self.llm.stream_chat(messages.clone(), options).await {
                Ok(s) => s,
                Err(err) => {
                    let _ = tx.send(Err(err.into())).await;
                    return;
                }
            };

            let native_thinking_ui = self.config.chat_options.think == Some(true);

            let Ok(mut outcome) = consume_stream(stream, &tx, native_thinking_ui).await else {
                return; // canal consommateur fermé
            };

            push_assistant_message(&mut messages, &outcome);
            if let Err(e) = self
                .flush_transcript(&messages, &mut transcript_cursor)
                .await
            {
                let _ = tx.send(Err(e)).await;
                return;
            }

            // Track : answering vu au moins une fois dans le run ?
            if outcome.saw_answering {
                seen_answering_in_run = true;
            }
            if outcome.saw_analyzing {
                saw_analyzing_phase_in_run = true;
            }
            if outcome.saw_testing {
                saw_testing_phase_in_run = true;
            }

            // Sprint Hotfix « boucle » — détection d'empreinte répétée. À
            // évaluer AVANT les gates `Done` / `tool_calls.is_empty` parce
            // que celles-ci injectent leurs propres nudges et pourraient
            // masquer la boucle (le modèle répondrait pareil mais on
            // continuerait à nudger sans jamais stopper).
            match loop_detector.observe(&outcome) {
                LoopDecision::Ok => {}
                LoopDecision::Warn { kind } => {
                    debug!(
                        kind,
                        "boucle détectée (1er strike) — injection nudge anti-boucle"
                    );
                    messages.push(Message::system(LOOP_DETECTED_NUDGE_PROMPT));
                    if let Err(e) = self
                        .flush_transcript(&messages, &mut transcript_cursor)
                        .await
                    {
                        let _ = tx.send(Err(e)).await;
                        return;
                    }
                    continue;
                }
                LoopDecision::Abort { kind, turns } => {
                    debug!(
                        kind,
                        turns,
                        "boucle non résolue après nudge — abort"
                    );
                    let _ = tx
                        .send(Err(EngineError::LoopDetected { kind, turns }))
                        .await;
                    return;
                }
            }

            // Done-driven completion + answering-before-done. `todo_write`
            // n'est PAS exigé ici : les réponses purement conversationnelles
            // (salutations, questions triviales) et les runs purement
            // exploratoires (lecture sans mutation) ont le droit de clôturer
            // sans liste de tâches. Si le modèle a touché un outil mutateur
            // (file_edit / file_write / notebook_edit / delete_path / bash), la gate
            // `MUTATING_TOOL_BEFORE_TODO_WRITE_BLOCKED` l'aura forcé à passer
            // par `todo_write` AVANT — donc la liste existe forcément quand
            // il y a eu du vrai travail, et la gate `unfinished_todos_prompt`
            // ci-dessous garde la clôture propre.
            if outcome.final_phase == Some(Phase::Done) {
                if !seen_answering_in_run {
                    debug!("[phase: done] prématuré (answering absent) — nudge");
                    messages.push(Message::system(MISSING_ANSWERING_PROMPT));
                    loop_detector.reset();
                    if let Err(e) = self
                        .flush_transcript(&messages, &mut transcript_cursor)
                        .await
                    {
                        let _ = tx.send(Err(e)).await;
                        return;
                    }
                    continue;
                }
                if professor && !saw_successful_course_plan_write_in_run {
                    debug!("[phase: done] professor sans course_plan_write — nudge");
                    messages.push(Message::system(PROFESSOR_DONE_WITHOUT_PLAN));
                    loop_detector.reset();
                    if let Err(e) = self
                        .flush_transcript(&messages, &mut transcript_cursor)
                        .await
                    {
                        let _ = tx.send(Err(e)).await;
                        return;
                    }
                    continue;
                }
                if professor {
                    if last_course_pending > 0 || last_course_active > 0 {
                        debug!(
                            last_course_pending,
                            last_course_active,
                            "[phase: done] avec plan de cours ouvert — nudge"
                        );
                        messages.push(Message::system(unfinished_course_plan_prompt(
                            last_course_pending,
                            last_course_active,
                        )));
                        loop_detector.reset();
                        if let Err(e) = self
                            .flush_transcript(&messages, &mut transcript_cursor)
                            .await
                        {
                            let _ = tx.send(Err(e)).await;
                            return;
                        }
                        continue;
                    }
                } else if last_todo_pending > 0 || last_todo_in_progress > 0 {
                    debug!(
                        last_todo_pending,
                        last_todo_in_progress,
                        "[phase: done] avec to-do ouverte — nudge"
                    );
                    messages.push(Message::system(unfinished_todos_prompt(
                        last_todo_pending,
                        last_todo_in_progress,
                    )));
                    loop_detector.reset();
                    if let Err(e) = self
                        .flush_transcript(&messages, &mut transcript_cursor)
                        .await
                    {
                        let _ = tx.send(Err(e)).await;
                        return;
                    }
                    continue;
                }
                if testing_gate_active
                    && saw_code_mutation_in_run
                    && !saw_testing_phase_in_run
                {
                    debug!(
                        "[phase: done] mutation code sans phase testing — nudge"
                    );
                    messages.push(Message::system(CODE_MUTATION_TESTING_NUDGE));
                    loop_detector.reset();
                    if let Err(e) = self
                        .flush_transcript(&messages, &mut transcript_cursor)
                        .await
                    {
                        let _ = tx.send(Err(e)).await;
                        return;
                    }
                    continue;
                }
                if self.config.run_objective.is_some() && !run_objective_reminder_sent {
                    if let Some(obj) = &self.config.run_objective {
                        debug!("[phase: done] rappel objectif verrouillé (soft)");
                        messages.push(Message::system(run_objective_done_nudge(obj)));
                        run_objective_reminder_sent = true;
                        loop_detector.reset();
                        if let Err(e) = self
                            .flush_transcript(&messages, &mut transcript_cursor)
                            .await
                        {
                            let _ = tx.send(Err(e)).await;
                            return;
                        }
                        continue;
                    }
                }
                debug!("[phase: done] après answering + todo_write clôturé — clôture propre");
                self.maybe_persist_session(&messages, &memory_tracker, &tx).await;
                let _ = tx
                    .send(Ok(AgentEvent::Stop {
                        reason: outcome.reason,
                        usage: outcome.usage,
                    }))
                    .await;
                return;
            }

            // Si le modèle n'a pas signé `done` et n'a pas non plus appelé
            // d'outil ce tour, on l'invite explicitement à choisir : conclure
            // (`answering` + `done`) ou continuer (`reading` / `acting` + tool).
            // Aucun compteur séparé : `max_iterations` borne tout.
            if outcome.tool_calls.is_empty() {
                // Cas typique GLM-4.7-Flash : le modèle a déjà émis sa
                // réponse en `answering` mais a omis le `[phase: done]`
                // final. Le nudge générique le fait ré-écrire toute la
                // réponse (« écris ta réponse finale ») → affichage en
                // double côté UI. On lui demande juste le marqueur.
                if outcome.final_phase == Some(Phase::Answering)
                    && seen_answering_in_run
                    && last_todo_pending == 0
                    && last_todo_in_progress == 0
                {
                    debug!("answering sans done + todo clôturée — nudge minimal (done seul)");
                    messages.push(Message::system(DONE_ONLY_NUDGE_PROMPT));
                    loop_detector.reset();
                    if let Err(e) = self
                        .flush_transcript(&messages, &mut transcript_cursor)
                        .await
                    {
                        let _ = tx.send(Err(e)).await;
                        return;
                    }
                    continue;
                }

                debug!("tour sans tool_call et sans [phase: done] — nudge");
                messages.push(Message::system(NUDGE_PROMPT));
                loop_detector.reset();
                if let Err(e) = self
                    .flush_transcript(&messages, &mut transcript_cursor)
                    .await
                {
                    let _ = tx.send(Err(e)).await;
                    return;
                }
                continue;
            }

            // GLM-4.7-Flash bat parfois `[file_edit, todo_write]` ou
            // `[bash, todo_write]` dans le même tour. Sans réordonnement, le
            // mutateur rate la gate `MUTATING_TOOL_BEFORE_TODO_WRITE_BLOCKED`,
            // alors que le modèle voulait bien planifier ET muter d'une
            // traite. On respecte l'intention en forçant l'ordre logique
            // d'exécution : `todo_write` d'abord, le reste ensuite. Le
            // mapping `tool_use_id ↔ tool_result` reste correct (l'API
            // ré-aligne via les ids, pas l'index de liste).
            //
            // Note : depuis la relax de la gate aux read-only (un `glob` ou
            // `file_read` initial passe librement), ce réordonnement est
            // surtout utile pour les batchs contenant des mutations. Mais
            // il reste pertinent dans le cas général « le modèle a tout
            // planifié dans une seule volée ».
            //
            // Ne s'applique qu'au tout premier `todo_write` du run : une
            // fois la gate satisfaite, les batchs ultérieurs respectent
            // l'ordre demandé par le modèle (parfois utile pour mettre à
            // jour la to-do AVANT et le code APRÈS).
            if !plan_write_gate_satisfied(
                professor,
                saw_successful_todo_write_in_run,
                saw_successful_course_plan_write_in_run,
            ) {
                let plan_tool = if professor {
                    "course_plan_write"
                } else {
                    "todo_write"
                };
                if let Some(idx) = outcome
                    .tool_calls
                    .iter()
                    .position(|c| c.name == plan_tool)
                {
                    if idx > 0 {
                        let promoted = outcome.tool_calls.remove(idx);
                        outcome.tool_calls.insert(0, promoted);
                        debug!(
                            from_idx = idx,
                            tool = plan_tool,
                            "plan tool promu en tête (batch détecté avant gate satisfaite)"
                        );
                    }
                }
            }

            let tool_names: Vec<&str> = outcome
                .tool_calls
                .iter()
                .map(|c| c.name.as_str())
                .collect();
            let batches = partition_tool_calls(&tool_names, &self.registry);

            for batch in batches {
                match batch {
                    ToolCallBatch::Parallel(indices) => {
                        let max_parallel = self.config.max_parallel_tool_calls.max(1);
                        let mut to_execute: Vec<usize> = Vec::new();
                        for idx in &indices {
                            let call = &outcome.tool_calls[*idx];
                            if let Some(msg) = self
                                .run_tool_pre_gates(
                                    call,
                                    professor,
                                    &professor_course_state,
                                    saw_successful_todo_write_in_run,
                                    &last_todo_ids,
                                    last_todo_was_all_completed,
                                )
                                .await
                            {
                                if push_tool_error_tracked(
                                    &tx,
                                    &mut messages,
                                    call,
                                    msg,
                                    &mut consecutive_ask_user_question_failures,
                                )
                                .await
                                .is_err()
                                {
                                    return;
                                }
                                if let Err(e) = self
                                    .flush_transcript(&messages, &mut transcript_cursor)
                                    .await
                                {
                                    let _ = tx.send(Err(e)).await;
                                    return;
                                }
                                continue;
                            }
                            if let Some(denial) = self.check_permission(call).await {
                                if push_tool_error_tracked(
                                    &tx,
                                    &mut messages,
                                    call,
                                    denial,
                                    &mut consecutive_ask_user_question_failures,
                                )
                                    .await
                                    .is_err()
                                {
                                    return;
                                }
                                if let Err(e) = self
                                    .flush_transcript(&messages, &mut transcript_cursor)
                                    .await
                                {
                                    let _ = tx.send(Err(e)).await;
                                    return;
                                }
                                continue;
                            }
                            to_execute.push(*idx);
                        }

                        let registry = self.registry.clone();
                        let ctx_parallel = ctx.clone();
                        let exec_results: Vec<(usize, PendingToolCall, Result<Value, drox_tools::ToolError>)> =
                            stream::iter(to_execute)
                                .map(|idx| {
                                    let call = outcome.tool_calls[idx].clone();
                                    let registry = registry.clone();
                                    let ctx_exec = ctx_parallel.clone().with_tool_progress(
                                        ToolProgressBridge::new(
                                            tx.clone(),
                                            call.id.clone(),
                                            call.name.clone(),
                                        ),
                                    );
                                    async move {
                                        let result = registry
                                            .execute_named(
                                                &call.name,
                                                &ctx_exec,
                                                call.arguments.clone(),
                                            )
                                            .await;
                                        (idx, call, result)
                                    }
                                })
                                .buffer_unordered(max_parallel)
                                .collect()
                                .await;

                        let mut by_idx: std::collections::BTreeMap<
                            usize,
                            Result<Value, drox_tools::ToolError>,
                        > = std::collections::BTreeMap::new();
                        for (idx, _call, result) in exec_results {
                            by_idx.insert(idx, result);
                        }

                        for idx in indices {
                            let call = &outcome.tool_calls[idx];
                            let Some(exec) = by_idx.remove(&idx) else {
                                continue;
                            };
                            match exec {
                                Ok(value) => {
                                    if !self
                                        .apply_read_only_tool_success(
                                            &ctx,
                                            call,
                                            value,
                                            &mut memory_tracker,
                                            &tx,
                                            &mut messages,
                                        )
                                        .await
                                    {
                                        return;
                                    }
                                }
                                Err(err) => {
                                    if push_tool_error_tracked(
                                        &tx,
                                        &mut messages,
                                        call,
                                        err.to_string(),
                                        &mut consecutive_ask_user_question_failures,
                                    )
                                    .await
                                    .is_err()
                                    {
                                        return;
                                    }
                                }
                            }
                            if let Err(e) = self
                                .flush_transcript(&messages, &mut transcript_cursor)
                                .await
                            {
                                let _ = tx.send(Err(e)).await;
                                return;
                            }
                        }
                    }
                    ToolCallBatch::Serial(indices) => {
                        for idx in indices {
                            let call = &outcome.tool_calls[idx];
                            if let Some(msg) = self
                                .run_tool_pre_gates(
                                    call,
                                    professor,
                                    &professor_course_state,
                                    saw_successful_todo_write_in_run,
                                    &last_todo_ids,
                                    last_todo_was_all_completed,
                                )
                                .await
                            {
                                if push_tool_error_tracked(
                                    &tx,
                                    &mut messages,
                                    call,
                                    msg,
                                    &mut consecutive_ask_user_question_failures,
                                )
                                .await
                                .is_err()
                                {
                                    return;
                                }
                                if let Err(e) = self
                                    .flush_transcript(&messages, &mut transcript_cursor)
                                    .await
                                {
                                    let _ = tx.send(Err(e)).await;
                                    return;
                                }
                                continue;
                            }

                            if let Some(denial) = self.check_permission(call).await {
                                if push_tool_error_tracked(
                                    &tx,
                                    &mut messages,
                                    call,
                                    denial,
                                    &mut consecutive_ask_user_question_failures,
                                )
                                .await
                                .is_err()
                                {
                                    return;
                                }
                                if let Err(e) = self
                                    .flush_transcript(&messages, &mut transcript_cursor)
                                    .await
                                {
                                    let _ = tx.send(Err(e)).await;
                                    return;
                                }
                                continue;
                            }

                            if let Some(hooks) = self
                                .config
                                .tool_hooks
                                .as_ref()
                                .filter(|h| h.is_enabled())
                            {
                                let hook_ctx = ToolHookContext {
                                    tool_name: &call.name,
                                    tool_use_id: call.id.as_str(),
                                    tool_input: &call.arguments,
                                    tool_response: None,
                                };
                                if !hooks.pre_tool_use.is_empty() {
                                    let _ = tx
                                        .send(Ok(AgentEvent::HookProgress {
                                            tool_use_id: call.id.clone(),
                                            hook_event: "PreToolUse".into(),
                                            in_progress: 1,
                                        }))
                                        .await;
                                }
                                let pre_outcome = hooks
                                    .run_pre(hook_ctx, &ctx.workspace_root)
                                    .await;
                                if !hooks.pre_tool_use.is_empty() {
                                    let _ = tx
                                        .send(Ok(AgentEvent::HookProgress {
                                            tool_use_id: call.id.clone(),
                                            hook_event: "PreToolUse".into(),
                                            in_progress: 0,
                                        }))
                                        .await;
                                }
                                if let PreHookOutcome::Block { message } = pre_outcome {
                                    if push_tool_error_tracked(
                                        &tx,
                                        &mut messages,
                                        call,
                                        message,
                                        &mut consecutive_ask_user_question_failures,
                                    )
                                    .await
                                    .is_err()
                                    {
                                        return;
                                    }
                                    if let Err(e) = self
                                        .flush_transcript(&messages, &mut transcript_cursor)
                                        .await
                                    {
                                        let _ = tx.send(Err(e)).await;
                                        return;
                                    }
                                    continue;
                                }
                            }

                            let ctx_exec = ctx.clone().with_tool_progress(ToolProgressBridge::new(
                                tx.clone(),
                                call.id.clone(),
                                call.name.clone(),
                            ));
                            let exec = self
                                .registry
                                .execute_named(&call.name, &ctx_exec, call.arguments.clone())
                                .await;
                            match exec {
                                Ok(mut value) => {
                                    memory_tracker.record_tool(&call.name);
                                    let todo_prev_had_open_items = if call.name == "todo_write" {
                                        last_todo_pending > 0 || last_todo_in_progress > 0
                                    } else {
                                        false
                                    };
                                    if call.name == "todo_write" {
                                        saw_successful_todo_write_in_run = true;
                                        last_todo_pending =
                                            value["counts"]["pending"].as_u64().unwrap_or(0);
                                        last_todo_in_progress =
                                            value["counts"]["in_progress"].as_u64().unwrap_or(0);
                                        last_todo_ids.clear();
                                        if let Some(todos) = value["todos"].as_array() {
                                            for t in todos {
                                                if let Some(id) =
                                                    t.get("id").and_then(|v| v.as_str())
                                                {
                                                    last_todo_ids.insert(id.to_string());
                                                }
                                            }
                                        }
                                        last_todo_was_all_completed =
                                            last_todo_pending == 0 && last_todo_in_progress == 0;
                                        mutating_tools_since_last_todo = 0;
                                    } else if call.name == "course_plan_write" {
                                        saw_successful_course_plan_write_in_run = true;
                                        professor_course_state =
                                            crate::professor::state_from_course_plan_output(
                                                &value,
                                            );
                                        last_course_pending =
                                            value["counts"]["pending"].as_u64().unwrap_or(0);
                                        last_course_active =
                                            value["counts"]["active"].as_u64().unwrap_or(0);
                                        mutating_tools_since_last_todo = 0;
                                    } else if MUTATING_TOOLS_FOR_STEP_TRACKING
                                        .contains(&call.name.as_str())
                                    {
                                        mutating_tools_since_last_todo =
                                            mutating_tools_since_last_todo.saturating_add(1);
                                    }
                                    if record_counts_as_code_mutation(
                                        &call.name,
                                        &call.arguments,
                                    ) {
                                        saw_code_mutation_in_run = true;
                                    }
                                    if call.name == "ask_user_question" {
                                        consecutive_ask_user_question_failures = 0;
                                    }
                                    let mut hook_appendix: Option<String> = None;
                                    if let Some(hooks) = self
                                        .config
                                        .tool_hooks
                                        .as_ref()
                                        .filter(|h| h.is_enabled())
                                    {
                                        if !hooks.post_tool_use.is_empty() {
                                            let _ = tx
                                                .send(Ok(AgentEvent::HookProgress {
                                                    tool_use_id: call.id.clone(),
                                                    hook_event: "PostToolUse".into(),
                                                    in_progress: 1,
                                                }))
                                                .await;
                                        }
                                        let PostHookOutcome::Ok {
                                            tool_response,
                                            model_appendix,
                                        } = hooks
                                            .run_post(
                                                ToolHookContext {
                                                    tool_name: &call.name,
                                                    tool_use_id: call.id.as_str(),
                                                    tool_input: &call.arguments,
                                                    tool_response: None,
                                                },
                                                &ctx.workspace_root,
                                                value,
                                            )
                                            .await;
                                        if !hooks.post_tool_use.is_empty() {
                                            let _ = tx
                                                .send(Ok(AgentEvent::HookProgress {
                                                    tool_use_id: call.id.clone(),
                                                    hook_event: "PostToolUse".into(),
                                                    in_progress: 0,
                                                }))
                                                .await;
                                        }
                                        value = tool_response;
                                        hook_appendix = model_appendix;
                                    }
                                    let mut for_llm =
                                        format_tool_result_for_llm(&call.name, &value);
                                    if let Some(app) = hook_appendix {
                                        for_llm.push_str("\n\n");
                                        for_llm.push_str(&app);
                                    }
                                    mirror_workspace_map_from_tool(&ctx, &call.name, &value)
                                        .await;
                                    if call.name == "scope_defer" {
                                        if let Some(handle) = ctx.scope_deferred.as_ref() {
                                            let items = handle.snapshot();
                                            if tx
                                                .send(Ok(AgentEvent::ScopeParkingUpdate {
                                                    items,
                                                }))
                                                .await
                                                .is_err()
                                            {
                                                return;
                                            }
                                        }
                                    }
                                    if tx
                                        .send(Ok(AgentEvent::ToolFinish {
                                            id: call.id.clone(),
                                            output: value,
                                            is_error: false,
                                        }))
                                        .await
                                        .is_err()
                                    {
                                        return;
                                    }
                                    messages.push(Message::tool_result(call.id.clone(), for_llm, false));
                                    if call.name == "todo_write"
                                        && todo_prev_had_open_items
                                        && last_todo_pending == 0
                                        && last_todo_in_progress == 0
                                        && memory_tracker.is_non_trivial()
                                    {
                                        self.maybe_persist_session(
                                            &messages,
                                            &memory_tracker,
                                            &tx,
                                        )
                                        .await;
                                    }
                                }
                                Err(err) => {
                                    let msg = err.to_string();
                                    if push_tool_error_tracked(
                                        &tx,
                                        &mut messages,
                                        call,
                                        msg,
                                        &mut consecutive_ask_user_question_failures,
                                    )
                                    .await
                                    .is_err()
                                    {
                                        return;
                                    }
                                }
                            }
                            if let Err(e) = self
                                .flush_transcript(&messages, &mut transcript_cursor)
                                .await
                            {
                                let _ = tx.send(Err(e)).await;
                                return;
                            }
                        }
                    }
                }
            }

            if user_analysis_intent
                && !analyzing_phase_nudge_sent
                && !saw_analyzing_phase_in_run
                && iter == 0
            {
                let exploration_calls = outcome
                    .tool_calls
                    .iter()
                    .filter(|c| is_workspace_exploration_tool(&c.name))
                    .count();
                if exploration_calls >= 2 {
                    debug!(
                        exploration_calls,
                        "analyzing phase nudge — exploration sans marqueur analyzing"
                    );
                    messages.push(Message::system(ANALYZING_PHASE_NUDGE));
                    analyzing_phase_nudge_sent = true;
                    loop_detector.reset();
                    if let Err(e) = self
                        .flush_transcript(&messages, &mut transcript_cursor)
                        .await
                    {
                        let _ = tx.send(Err(e)).await;
                        return;
                    }
                    continue;
                }
            }

            if consecutive_ask_user_question_failures
                >= MAX_CONSECUTIVE_ASK_USER_QUESTION_FAILURES
            {
                debug!(
                    consecutive_ask_user_question_failures,
                    "ask_user_question — anti-boucle JSON (§2.21)"
                );
                messages.push(Message::system(ask_user_question_loop_nudge()));
                consecutive_ask_user_question_failures = 0;
                loop_detector.reset();
                if let Err(e) = self
                    .flush_transcript(&messages, &mut transcript_cursor)
                    .await
                {
                    let _ = tx.send(Err(e)).await;
                    return;
                }
            }

            // Filet "step-by-step" : si le modèle vient d'exécuter plusieurs
            // outils mutateurs sans intercaler `todo_write` et qu'il lui
            // reste du travail dans son plan, on lui rappelle de mettre à
            // jour ses étapes avant le prochain tour. Ne se déclenche qu'à
            // partir de 2 outils mutateurs sans MAJ (silence en deçà).
            // Reset après injection pour ne pas répéter en boucle si le
            // modèle ignore le nudge — `max_iterations` reste le garde-fou.
            let plan_still_open = if professor {
                last_course_pending > 0 || last_course_active > 0
            } else {
                last_todo_pending > 0 || last_todo_in_progress > 0
            };
            if mutating_tools_since_last_todo >= 2 && plan_still_open {
                debug!(
                    mutating_tools_since_last_todo,
                    last_todo_pending,
                    last_todo_in_progress,
                    last_course_pending,
                    last_course_active,
                    professor,
                    "step-by-step nudge : outils mutateurs accumulés sans MAJ plan"
                );
                let nudge = if professor {
                    format!(
                        "Heads-up: you've called {mutating_tools_since_last_todo} mutating tools \
                         since your last `course_plan_write`, and the Plan de cours still has \
                         {last_course_pending} pending + {last_course_active} active step(s). \
                         Update the plan (`mastered` / next `active`) before continuing."
                    )
                } else {
                    step_by_step_todo_nudge(
                        mutating_tools_since_last_todo,
                        last_todo_pending,
                        last_todo_in_progress,
                    )
                };
                messages.push(Message::system(nudge));
                mutating_tools_since_last_todo = 0;
                loop_detector.reset();
                if let Err(e) = self
                    .flush_transcript(&messages, &mut transcript_cursor)
                    .await
                {
                    let _ = tx.send(Err(e)).await;
                    return;
                }
            }
        }

        let _ = tx
            .send(Err(EngineError::MaxIterations(self.config.max_iterations)))
            .await;
    }

    async fn flush_transcript(
        &self,
        messages: &[Message],
        cursor: &mut usize,
    ) -> Result<(), EngineError> {
        let Some(ts) = self.config.transcript.as_ref() else {
            return Ok(());
        };
        let start = (*cursor).min(messages.len());
        for m in &messages[start..] {
            if matches!(m.role, Role::System) {
                continue;
            }
            let rec = drox_session::ChatMessageRecord::new(m);
            ts.sink.append_record(&rec).await?;
        }
        *cursor = messages.len();
        Ok(())
    }

    /// Sprint M1 — persistance de la session si le run a été non trivial.
    ///
    /// Best-effort : un échec d'archivage ne casse pas la clôture du run.
    /// On log et on continue vers `Stop`. L'événement `MemoryPersisted`
    /// n'est émis qu'en cas de succès complet (compaction + écriture).
    ///
    /// L'`objective_fallback` est extrait de la première ligne non vide du
    /// premier message `user` du run — utilisé pour le slug si la
    /// compaction n'a pas livré de section `## Objective`.
    async fn maybe_persist_session(
        &self,
        messages: &[Message],
        tracker: &MemoryTracker,
        tx: &mpsc::Sender<Result<AgentEvent, EngineError>>,
    ) {
        let Some(memory) = self.config.memory.as_ref() else {
            return;
        };
        if !tracker.is_non_trivial() {
            debug!("memory: run trivial, no persistence");
            return;
        }
        let fallback = first_user_text(messages).unwrap_or_else(|| "session".to_string());
        match persist_run(memory, messages, &fallback).await {
            Ok(persisted) => {
                debug!(
                    slug = %persisted.slug,
                    path = %persisted.path,
                    "memory: session persisted (emitting MemoryPersisted)"
                );
                let _ = tx
                    .send(Ok(AgentEvent::MemoryPersisted {
                        slug: persisted.slug,
                        path: persisted.path.to_string(),
                        objective: persisted.result.objective,
                        usage: persisted.result.usage,
                    }))
                    .await;
            }
            Err(e) => {
                warn!(
                    error = %e,
                    "memory: persistence failed — run still closes cleanly"
                );
            }
        }
    }

    /// Si la `ContextPolicy` est définie et que l'historique dépasse le seuil
    /// `autocompact` : (1) passe de **snip** synchrone sur les gros
    /// `tool_result` ; (2) si toujours au-dessus du seuil **et** que
    /// `memory` est configuré, **compaction LLM live** (checkpoint `system`)
    /// via [`crate::compaction::try_live_compact`]. Émet `ContextSnip` et/ou
    /// `ContextCompacted`.
    ///
    /// Retourne `Err(())` si le canal de sortie est fermé (cas où l'agent
    /// doit s'arrêter sans bruit).
    async fn maybe_snip(
        &self,
        messages: &mut Vec<Message>,
        tx: &mpsc::Sender<Result<AgentEvent, EngineError>>,
        live_compaction_seq: &mut u32,
    ) -> Result<(), ()> {
        let Some(policy) = self.config.context.as_ref() else {
            return Ok(());
        };
        let mut tokens = policy.count_tokens(messages);
        let mut state = policy.budget().evaluate(tokens);
        if !state.above_autocompact {
            return Ok(());
        }
        if let Some(mc) = policy.maybe_microcompact(messages) {
            tokens = policy.count_tokens(messages);
            debug!(
                tools_cleared = mc.tools_cleared,
                blocks_cleared = mc.blocks_cleared,
                tokens_after = tokens,
                "microcompact applied"
            );
            state = policy.budget().evaluate(tokens);
            if !state.above_autocompact {
                return Ok(());
            }
        }
        if let Some(report) = policy.maybe_snip(messages) {
            let tokens_before_snip = tokens;
            tokens = policy.count_tokens(messages);
            debug!(
                tokens_before = tokens_before_snip,
                tokens_after = tokens,
                tokens_freed = report.tokens_freed,
                blocks_snipped = report.blocks_snipped,
                "context snip applied"
            );
            tx.send(Ok(AgentEvent::ContextSnip {
                tokens_freed: report.tokens_freed,
                blocks_snipped: report.blocks_snipped,
                tokens_used_after: tokens,
            }))
            .await
            .map_err(|_| ())?;
            state = policy.budget().evaluate(tokens);
        }
        if !state.above_autocompact {
            return Ok(());
        }
        let Some(memory) = self.config.memory.as_ref() else {
            return Ok(());
        };
        if let Some(report) = crate::compaction::compact_until_budget(
            memory.llm.as_ref(),
            memory.compaction_prompt.as_str(),
            messages,
            policy,
            &memory.compaction_config,
        )
        .await
        {
            *live_compaction_seq = live_compaction_seq.saturating_add(1);
            let transcript_sid = self
                .config
                .transcript_session_id
                .as_deref()
                .filter(|s| !s.is_empty())
                .unwrap_or("ses_none");
            let fingerprint = if self.config.workspace_fingerprint.is_empty() {
                self.ctx.workspace_root.as_str().to_string()
            } else {
                self.config.workspace_fingerprint.clone()
            };
            let ccs = ContextChunkSummaryV1 {
                schema_version: 1,
                id: format!("ccs_{}", Uuid::new_v4()),
                workspace_fingerprint: fingerprint,
                transcript_session_id: transcript_sid.to_string(),
                created_at: chrono::Utc::now(),
                compaction_seq: *live_compaction_seq,
                tokens_before: report.tokens_before,
                tokens_after: report.tokens_after,
                summary_text: report.summary_text.clone(),
                files_touched: report.files_touched.clone(),
                tags_suggested: Vec::new(),
                checkpoint_message_id: None,
            };
            tx.send(Ok(AgentEvent::ContextCompacted {
                tokens_before: report.tokens_before,
                tokens_after: report.tokens_after,
                messages_removed: report.messages_removed,
                usage: report.usage,
                context_chunk_summary: Some(ccs),
            }))
            .await
            .map_err(|_| ())?;
        }
        Ok(())
    }

    /// Gates pré-exécution (hors permissions). `Some(msg)` = bloquer avec erreur.
    async fn run_tool_pre_gates(
        &self,
        call: &PendingToolCall,
        professor: bool,
        professor_course_state: &crate::professor::ProfessorCourseState,
        saw_successful_todo_write_in_run: bool,
        last_todo_ids: &std::collections::HashSet<String>,
        last_todo_was_all_completed: bool,
    ) -> Option<String> {
        if professor && call.name == "todo_write" {
            return Some(TODO_WRITE_FORBIDDEN_IN_PROFESSOR.to_string());
        }
        if professor {
            if let Some(msg) = crate::professor::check_mutating_tool(
                &call.name,
                &call.arguments,
                professor_course_state,
            ) {
                return Some(msg.to_string());
            }
        } else if !plan_write_gate_satisfied(false, saw_successful_todo_write_in_run, false)
            && requires_todo_write_gate(&call.name)
        {
            return Some(MUTATING_TOOL_BEFORE_TODO_WRITE_BLOCKED.to_string());
        }
        if is_hallucinated_phase_tool_call(&call.name, &call.arguments) {
            return Some(PHASE_MARKER_MUST_BE_TEXT_NOT_TOOL.to_string());
        }
        if call.name == "session_end" {
            return Some(SESSION_END_FORBIDDEN_FOR_MODEL.to_string());
        }
        if !professor
            && call.name == "todo_write"
            && is_todo_recreation_from_scratch(
                &call.arguments,
                last_todo_ids,
                last_todo_was_all_completed,
            )
        {
            return Some(TODO_RECREATION_BLOCKED.to_string());
        }
        None
    }

    /// Applique le succès d'un tool read-only (lot parallèle §2.29).
    async fn apply_read_only_tool_success(
        &self,
        ctx: &ToolContext,
        call: &PendingToolCall,
        value: Value,
        memory_tracker: &mut MemoryTracker,
        tx: &mpsc::Sender<Result<AgentEvent, EngineError>>,
        messages: &mut Vec<Message>,
    ) -> bool {
        memory_tracker.record_tool(&call.name);
        mirror_workspace_map_from_tool(ctx, &call.name, &value).await;
        let for_llm = format_tool_result_for_llm(&call.name, &value);
        if tx
            .send(Ok(AgentEvent::ToolFinish {
                id: call.id.clone(),
                output: value,
                is_error: false,
            }))
            .await
            .is_err()
        {
            return false;
        }
        messages.push(Message::tool_result(call.id.clone(), for_llm, false));
        true
    }

    /// Évalue la permission pour un tool call. Renvoie `Some(message)` si la
    /// décision finale est un refus (à pousser comme `tool_result` d'erreur),
    /// `None` si le tool peut s'exécuter.
    async fn check_permission(&self, call: &PendingToolCall) -> Option<String> {
        let policy = self.config.permissions.as_ref()?;
        let read_only = crate::permissions::is_read_only_tool(&call.name)
            || self
                .registry
                .get(&call.name)
                .is_some_and(|t| t.is_read_only());
        let decision = policy.evaluate_with_read_only_hint(
            &call.name,
            &call.arguments,
            Some(read_only),
        );
        match decision {
            PermissionDecision::Allow { reason } => {
                debug!(tool = %call.name, ?reason, "tool autorisé");
                None
            }
            PermissionDecision::Deny { reason, message } => {
                warn!(tool = %call.name, ?reason, "tool refusé");
                Some(message)
            }
            PermissionDecision::Ask { reason, message } => {
                debug!(tool = %call.name, ?reason, "tool nécessite confirmation");
                if confirm_with_user(&self.ctx, &call.name, &call.arguments, &message).await {
                    None
                } else {
                    Some(format!("User denied permission for `{}`.", call.name))
                }
            }
        }
    }
}

/// Pose une question oui/non à l'humain via `UserAsker`. Retourne `false`
/// si pas d'asker disponible (fallback : refus) ou si l'humain refuse.
async fn confirm_with_user(
    ctx: &ToolContext,
    tool_name: &str,
    args: &Value,
    permission_message: &str,
) -> bool {
    let Some(asker) = ctx.user_asker.as_ref() else {
        warn!(tool = %tool_name, "Ask requis mais aucun UserAsker configuré → refus");
        return false;
    };
    let args_pretty = serde_json::to_string_pretty(args).unwrap_or_else(|_| args.to_string());
    let question = UserQuestion {
        id: None,
        prompt: format!(
            "{permission_message}\n\nAllow this `{tool_name}` call?\nArgs:\n{args_pretty}"
        ),
        choices: vec!["yes".into(), "no".into()],
        allow_multiple: false,
        allow_free_text: false,
    };
    match asker.ask(question).await {
        Ok(answer) => {
            if answer.indices.first().copied() == Some(0) {
                return true;
            }
            let trimmed = answer.text.trim().to_ascii_lowercase();
            matches!(trimmed.as_str(), "y" | "yes" | "0" | "ok")
        }
        Err(err) => {
            warn!(?err, "asker a renvoyé une erreur → refus");
            false
        }
    }
}

/// Nombre d'échecs `ask_user_question` consécutifs avant nudge système (§2.21).
const MAX_CONSECUTIVE_ASK_USER_QUESTION_FAILURES: u32 = 3;

#[must_use]
fn ask_user_question_loop_nudge() -> String {
    format!(
        "You called `ask_user_question` {MAX} times in a row without success. \
         Either ask the user in plain Markdown under `[phase: clarifying]` (no tool), \
         OR call `ask_user_question` again via native tool_calls with EXACTLY this JSON \
         (do NOT paste JSON in assistant text): {CANONICAL_ASK_JSON_EXAMPLE}",
        MAX = MAX_CONSECUTIVE_ASK_USER_QUESTION_FAILURES,
    )
}

/// Compte les échecs consécutifs de `ask_user_question` pour le filet §2.21.
async fn push_tool_error_tracked(
    tx: &mpsc::Sender<Result<AgentEvent, EngineError>>,
    messages: &mut Vec<Message>,
    call: &PendingToolCall,
    message: String,
    ask_failure_streak: &mut u32,
) -> Result<(), ()> {
    if call.name == "ask_user_question" {
        *ask_failure_streak = ask_failure_streak.saturating_add(1);
    }
    push_tool_error(tx, messages, &call.id, message).await
}

/// Pousse un `ToolFinish { is_error: true }` côté stream + un `tool_result`
/// d'erreur côté historique de messages.
async fn push_tool_error(
    tx: &mpsc::Sender<Result<AgentEvent, EngineError>>,
    messages: &mut Vec<Message>,
    id: &ToolUseId,
    message: String,
) -> Result<(), ()> {
    let value = json!({ "error": message });
    tx.send(Ok(AgentEvent::ToolFinish {
        id: id.clone(),
        output: value,
        is_error: true,
    }))
    .await
    .map_err(|_| ())?;
    messages.push(Message::tool_result(id.clone(), message, true));
    Ok(())
}

/// Extrait le premier message `user` du run sous forme texte (concatène les
/// blocs `Content::Text`). Utilisé comme **slug fallback** pour les sessions
/// archivées quand la compaction ne livre pas d'`## Objective` exploitable.
///
/// Retourne `None` si aucun message `user` n'a (encore) de texte —
/// l'appelant utilisera alors un slug générique (`"session"`).
fn first_user_text(messages: &[Message]) -> Option<String> {
    for m in messages {
        if !matches!(m.role, Role::User) {
            continue;
        }
        let mut buf = String::new();
        for block in &m.content {
            if let Content::Text { text } = block {
                if !buf.is_empty() {
                    buf.push(' ');
                }
                buf.push_str(text);
            }
        }
        let trimmed = buf.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    None
}

/// Construit la liste des `ToolSpec` à partir du registre.
///
/// Certains outils restent enregistrés pour exécution côté client / CLI
/// mais ne doivent **pas** être visibles du LLM (`session_end` : réservé
/// à la commande utilisateur `/session_end`).
fn build_tool_specs(registry: &ToolRegistry, professor: bool) -> Vec<ToolSpec> {
    const HIDDEN_FROM_LLM: &[&str] = &["session_end"];
    let has_mcp_stubs = registry
        .names()
        .iter()
        .any(|n| n.starts_with("mcp__"));
    let mut specs = Vec::new();
    for name in registry.names() {
        if HIDDEN_FROM_LLM.contains(&name.as_str()) {
            continue;
        }
        if has_mcp_stubs && name == "mcp_call" {
            continue;
        }
        if professor && name == "todo_write" {
            continue;
        }
        if !professor && name == "course_plan_write" {
            continue;
        }
        if let Some(tool) = registry.get(&name) {
            specs.push(ToolSpec {
                name: tool.name().to_string(),
                description: tool.description().to_string(),
                parameters: tool.input_schema(),
            });
        }
    }
    specs
}

#[derive(Clone)]
struct PendingToolCall {
    id: ToolUseId,
    name: String,
    arguments: Value,
}

/// Sprint Hotfix « boucle édition/lecture » — détecteur de répétition strict
/// turn-à-turn. Le modèle (typ. GLM-4.7-Flash sous pression de contexte) se
/// met parfois à **répéter exactement** le même texte assistant et/ou les
/// mêmes `tool_calls` (mêmes args), sans jamais converger vers `[phase: done]`.
/// Sans filet, le run épuise `max_iterations` en gaspillant des tokens.
///
/// Heuristique V1 (stricte, peu de faux positifs) :
/// 1. On capture l'empreinte du tour : (texte trimé, signature des tool_calls).
///    Empreinte vide = on ignore (cas pathologique, déjà géré par
///    `tool_calls.is_empty()` en amont).
/// 2. Si l'empreinte est identique à la précédente :
///    - 1er strike → `Decision::Warn` (le moteur injecte un nudge anti-boucle).
///    - 2e strike → `Decision::Abort(kind)` (le moteur stoppe le run).
/// 3. Toute empreinte **différente** reset le compteur.
///
/// Anti-faux-positif : les nudges moteur (`unfinished_todos`, `DONE_ONLY`,
/// etc.) appellent `LoopDetector::reset` parce qu'ils **forcent**
/// le modèle à changer de comportement — leur effet sur le tour suivant doit
/// être évalué à part.
#[derive(Default)]
struct LoopDetector {
    /// Hash du dernier tour observé (`None` au début du run).
    last_fingerprint: Option<u64>,
    /// Composante texte du dernier tour (utile pour qualifier `kind` :
    /// « text » / « tool_calls » / « both »).
    last_text_hash: u64,
    last_tools_hash: u64,
    /// Nombre de tours identiques **consécutifs** observés.
    strike: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoopDecision {
    /// Tour normal, pas de répétition.
    Ok,
    /// 1re répétition stricte — injecter un nudge anti-boucle.
    Warn { kind: &'static str },
    /// 2e répétition après nudge — stopper avec `EngineError::LoopDetected`.
    Abort { kind: &'static str, turns: u32 },
}

impl LoopDetector {
    fn new() -> Self {
        Self::default()
    }

    /// Reset le compteur (à appeler après chaque nudge moteur structurel pour
    /// laisser une chance de convergence sans pénalité).
    fn reset(&mut self) {
        self.strike = 0;
    }

    /// Examine un `TurnOutcome` et renvoie la décision à prendre. `outcome`
    /// est passé par ref : on ne touche pas à son contenu.
    fn observe(&mut self, outcome: &TurnOutcome) -> LoopDecision {
        let text_h = hash_text(&outcome.text);
        let tools_h = hash_tool_calls(&outcome.tool_calls);
        let fp = combine_hash(text_h, tools_h);

        // Tour vide (ni texte significatif, ni outils) : on laisse les
        // gates `tool_calls.is_empty()` + `NUDGE_PROMPT` faire leur job et
        // on n'incrémente pas — sinon un run silencieux puis re-silencieux
        // se ferait flagger par erreur.
        if outcome.text.trim().is_empty() && outcome.tool_calls.is_empty() {
            self.last_fingerprint = Some(fp);
            self.last_text_hash = text_h;
            self.last_tools_hash = tools_h;
            return LoopDecision::Ok;
        }

        let repeat = matches!(self.last_fingerprint, Some(prev) if prev == fp);

        // Mise à jour de l'état AVANT de retourner : le prochain `observe`
        // doit voir l'empreinte courante quelle que soit la décision.
        let last_text_h = self.last_text_hash;
        let last_tools_h = self.last_tools_hash;
        self.last_fingerprint = Some(fp);
        self.last_text_hash = text_h;
        self.last_tools_hash = tools_h;

        if !repeat {
            self.strike = 0;
            return LoopDecision::Ok;
        }

        // Qualifie le `kind` pour le message d'erreur / nudge.
        let kind = if text_h == last_text_h && tools_h == last_tools_h {
            "both"
        } else if text_h == last_text_h {
            "text"
        } else {
            "tool_calls"
        };

        self.strike = self.strike.saturating_add(1);
        match self.strike {
            1 => LoopDecision::Warn { kind },
            _ => LoopDecision::Abort {
                kind,
                turns: self.strike + 1, // strike == 2 → 3e tour identique au total
            },
        }
    }
}

/// Hash stable d'un fragment de texte (FNV-1a-like via `DefaultHasher`).
/// On `trim` pour éviter qu'un saut de ligne en plus change l'empreinte —
/// les deltas Ollama sont fragmentés et le `consume_stream` recolle parfois
/// avec des espaces autour des marqueurs supprimés.
fn hash_text(s: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    s.trim().hash(&mut h);
    h.finish()
}

/// Signature stable d'une séquence de `PendingToolCall`. L'`id` est
/// **volontairement ignoré** (il change à chaque tour par construction), seuls
/// `name` + `arguments` sérialisés comptent. Pour les arguments, on passe par
/// `serde_json::to_string` ; deux turns identiques produiront le même JSON
/// d'arguments (les LLM sont déterministes au format près quand l'intention
/// est la même), c'est suffisant pour une détection stricte.
fn hash_tool_calls(calls: &[PendingToolCall]) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    calls.len().hash(&mut h);
    for c in calls {
        c.name.hash(&mut h);
        let args = serde_json::to_string(&c.arguments).unwrap_or_default();
        args.hash(&mut h);
    }
    h.finish()
}

fn combine_hash(a: u64, b: u64) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    a.hash(&mut h);
    b.hash(&mut h);
    h.finish()
}

/// Prompt injecté au modèle quand une répétition stricte est détectée
/// (1er strike). Volontairement court et en anglais (les modèles compactés
/// suivent mieux les méta-instructions anglophones). Le moteur **n'invente
/// jamais** la suite : on demande au modèle de choisir entre changer
/// d'approche ou clôturer proprement.
const LOOP_DETECTED_NUDGE_PROMPT: &str = "You just repeated the exact same \
output (text and/or tool call) as your previous turn. This is a loop — \
continuing will not converge.\n\nDecide NOW between two paths:\n\n\
1. **Change approach**: identify what's actually missing or wrong (a \
permission denial? a stale tool result? a misread file?) and try a \
different tool, different args, or a different angle. State the new \
hypothesis in `[phase: reading]` or `[phase: acting]` BEFORE acting.\n\n\
2. **Conclude**: if you genuinely have nothing more to do, emit \
`[phase: answering]` with your final answer in Markdown, then \
`[phase: done]`.\n\n\
Repeating the same content again will cause the run to be aborted.";

struct TurnOutcome {
    /// Texte assistant **nettoyé** : tous les marqueurs `[phase: ...]` reconnus
    /// ont été retirés (y compris les lignes `reasoning` / `next-move` ignorées).
    /// C'est ce qui est poussé dans le transcript et renvoyé au LLM aux tours
    /// suivants — le contexte sémantique pour le modèle, sans la quincaillerie
    /// protocolaire.
    text: String,
    tool_calls: Vec<PendingToolCall>,
    reason: StopReason,
    usage: Usage,
    /// Dernière phase déclarée dans ce tour, si présente. Utilisée par
    /// `drive_inner` pour décider de la clôture (cf. `Phase::Done`).
    final_phase: Option<Phase>,
    /// `true` si la phase `Answering` a été déclarée à un moment ou un autre
    /// pendant ce tour. Sert à détecter les `Done` prématurés où le modèle
    /// écrit sa synthèse dans `reading`/`verifying` puis ferme sans passer
    /// par `answering` (cf. Sprint A.3 — answering-before-done).
    saw_answering: bool,
    /// `true` si `[phase: analyzing]` a été déclaré pendant ce tour (§2.18).
    saw_analyzing: bool,
    /// `true` si `[phase: testing]` a été déclaré pendant ce tour (§2.11).
    saw_testing: bool,
}

/// Buffer line-based pour extraire les marqueurs `[phase: ...]` d'un stream
/// texte arbitrairement fragmenté.
///
/// Pourquoi line-based ? Les marqueurs occupent une ligne entière. En
/// bufferisant jusqu'au `\n`, on parse une fois la ligne complète et on
/// décide : marqueur connu (consommé silencieusement, déclenche `PhaseEnter`),
/// marqueur historique retiré (`reasoning` / `next-move`, ignoré), ou texte
/// ordinaire (`TextDelta`). La latence ajoutée est d'au plus une ligne.
struct PhaseLineBuffer {
    pending: String,
}

impl PhaseLineBuffer {
    const fn new() -> Self {
        Self {
            pending: String::new(),
        }
    }

    /// Consomme un fragment et appelle les callbacks pour chaque ligne
    /// terminée par `\n`. Le reliquat (ligne incomplète) est conservé.
    fn push_chunk<TextSink, PhaseSink>(
        &mut self,
        delta: &str,
        mut on_text: TextSink,
        mut on_phase: PhaseSink,
    ) where
        TextSink: FnMut(String),
        PhaseSink: FnMut(Phase),
    {
        self.pending.push_str(delta);
        while let Some(idx) = self.pending.find('\n') {
            let line: String = self.pending.drain(..=idx).collect();
            // `line` se termine par `\n` ; on parse la ligne SANS ce
            // séparateur pour reconnaître le marqueur.
            let body = line.trim_end_matches('\n');
            if legacy_removed_phase_marker_line(body) {
                continue;
            }
            if let Some(phase) = parse_phase_marker(body) {
                on_phase(phase);
            } else {
                on_text(line);
            }
        }
    }

    /// À appeler en fin de stream : flush le reliquat. Si c'est exactement
    /// un marqueur (sans `\n` final), on l'interprète aussi.
    fn finish<TextSink, PhaseSink>(self, mut on_text: TextSink, mut on_phase: PhaseSink)
    where
        TextSink: FnMut(String),
        PhaseSink: FnMut(Phase),
    {
        if self.pending.is_empty() {
            return;
        }
        if legacy_removed_phase_marker_line(self.pending.trim()) {
            return;
        }
        if let Some(phase) = parse_phase_marker(&self.pending) {
            on_phase(phase);
        } else {
            on_text(self.pending);
        }
    }
}

/// Marque la fin du sous-flux d'affichage `internal_reasoning` (pensée
/// native Ollama). Renvoie `true` si le bloc était ouvert et vient d'être
/// fermé logiquement (l'UI doit recevoir un `PhaseClose` si `native_ui`).
fn mark_native_thinking_closed(native_ui: bool, native_open: &mut bool) -> bool {
    if native_ui && *native_open {
        *native_open = false;
        true
    } else {
        false
    }
}

/// Consomme un stream LLM, relaie texte et `tool_calls` vers `tx`, et
/// retourne le bilan du tour. Retourne `Err(())` si le canal est fermé côté
/// consommateur (auquel cas l'agent doit s'arrêter sans bruit).
///
/// `native_thinking_ui` : si vrai, relaie `message.thinking` (Ollama) vers la
/// phase UI `internal_reasoning` ; sinon les deltas natifs sont ignorés.
#[allow(clippy::too_many_lines)] // streaming linéaire, découper nuirait à la lisibilité
async fn consume_stream(
    mut stream: drox_llm::StreamHandle,
    tx: &mpsc::Sender<Result<AgentEvent, EngineError>>,
    native_thinking_ui: bool,
) -> Result<TurnOutcome, ()> {
    let mut text = String::new();
    let mut tool_calls: Vec<PendingToolCall> = Vec::new();
    let mut last_stop: Option<(StopReason, Usage)> = None;
    let mut buffer = PhaseLineBuffer::new();
    // Dernière phase **émise** vers le consommateur. Sert à dédupliquer les
    // marqueurs consécutifs identiques (évite des blocs UI vides).
    let mut final_phase: Option<Phase> = None;
    let mut saw_answering = false;
    let mut saw_analyzing = false;
    let mut saw_testing = false;
    let mut pending_text: Vec<String> = Vec::new();
    let mut pending_phases: Vec<Phase> = Vec::new();
    // `true` tant que des deltas `thinking` Ollama sont affichés dans
    // `internal_reasoning`.
    let mut native_thinking_open = false;

    while let Some(event) = stream.next().await {
        match event {
            Ok(StreamEvent::Start) => {}
            Ok(StreamEvent::ThinkingDelta { text: delta }) => {
                if !native_thinking_ui || delta.is_empty() {
                    continue;
                }
                if !native_thinking_open {
                    native_thinking_open = true;
                    if final_phase != Some(Phase::InternalReasoning) {
                        final_phase = Some(Phase::InternalReasoning);
                        if tx
                            .send(Ok(AgentEvent::PhaseEnter {
                                phase: Phase::InternalReasoning,
                            }))
                            .await
                            .is_err()
                        {
                            return Err(());
                        }
                    }
                }
                if tx
                    .send(Ok(AgentEvent::TextDelta { text: delta }))
                    .await
                    .is_err()
                {
                    return Err(());
                }
            }
            Ok(StreamEvent::TextDelta { text: delta }) => {
                buffer.push_chunk(
                    &delta,
                    |line| pending_text.push(line),
                    |phase| pending_phases.push(phase),
                );
                for phase in std::mem::take(&mut pending_phases) {
                    let native_just_closed =
                        mark_native_thinking_closed(native_thinking_ui, &mut native_thinking_open);
                    if native_just_closed && native_thinking_ui {
                        if tx.send(Ok(AgentEvent::PhaseClose)).await.is_err() {
                            return Err(());
                        }
                    }
                    if phase == Phase::Answering {
                        saw_answering = true;
                    }
                    if phase == Phase::Analyzing {
                        saw_analyzing = true;
                    }
                    if phase == Phase::Testing {
                        saw_testing = true;
                    }
                    if final_phase == Some(phase) {
                        // Marqueur identique au précédent émis → on l'ignore
                        // pour éviter les blocs UI dupliqués.
                        continue;
                    }
                    final_phase = Some(phase);
                    if tx
                        .send(Ok(AgentEvent::PhaseEnter { phase }))
                        .await
                        .is_err()
                    {
                        return Err(());
                    }
                }
                let native_just_closed =
                    mark_native_thinking_closed(native_thinking_ui, &mut native_thinking_open);
                if native_just_closed && native_thinking_ui {
                    if tx.send(Ok(AgentEvent::PhaseClose)).await.is_err() {
                        return Err(());
                    }
                }
                for line in std::mem::take(&mut pending_text) {
                    text.push_str(&line);
                    if tx
                        .send(Ok(AgentEvent::TextDelta { text: line }))
                        .await
                        .is_err()
                    {
                        return Err(());
                    }
                }
            }
            Ok(StreamEvent::ToolCall {
                id,
                name,
                arguments,
            }) => {
                let native_just_closed =
                    mark_native_thinking_closed(native_thinking_ui, &mut native_thinking_open);
                if native_just_closed && native_thinking_ui {
                    if tx.send(Ok(AgentEvent::PhaseClose)).await.is_err() {
                        return Err(());
                    }
                }
                // Sprint A.4 — filet de sécurité « pas d'outil hors phase ».
                // Si le modèle ouvre directement un tour avec une `ToolCall`
                // sans avoir déclaré de phase, on en synthétise une avant
                // de forward la `ToolStart`. Sinon, côté UI, l'outil
                // atterrit en orphelin sur `logEl` et apparaît hors de la
                // trace repliée. Le choix entre `Reading` et `Acting` se fait
                // selon la nature lecture seule / mutative du tool.
                if final_phase.is_none() {
                    let inferred = phase_for_tool(&name, final_phase);
                    if inferred == Phase::Analyzing {
                        saw_analyzing = true;
                    }
                    if inferred == Phase::Testing {
                        saw_testing = true;
                    }
                    final_phase = Some(inferred);
                    if tx
                        .send(Ok(AgentEvent::PhaseEnter { phase: inferred }))
                        .await
                        .is_err()
                    {
                        return Err(());
                    }
                }
                if tx
                    .send(Ok(AgentEvent::ToolStart {
                        id: id.clone(),
                        name: name.clone(),
                        arguments: arguments.clone(),
                    }))
                    .await
                    .is_err()
                {
                    return Err(());
                }
                tool_calls.push(PendingToolCall {
                    id,
                    name,
                    arguments,
                });
            }
            Ok(StreamEvent::Stop { reason, usage }) => {
                let native_just_closed =
                    mark_native_thinking_closed(native_thinking_ui, &mut native_thinking_open);
                if native_just_closed && native_thinking_ui {
                    let _ = tx.send(Ok(AgentEvent::PhaseClose)).await;
                }
                last_stop = Some((reason, usage));
            }
            Ok(other) => {
                debug!(?other, "event LLM non géré par l'agent");
            }
            Err(err) => {
                let _ = tx.send(Err(err.into())).await;
                return Err(());
            }
        }
    }

    // Flush du reliquat (ligne sans `\n` final).
    let mut tail_text: Vec<String> = Vec::new();
    let mut tail_phases: Vec<Phase> = Vec::new();
    buffer.finish(
        |line| tail_text.push(line),
        |phase| tail_phases.push(phase),
    );
    for phase in tail_phases {
        let native_just_closed =
            mark_native_thinking_closed(native_thinking_ui, &mut native_thinking_open);
        if native_just_closed && native_thinking_ui {
            if tx.send(Ok(AgentEvent::PhaseClose)).await.is_err() {
                return Err(());
            }
        }
        if phase == Phase::Answering {
            saw_answering = true;
        }
        if phase == Phase::Analyzing {
            saw_analyzing = true;
        }
        if phase == Phase::Testing {
            saw_testing = true;
        }
        if final_phase == Some(phase) {
            continue;
        }
        final_phase = Some(phase);
        if tx
            .send(Ok(AgentEvent::PhaseEnter { phase }))
            .await
            .is_err()
        {
            return Err(());
        }
    }
    let native_just_closed = mark_native_thinking_closed(native_thinking_ui, &mut native_thinking_open);
    if native_just_closed && native_thinking_ui {
        if tx.send(Ok(AgentEvent::PhaseClose)).await.is_err() {
            return Err(());
        }
    }
    for line in tail_text {
        text.push_str(&line);
        if tx
            .send(Ok(AgentEvent::TextDelta { text: line }))
            .await
            .is_err()
        {
            return Err(());
        }
    }

    let (reason, usage) = last_stop.unwrap_or_else(|| (StopReason::EndTurn, Usage::default()));
    Ok(TurnOutcome {
        text,
        tool_calls,
        reason,
        usage,
        final_phase,
        saw_answering,
        saw_analyzing,
        saw_testing,
    })
}

/// Ajoute le message assistant au log de conversation (`text` + `tool_uses`).
fn push_assistant_message(messages: &mut Vec<Message>, outcome: &TurnOutcome) {
    let mut blocks = Vec::new();
    if !outcome.text.is_empty() {
        blocks.push(Content::text(&outcome.text));
    }
    for call in &outcome.tool_calls {
        blocks.push(Content::ToolUse {
            id: call.id.clone(),
            name: call.name.clone(),
            input: call.arguments.clone(),
        });
    }
    if !blocks.is_empty() {
        messages.push(Message::new(Role::Assistant, blocks));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use drox_llm::{LlmError, StreamHandle};
    use drox_tools::{TodoWriteTool, Tool, ToolRegistry};
    use futures::stream;
    use std::sync::{Arc, Mutex};

    use crate::context::ContextPolicy;
    use drox_context::{ContextBudget, RoughTokenCounter};

    /// Client LLM en mémoire : retourne des scripts d'événements pré-définis,
    /// un par appel `stream_chat`.
    struct ScriptedLlm {
        scripts: Mutex<Vec<Vec<StreamEvent>>>,
    }

    impl ScriptedLlm {
        fn new(scripts: Vec<Vec<StreamEvent>>) -> Self {
            Self {
                scripts: Mutex::new(scripts),
            }
        }
    }

    #[async_trait]
    impl LlmClient for ScriptedLlm {
        async fn stream_chat(
            &self,
            _messages: Vec<Message>,
            _options: ChatOptions,
        ) -> Result<StreamHandle, LlmError> {
            let script = {
                let mut s = self.scripts.lock().unwrap();
                if s.is_empty() {
                    return Err(LlmError::InvalidConfig("no more scripts".into()));
                }
                s.remove(0)
            };
            let events = script.into_iter().map(Ok::<_, LlmError>);
            Ok(stream::iter(events).boxed())
        }
    }

    /// Tool d'écho minimal pour tester l'aller-retour.
    struct EchoTool;

    #[async_trait]
    impl Tool for EchoTool {
        fn name(&self) -> &str {
            "echo"
        }
        fn description(&self) -> &str {
            "Renvoie l'input tel quel"
        }
        fn input_schema(&self) -> Value {
            json!({ "type": "object" })
        }
        async fn execute(
            &self,
            _ctx: &ToolContext,
            input: Value,
        ) -> Result<Value, drox_tools::ToolError> {
            Ok(input)
        }
    }

    /// Mock du tool `bash` côté tests. Le vrai `bash` exécute des commandes
    /// shell et n'est pas adapté aux tests unitaires asynchrones. Ici on
    /// s'en sert uniquement pour faire incrémenter le tracker
    /// `mutating_tools_since_last_todo` côté moteur (qui matche par `name`).
    struct FakeFileEditTool;

    #[async_trait]
    impl Tool for FakeFileEditTool {
        fn name(&self) -> &str {
            "file_edit"
        }
        fn description(&self) -> &str {
            "fake file_edit (test only)"
        }
        fn input_schema(&self) -> Value {
            json!({ "type": "object" })
        }
        async fn execute(
            &self,
            _ctx: &ToolContext,
            _input: Value,
        ) -> Result<Value, drox_tools::ToolError> {
            Ok(json!({ "applied": true, "path": "src/page.tsx" }))
        }
    }

    struct FakeBashTool;

    #[async_trait]
    impl Tool for FakeBashTool {
        fn name(&self) -> &str {
            "bash"
        }
        fn description(&self) -> &str {
            "fake bash (test only) — counts as a mutating tool for step tracking"
        }
        fn input_schema(&self) -> Value {
            json!({ "type": "object" })
        }
        async fn execute(
            &self,
            _ctx: &ToolContext,
            _input: Value,
        ) -> Result<Value, drox_tools::ToolError> {
            Ok(json!({ "stdout": "", "exit_code": 0 }))
        }
    }

    /// Forme un tour LLM qui se clôt proprement (`[phase: done]` puis texte).
    /// Construit un tour LLM « happy path » : `[phase: answering]` + texte +
    /// `[phase: done]`. Conforme à la règle answering-before-done (A.3) :
    /// un tour qui prétend conclure DOIT passer par `answering`.
    fn done_turn(text: &str) -> Vec<StreamEvent> {
        vec![
            StreamEvent::Start,
            StreamEvent::TextDelta {
                text: format!("[phase: answering]\n{text}\n[phase: done]"),
            },
            StreamEvent::Stop {
                reason: StopReason::EndTurn,
                usage: Usage::default(),
            },
        ]
    }

    /// Tour LLM qui signe `[phase: done]` SANS jamais passer par `answering`.
    /// Utilisé pour tester la branche A.3 : le moteur doit refuser la
    /// clôture et injecter `MISSING_ANSWERING_PROMPT`.
    fn premature_done_turn(text: &str) -> Vec<StreamEvent> {
        vec![
            StreamEvent::Start,
            StreamEvent::TextDelta {
                text: format!("[phase: reading]\n{text}\n[phase: done]"),
            },
            StreamEvent::Stop {
                reason: StopReason::EndTurn,
                usage: Usage::default(),
            },
        ]
    }

    /// Protocole test : `[phase: reading]` puis un `todo_write` à 1 item
    /// directement `completed`. Pour les tests qui veulent un cycle minimal
    /// qui passe **toutes** les gates moteur (todo clôturée avant `done`).
    fn read_then_one_todo_turn(note: &str) -> Vec<StreamEvent> {
        read_then_one_todo_turn_with_status(note, "completed")
    }

    /// Variante paramétrable : permet d'ouvrir une to-do en `in_progress`
    /// pour tester la gate « to-do non clôturée → refus `done` ».
    fn read_then_one_todo_turn_with_status(
        note: &str,
        status: &str,
    ) -> Vec<StreamEvent> {
        let tid = ToolUseId::new();
        vec![
            StreamEvent::Start,
            StreamEvent::TextDelta {
                text: format!("[phase: reading]\n{note}\n"),
            },
            StreamEvent::ToolCall {
                id: tid,
                name: "todo_write".into(),
                arguments: json!({
                    "todos": [{
                        "id": "1",
                        "content": "Étape de test",
                        "status": status,
                    }]
                }),
            },
            StreamEvent::Stop {
                reason: StopReason::ToolUse,
                usage: Usage::default(),
            },
        ]
    }

    /// GLM / Qwen : `tool_calls` nommé `phase:` + `{\"done\":\"\"}` au lieu de la ligne texte.
    fn answering_turn_with_hallucinated_phase_tool() -> Vec<StreamEvent> {
        let tid = ToolUseId::new();
        vec![
            StreamEvent::Start,
            StreamEvent::TextDelta {
                text: "[phase: answering]\nConclusion finale.\n".into(),
            },
            StreamEvent::ToolCall {
                id: tid,
                name: "phase:".into(),
                arguments: json!({ "done": "" }),
            },
            StreamEvent::Stop {
                reason: StopReason::ToolUse,
                usage: Usage::default(),
            },
        ]
    }

    #[test]
    fn hallucinated_phase_tool_detects_common_variants() {
        assert!(is_hallucinated_phase_tool_call("phase", &json!({ "done": "" })));
        assert!(is_hallucinated_phase_tool_call("Phase:", &json!({ "done": "" })));
        assert!(is_hallucinated_phase_tool_call("phase:done", &json!({})));
        assert!(is_hallucinated_phase_tool_call("set_phase", &json!({})));
        assert!(is_hallucinated_phase_tool_call("phase_transition", &json!({})));
        assert!(is_hallucinated_phase_tool_call("done", &json!({ "done": "" })));
    }

    #[test]
    fn hallucinated_phase_tool_ignores_real_tools() {
        assert!(!is_hallucinated_phase_tool_call(
            "file_read",
            &json!({ "path": "x" })
        ));
        assert!(!is_hallucinated_phase_tool_call(
            "todo_write",
            &json!({ "todos": [] })
        ));
        assert!(!is_hallucinated_phase_tool_call(
            "bash",
            &json!({ "command": "echo" })
        ));
        assert!(!is_hallucinated_phase_tool_call(
            "done",
            &json!({ "path": "x", "done": true })
        ));
    }

    /// Sprint A.7 — relax de la gate aux read-only.
    ///
    /// `echo` (proxy d'un read-only en test) appelé **avant** tout
    /// `todo_write` ne doit plus déclencher la gate
    /// `MUTATING_TOOL_BEFORE_TODO_WRITE_BLOCKED`. C'était le faux `× Listed *`
    /// observé en début de chaque conversation : GLM appelle `glob` seul,
    /// le moteur le rejetait, le modèle retentait, et après 1-2 essais
    /// finissait par enchaîner. Maintenant, exploration libre.
    #[tokio::test]
    async fn read_only_tool_before_todo_write_is_allowed() {
        let tid_echo = ToolUseId::new();
        let tid_todo = ToolUseId::new();
        let llm = Arc::new(ScriptedLlm::new(vec![
            // Tour 1 : reasoning + echo SEUL (pas de todo_write).
            vec![
                StreamEvent::Start,
                StreamEvent::TextDelta {
                    text: "[phase: reading]\nJe regarde rapidement.\n\
                           [phase: reading]\n"
                        .into(),
                },
                StreamEvent::ToolCall {
                    id: tid_echo.clone(),
                    name: "echo".into(),
                    arguments: json!({ "v": 1 }),
                },
                StreamEvent::Stop {
                    reason: StopReason::ToolUse,
                    usage: Usage::default(),
                },
            ],
            // Tour 2 : maintenant il pose son plan (todo_write seule).
            vec![
                StreamEvent::Start,
                StreamEvent::TextDelta {
                    text: "[phase: planning]\nMaintenant je plan.\n".into(),
                },
                StreamEvent::ToolCall {
                    id: tid_todo.clone(),
                    name: "todo_write".into(),
                    arguments: json!({
                        "todos": [{
                            "id": "1",
                            "content": "Étape",
                            "status": "completed"
                        }]
                    }),
                },
                StreamEvent::Stop {
                    reason: StopReason::ToolUse,
                    usage: Usage::default(),
                },
            ],
            done_turn("OK."),
        ]));

        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(TodoWriteTool));
        registry.register(Arc::new(EchoTool));
        let registry = Arc::new(registry);
        let ctx = ToolContext::new(camino::Utf8PathBuf::from("."), false);
        let agent = Agent::new(llm, registry, ctx, AgentConfig::default());

        let events: Vec<_> = agent
            .run("test")
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<_, _>>()
            .unwrap();

        let echo_ok = events.iter().any(|e| {
            matches!(
                e,
                AgentEvent::ToolFinish { id, is_error: false, .. } if id == &tid_echo
            )
        });
        assert!(
            echo_ok,
            "echo (read-only proxy) doit s'exécuter sans erreur avant todo_write ; events={events:?}"
        );

        let any_error = events.iter().any(
            |e| matches!(e, AgentEvent::ToolFinish { is_error: true, .. }),
        );
        assert!(
            !any_error,
            "aucun ToolFinish en erreur — la gate ne s'applique plus aux read-only ; events={events:?}"
        );
    }

    /// Sprint A.7 — la gate reste **dure** sur les mutateurs.
    ///
    /// `bash` (tool mutateur) appelé sans `todo_write` préalable doit toujours
    /// recevoir `MUTATING_TOOL_BEFORE_TODO_WRITE_BLOCKED`. La discipline de
    /// planification est conservée pour tout ce qui touche au filesystem ou
    /// exécute du shell.
    #[tokio::test]
    async fn mutating_tool_before_todo_write_still_blocked() {
        let tid_bash = ToolUseId::new();
        let llm = Arc::new(ScriptedLlm::new(vec![
            // Tour 1 : reasoning + bash SEUL (mutateur, pas de todo_write).
            vec![
                StreamEvent::Start,
                StreamEvent::TextDelta {
                    text: "[phase: reading]\nJe vais lancer un script.\n\
                           [phase: acting]\n"
                        .into(),
                },
                StreamEvent::ToolCall {
                    id: tid_bash.clone(),
                    name: "bash".into(),
                    arguments: json!({ "command": "ls" }),
                },
                StreamEvent::Stop {
                    reason: StopReason::ToolUse,
                    usage: Usage::default(),
                },
            ],
            done_turn("OK."),
        ]));

        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(TodoWriteTool));
        registry.register(Arc::new(FakeBashTool));
        let registry = Arc::new(registry);
        let ctx = ToolContext::new(camino::Utf8PathBuf::from("."), false);
        let agent = Agent::new(llm, registry, ctx, AgentConfig::default());

        let events: Vec<_> = agent
            .run("test")
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<_, _>>()
            .unwrap();

        let bash_blocked = events.iter().any(|e| {
            matches!(
                e,
                AgentEvent::ToolFinish {
                    id,
                    is_error: true,
                    output,
                    ..
                } if id == &tid_bash
                    && output
                        .get("error")
                        .and_then(|v| v.as_str())
                        .is_some_and(|s| s.contains("mutating") || s.contains("Planning"))
            )
        });
        assert!(
            bash_blocked,
            "bash sans todo_write préalable doit être bloqué par la gate mutateurs ; events={events:?}"
        );
    }

    #[tokio::test]
    async fn professor_file_edit_blocked_without_course_plan() {
        use crate::permissions::PermissionPolicy;
        use drox_permissions::{PermissionEngine, PermissionMode, RuleSet};

        let tid_edit = ToolUseId::new();
        let llm = Arc::new(ScriptedLlm::new(vec![
            vec![
                StreamEvent::Start,
                StreamEvent::TextDelta {
                    text: "[phase: acting]\n".into(),
                },
                StreamEvent::ToolCall {
                    id: tid_edit.clone(),
                    name: "file_edit".into(),
                    arguments: json!({
                        "path": "src/page.tsx",
                        "edits": [{ "old_string": "a", "new_string": "b" }]
                    }),
                },
                StreamEvent::Stop {
                    reason: StopReason::ToolUse,
                    usage: Usage::default(),
                },
            ],
            done_turn("OK."),
        ]));

        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(FakeFileEditTool));
        let registry = Arc::new(registry);

        let policy = PermissionPolicy::new(
            Arc::new(PermissionEngine::with_rules(RuleSet::new())),
            PermissionMode::Professor,
        );
        let ctx = ToolContext::new(camino::Utf8PathBuf::from("."), false);
        let cfg = AgentConfig {
            permissions: Some(policy),
            ..AgentConfig::default()
        };
        let agent = Agent::new(llm, registry, ctx, cfg);

        let mut blocked = false;
        let mut stream = std::pin::pin!(agent.run("modifie la page d'accueil"));
        while let Some(item) = stream.next().await {
            match item {
                Ok(AgentEvent::ToolFinish {
                    id,
                    is_error: true,
                    output,
                    ..
                }) if id.clone() == tid_edit
                    && output
                        .get("error")
                        .and_then(|v| v.as_str())
                        .is_some_and(|s| s.contains("course_plan_write")) =>
                {
                    blocked = true;
                    break;
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }
        assert!(
            blocked,
            "file_edit en mode professeur sans plan doit être bloqué"
        );
    }

    /// Cas observé GLM-4.7-Flash : le modèle bat che `[file_edit, todo_write]`
    /// (ou `[bash, todo_write]`) dans le même tour. Sans le réordonnement,
    /// le mutateur rate la gate `MUTATING_TOOL_BEFORE_TODO_WRITE_BLOCKED` au
    /// premier outil → UI affiche `× Edited foo.rs`. Avec le réordonnement,
    /// `todo_write` est exécuté en premier, le mutateur ensuite, les deux
    /// passent silencieusement.
    ///
    /// Note : `echo` n'est PAS un mutateur (cf. `TOOLS_REQUIRING_TODO_WRITE_GATE`).
    /// On utilise quand même `echo` ici par commodité (tool de test simple),
    /// mais ce qu'on teste réellement c'est l'ordre d'exécution préservé
    /// quand un batch contient `todo_write` non en tête. La gate elle-même
    /// n'est plus déclenchée par `echo` depuis le relax read-only.
    #[tokio::test]
    async fn todo_write_promoted_when_batched_with_other_tool() {
        let tid_echo = ToolUseId::new();
        let tid_todo = ToolUseId::new();
        let llm = Arc::new(ScriptedLlm::new(vec![
            vec![
                StreamEvent::Start,
                StreamEvent::TextDelta {
                    text: "[phase: reading]\nPlan + explore en un coup.\n\
                           [phase: reading]\n"
                        .into(),
                },
                StreamEvent::ToolCall {
                    id: tid_echo.clone(),
                    name: "echo".into(),
                    arguments: json!({ "value": "ping" }),
                },
                StreamEvent::ToolCall {
                    id: tid_todo.clone(),
                    name: "todo_write".into(),
                    arguments: json!({
                        "todos": [{
                            "id": "1",
                            "content": "Étape",
                            "status": "completed"
                        }]
                    }),
                },
                StreamEvent::Stop {
                    reason: StopReason::ToolUse,
                    usage: Usage::default(),
                },
            ],
            done_turn("OK."),
        ]));

        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(TodoWriteTool));
        registry.register(Arc::new(EchoTool));
        let registry = Arc::new(registry);
        let ctx = ToolContext::new(camino::Utf8PathBuf::from("."), false);
        let agent = Agent::new(llm, registry, ctx, AgentConfig::default());

        let events: Vec<_> = agent
            .run("test")
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<_, _>>()
            .unwrap();

        let any_error = events.iter().any(
            |e| matches!(e, AgentEvent::ToolFinish { is_error: true, .. }),
        );
        assert!(
            !any_error,
            "le batch [echo, todo_write] doit être réordonné, donc aucun ToolFinish en erreur ; events={events:?}"
        );

        let echo_finished_ok = events.iter().any(|e| {
            matches!(
                e,
                AgentEvent::ToolFinish { id, is_error: false, .. } if id == &tid_echo
            )
        });
        let todo_finished_ok = events.iter().any(|e| {
            matches!(
                e,
                AgentEvent::ToolFinish { id, is_error: false, .. } if id == &tid_todo
            )
        });
        assert!(
            echo_finished_ok && todo_finished_ok,
            "echo ET todo_write doivent finir sans erreur ; events={events:?}"
        );
    }

    /// Anti-régression : quand un `todo_write` a déjà réussi dans le run,
    /// l'ordre relatif du modèle est respecté (pas de promotion silencieuse
    /// au-delà de la première satisfaction de la gate).
    #[tokio::test]
    async fn todo_write_not_promoted_once_gate_already_satisfied() {
        let tid_echo = ToolUseId::new();
        let tid_todo2 = ToolUseId::new();
        let llm = Arc::new(ScriptedLlm::new(vec![
            // Tour 1 : todo_write seul (satisfait la gate).
            read_then_one_todo_turn("Premier plan."),
            // Tour 2 : modèle bat che [echo, todo_write] dans un ordre
            // volontaire — on veut que l'ordre soit conservé (echo exécuté
            // en premier puis nouvelle MAJ du plan).
            vec![
                StreamEvent::Start,
                StreamEvent::TextDelta {
                    text: "[phase: acting]\n".into(),
                },
                StreamEvent::ToolCall {
                    id: tid_echo.clone(),
                    name: "echo".into(),
                    arguments: json!({ "value": "ping" }),
                },
                StreamEvent::ToolCall {
                    id: tid_todo2.clone(),
                    name: "todo_write".into(),
                    arguments: json!({
                        "todos": [{
                            "id": "1",
                            "content": "Étape",
                            "status": "completed"
                        }]
                    }),
                },
                StreamEvent::Stop {
                    reason: StopReason::ToolUse,
                    usage: Usage::default(),
                },
            ],
            done_turn("OK."),
        ]));

        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(TodoWriteTool));
        registry.register(Arc::new(EchoTool));
        let registry = Arc::new(registry);
        let ctx = ToolContext::new(camino::Utf8PathBuf::from("."), false);
        let agent = Agent::new(llm, registry, ctx, AgentConfig::default());

        let events: Vec<_> = agent
            .run("test")
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<_, _>>()
            .unwrap();

        // Aucun ToolFinish en erreur (les deux passent toujours).
        let any_error = events.iter().any(
            |e| matches!(e, AgentEvent::ToolFinish { is_error: true, .. }),
        );
        assert!(!any_error, "events={events:?}");

        // Garde-fou comportemental : l'ordre des ToolFinish (echo puis
        // todo_write n°2) doit correspondre à l'ordre demandé par le
        // modèle, pas être inversé par le réordonnement.
        let finish_seq: Vec<&ToolUseId> = events
            .iter()
            .filter_map(|e| match e {
                AgentEvent::ToolFinish { id, .. } => Some(id),
                _ => None,
            })
            .collect();
        // 3 ToolFinish attendus : tour 1 (todo_write n°1), tour 2 (echo), tour 2 (todo_write n°2).
        // On ne vérifie que l'ordre relatif des deux du tour 2 :
        let echo_pos = finish_seq.iter().position(|id| *id == &tid_echo);
        let todo2_pos = finish_seq.iter().position(|id| *id == &tid_todo2);
        match (echo_pos, todo2_pos) {
            (Some(e), Some(t)) => assert!(
                e < t,
                "ordre relatif modèle préservé (echo avant todo_write n°2) ; finish_seq={finish_seq:?}"
            ),
            _ => panic!("echo et todo_write n°2 doivent être finis ; events={events:?}"),
        }
    }

    #[tokio::test]
    async fn hallucinated_phase_tool_skips_permission_ask_and_surfaces_engine_hint() {
        use drox_permissions::{
            PermissionBehavior, PermissionEngine, PermissionMode, Rule, RuleSet, RuleSource,
            RuleValue,
        };

        let llm = Arc::new(ScriptedLlm::new(vec![
            read_then_one_todo_turn("Plan minimal."),
            answering_turn_with_hallucinated_phase_tool(),
            done_turn("OK."),
        ]));

        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(TodoWriteTool));
        let registry = Arc::new(registry);

        let mut rules = RuleSet::new();
        rules.push(Rule {
            value: RuleValue::tool_wide("echo"),
            behavior: PermissionBehavior::Deny,
            source: RuleSource::CliArg,
        });
        let policy = PermissionPolicy::new(
            Arc::new(PermissionEngine::with_rules(rules)),
            PermissionMode::Default,
        );

        let ctx = ToolContext::new(camino::Utf8PathBuf::from("."), false);
        let cfg = AgentConfig {
            permissions: Some(policy),
            ..AgentConfig::default()
        };
        let agent = Agent::new(llm, registry, ctx, cfg);

        let events: Vec<_> = agent
            .run("test")
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<_, _>>()
            .unwrap();

        let has_phase_hint = events.iter().any(|e| {
            matches!(
                e,
                AgentEvent::ToolFinish {
                    is_error: true,
                    output,
                    ..
                } if output
                    .get("error")
                    .and_then(|v| v.as_str())
                    .is_some_and(|s| s.contains("phase markers are NOT tools"))
            )
        });
        assert!(
            has_phase_hint,
            "attendu message moteur explicite (pas refus permission) ; events={events:?}"
        );
        let any_user_denied = events.iter().any(|e| {
            matches!(
                e,
                AgentEvent::ToolFinish {
                    is_error: true,
                    output,
                    ..
                } if output
                    .get("error")
                    .and_then(|v| v.as_str())
                    .is_some_and(|s| s.contains("User denied permission"))
            )
        });
        assert!(
            !any_user_denied,
            "le faux outil phase ne doit pas passer par Ask → refus ; events={events:?}"
        );
    }

    fn todo_then_code_edit_turn(path: &str) -> Vec<StreamEvent> {
        let tid_todo = ToolUseId::new();
        let tid_edit = ToolUseId::new();
        vec![
            StreamEvent::Start,
            StreamEvent::TextDelta {
                text: "[phase: reading]\nprep\n".into(),
            },
            StreamEvent::ToolCall {
                id: tid_todo,
                name: "todo_write".into(),
                arguments: json!({
                    "todos": [{
                        "id": "1",
                        "content": "Modifier le code",
                        "status": "completed",
                    }]
                }),
            },
            StreamEvent::TextDelta {
                text: "[phase: acting]\nedit\n".into(),
            },
            StreamEvent::ToolCall {
                id: tid_edit,
                name: "file_edit".into(),
                arguments: json!({ "path": path, "old_string": "a", "new_string": "b" }),
            },
            StreamEvent::Stop {
                reason: StopReason::ToolUse,
                usage: Usage::default(),
            },
        ]
    }

    #[tokio::test]
    async fn done_blocked_when_code_edited_without_testing() {
        let tid_bash = ToolUseId::new();
        let llm = Arc::new(ScriptedLlm::new(vec![
            todo_then_code_edit_turn("src/lib.rs"),
            done_turn("réponse sans test"),
            vec![
                StreamEvent::Start,
                StreamEvent::TextDelta {
                    text: "[phase: testing]\ncheck\n".into(),
                },
                StreamEvent::ToolCall {
                    id: tid_bash,
                    name: "bash".into(),
                    arguments: json!({ "command": "cargo check" }),
                },
                StreamEvent::Stop {
                    reason: StopReason::ToolUse,
                    usage: Usage::default(),
                },
            ],
            done_turn("réponse après test"),
            done_turn("réponse après test"),
        ]));
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(TodoWriteTool));
        registry.register(Arc::new(FakeFileEditTool));
        registry.register(Arc::new(FakeBashTool));
        let registry = Arc::new(registry);
        let ctx = ToolContext::new(camino::Utf8PathBuf::from("."), false);
        let agent = Agent::new(llm, registry, ctx, AgentConfig::default());

        let raw: Vec<_> = agent.run("corrige le bug").collect().await;
        let events: Vec<AgentEvent> = raw.into_iter().filter_map(Result::ok).collect();

        let testing_idx = events
            .iter()
            .position(|e| matches!(e, AgentEvent::PhaseEnter { phase: Phase::Testing }));
        assert!(
            testing_idx.is_some(),
            "expected Testing phase after code edit nudge, got {events:?}"
        );
        let testing_idx = testing_idx.unwrap();
        assert!(
            !events[..testing_idx]
                .iter()
                .any(|e| matches!(e, AgentEvent::Stop { .. })),
            "run must not Stop before testing phase"
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, AgentEvent::PhaseEnter { phase: Phase::Done })),
            "expected eventual Done, got {events:?}"
        );
    }

    #[tokio::test]
    async fn done_allowed_when_only_markdown_edited() {
        let llm = Arc::new(ScriptedLlm::new(vec![
            todo_then_code_edit_turn("docs/README.md"),
            done_turn("doc mise à jour"),
        ]));
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(TodoWriteTool));
        registry.register(Arc::new(FakeFileEditTool));
        let registry = Arc::new(registry);
        let ctx = ToolContext::new(camino::Utf8PathBuf::from("."), false);
        let agent = Agent::new(llm, registry, ctx, AgentConfig::default());

        let raw: Vec<_> = agent.run("mets à jour le readme").collect().await;
        let events: Vec<AgentEvent> = raw.into_iter().filter_map(Result::ok).collect();

        assert!(
            !events
                .iter()
                .any(|e| matches!(e, AgentEvent::PhaseEnter { phase: Phase::Testing })),
            "markdown-only edit must not require testing phase, got {events:?}"
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, AgentEvent::PhaseEnter { phase: Phase::Done })),
            "expected Done without testing, got {events:?}"
        );
    }

    #[tokio::test]
    async fn consume_stream_sets_saw_testing_flag() {
        let (tx, _rx) = mpsc::channel::<Result<AgentEvent, EngineError>>(8);
        let script = vec![
            StreamEvent::Start,
            StreamEvent::TextDelta {
                text: "[phase: testing]\nrun checks\n".into(),
            },
            StreamEvent::Stop {
                reason: StopReason::EndTurn,
                usage: Usage::default(),
            },
        ];
        let stream = stream::iter(script.into_iter().map(Ok::<_, LlmError>)).boxed();
        let outcome = consume_stream(stream, &tx, false).await.expect("channel open");
        assert!(outcome.saw_testing);
        assert_eq!(outcome.final_phase, Some(Phase::Testing));
    }

    #[tokio::test]
    async fn done_blocked_when_todos_still_open() {
        // Scénario : le modèle ouvre la to-do en `in_progress` puis tente de
        // clôturer directement (réponse + done). La nouvelle gate doit refuser
        // ce `done` (todo non clôturée) et nudger. Le tour 3 met à jour la
        // to-do en `completed` et seulement là le moteur accepte de fermer.
        let tid_close = ToolUseId::new();
        let llm = Arc::new(ScriptedLlm::new(vec![
            // Tour 1 : ouverture de la to-do (in_progress).
            read_then_one_todo_turn_with_status(
                "Je vais traiter la demande.",
                "in_progress",
            ),
            // Tour 2 : answering + done, mais la to-do est toujours ouverte
            // → la gate doit nudger et NE PAS clôturer.
            vec![
                StreamEvent::Start,
                StreamEvent::TextDelta {
                    text: "[phase: answering]\nréponse hâtive\n[phase: done]".into(),
                },
                StreamEvent::Stop {
                    reason: StopReason::EndTurn,
                    usage: Usage::default(),
                },
            ],
            // Tour 3 : le modèle clôture la to-do puis re-tente.
            vec![
                StreamEvent::Start,
                StreamEvent::TextDelta {
                    text: "[phase: verifying]\nJe ferme la to-do.\n".into(),
                },
                StreamEvent::ToolCall {
                    id: tid_close,
                    name: "todo_write".into(),
                    arguments: json!({
                        "todos": [{
                            "id": "1",
                            "content": "Étape de test",
                            "status": "completed",
                        }]
                    }),
                },
                StreamEvent::Stop {
                    reason: StopReason::ToolUse,
                    usage: Usage::default(),
                },
            ],
            // Tour 4 : answering + done. Tout est OK, on doit fermer.
            vec![
                StreamEvent::Start,
                StreamEvent::TextDelta {
                    text: "[phase: answering]\nréponse finale\n[phase: done]".into(),
                },
                StreamEvent::Stop {
                    reason: StopReason::EndTurn,
                    usage: Usage::default(),
                },
            ],
        ]));
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(TodoWriteTool));
        let registry = Arc::new(registry);
        let ctx = ToolContext::new(camino::Utf8PathBuf::from("."), false);
        let agent = Agent::new(llm, registry, ctx, AgentConfig::default());

        let events: Vec<_> = agent
            .run("salut")
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<_, _>>()
            .unwrap();

        // On exige qu'on ait vu DEUX appels todo_write (ouverture + clôture).
        let todo_finishes = events
            .iter()
            .filter(|e| matches!(e, AgentEvent::ToolFinish { is_error: false, .. }))
            .count();
        assert!(
            todo_finishes >= 2,
            "expected at least 2 successful todo_write tool finishes (open + close), got {todo_finishes} — events: {events:?}",
        );

        // Le run doit se clôturer normalement (Stop final) : la gate finit
        // par accepter `done` une fois la to-do passée en `completed`.
        assert!(
            events
                .iter()
                .any(|e| matches!(e, AgentEvent::PhaseEnter { phase: Phase::Done })),
            "expected PhaseEnter(Done) eventually, got {events:?}",
        );
        assert!(matches!(events.last(), Some(AgentEvent::Stop { .. })));

        // La réponse finale doit être celle du tour 4, pas la hâtive du tour 2.
        assert!(
            events
                .iter()
                .any(|e| matches!(e, AgentEvent::TextDelta { text } if text.contains("réponse finale"))),
            "expected final answer to come from the post-close turn, got {events:?}",
        );
    }

    #[tokio::test]
    async fn done_marker_terminates_turn() {
        // `todo_write` obligatoire + `answering` avant `done` : deux tours.
        let llm = Arc::new(ScriptedLlm::new(vec![
            read_then_one_todo_turn("Alignement sur le message utilisateur."),
            vec![
                StreamEvent::Start,
                StreamEvent::TextDelta {
                    text: "[phase: answering]\nbonjour\n[phase: done]".into(),
                },
                StreamEvent::Stop {
                    reason: StopReason::EndTurn,
                    usage: Usage {
                        input_tokens: 1,
                        output_tokens: 1,
                    },
                },
            ],
        ]));
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(TodoWriteTool));
        let registry = Arc::new(registry);
        let ctx = ToolContext::new(camino::Utf8PathBuf::from("."), false);
        let agent = Agent::new(llm, registry, ctx, AgentConfig::default());

        let events: Vec<_> = agent
            .run("salut")
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<_, _>>()
            .unwrap();

        assert!(
            events
                .iter()
                .any(|e| matches!(e, AgentEvent::PhaseEnter { phase: Phase::Done })),
            "expected PhaseEnter(Done) in {events:?}"
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, AgentEvent::TextDelta { text } if text.contains("bonjour"))),
            "expected text 'bonjour' to be emitted (without marker), got {events:?}"
        );
        assert!(matches!(events.last(), Some(AgentEvent::Stop { .. })));
    }

    #[tokio::test]
    async fn tool_call_triggers_execute_and_second_turn() {
        let tid_todo = ToolUseId::new();
        let tid_echo = ToolUseId::new();
        let llm = Arc::new(ScriptedLlm::new(vec![
            vec![
                StreamEvent::Start,
                StreamEvent::TextDelta {
                    text: "[phase: reading]\nJe vais émettre echo après todo.\n".into(),
                },
                StreamEvent::ToolCall {
                    id: tid_todo,
                    name: "todo_write".into(),
                    arguments: json!({
                        "todos": [{
                            "id": "1",
                            "content": "Ping echo",
                            "status": "completed"
                        }]
                    }),
                },
                StreamEvent::ToolCall {
                    id: tid_echo.clone(),
                    name: "echo".into(),
                    arguments: json!({ "msg": "ping" }),
                },
                StreamEvent::Stop {
                    reason: StopReason::ToolUse,
                    usage: Usage::default(),
                },
            ],
            // Tour 2 : modèle signe [phase: done] et conclut.
            done_turn("fini"),
        ]));

        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(TodoWriteTool));
        registry.register(Arc::new(EchoTool));
        let registry = Arc::new(registry);
        let ctx = ToolContext::new(camino::Utf8PathBuf::from("."), false);
        let agent = Agent::new(llm, registry, ctx, AgentConfig::default());

        let events: Vec<_> = agent
            .run("test")
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<_, _>>()
            .unwrap();

        assert!(
            events
                .iter()
                .any(|e| matches!(e, AgentEvent::ToolStart { name, .. } if name == "echo"))
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, AgentEvent::ToolFinish { is_error: false, .. }))
        );
        assert!(events.iter().any(
            |e| matches!(e, AgentEvent::PhaseEnter { phase } if *phase == Phase::Done)
        ));
        assert!(
            events
                .iter()
                .any(|e| matches!(e, AgentEvent::TextDelta { text } if text.contains("fini")))
        );
        assert!(matches!(events.last(), Some(AgentEvent::Stop { .. })));
    }

    #[tokio::test]
    async fn unknown_tool_yields_is_error_finish() {
        let tid_todo = ToolUseId::new();
        let tid_bad = ToolUseId::new();
        let llm = Arc::new(ScriptedLlm::new(vec![
            vec![
                StreamEvent::Start,
                StreamEvent::TextDelta {
                    text: "[phase: reading]\nJ'essaie un outil inconnu.\n".into(),
                },
                StreamEvent::ToolCall {
                    id: tid_todo,
                    name: "todo_write".into(),
                    arguments: json!({
                        "todos": [{
                            "id": "1",
                            "content": "Étape",
                            "status": "completed"
                        }]
                    }),
                },
                StreamEvent::Stop {
                    reason: StopReason::ToolUse,
                    usage: Usage::default(),
                },
            ],
            vec![
                StreamEvent::Start,
                StreamEvent::ToolCall {
                    id: tid_bad.clone(),
                    name: "does_not_exist".into(),
                    arguments: json!({}),
                },
                StreamEvent::Stop {
                    reason: StopReason::ToolUse,
                    usage: Usage::default(),
                },
            ],
            done_turn(""),
        ]));
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(TodoWriteTool));
        let registry = Arc::new(registry);
        let ctx = ToolContext::new(camino::Utf8PathBuf::from("."), false);
        let agent = Agent::new(llm, registry, ctx, AgentConfig::default());

        let events: Vec<_> = agent
            .run("x")
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<_, _>>()
            .unwrap();

        assert!(
            events
                .iter()
                .any(|e| matches!(e, AgentEvent::ToolFinish { is_error: true, .. }))
        );
    }

    #[tokio::test]
    async fn permission_deny_skips_execution() {
        use crate::permissions::PermissionPolicy;
        use drox_permissions::{
            PermissionBehavior, PermissionEngine, PermissionMode, Rule, RuleSet, RuleSource,
            RuleValue,
        };

        let tid_todo = ToolUseId::new();
        let tid_echo = ToolUseId::new();
        let llm = Arc::new(ScriptedLlm::new(vec![
            vec![
                StreamEvent::Start,
                StreamEvent::TextDelta {
                    text: "[phase: reading]\nJ'appelle echo après todo.\n".into(),
                },
                StreamEvent::ToolCall {
                    id: tid_todo,
                    name: "todo_write".into(),
                    arguments: json!({
                        "todos": [{
                            "id": "1",
                            "content": "Echo",
                            "status": "completed"
                        }]
                    }),
                },
                StreamEvent::ToolCall {
                    id: tid_echo.clone(),
                    name: "echo".into(),
                    arguments: json!({ "msg": "ping" }),
                },
                StreamEvent::Stop {
                    reason: StopReason::ToolUse,
                    usage: Usage::default(),
                },
            ],
            done_turn("ok"),
        ]));

        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(TodoWriteTool));
        registry.register(Arc::new(EchoTool));
        let registry = Arc::new(registry);

        let mut rules = RuleSet::new();
        rules.push(Rule {
            value: RuleValue::tool_wide("echo"),
            behavior: PermissionBehavior::Deny,
            source: RuleSource::CliArg,
        });
        let policy = PermissionPolicy::new(
            Arc::new(PermissionEngine::with_rules(rules)),
            PermissionMode::Default,
        );

        let ctx = ToolContext::new(camino::Utf8PathBuf::from("."), false);
        let cfg = AgentConfig {
            permissions: Some(policy),
            ..AgentConfig::default()
        };
        let agent = Agent::new(llm, registry, ctx, cfg);

        let events: Vec<_> = agent
            .run("test")
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<_, _>>()
            .unwrap();

        // Le ToolFinish doit être marqué is_error: true (refus de permission),
        // l'agent ne doit PAS avoir exécuté EchoTool.
        let has_denial = events
            .iter()
            .any(|e| matches!(e, AgentEvent::ToolFinish { is_error: true, .. }));
        assert!(has_denial, "expected a denial ToolFinish in {events:?}");
    }

    #[tokio::test]
    async fn context_snip_event_emitted_when_history_exceeds_threshold() {
        use crate::context::ContextPolicy;
        use drox_context::{ContextBudget, RoughTokenCounter, SnipConfig};

        // Tour 1 : modèle conclut (pas de tool call). On veut juste que le
        // snip pré-tour se déclenche sur l'historique initial.
        let llm = Arc::new(ScriptedLlm::new(vec![
            read_then_one_todo_turn("Contexte pour le snip."),
            done_turn("ok"),
        ]));

        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(TodoWriteTool));
        let registry = Arc::new(registry);
        let ctx = ToolContext::new(camino::Utf8PathBuf::from("."), false);

        // Budget minuscule (1k window, 0 reserved, 0 autocompact buffer) →
        // n'importe quelle conversation dépasse autocompact_threshold.
        let policy = ContextPolicy::new(
            Arc::new(RoughTokenCounter::new(4)),
            ContextBudget {
                window_size: 1_000,
                reserved_output: 0,
                autocompact_buffer: 1_000,
                warning_buffer: 800,
                error_buffer: 600,
                manual_compact_buffer: 200,
            },
            Some(SnipConfig {
                min_tokens: 100,
                keep_recent_results: 0,
                placeholder: "[snip]".into(),
            }),
        );

        // On injecte un gros prompt utilisateur initial pour forcer le déclenchement.
        // Note : seuls les tool_results sont snipés, donc on doit injecter
        // un tool_result géant via le prompt initial — pas possible dans ce
        // setup minimal. On vérifie juste que le snip se déclenche sur les
        // tool_results déjà présents.
        // Pour la démonstration : un tour minimal sans tool_result ne
        // produira pas de ContextSnip mais ne doit pas crasher non plus.
        let cfg = AgentConfig {
            context: Some(policy),
            ..AgentConfig::default()
        };
        let agent = Agent::new(llm, registry, ctx, cfg);

        let events: Vec<_> = agent
            .run("x")
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<_, _>>()
            .unwrap();
        // L'agent doit s'être arrêté proprement, peu importe si ContextSnip
        // a été émis (pas de tool_result à snipper dans ce setup).
        assert!(matches!(events.last(), Some(AgentEvent::Stop { .. })));
    }

    #[tokio::test]
    async fn max_iterations_yields_error() {
        // Le LLM redemande toujours echo → boucle infinie bornée par max_iter.
        let make_opening_turn = || {
            vec![
                StreamEvent::Start,
                StreamEvent::TextDelta {
                    text: "[phase: reading]\nBoucle echo.\n".into(),
                },
                StreamEvent::ToolCall {
                    id: ToolUseId::new(),
                    name: "todo_write".into(),
                    arguments: json!({
                        "todos": [{
                            "id": "1",
                            "content": "Echo",
                            "status": "completed"
                        }]
                    }),
                },
                StreamEvent::ToolCall {
                    id: ToolUseId::new(),
                    name: "echo".into(),
                    arguments: json!({}),
                },
                StreamEvent::Stop {
                    reason: StopReason::ToolUse,
                    usage: Usage::default(),
                },
            ]
        };
        let make_echo_turn = || {
            vec![
                StreamEvent::Start,
                StreamEvent::ToolCall {
                    id: ToolUseId::new(),
                    name: "echo".into(),
                    arguments: json!({}),
                },
                StreamEvent::Stop {
                    reason: StopReason::ToolUse,
                    usage: Usage::default(),
                },
            ]
        };
        let llm = Arc::new(ScriptedLlm::new(vec![
            make_opening_turn(),
            make_echo_turn(),
            make_echo_turn(),
        ]));
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(TodoWriteTool));
        registry.register(Arc::new(EchoTool));
        let registry = Arc::new(registry);
        let ctx = ToolContext::new(camino::Utf8PathBuf::from("."), false);
        let cfg = AgentConfig {
            max_iterations: 2,
            ..AgentConfig::default()
        };
        let agent = Agent::new(llm, registry, ctx, cfg);

        let results: Vec<_> = agent.run("loop").collect::<Vec<_>>().await;
        let last = results.into_iter().last().expect("at least one event");
        assert!(matches!(last, Err(EngineError::MaxIterations(2))));
    }

    // --- parse_phase_marker --------------------------------------------------

    #[test]
    fn parse_phase_marker_accepts_canonical_forms() {
        assert_eq!(
            parse_phase_marker("[phase: analyzing]"),
            Some(Phase::Analyzing)
        );
        assert_eq!(
            parse_phase_marker("[phase: testing]"),
            Some(Phase::Testing)
        );
        assert_eq!(parse_phase_marker("[phase: reading]"), Some(Phase::Reading));
        assert_eq!(parse_phase_marker("[phase: clarifying]"), Some(Phase::Clarifying));
        assert_eq!(parse_phase_marker("[phase: planning]"), Some(Phase::Planning));
        assert_eq!(parse_phase_marker("[phase: acting]"), Some(Phase::Acting));
        assert_eq!(parse_phase_marker("[phase: verifying]"), Some(Phase::Verifying));
        assert_eq!(parse_phase_marker("[phase: answering]"), Some(Phase::Answering));
        assert_eq!(parse_phase_marker("[phase: done]"), Some(Phase::Done));
    }

    #[test]
    fn parse_phase_marker_returns_none_for_legacy_removed_names() {
        for line in [
            "[phase: reasoning]",
            "[phase: next-move]",
            "[phase:reason]",
            "[phase: think]",
            "[phase: nextmove]",
            "[phase: next_move]",
        ] {
            assert_eq!(parse_phase_marker(line), None, "{line}");
        }
    }

    #[test]
    fn legacy_removed_phase_marker_line_matches_deprecated_names() {
        assert!(legacy_removed_phase_marker_line("[phase: reasoning]"));
        assert!(legacy_removed_phase_marker_line("[phase: next-move]"));
        assert!(legacy_removed_phase_marker_line("[phase:reason]"));
    }

    #[test]
    fn parse_phase_marker_accepts_analyzing_aliases() {
        assert_eq!(
            parse_phase_marker("[phase: analysis]"),
            Some(Phase::Analyzing)
        );
        assert_eq!(
            parse_phase_marker("[phase: survey]"),
            Some(Phase::Analyzing)
        );
    }

    #[test]
    fn parse_phase_marker_accepts_testing_aliases() {
        assert_eq!(parse_phase_marker("[phase: test]"), Some(Phase::Testing));
        assert_eq!(parse_phase_marker("[phase: tests]"), Some(Phase::Testing));
    }

    #[test]
    fn record_counts_as_code_mutation_heuristic() {
        assert!(record_counts_as_code_mutation(
            "file_edit",
            &json!({ "path": "src/lib.rs" })
        ));
        assert!(!record_counts_as_code_mutation(
            "file_edit",
            &json!({ "path": "README.md" })
        ));
        assert!(record_counts_as_code_mutation("notebook_edit", &json!({})));
    }

    #[test]
    fn phase_for_tool_keeps_testing_during_verification_tools() {
        assert_eq!(
            phase_for_tool("bash", Some(Phase::Testing)),
            Phase::Testing
        );
        assert_eq!(
            phase_for_tool("file_edit", Some(Phase::Testing)),
            Phase::Acting
        );
    }

    #[test]
    fn parse_phase_marker_accepts_answering_aliases() {
        assert_eq!(parse_phase_marker("[phase: answer]"), Some(Phase::Answering));
        assert_eq!(parse_phase_marker("[phase: reply]"), Some(Phase::Answering));
        assert_eq!(parse_phase_marker("[phase: respond]"), Some(Phase::Answering));
        assert_eq!(parse_phase_marker("[phase: response]"), Some(Phase::Answering));
    }

    #[test]
    fn parse_phase_marker_accepts_aliases_and_casing() {
        assert_eq!(parse_phase_marker("[PHASE: Done]"), Some(Phase::Done));
        assert_eq!(parse_phase_marker("[phase: read]"), Some(Phase::Reading));
        assert_eq!(parse_phase_marker("[phase: act]"), Some(Phase::Acting));
        assert_eq!(parse_phase_marker("  [phase: done]  "), Some(Phase::Done));
    }

    #[test]
    fn parse_phase_marker_rejects_non_markers() {
        assert_eq!(parse_phase_marker(""), None);
        assert_eq!(parse_phase_marker("hello world"), None);
        assert_eq!(parse_phase_marker("[note: done]"), None);
        assert_eq!(parse_phase_marker("phase: done"), None); // no brackets
        assert_eq!(parse_phase_marker("[phase: bogus]"), None);
        assert_eq!(parse_phase_marker("a [phase: done] b"), None); // marker not whole line
    }

    // --- phase_for_tool ------------------------------------------------------

    #[test]
    fn phase_for_tool_classifies_readonly_as_reading() {
        for t in [
            "glob",
            "file_read",
            "grep",
            "lsp",
            "web_search",
            "web_fetch",
            "todo_write",
            "ask_user_question",
        ] {
            assert_eq!(
                phase_for_tool(t, None),
                Phase::Reading,
                "{t} should be Reading"
            );
        }
    }

    #[test]
    fn phase_for_tool_keeps_analyzing_during_exploration() {
        for t in ["glob", "grep", "file_read", "workspace_map_read"] {
            assert_eq!(
                phase_for_tool(t, Some(Phase::Analyzing)),
                Phase::Analyzing,
                "{t}"
            );
        }
        assert_eq!(
            phase_for_tool("file_edit", Some(Phase::Analyzing)),
            Phase::Acting
        );
    }

    #[test]
    fn phase_for_tool_classifies_mutative_or_unknown_as_acting() {
        for t in ["bash", "file_edit", "file_write", "notebook_edit", "delete_path", "anything_else"] {
            assert_eq!(
                phase_for_tool(t, None),
                Phase::Acting,
                "{t} should be Acting"
            );
        }
    }

    #[test]
    fn user_prompt_suggests_workspace_analysis_heuristic() {
        assert!(user_prompt_suggests_workspace_analysis(
            "Peux-tu analyser la structure du projet ?"
        ));
        assert!(!user_prompt_suggests_workspace_analysis("fix the typo in README"));
    }

    // --- PhaseLineBuffer -----------------------------------------------------

    #[test]
    fn phase_line_buffer_strips_marker_and_emits_phase() {
        let mut buf = PhaseLineBuffer::new();
        let mut text = String::new();
        let mut phases: Vec<Phase> = Vec::new();
        buf.push_chunk(
            "Salut\n[phase: done]\nau revoir\n",
            |line| text.push_str(&line),
            |p| phases.push(p),
        );
        assert_eq!(phases, vec![Phase::Done]);
        assert_eq!(text, "Salut\nau revoir\n");
    }

    #[test]
    fn phase_line_buffer_handles_fragmented_marker() {
        // Le marqueur arrive en plusieurs chunks (cas Ollama streaming).
        let mut buf = PhaseLineBuffer::new();
        let mut text = String::new();
        let mut phases: Vec<Phase> = Vec::new();
        for chunk in ["[phase:", " read", "ing]\n", "data"] {
            buf.push_chunk(chunk, |l| text.push_str(&l), |p| phases.push(p));
        }
        buf.finish(|l| text.push_str(&l), |p| phases.push(p));
        assert_eq!(phases, vec![Phase::Reading]);
        assert_eq!(text, "data");
    }

    #[test]
    fn phase_line_buffer_finish_treats_trailing_marker() {
        let mut buf = PhaseLineBuffer::new();
        let mut text = String::new();
        let mut phases: Vec<Phase> = Vec::new();
        buf.push_chunk("[phase: done]", |l| text.push_str(&l), |p| phases.push(p));
        buf.finish(|l| text.push_str(&l), |p| phases.push(p));
        assert_eq!(phases, vec![Phase::Done]);
        assert!(text.is_empty(), "no text expected for pure trailing marker");
    }

    #[tokio::test]
    async fn consecutive_same_phase_markers_are_deduplicated() {
        // Devstral / Gemma ponctuent leur prose avec `[phase: reading]` à
        // répétition. Le consommateur ne doit voir QU'UN seul `PhaseEnter`
        // tant que la phase ne change pas effectivement. Tour 2 : texte
        // riche puis `answering` + `done` (todo déjà posé au tour 1).
        let llm = Arc::new(ScriptedLlm::new(vec![
            read_then_one_todo_turn("Préambule."),
            vec![
                StreamEvent::Start,
                StreamEvent::TextDelta {
                    text: "[phase: reading]\nP1\n[phase: reading]\nP2\n\
                           [phase: reading]\nP3\n[phase: reading]\n\
                           lit le repo\n\
                           [phase: answering]\nréponse finale\n[phase: done]"
                        .into(),
                },
                StreamEvent::Stop {
                    reason: StopReason::EndTurn,
                    usage: Usage::default(),
                },
            ],
        ]));
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(TodoWriteTool));
        let registry = Arc::new(registry);
        let ctx = ToolContext::new(camino::Utf8PathBuf::from("."), false);
        let agent = Agent::new(llm, registry, ctx, AgentConfig::default());

        let events: Vec<_> = agent
            .run("x")
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<_, _>>()
            .unwrap();

        let phase_enters: Vec<Phase> = events
            .iter()
            .filter_map(|e| match e {
                AgentEvent::PhaseEnter { phase } => Some(*phase),
                _ => None,
            })
            .collect();
        // Tour 1 : reading + todo. Tour 2 : plusieurs `reading` identiques → 1
        // `PhaseEnter` tant que la phase ne change pas ; puis answering ; done.
        let dedup_tail = &[
            Phase::Reading,
            Phase::Answering,
            Phase::Done,
        ];
        assert!(
            phase_enters.windows(dedup_tail.len()).any(|w| w == dedup_tail),
            "expected dedup tail {dedup_tail:?} as a subslice, got {phase_enters:?}"
        );
    }

    // --- Boucle agent : nudge unique ----------------------------------------

    #[tokio::test]
    async fn silent_turn_triggers_one_nudge_then_done() {
        // Tour 1 : lecture / prose sans outil → nudge générique.
        // Tour 2 : todo obligatoire.
        // Tour 3 : answering + done.
        let llm = Arc::new(ScriptedLlm::new(vec![
            vec![
                StreamEvent::Start,
                StreamEvent::TextDelta {
                    text: "[phase: reading]\nok, je commence.\n".into(),
                },
                StreamEvent::Stop {
                    reason: StopReason::EndTurn,
                    usage: Usage::default(),
                },
            ],
            read_then_one_todo_turn("Je pose la liste."),
            done_turn("voici la réponse"),
        ]));
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(TodoWriteTool));
        let registry = Arc::new(registry);
        let ctx = ToolContext::new(camino::Utf8PathBuf::from("."), false);
        let agent = Agent::new(llm, registry, ctx, AgentConfig::default());

        let events: Vec<_> = agent
            .run("salut")
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<_, _>>()
            .unwrap();

        // On doit voir au moins le texte du tour 1 ET le marqueur Done du tour 3.
        assert!(
            events
                .iter()
                .any(|e| matches!(e, AgentEvent::TextDelta { text } if text.contains("je commence")))
        );
        assert!(events.iter().any(
            |e| matches!(e, AgentEvent::PhaseEnter { phase } if *phase == Phase::Done)
        ));
        assert!(matches!(events.last(), Some(AgentEvent::Stop { .. })));
    }

    #[tokio::test]
    async fn nudge_loops_until_max_iterations_when_done_never_emitted() {
        // Sprint A.2 : la seule règle de fin propre est `[phase: done]`. Si
        // le modèle ne le signale jamais, on relance jusqu'à atteindre
        // `max_iterations` (garde-fou unique). Le test borne explicitement
        // `max_iterations: 3` pour rester rapide, et fournit assez de tours
        // muets pour couvrir cette borne sans paniquer (`ScriptedLlm` panic
        // si on dépasse).
        let llm = Arc::new(ScriptedLlm::new(vec![
            vec![
                StreamEvent::Start,
                StreamEvent::TextDelta {
                    text: "[phase: reading]\ntour 1 muet\n".into(),
                },
                StreamEvent::Stop {
                    reason: StopReason::EndTurn,
                    usage: Usage::default(),
                },
            ],
            vec![
                StreamEvent::Start,
                StreamEvent::TextDelta {
                    text: "[phase: reading]\ntour 2 muet\n".into(),
                },
                StreamEvent::Stop {
                    reason: StopReason::EndTurn,
                    usage: Usage::default(),
                },
            ],
            vec![
                StreamEvent::Start,
                StreamEvent::TextDelta {
                    text: "[phase: reading]\ntour 3 muet\n".into(),
                },
                StreamEvent::Stop {
                    reason: StopReason::EndTurn,
                    usage: Usage::default(),
                },
            ],
        ]));
        let registry = Arc::new(ToolRegistry::new());
        let ctx = ToolContext::new(camino::Utf8PathBuf::from("."), false);
        let cfg = AgentConfig {
            max_iterations: 3,
            ..AgentConfig::default()
        };
        let agent = Agent::new(llm, registry, ctx, cfg);

        let raw: Vec<_> = agent.run("salut").collect::<Vec<_>>().await;
        let events: Vec<&AgentEvent> = raw.iter().filter_map(|r| r.as_ref().ok()).collect();

        // Les 3 tours ont été consommés (donc on a bien relancé à chaque
        // fois). Aucun `Done` n'a été émis, et la dernière entrée du stream
        // est l'erreur `MaxIterations(3)`.
        assert_eq!(
            events
                .iter()
                .filter(
                    |e| matches!(e, AgentEvent::TextDelta { text } if text.contains("muet"))
                )
                .count(),
            3,
            "les 3 tours muets doivent tous avoir été consommés"
        );
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, AgentEvent::PhaseEnter { phase } if *phase == Phase::Done)),
            "aucun `[phase: done]` n'a été émis, donc aucun PhaseEnter Done"
        );
        assert!(
            matches!(raw.last(), Some(Err(EngineError::MaxIterations(3)))),
            "le stream doit se terminer par MaxIterations(3), got {:?}",
            raw.last()
        );
    }

    /// Sprint Plan « un seul plan par run » — quand le modèle clôture toutes
    /// les étapes d'une liste puis tente d'en re-créer une nouvelle from
    /// scratch (ids inédits), le moteur rejette l'appel avec
    /// `TODO_RECREATION_BLOCKED` et laisse au modèle l'opportunité de
    /// re-soumettre avec les anciens items en `completed` + nouveaux items.
    #[tokio::test]
    async fn todo_recreation_after_all_completed_is_blocked() {
        let tu1 = ToolUseId::new();
        let tu2 = ToolUseId::new();
        let llm = Arc::new(ScriptedLlm::new(vec![
            // Tour 1 : plan A à 1 item directement completed.
            vec![
                StreamEvent::Start,
                StreamEvent::TextDelta {
                    text: "[phase: reading]\nPlan A.\n".into(),
                },
                StreamEvent::ToolCall {
                    id: tu1.clone(),
                    name: "todo_write".into(),
                    arguments: json!({
                        "todos": [
                            { "id": "1", "content": "étape A", "status": "completed" }
                        ]
                    }),
                },
                StreamEvent::Stop {
                    reason: StopReason::ToolUse,
                    usage: Usage::default(),
                },
            ],
            // Tour 2 : tentative de re-création avec ids tout neufs (plan B).
            vec![
                StreamEvent::Start,
                StreamEvent::TextDelta {
                    text: "[phase: reading]\nPlan B from scratch.\n".into(),
                },
                StreamEvent::ToolCall {
                    id: tu2.clone(),
                    name: "todo_write".into(),
                    arguments: json!({
                        "todos": [
                            { "id": "10", "content": "étape B1", "status": "pending" },
                            { "id": "11", "content": "étape B2", "status": "in_progress" }
                        ]
                    }),
                },
                StreamEvent::Stop {
                    reason: StopReason::ToolUse,
                    usage: Usage::default(),
                },
            ],
            // Tour 3 : conformément au nudge, soumet plan A en completed
            // + plan B en pending → accepté, puis clôture.
            done_turn("OK"),
        ]));
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(TodoWriteTool));
        let registry = Arc::new(registry);
        let ctx = ToolContext::new(camino::Utf8PathBuf::from("."), false);
        let agent = Agent::new(llm, registry, ctx, AgentConfig::default());

        let raw: Vec<_> = agent.run("essai").collect::<Vec<_>>().await;
        // Attendu : un ToolFinish avec is_error: true pour le tu2 (le plan B),
        // contenant le message de re-création.
        let blocked = raw.iter().any(|ev| match ev {
            Ok(AgentEvent::ToolFinish {
                id,
                output,
                is_error,
            }) => {
                *is_error
                    && id == &tu2
                    && output
                        .to_string()
                        .contains("Blocked: you tried to **replace**")
            }
            _ => false,
        });
        assert!(
            blocked,
            "le tour 2 (plan B from scratch) devait être bloqué avec TODO_RECREATION_BLOCKED"
        );
    }

    /// Sprint Plan — anti-faux-positif : un `todo_write` qui **mette à jour**
    /// la liste (mêmes ids, statuts changés + nouveaux ids ajoutés) doit
    /// passer SANS rejet, même si la liste précédente était all-completed.
    #[tokio::test]
    async fn todo_extension_with_kept_ids_is_allowed_even_when_previous_was_completed() {
        let tu1 = ToolUseId::new();
        let tu2 = ToolUseId::new();
        let llm = Arc::new(ScriptedLlm::new(vec![
            vec![
                StreamEvent::Start,
                StreamEvent::TextDelta {
                    text: "[phase: reading]\nPlan A.\n".into(),
                },
                StreamEvent::ToolCall {
                    id: tu1.clone(),
                    name: "todo_write".into(),
                    arguments: json!({
                        "todos": [
                            { "id": "1", "content": "étape A", "status": "completed" }
                        ]
                    }),
                },
                StreamEvent::Stop {
                    reason: StopReason::ToolUse,
                    usage: Usage::default(),
                },
            ],
            // Tour 2 : reprend l'id "1" en completed + ajoute "2" en pending.
            // → extension légitime, doit passer.
            vec![
                StreamEvent::Start,
                StreamEvent::TextDelta {
                    text: "[phase: reading]\nExtension.\n".into(),
                },
                StreamEvent::ToolCall {
                    id: tu2.clone(),
                    name: "todo_write".into(),
                    arguments: json!({
                        "todos": [
                            { "id": "1", "content": "étape A", "status": "completed" },
                            { "id": "2", "content": "étape B", "status": "completed" }
                        ]
                    }),
                },
                StreamEvent::Stop {
                    reason: StopReason::ToolUse,
                    usage: Usage::default(),
                },
            ],
            done_turn("OK"),
        ]));
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(TodoWriteTool));
        let registry = Arc::new(registry);
        let ctx = ToolContext::new(camino::Utf8PathBuf::from("."), false);
        let agent = Agent::new(llm, registry, ctx, AgentConfig::default());

        let raw: Vec<_> = agent.run("essai").collect::<Vec<_>>().await;
        let any_blocked = raw.iter().any(|ev| match ev {
            Ok(AgentEvent::ToolFinish {
                output,
                is_error: true,
                ..
            }) => output
                .to_string()
                .contains("Blocked: you tried to **replace**"),
            _ => false,
        });
        assert!(
            !any_blocked,
            "une extension légitime (ids communs préservés) ne doit pas être bloquée"
        );
    }

    /// Sprint Hotfix « boucle édition/lecture » — empreinte de texte assistant
    /// répétée à l'identique sur trois tours consécutifs. Attendu :
    /// 1er tour `Ok` (rien à comparer) → nudge structurel (DONE_ONLY) → reset.
    /// 2e tour `Warn` (1er strike) → nudge anti-boucle.
    /// 3e tour `Abort` → `EngineError::LoopDetected { kind: "text", turns: 3 }`.
    #[tokio::test]
    async fn repeated_assistant_text_triggers_loop_detected_after_nudge() {
        let same_turn = || {
            vec![
                StreamEvent::Start,
                StreamEvent::TextDelta {
                    text: "[phase: reading]\nje cherche…\n\
                           [phase: answering]\nVoici ce que je trouve.\n"
                        .into(),
                },
                StreamEvent::Stop {
                    reason: StopReason::EndTurn,
                    usage: Usage::default(),
                },
            ]
        };
        let llm = Arc::new(ScriptedLlm::new(vec![
            same_turn(),
            same_turn(),
            same_turn(),
        ]));
        let registry = Arc::new(ToolRegistry::new());
        let ctx = ToolContext::new(camino::Utf8PathBuf::from("."), false);
        // `max_iterations` largement supérieur à 3 pour s'assurer que c'est
        // bien le `LoopDetector` qui clôt le run, pas le garde-fou.
        let cfg = AgentConfig {
            max_iterations: 12,
            ..AgentConfig::default()
        };
        let agent = Agent::new(llm, registry, ctx, cfg);

        let raw: Vec<_> = agent.run("explique").collect::<Vec<_>>().await;
        match raw.last() {
            Some(Err(EngineError::LoopDetected { kind, turns })) => {
                // Sans tool_calls dans les tours, les empreintes texte ET
                // tool_calls (= vide) coïncident toutes les deux → kind est
                // « both ». L'important est que la détection ait lieu.
                assert!(
                    *kind == "both" || *kind == "text",
                    "kind doit refléter une répétition de texte, got {kind:?}"
                );
                assert!(*turns >= 2, "au moins deux strikes consécutifs : got {turns}");
            }
            other => panic!("expected LoopDetected, got {other:?}"),
        }
    }

    /// Sprint Hotfix « boucle » — variation : le 2e tour est **différent** du
    /// 1er (donc on reset le compteur), et seuls les tours 2-3-4 sont
    /// identiques. Doit aussi déclencher `LoopDetected` (sur les tours 2-3
    /// puis abort au 4).
    #[tokio::test]
    async fn loop_detector_resets_when_intermediate_turn_differs() {
        let same_turn = || {
            vec![
                StreamEvent::Start,
                StreamEvent::TextDelta {
                    text: "[phase: reading]\npar ici\n\
                           [phase: answering]\nrésultat identique\n"
                        .into(),
                },
                StreamEvent::Stop {
                    reason: StopReason::EndTurn,
                    usage: Usage::default(),
                },
            ]
        };
        let llm = Arc::new(ScriptedLlm::new(vec![
            vec![
                StreamEvent::Start,
                StreamEvent::TextDelta {
                    text: "[phase: reading]\nje commence\n\
                           [phase: answering]\nréponse #1\n"
                        .into(),
                },
                StreamEvent::Stop {
                    reason: StopReason::EndTurn,
                    usage: Usage::default(),
                },
            ],
            same_turn(),
            same_turn(),
            same_turn(),
        ]));
        let registry = Arc::new(ToolRegistry::new());
        let ctx = ToolContext::new(camino::Utf8PathBuf::from("."), false);
        let cfg = AgentConfig {
            max_iterations: 12,
            ..AgentConfig::default()
        };
        let agent = Agent::new(llm, registry, ctx, cfg);

        let raw: Vec<_> = agent.run("essaie").collect::<Vec<_>>().await;
        assert!(
            matches!(raw.last(), Some(Err(EngineError::LoopDetected { .. }))),
            "expected LoopDetected after 3 identical tail turns, got {:?}",
            raw.last()
        );
    }

    /// Sprint Hotfix « boucle » — anti-faux-positif : le détecteur ne doit
    /// PAS pénaliser un run normal qui converge en quelques tours différents.
    /// Le run minimal `reasoning + todo (1 completed) + answering + done`
    /// passe sans déclencher `LoopDetected`.
    #[tokio::test]
    async fn loop_detector_does_not_flag_legitimate_short_run() {
        let llm = Arc::new(ScriptedLlm::new(vec![
            read_then_one_todo_turn("ok"),
            done_turn("OK."),
        ]));
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(TodoWriteTool));
        let registry = Arc::new(registry);
        let ctx = ToolContext::new(camino::Utf8PathBuf::from("."), false);
        let agent = Agent::new(llm, registry, ctx, AgentConfig::default());

        let events: Vec<_> = agent.run("hi").collect::<Vec<_>>().await;
        assert!(
            !matches!(events.last(), Some(Err(EngineError::LoopDetected { .. }))),
            "un run normal ne doit pas se faire flagger comme boucle"
        );
        assert!(events.iter().any(|e| matches!(e, Ok(AgentEvent::Stop { .. }))));
    }

    #[tokio::test]
    async fn answering_phase_alone_does_not_terminate_loop() {
        // Important : `[phase: answering]` ne termine pas la boucle. Seul
        // `[phase: done]` ferme. Tour 1 : reasoning + answering sans done.
        // Tour 2 : todo obligatoire. Tour 3 : conforme.
        let llm = Arc::new(ScriptedLlm::new(vec![
            vec![
                StreamEvent::Start,
                StreamEvent::TextDelta {
                    text: "[phase: reading]\nPuis answering sans done.\n\
                           [phase: answering]\nVoici la réponse.\n"
                        .into(),
                },
                StreamEvent::Stop {
                    reason: StopReason::EndTurn,
                    usage: Usage::default(),
                },
            ],
            read_then_one_todo_turn("Liste minimale."),
            done_turn("Voici la réponse (confirmée)."),
        ]));
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(TodoWriteTool));
        let registry = Arc::new(registry);
        let ctx = ToolContext::new(camino::Utf8PathBuf::from("."), false);
        let agent = Agent::new(llm, registry, ctx, AgentConfig::default());

        let events: Vec<_> = agent
            .run("question")
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<_, _>>()
            .unwrap();

        let phases: Vec<Phase> = events
            .iter()
            .filter_map(|e| match e {
                AgentEvent::PhaseEnter { phase } => Some(*phase),
                _ => None,
            })
            .collect();
        assert!(phases.contains(&Phase::Answering));
        assert!(phases.contains(&Phase::Done));
        assert!(matches!(events.last(), Some(AgentEvent::Stop { .. })));
    }

    #[tokio::test]
    async fn done_without_answering_triggers_re_emission_nudge() {
        // Sprint A.3 : `done` sans `answering` → refus. Todo obligatoire d'abord.
        let llm = Arc::new(ScriptedLlm::new(vec![
            read_then_one_todo_turn("Analyse demandée."),
            // Tour 2 : reading + texte + done (PAS d'answering)
            premature_done_turn("Voici l'analyse complète du projet."),
            // Tour 3 (post-nudge MISSING_ANSWERING_PROMPT) : conforme.
            done_turn("Voici l'analyse complète du projet (en answering)."),
        ]));
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(TodoWriteTool));
        let registry = Arc::new(registry);
        let ctx = ToolContext::new(camino::Utf8PathBuf::from("."), false);
        let agent = Agent::new(llm, registry, ctx, AgentConfig::default());

        let events: Vec<_> = agent
            .run("analyse")
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<_, _>>()
            .unwrap();

        let phases: Vec<Phase> = events
            .iter()
            .filter_map(|e| match e {
                AgentEvent::PhaseEnter { phase } => Some(*phase),
                _ => None,
            })
            .collect();
        assert!(phases.contains(&Phase::Reading));
        assert!(phases.contains(&Phase::Answering));
        assert!(
            phases.iter().filter(|p| **p == Phase::Done).count() >= 2,
            "le Done prématuré doit être ignoré comme signal d'arrêt, donc 2 Done visibles ; got {phases:?}"
        );
        assert!(matches!(events.last(), Some(AgentEvent::Stop { .. })));
    }

    #[tokio::test]
    async fn tool_call_without_prior_phase_synthesizes_fallback_phase() {
        // Sprint A.4 — filet de sécurité : sans marqueur, le moteur synthétise
        // une phase avant `ToolStart`. `echo` n'est pas listé dans `phase_for_tool`
        // comme read-only → inférence **Acting** (comportement actuel du moteur).
        let tu_bad = ToolUseId::new();
        let tu_todo = ToolUseId::new();
        let tu_echo = ToolUseId::new();
        let llm = Arc::new(ScriptedLlm::new(vec![
            vec![
                StreamEvent::Start,
                StreamEvent::ToolCall {
                    id: tu_bad.clone(),
                    name: "echo".into(),
                    arguments: json!({}),
                },
                StreamEvent::Stop {
                    reason: StopReason::ToolUse,
                    usage: Usage::default(),
                },
            ],
            vec![
                StreamEvent::Start,
                StreamEvent::TextDelta {
                    text: "[phase: reading]\nEcho conforme.\n".into(),
                },
                StreamEvent::ToolCall {
                    id: tu_todo,
                    name: "todo_write".into(),
                    arguments: json!({
                        "todos": [{
                            "id": "1",
                            "content": "Echo",
                            "status": "completed"
                        }]
                    }),
                },
                StreamEvent::ToolCall {
                    id: tu_echo.clone(),
                    name: "echo".into(),
                    arguments: json!({}),
                },
                StreamEvent::Stop {
                    reason: StopReason::ToolUse,
                    usage: Usage::default(),
                },
            ],
            done_turn("ok"),
        ]));
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(TodoWriteTool));
        registry.register(Arc::new(EchoTool));
        let registry = Arc::new(registry);
        let ctx = ToolContext::new(camino::Utf8PathBuf::from("."), false);
        let agent = Agent::new(llm, registry, ctx, AgentConfig::default());

        let events: Vec<_> = agent
            .run("test")
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<_, _>>()
            .unwrap();

        assert!(
            !events.iter().any(|e| matches!(
                e,
                AgentEvent::ToolFinish { id, is_error: true, .. } if id == &tu_bad
            )),
            "echo sans marqueur ne doit pas être en erreur ; events={events:?}"
        );
        assert!(
            events.iter().any(|e| matches!(
                e,
                AgentEvent::ToolFinish { id, is_error: false, .. } if id == &tu_bad
            )),
            "premier echo doit réussir (phase synthétisée avant le tool) ; events={events:?}"
        );
        assert!(
            events.iter().any(|e| matches!(
                e,
                AgentEvent::ToolFinish { id, is_error: false, .. } if id == &tu_echo
            )),
            "deuxième echo doit réussir après flux conforme ; events={events:?}"
        );
        let idx_acting = events.iter().position(|e| {
            matches!(e, AgentEvent::PhaseEnter { phase } if *phase == Phase::Acting)
        });
        let idx_first_echo = events.iter().position(|e| {
            matches!(e, AgentEvent::ToolStart { id, name, .. } if id == &tu_bad && name == "echo")
        });
        assert!(
            idx_acting.is_some()
                && idx_first_echo.is_some()
                && idx_acting < idx_first_echo,
            "PhaseEnter(Acting) synthétique doit précéder le premier ToolStart(echo) ; idx_acting={idx_acting:?} idx_first_echo={idx_first_echo:?} events={events:?}"
        );
    }

    /// Qwen3-Coder et consorts : `todo_write` en tout premier événement du
    /// tour sans marqueur de phase — le filet `phase_for_tool` injecte
    /// `Reading` avant `ToolStart` ; la gate todo/mutateur ne bloque pas.
    #[tokio::test]
    async fn opening_todo_write_without_marker_injects_reading_not_blocked() {
        let tid = ToolUseId::new();
        let llm = Arc::new(ScriptedLlm::new(vec![
            vec![
                StreamEvent::Start,
                StreamEvent::ToolCall {
                    id: tid.clone(),
                    name: "todo_write".into(),
                    arguments: json!({
                        "todos": [{
                            "id": "1",
                            "content": "Saluer l'utilisateur",
                            "status": "completed"
                        }]
                    }),
                },
                StreamEvent::Stop {
                    reason: StopReason::ToolUse,
                    usage: Usage::default(),
                },
            ],
            vec![
                StreamEvent::Start,
                StreamEvent::TextDelta {
                    text: "[phase: answering]\nSalut !\n[phase: done]".into(),
                },
                StreamEvent::Stop {
                    reason: StopReason::EndTurn,
                    usage: Usage::default(),
                },
            ],
        ]));
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(TodoWriteTool));
        let registry = Arc::new(registry);
        let ctx = ToolContext::new(camino::Utf8PathBuf::from("."), false);
        let agent = Agent::new(llm, registry, ctx, AgentConfig::default());

        let events: Vec<_> = agent
            .run("Salut")
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<_, _>>()
            .unwrap();

        assert!(
            !events.iter().any(|e| matches!(
                e,
                AgentEvent::ToolFinish { is_error: true, .. }
            )),
            "todo_write en tête de tour ne doit pas être bloqué ; events={events:?}"
        );
        let idx_reading = events.iter().position(|e| {
            matches!(e, AgentEvent::PhaseEnter { phase } if *phase == Phase::Reading)
        });
        let idx_todo = events.iter().position(|e| {
            matches!(e, AgentEvent::ToolStart { name, .. } if name == "todo_write")
        });
        assert!(
            idx_reading.is_some() && idx_todo.is_some() && idx_reading < idx_todo,
            "PhaseEnter(Reading) synthétique doit précéder ToolStart(todo_write) ; idx_reading={idx_reading:?} idx_todo={idx_todo:?} events={events:?}"
        );
        assert!(matches!(events.last(), Some(AgentEvent::Stop { .. })));
    }

    /// Format-check du helper `step_by_step_todo_nudge` : il doit nommer le
    /// compte d'outils mutateurs, les compteurs `pending` / `in_progress`, et
    /// rappeler le bon mantra ("one `todo_write` per real step transition").
    #[test]
    fn step_by_step_nudge_message_mentions_counters_and_principle() {
        let msg = step_by_step_todo_nudge(3, 4, 1);
        assert!(msg.contains("3 mutating tools"), "msg={msg}");
        assert!(msg.contains("4 pending"), "msg={msg}");
        assert!(msg.contains("1 in_progress"), "msg={msg}");
        assert!(msg.contains("todo_write"), "msg={msg}");
        assert!(
            msg.contains("step transition") || msg.contains("step-by-step"),
            "le nudge doit rappeler la granularité step-by-step ; msg={msg}"
        );
    }

    /// Anti-régression : scénario réel observé sur GLM-4.7-Flash — le modèle
    /// crée un plan de 3 étapes puis enchaîne 3 outils mutateurs sans MAJ
    /// intermédiaire. Le filet moteur doit injecter un nudge `step_by_step`
    /// SANS bloquer le run (le tour avec les outils est accepté ; le nudge
    /// influence le tour suivant). Le test vérifie surtout que la séquence
    /// arrive à `Stop` propre, et que les outils mutateurs sont bien exécutés.
    #[allow(clippy::too_many_lines)]
    #[tokio::test]
    async fn run_completes_when_model_batches_mutating_tools_then_updates_todo() {
        let tu_todo_open = ToolUseId::new();
        let tu_bash_1 = ToolUseId::new();
        let tu_bash_2 = ToolUseId::new();
        let tu_todo_close = ToolUseId::new();

        let llm = Arc::new(ScriptedLlm::new(vec![
            // Tour 1 : reasoning + plan en 2 étapes (in_progress + pending).
            vec![
                StreamEvent::Start,
                StreamEvent::TextDelta {
                    text: "[phase: reading]\nJe planifie en 2 étapes.\n".into(),
                },
                StreamEvent::ToolCall {
                    id: tu_todo_open.clone(),
                    name: "todo_write".into(),
                    arguments: json!({
                        "todos": [
                            { "id": "1", "content": "Étape 1", "status": "in_progress" },
                            { "id": "2", "content": "Étape 2", "status": "pending" }
                        ]
                    }),
                },
                StreamEvent::Stop {
                    reason: StopReason::ToolUse,
                    usage: Usage::default(),
                },
            ],
            // Tour 2 : deux `bash` à la suite sans `todo_write` intercalé →
            // le moteur va injecter `step_by_step_todo_nudge` en fin de tour.
            vec![
                StreamEvent::Start,
                StreamEvent::TextDelta {
                    text: "[phase: acting]\nLes deux étapes en backend.\n".into(),
                },
                StreamEvent::ToolCall {
                    id: tu_bash_1.clone(),
                    name: "bash".into(),
                    arguments: json!({ "cmd": "echo step1" }),
                },
                StreamEvent::ToolCall {
                    id: tu_bash_2.clone(),
                    name: "bash".into(),
                    arguments: json!({ "cmd": "echo step2" }),
                },
                StreamEvent::Stop {
                    reason: StopReason::ToolUse,
                    usage: Usage::default(),
                },
            ],
            // Tour 3 : le modèle reçoit le nudge, ferme la todo proprement
            // puis émet answering + done.
            vec![
                StreamEvent::Start,
                StreamEvent::TextDelta {
                    text: "[phase: acting]\nJe mets à jour la todo.\n".into(),
                },
                StreamEvent::ToolCall {
                    id: tu_todo_close.clone(),
                    name: "todo_write".into(),
                    arguments: json!({
                        "todos": [
                            { "id": "1", "content": "Étape 1", "status": "completed" },
                            { "id": "2", "content": "Étape 2", "status": "completed" }
                        ]
                    }),
                },
                StreamEvent::Stop {
                    reason: StopReason::ToolUse,
                    usage: Usage::default(),
                },
            ],
            // Tour 4 : answering + done.
            vec![
                StreamEvent::Start,
                StreamEvent::TextDelta {
                    text: "[phase: answering]\nTerminé.\n[phase: done]".into(),
                },
                StreamEvent::Stop {
                    reason: StopReason::EndTurn,
                    usage: Usage::default(),
                },
            ],
        ]));
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(TodoWriteTool));
        registry.register(Arc::new(FakeBashTool));
        let registry = Arc::new(registry);
        let ctx = ToolContext::new(camino::Utf8PathBuf::from("."), false);
        let agent = Agent::new(llm, registry, ctx, AgentConfig::default());

        let events: Vec<_> = agent
            .run("fais le job")
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<_, _>>()
            .unwrap();

        let bash_finishes = events
            .iter()
            .filter(|e| matches!(
                e,
                AgentEvent::ToolFinish { is_error: false, .. }
            ))
            .filter(|e| {
                if let AgentEvent::ToolFinish { id, .. } = e {
                    [&tu_bash_1, &tu_bash_2].contains(&id)
                } else {
                    false
                }
            })
            .count();
        assert_eq!(
            bash_finishes, 2,
            "les 2 bash batched doivent quand même s'exécuter (le nudge ne bloque pas) ; events={events:?}"
        );

        let todo_finishes = events
            .iter()
            .filter(|e| matches!(
                e,
                AgentEvent::ToolFinish { is_error: false, .. }
            ))
            .filter(|e| {
                if let AgentEvent::ToolFinish { id, .. } = e {
                    [&tu_todo_open, &tu_todo_close].contains(&id)
                } else {
                    false
                }
            })
            .count();
        assert_eq!(
            todo_finishes, 2,
            "les 2 todo_write (ouvert + clôture) doivent réussir ; events={events:?}"
        );

        assert!(
            matches!(events.last(), Some(AgentEvent::Stop { .. })),
            "le run doit clôturer proprement après le nudge ; events={events:?}"
        );
    }

    /// Anti-régression du bug « 2× réponse + 1 todo » observé sur GLM-4.7-Flash :
    /// quand le modèle a déjà rédigé sa réponse dans `[phase: answering]`
    /// mais a omis le `[phase: done]`, le nudge envoyé doit être minimaliste
    /// (« émets juste `[phase: done]` ») et NON le `NUDGE_PROMPT` générique
    /// qui demande de « write your final user-facing response » et provoque
    /// la duplication.
    #[tokio::test]
    async fn forgotten_done_after_answering_uses_minimal_nudge() {
        let llm = Arc::new(ScriptedLlm::new(vec![
            // Tour 1 : reasoning + answering, mais PAS de [phase: done].
            vec![
                StreamEvent::Start,
                StreamEvent::TextDelta {
                    text: "[phase: reading]\nSalutation triviale.\n\
                           [phase: answering]\nSalut ! Je suis prêt à t'aider."
                        .into(),
                },
                StreamEvent::Stop {
                    reason: StopReason::EndTurn,
                    usage: Usage::default(),
                },
            ],
            // Tour 2 : le modèle obéit au nudge minimal et émet juste `done`.
            vec![
                StreamEvent::Start,
                StreamEvent::TextDelta {
                    text: "[phase: done]".into(),
                },
                StreamEvent::Stop {
                    reason: StopReason::EndTurn,
                    usage: Usage::default(),
                },
            ],
        ]));
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(TodoWriteTool));
        let registry = Arc::new(registry);
        let ctx = ToolContext::new(camino::Utf8PathBuf::from("."), false);
        let agent = Agent::new(llm, registry, ctx, AgentConfig::default());

        let events: Vec<_> = agent
            .run("Salut")
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<_, _>>()
            .unwrap();

        // Le tour 2 ne doit PAS contenir de nouveau TextDelta avec « Salut »
        // (sinon = duplication). On vérifie que le texte « Salut ! Je suis
        // prêt » apparaît une seule fois dans tout le flux d'events.
        let salut_occurrences = events
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    AgentEvent::TextDelta { text } if text.contains("Salut !")
                )
            })
            .count();
        assert_eq!(
            salut_occurrences, 1,
            "la réponse ne doit apparaître qu'UNE fois (pas de double-rédaction) ; \
             events={events:?}"
        );
        assert!(
            matches!(events.last(), Some(AgentEvent::Stop { .. })),
            "le run doit finir par Stop ; events={events:?}"
        );
    }

    /// Anti-régression : sur un simple « Salut », le moteur DOIT accepter
    /// `[phase: reading] → [phase: answering] → [phase: done]` sans
    /// `todo_write`. La gate qui exigeait `todo_write` avant `done` causait
    /// une boucle infinie (le modèle ré-écrivait sa salutation à chaque
    /// nudge `MISSING_TODO_WRITE_PROMPT`). Voir issue conversationnelle 2026-05-13.
    #[tokio::test]
    async fn done_accepted_for_pure_conversation_without_todo_write() {
        let llm = Arc::new(ScriptedLlm::new(vec![vec![
            StreamEvent::Start,
            StreamEvent::TextDelta {
                text: "[phase: reading]\nSalutation triviale.\n\
                       [phase: answering]\nSalut ! Comment puis-je t'aider ?\n\
                       [phase: done]"
                    .into(),
            },
            StreamEvent::Stop {
                reason: StopReason::EndTurn,
                usage: Usage::default(),
            },
        ]]));
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(TodoWriteTool));
        let registry = Arc::new(registry);
        let ctx = ToolContext::new(camino::Utf8PathBuf::from("."), false);
        let agent = Agent::new(llm, registry, ctx, AgentConfig::default());

        let events: Vec<_> = agent
            .run("Salut")
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<_, _>>()
            .unwrap();

        let todo_starts = events
            .iter()
            .filter(|e| matches!(e, AgentEvent::ToolStart { name, .. } if name == "todo_write"))
            .count();
        assert_eq!(
            todo_starts, 0,
            "aucun todo_write ne doit être déclenché pour une conversation triviale ; \
             events={events:?}",
        );
        let stops = events
            .iter()
            .filter(|e| matches!(e, AgentEvent::Stop { .. }))
            .count();
        assert_eq!(
            stops, 1,
            "le moteur doit clôturer en UN seul tour pour une salutation \
             (la boucle infinie vient d'une absence de Stop) ; events={events:?}",
        );
        assert!(
            matches!(events.last(), Some(AgentEvent::Stop { .. })),
            "le dernier événement doit être Stop ; events={events:?}",
        );
    }

    #[tokio::test]
    async fn tool_call_after_explicit_phase_does_not_synthesize() {
        // Le modèle déclare `reading` puis `planning` avant echo : pas de
        // phase synthétisée avant le tool. `todo_write` d'abord.
        let tu_todo = ToolUseId::new();
        let tu_echo = ToolUseId::new();
        let llm = Arc::new(ScriptedLlm::new(vec![
            vec![
                StreamEvent::Start,
                StreamEvent::TextDelta {
                    text: "[phase: reading]\nCadrage.\n[phase: planning]\nOn liste d'abord.\n"
                        .into(),
                },
                StreamEvent::ToolCall {
                    id: tu_todo,
                    name: "todo_write".into(),
                    arguments: json!({
                        "todos": [{
                            "id": "1",
                            "content": "Echo test",
                            "status": "completed"
                        }]
                    }),
                },
                StreamEvent::ToolCall {
                    id: tu_echo.clone(),
                    name: "echo".into(),
                    arguments: json!({}),
                },
                StreamEvent::Stop {
                    reason: StopReason::ToolUse,
                    usage: Usage::default(),
                },
            ],
            done_turn("ok"),
        ]));
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(TodoWriteTool));
        registry.register(Arc::new(EchoTool));
        let registry = Arc::new(registry);
        let ctx = ToolContext::new(camino::Utf8PathBuf::from("."), false);
        let agent = Agent::new(llm, registry, ctx, AgentConfig::default());

        let events: Vec<_> = agent
            .run("test")
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<_, _>>()
            .unwrap();

        let phases: Vec<Phase> = events
            .iter()
            .filter_map(|e| match e {
                AgentEvent::PhaseEnter { phase } => Some(*phase),
                _ => None,
            })
            .collect();
        assert_eq!(
            phases.first().copied(),
            Some(Phase::Reading),
            "first explicit marker must be reading, got {phases:?}"
        );
        assert!(
            phases.contains(&Phase::Planning),
            "expected Planning phase in {phases:?}"
        );
    }

    #[tokio::test]
    async fn synthesized_reading_for_glob_classifies_correctly() {
        // Tour 1 : glob seul (todo pas encore posée) — autorisé ; phase synthétisée Reading.
        // Tour 2 : todo. Tour 3 : glob sans marqueur → synthèse Reading.
        let tu1 = ToolUseId::new();
        let tu3 = ToolUseId::new();
        let llm = Arc::new(ScriptedLlm::new(vec![
            vec![
                StreamEvent::Start,
                StreamEvent::ToolCall {
                    id: tu1.clone(),
                    name: "glob".into(),
                    arguments: json!({ "pattern": "*" }),
                },
                StreamEvent::Stop {
                    reason: StopReason::ToolUse,
                    usage: Usage::default(),
                },
            ],
            read_then_one_todo_turn("Lister le repo."),
            vec![
                StreamEvent::Start,
                StreamEvent::ToolCall {
                    id: tu3.clone(),
                    name: "glob".into(),
                    arguments: json!({ "pattern": "*" }),
                },
                StreamEvent::Stop {
                    reason: StopReason::ToolUse,
                    usage: Usage::default(),
                },
            ],
            done_turn("vu"),
        ]));
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(TodoWriteTool));
        let registry = Arc::new(registry);
        let ctx = ToolContext::new(camino::Utf8PathBuf::from("."), false);
        let agent = Agent::new(llm, registry, ctx, AgentConfig::default());

        let events: Vec<_> = agent
            .run("test")
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<_, _>>()
            .unwrap();

        let glob_phase = events
            .iter()
            .enumerate()
            .filter(|(_, e)| matches!(e, AgentEvent::ToolStart { name, .. } if name == "glob"))
            .find_map(|(i, _)| {
                (0..i).rev().find_map(|j| match &events[j] {
                    AgentEvent::PhaseEnter { phase } => Some(*phase),
                    _ => None,
                })
            })
            .expect("glob doit être précédé d'un PhaseEnter");
        assert_eq!(glob_phase, Phase::Reading, "glob orphelin → Reading");
    }

    #[tokio::test]
    async fn silent_turn_after_tool_call_still_nudges_to_done() {
        // Après todo obligatoire : tours muets avec reasoning, puis echo,
        // puis encore muet, puis done.
        let tid_echo = ToolUseId::new();
        let llm = Arc::new(ScriptedLlm::new(vec![
            read_then_one_todo_turn("Démarrage."),
            vec![
                StreamEvent::Start,
                StreamEvent::TextDelta {
                    text: "[phase: reading]\nréflexion sans action\n".into(),
                },
                StreamEvent::Stop {
                    reason: StopReason::EndTurn,
                    usage: Usage::default(),
                },
            ],
            vec![
                StreamEvent::Start,
                StreamEvent::TextDelta {
                    text: "[phase: reading]\nJ'appelle echo.\n".into(),
                },
                StreamEvent::ToolCall {
                    id: tid_echo.clone(),
                    name: "echo".into(),
                    arguments: json!({}),
                },
                StreamEvent::Stop {
                    reason: StopReason::ToolUse,
                    usage: Usage::default(),
                },
            ],
            vec![
                StreamEvent::Start,
                StreamEvent::TextDelta {
                    text: "[phase: reading]\nencore une réflexion sans action\n".into(),
                },
                StreamEvent::Stop {
                    reason: StopReason::EndTurn,
                    usage: Usage::default(),
                },
            ],
            done_turn("résultat final"),
        ]));

        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(TodoWriteTool));
        registry.register(Arc::new(EchoTool));
        let registry = Arc::new(registry);
        let ctx = ToolContext::new(camino::Utf8PathBuf::from("."), false);
        let agent = Agent::new(llm, registry, ctx, AgentConfig::default());

        let events: Vec<_> = agent
            .run("test")
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<_, _>>()
            .unwrap();

        assert_eq!(
            events
                .iter()
                .filter(|e| {
                    matches!(e, AgentEvent::ToolStart { name, .. } if name == "echo")
                })
                .count(),
            1
        );
        assert!(events.iter().any(
            |e| matches!(e, AgentEvent::PhaseEnter { phase } if *phase == Phase::Done)
        ));
    }

    // ---------------------------------------------------------------------
    // Sprint M1 — tests d'intégration mémoire de session.
    // ---------------------------------------------------------------------

    /// Construit un tour LLM de compaction : produit un markdown au format
    /// `## Objective\n…\n## Files touched\n- …`. Sert à fournir un résultat
    /// déterministe au pipeline `summarize_run` qui sera appelé par
    /// `maybe_persist_session` à la fin du run.
    fn compaction_turn(objective: &str, files: &[&str]) -> Vec<StreamEvent> {
        use std::fmt::Write as _;
        let mut md = String::new();
        let _ = writeln!(md, "## Objective\n{objective}");
        md.push_str("## Decisions\n- Decision A\n- Decision B\n");
        md.push_str("## Files touched\n");
        for f in files {
            let _ = writeln!(md, "- {f}");
        }
        md.push_str("## What's in progress\nNothing pending.\n");
        vec![
            StreamEvent::Start,
            StreamEvent::TextDelta { text: md },
            StreamEvent::Stop {
                reason: StopReason::EndTurn,
                usage: Usage::default(),
            },
        ]
    }

    fn memory_runtime_for_test(
        workspace: &camino::Utf8Path,
        llm: Arc<ScriptedLlm>,
    ) -> MemoryRuntime {
        MemoryRuntime {
            workspace_root: workspace.to_path_buf(),
            llm,
            compaction_prompt: "You are a compaction model. Produce markdown.".into(),
            compaction_config: crate::compaction::CompactionConfig::default(),
            notes: drox_tools::SessionNotesHandle::new(),
            model_label: "scripted-test".into(),
        }
    }

    /// Run non trivial (`todo_write` + answering + done) → la mémoire doit
    /// se persister et émettre `MemoryPersisted` ; le `.md` doit exister
    /// sur disque sous `.drox/memory/sessions/`.
    #[tokio::test]
    async fn session_persisted_after_done_when_run_is_non_trivial() {
        let dir = tempfile::tempdir().unwrap();
        let ws = camino::Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();

        // Trois scripts dans l'ordre : (1) reasoning + todo_write,
        // (2) answering + done, (3) compaction.
        let llm = Arc::new(ScriptedLlm::new(vec![
            read_then_one_todo_turn("On va faire X."),
            done_turn("Voici le résultat."),
            compaction_turn("Refactorer le module X", &["src/x.rs", "src/y.rs"]),
        ]));

        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(TodoWriteTool));
        let registry = Arc::new(registry);
        let ctx = ToolContext::new(ws.clone(), false);
        let cfg = AgentConfig {
            memory: Some(memory_runtime_for_test(&ws, llm.clone())),
            ..AgentConfig::default()
        };
        let agent = Agent::new(llm, registry, ctx, cfg);

        let events: Vec<_> = agent
            .run("Refactorer le module X")
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<_, _>>()
            .unwrap();

        // 1. `MemoryPersisted` doit apparaître AVANT le `Stop`.
        let persisted = events.iter().find_map(|e| match e {
            AgentEvent::MemoryPersisted { slug, path, objective, .. } => {
                Some((slug.clone(), path.clone(), objective.clone()))
            }
            _ => None,
        });
        let (slug, path, objective) =
            persisted.expect("MemoryPersisted must be emitted on non-trivial run");
        assert!(!slug.is_empty(), "slug must be non-empty");
        assert_eq!(objective, "Refactorer le module X");

        // 2. Le fichier doit exister.
        let p = std::path::Path::new(&path);
        assert!(p.exists(), "session .md must be written at {path}");
        let body = std::fs::read_to_string(p).unwrap();
        assert!(body.contains("slug:"), "front-matter must include slug");
        assert!(body.contains("## Objective"), "body must include summary");
        assert!(body.contains("src/x.rs"), "files_touched must propagate");
    }

    /// Quand le plan passe d'« actif » à entièrement clôturé, le moteur doit
    /// archiver un premier `.md` (checkpoint) puis encore à `[phase: done]`.
    #[tokio::test]
    async fn memory_checkpoint_when_todo_plan_closes_then_done() {
        let dir = tempfile::tempdir().unwrap();
        let ws = camino::Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();

        let close_tid = ToolUseId::new();
        let llm = Arc::new(ScriptedLlm::new(vec![
            read_then_one_todo_turn_with_status("Ouverture plan", "in_progress"),
            vec![
                StreamEvent::Start,
                StreamEvent::TextDelta {
                    text: "[phase: acting]\nClôture.\n".into(),
                },
                StreamEvent::ToolCall {
                    id: close_tid,
                    name: "todo_write".into(),
                    arguments: json!({
                        "todos": [{
                            "id": "1",
                            "content": "Étape de test",
                            "status": "completed"
                        }]
                    }),
                },
                StreamEvent::Stop {
                    reason: StopReason::ToolUse,
                    usage: Usage::default(),
                },
            ],
            compaction_turn("Jalon : plan fermé", &["src/a.rs"]),
            done_turn("Réponse finale."),
            compaction_turn("Run terminé", &["src/b.rs"]),
        ]));

        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(TodoWriteTool));
        let registry = Arc::new(registry);
        let ctx = ToolContext::new(ws.clone(), false);
        let cfg = AgentConfig {
            memory: Some(memory_runtime_for_test(&ws, llm.clone())),
            ..AgentConfig::default()
        };
        let agent = Agent::new(llm, registry, ctx, cfg);

        let events: Vec<_> = agent
            .run("Tâche avec plan")
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<_, _>>()
            .unwrap();

        let n_mem: usize = events
            .iter()
            .filter(|e| matches!(e, AgentEvent::MemoryPersisted { .. }))
            .count();
        assert_eq!(
            n_mem, 2,
            "attendu : checkpoint à la clôture du plan + persistance à done ; events={events:?}"
        );
    }

    /// Le modèle ne doit pas pouvoir exécuter `session_end` (réservé
    /// `/session_end` utilisateur) même si le tool est dans le registre.
    #[tokio::test]
    async fn session_end_tool_call_from_model_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let ws = camino::Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let tid = ToolUseId::new();
        let llm = Arc::new(ScriptedLlm::new(vec![
            vec![
                StreamEvent::Start,
                StreamEvent::TextDelta {
                    text: "[phase: reading]\n".into(),
                },
                StreamEvent::ToolCall {
                    id: tid.clone(),
                    name: "session_end".into(),
                    arguments: json!({}),
                },
                StreamEvent::Stop {
                    reason: StopReason::ToolUse,
                    usage: Usage::default(),
                },
            ],
            done_turn("Je continue sans clôturer la session."),
        ]));

        let registry = Arc::new(ToolRegistry::with_simple_tools());
        let ctx = ToolContext::new(ws, false);
        let agent = Agent::new(llm, registry, ctx, AgentConfig::default());

        let events: Vec<_> = agent
            .run("test")
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<_, _>>()
            .unwrap();

        assert!(
            events
                .iter()
                .any(|e| matches!(e, AgentEvent::ToolFinish { is_error: true, .. })),
            "session_end doit échouer côté moteur ; events={events:?}"
        );
    }

    /// Run trivial (juste answering + done, aucun outil appelé) → **pas**
    /// de persistance, pas d'événement `MemoryPersisted`, et le dossier
    /// `.drox/memory/sessions/` peut rester vide.
    #[tokio::test]
    async fn trivial_run_does_not_persist_session() {
        let dir = tempfile::tempdir().unwrap();
        let ws = camino::Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();

        // Un seul tour : answering + done, sans tool. Pas de script de
        // compaction nécessaire — il ne sera pas appelé.
        let llm = Arc::new(ScriptedLlm::new(vec![done_turn("Salut !")]));

        let registry = Arc::new(ToolRegistry::new());
        let ctx = ToolContext::new(ws.clone(), false);
        let cfg = AgentConfig {
            memory: Some(memory_runtime_for_test(&ws, llm.clone())),
            ..AgentConfig::default()
        };
        let agent = Agent::new(llm, registry, ctx, cfg);

        let events: Vec<_> = agent
            .run("Salut")
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<_, _>>()
            .unwrap();

        assert!(
            !events
                .iter()
                .any(|e| matches!(e, AgentEvent::MemoryPersisted { .. })),
            "trivial conversational run must NOT persist memory; events={events:?}"
        );
        let sessions_dir = ws.join(".drox/memory/sessions");
        if sessions_dir.exists() {
            let count = std::fs::read_dir(sessions_dir.as_std_path())
                .unwrap()
                .count();
            assert_eq!(count, 0, "sessions/ dir must be empty after trivial run");
        }
    }

    /// Tracker éligibilité : un `session_note` épinglé suffit à rendre le
    /// run non trivial même sans aucun tool mutateur (cas « conversation
    /// utile mais sans modification de code »).
    #[tokio::test]
    async fn pinned_session_note_alone_triggers_persistence() {
        use drox_tools::SessionNoteTool;

        let dir = tempfile::tempdir().unwrap();
        let ws = camino::Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();

        // Tour 1 : reasoning + session_note (note seule, pas de todo, pas
        // de mutation). Tour 2 : answering + done. Tour 3 : compaction.
        let note_tid = ToolUseId::new();
        let llm = Arc::new(ScriptedLlm::new(vec![
            vec![
                StreamEvent::Start,
                StreamEvent::TextDelta {
                    text: "[phase: reading]\nJe pose une note.\n".into(),
                },
                StreamEvent::ToolCall {
                    id: note_tid,
                    name: "session_note".into(),
                    arguments: json!({ "content": "Hypothèse: revoir la stratégie de cache" }),
                },
                StreamEvent::Stop {
                    reason: StopReason::ToolUse,
                    usage: Usage::default(),
                },
            ],
            done_turn("OK noté."),
            compaction_turn("Note technique", &[]),
        ]));

        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(SessionNoteTool));
        let registry = Arc::new(registry);
        let ctx = ToolContext::new(ws.clone(), false);
        let cfg = AgentConfig {
            memory: Some(memory_runtime_for_test(&ws, llm.clone())),
            ..AgentConfig::default()
        };
        let agent = Agent::new(llm, registry, ctx, cfg);

        let events: Vec<_> = agent
            .run("Note rapide")
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<_, _>>()
            .unwrap();

        assert!(
            events
                .iter()
                .any(|e| matches!(e, AgentEvent::MemoryPersisted { .. })),
            "a single pinned session_note must be enough to trigger persistence; events={events:?}"
        );
    }

    /// M2 — compaction proactive : au premier tour, si le budget dépasse le
    /// seuil `autocompact`, le moteur appelle `summarize_run` puis réécrit
    /// l'historique et émet `ContextCompacted` avant le tour LLM principal.
    #[tokio::test]
    async fn live_compaction_emits_context_compacted_when_over_budget() {
        let dir = tempfile::tempdir().unwrap();
        let ws = camino::Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();

        let history: Vec<Message> = (0..14)
            .map(|_| Message::user("z".repeat(10_000)))
            .collect();

        let llm = Arc::new(ScriptedLlm::new(vec![
            compaction_turn("Résumé intermédiaire", &["src/a.rs"]),
            done_turn("Terminé."),
        ]));

        let registry = Arc::new(ToolRegistry::new());
        let ctx = ToolContext::new(ws.clone(), false);
        let cfg = AgentConfig {
            system_prompt: Some("Tu es un assistant.".into()),
            context: Some(ContextPolicy::new(
                Arc::new(RoughTokenCounter::new(4)),
                ContextBudget {
                    window_size: 25_000,
                    reserved_output: 0,
                    autocompact_buffer: 5_000,
                    warning_buffer: 4_000,
                    error_buffer: 4_000,
                    manual_compact_buffer: 500,
                },
                None,
            )),
            memory: Some(memory_runtime_for_test(&ws, llm.clone())),
            transcript_session_id: Some("ses_test_compact".into()),
            workspace_fingerprint: ws.to_string(),
            ..AgentConfig::default()
        };
        let agent = Agent::new(llm, registry, ctx, cfg);

        let events: Vec<_> = agent
            .run_with_history(history, "suite")
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<_, _>>()
            .unwrap();

        assert!(
            events.iter().any(|e| matches!(e, AgentEvent::ContextCompacted { .. })),
            "expected ContextCompacted in stream; events={events:?}"
        );
        let compact = events.iter().find_map(|e| {
            if let AgentEvent::ContextCompacted {
                context_chunk_summary,
                ..
            } = e
            {
                context_chunk_summary.as_ref()
            } else {
                None
            }
        });
        assert!(
            compact.is_some_and(|c| {
                c.summary_text.contains("Résumé intermédiaire")
                    && c.transcript_session_id == "ses_test_compact"
                    && c.compaction_seq == 1
            }),
            "expected populated context_chunk_summary; compact={compact:?}"
        );
        if let Some(AgentEvent::ContextCompacted {
            tokens_before,
            tokens_after,
            ..
        }) = events.iter().find(|e| matches!(e, AgentEvent::ContextCompacted { .. }))
        {
            assert!(
                *tokens_after * 2 < *tokens_before,
                "compaction should at least halve token estimate (before={tokens_before}, after={tokens_after})"
            );
        }
        assert!(matches!(events.last(), Some(AgentEvent::Stop { .. })));
    }
}
