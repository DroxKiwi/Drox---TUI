# Backends LLM

## Principe

Drox TUI **n’embarque pas** de modèle. Il parle HTTP à **ton** serveur :

- [Ollama](https://ollama.com/) local (recommandé pour débuter)
- API **OpenAI-compatible** (vLLM, LM Studio, clouds que tu configures)

Aucun cloud KDDS imposé.

## Crate `drox-llm`

- `LlmConfig::try_from_str(url, model)` choisit le style d’API
- Client unifié (type historique `OllamaClient`) pour le streaming
- Auth : `DROX_API_KEY`, headers custom selon le profil

## Bibliothèque de connexions (TUI)

Fichier : `drox/crates/drox-tui/src/engine/connection_library.rs`

Presets : Ollama local, Ollama Cloud, Mistral, OVH, Hugging Face, Scaleway, vLLM, LM Studio, OpenAI-compat…

Persistance : `~/.drox/tui-preferences.json` (`llm_connection`, profils).

UI : slash `/server`, raccourci `Ctrl+Shift+L` — dialogue + probe async (`engine/llm_connection.rs`).

## Env

Chargés au boot (`drox-cli` `env_file`) depuis `<workspace>/.drox/.env` puis `~/.drox/.env` :

- `DROX_SERVER`, `DROX_MODEL`, `DROX_API_KEY`

Flags CLI `--server` / workspace peuvent surcharger.

## Docs produit

- [`docs/2.0.5/PROVIDERS.md`](../2.0.5/PROVIDERS.md)
- Plan historique connexions sous `docs/2.0.5/`
