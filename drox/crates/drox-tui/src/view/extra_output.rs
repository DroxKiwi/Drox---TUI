//! Formatage fil : plan mode, LSP, skills, worktree, task.

use drox_types::ToolUseId;
use serde_json::Value;

use super::lines_viewer::LinesViewerState;
use super::tool_output::{format_error, push_wrapped_body, push_wrapped_body_expandable};

const MAX_LINES: usize = 20;
const MAX_ITEMS: usize = 20;
const LINE_MAX: usize = 120;
/// Lignes max affichées inline pour `.drox/plan.md` (`/plan`).
pub const PLAN_DOCUMENT_MAX_LINES: usize = 48;

pub fn plan_document_expandable(body: &str, truncated: bool) -> bool {
    truncated || body.lines().count() > PLAN_DOCUMENT_MAX_LINES
}

#[must_use]
pub fn plan_document_viewer(
    id: ToolUseId,
    path: &str,
    body: &str,
) -> Option<LinesViewerState> {
    let body = body.trim();
    if body.is_empty() {
        return None;
    }
    let lines: Vec<String> = body.lines().map(str::to_string).collect();
    Some(LinesViewerState::from_plain(
        id,
        format!(" [PLAN] {path} "),
        lines,
    ))
}

pub fn format_exit_plan_mode_start(id: &ToolUseId, args: &Value) -> Vec<String> {
    let plan = args.get("plan").and_then(Value::as_str).unwrap_or("").trim();
    let mut lines = vec![format!("▸ exit_plan_mode ({id}) plan proposé")];
    if plan.is_empty() {
        lines.push("    (plan vide)".to_string());
    } else {
        push_wrapped_body_expandable(&mut lines, plan, MAX_LINES);
    }
    lines
}

#[must_use]
pub fn exit_plan_mode_plan_expandable(plan: &str) -> bool {
    plan.lines().count() > MAX_LINES
}

#[must_use]
pub fn exit_plan_mode_viewer_from_plan(id: ToolUseId, plan: &str) -> Option<LinesViewerState> {
    let plan = plan.trim();
    if plan.is_empty() {
        return None;
    }
    let lines: Vec<String> = plan.lines().map(str::to_string).collect();
    Some(LinesViewerState::from_plain(
        id,
        " exit_plan_mode — plan ",
        lines,
    ))
}

#[must_use]
pub fn skill_list_is_expandable(output: &Value) -> bool {
    output
        .get("skills")
        .and_then(Value::as_array)
        .is_some_and(|skills| skills.len() > MAX_ITEMS)
}

#[must_use]
pub fn skill_list_viewer_from_output(id: ToolUseId, output: &Value) -> Option<LinesViewerState> {
    let skills = output.get("skills").and_then(Value::as_array)?;
    let count = output
        .get("count")
        .and_then(Value::as_u64)
        .unwrap_or(skills.len() as u64);
    let lines: Vec<String> = skills
        .iter()
        .map(|s| {
            let name = s.get("name").and_then(Value::as_str).unwrap_or("?");
            let desc = s
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or("");
            format!("{name} — {}", truncate(desc, LINE_MAX))
        })
        .collect();
    if lines.is_empty() {
        return None;
    }
    Some(LinesViewerState::from_plain(
        id,
        format!(" skill_list — {count} skill(s) "),
        lines,
    ))
}

pub fn format_exit_plan_mode_finish(id: &ToolUseId, output: &Value, is_error: bool) -> Vec<String> {
    if is_error {
        return format_error("exit_plan_mode", id, output);
    }
    let accepted = output.get("accepted").and_then(Value::as_bool).unwrap_or(false);
    let response = output
        .get("user_response")
        .and_then(Value::as_str)
        .unwrap_or("");
    let tag = if accepted { "approuvé" } else { "refusé" };
    vec![
        format!("◂ exit_plan_mode ({id}) [{tag}]"),
        format!("    réponse : {}", truncate(response, LINE_MAX)),
    ]
}

pub fn format_lsp_start(id: &ToolUseId, args: &Value) -> Vec<String> {
    let op = args.get("op").and_then(Value::as_str).unwrap_or("?");
    let path = args
        .get("path")
        .and_then(Value::as_str)
        .map(|p| format!(" {p}"))
        .unwrap_or_default();
    let query = args
        .get("query")
        .or_else(|| args.get("symbol"))
        .and_then(Value::as_str)
        .map(|q| format!(" «{q}»"))
        .unwrap_or_default();
    vec![format!("▸ lsp ({id}) {op}{path}{query}")]
}

pub fn format_lsp_finish(id: &ToolUseId, output: &Value, is_error: bool) -> Vec<String> {
    if is_error {
        return format_error("lsp", id, output);
    }
    let op = output.get("op").and_then(Value::as_str).unwrap_or("?");
    let truncated = output.get("truncated").and_then(Value::as_bool).unwrap_or(false);
    let mut lines = vec![format!(
        "◂ lsp ({id}) {op}{}",
        if truncated { " (tronqué)" } else { "" }
    )];
    if let Some(hint) = output.get("hint").and_then(Value::as_str) {
        lines.push(format!("    · {}", truncate(hint, LINE_MAX)));
    }
    match op {
        "diagnostics" => append_diagnostic_results(&mut lines, output),
        "workspace_symbol" => append_symbol_results(&mut lines, output),
        "definition" | "references" => append_location_results(&mut lines, output),
        "hover" => {
            if let Some(text) = output.get("content").and_then(Value::as_str) {
                push_wrapped_body_expandable(&mut lines, text, MAX_LINES);
            }
        }
        _ => push_json_preview(&mut lines, output),
    }
    lines
}

pub fn format_skill_list_finish(id: &ToolUseId, output: &Value, is_error: bool) -> Vec<String> {
    if is_error {
        return format_error("skill_list", id, output);
    }
    let skills = output
        .get("skills")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let count = output
        .get("count")
        .and_then(Value::as_u64)
        .unwrap_or(skills.len() as u64);
    let mut lines = vec![format!("◂ skill_list ({id}) {count} skill(s)")];
    for s in skills.iter().take(MAX_ITEMS) {
        let name = s.get("name").and_then(Value::as_str).unwrap_or("?");
        let desc = s
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("");
        lines.push(format!(
            "    {name} — {}",
            truncate(desc, LINE_MAX)
        ));
    }
    if skills.len() > MAX_ITEMS {
        lines.push(format!(
            "    … +{} skills (e pour parcourir)",
            skills.len() - MAX_ITEMS
        ));
    }
    lines
}

pub fn format_skill_read_finish(id: &ToolUseId, output: &Value, is_error: bool) -> Vec<String> {
    if is_error {
        return format_error("skill_read", id, output);
    }
    let name = output.get("name").and_then(Value::as_str).unwrap_or("?");
    let path = output.get("path").and_then(Value::as_str).unwrap_or("?");
    let truncated = output.get("truncated").and_then(Value::as_bool).unwrap_or(false);
    let mut lines = vec![format!(
        "◂ skill_read ({id}) {name} ({path}){}",
        if truncated { " [tronqué]" } else { "" }
    )];
    if let Some(content) = output.get("content").and_then(Value::as_str) {
        push_wrapped_body_expandable(&mut lines, content, MAX_LINES);
    }
    lines
}

#[must_use]
pub fn lsp_is_expandable(output: &Value) -> bool {
    if output.get("truncated").and_then(Value::as_bool) == Some(true) {
        return true;
    }
    if let Some(results) = output.get("results").and_then(Value::as_array) {
        if results.len() > MAX_ITEMS {
            return true;
        }
    }
    if let Some(text) = output.get("content").and_then(Value::as_str) {
        if text.lines().count() > MAX_LINES {
            return true;
        }
    }
    collect_lsp_body(output).len() > MAX_LINES
}

#[must_use]
pub fn lsp_viewer_from_output(id: ToolUseId, output: &Value) -> Option<LinesViewerState> {
    let op = output.get("op").and_then(Value::as_str)?;
    let body = collect_lsp_body(output);
    if body.is_empty() {
        return None;
    }
    let truncated = output.get("truncated").and_then(Value::as_bool).unwrap_or(false);
    let suffix = if truncated { " · tronqué" } else { "" };
    Some(LinesViewerState::from_plain(
        id,
        format!(" lsp — {op}{suffix} "),
        body,
    ))
}

#[must_use]
pub fn skill_read_is_expandable(output: &Value) -> bool {
    if output.get("truncated").and_then(Value::as_bool) == Some(true) {
        return true;
    }
    output
        .get("content")
        .and_then(Value::as_str)
        .is_some_and(|c| c.lines().count() > MAX_LINES)
}

#[must_use]
pub fn skill_read_viewer_from_output(id: ToolUseId, output: &Value) -> Option<LinesViewerState> {
    let content = output.get("content").and_then(Value::as_str)?;
    let name = output.get("name").and_then(Value::as_str).unwrap_or("?");
    let lines: Vec<String> = content.lines().map(str::to_string).collect();
    if lines.is_empty() {
        return None;
    }
    Some(LinesViewerState::from_plain(
        id,
        format!(" skill_read — {name} "),
        lines,
    ))
}

#[must_use]
pub fn task_is_expandable(output: &Value) -> bool {
    output
        .get("report")
        .and_then(Value::as_str)
        .is_some_and(|r| r.lines().count() > MAX_LINES)
}

#[must_use]
pub fn task_viewer_from_output(id: ToolUseId, output: &Value) -> Option<LinesViewerState> {
    let report = output.get("report").and_then(Value::as_str)?;
    let kind = output
        .get("subagent_type")
        .and_then(Value::as_str)
        .unwrap_or("explore");
    let lines: Vec<String> = report.lines().map(str::to_string).collect();
    if lines.is_empty() {
        return None;
    }
    Some(LinesViewerState::from_plain(
        id,
        format!(" task — {kind} "),
        lines,
    ))
}

pub fn format_git_worktree_enter_start(id: &ToolUseId, args: &Value) -> Vec<String> {
    let name = args
        .get("name")
        .and_then(Value::as_str)
        .map(|n| format!(" {n}"))
        .unwrap_or_else(|| " (auto)".to_string());
    vec![format!("▸ git_worktree_enter ({id}){name}")]
}

pub fn format_git_worktree_enter_finish(id: &ToolUseId, output: &Value, is_error: bool) -> Vec<String> {
    if is_error {
        return format_error("git_worktree_enter", id, output);
    }
    format_worktree_message("git_worktree_enter", id, output)
}

pub fn format_git_worktree_exit_start(id: &ToolUseId, args: &Value) -> Vec<String> {
    let action = args
        .get("action")
        .and_then(Value::as_str)
        .unwrap_or("?");
    vec![format!("▸ git_worktree_exit ({id}) action={action}")]
}

pub fn format_git_worktree_exit_finish(id: &ToolUseId, output: &Value, is_error: bool) -> Vec<String> {
    if is_error {
        return format_error("git_worktree_exit", id, output);
    }
    format_worktree_message("git_worktree_exit", id, output)
}

pub fn format_task_start(id: &ToolUseId, args: &Value) -> Vec<String> {
    let desc = args
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or("?");
    let kind = args
        .get("subagent_type")
        .and_then(Value::as_str)
        .unwrap_or("explore");
    let thorough = args
        .get("thoroughness")
        .and_then(Value::as_str)
        .map(|t| format!(" [{t}]"))
        .unwrap_or_default();
    vec![format!(
        "▸ task ({id}) {kind}{thorough} {}",
        truncate(desc, LINE_MAX)
    )]
}

pub fn format_task_finish(id: &ToolUseId, output: &Value, is_error: bool) -> Vec<String> {
    if is_error {
        return format_error("task", id, output);
    }
    let kind = output
        .get("subagent_type")
        .and_then(Value::as_str)
        .unwrap_or("explore");
    let mut lines = vec![format!("◂ task ({id}) {kind} terminé")];
    if let Some(report) = output.get("report").and_then(Value::as_str) {
        push_wrapped_body_expandable(&mut lines, report, MAX_LINES);
    }
    lines
}

fn format_worktree_message(name: &str, id: &ToolUseId, output: &Value) -> Vec<String> {
    let branch = output
        .get("worktree_branch")
        .and_then(Value::as_str)
        .unwrap_or("?");
    let path = output
        .get("worktree_path")
        .and_then(Value::as_str)
        .unwrap_or("?");
    let mut lines = vec![format!("◂ {name} ({id}) {branch} @ {path}")];
    if let Some(msg) = output.get("message").and_then(Value::as_str) {
        push_wrapped_body_expandable(&mut lines, msg, 4);
    }
    lines
}

#[must_use]
pub fn worktree_is_expandable(output: &Value) -> bool {
    output
        .get("message")
        .and_then(Value::as_str)
        .is_some_and(|m| m.lines().count() > 4)
}

#[must_use]
pub fn worktree_viewer_from_output(
    id: ToolUseId,
    tool: &str,
    output: &Value,
) -> Option<LinesViewerState> {
    let branch = output
        .get("worktree_branch")
        .and_then(Value::as_str)
        .unwrap_or("?");
    let path = output
        .get("worktree_path")
        .and_then(Value::as_str)
        .unwrap_or("?");
    let message = output.get("message").and_then(Value::as_str)?;
    let mut lines: Vec<String> = vec![
        format!("{branch} @ {path}"),
        String::new(),
    ];
    lines.extend(message.lines().map(str::to_string));
    Some(LinesViewerState::from_plain(
        id,
        format!(" {tool} "),
        lines,
    ))
}

fn append_diagnostic_results(lines: &mut Vec<String>, output: &Value) {
    let results = output
        .get("results")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let scope = output
        .get("scope")
        .and_then(Value::as_str)
        .unwrap_or("?");
    lines.push(format!(
        "    scope: {scope} ({} diag)",
        output
            .get("total")
            .and_then(Value::as_u64)
            .unwrap_or(results.len() as u64)
    ));
    for r in results.iter().take(MAX_ITEMS) {
        let path = r.get("path").and_then(Value::as_str).unwrap_or("?");
        let sev = r.get("severity").and_then(Value::as_str).unwrap_or("?");
        let msg = r.get("message").and_then(Value::as_str).unwrap_or("");
        let line = r
            .get("range")
            .and_then(|rng| rng.get("start"))
            .and_then(|s| s.get("line"))
            .and_then(Value::as_u64)
            .map(|n| n + 1)
            .unwrap_or(0);
        lines.push(format!(
            "    {sev} {path}:{line}: {}",
            truncate(msg, LINE_MAX)
        ));
    }
    if results.len() > MAX_ITEMS {
        lines.push(format!(
            "    … +{} diagnostics (e pour parcourir)",
            results.len() - MAX_ITEMS
        ));
    }
}

fn collect_lsp_body(output: &Value) -> Vec<String> {
    let op = output.get("op").and_then(Value::as_str).unwrap_or("?");
    let mut lines = Vec::new();
    if let Some(hint) = output.get("hint").and_then(Value::as_str) {
        lines.push(format!("· {}", truncate(hint, LINE_MAX)));
    }
    match op {
        "diagnostics" => collect_diagnostic_results(&mut lines, output),
        "workspace_symbol" => collect_symbol_results(&mut lines, output),
        "definition" | "references" => collect_location_results(&mut lines, output),
        "hover" => {
            if let Some(text) = output.get("content").and_then(Value::as_str) {
                lines.extend(text.lines().map(str::to_string));
            }
        }
        _ => {
            let body = serde_json::to_string_pretty(output).unwrap_or_else(|_| "{}".into());
            lines.extend(body.lines().map(str::to_string));
        }
    }
    lines
}

fn collect_diagnostic_results(lines: &mut Vec<String>, output: &Value) {
    let results = output
        .get("results")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let scope = output
        .get("scope")
        .and_then(Value::as_str)
        .unwrap_or("?");
    lines.push(format!(
        "scope: {scope} ({} diag)",
        output
            .get("total")
            .and_then(Value::as_u64)
            .unwrap_or(results.len() as u64)
    ));
    for r in results {
        let path = r.get("path").and_then(Value::as_str).unwrap_or("?");
        let sev = r.get("severity").and_then(Value::as_str).unwrap_or("?");
        let msg = r.get("message").and_then(Value::as_str).unwrap_or("");
        let line = r
            .get("range")
            .and_then(|rng| rng.get("start"))
            .and_then(|s| s.get("line"))
            .and_then(Value::as_u64)
            .map(|n| n + 1)
            .unwrap_or(0);
        lines.push(format!("{sev} {path}:{line}: {}", truncate(msg, LINE_MAX)));
    }
}

fn append_symbol_results(lines: &mut Vec<String>, output: &Value) {
    let query = output.get("query").and_then(Value::as_str).unwrap_or("?");
    let results = output
        .get("results")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    lines.push(format!("    query: {query} ({} symbole(s))", results.len()));
    for r in results.iter().take(MAX_ITEMS) {
        let name = r.get("name").and_then(Value::as_str).unwrap_or("?");
        let kind = r.get("kind").and_then(Value::as_str).unwrap_or("?");
        let path = r
            .get("location")
            .and_then(|l| l.get("path"))
            .and_then(Value::as_str)
            .unwrap_or("?");
        lines.push(format!("    {kind} {name} @ {path}"));
    }
    if results.len() > MAX_ITEMS {
        lines.push(format!(
            "    … +{} symboles (e pour parcourir)",
            results.len() - MAX_ITEMS
        ));
    }
}

fn collect_symbol_results(lines: &mut Vec<String>, output: &Value) {
    let query = output.get("query").and_then(Value::as_str).unwrap_or("?");
    let results = output
        .get("results")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    lines.push(format!("query: {query} ({} symbole(s))", results.len()));
    for r in results {
        let name = r.get("name").and_then(Value::as_str).unwrap_or("?");
        let kind = r.get("kind").and_then(Value::as_str).unwrap_or("?");
        let path = r
            .get("location")
            .and_then(|l| l.get("path"))
            .and_then(Value::as_str)
            .unwrap_or("?");
        lines.push(format!("{kind} {name} @ {path}"));
    }
}

fn append_location_results(lines: &mut Vec<String>, output: &Value) {
    if let Some(origin) = output.get("origin") {
        let path = origin.get("path").and_then(Value::as_str).unwrap_or("?");
        let line = origin
            .get("position")
            .and_then(|p| p.get("line"))
            .and_then(Value::as_u64)
            .map(|n| n + 1)
            .unwrap_or(0);
        lines.push(format!("    origine: {path}:{line}"));
    }
    let results = output
        .get("results")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    for r in results.iter().take(MAX_ITEMS) {
        let path = r.get("path").and_then(Value::as_str).unwrap_or("?");
        let line = r
            .get("range")
            .and_then(|rng| rng.get("start"))
            .and_then(|s| s.get("line"))
            .and_then(Value::as_u64)
            .map(|n| n + 1)
            .unwrap_or(0);
        lines.push(format!("    → {path}:{line}"));
    }
    if results.len() > MAX_ITEMS {
        lines.push(format!(
            "    … +{} emplacements (e pour parcourir)",
            results.len() - MAX_ITEMS
        ));
    }
}

fn collect_location_results(lines: &mut Vec<String>, output: &Value) {
    if let Some(origin) = output.get("origin") {
        let path = origin.get("path").and_then(Value::as_str).unwrap_or("?");
        let line = origin
            .get("position")
            .and_then(|p| p.get("line"))
            .and_then(Value::as_u64)
            .map(|n| n + 1)
            .unwrap_or(0);
        lines.push(format!("origine: {path}:{line}"));
    }
    let results = output
        .get("results")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    for r in results {
        let path = r.get("path").and_then(Value::as_str).unwrap_or("?");
        let line = r
            .get("range")
            .and_then(|rng| rng.get("start"))
            .and_then(|s| s.get("line"))
            .and_then(Value::as_u64)
            .map(|n| n + 1)
            .unwrap_or(0);
        lines.push(format!("→ {path}:{line}"));
    }
}

fn push_json_preview(lines: &mut Vec<String>, output: &Value) {
    let body = serde_json::to_string_pretty(output).unwrap_or_else(|_| "{}".into());
    push_wrapped_body(lines, &body, MAX_LINES);
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use drox_types::ToolUseId;

    #[test]
    fn exit_plan_mode_shows_decision() {
        let id = ToolUseId::new();
        let out = serde_json::json!({ "accepted": true, "user_response": "Oui" });
        let lines = format_exit_plan_mode_finish(&id, &out, false);
        assert!(lines.iter().any(|l| l.contains("approuvé")));
    }

    #[test]
    fn lsp_diagnostics_line() {
        let id = ToolUseId::new();
        let out = serde_json::json!({
            "op": "diagnostics",
            "scope": "src/main.rs",
            "total": 1,
            "results": [{
                "path": "src/main.rs",
                "severity": "error",
                "message": "expected `;`",
                "range": { "start": { "line": 4, "character": 0 } }
            }]
        });
        let lines = format_lsp_finish(&id, &out, false);
        assert!(lines.iter().any(|l| l.contains("error") && l.contains("main.rs:5")));
    }

    #[test]
    fn skill_list_names() {
        let id = ToolUseId::new();
        let out = serde_json::json!({
            "count": 1,
            "skills": [{ "name": "commit", "description": "help" }]
        });
        let lines = format_skill_list_finish(&id, &out, false);
        assert!(lines.iter().any(|l| l.contains("commit")));
    }

    #[test]
    fn task_report_body() {
        let id = ToolUseId::new();
        let out = serde_json::json!({
            "subagent_type": "explore",
            "report": "Found module auth"
        });
        let lines = format_task_finish(&id, &out, false);
        assert!(lines.iter().any(|l| l.contains("Found module auth")));
    }

    #[test]
    fn plan_document_expandable_when_truncated() {
        let short = "line\n".repeat(10);
        assert!(!plan_document_expandable(&short, false));
        assert!(plan_document_expandable(&short, true));
    }

    #[test]
    fn plan_document_viewer_has_lines() {
        let id = ToolUseId::new();
        let body = (0..60).map(|i| format!("step {i}")).collect::<Vec<_>>().join("\n");
        let viewer = plan_document_viewer(id, ".drox/plan.md", &body).expect("viewer");
        assert_eq!(viewer.lines.len(), 60);
    }
}
