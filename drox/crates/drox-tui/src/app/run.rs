//! Boucle événements principale (crossterm + ratatui + moteur).

use std::collections::{HashMap, VecDeque};
use std::io;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Context;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use drox_cli::env_file;
use drox_engine::{AgentEvent, EngineError};
use drox_session::{default_sessions_dir, list_sessions};
use drox_tools::UserAnswer;
use drox_types::ToolUseId;
use serde_json::Value;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use tokio::sync::mpsc;

use crate::asker::{parse_answer, AskCoordinator, PendingAsk};
use crate::engine::{apply_agent_event, compact_checkpoint_preview, cycle_preset_index, BindingAction, CompactOutcome, EngineRuntime, TuiKeybindings, VimComposer, VimKeyResult, VimMode, CONTEXT_CUSTOM_INDEX};
use crate::engine::status_bar::StatusBarSnapshot;
use crate::slash::{apply_theme, filter_entries, handle_config, handle_hooks, handle_plan, handle_slash, PendingSlash, SlashOutcome, SLASH_PALETTE_ENTRIES};
use crate::ui::TuiThemeSetting;
use crate::terminal;
use crate::ui;
use crate::view::{
    bash_progress, compute_permission_preview, find_history_match_indices, find_matching_line_indices,
    messages_to_log_entries, parse_plan_approval_preview, scroll_to_line, ActiveBashRun,
    ActiveHookProgress, LogEntry, TranscriptSearchState,
};
use crate::i18n::keys_p1 as sk;
use crate::AppConfig;

use crate::app::state::{
    AiServerDialog, AiServerSelectFocus, AiServerStep, AppPhase, AppState, AuthTypeChoice,
    CloudProviderChoice, ComposerMode, ComposerSuggestionDialog, ConfigureField, CopyDialog,
    DeploymentKind,
    OnboardingDialog, PersonalEngineChoice, PromptDialog, RewindChoiceView, RewindDialog, RunStatus,
    SettingsDialog, SettingsRowKind,
    SlashPaletteDialog, ThemeDialog, WorkspaceDialog, WorkspaceField, WorkspaceStep,
};
use crate::engine::at_typeahead::{self, AtFileIndex};
use crate::engine::unified_suggestions::{
    self, preserve_cursor, ComposerSuggestionItem, SkillSuggestionEntry, SuggestionKind,
};
use crate::engine::preferences::{load_preferences, load_preferences_detailed, preferences_path, record_recent_workspace, resolve_llm_startup, save_llm_connection, llm_connection_from_config, TuiPreferences};
use crate::terminal::typed_char;
use crate::ui::resolve_palette;
use crate::engine::notices::{NoticeLevel, StatusNotice};
use crate::terminal::set_terminal_title;

/// Application TUI complète.
pub struct App {
    config: AppConfig,
    state: AppState,
    runtime: Option<Arc<EngineRuntime>>,
    ask: Arc<AskCoordinator>,
    agent_rx: Option<mpsc::Receiver<Result<AgentEvent, EngineError>>>,
    agent_handle: Option<tokio::task::JoinHandle<()>>,
    pending_reply: Option<PendingAsk>,
    tool_names: HashMap<ToolUseId, String>,
    auto_scroll: bool,
    message_queue: VecDeque<String>,
    input_history: Vec<String>,
    history_cursor: Option<usize>,
    pending_hydrate: bool,
    pending_title_refresh: bool,
    pending_slash: Option<PendingSlash>,
    ctrl_c_streak: u8,
    last_ctrl_c: Option<Instant>,
    /// Recherche historique composer (`Ctrl+R`) — état dans `App`, pas `AppState`.
    history_search: Option<crate::view::HistorySearchState>,
    /// Dernière requête recherche transcript (réutilisée au prochain Ctrl+F).
    last_transcript_query: String,
    status_snapshot: StatusBarSnapshot,
    session_started: Instant,
    status_tick: u8,
    keybindings: TuiKeybindings,
    tui_prefs: TuiPreferences,
    /// Index fichiers pour le typeahead `@` (lazy, invalidé au changement de workspace).
    at_file_index: Option<AtFileIndex>,
    /// Empreinte des racines indexées (workspace + `/add-dir`).
    at_file_roots_key: Option<String>,
    /// Contenus des collages smart paste (`[Pasted text #N]`).
    paste_store: crate::engine::PastedTextStore,
    /// Cache noms skills pour complétion `/skills`.
    skill_suggestions: Vec<SkillSuggestionEntry>,
    /// Mode vim composer (`/vim`).
    vim: VimComposer,
    /// Résultat async test connexion IA.
    ai_server_test_rx: Option<mpsc::Receiver<Result<Vec<String>, String>>>,
}

impl App {
    #[must_use]
    pub fn new(config: AppConfig) -> Self {
        let tui_prefs = crate::engine::preferences::load_preferences();
        let mut vim = VimComposer::default();
        vim.set_enabled(tui_prefs.vim_enabled, "");
        Self {
            config,
            state: AppState::new(),
            runtime: None,
            ask: Arc::new(AskCoordinator::new()),
            agent_rx: None,
            agent_handle: None,
            pending_reply: None,
            tool_names: HashMap::new(),
            auto_scroll: true,
            message_queue: VecDeque::new(),
            input_history: Vec::new(),
            history_cursor: None,
            pending_hydrate: false,
            pending_title_refresh: false,
            pending_slash: None,
            ctrl_c_streak: 0,
            last_ctrl_c: None,
            history_search: None,
            last_transcript_query: String::new(),
            status_snapshot: StatusBarSnapshot::default(),
            session_started: Instant::now(),
            status_tick: 0,
            keybindings: TuiKeybindings::load(),
            tui_prefs,
            at_file_index: None,
            at_file_roots_key: None,
            paste_store: crate::engine::PastedTextStore::default(),
            skill_suggestions: Vec::new(),
            vim,
            ai_server_test_rx: None,
        }
    }

    /// Lance la boucle jusqu'à quit explicite.
    pub async fn run(&mut self) -> anyhow::Result<()> {
        env_file::load_default(Some(self.config.workspace.as_std_path()));

        let loaded = load_preferences_detailed();
        self.tui_prefs = loaded.prefs.clone();
        let llm_configured = resolve_llm_startup(&mut self.config, &self.tui_prefs);
        self.state.llm_configured = llm_configured;

        self.state.theme = self.tui_prefs.theme;
        self.state.session_accent = self.tui_prefs.session_color;
        self.state.palette = resolve_palette(self.state.theme, self.state.session_accent);
        self.state.animations_enabled = self.tui_prefs.animations_enabled;
        self.state.mouse_enabled = self.tui_prefs.mouse_enabled;
        crate::i18n::set_locale(self.tui_prefs.ui_locale);
        if loaded.theme_migrated_to_drox {
            self.state.status_line = crate::i18n::t(sk::STATUS_THEME_MIGRATED).into();
        }

        let _guard = terminal::setup(self.tui_prefs.mouse_enabled)
            .context("échec initialisation terminal")?;
        let mut stdout = io::stdout();
        let backend = CrosstermBackend::new(&mut stdout);
        let mut term = Terminal::new(backend).context("création terminal ratatui")?;

        ui::boot_splash::play(&mut term, &self.state.palette)
            .context("animation démarrage")?;
        if self.tui_prefs.mouse_enabled {
            terminal::set_mouse_capture(true).context("capture souris")?;
        }

        let runtime = Arc::new(
            EngineRuntime::bootstrap(&self.config, Arc::clone(&self.ask))
                .await
                .context("bootstrap moteur")?,
        );
        self.runtime = Some(Arc::clone(&runtime));
        self.state.push_system(format!("Workspace : {}", runtime.workspace));
        if llm_configured {
            self.state.push_system(format!(
                "LLM : {} @ {}",
                self.config.model, self.config.server
            ));
        } else {
            self.state.push_system(
                "Serveur IA non configuré — Ctrl+Shift+L ou /server pour connecter Ollama et choisir un modèle.",
            );
        }
        self.state.push_system(format!(
            "Session : {} ({})",
            runtime.session_id(),
            runtime.transcript_path()
        ));
        self.hydrate_transcript_ui(true).await?;
        self.refresh_session_title().await;
        self.sync_terminal_title();
        self.refresh_status_snapshot().await;
        self.refresh_mcp_panel().await;
        self.state.status_notices = runtime.collect_startup_notices(&self.config);
        if !llm_configured {
            self.state.status_notices.insert(
                0,
                StatusNotice {
                    level: NoticeLevel::Warn,
                    text: "Serveur IA non configuré — Ctrl+Shift+L ou /server : adresse Ollama, test, choix du modèle."
                        .into(),
                },
            );
            self.state.status_line =
                "Ctrl+Shift+L — configurer le serveur IA (Ollama)".into();
        }
        self.load_skill_suggestions(&runtime).await;

        let prefs = crate::engine::preferences::load_preferences();
        if !llm_configured {
            self.open_ai_server_dialog();
            self.state
                .push_toast(crate::i18n::t(sk::TOAST_LLM_REQUIRED));
        } else if !prefs.onboarding_done {
            self.state.onboarding = Some(OnboardingDialog::default());
            self.state.phase = AppPhase::Onboarding;
        }

        loop {
            if self.pending_hydrate {
                self.pending_hydrate = false;
                self.hydrate_transcript_ui(true).await?;
                self.refresh_session_title().await;
            }
            if self.pending_title_refresh {
                self.pending_title_refresh = false;
                self.refresh_session_title().await;
            }
            if let Some(cmd) = self.pending_slash.take() {
                self.execute_slash(cmd).await?;
            }
            if self.status_tick == 0 {
                self.refresh_status_snapshot().await;
                if self.keybindings.reload_if_changed() {
                    self.state.status_line =
                        format!("Keybindings rechargés : {}", self.keybindings.path());
                }
            }
            self.status_tick = self.status_tick.wrapping_add(1);
            if self.status_tick >= 25 {
                self.status_tick = 0;
            }
            self.state.ui_frame_tick = self.state.ui_frame_tick.wrapping_add(1);
            self.state.tick_modal_anim();
            self.state.clear_expired_toast();
            if let Some(ref bash) = self.state.active_bash {
                self.state.status_line = bash_progress::status_hint(bash);
            }
            self.poll_agent_events();
            self.poll_ai_server_test();
            self.sync_prompt_modal();

            let model = if self.state.llm_configured {
                self.config.model.clone()
            } else {
                "IA non configurée".into()
            };
            let (model, workspace, permission_mode, plan_mode) = {
                let rt = self.runtime.as_ref().unwrap();
                (
                    model,
                    rt.workspace.to_string(),
                    rt.permission_mode().short_title().to_string(),
                    rt.plan_mode(),
                )
            };

            term.draw(|f| {
                ui::draw(
                    f,
                    &mut self.state,
                    self.history_search.as_ref(),
                    &self.status_snapshot,
                    &model,
                    &workspace,
                    &permission_mode,
                    plan_mode,
                    &self.vim,
                    &self.tui_prefs,
                )
            })
            .context("rendu frame TUI")?;

            if event::poll(Duration::from_millis(80)).context("poll événements")? {
                match event::read().context("lecture événement")? {
                    Event::Key(key) => {
                        // Windows émet Press + Release pour chaque touche ; ignorer Release.
                        if !matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
                            continue;
                        }
                        if self.handle_key(key)? {
                            break;
                        }
                    }
                    Event::Paste(text) => {
                        self.handle_paste(text);
                    }
                    Event::Resize(w, h) => {
                        term.resize(Rect::new(0, 0, w, h))
                            .context("resize terminal")?;
                    }
                    Event::Mouse(mouse) => {
                        self.handle_mouse(mouse);
                    }
                    _ => {}
                }
            }
        }

        self.flush_llm_preferences();
        Ok(())
    }

    /// Ecrit la connexion IA courante sur disque (quit ou filet de securite).
    fn flush_llm_preferences(&mut self) {
        if !self.state.llm_configured {
            return;
        }
        let Some(prefs) = llm_connection_from_config(&self.config) else {
            return;
        };
        match save_llm_connection(&prefs) {
            Ok(()) => {
                self.tui_prefs.llm_connection = Some(prefs);
                tracing::info!(path = %preferences_path(), "connexion IA persistee au quit");
            }
            Err(e) => {
                tracing::warn!(error = %e, "echec persistance connexion IA au quit");
            }
        }
    }

    fn poll_agent_events(&mut self) {
        let batch: Vec<_> = {
            let Some(rx) = self.agent_rx.as_mut() else {
                return;
            };
            let mut batch = Vec::new();
            while let Ok(event) = rx.try_recv() {
                batch.push(event);
            }
            batch
        };

        if !batch.is_empty() {
            self.status_tick = 0;
        }

        for event in batch {
            match event {
                Ok(ev) => {
                    let is_stop = matches!(ev, AgentEvent::Stop { .. });
                    if let AgentEvent::PhaseEnter { phase } = &ev {
                        self.state.active_phase = Some(*phase);
                    }
                    if let AgentEvent::ToolStart { id, name, arguments } = &ev {
                        if name == "bash" {
                            self.state.active_bash =
                                ActiveBashRun::from_tool_start(id.clone(), arguments);
                        }
                    }
                    if let AgentEvent::ToolProgress {
                        id,
                        name,
                        output,
                        total_lines,
                        ..
                    } = &ev
                    {
                        if name == "bash" {
                            if let Some(bash) = self.state.active_bash.as_mut() {
                                if bash.id == *id {
                                    bash.apply_progress(output, *total_lines);
                                }
                            }
                        }
                    }
                    if let AgentEvent::HookProgress {
                        tool_use_id,
                        hook_event,
                        in_progress,
                    } = &ev
                    {
                        self.state.hook_progress = ActiveHookProgress::from_event(
                            tool_use_id.clone(),
                            hook_event.clone(),
                            *in_progress,
                        );
                    }
                    apply_agent_event(
                        &mut self.state.entries,
                        &mut self.state.streaming,
                        &mut self.tool_names,
                        &ev,
                    );
                    if matches!(ev, AgentEvent::PhaseClose) {
                        self.state.active_phase = None;
                        self.state.auto_collapse_internal_reasoning();
                    }
                    if let AgentEvent::ToolFinish { id, output, is_error, .. } = &ev {
                        if self
                            .state
                            .active_bash
                            .as_ref()
                            .is_some_and(|b| b.id == *id)
                        {
                            self.state.active_bash = None;
                        }
                        if !is_error {
                            if let Some(name) = self.tool_names.get(id) {
                                match name.as_str() {
                                    "todo_write" => self.state.apply_todo_output(output),
                                    "course_plan_write" => {
                                        self.state.apply_course_plan_output(output)
                                    }
                                    "exit_plan_mode" => {
                                        if let (Some(accepted), Some(response)) = (
                                            output.get("accepted").and_then(Value::as_bool),
                                            output.get("user_response").and_then(Value::as_str),
                                        ) {
                                            self.state.push_entry(LogEntry::PlanApproval {
                                                accepted,
                                                response: response.to_string(),
                                            });
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    if let Some(rt) = self.runtime.as_ref() {
                        rt.record_ui_stats_event(&ev);
                    }
                    if self.auto_scroll {
                        self.state.scroll = 0;
                    }
                    if is_stop {
                        self.finish_run(RunStatus::Completed);
                    }
                }
                Err(e) => {
                    if let Some(view) = crate::view::api_error::ApiErrorView::from_engine(&e) {
                        self.state
                            .push_entry(crate::view::LogEntry::ApiError(view));
                    } else {
                        self.state.push_entry(crate::view::LogEntry::Error {
                            text: e.to_string(),
                        });
                    }
                    self.finish_run(RunStatus::Error);
                }
            }
        }
    }

    fn sync_prompt_modal(&mut self) {
        if self.state.prompt.is_some() || self.pending_reply.is_some() {
            return;
        }
        if !self.ask.has_queued() {
            self.state.permission_queue_waiting = 0;
            return;
        }
        let Some(pending) = self.ask.take_next() else {
            self.state.permission_queue_waiting = 0;
            return;
        };
        self.state.permission_queue_waiting = self.ask.queued_count();
        let file_preview = self
            .runtime
            .as_ref()
            .and_then(|rt| {
                compute_permission_preview(&rt.workspace, &pending.question.prompt)
                    .or_else(|| parse_plan_approval_preview(&pending.question.prompt))
            });
        let dialog = PromptDialog {
            question: pending.question.clone(),
            buffer: String::new(),
            choice_index: 0,
            file_preview,
            body_scroll: 0,
        };
        self.state.phase = AppPhase::Prompt;
        self.state.reset_modal_anim();
        self.state.pending_ask = true;
        self.state.prompt = Some(dialog);
        self.pending_reply = Some(pending);
        self.state.status_line = if self.state.permission_queue_waiting > 0 {
            crate::i18n::tf(
                crate::i18n::keys::MODAL_PERMISSION_QUEUE,
                &self.state.permission_queue_waiting.to_string(),
            )
        } else {
            crate::i18n::t(sk::STATUS_PERMISSION_WAITING).into()
        };
    }

    fn finish_run(&mut self, status: RunStatus) {
        match status {
            RunStatus::Cancelled => {
                if let Some(handle) = self.agent_handle.take() {
                    handle.abort();
                }
            }
            _ => {
                self.agent_handle.take();
            }
        }
        self.agent_rx = None;
        self.state.phase = AppPhase::Idle;
        self.state.streaming = None;
        self.state.active_bash = None;
        self.state.hook_progress = None;
        self.state.scroll_viewer = None;
        self.state.run_started = None;
        self.state.active_phase = None;
        self.state.last_run = status;
        self.state.status_line = match status {
            RunStatus::Completed => crate::i18n::t(sk::STATUS_AGENT_IDLE).into(),
            RunStatus::Cancelled => crate::i18n::t(sk::STATUS_RUN_ABORTED).into(),
            RunStatus::Error => crate::i18n::t(sk::STATUS_RUN_ERROR).into(),
            RunStatus::None => crate::i18n::t(sk::STATUS_AGENT_IDLE).into(),
        };
        self.drain_message_queue();
    }

    fn cancel_run(&mut self) {
        if self.state.phase != AppPhase::Running {
            return;
        }
        if let Some(handle) = self.agent_handle.take() {
            handle.abort();
        }
        self.agent_rx = None;
        if let Some(buf) = self.state.streaming.take() {
            if !buf.trim().is_empty() {
                self.state
                    .push_entry(crate::view::LogEntry::Assistant { text: buf });
            }
        }
        self.state.push_entry(crate::view::LogEntry::RunCancelled {
            reason: "Esc / Ctrl+C".into(),
        });
        self.finish_run(RunStatus::Cancelled);
    }

    fn drain_message_queue(&mut self) {
        self.state.queued_messages = self.message_queue.len();
        if self.state.phase != AppPhase::Idle || self.agent_rx.is_some() {
            return;
        }
        if let Some(next) = self.message_queue.pop_front() {
            self.state.queued_messages = self.message_queue.len();
            if let Err(e) = self.start_run(next) {
                self.state.push_system(format!("Erreur lancement file : {e}"));
            }
        }
    }

    fn enqueue_or_run(&mut self, raw: String) -> anyhow::Result<()> {
        if self.state.phase == AppPhase::Running {
            self.message_queue.push_back(raw);
            self.state.queued_messages = self.message_queue.len();
            self.state.push_system(format!(
                "Message en file ({}) — exécution après le run courant",
                self.state.queued_messages
            ));
            return Ok(());
        }
        self.start_run(raw)
    }

    fn start_run(&mut self, prompt: String) -> anyhow::Result<()> {
        if !self.state.llm_configured {
            self.state.push_system(
                "Serveur IA non configuré — Ctrl+Shift+L ou /server avant d'envoyer un message.",
            );
            self.open_ai_server_dialog();
            return Ok(());
        }
        let runtime = self
            .runtime
            .as_ref()
            .context("moteur non initialisé")?
            .clone();

        // Expansion paste + `@fichier` : le fil affiche le texte saisi, l'agent reçoit le contenu complet.
        let roots = runtime.working_directories();
        let prepared =
            crate::engine::prepare_user_prompt(&mut self.paste_store, &prompt, &roots, &runtime.drox_ignore);
        for note in &prepared.notes {
            self.state.push_system(note.clone());
        }
        let user_blocks = prepared.user_blocks;

        self.state.push_user(prompt);
        self.state.phase = AppPhase::Running;
        self.state.run_started = Some(Instant::now());
        self.state.active_phase = None;
        self.state.last_run = RunStatus::None;
        self.state.status_line = crate::i18n::t(sk::STATUS_AGENT_RUNNING).into();
        self.auto_scroll = true;
        self.state.scroll = 0;

        let (tx, rx) = mpsc::channel(64);
        self.agent_rx = Some(rx);
        let handle = runtime.spawn_run(user_blocks, Arc::clone(&self.ask), tx);
        self.agent_handle = Some(handle);
        Ok(())
    }

    fn new_session(&mut self) -> anyhow::Result<()> {
        if self.state.phase == AppPhase::Running {
            self.state
                .push_system(crate::i18n::t(sk::SLASH_MSG_RUN_BLOCKED));
            return Ok(());
        }
        let runtime = self.runtime.as_ref().context("moteur non initialisé")?;
        runtime.rotate_session();
        self.paste_store.clear();
        self.state.clear_transcript();
        self.state.push_system(format!(
            "Nouvelle session : {} ({})",
            runtime.session_id(),
            runtime.transcript_path()
        ));
        Ok(())
    }

    fn resume_session(&mut self, sid: String) -> anyhow::Result<()> {
        if self.state.phase == AppPhase::Running {
            self.state
                .push_system(crate::i18n::t(sk::SLASH_MSG_RUN_BLOCKED));
            return Ok(());
        }
        let runtime = self.runtime.as_ref().context("moteur non initialisé")?;
        runtime.resume_session(&sid)?;
        self.pending_hydrate = true;
        self.state.push_system(format!(
            "Session reprise : {} ({})",
            runtime.session_id(),
            runtime.transcript_path()
        ));
        Ok(())
    }

    async fn hydrate_transcript_ui(&mut self, announce_loaded: bool) -> anyhow::Result<()> {
        let runtime = self.runtime.as_ref().context("moteur non initialisé")?;
        let messages = runtime.load_history().await;
        if messages.is_empty() {
            self.state.clear_transcript();
            return Ok(());
        }
        let count = messages.len();
        let (entries, names) = messages_to_log_entries(&messages);
        self.state.clear_transcript();
        self.state.entries = entries;
        self.tool_names = names;
        self.auto_scroll = true;
        self.state.scroll = 0;
        if announce_loaded {
            self.state.push_system(format!("Historique chargé ({count} messages transcript)."));
        }
        self.state.rebuild_side_panels_from_entries();
        Ok(())
    }

    fn handle_rewind_key(&mut self, key: KeyEvent) -> bool {
        let Some(dialog) = self.state.rewind.as_mut() else {
            self.state.phase = AppPhase::Idle;
            return false;
        };
        match key.code {
            KeyCode::Esc => {
                self.state.rewind = None;
                self.state.phase = AppPhase::Idle;
                self.state.status_line = crate::i18n::t(sk::STATUS_REWIND_CANCELLED).into();
            }
            KeyCode::Up => {
                if dialog.cursor > 0 {
                    dialog.cursor -= 1;
                }
            }
            KeyCode::Down if dialog.cursor + 1 < dialog.choices.len() => {
                dialog.cursor += 1;
            }
            KeyCode::Enter if !dialog.choices.is_empty() => {
                let idx = dialog.choices[dialog.cursor].message_index;
                self.state.rewind = None;
                self.state.phase = AppPhase::Idle;
                self.pending_slash = Some(PendingSlash::RewindApply {
                    message_index: idx,
                });
            }
            _ => {}
        }
        false
    }

    fn handle_tool_output_viewer_key(&mut self, key: KeyEvent) -> bool {
        const PAGE: usize = 18;
        if self.keybindings.matches(BindingAction::ExpandBash, &key)
            || key.code == KeyCode::Esc
        {
            self.state.scroll_viewer = None;
            self.state.status_line = crate::i18n::t(sk::STATUS_VIEWER_CLOSED).into();
            return false;
        }
        let Some(viewer) = self.state.scroll_viewer.as_mut() else {
            return false;
        };
        if self.keybindings.matches(BindingAction::ScrollUp, &key) {
            viewer.scroll_page(PAGE, PAGE);
        } else if self.keybindings.matches(BindingAction::ScrollDown, &key) {
            viewer.scroll_page_down(PAGE, PAGE);
        }
        false
    }

    fn handle_copy_key(&mut self, key: KeyEvent) -> bool {
        let Some(dialog) = self.state.copy.as_mut() else {
            self.state.phase = AppPhase::Idle;
            return false;
        };
        match key.code {
            KeyCode::Esc => {
                self.state.copy = None;
                self.state.phase = AppPhase::Idle;
                self.state.status_line = crate::i18n::t(sk::STATUS_COPY_CANCELLED).into();
            }
            KeyCode::Up if dialog.cursor > 0 => {
                dialog.cursor -= 1;
            }
            KeyCode::Down if dialog.cursor + 1 < dialog.choices.len() => {
                dialog.cursor += 1;
            }
            KeyCode::Char('w') | KeyCode::Char('W') => {
                let choice = dialog.choices[dialog.cursor].clone();
                let full = dialog.full_text.clone();
                let blocks = dialog.blocks.clone();
                self.state.copy = None;
                self.state.phase = AppPhase::Idle;
                self.pending_slash = Some(PendingSlash::CopyApply {
                    full_text: full,
                    blocks,
                    choice,
                    clipboard: false,
                });
            }
            KeyCode::Enter if !dialog.choices.is_empty() => {
                let choice = dialog.choices[dialog.cursor].clone();
                let full = dialog.full_text.clone();
                let blocks = dialog.blocks.clone();
                self.state.copy = None;
                self.state.phase = AppPhase::Idle;
                self.pending_slash = Some(PendingSlash::CopyApply {
                    full_text: full,
                    blocks,
                    choice,
                    clipboard: true,
                });
            }
            _ => {}
        }
        false
    }

    fn handle_theme_key(&mut self, key: KeyEvent) -> bool {
        let Some(dialog) = self.state.theme_dialog.as_mut() else {
            self.state.phase = AppPhase::Idle;
            return false;
        };
        match key.code {
            KeyCode::Esc => {
                self.state.theme_dialog = None;
                self.state.phase = AppPhase::Idle;
                self.state.status_line = crate::i18n::t(sk::STATUS_THEME_CANCELLED).into();
            }
            KeyCode::Up if dialog.cursor > 0 => {
                dialog.cursor -= 1;
            }
            KeyCode::Down if dialog.cursor + 1 < TuiThemeSetting::ALL.len() => {
                dialog.cursor += 1;
            }
            KeyCode::Enter => {
                let theme = TuiThemeSetting::ALL[dialog.cursor];
                self.state.theme_dialog = None;
                self.state.phase = AppPhase::Idle;
                apply_theme(&mut self.state, theme);
                self.state
                    .push_system(format!("Thème : {}", theme.label()));
                self.state.status_line = crate::i18n::t(sk::STATUS_THEME_APPLIED).into();
            }
            _ => {}
        }
        false
    }

    fn open_settings_dialog(&mut self) {
        if self.state.phase == AppPhase::Running {
            self.state
                .push_system(crate::i18n::t(sk::STATUS_AGENT_BUSY));
            return;
        }
        self.tui_prefs = crate::engine::preferences::load_preferences();
        self.state.settings_dialog = Some(SettingsDialog { cursor: 0 });
        self.state.reset_modal_anim();
        self.state.phase = AppPhase::Settings;
    }

    fn close_settings_dialog(&mut self) {
        self.state.settings_dialog = None;
        self.state.phase = AppPhase::Idle;
    }

    fn handle_settings_key(&mut self, key: KeyEvent) -> bool {
        if self.state.settings_dialog.is_none() {
            self.state.phase = AppPhase::Idle;
            return false;
        }
        let row_count = SettingsRowKind::ALL.len();
        match key.code {
            KeyCode::Esc => self.close_settings_dialog(),
            KeyCode::Up => {
                if let Some(dialog) = self.state.settings_dialog.as_mut() {
                    dialog.cursor = dialog.cursor.saturating_sub(1);
                }
            }
            KeyCode::Down => {
                if let Some(dialog) = self.state.settings_dialog.as_mut() {
                    if dialog.cursor + 1 < row_count {
                        dialog.cursor += 1;
                    }
                }
            }
            KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Left | KeyCode::Right => {
                let cursor = self
                    .state
                    .settings_dialog
                    .as_ref()
                    .map(|d| d.cursor)
                    .unwrap_or(0);
                self.toggle_settings_row(SettingsRowKind::ALL[cursor]);
            }
            _ => {}
        }
        false
    }

    fn toggle_settings_row(&mut self, row: SettingsRowKind) {
        match row {
            SettingsRowKind::Language => {
                let next = match self.tui_prefs.ui_locale {
                    crate::i18n::UiLocale::Fr => crate::i18n::UiLocale::En,
                    crate::i18n::UiLocale::En => crate::i18n::UiLocale::Fr,
                };
                self.apply_ui_locale(next);
            }
            SettingsRowKind::Animations => {
                self.apply_animations_setting(!self.state.animations_enabled);
            }
            SettingsRowKind::Mouse => {
                self.apply_mouse_setting(!self.state.mouse_enabled);
            }
            SettingsRowKind::Vim => {
                self.apply_vim_setting(!self.tui_prefs.vim_enabled);
            }
            SettingsRowKind::Updates => {
                self.apply_update_enabled(!self.tui_prefs.update.enabled);
            }
        }
    }

    fn apply_ui_locale(&mut self, locale: crate::i18n::UiLocale) {
        crate::i18n::set_locale(locale);
        self.tui_prefs.ui_locale = locale;
        let mut prefs = crate::engine::preferences::load_preferences();
        prefs.ui_locale = locale;
        if let Err(e) = crate::engine::preferences::save_preferences(&prefs) {
            self.state.push_system(format!(
                "{} — persist: {e}",
                crate::i18n::tf2(
                    crate::i18n::keys::LANGUAGE_CHANGED,
                    locale.label(),
                    locale.code(),
                )
            ));
        } else {
            self.state.push_system(crate::i18n::tf2(
                crate::i18n::keys::LANGUAGE_CHANGED,
                locale.label(),
                locale.code(),
            ));
        }
    }

    fn apply_animations_setting(&mut self, enabled: bool) {
        self.state.animations_enabled = enabled;
        self.tui_prefs.animations_enabled = enabled;
        let mut prefs = crate::engine::preferences::load_preferences();
        prefs.animations_enabled = enabled;
        if let Err(e) = crate::engine::preferences::save_preferences(&prefs) {
            self.state.push_system(format!(
                "Animations {} (échec persistance : {e})",
                if enabled { "activées" } else { "désactivées" }
            ));
        } else {
            self.state.push_system(crate::i18n::t(if enabled {
                sk::STATUS_ANIMATIONS_ON
            } else {
                sk::STATUS_ANIMATIONS_OFF
            }));
        }
    }

    fn apply_vim_setting(&mut self, enabled: bool) {
        self.tui_prefs.vim_enabled = enabled;
        self.vim
            .set_enabled(enabled, &self.state.composer_buffer);
        let mut prefs = crate::engine::preferences::load_preferences();
        prefs.vim_enabled = enabled;
        if let Err(e) = crate::engine::preferences::save_preferences(&prefs) {
            self.state.push_system(format!("Préférences : {e}"));
        }
        let msg = if enabled {
            "Mode vim activé — Esc bascule INSERT/NORMAL · /vim pour désactiver"
        } else {
            "Mode vim désactivé — édition standard"
        };
        self.state.push_system(msg);
        self.state.status_line = msg.into();
    }

    fn apply_update_enabled(&mut self, enabled: bool) {
        self.tui_prefs.update.enabled = enabled;
        let mut prefs = crate::engine::preferences::load_preferences();
        prefs.update.enabled = enabled;
        if let Err(e) = crate::engine::preferences::save_preferences(&prefs) {
            self.state
                .push_system(format!("Mises à jour : échec persistance — {e}"));
        } else {
            self.state.push_system(crate::i18n::t(if enabled {
                crate::i18n::keys_update::UPDATE_ON
            } else {
                crate::i18n::keys_update::UPDATE_OFF
            }));
        }
    }

    fn push_update_status(&mut self) {
        for line in self.tui_prefs.update.format_status_lines() {
            self.state.push_system(line);
        }
    }

    fn apply_update_command(&mut self, cmd: crate::slash::UpdateCommand) {
        use crate::i18n::keys_update as u;
        use crate::slash::UpdateCommand;

        match cmd {
            UpdateCommand::ShowHelp => {
                self.state.push_system(crate::i18n::t(u::UPDATE_HELP_BODY));
                self.push_update_status();
            }
            UpdateCommand::Check => {
                self.state.push_system(crate::i18n::t(u::UPDATE_CHECK_STUB));
            }
            UpdateCommand::On => self.apply_update_enabled(true),
            UpdateCommand::Off => self.apply_update_enabled(false),
            UpdateCommand::Snooze { days } => {
                let until = (chrono::Utc::now() + chrono::Duration::days(days as i64))
                    .to_rfc3339();
                self.tui_prefs.update.snooze_until = Some(until.clone());
                let mut prefs = crate::engine::preferences::load_preferences();
                prefs.update.snooze_until = Some(until);
                let _ = crate::engine::preferences::save_preferences(&prefs);
                self.state
                    .push_system(crate::i18n::tf(u::UPDATE_SNOOZE, &days.to_string()));
            }
            UpdateCommand::Dismiss => {
                let version = env!("CARGO_PKG_VERSION").to_string();
                self.tui_prefs.update.dismissed_version = Some(version.clone());
                let mut prefs = crate::engine::preferences::load_preferences();
                prefs.update.dismissed_version = Some(version);
                let _ = crate::engine::preferences::save_preferences(&prefs);
                self.state.push_system(crate::i18n::t(u::UPDATE_DISMISS));
            }
            UpdateCommand::Install => {
                self.state.push_system(crate::i18n::t(u::UPDATE_INSTALL_STUB));
            }
        }
    }

    /// `true` = quitter l'application.
    fn handle_key(&mut self, key: KeyEvent) -> anyhow::Result<bool> {
        if self.keybindings.matches(BindingAction::Quit, &key) {
            return Ok(true);
        }

        if self.keybindings.matches(BindingAction::QuitConfirm, &key) {
            return self.handle_ctrl_c();
        }

        // Raccourcis globaux (modals) — actifs sauf pendant un run agent.
        if self.state.phase != AppPhase::Running {
            if self.keybindings.matches(BindingAction::AiServer, &key) {
                self.open_ai_server_dialog();
                return Ok(false);
            }
            if self.keybindings.matches(BindingAction::Workspace, &key) {
                self.open_workspace_dialog(None);
                return Ok(false);
            }
        }

        if self.state.phase == AppPhase::Prompt {
            return Ok(self.handle_prompt_key(key));
        }

        if self.state.phase == AppPhase::Rewind {
            return Ok(self.handle_rewind_key(key));
        }
        if self.state.phase == AppPhase::Copy {
            return Ok(self.handle_copy_key(key));
        }

        if self.state.phase == AppPhase::Theme {
            return Ok(self.handle_theme_key(key));
        }

        if self.state.phase == AppPhase::SlashPalette {
            return self.handle_slash_palette_key(key);
        }

        if self.state.phase == AppPhase::Onboarding {
            return Ok(self.handle_onboarding_key(key));
        }

        if self.state.phase == AppPhase::AiServer {
            return Ok(self.handle_ai_server_key(key));
        }

        if self.state.phase == AppPhase::Workspace {
            return Ok(self.handle_workspace_key(key));
        }

        if self.state.phase == AppPhase::Settings {
            return Ok(self.handle_settings_key(key));
        }

        if self.state.scroll_viewer.is_some() {
            return Ok(self.handle_tool_output_viewer_key(key));
        }

        if self.state.transcript_search.is_some() {
            return Ok(self.handle_transcript_search_key(key));
        }

        if self.history_search.is_some() {
            return Ok(self.handle_history_search_key(key));
        }

        // Typeahead `@fichier` — prioritaire sur historique ↑↓ quand des candidats existent.
        if key.code == KeyCode::Esc {
            if self.vim.enabled
                && self.vim.mode == VimMode::Insert
                && self.state.phase != AppPhase::Running
                && self.state.composer_mode != ComposerMode::Bash
            {
                self.apply_vim_key(&key);
                return Ok(false);
            }
            if self.state.composer_help {
                self.state.composer_help = false;
                return Ok(false);
            }
            if self.state.scroll_viewer.is_some() {
                self.state.scroll_viewer = None;
                return Ok(false);
            }
            if self.state.composer_suggestions.is_some() {
                self.state.composer_suggestions = None;
                return Ok(false);
            }
        }

        if self
            .state
            .composer_suggestions
            .as_ref()
            .is_some_and(|s| !s.items.is_empty())
        {
            match key.code {
                KeyCode::Tab => {
                    self.apply_composer_suggestion();
                    return Ok(false);
                }
                KeyCode::Up => {
                    if let Some(s) = self.state.composer_suggestions.as_mut() {
                        if s.cursor > 0 {
                            s.cursor -= 1;
                        }
                    }
                    return Ok(false);
                }
                KeyCode::Down => {
                    if let Some(s) = self.state.composer_suggestions.as_mut() {
                        if s.cursor + 1 < s.items.len() {
                            s.cursor += 1;
                        }
                    }
                    return Ok(false);
                }
                _ => {}
            }
        }

        if self.keybindings.matches(BindingAction::CancelRun, &key)
            && self.state.phase == AppPhase::Running
        {
            self.cancel_run();
            return Ok(false);
        }

        if self.keybindings.matches(BindingAction::TranscriptSearch, &key) {
            if self.state.phase != AppPhase::Running && self.history_search.is_none() {
                self.start_transcript_search();
            }
        } else if self.keybindings.matches(BindingAction::HistorySearch, &key) {
            if self.state.phase != AppPhase::Running
                && self.state.transcript_search.is_none()
                && self.state.prompt.is_none()
            {
                self.toggle_history_search();
            }
        } else if self.keybindings.matches(BindingAction::CancelRun, &key) {
            if self.state.composer_mode == ComposerMode::Bash {
                if self.state.composer_buffer.is_empty() {
                    self.state.composer_mode = ComposerMode::Normal;
                    self.state.status_line = crate::i18n::t(sk::STATUS_BASH_EXITED).into();
                } else {
                    self.state.composer_buffer.clear();
                }
                return Ok(false);
            }
            return Ok(true);
        } else if self.keybindings.matches(BindingAction::HistoryUp, &key)
            && self.state.phase != AppPhase::Running
            && self.state.composer_buffer.is_empty()
        {
            self.history_prev();
        } else if self.keybindings.matches(BindingAction::HistoryDown, &key)
            && self.state.phase != AppPhase::Running
            && self.state.composer_buffer.is_empty()
        {
            self.history_next();
        } else if self.vim.enabled
            && self.state.phase != AppPhase::Running
            && matches!(
                key.code,
                KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down
            )
        {
            self.apply_vim_key(&key);
        } else if self.keybindings.matches(BindingAction::ScrollUp, &key) {
            self.auto_scroll = false;
            self.state.scroll = self.state.scroll.saturating_add(3);
        } else if self.keybindings.matches(BindingAction::ScrollDown, &key) {
            self.state.scroll = self.state.scroll.saturating_sub(3);
            if self.state.scroll == 0 {
                self.auto_scroll = true;
            }
        } else if self.keybindings.matches(BindingAction::Multiline, &key)
            && self.state.phase != AppPhase::Running
        {
            self.state.composer_mode = ComposerMode::Multiline;
            if self.vim.enabled {
                self.vim.insert_text(&mut self.state.composer_buffer, "\n");
            } else {
                self.state.composer_buffer.push('\n');
            }
        } else if self.keybindings.matches(BindingAction::Submit, &key)
            && !key.modifiers.contains(KeyModifiers::SHIFT)
            && self.state.phase != AppPhase::Running
            && self.submit_composer()?
        {
            return Ok(true);
        } else if self.keybindings.matches(BindingAction::ExpandBash, &key)
            && self.state.phase != AppPhase::Running
            && self.state.composer_buffer.is_empty()
        {
            if let Some(kind) = self.state.toggle_latest_expandable() {
                self.auto_scroll = true;
                self.state.scroll = 0;
                self.state.status_line = crate::i18n::tf(sk::STATUS_DISPLAY_TOGGLED, kind);
            }
        } else if key.code == KeyCode::Char('y')
            && !key.modifiers.contains(KeyModifiers::CONTROL)
            && !self.vim.enabled
            && self.state.composer_buffer.is_empty()
            && self.state.phase == AppPhase::Idle
        {
            use crate::engine::copy_cmd::{collect_recent_assistant_texts, try_copy_clipboard};
            if let Some(text) = collect_recent_assistant_texts(&self.state.entries).first() {
                if try_copy_clipboard(text) {
                    self.state.push_toast(crate::i18n::t(sk::TOAST_ASSISTANT_COPIED));
                } else {
                    self.state
                        .push_system(crate::i18n::t(sk::SYSTEM_CLIPBOARD_UNAVAILABLE));
                }
            } else {
                self.state
                    .push_system(crate::i18n::t(sk::SYSTEM_COPY_NO_ASSISTANT));
            }
        } else if let Some(c) = typed_char(&key) {
            if self.state.phase != AppPhase::Running {
                if self.try_composer_special_char(c, &key)? {
                    return Ok(false);
                }
                if self.vim.enabled {
                    self.apply_vim_key(&key);
                    return Ok(false);
                }
                self.state.composer_buffer.push(c);
                self.refresh_composer_suggestions();
            }
        } else if key.code == KeyCode::Backspace && self.state.phase != AppPhase::Running {
            if self.vim.enabled {
                self.apply_vim_key(&key);
            } else {
                self.state.composer_buffer.pop();
                self.refresh_composer_suggestions();
            }
        }

        Ok(false)
    }

    fn handle_ctrl_c(&mut self) -> anyhow::Result<bool> {
        let now = Instant::now();
        if self
            .last_ctrl_c
            .is_some_and(|t| now.duration_since(t) < Duration::from_secs(2))
        {
            self.ctrl_c_streak += 1;
        } else {
            self.ctrl_c_streak = 1;
        }
        self.last_ctrl_c = Some(now);

        if self.state.phase == AppPhase::Running {
            self.cancel_run();
            self.state.status_line = crate::i18n::t(sk::STATUS_RUN_CANCELLED).into();
            return Ok(false);
        }

        if self.ctrl_c_streak >= 2 {
            return Ok(true);
        }
        self.state.status_line = crate::i18n::t(sk::STATUS_CTRL_C_QUIT).into();
        Ok(false)
    }

    fn submit_composer(&mut self) -> anyhow::Result<bool> {
        self.state.composer_suggestions = None;
        self.state.composer_help = false;
        let mode = self.state.composer_mode;
        let raw = self.state.composer_buffer.trim().to_string();
        if raw.is_empty() {
            if mode == ComposerMode::Bash {
                self.state.composer_mode = ComposerMode::Normal;
                self.state.status_line = crate::i18n::t(sk::STATUS_BASH_EXITED).into();
            }
            return Ok(false);
        }

        if mode == ComposerMode::Bash {
            self.state.composer_buffer.clear();
            self.state.composer_mode = ComposerMode::Normal;
            self.history_cursor = None;
            if self.input_history.last().map(String::as_str) != Some(raw.as_str()) {
                self.input_history.push(raw.clone());
            }
            self.pending_slash = Some(PendingSlash::BashExec { command: raw });
            return Ok(false);
        }

        if let Some(stripped) = raw.strip_prefix('!') {
            let cmd = stripped.trim();
            self.state.composer_buffer.clear();
            self.state.composer_mode = ComposerMode::Normal;
            self.history_cursor = None;
            if cmd.is_empty() {
                self.state.composer_mode = ComposerMode::Bash;
                self.state.status_line = crate::i18n::t(sk::STATUS_BASH_MODE).into();
                return Ok(false);
            }
            if self.input_history.last().map(String::as_str) != Some(cmd) {
                self.input_history.push(cmd.to_string());
            }
            self.pending_slash = Some(PendingSlash::BashExec {
                command: cmd.to_string(),
            });
            return Ok(false);
        }

        self.state.composer_buffer.clear();
        self.state.composer_mode = ComposerMode::Normal;
        self.vim.sync_cursor_end(&self.state.composer_buffer);
        self.history_cursor = None;
        if self.input_history.last().map(String::as_str) != Some(raw.as_str()) {
            self.input_history.push(raw.clone());
        }

        let runtime = self
            .runtime
            .as_ref()
            .context("moteur non initialisé")?
            .clone();

        let outcome = handle_slash(&raw, &mut self.state, &runtime);
        self.apply_slash_outcome(outcome, &runtime)
    }

    fn dispatch_slash_command(&mut self, raw: &str) -> anyhow::Result<bool> {
        let runtime = self
            .runtime
            .as_ref()
            .context("moteur non initialisé")?
            .clone();
        let outcome = handle_slash(raw, &mut self.state, &runtime);
        self.apply_slash_outcome(outcome, &runtime)
    }

    fn apply_slash_outcome(
        &mut self,
        outcome: SlashOutcome,
        runtime: &EngineRuntime,
    ) -> anyhow::Result<bool> {
        match outcome {
            SlashOutcome::Handled => {}
            SlashOutcome::Quit => return Ok(true),
            SlashOutcome::RunPrompt(prompt) => {
                self.enqueue_or_run(prompt)?;
            }
            SlashOutcome::NewSession => {
                self.new_session()?;
                self.pending_title_refresh = true;
            }
            SlashOutcome::ResumeSession(sid) => {
                self.resume_session(sid)?;
            }
            SlashOutcome::Compact => {
                self.pending_slash = Some(PendingSlash::Compact);
            }
            SlashOutcome::MemoryList { limit } => {
                self.pending_slash = Some(PendingSlash::MemoryList { limit });
            }
            SlashOutcome::MemoryRead { slug } => {
                self.pending_slash = Some(PendingSlash::MemoryRead { slug });
            }
            SlashOutcome::Permissions => {
                self.pending_slash = Some(PendingSlash::Permissions);
            }
            SlashOutcome::Plan { args } => {
                match handle_plan(&args, &mut self.state, runtime) {
                    SlashOutcome::RunPrompt(p) => {
                        self.enqueue_or_run(p)?;
                    }
                    SlashOutcome::Handled => {}
                    other => {
                        // handle_plan ne renvoie que Handled ou RunPrompt
                        let _ = other;
                    }
                }
            }
            SlashOutcome::Context => {
                self.pending_slash = Some(PendingSlash::Context);
            }
            SlashOutcome::Hooks { args } => {
                handle_hooks(&args, &mut self.state, runtime);
            }
            SlashOutcome::Config => {
                handle_config(&mut self.state, runtime);
            }
            SlashOutcome::Doctor => {
                self.pending_slash = Some(PendingSlash::Doctor);
            }
            SlashOutcome::Mcp { args } => {
                self.pending_slash = Some(PendingSlash::Mcp { args });
            }
            SlashOutcome::Skills { args } => {
                self.pending_slash = Some(PendingSlash::Skills { args });
            }
            SlashOutcome::Cost => {
                self.pending_slash = Some(PendingSlash::Cost);
            }
            SlashOutcome::Diff => {
                self.pending_slash = Some(PendingSlash::Diff);
            }
            SlashOutcome::Files => {
                self.pending_slash = Some(PendingSlash::Files);
            }
            SlashOutcome::Branch => {
                self.pending_slash = Some(PendingSlash::Branch);
            }
            SlashOutcome::Rewind => {
                self.pending_slash = Some(PendingSlash::Rewind);
            }
            SlashOutcome::Export { filename } => {
                self.pending_slash = Some(PendingSlash::Export { filename });
            }
            SlashOutcome::ThemePicker => {
                let cursor = TuiThemeSetting::ALL
                    .iter()
                    .position(|t| *t == self.state.theme)
                    .unwrap_or(0);
                self.state.theme_dialog = Some(ThemeDialog { cursor });
                self.state.reset_modal_anim();
                self.state.phase = AppPhase::Theme;
                self.state.status_line = crate::i18n::t(sk::STATUS_THEME_PICKER).into();
            }
            SlashOutcome::Keybindings { args } => {
                self.pending_slash = Some(PendingSlash::Keybindings { args });
            }
            SlashOutcome::TerminalSetup => {
                use crate::engine::keybindings_cmd::init_keybindings_file;
                use crate::engine::terminal_setup_cmd::format_terminal_setup_lines;
                let init_ok = init_keybindings_file().is_ok();
                for line in format_terminal_setup_lines(init_ok) {
                    self.state.push_system(line);
                }
            }
            SlashOutcome::Init => {
                self.pending_slash = Some(PendingSlash::Init);
            }
            SlashOutcome::Rename { title } => {
                self.pending_slash = Some(PendingSlash::Rename { title });
            }
            SlashOutcome::Copy { age } => {
                self.pending_slash = Some(PendingSlash::Copy { age });
            }
            SlashOutcome::MemorySearch { query, limit } => {
                self.pending_slash = Some(PendingSlash::MemorySearch { query, limit });
            }
            SlashOutcome::Statusline => {
                self.pending_slash = Some(PendingSlash::Statusline);
            }
            SlashOutcome::Settings => {
                self.open_settings_dialog();
            }
            SlashOutcome::SettingsAnimations { enabled } => {
                self.apply_animations_setting(enabled);
            }
            SlashOutcome::SettingsMouse { enabled } => {
                self.apply_mouse_setting(enabled);
            }
            SlashOutcome::Onboarding => {
                self.state.onboarding = Some(OnboardingDialog::default());
                self.state.reset_modal_anim();
                self.state.phase = AppPhase::Onboarding;
            }
            SlashOutcome::SettingsLocale { locale } => {
                self.apply_ui_locale(locale);
            }
            SlashOutcome::AiServer => {
                self.open_ai_server_dialog();
            }
            SlashOutcome::Workspace { initial } => {
                self.open_workspace_dialog(initial);
            }
            SlashOutcome::ToggleVim => {
                self.apply_vim_setting(!self.tui_prefs.vim_enabled);
            }
            SlashOutcome::Update(cmd) => {
                self.apply_update_command(cmd);
            }
        }
        Ok(false)
    }

    async fn execute_slash(&mut self, cmd: PendingSlash) -> anyhow::Result<()> {
        if self.state.phase == AppPhase::Running {
            self.state
                .push_system(crate::i18n::t(sk::STATUS_AGENT_BUSY));
            return Ok(());
        }
        let runtime = self.runtime.as_ref().context("moteur non initialisé")?;
        match cmd {
            PendingSlash::Compact => {
                self.state.status_line = crate::i18n::t(sk::STATUS_COMPACTION_RUNNING).into();
                match runtime.compact_current_transcript().await {
                    Ok(CompactOutcome::Live(report)) => {
                        self.hydrate_transcript_ui(false).await?;
                        self.state.push_entry(LogEntry::ContextCompacted {
                            tokens_before: report.tokens_before,
                            tokens_after: report.tokens_after,
                            messages_removed: report.messages_removed,
                        });
                        let preview = compact_checkpoint_preview(&report.summary_text);
                        if !preview.is_empty() {
                            self.state.push_system(preview);
                        }
                        self.state.status_line = crate::i18n::t(sk::STATUS_COMPACTION_DONE).into();
                    }
                    Ok(CompactOutcome::Preview(result)) => {
                        self.state.push_system(format!(
                            "Aperçu compaction (historique trop court pour réécriture live) — objectif : {}",
                            result.objective
                        ));
                        let summary = if result.summary.len() > 1200 {
                            format!("{}…", &result.summary[..1200])
                        } else {
                            result.summary.clone()
                        };
                        self.state.push_system(summary);
                        self.state.status_line = crate::i18n::t(sk::STATUS_COMPACTION_PREVIEW).into();
                    }
                    Err(e) => {
                        self.state.push_system(format!("Compaction : {e:#}"));
                        self.state.status_line = crate::i18n::t(sk::STATUS_COMPACTION_FAILED).into();
                    }
                }
            }
            PendingSlash::MemoryList { limit } => {
                match runtime.list_archived_memory(limit).await {
                    Ok(lines) if lines.is_empty() => {
                        self.state.push_system(
                            "Aucune session archivée (.drox/memory/sessions/).",
                        );
                    }
                    Ok(lines) => {
                        self.state
                            .push_system(format!("Mémoire workspace ({limit} max) :"));
                        for line in lines {
                            self.state.push_system(line);
                        }
                    }
                    Err(e) => self.state.push_system(format!("Mémoire : {e:#}")),
                }
            }
            PendingSlash::MemoryRead { slug } => match runtime.read_archived_memory(&slug).await {
                Ok(body) => {
                    self.state.push_system(format!("── mémoire [{slug}] ──"));
                    for line in body.lines() {
                        self.state.push_system(line.to_string());
                    }
                }
                Err(e) => self.state.push_system(format!("Mémoire [{slug}] : {e:#}")),
            },
            PendingSlash::MemorySearch { query, limit } => {
                match runtime.search_archived_memory(&query, limit).await {
                    Ok(lines) if lines.is_empty() => {
                        self.state.push_system(format!(
                            "Aucun résultat mémoire pour « {query} » (.drox/memory/sessions/)."
                        ));
                    }
                    Ok(lines) => {
                        self.state
                            .push_system(format!("Recherche mémoire « {query} » :"));
                        for line in lines {
                            self.state.push_system(line);
                        }
                        self.state
                            .push_system("Lire en entier : /memory <slug>");
                    }
                    Err(e) => self.state.push_system(format!("Recherche mémoire : {e:#}")),
                }
            }
            PendingSlash::ApplyAiServer { index } => {
                if let Err(e) = self.apply_ai_server_model(index).await {
                    self.state.push_system(format!("Connexion IA : {e:#}"));
                    self.state.status_line = crate::i18n::t(sk::STATUS_LLM_SAVE_FAILED).into();
                }
            }
            PendingSlash::ApplyWorkspace { path } => {
                if let Err(e) = self.apply_workspace_switch(path).await {
                    self.state.push_system(format!("Workspace : {e:#}"));
                    self.state.status_line = crate::i18n::t(sk::STATUS_WORKSPACE_CHANGE_FAILED).into();
                }
            }
            PendingSlash::Statusline => {
                for line in crate::engine::format_statusline_lines(runtime, &self.status_snapshot)
                {
                    self.state.push_system(line);
                }
            }
            PendingSlash::Permissions => {
                for line in runtime.format_permissions_lines() {
                    self.state.push_system(line);
                }
            }
            PendingSlash::Context => {
                for line in runtime.format_context_lines().await {
                    self.state.push_system(line);
                }
            }
            PendingSlash::Doctor => {
                self.state.status_line = crate::i18n::t(sk::STATUS_DOCTOR_RUNNING).into();
                for line in runtime.run_doctor_checks().await {
                    self.state.push_system(line);
                }
                self.state.status_line = crate::i18n::t(sk::STATUS_DOCTOR_DONE).into();
            }
            PendingSlash::Mcp { args } => {
                self.state.status_line = crate::i18n::t(sk::STATUS_MCP_RUNNING).into();
                for line in runtime.run_mcp_command(&args).await {
                    self.state.push_system(line);
                }
                self.refresh_mcp_panel().await;
                self.state.status_line.clear();
            }
            PendingSlash::Skills { args } => {
                for line in runtime.run_skills_command(&args).await {
                    self.state.push_system(line);
                }
            }
            PendingSlash::Cost => {
                for line in runtime.format_cost_lines().await {
                    self.state.push_system(line);
                }
            }
            PendingSlash::Diff => {
                for line in runtime.format_git_diff_lines().await {
                    self.state.push_system(line);
                }
            }
            PendingSlash::Files => {
                for line in runtime.format_context_files_lines(&self.state.entries) {
                    self.state.push_system(line);
                }
            }
            PendingSlash::Branch => {
                for line in runtime.format_branch_lines() {
                    self.state.push_system(line);
                }
            }
            PendingSlash::Init => {
                for line in runtime.format_init_status_lines() {
                    self.state.push_system(line);
                }
            }
            PendingSlash::Rewind => match runtime.list_rewind_choices().await {
                Ok(choices) if choices.is_empty() => {
                    self.state
                        .push_system("Aucun message utilisateur — rien à rembobiner.");
                }
                Ok(choices) => {
                    let cursor = choices.len().saturating_sub(1);
                    self.state.rewind = Some(RewindDialog {
                        choices: choices
                            .into_iter()
                            .map(|c| RewindChoiceView {
                                message_index: c.message_index,
                                label: c.label,
                                restore_text: c.restore_text,
                            })
                            .collect(),
                        cursor,
                    });
                    self.state.phase = AppPhase::Rewind;
                    self.state.status_line = crate::i18n::t(sk::STATUS_REWIND_PICKER).into();
                }
                Err(e) => self.state.push_system(format!("Rewind : {e:#}")),
            },
            PendingSlash::RewindApply { message_index } => {
                match runtime.rewind_to_message_index(message_index).await {
                    Ok(report) => {
                        self.hydrate_transcript_ui(false).await?;
                        self.state.push_system(format!(
                            "Rembobiné — {} message(s) conservé(s), {} supprimé(s).",
                            report.messages_kept, report.messages_removed
                        ));
                        if !report.restore_text.is_empty() {
                            self.state.composer_buffer = report.restore_text;
                        }
                        self.state.status_line = crate::i18n::t(sk::STATUS_TRANSCRIPT_REWOUND).into();
                    }
                    Err(e) => self.state.push_system(format!("Rewind : {e:#}")),
                }
            }
            PendingSlash::Export { filename } => {
                match runtime.export_transcript(&filename).await {
                    Ok(report) => {
                        self.state.push_system(format!(
                            "Conversation exportée : {} ({} octets, {} messages)",
                            report.path, report.bytes, report.message_count
                        ));
                        self.state.status_line = crate::i18n::t(sk::STATUS_EXPORT_DONE).into();
                    }
                    Err(e) => self.state.push_system(format!("Export : {e:#}")),
                }
            }
            PendingSlash::Keybindings { args } => {
                use crate::engine::keybindings_cmd::{
                    format_builtin_keybindings_lines, init_keybindings_file,
                };
                let args_trim = args.trim();
                if args_trim.eq_ignore_ascii_case("reload") {
                    match self.keybindings.reload() {
                        Ok(()) => {
                            self.state.push_system(format!(
                                "Keybindings rechargés : {}",
                                self.keybindings.path()
                            ));
                        }
                        Err(e) => self.state.push_system(format!("Keybindings reload : {e:#}")),
                    }
                } else if args_trim.eq_ignore_ascii_case("init") {
                    match init_keybindings_file() {
                        Ok((created, path)) => {
                            let msg = if created {
                                format!("Template keybindings créé : {path}")
                            } else {
                                format!("Keybindings existant (non écrasé) : {path}")
                            };
                            self.state.push_system(msg);
                            let _ = self.keybindings.reload();
                        }
                        Err(e) => self.state.push_system(format!("Keybindings init : {e:#}")),
                    }
                }
                for line in format_builtin_keybindings_lines() {
                    self.state.push_system(line);
                }
            }
            PendingSlash::Rename { title } => {
                use crate::engine::rename_cmd::rename_session;
                match rename_session(
                    &runtime.sessions_dir,
                    &runtime.session_id(),
                    &runtime.transcript_path(),
                    title.as_deref(),
                )
                .await
                {
                    Ok(new_title) => {
                        self.state.session_title = new_title.clone();
                        self.sync_terminal_title();
                        self.state
                            .push_system(format!("Session renommée : {new_title}"));
                        self.state.status_line = crate::i18n::t(sk::STATUS_SESSION_RENAMED).into();
                    }
                    Err(e) => self.state.push_system(format!("Rename : {e:#}")),
                }
            }
            PendingSlash::Copy { age } => {
                use crate::engine::copy_cmd::{prepare_copy, CopyPlan};
                match prepare_copy(
                    &self.state.entries,
                    age,
                    self.tui_prefs.copy_full_response,
                ) {
                    CopyPlan::Immediate { text, filename } => {
                        match crate::engine::copy_cmd::copy_text(&text, &filename).await {
                            Ok(msg) => {
                                self.state.push_system(msg);
                                self.state.status_line = crate::i18n::t(sk::STATUS_COPY_DONE).into();
                            }
                            Err(e) => self.state.push_system(format!("Copy : {e:#}")),
                        }
                    }
                    CopyPlan::Picker { full_text, blocks } => {
                        self.state.copy = Some(CopyDialog::new(full_text, blocks));
                        self.state.phase = AppPhase::Copy;
                        self.state.status_line = crate::i18n::t(sk::STATUS_COPY_PICKER).into();
                    }
                    CopyPlan::Error(msg) => self.state.push_system(msg),
                }
            }
            PendingSlash::CopyApply {
                full_text,
                blocks,
                choice,
                clipboard,
            } => {
                use crate::engine::copy_cmd::{selection_content, CopyChoice};
                use crate::engine::preferences::{load_preferences, save_preferences};
                if matches!(choice, CopyChoice::AlwaysFullResponse) {
                    let mut prefs = load_preferences();
                    prefs.copy_full_response = true;
                    if save_preferences(&prefs).is_ok() {
                        self.tui_prefs = prefs;
                    }
                }
                let (text, filename) = selection_content(&full_text, &blocks, &choice);
                let result = if clipboard {
                    crate::engine::copy_cmd::copy_text(&text, &filename).await
                } else {
                    crate::engine::copy_cmd::write_text_file(&text, &filename).await
                };
                match result {
                    Ok(msg) => {
                        let suffix = if matches!(choice, CopyChoice::AlwaysFullResponse) {
                            "\nPréférence enregistrée (copie complète par défaut)."
                        } else {
                            ""
                        };
                        self.state.push_system(format!("{msg}{suffix}"));
                        if clipboard {
                            self.state.push_toast(crate::i18n::t(sk::TOAST_CLIPBOARD));
                        }
                        self.state.status_line = crate::i18n::t(sk::STATUS_COPY_DONE).into();
                    }
                    Err(e) => self.state.push_system(format!("Copy : {e:#}")),
                }
            }
            PendingSlash::BashExec { command } => {
                if runtime.plan_mode() {
                    self.state
                        .push_system("Mode plan actif — bash intégré désactivé.");
                    return Ok(());
                }
                use crate::engine::bash_mode::BashModeProgressSink;

                self.state
                    .push_entry(LogEntry::BashModeInput { command: command.clone() });
                let (sink, mut prx) = BashModeProgressSink::channel();
                self.state.active_bash = Some(ActiveBashRun::from_user_command(&command));
                self.state.status_line = format!("! {command}");

                let ask = Arc::clone(&self.ask);
                let cmd = command.clone();
                let rt = Arc::clone(runtime);
                let progress = Arc::new(sink);
                let handle = tokio::spawn(async move {
                    rt.execute_user_bash(ask, &cmd, Some(progress)).await
                });

                while !handle.is_finished() {
                    while let Ok((out, lines)) = prx.try_recv() {
                        if let Some(b) = self.state.active_bash.as_mut() {
                            b.apply_progress(&out, lines);
                        }
                    }
                    tokio::time::sleep(Duration::from_millis(40)).await;
                }
                while let Ok((out, lines)) = prx.try_recv() {
                    if let Some(b) = self.state.active_bash.as_mut() {
                        b.apply_progress(&out, lines);
                    }
                }

                let output_id = self
                    .state
                    .active_bash
                    .as_ref()
                    .map(|b| b.id.clone())
                    .unwrap_or_else(ToolUseId::new);
                self.state.active_bash = None;

                match handle.await.context("tâche bash mode")? {
                    Ok(output) => {
                        let exit = output
                            .get("exit_code")
                            .and_then(Value::as_i64)
                            .unwrap_or(0);
                        let timed_out = output
                            .get("timed_out")
                            .and_then(Value::as_bool)
                            .unwrap_or(false);
                        let is_error = exit != 0 || timed_out;
                        self.state.push_entry(LogEntry::BashModeOutput {
                            id: output_id,
                            output,
                            is_error,
                        });
                        self.state.status_line = if is_error {
                            "bash terminé (erreur)".into()
                        } else {
                            "bash terminé".into()
                        };
                    }
                    Err(e) => {
                        self.state
                            .push_entry(LogEntry::Error { text: format!("bash : {e}") });
                        self.state.status_line = crate::i18n::t(sk::STATUS_BASH_ERROR).into();
                    }
                }
            }
        }
        Ok(())
    }

    async fn refresh_session_title(&mut self) {
        let Some(runtime) = self.runtime.as_ref() else {
            return;
        };
        self.state.session_title = crate::engine::rename_cmd::load_display_title(
            &runtime.sessions_dir,
            &runtime.session_id(),
        )
        .await;
        self.sync_terminal_title();
    }

    fn sync_terminal_title(&self) {
        if !self.tui_prefs.terminal_title_from_rename {
            return;
        }
        let title = if self.state.session_title.is_empty() {
            "Drox".to_string()
        } else {
            self.state.session_title.clone()
        };
        set_terminal_title(&title);
    }

    fn handle_prompt_key(&mut self, key: KeyEvent) -> bool {
        enum PromptAction {
            Skip,
            Answer(UserAnswer),
        }

        let action = {
            let Some(dialog) = self.state.prompt.as_mut() else {
                return false;
            };

            match key.code {
                KeyCode::Esc => Some(PromptAction::Skip),
                KeyCode::Up if !dialog.question.choices.is_empty() => {
                    dialog.choice_index = dialog.choice_index.saturating_sub(1);
                    None
                }
                KeyCode::Down if !dialog.question.choices.is_empty() => {
                    let max = dialog.question.choices.len().saturating_sub(1);
                    dialog.choice_index = (dialog.choice_index + 1).min(max);
                    None
                }
                KeyCode::PageUp => {
                    dialog.body_scroll = dialog.body_scroll.saturating_sub(3);
                    None
                }
                KeyCode::PageDown => {
                    dialog.body_scroll = dialog.body_scroll.saturating_add(3);
                    None
                }
                KeyCode::Char(c)
                    if dialog.question.choices.is_empty() || dialog.question.allow_free_text =>
                {
                    dialog.buffer.push(c);
                    None
                }
                KeyCode::Backspace => {
                    dialog.buffer.pop();
                    None
                }
                KeyCode::Char('y') | KeyCode::Char('Y')
                    if dialog.question.choices.len() == 2
                        && dialog.question.choices[0].eq_ignore_ascii_case("yes") =>
                {
                    Some(PromptAction::Answer(parse_answer(&dialog.question, "1".into())))
                }
                KeyCode::Char('n') | KeyCode::Char('N')
                    if dialog.question.choices.len() == 2
                        && dialog.question.choices[0].eq_ignore_ascii_case("yes") =>
                {
                    Some(PromptAction::Answer(parse_answer(&dialog.question, "2".into())))
                }
                KeyCode::Enter => {
                    let question = dialog.question.clone();
                    let answer = if question.choices.is_empty() {
                        parse_answer(&question, dialog.buffer.clone())
                    } else if !dialog.buffer.is_empty() {
                        parse_answer(&question, dialog.buffer.clone())
                    } else {
                        parse_answer(&question, (dialog.choice_index + 1).to_string())
                    };
                    Some(PromptAction::Answer(answer))
                }
                _ => None,
            }
        };

        match action {
            Some(PromptAction::Skip) => self.complete_prompt(UserAnswer {
                id: self
                    .state
                    .prompt
                    .as_ref()
                    .and_then(|d| d.question.id.clone()),
                text: String::new(),
                indices: Vec::new(),
                skipped: true,
            }),
            Some(PromptAction::Answer(answer)) => self.complete_prompt(answer),
            None => {}
        }
        false
    }

    fn complete_prompt(&mut self, answer: UserAnswer) {
        if let Some(pending) = self.pending_reply.take() {
            pending.complete(answer);
        }
        self.state.prompt = None;
        self.state.pending_ask = false;

        if self.ask.has_queued() {
            self.sync_prompt_modal();
            return;
        }

        self.state.permission_queue_waiting = 0;
        if self.state.phase == AppPhase::Prompt {
            self.state.phase = if self.agent_rx.is_some() {
                AppPhase::Running
            } else {
                AppPhase::Idle
            };
        }
        self.state.status_line = if self.agent_rx.is_some() {
            crate::i18n::t(sk::STATUS_AGENT_RUNNING).into()
        } else {
            crate::i18n::t(sk::STATUS_AGENT_IDLE).into()
        };
    }

    fn history_prev(&mut self) {
        if self.input_history.is_empty() {
            return;
        }
        let len = self.input_history.len();
        let idx = match self.history_cursor {
            Some(i) => i.saturating_sub(1),
            None => len - 1,
        };
        self.history_cursor = Some(idx);
        self.state.composer_buffer = self.input_history[idx].clone();
        self.vim.sync_cursor_end(&self.state.composer_buffer);
    }

    fn history_next(&mut self) {
        let Some(cur) = self.history_cursor else {
            return;
        };
        if cur + 1 >= self.input_history.len() {
            self.history_cursor = None;
            self.state.composer_buffer.clear();
            self.vim.sync_cursor_end(&self.state.composer_buffer);
        } else {
            let next = cur + 1;
            self.history_cursor = Some(next);
            self.state.composer_buffer = self.input_history[next].clone();
            self.vim.sync_cursor_end(&self.state.composer_buffer);
        }
    }

    fn start_transcript_search(&mut self) {
        let lines = self.state.flattened_log_lines();
        let query = self.last_transcript_query.clone();
        let match_lines = find_matching_line_indices(&lines, &query);
        let search = TranscriptSearchState::new(query, match_lines);
        if let Some(line_idx) = search.current_line_index() {
            let inner_h = 20usize;
            self.auto_scroll = false;
            self.state.scroll = scroll_to_line(lines.len(), inner_h, line_idx);
        }
        self.state.transcript_search = Some(search);
        self.state.status_line = crate::i18n::t(sk::STATUS_TRANSCRIPT_SEARCH).into();
    }

    fn refresh_transcript_search(&mut self) {
        let query = self
            .state
            .transcript_search
            .as_ref()
            .map(|s| s.query.clone())
            .unwrap_or_default();
        let lines = self.state.flattened_log_lines();
        let match_lines = find_matching_line_indices(&lines, &query);
        let Some(search) = self.state.transcript_search.as_mut() else {
            return;
        };
        search.match_lines = match_lines;
        search.current = 0;
        if let Some(line_idx) = search.current_line_index() {
            self.auto_scroll = false;
            self.state.scroll = scroll_to_line(lines.len(), 20, line_idx);
        }
    }

    fn focus_transcript_match(&mut self) {
        let Some(search) = self.state.transcript_search.as_ref() else {
            return;
        };
        let Some(line_idx) = search.current_line_index() else {
            return;
        };
        let total = self.state.flattened_log_lines().len();
        self.auto_scroll = false;
        self.state.scroll = scroll_to_line(total, 20, line_idx);
    }

    fn close_transcript_search(&mut self, keep_query: bool) {
        if let Some(search) = self.state.transcript_search.take() {
            if keep_query {
                self.last_transcript_query = search.query;
            }
        }
        self.state.status_line = crate::i18n::t(sk::STATUS_SEARCH_CLOSED).into();
    }

    fn handle_transcript_search_key(&mut self, key: KeyEvent) -> bool {
        if self.keybindings.matches(BindingAction::CancelRun, &key) {
            self.close_transcript_search(false);
        } else if self.keybindings.matches(BindingAction::Submit, &key)
            && !key.modifiers.contains(KeyModifiers::SHIFT)
        {
            self.close_transcript_search(true);
        } else if self.keybindings.matches(BindingAction::SearchNext, &key) {
            if let Some(search) = self.state.transcript_search.as_mut() {
                search.next_match();
            }
            self.focus_transcript_match();
        } else if self.keybindings.matches(BindingAction::SearchPrev, &key) {
            if let Some(search) = self.state.transcript_search.as_mut() {
                search.prev_match();
            }
            self.focus_transcript_match();
        } else if key.code == KeyCode::Backspace {
            if let Some(search) = self.state.transcript_search.as_mut() {
                search.query.pop();
                self.refresh_transcript_search();
            }
        } else if let KeyCode::Char(c) = key.code {
            if !key.modifiers.contains(KeyModifiers::CONTROL) {
                if let Some(search) = self.state.transcript_search.as_mut() {
                    search.query.push(c);
                    self.refresh_transcript_search();
                }
            }
        }
        false
    }

    fn toggle_history_search(&mut self) {
        self.history_search = Some(crate::view::HistorySearchState {
            query: String::new(),
            saved_buffer: self.state.composer_buffer.clone(),
            saved_history_cursor: self.history_cursor,
            match_indices: Vec::new(),
            current: 0,
        });
        self.state.status_line = crate::i18n::t(sk::STATUS_HISTORY_SEARCH).into();
    }

    fn refresh_history_search(&mut self) {
        let Some(search) = self.history_search.as_mut() else {
            return;
        };
        search.match_indices = find_history_match_indices(&self.input_history, &search.query);
        search.current = 0;
        if let Some(idx) = search.current_history_index() {
            self.state.composer_buffer = self.input_history[idx].clone();
            self.history_cursor = Some(idx);
        } else if search.query.is_empty() {
            self.state.composer_buffer = search.saved_buffer.clone();
            self.history_cursor = search.saved_history_cursor;
        }
    }

    fn handle_history_search_key(&mut self, key: KeyEvent) -> bool {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Esc => {
                if let Some(search) = self.history_search.take() {
                    self.state.composer_buffer = search.saved_buffer;
                    self.history_cursor = search.saved_history_cursor;
                }
                self.state.status_line = crate::i18n::t(sk::STATUS_HISTORY_CANCELLED).into();
            }
            KeyCode::Enter => {
                self.history_search = None;
                self.state.status_line = crate::i18n::t(sk::STATUS_HISTORY_ACCEPTED).into();
            }
            KeyCode::Char('r') if ctrl => {
                if let Some(search) = self.history_search.as_mut() {
                    search.next_match();
                    if let Some(idx) = search.current_history_index() {
                        self.state.composer_buffer = self.input_history[idx].clone();
                        self.history_cursor = Some(idx);
                    }
                }
            }
            KeyCode::Backspace => {
                if let Some(search) = self.history_search.as_mut() {
                    if search.query.is_empty() {
                        if let Some(saved) = self.history_search.take() {
                            self.state.composer_buffer = saved.saved_buffer;
                            self.history_cursor = saved.saved_history_cursor;
                            self.state.status_line = crate::i18n::t(sk::STATUS_HISTORY_CANCELLED).into();
                        }
                    } else {
                        search.query.pop();
                        self.refresh_history_search();
                    }
                }
            }
            KeyCode::Char(c) if !ctrl => {
                if let Some(search) = self.history_search.as_mut() {
                    search.query.push(c);
                    self.refresh_history_search();
                }
            }
            _ => {}
        }
        false
    }

    async fn refresh_status_snapshot(&mut self) {
        let Some(runtime) = self.runtime.as_ref() else {
            return;
        };
        self.status_snapshot = runtime
            .build_status_snapshot(self.session_started)
            .await;
    }

    async fn refresh_mcp_panel(&mut self) {
        let Some(runtime) = self.runtime.as_ref() else {
            return;
        };
        self.state.mcp_snapshot = runtime.build_mcp_panel_snapshot().await;
    }

    fn open_ai_server_dialog(&mut self) {
        if self.state.phase == AppPhase::Running {
            self.state
                .push_system(crate::i18n::t(sk::STATUS_AGENT_BUSY));
            return;
        }
        let saved = self.tui_prefs.llm_connection.as_ref();
        let server = saved
            .map(|c| c.server.clone())
            .unwrap_or_else(|| self.config.server.clone());
        let api_key = saved
            .and_then(|c| c.api_key.clone())
            .or_else(|| self.config.api_key.clone())
            .unwrap_or_default();
        let num_ctx = saved.map(|c| c.num_ctx).unwrap_or(self.config.num_ctx);
        let max_iterations = saved
            .map(|c| c.max_iterations)
            .unwrap_or(self.config.max_iterations);
        let active_profile = self
            .tui_prefs
            .connection_library
            .active_profile()
            .cloned();
        let current_model = saved
            .map(|c| c.model.clone())
            .unwrap_or_else(|| self.config.model.clone());
        let dialog = AiServerDialog::from_saved(
            server,
            api_key,
            current_model,
            num_ctx,
            max_iterations,
            active_profile.as_ref(),
        );
        let needs_model_refresh = dialog.step == AiServerStep::SelectModel && dialog.models.is_empty();
        self.state.ai_server = Some(dialog);
        self.state.reset_modal_anim();
        self.state.phase = AppPhase::AiServer;
        self.state.status_line = crate::i18n::t(sk::STATUS_SERVER_DIALOG).into();
        if needs_model_refresh {
            self.start_ai_server_test();
        }
    }

    fn close_ai_server_dialog(&mut self) {
        let should_persist = self
            .state
            .ai_server
            .as_ref()
            .is_some_and(|d| !d.models.is_empty());
        if should_persist {
            if let Err(e) = self.persist_ai_server_dialog_draft() {
                tracing::warn!(error = %e, "echec persistance brouillon /server");
            } else {
                tracing::info!("brouillon connexion IA persistee a la fermeture /server");
            }
        }
        self.state.ai_server = None;
        self.ai_server_test_rx = None;
        self.state.phase = AppPhase::Idle;
    }

    fn open_workspace_dialog(&mut self, initial: Option<String>) {
        if self.state.phase == AppPhase::Running {
            self.state
                .push_system(crate::i18n::t(sk::STATUS_AGENT_BUSY));
            return;
        }
        let current = self
            .runtime
            .as_ref()
            .map(|r| r.workspace.to_string())
            .unwrap_or_else(|| self.config.workspace.to_string());
        let recents = vec![];
        self.state.workspace_dialog =
            Some(WorkspaceDialog::new(current, recents, initial));
        self.state.reset_modal_anim();
        self.state.phase = AppPhase::Workspace;
        self.state.status_line =
            "/workspace — changer le répertoire de travail (Ctrl+Shift+W)".into();
    }

    fn close_workspace_dialog(&mut self) {
        self.state.workspace_dialog = None;
        if self.state.phase == AppPhase::Workspace {
            self.state.phase = AppPhase::Idle;
        }
    }

    fn cycle_workspace_focus(&mut self, reverse: bool) {
        let Some(dialog) = self.state.workspace_dialog.as_mut() else {
            return;
        };
        if dialog.step != WorkspaceStep::Edit {
            return;
        }
        dialog.focus = match (dialog.focus, reverse) {
            (WorkspaceField::Browser, false) => WorkspaceField::SelectButton,
            (WorkspaceField::SelectButton, false) => WorkspaceField::Path,
            (WorkspaceField::Path, false) => WorkspaceField::Browser,
            (WorkspaceField::Browser, true) => WorkspaceField::Path,
            (WorkspaceField::SelectButton, true) => WorkspaceField::Browser,
            (WorkspaceField::Path, true) => WorkspaceField::SelectButton,
        };
        if dialog.focus == WorkspaceField::Path {
            dialog.path = dialog.location.display_path();
            dialog.path_cursor = dialog.path.len();
        }
    }

    fn validate_workspace_dialog(&mut self) {
        let Some(input) = self
            .state
            .workspace_dialog
            .as_ref()
            .map(|d| d.path.trim().to_string())
        else {
            return;
        };
        if input.is_empty() {
            if let Some(dialog) = self.state.workspace_dialog.as_mut() {
                dialog.status = crate::i18n::t(crate::i18n::keys::WORKSPACE_STATUS_PATH_REQUIRED).into();
            }
            return;
        }
        let current = self.runtime.as_ref().map(|r| r.workspace.clone());
        match crate::engine::validate_workspace_path(&input) {
            Ok(canonical) => {
                let Some(dialog) = self.state.workspace_dialog.as_mut() else {
                    return;
                };
                if current.is_some_and(|c| c == canonical) {
                    dialog.status = crate::i18n::t(crate::i18n::keys::WORKSPACE_STATUS_ALREADY_CURRENT).into();
                    dialog.validated = None;
                    return;
                }
                dialog.validated = Some(canonical);
                dialog.step = WorkspaceStep::Confirm;
                dialog.status = crate::i18n::t(crate::i18n::keys::WORKSPACE_STATUS_ENTER_CONFIRM).into();
            }
            Err(e) => {
                if let Some(dialog) = self.state.workspace_dialog.as_mut() {
                    dialog.status = e;
                    dialog.validated = None;
                }
            }
        }
    }

    fn handle_workspace_key(&mut self, key: KeyEvent) -> bool {
        let Some(dialog) = self.state.workspace_dialog.as_mut() else {
            self.state.phase = AppPhase::Idle;
            return false;
        };

        if dialog.step == WorkspaceStep::Confirm {
            match key.code {
                KeyCode::Esc => {
                    dialog.step = WorkspaceStep::Edit;
                    dialog.validated = None;
                    dialog.status =
                        crate::i18n::t(crate::i18n::keys::WORKSPACE_STATUS_HINT_NAV).into();
                }
                KeyCode::Enter => {
                    if let Some(path) = dialog.validated.clone() {
                        self.close_workspace_dialog();
                        self.pending_slash = Some(PendingSlash::ApplyWorkspace { path });
                    }
                }
                _ => {}
            }
            return false;
        }

        match key.code {
            KeyCode::Esc => {
                self.close_workspace_dialog();
                self.state.status_line = crate::i18n::t(sk::STATUS_WORKSPACE_CANCELLED).into();
            }
            KeyCode::Tab => self.cycle_workspace_focus(false),
            KeyCode::BackTab => self.cycle_workspace_focus(true),
            KeyCode::Up if dialog.focus == WorkspaceField::Browser && dialog.browse_cursor > 0 => {
                dialog.browse_cursor -= 1;
            }
            KeyCode::Down
                if dialog.focus == WorkspaceField::Browser
                    && dialog.browse_cursor + 1 < dialog.entries.len() =>
            {
                dialog.browse_cursor += 1;
            }
            KeyCode::Enter if dialog.focus == WorkspaceField::SelectButton => {
                if dialog.location.selected_dir().is_some() {
                    dialog.path = dialog.location.display_path();
                    dialog.path_cursor = dialog.path.len();
                    self.validate_workspace_dialog();
                } else {
                    dialog.status =
                        crate::i18n::t(crate::i18n::keys::WORKSPACE_STATUS_PICK_FOLDER).into();
                }
            }
            KeyCode::Enter
                if dialog.focus == WorkspaceField::Browser
                    && key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                if dialog.location.selected_dir().is_some() {
                    dialog.path = dialog.location.display_path();
                    dialog.path_cursor = dialog.path.len();
                    self.validate_workspace_dialog();
                }
            }
            KeyCode::Enter if dialog.focus == WorkspaceField::Browser => {
                if let Some(entry) = dialog.entries.get(dialog.browse_cursor).cloned() {
                    if dialog.location.enter(&entry) {
                        dialog.reload_browser();
                    }
                }
            }
            KeyCode::Enter if dialog.focus == WorkspaceField::Path => {
                self.validate_workspace_dialog();
            }
            KeyCode::Backspace if dialog.focus == WorkspaceField::Browser => {
                dialog.location = dialog.location.parent_location();
                dialog.reload_browser();
            }
            KeyCode::Left => {
                if dialog.focus == WorkspaceField::Path && dialog.path_cursor > 0 {
                    dialog.path_cursor -= 1;
                } else if dialog.focus == WorkspaceField::Browser {
                    dialog.location = dialog.location.parent_location();
                    dialog.reload_browser();
                }
            }
            KeyCode::Right => {
                if dialog.focus == WorkspaceField::Path
                    && dialog.path_cursor < dialog.path.len()
                {
                    dialog.path_cursor += 1;
                }
            }
            KeyCode::Backspace if dialog.focus == WorkspaceField::Path => {
                if dialog.path_cursor > 0 && dialog.path_cursor <= dialog.path.len() {
                    dialog.path.remove(dialog.path_cursor - 1);
                    dialog.path_cursor -= 1;
                }
            }
            KeyCode::Delete if dialog.focus == WorkspaceField::Path => {
                if dialog.path_cursor < dialog.path.len() {
                    dialog.path.remove(dialog.path_cursor);
                }
            }
            _ if dialog.focus == WorkspaceField::Path => {
                if let Some(c) = typed_char(&key) {
                    dialog.path.insert(dialog.path_cursor, c);
                    dialog.path_cursor += 1;
                }
            }
            _ => {}
        }
        false
    }

    async fn apply_workspace_switch(
        &mut self,
        workspace: camino::Utf8PathBuf,
    ) -> anyhow::Result<()> {
        if self.state.phase == AppPhase::Running {
            self.state
                .push_system(crate::i18n::t(sk::STATUS_AGENT_BUSY));
            return Ok(());
        }

        self.config.workspace = workspace.clone();
        self.config.session = None;

        env_file::load_default(Some(self.config.workspace.as_std_path()));

        let runtime = Arc::new(
            EngineRuntime::bootstrap(&self.config, Arc::clone(&self.ask))
                .await
                .context("re-bootstrap workspace")?,
        );
        self.runtime = Some(Arc::clone(&runtime));

        self.paste_store.clear();
        self.at_file_index = None;
        self.at_file_roots_key = None;
        self.message_queue.clear();
        self.state.queued_messages = 0;
        self.tool_names.clear();
        self.history_cursor = None;
        self.history_search = None;
        self.state.transcript_search = None;
        self.state.composer_suggestions = None;
        self.state.slash_palette = None;
        self.state.session_title.clear();
        self.state.composer_buffer.clear();
        self.state.clear_transcript();

        self.state
            .push_system(format!("Workspace : {}", runtime.workspace));
        if self.state.llm_configured {
            self.state.push_system(format!(
                "LLM : {} @ {}",
                self.config.model, self.config.server
            ));
        }
        self.state.push_system(format!(
            "Nouvelle session : {} ({})",
            runtime.session_id(),
            runtime.transcript_path()
        ));

        record_recent_workspace(workspace.as_str())?;
        self.tui_prefs = crate::engine::preferences::load_preferences();

        self.refresh_session_title().await;
        self.sync_terminal_title();
        self.refresh_status_snapshot().await;
        self.refresh_mcp_panel().await;
        self.load_skill_suggestions(&runtime).await;
        self.state.status_notices = runtime.collect_startup_notices(&self.config);
        if !self.state.llm_configured {
            self.state.status_notices.insert(
                0,
                StatusNotice {
                    level: NoticeLevel::Warn,
                    text: "Serveur IA non configuré — Ctrl+Shift+L ou /server : adresse Ollama, test, choix du modèle."
                        .into(),
                },
            );
        }

        self.state.status_line =
            crate::i18n::tf(sk::STATUS_WORKSPACE_CHANGED, runtime.workspace.as_str());
        self.state
            .push_toast(crate::i18n::t(sk::TOAST_WORKSPACE_CHANGED));
        Ok(())
    }

    fn start_ai_server_test(&mut self) {
        let Some(dialog) = self.state.ai_server.as_mut() else {
            return;
        };
        if dialog.step == AiServerStep::Testing {
            return;
        }
        if dialog.server.trim().is_empty() {
            dialog.status = crate::i18n::t(crate::i18n::keys::SERVER_STATUS_URL_REQUIRED).into();
            dialog.configure_focus = ConfigureField::Url;
            return;
        }
        if dialog.auth_type == AuthTypeChoice::ApiKeyHeader && dialog.auth_header_name.trim().is_empty() {
            dialog.status = crate::i18n::t(crate::i18n::keys::SERVER_STATUS_HEADER_REQUIRED).into();
            dialog.configure_focus = ConfigureField::AuthHeaderName;
            return;
        }
        let profile = dialog.build_probe_profile();
        dialog.step = AiServerStep::Testing;
        dialog.status = crate::i18n::t(crate::i18n::keys::SERVER_STATUS_TESTING).into();

        let (tx, rx) = mpsc::channel(1);
        self.ai_server_test_rx = Some(rx);
        tokio::spawn(async move {
            let result = crate::engine::probe_connection(&profile)
                .await
                .map_err(|e| e.to_string());
            let _ = tx.send(result).await;
        });
    }

    fn poll_ai_server_test(&mut self) {
        let Some(rx) = self.ai_server_test_rx.as_mut() else {
            return;
        };
        let Ok(result) = rx.try_recv() else {
            return;
        };
        self.ai_server_test_rx = None;
        let persist_after = match result {
            Ok(models) if models.is_empty() => {
                if let Some(dialog) = self.state.ai_server.as_mut() {
                    dialog.step = AiServerStep::ConfigureConnection;
                    dialog.status =
                        crate::i18n::t(crate::i18n::keys::SERVER_STATUS_NO_MODELS).into();
                }
                None
            }
            Ok(models) => {
                let current = self.config.model.clone();
                let model_count = models.len();
                if let Some(dialog) = self.state.ai_server.as_mut() {
                    dialog.model_cursor =
                        models.iter().position(|m| m == &current).unwrap_or(0);
                    dialog.models = models;
                    dialog.step = AiServerStep::SelectModel;
                    dialog.select_focus = AiServerSelectFocus::ModelList;
                }
                Some(model_count)
            }
            Err(err) => {
                if let Some(dialog) = self.state.ai_server.as_mut() {
                    dialog.step = AiServerStep::ConfigureConnection;
                    dialog.status = crate::i18n::tf(crate::i18n::keys::SERVER_STATUS_CONN_FAILED, &err);
                }
                None
            }
        };
        if let Some(model_count) = persist_after {
            if let Err(e) = self.persist_ai_server_verified_connection() {
                tracing::warn!(error = %e, "echec persistance connexion /server apres test");
            }
            if let Some(dialog) = self.state.ai_server.as_mut() {
                dialog.status = crate::i18n::tf(
                    crate::i18n::keys::SERVER_STATUS_CONN_OK,
                    &model_count.to_string(),
                );
            }
        }
    }

    fn upsert_connection_profile(&mut self, mut profile: crate::engine::ConnectionProfile) -> anyhow::Result<()> {
        let mut library = self.tui_prefs.connection_library.clone();
        library.ensure_builtin_presets();
        if let Some(active_id) = library.active_profile_id.clone() {
            profile.id = active_id;
            library.upsert_profile(profile.clone());
        } else {
            let id = profile.id.clone();
            library.upsert_profile(profile.clone());
            library.set_active(&id).map_err(anyhow::Error::msg)?;
        }
        crate::engine::save_connection_library(&library)?;
        if let Some(legacy) = crate::engine::profile_to_legacy_prefs(&profile) {
            crate::engine::save_llm_connection(&legacy)?;
        }
        self.tui_prefs = load_preferences();
        Ok(())
    }

    fn persist_ai_server_verified_connection(&mut self) -> anyhow::Result<()> {
        let dialog = self
            .state
            .ai_server
            .as_ref()
            .context("dialogue connexion IA")?;
        let mut profile = dialog.build_probe_profile();
        profile.connection_verified = true;
        profile.verified_models = dialog.models.clone();
        profile.num_ctx = dialog.resolved_num_ctx().map_err(anyhow::Error::msg)?;
        profile.max_iterations = dialog.resolved_max_iterations().map_err(anyhow::Error::msg)?;
        if profile.default_model.as_ref().is_none_or(|m| m.trim().is_empty()) {
            profile.default_model = Some(self.config.model.clone())
                .filter(|m| !m.trim().is_empty());
        }
        self.upsert_connection_profile(profile)
    }

    fn persist_ai_server_dialog_draft(&mut self) -> anyhow::Result<()> {
        let dialog = self
            .state
            .ai_server
            .as_ref()
            .context("dialogue connexion IA")?;
        let model = dialog
            .models
            .get(dialog.model_cursor)
            .context("modele non selectionne")?
            .clone();
        let mut profile = dialog.build_probe_profile();
        profile.default_model = Some(model);
        profile.num_ctx = dialog.resolved_num_ctx().map_err(anyhow::Error::msg)?;
        profile.max_iterations = dialog.resolved_max_iterations().map_err(anyhow::Error::msg)?;
        profile.connection_verified = true;
        profile.verified_models = dialog.models.clone();
        self.upsert_connection_profile(profile)
    }

    async fn apply_ai_server_model(&mut self, index: usize) -> anyhow::Result<()> {
        let profile = {
            let dialog = self
                .state
                .ai_server
                .as_ref()
                .context("dialogue connexion IA")?;
            let model = dialog
                .models
                .get(index)
                .context("index modele invalide")?
                .clone();
            let num_ctx = dialog.resolved_num_ctx().map_err(anyhow::Error::msg)?;
            let max_iterations = dialog.resolved_max_iterations().map_err(anyhow::Error::msg)?;
            let mut profile = dialog.build_probe_profile();
            profile.default_model = Some(model.clone());
            profile.num_ctx = num_ctx;
            profile.max_iterations = max_iterations;
            profile.connection_verified = true;
            profile.verified_models = dialog.models.clone();
            profile
        };

        let model = profile.default_model.clone().unwrap_or_default();
        let server = profile.base_url.clone();
        let api_key = crate::engine::profile_to_legacy_prefs(&profile).and_then(|p| p.api_key);
        let num_ctx = profile.num_ctx;
        let max_iterations = profile.max_iterations;

        self.upsert_connection_profile(profile.clone())?;

        self.config.server = server.clone();
        self.config.model = model.clone();
        self.config.api_key = api_key.clone();
        self.config.num_ctx = num_ctx;
        self.config.max_iterations = max_iterations;

        let runtime = self.runtime.as_ref().context("moteur non initialise")?;
        runtime.apply_llm_connection_profile(&profile)?;
        runtime.set_max_iterations(max_iterations);

        self.state.llm_configured = true;

        self.close_ai_server_dialog();
        let prefs_path = preferences_path();
        self.state.push_system(format!(
            "Connexion IA : {} @ {server} — modele `{model}` — context {num_ctx} — max_iter {max_iterations}",
            profile.name
        ));
        self.state
            .push_system(format!("Preferences persistees : {prefs_path}"));
        self.state.push_toast(crate::i18n::t(sk::TOAST_LLM_SAVED));
        self.refresh_status_snapshot().await;
        self.state.status_line = crate::i18n::t(sk::STATUS_LLM_SAVED).into();

        if !self.tui_prefs.onboarding_done {
            self.state.onboarding = Some(OnboardingDialog::default());
            self.state.phase = AppPhase::Onboarding;
        }
        Ok(())
    }

    fn cycle_ai_server_context_preset(&mut self, reverse: bool) {
        if let Some(dialog) = self.state.ai_server.as_mut() {
            dialog.context_preset_index = cycle_preset_index(dialog.context_preset_index, reverse);
        }
    }

    fn cycle_ai_server_select_focus(&mut self, reverse: bool) {
        if let Some(dialog) = self.state.ai_server.as_mut() {
            dialog.select_focus = match dialog.select_focus {
                AiServerSelectFocus::ModelList => {
                    if reverse {
                        AiServerSelectFocus::ResetWizard
                    } else {
                        AiServerSelectFocus::NumCtx
                    }
                }
                AiServerSelectFocus::NumCtx => {
                    if reverse {
                        AiServerSelectFocus::ModelList
                    } else {
                        AiServerSelectFocus::MaxIterations
                    }
                }
                AiServerSelectFocus::MaxIterations => {
                    if reverse {
                        AiServerSelectFocus::NumCtx
                    } else {
                        AiServerSelectFocus::ResetWizard
                    }
                }
                AiServerSelectFocus::ResetWizard => {
                    if reverse {
                        AiServerSelectFocus::MaxIterations
                    } else {
                        AiServerSelectFocus::ModelList
                    }
                }
            };
        }
    }

    fn handle_ai_server_key(&mut self, key: KeyEvent) -> bool {
        if self.state.ai_server.is_none() {
            self.state.phase = AppPhase::Idle;
            return false;
        }

        if self
            .state
            .ai_server
            .as_ref()
            .is_some_and(|d| d.step == AiServerStep::Testing)
        {
            if key.code == KeyCode::Esc {
                if let Some(d) = self.state.ai_server.as_mut() {
                    d.step = AiServerStep::ConfigureConnection;
                    d.status = crate::i18n::t(crate::i18n::keys::SERVER_STATUS_TEST_CANCELLED).into();
                }
            }
            return false;
        }

        if key.code == KeyCode::Esc
            && self.state.ai_server.as_ref().is_some_and(|d| {
                d.step == AiServerStep::ChooseDeployment
            })
        {
            self.close_ai_server_dialog();
            self.state.status_line = crate::i18n::t(sk::STATUS_LLM_CANCELLED).into();
            return false;
        }

        if matches!(key.code, KeyCode::Tab | KeyCode::BackTab)
            && self.state.ai_server.as_ref().is_some_and(|d| d.step == AiServerStep::SelectModel)
        {
            self.cycle_ai_server_select_focus(
                key.code == KeyCode::BackTab || key.modifiers.contains(KeyModifiers::SHIFT),
            );
            return false;
        }

        if matches!(key.code, KeyCode::Left | KeyCode::Right)
            && self.state.ai_server.as_ref().is_some_and(|d| {
                d.step == AiServerStep::SelectModel
                    && d.select_focus == AiServerSelectFocus::NumCtx
                    && d.context_preset_index != CONTEXT_CUSTOM_INDEX
            })
        {
            self.cycle_ai_server_context_preset(key.code == KeyCode::Left);
            return false;
        }

        if key.code == KeyCode::Enter {
            if let Some(d) = self.state.ai_server.as_ref() {
                if d.step == AiServerStep::ConfigureConnection
                    && d.configure_focus == ConfigureField::TestButton
                {
                    self.start_ai_server_test();
                    return false;
                }
                if d.step == AiServerStep::ConfirmReset {
                    self.state.ai_server = Some(AiServerDialog::new_wizard());
                    self.state.status_line =
                        "/server — nouvelle configuration (Ctrl+Shift+L)".into();
                    return false;
                }
                if d.step == AiServerStep::SelectModel {
                    if d.select_focus == AiServerSelectFocus::ResetWizard {
                        if let Some(dialog) = self.state.ai_server.as_mut() {
                            dialog.begin_reset_wizard();
                        }
                        return false;
                    }
                    let num_ok = d.resolved_num_ctx();
                    let max_ok = d.resolved_max_iterations();
                    let idx = d.model_cursor;
                    match (num_ok, max_ok) {
                        (Ok(_), Ok(_)) => {
                            self.pending_slash =
                                Some(PendingSlash::ApplyAiServer { index: idx });
                        }
                        (Err(e), _) => {
                            if let Some(dialog) = self.state.ai_server.as_mut() {
                                dialog.select_focus = AiServerSelectFocus::NumCtx;
                                dialog.status = e;
                            }
                        }
                        (Ok(_), Err(e)) => {
                            if let Some(dialog) = self.state.ai_server.as_mut() {
                                dialog.select_focus = AiServerSelectFocus::MaxIterations;
                                dialog.status = e;
                            }
                        }
                    }
                    return false;
                }
            }
        }

        let Some(dialog) = self.state.ai_server.as_mut() else {
            self.state.phase = AppPhase::Idle;
            return false;
        };

        match dialog.step {
            AiServerStep::ConfirmReset => match key.code {
                KeyCode::Esc => {
                    dialog.wizard_back();
                }
                _ => {}
            },
            AiServerStep::ChooseDeployment
            | AiServerStep::ChoosePersonalEngine
            | AiServerStep::ChooseCloudProvider => {
                let list_len = match dialog.step {
                    AiServerStep::ChooseDeployment => DeploymentKind::ALL.len(),
                    AiServerStep::ChoosePersonalEngine => PersonalEngineChoice::ALL.len(),
                    AiServerStep::ChooseCloudProvider => CloudProviderChoice::ALL.len(),
                    _ => 0,
                };
                match key.code {
                    KeyCode::Esc => {
                        dialog.wizard_back();
                    }
                    KeyCode::Up if dialog.list_cursor > 0 => {
                        dialog.list_cursor -= 1;
                    }
                    KeyCode::Down if dialog.list_cursor + 1 < list_len => {
                        dialog.list_cursor += 1;
                    }
                    KeyCode::Enter => {
                        dialog.wizard_advance_list();
                    }
                    _ => {}
                }
            }
            AiServerStep::ConfigureConnection => match key.code {
                KeyCode::Esc => {
                    dialog.wizard_back();
                }
                KeyCode::Tab => {
                    dialog.cycle_configure_focus(key.modifiers.contains(KeyModifiers::SHIFT));
                }
                KeyCode::BackTab => {
                    dialog.cycle_configure_focus(true);
                }
                KeyCode::Enter if dialog.configure_focus == ConfigureField::BackButton => {
                    dialog.wizard_back();
                }
                KeyCode::Enter if dialog.configure_focus == ConfigureField::AddExtraHeader => {
                    dialog.add_extra_header_from_inputs();
                }
                KeyCode::Left if dialog.configure_focus == ConfigureField::AuthType => {
                    dialog.auth_type = dialog.auth_type.prev();
                }
                KeyCode::Right if dialog.configure_focus == ConfigureField::AuthType => {
                    dialog.auth_type = dialog.auth_type.next();
                }
                KeyCode::Left => {
                    if let Some((_, cur)) = dialog.configure_active_buffer() {
                        if *cur > 0 {
                            *cur -= 1;
                        }
                    }
                }
                KeyCode::Right => {
                    if let Some((buf, cur)) = dialog.configure_active_buffer() {
                        if *cur < buf.len() {
                            *cur += 1;
                        }
                    }
                }
                KeyCode::Backspace => {
                    if let Some((buf, cur)) = dialog.configure_active_buffer() {
                        if *cur > 0 && *cur <= buf.len() {
                            buf.remove(*cur - 1);
                            *cur -= 1;
                        }
                    }
                }
                KeyCode::Delete => {
                    if let Some((buf, cur)) = dialog.configure_active_buffer() {
                        if *cur < buf.len() {
                            buf.remove(*cur);
                        }
                    }
                }
                KeyCode::Char(c)
                    if !key.modifiers.contains(KeyModifiers::CONTROL)
                        && dialog.configure_active_buffer().is_some() =>
                {
                    if let Some((buf, cur)) = dialog.configure_active_buffer() {
                        buf.insert(*cur, c);
                        *cur += 1;
                    }
                }
                _ => {}
            },
            AiServerStep::SelectModel => match key.code {
                KeyCode::Esc => {
                    dialog.wizard_back();
                }
                KeyCode::Up
                    if dialog.select_focus == AiServerSelectFocus::ModelList
                        && dialog.model_cursor > 0 =>
                {
                    dialog.model_cursor -= 1;
                }
                KeyCode::Down
                    if dialog.select_focus == AiServerSelectFocus::ModelList
                        && dialog.model_cursor + 1 < dialog.models.len() =>
                {
                    dialog.model_cursor += 1;
                }
                KeyCode::Left if dialog.select_focus == AiServerSelectFocus::NumCtx => {
                    if dialog.context_custom_cursor > 0 {
                        dialog.context_custom_cursor -= 1;
                    }
                }
                KeyCode::Right if dialog.select_focus == AiServerSelectFocus::NumCtx => {
                    if dialog.context_custom_cursor < dialog.context_custom.len() {
                        dialog.context_custom_cursor += 1;
                    }
                }
                KeyCode::Left if dialog.select_focus == AiServerSelectFocus::MaxIterations => {
                    if dialog.max_iterations_cursor > 0 {
                        dialog.max_iterations_cursor -= 1;
                    }
                }
                KeyCode::Right if dialog.select_focus == AiServerSelectFocus::MaxIterations => {
                    if dialog.max_iterations_cursor < dialog.max_iterations.len() {
                        dialog.max_iterations_cursor += 1;
                    }
                }
                KeyCode::Backspace if dialog.select_focus == AiServerSelectFocus::NumCtx => {
                    if dialog.context_preset_index == CONTEXT_CUSTOM_INDEX
                        && dialog.context_custom_cursor > 0
                    {
                        dialog
                            .context_custom
                            .remove(dialog.context_custom_cursor - 1);
                        dialog.context_custom_cursor -= 1;
                    }
                }
                KeyCode::Backspace if dialog.select_focus == AiServerSelectFocus::MaxIterations => {
                    if dialog.max_iterations_cursor > 0
                        && dialog.max_iterations_cursor <= dialog.max_iterations.len()
                    {
                        dialog
                            .max_iterations
                            .remove(dialog.max_iterations_cursor - 1);
                        dialog.max_iterations_cursor -= 1;
                    }
                }
                KeyCode::Delete if dialog.select_focus == AiServerSelectFocus::NumCtx => {
                    if dialog.context_preset_index == CONTEXT_CUSTOM_INDEX
                        && dialog.context_custom_cursor < dialog.context_custom.len()
                    {
                        dialog.context_custom.remove(dialog.context_custom_cursor);
                    }
                }
                KeyCode::Delete if dialog.select_focus == AiServerSelectFocus::MaxIterations => {
                    if dialog.max_iterations_cursor < dialog.max_iterations.len() {
                        dialog.max_iterations.remove(dialog.max_iterations_cursor);
                    }
                }
                KeyCode::Char(c)
                    if !key.modifiers.contains(KeyModifiers::CONTROL)
                        && dialog.select_focus == AiServerSelectFocus::NumCtx =>
                {
                    if dialog.context_preset_index != CONTEXT_CUSTOM_INDEX {
                        dialog.context_preset_index = CONTEXT_CUSTOM_INDEX;
                        dialog.context_custom.clear();
                        dialog.context_custom_cursor = 0;
                    }
                    if c.is_ascii_digit() {
                        dialog
                            .context_custom
                            .insert(dialog.context_custom_cursor, c);
                        dialog.context_custom_cursor += 1;
                    }
                }
                KeyCode::Char(c)
                    if !key.modifiers.contains(KeyModifiers::CONTROL)
                        && dialog.select_focus == AiServerSelectFocus::MaxIterations
                        && c.is_ascii_digit() =>
                {
                    dialog
                        .max_iterations
                        .insert(dialog.max_iterations_cursor, c);
                    dialog.max_iterations_cursor += 1;
                }
                _ => {}
            },
            AiServerStep::Testing => {}
        }
        false
    }

    fn finish_onboarding(&mut self) {
        self.state.onboarding = None;
        self.state.phase = AppPhase::Idle;
        let _ = crate::engine::preferences::mark_onboarding_done();
        self.state.push_toast(crate::i18n::t(sk::TOAST_ONBOARDING_DONE));
    }

    fn handle_onboarding_key(&mut self, key: KeyEvent) -> bool {
        const STEPS: usize = 9;
        match key.code {
            KeyCode::Esc => {
                self.finish_onboarding();
            }
            KeyCode::Enter => {
                if let Some(ref mut d) = self.state.onboarding {
                    if d.step + 1 >= STEPS {
                        self.finish_onboarding();
                    } else {
                        d.step += 1;
                    }
                }
            }
            _ => {}
        }
        false
    }

    fn open_slash_palette(&mut self, filter: String) {
        let matches = filter_entries(&filter);
        self.state.slash_palette = Some(SlashPaletteDialog {
            filter,
            matches,
            cursor: 0,
        });
        self.state.reset_modal_anim();
        self.state.phase = AppPhase::SlashPalette;
        self.state.status_line = crate::i18n::t(sk::STATUS_SLASH_PALETTE).into();
    }

    fn refilter_slash_palette(&mut self) {
        let Some(dialog) = self.state.slash_palette.as_mut() else {
            return;
        };
        dialog.matches = filter_entries(&dialog.filter);
        dialog.cursor = dialog.cursor.min(dialog.matches.len().saturating_sub(1));
    }

    fn handle_slash_palette_key(&mut self, key: KeyEvent) -> anyhow::Result<bool> {
        let Some(dialog) = self.state.slash_palette.as_mut() else {
            self.state.phase = AppPhase::Idle;
            return Ok(false);
        };
        match key.code {
            KeyCode::Esc => {
                self.state.slash_palette = None;
                self.state.phase = AppPhase::Idle;
                self.state.status_line = crate::i18n::t(sk::STATUS_SLASH_PALETTE_CLOSED).into();
            }
            KeyCode::Up if dialog.cursor > 0 => {
                dialog.cursor -= 1;
            }
            KeyCode::Down if dialog.cursor + 1 < dialog.matches.len() => {
                dialog.cursor += 1;
            }
            KeyCode::Enter => {
                if let Some(&idx) = dialog.matches.get(dialog.cursor) {
                    let command = SLASH_PALETTE_ENTRIES[idx].command.to_string();
                    self.state.slash_palette = None;
                    self.state.phase = AppPhase::Idle;
                    return self.dispatch_slash_command(&command);
                }
            }
            KeyCode::Backspace => {
                dialog.filter.pop();
                self.refilter_slash_palette();
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                dialog.filter.push(c);
                self.refilter_slash_palette();
            }
            _ => {}
        }
        Ok(false)
    }

    /// Recalcule les suggestions unifiées (`@` · `/` · skills).
    fn refresh_composer_suggestions(&mut self) {
        if self.state.composer_mode == ComposerMode::Bash
            || self.state.phase == AppPhase::Running
            || self.history_search.is_some()
        {
            self.state.composer_suggestions = None;
            return;
        }

        let Some(rt) = self.runtime.as_ref() else {
            return;
        };

        let roots = rt.working_directories();
        let roots_key = roots
            .iter()
            .map(|r| r.as_str())
            .collect::<Vec<_>>()
            .join("\0");
        if self.at_file_roots_key.as_deref() != Some(roots_key.as_str()) {
            self.at_file_index = Some(AtFileIndex::build_roots(&roots, &rt.drox_ignore));
            self.at_file_roots_key = Some(roots_key);
        }

        let Some(mut dialog) = unified_suggestions::build_suggestions(
            &self.state.composer_buffer,
            self.at_file_index.as_ref(),
            &self.skill_suggestions,
        ) else {
            self.state.composer_suggestions = None;
            return;
        };

        dialog.cursor = preserve_cursor(self.state.composer_suggestions.as_ref(), &dialog);
        self.state.composer_suggestions = Some(dialog);
    }

    /// Tab — applique la suggestion surlignée.
    fn apply_composer_suggestion(&mut self) {
        let Some(dialog) = self.state.composer_suggestions.clone() else {
            return;
        };
        let Some(item) = dialog.items.get(dialog.cursor) else {
            return;
        };
        self.apply_suggestion_item(item, &dialog);
        self.refresh_composer_suggestions();
    }

    fn apply_suggestion_item(
        &mut self,
        item: &ComposerSuggestionItem,
        dialog: &ComposerSuggestionDialog,
    ) {
        match item.kind {
            SuggestionKind::File => {
                let Some(ctx) = dialog.file_ctx.as_ref() else {
                    return;
                };
                self.state.composer_buffer =
                    at_typeahead::apply_completion(&self.state.composer_buffer, ctx, &item.payload);
                self.state.status_line =
                    crate::i18n::tf(sk::STATUS_AT_REF_INSERTED, &item.payload);
            }
            SuggestionKind::Slash => {
                self.state.composer_buffer = item.payload.clone();
                self.state.status_line =
                    crate::i18n::tf(sk::STATUS_SLASH_CMD_READY, &item.label);
            }
            SuggestionKind::Skill => {
                self.state.composer_buffer = format!("/skills {}", item.payload);
                self.state.status_line =
                    crate::i18n::tf(sk::STATUS_SKILL_READY, &item.payload);
            }
        }
    }

    async fn load_skill_suggestions(&mut self, runtime: &EngineRuntime) {
        use drox_tools::skills::load_skills_catalog;
        self.skill_suggestions = match load_skills_catalog(&runtime.workspace).await {
            Ok(catalog) => catalog
                .into_iter()
                .map(|e| SkillSuggestionEntry {
                    name: e.name,
                    description: e.description,
                })
                .collect(),
            Err(_) => Vec::new(),
        };
    }

    /// Touches réservées quand le composer est vide (avant le handler vim).
    fn try_composer_special_char(&mut self, c: char, key: &KeyEvent) -> anyhow::Result<bool> {
        if c == '!'
            && self.state.composer_mode == ComposerMode::Normal
            && self.state.composer_buffer.is_empty()
        {
            self.state.composer_mode = ComposerMode::Bash;
            self.state.status_line = crate::i18n::t(sk::STATUS_BASH_MODE_FOOTER).into();
            return Ok(true);
        }
        if c == '/'
            && self.state.composer_buffer.is_empty()
            && self.keybindings.matches(BindingAction::SlashPalette, key)
        {
            self.open_slash_palette(String::new());
            return Ok(true);
        }
        if c == '?'
            && self.state.phase != AppPhase::Running
            && self.state.composer_buffer.is_empty()
        {
            self.state.composer_help = !self.state.composer_help;
            return Ok(true);
        }
        Ok(false)
    }

    fn apply_vim_key(&mut self, key: &KeyEvent) {
        match self
            .vim
            .handle_key(key, &mut self.state.composer_buffer)
        {
            VimKeyResult::NotHandled => {}
            VimKeyResult::Handled {
                refresh_suggestions,
                status,
            } => {
                if refresh_suggestions {
                    self.refresh_composer_suggestions();
                }
                if let Some(s) = status {
                    self.state.status_line = s;
                }
            }
        }
    }

    /// Collage presse-papiers (mode bracketed paste crossterm).
    fn handle_paste(&mut self, text: String) {
        if self.state.phase == AppPhase::Running
            || self.history_search.is_some()
            || self.state.composer_mode == ComposerMode::Bash
        {
            return;
        }

        // Collage vide : tenter une image clipboard (Windows — leak macOS natif plus tard).
        if text.trim().is_empty() {
            if let Some(img) = crate::engine::image_paste::read_clipboard_image() {
                let ins = self.paste_store.ingest_image(img);
                if self.vim.enabled {
                    self.vim.insert_text(&mut self.state.composer_buffer, &ins);
                } else {
                    self.state.composer_buffer.push_str(&ins);
                }
                self.refresh_composer_suggestions();
                self.state.status_line = crate::i18n::tf(sk::STATUS_IMAGE_PASTED, &ins);
            }
            return;
        }

        // Chemin fichier image collé en texte.
        if let Some(path) = crate::engine::image_paste::as_image_file_path(&text) {
            let roots = self
                .runtime
                .as_ref()
                .map(|r| r.working_directories())
                .unwrap_or_else(Vec::new);
            if let Some(img) = crate::engine::image_paste::read_image_file(&path, &roots) {
                let label = img.label.clone();
                let ins = self.paste_store.ingest_image(img);
                if self.vim.enabled {
                    self.vim.insert_text(&mut self.state.composer_buffer, &ins);
                } else {
                    self.state.composer_buffer.push_str(&ins);
                }
                self.refresh_composer_suggestions();
                self.state.status_line =
                    crate::i18n::tf2(sk::STATUS_IMAGE_PATH, &label, &ins);
                return;
            }
        }

        let insert = self.paste_store.ingest_paste(&text);
        let summary = if insert.contains("[Pasted text #") {
            format!("Collage réduit — {insert}")
        } else {
            format!("Collage ({})", text.len())
        };
        if self.vim.enabled {
            self.vim.insert_text(&mut self.state.composer_buffer, &insert);
        } else {
            self.state.composer_buffer.push_str(&insert);
        }
        self.refresh_composer_suggestions();
        self.state.status_line = summary;
    }
}

/// Liste les sessions puis quitte (mode `--list-sessions`).
pub async fn print_sessions_list(session_dir: Option<camino::Utf8PathBuf>) -> anyhow::Result<()> {
    let dir = match session_dir {
        Some(d) => d,
        None => default_sessions_dir().context("répertoire sessions")?,
    };
    let entries = list_sessions(&dir).await.context("liste sessions")?;
    if entries.is_empty() {
        println!("(aucune session)");
    } else {
        for e in entries {
            println!("{} — {} o — {:?}", e.id, e.size_bytes, e.path);
        }
    }
    Ok(())
}

#[path = "mouse.rs"]
mod mouse;
