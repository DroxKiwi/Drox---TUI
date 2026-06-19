//! Détection de règles `Allow` inatteignables (masquées par ask/deny tool-wide).
//!
//! Aligné sur `src/utils/permissions/shadowedRuleDetection.ts`.

use crate::rule::{PermissionBehavior, Rule, RuleSet, RuleSource, format_rule};

/// Type d'ombre portée (ask = toujours prompt ; deny = bloqué).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadowType {
    Ask,
    Deny,
}

/// Règle allow spécifique rendue inutile par une règle tool-wide.
#[derive(Debug, Clone)]
pub struct UnreachableRule {
    pub rule: Rule,
    pub reason: String,
    pub shadowed_by: Rule,
    pub shadow_type: ShadowType,
    pub fix: String,
}

/// Options (sandbox Bash — réservé V2 ; conservé pour compat API leak).
#[derive(Debug, Clone, Default)]
pub struct DetectUnreachableOptions {
    pub sandbox_auto_allow_enabled: bool,
}

/// Détecte les règles `Allow` avec contenu masquées par ask/deny tool-wide.
#[must_use]
pub fn detect_unreachable_rules(
    rules: &RuleSet,
    options: &DetectUnreachableOptions,
) -> Vec<UnreachableRule> {
    let allow_rules: Vec<Rule> = rules
        .iter_behavior(PermissionBehavior::Allow)
        .filter(|r| r.value.rule_content.is_some())
        .cloned()
        .collect();
    let ask_rules: Vec<Rule> = rules
        .iter_behavior(PermissionBehavior::Ask)
        .cloned()
        .collect();
    let deny_rules: Vec<Rule> = rules
        .iter_behavior(PermissionBehavior::Deny)
        .cloned()
        .collect();

    let mut out = Vec::new();
    for allow in allow_rules {
        if let Some(shadowed) = shadowed_by_deny(&allow, &deny_rules) {
            out.push(shadowed);
            continue;
        }
        if let Some(shadowed) = shadowed_by_ask(&allow, &ask_rules, options) {
            out.push(shadowed);
        }
    }
    out
}

fn shadowed_by_deny(allow: &Rule, deny_rules: &[Rule]) -> Option<UnreachableRule> {
    let shadow = deny_rules.iter().find(|d| {
        d.value.tool_name == allow.value.tool_name && d.value.rule_content.is_none()
    })?;
    Some(build_unreachable(allow, shadow, ShadowType::Deny))
}

fn shadowed_by_ask(
    allow: &Rule,
    ask_rules: &[Rule],
    options: &DetectUnreachableOptions,
) -> Option<UnreachableRule> {
    let shadow = ask_rules.iter().find(|a| {
        a.value.tool_name == allow.value.tool_name && a.value.rule_content.is_none()
    })?;

    if allow.value.tool_name == "bash" && options.sandbox_auto_allow_enabled {
        if !is_shared_source(shadow.source) {
            return None;
        }
    }

    Some(build_unreachable(allow, shadow, ShadowType::Ask))
}

fn is_shared_source(source: RuleSource) -> bool {
    matches!(source, RuleSource::ProjectSettings)
}

fn build_unreachable(allow: &Rule, shadow: &Rule, shadow_type: ShadowType) -> UnreachableRule {
    let tool = shadow.value.tool_name.clone();
    let shadow_src = shadow.source.display_name();
    let allow_src = allow.source.display_name();
    let reason = match shadow_type {
        ShadowType::Deny => format!(
            "Bloquée par la règle deny tool-wide `{tool}` (source : {shadow_src})"
        ),
        ShadowType::Ask => format!(
            "Masquée par la règle ask tool-wide `{tool}` (source : {shadow_src})"
        ),
    };
    let fix = match shadow_type {
        ShadowType::Deny => format!(
            "Retirez la règle deny `{tool}` dans {shadow_src}, ou la règle allow `{}` dans {allow_src}",
            format_rule(&allow.value)
        ),
        ShadowType::Ask => format!(
            "Retirez la règle ask `{tool}` dans {shadow_src}, ou la règle allow `{}` dans {allow_src}",
            format_rule(&allow.value)
        ),
    };
    UnreachableRule {
        rule: allow.clone(),
        reason,
        shadowed_by: shadow.clone(),
        shadow_type,
        fix,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rule::{RuleValue, parse_rule};

    fn allow_rule(tool: &str, content: &str) -> Rule {
        Rule {
            value: RuleValue::with_content(tool, content),
            behavior: PermissionBehavior::Allow,
            source: RuleSource::UserSettings,
        }
    }

    fn tool_wide(behavior: PermissionBehavior, tool: &str, source: RuleSource) -> Rule {
        Rule {
            value: parse_rule(tool),
            behavior,
            source,
        }
    }

    #[test]
    fn detects_ask_shadowing_bash_allow() {
        let mut set = RuleSet::new();
        set.push(tool_wide(PermissionBehavior::Ask, "bash", RuleSource::ProjectSettings));
        set.push(allow_rule("bash", "ls:*"));
        let unreachable = detect_unreachable_rules(&set, &DetectUnreachableOptions::default());
        assert_eq!(unreachable.len(), 1);
        assert_eq!(unreachable[0].shadow_type, ShadowType::Ask);
    }

    #[test]
    fn detects_deny_shadowing() {
        let mut set = RuleSet::new();
        set.push(tool_wide(PermissionBehavior::Deny, "bash", RuleSource::UserSettings));
        set.push(allow_rule("bash", "echo *"));
        let unreachable = detect_unreachable_rules(&set, &DetectUnreachableOptions::default());
        assert_eq!(unreachable.len(), 1);
        assert_eq!(unreachable[0].shadow_type, ShadowType::Deny);
    }
}
