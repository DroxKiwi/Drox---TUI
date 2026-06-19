//! Handler `/doctor` (async).

use super::SlashOutcome;

pub fn handle_doctor() -> SlashOutcome {
    SlashOutcome::Doctor
}
