//! Décodage clavier terminal (AltGr Windows, etc.).

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

/// Caractère saisi pour champs texte (composer, chemins…).
///
/// Sur Windows, AltGr envoie souvent `CONTROL | ALT` — ne pas le confondre avec un raccourci Ctrl.
#[must_use]
pub fn typed_char(key: &KeyEvent) -> Option<char> {
    if !matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
        return None;
    }
    match key.code {
        KeyCode::Char(c) if allows_text_modifiers(key.modifiers) => Some(c),
        _ => None,
    }
}

#[must_use]
pub fn allows_text_modifiers(m: KeyModifiers) -> bool {
    if m.contains(KeyModifiers::CONTROL) && m.contains(KeyModifiers::ALT) {
        return true;
    }
    !m.intersects(KeyModifiers::CONTROL | KeyModifiers::SUPER)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn altgr_is_typing_not_shortcut() {
        let key = KeyEvent::new(
            KeyCode::Char('\\'),
            KeyModifiers::CONTROL | KeyModifiers::ALT,
        );
        assert_eq!(typed_char(&key), Some('\\'));
    }

    #[test]
    fn ctrl_c_is_not_typing() {
        let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(typed_char(&key), None);
    }
}
