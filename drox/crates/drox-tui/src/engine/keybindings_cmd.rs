//! `/keybindings` — raccourcis intégrés + template `~/.drox/keybindings.json`.

use camino::Utf8PathBuf;

pub use super::keybindings::init_keybindings_file;

#[must_use]
pub fn keybindings_path() -> Utf8PathBuf {
    dirs::home_dir()
        .and_then(|h| Utf8PathBuf::from_path_buf(h.join(".drox").join("keybindings.json")).ok())
        .unwrap_or_else(|| Utf8PathBuf::from(".drox/keybindings.json"))
}

#[must_use]
pub fn format_builtin_keybindings_lines() -> Vec<String> {
    vec![
        "Raccourcis TUI (défauts + surcharge `~/.drox/keybindings.json`)".into(),
        "  Entrée          envoyer le message".into(),
        "  !               mode bash (shell direct)".into(),
        "  Shift+Entrée    nouvelle ligne composer".into(),
        "  ↑ / ↓           historique composer (buffer vide)".into(),
        "  Ctrl+F          recherche dans le fil".into(),
        "  Ctrl+R          recherche historique composer".into(),
        "  /               palette commandes slash".into(),
        "  e               viewer outil / diff / LSP / MCP · repli phase".into(),
        "  PgUp / PgDown   scroll fil".into(),
        "  Esc (run)       annuler le run agent".into(),
        "  Esc (idle)      quitter".into(),
        "  Ctrl+C (run)    annuler le run".into(),
        "  Ctrl+C ×2       quitter (idle)".into(),
        "  Ctrl+Q          quitter".into(),
        "  Ctrl+Shift+L    connexion serveur IA (/server)".into(),
        format!("  Fichier config : {}", keybindings_path()),
        "  `/keybindings init` — template JSON · `/keybindings reload` — rechargement à chaud".into(),
    ]
}

#[must_use]
pub fn generate_keybindings_template() -> String {
    let doc = serde_json::json!({
        "$schema": "https://drox.local/schemas/tui-keybindings-v1.json",
        "$comment": "Modifier ce fichier puis `/keybindings reload` ou attendre ~2s (mtime)",
        "version": 1,
        "bindings": {
            "submit": "enter",
            "multiline": "shift+enter",
            "history_up": "up",
            "history_down": "down",
            "expand_bash": "e",
            "transcript_search": "ctrl+f",
            "history_search": "ctrl+r",
            "slash_palette": "/",
            "search_next": "ctrl+n",
            "search_prev": "ctrl+p",
            "scroll_up": "pageup",
            "scroll_down": "pagedown",
            "cancel_run": "escape",
            "quit": "ctrl+q",
            "quit_confirm": "ctrl+c",
            "ai_server": "ctrl+shift+l",
            "workspace": "ctrl+shift+w"
        }
    });
    format!("{}\n", serde_json::to_string_pretty(&doc).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_is_valid_json() {
        let raw = generate_keybindings_template();
        let v: serde_json::Value = serde_json::from_str(&raw).expect("json template");
        assert_eq!(v.get("version").and_then(|v| v.as_i64()), Some(1));
    }

    #[test]
    fn builtin_lines_not_empty() {
        assert!(format_builtin_keybindings_lines().len() > 5);
    }
}
