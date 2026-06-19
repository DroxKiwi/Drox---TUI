//! Handlers des méthodes du protocole drox JSON-RPC v1.
//!
//! Les handlers ne touchent pas directement à `stdout` : ils renvoient un
//! `Value` ou un [`RpcError`], le [`super::server::Server`] s'occupe de
//! sérialiser la `Response` et de pousser les notifications.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use camino::{Utf8Path, Utf8PathBuf};
use drox_engine::{
    Agent, AgentConfig, AgentEvent, CompactionConfig, ContextPolicy, JsonlTranscriptSink,
    LayeredConfig, MemoryRuntime, PermissionEngine, PermissionMode, PermissionPolicy, SessionError,
    SessionNotesHandle, TranscriptSessionConfig, WorkspaceMapStore, default_sessions_dir,
    default_tool_registry, EngineSubagentExecutor, SubagentSettings,
    format_sessions_listing_for_prompt, list_sessions, load_memdir, load_sessions_listing,
    memdir_system_prefix, read_session_ui_stats, read_transcript, session_ui_stats_path,
    summarize_run, transcript_path, write_session_ui_stats, DroxIgnoreMatcher,
};
use drox_llm::{ChatOptions, LlmConfig, OllamaClient};
use drox_mcp::McpHub;
use drox_permissions::{
    DetectUnreachableOptions, PathMatchContext, PermissionBehavior, Rule, RuleSet, RuleSource,
    detect_unreachable_rules, format_rule, parse_rule,
};
use drox_tools::{
    ScopeDeferredHandle, ToolContext, ToolError, ToolRegistry, UserAnswer, UserAsker, UserQuestion,
    register_mcp_tools,
};
use drox_types::{Content, SessionId};
use futures::StreamExt;
use serde_json::{Value, json};
use tracing::warn;

use super::protocol::{
    AgentCancelParams, AgentCancelResult, AgentDoneNotification, AgentEventNotification,
    AgentRunParams, AgentRunResult, InitializeParams, InitializeResult, RunStatus,
    SessionCompactParams, SessionCompactResult, SessionCompactUsageDto, SessionListEntryDto,
    SessionListParams, SessionReadParams, SessionReadResult, UserAskParams, UserAskQuestion,
    UserAskOption, UserAskResult,
};
use super::remote_tool::RemoteTool;
use super::server::Server;
use super::{CONFIG_ERROR, ENGINE_ERROR, INVALID_PARAMS, RUN_NOT_FOUND, RpcError};
use uuid::Uuid;

/// Statut de fin d'une tâche `agent.run`. Utilisé pour signaler le succès
/// même lorsque le `JoinHandle` est consommé par `cancel_run`.
#[derive(Debug, Clone, Copy)]
pub enum RunOutcome {
    Completed,
    Errored,
}

// Garde la signature `async` pour rester homogène avec les autres handlers
// dispatchés dans `server::handle_request`.
#[allow(clippy::unused_async)]
pub async fn initialize(server: &Server, params: Option<Value>) -> Result<Value, RpcError> {
    let parsed: InitializeParams = decode_optional(params)?;
    if let Some(caps) = parsed.client_capabilities {
        if !caps.executable_tools.is_empty() {
            tracing::info!(
                tools = ?caps.executable_tools,
                "client declared executable tools (delegated via tool/exec)"
            );
        }
        server.set_executable_tools(caps.executable_tools);
        if caps.interactive_ask {
            tracing::info!("client declared `interactiveAsk` — RpcUserAsker will be installed");
        }
        server.set_interactive_ask(caps.interactive_ask);
    }
    let r = InitializeResult::current();
    serde_json::to_value(r).map_err(internal)
}

pub async fn session_list(params: Option<Value>) -> Result<Value, RpcError> {
    let params: SessionListParams = decode_optional(params)?;
    let dir = resolve_session_dir(params.dir)?;
    let entries = list_sessions(&dir)
        .await
        .map_err(|e| RpcError::new(ENGINE_ERROR, format!("session.list failed: {e}")))?;
    let dtos: Vec<SessionListEntryDto> = entries
        .into_iter()
        .map(|e| SessionListEntryDto {
            id: e.id.to_string(),
            modified_secs: e.modified_secs,
            size_bytes: e.size_bytes,
        })
        .collect();
    serde_json::to_value(dtos).map_err(internal)
}

pub async fn session_read(params: Option<Value>) -> Result<Value, RpcError> {
    let params: SessionReadParams = decode_required(params, "session.read requires { id, dir? }")?;
    if !params.id.starts_with("ses_") {
        return Err(RpcError::new(INVALID_PARAMS, "id must start with `ses_`"));
    }
    let dir = resolve_session_dir(params.dir)?;
    let id = SessionId::from_string(params.id);
    let path = transcript_path(&dir, &id);
    let messages = match read_transcript(&path).await {
        Ok(m) => m,
        Err(SessionError::NotFound(_)) => Vec::new(),
        Err(e) => {
            return Err(RpcError::new(
                ENGINE_ERROR,
                format!("session.read failed: {e}"),
            ));
        }
    };
    let stats_path = session_ui_stats_path(&dir, &id);
    let ui_stats = read_session_ui_stats(&stats_path).await;
    serde_json::to_value(SessionReadResult { messages, ui_stats }).map_err(internal)
}

pub async fn session_compact(params: Option<Value>) -> Result<Value, RpcError> {
    let p: SessionCompactParams = decode_required(
        params,
        "session.compact requires { id, dir?, server?, model?, apiKey?, headers? }",
    )?;
    if !p.id.starts_with("ses_") {
        return Err(RpcError::new(INVALID_PARAMS, "id must start with `ses_`"));
    }
    let dir = resolve_session_dir(p.dir)?;
    let id = SessionId::from_string(p.id);
    let path = transcript_path(&dir, &id);
    let messages = match read_transcript(&path).await {
        Ok(m) => m,
        Err(SessionError::NotFound(_)) => Vec::new(),
        Err(e) => {
            return Err(RpcError::new(
                ENGINE_ERROR,
                format!("session.compact read_transcript failed: {e}"),
            ));
        }
    };
    if messages.is_empty() {
        return Err(RpcError::new(
            INVALID_PARAMS,
            "session transcript is empty — send at least one message in this session before /compact",
        ));
    }

    let llm = build_ollama_from_llm_connect_fields(
        p.server,
        p.model,
        p.api_key,
        &p.headers,
    )?;

    let result = summarize_run(
        llm.as_ref(),
        crate::prompts::COMPACTION_PROMPT,
        &messages,
        &[],
        &CompactionConfig::default(),
    )
    .await
    .map_err(|e| RpcError::new(ENGINE_ERROR, e.to_string()))?;

    let usage = result.usage.map(|u| SessionCompactUsageDto {
        input_tokens: u.input_tokens,
        output_tokens: u.output_tokens,
    });
    let objective = (!result.objective.trim().is_empty()).then_some(result.objective);

    let dto = SessionCompactResult {
        summary: result.summary,
        objective,
        files_touched: result.files_touched,
        usage,
    };
    serde_json::to_value(dto).map_err(internal)
}

pub async fn agent_run(server: Server, params: Option<Value>) -> Result<Value, RpcError> {
    let params: AgentRunParams = decode_required(params, "agent.run requires `prompt`")?;
    if params.prompt.is_empty() {
        return Err(RpcError::new(INVALID_PARAMS, "prompt must not be empty"));
    }

    // Le run_id est alloué **avant** la construction de l'agent pour que les
    // wrappers `RemoteTool` puissent l'embarquer.
    let run_id = server.allocate_run_id();
    let agent_setup = build_agent_setup(&server, &run_id, &params).await?;

    let server_for_task = server.clone();
    let run_id_for_task = run_id.clone();
    let user_blocks = build_user_blocks(&params);
    let image_count = params.images.len();
    if image_count > 0 {
        tracing::info!(
            run_id = %run_id,
            images = image_count,
            "agent.run with image attachments"
        );
    }

    let join = tokio::spawn(async move {
        let outcome = drive_run(
            &server_for_task,
            run_id_for_task.clone(),
            agent_setup,
            user_blocks,
        )
        .await
        .unwrap_or_else(|status| status);
        server_for_task.forget_run(&run_id_for_task);
        outcome
    });

    server.register_run(run_id.clone(), join);
    serde_json::to_value(AgentRunResult { run_id }).map_err(internal)
}

pub fn agent_cancel(server: &Server, params: Option<Value>) -> Result<Value, RpcError> {
    let params: AgentCancelParams = decode_required(params, "agent.cancel requires `runId`")?;
    let cancelled = server.cancel_run(&params.run_id);
    if !cancelled {
        return Err(RpcError::new(
            RUN_NOT_FOUND,
            format!("no active run with id `{}`", params.run_id),
        ));
    }
    serde_json::to_value(AgentCancelResult { cancelled: true }).map_err(internal)
}

// --------------------------------------------------------------------------
// Internal helpers
// --------------------------------------------------------------------------

/// Tout ce dont la tâche `drive_run` a besoin pour piloter un agent. Construit
/// **avant** le spawn afin que les erreurs de configuration soient renvoyées
/// directement dans la réponse à `agent.run`.
struct AgentSetup {
    agent: Agent,
    history: Vec<drox_types::Message>,
    /// Persistance des compteurs webview (↑ / ↓ / ctx) pour reprise de session.
    ui_stats_path: Option<Utf8PathBuf>,
}

fn build_llm_config(
    server: Option<String>,
    model: Option<String>,
    api_key: Option<String>,
    headers: &BTreeMap<String, String>,
) -> Result<LlmConfig, RpcError> {
    let server_url = server.unwrap_or_else(|| {
        std::env::var("DROX_SERVER").unwrap_or_else(|_| "http://localhost:11434".into())
    });
    let model = model.unwrap_or_else(|| {
        std::env::var("DROX_MODEL").unwrap_or_else(|_| "llama3.2".into())
    });

    let mut llm_config = LlmConfig::try_from_str(&server_url, &model)
        .map_err(|e| RpcError::new(CONFIG_ERROR, format!("invalid LLM config: {e}")))?;
    if let Some(k) = api_key.or_else(|| std::env::var("DROX_API_KEY").ok()) {
        llm_config = llm_config.with_api_key(k);
    }
    for (name, value) in headers {
        llm_config = llm_config.with_header(name, value);
    }
    if let Some(n) = env_i64("DROX_NUM_PREDICT") {
        llm_config = llm_config.with_num_predict(n);
    }
    if let Some(n) = env_i64("DROX_NUM_CTX") {
        llm_config = llm_config.with_num_ctx(n);
    }
    if let Some(v) = env_f32("DROX_TOP_P") {
        llm_config = llm_config.with_top_p(Some(v));
    }
    if let Some(n) = env_i64("DROX_TOP_K") {
        llm_config = llm_config.with_top_k(Some(n));
    }
    if let Some(v) = env_f32("DROX_REPEAT_PENALTY") {
        llm_config = llm_config.with_repeat_penalty(Some(v));
    }
    if let Some(n) = env_i64("DROX_SEED") {
        llm_config = llm_config.with_seed(Some(n));
    }
    if let Some(v) = env_f32("DROX_MIN_P") {
        llm_config = llm_config.with_min_p(Some(v));
    }
    if let Some(v) = env_f32("DROX_PRESENCE_PENALTY") {
        llm_config = llm_config.with_presence_penalty(Some(v));
    }
    if let Some(v) = env_f32("DROX_FREQUENCY_PENALTY") {
        llm_config = llm_config.with_frequency_penalty(Some(v));
    }
    if let Ok(s) = std::env::var("DROX_KEEP_ALIVE") {
        let trimmed = s.trim();
        if !trimmed.is_empty() {
            llm_config = llm_config.with_keep_alive(Some(trimmed.to_owned()));
        }
    }
    Ok(llm_config)
}

fn build_ollama_from_llm_connect_fields(
    server: Option<String>,
    model: Option<String>,
    api_key: Option<String>,
    headers: &BTreeMap<String, String>,
) -> Result<Arc<OllamaClient>, RpcError> {
    let llm_config = build_llm_config(server, model, api_key, headers)?;
    Ok(Arc::new(
        OllamaClient::new(llm_config)
            .map_err(|e| RpcError::new(CONFIG_ERROR, format!("LLM init failed: {e}")))?,
    ))
}

#[allow(clippy::too_many_lines)] // plomberie linéaire : workspace + memory + permissions + transcript + registry
async fn build_agent_setup(
    server: &Server,
    run_id: &str,
    params: &AgentRunParams,
) -> Result<AgentSetup, RpcError> {
    let llm_config = build_llm_config(
        params.server.clone(),
        params.model.clone(),
        params.api_key.clone(),
        &params.headers,
    )?;
    let num_ctx = llm_config.num_ctx.max(2048) as usize;
    let llm = Arc::new(
        OllamaClient::new(llm_config)
            .map_err(|e| RpcError::new(CONFIG_ERROR, format!("LLM init failed: {e}")))?,
    );

    let workspace = resolve_workspace(params.workspace.clone())?;
    let workspace_fingerprint = workspace.as_str().to_string();

    let mem = load_memdir(workspace.as_path())
        .await
        .map_err(|e| RpcError::new(ENGINE_ERROR, format!("memdir: {e}")))?;
    let base_system = merge_optional_system(params.system.clone(), memdir_system_prefix(&mem));
    let mut base_system = crate::prompts::prepend_core_system_prompt(base_system);
    // Sprint M1 — listing court des sessions archivées injecté en début de
    // system prompt (compact, ≤ 10 entrées par défaut). Échec silencieux :
    // si le dossier est inaccessible ou que les fichiers sont mal formés,
    // on ne casse pas le run, on log juste.
    let memory_listing = load_sessions_listing(
        workspace.as_path(),
        drox_engine::DEFAULT_LISTING_LIMIT,
    )
    .await
    .unwrap_or_else(|e| {
        warn!(error = %e, "memory: failed to load sessions listing — ignoring");
        Vec::new()
    });
    if let Some(block) = format_sessions_listing_for_prompt(&memory_listing) {
        base_system.push_str("\n\n");
        base_system.push_str(&block);
    }
    let skills_listing = drox_engine::load_skills_catalog(workspace.as_path())
        .await
        .unwrap_or_else(|e| {
            warn!(error = %e, "skills: failed to load catalog — ignoring");
            Vec::new()
        });
    if let Some(block) = drox_engine::format_skills_listing_for_prompt(&skills_listing) {
        base_system.push_str("\n\n");
        base_system.push_str(&block);
    }
    let drox_ignore = DroxIgnoreMatcher::load_or_create(workspace.clone())
        .await
        .map_err(|e| RpcError::new(ENGINE_ERROR, format!("droxignore: {e}")))?;
    base_system.push_str("\n\n");
    base_system.push_str(&drox_ignore.format_for_prompt());
    let workspace_map = WorkspaceMapStore::load_or_create(
        workspace.clone(),
        workspace_fingerprint.clone(),
        Some(drox_ignore.clone()),
    )
        .await
        .map_err(|e| RpcError::new(ENGINE_ERROR, format!("workspace_map: {e}")))?;
    if let Some(block) = workspace_map.format_for_prompt() {
        base_system.push_str("\n\n");
        base_system.push_str(&block);
    }
    let base_system = Some(base_system);
    let lang = crate::language::from_env();
    if let Some(ref l) = lang {
        tracing::debug!(language = %l.display, "langue principale appliquée (jsonrpc)");
    }
    let mut system_merged = crate::language::merge_into_system(base_system, lang.as_ref());
    if params.native_thinking == Some(true) {
        match &mut system_merged {
            Some(s) => {
                s.push_str("\n\n");
                s.push_str(crate::prompts::NATIVE_THINKING_REASONING_SUPPLEMENT);
            }
            None => {
                system_merged =
                    Some(crate::prompts::NATIVE_THINKING_REASONING_SUPPLEMENT.to_string());
            }
        }
    }

    let (mode, policy) = build_permission_policy(params, &workspace)?;

    // Mode Professeur : propositions seules côté client (`applyFsWrites: false`).
    let apply = if mode.is_professor() {
        false
    } else {
        params.apply_edits.unwrap_or(false)
    };

    if mode.is_professor() {
        match &mut system_merged {
            Some(s) => {
                s.push_str("\n\n");
                s.push_str(crate::prompts::PROFESSOR_MODE_SUPPLEMENT);
            }
            None => {
                system_merged = Some(crate::prompts::PROFESSOR_MODE_SUPPLEMENT.to_string());
            }
        }
    }

    if let Some(notice) = format_disabled_tools_notice(&params.disabled_tools) {
        match &mut system_merged {
            Some(s) => s.push_str(&notice),
            None => system_merged = Some(notice.trim_start().to_string()),
        }
    }

    let (history, transcript, ui_stats_path) =
        build_transcript(params, system_merged.as_deref()).await?;

    let model_label = params
        .model
        .clone()
        .unwrap_or_else(|| std::env::var("DROX_MODEL").unwrap_or_else(|_| "llama3.2".into()));

    let mcp_hub = McpHub::discover(&workspace).await;
    let mut registry = default_tool_registry();
    let mcp_enabled = params.mcp_tools_enabled.unwrap_or(true);
    if mcp_enabled {
        if let Some(ref hub) = mcp_hub {
            let dynamic = register_mcp_tools(&mut registry, hub).await;
            tracing::info!(
                dynamic_tools = dynamic,
                servers = ?hub.server_names(),
                path = %hub.config_path().display(),
                "MCP tools enregistrés"
            );
        }
    } else {
        tracing::info!("outils MCP désactivés pour ce run (paramètres workspace)");
    }
    apply_disabled_tools(&mut registry, &params.disabled_tools);
    if !params.disabled_tools.is_empty() {
        tracing::info!(
            disabled = ?params.disabled_tools,
            remaining = registry.names().len(),
            "registre tools après filtre workspace"
        );
    }

    let subagent_settings = SubagentSettings {
        enabled: params.subagents_enabled.unwrap_or(false),
        max_iterations: params.subagents_max_iterations.unwrap_or(15).clamp(1, 50),
        max_concurrent: params.subagents_max_concurrent.unwrap_or(1).clamp(1, 8),
    };
    if subagent_settings.enabled {
        registry.register_subagent_task();
        tracing::info!(
            max_iter = subagent_settings.max_iterations,
            max_concurrent = subagent_settings.max_concurrent,
            "sous-agents activés — tool `task` enregistré"
        );
    }

    let registry = Arc::new(wrap_executable_tools(registry, server, run_id));
    // Sprint Questions bloquantes (§2.13) — si le client a annoncé
    // `interactiveAsk` à `initialize`, on installe un asker qui délègue à
    // `user/ask` (carte modale dans la webview). Sinon, l'asker refusant
    // historique reste actif et le tool retournera `ToolError::Interactive`.
    let asker: Arc<dyn UserAsker> = if server.supports_interactive_ask() {
        Arc::new(RpcUserAsker {
            server: server.clone(),
            run_id: run_id.to_string(),
        })
    } else {
        Arc::new(RefuseAsker)
    };
    let scope_deferred = ScopeDeferredHandle::new();
    let mut ctx = ToolContext::new(workspace.clone(), apply)
        .with_plan_mode(mode == PermissionMode::Plan)
        .with_user_asker(asker)
        .with_scope_deferred(scope_deferred)
        .with_workspace_map(workspace_map.clone())
        .with_drox_ignore(drox_ignore);
    if let Some(hub) = mcp_hub {
        tracing::info!(
            servers = ?hub.server_names(),
            path = %hub.config_path().display(),
            "MCP hub chargé"
        );
        ctx = ctx.with_mcp_hub(hub);
    }

    let chat_options = ChatOptions {
        temperature: params.temperature,
        max_tokens: params.max_tokens,
        stop_sequences: Vec::new(),
        tools: Vec::new(),
        think: params.native_thinking,
        ..ChatOptions::default()
    };

    if subagent_settings.enabled {
        let executor = Arc::new(EngineSubagentExecutor::new(
            llm.clone(),
            chat_options.clone(),
            subagent_settings.clone(),
        ));
        ctx = ctx
            .with_subagent_settings(subagent_settings)
            .with_subagent_executor(executor);
    }

    let tool_hooks = drox_hooks::load_merged(&workspace);

    // Sprint M1 — mémoire de session. `MemoryRuntime` partage son
    // `SessionNotesHandle` avec le `ToolContext` (via le moteur, à l'entrée
    // de `drive_inner`). Le même client LLM est réutilisé pour la
    // compaction — c'est intentionnel pour V1 : un sprint suivant pourra
    // permettre un modèle dédié (plus petit, plus rapide) si besoin.
    let memory_runtime = MemoryRuntime {
        workspace_root: workspace,
        llm: llm.clone(),
        compaction_prompt: crate::prompts::COMPACTION_PROMPT.to_string(),
        compaction_config: CompactionConfig::default(),
        notes: SessionNotesHandle::new(),
        model_label: model_label.clone(),
    };
    if tool_hooks.is_enabled() {
        tracing::info!(
            pre = tool_hooks.pre_tool_use.len(),
            post = tool_hooks.post_tool_use.len(),
            "tool hooks actifs (.drox/hooks.json)"
        );
    }

    let agent_config = AgentConfig {
        system_prompt: system_merged,
        max_iterations: params.max_iterations.unwrap_or(12),
        chat_options,
        permissions: Some(policy),
        context: Some(ContextPolicy::for_model_context_window(num_ctx)),
        transcript,
        memory: Some(memory_runtime),
        transcript_session_id: params.session_id.clone(),
        workspace_fingerprint,
        max_parallel_tool_calls: drox_engine::DEFAULT_MAX_PARALLEL_TOOL_CALLS,
        tool_hooks: if tool_hooks.is_enabled() {
            Some(tool_hooks)
        } else {
            None
        },
        run_objective: params.run_objective.clone(),
    };

    let agent = Agent::new(llm, registry, ctx, agent_config);
    Ok(AgentSetup {
        agent,
        history,
        ui_stats_path,
    })
}

async fn drive_run(
    server: &Server,
    run_id: String,
    setup: AgentSetup,
    user_blocks: Vec<Content>,
) -> Result<RunOutcome, RunOutcome> {
    let AgentSetup {
        agent,
        history,
        ui_stats_path,
    } = setup;
    let mut stream = agent.run_with_history_blocks(history, user_blocks);
    let mut errored = false;

    while let Some(event) = stream.next().await {
        match event {
            Ok(ev) => {
                if let Some(ref p) = ui_stats_path {
                    if let Err(e) = merge_ui_stats_from_agent_event(p, &ev).await {
                        warn!(error = %e, path = %p, "session ui-stats persist failed");
                    }
                }
                let is_stop = matches!(&ev, AgentEvent::Stop { .. });
                server
                    .notify(
                        "agent/event",
                        AgentEventNotification {
                            run_id: run_id.clone(),
                            event: ev,
                        },
                    )
                    .await;
                if is_stop {
                    break;
                }
            }
            Err(e) => {
                errored = true;
                server
                    .notify(
                        "agent/done",
                        AgentDoneNotification {
                            run_id: run_id.clone(),
                            status: RunStatus::Error,
                            error: Some(e.to_string()),
                        },
                    )
                    .await;
                break;
            }
        }
    }

    if !errored {
        server
            .notify(
                "agent/done",
                AgentDoneNotification {
                    run_id,
                    status: RunStatus::Completed,
                    error: None,
                },
            )
            .await;
    }

    if errored {
        Err(RunOutcome::Errored)
    } else {
        Ok(RunOutcome::Completed)
    }
}

/// Lit une variable d'environnement et tente de la parser en `i64`. Renvoie
/// `None` si la variable est absente, vide ou non parseable. Utilisée pour
/// `DROX_NUM_PREDICT` / `DROX_NUM_CTX` qui contournent les defaults
/// `LlmConfig`.
fn env_i64(key: &str) -> Option<i64> {
    std::env::var(key).ok().and_then(|v| {
        let trimmed = v.trim();
        if trimmed.is_empty() {
            None
        } else {
            trimmed.parse::<i64>().ok()
        }
    })
}

/// Pendant `f32` de [`env_i64`]. Utilisée pour `DROX_TOP_P` /
/// `DROX_REPEAT_PENALTY` qui sont des floats côté Ollama.
fn env_f32(key: &str) -> Option<f32> {
    std::env::var(key).ok().and_then(|v| {
        let trimmed = v.trim();
        if trimmed.is_empty() {
            None
        } else {
            trimmed.parse::<f32>().ok()
        }
    })
}

/// Construit les blocs `Content` du message utilisateur pour le nouveau tour.
/// Le prompt textuel est toujours présent en tête, suivi des images
/// éventuelles. L'ordre est délibéré : la plupart des modèles vision (Ollama
/// vision, Gemma3-Vision, Qwen-VL) ancrent le texte avant les images.
fn build_user_blocks(params: &AgentRunParams) -> Vec<Content> {
    let mut blocks: Vec<Content> = Vec::with_capacity(1 + params.images.len());
    blocks.push(Content::text(params.prompt.clone()));
    for img in &params.images {
        blocks.push(Content::image(img.mime.clone(), img.data.clone()));
    }
    blocks
}

/// Retire du registre les outils désactivés dans les paramètres workspace (§2.17).
fn apply_disabled_tools(registry: &mut ToolRegistry, disabled: &[String]) {
    const ALWAYS_ACTIVE: &[&str] = &["ask_user_question", "todo_write"];
    for name in disabled {
        let trimmed = name.trim();
        if trimmed.is_empty() || ALWAYS_ACTIVE.contains(&trimmed) {
            continue;
        }
        if registry.remove(trimmed) {
            tracing::debug!(tool = %trimmed, "outil retiré du registre (désactivé)");
        }
    }
}

/// Bloc prompt indiquant au modèle quels outils ne sont pas disponibles.
fn format_disabled_tools_notice(disabled: &[String]) -> Option<String> {
    const ALWAYS_ACTIVE: &[&str] = &["ask_user_question", "todo_write"];
    let names: Vec<&str> = disabled
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty() && !ALWAYS_ACTIVE.contains(s))
        .collect();
    if names.is_empty() {
        return None;
    }
    Some(format!(
        "\n\n## Outils indisponibles dans ce workspace\n\
         L'utilisateur a **désactivé** les outils suivants — ne tente pas de les appeler : {}.",
        names.join(", ")
    ))
}

/// Construit la registry de tools pour un run donné : on part de la registry
/// par défaut, puis pour chaque nom déclaré par le client dans
/// `executableTools` on remplace l'implémentation locale par un
/// [`RemoteTool`] qui ré-émet l'appel via `tool/exec`. Si le nom n'existe pas
/// dans la registry locale, il est ignoré (rien à wrapper).
fn wrap_executable_tools(mut registry: ToolRegistry, server: &Server, run_id: &str) -> ToolRegistry {
    let remote_names = server.executable_tools();
    if remote_names.is_empty() {
        return registry;
    }
    for name in remote_names {
        let Some(inner) = registry.get(&name) else {
            tracing::warn!(
                %name,
                "client declared executable tool but no local counterpart exists; ignored"
            );
            continue;
        };
        let wrapper = RemoteTool::wrap(server.clone(), &inner, run_id.to_string());
        registry.register(Arc::new(wrapper));
    }
    registry
}

fn build_permission_policy(
    params: &AgentRunParams,
    workspace: &Utf8PathBuf,
) -> Result<(PermissionMode, PermissionPolicy), RpcError> {
    let no_settings = params.no_settings.unwrap_or(false);
    let (user_path, project_path, local_path): (Option<PathBuf>, Option<PathBuf>, Option<PathBuf>) =
        if no_settings {
            (None, None, None)
        } else {
            let user = dirs::home_dir().map(|h| h.join(".drox").join("settings.json"));
            let drox_dir = workspace.as_std_path().join(".drox");
            let project = Some(drox_dir.join("settings.json"));
            let local = Some(drox_dir.join("settings.local.json"));
            (user, project, local)
        };

    let layered = LayeredConfig::load(
        user_path.as_deref(),
        project_path.as_deref(),
        local_path.as_deref(),
    )
    .map_err(|e| RpcError::new(CONFIG_ERROR, format!("settings: {e}")))?;

    let mut rules: RuleSet = layered.build_rule_set();
    push_rules(&mut rules, &params.allow, PermissionBehavior::Allow);
    push_rules(&mut rules, &params.ask, PermissionBehavior::Ask);
    push_rules(&mut rules, &params.deny, PermissionBehavior::Deny);

    let mode = params.mode.as_deref().map_or_else(
        || layered.effective_mode().unwrap_or(PermissionMode::Default),
        PermissionMode::from_str_lossy,
    );

    for unreachable in detect_unreachable_rules(&rules, &DetectUnreachableOptions::default()) {
        tracing::warn!(
            rule = %format_rule(&unreachable.rule.value),
            reason = %unreachable.reason,
            fix = %unreachable.fix,
            "règle de permission inatteignable (masquée)"
        );
    }

    let home = dirs::home_dir().unwrap_or_else(|| workspace.as_std_path().to_path_buf());
    let engine = Arc::new(
        PermissionEngine::with_rules(rules).with_path_context(PathMatchContext::new(
            workspace.as_std_path(),
            home,
        )),
    );
    Ok((mode, PermissionPolicy::new(engine, mode)))
}

fn push_rules(set: &mut RuleSet, raw: &[String], behavior: PermissionBehavior) {
    for s in raw {
        set.push(Rule {
            value: parse_rule(s),
            behavior,
            source: RuleSource::CliArg,
        });
    }
}

async fn build_transcript(
    params: &AgentRunParams,
    system_merged: Option<&str>,
) -> Result<
    (
        Vec<drox_types::Message>,
        Option<TranscriptSessionConfig>,
        Option<Utf8PathBuf>,
    ),
    RpcError,
> {
    let Some(ref sid) = params.session_id else {
        return Ok((Vec::new(), None, None));
    };
    if !sid.starts_with("ses_") {
        return Err(RpcError::new(
            INVALID_PARAMS,
            "sessionId must start with `ses_`",
        ));
    }
    let dir = resolve_session_dir(params.session_dir.clone())?;
    let id = SessionId::from_string(sid.clone());
    let path = transcript_path(&dir, &id);
    let stats_path = session_ui_stats_path(&dir, &id);
    let history = match read_transcript(&path).await {
        Ok(h) => h,
        Err(SessionError::NotFound(_)) => Vec::new(),
        Err(e) => return Err(RpcError::new(ENGINE_ERROR, format!("session read: {e}"))),
    };
    let append_from = history.len() + usize::from(system_merged.is_some());
    let sink = JsonlTranscriptSink::arc(path);
    Ok((
        history,
        Some(TranscriptSessionConfig {
            sink,
            append_from_message_index: append_from,
        }),
        Some(stats_path),
    ))
}

fn usize_to_u32_saturated(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// Met à jour le fichier `*.ui-stats.json` comme le ferait la webview
/// (cumuls ↑↓ + dernier ctx).
async fn merge_ui_stats_from_agent_event(
    path: &Utf8Path,
    ev: &AgentEvent,
) -> Result<(), SessionError> {
    let mut s = read_session_ui_stats(path).await.unwrap_or_default();
    let mut dirty = false;
    match ev {
        AgentEvent::Stop { usage, .. } => {
            if usage.input_tokens > 0 || usage.output_tokens > 0 {
                s.total_in = s.total_in.saturating_add(u64::from(usage.input_tokens));
                s.total_out = s.total_out.saturating_add(u64::from(usage.output_tokens));
                if usage.input_tokens > 0 {
                    s.ctx = usage.input_tokens;
                }
                dirty = true;
            }
        }
        AgentEvent::ContextSnip {
            tokens_used_after, ..
        } => {
            s.ctx = usize_to_u32_saturated(*tokens_used_after);
            dirty = true;
        }
        AgentEvent::ContextCompacted {
            tokens_after,
            usage,
            ..
        } => {
            s.ctx = usize_to_u32_saturated(*tokens_after);
            if let Some(u) = usage {
                if u.input_tokens > 0 || u.output_tokens > 0 {
                    s.total_in = s.total_in.saturating_add(u64::from(u.input_tokens));
                    s.total_out = s.total_out.saturating_add(u64::from(u.output_tokens));
                }
            }
            dirty = true;
        }
        _ => {}
    }
    if dirty {
        write_session_ui_stats(path, &s).await?;
    }
    Ok(())
}

fn resolve_session_dir(arg: Option<Utf8PathBuf>) -> Result<Utf8PathBuf, RpcError> {
    if let Some(p) = arg {
        return Ok(p);
    }
    default_sessions_dir().map_err(|e| RpcError::new(CONFIG_ERROR, format!("session dir: {e}")))
}

fn resolve_workspace(arg: Option<Utf8PathBuf>) -> Result<Utf8PathBuf, RpcError> {
    let raw = match arg {
        Some(p) => p,
        None => Utf8PathBuf::try_from(
            std::env::current_dir()
                .map_err(|e| RpcError::new(CONFIG_ERROR, format!("cwd: {e}")))?,
        )
        .map_err(|e| RpcError::new(CONFIG_ERROR, format!("cwd not UTF-8: {e}")))?,
    };
    let canonical = std::fs::canonicalize(raw.as_std_path())
        .map_err(|e| RpcError::new(CONFIG_ERROR, format!("workspace `{raw}`: {e}")))?;
    Utf8PathBuf::try_from(canonical)
        .map_err(|e| RpcError::new(CONFIG_ERROR, format!("workspace not UTF-8: {e}")))
}

fn merge_optional_system(cli: Option<String>, mem: Option<String>) -> Option<String> {
    match (cli, mem) {
        (None, None) => None,
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (Some(a), Some(b)) => Some(format!("{a}\n\n{b}")),
    }
}

fn decode_optional<T: serde::de::DeserializeOwned + Default>(
    params: Option<Value>,
) -> Result<T, RpcError> {
    match params {
        None | Some(Value::Null) => Ok(T::default()),
        Some(v) => serde_json::from_value(v)
            .map_err(|e| RpcError::new(INVALID_PARAMS, format!("invalid params: {e}"))),
    }
}

fn decode_required<T: serde::de::DeserializeOwned>(
    params: Option<Value>,
    missing_msg: &str,
) -> Result<T, RpcError> {
    let Some(v) = params else {
        return Err(RpcError::new(INVALID_PARAMS, missing_msg.to_string()));
    };
    serde_json::from_value(v).map_err(|e| {
        RpcError::new(INVALID_PARAMS, format!("invalid params: {e}"))
            .with_data(json!({ "hint": missing_msg }))
    })
}

fn internal<E: std::fmt::Display>(e: E) -> RpcError {
    RpcError::new(super::INTERNAL_ERROR, e.to_string())
}

/// `UserAsker` qui refuse systématiquement les questions interactives.
///
/// Sélectionné quand `clientCapabilities.interactiveAsk` est `false` (ou
/// absent). Le client doit alors pré-configurer ses règles
/// `allow`/`ask`/`deny` ou utiliser `mode: "bypassPermissions"` /
/// `acceptEdits` pour éviter les blocages. Le tool `ask_user_question`
/// retournera systématiquement `ToolError::Interactive` dans ce cas.
struct RefuseAsker;

#[async_trait]
impl UserAsker for RefuseAsker {
    async fn ask(&self, q: UserQuestion) -> Result<UserAnswer, ToolError> {
        warn!(prompt = %q.prompt, "RefuseAsker active (client did not declare interactiveAsk)");
        Err(ToolError::interactive(
            "interactive prompts are disabled : client did not declare `interactiveAsk` \
             capability. Pre-configure allow/ask/deny rules or set mode to acceptEdits / \
             bypassPermissions.",
        ))
    }
}

/// `UserAsker` qui délègue au client via la requête serveur→client
/// `user/ask` (Sprint Questions bloquantes — §2.13 du backlog).
///
/// Le wrapper `ask_many` envoie **une seule** requête JSON-RPC contenant la
/// liste complète des questions ; cela permet au client de matérialiser la
/// carte « Questions » avec sa file 1 of N en une seule passe UI (et donc
/// au modèle d'attendre **un seul** round-trip réseau quel que soit le
/// nombre de questions).
///
/// L'implémentation par défaut de `ask` (mono-question) re-dispatch vers
/// `ask_many` pour rester cohérente.
struct RpcUserAsker {
    server: Server,
    run_id: String,
}

#[async_trait]
impl UserAsker for RpcUserAsker {
    async fn ask(&self, question: UserQuestion) -> Result<UserAnswer, ToolError> {
        let mut answers = self.ask_many(vec![question], None).await?;
        answers
            .pop()
            .ok_or_else(|| ToolError::interactive("user/ask returned no answer"))
    }

    async fn ask_many(
        &self,
        questions: Vec<UserQuestion>,
        title: Option<String>,
    ) -> Result<Vec<UserAnswer>, ToolError> {
        let ask_id = Uuid::new_v4().to_string();
        let rpc_questions: Vec<UserAskQuestion> = questions
            .iter()
            .enumerate()
            .map(|(i, q)| UserAskQuestion {
                id: q
                    .id
                    .clone()
                    .unwrap_or_else(|| format!("q{}", i + 1)),
                prompt: q.prompt.clone(),
                options: q
                    .choices
                    .iter()
                    .enumerate()
                    .map(|(j, label)| UserAskOption {
                        id: format!("opt{}", j + 1),
                        label: label.clone(),
                    })
                    .collect(),
                allow_multiple: q.allow_multiple,
                // On respecte le toggle remonté par le modèle via le tool
                // (cf. `AskQuestionItem::allow_free_text`). L'UI cliente
                // affiche le champ « détails optionnels » uniquement si ce
                // drapeau ou l'absence d'options structurées le justifie.
                allow_free_text: q.allow_free_text,
            })
            .collect();

        let params = UserAskParams {
            run_id: self.run_id.clone(),
            ask_id,
            title,
            questions: rpc_questions.clone(),
        };

        let value = self
            .server
            .send_request("user/ask", &params)
            .await
            .map_err(|e| {
                ToolError::interactive(format!(
                    "user/ask rejected: {} (code={})",
                    e.message, e.code
                ))
            })?;

        let result: UserAskResult = serde_json::from_value(value).map_err(|e| {
            ToolError::interactive(format!("user/ask returned malformed result: {e}"))
        })?;

        // Reconstruit les `UserAnswer` en s'alignant sur l'ordre **et** les
        // ids des questions envoyées : un client correct renvoie les mêmes
        // ids dans le même ordre, mais on tolère le désordre en cherchant
        // par id pour éviter les bugs subtils côté UI.
        let mut answers = Vec::with_capacity(questions.len());
        for (i, q_in) in questions.iter().enumerate() {
            let q_rpc = &rpc_questions[i];
            let matched = result
                .answers
                .iter()
                .find(|a| a.id == q_rpc.id)
                .or_else(|| result.answers.get(i));
            let Some(a) = matched else {
                return Err(ToolError::interactive(format!(
                    "user/ask missing answer for `{}`",
                    q_rpc.id
                )));
            };

            // Map `option_ids` ("opt1", "opt2"...) → indices 0-based dans
            // `q.choices` côté tool.
            let indices: Vec<usize> = a
                .option_ids
                .iter()
                .filter_map(|opt_id| {
                    q_rpc
                        .options
                        .iter()
                        .position(|o| &o.id == opt_id)
                })
                .collect();

            // Le `text` rendu au tool combine free_text + labels d'options
            // (joints par `,`) pour rester rétro-compatible avec les askers
            // mono-question existants qui ne regardent que `text`.
            let label_join = indices
                .iter()
                .filter_map(|i| q_in.choices.get(*i))
                .cloned()
                .collect::<Vec<_>>()
                .join(",");
            let text = if a.free_text.trim().is_empty() {
                label_join
            } else if label_join.is_empty() {
                a.free_text.clone()
            } else {
                format!("{label_join}\n{}", a.free_text)
            };

            answers.push(UserAnswer {
                id: Some(a.id.clone()),
                text,
                indices,
                skipped: a.skipped,
            });
        }

        Ok(answers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_server() -> Server {
        let (tx, _rx) = tokio::sync::mpsc::channel::<String>(8);
        Server::new(tx)
    }

    #[tokio::test]
    async fn initialize_handler_ignores_missing_params() {
        let server = fresh_server();
        let v = initialize(&server, None).await.unwrap();
        assert_eq!(v["serverName"], json!("drox"));
        assert_eq!(v["protocolVersion"], json!("1.0"));
    }

    #[tokio::test]
    async fn initialize_handler_accepts_explicit_params() {
        let server = fresh_server();
        let params = json!({
            "protocolVersion": "1.0",
            "clientName": "vscode-drox",
            "clientVersion": "0.1.0"
        });
        let v = initialize(&server, Some(params)).await.unwrap();
        assert_eq!(v["capabilities"]["sessions"], json!(true));
    }

    #[test]
    fn apply_disabled_tools_removes_bash_keeps_file_read() {
        let mut reg = default_tool_registry();
        apply_disabled_tools(&mut reg, &["bash".into(), "todo_write".into()]);
        assert!(reg.get("bash").is_none());
        assert!(reg.get("file_read").is_some());
        assert!(reg.get("todo_write").is_some());
    }

    #[tokio::test]
    async fn initialize_records_executable_tools_from_client() {
        let server = fresh_server();
        let params = json!({
            "clientCapabilities": {
                "executableTools": ["file_write", "bash"]
            }
        });
        initialize(&server, Some(params)).await.unwrap();
        assert!(server.is_executable_remotely("file_write"));
        assert!(server.is_executable_remotely("bash"));
        assert!(!server.is_executable_remotely("file_read"));
    }

    #[tokio::test]
    async fn build_tool_registry_wraps_declared_tools() {
        let server = fresh_server();
        server.set_executable_tools(["file_write".to_string()]);
        let registry = wrap_executable_tools(default_tool_registry(), &server, "run_test");
        // Le tool est toujours présent — mais c'est maintenant le wrapper, pas l'impl locale.
        assert!(registry.get("file_write").is_some());
        // Les autres restent inchangés et fonctionnels.
        assert!(registry.get("file_read").is_some());
    }

    #[tokio::test]
    async fn session_read_rejects_bad_id() {
        let err = session_read(Some(json!({ "id": "not_a_session" })))
            .await
            .unwrap_err();
        assert_eq!(err.code, INVALID_PARAMS);
    }

    #[tokio::test]
    async fn session_compact_rejects_bad_id() {
        let err = session_compact(Some(json!({ "id": "not_a_session" })))
            .await
            .unwrap_err();
        assert_eq!(err.code, INVALID_PARAMS);
    }

    #[tokio::test]
    async fn session_compact_rejects_empty_transcript() {
        let err = session_compact(Some(json!({ "id": "ses_aaaaaaaaaaaaaaaa" })))
            .await
            .unwrap_err();
        assert_eq!(err.code, INVALID_PARAMS);
    }

    #[test]
    fn agent_cancel_rejects_missing_params() {
        let (tx, _rx) = tokio::sync::mpsc::channel::<String>(1);
        let server = Server::new(tx);
        let err = agent_cancel(&server, None).unwrap_err();
        assert_eq!(err.code, INVALID_PARAMS);
    }

    #[tokio::test]
    async fn refuse_asker_returns_interactive_error() {
        let asker = RefuseAsker;
        let res = asker
            .ask(UserQuestion {
                id: None,
                prompt: "yes?".into(),
                choices: Vec::new(),
                allow_multiple: false,
                allow_free_text: false,
            })
            .await;
        match res {
            Err(ToolError::Interactive(msg)) => {
                assert!(
                    msg.contains("interactive prompts are disabled")
                        || msg.contains("interactiveAsk"),
                    "got: {msg}",
                );
            }
            other => panic!("expected Interactive error, got {other:?}"),
        }
    }
}
