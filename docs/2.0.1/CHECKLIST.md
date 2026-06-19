# Checklist — Drox TUI (`2.0.1`)

Cocher `[x]` quand validé. Noter anomalies en commentaire sous la ligne.

**Environnement** : OS ___ · Terminal ___ · Modèle ___ · Date ___

**Ligne** : produit `2.0.1` · moteur dérivé IDE `1.5.0`

---

## Démarrage & config

- [ ] Voir [PLAN-I18N-EN.md](./PLAN-I18N-EN.md) (passage UI en anglais)
- [ ] `cargo run -p drox-tui -- --workspace .` démarre sans erreur
- [ ] Modale `/server` ou prefs — connexion LLM persistée après relance
- [ ] Status line affiche modèle, workspace, tokens
- [ ] `/doctor` — LLM et workspace OK
- [ ] `/help` — liste des slash commands
- [ ] Quitte propre : Ctrl+C×2 / terminal restauré

## Fil & affichage

- [ ] Message user visible après envoi
- [ ] Réponse assistant streamée
- [ ] Markdown : titres, blocs code
- [ ] Appel d''outil visible (ex. `file_read`)
- [ ] `e` — viewer scrollable
- [ ] PgUp / PgDown — scroll fil
- [ ] `Ctrl+F` — recherche dans le fil

## Composer & saisie

- [ ] Entrée envoie · Shift+Entrée nouvelle ligne
- [ ] ↑ / ↓ — historique
- [ ] `@` — typeahead fichier
- [ ] `/` (vide) — palette slash (Entrée exécute)
- [ ] `Ctrl+Shift+L` — modale serveur IA
- [ ] `Ctrl+Shift+W` — modale workspace
- [ ] `!` — mode bash

## Agent & annulation

- [ ] Run simple sans tool
- [ ] Run avec tool read-only
- [ ] Esc / Ctrl+C pendant run — annulation propre
- [ ] File de messages — traitement séquentiel

## Permissions & questions

- [ ] Sans `--apply` : écriture → modal permission
- [ ] Plan long `exit_plan_mode` — choix visibles (footer modal)
- [ ] PgUp/PgDn — scroll corps modal Question
- [ ] Accept / deny / Esc skip

## Sessions

- [ ] `/newsession` ou nouvelle session
- [ ] Quit + `--session ses_…` ou `/resume` — fil rejoué
- [ ] `/export` — export transcript

## Personnalisation (smoke)

- [ ] `/theme` · `/color` · `/settings`
- [ ] `/vim` (optionnel)

---

## Bugs trouvés

| # | Description | Repro | Priorité |
|---|-------------|-------|----------|
| 1 | | | |
| 2 | | | |