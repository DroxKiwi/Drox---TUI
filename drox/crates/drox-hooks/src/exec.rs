//! Exécution d'une commande hook (stdin JSON, timeout, exit code).

use std::process::Stdio;
use std::time::Duration;

use camino::Utf8Path;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::timeout;

use crate::config::{command_allowed, CommandHook, HooksSettings};

/// Résultat d'une commande hook.
#[derive(Debug, Clone)]
pub struct CommandHookResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}

/// Exécute `hook.command` avec `input_json` sur stdin.
pub async fn run_command_hook(
    hook: &CommandHook,
    settings: &HooksSettings,
    workspace_root: &Utf8Path,
    input_json: &str,
) -> Result<CommandHookResult, String> {
    command_allowed(settings, &hook.command)?;
    let timeout_secs = hook.timeout.unwrap_or(settings.default_timeout_secs);
    let mut child = Command::new(if cfg!(windows) { "cmd" } else { "sh" });
    if cfg!(windows) {
        child.args(["/C", &hook.command]);
    } else {
        child.args(["-c", &hook.command]);
    }
    child
        .current_dir(workspace_root.as_std_path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = child
        .spawn()
        .map_err(|e| format!("hook spawn failed: {e}"))?;

    let run = async {
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(input_json.as_bytes())
                .await
                .map_err(|e| format!("hook stdin write: {e}"))?;
        }
        child
            .wait_with_output()
            .await
            .map_err(|e| format!("hook wait: {e}"))
    };

    let output = match timeout(Duration::from_secs(timeout_secs), run).await {
        Ok(Ok(out)) => out,
        Ok(Err(e)) => return Err(e),
        Err(_) => {
            return Ok(CommandHookResult {
                exit_code: -1,
                stdout: String::new(),
                stderr: format!("hook timed out after {timeout_secs}s"),
                timed_out: true,
            });
        }
    };

    Ok(CommandHookResult {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        timed_out: false,
    })
}
