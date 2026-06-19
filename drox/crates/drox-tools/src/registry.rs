//! Registre des tools disponibles.

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::Value;

use crate::context::ToolContext;
use crate::error::ToolError;
use crate::simple::{
    AskUserQuestionTool, BashTool, DeletePathTool, ExitPlanModeTool, FileEditTool, FileReadTool,
    FileWriteTool, GlobTool, GrepTool, LspTool, MemoryListTool, MemoryReadTool, NotebookEditTool,
    SkillListTool, SkillReadTool, GitWorktreeEnterTool, GitWorktreeExitTool, CopyPathTool,
    CoursePlanWriteTool,     ScopeDeferTool, SessionCompactTool, SessionEndTool, SessionSearchTool, SessionNoteTool,
    TaskTool, TodoWriteTool, WebFetchTool, WebSearchTool, WorkspaceMapNoteTool, WorkspaceMapReadTool,
};
use crate::tool::{DynTool, Tool};

fn coerce_tool<T: Tool + Sized + 'static>(tool: T) -> DynTool {
    Arc::new(tool)
}

/// Registre thread-safe des tools par nom.
#[derive(Clone, Default)]
pub struct ToolRegistry {
    tools: HashMap<String, DynTool>,
}

impl ToolRegistry {
    /// Crée un registre vide.
    #[must_use]
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Registre par défaut (sprints 1.3 + 1.5 + 2.2.4 + M1).
    ///
    /// Tools enregistrés :
    /// - `file_read`, `file_write`, `delete_path`, `grep`, `glob` (1.3)
    /// - `file_edit`, `notebook_edit`, `web_fetch`, `ask_user_question`, `exit_plan_mode` (1.5)
    /// - `bash` (2.2.4) — souvent shadowé par un `RemoteTool` côté serveur
    ///   JSON-RPC lorsque le client déclare la capability.
    /// - `session_note`, `memory_read`, `memory_list` (M1, mémoire de session).
    ///   `session_note` est inerte (renvoie une erreur explicite) si le
    ///   moteur n'a pas câblé `ToolContext::session_notes`.
    /// - **`session_end`** — enregistré pour cohérence / stub CLI ; **non
    ///   exposé au LLM** (filtré dans `drox-engine::agent::build_tool_specs`) :
    ///   la clôture de session est la commande `/session_end` côté utilisateur.
    /// - **`session_search`** — recherche mémoire longue (client IDE).
    /// - **`session_compact`** — compaction transcript `session.compact` (client IDE).
    #[must_use]
    pub fn with_simple_tools() -> Self {
        let mut reg = Self::new();
        reg.register(coerce_tool(FileReadTool));
        reg.register(coerce_tool(FileWriteTool));
        reg.register(coerce_tool(DeletePathTool));
        reg.register(coerce_tool(CopyPathTool));
        reg.register(coerce_tool(GrepTool));
        reg.register(coerce_tool(GlobTool));
        reg.register(coerce_tool(FileEditTool));
        reg.register(coerce_tool(NotebookEditTool));
        reg.register(coerce_tool(WebFetchTool));
        reg.register(coerce_tool(WebSearchTool));
        reg.register(coerce_tool(LspTool));
        reg.register(coerce_tool(AskUserQuestionTool));
        reg.register(coerce_tool(ExitPlanModeTool));
        reg.register(coerce_tool(BashTool));
        reg.register(coerce_tool(TodoWriteTool));
        reg.register(coerce_tool(CoursePlanWriteTool));
        reg.register(coerce_tool(ScopeDeferTool));
        reg.register(coerce_tool(WorkspaceMapReadTool));
        reg.register(coerce_tool(WorkspaceMapNoteTool));
        reg.register(coerce_tool(SessionNoteTool));
        reg.register(coerce_tool(MemoryReadTool));
        reg.register(coerce_tool(MemoryListTool));
        reg.register(coerce_tool(SkillReadTool));
        reg.register(coerce_tool(SkillListTool));
        reg.register(coerce_tool(GitWorktreeEnterTool));
        reg.register(coerce_tool(GitWorktreeExitTool));
        reg.register(coerce_tool(SessionCompactTool));
        reg.register(coerce_tool(SessionEndTool));
        reg.register(coerce_tool(SessionSearchTool));
        reg
    }

    /// Enregistre `task` (sous-agent Explore, §2.10). À appeler uniquement si
    /// `SubagentSettings::enabled` est vrai.
    pub fn register_subagent_task(&mut self) {
        self.register(coerce_tool(TaskTool));
    }

    /// Enregistre un tool (écrase si le nom existe déjà).
    pub fn register(&mut self, tool: DynTool) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    /// Liste les noms des tools enregistrés (ordre non garanti).
    #[must_use]
    pub fn names(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }

    /// Récupère un tool par son nom.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<DynTool> {
        self.tools.get(name).cloned()
    }

    /// Retire un tool du registre (`true` s'il existait).
    pub fn remove(&mut self, name: &str) -> bool {
        self.tools.remove(name).is_some()
    }

    /// `true` si le tool peut s'exécuter en parallèle avec d'autres du même tour.
    #[must_use]
    pub fn is_concurrency_safe(&self, name: &str) -> bool {
        self.get(name)
            .is_some_and(|t| t.is_concurrency_safe())
    }

    /// Exécute un tool par nom.
    pub async fn execute_named(
        &self,
        name: &str,
        ctx: &ToolContext,
        input: Value,
    ) -> Result<Value, ToolError> {
        let tool = self
            .get(name)
            .ok_or_else(|| ToolError::UnknownTool(name.to_string()))?;
        tool.execute(ctx, input).await
    }
}
