//! Handler `/hooks`.

use crate::app::AppState;
use crate::engine::EngineRuntime;

use super::SlashOutcome;

pub fn handle_hooks(args: &str, state: &mut AppState, runtime: &EngineRuntime) -> SlashOutcome {
    let sub = args.split_whitespace().next().unwrap_or("").to_ascii_lowercase();
    if sub == "reload" {
        let active = runtime.reload_hooks();
        if active {
            state.push_system("Hooks rechargés — actifs pour les prochains runs.");
        } else {
            state.push_system(
                "Hooks rechargés — aucune entrée Pre/Post (fichiers absents ou vides).",
            );
        }
        return SlashOutcome::Handled;
    }

    for line in runtime.format_hooks_lines() {
        state.push_system(line);
    }
    if sub.is_empty() {
        state.push_system("Astuce : /hooks reload pour recharger après édition du JSON.");
    }
    SlashOutcome::Handled
}
