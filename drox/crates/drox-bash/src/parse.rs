//! Parse Bash via tree-sitter.

use tree_sitter::{Parser, Tree};

use crate::error::BashError;

/// Parse une chaîne Bash et retourne l'arbre tree-sitter.
///
/// Échoue uniquement si le parseur ne peut pas charger la grammaire (très rare).
pub fn parse_bash(source: &str) -> Result<Tree, BashError> {
    let mut parser = Parser::new();
    let language: tree_sitter::Language = tree_sitter_bash::LANGUAGE.into();
    parser
        .set_language(&language)
        .map_err(|_| BashError::ParserInit)?;
    parser.parse(source, None).ok_or(BashError::ParserInit)
}
