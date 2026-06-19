//! Tool `bash` — exécute une commande shell sous le workspace.

use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::io::AsyncReadExt;
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tokio::time::timeout;

use crate::context::ToolContext;
use crate::error::ToolError;
use crate::progress::ShellProgressUpdate;
use crate::tool::Tool;

const DEFAULT_TIMEOUT_MS: u64 = 120_000;
const MAX_TIMEOUT_MS: u64 = 600_000;
const MAX_STREAM_BYTES: usize = 30 * 1024;
const PROGRESS_INTERVAL_MS: u64 = 150;
const TAIL_LINES: usize = 5;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BashInput {
    pub command: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

pub struct BashTool;

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

    fn description(&self) -> &str {
        "Exécute une commande shell dans le workspace (cmd /C sous Windows, sh -c ailleurs). \
         Renvoie { command, exit_code, stdout, stderr, timed_out, duration_ms }."
    }

    fn input_schema(&self) -> Value {
        serde_json::to_value(schemars::schema_for!(BashInput)).unwrap_or(Value::Null)
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<Value, ToolError> {
        let args: BashInput = serde_json::from_value(input)?;
        let command = args.command.trim();
        if command.is_empty() {
            return Err(ToolError::invalid_args("command must not be empty"));
        }

        if ctx.plan_mode {
            return Err(ToolError::plan_violation("bash"));
        }

        let timeout_ms = args
            .timeout_ms
            .map_or(DEFAULT_TIMEOUT_MS, |ms| ms.min(MAX_TIMEOUT_MS));

        let started = Instant::now();
        let cwd = ctx.effective_workspace();
        let mut cmd = build_shell_command(command);
        cmd.current_dir(cwd.as_std_path())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        let mut child = cmd
            .spawn()
            .map_err(|e| ToolError::io(cwd.clone(), e))?;

        let buffers = Arc::new(Mutex::new(StreamBuffers::default()));
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let mut tasks = Vec::new();
        if let Some(out) = stdout {
            let bufs = Arc::clone(&buffers);
            let ctx_pipe = ctx.clone();
            tasks.push(tokio::spawn(async move {
                read_pipe(out, StreamKind::Stdout, bufs, ctx_pipe, started).await;
            }));
        }
        if let Some(err) = stderr {
            let bufs = Arc::clone(&buffers);
            let ctx_pipe = ctx.clone();
            tasks.push(tokio::spawn(async move {
                read_pipe(err, StreamKind::Stderr, bufs, ctx_pipe, started).await;
            }));
        }

        let wait_result = timeout(
            Duration::from_millis(timeout_ms),
            wait_child(&mut child, tasks),
        )
        .await;

        let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let bufs = buffers.lock().await.clone();

        match wait_result {
            Ok(Ok(status)) => {
                let (stdout, stdout_trunc) = decode_truncated(bufs.stdout.as_bytes());
                let (stderr, stderr_trunc) = decode_truncated(bufs.stderr.as_bytes());
                report_shell_progress(ctx, &bufs.combined_display(), started);
                Ok(json!({
                    "command": command,
                    "exit_code": status.code(),
                    "stdout": stdout,
                    "stderr": stderr,
                    "stdout_truncated": stdout_trunc,
                    "stderr_truncated": stderr_trunc,
                    "timed_out": false,
                    "duration_ms": duration_ms,
                    "description": args.description,
                }))
            }
            Ok(Err(e)) => Err(ToolError::io(cwd.clone(), e)),
            Err(_elapsed) => {
                let _ = child.kill().await;
                report_shell_progress(ctx, &bufs.combined_display(), started);
                Ok(json!({
                    "command": command,
                    "exit_code": null,
                    "stdout": "",
                    "stderr": "",
                    "stdout_truncated": false,
                    "stderr_truncated": false,
                    "timed_out": true,
                    "duration_ms": duration_ms,
                    "description": args.description,
                }))
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
struct StreamBuffers {
    stdout: String,
    stderr: String,
}

impl StreamBuffers {
    fn combined_display(&self) -> String {
        let mut out = self.stdout.clone();
        if !self.stderr.is_empty() {
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(&self.stderr);
        }
        out
    }
}

#[derive(Clone, Copy)]
enum StreamKind {
    Stdout,
    Stderr,
}

async fn wait_child(
    child: &mut Child,
    pump_tasks: Vec<tokio::task::JoinHandle<()>>,
) -> std::io::Result<std::process::ExitStatus> {
    let status = child.wait().await?;
    for t in pump_tasks {
        let _ = t.await;
    }
    Ok(status)
}

async fn read_pipe<R>(
    mut reader: R,
    kind: StreamKind,
    buffers: Arc<Mutex<StreamBuffers>>,
    ctx: ToolContext,
    started: Instant,
) where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut chunk = [0u8; 4096];
    let mut last_report = Instant::now();
    loop {
        match reader.read(&mut chunk).await {
            Ok(0) => break,
            Ok(n) => {
                let piece = String::from_utf8_lossy(&chunk[..n]);
                {
                    let mut b = buffers.lock().await;
                    match kind {
                        StreamKind::Stdout => b.stdout.push_str(&piece),
                        StreamKind::Stderr => b.stderr.push_str(&piece),
                    }
                }
                if last_report.elapsed() >= Duration::from_millis(PROGRESS_INTERVAL_MS) {
                    let snapshot = buffers.lock().await.clone();
                    report_shell_progress(&ctx, &snapshot.combined_display(), started);
                    last_report = Instant::now();
                }
            }
            Err(_) => break,
        }
    }
    let snapshot = buffers.lock().await.clone();
    report_shell_progress(&ctx, &snapshot.combined_display(), started);
}

fn report_shell_progress(ctx: &ToolContext, full: &str, started: Instant) {
    let Some(sink) = ctx.tool_progress.as_ref() else {
        return;
    };
    let lines: Vec<&str> = full.lines().collect();
    let tail: String = lines
        .iter()
        .rev()
        .take(TAIL_LINES)
        .copied()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("\n");
    let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    sink.report_shell(ShellProgressUpdate {
        output: tail,
        full_output: full.to_string(),
        elapsed_ms,
        total_lines: lines.len(),
    });
}

fn build_shell_command(command: &str) -> Command {
    #[cfg(windows)]
    {
        let mut c = Command::new("cmd");
        c.arg("/C").arg(command);
        c
    }
    #[cfg(not(windows))]
    {
        let mut c = Command::new("sh");
        c.arg("-c").arg(command);
        c
    }
}

fn decode_truncated(bytes: &[u8]) -> (String, bool) {
    if bytes.len() <= MAX_STREAM_BYTES {
        return (String::from_utf8_lossy(bytes).into_owned(), false);
    }
    let head = &bytes[..MAX_STREAM_BYTES];
    let mut out = String::from_utf8_lossy(head).into_owned();
    out.push_str("\n…[truncated]");
    (out, true)
}

#[cfg(test)]
mod tests {
    use camino::Utf8PathBuf;
    use serde_json::json;

    use super::*;
    use crate::registry::ToolRegistry;

    fn make_ctx() -> ToolContext {
        let tmp = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        std::mem::forget(tmp);
        ToolContext::new(root, true)
    }

    #[tokio::test]
    async fn empty_command_rejected() {
        let ctx = make_ctx();
        let reg = ToolRegistry::with_simple_tools();
        let err = reg
            .execute_named("bash", &ctx, json!({ "command": "   " }))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidArgs(_)));
    }

    #[tokio::test]
    async fn plan_mode_blocks_bash() {
        let mut ctx = make_ctx();
        ctx.plan_mode = true;
        let reg = ToolRegistry::with_simple_tools();
        let err = reg
            .execute_named("bash", &ctx, json!({ "command": "echo hi" }))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::PlanModeViolation(_)));
    }

    #[tokio::test]
    async fn echo_runs_and_returns_stdout() {
        let ctx = make_ctx();
        let reg = ToolRegistry::with_simple_tools();
        let v = reg
            .execute_named("bash", &ctx, json!({ "command": "echo hello-drox" }))
            .await
            .unwrap();
        assert_eq!(v["timed_out"], false);
        assert_eq!(v["exit_code"], 0);
        let stdout = v["stdout"].as_str().unwrap_or("");
        assert!(
            stdout.contains("hello-drox"),
            "stdout did not contain expected token: {stdout:?}"
        );
    }
}
