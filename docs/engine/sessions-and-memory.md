# Sessions et mémoire

## Transcripts

Chaque session agent porte un id `ses_<uuid>`.

| Donnée | Chemin typique |
|--------|----------------|
| Transcript | `~/.drox/sessions/ses_*.jsonl` |
| Meta / stats UI | `ses_*.meta.json`, `ses_*.ui-stats.json` |
| Override | `--session-dir` |

Modules : `drox-session` (`paths.rs`, sink JSONL).

Actions TUI : nouvelle session, `/resume`, rejeu UI depuis JSONL (`hydrate_transcript_ui`).

## Prefs et logs TUI

| Fichier | Rôle |
|---------|------|
| `~/.drox/tui-preferences.json` | Connexion LLM, thème, MAJ, profils |
| `~/.drox/keybindings.json` | Raccourcis |
| `~/.drox/tui.log` | Logs (pas sur stderr — écran alternatif) |

## Workspace `.drox/`

| Fichier / dossier | Rôle |
|-------------------|------|
| `.env` / `env` | `DROX_SERVER`, `DROX_MODEL`, `DROX_API_KEY`, … |
| `settings.json` (+ local) | Permissions |
| `hooks.json` | Hooks tools |
| `memory/sessions/` | Notes markdown long terme |
| `MEMORY.md`, `DROX.md` | Memdir injecté dans le system prompt |

Ignore agent : `<workspace>/.droxignore`.

## Compaction

Gérée dans `drox-context` + politique de `AgentConfig` pendant `drive_inner` : le contexte est snippé / compacté pour rester dans le budget tokens.
