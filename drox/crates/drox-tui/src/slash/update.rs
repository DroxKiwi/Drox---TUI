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
            state.push_system(i18n::tf(u::UPDATE_UNKNOWN_SUB, &sub));
            state.push_system(i18n::t(u::UPDATE_HELP_BODY));
            SlashOutcome::Handled
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::AppState;

    fn outcome_sub(input: &str) -> Option<UpdateCommand> {
        let mut state = AppState::default();
        match handle_update(input, &mut state) {
            SlashOutcome::Update(cmd) => Some(cmd),
            _ => None,
        }
    }

    #[test]
    fn parses_on_off_and_check() {
        assert_eq!(outcome_sub(""), Some(UpdateCommand::ShowHelp));
        assert_eq!(outcome_sub("on"), Some(UpdateCommand::On));
        assert_eq!(outcome_sub("ON"), Some(UpdateCommand::On));
        assert_eq!(outcome_sub("off"), Some(UpdateCommand::Off));
        assert_eq!(outcome_sub("check"), Some(UpdateCommand::Check));
        assert_eq!(outcome_sub("snooze 14"), Some(UpdateCommand::Snooze { days: 14 }));
        assert_eq!(outcome_sub("install"), Some(UpdateCommand::Install));
    }

    #[test]
    fn unknown_subcommand_stays_handled() {
        let mut state = AppState::default();
        assert!(matches!(
            handle_update("typo", &mut state),
            SlashOutcome::Handled
        ));
    }
}
