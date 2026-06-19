//! Sous-agents (§2.10) — V1 : type **Explore** (lecture seule), tool `task`.

use std::sync::Arc;

use async_trait::async_trait;
use drox_llm::{ChatOptions, LlmClient};
use drox_tools::{
    DynTool, FileReadTool, GlobTool, GrepTool, LspTool, SubagentExecutor, SubagentSettings,
    ToolContext, ToolError, ToolRegistry, WebFetchTool, WebSearchTool, WorkspaceMapReadTool,
};
use futures::StreamExt;
use tokio::sync::Semaphore;
use tracing::{debug, warn};

use crate::agent::{Agent, AgentConfig};
use crate::error::EngineError;
use crate::event::AgentEvent;

fn coerce_tool<T: drox_tools::Tool + Sized + 'static>(tool: T) -> DynTool {
    Arc::new(tool)
}

/// Registre minimal pour un sous-agent Explore (pas de `task`, bash, écriture).
#[must_use]
pub fn explore_tool_registry() -> ToolRegistry {
    let mut reg = ToolRegistry::new();
    reg.register(coerce_tool(FileReadTool));
    reg.register(coerce_tool(GlobTool));
    reg.register(coerce_tool(GrepTool));
    reg.register(coerce_tool(LspTool));
    reg.register(coerce_tool(WebFetchTool));
    reg.register(coerce_tool(WebSearchTool));
    reg.register(coerce_tool(WorkspaceMapReadTool));
    reg
}

const EXPLORE_SYSTEM_PROMPT: &str = "You are an **Explore** sub-agent (read-only) spawned by the main Drox agent.\n\
    \n\
    Your job:\n\
    - Search the codebase with `grep`, `glob`, `file_read`, `lsp` as needed.\n\
    - Do **not** modify files, run shell commands, or spawn nested sub-agents.\n\
    - Produce a **concise structured report** for the parent agent: findings, file paths, \
      and a short recommendation.\n\
    - Use `[phase: answering]` for your report, then `[phase: done]` when finished.\n\
    \n\
    Prefer targeted tool use over exhaustive listing. If the task is narrow, stop early.";

fn thoroughness_supplement(thoroughness: Option<&str>) -> &'static str {
    let Some(raw) = thoroughness else {
        return "";
    };
    match raw.trim().to_ascii_lowercase().as_str() {
        "quick" => "\n\nThoroughness: **quick** — minimal tool calls, fastest answer.",
        "medium" => "\n\nThoroughness: **medium** — balanced search depth.",
        "very thorough" | "very_thorough" | "very-thorough" => {
            "\n\nThoroughness: **very thorough** — search broadly across naming conventions and folders."
        }
        _ => "",
    }
}

/// Exécuteur moteur branché sur `ToolContext::subagent_executor`.
pub struct EngineSubagentExecutor {
    llm: Arc<dyn LlmClient>,
    chat_options: ChatOptions,
    settings: SubagentSettings,
    semaphore: Arc<Semaphore>,
}

impl EngineSubagentExecutor {
    #[must_use]
    pub fn new(
        llm: Arc<dyn LlmClient>,
        chat_options: ChatOptions,
        settings: SubagentSettings,
    ) -> Self {
        let permits = settings.max_concurrent.max(1);
        Self {
            llm,
            chat_options,
            settings,
            semaphore: Arc::new(Semaphore::new(permits)),
        }
    }
}

#[async_trait]
impl SubagentExecutor for EngineSubagentExecutor {
    async fn run_explore(
        &self,
        description: String,
        thoroughness: Option<String>,
        parent_ctx: &ToolContext,
    ) -> Result<String, ToolError> {
        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|_| ToolError::remote("subagent semaphore closed"))?;

        let mut system = EXPLORE_SYSTEM_PROMPT.to_string();
        system.push_str(thoroughness_supplement(thoroughness.as_deref()));

        let explore_ctx = ToolContext {
            workspace_root: parent_ctx.effective_workspace(),
            apply_fs_writes: false,
            plan_mode: true,
            user_asker: None,
            session_notes: None,
            mcp_hub: None,
            scope_deferred: None,
            workspace_map: parent_ctx.workspace_map.clone(),
            drox_ignore: parent_ctx.drox_ignore.clone(),
            subagent_settings: None,
            subagent_executor: None,
            tool_progress: None,
        };

        let registry = Arc::new(explore_tool_registry());
        let agent = Agent::new(
            self.llm.clone(),
            registry,
            explore_ctx,
            AgentConfig {
                system_prompt: Some(system),
                max_iterations: self.settings.max_iterations.max(1),
                chat_options: self.chat_options.clone(),
                permissions: None,
                context: None,
                transcript: None,
                memory: None,
                transcript_session_id: None,
                workspace_fingerprint: String::new(),
                max_parallel_tool_calls: 4,
                tool_hooks: None,
                run_objective: None,
            },
        );

        let prompt = format!(
            "## Subagent task\n\n{}\n\nReturn your findings as a structured report for the parent agent.",
            description.trim()
        );

        debug!(
            max_iter = self.settings.max_iterations,
            "explore subagent start"
        );

        let mut stream = agent.run(prompt);
        let mut report = String::new();
        let mut hit_max = false;

        while let Some(event) = stream.next().await {
            match event {
                Ok(AgentEvent::TextDelta { text }) => report.push_str(&text),
                Ok(AgentEvent::Stop { .. }) => break,
                Err(EngineError::MaxIterations(_)) => {
                    hit_max = true;
                    break;
                }
                Err(e) => {
                    warn!(error = %e, "explore subagent engine error");
                    return Err(ToolError::remote(format!("subagent failed: {e}")));
                }
                _ => {}
            }
        }

        let trimmed = report.trim();
        if trimmed.is_empty() {
            return Ok(
                "(Sous-agent Explore terminé sans texte — vérifiez les logs ou relancez avec une description plus précise.)"
                    .to_string(),
            );
        }
        if hit_max {
            Ok(format!(
                "{trimmed}\n\n---\n*(Rapport tronqué : plafond d'itérations sous-agent atteint.)*"
            ))
        } else {
            Ok(trimmed.to_string())
        }
    }
}
