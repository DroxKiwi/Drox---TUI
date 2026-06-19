//! Configuration et état mutable de la session TUI.

use std::collections::HashSet;

use camino::Utf8PathBuf;
use drox_tools::UserQuestion;
use drox_types::ToolUseId;
use serde_json::Value;

use ratatui::text::Line;

use crate::view::{
    bash_progress, highlight_line, hook_progress, render, ActiveBashRun, ActiveHookProgress, LogEntry,
    PermissionPreview, ScrollViewerState, TranscriptSearchState,
};
use crate::ui::{resolve_palette, SessionAccent, ThemePalette, TuiThemeSetting};

/// Item affiché dans le panneau todos.
#[derive(Debug, Clone)]
pub struct TodoItemView {
    pub id: String,
    pub content: String,
    pub status: String,
}

/// Snapshot courant de la to-do list session.
#[derive(Debug, Clone)]
pub struct TodoSnapshot {
    pub items: Vec<TodoItemView>,
    pub summary: String,
}

/// Étape du plan de cours affichée dans le panneau.
#[derive(Debug, Clone)]
pub struct CourseStepView {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub status: String,
}

/// Snapshot courant du plan de cours.
#[derive(Debug, Clone)]
pub struct CourseSnapshot {
    pub title: String,
    pub steps: Vec<CourseStepView>,
    pub summary: String,
}

/// Paramètres de démarrage (CLI / env).
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub server: String,
    pub model: String,
    pub workspace: Utf8PathBuf,
    pub apply: bool,
    pub plan_mode: bool,
    pub mode: Option<String>,
    pub allow: Vec<String>,
    pub ask: Vec<String>,
    pub deny: Vec<String>,
    pub no_settings: bool,
    pub max_iterations: usize,
    pub api_key: Option<String>,
    /// Fenetre de contexte LLM (`num_ctx` Ollama).
    pub num_ctx: i64,
    /// Reprend un transcript `ses_…` existant.
    pub session: Option<String>,
    /// Répertoire des transcripts (défaut `~/.drox/sessions`).
    pub session_dir: Option<Utf8PathBuf>,
}

/// Phase haute niveau de l'écran.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppPhase {
    #[default]
    Idle,
    Running,
    Prompt,
    Rewind,
    Theme,
    SlashPalette,
    Copy,
    Onboarding,
    AiServer,
    Workspace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ComposerMode {
    #[default]
    Normal,
    Multiline,
    /// Mode bash intégré (`!`) — exécution shell sans agent.
    Bash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RunStatus {
    #[default]
    None,
    Completed,
    Cancelled,
    Error,
}

/// Dialogue question / permission en cours.
#[derive(Debug, Clone)]
pub struct PromptDialog {
    pub question: UserQuestion,
    pub buffer: String,
    pub choice_index: usize,
    pub file_preview: Option<PermissionPreview>,
}

/// Sélecteur `/rewind` — choix d'un message utilisateur.
#[derive(Debug, Clone)]
pub struct RewindDialog {
    pub choices: Vec<RewindChoiceView>,
    pub cursor: usize,
}

/// Assistant premier lancement (`/onboarding`).
#[derive(Debug, Clone, Default)]
pub struct OnboardingDialog {
    pub step: usize,
}

/// Étape du dialogue connexion IA (`/server`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiServerStep {
    Configure,
    Testing,
    SelectModel,
}

/// Champ actif du formulaire connexion IA.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiServerField {
    Server,
    ApiKey,
    NumCtx,
    TestButton,
}

/// Focus clavier sur l'etape choix du modele (`/server`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AiServerSelectFocus {
    #[default]
    ModelList,
    MaxIterations,
}

/// Modal connexion serveur IA (`/server`, Ctrl+Shift+L).
#[derive(Debug, Clone)]
pub struct AiServerDialog {
    pub engine: crate::engine::LlmEngineKind,
    pub server: String,
    pub server_cursor: usize,
    pub api_key: String,
    pub api_key_cursor: usize,
    pub focus: AiServerField,
    pub step: AiServerStep,
    pub status: String,
    pub models: Vec<String>,
    pub model_cursor: usize,
    pub context_preset_index: usize,
    pub context_custom: String,
    pub context_custom_cursor: usize,
    pub max_iterations: String,
    pub max_iterations_cursor: usize,
    pub select_focus: AiServerSelectFocus,
}

impl AiServerDialog {
    #[must_use]
    pub fn from_current(server: String, api_key: String, num_ctx: i64, max_iterations: usize) -> Self {
        let max_iterations = max_iterations.to_string();
        let context_preset_index = crate::engine::preset_index_for(num_ctx);
        let context_custom = if context_preset_index == crate::engine::CONTEXT_CUSTOM_INDEX {
            num_ctx.to_string()
        } else {
            String::new()
        };
        Self {
            engine: crate::engine::LlmEngineKind::Ollama,
            server_cursor: server.len(),
            api_key_cursor: api_key.len(),
            server,
            api_key,
            focus: AiServerField::Server,
            step: AiServerStep::Configure,
            status: "Adresse serveur - Tab - Entree sur Tester".into(),
            models: Vec::new(),
            model_cursor: 0,
            context_preset_index,
            context_custom: context_custom.clone(),
            context_custom_cursor: context_custom.len(),
            max_iterations: max_iterations.clone(),
            max_iterations_cursor: max_iterations.len(),
            select_focus: AiServerSelectFocus::ModelList,
        }
    }

    pub fn active_buffer_and_cursor(&mut self) -> (&mut String, &mut usize) {
        match self.focus {
            AiServerField::Server => (&mut self.server, &mut self.server_cursor),
            AiServerField::ApiKey => (&mut self.api_key, &mut self.api_key_cursor),
            AiServerField::NumCtx if self.context_preset_index == crate::engine::CONTEXT_CUSTOM_INDEX => {
                (&mut self.context_custom, &mut self.context_custom_cursor)
            }
            AiServerField::NumCtx | AiServerField::TestButton => {
                (&mut self.server, &mut self.server_cursor)
            }
        }
    }

    pub fn resolved_num_ctx(&self) -> Result<i64, String> {
        crate::engine::resolve_num_ctx(self.context_preset_index, &self.context_custom)
    }

    pub fn resolved_max_iterations(&self) -> Result<usize, String> {
        crate::engine::parse_max_iterations(&self.max_iterations)
    }
}

/// Étape modal `/workspace`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceStep {
    Edit,
    Confirm,
}

/// Focus clavier dans le modal workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceField {
    Recents,
    Path,
    ValidateButton,
}

/// Modal changement workspace (`/workspace`, Ctrl+Shift+W).
#[derive(Debug, Clone)]
pub struct WorkspaceDialog {
    pub path: String,
    pub path_cursor: usize,
    pub recents: Vec<String>,
    pub recent_cursor: usize,
    pub focus: WorkspaceField,
    pub step: WorkspaceStep,
    pub status: String,
    pub validated: Option<Utf8PathBuf>,
}

impl WorkspaceDialog {
    #[must_use]
    pub fn new(current_path: String, recents: Vec<String>, initial: Option<String>) -> Self {
        let path = initial.unwrap_or(current_path);
        let path_cursor = path.len();
        let focus = if recents.is_empty() {
            WorkspaceField::Path
        } else {
            WorkspaceField::Recents
        };
        Self {
            path,
            path_cursor,
            recents,
            recent_cursor: 0,
            focus,
            step: WorkspaceStep::Edit,
            status: "Saisissez un chemin ou choisissez un récent · Entrée sur « Vérifier »".into(),
            validated: None,
        }
    }
}

/// Entrée affichable dans le sélecteur rewind.
#[derive(Debug, Clone)]
pub struct RewindChoiceView {
    pub message_index: usize,
    pub label: String,
    pub restore_text: String,
}

/// Sélecteur `/theme`.
#[derive(Debug, Clone)]
pub struct ThemeDialog {
    pub cursor: usize,
}

/// Sélecteur `/copy` — réponse complète ou bloc de code.
#[derive(Debug, Clone)]
pub struct CopyDialog {
    pub full_text: String,
    pub blocks: Vec<crate::engine::copy_cmd::CopyCodeBlock>,
    pub choices: Vec<crate::engine::copy_cmd::CopyChoice>,
    pub cursor: usize,
}

impl CopyDialog {
    #[must_use]
    pub fn new(
        full_text: String,
        blocks: Vec<crate::engine::copy_cmd::CopyCodeBlock>,
    ) -> Self {
        use crate::engine::copy_cmd::CopyChoice;
        let mut choices = vec![CopyChoice::FullResponse];
        for i in 0..blocks.len() {
            choices.push(CopyChoice::CodeBlock(i));
        }
        choices.push(CopyChoice::AlwaysFullResponse);
        Self {
            full_text,
            blocks,
            choices,
            cursor: 0,
        }
    }
}

/// Popup suggestions composer (`@` · `/` · skills).
pub use crate::engine::unified_suggestions::ComposerSuggestionDialog;

/// Sélecteur palette slash (`/`).
#[derive(Debug, Clone)]
pub struct SlashPaletteDialog {
    pub filter: String,
    pub matches: Vec<usize>,
    pub cursor: usize,
}

/// État UI persistant pendant la session TUI.
pub struct AppState {
    pub phase: AppPhase,
    pub theme: TuiThemeSetting,
    pub session_accent: Option<SessionAccent>,
    pub palette: ThemePalette,
    pub composer_mode: ComposerMode,
    pub composer_buffer: String,
    pub status_line: String,
    pub last_run: RunStatus,
    pub entries: Vec<LogEntry>,
    pub streaming: Option<String>,
    pub scroll: u16,
    pub prompt: Option<PromptDialog>,
    pub rewind: Option<RewindDialog>,
    pub theme_dialog: Option<ThemeDialog>,
    pub pending_ask: bool,
    /// Messages en attente pendant un run agent.
    pub queued_messages: usize,
    /// Sorties outils développées dans le fil (`e` : bash, file_read, grep, glob, web_fetch, web_search).
    pub expanded_tools: HashSet<ToolUseId>,
    /// Blocs phase repliés (index `PhaseOpen` dans `entries`).
    pub collapsed_phases: HashSet<usize>,
    /// Dernier état `todo_write` (panneau live).
    pub todo_snapshot: Option<TodoSnapshot>,
    /// Dernier plan de cours (`course_plan_write`).
    pub course_snapshot: Option<CourseSnapshot>,
    /// Serveurs MCP configurés (panneau live).
    pub mcp_snapshot: Option<crate::engine::McpPanelSnapshot>,
    /// Onboarding premier lancement.
    pub onboarding: Option<OnboardingDialog>,
    /// Connexion serveur IA (`/server`).
    pub ai_server: Option<AiServerDialog>,
    /// Changement workspace (`/workspace`).
    pub workspace_dialog: Option<WorkspaceDialog>,
    /// Toast éphémère (coin bas-droit).
    pub toast: Option<crate::widgets::toast::Toast>,
    /// Recherche dans le fil (`Ctrl+F`).
    pub transcript_search: Option<TranscriptSearchState>,
    pub slash_palette: Option<SlashPaletteDialog>,
    /// Sélecteur `/copy`.
    pub copy: Option<CopyDialog>,
    /// Complétion unifiée (`@` · `/` · skills).
    pub composer_suggestions: Option<ComposerSuggestionDialog>,
    /// Menu d'aide composer (`?`).
    pub composer_help: bool,
    /// Bannières contextuelles sous le header (§11.4 / §11.5).
    pub status_notices: Vec<crate::engine::notices::StatusNotice>,
    /// Bash en cours (`ToolStart` sans `ToolFinish` encore).
    pub active_bash: Option<ActiveBashRun>,
    /// Hook Pre/Post tool en cours.
    pub hook_progress: Option<ActiveHookProgress>,
    /// Viewer scrollable outil (`e` : bash, file_read, grep, glob, web_fetch, web_search, diff, lsp, skill, mcp, task).
    pub scroll_viewer: Option<ScrollViewerState>,
    /// Compteur frame pour spinner bash.
    pub ui_frame_tick: u8,
    /// Permissions/questions encore en file (hors modal courante).
    pub permission_queue_waiting: usize,
    /// Titre affichable (`ses_*.meta.json` ou id session).
    pub session_title: String,
    /// Début du run agent courant (spinner §3.7).
    pub run_started: Option<std::time::Instant>,
    /// Phase agent active (`PhaseEnter` sans `PhaseClose`).
    pub active_phase: Option<drox_engine::Phase>,
    /// Compteur d'invalidation du cache de rendu fil.
    pub log_revision: u64,
    /// Connexion IA validée (`/server` ou CLI explicite).
    pub llm_configured: bool,
    /// Cache lignes fil (hors streaming / bash live).
    log_render_cache: Option<crate::view::log_cache::LogRenderCache>,
}

/// Cible du dernier outil expansible dans le fil.
enum LatestExpandable<'a> {
    FileRead {
        id: ToolUseId,
        output: &'a Value,
    },
    Bash {
        id: ToolUseId,
        output: &'a Value,
        is_error: bool,
        user_mode: bool,
        command: Option<String>,
    },
    Grep {
        id: ToolUseId,
        output: &'a Value,
    },
    Glob {
        id: ToolUseId,
        output: &'a Value,
        pattern: Option<String>,
    },
    WebFetch {
        id: ToolUseId,
        output: &'a Value,
    },
    WebSearch {
        id: ToolUseId,
        output: &'a Value,
    },
    FileEdit {
        id: ToolUseId,
        output: &'a Value,
    },
    NotebookEdit {
        id: ToolUseId,
        output: &'a Value,
    },
    Lsp {
        id: ToolUseId,
        output: &'a Value,
    },
    SkillRead {
        id: ToolUseId,
        output: &'a Value,
    },
    Task {
        id: ToolUseId,
        output: &'a Value,
    },
    Mcp {
        id: ToolUseId,
        name: String,
        output: &'a Value,
    },
    ExitPlanMode {
        id: ToolUseId,
        plan: String,
    },
    SkillList {
        id: ToolUseId,
        output: &'a Value,
    },
    Worktree {
        id: ToolUseId,
        name: String,
        output: &'a Value,
    },
    PlanDocument {
        path: String,
        body: String,
    },
}

impl AppState {
    #[must_use]
    pub fn new() -> Self {
        let prefs = crate::engine::preferences::load_preferences();
        let palette = resolve_palette(prefs.theme, prefs.session_color);
        Self {
            theme: prefs.theme,
            session_accent: prefs.session_color,
            palette,
            status_line: "Prêt — @fichier · Ctrl+F fil · Ctrl+R historique · ! bash · Entrée envoyer".into(),
            entries: Vec::new(),
            streaming: None,
            scroll: 0,
            prompt: None,
            rewind: None,
            theme_dialog: None,
            pending_ask: false,
            queued_messages: 0,
            expanded_tools: HashSet::new(),
            collapsed_phases: HashSet::new(),
            todo_snapshot: None,
            course_snapshot: None,
            mcp_snapshot: None,
            onboarding: None,
            ai_server: None,
            workspace_dialog: None,
            toast: None,
            transcript_search: None,
            slash_palette: None,
            copy: None,
            composer_suggestions: None,
            composer_help: false,
            status_notices: Vec::new(),
            active_bash: None,
            hook_progress: None,
            scroll_viewer: None,
            ui_frame_tick: 0,
            permission_queue_waiting: 0,
            session_title: String::new(),
            run_started: None,
            active_phase: None,
            log_revision: 0,
            log_render_cache: None,
            phase: AppPhase::Idle,
            composer_mode: ComposerMode::Normal,
            composer_buffer: String::new(),
            last_run: RunStatus::None,
            llm_configured: false,
        }
    }

    pub fn push_entry(&mut self, entry: LogEntry) {
        self.entries.push(entry);
        const MAX: usize = 500;
        if self.entries.len() > MAX {
            self.entries.drain(0..self.entries.len() - MAX);
        }
        self.bump_log_revision();
    }

    fn bump_log_revision(&mut self) {
        self.log_revision = self.log_revision.wrapping_add(1);
        self.log_render_cache = None;
    }

    pub fn push_system(&mut self, text: impl Into<String>) {
        self.push_entry(LogEntry::System { text: text.into() });
    }

    pub fn push_user(&mut self, text: impl Into<String>) {
        self.push_entry(LogEntry::User { text: text.into() });
    }

    pub fn clear_transcript(&mut self) {
        self.entries.clear();
        self.streaming = None;
        self.scroll = 0;
        self.expanded_tools.clear();
        self.collapsed_phases.clear();
        self.todo_snapshot = None;
        self.course_snapshot = None;
        self.status_notices.clear();
        self.active_bash = None;
        self.hook_progress = None;
        self.scroll_viewer = None;
        self.run_started = None;
        self.active_phase = None;
        self.bump_log_revision();
    }

    /// Met à jour le panneau todos depuis un output `todo_write`.
    pub fn apply_todo_output(&mut self, output: &Value) {
        let Some(items) = parse_todo_items(output) else {
            return;
        };
        let summary = output
            .get("summary")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        self.todo_snapshot = Some(TodoSnapshot { items, summary });
    }

    /// Met à jour le panneau cours depuis un output `course_plan_write`.
    pub fn apply_course_plan_output(&mut self, output: &Value) {
        let Some((title, steps, summary)) = parse_course_plan(output) else {
            return;
        };
        self.course_snapshot = Some(CourseSnapshot {
            title,
            steps,
            summary,
        });
    }

    /// Reconstruit todos + plan cours depuis le fil (rejeu transcript).
    pub fn rebuild_side_panels_from_entries(&mut self) {
        self.todo_snapshot = None;
        self.course_snapshot = None;
        let todo_outputs: Vec<(String, Value)> = self
            .entries
            .iter()
            .filter_map(|entry| {
                if let LogEntry::ToolFinish {
                    name,
                    output,
                    is_error: false,
                    ..
                } = entry
                {
                    Some((name.clone(), output.clone()))
                } else {
                    None
                }
            })
            .collect();
        for (name, output) in todo_outputs {
            match name.as_str() {
                "todo_write" => self.apply_todo_output(&output),
                "course_plan_write" => self.apply_course_plan_output(&output),
                _ => {}
            }
        }
    }

    /// Bascule l'affichage développé du dernier outil expansible ou bloc phase.
    pub fn toggle_latest_expandable(&mut self) -> Option<&'static str> {
        match self.latest_expandable() {
            Some(LatestExpandable::FileRead { id, output }) => {
                let id = id.clone();
                let output = output.clone();
                let viewer = crate::view::file_read_viewer::FileReadViewerState::from_output(
                    id.clone(),
                    &output,
                )?;
                self.toggle_scroll_viewer(id, ScrollViewerState::FileRead(viewer))
                    .then_some("file_read")
            }
            Some(LatestExpandable::Bash {
                id,
                output,
                is_error,
                user_mode,
                command,
            }) => {
                let id = id.clone();
                let output = output.clone();
                let viewer = crate::view::bash_viewer::BashViewerState::from_output(
                    id.clone(),
                    &output,
                    is_error,
                    user_mode,
                    command,
                )?;
                self.toggle_scroll_viewer(id, ScrollViewerState::Bash(viewer))
                    .then_some("bash")
            }
            Some(LatestExpandable::Grep { id, output }) => {
                let id = id.clone();
                let output = output.clone();
                let viewer =
                    crate::view::grep_viewer::GrepViewerState::from_output(id.clone(), &output)?;
                self.toggle_scroll_viewer(id, ScrollViewerState::Grep(viewer))
                    .then_some("grep")
            }
            Some(LatestExpandable::Glob { id, output, pattern }) => {
                let id = id.clone();
                let output = output.clone();
                let viewer = crate::view::glob_viewer::GlobViewerState::from_output(
                    id.clone(),
                    &output,
                    pattern,
                )?;
                self.toggle_scroll_viewer(id, ScrollViewerState::Glob(viewer))
                    .then_some("glob")
            }
            Some(LatestExpandable::WebFetch { id, output }) => {
                let id = id.clone();
                let output = output.clone();
                let viewer = crate::view::web_fetch_viewer::WebFetchViewerState::from_output(
                    id.clone(),
                    &output,
                )?;
                self.toggle_scroll_viewer(id, ScrollViewerState::WebFetch(viewer))
                    .then_some("web_fetch")
            }
            Some(LatestExpandable::WebSearch { id, output }) => {
                let id = id.clone();
                let output = output.clone();
                let viewer = crate::view::web_search_viewer::WebSearchViewerState::from_output(
                    id.clone(),
                    &output,
                )?;
                self.toggle_scroll_viewer(id, ScrollViewerState::WebSearch(viewer))
                    .then_some("web_search")
            }
            Some(LatestExpandable::FileEdit { id, output }) => {
                let id = id.clone();
                let output = output.clone();
                let viewer = crate::view::tool_output::diff_viewer_from_output(
                    "file_edit",
                    id.clone(),
                    &output,
                )?;
                self.toggle_scroll_viewer(id, ScrollViewerState::Lines(viewer))
                    .then_some("file_edit")
            }
            Some(LatestExpandable::NotebookEdit { id, output }) => {
                let id = id.clone();
                let output = output.clone();
                let viewer = crate::view::tool_output::diff_viewer_from_output(
                    "notebook_edit",
                    id.clone(),
                    &output,
                )?;
                self.toggle_scroll_viewer(id, ScrollViewerState::Lines(viewer))
                    .then_some("notebook_edit")
            }
            Some(LatestExpandable::Lsp { id, output }) => {
                let id = id.clone();
                let output = output.clone();
                let viewer =
                    crate::view::extra_output::lsp_viewer_from_output(id.clone(), &output)?;
                self.toggle_scroll_viewer(id, ScrollViewerState::Lines(viewer))
                    .then_some("lsp")
            }
            Some(LatestExpandable::SkillRead { id, output }) => {
                let id = id.clone();
                let output = output.clone();
                let viewer = crate::view::extra_output::skill_read_viewer_from_output(
                    id.clone(),
                    &output,
                )?;
                self.toggle_scroll_viewer(id, ScrollViewerState::Lines(viewer))
                    .then_some("skill_read")
            }
            Some(LatestExpandable::Task { id, output }) => {
                let id = id.clone();
                let output = output.clone();
                let viewer =
                    crate::view::extra_output::task_viewer_from_output(id.clone(), &output)?;
                self.toggle_scroll_viewer(id, ScrollViewerState::Lines(viewer))
                    .then_some("task")
            }
            Some(LatestExpandable::Mcp { id, name, output }) => {
                let id = id.clone();
                let name = name.clone();
                let output = output.clone();
                let viewer = crate::view::mcp_output::mcp_viewer_from_output(
                    &name,
                    id.clone(),
                    &output,
                )?;
                self.toggle_scroll_viewer(id, ScrollViewerState::Lines(viewer))
                    .then_some("mcp")
            }
            Some(LatestExpandable::ExitPlanMode { id, plan }) => {
                let id = id.clone();
                let viewer = crate::view::extra_output::exit_plan_mode_viewer_from_plan(
                    id.clone(),
                    &plan,
                )?;
                self.toggle_scroll_viewer(id, ScrollViewerState::Lines(viewer))
                    .then_some("exit_plan_mode")
            }
            Some(LatestExpandable::SkillList { id, output }) => {
                let id = id.clone();
                let output = output.clone();
                let viewer = crate::view::extra_output::skill_list_viewer_from_output(
                    id.clone(),
                    &output,
                )?;
                self.toggle_scroll_viewer(id, ScrollViewerState::Lines(viewer))
                    .then_some("skill_list")
            }
            Some(LatestExpandable::Worktree { id, name, output }) => {
                let id = id.clone();
                let name = name.clone();
                let output = output.clone();
                let viewer = crate::view::extra_output::worktree_viewer_from_output(
                    id.clone(),
                    &name,
                    &output,
                )?;
                self.toggle_scroll_viewer(id, ScrollViewerState::Lines(viewer))
                    .then_some("worktree")
            }
            Some(LatestExpandable::PlanDocument { path, body }) => {
                let id = ToolUseId::from_string(format!("plan-doc:{path}"));
                let viewer =
                    crate::view::extra_output::plan_document_viewer(id.clone(), &path, &body)?;
                self.toggle_scroll_viewer(id, ScrollViewerState::Lines(viewer))
                    .then_some("plan")
            }
            None => self.toggle_latest_phase_block().then_some("phase"),
        }
    }

    fn latest_expandable(&self) -> Option<LatestExpandable<'_>> {
        for entry in self.entries.iter().rev() {
            match entry {
                LogEntry::PlanDocument {
                    path,
                    body,
                    truncated,
                } if crate::view::extra_output::plan_document_expandable(body, *truncated) => {
                    return Some(LatestExpandable::PlanDocument {
                        path: path.clone(),
                        body: body.clone(),
                    });
                }
                LogEntry::ToolFinish {
                    id,
                    name,
                    output,
                    is_error: false,
                } if name == "file_read" => {
                    return Some(LatestExpandable::FileRead {
                        id: id.clone(),
                        output,
                    });
                }
                LogEntry::ToolFinish {
                    id,
                    name,
                    output,
                    is_error,
                } if name == "bash" => {
                    return Some(LatestExpandable::Bash {
                        id: id.clone(),
                        output,
                        is_error: *is_error,
                        user_mode: false,
                        command: output
                            .get("command")
                            .and_then(Value::as_str)
                            .map(str::to_string),
                    });
                }
                LogEntry::ToolFinish {
                    id,
                    name,
                    output,
                    is_error: false,
                } if name == "grep" => {
                    return Some(LatestExpandable::Grep {
                        id: id.clone(),
                        output,
                    });
                }
                LogEntry::ToolFinish {
                    id,
                    name,
                    output,
                    is_error: false,
                } if name == "glob" => {
                    let pattern = self.glob_pattern_for(id);
                    return Some(LatestExpandable::Glob {
                        id: id.clone(),
                        output,
                        pattern,
                    });
                }
                LogEntry::ToolFinish {
                    id,
                    name,
                    output,
                    is_error: false,
                } if name == "web_fetch" => {
                    return Some(LatestExpandable::WebFetch {
                        id: id.clone(),
                        output,
                    });
                }
                LogEntry::ToolFinish {
                    id,
                    name,
                    output,
                    is_error: false,
                } if name == "web_search" => {
                    return Some(LatestExpandable::WebSearch {
                        id: id.clone(),
                        output,
                    });
                }
                LogEntry::ToolFinish {
                    id,
                    name,
                    output,
                    is_error: false,
                } if name == "file_edit"
                    && crate::view::tool_output::diff_is_expandable(output) =>
                {
                    return Some(LatestExpandable::FileEdit {
                        id: id.clone(),
                        output,
                    });
                }
                LogEntry::ToolFinish {
                    id,
                    name,
                    output,
                    is_error: false,
                } if name == "notebook_edit"
                    && crate::view::tool_output::diff_is_expandable(output) =>
                {
                    return Some(LatestExpandable::NotebookEdit {
                        id: id.clone(),
                        output,
                    });
                }
                LogEntry::ToolFinish {
                    id,
                    name,
                    output,
                    is_error: false,
                } if name == "lsp" && crate::view::extra_output::lsp_is_expandable(output) => {
                    return Some(LatestExpandable::Lsp {
                        id: id.clone(),
                        output,
                    });
                }
                LogEntry::ToolFinish {
                    id,
                    name,
                    output,
                    is_error: false,
                } if name == "skill_read"
                    && crate::view::extra_output::skill_read_is_expandable(output) =>
                {
                    return Some(LatestExpandable::SkillRead {
                        id: id.clone(),
                        output,
                    });
                }
                LogEntry::ToolFinish {
                    id,
                    name,
                    output,
                    is_error: false,
                } if name == "task" && crate::view::extra_output::task_is_expandable(output) => {
                    return Some(LatestExpandable::Task {
                        id: id.clone(),
                        output,
                    });
                }
                LogEntry::ToolFinish {
                    id,
                    name,
                    output,
                    is_error: false,
                } if crate::view::mcp_output::is_mcp_tool(name)
                    && crate::view::mcp_output::mcp_is_expandable(name, output) =>
                {
                    return Some(LatestExpandable::Mcp {
                        id: id.clone(),
                        name: name.clone(),
                        output,
                    });
                }
                LogEntry::ToolFinish {
                    id,
                    name,
                    output,
                    is_error: false,
                } if name == "exit_plan_mode" => {
                    if let Some(plan) = self.exit_plan_mode_plan_for(id) {
                        if crate::view::extra_output::exit_plan_mode_plan_expandable(&plan) {
                            return Some(LatestExpandable::ExitPlanMode {
                                id: id.clone(),
                                plan,
                            });
                        }
                    }
                }
                LogEntry::ToolFinish {
                    id,
                    name,
                    output,
                    is_error: false,
                } if name == "skill_list"
                    && crate::view::extra_output::skill_list_is_expandable(output) =>
                {
                    return Some(LatestExpandable::SkillList {
                        id: id.clone(),
                        output,
                    });
                }
                LogEntry::ToolFinish {
                    id,
                    name,
                    output,
                    is_error: false,
                } if (name == "git_worktree_enter" || name == "git_worktree_exit")
                    && crate::view::extra_output::worktree_is_expandable(output) =>
                {
                    return Some(LatestExpandable::Worktree {
                        id: id.clone(),
                        name: name.clone(),
                        output,
                    });
                }
                LogEntry::BashModeOutput {
                    id,
                    output,
                    is_error,
                } => {
                    let command = self.bash_mode_command_for(id);
                    return Some(LatestExpandable::Bash {
                        id: id.clone(),
                        output,
                        is_error: *is_error,
                        user_mode: true,
                        command,
                    });
                }
                _ => {}
            }
        }
        None
    }

    fn exit_plan_mode_plan_for(&self, id: &ToolUseId) -> Option<String> {
        self.entries.iter().find_map(|entry| {
            if let LogEntry::ToolStart {
                id: oid,
                name,
                arguments,
            } = entry
            {
                if oid == id && name == "exit_plan_mode" {
                    return arguments
                        .get("plan")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                }
            }
            None
        })
    }

    fn glob_pattern_for(&self, id: &ToolUseId) -> Option<String> {
        self.entries.iter().rev().find_map(|entry| {
            if let LogEntry::ToolStart {
                id: oid,
                name,
                arguments,
            } = entry
            {
                if oid == id && name == "glob" {
                    return arguments
                        .get("pattern")
                        .or_else(|| arguments.get("glob_pattern"))
                        .and_then(Value::as_str)
                        .map(str::to_string);
                }
            }
            None
        })
    }

    fn bash_mode_command_for(&self, id: &ToolUseId) -> Option<String> {
        let mut after_output = false;
        for entry in self.entries.iter().rev() {
            if let LogEntry::BashModeOutput { id: oid, .. } = entry {
                if oid == id {
                    after_output = true;
                }
                continue;
            }
            if after_output {
                if let LogEntry::BashModeInput { command } = entry {
                    return Some(command.clone());
                }
                break;
            }
        }
        None
    }

    fn toggle_scroll_viewer(&mut self, id: ToolUseId, viewer: ScrollViewerState) -> bool {
        if self
            .scroll_viewer
            .as_ref()
            .is_some_and(|v| v.tool_id() == &id)
        {
            self.scroll_viewer = None;
            return true;
        }
        self.scroll_viewer = Some(viewer);
        true
    }

    /// Replie automatiquement le dernier bloc `internal_reasoning` fermé.
    pub fn auto_collapse_internal_reasoning(&mut self) {
        let Some((idx, phase)) = self
            .entries
            .iter()
            .enumerate()
            .rev()
            .find_map(|(i, e)| match e {
                LogEntry::PhaseOpen { phase } => Some((i, *phase)),
                _ => None,
            })
        else {
            return;
        };
        if phase == drox_engine::Phase::InternalReasoning {
            self.collapsed_phases.insert(idx);
            self.bump_log_revision();
        }
    }

    /// Bascule le dernier bloc phase dans le fil.
    pub fn toggle_latest_phase_block(&mut self) -> bool {
        let Some(idx) = self.entries.iter().enumerate().rev().find_map(|(i, e)| {
            if matches!(e, LogEntry::PhaseOpen { .. }) {
                Some(i)
            } else {
                None
            }
        }) else {
            return false;
        };
        if self.collapsed_phases.contains(&idx) {
            self.collapsed_phases.remove(&idx);
        } else {
            self.collapsed_phases.insert(idx);
        }
        self.bump_log_revision();
        true
    }

    #[must_use]
    pub fn flattened_log_lines(&mut self) -> Vec<Line<'static>> {
        self.flattened_log_lines_internal(None)
    }

    #[must_use]
    pub fn flattened_log_lines_display(&mut self) -> Vec<Line<'static>> {
        let search = self.transcript_search.clone();
        self.flattened_log_lines_internal(search.as_ref())
    }

    fn ensure_log_cache(&mut self) {
        let key = crate::view::log_cache::cache_key(
            self.log_revision,
            &self.entries,
            &self.expanded_tools,
            &self.collapsed_phases,
        );
        if self
            .log_render_cache
            .as_ref()
            .is_some_and(|c| c.key == key)
        {
            return;
        }
        self.log_render_cache = Some(crate::view::log_cache::LogRenderCache::build(
            self.log_revision,
            &self.entries,
            &self.expanded_tools,
            &self.collapsed_phases,
        ));
    }

    fn flattened_log_lines_internal(
        &mut self,
        search: Option<&TranscriptSearchState>,
    ) -> Vec<Line<'static>> {
        self.ensure_log_cache();
        let mut lines = self
            .log_render_cache
            .as_ref()
            .map(|c| c.lines.clone())
            .unwrap_or_default();
        if let Some(ref s) = self.streaming {
            if !s.trim().is_empty() {
                let thinking = self.active_phase == Some(drox_engine::Phase::InternalReasoning);
                lines.extend(render::render_streaming(s, thinking));
            }
        }
        if self.phase == AppPhase::Running {
            lines.extend(crate::view::run_spinner::activity_lines(
                self.ui_frame_tick,
                self.run_started,
                self.active_phase,
                self.streaming.as_ref().is_some_and(|s| !s.trim().is_empty()),
                self.active_bash.is_some(),
            ));
        }
        if let Some(ref bash) = self.active_bash {
            lines.extend(bash_progress::progress_lines(bash, self.ui_frame_tick));
        }
        if let Some(ref hook) = self.hook_progress {
            lines.extend(hook_progress::progress_lines(hook, self.ui_frame_tick));
        }
        let Some(search) = search else {
            return lines;
        };
        if search.query.is_empty() {
            return lines;
        }
        let current = search.current_line_index();
        lines
            .iter()
            .enumerate()
            .map(|(idx, line)| {
                let is_current = current == Some(idx);
                let has_match = search.match_lines.contains(&idx);
                if has_match {
                    highlight_line(line, &search.query, is_current)
                } else {
                    line.clone()
                }
            })
            .collect()
    }
}

fn parse_todo_items(output: &Value) -> Option<Vec<TodoItemView>> {
    let arr = output.get("todos")?.as_array()?;
    let mut items = Vec::new();
    for v in arr {
        let id = v.get("id").and_then(Value::as_str)?.to_string();
        let content = v.get("content").and_then(Value::as_str)?.to_string();
        let status = v
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("pending")
            .to_string();
        items.push(TodoItemView {
            id,
            content,
            status,
        });
    }
    Some(items)
}

fn parse_course_plan(output: &Value) -> Option<(String, Vec<CourseStepView>, String)> {
    let title = output
        .get("courseTitle")
        .or_else(|| output.get("course_title"))
        .and_then(Value::as_str)?
        .to_string();
    let steps_arr = output.get("steps")?.as_array()?;
    let mut steps = Vec::new();
    for s in steps_arr {
        steps.push(CourseStepView {
            id: s.get("id").and_then(Value::as_str).unwrap_or("?").to_string(),
            title: s.get("title").and_then(Value::as_str).unwrap_or("?").to_string(),
            kind: s
                .get("kind")
                .and_then(Value::as_str)
                .unwrap_or("lesson")
                .to_string(),
            status: s
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("pending")
                .to_string(),
        });
    }
    let summary = output
        .get("summary")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    Some((title, steps, summary))
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
