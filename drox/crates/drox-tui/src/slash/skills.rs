//! Handler `/skills` (async).

use super::SlashOutcome;

pub fn handle_skills(args: &str) -> SlashOutcome {
    SlashOutcome::Skills {
        args: args.to_string(),
    }
}
