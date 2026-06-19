//! Chargement et correspondance des raccourcis `~/.drox/keybindings.json`.

use std::collections::HashMap;
use std::fs;
use std::time::SystemTime;

use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::Deserialize;

use super::keybindings_cmd::{generate_keybindings_template, keybindings_path};

/// Action rebindable via le fichier JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BindingAction {
    Submit,
    Multiline,
    HistoryUp,
    HistoryDown,
    ExpandBash,
    TranscriptSearch,
    HistorySearch,
    SlashPalette,
    SearchNext,
    SearchPrev,
    ScrollUp,
    ScrollDown,
    CancelRun,
    Quit,
    QuitConfirm,
    AiServer,
    Workspace,
}

impl BindingAction {
    const ALL: &'static [Self] = &[
        Self::Submit,
        Self::Multiline,
        Self::HistoryUp,
        Self::HistoryDown,
        Self::ExpandBash,
        Self::TranscriptSearch,
        Self::HistorySearch,
        Self::SlashPalette,
        Self::SearchNext,
        Self::SearchPrev,
        Self::ScrollUp,
        Self::ScrollDown,
        Self::CancelRun,
        Self::Quit,
        Self::QuitConfirm,
    ];

    fn json_key(self) -> &'static str {
        match self {
            Self::Submit => "submit",
            Self::Multiline => "multiline",
            Self::HistoryUp => "history_up",
            Self::HistoryDown => "history_down",
            Self::ExpandBash => "expand_bash",
            Self::TranscriptSearch => "transcript_search",
            Self::HistorySearch => "history_search",
            Self::SlashPalette => "slash_palette",
            Self::SearchNext => "search_next",
            Self::SearchPrev => "search_prev",
            Self::ScrollUp => "scroll_up",
            Self::ScrollDown => "scroll_down",
            Self::CancelRun => "cancel_run",
            Self::Quit => "quit",
            Self::QuitConfirm => "quit_confirm",
            Self::AiServer => "ai_server",
            Self::Workspace => "workspace",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedKey {
    code: KeyCode,
    modifiers: KeyModifiers,
}

/// Raccourcis actifs (défauts + surcharge fichier).
#[derive(Debug, Clone)]
pub struct TuiKeybindings {
    map: HashMap<BindingAction, ParsedKey>,
    path: Utf8PathBuf,
    mtime: Option<SystemTime>,
}

impl TuiKeybindings {
    /// Charge depuis le disque ou retombe sur les valeurs intégrées.
    #[must_use]
    pub fn load() -> Self {
        let path = keybindings_path();
        let mut kb = Self {
            map: default_bindings(),
            path: path.clone(),
            mtime: file_mtime(&path),
        };
        if let Ok(raw) = fs::read_to_string(path.as_std_path()) {
            kb.merge_file(&raw);
        }
        kb
    }

    /// Recharge si le fichier a changé depuis le dernier chargement.
    pub fn reload_if_changed(&mut self) -> bool {
        let path = keybindings_path();
        let new_mtime = file_mtime(&path);
        if new_mtime == self.mtime && path == self.path {
            return false;
        }
        self.path = path.clone();
        self.mtime = new_mtime;
        self.map = default_bindings();
        if let Ok(raw) = fs::read_to_string(path.as_std_path()) {
            self.merge_file(&raw);
        }
        true
    }

    /// Recharge explicitement (`/keybindings reload`).
    pub fn reload(&mut self) -> anyhow::Result<()> {
        self.path = keybindings_path();
        self.mtime = file_mtime(&self.path);
        self.map = default_bindings();
        if self.path.exists() {
            let raw = fs::read_to_string(self.path.as_std_path())
                .with_context(|| format!("lecture {}", self.path))?;
            self.merge_file(&raw);
        }
        Ok(())
    }

    #[must_use]
    pub fn path(&self) -> &Utf8Path {
        &self.path
    }

    #[must_use]
    pub fn matches(&self, action: BindingAction, key: &KeyEvent) -> bool {
        self.map
            .get(&action)
            .is_some_and(|parsed| key_matches(parsed, key))
    }

    fn merge_file(&mut self, raw: &str) {
        let Ok(doc) = serde_json::from_str::<KeybindingsFile>(raw) else {
            return;
        };
        for (action, spec) in doc.bindings {
            let Some(parsed) = parse_key_spec(&spec) else {
                continue;
            };
            if let Some(act) = action_from_json_key(&action) {
                self.map.insert(act, parsed);
            }
        }
    }
}

#[derive(Debug, Deserialize)]
struct KeybindingsFile {
    #[serde(default)]
    bindings: HashMap<String, String>,
}

fn default_bindings() -> HashMap<BindingAction, ParsedKey> {
    let defaults = [
        (BindingAction::Submit, "enter"),
        (BindingAction::Multiline, "shift+enter"),
        (BindingAction::HistoryUp, "up"),
        (BindingAction::HistoryDown, "down"),
        (BindingAction::ExpandBash, "e"),
        (BindingAction::TranscriptSearch, "ctrl+f"),
        (BindingAction::HistorySearch, "ctrl+r"),
        (BindingAction::SlashPalette, "/"),
        (BindingAction::SearchNext, "ctrl+n"),
        (BindingAction::SearchPrev, "ctrl+p"),
        (BindingAction::ScrollUp, "pageup"),
        (BindingAction::ScrollDown, "pagedown"),
        (BindingAction::CancelRun, "escape"),
        (BindingAction::Quit, "ctrl+q"),
        (BindingAction::QuitConfirm, "ctrl+c"),
        (BindingAction::AiServer, "ctrl+shift+l"),
        (BindingAction::Workspace, "ctrl+shift+w"),
    ];
    defaults
        .into_iter()
        .filter_map(|(a, s)| parse_key_spec(s).map(|p| (a, p)))
        .collect()
}

fn action_from_json_key(key: &str) -> Option<BindingAction> {
    BindingAction::ALL
        .iter()
        .copied()
        .find(|a| a.json_key() == key)
}

fn parse_key_spec(spec: &str) -> Option<ParsedKey> {
    let lower = spec.trim().to_ascii_lowercase();
    if lower.is_empty() {
        return None;
    }
    let parts: Vec<&str> = lower.split('+').collect();
    let (code_str, mods) = if parts.len() == 1 {
        (parts[0], KeyModifiers::empty())
    } else {
        let code_str = *parts.last()?;
        let mut mods = KeyModifiers::empty();
        for part in &parts[..parts.len() - 1] {
            match *part {
                "ctrl" | "control" => mods |= KeyModifiers::CONTROL,
                "shift" => mods |= KeyModifiers::SHIFT,
                "alt" => mods |= KeyModifiers::ALT,
                _ => return None,
            }
        }
        (code_str, mods)
    };

    let code = match code_str {
        "enter" | "return" => KeyCode::Enter,
        "escape" | "esc" => KeyCode::Esc,
        "backspace" => KeyCode::Backspace,
        "pageup" => KeyCode::PageUp,
        "pagedown" => KeyCode::PageDown,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "tab" => KeyCode::Tab,
        "space" => KeyCode::Char(' '),
        s if s.len() == 1 => {
            let c = s.chars().next()?;
            KeyCode::Char(c)
        }
        _ => return None,
    };

    Some(ParsedKey { code, modifiers: mods })
}

fn key_matches(parsed: &ParsedKey, event: &KeyEvent) -> bool {
    if !codes_match(parsed.code, event.code) {
        return false;
    }
    if !event.modifiers.contains(parsed.modifiers) {
        return false;
    }
    if parsed.modifiers.is_empty()
        && event
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
    {
        return false;
    }
    true
}

/// Windows envoie souvent `Char('L')` pour Ctrl+Shift+L alors que le binding est `Char('l')`.
fn codes_match(expected: KeyCode, actual: KeyCode) -> bool {
    match (expected, actual) {
        (KeyCode::Char(a), KeyCode::Char(b)) => a.eq_ignore_ascii_case(&b),
        _ => expected == actual,
    }
}

fn file_mtime(path: &Utf8Path) -> Option<SystemTime> {
    fs::metadata(path.as_std_path())
        .ok()
        .and_then(|m| m.modified().ok())
}

/// Crée le template si absent (délègue à `keybindings_cmd`).
pub fn init_keybindings_file() -> anyhow::Result<(bool, Utf8PathBuf)> {
    let path = keybindings_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent.as_std_path()).context("création ~/.drox")?;
    }
    if path.exists() {
        return Ok((false, path));
    }
    fs::write(path.as_std_path(), generate_keybindings_template())
        .context("écriture keybindings.json")?;
    Ok((true, path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ctrl_f() {
        let p = parse_key_spec("ctrl+f").unwrap();
        assert_eq!(p.code, KeyCode::Char('f'));
        assert!(p.modifiers.contains(KeyModifiers::CONTROL));
    }

    #[test]
    fn matches_submit_enter() {
        let kb = TuiKeybindings {
            map: default_bindings(),
            path: Utf8PathBuf::from("."),
            mtime: None,
        };
        let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::empty());
        assert!(kb.matches(BindingAction::Submit, &key));
    }

    #[test]
    fn rejects_ctrl_with_plain_e() {
        let p = parse_key_spec("e").unwrap();
        let key = KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL);
        assert!(!key_matches(&p, &key));
    }

    #[test]
    fn matches_ctrl_shift_l_case_insensitive() {
        let kb = TuiKeybindings {
            map: default_bindings(),
            path: Utf8PathBuf::from("."),
            mtime: None,
        };
        let lower = KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL | KeyModifiers::SHIFT);
        let upper = KeyEvent::new(KeyCode::Char('L'), KeyModifiers::CONTROL | KeyModifiers::SHIFT);
        assert!(kb.matches(BindingAction::AiServer, &lower));
        assert!(kb.matches(BindingAction::AiServer, &upper));
    }
}
