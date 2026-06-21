# Checklist — Ligne 2.0.3

## `/update`

- [ ] Entrée palette slash `/update`
- [ ] Handler slash + sous-commandes
- [ ] Clés i18n FR/EN
- [ ] Prefs `update.*` persistées
- [ ] `engine/update.rs` + fetch `latest.json`
- [ ] `publish-or.ps1` génère `latest.json`
- [ ] Bandeau opt-in (M3)
- [ ] Aucune requête réseau au boot par défaut

## Linux

- [ ] `publish-or` inclut archive Linux
- [ ] Asset Linux sur GitHub Release
- [ ] Racines workspace (`$HOME`, etc.)
- [ ] QA terminal Linux (≥ 1 environnement réel)
- [ ] CI `ubuntu-latest` (tests)

## Non-régression

- [ ] `cargo test -p drox-tui`
- [ ] Build installateur Windows
- [ ] Certification local-first (opt-in MAJ)

## Release

- [ ] Bump version 2.0.3
- [ ] RELEASE_NOTES OR
- [ ] Merge `2.0.3` → `main`
