# Checklist — Ligne 2.0.4

## Diff visuel

- [ ] M1 — `/diff` affiche `git diff` unifié en overlay
- [ ] M2 — `/diff <fichier>` + navigation status
- [ ] M3 — Numéros de ligne + thème diff
- [ ] M4 — Bandeau fin de run agent + lien permission
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

- [ ] Bump version 2.0.4
- [ ] RELEASE_NOTES OR
- [ ] Branche `2.0.4` → merge `main`
