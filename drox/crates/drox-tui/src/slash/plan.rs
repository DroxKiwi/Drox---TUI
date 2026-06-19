//! Handler `/plan`.

use crate::app::AppState;
use crate::engine::{EngineRuntime, PlanModeChange};
use crate::view::LogEntry;

use super::SlashOutcome;

const MAX_PLAN_LINES: usize = 48;

pub fn handle_plan(args: &str, state: &mut AppState, runtime: &EngineRuntime) -> SlashOutcome {
    let sub = args.split_whitespace().next().map(str::to_ascii_lowercase);
    if matches!(sub.as_deref(), Some("off") | Some("default") | Some("exit")) {
        match runtime.disable_plan_mode() {
            PlanModeChange::Disabled => {
                state.push_system("Mode plan désactivé.");
            }
            PlanModeChange::AlreadyOff => {
                state.push_system("Pas en mode plan.");
            }
            PlanModeChange::Enabled | PlanModeChange::AlreadyOn => {}
        }
        return SlashOutcome::Handled;
    }

    if !runtime.plan_mode() {
        match runtime.enable_plan_mode() {
            PlanModeChange::Enabled => {
                let objective = {
                    let prompt = args.trim();
                    if prompt.is_empty() {
                        None
                    } else {
                        Some(prompt.to_string())
                    }
                };
                state.push_entry(LogEntry::PlanActivated { objective });
            }
            PlanModeChange::AlreadyOn => {
                state.push_system("Déjà en mode plan.");
            }
            PlanModeChange::Disabled | PlanModeChange::AlreadyOff => {}
        }
        let prompt = args.trim();
        if !prompt.is_empty() {
            return SlashOutcome::RunPrompt(prompt.to_string());
        }
        return SlashOutcome::Handled;
    }

    let path = runtime.plan_file_path().to_string();
    match runtime.read_plan_file() {
        Ok(body) if !body.trim().is_empty() => {
            let truncated = body.lines().count() > MAX_PLAN_LINES;
            state.push_entry(LogEntry::PlanDocument {
                path,
                body,
                truncated,
            });
        }
        Ok(_) => {
            state.push_system(format!("Mode plan actif — fichier vide : {path}"));
        }
        Err(_) => {
            state.push_system(format!("Mode plan actif — aucun plan ({path})"));
        }
    }
    SlashOutcome::Handled
}
