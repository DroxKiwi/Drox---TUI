//! Bibliothèque partagée du binaire `drox` (prompts, env, langue).
//!
//! Utilisée par `drox-tui` et autres clients sans dupliquer la configuration
//! système du moteur.

pub mod env_file;
pub mod language;
pub mod prompts;
