# Plan - English UI and prompts (branch 0.0.0)

**Status:** draft  
**Goal:** All user-facing TUI strings and all LLM system prompts in **English** by default.

## Current state

| Area | Language today | i18n ready? |
|------|----------------|-------------|
| TUI widgets | Mostly French | No |
| System messages in transcript | Mixed FR | No |
| Slash palette descriptions | French | No |
| Engine system prompt | English | N/A |
| drox-cli prompts | English | N/A |
| DROX_PRIMARY_LANGUAGE | Assistant reply language only | Partial |

**Conclusion:** No TUI multilingual framework exists. `DROX_PRIMARY_LANGUAGE` only affects assistant response language, not the interface.

## Work packages

### P1 - Inventory
- Grep French strings in `drox-tui` (widgets, run.rs, slash, notices, preferences)
- List widget files, run.rs, slash, notices, preferences
- Exclude tests that assert French copy (update assertions)

### P2 - TUI English pass
1. Modals (`/server`, `/workspace`, onboarding, theme, copy, rewind)
2. Composer, footer, help
3. Notices and status lines
4. Slash palette and `/help` text
5. Toasts and push_system messages

### P3 - System prompts
- Audit `drox-cli::language.rs` and `build_system_prompt`
- English default for `language::merge_into_system` when env unset
- Document that `DROX_PRIMARY_LANGUAGE` is reply-only

### P4 - Tests and docs
- Update string assertions
- README and CHECKLIST in English (or bilingual note)
- `cargo test -p drox-tui` green

### P5 - Future i18n (post-0.0.0)
- Optional `tui-preferences.json` locale (`en`, `fr`)
- Central `strings.rs` or lightweight macro crate

## Acceptance
- Fresh install: UI in English
- System prompt English unless `DROX_PRIMARY_LANGUAGE` set
- `/server` and Ctrl+Shift+L work from any non-running phase
- Context max presets: 16k / 32k / 64k / 128k / 256k / 512k / 1M / custom
- `cargo test -p drox-tui` green

## Same branch fixes (done)
- `/server` + Ctrl+Shift+L global shortcuts (case-insensitive on Windows)
- Slash palette executes commands on Enter (not insert-only)
- `num_ctx` in `/server` modal + `tui-preferences.json`