//! Handler `/init`.

use crate::engine::INIT_AGENT_PROMPT;

use super::SlashOutcome;

pub fn handle_init(args: &str) -> SlashOutcome {
    let sub = args.split_whitespace().next().unwrap_or("").to_ascii_lowercase();
    if sub == "run" || sub == "go" {
        return SlashOutcome::RunPrompt(INIT_AGENT_PROMPT.to_string());
    }
    SlashOutcome::Init
}
