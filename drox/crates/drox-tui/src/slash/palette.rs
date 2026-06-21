//! Liste des commandes slash pour la palette (`/`).

use crate::i18n::{self, keys_p1 as k};

/// Entrée palette : commande + clé description i18n.
#[derive(Debug, Clone, Copy)]
pub struct SlashPaletteEntry {
    pub command: &'static str,
    pub desc_key: &'static str,
}

impl SlashPaletteEntry {
    #[must_use]
    pub fn description(self) -> &'static str {
        i18n::t(self.desc_key)
    }
}

pub const ENTRIES: &[SlashPaletteEntry] = &[
    SlashPaletteEntry { command: "/help", desc_key: k::SLASH_PALETTE_HELP },
    SlashPaletteEntry { command: "/server", desc_key: k::SLASH_PALETTE_SERVER },
    SlashPaletteEntry { command: "/workspace", desc_key: k::SLASH_PALETTE_WORKSPACE },
    SlashPaletteEntry { command: "/clear", desc_key: k::SLASH_PALETTE_CLEAR },
    SlashPaletteEntry { command: "/status", desc_key: k::SLASH_PALETTE_STATUS },
    SlashPaletteEntry { command: "/context", desc_key: k::SLASH_PALETTE_CONTEXT },
    SlashPaletteEntry { command: "/cost", desc_key: k::SLASH_PALETTE_COST },
    SlashPaletteEntry { command: "/compact", desc_key: k::SLASH_PALETTE_COMPACT },
    SlashPaletteEntry { command: "/memory", desc_key: k::SLASH_PALETTE_MEMORY },
    SlashPaletteEntry { command: "/search", desc_key: k::SLASH_PALETTE_SEARCH },
    SlashPaletteEntry { command: "/permissions", desc_key: k::SLASH_PALETTE_PERMISSIONS },
    SlashPaletteEntry { command: "/plan", desc_key: k::SLASH_PALETTE_PLAN },
    SlashPaletteEntry { command: "/config", desc_key: k::SLASH_PALETTE_CONFIG },
    SlashPaletteEntry { command: "/doctor", desc_key: k::SLASH_PALETTE_DOCTOR },
    SlashPaletteEntry { command: "/hooks", desc_key: k::SLASH_PALETTE_HOOKS },
    SlashPaletteEntry { command: "/mcp", desc_key: k::SLASH_PALETTE_MCP },
    SlashPaletteEntry { command: "/skills", desc_key: k::SLASH_PALETTE_SKILLS },
    SlashPaletteEntry { command: "/session", desc_key: k::SLASH_PALETTE_SESSION },
    SlashPaletteEntry { command: "/rename", desc_key: k::SLASH_PALETTE_RENAME },
    SlashPaletteEntry { command: "/sessions", desc_key: k::SLASH_PALETTE_SESSIONS },
    SlashPaletteEntry { command: "/resume", desc_key: k::SLASH_PALETTE_RESUME },
    SlashPaletteEntry { command: "/rewind", desc_key: k::SLASH_PALETTE_REWIND },
    SlashPaletteEntry { command: "/copy", desc_key: k::SLASH_PALETTE_COPY },
    SlashPaletteEntry { command: "/add-dir", desc_key: k::SLASH_PALETTE_ADD_DIR },
    SlashPaletteEntry { command: "/export", desc_key: k::SLASH_PALETTE_EXPORT },
    SlashPaletteEntry { command: "/diff", desc_key: k::SLASH_PALETTE_DIFF },
    SlashPaletteEntry { command: "/files", desc_key: k::SLASH_PALETTE_FILES },
    SlashPaletteEntry { command: "/branch", desc_key: k::SLASH_PALETTE_BRANCH },
    SlashPaletteEntry { command: "/theme", desc_key: k::SLASH_PALETTE_THEME },
    SlashPaletteEntry { command: "/color", desc_key: k::SLASH_PALETTE_COLOR },
    SlashPaletteEntry { command: "/vim", desc_key: k::SLASH_PALETTE_VIM },
    SlashPaletteEntry { command: "/keybindings", desc_key: k::SLASH_PALETTE_KEYBINDINGS },
    SlashPaletteEntry { command: "/terminal-setup", desc_key: k::SLASH_PALETTE_TERMINAL_SETUP },
    SlashPaletteEntry { command: "/settings", desc_key: k::SLASH_PALETTE_SETTINGS },
    SlashPaletteEntry { command: "/language", desc_key: k::SLASH_PALETTE_LANGUAGE },
    SlashPaletteEntry { command: "/onboarding", desc_key: k::SLASH_PALETTE_ONBOARDING },
    SlashPaletteEntry { command: "/init", desc_key: k::SLASH_PALETTE_INIT },
    SlashPaletteEntry { command: "/sandbox", desc_key: k::SLASH_PALETTE_SANDBOX },
    SlashPaletteEntry { command: "/review", desc_key: k::SLASH_PALETTE_REVIEW },
    SlashPaletteEntry { command: "/security-review", desc_key: k::SLASH_PALETTE_SECURITY_REVIEW },
    SlashPaletteEntry { command: "/statusline", desc_key: k::SLASH_PALETTE_STATUSLINE },
    SlashPaletteEntry { command: "/exit", desc_key: k::SLASH_PALETTE_EXIT },
];

/// Filtre les entrées par préfixe ou sous-chaîne (insensible à la casse).
#[must_use]
pub fn filter_entries(filter: &str) -> Vec<usize> {
    let raw = filter.trim();
    let needle = raw.strip_prefix('/').unwrap_or(raw).to_ascii_lowercase();
    if needle.is_empty() {
        return (0..ENTRIES.len()).collect();
    }
    ENTRIES
        .iter()
        .enumerate()
        .filter_map(|(i, e)| {
            let cmd = e.command.trim_start_matches('/').to_ascii_lowercase();
            let desc = e.description().to_ascii_lowercase();
            if cmd.starts_with(&needle) || e.command.to_ascii_lowercase().contains(&needle) || desc.contains(&needle) {
                Some(i)
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_by_prefix() {
        let hits = filter_entries("mem");
        assert!(hits.iter().any(|&i| ENTRIES[i].command == "/memory"));
    }

    #[test]
    fn empty_filter_lists_all() {
        assert_eq!(filter_entries("").len(), ENTRIES.len());
    }
}
