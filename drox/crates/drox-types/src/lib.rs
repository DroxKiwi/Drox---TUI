//! `drox-types` — types partagés, schémas, erreurs.
//!
//! Cette crate ne contient pas de logique runtime. Elle définit les contrats
//! (types serde, enums, traits sans default impl) consommés par toutes les
//! autres crates du workspace.
//!
//! Voir `docs/INVENTAIRE-NOYAU-MOTEUR.md` § 2.1 pour la liste des fichiers
//! TypeScript à porter dans cette crate.

pub mod ids;
pub mod messages;
pub mod stream;

pub use ids::{AgentId, MessageId, SessionId, ToolUseId};
pub use messages::{Content, Message, Role};
pub use stream::{StopReason, StreamEvent, Usage};
