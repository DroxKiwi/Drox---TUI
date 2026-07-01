# Checklist — Ligne 2.0.5 (connexions LLM)

## C0 — Audit

- [ ] Repro échecs connexion cloud actuelle (Ollama Cloud)
- [ ] Repro self-hosted vLLM / LM Studio
- [ ] Corriger `profile_to_llm_config` si gaps
- [ ] Tests mock auth headers

## C1 — Self-hosted

- [ ] Formulaires Ollama, vLLM, LM Studio, OpenAI-compat
- [ ] Éditeur headers (clé/valeur) self-hosted + Custom
- [ ] `probe_connection` généralisé
- [ ] i18n FR/EN

## C2 — Cloud

- [ ] `LlmProvider` + `CloudProviderChoice` : Mistral, OVH, HF, Scaleway
- [ ] Connecteur Ollama Cloud (revue doc officielle)
- [ ] Connecteur Mistral
- [ ] Connecteur OVHcloud AI Endpoints
- [ ] Connecteur Hugging Face
- [ ] Connecteur Scaleway Generative APIs
- [ ] Formulaire dédié par prestataire + lien doc

## C3 — Release

- [ ] Migration profils JSON existants
- [ ] `/doctor` connexion active
- [ ] README utilisateur
- [ ] RELEASE_NOTES OR
- [ ] Merge → `main`

## Déjà fait

- [x] Bump version workspace `2.0.5`
- [x] Spec produit `docs/2.0.5/` (connexions)
- [x] Report multi-pane → `docs/2.0.6/`
- [x] Report signing → `docs/2.0.7/`
