# Plan — English UI and prompts (line `2.0.1`)

**Status:** draft  
**Goal:** All user-facing TUI strings and LLM system prompts in **English** by default.

## Current state

| Area | Language today | i18n ready? |
|---|---|---|
| TUI widgets | Mostly French | No |
| System messages in transcript | Mixed FR | No |
| Slash palette descriptions | French | No |
| Engine system prompt | English | N/A |
| `drox-cli` prompts | English | N/A |
| `DROX_PRIMARY_LANGUAGE` | Assistant reply language only | Partial |

**Conclusion:** No TUI i18n framework exists. `DROX_PRIMARY_LANGUAGE` affects assistant responses, not the interface.

## Work packages

### P1 — Inventory
- Grep French strings in `drox-tui` (widgets, `run.rs`, slash, notices, preferences)
- Exclude tests that assert French copy (update assertions)

### P2 — TUI English pass
1. Modals `/server`, `/workspace`, onboarding, theme, copy, rewind
2. Composer, footer, help
3. Notices and status lines
4. Slash palette and `/help` text
5. Toasts and `push_system` messages

### P3 — System prompts
- Audit `drox-cli::language` and `build_system_prompt`
- English default when env unset
- Document that `DROX_PRIMARY_LANGUAGE` is reply-only

### P4 — Tests and docs
- Update string assertions
- `cargo test -p drox-tui` green

### P5 — Future i18n (post-2.0.1)
- Optional locale in `tui-preferences.json`
- Central strings module
- Do not block `2.0.1` on full i18n

## Acceptance
- Fresh install: UI in English
- Shortcuts and server modal behavior unchanged
- `cargo test -p drox-tui` green