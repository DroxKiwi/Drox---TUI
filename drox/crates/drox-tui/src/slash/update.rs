//! Handler `/update` (opt-in MAJ TUI).

use crate::app::AppState;
use crate::i18n::{self, keys_update as u};

use super::SlashOutcome;

/// Sous-commande `/update` déléguée à `run.rs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateCommand {
    ShowHelp,
    Check,
    On,
    Off,
    Snooze { days: u32 },
    Dismiss,
    Install,
}

pub fn handle_update(args: &str, state: &mut AppState) -> SlashOutcome {
    let sub = args.split_whitespace().next().unwrap_or("").to_ascii_lowercase();
    match sub.as_str() {
        "" => SlashOutcome::Update(UpdateCommand::ShowHelp),
        "check" => SlashOutcome::Update(UpdateCommand::Check),
        "on" | "enable" | "true" | "1" => SlashOutcome::Update(UpdateCommand::On),
        "off" | "disable" | "false" | "0" => SlashOutcome::Update(UpdateCommand::Off),
        "snooze" => {
            let days = args
                .split_whitespace()
                .nth(1)
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(7);
            if days == 0 {
                state.push_system(i18n::t(u::UPDATE_SNOOZE_USAGE));
                SlashOutcome::Handled
            } else {
                SlashOutcome::Update(UpdateCommand::Snooze { days })
            }
        }
        "dismiss" | "ignore" => SlashOutcome::Update(UpdateCommand::Dismiss),
        "install" => SlashOutcome::Update(UpdateCommand::Install),
        "help" => {
            state.push_system(i18n::t(u::UPDATE_HELP_BODY));
            SlashOutcome::Handled
        }
        _ => {
            state.push_system(i18n::t(u::UPDATE_HELP_BODY));
            SlashOutcome::Handled
        }
    }
}
