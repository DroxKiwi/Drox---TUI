//! Pont interactif moteur ↔ TUI (questions / permissions).

mod answer;
mod coordinator;

pub use answer::parse_answer;
pub use coordinator::{AskCoordinator, PendingAsk, TuiUserAsker};
