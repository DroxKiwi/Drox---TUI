//! Règles de permission : parsing, sérialisation, agrégation.
//!
//! Une règle a la forme textuelle :
//!
//! ```text
//! ToolName             // matche le tool entier (toute invocation)
//! ToolName(content)    // matche un contenu spécifique (cf. matcher)
//! ```
//!
//! Les parenthèses dans le `content` doivent être échappées en `\(` / `\)`,
//! et les backslashes en `\\` (compatible avec le format TS d'origine).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::tool_names::normalize_rule_tool_name;

/// Comportement associé à une règle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PermissionBehavior {
    /// La règle autorise l'usage du tool sans intervention humaine.
    Allow,
    /// La règle force une demande de confirmation à l'humain.
    Ask,
    /// La règle bloque l'usage du tool.
    Deny,
}

/// Provenance d'une règle. Utilisée pour ordonner / afficher / supprimer
/// (les sources read-only ne peuvent pas être modifiées dynamiquement).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RuleSource {
    /// Réglages utilisateur globaux (`~/.drox/settings.json`).
    UserSettings,
    /// Réglages projet (`<workspace>/.drox/settings.json`).
    ProjectSettings,
    /// Réglages local non versionnés (`<workspace>/.drox/settings.local.json`).
    LocalSettings,
    /// Argument CLI (`--allow Bash(npm:*)`).
    CliArg,
    /// Décision en mémoire pour la session courante (réponse "always" à un ask).
    Session,
}

impl RuleSource {
    /// Nom court pour affichage humain.
    #[must_use]
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::UserSettings => "user settings",
            Self::ProjectSettings => "project settings",
            Self::LocalSettings => "local settings",
            Self::CliArg => "CLI argument",
            Self::Session => "session",
        }
    }
}

/// Valeur d'une règle : nom du tool + contenu optionnel.
///
/// `tool_name` peut être un nom canonique (`Bash`, `FileEdit`, …) ou un nom
/// MCP qualifié (`mcp__server__tool`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RuleValue {
    /// Nom du tool ciblé.
    pub tool_name: String,
    /// Contenu de la règle (interprété selon le tool ; voir `matcher`).
    pub rule_content: Option<String>,
}

impl RuleValue {
    /// Construit une règle qui matche **tout** appel d'un tool.
    #[must_use]
    pub fn tool_wide(tool_name: impl Into<String>) -> Self {
        Self {
            tool_name: tool_name.into(),
            rule_content: None,
        }
    }

    /// Construit une règle ciblée sur un contenu spécifique.
    #[must_use]
    pub fn with_content(tool_name: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            tool_name: tool_name.into(),
            rule_content: Some(content.into()),
        }
    }
}

/// Parse une chaîne de règle au format `ToolName` ou `ToolName(content)`.
///
/// Renvoie une `RuleValue` avec `rule_content = None` pour les cas dégénérés
/// (parenthèses absentes ou malformées).
#[must_use]
pub fn parse_rule(rule_str: &str) -> RuleValue {
    let trimmed = rule_str.trim();

    let Some(open_idx) = find_first_unescaped(trimmed, '(') else {
        return RuleValue::tool_wide(normalize_rule_tool_name(trimmed));
    };
    let Some(close_idx) = find_last_unescaped(trimmed, ')') else {
        return RuleValue::tool_wide(trimmed);
    };
    if close_idx <= open_idx || close_idx != trimmed.len() - 1 {
        return RuleValue::tool_wide(trimmed);
    }

    let tool_name = normalize_rule_tool_name(&trimmed[..open_idx]);
    if tool_name.is_empty() {
        return RuleValue::tool_wide(trimmed);
    }
    let raw_content = &trimmed[open_idx + 1..close_idx];
    if raw_content.is_empty() || raw_content == "*" {
        return RuleValue::tool_wide(tool_name);
    }
    let content = unescape_content(raw_content);
    RuleValue::with_content(tool_name, content)
}

/// Sérialise une `RuleValue` au format textuel `Tool` / `Tool(content)`.
#[must_use]
pub fn format_rule(value: &RuleValue) -> String {
    value.rule_content.as_ref().map_or_else(
        || value.tool_name.clone(),
        |content| format!("{}({})", value.tool_name, escape_content(content)),
    )
}

/// Échappe les parenthèses et les backslashes dans un contenu de règle.
fn escape_content(content: &str) -> String {
    let mut out = String::with_capacity(content.len());
    for ch in content.chars() {
        match ch {
            '\\' => out.push_str(r"\\"),
            '(' => out.push_str(r"\("),
            ')' => out.push_str(r"\)"),
            other => out.push(other),
        }
    }
    out
}

/// Dé-échappe un contenu de règle (inverse de `escape_content`).
fn unescape_content(content: &str) -> String {
    let mut out = String::with_capacity(content.len());
    let mut chars = content.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.peek() {
                Some(&'(') => {
                    chars.next();
                    out.push('(');
                }
                Some(&')') => {
                    chars.next();
                    out.push(')');
                }
                Some(&'\\') => {
                    chars.next();
                    out.push('\\');
                }
                _ => out.push('\\'),
            }
        } else {
            out.push(ch);
        }
    }
    out
}

fn find_first_unescaped(s: &str, target: char) -> Option<usize> {
    for (idx, ch) in s.char_indices() {
        if ch == target && !is_escaped(s, idx) {
            return Some(idx);
        }
    }
    None
}

fn find_last_unescaped(s: &str, target: char) -> Option<usize> {
    let mut last = None;
    for (idx, ch) in s.char_indices() {
        if ch == target && !is_escaped(s, idx) {
            last = Some(idx);
        }
    }
    last
}

fn is_escaped(s: &str, idx: usize) -> bool {
    let mut backslashes = 0_usize;
    let mut walker = idx;
    while walker > 0 {
        walker -= 1;
        if s.as_bytes()[walker] == b'\\' {
            backslashes += 1;
        } else {
            break;
        }
    }
    backslashes % 2 == 1
}

/// Une règle complète : valeur + comportement + source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    pub value: RuleValue,
    pub behavior: PermissionBehavior,
    pub source: RuleSource,
}

/// Ensemble de règles, indexé pour des lookups efficaces.
#[derive(Debug, Clone, Default)]
pub struct RuleSet {
    rules: Vec<Rule>,
}

impl RuleSet {
    /// Construit un `RuleSet` vide.
    #[must_use]
    pub const fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Ajoute une règle.
    pub fn push(&mut self, rule: Rule) {
        self.rules.push(rule);
    }

    /// Itère sur toutes les règles d'un certain comportement, sans filtre.
    pub fn iter_behavior(&self, behavior: PermissionBehavior) -> impl Iterator<Item = &Rule> + '_ {
        self.rules.iter().filter(move |r| r.behavior == behavior)
    }

    /// Itère sur les règles tool-wide (sans `rule_content`) pour un comportement.
    pub fn iter_tool_wide(&self, behavior: PermissionBehavior) -> impl Iterator<Item = &Rule> + '_ {
        self.rules
            .iter()
            .filter(move |r| r.behavior == behavior && r.value.rule_content.is_none())
    }

    /// Toutes les règles applicables à un tool donné, avec contenu.
    pub fn iter_for_tool<'a>(
        &'a self,
        tool_name: &'a str,
        behavior: PermissionBehavior,
    ) -> impl Iterator<Item = &'a Rule> + 'a {
        self.rules.iter().filter(move |r| {
            r.behavior == behavior
                && r.value.tool_name == tool_name
                && r.value.rule_content.is_some()
        })
    }

    /// Nombre total de règles, toutes catégories confondues.
    #[must_use]
    pub fn len(&self) -> usize {
        self.rules.len()
    }

    /// `true` si le `RuleSet` ne contient aucune règle.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Regroupe les règles par `(source, behavior)` pour debug / introspection.
    #[must_use]
    pub fn group_by_source_behavior(
        &self,
    ) -> BTreeMap<(RuleSource, PermissionBehavior), Vec<String>> {
        let mut out: BTreeMap<(RuleSource, PermissionBehavior), Vec<String>> = BTreeMap::new();
        for rule in &self.rules {
            out.entry((rule.source, rule.behavior))
                .or_default()
                .push(format_rule(&rule.value));
        }
        out
    }
}

// `BTreeMap` clés : on dérive Ord pour les variantes utilisées.
impl PartialOrd for RuleSource {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for RuleSource {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (*self as u8).cmp(&(*other as u8))
    }
}
impl PartialOrd for PermissionBehavior {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for PermissionBehavior {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (*self as u8).cmp(&(*other as u8))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn parses_tool_only() {
        let v = parse_rule("Bash");
        assert_eq!(v, RuleValue::tool_wide("Bash"));
    }

    #[test]
    fn parses_leak_edit_alias() {
        let v = parse_rule("Edit(**/*.env)");
        assert_eq!(
            v,
            RuleValue::with_content("file_edit", "**/*.env")
        );
    }

    #[test]
    fn parses_tool_with_content() {
        let v = parse_rule("Bash(npm install)");
        assert_eq!(v, RuleValue::with_content("Bash", "npm install"));
    }

    #[test]
    fn parses_empty_content_as_tool_wide() {
        assert_eq!(parse_rule("Bash()"), RuleValue::tool_wide("Bash"));
        assert_eq!(parse_rule("Bash(*)"), RuleValue::tool_wide("Bash"));
    }

    #[test]
    fn parses_escaped_parens() {
        let v = parse_rule(r#"Bash(python -c "print\(1\)")"#);
        assert_eq!(
            v,
            RuleValue::with_content("Bash", r#"python -c "print(1)""#)
        );
    }

    #[test]
    fn round_trips_with_escapes() {
        let original = RuleValue::with_content("Bash", r#"python -c "print(1)""#);
        let s = format_rule(&original);
        let back = parse_rule(&s);
        assert_eq!(original, back);
    }

    #[test]
    fn malformed_falls_back_to_tool_wide() {
        assert_eq!(parse_rule("(foo)"), RuleValue::tool_wide("(foo)"));
        assert_eq!(
            parse_rule("Bash(unclosed"),
            RuleValue::tool_wide("Bash(unclosed")
        );
    }

    #[test]
    fn ruleset_iter_filters_correctly() {
        let mut set = RuleSet::new();
        set.push(Rule {
            value: RuleValue::tool_wide("Bash"),
            behavior: PermissionBehavior::Allow,
            source: RuleSource::CliArg,
        });
        set.push(Rule {
            value: RuleValue::with_content("Bash", "rm -rf /"),
            behavior: PermissionBehavior::Deny,
            source: RuleSource::UserSettings,
        });
        set.push(Rule {
            value: RuleValue::with_content("Bash", "npm install"),
            behavior: PermissionBehavior::Allow,
            source: RuleSource::ProjectSettings,
        });

        assert_eq!(set.iter_behavior(PermissionBehavior::Allow).count(), 2);
        assert_eq!(set.iter_tool_wide(PermissionBehavior::Allow).count(), 1);
        assert_eq!(
            set.iter_for_tool("Bash", PermissionBehavior::Allow).count(),
            1
        );
        assert_eq!(
            set.iter_for_tool("Bash", PermissionBehavior::Deny).count(),
            1
        );
    }
}
