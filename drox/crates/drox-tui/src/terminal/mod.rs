//! Gestion du terminal brut (raw mode, alternate screen).

mod title;

pub use title::{clear_terminal_title, set_terminal_title};

use std::io::{self, Stdout};

use anyhow::Context;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::event::{DisableBracketedPaste, EnableBracketedPaste};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::terminal::ClearType;
use crossterm::ExecutableCommand;

/// Restaure le terminal à la sortie (RAII).
pub struct TerminalGuard {
    mouse_enabled: bool,
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        clear_terminal_title();
        let _ = teardown(self.mouse_enabled);
    }
}

/// Active raw mode + écran alternatif (+ capture souris si demandée).
pub fn setup(mouse_enabled: bool) -> anyhow::Result<TerminalGuard> {
    enable_raw_mode().context("enable_raw_mode")?;
    let mut stdout = io::stdout();
    stdout
        .execute(EnterAlternateScreen)
        .context("EnterAlternateScreen")?;
    stdout
        .execute(crossterm::terminal::Clear(ClearType::All))
        .context("clear screen")?;
    stdout
        .execute(EnableBracketedPaste)
        .context("EnableBracketedPaste")?;
    if mouse_enabled {
        let _ = stdout.execute(EnableMouseCapture);
    }
    Ok(TerminalGuard { mouse_enabled })
}

/// Active ou désactive la capture souris sans quitter le raw mode.
pub fn set_mouse_capture(enabled: bool) -> io::Result<()> {
    let mut stdout = io::stdout();
    if enabled {
        stdout.execute(EnableMouseCapture)?;
    } else {
        stdout.execute(DisableMouseCapture)?;
    }
    Ok(())
}

fn teardown(mouse_enabled: bool) -> anyhow::Result<()> {
    let mut stdout = io::stdout();
    let _ = stdout.execute(DisableBracketedPaste);
    if mouse_enabled {
        let _ = stdout.execute(DisableMouseCapture);
    }
    disable_raw_mode().context("disable_raw_mode")?;
    stdout
        .execute(LeaveAlternateScreen)
        .context("LeaveAlternateScreen")?;
    Ok(())
}

/// Type alias pour le backend ratatui courant.
pub type TuiStdout = Stdout;
