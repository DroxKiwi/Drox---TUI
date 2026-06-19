//! Compaction — résume un run pour persistance dans `.drox/memory/sessions/`.
//!
//! Sprint M1 (architecture mémoire unifiée). La compaction est le seul
//! mécanisme qui produit un résumé : selon où on appelle [`summarize_run`],
//! ce résumé est soit
//!
//! - **persisté** (fin de run, ce module aujourd'hui) ;
//! - **réinjecté dans le contexte** pour soulager la fenêtre (à venir, M2).
//!
//! Au V1 on n'expose que l'usage « persistance ». L'usage « live »
//! (`try_live_compact`) réutilise la même fonction [`summarize_run`] sur un
//! préfixe d'historique et remplace ce préfixe par un message `system`
//! checkpoint — voir [`try_live_compact`].
//!
//! ## Pipeline
//!
//! 1. Le moteur (à `[phase: done]` réussi, run **non trivial**) appelle
//!    [`summarize_run`] avec :
//!    - le client LLM,
//!    - le prompt système de compaction (cf. `drox-cli/prompts.rs`),
//!    - l'historique complet du run,
//!    - les notes épinglées via `session_note`,
//!    - une [`CompactionConfig`] (`temperature`, `max_tokens`).
//! 2. La fonction condense l'historique en un blob textuel signé (rôle +
//!    contenu, `tool_results` tronqués si nécessaire), l'envoie au modèle avec
//!    le prompt système, et collecte le markdown produit.
//! 3. Elle ré-extrait du markdown l'`objective` (1 ligne) et la liste des
//!    `files_touched` (section dédiée) pour remplir le front-matter de la
//!    session persistée.
//!
//! Si l'extraction rate (le modèle a livré du markdown libre sans suivre
//! le format), on dégrade proprement : `objective` = première ligne non
//! vide, `files_touched` = liste vide. Le body markdown brut reste
//! sauvegardé tel quel — l'utilisateur peut toujours le lire.

use std::fmt::Write as _;

use drox_llm::{ChatOptions, LlmClient};
use drox_types::{Content, Message, Role, StreamEvent, Usage};
use drox_tools::SessionNote;
use futures::StreamExt;
use tracing::{debug, warn};

use crate::context::ContextPolicy;
use crate::error::EngineError;

/// Limite d'un `tool_result` dans le condensé envoyé au modèle de compaction.
const DEFAULT_SUMMARIZE_TOOL_RESULT_TRUNCATE: usize = 300;

/// Limite par défaut d'un `tool_result` réinjecté dans la conversation à
/// résumer (persist M1). Au-delà, le bloc est tronqué.
const DEFAULT_TOOL_RESULT_TRUNCATE: usize = 800;

/// Préambule placé devant chaque `tool_result` tronqué dans le condensé.
/// Permet au modèle de compaction de **savoir** que le contenu est tronqué
/// (et donc de ne pas extrapoler ce qu'il y avait au-delà).
const TRUNCATE_SUFFIX: &str = "\n[... truncated for summarization ...]";

/// Configuration de la compaction.
#[derive(Debug, Clone)]
pub struct CompactionConfig {
    /// Température du modèle pendant la compaction. On veut un résumé
    /// **factuel**, pas créatif → faible. 0.0–0.2.
    pub temperature: f32,
    /// Limite de tokens de génération. Le résumé doit tenir en ~600-1500
    /// tokens : assez pour structurer plusieurs sections, pas tant que ça
    /// devienne un mini-roman.
    pub max_summary_tokens: u32,
    /// Borne sur la taille d'un `tool_result` dans le condensé **persist** M1.
    pub tool_result_truncate_chars: usize,
    /// Borne plus agressive pour le condensé du tour `summarize_run` (live).
    pub summarize_tool_result_truncate_chars: usize,
}

impl Default for CompactionConfig {
    fn default() -> Self {
        Self {
            temperature: 0.1,
            max_summary_tokens: 1200,
            tool_result_truncate_chars: DEFAULT_TOOL_RESULT_TRUNCATE,
            summarize_tool_result_truncate_chars: DEFAULT_SUMMARIZE_TOOL_RESULT_TRUNCATE,
        }
    }
}

/// Résultat d'une compaction réussie.
#[derive(Debug, Clone)]
pub struct CompactionResult {
    /// **Objectif court** extrait du résumé (1 ligne, utile pour le
    /// front-matter et pour le listing). `String::new()` si le modèle n'a
    /// pas fourni la section attendue.
    pub objective: String,
    /// Fichiers explicitement nommés par le modèle dans la section
    /// `## Files touched` (ou `## Fichiers touchés`). Vide si absent.
    pub files_touched: Vec<String>,
    /// Body markdown brut produit par le modèle. C'est ce qui est écrit
    /// dans le `.md` persistant.
    pub summary: String,
    /// Comptage tokens du tour LLM de compaction (utile pour télémetrie).
    pub usage: Option<Usage>,
}

/// Appelle le LLM pour produire un résumé persistable du run.
///
/// `messages` est l'historique complet à résumer — en pratique : le system
/// prompt initial + le message user d'origine + les tours assistant/tool
/// du run. Les notes `session_note` sont concaténées séparément dans le
/// payload utilisateur de la compaction (zone « Pinned notes »).
///
/// `compaction_system_prompt` est le prompt qui décrit au modèle le format
/// attendu (cf. `drox-cli/prompts::COMPACTION_PROMPT`). Volontairement
/// passé en paramètre plutôt qu'embarqué dans `drox-engine` pour ne pas
/// dupliquer la convention de prompts entre crates.
/// Résume pour persistance M1 (condensé standard).
pub async fn summarize_run(
    llm: &dyn LlmClient,
    compaction_system_prompt: &str,
    messages: &[Message],
    notes: &[SessionNote],
    config: &CompactionConfig,
) -> Result<CompactionResult, EngineError> {
    summarize_run_inner(
        llm,
        compaction_system_prompt,
        messages,
        notes,
        config,
        false,
    )
    .await
}

/// Résume pour compaction live (condensé plus agressif sur les `tool_result`).
async fn summarize_run_for_live(
    llm: &dyn LlmClient,
    compaction_system_prompt: &str,
    messages: &[Message],
    notes: &[SessionNote],
    config: &CompactionConfig,
) -> Result<CompactionResult, EngineError> {
    summarize_run_inner(
        llm,
        compaction_system_prompt,
        messages,
        notes,
        config,
        true,
    )
    .await
}

async fn summarize_run_inner(
    llm: &dyn LlmClient,
    compaction_system_prompt: &str,
    messages: &[Message],
    notes: &[SessionNote],
    config: &CompactionConfig,
    for_live_summarize: bool,
) -> Result<CompactionResult, EngineError> {
    let condensed = condense_messages_for_summary(messages, notes, config, for_live_summarize);
    let summary_messages = vec![
        Message::system(compaction_system_prompt),
        Message::user(condensed),
    ];
    let options = ChatOptions::default()
        .with_temperature(config.temperature)
        .with_max_tokens(config.max_summary_tokens);

    let mut stream = llm
        .stream_chat(summary_messages, options)
        .await
        .map_err(EngineError::from)?;
    let mut buffer = String::new();
    let mut usage: Option<Usage> = None;
    while let Some(ev) = stream.next().await {
        match ev.map_err(EngineError::from)? {
            StreamEvent::TextDelta { text } => buffer.push_str(&text),
            StreamEvent::Stop { usage: u, .. } => usage = Some(u),
            _ => {}
        }
    }
    if buffer.trim().is_empty() {
        warn!("compaction: LLM returned empty summary");
    }
    let (objective, files_touched) = extract_metadata(&buffer);
    Ok(CompactionResult {
        objective,
        files_touched,
        summary: buffer,
        usage,
    })
}

/// Messages conservés en fin d'historique (borne haute ; le split peut réduire la queue).
pub const LIVE_COMPACT_TAIL_KEEP_MESSAGES: usize = 4;

/// Fraction max de la fenêtre effective pour la queue tail (tokens).
pub const LIVE_COMPACT_MAX_TAIL_RATIO: f32 = 0.20;

/// Borne basse de jetons (dans `messages[1..split]`) avant d'appeler le LLM.
pub const LIVE_COMPACT_MIN_PREFIX_TOKENS: usize = 3_000;

/// Nombre max de passes microcompact + snip + summarize par déclenchement.
pub const LIVE_COMPACT_MAX_PASSES: u32 = 3;

/// Taille max du checkpoint injecté (caractères UTF-8).
pub const CHECKPOINT_MAX_CHARS: usize = 3_000;

const CHECKPOINT_PREAMBLE: &str = "[context checkpoint — earlier messages compressed by the engine]\n\n";

/// Rapport d'une compaction live réussie.
#[derive(Debug, Clone)]
pub struct LiveCompactReport {
    pub tokens_before: usize,
    pub tokens_after: usize,
    /// Diminution du nombre de messages (`len_avant - len_après`).
    pub messages_removed: usize,
    pub usage: Option<Usage>,
    /// Markdown produit par le tour `summarize_run` (identique au checkpoint).
    pub summary_text: String,
    pub files_touched: Vec<String>,
}

/// Choisit l'index `split` tel que `messages[split..]` soit la queue préservée
/// et `messages[..split]` le préfixe à résumer (le message `messages[0]`
/// — en pratique le system prompt — est **toujours** repris tel quel dans
/// [`try_live_compact`], le résumé porte sur `messages[1..split]`).
#[must_use]
pub fn choose_live_compact_split_idx(
    messages: &[Message],
    policy: &ContextPolicy,
    tail_keep: usize,
    min_prefix_tokens: usize,
) -> Option<usize> {
    let n = messages.len();
    if tail_keep == 0 || n <= tail_keep.saturating_add(1) {
        return None;
    }
    let max_tail_tokens = (policy.budget().effective_window() as f32 * LIVE_COMPACT_MAX_TAIL_RATIO)
        as usize;

    let mut split = n.saturating_sub(tail_keep);
    if split <= 1 {
        return None;
    }

    // Réduire la queue tant qu'elle dépasse le plafond de tokens (augmenter `split`).
    while split + 1 < n {
        let tail_tokens = policy.count_tokens(&messages[split..]);
        if tail_tokens <= max_tail_tokens {
            break;
        }
        split += 1;
    }

    let mid = messages.get(1..split)?;
    if mid.is_empty() {
        return None;
    }
    let prefix_tokens = policy.count_tokens(mid);
    if prefix_tokens < min_prefix_tokens {
        return None;
    }
    Some(split)
}

/// Construit un checkpoint court à partir du markdown de compaction (sections structurées).
#[must_use]
pub fn format_compact_checkpoint(summary_md: &str) -> String {
    let (objective, files) = extract_metadata(summary_md);
    let mut body = String::new();
    if !objective.is_empty() {
        let _ = writeln!(body, "## Objective\n{objective}");
    }
    append_section_excerpt(summary_md, "decision", "## Decisions", &mut body, 400);
    append_section_excerpt(summary_md, "en cours", "## En cours", &mut body, 300);
    if !files.is_empty() {
        let _ = writeln!(body, "## Files touched");
        for path in files.iter().take(15) {
            let _ = writeln!(body, "- {path}");
        }
    }
    if body.trim().is_empty() {
        body = truncate_chars(summary_md, 800);
    }
    let full = format!("{CHECKPOINT_PREAMBLE}{body}");
    truncate_chars(&full, CHECKPOINT_MAX_CHARS)
}

fn append_section_excerpt(
    markdown: &str,
    heading_contains: &str,
    heading_out: &str,
    out: &mut String,
    max_body_chars: usize,
) {
    let lines: Vec<&str> = markdown.lines().collect();
    let mut in_section = false;
    let mut section_body = String::new();
    for line in lines {
        let trimmed = line.trim();
        if trimmed.starts_with("## ") {
            if in_section {
                break;
            }
            if trimmed[3..].to_ascii_lowercase().contains(&heading_contains.to_ascii_lowercase()) {
                in_section = true;
                continue;
            }
        } else if in_section {
            if !trimmed.is_empty() {
                let _ = writeln!(section_body, "{trimmed}");
            }
        }
    }
    if in_section && !section_body.trim().is_empty() {
        let _ = writeln!(out, "{heading_out}");
        out.push_str(&truncate_chars(section_body.trim(), max_body_chars));
        out.push('\n');
    }
}

/// Compaction proactive : résume `messages[..split]` via [`summarize_run`],
/// puis remplace ce préfixe par `messages[0]` + un `system` checkpoint + queue.
///
/// En cas d'échec LLM, l'historique n'est **pas** modifié.
pub async fn try_live_compact(
    llm: &dyn LlmClient,
    compaction_system_prompt: &str,
    messages: &mut Vec<Message>,
    policy: &ContextPolicy,
    config: &CompactionConfig,
) -> Option<LiveCompactReport> {
    let split = choose_live_compact_split_idx(
        messages,
        policy,
        LIVE_COMPACT_TAIL_KEEP_MESSAGES,
        LIVE_COMPACT_MIN_PREFIX_TOKENS,
    )?;
    let old_len = messages.len();
    let tokens_before = policy.count_tokens(messages);
    let prefix = messages.get(..split)?.to_vec();
    let result = match summarize_run_for_live(
        llm,
        compaction_system_prompt,
        &prefix,
        &[],
        config,
    )
    .await
    {
        Ok(r) => r,
        Err(e) => {
            warn!(error = %e, "live compaction: summarize_run failed — leaving history untouched");
            return None;
        }
    };
    let tail = messages.get(split..)?.to_vec();
    let head = messages.first()?.clone();
    let checkpoint_body = format_compact_checkpoint(&result.summary);
    let checkpoint = Message::system(checkpoint_body);
    let mut new_msgs = Vec::with_capacity(2 + tail.len());
    new_msgs.push(head);
    new_msgs.push(checkpoint);
    new_msgs.extend(tail);
    *messages = new_msgs;
    let tokens_after = policy.count_tokens(messages);
    let messages_removed = old_len.saturating_sub(messages.len());
    debug!(
        tokens_before,
        tokens_after,
        messages_removed,
        "live compaction checkpoint applied"
    );
    Some(LiveCompactReport {
        tokens_before,
        tokens_after,
        messages_removed,
        usage: result.usage,
        summary_text: result.summary,
        files_touched: result.files_touched,
    })
}

/// Microcompact + snip agressif + summarize en boucle jusqu'au seuil ou max passes.
pub async fn compact_until_budget(
    llm: &dyn LlmClient,
    compaction_system_prompt: &str,
    messages: &mut Vec<Message>,
    policy: &ContextPolicy,
    config: &CompactionConfig,
) -> Option<LiveCompactReport> {
    use drox_context::{MicrocompactConfig, microcompact_messages};

    let threshold = policy.budget().autocompact_threshold();
    let mc_config = MicrocompactConfig::default();
    let mut last_report: Option<LiveCompactReport> = None;

    for _pass in 0..LIVE_COMPACT_MAX_PASSES {
        let tokens_start = policy.count_tokens(messages);
        if tokens_start < threshold {
            break;
        }

        let mc = microcompact_messages(messages, &mc_config);
        if mc.blocks_cleared > 0 {
            debug!(
                tools_cleared = mc.tools_cleared,
                blocks_cleared = mc.blocks_cleared,
                "microcompact applied before live compact"
            );
        }

        if let Some(report) = policy.maybe_snip_aggressive(messages) {
            debug!(
                tokens_freed = report.tokens_freed,
                blocks_snipped = report.blocks_snipped,
                "aggressive snip during compact loop"
            );
        }

        if policy.count_tokens(messages) < threshold {
            break;
        }

        let Some(report) = try_live_compact(
            llm,
            compaction_system_prompt,
            messages,
            policy,
            config,
        )
        .await
        else {
            break;
        };

        let reduced_enough = report.tokens_after < threshold;
        let meaningful = report.tokens_after * 100
            < tokens_start.saturating_mul(95);
        last_report = Some(report);
        if reduced_enough || !meaningful {
            break;
        }
    }

    last_report
}

/// Aplatit la conversation en un blob texte signé par rôle. Coupe les
/// `tool_result` trop gros (le modèle de compaction n'en a pas besoin du
/// dump complet, juste de l'intention).
///
/// Format produit :
///
/// ```text
/// === Conversation transcript ===
/// [user] Help me refactor the auth layer
/// [assistant] [phase: reading] …
/// [assistant tool_call] file_read { "path": "src/auth.rs" }
/// [tool_result tool_id=abc] (1234 bytes) <troncated content>
/// [assistant] [phase: planning] …
/// …
///
/// === Pinned notes from the model ===
/// - Décision sqlx > diesel : compat tokio
/// - TODO : vérifier retry 429
/// ```
fn condense_messages_for_summary(
    messages: &[Message],
    notes: &[SessionNote],
    config: &CompactionConfig,
    for_live_summarize: bool,
) -> String {
    let tool_truncate = if for_live_summarize {
        config.summarize_tool_result_truncate_chars
    } else {
        config.tool_result_truncate_chars
    };
    let mut out = String::with_capacity(messages.len() * 256);
    out.push_str("=== Conversation transcript ===\n");
    for msg in messages {
        let role_tag = match msg.role {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::Tool => "tool_result",
        };
        for block in &msg.content {
            match block {
                Content::Text { text } => {
                    if text.trim().is_empty() {
                        continue;
                    }
                    let _ = writeln!(out, "[{role_tag}] {text}");
                }
                Content::Image { mime, data } => {
                    let _ = writeln!(
                        out,
                        "[{role_tag} image] {mime} ({} bytes base64)",
                        data.len()
                    );
                }
                Content::ToolUse { name, input, .. } => {
                    let args = serde_json::to_string(input).unwrap_or_else(|_| "{}".into());
                    let args_truncated = truncate_chars(&args, 400);
                    let _ = writeln!(out, "[assistant tool_call] {name} {args_truncated}");
                }
                Content::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                } => {
                    let len_hint = content.len();
                    let body = truncate_chars(content, tool_truncate);
                    let err_tag = if *is_error { " ERROR" } else { "" };
                    let _ = writeln!(
                        out,
                        "[tool_result id={tool_use_id}{err_tag}] ({len_hint} bytes) {body}",
                    );
                }
                _ => {}
            }
        }
    }
    if !notes.is_empty() {
        out.push_str("\n=== Pinned notes from the model (session_note) ===\n");
        for n in notes {
            let _ = writeln!(out, "- {}", n.content);
        }
    }
    out
}

/// Tronque une string en caractères (pas en bytes, important pour UTF-8).
/// Ajoute un suffixe explicite si tronqué.
fn truncate_chars(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max_chars).collect();
    out.push_str(TRUNCATE_SUFFIX);
    out
}

/// Extrait l'`objective` (1 ligne) et la liste `files_touched` du markdown
/// produit par le modèle.
///
/// Conventions attendues (cf. `COMPACTION_PROMPT`) :
///
/// - `## Objective` (ou `## Objectif`) puis le contenu sur les lignes
///   suivantes — on prend la **première ligne non vide** de cette section.
/// - `## Files touched` (ou `## Fichiers touchés`) puis une liste markdown
///   `- path/un` / `- path/deux`.
///
/// Tolérant : si une section manque, on dégrade silencieusement (chaîne
/// vide / vec vide). Le body markdown brut reste persisté.
fn extract_metadata(markdown: &str) -> (String, Vec<String>) {
    let lines: Vec<&str> = markdown.lines().collect();
    let mut objective = String::new();
    let mut files: Vec<String> = Vec::new();
    let mut state = SectionState::Other;
    for line in lines {
        let trimmed = line.trim();
        if let Some(h) = trimmed.strip_prefix("## ").or_else(|| trimmed.strip_prefix("# ")) {
            let head = h.trim().to_ascii_lowercase();
            state = match head.as_str() {
                "objective" | "objectif" | "goal" => SectionState::Objective,
                "files touched"
                | "files_touched"
                | "fichiers touchés"
                | "fichiers touches"
                | "fichiers" => SectionState::Files,
                _ => SectionState::Other,
            };
            continue;
        }
        match state {
            SectionState::Objective => {
                if objective.is_empty() && !trimmed.is_empty() {
                    objective = trimmed.trim_start_matches("- ").trim().to_string();
                }
            }
            SectionState::Files => {
                if let Some(item) = trimmed.strip_prefix("- ") {
                    let path = item.trim().trim_matches(|c: char| c == '`' || c == '"');
                    if !path.is_empty() {
                        files.push(path.to_string());
                    }
                }
            }
            SectionState::Other => {}
        }
    }
    debug!(
        objective_len = objective.len(),
        files_count = files.len(),
        "compaction: metadata extracted from summary"
    );
    (objective, files)
}

#[derive(Clone, Copy)]
enum SectionState {
    Objective,
    Files,
    Other,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use drox_types::ToolUseId;
    use std::sync::Arc;

    use crate::context::ContextPolicy;
    use drox_context::{ContextBudget, RoughTokenCounter, SnipConfig};

    fn note(content: &str) -> SessionNote {
        SessionNote {
            content: content.into(),
            created_at: Utc::now(),
        }
    }

    fn user(text: &str) -> Message {
        Message::user(text)
    }

    fn assistant(text: &str) -> Message {
        Message::assistant(text)
    }

    fn tool_result(text: &str, is_error: bool) -> Message {
        Message {
            role: Role::Tool,
            content: vec![Content::ToolResult {
                tool_use_id: ToolUseId::new(),
                content: text.to_string(),
                is_error,
            }],
        }
    }

    #[test]
    fn condense_renders_roles_and_truncates_big_tool_results() {
        let big = "x".repeat(5_000);
        let msgs = vec![
            user("Refactor auth"),
            assistant("[phase: reading]"),
            tool_result(&big, false),
        ];
        let cfg = CompactionConfig::default();
        let out = condense_messages_for_summary(&msgs, &[], &cfg, false);
        assert!(out.contains("[user] Refactor auth"));
        assert!(out.contains("[assistant] [phase: reading]"));
        assert!(out.contains(TRUNCATE_SUFFIX), "big tool_result must be truncated");
        assert!(
            out.len() < big.len(),
            "condensed must be smaller than raw tool_result"
        );
    }

    #[test]
    fn condense_includes_pinned_notes_when_present() {
        let msgs = vec![user("hi")];
        let notes = vec![note("Decision X"), note("TODO Y")];
        let out = condense_messages_for_summary(&msgs, &notes, &CompactionConfig::default(), false);
        assert!(out.contains("=== Pinned notes from the model"));
        assert!(out.contains("- Decision X"));
        assert!(out.contains("- TODO Y"));
    }

    #[test]
    fn condense_omits_notes_section_when_empty() {
        let msgs = vec![user("hi")];
        let out = condense_messages_for_summary(&msgs, &[], &CompactionConfig::default(), false);
        assert!(!out.contains("Pinned notes"));
    }

    #[test]
    fn extract_metadata_picks_first_objective_line_and_file_list() {
        let md = "\
## Objective
Refactor authentication layer to use sqlx
## Decisions
- chose sqlx
## Files touched
- src/auth.rs
- `Cargo.toml`
- \"src/db/pool.rs\"
## En cours
nothing
";
        let (obj, files) = extract_metadata(md);
        assert_eq!(obj, "Refactor authentication layer to use sqlx");
        assert_eq!(
            files,
            vec![
                "src/auth.rs".to_string(),
                "Cargo.toml".to_string(),
                "src/db/pool.rs".to_string(),
            ]
        );
    }

    #[test]
    fn extract_metadata_handles_french_section_titles() {
        let md = "\
## Objectif
Refactorer la couche auth
## Fichiers touchés
- src/auth.rs
";
        let (obj, files) = extract_metadata(md);
        assert_eq!(obj, "Refactorer la couche auth");
        assert_eq!(files, vec!["src/auth.rs".to_string()]);
    }

    #[test]
    fn extract_metadata_returns_empty_when_sections_missing() {
        let md = "Random freeform text without any section header.";
        let (obj, files) = extract_metadata(md);
        assert!(obj.is_empty());
        assert!(files.is_empty());
    }

    #[test]
    fn truncate_chars_preserves_short_strings() {
        assert_eq!(truncate_chars("hello", 10), "hello");
    }

    #[test]
    fn truncate_chars_safely_cuts_unicode() {
        let s = "héllo wörld!"; // multibytes
        let t = truncate_chars(s, 5);
        assert!(t.starts_with("héllo"));
        assert!(t.contains(TRUNCATE_SUFFIX));
    }

    #[test]
    fn choose_live_split_none_when_too_few_messages() {
        let policy = ContextPolicy::default();
        let msgs = vec![Message::system("s"), Message::user("u")];
        assert!(choose_live_compact_split_idx(
            &msgs,
            &policy,
            LIVE_COMPACT_TAIL_KEEP_MESSAGES,
            LIVE_COMPACT_MIN_PREFIX_TOKENS
        )
        .is_none());
    }

    #[test]
    fn format_compact_checkpoint_caps_length() {
        let verbose = format!(
            "## Objective\nShort goal\n## Decisions\n{}\n## Files touched\n- a.rs\n",
            "x".repeat(8_000)
        );
        let cp = format_compact_checkpoint(&verbose);
        assert!(cp.contains("Short goal"));
        assert!(cp.len() <= CHECKPOINT_MAX_CHARS + 4);
        assert!(!cp.contains(&"x".repeat(1000)));
    }

    #[test]
    fn choose_live_split_shrinks_fat_tail() {
        let policy = ContextPolicy::new(
            Arc::new(RoughTokenCounter::new(4)),
            ContextBudget::with_window(20_000),
            None,
        );
        let mut msgs = vec![Message::system("sys")];
        for _ in 0..6 {
            msgs.push(Message::user("u".repeat(500)));
        }
        for _ in 0..4 {
            msgs.push(Message::user("z".repeat(12_000)));
        }
        let split = choose_live_compact_split_idx(
            &msgs,
            &policy,
            LIVE_COMPACT_TAIL_KEEP_MESSAGES,
            1_000,
        )
        .expect("split");
        let tail_msgs = msgs.len() - split;
        assert!(
            tail_msgs < LIVE_COMPACT_TAIL_KEEP_MESSAGES,
            "fat tail should shrink below message cap, tail_msgs={tail_msgs}"
        );
    }

    #[test]
    fn choose_live_split_honours_tail_keep_and_min_prefix_tokens() {
        let policy = ContextPolicy::new(
            Arc::new(RoughTokenCounter::new(4)),
            ContextBudget::default(),
            Some(SnipConfig::default()),
        );
        let mut msgs = vec![Message::system("sys")];
        for _ in 0..14 {
            msgs.push(Message::user("z".repeat(10_000)));
        }
        let split = choose_live_compact_split_idx(
            &msgs,
            &policy,
            LIVE_COMPACT_TAIL_KEEP_MESSAGES,
            LIVE_COMPACT_MIN_PREFIX_TOKENS,
        )
        .expect("prefix must be fat enough");
        assert_eq!(split, msgs.len() - LIVE_COMPACT_TAIL_KEEP_MESSAGES);
    }
}
