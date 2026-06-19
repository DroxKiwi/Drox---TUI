//! Handler `/mcp` (async).

use super::SlashOutcome;

pub fn handle_mcp(args: &str) -> SlashOutcome {
    SlashOutcome::Mcp {
        args: args.to_string(),
    }
}
