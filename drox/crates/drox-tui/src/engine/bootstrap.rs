//! Initialisation moteur (aligné `drox-cli`, sans services externes).

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use drox_cli::{language, prompts};
use drox_engine::{
    default_tool_registry, load_sessions_listing, load_skills_catalog, Agent, AgentConfig,
    ContextPolicy, DroxIgnoreMatcher, JsonlTranscriptSink, LayeredConfig, MemoryRuntime,
    PermissionEngine, PermissionMode, PermissionPolicy, Phase, TranscriptSessionConfig,
    WorkspaceMapStore, AgentEvent, EngineError, DEFAULT_LISTING_LIMIT, DEFAULT_MAX_PARALLEL_TOOL_CALLS,
};
use drox_llm::{ChatOptions, LlmConfig, OllamaClient};
use drox_permissions::{
    DetectUnreachableOptions, PathMatchContext, PermissionBehavior, Rule, RuleSet, RuleSource,
    detect_unreachable_rules, parse_rule,
};
use drox_session::{default_sessions_dir, format_sessions_listing_for_prompt, read_transcript, transcript_path};
use drox_tools::{ScopeDeferredHandle, ToolContext};
use drox_types::{Message, SessionId, ToolUseId};
use futures::StreamExt;
use parking_lot::Mutex;
use tokio::sync::mpsc;
use tracing::warn;
use uuid::Uuid;

use crate::app::AppConfig;
use crate::asker::{AskCoordinator, TuiUserAsker};
use crate::engine::connection_library::{apply_auth_to_config, legacy_api_key_to_auth, profile_to_llm_config, ConnectionProfile};
use crate::view::LogEntry;

struct LlmRuntimeState {
    client: Arc<OllamaClient>,
    model_label: String,
}

/// Runtime moteur partagé entre les tours du REPL TUI.
pub struct EngineRuntime {
    llm_state: parking_lot::RwLock<LlmRuntimeState>,
    pub registry: Arc<drox_tools::ToolRegistry>,
    pub workspace: Utf8PathBuf,
    permission_engine: Arc<PermissionEngine>,
    mutable: Mutex<RuntimeMutable>,
    pub system_prompt: String,
    pub(crate) memory: parking_lot::RwLock<MemoryRuntime>,
    hooks: Mutex<Option<drox_engine::ToolHooksConfig>>,
    max_iterations: parking_lot::RwLock<usize>,
    pub apply_fs_writes: bool,
    pub workspace_map: WorkspaceMapStore,
    pub drox_ignore: DroxIgnoreMatcher,
    /// Répertoires additionnels (`/add-dir`) — session uniquement.
    additional_dirs: Mutex<Vec<Utf8PathBuf>>,
    pub scope_deferred: ScopeDeferredHandle,
    pub sessions_dir: Utf8PathBuf,
    session: Mutex<SessionInfo>,
    boot: parking_lot::RwLock<AppConfig>,
    num_ctx: parking_lot::RwLock<usize>,
}

#[derive(Debug, Clone)]
struct RuntimeMutable {
    permission_mode: PermissionMode,
    plan_mode: bool,
    pre_plan_mode: Option<PermissionMode>,
}

/// Résultat d'un basculement mode plan (`/plan`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanModeChange {
    Enabled,
    Disabled,
    AlreadyOn,
    AlreadyOff,
}

#[derive(Debug, Clone)]
struct SessionInfo {
    id: String,
    transcript_path: Utf8PathBuf,
}

impl EngineRuntime {
    #[must_use]
    pub fn session_id(&self) -> String {
        self.session.lock().id.clone()
    }

    #[must_use]
    pub fn transcript_path(&self) -> Utf8PathBuf {
        self.session.lock().transcript_path.clone()
    }
    /// Nouvelle session (nouveau fichier transcript).
    pub fn rotate_session(&self) {
        let session_id = format!("ses_{}", Uuid::new_v4().as_simple());
        let sid = SessionId::from_string(session_id.clone());
        let path = transcript_path(&self.sessions_dir, &sid);
        *self.session.lock() = SessionInfo {
            id: session_id,
            transcript_path: path,
        };
    }

    /// Reprend une session existante par identifiant `ses_…`.
    pub fn resume_session(&self, sid: &str) -> anyhow::Result<()> {
        if !sid.starts_with("ses_") {
            anyhow::bail!("identifiant session invalide (attendu ses_…)");
        }
        let id = SessionId::from_string(sid.to_string());
        *self.session.lock() = SessionInfo {
            id: sid.to_string(),
            transcript_path: transcript_path(&self.sessions_dir, &id),
        };
        Ok(())
    }

    /// Remplace le client LLM courant (prochains runs agent).
    pub fn apply_llm_connection(
        &self,
        server: &str,
        model: &str,
        api_key: Option<&str>,
        num_ctx: i64,
    ) -> anyhow::Result<()> {
        let api_key_owned = api_key.map(str::to_string);
        let config = build_llm_config(server, model, &api_key_owned, num_ctx, &BTreeMap::new())?;
        self.apply_llm_config(config, server, model, api_key_owned, num_ctx)
    }

    /// Applique un profil complet (auth + headers custom).
    pub fn apply_llm_connection_profile(&self, profile: &ConnectionProfile) -> anyhow::Result<()> {
        let model = profile
            .effective_model()
            .context("modele requis dans le profil")?;
        let config = profile_to_llm_config(profile).context("config LLM profil")?;
        let api_key_owned = profile
            .auth
            .as_legacy_api_key()
            .or_else(|| profile.extra_headers.get("x-api-key").cloned());
        self.apply_llm_config(
            config,
            profile.base_url.trim(),
            model,
            api_key_owned,
            profile.num_ctx,
        )?;
        self.set_max_iterations(profile.max_iterations);
        Ok(())
    }

    fn apply_llm_config(
        &self,
        config: LlmConfig,
        server: &str,
        model: &str,
        api_key_owned: Option<String>,
        num_ctx: i64,
    ) -> anyhow::Result<()> {
        let client = Arc::new(OllamaClient::new(config).context("client LLM")?);
        let ctx = num_ctx.max(2048) as usize;

        {
            let mut llm = self.llm_state.write();
            llm.client = client.clone();
            llm.model_label = model.to_string();
        }
        {
            let mut memory = self.memory.write();
            memory.llm = client;
            memory.model_label = model.to_string();
        }
        {
            let mut boot = self.boot.write();
            boot.server = server.to_string();
            boot.model = model.to_string();
            boot.api_key = api_key_owned;
            boot.num_ctx = num_ctx;
        }
        *self.num_ctx.write() = ctx;
        Ok(())
    }

    #[must_use]
    pub fn num_ctx(&self) -> usize {
        *self.num_ctx.read()
    }

    #[must_use]
    pub fn max_iterations(&self) -> usize {
        *self.max_iterations.read()
    }

    pub fn set_max_iterations(&self, value: usize) {
        let v = value.clamp(1, 256);
        *self.max_iterations.write() = v;
        self.boot.write().max_iterations = v;
    }

    pub async fn bootstrap(config: &AppConfig, _ask: Arc<AskCoordinator>) -> anyhow::Result<Self> {
        let workspace = resolve_workspace(Some(config.workspace.clone()))?;
        let workspace_fingerprint = workspace.as_str().to_string();

        let llm_config = build_llm_config(
            &config.server,
            &config.model,
            &config.api_key,
            config.num_ctx,
            &BTreeMap::new(),
        )?;
        let num_ctx = llm_config.num_ctx.max(2048) as usize;
        let llm = Arc::new(OllamaClient::new(llm_config).context("client LLM")?);
        let registry = Arc::new(default_tool_registry());

        let (mode, permission_engine) = build_permission_engine(&workspace, config)?;
        let drox_ignore = DroxIgnoreMatcher::load_or_create(workspace.clone()).await?;
        let workspace_map = WorkspaceMapStore::load_or_create(
            workspace.clone(),
            workspace_fingerprint,
            Some(drox_ignore.clone()),
        )
        .await?;

        let mut system = build_system_prompt(&workspace, &workspace_map, &drox_ignore).await?;
        system = language::merge_into_system(Some(system), language::from_env().as_ref())
            .expect("system prompt toujours défini après build");

        let sessions_dir = config
            .session_dir
            .clone()
            .or_else(|| default_sessions_dir().ok())
            .context("sessions ~/.drox/sessions")?;

        let (session_id, transcript_path) = if let Some(ref sid) = config.session {
            if !sid.starts_with("ses_") {
                anyhow::bail!("--session doit être un identifiant ses_…");
            }
            let id = SessionId::from_string(sid.clone());
            (sid.clone(), transcript_path(&sessions_dir, &id))
        } else {
            let session_id = format!("ses_{}", Uuid::new_v4().as_simple());
            let sid = SessionId::from_string(session_id.clone());
            (session_id, transcript_path(&sessions_dir, &sid))
        };

        let memory = MemoryRuntime {
            workspace_root: workspace.clone(),
            llm: llm.clone(),
            compaction_prompt: prompts::COMPACTION_PROMPT.to_string(),
            compaction_config: drox_engine::CompactionConfig::default(),
            notes: drox_engine::SessionNotesHandle::new(),
            model_label: config.model.clone(),
        };

        let tool_hooks = drox_engine::load_tool_hooks(&workspace);
        let plan_mode = config.plan_mode || mode == PermissionMode::Plan;
        Ok(Self {
            llm_state: parking_lot::RwLock::new(LlmRuntimeState {
                client: llm,
                model_label: config.model.clone(),
            }),
            registry,
            workspace,
            permission_engine,
            mutable: Mutex::new(RuntimeMutable {
                permission_mode: mode,
                plan_mode,
                pre_plan_mode: None,
            }),
            system_prompt: system,
            memory: parking_lot::RwLock::new(memory),
            hooks: Mutex::new(if tool_hooks.is_enabled() {
                Some(tool_hooks)
            } else {
                None
            }),
            max_iterations: parking_lot::RwLock::new(config.max_iterations),
            apply_fs_writes: config.apply,
            workspace_map,
            drox_ignore,
            additional_dirs: Mutex::new(Vec::new()),
            scope_deferred: ScopeDeferredHandle::new(),
            sessions_dir,
            session: Mutex::new(SessionInfo {
                id: session_id,
                transcript_path,
            }),
            boot: parking_lot::RwLock::new(config.clone()),
            num_ctx: parking_lot::RwLock::new(num_ctx),
        })
    }

    #[must_use]
    pub fn llm(&self) -> Arc<OllamaClient> {
        self.llm_state.read().client.clone()
    }

    #[must_use]
    pub fn model_label(&self) -> String {
        self.llm_state.read().model_label.clone()
    }

    #[must_use]
    pub fn hooks_active(&self) -> bool {
        self.hooks.lock().is_some()
    }

    pub(crate) fn boot_config(&self) -> AppConfig {
        self.boot.read().clone()
    }

    /// Racines pour résolution de chemins : workspace principal + `/add-dir`.
    #[must_use]
    pub fn working_directories(&self) -> Vec<Utf8PathBuf> {
        let mut roots = vec![self.workspace.clone()];
        roots.extend(self.additional_dirs.lock().clone());
        roots
    }

    /// Ajoute un répertoire de travail (chemin canonique UTF-8).
    pub fn add_working_directory(&self, path: Utf8PathBuf) -> anyhow::Result<()> {
        let mut dirs = self.additional_dirs.lock();
        if dirs.iter().any(|d| d == &path) {
            anyhow::bail!("répertoire déjà enregistré : {path}");
        }
        dirs.push(path);
        Ok(())
    }

    #[must_use]
    pub fn permission_mode(&self) -> PermissionMode {
        self.mutable.lock().permission_mode
    }

    #[must_use]
    pub fn plan_mode(&self) -> bool {
        self.mutable.lock().plan_mode
    }

    #[must_use]
    pub fn permission_policy(&self) -> PermissionPolicy {
        let mode = self.mutable.lock().permission_mode;
        PermissionPolicy::new(Arc::clone(&self.permission_engine), mode)
    }

    /// Active le mode plan (lecture seule côté écritures).
    pub fn enable_plan_mode(&self) -> PlanModeChange {
        let mut m = self.mutable.lock();
        if m.plan_mode {
            return PlanModeChange::AlreadyOn;
        }
        m.pre_plan_mode = Some(m.permission_mode);
        m.permission_mode = PermissionMode::Plan;
        m.plan_mode = true;
        PlanModeChange::Enabled
    }

    /// Désactive le mode plan et restaure le mode précédent.
    pub fn disable_plan_mode(&self) -> PlanModeChange {
        let mut m = self.mutable.lock();
        if !m.plan_mode {
            return PlanModeChange::AlreadyOff;
        }
        m.permission_mode = m.pre_plan_mode.take().unwrap_or(PermissionMode::Default);
        m.plan_mode = false;
        PlanModeChange::Disabled
    }

    /// Chemin conventionnel du plan workspace (`.drox/plan.md`).
    #[must_use]
    pub fn plan_file_path(&self) -> Utf8PathBuf {
        self.workspace.join(".drox/plan.md")
    }

    /// Lit le plan markdown s'il existe.
    pub fn read_plan_file(&self) -> anyhow::Result<String> {
        let path = self.plan_file_path();
        std::fs::read_to_string(path.as_std_path()).context("lecture .drox/plan.md")
    }

    /// Recharge `.drox/hooks.json` (projet + utilisateur) pour les prochains runs.
    #[must_use]
    pub fn reload_hooks(&self) -> bool {
        let loaded = drox_engine::load_tool_hooks(&self.workspace);
        let enabled = loaded.is_enabled();
        *self.hooks.lock() = if enabled { Some(loaded) } else { None };
        enabled
    }

    /// Lignes pour `/hooks` (état fusionné sur disque).
    #[must_use]
    pub fn format_hooks_lines(&self) -> Vec<String> {
        let merged = drox_engine::load_tool_hooks(&self.workspace);
        let active = self.hooks.lock().is_some();
        super::hooks::format_hooks_config(&self.workspace, &merged, active)
    }

    fn tool_context(&self, ask: Arc<AskCoordinator>) -> ToolContext {
        let plan_mode = self.plan_mode();
        ToolContext::new(self.workspace.clone(), self.apply_fs_writes)
            .with_plan_mode(plan_mode)
            .with_user_asker(Arc::new(TuiUserAsker::new(ask)))
            .with_scope_deferred(self.scope_deferred.clone())
            .with_workspace_map(self.workspace_map.clone())
            .with_drox_ignore(self.drox_ignore.clone())
    }

    /// Exécute une commande shell utilisateur (`!` mode) via le tool `bash`.
    pub async fn execute_user_bash(
        &self,
        ask: Arc<AskCoordinator>,
        command: &str,
        progress: Option<Arc<dyn drox_tools::ToolProgressSink>>,
    ) -> Result<serde_json::Value, drox_tools::ToolError> {
        let mut ctx = self.tool_context(ask);
        if let Some(sink) = progress {
            ctx = ctx.with_tool_progress(sink);
        }
        self.registry
            .execute_named(
                "bash",
                &ctx,
                serde_json::json!({ "command": command }),
            )
            .await
    }

    fn build_agent(&self, ask: Arc<AskCoordinator>) -> Agent {
        let ctx = self.tool_context(ask);
        let transcript_path = self.transcript_path();
        let sink = JsonlTranscriptSink::arc(transcript_path.clone());
        let history_len = read_transcript_line_count(&transcript_path);
        let append_from = history_len + usize::from(!self.system_prompt.is_empty());
        Agent::new(
            self.llm(),
            self.registry.clone(),
            ctx,
            AgentConfig {
                system_prompt: Some(self.system_prompt.clone()),
                max_iterations: self.max_iterations(),
                chat_options: ChatOptions::default(),
                permissions: Some(self.permission_policy()),
                context: Some(ContextPolicy::for_model_context_window(self.num_ctx())),
                transcript: Some(TranscriptSessionConfig {
                    sink,
                    append_from_message_index: append_from,
                }),
                memory: Some(self.memory.read().clone()),
                transcript_session_id: Some(self.session_id()),
                workspace_fingerprint: self.workspace.as_str().to_string(),
                max_parallel_tool_calls: DEFAULT_MAX_PARALLEL_TOOL_CALLS,
                tool_hooks: self.hooks.lock().clone(),
                run_objective: None,
            },
        )
    }

    pub async fn load_history(&self) -> Vec<Message> {
        let path = self.transcript_path();
        match read_transcript(&path).await {
            Ok(h) => h,
            Err(e) => {
                warn!(error = %e, "transcript illisible");
                Vec::new()
            }
        }
    }

    pub fn spawn_run(
        self: &Arc<Self>,
        user_blocks: Vec<drox_types::Content>,
        ask: Arc<AskCoordinator>,
        out: mpsc::Sender<Result<AgentEvent, EngineError>>,
    ) -> tokio::task::JoinHandle<()> {
        let runtime = Arc::clone(self);
        tokio::spawn(async move {
            let history = runtime.load_history().await;
            let agent = runtime.build_agent(ask);
            let mut stream = agent.run_with_history_blocks(history, user_blocks);
            while let Some(event) = stream.next().await {
                if out.send(event).await.is_err() {
                    break;
                }
            }
        })
    }
}

/// Destination du buffer streaming lors d'un flush (aligné `transcript_replay`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StreamFlushKind {
    /// Texte rattaché au bloc `PhaseOpen` courant.
    PhaseLine,
    /// Message assistant autonome (hors bloc phase).
    Assistant,
}

fn stream_flush_kind(entries: &[LogEntry]) -> StreamFlushKind {
    for entry in entries.iter().rev() {
        match entry {
            LogEntry::PhaseLine { .. } => continue,
            LogEntry::PhaseOpen { .. } => return StreamFlushKind::PhaseLine,
            _ => return StreamFlushKind::Assistant,
        }
    }
    StreamFlushKind::Assistant
}

fn last_phase_open(entries: &[LogEntry]) -> Option<Phase> {
    for entry in entries.iter().rev() {
        match entry {
            LogEntry::PhaseLine { .. } => continue,
            LogEntry::PhaseOpen { phase } => return Some(*phase),
            _ => return None,
        }
    }
    None
}

fn flush_stream(
    entries: &mut Vec<LogEntry>,
    streaming: &mut Option<String>,
    kind: StreamFlushKind,
) {
    if let Some(buf) = streaming.take() {
        if !buf.trim().is_empty() {
            match kind {
                StreamFlushKind::PhaseLine => {
                    entries.push(LogEntry::PhaseLine { text: buf });
                }
                StreamFlushKind::Assistant => {
                    entries.push(LogEntry::Assistant { text: buf });
                }
            }
        }
    }
}

pub fn apply_agent_event(
    entries: &mut Vec<LogEntry>,
    streaming: &mut Option<String>,
    tool_names: &mut HashMap<ToolUseId, String>,
    event: &AgentEvent,
) {
    match event {
        AgentEvent::PhaseEnter { phase } => {
            let had_stream = streaming.as_ref().is_some_and(|s| !s.trim().is_empty());
            flush_stream(entries, streaming, stream_flush_kind(entries));
            if *phase == Phase::Done {
                return;
            }
            let duplicate_answering = *phase == Phase::Answering
                && !had_stream
                && last_phase_open(entries) == Some(Phase::Answering);
            if !duplicate_answering {
                entries.push(LogEntry::PhaseOpen { phase: *phase });
            }
        }
        AgentEvent::PhaseClose => {
            flush_stream(entries, streaming, stream_flush_kind(entries));
        }
        AgentEvent::TextDelta { text } => {
            streaming.get_or_insert_with(String::new).push_str(text);
        }
        AgentEvent::ToolStart { id, name, arguments } => {
            flush_stream(entries, streaming, stream_flush_kind(entries));
            tool_names.insert(id.clone(), name.clone());
            entries.push(LogEntry::ToolStart {
                id: id.clone(),
                name: name.clone(),
                arguments: arguments.clone(),
            });
        }
        AgentEvent::ToolFinish { id, output, is_error } => {
            let name = tool_names
                .get(id)
                .cloned()
                .unwrap_or_else(|| "?".into());
            entries.push(LogEntry::ToolFinish {
                id: id.clone(),
                name,
                output: output.clone(),
                is_error: *is_error,
            });
        }
        AgentEvent::ToolProgress { .. } => {}
        AgentEvent::ContextSnip { tokens_freed, blocks_snipped, .. } => {
            entries.push(LogEntry::ContextSnip {
                tokens_freed: *tokens_freed,
                blocks_snipped: *blocks_snipped,
            });
        }
        AgentEvent::ContextCompacted {
            tokens_before,
            tokens_after,
            messages_removed,
            ..
        } => entries.push(LogEntry::ContextCompacted {
            tokens_before: *tokens_before,
            tokens_after: *tokens_after,
            messages_removed: *messages_removed,
        }),
        AgentEvent::MemoryPersisted { slug, objective, .. } => entries.push(LogEntry::MemoryPersisted {
            slug: slug.clone(),
            objective: objective.clone(),
        }),
        AgentEvent::RunObjective { text } => entries.push(LogEntry::RunObjective { text: text.clone() }),
        AgentEvent::ScopeParkingUpdate { .. } => {}
        AgentEvent::Stop { .. } => {
            flush_stream(entries, streaming, stream_flush_kind(entries));
        }
        _ => {}
    }
}

#[cfg(test)]
mod stream_flush_tests {
    use super::*;
    use drox_types::ToolUseId;

    fn simulate(events: &[AgentEvent]) -> (Vec<LogEntry>, Option<String>) {
        let mut entries = Vec::new();
        let mut streaming = None;
        let mut tool_names = HashMap::new();
        for event in events {
            apply_agent_event(&mut entries, &mut streaming, &mut tool_names, event);
        }
        (entries, streaming)
    }

    #[test]
    fn answering_text_lives_under_phase_block_not_assistant() {
        let (entries, _) = simulate(&[
            AgentEvent::PhaseEnter {
                phase: Phase::Answering,
            },
            AgentEvent::TextDelta {
                text: "Bonjour !\n".into(),
            },
            AgentEvent::PhaseEnter { phase: Phase::Done },
        ]);
        assert!(entries.iter().any(|e| {
            matches!(e, LogEntry::PhaseOpen { phase: Phase::Answering })
        }));
        assert!(entries.iter().any(|e| {
            matches!(e, LogEntry::PhaseLine { text } if text.contains("Bonjour"))
        }));
        assert!(!entries.iter().any(|e| matches!(e, LogEntry::Assistant { .. })));
        assert!(!entries.iter().any(|e| {
            matches!(e, LogEntry::PhaseOpen { phase: Phase::Done })
        }));
    }

    #[test]
    fn stop_without_phase_open_emits_assistant() {
        let (entries, _) = simulate(&[
            AgentEvent::TextDelta {
                text: "salut".into(),
            },
            AgentEvent::Stop {
                reason: drox_types::StopReason::EndTurn,
                usage: drox_types::Usage::default(),
            },
        ]);
        assert!(entries.iter().any(|e| {
            matches!(e, LogEntry::Assistant { text } if text.contains("salut"))
        }));
    }

    #[test]
    fn duplicate_answering_header_skipped_on_empty_nudge() {
        let (entries, _) = simulate(&[
            AgentEvent::PhaseEnter {
                phase: Phase::Answering,
            },
            AgentEvent::TextDelta {
                text: "Réponse finale.\n".into(),
            },
            AgentEvent::PhaseEnter { phase: Phase::Done },
            AgentEvent::PhaseEnter {
                phase: Phase::Answering,
            },
            AgentEvent::PhaseEnter { phase: Phase::Done },
        ]);
        let answering_headers = entries
            .iter()
            .filter(|e| matches!(e, LogEntry::PhaseOpen { phase: Phase::Answering }))
            .count();
        assert_eq!(answering_headers, 1);
    }

    #[test]
    fn tool_start_flushes_into_open_phase() {
        let (entries, _) = simulate(&[
            AgentEvent::PhaseEnter {
                phase: Phase::Acting,
            },
            AgentEvent::TextDelta {
                text: "je modifie…\n".into(),
            },
            AgentEvent::ToolStart {
                id: ToolUseId::new(),
                name: "file_write".into(),
                arguments: serde_json::json!({}),
            },
        ]);
        assert!(entries.iter().any(|e| matches!(e, LogEntry::PhaseLine { .. })));
        assert!(!entries.iter().any(|e| matches!(e, LogEntry::Assistant { .. })));
    }
}

fn read_transcript_line_count(path: &Utf8Path) -> usize {
    std::fs::read_to_string(path.as_std_path())
        .map(|s| s.lines().filter(|l| !l.trim().is_empty()).count())
        .unwrap_or(0)
}

fn resolve_workspace(arg: Option<Utf8PathBuf>) -> anyhow::Result<Utf8PathBuf> {
    let raw = arg.unwrap_or_else(|| {
        Utf8PathBuf::try_from(std::env::current_dir().unwrap()).unwrap_or_else(|_| Utf8PathBuf::from("."))
    });
    let canonical = std::fs::canonicalize(raw.as_std_path())
        .with_context(|| format!("workspace: {raw}"))?;
    Utf8PathBuf::try_from(canonical).context("workspace non UTF-8")
}

pub(crate) fn build_llm_config(
    server: &str,
    model: &str,
    api_key: &Option<String>,
    num_ctx: i64,
    extra_headers: &BTreeMap<String, String>,
) -> anyhow::Result<LlmConfig> {
    let mut config = LlmConfig::try_from_str(server, model)?;
    config = config.with_num_ctx(num_ctx.max(2048));
    let key = api_key
        .as_ref()
        .filter(|k| !k.trim().is_empty())
        .cloned()
        .or_else(|| {
            std::env::var("DROX_API_KEY")
                .ok()
                .filter(|k| !k.trim().is_empty())
        });
    let auth = legacy_api_key_to_auth(key.as_deref(), server);
    apply_auth_to_config(&mut config, &auth, extra_headers);
    Ok(config)
}

fn build_permission_engine(
    workspace: &Utf8PathBuf,
    config: &AppConfig,
) -> anyhow::Result<(PermissionMode, Arc<PermissionEngine>)> {
    let user = dirs::home_dir().map(|h| h.join(".drox").join("settings.json"));
    let drox_dir = workspace.as_std_path().join(".drox");
    let layered = if config.no_settings {
        LayeredConfig::default()
    } else {
        LayeredConfig::load(
            user.as_deref(),
            Some(drox_dir.join("settings.json").as_path()),
            Some(drox_dir.join("settings.local.json").as_path()),
        )
        .context("settings")?
    };

    let mut rules: RuleSet = layered.build_rule_set();
    for raw in &config.allow {
        rules.push(Rule {
            value: parse_rule(raw),
            behavior: PermissionBehavior::Allow,
            source: RuleSource::CliArg,
        });
    }
    for raw in &config.ask {
        rules.push(Rule {
            value: parse_rule(raw),
            behavior: PermissionBehavior::Ask,
            source: RuleSource::CliArg,
        });
    }
    for raw in &config.deny {
        rules.push(Rule {
            value: parse_rule(raw),
            behavior: PermissionBehavior::Deny,
            source: RuleSource::CliArg,
        });
    }

    let mode = if config.plan_mode {
        PermissionMode::Plan
    } else if let Some(ref m) = config.mode {
        PermissionMode::from_str_lossy(m)
    } else if let Some(m) = layered.effective_mode() {
        m
    } else {
        PermissionMode::Default
    };

    for u in detect_unreachable_rules(&rules, &DetectUnreachableOptions::default()) {
        warn!(reason = %u.reason, "permission shadowed");
    }

    let home = dirs::home_dir().unwrap_or_else(|| workspace.as_std_path().to_path_buf());
    let engine = Arc::new(
        PermissionEngine::with_rules(rules).with_path_context(PathMatchContext::new(
            workspace.as_std_path(),
            home,
        )),
    );
    Ok((mode, engine))
}

async fn build_system_prompt(
    workspace: &Utf8PathBuf,
    workspace_map: &WorkspaceMapStore,
    drox_ignore: &DroxIgnoreMatcher,
) -> anyhow::Result<String> {
    let mem = drox_engine::load_memdir(workspace.as_path()).await?;
    let mut system = prompts::prepend_core_system_prompt(drox_engine::memdir_system_prefix(&mem));
    let listing = load_sessions_listing(workspace.as_path(), DEFAULT_LISTING_LIMIT)
        .await
        .unwrap_or_default();
    if let Some(block) = format_sessions_listing_for_prompt(&listing) {
        system.push_str("\n\n");
        system.push_str(&block);
    }
    let skills = load_skills_catalog(workspace.as_path()).await.unwrap_or_default();
    if let Some(block) = drox_engine::format_skills_listing_for_prompt(&skills) {
        system.push_str("\n\n");
        system.push_str(&block);
    }
    system.push_str("\n\n");
    system.push_str(&drox_ignore.format_for_prompt());
    if let Some(block) = workspace_map.format_for_prompt() {
        system.push_str("\n\n");
        system.push_str(&block);
    }
    Ok(system)
}
