# Checklist — Ligne 2.0.4

## Diff visuel

- [x] M1 — `/diff` affiche `git diff` unifié en overlay
- [x] `/diff --stat` résumé compact
- [x] Fil agent : `e` sur tout diff non vide
- [x] M2 — `/diff <fichier>` + navigation status
- [x] M3 — Numéros de ligne + thème diff + file_write
- [x] M4 — Bandeau fin de run + diff permission (`e`)
- [ ] Tests `diff_cmd` + viewer
- [ ] i18n FR/EN

## Code signing

- [ ] Certificat Authenticode (OV ou EV) commandé
- [ ] `sign-release.ps1` + Inno `SignTool`
- [ ] CI signature sur release Windows
- [ ] Clé GPG release + `.asc` Linux
- [ ] README OR section vérification éditeur
- [ ] QA SmartScreen (VM propre)
- [ ] QA `gpg --verify` Linux

## Animation IDE

- [ ] Spec splash alignée TUI (`docs/animation-start/`)
- [ ] Extraction / port dans fork VS Code Drox
- [ ] Preview animation dans IDE

## Release

- [x] Bump version 2.0.4 (workspace)
- [ ] RELEASE_NOTES OR
- [ ] Merge `2.0.4` → `main`
