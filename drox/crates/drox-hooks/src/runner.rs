//! Orchestration PreToolUse / PostToolUse.

use camino::Utf8Path;
use serde_json::{json, Value};
use tracing::debug;

use crate::config::ToolHooksConfig;
use crate::exec::{run_command_hook, CommandHookResult};
use crate::matcher::tool_matches;

/// Contexte d'un appel d'outil pour les hooks.
#[derive(Debug, Clone)]
pub struct ToolHookContext<'a> {
    pub tool_name: &'a str,
    pub tool_use_id: &'a str,
    pub tool_input: &'a Value,
    pub tool_response: Option<&'a Value>,
}

/// Issue d'un PreToolUse.
#[derive(Debug, Clone)]
pub enum PreHookOutcome {
    Continue,
    /// Exit code 2 — ne pas exécuter le tool ; message pour le modèle.
    Block { message: String },
}

/// Issue d'un PostToolUse.
#[derive(Debug, Clone)]
pub enum PostHookOutcome {
    /// Réponse tool éventuellement enrichie (stdout hook).
    Ok {
        tool_response: Value,
        model_appendix: Option<String>,
    },
}

impl ToolHooksConfig {
    /// Hooks avant exécution du tool (série uniquement en V1).
    pub async fn run_pre(
        &self,
        ctx: ToolHookContext<'_>,
        workspace_root: &Utf8Path,
    ) -> PreHookOutcome {
        if self.pre_tool_use.is_empty() {
            return PreHookOutcome::Continue;
        }
        let payload = json!({
            "tool_name": ctx.tool_name,
            "tool_input": ctx.tool_input,
            "tool_use_id": ctx.tool_use_id,
        });
        let input_str = payload.to_string();

        for entry in &self.pre_tool_use {
            if !tool_matches(&entry.matcher, ctx.tool_name) {
                continue;
            }
            for hook in &entry.hooks {
                if let Some(ref cond) = hook.if_condition {
                    if !crate::matcher::if_condition_matches(cond, ctx.tool_name, ctx.tool_input) {
                        continue;
                    }
                }
                debug!(
                    tool = ctx.tool_name,
                    command = %hook.command,
                    "PreToolUse hook"
                );
                let result = match run_command_hook(hook, &self.settings, workspace_root, &input_str)
                    .await
                {
                    Ok(r) => r,
                    Err(e) => {
                        return PreHookOutcome::Block {
                            message: format!("[PreToolUse hook error] {e}"),
                        };
                    }
                };
                if let Some(block) = interpret_pre_result(&result, ctx.tool_name) {
                    return PreHookOutcome::Block { message: block };
                }
            }
        }
        PreHookOutcome::Continue
    }

    /// Hooks après succès du tool.
    pub async fn run_post(
        &self,
        ctx: ToolHookContext<'_>,
        workspace_root: &Utf8Path,
        tool_response: Value,
    ) -> PostHookOutcome {
        if self.post_tool_use.is_empty() {
            return PostHookOutcome::Ok {
                tool_response,
                model_appendix: None,
            };
        }
        let payload = json!({
            "tool_name": ctx.tool_name,
            "tool_input": ctx.tool_input,
            "tool_response": &tool_response,
            "tool_use_id": ctx.tool_use_id,
        });
        let input_str = payload.to_string();
        let mut appendix_parts: Vec<String> = Vec::new();

        for entry in &self.post_tool_use {
            if !tool_matches(&entry.matcher, ctx.tool_name) {
                continue;
            }
            for hook in &entry.hooks {
                if let Some(ref cond) = hook.if_condition {
                    if !crate::matcher::if_condition_matches(cond, ctx.tool_name, ctx.tool_input) {
                        continue;
                    }
                }
                debug!(
                    tool = ctx.tool_name,
                    command = %hook.command,
                    "PostToolUse hook"
                );
                let result = match run_command_hook(hook, &self.settings, workspace_root, &input_str)
                    .await
                {
                    Ok(r) => r,
                    Err(e) => {
                        appendix_parts.push(format!("[PostToolUse hook error] {e}"));
                        continue;
                    }
                };
                if let Some(msg) = interpret_post_result(&result, ctx.tool_name) {
                    appendix_parts.push(msg);
                }
            }
        }

        let model_appendix = if appendix_parts.is_empty() {
            None
        } else {
            Some(appendix_parts.join("\n\n"))
        };

        PostHookOutcome::Ok {
            tool_response,
            model_appendix,
        }
    }
}

fn interpret_pre_result(result: &CommandHookResult, tool_name: &str) -> Option<String> {
    if result.timed_out {
        return Some(format!(
            "[PreToolUse:{tool_name}] hook timed out: {}",
            result.stderr.trim()
        ));
    }
    match result.exit_code {
        0 => None,
        2 => Some(format!(
            "[PreToolUse:{tool_name}] hook blocked this tool call.\n{}",
            result.stderr.trim()
        )),
        code => {
            tracing::warn!(
                tool = tool_name,
                code,
                stderr = %result.stderr,
                "PreToolUse hook non-zero exit (non-blocking)"
            );
            None
        }
    }
}

fn interpret_post_result(result: &CommandHookResult, tool_name: &str) -> Option<String> {
    if result.timed_out {
        return Some(format!(
            "[PostToolUse:{tool_name}] hook timed out: {}",
            result.stderr.trim()
        ));
    }
    match result.exit_code {
        0 => {
            let out = result.stdout.trim();
            if out.is_empty() {
                None
            } else {
                Some(format!("[PostToolUse:{tool_name}]\n{out}"))
            }
        }
        2 => Some(format!(
            "[PostToolUse:{tool_name}] hook reported an issue:\n{}",
            result.stderr.trim()
        )),
        code => {
            tracing::warn!(
                tool = tool_name,
                code,
                stderr = %result.stderr,
                "PostToolUse hook non-zero exit (non-blocking)"
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8Path;
    use serde_json::json;

    fn echo_exit_2_hook() -> ToolHooksConfig {
        let cmd = if cfg!(windows) {
            "echo block 1>&2 & exit /b 2"
        } else {
            "echo block >&2; exit 2"
        };
        ToolHooksConfig {
            pre_tool_use: vec![crate::config::HookMatcherEntry {
                matcher: "bash".into(),
                hooks: vec![crate::config::CommandHook {
                    hook_type: "command".into(),
                    command: cmd.into(),
                    timeout: Some(5),
                    if_condition: None,
                }],
            }],
            post_tool_use: Vec::new(),
            settings: Default::default(),
        }
    }

    #[tokio::test]
    async fn pre_tool_use_exit_2_blocks() {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8Path::from_path(dir.path()).unwrap();
        let hooks = echo_exit_2_hook();
        let ctx = ToolHookContext {
            tool_name: "bash",
            tool_use_id: "tu_test",
            tool_input: &json!({ "command": "echo hi" }),
            tool_response: None,
        };
        match hooks.run_pre(ctx, root).await {
            PreHookOutcome::Block { message } => {
                assert!(message.contains("blocked"), "{message}");
            }
            PreHookOutcome::Continue => panic!("expected block"),
        }
    }

    #[tokio::test]
    async fn post_tool_use_appends_stdout() {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8Path::from_path(dir.path()).unwrap();
        let hooks = ToolHooksConfig {
            pre_tool_use: Vec::new(),
            post_tool_use: vec![crate::config::HookMatcherEntry {
                matcher: "file_read".into(),
                hooks: vec![crate::config::CommandHook {
                    hook_type: "command".into(),
                    command: "echo hook-ok".into(),
                    timeout: Some(5),
                    if_condition: None,
                }],
            }],
            settings: Default::default(),
        };
        let ctx = ToolHookContext {
            tool_name: "file_read",
            tool_use_id: "tu_test",
            tool_input: &json!({ "path": "README.md" }),
            tool_response: None,
        };
        let PostHookOutcome::Ok {
            model_appendix, ..
        } = hooks
            .run_post(ctx, root, json!({ "content": "x" }))
            .await;
        let app = model_appendix.expect("stdout appendix");
        assert!(app.contains("hook-ok"), "{app}");
    }

    #[test]
    fn allowed_commands_enforced() {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8Path::from_path(dir.path()).unwrap();
        let hooks = ToolHooksConfig {
            pre_tool_use: vec![crate::config::HookMatcherEntry {
                matcher: "*".into(),
                hooks: vec![crate::config::CommandHook {
                    hook_type: "command".into(),
                    command: "npm run lint".into(),
                    timeout: Some(5),
                    if_condition: None,
                }],
            }],
            post_tool_use: Vec::new(),
            settings: crate::config::HooksSettings {
                default_timeout_secs: 5,
                allowed_commands: vec!["echo".into()],
            },
        };
        let rt = tokio::runtime::Runtime::new().unwrap();
        let ctx = ToolHookContext {
            tool_name: "file_edit",
            tool_use_id: "tu_test",
            tool_input: &json!({}),
            tool_response: None,
        };
        let outcome = rt.block_on(hooks.run_pre(ctx, root));
        assert!(
            matches!(outcome, PreHookOutcome::Block { .. }),
            "npm should be blocked by allowlist"
        );
    }
}
