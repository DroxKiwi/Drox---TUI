//! Gestion du terminal brut (raw mode, alternate screen).

mod title;

pub use title::{clear_terminal_title, set_terminal_title};

use std::io::{self, Stdout};

use anyhow::Context;
use crossterm::event::{DisableBracketedPaste, EnableBracketedPaste};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::terminal::ClearType;
use crossterm::ExecutableCommand;

/// Restaure le terminal à la sortie (RAII).
pub struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        clear_terminal_title();
        let _ = teardown();
    }
}

/// Active raw mode + écran alternatif.
pub fn setup() -> anyhow::Result<TerminalGuard> {
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
    Ok(TerminalGuard)
}

fn teardown() -> anyhow::Result<()> {
    let mut stdout = io::stdout();
    let _ = stdout.execute(DisableBracketedPaste);
    disable_raw_mode().context("disable_raw_mode")?;
    stdout
        .execute(LeaveAlternateScreen)
        .context("LeaveAlternateScreen")?;
    Ok(())
}

/// Type alias pour le backend ratatui courant.
pub type TuiStdout = Stdout;
