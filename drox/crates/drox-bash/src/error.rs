//! Erreurs du parseur / analyseur Bash.

use thiserror::Error;

/// Erreur retournée par les API publiques de `drox-bash`.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum BashError {
    /// Impossible d'initialiser le parseur tree-sitter.
    #[error("failed to initialize bash parser")]
    ParserInit,

    /// Texte source non UTF-8 valide sur une plage de nœud.
    #[error("invalid UTF-8 in bash source")]
    Utf8(#[from] std::str::Utf8Error),

    /// Trop de sous-commandes extraites (protection `DoS`, alignée sur le TS).
    #[error("too many bash subcommands: {0} (max {MAX_SUBCOMMANDS})")]
    TooManySubcommands(usize),

    /// Arbre syntaxique avec erreurs de parse (on retombe sur une analyse conservative).
    #[error("bash parse tree contains errors")]
    TreeHasErrors,
}

/// Plafond aligné sur `MAX_SUBCOMMANDS_FOR_SECURITY_CHECK` dans `bashPermissions.ts`.
pub const MAX_SUBCOMMANDS: usize = 50;
