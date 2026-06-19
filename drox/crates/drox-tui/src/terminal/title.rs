//! Titre dynamique de l'onglet terminal (OSC 2 / BEL).

use std::io::{self, Write};

/// Met à jour le titre de l'onglet/fenêtre terminal.
pub fn set_terminal_title(title: &str) {
    let clean = strip_escape_sequences(title);
    if clean.is_empty() {
        return;
    }
    // stderr : évite de polluer le buffer ratatui sur stdout.
    let seq = format!("\x1b]0;{clean}\x07");
    let _ = io::stderr().write_all(seq.as_bytes());
    #[cfg(windows)]
    {
        // Windows Terminal et conhost récents honorent OSC ; `clip` reste sur stdout.
        let _ = io::stdout().write_all(seq.as_bytes());
    }
}

/// Restaure un titre neutre à la sortie.
pub fn clear_terminal_title() {
    set_terminal_title("Drox");
}

fn strip_escape_sequences(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if chars.next_if_eq(&'[').is_some() {
                for ch in chars.by_ref() {
                    if ch.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(c);
    }
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_ansi_from_title() {
        assert_eq!(strip_escape_sequences("\x1b[1;32mok\x1b[0m"), "ok");
    }
}
