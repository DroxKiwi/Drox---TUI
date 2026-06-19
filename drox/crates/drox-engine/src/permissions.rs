//! Branchement de `drox-permissions` sur la boucle agent.
//!
//! On encapsule un `PermissionEngine` + un `PermissionMode` dans une
//! `PermissionPolicy`, et on extrait dynamiquement la "cible" (`content`,
//! `is_write`, `is_read_only`) à partir du nom du tool et de ses arguments
//! JSON. Cette heuristique simple est suffisante pour les tools du sprint
//! 1.5 ; quand le registre deviendra dynamique (MCP, plugins), ce module
//! évoluera vers un trait `ToolPermissionTarget`.

use std::sync::Arc;

use drox_bash::{
    BashError, auto_deny_message, kind_of_segment, permission_flags, split_command_segments,
};
use drox_permissions::{
    DecisionReason, PermissionDecision, PermissionEngine, PermissionMode, PermissionTarget,
};
use serde_json::Value;

/// Politique de permissions appliquée par l'agent.
#[derive(Clone)]
pub struct PermissionPolicy {
    /// Moteur de règles partageable.
    pub engine: Arc<PermissionEngine>,
    /// Mode courant.
    pub mode: PermissionMode,
}

impl PermissionPolicy {
    /// Construit une politique avec moteur + mode.
    #[must_use]
    pub const fn new(engine: Arc<PermissionEngine>, mode: PermissionMode) -> Self {
        Self { engine, mode }
    }

    /// Politique permissive : aucun jugement, tout passe.
    #[must_use]
    pub fn permissive() -> Self {
        Self {
            engine: Arc::new(PermissionEngine::new()),
            mode: PermissionMode::BypassPermissions,
        }
    }

    /// Évalue la décision pour un appel `tool_name(args)`.
    #[must_use]
    pub fn evaluate(&self, tool_name: &str, args: &Value) -> PermissionDecision {
        self.evaluate_with_read_only_hint(tool_name, args, None)
    }

    /// Comme [`Self::evaluate`] avec hint lecture seule issu du registre tool.
    #[must_use]
    pub fn evaluate_with_read_only_hint(
        &self,
        tool_name: &str,
        args: &Value,
        read_only_hint: Option<bool>,
    ) -> PermissionDecision {
        if tool_name == "bash" {
            if let Some(cmd) = extract_content(tool_name, args) {
                return self.evaluate_bash_compound(cmd);
            }
        }
        if tool_name == "copy_path" {
            return self.evaluate_copy_path(args, read_only_hint);
        }
        let effective =
            mcp_qualified_tool_name(tool_name, args).unwrap_or_else(|| tool_name.to_string());
        self.evaluate_single(
            &effective,
            extract_content(tool_name, args),
            read_only_hint,
        )
    }

    /// `copy_path` : évalue source **et** destination (règles fichier §2.30).
    #[must_use]
    fn evaluate_copy_path(&self, args: &Value, read_only_hint: Option<bool>) -> PermissionDecision {
        let src = args.get("source").and_then(Value::as_str);
        let dest = args.get("destination").and_then(Value::as_str);
        match (src, dest) {
            (None, None) => self.evaluate_single("copy_path", None, read_only_hint),
            (Some(s), None) => self.evaluate_single("copy_path", Some(s), read_only_hint),
            (None, Some(d)) => self.evaluate_single("copy_path", Some(d), read_only_hint),
            (Some(s), Some(d)) => self
                .evaluate_single("copy_path", Some(s), read_only_hint)
                .merge_compound(self.evaluate_single("copy_path", Some(d), read_only_hint)),
        }
    }

    /// Une seule cible (non composée).
    #[must_use]
    fn evaluate_single(
        &self,
        tool_name: &str,
        content: Option<&str>,
        read_only_hint: Option<bool>,
    ) -> PermissionDecision {
        let is_read_only =
            read_only_hint.unwrap_or_else(|| is_read_only_tool(tool_name));
        let target = PermissionTarget {
            tool_name,
            content,
            is_write: is_write_tool(tool_name),
            is_read_only,
        };
        self.engine.evaluate(&target, self.mode)
    }

    /// Découpe une ligne Bash (`&&`, pipelines, substitutions) et agrège les décisions.
    #[must_use]
    fn evaluate_bash_compound(&self, full_command: &str) -> PermissionDecision {
        match split_command_segments(full_command) {
            Ok(segments) => {
                if segments.is_empty() {
                    return self.evaluate_bash_segment("");
                }
                let mut iter = segments.iter();
                let mut acc =
                    self.evaluate_bash_segment(iter.next().expect("non-empty").as_str());
                for seg in iter {
                    acc = acc.merge_compound(self.evaluate_bash_segment(seg.as_str()));
                }
                acc
            }
            Err(BashError::TooManySubcommands(n)) => PermissionDecision::Ask {
                reason: DecisionReason::Default,
                message: format!(
                    "La commande Bash est trop fragmentée ({n} segments) ; validation humaine requise."
                ),
            },
            Err(_) => PermissionDecision::Ask {
                reason: DecisionReason::Default,
                message: "Analyse Bash indisponible ; validation humaine requise.".to_string(),
            },
        }
    }

    /// Évalue une sous-commande Bash avec le classifieur `drox-bash` (§2.27).
    #[must_use]
    fn evaluate_bash_segment(&self, segment: &str) -> PermissionDecision {
        let trimmed = segment.trim();
        if trimmed.is_empty() {
            return self.evaluate_single("bash", None, None);
        }

        let kind = kind_of_segment(trimmed);
        let (is_write, is_read_only) = permission_flags(kind);
        let target = PermissionTarget {
            tool_name: "bash",
            content: Some(trimmed),
            is_write,
            is_read_only,
        };
        let mut decision = self.engine.evaluate(&target, self.mode);

        if let Some(hint) = auto_deny_message(trimmed) {
            let allowed_by_rule = matches!(
                &decision,
                PermissionDecision::Allow {
                    reason: DecisionReason::Rule { .. },
                    ..
                }
            );
            if !allowed_by_rule && !matches!(self.mode, PermissionMode::BypassPermissions) {
                decision = PermissionDecision::Deny {
                    reason: DecisionReason::Default,
                    message: format!(
                        "Commande Bash refusée ({hint}). Pour autoriser explicitement, ajoutez une règle `Allow` pour `bash({trimmed})`."
                    ),
                };
            }
        }

        decision
    }
}

impl std::fmt::Debug for PermissionPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PermissionPolicy")
            .field("mode", &self.mode)
            .field("rules_count", &self.engine.rules().len())
            .finish()
    }
}

/// Heuristique de "valeur cible" par tool. Renvoie un `Option<&str>` qui
/// vit aussi longtemps que `args`.
fn extract_content<'a>(tool_name: &str, args: &'a Value) -> Option<&'a str> {
    match tool_name {
        "file_read" | "file_write" | "file_edit" | "notebook_edit" | "lsp" | "delete_path" => args
            .get("path")
            .or_else(|| args.get("file_path"))
            .and_then(Value::as_str),
        "bash" => args.get("command").and_then(Value::as_str),
        "web_fetch" => args.get("url").and_then(Value::as_str),
        "web_search" => args.get("query").and_then(Value::as_str),
        "session_compact" => args.get("reason").and_then(Value::as_str),
        "mcp_call" => args.get("server").and_then(Value::as_str),
        "read_mcp_resource" => args.get("uri").and_then(Value::as_str),
        _ => None,
    }
}

/// Nom qualifié style leak (`mcp__server__tool`) pour les règles permissions.
fn mcp_qualified_tool_name(tool_name: &str, args: &Value) -> Option<String> {
    if tool_name != "mcp_call" {
        return None;
    }
    let server = args.get("server")?.as_str()?;
    let tool = args.get("tool")?.as_str()?;
    Some(format!(
        "mcp__{}__{}",
        normalize_mcp_segment(server),
        normalize_mcp_segment(tool)
    ))
}

fn normalize_mcp_segment(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// `true` si le tool modifie le système (filesystem ou environnement).
const fn is_write_tool(tool_name: &str) -> bool {
    matches!(
        tool_name.as_bytes(),
        b"file_write"
            | b"file_edit"
            | b"notebook_edit"
            | b"delete_path"
            | b"copy_path"
            | b"bash"
            | b"git_worktree_enter"
            | b"git_worktree_exit"
            | b"session_compact"
    )
}

/// `true` si le tool ne fait que lire / observer.
///
/// Inclut (sprint M1) les tools mémoire de session : `session_note` écrit
/// dans un stock RAM partagé (jamais sur disque côté tool), `memory_read` /
/// `memory_list` lisent `.drox/memory/sessions/`. Aucun effet de bord sur
/// le workspace utilisateur → auto-allow par défaut.
/// Heuristique lecture seule par nom (tools statiques du registre par défaut).
#[must_use]
pub fn is_read_only_tool(tool_name: &str) -> bool {
    matches!(
        tool_name.as_bytes(),
        b"file_read"
            | b"grep"
            | b"glob"
            | b"web_fetch"
            | b"web_search"
            | b"lsp"
            | b"ask_user_question"
            | b"exit_plan_mode"
            | b"todo_write"
            | b"course_plan_write"
            | b"session_note"
            | b"memory_read"
            | b"memory_list"
            | b"skill_read"
            | b"skill_list"
            | b"session_end"
            | b"session_search"
            | b"list_mcp_resources"
            | b"read_mcp_resource"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use drox_permissions::{
        PathMatchContext, PermissionBehavior, Rule, RuleSet, RuleSource, RuleValue, parse_rule,
    };
    use serde_json::json;

    fn engine_with(rules: Vec<Rule>) -> Arc<PermissionEngine> {
        let mut set = RuleSet::new();
        for r in rules {
            set.push(r);
        }
        Arc::new(PermissionEngine::with_rules(set))
    }

    #[test]
    fn extracts_path_for_file_tools() {
        let args = json!({ "path": "/tmp/foo.txt", "content": "hi" });
        assert_eq!(extract_content("file_write", &args), Some("/tmp/foo.txt"));
        assert_eq!(extract_content("file_edit", &args), Some("/tmp/foo.txt"));
        assert_eq!(extract_content("file_read", &args), Some("/tmp/foo.txt"));
    }

    #[test]
    fn extracts_command_for_bash() {
        let args = json!({ "command": "echo hi" });
        assert_eq!(extract_content("bash", &args), Some("echo hi"));
    }

    #[test]
    fn extracts_path_for_delete_path() {
        let args = json!({ "path": "src/old.txt" });
        assert_eq!(extract_content("delete_path", &args), Some("src/old.txt"));
    }

    #[test]
    fn extracts_path_for_notebook_edit_file_path_alias() {
        let args = json!({ "file_path": "nb.ipynb", "edits": [] });
        assert_eq!(
            extract_content("notebook_edit", &args),
            Some("nb.ipynb")
        );
    }

    #[test]
    fn unknown_tool_has_no_content() {
        let args = json!({ "foo": "bar" });
        assert_eq!(extract_content("unknown", &args), None);
    }

    #[test]
    fn read_only_tools_auto_allowed_in_default_mode() {
        let policy = PermissionPolicy::new(engine_with(vec![]), PermissionMode::Default);
        let decision = policy.evaluate("file_read", &json!({ "path": "README.md" }));
        assert!(decision.is_allow(), "got {decision:?}");
    }

    #[test]
    fn write_tools_ask_in_default_mode() {
        let policy = PermissionPolicy::new(engine_with(vec![]), PermissionMode::Default);
        let decision = policy.evaluate("file_write", &json!({ "path": "/tmp/x", "content": "" }));
        assert!(decision.is_ask(), "got {decision:?}");
    }

    #[test]
    fn write_tools_allowed_in_accept_edits() {
        let policy = PermissionPolicy::new(engine_with(vec![]), PermissionMode::AcceptEdits);
        let decision = policy.evaluate("file_write", &json!({ "path": "/tmp/x", "content": "" }));
        assert!(decision.is_allow(), "got {decision:?}");
    }

    #[test]
    fn write_tools_blocked_in_plan_mode() {
        let policy = PermissionPolicy::new(engine_with(vec![]), PermissionMode::Plan);
        let decision = policy.evaluate("file_write", &json!({ "path": "/tmp/x", "content": "" }));
        assert!(decision.is_deny(), "got {decision:?}");
    }

    #[test]
    fn write_tools_allowed_in_professor_mode() {
        let policy = PermissionPolicy::new(engine_with(vec![]), PermissionMode::Professor);
        let decision = policy.evaluate("file_write", &json!({ "path": "/tmp/x", "content": "" }));
        assert!(decision.is_allow(), "got {decision:?}");
    }

    #[test]
    fn delete_path_blocked_in_plan_mode() {
        let policy = PermissionPolicy::new(engine_with(vec![]), PermissionMode::Plan);
        let decision = policy.evaluate("delete_path", &json!({ "path": "old.txt" }));
        assert!(decision.is_deny(), "got {decision:?}");
    }

    #[test]
    fn rule_with_command_pattern_allows_bash() {
        let engine = engine_with(vec![Rule {
            value: RuleValue::with_content("bash", "echo *"),
            behavior: PermissionBehavior::Allow,
            source: RuleSource::UserSettings,
        }]);
        let policy = PermissionPolicy::new(engine, PermissionMode::Default);
        let decision = policy.evaluate("bash", &json!({ "command": "echo hello" }));
        assert!(decision.is_allow(), "got {decision:?}");
    }

    #[test]
    fn deny_rule_beats_bypass_mode() {
        let engine = engine_with(vec![Rule {
            value: RuleValue::with_content("bash", "rm -rf *"),
            behavior: PermissionBehavior::Deny,
            source: RuleSource::UserSettings,
        }]);
        let policy = PermissionPolicy::new(engine, PermissionMode::BypassPermissions);
        let decision = policy.evaluate("bash", &json!({ "command": "rm -rf /tmp/x" }));
        assert!(decision.is_deny(), "got {decision:?}");
    }

    #[test]
    fn bash_compound_denies_if_any_segment_denied() {
        let engine = engine_with(vec![Rule {
            value: RuleValue::with_content("bash", "rm -rf *"),
            behavior: PermissionBehavior::Deny,
            source: RuleSource::UserSettings,
        }]);
        let policy = PermissionPolicy::new(engine, PermissionMode::Default);
        let decision = policy.evaluate("bash", &json!({ "command": "echo ok && rm -rf /tmp/z" }));
        assert!(decision.is_deny(), "got {decision:?}");
    }

    #[test]
    fn bash_ls_auto_allowed_in_default_mode() {
        let policy = PermissionPolicy::new(engine_with(vec![]), PermissionMode::Default);
        let decision = policy.evaluate("bash", &json!({ "command": "ls -la" }));
        assert!(decision.is_allow(), "got {decision:?}");
    }

    #[test]
    fn bash_rm_rf_auto_denied_without_rule() {
        let policy = PermissionPolicy::new(engine_with(vec![]), PermissionMode::Default);
        let decision = policy.evaluate("bash", &json!({ "command": "rm -rf /tmp/z" }));
        assert!(decision.is_deny(), "got {decision:?}");
        if let PermissionDecision::Deny { message, .. } = decision {
            assert!(message.contains("refusée") || message.contains("refused"));
        }
    }

    #[test]
    fn bash_curl_asks_in_default_mode() {
        let policy = PermissionPolicy::new(engine_with(vec![]), PermissionMode::Default);
        let decision = policy.evaluate("bash", &json!({ "command": "curl https://example.com" }));
        assert!(decision.is_ask(), "got {decision:?}");
    }

    #[test]
    fn bash_compound_read_then_mutate() {
        let policy = PermissionPolicy::new(engine_with(vec![]), PermissionMode::Default);
        let decision = policy.evaluate("bash", &json!({ "command": "git status && npm install" }));
        assert!(decision.is_ask(), "got {decision:?}");
    }

    #[test]
    fn copy_path_denies_when_destination_matches_edit_deny_glob() {
        let root = std::env::temp_dir().join("drox_policy_copy_env");
        let _ = std::fs::create_dir_all(root.join("out"));
        let src = root.join("README.md");
        let dest = root.join("out/.env");
        let _ = std::fs::write(&src, "ok");
        let _ = std::fs::write(&dest, "old");

        let mut set = RuleSet::new();
        set.push(Rule {
            value: parse_rule("Edit(**/*.env)"),
            behavior: PermissionBehavior::Deny,
            source: RuleSource::ProjectSettings,
        });
        let engine = PermissionEngine::with_rules(set)
            .with_path_context(PathMatchContext::new(&root, root.join("home")));
        let policy = PermissionPolicy::new(Arc::new(engine), PermissionMode::AcceptEdits);

        let decision = policy.evaluate(
            "copy_path",
            &json!({
                "source": src.to_str().unwrap(),
                "destination": dest.to_str().unwrap()
            }),
        );
        assert!(decision.is_deny(), "got {decision:?}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn file_edit_deny_glob_via_leak_edit_alias() {
        let root = std::env::temp_dir().join("drox_policy_perm_env");
        let _ = std::fs::create_dir_all(root.join("lib"));
        let env_path = root.join("lib/.env");
        let _ = std::fs::write(&env_path, "X=1");

        let mut set = RuleSet::new();
        set.push(Rule {
            value: parse_rule("Edit(**/*.env)"),
            behavior: PermissionBehavior::Deny,
            source: RuleSource::ProjectSettings,
        });
        let engine = PermissionEngine::with_rules(set)
            .with_path_context(PathMatchContext::new(&root, root.join("home")));
        let policy = PermissionPolicy::new(Arc::new(engine), PermissionMode::AcceptEdits);

        let decision = policy.evaluate(
            "file_edit",
            &json!({ "path": env_path.to_str().unwrap() }),
        );
        assert!(decision.is_deny(), "got {decision:?}");
        let _ = std::fs::remove_dir_all(&root);
    }
}
