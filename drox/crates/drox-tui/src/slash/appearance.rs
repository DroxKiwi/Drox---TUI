//! Slash `/theme`, `/color`.

use crate::app::AppState;
use crate::engine::preferences::persist_from_state;
use crate::slash::SlashOutcome;
use crate::ui::{resolve_palette, SessionAccent, TuiThemeSetting};

pub fn handle_theme(args: &str, state: &mut AppState) -> SlashOutcome {
    let arg = args.trim();
    if arg.is_empty() {
        return SlashOutcome::ThemePicker;
    }
    let Some(theme) = TuiThemeSetting::from_slug(arg) else {
        let list = TuiThemeSetting::ALL
            .iter()
            .map(|t| t.label())
            .collect::<Vec<_>>()
            .join(", ");
        state.push_system(format!("Thème inconnu : `{arg}`. Disponibles : {list}"));
        return SlashOutcome::Handled;
    };
    apply_theme(state, theme);
    state.push_system(format!("Thème : {}", theme.label()));
    SlashOutcome::Handled
}

pub fn handle_color(args: &str, state: &mut AppState) -> SlashOutcome {
    let arg = args.trim().to_ascii_lowercase();
    if arg.is_empty() {
        let list = SessionAccent::ALL
            .iter()
            .map(|c| c.label())
            .collect::<Vec<_>>()
            .join(", ");
        let current = state
            .session_accent
            .map(|c| c.label())
            .unwrap_or("default");
        state.push_system(format!(
            "Couleur session courante : {current}. Disponibles : {list}, default"
        ));
        return SlashOutcome::Handled;
    }
    if matches!(arg.as_str(), "default" | "reset" | "none" | "gray" | "grey") {
        state.session_accent = None;
        state.palette = resolve_palette(state.theme, None);
        if persist_from_state(state).is_err() {
            state.push_system("Couleur réinitialisée (échec persistance disque).");
        } else {
            state.push_system("Couleur session réinitialisée (default).");
        }
        return SlashOutcome::Handled;
    }
    let Some(accent) = SessionAccent::from_slug(&arg) else {
        let list = SessionAccent::ALL
            .iter()
            .map(|c| c.label())
            .collect::<Vec<_>>()
            .join(", ");
        state.push_system(format!("Couleur invalide : `{arg}`. Disponibles : {list}, default"));
        return SlashOutcome::Handled;
    };
    state.session_accent = Some(accent);
    state.palette = resolve_palette(state.theme, Some(accent));
    if persist_from_state(state).is_err() {
        state.push_system(format!(
            "Couleur session : {} (échec persistance disque)",
            accent.label()
        ));
    } else {
        state.push_system(format!("Couleur session : {}", accent.label()));
    }
    SlashOutcome::Handled
}

pub fn apply_theme(state: &mut AppState, theme: TuiThemeSetting) {
    state.theme = theme;
    state.palette = resolve_palette(theme, state.session_accent);
    let mut prefs = crate::engine::preferences::load_preferences();
    prefs.theme = theme;
    prefs.session_color = state.session_accent;
    let _ = crate::engine::preferences::save_preferences(&prefs);
}
