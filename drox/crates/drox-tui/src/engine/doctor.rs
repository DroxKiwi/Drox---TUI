//! Diagnostic environnement (`/doctor`).

use std::path::Path;
use std::time::SystemTime;

use camino::{Utf8Path, Utf8PathBuf};
use drox_hooks::load_hooks_file;
use drox_mcp::McpJsonFile;
use drox_permissions::SettingsFile;

use super::EngineRuntime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CheckStatus {
    Ok,
    Warn,
    Fail,
}

#[derive(Debug, Clone)]
struct Check {
    status: CheckStatus,
    label: &'static str,
    detail: String,
}

impl EngineRuntime {
    /// Exécute les contrôles santé et renvoie les lignes pour le fil TUI.
    pub async fn run_doctor_checks(&self) -> Vec<String> {
        let mut checks = Vec::new();

        checks.push(self.check_version());
        checks.extend(self.check_workspace());
        checks.extend(self.check_sessions_dir());
        checks.extend(self.check_settings_files());
        checks.extend(self.check_hooks_files());
        checks.extend(self.check_mcp_config().await);
        checks.extend(self.check_shell_tools().await);
        checks.push(self.check_transcript().await);
        checks.push(self.check_memory_files().await);
        checks.push(self.check_llm().await);

        format_report(checks)
    }
}

fn format_report(checks: Vec<Check>) -> Vec<String> {
    let mut lines = vec!["Diagnostic Drox (/doctor)".into()];
    let mut ok = 0usize;
    let mut warn = 0usize;
    let mut fail = 0usize;

    for c in &checks {
        match c.status {
            CheckStatus::Ok => ok += 1,
            CheckStatus::Warn => warn += 1,
            CheckStatus::Fail => fail += 1,
        }
        lines.push(format!(
            "  [{}] {} — {}",
            status_tag(c.status),
            c.label,
            c.detail
        ));
    }

    lines.push(format!("Résumé : {ok} OK · {warn} avert. · {fail} échec(s)"));
    if fail > 0 {
        lines.push("Des échecs bloquent un run agent fiable — corriger avant usage intensif.".into());
    } else if warn > 0 {
        lines.push("Avertissements non bloquants — voir /config pour le détail.".into());
    }
    lines
}

fn status_tag(s: CheckStatus) -> &'static str {
    match s {
        CheckStatus::Ok => "OK",
        CheckStatus::Warn => "WARN",
        CheckStatus::Fail => "FAIL",
    }
}

fn check(label: &'static str, status: CheckStatus, detail: String) -> Check {
    Check {
        status,
        label,
        detail: detail,
    }
}

impl EngineRuntime {
    fn check_version(&self) -> Check {
        check(
            "drox-tui",
            CheckStatus::Ok,
            format!("v{}", env!("CARGO_PKG_VERSION")),
        )
    }

    fn check_workspace(&self) -> Vec<Check> {
        let p = self.workspace.as_std_path();
        if !p.is_dir() {
            return vec![check(
                "workspace",
                CheckStatus::Fail,
                format!("{} n'est pas un répertoire", self.workspace),
            )];
        }
        let readable = std::fs::read_dir(p).is_ok();
        vec![
            check(
                "workspace",
                if readable {
                    CheckStatus::Ok
                } else {
                    CheckStatus::Fail
                },
                self.workspace.to_string(),
            ),
            check(
                ".drox/",
                if self.workspace.join(".drox").is_dir() {
                    CheckStatus::Ok
                } else {
                    CheckStatus::Warn
                },
                if self.workspace.join(".drox").is_dir() {
                    "présent".to_string()
                } else {
                    "absent (créé à la demande par certains tools)".to_string()
                },
            ),
        ]
    }

    fn check_sessions_dir(&self) -> Vec<Check> {
        let path = self.sessions_dir.as_std_path();
        if !path.is_dir() {
            return vec![check(
                "sessions",
                CheckStatus::Warn,
                format!("{} absent — sera créé au premier transcript", self.sessions_dir),
            )];
        }
        let writable = dir_writable(path);
        vec![check(
            "sessions",
            if writable {
                CheckStatus::Ok
            } else {
                CheckStatus::Fail
            },
            format!(
                "{} ({})",
                self.sessions_dir,
                if writable {
                    "inscriptible"
                } else {
                    "non inscriptible"
                }
            ),
        )]
    }

    fn check_settings_files(&self) -> Vec<Check> {
        if self.boot_config().no_settings {
            return vec![check(
                "settings",
                CheckStatus::Warn,
                "fichiers ignorés (--no-settings)".to_string(),
            )];
        }
        let mut out = Vec::new();
        let paths = settings_paths(&self.workspace);
        out.push(parse_settings_file("settings user", &paths.user));
        out.push(parse_settings_file("settings projet", &paths.project));
        out.push(parse_settings_file("settings local", &paths.local));
        out
    }

    fn check_hooks_files(&self) -> Vec<Check> {
        let mut out = Vec::new();
        if let Some(user) = user_hooks_path() {
            out.push(parse_hooks_file("hooks user", &user));
        }
        out.push(parse_hooks_file(
            "hooks projet",
            &self.workspace.join(".drox/hooks.json"),
        ));
        if out.iter().all(|c| c.detail.contains("absent")) {
            out.push(check(
                "hooks actifs",
                CheckStatus::Ok,
                "aucun fichier hooks (optionnel)".to_string(),
            ));
        } else if self.hooks_active() {
            out.push(check(
                "hooks actifs",
                CheckStatus::Ok,
                "chargés en runtime".to_string(),
            ));
        }
        out
    }

    async fn check_mcp_config(&self) -> Vec<Check> {
        let candidates = [
            self.workspace.join(".mcp.json"),
            self.workspace.join("mcp.json"),
        ];
        let mut found = false;
        let mut out = Vec::new();
        for path in candidates {
            if !path.is_file() {
                continue;
            }
            found = true;
            match McpJsonFile::load(path.as_std_path()).await {
                Ok(file) => {
                    let n = file.mcp_servers.len();
                    out.push(check(
                        "mcp.json",
                        CheckStatus::Ok,
                        format!("{path} — {n} serveur(s)"),
                    ));
                }
                Err(e) => {
                    out.push(check(
                        "mcp.json",
                        CheckStatus::Fail,
                        format!("{path} : {e}"),
                    ));
                }
            }
        }
        if !found {
            out.push(check(
                "mcp",
                CheckStatus::Ok,
                "aucun .mcp.json (optionnel)".to_string(),
            ));
        }
        out
    }

    async fn check_shell_tools(&self) -> Vec<Check> {
        let shell = if cfg!(windows) {
            ("powershell", vec!["-NoProfile", "-Command", "$PSVersionTable.PSVersion"])
        } else {
            ("sh", vec!["-c", "echo ok"])
        };
        vec![
            probe_command("git", "git", &["--version"]).await,
            probe_command("shell", shell.0, &shell.1).await,
        ]
    }

    async fn check_transcript(&self) -> Check {
        let path = self.transcript_path();
        let parent = path.parent();
        let parent_writable = parent.is_some_and(|p| dir_writable(p.as_std_path()));
        let exists = path.is_file();
        let detail = if exists {
            let lines = std::fs::read_to_string(path.as_std_path())
                .map(|s| s.lines().filter(|l| !l.trim().is_empty()).count())
                .unwrap_or(0);
            format!("{path} ({lines} lignes)")
        } else {
            format!("{path} (nouveau)")
        };
        check(
            "transcript",
            if parent_writable {
                CheckStatus::Ok
            } else {
                CheckStatus::Fail
            },
            detail,
        )
    }

    async fn check_memory_files(&self) -> Check {
        match drox_engine::load_memdir(self.workspace.as_path()).await {
            Ok(mem) => {
                let n = usize::from(mem.memory_md.is_some()) + usize::from(mem.drox_md.is_some());
                check(
                    "mémoire workspace",
                    CheckStatus::Ok,
                    if n > 0 {
                        format!("{n} fichier(s) MEMORY.md/DROX.md")
                    } else {
                        "MEMORY.md / DROX.md absents (optionnel)".to_string()
                    },
                )
            }
            Err(e) => check(
                "mémoire workspace",
                CheckStatus::Warn,
                format!("lecture : {e:#}"),
            ),
        }
    }

    async fn check_llm(&self) -> Check {
        let llm = self.llm();
        let server = llm.server_url().to_string();
        let model = llm.configured_model().to_string();
        match llm.list_installed_models().await {
            Ok(models) => {
                if models.is_empty() {
                    return check(
                        "LLM",
                        CheckStatus::Warn,
                        format!("{server} joignable mais aucun modèle listé"),
                    );
                }
                if model_installed(&model, &models) {
                    check(
                        "LLM",
                        CheckStatus::Ok,
                        format!("{server} — modèle `{model}` présent ({})", models.len()),
                    )
                } else {
                    let sample: Vec<_> = models.iter().take(5).cloned().collect();
                    check(
                        "LLM",
                        CheckStatus::Fail,
                        format!(
                            "{server} — modèle `{model}` absent parmi {} (ex. {})",
                            models.len(),
                            sample.join(", ")
                        ),
                    )
                }
            }
            Err(e) => {
                let retryable = e.is_retryable();
                check(
                    "LLM",
                    if retryable {
                        CheckStatus::Fail
                    } else {
                        CheckStatus::Warn
                    },
                    format!("{server} /api/tags : {e}"),
                )
            }
        }
    }
}

fn model_installed(configured: &str, installed: &[String]) -> bool {
    let base = configured.split(':').next().unwrap_or(configured);
    installed.iter().any(|name| {
        name == configured
            || name.starts_with(&format!("{configured}:"))
            || name.starts_with(&format!("{base}:"))
            || name.split(':').next() == Some(base)
    })
}

async fn probe_command(label: &'static str, program: &str, args: &[&str]) -> Check {
    match tokio::process::Command::new(program)
        .args(args)
        .output()
        .await
    {
        Ok(out) if out.status.success() => {
            let version = String::from_utf8_lossy(&out.stdout)
                .lines()
                .next()
                .unwrap_or("ok")
                .trim()
                .to_string();
            check(label, CheckStatus::Ok, version)
        }
        Ok(out) => check(
            label,
            CheckStatus::Warn,
            format!(
                "code {}",
                out.status.code().unwrap_or(-1)
            ),
        ),
        Err(e) => check(label, CheckStatus::Warn, format!("introuvable ou erreur : {e}")),
    }
}

fn parse_settings_file(label: &'static str, path: &Path) -> Check {
    if !path.is_file() {
        return check(label, CheckStatus::Ok, format!("{} (absent)", path.display()));
    }
    match SettingsFile::load(path) {
        Ok(s) => {
            let n = s.permissions.allow.len() + s.permissions.ask.len() + s.permissions.deny.len();
            let mode = s
                .mode
                .map(|m| format!(" · mode={m:?}"))
                .unwrap_or_default();
            check(
                label,
                CheckStatus::Ok,
                format!("{} — {n} règle(s){mode}", path.display()),
            )
        }
        Err(e) => check(
            label,
            CheckStatus::Fail,
            format!("{} : {e}", path.display()),
        ),
    }
}

fn parse_hooks_file(label: &'static str, path: &Utf8Path) -> Check {
    if !path.is_file() {
        return check(label, CheckStatus::Ok, format!("{path} (absent)"));
    }
    match load_hooks_file(path) {
        Ok(f) => {
            let n = f.pre_tool_use.len() + f.post_tool_use.len();
            check(label, CheckStatus::Ok, format!("{path} — {n} matcher(s)"))
        }
        Err(e) => check(label, CheckStatus::Fail, format!("{path} : {e}")),
    }
}

struct SettingsPaths {
    user: std::path::PathBuf,
    project: std::path::PathBuf,
    local: std::path::PathBuf,
}

fn settings_paths(workspace: &Utf8Path) -> SettingsPaths {
    let user = dirs::home_dir()
        .map(|h| h.join(".drox").join("settings.json"))
        .unwrap_or_else(|| Path::new("/nonexistent").to_path_buf());
    let drox = workspace.as_std_path().join(".drox");
    SettingsPaths {
        user,
        project: drox.join("settings.json"),
        local: drox.join("settings.local.json"),
    }
}

fn user_hooks_path() -> Option<Utf8PathBuf> {
    dirs::home_dir().and_then(|h| Utf8PathBuf::from_path_buf(h.join(".drox/hooks.json")).ok())
}

fn dir_writable(path: &Path) -> bool {
    if !path.is_dir() {
        return false;
    }
    let probe = path.join(format!(
        ".drox_doctor_{}",
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
    {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_installed_matches_tag() {
        let models = vec!["llama3.2:latest".into(), "gemma2:2b".into()];
        assert!(model_installed("llama3.2", &models));
        assert!(model_installed("llama3.2:latest", &models));
        assert!(!model_installed("mistral", &models));
    }
}
