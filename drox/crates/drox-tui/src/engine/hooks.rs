use camino::{Utf8Path, Utf8PathBuf};
use drox_engine::ToolHooksConfig;
use drox_hooks::{CommandHook, HookMatcherEntry};

/// Formate la config hooks pour affichage TUI.
#[must_use]
pub fn format_hooks_config(
    workspace: &Utf8Path,
    config: &ToolHooksConfig,
    active_in_runtime: bool,
) -> Vec<String> {
    let project_path = workspace.join(".drox/hooks.json");
    let user_path = dirs::home_dir()
        .and_then(|h| Utf8PathBuf::from_path_buf(h.join(".drox").join("hooks.json")).ok());

    let mut lines = vec![
        "Hooks tool (PreToolUse / PostToolUse)".into(),
        format!(
            "  runtime : {}",
            if active_in_runtime {
                "actifs pour les prochains runs"
            } else {
                "aucun hook actif en mémoire"
            }
        ),
        format_file_line("projet", &project_path),
    ];
    if let Some(ref user) = user_path {
        lines.push(format_file_line("utilisateur", user));
    }

    if !config.is_enabled() {
        lines.push("(aucune entrée Pre/Post — fichiers absents ou vides)".into());
        return lines;
    }

    lines.push(format!(
        "  timeout défaut : {}s",
        config.settings.default_timeout_secs
    ));
    if !config.settings.allowed_commands.is_empty() {
        lines.push(format!(
            "  allowed_commands : {}",
            config.settings.allowed_commands.join(", ")
        ));
    }

    append_phase("PreToolUse", &config.pre_tool_use, &mut lines);
    append_phase("PostToolUse", &config.post_tool_use, &mut lines);
    lines
}

fn format_file_line(label: &str, path: &Utf8Path) -> String {
    let status = if path.is_file() {
        "présent"
    } else {
        "absent"
    };
    format!("  {label} : {path} ({status})")
}

fn append_phase(name: &str, entries: &[HookMatcherEntry], lines: &mut Vec<String>) {
    lines.push(format!("— {name} ({} matcher(s))", entries.len()));
    if entries.is_empty() {
        lines.push("  (vide)".into());
        return;
    }
    for entry in entries {
        lines.push(format!(
            "  matcher `{}` — {} hook(s)",
            entry.matcher,
            entry.hooks.len()
        ));
        for (i, hook) in entry.hooks.iter().enumerate() {
            lines.push(format!("    [{}] {}", i + 1, summarize_hook(hook)));
        }
    }
}

fn summarize_hook(hook: &CommandHook) -> String {
    let mut parts = vec![format!("$ {}", truncate_cmd(&hook.command, 96))];
    if let Some(cond) = hook.if_condition.as_deref().filter(|s| !s.is_empty()) {
        parts.push(format!("if `{cond}`"));
    }
    if let Some(t) = hook.timeout {
        parts.push(format!("timeout={t}s"));
    }
    parts.join(" · ")
}

fn truncate_cmd(cmd: &str, max: usize) -> String {
    let one_line = cmd.lines().next().unwrap_or(cmd).trim();
    if one_line.len() <= max {
        one_line.to_string()
    } else {
        format!("{}…", &one_line[..max])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use drox_hooks::CommandHook;

    #[test]
    fn empty_hooks_message() {
        let ws = Utf8Path::new(".");
        let cfg = ToolHooksConfig::empty();
        let lines = format_hooks_config(ws, &cfg, false);
        assert!(lines.iter().any(|l| l.contains("aucune entrée")));
    }

    #[test]
    fn summarizes_hook_with_if() {
        let s = summarize_hook(&CommandHook {
            hook_type: "command".to_string(),
            command: "echo test".to_string(),
            timeout: Some(5),
            if_condition: Some("bash".to_string()),
        });
        assert!(s.contains("echo test"));
        assert!(s.contains("if `bash`"));
    }
}
