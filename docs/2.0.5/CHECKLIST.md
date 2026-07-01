# Checklist — Ligne 2.0.5 (connexions LLM)

## C0 — Audit

- [x] Repro échecs connexion cloud (Ollama Cloud 401 — header `x-api-key` au lieu de Bearer)
- [ ] Repro self-hosted vLLM / LM Studio (à valider manuellement)
- [x] Corriger `profile_to_llm_config` + chemins legacy/bootstrap
- [x] Tests auth headers (`connection_library`, `llm_connection`, `openai_compat`)
- [x] Messages probe actionnables (401/403/404 via `format_probe_error`)

## C1 — Self-hosted

- [x] Formulaires Ollama, vLLM, LM Studio, OpenAI-compat (presets + defaults wizard)
- [x] Éditeur headers (clé/valeur) self-hosted + Custom
- [x] `probe_connection` généralisé (Ollama natif + OpenAI-compat)
- [x] i18n FR/EN moteurs + lien doc officielle à l'étape connexion

## C2 — Cloud

- [x] `LlmProvider` + `CloudProviderChoice` : Mistral, OVH, HF, Scaleway
- [x] Connecteur Ollama Cloud (Bearer, revue doc)
- [x] Connecteurs Mistral / OVH / HF / Scaleway (OpenAI-compat + presets URL)
- [x] Formulaire wizard par prestataire (defaults, auth Bearer verrouillée si requis)
- [x] Lien doc officielle dans l'écran configuration

## C3 — Release

- [x] Migration profils JSON (`repair_misclassified_cloud_auth`)
- [x] `/doctor` — messages connexion LLM améliorés
- [x] README utilisateur (section moteurs cloud)
- [x] RELEASE_NOTES OR
- [ ] Merge → `main`

## Déjà fait (hors jalons)

- [x] Bump version workspace `2.0.5`
- [x] Spec produit `docs/2.0.5/` (connexions)
- [x] Report multi-pane → `docs/2.0.6/`
- [x] Report signing → `docs/2.0.7/`
