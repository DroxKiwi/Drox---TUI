//! Handler `/config`.

use crate::app::AppState;
use crate::engine::EngineRuntime;

use super::SlashOutcome;

pub fn handle_config(state: &mut AppState, runtime: &EngineRuntime) -> SlashOutcome {
    for line in runtime.format_config_lines() {
        state.push_system(line);
    }
    SlashOutcome::Handled
}
