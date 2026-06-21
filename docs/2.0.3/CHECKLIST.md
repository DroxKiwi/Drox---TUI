# Checklist — Ligne 2.0.3

## `/update`

- [x] Entrée palette slash `/update`
- [x] Handler slash + sous-commandes
- [x] Clés i18n FR/EN
- [x] Prefs `update.*` persistées
- [x] `engine/update.rs` + fetch `latest.json`
- [x] `publish-or.ps1` génère `latest.json`
- [x] Bandeau opt-in (M3)
- [x] Aucune requête réseau au boot par défaut

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
