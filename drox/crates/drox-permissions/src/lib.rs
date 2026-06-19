//! `drox-permissions` — décision allow/ask/deny pour chaque tool call.
//!
//! Cette crate ne sait rien des tools eux-mêmes : elle exige du caller qu'il
//! fournisse un `PermissionTarget` (nom du tool + contenu cible + flags
//! `is_write` / `is_read_only`). Le moteur applique ensuite un pipeline
//! déterministe inspiré du système TS d'origine (`Deny > Ask > Allow`,
//! lui-même modulé par le `PermissionMode` courant).
//!
//! ## Exemple
//!
//! ```
//! use drox_permissions::{
//!     PermissionEngine, PermissionMode, PermissionTarget, Rule, RuleSet, RuleSource,
//!     RuleValue, PermissionBehavior,
//! };
//!
//! let mut rules = RuleSet::new();
//! rules.push(Rule {
//!     value: RuleValue::with_content("Bash", "npm:*"),
//!     behavior: PermissionBehavior::Allow,
//!     source: RuleSource::UserSettings,
//! });
//! let engine = PermissionEngine::with_rules(rules);
//!
//! let target = PermissionTarget::tool("Bash").with_content("npm install drox");
//! assert!(engine.evaluate(&target, PermissionMode::Default).is_allow());
//! ```
//!
//! Voir `docs/INVENTAIRE-NOYAU-MOTEUR.md` § 2.6 et le journal sprint 1.7.

pub mod config;
pub mod engine;
pub mod error;
pub mod matcher;
pub mod mode;
pub mod path_matcher;
pub mod rule;
pub mod shadowed;
pub mod tool_names;

pub use config::{LayeredConfig, PermissionsBlock, SettingsFile};
pub use engine::{DecisionReason, PermissionDecision, PermissionEngine, PermissionTarget};
pub use error::PermissionError;
pub use matcher::ShellPattern;
pub use mode::PermissionMode;
pub use path_matcher::{PathMatchContext, dangerous_path_reason, path_rule_matches};
pub use rule::{PermissionBehavior, Rule, RuleSet, RuleSource, RuleValue, format_rule, parse_rule};
pub use shadowed::{
    DetectUnreachableOptions, ShadowType, UnreachableRule, detect_unreachable_rules,
};
pub use tool_names::{normalize_rule_tool_name, primary_rule_tool_name, uses_path_patterns};
