//! `drox-bash` — parser Bash (tree-sitter) et classifieur de commandes.
//!
//! Sprint 1.8 : découpe `simple_command` via l’AST, classification heuristique
//! (lecture / mutation / réseau / destructif), message informatif pour les
//! motifs destructifs connus (inspiré de `destructiveCommandWarning.ts`).
//!
//! Le `bashClassifier.ts` du dépôt TS est un stub ; cette crate définit la
//! logique côté Rust (voir `docs/INVENTAIRE-NOYAU-MOTEUR.md` § 5.1).

pub mod classify;
pub mod error;
pub mod parse;
pub mod split;

pub use classify::{
    BashCommandKind, auto_deny_message, destructive_hint, first_executable_token, kind_of_segment,
    permission_flags,
};
pub use error::{BashError, MAX_SUBCOMMANDS};
pub use parse::parse_bash;
pub use split::split_command_segments;
